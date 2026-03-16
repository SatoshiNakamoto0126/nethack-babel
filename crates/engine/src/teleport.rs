//! Teleportation system for NetHack Babel.
//!
//! Implements random teleportation, controlled teleportation, level
//! teleportation, and teleportitis (involuntary random teleport).
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;

use crate::action::Position;
use crate::dungeon::{DungeonBranch, Terrain};
use crate::event::EngineEvent;
use crate::movement::is_passable;
use crate::status::Intrinsics;
use crate::world::{GameWorld, Positioned};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Teleportitis triggers with probability 1/TELEPORTITIS_CHANCE per turn.
pub const TELEPORTITIS_CHANCE: u32 = 85;

// ---------------------------------------------------------------------------
// Random teleport
// ---------------------------------------------------------------------------

/// Teleport an entity to a random valid position on the current level.
///
/// Picks walkable positions and moves the entity there.  Emits an
/// `EntityTeleported` event with the old and new positions.
pub fn random_teleport(
    world: &mut GameWorld,
    entity: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let current_pos = match world.get_component::<Positioned>(entity) {
        Some(p) => p.0,
        None => return events,
    };

    let target = match find_random_walkable(world, rng) {
        Some(pos) => pos,
        None => return events, // No valid position (shouldn't happen).
    };

    // Move the entity.
    if let Some(mut p) = world.get_component_mut::<Positioned>(entity) {
        p.0 = target;
    }

    events.push(EngineEvent::EntityTeleported {
        entity,
        from: current_pos,
        to: target,
    });
    events.push(EngineEvent::msg("teleport-random"));

    events
}

// ---------------------------------------------------------------------------
// Controlled teleport
// ---------------------------------------------------------------------------

/// Teleport an entity to a specific target position.
///
/// Validates that the target is in bounds and walkable.  If the target
/// is invalid, returns an error message event and does not move.
pub fn controlled_teleport(
    world: &mut GameWorld,
    entity: Entity,
    target_pos: Position,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let current_pos = match world.get_component::<Positioned>(entity) {
        Some(p) => p.0,
        None => return events,
    };

    // Validate target.
    if !world.dungeon().current_level.in_bounds(target_pos) {
        events.push(EngineEvent::msg("teleport-invalid-target"));
        return events;
    }

    let terrain = match world.dungeon().current_level.get(target_pos) {
        Some(cell) => cell.terrain,
        None => {
            events.push(EngineEvent::msg("teleport-invalid-target"));
            return events;
        }
    };

    if !is_passable(terrain) {
        events.push(EngineEvent::msg("teleport-invalid-target"));
        return events;
    }

    // Move the entity.
    if let Some(mut p) = world.get_component_mut::<Positioned>(entity) {
        p.0 = target_pos;
    }

    events.push(EngineEvent::EntityTeleported {
        entity,
        from: current_pos,
        to: target_pos,
    });
    events.push(EngineEvent::msg("teleport-controlled"));

    events
}

// ---------------------------------------------------------------------------
// Level teleport
// ---------------------------------------------------------------------------

/// Teleport the entity to a different dungeon depth.
///
/// Uses the existing level transition infrastructure (depth string).
/// The target depth is clamped to valid range.  Emits a `LevelChanged`
/// event.
pub fn level_teleport(
    world: &mut GameWorld,
    entity: Entity,
    target_depth: i32,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let current_depth = world.dungeon().current_depth();
    let from_depth_str = format!("Dlvl:{}", current_depth);

    // Clamp to valid depth range [1, max_depth].
    let max_depth = world.dungeon().max_depth().max(1);
    let clamped = target_depth.clamp(1, max_depth);
    let to_depth_str = format!("Dlvl:{}", clamped);

    if clamped == current_depth {
        events.push(EngineEvent::msg("teleport-same-level"));
        return events;
    }

    // Generate and switch to the target level.
    let generated = crate::map_gen::generate_level(clamped as u8, rng);
    world
        .dungeon_mut()
        .set_current_level(generated.map, clamped);

    // Place entity at a walkable position on the new level.
    if let Some(pos) = find_random_walkable(world, rng)
        && let Some(mut p) = world.get_component_mut::<Positioned>(entity)
    {
        p.0 = pos;
    }

    events.push(EngineEvent::LevelChanged {
        entity,
        from_depth: from_depth_str,
        to_depth: to_depth_str,
    });
    events.push(EngineEvent::msg("teleport-level"));

    events
}

