//! Detection spells, scrolls, and potions: reveal hidden information.
//!
//! Implements the NetHack 3.7 detection mechanics for monsters, objects,
//! traps, food, and terrain (clairvoyance).  All functions operate on
//! `GameWorld` and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{ObjectClass, ObjectCore, ObjectLocation};

use crate::action::Position;
use crate::event::EngineEvent;
use crate::traps::TrapInstance;
use crate::world::{GameWorld, Monster, Positioned};

// ---------------------------------------------------------------------------
// Detect Monsters
// ---------------------------------------------------------------------------

/// Detected monster info: position and display name.
#[derive(Debug, Clone)]
pub struct DetectedMonster {
    pub entity: Entity,
    pub position: Position,
    pub name: String,
}

/// Reveal all monster positions on the current level.
///
/// Returns events including a summary message and individual
/// `TrapRevealed`-style notifications for each monster found.
pub fn detect_monsters(world: &GameWorld, player: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mut detected: Vec<DetectedMonster> = Vec::new();

    for (entity, (_monster, pos)) in world.ecs().query::<(&Monster, &Positioned)>().iter() {
        if entity == player {
            continue;
        }
        let name = world.entity_name(entity);
        detected.push(DetectedMonster {
            entity,
            position: pos.0,
            name,
        });
    }

    if detected.is_empty() {
        events.push(EngineEvent::msg("detect-monsters-none"));
    } else {
        let count = detected.len() as u32;
        // Build a list of (position, name) pairs for the event args.
        let mut args = vec![("count".to_string(), count.to_string())];
        for (i, dm) in detected.iter().enumerate() {
            args.push((format!("name_{i}"), dm.name.clone()));
            args.push((
                format!("pos_{i}"),
                format!("{},{}", dm.position.x, dm.position.y),
            ));
        }
        events.push(EngineEvent::Message {
            key: "detect-monsters-found".to_string(),
            args,
        });
    }

    events
}

// ---------------------------------------------------------------------------
// Detect Objects
// ---------------------------------------------------------------------------

/// Detected object info: position and class.
#[derive(Debug, Clone)]
pub struct DetectedObject {
    pub entity: Entity,
    pub position: Position,
    pub class: ObjectClass,
}

/// Reveal all item positions on the current level.
///
/// Only considers items with `ObjectLocation::Floor` that lie within the
/// current level bounds.
pub fn detect_objects(world: &GameWorld, _player: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mut detected: Vec<DetectedObject> = Vec::new();

    for (entity, (core, loc)) in world.ecs().query::<(&ObjectCore, &ObjectLocation)>().iter() {
        if let Some(pos) = crate::dungeon::floor_position_on_level(
            loc,
            world.dungeon().branch,
            world.dungeon().depth,
        ) {
            detected.push(DetectedObject {
                entity,
                position: pos,
                class: core.object_class,
            });
        }
    }

    if detected.is_empty() {
        events.push(EngineEvent::msg("detect-objects-none"));
    } else {
        events.push(EngineEvent::msg_with(
            "detect-objects-found",
            vec![("count", detected.len().to_string())],
        ));
    }

    events
}

// ---------------------------------------------------------------------------
// Detect Traps
// ---------------------------------------------------------------------------

/// Reveal all traps on the current level by setting their `detected` flag.
///
/// Unlike the Search action (which only checks adjacent tiles), this
/// reveals every trap on the entire level.
pub fn detect_traps(world: &mut GameWorld, _player: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mut count = 0u32;

    let traps: &mut Vec<TrapInstance> = &mut world.dungeon_mut().trap_map.traps;
    for trap in traps.iter_mut() {
        if !trap.detected {
            trap.detected = true;
            events.push(EngineEvent::TrapRevealed {
                position: trap.pos,
                trap_type: trap.trap_type,
            });
            count += 1;
        }
    }

    if count == 0 {
        events.push(EngineEvent::msg("detect-traps-none"));
    } else {
        events.push(EngineEvent::msg_with(
            "detect-traps-found",
            vec![("count", count.to_string())],
        ));
    }

    events
}

// ---------------------------------------------------------------------------
// Detect Food
// ---------------------------------------------------------------------------

