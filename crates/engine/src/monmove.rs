//! Monster movement mechanics: pathfinding, position finding, flee/scare,
//! trap avoidance, and the low-level movement dispatch.
//!
//! Ported from C NetHack's `monmove.c` (~2,395 lines).  This module
//! provides the building blocks that `monster_ai.rs` (high-level AI) uses
//! for movement decisions.  All functions are pure: `GameWorld` + RNG in,
//! events out.  Zero IO.

use bitflags::bitflags;
use hecs::Entity;
use rand::Rng;

use nethack_babel_data::MonsterFlags;

use crate::action::{Direction, Position};
use crate::dungeon::Terrain;
use crate::event::EngineEvent;
use crate::monster_ai::MonsterSpeciesFlags;
use crate::traps::TrapType;
use crate::world::{
    Boulder, GameWorld, HitPoints, Monster, Positioned,
};

// ---------------------------------------------------------------------------
// Movement result
// ---------------------------------------------------------------------------

/// Result of a monster movement attempt, mirroring C's `MMOVE_*` constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveResult {
    /// Monster did not move (trapped, hiding, eating, etc.).
    Nothing,
    /// Monster moved successfully.
    Moved,
    /// Monster died during movement (trap, etc.).
    Died,
    /// Monster completed an action (ate, opened door) but didn't relocate.
    Done,
}

// ---------------------------------------------------------------------------
// Position-finding flags (mfndpos)
// ---------------------------------------------------------------------------

bitflags! {
    /// Flags controlling which positions are acceptable for `mfndpos`.
    /// Mirrors C's `ALLOW_*` / `NOTONL` defines from `mfndpos.h`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MfndPosFlags: u32 {
        /// Allow moving to positions with traps.
        const ALLOW_TRAPS    = 0x0001;
        /// Allow moving through walls (phasing).
        const ALLOW_WALL     = 0x0002;
        /// Allow digging through rock/trees.
        const ALLOW_DIG      = 0x0004;
        /// Allow moving into water/pool tiles.
        const ALLOW_WATER    = 0x0008;
        /// Allow moving onto lava tiles.
        const ALLOW_LAVA     = 0x0010;
        /// Allow attacking other monsters at the target.
        const ALLOW_M        = 0x0020;
        /// Allow attacking the player at the target.
        const ALLOW_U        = 0x0040;
        /// Allow moving onto scare objects (Elbereth, etc.).
        const ALLOW_SCARY    = 0x0080;
        /// Allow busting through doors.
        const BUSTDOOR       = 0x0100;
        /// Do not step on Elbereth or scare scrolls.
        const NOTONL         = 0x0200;
        /// Permit all positions (confused monster).
        const ALLOW_ALL      = 0x0400;
    }
}

// ---------------------------------------------------------------------------
// Monster track (position memory)
// ---------------------------------------------------------------------------

/// ECS component: a short history of positions the monster has visited.
///
/// Used for backtracking when the monster can't see the player.
/// Mirrors C's `mtmp->mtrack[]`.
#[derive(Debug, Clone)]
pub struct MonsterTrack {
    /// Ring buffer of recent positions, newest first.
    pub positions: [Position; TRACK_LEN],
    /// Number of valid entries (0..=TRACK_LEN).
    pub count: usize,
}

const TRACK_LEN: usize = 10;

impl Default for MonsterTrack {
    fn default() -> Self {
        Self {
            positions: [Position::new(0, 0); TRACK_LEN],
            count: 0,
        }
    }
}

impl MonsterTrack {
    /// Push a new position onto the track (most recent first).
    pub fn push(&mut self, pos: Position) {
        // Shift existing entries right.
        for i in (1..TRACK_LEN).rev() {
            self.positions[i] = self.positions[i - 1];
        }
        self.positions[0] = pos;
        self.count = (self.count + 1).min(TRACK_LEN);
    }

    /// Clear the track (e.g. when the monster teleports).
    pub fn clear(&mut self) {
        self.count = 0;
    }

