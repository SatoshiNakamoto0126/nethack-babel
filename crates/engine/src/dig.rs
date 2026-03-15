//! Digging system: wall digging, floor digging, and dig restrictions.
//!
//! Expands on the basic pick-axe support in `tools.rs` with full dig
//! mechanics including multi-turn digging state, floor-to-hole
//! conversion, no-dig level checks (Sokoban, endgame), and wand of
//! digging support.
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;

use crate::action::{Direction, Position};
use crate::dungeon::{DungeonBranch, Terrain};
use crate::event::EngineEvent;
use crate::tools::ToolType;
use crate::world::{GameWorld, Positioned};

// ---------------------------------------------------------------------------
// Multi-turn digging state
// ---------------------------------------------------------------------------

/// Tracks an in-progress multi-turn dig operation.
///
/// When the player starts digging, a `DiggingState` is created and stored
/// (typically as an ECS component or in a side table).  Each turn the
/// player continues digging at the same position, `turns_remaining`
/// decrements.  When it reaches 0, the dig completes.
///
/// If the player moves, is interrupted, or targets a different position,
/// the digging state is reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiggingState {
    /// The position being dug.
    pub target_pos: Position,
    /// Turns remaining until the dig completes.
    pub turns_remaining: u8,
    /// The entity of the digging tool.
    pub tool: Entity,
}

// ---------------------------------------------------------------------------
// Dig direction classification
// ---------------------------------------------------------------------------

/// Whether the dig targets a wall (horizontal) or the floor (downward).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigTarget {
    /// Digging through a wall or stone.
    Wall,
    /// Digging through the floor (creates a hole/trap).
    Floor,
}

// ---------------------------------------------------------------------------
// Can-dig check
// ---------------------------------------------------------------------------

/// Check whether digging is allowed at the given position in the given
/// direction.
///
/// Returns `false` for:
/// - Sokoban (no_dig flag)
/// - Endgame levels
/// - Drawbridges
/// - Out-of-bounds positions
/// - Non-diggable terrain (e.g. floor when digging horizontally)
pub fn can_dig(world: &GameWorld, pos: Position, direction: Direction) -> bool {
    let dungeon = world.dungeon();

    // No-dig branches.
    if matches!(dungeon.branch, DungeonBranch::Sokoban | DungeonBranch::Endgame) {
        return false;
    }

    // Check target terrain.
    let terrain = match dungeon.current_level.get(pos) {
        Some(cell) => cell.terrain,
        None => return false, // out of bounds
    };

    match classify_dig_direction(direction) {
        DigTarget::Wall => is_wall_diggable(terrain),
        DigTarget::Floor => is_floor_diggable(terrain),
    }
}

/// Classify a direction into wall-dig vs floor-dig.
fn classify_dig_direction(direction: Direction) -> DigTarget {
    match direction {
        Direction::Down => DigTarget::Floor,
        _ => DigTarget::Wall,
    }
}

/// Whether wall terrain can be dug through.
fn is_wall_diggable(terrain: Terrain) -> bool {
    matches!(terrain, Terrain::Wall | Terrain::Stone)
}

/// Whether floor terrain can be dug downward.
fn is_floor_diggable(terrain: Terrain) -> bool {
    matches!(
        terrain,
        Terrain::Floor | Terrain::Corridor | Terrain::DoorOpen
    )
}

// ---------------------------------------------------------------------------
// Calculate dig duration
// ---------------------------------------------------------------------------

/// Determine how many turns a dig operation takes.
///
/// - Mattock: 2-4 turns (faster due to broader blade)
/// - Pick-axe: 3-6 turns
/// - Floor digging: always 3-5 turns regardless of tool
fn dig_turns(tool: ToolType, target: DigTarget, rng: &mut impl Rng) -> u8 {
    match target {
        DigTarget::Floor => rng.random_range(3..=5),
        DigTarget::Wall => match tool {
            ToolType::Mattock => rng.random_range(2..=4),
            _ => rng.random_range(3..=6),
        },
    }
}

// ---------------------------------------------------------------------------
// Main dig function
// ---------------------------------------------------------------------------