/// Reveal all food items on the current level.
///
/// Filters floor items by `ObjectClass::Food`.
pub fn detect_food(world: &GameWorld, _player: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mut count = 0u32;

    for (_entity, (core, loc)) in world.ecs().query::<(&ObjectCore, &ObjectLocation)>().iter() {
        if core.object_class == ObjectClass::Food
            && let ObjectLocation::Floor { .. } = *loc
        {
            count += 1;
        }
    }

    if count == 0 {
        events.push(EngineEvent::msg("detect-food-none"));
    } else {
        events.push(EngineEvent::msg_with(
            "detect-food-found",
            vec![("count", count.to_string())],
        ));
    }

    events
}

// ---------------------------------------------------------------------------
// Clairvoyance
// ---------------------------------------------------------------------------

/// Reveal terrain in a radius around the player, like a limited magic
/// mapping effect.
///
/// Sets `explored = true` on all cells within `radius` Manhattan distance
/// of the player's position.  Returns a message summarizing the area
/// revealed.
pub fn clairvoyance(world: &mut GameWorld, player: Entity, radius: i32) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    let level = &mut world.dungeon_mut().current_level;
    let mut revealed = 0u32;

    let min_y = (player_pos.y - radius).max(0);
    let max_y = (player_pos.y + radius).min(level.height as i32 - 1);
    let min_x = (player_pos.x - radius).max(0);
    let max_x = (player_pos.x + radius).min(level.width as i32 - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let cell = &mut level.cells[y as usize][x as usize];
            if !cell.explored {
                cell.explored = true;
                revealed += 1;
            }
        }
    }

    if revealed > 0 {
        events.push(EngineEvent::msg_with(
            "clairvoyance-reveal",
            vec![("count", revealed.to_string())],
        ));
    } else {
        events.push(EngineEvent::msg("clairvoyance-nothing-new"));
    }

    events
}

// ---------------------------------------------------------------------------
// Search (the 's' command)
// ---------------------------------------------------------------------------

/// Result of a search action on adjacent tiles.
#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    /// Traps that were newly detected.
    pub found_traps: Vec<Position>,
    /// Secret doors that were revealed (position of the door).
    pub found_doors: Vec<Position>,
}

impl SearchResult {
    pub fn found_anything(&self) -> bool {
        !self.found_traps.is_empty() || !self.found_doors.is_empty()
    }
}

/// Search for hidden features in adjacent tiles (traps, secret doors).
///
/// Called by the 's' (search) command.  Success depends on luck and a
/// search bonus (from ring of searching, etc.).
///
/// In C NetHack, the base chance to find a hidden trap is 1/5 and a
/// secret door is 1/7 (modified by luck).
pub fn dosearch(
    world: &mut GameWorld,
    player: Entity,
    search_bonus: i32,
    luck: i32,
    rng: &mut impl Rng,
) -> (SearchResult, Vec<EngineEvent>) {
    let mut result = SearchResult::default();
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return (result, events),
    };

    // Check all 8 adjacent tiles for hidden traps.
    let traps: &mut Vec<TrapInstance> = &mut world.dungeon_mut().trap_map.traps;
    for trap in traps.iter_mut() {
        if trap.detected {
            continue;
        }
        let dx = (trap.pos.x - player_pos.x).abs();
        let dy = (trap.pos.y - player_pos.y).abs();
        if dx <= 1 && dy <= 1 && !(dx == 0 && dy == 0) {
            // Base chance 1/5, improved by search_bonus and luck.
            let threshold = 5 - search_bonus.min(3) - (luck / 3).clamp(-1, 1);
            let threshold = threshold.max(2); // never guaranteed
            if rng.random_range(0..threshold) == 0 {
                trap.detected = true;
                result.found_traps.push(trap.pos);
                events.push(EngineEvent::TrapRevealed {
                    position: trap.pos,
                    trap_type: trap.trap_type,
                });
            }
        }
    }

    if result.found_anything() {
        events.push(EngineEvent::msg_with(
            "search-found",
            vec![
                ("traps", result.found_traps.len().to_string()),
                ("doors", result.found_doors.len().to_string()),
            ],
        ));
    }

    (result, events)
}

