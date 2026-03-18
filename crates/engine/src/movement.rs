//! Player movement, collision, and door interaction.
//!
//! This module implements the core movement logic described in the movement
//! spec (sections 3-5): directional movement with terrain checks, door
//! interaction (open/close/kick), pet displacement, and diagonal squeeze
//! restrictions.
//!
//! All functions are pure: they take a `GameWorld` plus an RNG, mutate
//! world state, and return a `Vec<EngineEvent>` describing what happened.
//! There is zero IO.

use hecs::Entity;
use rand::Rng;

use crate::action::{Direction, Position};
use crate::combat::resolve_melee_attack;
use crate::dungeon::Terrain;
use crate::event::{EngineEvent, HpSource};
use crate::religion::rnl;
use crate::status;
use crate::steed;
use crate::world::{
    Attributes, CarryWeight, GameWorld, Monster, Peaceful, PlayerCombat, Positioned, Tame,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Weight threshold above which diagonal squeeze is blocked (spec: 600).
const WT_TOOMUCH_DIAGONAL: u32 = 600;

// ---------------------------------------------------------------------------
// Top-level movement entry point
// ---------------------------------------------------------------------------

/// Resolve a single-step player movement in the given direction.
///
/// Handles terrain passability, door auto-open, monster collision, pet
/// displacement, and diagonal squeeze restrictions.  Returns the events
/// produced during resolution.
pub fn resolve_move(
    world: &mut GameWorld,
    direction: Direction,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player = world.player();
    let current_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    let target_pos = current_pos.step(direction);

    // Bounds check.
    if !world.dungeon().current_level.in_bounds(target_pos) {
        events.push(EngineEvent::msg("bump-wall"));
        return events;
    }

    // Diagonal squeeze check.
    if direction.is_diagonal() && !diagonal_ok(world, current_pos, target_pos) {
        events.push(EngineEvent::msg("diagonal-squeeze-blocked"));
        return events;
    }

    // Check for entity at target (monster or pet).
    if let Some(occupant) = entity_at(world, target_pos, player) {
        if is_tame(world, occupant) {
            // Swap positions with pet.
            swap_with_pet(
                world,
                player,
                occupant,
                current_pos,
                target_pos,
                &mut events,
            );
            return events;
        }
        if is_peaceful(world, occupant) {
            events.push(EngineEvent::msg_with(
                "peaceful-monster-blocks",
                vec![("monster", world.entity_name(occupant))],
            ));
            return events;
        }
        if is_monster(world, occupant) {
            // Trigger melee attack; do not move.
            resolve_melee_attack(world, player, occupant, rng, &mut events);
            return events;
        }
    }

    // Terrain check at target.
    let terrain = match world.dungeon().current_level.get(target_pos) {
        Some(cell) => cell.terrain,
        None => return events,
    };

    match terrain {
        // Passable terrain -- move the player.
        t if is_passable(t) => {
            move_player(world, player, current_pos, target_pos, &mut events);
        }

        // Closed door -- attempt to auto-open.
        Terrain::DoorClosed => {
            let mut open_events = try_open_door(world, target_pos, rng);
            events.append(&mut open_events);

            // If the door was successfully opened, also move the player onto
            // that tile.
            let after = world.dungeon().current_level.get(target_pos);
            if after.is_some_and(|c| is_passable(c.terrain)) {
                move_player(world, player, current_pos, target_pos, &mut events);
            }
        }

        // Locked door -- cannot walk through.
        Terrain::DoorLocked => {
            events.push(EngineEvent::msg("door-locked"));
        }

        // Solid terrain (walls, stone, trees, iron bars).
        Terrain::Wall | Terrain::Stone | Terrain::Tree | Terrain::IronBars => {
            events.push(EngineEvent::msg("bump-wall"));
        }

        // Water -- warn and block.
        Terrain::Water | Terrain::Pool | Terrain::Moat => {
            events.push(EngineEvent::msg("swim-water"));
        }

        // Lava -- warn and block.
        Terrain::Lava => {
            events.push(EngineEvent::msg("swim-lava"));
        }

        // Anything else we haven't specifically handled -- treat as impassable.
        _ => {
            events.push(EngineEvent::msg("bump-wall"));
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Door interaction
// ---------------------------------------------------------------------------

/// Attempt to open a closed door at `position`.
///
/// Success formula (from spec section 4.1):
///   `rnl(20) < (STR + DEX + CON) / 3`
///
/// Uses luck-adjusted `rnl()` so positive luck improves success rate.
pub fn try_open_door(
    world: &mut GameWorld,
    position: Position,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let terrain = match world.dungeon().current_level.get(position) {
        Some(cell) => cell.terrain,
        None => return events,
    };

    if terrain != Terrain::DoorClosed {
        events.push(EngineEvent::msg("door-no-closed"));
        return events;
    }

    let player = world.player();
    let attrs = world
        .get_component::<Attributes>(player)
        .map(|a| *a)
        .unwrap_or_default();

    let luck = world
        .get_component::<PlayerCombat>(player)
        .map(|pc| pc.luck)
        .unwrap_or(0);

    let avg_attrib =
        (attrs.strength as i32 + attrs.dexterity as i32 + attrs.constitution as i32) / 3;

    let roll = rnl(rng, 20, luck);

    if roll < avg_attrib {
        // Success: open the door.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(position, Terrain::DoorOpen);
        events.push(EngineEvent::DoorOpened { position });
    } else {
        events.push(EngineEvent::msg("door-resist"));
    }

    events
}

/// Attempt to close an open door at `position`.
///
/// Success formula (from spec section 4.2):
///   `rn2(25) < (STR + DEX + CON) / 3`
///
/// Uses luck-adjusted `rnl()` for the roll.
pub fn try_close_door(
    world: &mut GameWorld,
    position: Position,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let terrain = match world.dungeon().current_level.get(position) {
        Some(cell) => cell.terrain,
        None => return events,
    };

    if terrain != Terrain::DoorOpen {
        events.push(EngineEvent::msg("door-no-open"));
        return events;
    }

    let player = world.player();
    let attrs = world
        .get_component::<Attributes>(player)
        .map(|a| *a)
        .unwrap_or_default();

    let luck = world
        .get_component::<PlayerCombat>(player)
        .map(|pc| pc.luck)
        .unwrap_or(0);

    let avg_attrib =
        (attrs.strength as i32 + attrs.dexterity as i32 + attrs.constitution as i32) / 3;

    let roll = rnl(rng, 25, luck);

    if roll < avg_attrib {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(position, Terrain::DoorClosed);
        events.push(EngineEvent::DoorClosed { position });
    } else {
        events.push(EngineEvent::msg("door-resist"));
    }

    events
}

/// Attempt to kick a door open at `position`.
///
/// Success formula (from spec section 4.3):
///   `rnl(35) < avg_attrib`
///
/// where `avg_attrib = (STR + DEX + CON) / 3`.
///
/// Uses luck-adjusted `rnl()` for the roll. On success: door becomes
/// `DoorOpen` (simplified; spec distinguishes broken vs. shattered,
/// which we can refine later).
pub fn try_kick_door(
    world: &mut GameWorld,
    position: Position,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let terrain = match world.dungeon().current_level.get(position) {
        Some(cell) => cell.terrain,
        None => return events,
    };

    match terrain {
        Terrain::DoorClosed | Terrain::DoorLocked => {}
        _ => {
            events.push(EngineEvent::msg("door-no-kick"));
            return events;
        }
    }

    let player = world.player();
    let attrs = world
        .get_component::<Attributes>(player)
        .map(|a| *a)
        .unwrap_or_default();

    let luck = world
        .get_component::<PlayerCombat>(player)
        .map(|pc| pc.luck)
        .unwrap_or(0);

    let avg_attrib =
        (attrs.strength as i32 + attrs.dexterity as i32 + attrs.constitution as i32) / 3;

    let roll = rnl(rng, 35, luck);

    if roll < avg_attrib {
        // Door is broken open.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(position, Terrain::DoorOpen);
        events.push(EngineEvent::DoorBroken { position });
        events.push(EngineEvent::msg("door-broken"));
    } else {
        events.push(EngineEvent::msg("door-kick-fail"));
    }

    events
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Whether the given terrain is passable for walking.
pub fn is_passable(terrain: Terrain) -> bool {
    terrain.is_walkable()
}

/// Check diagonal squeeze restriction.
///
/// When moving diagonally and both adjacent orthogonal cells are not
/// passable (`bad_rock`-style check), the player must not be carrying
/// more than `WT_TOOMUCH_DIAGONAL` (600) total weight.
///
/// Returns `true` if the diagonal move is allowed.
pub fn diagonal_ok(world: &GameWorld, from: Position, to: Position) -> bool {
    let dx = to.x - from.x;
    let dy = to.y - from.y;

    // Only relevant for actual diagonal moves.
    if dx == 0 || dy == 0 {
        return true;
    }

    let map = &world.dungeon().current_level;

    // The two orthogonal neighbors that form the "squeeze" gap.
    let adj_h = Position::new(from.x + dx, from.y);
    let adj_v = Position::new(from.x, from.y + dy);

    let h_blocked = map
        .get(adj_h)
        .is_none_or(|c| !is_passable(c.terrain) && !is_door(c.terrain));
    let v_blocked = map
        .get(adj_v)
        .is_none_or(|c| !is_passable(c.terrain) && !is_door(c.terrain));

    // If both orthogonal neighbors are blocked, this is a tight squeeze.
    if h_blocked && v_blocked {
        let player = world.player();
        let weight = world
            .get_component::<CarryWeight>(player)
            .map(|cw| cw.0)
            .unwrap_or(0);
        if weight > WT_TOOMUCH_DIAGONAL {
            return false;
        }
    }

    // Diagonal door restriction: cannot move diagonally into or out of a
    // door (closed or open -- i.e. an intact doorway).
    let from_is_door = map.get(from).is_some_and(|c| is_door(c.terrain));
    let to_is_door = map.get(to).is_some_and(|c| is_door(c.terrain));
    if from_is_door || to_is_door {
        return false;
    }

    true
}

/// Whether the terrain is a door variant (open, closed, or locked).
fn is_door(terrain: Terrain) -> bool {
    matches!(
        terrain,
        Terrain::DoorOpen | Terrain::DoorClosed | Terrain::DoorLocked
    )
}

/// Move the player entity from `from` to `to`, updating the ECS component.
fn move_player(
    world: &mut GameWorld,
    player: Entity,
    from: Position,
    to: Position,
    events: &mut Vec<EngineEvent>,
) {
    if let Some(mut pos) = world.get_component_mut::<Positioned>(player) {
        pos.0 = to;
    }
    events.push(EngineEvent::EntityMoved {
        entity: player,
        from,
        to,
    });
}

/// Swap positions between the player and a tame entity (pet).
fn swap_with_pet(
    world: &mut GameWorld,
    player: Entity,
    pet: Entity,
    player_pos: Position,
    pet_pos: Position,
    events: &mut Vec<EngineEvent>,
) {
    // Move player to pet's position.
    if let Some(mut p) = world.get_component_mut::<Positioned>(player) {
        p.0 = pet_pos;
    }
    // Move pet to player's old position.
    if let Some(mut p) = world.get_component_mut::<Positioned>(pet) {
        p.0 = player_pos;
    }

    events.push(EngineEvent::EntityMoved {
        entity: player,
        from: player_pos,
        to: pet_pos,
    });
    events.push(EngineEvent::EntityMoved {
        entity: pet,
        from: pet_pos,
        to: player_pos,
    });
    events.push(EngineEvent::msg("pet-swap"));
}

/// Find a non-player entity at `pos`, if any.
fn entity_at(world: &GameWorld, pos: Position, exclude: Entity) -> Option<Entity> {
    for (entity, positioned) in world.query::<Positioned>().iter() {
        if entity != exclude && positioned.0 == pos {
            return Some(entity);
        }
    }
    None
}

/// Whether an entity has the `Monster` component.
fn is_monster(world: &GameWorld, entity: Entity) -> bool {
    world.get_component::<Monster>(entity).is_some()
}

/// Whether an entity has the `Tame` component.
fn is_tame(world: &GameWorld, entity: Entity) -> bool {
    world.get_component::<Tame>(entity).is_some()
}

/// Whether an entity has the `Peaceful` component.
fn is_peaceful(world: &GameWorld, entity: Entity) -> bool {
    world.get_component::<Peaceful>(entity).is_some()
}

// ---------------------------------------------------------------------------
// Ice sliding
// ---------------------------------------------------------------------------

/// Process ice sliding: when an entity steps onto ice, they may slide
/// further in the same direction.
///
/// In NetHack, stepping onto ice causes the player to slide unless they
/// have levitation, flying, or are wearing boots of fumbling/have wounded legs.
///
/// Slides one additional tile in the same direction. If the next tile
/// is also ice (and passable), continues sliding. Stops on non-ice terrain,
/// walls, or collision with another entity.
///
/// Returns events describing the slide, and the final position.
pub fn ice_slide(
    world: &mut GameWorld,
    entity: Entity,
    direction: Direction,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player = entity;
    let levitating = status::is_levitating(world, player);

    // Levitation prevents sliding.
    if levitating {
        return events;
    }

    let mut current_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Check the current tile is ice.
    let on_ice = world
        .dungeon()
        .current_level
        .get(current_pos)
        .is_some_and(|c| c.terrain == Terrain::Ice);
    if !on_ice {
        return events;
    }

    // Fumbling makes you fall on ice, take d(2,3) damage, no sliding.
    if status::is_fumbling(world, player) {
        let damage = {
            let d1: i32 = rng.random_range(1..=3);
            let d2: i32 = rng.random_range(1..=3);
            d1 + d2
        };
        events.push(EngineEvent::msg("ice-fumble-fall"));
        events.push(EngineEvent::HpChange {
            entity: player,
            amount: -damage,
            new_hp: world
                .get_component::<crate::world::HitPoints>(player)
                .map(|hp| hp.current - damage)
                .unwrap_or(0),
            source: HpSource::Environment,
        });
        return events;
    }

    events.push(EngineEvent::msg("ice-slide"));

    // Slide up to 5 tiles (NetHack uses varying limits; 5 is reasonable).
    let max_slide = 5;
    for _ in 0..max_slide {
        let next_pos = current_pos.step(direction);

        // Bounds check.
        if !world.dungeon().current_level.in_bounds(next_pos) {
            break;
        }

        let next_terrain = match world.dungeon().current_level.get(next_pos) {
            Some(cell) => cell.terrain,
            None => break,
        };

        // Stop if terrain is not passable.
        if !is_passable(next_terrain) {
            break;
        }

        // Stop if an entity occupies the next position.
        if entity_at(world, next_pos, player).is_some() {
            break;
        }

        // Slide to the next position.
        let old_pos = current_pos;
        current_pos = next_pos;

        if let Some(mut p) = world.get_component_mut::<Positioned>(player) {
            p.0 = current_pos;
        }

        events.push(EngineEvent::EntityMoved {
            entity: player,
            from: old_pos,
            to: current_pos,
        });

        // Continue sliding only if the next tile is also ice.
        if next_terrain != Terrain::Ice {
            break;
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Pool/water movement
// ---------------------------------------------------------------------------

/// Attempt to move into water/pool/moat terrain.
///
/// In NetHack, entering water without levitation, flying, or magical
/// breathing is dangerous. The player may drown (simplified here as
/// damage + message).
///
/// - Levitating/flying: skip over safely.
/// - Magical breathing: can enter safely (amphibious).
/// - Otherwise: take drowning damage and emit warning.
///
/// Returns the events and whether the move was allowed.
pub fn try_pool_movement(
    world: &mut GameWorld,
    entity: Entity,
    _target_pos: Position,
    rng: &mut impl Rng,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();

    let levitating = status::is_levitating(world, entity);

    // Levitating: safely float over water.
    if levitating {
        events.push(EngineEvent::msg("water-float-over"));
        return (events, true);
    }

    // Mounted on a swimming steed: safe passage through water.
    if steed::is_mounted(world, entity)
        && let Some(steed_entity) = steed::get_steed(world, entity)
    {
        let steed_can_swim = world
            .get_component::<crate::monster_ai::MonsterSpeciesFlags>(steed_entity)
            .is_some_and(|f| f.0.contains(nethack_babel_data::MonsterFlags::SWIM));
        if steed_can_swim {
            events.push(EngineEvent::msg("steed-swims"));
            return (events, true);
        }
    }

    // Check for magical breathing (from status or item).
    let magical_breathing = world
        .get_component::<status::StatusEffects>(entity)
        .is_some_and(|s| s.magical_breathing > 0);

    if magical_breathing {
        events.push(EngineEvent::msg("water-swim"));
        return (events, true);
    }

    // No protection: take drowning damage.
    let damage: i32 = rng.random_range(3..=12);
    let new_hp = world
        .get_component::<crate::world::HitPoints>(entity)
        .map(|hp| hp.current - damage)
        .unwrap_or(0);

    events.push(EngineEvent::msg("water-drown-danger"));
    events.push(EngineEvent::HpChange {
        entity,
        amount: -damage,
        new_hp,
        source: HpSource::Environment,
    });

    // The player enters the water but is damaged.
    (events, true)
}

// ---------------------------------------------------------------------------
// Lava movement
// ---------------------------------------------------------------------------

/// Attempt to move into lava terrain.
///
/// - Levitating/flying: safely float over.
/// - Fire resistance: take reduced damage.
/// - Otherwise: severe burn damage.
///
/// Returns the events and whether the move was allowed.
pub fn try_lava_movement(
    world: &mut GameWorld,
    entity: Entity,
    _target_pos: Position,
    rng: &mut impl Rng,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();

    let levitating = status::is_levitating(world, entity);

    if levitating {
        events.push(EngineEvent::msg("lava-float-over"));
        return (events, true);
    }

    let fire_res = status::has_intrinsic_fire_res(world, entity);
    let damage: i32 = if fire_res {
        // Fire resistance greatly reduces lava damage.
        rng.random_range(1..=6)
    } else {
        // Full lava damage is brutal: d(6,6).
        (0..6).map(|_| rng.random_range(1i32..=6)).sum()
    };

    let new_hp = world
        .get_component::<crate::world::HitPoints>(entity)
        .map(|hp| hp.current - damage)
        .unwrap_or(0);

    if fire_res {
        events.push(EngineEvent::msg("lava-resist"));
    } else {
        events.push(EngineEvent::msg("lava-burn"));
    }

    events.push(EngineEvent::HpChange {
        entity,
        amount: -damage,
        new_hp,
        source: HpSource::Environment,
    });

    (events, true)
}

// ---------------------------------------------------------------------------
// Engulfed movement
// ---------------------------------------------------------------------------

/// Marker component: the entity is currently engulfed by another entity.
#[derive(Debug, Clone, Copy)]
pub struct Engulfed {
    /// The engulfing monster.
    pub engulfer: Entity,
}

/// Resolve movement while engulfed.
///
/// In NetHack, movement while engulfed is restricted. Moving causes
/// the player to attack the engulfer's interior (dealing damage).
/// The player does not actually change position on the map.
///
/// Returns events describing the attack.
pub fn resolve_engulfed_move(
    world: &mut GameWorld,
    entity: Entity,
    _direction: Direction,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let engulfer = match world.get_component::<Engulfed>(entity) {
        Some(eng) => eng.engulfer,
        None => return events, // Not engulfed, shouldn't be called.
    };

    // Check engulfer still exists.
    if world.get_component::<Positioned>(engulfer).is_none() {
        // Engulfer gone: auto-escape.
        if world.ecs_mut().remove_one::<Engulfed>(entity).is_ok() {
            events.push(EngineEvent::msg("engulf-escaped"));
        }
        return events;
    }

    // Deal 1d4 damage to the engulfer.
    let damage: i32 = rng.random_range(1..=4);
    let new_hp = world
        .get_component::<crate::world::HitPoints>(engulfer)
        .map(|hp| hp.current - damage)
        .unwrap_or(0);

    events.push(EngineEvent::msg("engulf-attack-interior"));
    events.push(EngineEvent::HpChange {
        entity: engulfer,
        amount: -damage,
        new_hp,
        source: HpSource::Combat,
    });

    // If the engulfer dies, free the player.
    if new_hp <= 0 {
        let _ = world.ecs_mut().remove_one::<Engulfed>(entity);
        events.push(EngineEvent::msg("engulf-monster-dies"));
    }

    events
}

// ---------------------------------------------------------------------------
// Fumble check
// ---------------------------------------------------------------------------

/// Check if the entity fumbles on movement (trips and falls).
///
/// In NetHack, the `fumbling` status causes the player to trip
/// periodically.  When fumbling triggers, the player takes 1-3 damage
/// and loses a turn.
///
/// Returns `(events, fumbled)`.  If `fumbled` is true, the caller
/// should cancel the movement.
pub fn fumble_check(
    world: &mut GameWorld,
    entity: Entity,
    rng: &mut impl Rng,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();

    if !status::is_fumbling(world, entity) {
        return (events, false);
    }

    // Fumbling triggers with 1/5 probability per move.
    if rng.random_range(0u32..5) != 0 {
        return (events, false);
    }

    let damage: i32 = rng.random_range(1..=3);
    let new_hp = world
        .get_component::<crate::world::HitPoints>(entity)
        .map(|hp| hp.current - damage)
        .unwrap_or(0);

    events.push(EngineEvent::msg("fumble-trip"));
    events.push(EngineEvent::HpChange {
        entity,
        amount: -damage,
        new_hp,
        source: HpSource::Environment,
    });

    (events, true)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::dungeon::Terrain;
    use crate::world::{
        Attributes, CarryWeight, HitPoints, Monster, Name, Peaceful, Positioned, Speed, Tame,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    /// Create a deterministic RNG for tests.
    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(12345)
    }

    /// Build a small test world with floor from (3,3) to (7,7), player at
    /// (5,5), surrounded by stone elsewhere.
    fn make_test_world() -> GameWorld {
        let mut world = GameWorld::new(Position::new(5, 5));
        for y in 3..=7 {
            for x in 3..=7 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        world
    }

    // ── Basic movement ──────────────────────────────────────────

    #[test]
    fn move_into_empty_floor() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let events = resolve_move(&mut world, Direction::East, &mut rng);

        // Should emit an EntityMoved event.
        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(moved, "expected EntityMoved event");

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(6, 5));
    }

    #[test]
    fn bump_into_wall() {
        let mut world = make_test_world();
        // Place a wall directly east of the player.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Wall);

        let mut rng = test_rng();
        let events = resolve_move(&mut world, Direction::East, &mut rng);

        // Player should not have moved.
        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(5, 5));

        // Should have a bump message.
        let has_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("bump")));
        assert!(has_msg, "expected bump message");
    }

    // ── Door interaction ────────────────────────────────────────

    #[test]
    fn open_closed_door() {
        let mut world = make_test_world();
        // Give the player high attributes so the door always opens.
        {
            let player = world.player();
            if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                attrs.strength = 25;
                attrs.dexterity = 25;
                attrs.constitution = 25;
            }
        }

        let door_pos = Position::new(6, 5);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(door_pos, Terrain::DoorClosed);

        let mut rng = test_rng();
        let events = try_open_door(&mut world, door_pos, &mut rng);

        // With STR+DEX+CON = 54, avg = 18, and roll in [0,20), high chance
        // of success. Use a seeded rng to guarantee it.
        let opened = events
            .iter()
            .any(|e| matches!(e, EngineEvent::DoorOpened { .. }));
        assert!(opened, "expected door to open with high attributes");

        let terrain = world.dungeon().current_level.get(door_pos).unwrap().terrain;
        assert_eq!(terrain, Terrain::DoorOpen);
    }

    #[test]
    fn locked_door_blocks_movement() {
        let mut world = make_test_world();
        let door_pos = Position::new(6, 5);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(door_pos, Terrain::DoorLocked);

        let mut rng = test_rng();
        let events = resolve_move(&mut world, Direction::East, &mut rng);

        // Player should not move.
        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(5, 5));

        // Should get "locked" message.
        let has_locked_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("locked")));
        assert!(has_locked_msg, "expected locked door message");
    }

    #[test]
    fn peaceful_monster_blocks_normal_movement() {
        let mut world = make_test_world();
        let monster = world.spawn((
            Monster,
            Positioned(Position::new(6, 5)),
            HitPoints {
                current: 10,
                max: 10,
            },
            Speed(12),
            Name("gnome".to_string()),
            Peaceful,
        ));
        let _ = monster;

        let mut rng = test_rng();
        let events = resolve_move(&mut world, Direction::East, &mut rng);

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(5, 5));
        assert!(events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "peaceful-monster-blocks")
        ));
    }

    #[test]
    fn kick_closed_door() {
        let mut world = make_test_world();
        // Attributes high enough that avg >= 35, guaranteeing kick success.
        {
            let player = world.player();
            if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                attrs.strength = 40;
                attrs.dexterity = 40;
                attrs.constitution = 40;
            }
        }

        let door_pos = Position::new(6, 5);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(door_pos, Terrain::DoorClosed);

        let mut rng = test_rng();
        let events = try_kick_door(&mut world, door_pos, &mut rng);

        let broken = events
            .iter()
            .any(|e| matches!(e, EngineEvent::DoorBroken { .. }));
        assert!(
            broken,
            "expected door to be broken open with high attributes"
        );

        let terrain = world.dungeon().current_level.get(door_pos).unwrap().terrain;
        assert_eq!(terrain, Terrain::DoorOpen);
    }

    #[test]
    fn kick_locked_door() {
        let mut world = make_test_world();
        {
            let player = world.player();
            if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                attrs.strength = 40;
                attrs.dexterity = 40;
                attrs.constitution = 40;
            }
        }

        let door_pos = Position::new(6, 5);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(door_pos, Terrain::DoorLocked);

        let mut rng = test_rng();
        let events = try_kick_door(&mut world, door_pos, &mut rng);

        let broken = events
            .iter()
            .any(|e| matches!(e, EngineEvent::DoorBroken { .. }));
        assert!(broken, "expected locked door to be kicked open");
    }

    // ── Diagonal squeeze ────────────────────────────────────────

    #[test]
    fn diagonal_squeeze_blocked_by_heavy_load() {
        let mut world = make_test_world();

        // Create a tight diagonal: walls at (6,5) and (5,4), floor at (6,4).
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Wall);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(5, 4), Terrain::Wall);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 4), Terrain::Floor);

        // Give the player heavy carry weight (> 600).
        let player = world.player();
        let _ = world.ecs_mut().insert_one(player, CarryWeight(700));

        let mut rng = test_rng();
        let events = resolve_move(&mut world, Direction::NorthEast, &mut rng);

        // Player should not have moved.
        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(5, 5));

        // Should have a squeeze message.
        let has_squeeze_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("squeeze")));
        assert!(has_squeeze_msg, "expected diagonal squeeze blocked message");
    }

    #[test]
    fn diagonal_squeeze_allowed_with_light_load() {
        let mut world = make_test_world();

        // Same tight diagonal setup.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Wall);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(5, 4), Terrain::Wall);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 4), Terrain::Floor);

        // Light carry weight (< 600).
        let player = world.player();
        let _ = world.ecs_mut().insert_one(player, CarryWeight(300));

        let mut rng = test_rng();
        let events = resolve_move(&mut world, Direction::NorthEast, &mut rng);

        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(moved, "expected move to succeed with light load");

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(6, 4));
    }

    // ── Monster collision ───────────────────────────────────────

    #[test]
    fn monster_at_target_triggers_attack() {
        let mut world = make_test_world();

        // Spawn a hostile monster at (6,5).
        let _monster = world.spawn((
            Monster,
            Positioned(Position::new(6, 5)),
            HitPoints {
                current: 10,
                max: 10,
            },
            Speed(12),
            Name("goblin".to_string()),
        ));

        let mut rng = test_rng();
        let events = resolve_move(&mut world, Direction::East, &mut rng);

        // Player should NOT have moved (attacking instead).
        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(5, 5));

        // Should have a combat event (MeleeHit or MeleeMiss).
        let has_combat = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
            )
        });
        assert!(
            has_combat,
            "expected melee combat event when walking into monster"
        );
    }

    #[test]
    fn pet_at_target_swaps_positions() {
        let mut world = make_test_world();

        // Spawn a tame pet at (6,5).
        let pet = world.spawn((
            Monster,
            Tame,
            Positioned(Position::new(6, 5)),
            HitPoints { current: 8, max: 8 },
            Speed(12),
            Name("little dog".to_string()),
        ));

        let mut rng = test_rng();
        let events = resolve_move(&mut world, Direction::East, &mut rng);

        // Player should be at (6,5) now.
        let player_pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(player_pos.0, Position::new(6, 5));

        // Pet should be at (5,5) (player's old position).
        let pet_pos = world.get_component::<Positioned>(pet).unwrap();
        assert_eq!(pet_pos.0, Position::new(5, 5));

        // Should have swap message.
        let has_swap_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("swap")));
        assert!(has_swap_msg, "expected pet swap message");
    }

    // ── Helper unit tests ───────────────────────────────────────

    #[test]
    fn is_passable_floor() {
        assert!(is_passable(Terrain::Floor));
        assert!(is_passable(Terrain::Corridor));
        assert!(is_passable(Terrain::DoorOpen));
        assert!(is_passable(Terrain::StairsUp));
        assert!(is_passable(Terrain::StairsDown));
    }

    #[test]
    fn is_passable_blocked() {
        assert!(!is_passable(Terrain::Wall));
        assert!(!is_passable(Terrain::Stone));
        assert!(!is_passable(Terrain::DoorClosed));
        assert!(!is_passable(Terrain::DoorLocked));
        assert!(!is_passable(Terrain::Lava));
        assert!(!is_passable(Terrain::Water));
        assert!(!is_passable(Terrain::Pool));
    }

    #[test]
    fn try_close_door_success() {
        let mut world = make_test_world();
        // High attributes.
        {
            let player = world.player();
            if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                attrs.strength = 25;
                attrs.dexterity = 25;
                attrs.constitution = 25;
            }
        }

        let door_pos = Position::new(6, 5);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(door_pos, Terrain::DoorOpen);

        let mut rng = test_rng();
        let events = try_close_door(&mut world, door_pos, &mut rng);

        let closed = events
            .iter()
            .any(|e| matches!(e, EngineEvent::DoorClosed { .. }));
        assert!(closed, "expected door to close with high attributes");

        let terrain = world.dungeon().current_level.get(door_pos).unwrap().terrain;
        assert_eq!(terrain, Terrain::DoorClosed);
    }

    #[test]
    fn moving_into_closed_door_auto_opens() {
        let mut world = make_test_world();
        // High attributes so the auto-open succeeds.
        {
            let player = world.player();
            if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                attrs.strength = 25;
                attrs.dexterity = 25;
                attrs.constitution = 25;
            }
        }

        let door_pos = Position::new(6, 5);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(door_pos, Terrain::DoorClosed);

        let mut rng = test_rng();
        let events = resolve_move(&mut world, Direction::East, &mut rng);

        // Door should be opened and player should have moved through.
        let opened = events
            .iter()
            .any(|e| matches!(e, EngineEvent::DoorOpened { .. }));
        assert!(opened, "expected DoorOpened event");

        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(moved, "expected player to move through opened door");

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(6, 5));
    }

    #[test]
    fn water_blocks_movement() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Water);

        let mut rng = test_rng();
        let events = resolve_move(&mut world, Direction::East, &mut rng);

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(5, 5));

        let has_water_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("water")));
        assert!(has_water_msg, "expected water warning message");
    }

    #[test]
    fn lava_blocks_movement() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Lava);

        let mut rng = test_rng();
        let events = resolve_move(&mut world, Direction::East, &mut rng);

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(5, 5));

        let has_lava_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("lava")));
        assert!(has_lava_msg, "expected lava warning message");
    }

    // ═══════════════════════════════════════════════════════════════
    // New tests: ice sliding, pool movement, lava movement,
    //            engulfed movement
    // ═══════════════════════════════════════════════════════════════

    /// Build a test world with an ice corridor: floor from (3,5) to (4,5),
    /// ice from (5,5) to (9,5), floor at (10,5).
    fn make_ice_world() -> GameWorld {
        let mut world = GameWorld::new(Position::new(4, 5));
        // Set everything to floor first in a small area.
        for y in 3..=7 {
            for x in 3..=11 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        // Ice corridor.
        for x in 5..=9 {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(x, 5), Terrain::Ice);
        }
        world
    }

    #[test]
    fn ice_slide_moves_across_ice() {
        let mut world = make_ice_world();
        let mut rng = test_rng();
        let player = world.player();

        // Move player onto ice first.
        if let Some(mut p) = world.get_component_mut::<Positioned>(player) {
            p.0 = Position::new(5, 5);
        }

        let events = ice_slide(&mut world, player, Direction::East, &mut rng);

        let pos = world.get_component::<Positioned>(player).unwrap().0;

        // Should have slid east across ice tiles.
        assert!(pos.x > 5, "player should have slid east, at {:?}", pos);
        assert!(!events.is_empty(), "should have movement events");
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "ice-slide"
        )));
    }

    #[test]
    fn ice_slide_stops_at_wall() {
        let mut world = make_ice_world();
        let mut rng = test_rng();
        let player = world.player();

        // Put a wall at (7, 5).
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(7, 5), Terrain::Wall);

        if let Some(mut p) = world.get_component_mut::<Positioned>(player) {
            p.0 = Position::new(5, 5);
        }

        let _events = ice_slide(&mut world, player, Direction::East, &mut rng);

        let pos = world.get_component::<Positioned>(player).unwrap().0;

        // Should stop before the wall.
        assert!(
            pos.x <= 6,
            "player should stop before wall at x=7, at {:?}",
            pos
        );
    }

    #[test]
    fn ice_slide_levitating_no_slide() {
        let mut world = make_ice_world();
        let mut rng = test_rng();
        let player = world.player();

        // Grant levitation.
        if let Some(mut s) = world.get_component_mut::<crate::status::StatusEffects>(player) {
            s.levitation = 100;
        }

        if let Some(mut p) = world.get_component_mut::<Positioned>(player) {
            p.0 = Position::new(5, 5);
        }

        let events = ice_slide(&mut world, player, Direction::East, &mut rng);

        assert!(events.is_empty(), "levitating should not slide on ice");

        let pos = world.get_component::<Positioned>(player).unwrap().0;
        assert_eq!(pos, Position::new(5, 5), "should not have moved");
    }

    #[test]
    fn ice_slide_fumble_takes_damage() {
        let mut world = make_ice_world();
        let mut rng = test_rng();
        let player = world.player();

        // Grant fumbling.
        if let Some(mut s) = world.get_component_mut::<crate::status::StatusEffects>(player) {
            s.fumbling = 10;
        }

        if let Some(mut p) = world.get_component_mut::<Positioned>(player) {
            p.0 = Position::new(5, 5);
        }

        let events = ice_slide(&mut world, player, Direction::East, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "ice-fumble-fall"
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::HpChange {
                source: HpSource::Environment,
                ..
            }
        )));
    }

    #[test]
    fn ice_slide_not_on_ice() {
        let mut world = make_ice_world();
        let mut rng = test_rng();
        let player = world.player();

        // Player is on floor (4, 5), not ice.
        let events = ice_slide(&mut world, player, Direction::East, &mut rng);
        assert!(events.is_empty(), "not on ice, should not slide");
    }

    #[test]
    fn pool_movement_levitating_safe() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Grant levitation.
        if let Some(mut s) = world.get_component_mut::<crate::status::StatusEffects>(player) {
            s.levitation = 100;
        }

        let target = Position::new(6, 5);
        let (events, allowed) = try_pool_movement(&mut world, player, target, &mut rng);

        assert!(allowed, "levitating should be allowed to cross water");
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "water-float-over"
        )));
    }

    #[test]
    fn pool_movement_magical_breathing_safe() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Grant magical breathing.
        if let Some(mut s) = world.get_component_mut::<crate::status::StatusEffects>(player) {
            s.magical_breathing = 100;
        }

        let target = Position::new(6, 5);
        let (events, allowed) = try_pool_movement(&mut world, player, target, &mut rng);

        assert!(allowed, "magical breathing should allow swimming");
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "water-swim"
        )));
    }

    #[test]
    fn pool_movement_unprotected_takes_damage() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let target = Position::new(6, 5);
        let (events, allowed) = try_pool_movement(&mut world, player, target, &mut rng);

        assert!(allowed, "unprotected should still enter water");
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::HpChange {
                source: HpSource::Environment,
                ..
            }
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "water-drown-danger"
        )));
    }

    #[test]
    fn lava_movement_levitating_safe() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        if let Some(mut s) = world.get_component_mut::<crate::status::StatusEffects>(player) {
            s.levitation = 100;
        }

        let target = Position::new(6, 5);
        let (events, allowed) = try_lava_movement(&mut world, player, target, &mut rng);

        assert!(allowed);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "lava-float-over"
        )));
        // No damage.
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, EngineEvent::HpChange { .. }))
        );
    }

    #[test]
    fn lava_movement_fire_resist_reduced_damage() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Grant fire resistance via intrinsic.
        if let Some(mut intr) = world.get_component_mut::<crate::status::Intrinsics>(player) {
            intr.fire_resistance = true;
        }

        let target = Position::new(6, 5);
        let (events, allowed) = try_lava_movement(&mut world, player, target, &mut rng);

        assert!(allowed);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "lava-resist"
        )));
        // Should take damage (reduced).
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::HpChange { .. }))
        );
    }

    #[test]
    fn lava_movement_no_resist_heavy_damage() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let target = Position::new(6, 5);
        let (events, _allowed) = try_lava_movement(&mut world, player, target, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "lava-burn"
        )));
        // Should take heavy damage.
        let damage_event = events.iter().find_map(|e| {
            if let EngineEvent::HpChange { amount, .. } = e {
                Some(*amount)
            } else {
                None
            }
        });
        assert!(damage_event.is_some());
        let dmg = damage_event.unwrap();
        assert!(dmg <= -6, "lava damage should be at least 6, got {}", dmg);
    }

    #[test]
    fn engulfed_move_attacks_engulfer() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Spawn a monster as the engulfer.
        let monster = world.spawn((
            Monster,
            Positioned(Position::new(5, 5)),
            crate::world::HitPoints {
                current: 50,
                max: 50,
            },
            Speed(12),
            Name("purple worm".to_string()),
        ));

        // Engulf the player.
        let _ = world
            .ecs_mut()
            .insert_one(player, Engulfed { engulfer: monster });

        let events = resolve_engulfed_move(&mut world, player, Direction::East, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "engulf-attack-interior"
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::HpChange { entity, source: HpSource::Combat, .. }
            if *entity == monster
        )));
    }

    #[test]
    fn engulfed_move_kills_engulfer() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Spawn a monster with 1 HP.
        let monster = world.spawn((
            Monster,
            Positioned(Position::new(5, 5)),
            crate::world::HitPoints {
                current: 1,
                max: 10,
            },
            Speed(12),
            Name("blob".to_string()),
        ));

        let _ = world
            .ecs_mut()
            .insert_one(player, Engulfed { engulfer: monster });

        let events = resolve_engulfed_move(&mut world, player, Direction::East, &mut rng);

        // Monster should die, player should escape.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "engulf-monster-dies"
        )));

        // Player should no longer be engulfed.
        assert!(
            world.get_component::<Engulfed>(player).is_none(),
            "player should be freed after engulfer dies"
        );
    }

    #[test]
    fn engulfed_move_not_engulfed_noop() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Not engulfed: should be a no-op.
        let events = resolve_engulfed_move(&mut world, player, Direction::East, &mut rng);
        assert!(events.is_empty());
    }

    #[test]
    fn engulfed_move_engulfer_gone_escape() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Create a dangling entity reference.
        let monster = world.spawn((
            Monster,
            Positioned(Position::new(5, 5)),
            crate::world::HitPoints {
                current: 10,
                max: 10,
            },
            Speed(12),
            Name("blob".to_string()),
        ));

        let _ = world
            .ecs_mut()
            .insert_one(player, Engulfed { engulfer: monster });

        // Despawn the engulfer.
        let _ = world.despawn(monster);

        let events = resolve_engulfed_move(&mut world, player, Direction::East, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "engulf-escaped"
        )));
    }

    // ── Fumble check tests ────────────────────────────────────────

    #[test]
    fn fumble_check_not_fumbling_noop() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let (events, fumbled) = fumble_check(&mut world, player, &mut rng);
        assert!(!fumbled);
        assert!(events.is_empty());
    }

    #[test]
    fn fumble_check_eventually_triggers() {
        let mut world = make_test_world();
        let player = world.player();

        // Grant fumbling.
        if let Some(mut s) = world.get_component_mut::<crate::status::StatusEffects>(player) {
            s.fumbling = 100;
        }

        let mut saw_fumble = false;
        for seed in 0..100u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let (events, fumbled) = fumble_check(&mut world, player, &mut rng);
            if fumbled {
                saw_fumble = true;
                assert!(events.iter().any(|e| matches!(
                    e,
                    EngineEvent::Message { key, .. } if key == "fumble-trip"
                )));
                assert!(events.iter().any(|e| matches!(
                    e,
                    EngineEvent::HpChange {
                        source: HpSource::Environment,
                        ..
                    }
                )));
                break;
            }
        }
        assert!(saw_fumble, "fumbling should eventually trigger a trip");
    }

    #[test]
    fn fumble_check_damage_range() {
        let mut world = make_test_world();
        let player = world.player();

        if let Some(mut s) = world.get_component_mut::<crate::status::StatusEffects>(player) {
            s.fumbling = 100;
        }

        let mut min_dmg = i32::MAX;
        let mut max_dmg = 0i32;
        for seed in 0..500u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let (events, fumbled) = fumble_check(&mut world, player, &mut rng);
            if fumbled {
                if let Some(dmg) = events.iter().find_map(|e| {
                    if let EngineEvent::HpChange { amount, .. } = e {
                        Some(-*amount)
                    } else {
                        None
                    }
                }) {
                    min_dmg = min_dmg.min(dmg);
                    max_dmg = max_dmg.max(dmg);
                }
            }
        }
        assert!(
            min_dmg >= 1,
            "fumble damage min should be >= 1, got {}",
            min_dmg
        );
        assert!(
            max_dmg <= 3,
            "fumble damage max should be <= 3, got {}",
            max_dmg
        );
    }

    // ── Steed integration: mounted speed ─────────────────────────

    #[test]
    fn test_mounted_uses_steed_speed() {
        let mut world = make_test_world();
        let player = world.player();
        let mut rng = test_rng();

        // Spawn a steed with Speed(18) and mount it.
        let steed_pos = Position::new(6, 5);
        let steed = world.spawn((
            Monster,
            Tame,
            Positioned(steed_pos),
            Name("warhorse".to_string()),
            Speed(18),
            HitPoints {
                current: 30,
                max: 30,
            },
        ));
        let _ = crate::steed::mount(&mut world, player, steed, &mut rng);
        assert!(crate::steed::is_mounted(&world, player));

        // Mounted speed should come from the steed.
        let speed = crate::steed::mounted_speed(&world, player);
        assert_eq!(speed, Some(18), "mounted speed should use steed's speed");
    }

    #[test]
    fn test_unmounted_uses_player_speed() {
        let world = make_test_world();
        let player = world.player();

        // Not mounted: mounted_speed returns None.
        let speed = crate::steed::mounted_speed(&world, player);
        assert_eq!(speed, None, "unmounted should have no steed speed");
    }

    // ── Steed integration: swim through water ────────────────────

    #[test]
    fn test_mounted_swim_through_water() {
        let mut world = make_test_world();
        let player = world.player();
        let mut rng = test_rng();

        // Spawn a swimming steed (with MonsterSpeciesFlags including SWIM).
        let steed_pos = Position::new(6, 5);
        let steed = world.spawn((
            Monster,
            Tame,
            Positioned(steed_pos),
            Name("water horse".to_string()),
            Speed(18),
            HitPoints {
                current: 30,
                max: 30,
            },
            crate::monster_ai::MonsterSpeciesFlags(nethack_babel_data::MonsterFlags::SWIM),
        ));
        let _ = crate::steed::mount(&mut world, player, steed, &mut rng);

        // Try pool movement while mounted on swimming steed.
        let (events, allowed) =
            try_pool_movement(&mut world, player, Position::new(6, 5), &mut rng);
        assert!(
            allowed,
            "mounted on swimming steed should allow water passage"
        );
        assert!(
            events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "steed-swims")),
            "should emit steed-swims message"
        );
    }

    // ── Luck-adjusted door tests ───────────────────────────────

    #[test]
    fn luck_improves_door_open_rate() {
        // Compare open success rate with luck=0 vs luck=13 over many trials.
        // Both use moderate attributes where success is not guaranteed.
        let trials = 200u64;
        let mut successes_no_luck = 0;
        let mut successes_high_luck = 0;

        for seed in 0..trials {
            // No luck
            {
                let mut world = make_test_world();
                let player = world.player();
                if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                    attrs.strength = 12;
                    attrs.dexterity = 12;
                    attrs.constitution = 12;
                }
                let door_pos = Position::new(6, 5);
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(door_pos, Terrain::DoorClosed);
                let mut rng = Pcg64::seed_from_u64(seed);
                let events = try_open_door(&mut world, door_pos, &mut rng);
                if events
                    .iter()
                    .any(|e| matches!(e, EngineEvent::DoorOpened { .. }))
                {
                    successes_no_luck += 1;
                }
            }
            // High luck
            {
                let mut world = make_test_world();
                let player = world.player();
                if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                    attrs.strength = 12;
                    attrs.dexterity = 12;
                    attrs.constitution = 12;
                }
                // Set high luck via PlayerCombat component.
                if let Some(mut pc) = world.get_component_mut::<PlayerCombat>(player) {
                    pc.luck = 13;
                }
                let door_pos = Position::new(6, 5);
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(door_pos, Terrain::DoorClosed);
                let mut rng = Pcg64::seed_from_u64(seed);
                let events = try_open_door(&mut world, door_pos, &mut rng);
                if events
                    .iter()
                    .any(|e| matches!(e, EngineEvent::DoorOpened { .. }))
                {
                    successes_high_luck += 1;
                }
            }
        }

        assert!(
            successes_high_luck > successes_no_luck,
            "high luck ({successes_high_luck}) should produce more opens than no luck ({successes_no_luck})"
        );
    }

    #[test]
    fn luck_improves_kick_door_rate() {
        let trials = 200u64;
        let mut successes_no_luck = 0;
        let mut successes_high_luck = 0;

        for seed in 0..trials {
            // No luck
            {
                let mut world = make_test_world();
                let player = world.player();
                if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                    attrs.strength = 14;
                    attrs.dexterity = 14;
                    attrs.constitution = 14;
                }
                let door_pos = Position::new(6, 5);
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(door_pos, Terrain::DoorLocked);
                let mut rng = Pcg64::seed_from_u64(seed);
                let events = try_kick_door(&mut world, door_pos, &mut rng);
                if events
                    .iter()
                    .any(|e| matches!(e, EngineEvent::DoorBroken { .. }))
                {
                    successes_no_luck += 1;
                }
            }
            // High luck
            {
                let mut world = make_test_world();
                let player = world.player();
                if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                    attrs.strength = 14;
                    attrs.dexterity = 14;
                    attrs.constitution = 14;
                }
                if let Some(mut pc) = world.get_component_mut::<PlayerCombat>(player) {
                    pc.luck = 13;
                }
                let door_pos = Position::new(6, 5);
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(door_pos, Terrain::DoorLocked);
                let mut rng = Pcg64::seed_from_u64(seed);
                let events = try_kick_door(&mut world, door_pos, &mut rng);
                if events
                    .iter()
                    .any(|e| matches!(e, EngineEvent::DoorBroken { .. }))
                {
                    successes_high_luck += 1;
                }
            }
        }

        assert!(
            successes_high_luck > successes_no_luck,
            "high luck ({successes_high_luck}) should produce more kicks than no luck ({successes_no_luck})"
        );
    }
}