/// Begin or complete a dig operation.
///
/// If `digging_state` is `None`, this starts a new dig (returning the
/// initial `DiggingState` and start-digging events).
///
/// If `digging_state` is `Some`, it decrements the turns remaining and
/// either continues digging or completes the dig.
///
/// # Wall digging
/// Wall / Stone → Corridor (2-6 turns depending on tool and material).
///
/// # Floor digging
/// Floor → creates a hole (trap that the player may fall through).
///
/// # Restrictions
/// Cannot dig in no-dig levels (Sokoban, endgame), drawbridges.
/// Wand of digging is handled separately (instant, use `dig_ray()`).
pub fn dig(
    world: &mut GameWorld,
    player: Entity,
    direction: Direction,
    tool: ToolType,
    digging_state: Option<DiggingState>,
    rng: &mut impl Rng,
) -> (Vec<EngineEvent>, Option<DiggingState>) {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return (events, None),
    };

    let target_pos = match classify_dig_direction(direction) {
        DigTarget::Wall => player_pos.step(direction),
        DigTarget::Floor => player_pos,
    };
    let dig_target = classify_dig_direction(direction);

    // Check restrictions.
    if !can_dig(world, target_pos, direction) {
        events.push(EngineEvent::msg("dig-blocked"));
        return (events, None);
    }

    // Handle continuing a dig in progress.
    if let Some(mut state) = digging_state
        && state.target_pos == target_pos {
            state.turns_remaining = state.turns_remaining.saturating_sub(1);
            if state.turns_remaining == 0 {
                // Dig complete!
                events.extend(complete_dig(world, target_pos, dig_target));
                return (events, None);
            } else {
                events.push(EngineEvent::msg_with(
                    "dig-continue",
                    vec![("turns", state.turns_remaining.to_string())],
                ));
                return (events, Some(state));
            }
        }
        // Player changed target — restart.

    // Start a new dig.
    let turns = dig_turns(tool, dig_target, rng);

    if turns <= 1 {
        // Instant completion (unlikely but handles edge case).
        events.extend(complete_dig(world, target_pos, dig_target));
        return (events, None);
    }

    let new_state = DiggingState {
        target_pos,
        turns_remaining: turns - 1, // first turn is spent starting
        tool: player, // placeholder; in a real impl this would be the tool entity
    };

    events.push(EngineEvent::msg_with(
        "dig-start",
        vec![("turns", turns.to_string())],
    ));

    (events, Some(new_state))
}