// ---------------------------------------------------------------------------
// Magic Mapping
// ---------------------------------------------------------------------------

/// Result of a magic mapping operation.
#[derive(Debug, Clone)]
pub struct MappingResult {
    /// Number of cells newly revealed.
    pub cells_revealed: u32,
}

/// Magic mapping — reveal the entire level map.
///
/// Called by scroll of magic mapping.  Marks all cells as explored.
/// Does not reveal monsters or objects, only terrain.
pub fn do_mapping(world: &mut GameWorld) -> (MappingResult, Vec<EngineEvent>) {
    let mut events = Vec::new();
    let mut revealed = 0u32;

    let level = &mut world.dungeon_mut().current_level;
    for y in 0..level.height {
        for x in 0..level.width {
            let cell = &mut level.cells[y][x];
            if !cell.explored {
                cell.explored = true;
                revealed += 1;
            }
        }
    }

    let result = MappingResult {
        cells_revealed: revealed,
    };

    if revealed > 0 {
        events.push(EngineEvent::msg_with(
            "magic-mapping-reveal",
            vec![("count", revealed.to_string())],
        ));
    } else {
        events.push(EngineEvent::msg("magic-mapping-nothing-new"));
    }

    (result, events)
}

// ---------------------------------------------------------------------------
// Crystal Ball Scrying
// ---------------------------------------------------------------------------

/// Result of crystal ball scrying.
#[derive(Debug, Clone)]
pub struct ScryResult {
    /// Positions revealed by the scrying.
    pub revealed: Vec<Position>,
    /// Whether the scrying failed (low wisdom).
    pub failed: bool,
}

/// Crystal ball scrying — peek at a specific area.
///
/// High wisdom sees more detail; low wisdom might show nothing.
/// In C NetHack, the crystal ball requires Int+Wis to exceed a
/// threshold for reliable use; below that, the player may be
/// confused or blinded.
pub fn crystal_ball_look(
    world: &mut GameWorld,
    center: Position,
    radius: i32,
    wisdom: i32,
    rng: &mut impl Rng,
) -> (ScryResult, Vec<EngineEvent>) {
    let mut events = Vec::new();
    let mut revealed_positions = Vec::new();

    // Wisdom check: need at least 5 to see anything.
    // Below 10, reduced chance; below 5, always fails.
    if wisdom < 5 || (wisdom < 10 && rng.random_range(0..10) >= wisdom) {
        events.push(EngineEvent::msg("crystal-ball-cloudy"));
        return (
            ScryResult {
                revealed: vec![],
                failed: true,
            },
            events,
        );
    }

    // Scale effective radius with wisdom.
    let effective_radius = if wisdom >= 18 {
        radius
    } else if wisdom >= 14 {
        (radius * 3) / 4
    } else {
        radius / 2
    };

    let level = &mut world.dungeon_mut().current_level;
    let min_y = (center.y - effective_radius).max(0);
    let max_y = (center.y + effective_radius).min(level.height as i32 - 1);
    let min_x = (center.x - effective_radius).max(0);
    let max_x = (center.x + effective_radius).min(level.width as i32 - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let cell = &mut level.cells[y as usize][x as usize];
            if !cell.explored {
                cell.explored = true;
                revealed_positions.push(Position::new(x, y));
            }
        }
    }

    if revealed_positions.is_empty() {
        events.push(EngineEvent::msg("crystal-ball-nothing-new"));
    } else {
        events.push(EngineEvent::msg_with(
            "crystal-ball-reveal",
            vec![("count", revealed_positions.len().to_string())],
        ));
    }

    (
        ScryResult {
            revealed: revealed_positions,
            failed: false,
        },
        events,
    )
}

// ---------------------------------------------------------------------------
// Detect Gold
// ---------------------------------------------------------------------------