// ---------------------------------------------------------------------------
// Teleportitis check
// ---------------------------------------------------------------------------

/// Check whether an entity should involuntarily teleport this turn.
///
/// Returns `true` if the entity has the teleportitis intrinsic and the
/// 1/85 random check succeeds.
pub fn teleportitis_check(world: &GameWorld, entity: Entity, rng: &mut impl Rng) -> bool {
    let has_teleportitis = world
        .get_component::<Intrinsics>(entity)
        .is_some_and(|i| i.teleportitis);

    has_teleportitis && rng.random_range(0..TELEPORTITIS_CHANCE) == 0
}

// ---------------------------------------------------------------------------
// Teleport control check
// ---------------------------------------------------------------------------

/// Check whether the entity has teleport control.
///
/// When true, random teleports become controlled (player picks the
/// destination instead of it being random).
pub fn teleport_control_check(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<Intrinsics>(entity)
        .is_some_and(|i| i.teleport_control)
}

// ---------------------------------------------------------------------------
// Teleport restriction
// ---------------------------------------------------------------------------

/// Levels where teleportation is restricted.
///
/// In NetHack, certain special levels prevent teleporting:
/// - Endgame levels
/// - Sokoban (no-teleport flag)
/// - Quest nemesis level (while nemesis lives)
///
/// Returns `true` if teleportation is blocked on the current level.
pub fn tele_restrict(world: &GameWorld) -> bool {
    let branch = world.dungeon().branch;

    match branch {
        // Sokoban forbids teleportation entirely.
        DungeonBranch::Sokoban => true,
        // Endgame planes are no-teleport.
        DungeonBranch::Endgame => true,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Safe teleport (avoid traps and occupied positions)
// ---------------------------------------------------------------------------

/// Teleport an entity to a random walkable position that has no trap
/// and is not occupied by another entity.
///
/// Falls back to `random_teleport` if no truly safe position is found
/// within 2000 probes.
pub fn safe_teleport(
    world: &mut GameWorld,
    entity: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if tele_restrict(world) {
        events.push(EngineEvent::msg("teleport-restricted"));
        return events;
    }

    let current_pos = match world.get_component::<Positioned>(entity) {
        Some(p) => p.0,
        None => return events,
    };

    let target = match find_safe_walkable(world, entity, rng) {
        Some(pos) => pos,
        None => {
            // Fall back to basic random teleport.
            return random_teleport(world, entity, rng);
        }
    };

    if let Some(mut p) = world.get_component_mut::<Positioned>(entity) {
        p.0 = target;
    }

    events.push(EngineEvent::EntityTeleported {
        entity,
        from: current_pos,
        to: target,
    });
    events.push(EngineEvent::msg("teleport-random"));

    events
}

// ---------------------------------------------------------------------------
// Monster teleport
// ---------------------------------------------------------------------------

/// Teleport a monster to a random walkable position.
///
/// Mirrors C `rloc()`.  Unlike player teleport, monsters always teleport
/// randomly (no controlled teleport).  Respects teleport restriction on
/// the current level.
pub fn teleport_monster(
    world: &mut GameWorld,
    monster: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if tele_restrict(world) {
        // Monsters cannot teleport on restricted levels either.
        return events;
    }

    let current_pos = match world.get_component::<Positioned>(monster) {
        Some(p) => p.0,
        None => return events,
    };

    let target = match find_random_walkable(world, rng) {
        Some(pos) => pos,
        None => return events,
    };

    if let Some(mut p) = world.get_component_mut::<Positioned>(monster) {
        p.0 = target;
    }

    events.push(EngineEvent::EntityTeleported {
        entity: monster,
        from: current_pos,
        to: target,
    });
    events.push(EngineEvent::msg("teleport-monster"));

    events
}

// ---------------------------------------------------------------------------
// Branch teleport (via magic portal)
// ---------------------------------------------------------------------------

/// Teleport the entity through a magic portal to another branch.
///
/// Looks up the portal at the entity's current position.  If found,
/// transitions to the target branch/depth/position.  If no portal
/// exists, emits a "no portal" message.
pub fn branch_teleport(
    world: &mut GameWorld,
    entity: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let current_pos = match world.get_component::<Positioned>(entity) {
        Some(p) => p.0,
        None => return events,
    };

    let branch = world.dungeon().branch;
    let depth = world.dungeon().current_depth();

    let (target_branch, target_depth, target_pos) =
        match world.dungeon().find_portal(branch, depth, current_pos) {
            Some(dest) => dest,
            None => {
                events.push(EngineEvent::msg("teleport-no-portal"));
                return events;
            }
        };

    let from_str = format!("{:?}:{}", branch, depth);
    let to_str = format!("{:?}:{}", target_branch, target_depth);

    // Generate the target level.
    let generated = crate::map_gen::generate_level(target_depth as u8, rng);
    world.dungeon_mut().branch = target_branch;
    world
        .dungeon_mut()
        .set_current_level(generated.map, target_depth);

    // Place entity at the portal destination, or fall back to random walkable.
    let final_pos = if world
        .dungeon()
        .current_level
        .get(target_pos)
        .is_some_and(|c| is_passable(c.terrain))
    {
        target_pos
    } else {
        find_random_walkable(world, rng).unwrap_or(target_pos)
    };

    if let Some(mut p) = world.get_component_mut::<Positioned>(entity) {
        p.0 = final_pos;
    }

    events.push(EngineEvent::LevelChanged {
        entity,
        from_depth: from_str,
        to_depth: to_str,
    });
    events.push(EngineEvent::msg("teleport-branch"));

    events
}

// ---------------------------------------------------------------------------
// Teleport-trap handler
// ---------------------------------------------------------------------------

/// Handle stepping on a teleport trap (TrapType::TeleportTrap).
///
/// If the entity has teleport control, emits a message requesting target
/// selection (the caller handles the prompt).  Otherwise, performs a
/// random teleport.
pub fn handle_teleport_trap(
    world: &mut GameWorld,
    entity: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    if tele_restrict(world) {
        let mut events = Vec::new();
        events.push(EngineEvent::msg("teleport-trap-restricted"));
        return events;
    }

    if teleport_control_check(world, entity) {
        // Signal the caller that the player should pick a destination.
        let mut events = Vec::new();
        events.push(EngineEvent::msg("teleport-trap-controlled"));
        return events;
    }

    random_teleport(world, entity, rng)
}

/// Handle stepping on a level-teleport trap (TrapType::LevelTeleport).
///
/// Picks a random target depth and calls `level_teleport`.
pub fn handle_level_teleport_trap(
    world: &mut GameWorld,
    entity: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    if tele_restrict(world) {
        let mut events = Vec::new();
        events.push(EngineEvent::msg("teleport-trap-restricted"));
        return events;
    }

    let max_depth = world.dungeon().max_depth().max(1);
    let target = rng.random_range(1..=max_depth);
    level_teleport(world, entity, target, rng)
}

/// Handle stepping on a magic portal (TrapType::MagicPortal).
///
/// Delegates to `branch_teleport`.
pub fn handle_magic_portal(
    world: &mut GameWorld,
    entity: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    branch_teleport(world, entity, rng)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find a random walkable position on the current level.
///
/// Tries up to 1000 random positions, then falls back to a linear scan.
fn find_random_walkable(world: &GameWorld, rng: &mut impl Rng) -> Option<Position> {
    let level = &world.dungeon().current_level;
    let width = level.width;
    let height = level.height;

    // Fast path: random probing.
    for _ in 0..1000 {
        let x = rng.random_range(0..width);
        let y = rng.random_range(0..height);
        let pos = Position::new(x as i32, y as i32);
        if let Some(cell) = level.get(pos)
            && is_passable(cell.terrain)
        {
            return Some(pos);
        }
    }

    // Slow fallback: linear scan.
    for y in 0..height {
        for x in 0..width {
            let pos = Position::new(x as i32, y as i32);
            if let Some(cell) = level.get(pos)
                && is_passable(cell.terrain)
            {
                return Some(pos);
            }
        }
    }

    None
}

/// Find a random walkable position that is also free of traps and other
/// entities (excluding the given entity).
///
/// Tries up to 2000 random probes.  Returns `None` if no safe spot found.
fn find_safe_walkable(world: &GameWorld, exclude: Entity, rng: &mut impl Rng) -> Option<Position> {
    let level = &world.dungeon().current_level;
    let width = level.width;
    let height = level.height;
    let trap_map = &world.dungeon().trap_map;

    for _ in 0..2000 {
        let x = rng.random_range(0..width);
        let y = rng.random_range(0..height);
        let pos = Position::new(x as i32, y as i32);

        if let Some(cell) = level.get(pos)
            && is_passable(cell.terrain)
            && trap_map.trap_at(pos).is_none()
        {
            // Check no entity occupies this position.
            let occupied = world
                .query::<Positioned>()
                .iter()
                .any(|(e, p)| e != exclude && p.0 == pos);
            if !occupied {
                return Some(pos);
            }
        }
    }

    None
}

/// Check if a given terrain is water/lava that would require swimming.
pub fn is_liquid_terrain(terrain: Terrain) -> bool {
    matches!(
        terrain,
        Terrain::Water | Terrain::Pool | Terrain::Moat | Terrain::Lava
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::dungeon::{DungeonBranch, LevelMap, MapCell, Terrain};
    use crate::status::Intrinsics;
    use crate::world::GameWorld;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    /// Create a test world with a small floor-filled map.
    fn make_test_world_with_floor() -> GameWorld {
        let mut world = GameWorld::new(Position::new(5, 5));
        // Replace the default map with a small 20x10 floor map.
        let mut level = LevelMap::new(20, 10);
        for y in 0..10 {
            for x in 0..20 {
                level.cells[y][x] = MapCell {
                    terrain: Terrain::Floor,
                    ..MapCell::default()
                };
            }
        }
        world.dungeon_mut().current_level = level;
        world
    }

    #[test]
    fn test_random_teleport_moves_entity() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        let before = world.get_component::<Positioned>(player).unwrap().0;

        let events = random_teleport(&mut world, player, &mut rng);

        let after = world.get_component::<Positioned>(player).unwrap().0;

        // Entity should have moved (overwhelming probability on a 20x10 map).
        // We check that the event was emitted correctly.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::EntityTeleported { from, to, .. }
            if *from == before
        )));

        // The entity's position should match the `to` in the event.
        let teleport_event = events
            .iter()
            .find(|e| matches!(e, EngineEvent::EntityTeleported { .. }))
            .unwrap();
        if let EngineEvent::EntityTeleported { to, .. } = teleport_event {
            assert_eq!(*to, after);
        }
    }

    #[test]
    fn test_controlled_teleport_to_target() {
        let mut world = make_test_world_with_floor();
        let player = world.player();

        let target = Position::new(10, 5);
        let events = controlled_teleport(&mut world, player, target);

        let pos = world.get_component::<Positioned>(player).unwrap().0;
        assert_eq!(pos, target);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::EntityTeleported { to, .. } if *to == target
        )));
    }

    #[test]
    fn test_controlled_teleport_invalid_wall() {
        let mut world = make_test_world_with_floor();
        let player = world.player();

        // Place a wall at a specific position.
        world.dungeon_mut().current_level.cells[3][15] = MapCell {
            terrain: Terrain::Wall,
            ..MapCell::default()
        };

        let before = world.get_component::<Positioned>(player).unwrap().0;

        let target = Position::new(15, 3);
        let events = controlled_teleport(&mut world, player, target);

        // Should not have moved.
        let after = world.get_component::<Positioned>(player).unwrap().0;
        assert_eq!(before, after);

        // Should emit invalid target message.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-invalid-target"
        )));
    }

    #[test]
    fn test_controlled_teleport_out_of_bounds() {
        let mut world = make_test_world_with_floor();
        let player = world.player();

        let before = world.get_component::<Positioned>(player).unwrap().0;

        let target = Position::new(100, 100); // Way out of bounds.
        let events = controlled_teleport(&mut world, player, target);

        let after = world.get_component::<Positioned>(player).unwrap().0;
        assert_eq!(before, after);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-invalid-target"
        )));
    }

    #[test]
    fn test_teleportitis_probability() {
        // Verify that teleportitis triggers approximately 1/85 of the time.
        let mut world = make_test_world_with_floor();
        let player = world.player();

        // Grant teleportitis intrinsic.
        if let Some(mut intr) = world.get_component_mut::<Intrinsics>(player) {
            intr.teleportitis = true;
        }

        let mut triggers = 0u32;
        let trials = 85_000u32;
        let mut rng = test_rng();

        for _ in 0..trials {
            if teleportitis_check(&world, player, &mut rng) {
                triggers += 1;
            }
        }

        // Expected ~1000 triggers (85000/85).  Allow wide tolerance.
        let expected = trials / TELEPORTITIS_CHANCE;
        let lower = expected / 2;
        let upper = expected * 2;
        assert!(
            triggers >= lower && triggers <= upper,
            "teleportitis triggers {} times in {} trials, expected ~{}",
            triggers,
            trials,
            expected
        );
    }

    #[test]
    fn test_teleportitis_without_intrinsic() {
        let world = make_test_world_with_floor();
        let player = world.player();
        let mut rng = test_rng();

        // Without the intrinsic, should never trigger.
        for _ in 0..1000 {
            assert!(!teleportitis_check(&world, player, &mut rng));
        }
    }

    #[test]
    fn test_teleport_control_overrides_random() {
        let mut world = make_test_world_with_floor();
        let player = world.player();

        // Without teleport control.
        assert!(!teleport_control_check(&world, player));

        // Grant teleport control.
        if let Some(mut intr) = world.get_component_mut::<Intrinsics>(player) {
            intr.teleport_control = true;
        }

        assert!(teleport_control_check(&world, player));

        // When control is active, the caller would route to
        // controlled_teleport instead of random_teleport.
        // Verify the controlled path works.
        let target = Position::new(8, 4);
        let events = controlled_teleport(&mut world, player, target);
        let pos = world.get_component::<Positioned>(player).unwrap().0;
        assert_eq!(pos, target);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::EntityTeleported { .. }))
        );
    }

    #[test]
    fn test_level_teleport_changes_depth() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        // Set max depth so we can teleport deeper.
        world.dungeon_mut().set_max_depth(10);

        let events = level_teleport(&mut world, player, 5, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::LevelChanged { to_depth, .. }
            if to_depth == "Dlvl:5"
        )));

        assert_eq!(world.dungeon().current_depth(), 5);
    }

    #[test]
    fn test_level_teleport_clamps_depth() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        world.dungeon_mut().set_max_depth(10);

        // Try to teleport beyond max depth.
        let events = level_teleport(&mut world, player, 99, &mut rng);

        // Should clamp to max depth.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::LevelChanged { to_depth, .. }
            if to_depth == "Dlvl:10"
        )));
    }

    #[test]
    fn test_level_teleport_same_level() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        let current = world.dungeon().current_depth();
        let events = level_teleport(&mut world, player, current, &mut rng);

        // Should not change level.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-same-level"
        )));
    }

    // ═══════════════════════════════════════════════════════════════
    // New tests: tele_restrict, safe_teleport, branch_teleport,
    //            handle_teleport_trap, handle_level_teleport_trap,
    //            handle_magic_portal, find_safe_walkable
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_tele_restrict_sokoban() {
        let mut world = make_test_world_with_floor();
        world.dungeon_mut().branch = DungeonBranch::Sokoban;
        assert!(tele_restrict(&world));
    }

    #[test]
    fn test_tele_restrict_endgame() {
        let mut world = make_test_world_with_floor();
        world.dungeon_mut().branch = DungeonBranch::Endgame;
        assert!(tele_restrict(&world));
    }

    #[test]
    fn test_tele_restrict_main_allowed() {
        let world = make_test_world_with_floor();
        // Main branch does not restrict teleport.
        assert!(!tele_restrict(&world));
    }

    #[test]
    fn test_safe_teleport_moves_entity() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        let before = world.get_component::<Positioned>(player).unwrap().0;

        let events = safe_teleport(&mut world, player, &mut rng);

        let after = world.get_component::<Positioned>(player).unwrap().0;

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::EntityTeleported { .. }))
        );
        // Should have moved (overwhelming probability).
        assert_ne!(before, after);
    }

    #[test]
    fn test_safe_teleport_blocked_in_sokoban() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        world.dungeon_mut().branch = DungeonBranch::Sokoban;

        let before = world.get_component::<Positioned>(player).unwrap().0;

        let events = safe_teleport(&mut world, player, &mut rng);

        let after = world.get_component::<Positioned>(player).unwrap().0;

        // Should not have moved.
        assert_eq!(before, after);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-restricted"
        )));
    }

    #[test]
    fn test_handle_teleport_trap_without_control() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        let events = handle_teleport_trap(&mut world, player, &mut rng);

        // Without teleport control, should do a random teleport.
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::EntityTeleported { .. }))
        );
    }

    #[test]
    fn test_handle_teleport_trap_with_control() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        // Grant teleport control.
        if let Some(mut intr) = world.get_component_mut::<Intrinsics>(player) {
            intr.teleport_control = true;
        }

        let events = handle_teleport_trap(&mut world, player, &mut rng);

        // Should emit controlled message, not actually teleport.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-trap-controlled"
        )));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, EngineEvent::EntityTeleported { .. }))
        );
    }

    #[test]
    fn test_handle_teleport_trap_restricted() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        world.dungeon_mut().branch = DungeonBranch::Sokoban;

        let events = handle_teleport_trap(&mut world, player, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-trap-restricted"
        )));
    }

    #[test]
    fn test_handle_level_teleport_trap() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        world.dungeon_mut().set_max_depth(10);

        let events = handle_level_teleport_trap(&mut world, player, &mut rng);

        // Should change level.
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::LevelChanged { .. }))
                || events.iter().any(|e| matches!(
                    e,
                    EngineEvent::Message { key, .. } if key == "teleport-same-level"
                ))
        );
    }

    #[test]
    fn test_handle_level_teleport_trap_restricted() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        world.dungeon_mut().branch = DungeonBranch::Endgame;

        let events = handle_level_teleport_trap(&mut world, player, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-trap-restricted"
        )));
    }

    #[test]
    fn test_branch_teleport_no_portal() {
        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        let events = branch_teleport(&mut world, player, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-no-portal"
        )));
    }

    #[test]
    fn test_branch_teleport_with_portal() {
        use crate::dungeon::PortalLink;

        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        // Player is at (5,5) on Main:1.
        // Add a portal at (5,5) going to Quest:1 at (3,3).
        world.dungeon_mut().add_portal(PortalLink {
            from_branch: DungeonBranch::Main,
            from_depth: 1,
            from_pos: Position::new(5, 5),
            to_branch: DungeonBranch::Quest,
            to_depth: 1,
            to_pos: Position::new(3, 3),
        });

        let events = branch_teleport(&mut world, player, &mut rng);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::LevelChanged { .. }))
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-branch"
        )));
        assert_eq!(world.dungeon().branch, DungeonBranch::Quest);
    }

    #[test]
    fn test_handle_magic_portal() {
        use crate::dungeon::PortalLink;

        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        let player = world.player();

        // Add portal at player position.
        world.dungeon_mut().add_portal(PortalLink {
            from_branch: DungeonBranch::Main,
            from_depth: 1,
            from_pos: Position::new(5, 5),
            to_branch: DungeonBranch::FortLudios,
            to_depth: 1,
            to_pos: Position::new(10, 10),
        });

        let events = handle_magic_portal(&mut world, player, &mut rng);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::LevelChanged { .. }))
        );
        assert_eq!(world.dungeon().branch, DungeonBranch::FortLudios);
    }

    #[test]
    fn test_is_liquid_terrain() {
        assert!(is_liquid_terrain(Terrain::Water));
        assert!(is_liquid_terrain(Terrain::Pool));
        assert!(is_liquid_terrain(Terrain::Moat));
        assert!(is_liquid_terrain(Terrain::Lava));
        assert!(!is_liquid_terrain(Terrain::Floor));
        assert!(!is_liquid_terrain(Terrain::Ice));
    }

    #[test]
    fn test_teleport_monster_moves() {
        use crate::world::{HitPoints, Monster, Name, Speed};

        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();

        let monster = world.spawn((
            Monster,
            Positioned(Position::new(3, 3)),
            HitPoints {
                current: 10,
                max: 10,
            },
            Speed(12),
            Name("goblin".to_string()),
        ));

        let events = teleport_monster(&mut world, monster, &mut rng);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::EntityTeleported { .. }))
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-monster"
        )));
    }

    #[test]
    fn test_teleport_monster_blocked_in_sokoban() {
        use crate::world::{HitPoints, Monster, Name, Speed};

        let mut world = make_test_world_with_floor();
        let mut rng = test_rng();
        world.dungeon_mut().branch = DungeonBranch::Sokoban;

        let monster = world.spawn((
            Monster,
            Positioned(Position::new(3, 3)),
            HitPoints {
                current: 10,
                max: 10,
            },
            Speed(12),
            Name("goblin".to_string()),
        ));

        let events = teleport_monster(&mut world, monster, &mut rng);

        // Blocked on Sokoban.
        assert!(events.is_empty());
    }
}