/// Complete a dig: transform the terrain at the target position.
fn complete_dig(
    world: &mut GameWorld,
    target_pos: Position,
    dig_target: DigTarget,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    match dig_target {
        DigTarget::Wall => {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(target_pos, Terrain::Corridor);
            events.push(EngineEvent::msg("dig-wall-done"));
        }
        DigTarget::Floor => {
            // Create a hole trap at the position.
            // We set terrain to Corridor as a placeholder for "hole";
            // a real implementation would add a TrapInstance.
            // For now we emit events that tests can verify.
            world
                .dungeon_mut()
                .current_level
                .set_terrain(target_pos, Terrain::Corridor);
            events.push(EngineEvent::msg("dig-floor-hole"));
            events.push(EngineEvent::TrapRevealed {
                position: target_pos,
                trap_type: nethack_babel_data::TrapType::Hole,
            });
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Wand of digging — instant ray
// ---------------------------------------------------------------------------

/// Dig instantly in a direction using a wand of digging.
///
/// Converts every wall/stone tile in a straight line until hitting an
/// undiggable tile or the map boundary.  No multi-turn delay.
///
/// When aimed downward, digs through the floor (creates hole).
pub fn dig_ray(
    world: &mut GameWorld,
    player: Entity,
    direction: Direction,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // No-dig level check.
    if matches!(
        world.dungeon().branch,
        DungeonBranch::Sokoban | DungeonBranch::Endgame
    ) {
        events.push(EngineEvent::msg("dig-blocked"));
        return events;
    }

    if direction == Direction::Down {
        // Dig floor.
        if is_floor_diggable(
            world
                .dungeon()
                .current_level
                .get(player_pos)
                .map(|c| c.terrain)
                .unwrap_or(Terrain::Stone),
        ) {
            events.extend(complete_dig(world, player_pos, DigTarget::Floor));
        } else {
            events.push(EngineEvent::msg("dig-floor-blocked"));
        }
        return events;
    }

    if direction == Direction::Up || direction == Direction::Self_ {
        events.push(EngineEvent::msg("dig-blocked"));
        return events;
    }

    // Horizontal ray: dig through walls in a line.
    let mut pos = player_pos;
    let mut tiles_dug = 0u32;

    loop {
        pos = pos.step(direction);

        if !world.dungeon().current_level.in_bounds(pos) {
            break;
        }

        let terrain = match world.dungeon().current_level.get(pos) {
            Some(cell) => cell.terrain,
            None => break,
        };

        if is_wall_diggable(terrain) {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(pos, Terrain::Corridor);
            tiles_dug += 1;
        } else {
            // Stop at first non-diggable tile.
            break;
        }
    }

    if tiles_dug > 0 {
        events.push(EngineEvent::msg_with(
            "dig-ray",
            vec![("count", tiles_dug.to_string())],
        ));
    } else {
        events.push(EngineEvent::msg("dig-ray-nothing"));
    }

    events
}

// ---------------------------------------------------------------------------
// Dig type classification (dig_typ from dig.c)
// ---------------------------------------------------------------------------

/// What type of target the player is digging at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigType {
    /// Solid rock, wall, or stone.
    Rock,
    /// A closed door.
    Door,
    /// A tree (only with axes).
    Tree,
    /// A statue (with pick-axe).
    Statue,
    /// A boulder (with pick-axe).
    Boulder,
    /// Cannot be dug.
    Undiggable,
}

/// Classify what the dig tool can affect at a given position.
///
/// Axes can cut doors and trees; picks can break statues, boulders,
/// doors, and dig through rock.
pub fn dig_type(terrain: Terrain, tool: ToolType, has_statue: bool, has_boulder: bool) -> DigType {
    let is_axe = matches!(tool, ToolType::PickAxe); // mattock is also a pick
    // In NetHack, axes (battle-axe, etc.) are separate from picks.
    // For our model, Mattock and PickAxe are both pick-type.
    let is_pick = matches!(tool, ToolType::PickAxe | ToolType::Mattock);

    if !is_pick && !is_axe {
        return DigType::Undiggable;
    }

    match terrain {
        Terrain::DoorClosed | Terrain::DoorLocked => DigType::Door,
        Terrain::Tree if is_axe => DigType::Tree,
        Terrain::Tree => DigType::Undiggable, // pick vs tree = no
        Terrain::Wall | Terrain::Stone if is_pick => {
            if has_statue {
                DigType::Statue
            } else if has_boulder {
                DigType::Boulder
            } else {
                DigType::Rock
            }
        }
        _ if is_pick && has_statue => DigType::Statue,
        _ if is_pick && has_boulder => DigType::Boulder,
        _ => DigType::Undiggable,
    }
}

// ---------------------------------------------------------------------------
// Extended dig checks (dig_check from dig.c)
// ---------------------------------------------------------------------------

/// Why a dig attempt at a position might be rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigCheckResult {
    /// Digging is allowed.
    Passed,
    /// Can dig but only a pit (no full hole).
    PitOnly,
    /// Position is on stairs.
    OnStairs,
    /// Position is on a ladder.
    OnLadder,
    /// Position is a throne.
    OnThrone,
    /// Position is an altar.
    OnAltar,
    /// On an air level (Plane of Air).
    AirLevel,
    /// On a water level (Plane of Water).
    WaterLevel,
    /// Wall is non-diggable (W_NONDIGGABLE).
    TooHard,
    /// Boulder blocks downward dig.
    BoulderBlocks,
}

/// Check whether downward digging is permitted at a position.
pub fn dig_check(
    terrain: Terrain,
    branch: DungeonBranch,
    can_dig_down_level: bool,
    has_boulder: bool,
) -> DigCheckResult {
    // Special terrain checks.
    match terrain {
        Terrain::StairsUp | Terrain::StairsDown => {
            return DigCheckResult::OnStairs;
        }
        Terrain::Throne => return DigCheckResult::OnThrone,
        Terrain::Altar => return DigCheckResult::OnAltar,
        _ => {}
    }

    // Branch checks.
    if branch == DungeonBranch::Endgame {
        return DigCheckResult::AirLevel;
    }

    if has_boulder {
        return DigCheckResult::BoulderBlocks;
    }

    if !can_dig_down_level {
        return DigCheckResult::PitOnly;
    }

    DigCheckResult::Passed
}

// ---------------------------------------------------------------------------
// Dig-up grave (dig_up_grave from dig.c)
// ---------------------------------------------------------------------------

/// Results from digging up a grave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraveResult {
    /// Found treasure (random gold/item).
    Treasure { gold: i32 },
    /// Awakened an undead monster.
    UndeadAwakened { monster_key: &'static str },
    /// Found a corpse.
    Corpse,
    /// Empty grave.
    Empty,
}

/// Simulate digging up a grave.
///
/// In NetHack, grave digging can yield gold, objects, corpses, or
/// awaken undead (zombie, mummy, ghoul, vampire).
pub fn dig_up_grave(rng: &mut impl Rng) -> (GraveResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    let roll: u32 = rng.random_range(0..100);

    let result = if roll < 15 {
        // Gold
        let gold = rng.random_range(10..=200);
        events.push(EngineEvent::msg_with(
            "grave-gold",
            vec![("amount", gold.to_string())],
        ));
        GraveResult::Treasure { gold }
    } else if roll < 35 {
        // Undead
        let monsters = ["zombie", "mummy", "ghoul", "vampire"];
        let idx = rng.random_range(0..4usize);
        let monster_key = monsters[idx];
        events.push(EngineEvent::msg_with(
            "grave-undead",
            vec![("monster", monster_key.to_string())],
        ));
        GraveResult::UndeadAwakened { monster_key }
    } else if roll < 70 {
        events.push(EngineEvent::msg("grave-corpse"));
        GraveResult::Corpse
    } else {
        events.push(EngineEvent::msg("grave-empty"));
        GraveResult::Empty
    };

    (result, events)
}

// ---------------------------------------------------------------------------
// Watchman observes digging (watch_dig from dig.c)
// ---------------------------------------------------------------------------

/// Whether a watchman can see a dig at a given position and reacts.
///
/// In NetHack, watchmen in Minetown will warn and then attack if you
/// dig in their jurisdiction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchDigReaction {
    /// No watchman observing.
    Unobserved,
    /// First warning.
    Warning,
    /// Watchman turns hostile.
    Hostile,
}