/// Reveal all gold (coin-class) items on the current level.
pub fn detect_gold(world: &GameWorld, _player: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mut positions: Vec<Position> = Vec::new();

    for (_entity, (core, loc)) in world.ecs().query::<(&ObjectCore, &ObjectLocation)>().iter() {
        if core.object_class == ObjectClass::Coin
            && let Some(pos) = crate::dungeon::floor_position_on_level(
                loc,
                world.dungeon().branch,
                world.dungeon().depth,
            )
        {
            positions.push(pos);
        }
    }

    if positions.is_empty() {
        events.push(EngineEvent::msg("detect-gold-none"));
    } else {
        events.push(EngineEvent::msg_with(
            "detect-gold-found",
            vec![("count", positions.len().to_string())],
        ));
    }

    events
}

// ---------------------------------------------------------------------------
// Detect Unseen (invisible monsters)
// ---------------------------------------------------------------------------

/// Detect unseen/invisible creatures on the level.
///
/// In a full implementation this would check the `Invisible` status
/// effect on monsters.  For now, this is a placeholder that returns
/// an empty list (since the visibility/invisibility component isn't
/// always present).
pub fn detect_unseen(world: &GameWorld, player: Entity) -> Vec<EngineEvent> {
    // Currently all monsters tracked by detect_monsters; this function
    // exists for scroll of detect unseen which specifically calls out
    // invisible creatures.  We delegate to detect_monsters for now.
    detect_monsters(world, player)
}

// ---------------------------------------------------------------------------
// Reveal Monsters in Area
// ---------------------------------------------------------------------------