    /// Get the most recent tracked position, if any.
    pub fn last_pos(&self) -> Option<Position> {
        if self.count > 0 {
            Some(self.positions[0])
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Monster goal
// ---------------------------------------------------------------------------

/// ECS component: the monster's current movement goal.
///
/// When a monster has a goal, it pathfinds toward that position.
/// Mirrors C's `mtmp->mgoal` and related strategy fields.
#[derive(Debug, Clone, Copy)]
pub struct MonsterGoal {
    pub target: Position,
    pub strategy: MoveStrategy,
}

/// Movement strategy for a monster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveStrategy {
    /// Move toward the player (default aggressive).
    Pursue,
    /// Move away from the player.
    Flee,
    /// Move toward a specific goal (stairs, item, etc.).
    GoTo,
    /// Wander randomly.
    Wander,
    /// Do nothing special (covetous returning, etc.).
    None,
}

// ---------------------------------------------------------------------------
// mfndpos — find valid adjacent positions
// ---------------------------------------------------------------------------

/// A candidate position from `mfndpos`.
#[derive(Debug, Clone, Copy)]
pub struct MfndPosEntry {
    pub pos: Position,
    pub dir: Direction,
    /// Distance to the monster's goal (for sorting).
    pub distance: i32,
}

/// Find all valid adjacent positions a monster can move to.
///
/// This is the core position-finding function, mirroring C's `mfndpos()`.
/// It examines all 8 neighbors of the monster's current position and
/// returns those that are valid given the monster's capabilities and the
/// provided flags.
pub fn mfndpos(
    world: &GameWorld,
    monster: Entity,
    monster_pos: Position,
    flags: MfndPosFlags,
) -> Vec<MfndPosEntry> {
    let map = &world.dungeon().current_level;
    let species_flags = world
        .get_component::<MonsterSpeciesFlags>(monster)
        .map(|f| f.0)
        .unwrap_or_else(MonsterFlags::empty);

    let mut results = Vec::with_capacity(8);

    for &dir in &Direction::PLANAR {
        let pos = monster_pos.step(dir);

        if !map.in_bounds(pos) {
            continue;
        }

        let cell = match map.get(pos) {
            Some(c) => c,
            None => continue,
        };

        // Check terrain passability.
        if !is_terrain_ok(cell.terrain, species_flags, flags) {
            continue;
        }

        // Check for other monsters at this position.
        if !flags.contains(MfndPosFlags::ALLOW_M) {
            let has_monster = world
                .ecs()
                .query::<(&Monster, &Positioned)>()
                .iter()
                .any(|(e, (_, p))| e != monster && p.0 == pos);
            if has_monster {
                continue;
            }
        }

        // Check for player at this position.
        if !flags.contains(MfndPosFlags::ALLOW_U)
            && let Some(player_pos) = world.get_component::<Positioned>(world.player())
            && player_pos.0 == pos
        {
            continue;
        }

        // Check for boulders.
        let has_boulder = world
            .ecs()
            .query::<(&Boulder, &Positioned)>()
            .iter()
            .any(|(_, (_, bp))| bp.0 == pos);
        if has_boulder && !species_flags.contains(MonsterFlags::TUNNEL) {
            continue;
        }

        // Check for traps (avoid unless ALLOW_TRAPS).
        if !flags.contains(MfndPosFlags::ALLOW_TRAPS)
            && !flags.contains(MfndPosFlags::ALLOW_ALL)
            && is_trap_at(world, pos)
            && !species_flags.contains(MonsterFlags::FLY)
        {
            continue;
        }

        // Check for scare effects (Elbereth, scare scroll).
        if flags.contains(MfndPosFlags::NOTONL) && is_scary_position(world, pos) {
            continue;
        }

        results.push(MfndPosEntry {
            pos,
            dir,
            distance: 0, // Caller fills this in.
        });
    }

    results
}

// ---------------------------------------------------------------------------
// distfleeck — distance, flee, and scare check
// ---------------------------------------------------------------------------

/// Output of `distfleeck`: distance classification and scare state.
#[derive(Debug, Clone, Copy)]
pub struct DistFleeResult {
    /// Monster is within bolt range of the player.
    pub in_range: bool,
    /// Monster is adjacent to the player.
    pub nearby: bool,
    /// Monster is scared (by Elbereth, scare scroll, etc.).
    pub scared: bool,
}

/// Check the monster's distance to the player and whether it's scared.
///
/// Mirrors C's `distfleeck()`.  Sets `in_range` (within `BOLT_LIM^2`),
/// `nearby` (adjacent), and `scared` (on a scary tile).
pub fn distfleeck(
    world: &GameWorld,
    _monster: Entity,
    monster_pos: Position,
    player_pos: Position,
    rng: &mut impl Rng,
) -> DistFleeResult {
    const BOLT_LIM: i32 = 8;

    let dist_sq = dist2(monster_pos, player_pos);
    let in_range = dist_sq <= BOLT_LIM * BOLT_LIM;
    let nearby = in_range && chebyshev_distance(monster_pos, player_pos) <= 1;

    let mut scared = false;
    if nearby {
        // Check if the player's position is scary to this monster.
        if is_scary_position(world, player_pos) {
            scared = true;
        }
        // Gremlins flee from light (simplified: 4/5 chance).
        if !scared && rng.random_range(0..5u32) != 0 {
            // Placeholder: no explicit gremlin flag check yet.
        }
    }

    DistFleeResult {
        in_range,
        nearby,
        scared,
    }
}

// ---------------------------------------------------------------------------
// m_move — core movement dispatch
// ---------------------------------------------------------------------------

/// Execute a single movement step for a monster.
///
/// This is the low-level movement dispatch, mirroring C's `m_move()`.
/// It handles:
/// - Trapped monsters (attempt to escape)
/// - Goal-directed movement (approach or flee)
/// - Position finding via `mfndpos`
/// - Track recording
///
/// Returns the movement result and any generated events.
pub fn m_move(
    world: &mut GameWorld,
    monster: Entity,
    goal: Option<Position>,
    approach: bool,
    rng: &mut impl Rng,
) -> (MoveResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    let monster_pos = match world.get_component::<Positioned>(monster) {
        Some(p) => p.0,
        None => return (MoveResult::Nothing, events),
    };

    // Build mfndpos flags based on monster state.
    let mut flags = MfndPosFlags::empty();

    let species_flags = world
        .get_component::<MonsterSpeciesFlags>(monster)
        .map(|f| f.0)
        .unwrap_or_else(MonsterFlags::empty);

    // Allow attacking the player if approaching.
    if approach {
        flags |= MfndPosFlags::ALLOW_U;
    }

    // Phasing monsters can go through walls.
    if species_flags.contains(MonsterFlags::WALLWALK) {
        flags |= MfndPosFlags::ALLOW_WALL;
    }

    // Flying monsters can cross water/lava.
    if species_flags.contains(MonsterFlags::FLY) {
        flags |= MfndPosFlags::ALLOW_WATER | MfndPosFlags::ALLOW_LAVA;
    }

    // Swimming monsters can cross water.
    if species_flags.contains(MonsterFlags::SWIM) {
        flags |= MfndPosFlags::ALLOW_WATER;
    }

    // Tunneling monsters can dig.
    if species_flags.contains(MonsterFlags::TUNNEL) {
        flags |= MfndPosFlags::ALLOW_DIG;
    }

    // Find valid positions.
    let mut candidates = mfndpos(world, monster, monster_pos, flags);

    if candidates.is_empty() {
        return (MoveResult::Nothing, events);
    }

    // Score each candidate by distance to goal.
    let target = goal.unwrap_or_else(|| {
        world
            .get_component::<Positioned>(world.player())
            .map(|p| p.0)
            .unwrap_or(monster_pos)
    });

    for entry in &mut candidates {
        entry.distance = dist2(entry.pos, target);
    }

    // Sort: approaching = closest first, fleeing = farthest first.
    if approach {
        candidates.sort_by_key(|e| e.distance);
    } else {
        candidates.sort_by_key(|e| std::cmp::Reverse(e.distance));
    }

    // Try each candidate in order.
    for entry in &candidates {
        let to = entry.pos;

        // Check for player at target (triggers combat, handled by AI layer).
        if let Some(player_pos) = world.get_component::<Positioned>(world.player())
            && player_pos.0 == to
        {
            // Signal that we want to attack — the AI layer handles this.
            return (MoveResult::Nothing, events);
        }

        // Check for another monster at target.
        let occupied = world
            .ecs()
            .query::<(&Monster, &Positioned)>()
            .iter()
            .any(|(e, (_, p))| e != monster && p.0 == to);
        if occupied {
            continue;
        }

        // Record track before moving.
        if let Some(mut track) = world.get_component_mut::<MonsterTrack>(monster) {
            track.push(monster_pos);
        }

        // Execute the move.
        if let Some(mut pos) = world.get_component_mut::<Positioned>(monster) {
            pos.0 = to;
        }

        events.push(EngineEvent::EntityMoved {
            entity: monster,
            from: monster_pos,
            to,
        });

        // Check for traps at the new position.
        let trap_events = check_trap_at_pos(world, monster, to, rng);
        events.extend(trap_events);

        return (MoveResult::Moved, events);
    }

    (MoveResult::Nothing, events)
}

// ---------------------------------------------------------------------------
// onscary — scare position check
// ---------------------------------------------------------------------------

/// Check if a position is scary to monsters (Elbereth engraving or
/// scroll of scare monster on the ground).
///
/// Mirrors C's `onscary()`.
pub fn is_scary_position(world: &GameWorld, pos: Position) -> bool {
    // Check for Elbereth engraving.
    if crate::engrave::is_elbereth_at(&world.dungeon().engraving_map, pos) {
        return true;
    }

    // Check for scroll of scare monster on the floor.
    // (Simplified: we'd need to check for SCR_SCARE_MONSTER objects at pos.)
    // For now, only Elbereth is checked.
    false
}

/// Check if a specific monster is immune to scare effects.
///
/// Some monsters ignore Elbereth: unique monsters, humans, shopkeepers,
/// priests, blind monsters (can't see the engraving).
pub fn is_scare_immune(
    species_flags: MonsterFlags,
    is_blind: bool,
    is_covetous: bool,
    is_peaceful: bool,
) -> bool {
    // Blind monsters can't read Elbereth.
    if is_blind {
        return true;
    }
    // Peaceful monsters don't care.
    if is_peaceful {
        return true;
    }
    // Covetous monsters (Wizard, quest nemesis) ignore Elbereth.
    if is_covetous {
        return true;
    }
    // Humans ignore Elbereth.
    if species_flags.contains(MonsterFlags::HUMANOID)
        && !species_flags.contains(MonsterFlags::ANIMAL)
    {
        // Simplified: in C, only S_HUMAN class ignores, not all humanoids.
        // We'll be more conservative.
    }
    false
}

// ---------------------------------------------------------------------------
// Trap awareness
// ---------------------------------------------------------------------------

/// Check if there's a known trap at the given position.
fn is_trap_at(world: &GameWorld, pos: Position) -> bool {
    world.dungeon().trap_map.trap_at(pos).is_some()
}

/// Check for traps when a monster moves onto a position.
/// Returns events for any triggered traps.
fn check_trap_at_pos(
    world: &mut GameWorld,
    monster: Entity,
    pos: Position,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let trap_type = world.dungeon().trap_map.trap_at(pos).map(|t| t.trap_type);
    if let Some(trap_type) = trap_type {
        match trap_type {
            TrapType::Pit | TrapType::SpikedPit => {
                // Flying monsters avoid pits.
                let species_flags = world
                    .get_component::<MonsterSpeciesFlags>(monster)
                    .map(|f| f.0)
                    .unwrap_or_else(MonsterFlags::empty);
                if !species_flags.contains(MonsterFlags::FLY) {
                    let damage = match trap_type {
                        TrapType::SpikedPit => rng.random_range(2..=10i32),
                        _ => rng.random_range(1..=6i32),
                    };
                    if let Some(mut hp) = world.get_component_mut::<HitPoints>(monster) {
                        hp.current -= damage;
                    }
                    events.push(EngineEvent::TrapTriggered {
                        entity: monster,
                        trap_type,
                        position: pos,
                    });
                }
            }
            TrapType::TeleportTrap => {
                // Teleport the monster to a random position.
                let map = &world.dungeon().current_level;
                let mut floor_tiles = Vec::new();
                for y in 0..map.height {
                    for x in 0..map.width {
                        let p = Position::new(x as i32, y as i32);
                        if let Some(cell) = map.get(p)
                            && cell.terrain.is_walkable()
                            && p != pos
                        {
                            floor_tiles.push(p);
                        }
                    }
                }
                if !floor_tiles.is_empty() {
                    let dest = floor_tiles[rng.random_range(0..floor_tiles.len())];
                    if let Some(mut mpos) = world.get_component_mut::<Positioned>(monster) {
                        mpos.0 = dest;
                    }
                    // Clear track on teleport.
                    if let Some(mut track) = world.get_component_mut::<MonsterTrack>(monster) {
                        track.clear();
                    }
                    events.push(EngineEvent::TrapTriggered {
                        entity: monster,
                        trap_type,
                        position: pos,
                    });
                    events.push(EngineEvent::EntityTeleported {
                        entity: monster,
                        from: pos,
                        to: dest,
                    });
                }
            }
            TrapType::BearTrap => {
                // Non-flying monsters get trapped.
                let species_flags = world
                    .get_component::<MonsterSpeciesFlags>(monster)
                    .map(|f| f.0)
                    .unwrap_or_else(MonsterFlags::empty);
                if !species_flags.contains(MonsterFlags::FLY) {
                    events.push(EngineEvent::TrapTriggered {
                        entity: monster,
                        trap_type,
                        position: pos,
                    });
                    // Monster is now trapped — could add a Trapped component.
                }
            }
            _ => {
                // Other trap types: simplified handling.
                events.push(EngineEvent::TrapTriggered {
                    entity: monster,
                    trap_type,
                    position: pos,
                });
            }
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Door interaction for monsters
// ---------------------------------------------------------------------------

/// Attempt to open or break a door at `door_pos` for a monster.
///
/// Returns events describing the door interaction, or empty if the monster
/// can't interact with the door.
pub fn monster_door_interaction(
    world: &mut GameWorld,
    monster: Entity,
    door_pos: Position,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let species_flags = world
        .get_component::<MonsterSpeciesFlags>(monster)
        .map(|f| f.0)
        .unwrap_or_else(MonsterFlags::empty);

    let terrain = match world.dungeon().current_level.get(door_pos) {
        Some(cell) => cell.terrain,
        None => return events,
    };

    let can_open = !species_flags.contains(MonsterFlags::NOHANDS)
        && species_flags.contains(MonsterFlags::HUMANOID);
    let can_bust = species_flags.contains(MonsterFlags::GIANT);

    match terrain {
        Terrain::DoorClosed => {
            if can_open {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(door_pos, Terrain::DoorOpen);
                events.push(EngineEvent::DoorOpened { position: door_pos });
            } else if can_bust {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(door_pos, Terrain::DoorOpen);
                events.push(EngineEvent::DoorBroken { position: door_pos });
            }
            // Amorphous monsters can flow under doors (handled in terrain check).
        }
        Terrain::DoorLocked if can_bust => {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(door_pos, Terrain::DoorOpen);
            events.push(EngineEvent::DoorBroken { position: door_pos });
        }
        _ => {}
    }

    events
}

// ---------------------------------------------------------------------------
// Approach/retreat calculation
// ---------------------------------------------------------------------------

/// Determine the approach value for a monster.
///
/// Returns a value indicating how eagerly the monster should approach:
/// - Positive: approach (pursue)
/// - Zero: hold position
/// - Negative: retreat (flee)
///
/// Mirrors C's `appr` calculation in `m_move()`.
pub fn calc_approach(
    world: &GameWorld,
    monster: Entity,
    monster_pos: Position,
    player_pos: Position,
    is_scared: bool,
) -> i32 {
    if is_scared {
        return -1;
    }

    // Check HP-based flee.
    let (current_hp, max_hp) = match world.get_component::<HitPoints>(monster) {
        Some(hp) => (hp.current, hp.max),
        None => (1, 1),
    };

    // Low HP: consider fleeing.
    if max_hp >= 3 && current_hp < max_hp / 3 {
        return -1;
    }

    // Default: approach if hostile.
    let dist = chebyshev_distance(monster_pos, player_pos);
    if dist > 1 {
        1 // Approach
    } else {
        0 // Adjacent: melee (handled by AI)
    }
}

// ---------------------------------------------------------------------------
// Movement with goal (pathfinding)
// ---------------------------------------------------------------------------

/// Move a monster one step toward a goal, using `mfndpos` for position
/// validation and distance-based scoring.
///
/// This is the main entry point for goal-directed movement, equivalent
/// to the pathfinding portion of C's `m_move()`.
pub fn move_toward_goal(
    world: &mut GameWorld,
    monster: Entity,
    goal: Position,
    rng: &mut impl Rng,
) -> (MoveResult, Vec<EngineEvent>) {
    m_move(world, monster, Some(goal), true, rng)
}

/// Move a monster one step away from a threat.
pub fn move_away_from(
    world: &mut GameWorld,
    monster: Entity,
    threat: Position,
    rng: &mut impl Rng,
) -> (MoveResult, Vec<EngineEvent>) {
    m_move(world, monster, Some(threat), false, rng)
}

/// Move a monster in a random direction (wander).
pub fn wander_move(
    world: &mut GameWorld,
    monster: Entity,
    rng: &mut impl Rng,
) -> (MoveResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    let monster_pos = match world.get_component::<Positioned>(monster) {
        Some(p) => p.0,
        None => return (MoveResult::Nothing, events),
    };

    let flags = MfndPosFlags::empty();
    let mut candidates = mfndpos(world, monster, monster_pos, flags);

    if candidates.is_empty() {
        return (MoveResult::Nothing, events);
    }

    // Shuffle candidates for randomness.
    for i in (1..candidates.len()).rev() {
        let j = rng.random_range(0..=i);
        candidates.swap(i, j);
    }

    // Try the first valid candidate.
    for entry in &candidates {
        let to = entry.pos;

        // Avoid player position (combat handled elsewhere).
        if let Some(player_pos) = world.get_component::<Positioned>(world.player())
            && player_pos.0 == to
        {
            continue;
        }

        // Record track.
        if let Some(mut track) = world.get_component_mut::<MonsterTrack>(monster) {
            track.push(monster_pos);
        }

        // Execute move.
        if let Some(mut pos) = world.get_component_mut::<Positioned>(monster) {
            pos.0 = to;
        }

        events.push(EngineEvent::EntityMoved {
            entity: monster,
            from: monster_pos,
            to,
        });

        let trap_events = check_trap_at_pos(world, monster, to, rng);
        events.extend(trap_events);

        return (MoveResult::Moved, events);
    }

    (MoveResult::Nothing, events)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Squared Euclidean distance between two positions.
/// Matches C's `dist2()`.
#[inline]
fn dist2(a: Position, b: Position) -> i32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

/// Chebyshev (king-move) distance between two positions.
#[inline]
fn chebyshev_distance(a: Position, b: Position) -> i32 {
    let dx = (a.x - b.x).abs();
    let dy = (a.y - b.y).abs();
    dx.max(dy)
}

/// Check if terrain is passable for a monster with given flags and mfndpos flags.
fn is_terrain_ok(terrain: Terrain, species: MonsterFlags, flags: MfndPosFlags) -> bool {
    // Always passable: standard walkable terrain.
    if terrain.is_walkable() {
        return true;
    }

    // Allow-all (confused) permits everything.
    if flags.contains(MfndPosFlags::ALLOW_ALL) {
        return true;
    }

    match terrain {
        Terrain::Wall | Terrain::Stone => {
            flags.contains(MfndPosFlags::ALLOW_WALL)
                || species.contains(MonsterFlags::WALLWALK)
                || (flags.contains(MfndPosFlags::ALLOW_DIG)
                    && species.contains(MonsterFlags::TUNNEL))
        }
        Terrain::Pool | Terrain::Moat | Terrain::Water => {
            flags.contains(MfndPosFlags::ALLOW_WATER)
                || species.contains(MonsterFlags::FLY)
                || species.contains(MonsterFlags::SWIM)
                || species.contains(MonsterFlags::AMORPHOUS)
        }
        Terrain::Lava => {
            flags.contains(MfndPosFlags::ALLOW_LAVA)
                || species.contains(MonsterFlags::FLY)
        }
        Terrain::DoorClosed | Terrain::DoorLocked => {
            flags.contains(MfndPosFlags::BUSTDOOR)
                || species.contains(MonsterFlags::AMORPHOUS)
        }
        Terrain::IronBars => species.contains(MonsterFlags::AMORPHOUS),
        Terrain::Tree => {
            species.contains(MonsterFlags::FLY)
                || (flags.contains(MfndPosFlags::ALLOW_DIG)
                    && species.contains(MonsterFlags::TUNNEL))
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dungeon::Terrain;
    use crate::world::Speed;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    fn make_test_world() -> GameWorld {
        let mut world = GameWorld::new(Position::new(5, 5));
        let map = &mut world.dungeon_mut().current_level;
        for y in 0..21 {
            for x in 0..80 {
                map.set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        world
    }

    fn spawn_test_monster(world: &mut GameWorld, pos: Position) -> Entity {
        let order = world.next_creation_order();
        world.spawn((
            Monster,
            Positioned(pos),
            HitPoints { current: 10, max: 10 },
            Speed(12),
            order,
        ))
    }

    fn spawn_monster_with_flags(
        world: &mut GameWorld,
        pos: Position,
        flags: MonsterFlags,
    ) -> Entity {
        let order = world.next_creation_order();
        let entity = world.spawn((
            Monster,
            Positioned(pos),
            HitPoints { current: 10, max: 10 },
            Speed(12),
            order,
        ));
        let _ = world.ecs_mut().insert_one(entity, MonsterSpeciesFlags(flags));
        entity
    }

    // ── MonsterTrack tests ───────────────────────────────────────────

    #[test]
    fn track_push_and_retrieve() {
        let mut track = MonsterTrack::default();
        assert_eq!(track.count, 0);
        assert!(track.last_pos().is_none());

        track.push(Position::new(1, 1));
        assert_eq!(track.count, 1);
        assert_eq!(track.last_pos(), Some(Position::new(1, 1)));

        track.push(Position::new(2, 2));
        assert_eq!(track.count, 2);
        assert_eq!(track.last_pos(), Some(Position::new(2, 2)));
    }

    #[test]
    fn track_overflow_wraps() {
        let mut track = MonsterTrack::default();
        for i in 0..20 {
            track.push(Position::new(i, 0));
        }
        assert_eq!(track.count, TRACK_LEN);
        assert_eq!(track.last_pos(), Some(Position::new(19, 0)));
    }

    #[test]
    fn track_clear() {
        let mut track = MonsterTrack::default();
        track.push(Position::new(1, 1));
        track.clear();
        assert_eq!(track.count, 0);
        assert!(track.last_pos().is_none());
    }

    // ── mfndpos tests ────────────────────────────────────────────────

    #[test]
    fn mfndpos_finds_all_8_neighbors_on_open_floor() {
        let mut world = make_test_world();
        let monster = spawn_test_monster(&mut world, Position::new(10, 10));

        let results = mfndpos(&world, monster, Position::new(10, 10), MfndPosFlags::empty());

        // All 8 neighbors should be valid (open floor, no other entities nearby).
        // But player is at (5,5), not adjacent, so no conflict.
        assert_eq!(results.len(), 8);
    }

    #[test]
    fn mfndpos_excludes_walls() {
        let mut world = make_test_world();
        // Place walls on 3 sides.
        world.dungeon_mut().current_level.set_terrain(Position::new(9, 10), Terrain::Wall);
        world.dungeon_mut().current_level.set_terrain(Position::new(11, 10), Terrain::Wall);
        world.dungeon_mut().current_level.set_terrain(Position::new(10, 9), Terrain::Wall);

        let monster = spawn_test_monster(&mut world, Position::new(10, 10));
        let results = mfndpos(&world, monster, Position::new(10, 10), MfndPosFlags::empty());

        // 3 cardinal walls removed, plus 2 diagonals adjacent to walls
        // Actually walls at (9,10), (11,10), (10,9) block those 3 cardinals.
        // Diagonals (9,9), (11,9) also touch walls but terrain there is floor.
        // So we should get 5 valid positions.
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn mfndpos_excludes_other_monsters() {
        let mut world = make_test_world();
        let monster = spawn_test_monster(&mut world, Position::new(10, 10));
        let _blocker = spawn_test_monster(&mut world, Position::new(11, 10));

        let results = mfndpos(&world, monster, Position::new(10, 10), MfndPosFlags::empty());

        // One neighbor blocked by another monster.
        assert_eq!(results.len(), 7);
    }

    #[test]
    fn mfndpos_allows_monsters_with_flag() {
        let mut world = make_test_world();
        let monster = spawn_test_monster(&mut world, Position::new(10, 10));
        let _blocker = spawn_test_monster(&mut world, Position::new(11, 10));

        let results = mfndpos(
            &world,
            monster,
            Position::new(10, 10),
            MfndPosFlags::ALLOW_M,
        );

        assert_eq!(results.len(), 8);
    }

    #[test]
    fn mfndpos_excludes_player_position() {
        let mut world = make_test_world();
        // Player is at (5, 5). Place monster adjacent.
        let monster = spawn_test_monster(&mut world, Position::new(5, 6));

        let results = mfndpos(&world, monster, Position::new(5, 6), MfndPosFlags::empty());

        // Player at (5,5) blocks that position.
        let has_player_pos = results.iter().any(|e| e.pos == Position::new(5, 5));
        assert!(!has_player_pos);
    }

    #[test]
    fn mfndpos_allows_player_with_flag() {
        let mut world = make_test_world();
        let monster = spawn_test_monster(&mut world, Position::new(5, 6));

        let results = mfndpos(
            &world,
            monster,
            Position::new(5, 6),
            MfndPosFlags::ALLOW_U,
        );

        let has_player_pos = results.iter().any(|e| e.pos == Position::new(5, 5));
        assert!(has_player_pos);
    }

    #[test]
    fn mfndpos_phasing_through_walls() {
        let mut world = make_test_world();
        world.dungeon_mut().current_level.set_terrain(Position::new(11, 10), Terrain::Wall);

        let monster = spawn_monster_with_flags(
            &mut world,
            Position::new(10, 10),
            MonsterFlags::WALLWALK,
        );

        let results = mfndpos(&world, monster, Position::new(10, 10), MfndPosFlags::empty());

        // Wall at (11,10) should be accessible for wall-walking monster.
        let has_wall_pos = results.iter().any(|e| e.pos == Position::new(11, 10));
        assert!(has_wall_pos);
    }

    // ── distfleeck tests ─────────────────────────────────────────────

    #[test]
    fn distfleeck_nearby_when_adjacent() {
        let world = make_test_world();
        let mut rng = test_rng();

        let monster_pos = Position::new(5, 6);
        let player_pos = Position::new(5, 5);

        let result = distfleeck(
            &world,
            world.player(), // entity doesn't matter for distance calc
            monster_pos,
            player_pos,
            &mut rng,
        );

        assert!(result.in_range);
        assert!(result.nearby);
    }

    #[test]
    fn distfleeck_not_nearby_when_far() {
        let world = make_test_world();
        let mut rng = test_rng();

        let result = distfleeck(
            &world,
            world.player(),
            Position::new(40, 10),
            Position::new(5, 5),
            &mut rng,
        );

        assert!(!result.in_range);
        assert!(!result.nearby);
    }

    // ── m_move tests ─────────────────────────────────────────────────

    #[test]
    fn m_move_toward_goal() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_test_monster(&mut world, Position::new(10, 10));
        let goal = Position::new(15, 10);

        let (result, events) = m_move(&mut world, monster, Some(goal), true, &mut rng);

        assert_eq!(result, MoveResult::Moved);
        assert!(!events.is_empty());

        // Monster should have moved closer to goal.
        let new_pos = world.get_component::<Positioned>(monster).unwrap().0;
        let old_dist = dist2(Position::new(10, 10), goal);
        let new_dist = dist2(new_pos, goal);
        assert!(new_dist < old_dist);
    }

    #[test]
    fn m_move_away_from_threat() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_test_monster(&mut world, Position::new(10, 10));
        let threat = Position::new(10, 9);

        let (result, events) = m_move(&mut world, monster, Some(threat), false, &mut rng);

        assert_eq!(result, MoveResult::Moved);
        assert!(!events.is_empty());

        // Monster should have moved away from threat.
        let new_pos = world.get_component::<Positioned>(monster).unwrap().0;
        let old_dist = dist2(Position::new(10, 10), threat);
        let new_dist = dist2(new_pos, threat);
        assert!(new_dist > old_dist);
    }

    #[test]
    fn m_move_nothing_when_boxed_in() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Surround monster with walls.
        let center = Position::new(10, 10);
        for &dir in &Direction::PLANAR {
            let wall_pos = center.step(dir);
            world.dungeon_mut().current_level.set_terrain(wall_pos, Terrain::Wall);
        }

        let monster = spawn_test_monster(&mut world, center);
        let (result, _events) = m_move(&mut world, monster, Some(Position::new(15, 10)), true, &mut rng);

        assert_eq!(result, MoveResult::Nothing);
    }

    #[test]
    fn wander_move_goes_somewhere() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let monster = spawn_test_monster(&mut world, Position::new(10, 10));

        let (result, events) = wander_move(&mut world, monster, &mut rng);

        assert_eq!(result, MoveResult::Moved);
        assert!(!events.is_empty());

        let new_pos = world.get_component::<Positioned>(monster).unwrap().0;
        assert_ne!(new_pos, Position::new(10, 10));
    }

    // ── calc_approach tests ──────────────────────────────────────────

    #[test]
    fn calc_approach_positive_when_far() {
        let mut world = make_test_world();
        let monster = spawn_test_monster(&mut world, Position::new(20, 10));

        let appr = calc_approach(
            &world,
            monster,
            Position::new(20, 10),
            Position::new(5, 5),
            false,
        );

        assert!(appr > 0);
    }

    #[test]
    fn calc_approach_negative_when_scared() {
        let mut world = make_test_world();
        let monster = spawn_test_monster(&mut world, Position::new(10, 10));

        let appr = calc_approach(
            &world,
            monster,
            Position::new(10, 10),
            Position::new(5, 5),
            true, // scared
        );

        assert!(appr < 0);
    }

    #[test]
    fn calc_approach_negative_when_low_hp() {
        let mut world = make_test_world();
        let monster = spawn_test_monster(&mut world, Position::new(10, 10));

        // Set HP to critical level.
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(monster) {
            hp.current = 1;
            hp.max = 10;
        }

        let appr = calc_approach(
            &world,
            monster,
            Position::new(10, 10),
            Position::new(5, 5),
            false,
        );

        assert!(appr < 0);
    }

    // ── Door interaction tests ───────────────────────────────────────

    #[test]
    fn monster_opens_closed_door() {
        let mut world = make_test_world();
        let door_pos = Position::new(10, 10);
        world.dungeon_mut().current_level.set_terrain(door_pos, Terrain::DoorClosed);

        let monster = spawn_monster_with_flags(
            &mut world,
            Position::new(9, 10),
            MonsterFlags::HUMANOID,
        );

        let events = monster_door_interaction(&mut world, monster, door_pos);

        assert!(!events.is_empty());
        assert!(matches!(events[0], EngineEvent::DoorOpened { .. }));

        let terrain = world.dungeon().current_level.get(door_pos).unwrap().terrain;
        assert_eq!(terrain, Terrain::DoorOpen);
    }

    #[test]
    fn animal_cannot_open_door() {
        let mut world = make_test_world();
        let door_pos = Position::new(10, 10);
        world.dungeon_mut().current_level.set_terrain(door_pos, Terrain::DoorClosed);

        // No HUMANOID flag, no GIANT flag.
        let monster = spawn_monster_with_flags(
            &mut world,
            Position::new(9, 10),
            MonsterFlags::ANIMAL,
        );

        let events = monster_door_interaction(&mut world, monster, door_pos);

        assert!(events.is_empty());
        let terrain = world.dungeon().current_level.get(door_pos).unwrap().terrain;
        assert_eq!(terrain, Terrain::DoorClosed);
    }

    #[test]
    fn giant_breaks_locked_door() {
        let mut world = make_test_world();
        let door_pos = Position::new(10, 10);
        world.dungeon_mut().current_level.set_terrain(door_pos, Terrain::DoorLocked);

        let monster = spawn_monster_with_flags(
            &mut world,
            Position::new(9, 10),
            MonsterFlags::GIANT,
        );

        let events = monster_door_interaction(&mut world, monster, door_pos);

        assert!(!events.is_empty());
        assert!(matches!(events[0], EngineEvent::DoorBroken { .. }));

        let terrain = world.dungeon().current_level.get(door_pos).unwrap().terrain;
        assert_eq!(terrain, Terrain::DoorOpen);
    }

    // ── Terrain passability tests ────────────────────────────────────

    #[test]
    fn terrain_ok_floor_always() {
        assert!(is_terrain_ok(
            Terrain::Floor,
            MonsterFlags::empty(),
            MfndPosFlags::empty(),
        ));
    }

    #[test]
    fn terrain_ok_wall_needs_wallwalk() {
        assert!(!is_terrain_ok(
            Terrain::Wall,
            MonsterFlags::empty(),
            MfndPosFlags::empty(),
        ));
        assert!(is_terrain_ok(
            Terrain::Wall,
            MonsterFlags::WALLWALK,
            MfndPosFlags::empty(),
        ));
    }

    #[test]
    fn terrain_ok_water_for_swimmers() {
        assert!(!is_terrain_ok(
            Terrain::Pool,
            MonsterFlags::empty(),
            MfndPosFlags::empty(),
        ));
        assert!(is_terrain_ok(
            Terrain::Pool,
            MonsterFlags::SWIM,
            MfndPosFlags::empty(),
        ));
        assert!(is_terrain_ok(
            Terrain::Pool,
            MonsterFlags::FLY,
            MfndPosFlags::empty(),
        ));
    }

    #[test]
    fn terrain_ok_lava_for_flyers() {
        assert!(!is_terrain_ok(
            Terrain::Lava,
            MonsterFlags::empty(),
            MfndPosFlags::empty(),
        ));
        assert!(is_terrain_ok(
            Terrain::Lava,
            MonsterFlags::FLY,
            MfndPosFlags::empty(),
        ));
        assert!(!is_terrain_ok(
            Terrain::Lava,
            MonsterFlags::SWIM,
            MfndPosFlags::empty(),
        ));
    }

    #[test]
    fn terrain_ok_closed_door_for_amorphous() {
        assert!(!is_terrain_ok(
            Terrain::DoorClosed,
            MonsterFlags::empty(),
            MfndPosFlags::empty(),
        ));
        assert!(is_terrain_ok(
            Terrain::DoorClosed,
            MonsterFlags::AMORPHOUS,
            MfndPosFlags::empty(),
        ));
    }

    #[test]
    fn terrain_ok_allow_all_permits_everything() {
        assert!(is_terrain_ok(
            Terrain::Wall,
            MonsterFlags::empty(),
            MfndPosFlags::ALLOW_ALL,
        ));
        assert!(is_terrain_ok(
            Terrain::Lava,
            MonsterFlags::empty(),
            MfndPosFlags::ALLOW_ALL,
        ));
    }
}
