//! Lock mechanics: picking locks, forcing locks, and locking doors.
//!
//! Provides full lock interaction beyond the basic tool-dispatch in
//! `tools.rs`.  Supports doors (via terrain) and containers (via ECS
//! `Container` component).
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;

use crate::action::Position;
use crate::dungeon::Terrain;
use crate::environment::Container;
use crate::event::EngineEvent;
use crate::tools::ToolType;
use crate::world::{Attributes, GameWorld, Positioned};

// ---------------------------------------------------------------------------
// Dice helpers (matching NetHack conventions)
// ---------------------------------------------------------------------------

/// `rn2(x)` — random in [0, x-1].
#[inline]
fn rn2<R: Rng>(rng: &mut R, x: u32) -> u32 {
    if x == 0 {
        return 0;
    }
    rng.random_range(0..x)
}

// ---------------------------------------------------------------------------
// 1. Pick lock — use a key/lockpick/credit card
// ---------------------------------------------------------------------------

/// Attempt to pick/unlock a locked door at `target_pos` or a locked
/// container at `target_pos` using a lock-manipulation tool.
///
/// Success rates (NetHack-style, skill-influenced):
/// - **Key**: 100% success.
/// - **Lock pick**: succeeds if `rn2(skill + 10) > 9` (~50-90% depending
///   on skill).  On failure, 1/25 chance the lockpick breaks.
/// - **Credit card**: succeeds if `rn2(skill + 20) > 19` (~5-60% depending
///   on skill).
///
/// `skill` ranges from 0 (unskilled) to ~20 (expert).  A reasonable
/// default for an unskilled player is 0.
///
/// On success for doors: `DoorLocked` → `DoorClosed`.
/// On success for containers: `locked` flag cleared.
pub fn pick_lock(
    world: &mut GameWorld,
    player: Entity,
    target_pos: Position,
    tool: ToolType,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Determine player skill (simplified: use dexterity / 3 as proxy).
    let skill: u32 = world
        .get_component::<Attributes>(player)
        .map(|a| (a.dexterity / 3) as u32)
        .unwrap_or(3);

    // Check for a locked door at target_pos.
    let terrain = world
        .dungeon()
        .current_level
        .get(target_pos)
        .map(|c| c.terrain);
    if terrain == Some(Terrain::DoorLocked) {
        return pick_lock_door(world, target_pos, tool, skill, rng);
    }

    // Check for a locked container at target_pos.
    let container_entity = find_locked_container_at(world, target_pos);
    if let Some(container_e) = container_entity {
        return pick_lock_container(world, container_e, tool, skill, rng);
    }

    events.push(EngineEvent::msg("lock-no-target"));
    events
}

/// Internal: attempt to pick a locked door.
fn pick_lock_door(
    world: &mut GameWorld,
    target_pos: Position,
    tool: ToolType,
    skill: u32,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let success = lock_tool_success(tool, skill, rng);

    if success {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(target_pos, Terrain::DoorClosed);
        events.push(EngineEvent::msg("lock-pick-success"));
        events.push(EngineEvent::DoorClosed {
            position: target_pos,
        });
    } else {
        events.push(EngineEvent::msg("lock-pick-fail"));
        events.extend(maybe_break_lockpick(tool, rng));
    }

    events
}

/// Internal: attempt to pick a locked container.
fn pick_lock_container(
    world: &mut GameWorld,
    container_e: Entity,
    tool: ToolType,
    skill: u32,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let success = lock_tool_success(tool, skill, rng);

    if success {
        if let Some(mut container) = world.get_component_mut::<Container>(container_e) {
            container.locked = false;
        }
        events.push(EngineEvent::msg("lock-pick-container-success"));
    } else {
        events.push(EngineEvent::msg("lock-pick-fail"));
        events.extend(maybe_break_lockpick(tool, rng));
    }

    events
}

/// Roll for lock-tool success based on tool type and skill.
fn lock_tool_success(tool: ToolType, skill: u32, rng: &mut impl Rng) -> bool {
    match tool {
        ToolType::Key => true,
        ToolType::LockPick => rn2(rng, skill + 10) > 9,
        ToolType::CreditCard => rn2(rng, skill + 20) > 19,
        _ => false,
    }
}

/// On lockpick failure, 1/25 chance the pick breaks.
fn maybe_break_lockpick(tool: ToolType, rng: &mut impl Rng) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    if tool == ToolType::LockPick && rn2(rng, 25) == 0 {
        events.push(EngineEvent::msg("lock-lockpick-breaks"));
    }
    events
}