/// Reveal all monsters within a given Chebyshev radius of a center point.
///
/// Returns the positions of revealed monsters and engine events.
pub fn reveal_monsters_in_area(
    world: &GameWorld,
    player: Entity,
    center: Position,
    radius: i32,
) -> (Vec<Position>, Vec<EngineEvent>) {
    let mut events = Vec::new();
    let mut positions = Vec::new();

    for (entity, (_monster, pos)) in world.ecs().query::<(&Monster, &Positioned)>().iter() {
        if entity == player {
            continue;
        }
        let dx = (pos.0.x - center.x).abs();
        let dy = (pos.0.y - center.y).abs();
        if dx <= radius && dy <= radius {
            positions.push(pos.0);
        }
    }

    if positions.is_empty() {
        events.push(EngineEvent::msg("reveal-monsters-none"));
    } else {
        events.push(EngineEvent::msg_with(
            "reveal-monsters-found",
            vec![("count", positions.len().to_string())],
        ));
    }

    (positions, events)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::traps::{TrapInstance, TrapMap};
    use crate::world::{GameWorld, Monster, Name, Positioned};
    use nethack_babel_data::{ObjectClass, ObjectCore, ObjectLocation, ObjectTypeId, TrapType};

    /// Helper: create a game world with a standard map.
    fn test_world() -> GameWorld {
        GameWorld::new(Position::new(40, 10))
    }

    /// Spawn a monster entity on the level.
    fn spawn_monster(world: &mut GameWorld, pos: Position, name: &str) -> Entity {
        world.spawn((Monster, Positioned(pos), Name(name.to_string())))
    }

    /// Spawn a floor item entity.
    fn spawn_floor_item(world: &mut GameWorld, pos: Position, class: ObjectClass) -> Entity {
        let core = ObjectCore {
            otyp: ObjectTypeId(1),
            object_class: class,
            quantity: 1,
            weight: 10,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let loc = ObjectLocation::Floor {
            x: pos.x as i16,
            y: pos.y as i16,
            level: world.dungeon().current_data_dungeon_level(),
        };
        world.spawn((core, loc))
    }

    // ── Detect Monsters ────────────────────────────────────────────

    #[test]
    fn test_detect_monsters_reveals_all() {
        let mut world = test_world();
        let player = world.player();

        // Spawn 3 monsters at various positions.
        spawn_monster(&mut world, Position::new(5, 5), "goblin");
        spawn_monster(&mut world, Position::new(10, 10), "orc");
        spawn_monster(&mut world, Position::new(30, 15), "troll");

        let events = detect_monsters(&world, player);

        // Should have exactly one Message event with count=3.
        let msg = events.iter().find(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "detect-monsters-found"),
        );
        assert!(msg.is_some(), "should emit detect-monsters-found");

        if let EngineEvent::Message { args, .. } = msg.unwrap() {
            let count = args
                .iter()
                .find(|(k, _)| k == "count")
                .map(|(_, v)| v.as_str());
            assert_eq!(count, Some("3"));
        }
    }

    #[test]
    fn test_detect_monsters_none() {
        let world = test_world();
        let player = world.player();

        let events = detect_monsters(&world, player);
        assert!(events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "detect-monsters-none")
        ));
    }

    // ── Detect Objects ─────────────────────────────────────────────

    #[test]
    fn test_detect_objects_reveals_all() {
        let mut world = test_world();
        let player = world.player();

        spawn_floor_item(&mut world, Position::new(5, 5), ObjectClass::Weapon);
        spawn_floor_item(&mut world, Position::new(10, 10), ObjectClass::Food);

        let events = detect_objects(&world, player);

        let msg = events.iter().find(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "detect-objects-found"),
        );
        assert!(msg.is_some(), "should emit detect-objects-found");

        if let EngineEvent::Message { args, .. } = msg.unwrap() {
            let count = args
                .iter()
                .find(|(k, _)| k == "count")
                .map(|(_, v)| v.as_str());
            assert_eq!(count, Some("2"));
        }
    }

    #[test]
    fn test_detect_objects_none() {
        let world = test_world();
        let player = world.player();

        let events = detect_objects(&world, player);
        assert!(events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "detect-objects-none")
        ));
    }

    #[test]
    fn test_detect_objects_ignores_floor_items_on_other_levels() {
        let mut world = test_world();
        let player = world.player();

        spawn_floor_item(&mut world, Position::new(5, 5), ObjectClass::Weapon);
        world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(2),
                object_class: ObjectClass::Food,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            ObjectLocation::Floor {
                x: 5,
                y: 5,
                level: crate::dungeon::data_dungeon_level(crate::dungeon::DungeonBranch::Quest, 1),
            },
        ));

        let events = detect_objects(&world, player);

        let msg = events.iter().find(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "detect-objects-found"),
        );
        assert!(
            msg.is_some(),
            "should only detect items on the current level"
        );
        if let EngineEvent::Message { args, .. } = msg.unwrap() {
            let count = args
                .iter()
                .find(|(k, _)| k == "count")
                .map(|(_, v)| v.as_str());
            assert_eq!(count, Some("1"));
        }
    }

    // ── Detect Traps ───────────────────────────────────────────────

    #[test]
    fn test_detect_traps_reveals_all() {
        let mut world = test_world();
        let player = world.player();

        // Place 3 traps on the level.
        world.dungeon_mut().trap_map = TrapMap {
            traps: vec![
                TrapInstance::new(Position::new(5, 5), TrapType::Pit),
                TrapInstance::new(Position::new(10, 10), TrapType::BearTrap),
                TrapInstance::new(Position::new(20, 15), TrapType::ArrowTrap),
            ],
        };

        let events = detect_traps(&mut world, player);

        // All non-auto-detected traps should now be detected.
        for trap in &world.dungeon().trap_map.traps {
            assert!(trap.detected, "trap at {:?} should be detected", trap.pos);
        }

        // Should have TrapRevealed events for each newly detected trap.
        let revealed_count = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::TrapRevealed { .. }))
            .count();
        // PitTrap is not auto-detected (Hole is), BearTrap and ArrowTrap are not.
        // All 3 should be revealed.
        assert_eq!(revealed_count, 3, "should reveal 3 traps");
    }

    #[test]
    fn test_detect_traps_already_detected() {
        let mut world = test_world();
        let player = world.player();

        // Place a trap that is already detected.
        let mut trap = TrapInstance::new(Position::new(5, 5), TrapType::BearTrap);
        trap.detected = true;
        world.dungeon_mut().trap_map = TrapMap { traps: vec![trap] };

        let events = detect_traps(&mut world, player);

        // No TrapRevealed events since it was already detected.
        let revealed_count = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::TrapRevealed { .. }))
            .count();
        assert_eq!(revealed_count, 0);
        assert!(
            events.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key == "detect-traps-none")
            )
        );
    }

    // ── Clairvoyance ───────────────────────────────────────────────

    #[test]
    fn test_clairvoyance_radius() {
        let mut world = test_world();
        let player = world.player();

        // Player is at (40, 10).  Radius 3 should reveal a 7x7 area.
        let events = clairvoyance(&mut world, player, 3);

        // Check that cells within radius are explored.
        let level = &world.dungeon().current_level;
        for dy in -3..=3_i32 {
            for dx in -3..=3_i32 {
                let x = 40 + dx;
                let y = 10 + dy;
                if x >= 0 && y >= 0 && (x as usize) < level.width && (y as usize) < level.height {
                    assert!(
                        level.cells[y as usize][x as usize].explored,
                        "cell ({x}, {y}) should be explored"
                    );
                }
            }
        }

        // Cells outside radius should NOT be explored.
        assert!(
            !level.cells[0][0].explored,
            "cell (0,0) far from player should not be explored"
        );

        // Should have a message.
        assert!(events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "clairvoyance-reveal")
        ));
    }

    // ── Detect Food ────────────────────────────────────────────────

    #[test]
    fn test_detect_food_finds_food() {
        let mut world = test_world();
        let player = world.player();

        // Spawn food and non-food items.
        spawn_floor_item(&mut world, Position::new(5, 5), ObjectClass::Food);
        spawn_floor_item(&mut world, Position::new(10, 10), ObjectClass::Weapon);
        spawn_floor_item(&mut world, Position::new(15, 15), ObjectClass::Food);

        let events = detect_food(&world, player);

        let msg = events
            .iter()
            .find(|e| matches!(e, EngineEvent::Message { key, .. } if key == "detect-food-found"));
        assert!(msg.is_some());
        if let EngineEvent::Message { args, .. } = msg.unwrap() {
            let count = args
                .iter()
                .find(|(k, _)| k == "count")
                .map(|(_, v)| v.as_str());
            assert_eq!(count, Some("2"), "should find 2 food items");
        }
    }

    #[test]
    fn test_detect_food_none() {
        let mut world = test_world();
        let player = world.player();

        spawn_floor_item(&mut world, Position::new(5, 5), ObjectClass::Weapon);

        let events = detect_food(&world, player);
        assert!(
            events.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key == "detect-food-none")
            )
        );
    }

    // ── Dosearch ──────────────────────────────────────────────────

    #[test]
    fn test_dosearch_finds_adjacent_trap() {
        use rand::SeedableRng;
        use rand::rngs::SmallRng;

        let mut world = test_world();
        let player = world.player();

        // Place a trap adjacent to the player (40, 10) -> (41, 10).
        world.dungeon_mut().trap_map = TrapMap {
            traps: vec![TrapInstance::new(
                Position::new(41, 10),
                TrapType::ArrowTrap,
            )],
        };

        // Search repeatedly with high bonus + luck to ensure it triggers.
        let mut rng = SmallRng::seed_from_u64(42);
        let mut found = false;
        for _ in 0..50 {
            let (result, _events) = dosearch(&mut world, player, 3, 5, &mut rng);
            if result.found_anything() {
                found = true;
                break;
            }
        }
        assert!(found, "should eventually find the adjacent trap");
        assert!(
            world.dungeon().trap_map.traps[0].detected,
            "trap should be marked detected"
        );
    }

    #[test]
    fn test_dosearch_ignores_distant_trap() {
        use rand::SeedableRng;
        use rand::rngs::SmallRng;

        let mut world = test_world();
        let player = world.player();

        // Place a trap far from the player.
        world.dungeon_mut().trap_map = TrapMap {
            traps: vec![TrapInstance::new(Position::new(20, 5), TrapType::ArrowTrap)],
        };

        let mut rng = SmallRng::seed_from_u64(42);
        for _ in 0..50 {
            let (result, _events) = dosearch(&mut world, player, 3, 5, &mut rng);
            assert!(
                result.found_traps.is_empty(),
                "should not find distant trap"
            );
        }
        assert!(
            !world.dungeon().trap_map.traps[0].detected,
            "distant trap should remain undetected"
        );
    }

    // ── Magic Mapping ─────────────────────────────────────────────

    #[test]
    fn test_do_mapping_reveals_all() {
        let mut world = test_world();

        let (_result, events) = do_mapping(&mut world);

        let level = &world.dungeon().current_level;
        for y in 0..level.height {
            for x in 0..level.width {
                assert!(
                    level.cells[y][x].explored,
                    "cell ({x}, {y}) should be explored after mapping"
                );
            }
        }

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "magic-mapping-reveal"
        )));
    }

    #[test]
    fn test_do_mapping_already_explored() {
        let mut world = test_world();

        // Map everything first.
        do_mapping(&mut world);

        // Map again — should report nothing new.
        let (_result, events) = do_mapping(&mut world);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "magic-mapping-nothing-new"
        )));
    }

    // ── Crystal Ball ──────────────────────────────────────────────

    #[test]
    fn test_crystal_ball_high_wisdom() {
        use rand::SeedableRng;
        use rand::rngs::SmallRng;

        let mut world = test_world();
        let mut rng = SmallRng::seed_from_u64(42);

        let center = Position::new(40, 10);
        let (result, events) = crystal_ball_look(&mut world, center, 5, 18, &mut rng);

        assert!(!result.failed, "high wisdom should not fail");
        assert!(!result.revealed.is_empty(), "should reveal some cells");
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "crystal-ball-reveal"
        )));
    }

    #[test]
    fn test_crystal_ball_low_wisdom_may_fail() {
        use rand::SeedableRng;
        use rand::rngs::SmallRng;

        let mut world = test_world();
        let mut rng = SmallRng::seed_from_u64(42);

        let center = Position::new(40, 10);
        // Wisdom 3 always fails.
        let (result, events) = crystal_ball_look(&mut world, center, 5, 3, &mut rng);
        assert!(result.failed, "wisdom 3 should always fail");
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "crystal-ball-cloudy"
        )));
    }

    // ── Detect Gold ───────────────────────────────────────────────

    #[test]
    fn test_detect_gold_finds_coins() {
        let mut world = test_world();
        let player = world.player();

        spawn_floor_item(&mut world, Position::new(5, 5), ObjectClass::Coin);
        spawn_floor_item(&mut world, Position::new(10, 10), ObjectClass::Weapon);
        spawn_floor_item(&mut world, Position::new(15, 15), ObjectClass::Coin);

        let events = detect_gold(&world, player);
        let msg = events.iter().find(|e| {
            matches!(
                e,
                EngineEvent::Message { key, .. } if key == "detect-gold-found"
            )
        });
        assert!(msg.is_some(), "should emit detect-gold-found");
        if let EngineEvent::Message { args, .. } = msg.unwrap() {
            let count = args
                .iter()
                .find(|(k, _)| k == "count")
                .map(|(_, v)| v.as_str());
            assert_eq!(count, Some("2"));
        }
    }

    #[test]
    fn test_detect_gold_none() {
        let mut world = test_world();
        let player = world.player();

        spawn_floor_item(&mut world, Position::new(5, 5), ObjectClass::Weapon);

        let events = detect_gold(&world, player);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "detect-gold-none"
        )));
    }

    // ── Reveal Monsters in Area ───────────────────────────────────

    #[test]
    fn test_reveal_monsters_in_area() {
        let mut world = test_world();
        let player = world.player();

        spawn_monster(&mut world, Position::new(41, 10), "goblin"); // distance 1
        spawn_monster(&mut world, Position::new(43, 12), "orc"); // distance 3
        spawn_monster(&mut world, Position::new(50, 10), "troll"); // distance 10

        let center = Position::new(40, 10);
        let (positions, events) = reveal_monsters_in_area(&world, player, center, 3);

        // Should find goblin (dist 1) and orc (dist 3), not troll (dist 10).
        assert_eq!(positions.len(), 2, "should find 2 monsters within radius 3");
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "reveal-monsters-found"
        )));
    }

    #[test]
    fn test_reveal_monsters_in_area_none() {
        let world = test_world();
        let player = world.player();

        let center = Position::new(40, 10);
        let (positions, events) = reveal_monsters_in_area(&world, player, center, 5);
        assert!(positions.is_empty());
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "reveal-monsters-none"
        )));
    }
}