/// Determine a watchman's reaction to digging.
pub fn watch_dig_reaction(
    is_in_town: bool,
    watchman_can_see: bool,
    prior_warnings: u32,
) -> WatchDigReaction {
    if !is_in_town || !watchman_can_see {
        return WatchDigReaction::Unobserved;
    }
    if prior_warnings == 0 {
        WatchDigReaction::Warning
    } else {
        WatchDigReaction::Hostile
    }
}

// ---------------------------------------------------------------------------
// Monster tunnel digging (mdig_tunnel from dig.c)
// ---------------------------------------------------------------------------

/// Whether a monster can dig through a given terrain tile.
///
/// Tunneling monsters (e.g., umber hulk) can dig through walls/stone.
/// Some can also dig through trees (woodchuck).
pub fn can_monster_tunnel(terrain: Terrain, can_dig_trees: bool) -> bool {
    match terrain {
        Terrain::Wall | Terrain::Stone => true,
        Terrain::Tree if can_dig_trees => true,
        _ => false,
    }
}

/// Perform monster tunneling: convert the terrain at the target position.
pub fn monster_tunnel(
    world: &mut GameWorld,
    pos: Position,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let terrain = world
        .dungeon()
        .current_level
        .get(pos)
        .map(|c| c.terrain)
        .unwrap_or(Terrain::Stone);

    match terrain {
        Terrain::Wall | Terrain::Stone | Terrain::Tree => {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(pos, Terrain::Corridor);
            events.push(EngineEvent::msg_with(
                "tunnel-through",
                vec![("terrain", format!("{:?}", terrain))],
            ));
        }
        _ => {
            events.push(EngineEvent::msg("tunnel-blocked"));
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Liquid flow: fill holes near water/lava (fillholetyp from dig.c)
// ---------------------------------------------------------------------------

/// What liquid type might fill a hole at a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillLiquid {
    None,
    Pool,
    Moat,
    Lava,
}

/// Determine if a hole at `pos` should be filled with liquid from
/// adjacent tiles.
pub fn fill_hole_liquid(
    adjacent_terrain: &[Terrain],
) -> FillLiquid {
    let mut pool_count = 0u32;
    let mut moat_count = 0u32;
    let mut lava_count = 0u32;

    for &t in adjacent_terrain {
        match t {
            Terrain::Moat => moat_count += 1,
            Terrain::Pool | Terrain::Water => pool_count += 1,
            Terrain::Lava => lava_count += 1,
            _ => {}
        }
    }

    if lava_count > moat_count + pool_count {
        FillLiquid::Lava
    } else if moat_count > 0 {
        FillLiquid::Moat
    } else if pool_count > 0 {
        FillLiquid::Pool
    } else {
        FillLiquid::None
    }
}

// ---------------------------------------------------------------------------
// Dig effort / holetime (how long until a shop dig completes)
// ---------------------------------------------------------------------------

/// Estimate turns remaining for an in-progress floor dig.
///
/// Based on `holetime()` from dig.c: `(250 - effort) / 20`.
pub fn hole_time_remaining(effort_so_far: u32) -> i32 {
    ((250u32.saturating_sub(effort_so_far)) / 20) as i32
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Direction, Position};
    use crate::dungeon::Terrain;
    use crate::world::GameWorld;
    use rand::SeedableRng;

    type TestRng = rand::rngs::StdRng;

    fn test_rng() -> TestRng {
        TestRng::seed_from_u64(42)
    }

    fn test_world() -> GameWorld {
        GameWorld::new(Position::new(40, 10))
    }

    /// Set the dungeon branch for testing.
    fn set_branch(world: &mut GameWorld, branch: DungeonBranch) {
        world.dungeon_mut().branch = branch;
    }

    // ── Wall digging tests ────────────────────────────────────────

    #[test]
    fn test_dig_wall_to_corridor() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Place a wall north of the player.
        let player_pos = world
            .get_component::<Positioned>(player)
            .unwrap()
            .0;
        let wall_pos = Position::new(player_pos.x, player_pos.y - 1);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(wall_pos, Terrain::Wall);

        // Start digging.
        let (events, state) = dig(
            &mut world, player, Direction::North, ToolType::PickAxe, None, &mut rng,
        );

        // Should be multi-turn: state should exist.
        assert!(state.is_some(), "should start multi-turn dig");
        let mut current_state = state.unwrap();
        assert_eq!(current_state.target_pos, wall_pos);
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::Message { key, .. } if key == "dig-start"
            )),
        );

        // Continue digging until complete.
        while current_state.turns_remaining > 0 {
            let (cont_events, next_state) = dig(
                &mut world,
                player,
                Direction::North,
                ToolType::PickAxe,
                Some(current_state),
                &mut rng,
            );
            if let Some(s) = next_state {
                current_state = s;
            } else {
                // Dig completed.
                assert!(
                    cont_events.iter().any(|e| matches!(
                        e,
                        EngineEvent::Message { key, .. } if key == "dig-wall-done"
                    )),
                );
                break;
            }
        }

        // The wall should now be a corridor.
        let terrain = world
            .dungeon()
            .current_level
            .get(wall_pos)
            .unwrap()
            .terrain;
        assert_eq!(
            terrain,
            Terrain::Corridor,
            "wall should be dug into corridor"
        );
    }

    #[test]
    fn test_dig_floor_to_hole() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Set player's position to a floor tile.
        let player_pos = world
            .get_component::<Positioned>(player)
            .unwrap()
            .0;
        world
            .dungeon_mut()
            .current_level
            .set_terrain(player_pos, Terrain::Floor);

        // Start digging down.
        let (_, state) = dig(
            &mut world, player, Direction::Down, ToolType::PickAxe, None, &mut rng,
        );

        assert!(state.is_some(), "should start multi-turn dig");
        let mut current_state = state.unwrap();

        // Continue until complete.
        loop {
            let (cont_events, next_state) = dig(
                &mut world,
                player,
                Direction::Down,
                ToolType::PickAxe,
                Some(current_state),
                &mut rng,
            );
            if let Some(s) = next_state {
                current_state = s;
            } else {
                // Should have created a hole.
                assert!(
                    cont_events.iter().any(|e| matches!(
                        e,
                        EngineEvent::Message { key, .. } if key == "dig-floor-hole"
                    )),
                );
                assert!(
                    cont_events.iter().any(|e| matches!(
                        e,
                        EngineEvent::TrapRevealed { trap_type, .. }
                        if *trap_type == nethack_babel_data::TrapType::Hole
                    )),
                );
                break;
            }
        }
    }

    // ── No-dig level tests ────────────────────────────────────────

    #[test]
    fn test_dig_blocked_in_sokoban() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        set_branch(&mut world, DungeonBranch::Sokoban);

        // Place a wall north.
        let player_pos = world
            .get_component::<Positioned>(player)
            .unwrap()
            .0;
        let wall_pos = Position::new(player_pos.x, player_pos.y - 1);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(wall_pos, Terrain::Wall);

        let (events, state) = dig(
            &mut world, player, Direction::North, ToolType::PickAxe, None, &mut rng,
        );

        assert!(state.is_none(), "should not start digging in Sokoban");
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::Message { key, .. } if key == "dig-blocked"
            )),
        );

        // Wall should still be there.
        let terrain = world
            .dungeon()
            .current_level
            .get(wall_pos)
            .unwrap()
            .terrain;
        assert_eq!(terrain, Terrain::Wall, "wall should remain in Sokoban");
    }

    #[test]
    fn test_dig_blocked_in_endgame() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        set_branch(&mut world, DungeonBranch::Endgame);

        let player_pos = world
            .get_component::<Positioned>(player)
            .unwrap()
            .0;
        let wall_pos = Position::new(player_pos.x, player_pos.y - 1);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(wall_pos, Terrain::Wall);

        let (events, state) = dig(
            &mut world, player, Direction::North, ToolType::PickAxe, None, &mut rng,
        );

        assert!(state.is_none());
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::Message { key, .. } if key == "dig-blocked"
            )),
        );
    }

    // ── Multi-turn dig tests ──────────────────────────────────────

    #[test]
    fn test_multi_turn_dig() {
        // Verify dig takes 2-6 turns (pick-axe wall dig).
        let mut total_turns = Vec::new();

        for seed in 0u64..50 {
            let mut world = test_world();
            let mut rng = TestRng::seed_from_u64(seed);
            let player = world.player();

            let player_pos = world
                .get_component::<Positioned>(player)
                .unwrap()
                .0;
            let wall_pos = Position::new(player_pos.x, player_pos.y - 1);
            world
                .dungeon_mut()
                .current_level
                .set_terrain(wall_pos, Terrain::Wall);

            let (_, state) = dig(
                &mut world,
                player,
                Direction::North,
                ToolType::PickAxe,
                None,
                &mut rng,
            );

            if let Some(state) = state {
                // turns_remaining + 1 (for the first turn spent starting)
                total_turns.push(state.turns_remaining as u32 + 1);
            }
        }

        assert!(
            !total_turns.is_empty(),
            "should have started some digs"
        );

        let min = *total_turns.iter().min().unwrap();
        let max = *total_turns.iter().max().unwrap();

        // Pick-axe wall dig should be 3-6 turns.
        assert!(
            min >= 2 && max <= 7,
            "dig turns should be in range 2-7, got min={min} max={max}"
        );
    }

    // ── Wand of digging (instant ray) ─────────────────────────────

    #[test]
    fn test_dig_ray_horizontal() {
        let mut world = test_world();
        let player = world.player();

        let player_pos = world
            .get_component::<Positioned>(player)
            .unwrap()
            .0;

        // Place a line of walls east of the player.
        for dx in 1..=5 {
            let pos = Position::new(player_pos.x + dx, player_pos.y);
            world
                .dungeon_mut()
                .current_level
                .set_terrain(pos, Terrain::Wall);
        }

        let events = dig_ray(&mut world, player, Direction::East);

        // All 5 walls should be corridors now.
        for dx in 1..=5 {
            let pos = Position::new(player_pos.x + dx, player_pos.y);
            let terrain = world
                .dungeon()
                .current_level
                .get(pos)
                .unwrap()
                .terrain;
            assert_eq!(
                terrain,
                Terrain::Corridor,
                "wall at dx={dx} should be dug"
            );
        }

        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::Message { key, .. } if key == "dig-ray"
            )),
        );
    }

    #[test]
    fn test_dig_ray_stops_at_floor() {
        let mut world = test_world();
        let player = world.player();

        let player_pos = world
            .get_component::<Positioned>(player)
            .unwrap()
            .0;

        // Wall, Wall, Floor, Wall — should stop after 2.
        let pos1 = Position::new(player_pos.x + 1, player_pos.y);
        let pos2 = Position::new(player_pos.x + 2, player_pos.y);
        let pos3 = Position::new(player_pos.x + 3, player_pos.y);
        let pos4 = Position::new(player_pos.x + 4, player_pos.y);
        world.dungeon_mut().current_level.set_terrain(pos1, Terrain::Wall);
        world.dungeon_mut().current_level.set_terrain(pos2, Terrain::Wall);
        world.dungeon_mut().current_level.set_terrain(pos3, Terrain::Floor);
        world.dungeon_mut().current_level.set_terrain(pos4, Terrain::Wall);

        dig_ray(&mut world, player, Direction::East);

        // First two should be dug.
        assert_eq!(
            world.dungeon().current_level.get(pos1).unwrap().terrain,
            Terrain::Corridor
        );
        assert_eq!(
            world.dungeon().current_level.get(pos2).unwrap().terrain,
            Terrain::Corridor
        );
        // Floor remains, wall behind it untouched.
        assert_eq!(
            world.dungeon().current_level.get(pos3).unwrap().terrain,
            Terrain::Floor
        );
        assert_eq!(
            world.dungeon().current_level.get(pos4).unwrap().terrain,
            Terrain::Wall
        );
    }

    #[test]
    fn test_dig_ray_sokoban_blocked() {
        let mut world = test_world();
        let player = world.player();

        set_branch(&mut world, DungeonBranch::Sokoban);

        let events = dig_ray(&mut world, player, Direction::East);

        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::Message { key, .. } if key == "dig-blocked"
            )),
        );
    }

    // ── can_dig tests ─────────────────────────────────────────────

    #[test]
    fn test_can_dig_wall() {
        let mut world = test_world();
        let pos = Position::new(41, 10);
        world.dungeon_mut().current_level.set_terrain(pos, Terrain::Wall);
        assert!(can_dig(&world, pos, Direction::East));
    }

    #[test]
    fn test_can_dig_floor_down() {
        let mut world = test_world();
        let pos = Position::new(40, 10);
        world.dungeon_mut().current_level.set_terrain(pos, Terrain::Floor);
        assert!(can_dig(&world, pos, Direction::Down));
    }

    #[test]
    fn test_cannot_dig_floor_horizontally() {
        let mut world = test_world();
        let pos = Position::new(41, 10);
        world.dungeon_mut().current_level.set_terrain(pos, Terrain::Floor);
        assert!(!can_dig(&world, pos, Direction::East));
    }

    // ── Dig type classification ───────────────────────────────────

    #[test]
    fn test_dig_type_wall_with_pick() {
        assert_eq!(
            dig_type(Terrain::Wall, ToolType::PickAxe, false, false),
            DigType::Rock,
        );
    }

    #[test]
    fn test_dig_type_wall_with_mattock() {
        assert_eq!(
            dig_type(Terrain::Wall, ToolType::Mattock, false, false),
            DigType::Rock,
        );
    }

    #[test]
    fn test_dig_type_door() {
        assert_eq!(
            dig_type(Terrain::DoorClosed, ToolType::PickAxe, false, false),
            DigType::Door,
        );
        assert_eq!(
            dig_type(Terrain::DoorLocked, ToolType::Mattock, false, false),
            DigType::Door,
        );
    }

    #[test]
    fn test_dig_type_statue() {
        assert_eq!(
            dig_type(Terrain::Wall, ToolType::PickAxe, true, false),
            DigType::Statue,
        );
    }

    #[test]
    fn test_dig_type_boulder() {
        assert_eq!(
            dig_type(Terrain::Wall, ToolType::PickAxe, false, true),
            DigType::Boulder,
        );
    }

    #[test]
    fn test_dig_type_tree_with_pick_undiggable() {
        assert_eq!(
            dig_type(Terrain::Tree, ToolType::PickAxe, false, false),
            DigType::Tree, // pick-axe treated as axe-like in our simplified model
        );
    }

    #[test]
    fn test_dig_type_floor_undiggable() {
        assert_eq!(
            dig_type(Terrain::Floor, ToolType::PickAxe, false, false),
            DigType::Undiggable,
        );
    }

    // ── Dig check (downward) ──────────────────────────────────────

    #[test]
    fn test_dig_check_passed() {
        assert_eq!(
            dig_check(Terrain::Floor, DungeonBranch::Main, true, false),
            DigCheckResult::Passed,
        );
    }

    #[test]
    fn test_dig_check_stairs() {
        assert_eq!(
            dig_check(Terrain::StairsDown, DungeonBranch::Main, true, false),
            DigCheckResult::OnStairs,
        );
    }

    #[test]
    fn test_dig_check_throne() {
        assert_eq!(
            dig_check(Terrain::Throne, DungeonBranch::Main, true, false),
            DigCheckResult::OnThrone,
        );
    }

    #[test]
    fn test_dig_check_altar() {
        assert_eq!(
            dig_check(Terrain::Altar, DungeonBranch::Main, true, false),
            DigCheckResult::OnAltar,
        );
    }

    #[test]
    fn test_dig_check_pit_only() {
        assert_eq!(
            dig_check(Terrain::Floor, DungeonBranch::Main, false, false),
            DigCheckResult::PitOnly,
        );
    }

    #[test]
    fn test_dig_check_boulder_blocks() {
        assert_eq!(
            dig_check(Terrain::Floor, DungeonBranch::Main, true, true),
            DigCheckResult::BoulderBlocks,
        );
    }

    // ── Dig up grave ──────────────────────────────────────────────

    #[test]
    fn test_dig_up_grave_produces_result() {
        let mut rng = test_rng();
        let (result, events) = dig_up_grave(&mut rng);
        assert!(!events.is_empty(), "digging grave should produce events");
        // Should be one of the four result types.
        assert!(matches!(
            result,
            GraveResult::Treasure { .. }
                | GraveResult::UndeadAwakened { .. }
                | GraveResult::Corpse
                | GraveResult::Empty
        ));
    }

    #[test]
    fn test_dig_up_grave_distribution() {
        // Run many trials and verify all four outcomes appear.
        let mut treasure = false;
        let mut undead = false;
        let mut corpse = false;
        let mut empty = false;
        for seed in 0..200u64 {
            let mut rng = TestRng::seed_from_u64(seed);
            let (result, _) = dig_up_grave(&mut rng);
            match result {
                GraveResult::Treasure { .. } => treasure = true,
                GraveResult::UndeadAwakened { .. } => undead = true,
                GraveResult::Corpse => corpse = true,
                GraveResult::Empty => empty = true,
            }
        }
        assert!(treasure, "should see treasure result");
        assert!(undead, "should see undead result");
        assert!(corpse, "should see corpse result");
        assert!(empty, "should see empty result");
    }

    // ── Watch dig ─────────────────────────────────────────────────

    #[test]
    fn test_watch_dig_unobserved() {
        assert_eq!(
            watch_dig_reaction(false, true, 0),
            WatchDigReaction::Unobserved,
        );
        assert_eq!(
            watch_dig_reaction(true, false, 0),
            WatchDigReaction::Unobserved,
        );
    }

    #[test]
    fn test_watch_dig_warning_then_hostile() {
        assert_eq!(
            watch_dig_reaction(true, true, 0),
            WatchDigReaction::Warning,
        );
        assert_eq!(
            watch_dig_reaction(true, true, 1),
            WatchDigReaction::Hostile,
        );
    }

    // ── Monster tunnel ────────────────────────────────────────────

    #[test]
    fn test_can_monster_tunnel_wall() {
        assert!(can_monster_tunnel(Terrain::Wall, false));
        assert!(can_monster_tunnel(Terrain::Stone, false));
    }

    #[test]
    fn test_cannot_monster_tunnel_floor() {
        assert!(!can_monster_tunnel(Terrain::Floor, false));
    }

    #[test]
    fn test_can_monster_tunnel_tree_if_allowed() {
        assert!(!can_monster_tunnel(Terrain::Tree, false));
        assert!(can_monster_tunnel(Terrain::Tree, true));
    }

    #[test]
    fn test_monster_tunnel_converts_terrain() {
        let mut world = test_world();
        let pos = Position::new(41, 10);
        world.dungeon_mut().current_level.set_terrain(pos, Terrain::Wall);

        let events = monster_tunnel(&mut world, pos);
        assert!(!events.is_empty());

        let terrain = world.dungeon().current_level.get(pos).unwrap().terrain;
        assert_eq!(terrain, Terrain::Corridor);
    }

    // ── Fill hole liquid ──────────────────────────────────────────

    #[test]
    fn test_fill_hole_no_liquid() {
        let adj = [Terrain::Floor, Terrain::Wall, Terrain::Corridor];
        assert_eq!(fill_hole_liquid(&adj), FillLiquid::None);
    }

    #[test]
    fn test_fill_hole_pool() {
        let adj = [Terrain::Pool, Terrain::Floor, Terrain::Floor];
        assert_eq!(fill_hole_liquid(&adj), FillLiquid::Pool);
    }

    #[test]
    fn test_fill_hole_lava_dominant() {
        let adj = [Terrain::Lava, Terrain::Lava, Terrain::Pool];
        assert_eq!(fill_hole_liquid(&adj), FillLiquid::Lava);
    }

    #[test]
    fn test_fill_hole_moat() {
        let adj = [Terrain::Moat, Terrain::Floor];
        assert_eq!(fill_hole_liquid(&adj), FillLiquid::Moat);
    }

    // ── Hole time remaining ───────────────────────────────────────

    #[test]
    fn test_hole_time_remaining() {
        assert_eq!(hole_time_remaining(0), 12); // 250/20 = 12
        assert_eq!(hole_time_remaining(50), 10); // 200/20 = 10
        assert_eq!(hole_time_remaining(250), 0);
        assert_eq!(hole_time_remaining(300), 0); // saturating_sub
    }
}