/// Find a locked container entity at the given position.
fn find_locked_container_at(world: &GameWorld, pos: Position) -> Option<Entity> {
    for (entity, (_container, positioned)) in
        world.ecs().query::<(&Container, &Positioned)>().iter()
    {
        if positioned.0 == pos && _container.locked {
            return Some(entity);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// 2. Force lock — bash open with weapon or tool
// ---------------------------------------------------------------------------

/// Attempt to force open a locked door or container by brute strength.
///
/// Uses the player's STR stat: succeeds if `rn2(STR) > 10`.
/// On failure, there is no weapon damage in this simplified model.
///
/// On success for doors: `DoorLocked` → `DoorBroken`.
/// On success for containers: `locked` flag cleared, container may be
/// destroyed (10% chance for non-bag containers).
pub fn force_lock(
    world: &mut GameWorld,
    player: Entity,
    target_pos: Position,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let strength: u32 = world
        .get_component::<Attributes>(player)
        .map(|a| a.strength as u32)
        .unwrap_or(10);

    // Check for a locked door.
    let terrain = world
        .dungeon()
        .current_level
        .get(target_pos)
        .map(|c| c.terrain);
    if terrain == Some(Terrain::DoorLocked) {
        let success = rn2(rng, strength) > 10;
        if success {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(target_pos, Terrain::DoorOpen);
            events.push(EngineEvent::msg("lock-force-success"));
            events.push(EngineEvent::DoorBroken {
                position: target_pos,
            });
        } else {
            events.push(EngineEvent::msg("lock-force-fail"));
        }
        return events;
    }

    // Check for a locked container.
    let container_entity = find_locked_container_at(world, target_pos);
    if let Some(container_e) = container_entity {
        let success = rn2(rng, strength) > 10;
        if success {
            if let Some(mut container) = world.get_component_mut::<Container>(container_e) {
                container.locked = false;
            }
            events.push(EngineEvent::msg("lock-force-container-success"));
        } else {
            events.push(EngineEvent::msg("lock-force-fail"));
        }
        return events;
    }

    events.push(EngineEvent::msg("lock-no-target"));
    events
}

// ---------------------------------------------------------------------------
// 3. Lock door — use a key to lock a closed door
// ---------------------------------------------------------------------------

/// Lock a closed (unlocked) door at `target_pos` using a key.
///
/// Only keys can lock doors (not lockpicks or credit cards).
/// Changes `DoorClosed` → `DoorLocked`.
pub fn lock_door(
    world: &mut GameWorld,
    _player: Entity,
    target_pos: Position,
    key: ToolType,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if key != ToolType::Key {
        events.push(EngineEvent::msg("lock-need-key"));
        return events;
    }

    let terrain = world
        .dungeon()
        .current_level
        .get(target_pos)
        .map(|c| c.terrain);
    if terrain == Some(Terrain::DoorClosed) {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(target_pos, Terrain::DoorLocked);
        events.push(EngineEvent::msg("lock-door-locked"));
        events.push(EngineEvent::DoorLocked {
            position: target_pos,
        });
    } else if terrain == Some(Terrain::DoorLocked) {
        events.push(EngineEvent::msg("lock-already-locked"));
    } else {
        events.push(EngineEvent::msg("lock-no-door"));
    }

    events
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
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

    /// Place a locked door east of the player.
    fn place_locked_door(world: &mut GameWorld) -> Position {
        let player_pos = world.get_component::<Positioned>(world.player()).unwrap().0;
        let door_pos = Position::new(player_pos.x + 1, player_pos.y);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(door_pos, Terrain::DoorLocked);
        door_pos
    }

    /// Place a closed door east of the player.
    fn place_closed_door(world: &mut GameWorld) -> Position {
        let player_pos = world.get_component::<Positioned>(world.player()).unwrap().0;
        let door_pos = Position::new(player_pos.x + 1, player_pos.y);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(door_pos, Terrain::DoorClosed);
        door_pos
    }

    // ── Pick lock tests ───────────────────────────────────────────

    #[test]
    fn test_pick_lock_key_always_succeeds() {
        // Key has 100% success rate across many trials.
        for seed in 0u64..50 {
            let mut world = test_world();
            let mut rng = TestRng::seed_from_u64(seed);
            let player = world.player();
            let door_pos = place_locked_door(&mut world);

            let events = pick_lock(&mut world, player, door_pos, ToolType::Key, &mut rng);

            let terrain = world.dungeon().current_level.get(door_pos).unwrap().terrain;
            assert_eq!(
                terrain,
                Terrain::DoorClosed,
                "key should always unlock (seed={seed})"
            );
            assert!(
                events.iter().any(|e| matches!(
                    e,
                    EngineEvent::Message { key, .. } if key == "lock-pick-success"
                )),
                "expected success message (seed={seed})"
            );
        }
    }

    #[test]
    fn test_pick_lock_lockpick_probabilistic() {
        let mut successes = 0;
        let trials = 200;

        for seed in 0..trials {
            let mut world = test_world();
            let mut rng = TestRng::seed_from_u64(seed);
            let player = world.player();
            let door_pos = place_locked_door(&mut world);

            let events = pick_lock(&mut world, player, door_pos, ToolType::LockPick, &mut rng);

            if events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::Message { key, .. } if key == "lock-pick-success"
                )
            }) {
                successes += 1;
            }
        }

        // With default dex=10 → skill=3, success = rn2(13)>9, so
        // P(success) = 3/13 ≈ 23%.  Allow wide range.
        assert!(
            successes > 10 && successes < 150,
            "lockpick success rate ({successes}/{trials}) should be in reasonable range"
        );
    }

    #[test]
    fn test_pick_lock_break_chance() {
        // Over many failures, some should report a broken lockpick.
        let mut breaks = 0;
        let trials = 500;

        for seed in 0..trials {
            let mut world = test_world();
            let mut rng = TestRng::seed_from_u64(seed);
            let player = world.player();
            let door_pos = place_locked_door(&mut world);

            let events = pick_lock(&mut world, player, door_pos, ToolType::LockPick, &mut rng);

            if events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::Message { key, .. } if key == "lock-lockpick-breaks"
                )
            }) {
                breaks += 1;
            }
        }

        // Break chance is 1/25 of failures.  With ~77% failure rate,
        // expected breaks ≈ 500 * 0.77 * 0.04 ≈ 15.  Allow wide range.
        assert!(
            breaks > 0,
            "lockpick should break at least once in {trials} trials"
        );
        assert!(breaks < 100, "lockpick breaks ({breaks}) too frequent");
    }

    // ── Force lock tests ──────────────────────────────────────────

    #[test]
    fn test_force_lock_str_based() {
        // With default STR=10, rn2(10)>10 always fails.
        // Boost STR to 18 for better success odds.
        let mut successes = 0;
        let trials = 100;

        for seed in 0..trials {
            let mut world = test_world();
            let mut rng = TestRng::seed_from_u64(seed);
            let player = world.player();

            // Boost strength.
            if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                attrs.strength = 18;
            }

            let door_pos = place_locked_door(&mut world);
            let events = force_lock(&mut world, player, door_pos, &mut rng);

            if events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::Message { key, .. } if key == "lock-force-success"
                )
            }) {
                successes += 1;
            }
        }

        // rn2(18) > 10 → values 11..17, so P = 7/18 ≈ 39%.
        assert!(
            successes > 15 && successes < 70,
            "force lock success rate ({successes}/{trials}) should be ~39% with STR 18"
        );
    }

    #[test]
    fn test_force_lock_low_str_fails() {
        // With STR=10, rn2(10) produces 0..9, none > 10 → always fails.
        for seed in 0u64..20 {
            let mut world = test_world();
            let mut rng = TestRng::seed_from_u64(seed);
            let player = world.player();
            let door_pos = place_locked_door(&mut world);

            let events = force_lock(&mut world, player, door_pos, &mut rng);

            let terrain = world.dungeon().current_level.get(door_pos).unwrap().terrain;
            assert_eq!(
                terrain,
                Terrain::DoorLocked,
                "low STR should not force open (seed={seed})"
            );
            assert!(
                events.iter().any(|e| matches!(
                    e,
                    EngineEvent::Message { key, .. } if key == "lock-force-fail"
                )),
                "expected failure message"
            );
        }
    }

    // ── Lock door tests ───────────────────────────────────────────

    #[test]
    fn test_lock_door_with_key() {
        let mut world = test_world();
        let player = world.player();
        let door_pos = place_closed_door(&mut world);

        let events = lock_door(&mut world, player, door_pos, ToolType::Key);

        let terrain = world.dungeon().current_level.get(door_pos).unwrap().terrain;
        assert_eq!(
            terrain,
            Terrain::DoorLocked,
            "closed door should become locked"
        );

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "lock-door-locked"
        )),);
    }

    #[test]
    fn test_lock_door_requires_key() {
        let mut world = test_world();
        let player = world.player();
        let door_pos = place_closed_door(&mut world);

        let events = lock_door(&mut world, player, door_pos, ToolType::LockPick);

        let terrain = world.dungeon().current_level.get(door_pos).unwrap().terrain;
        assert_eq!(
            terrain,
            Terrain::DoorClosed,
            "lockpick should not lock a door"
        );

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "lock-need-key"
        )),);
    }

    #[test]
    fn test_pick_lock_no_target() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Target a position with no locked door or container (default stone).
        let empty_pos = Position::new(50, 10);
        let events = pick_lock(&mut world, player, empty_pos, ToolType::Key, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "lock-no-target"
        )),);
    }
}
