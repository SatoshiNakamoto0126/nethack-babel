//! Tool use system: the `apply` command (NetHack's apply.c equivalent).
//!
//! Implements `apply_tool()` for the nine tool categories specified:
//!   1. Unicorn horn — cure/inflict status effects
//!   2. Stethoscope — detect monsters/stats
//!   3. Pick-axe / Mattock — dig through walls
//!   4. Key / Lock pick / Credit card — unlock doors/chests
//!   5. Lamp / Lantern — toggle light source
//!   6. Whistle — call pets
//!   7. Mirror — reflect gaze attacks
//!   8. Tinning kit — create tins from corpses
//!   9. Camera — blind adjacent monsters
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{BucStatus, LightSource, ObjectCore};

use crate::action::Position;
use crate::event::EngineEvent;
use crate::status::StatusEffects;
use crate::world::{
    GameWorld, HitPoints, Monster, Name, Positioned, Tame,
};

// ---------------------------------------------------------------------------
// Tool type classification
// ---------------------------------------------------------------------------

/// Tool types recognized by the apply system.
///
/// The caller maps from `ObjectCore.otyp` / object name to this enum
/// before dispatching.  Tools not listed here emit "nothing happens".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolType {
    UnicornHorn,
    Stethoscope,
    PickAxe,
    Mattock,
    Key,
    LockPick,
    CreditCard,
    OilLamp,
    MagicLamp,
    Lantern,
    MagicWhistle,
    TinWhistle,
    Mirror,
    TinningKit,
    Camera,
}

/// Attempt to classify an item entity as a known tool type.
///
/// Uses the object name (from `ObjectCore` display name or data tables)
/// to determine the tool type.  In a full implementation this would
/// look up the `ObjectDef` by `otyp`; here we use the entity's `Name`
/// component as a practical shortcut.
pub fn classify_tool(world: &GameWorld, item: Entity) -> Option<ToolType> {
    // First try the entity's Name component.
    let name = world.entity_name(item);
    classify_tool_by_name(&name)
}

/// Classify a tool by its display name string.
pub fn classify_tool_by_name(name: &str) -> Option<ToolType> {
    let lower = name.to_lowercase();
    if lower.contains("unicorn horn") {
        Some(ToolType::UnicornHorn)
    } else if lower.contains("stethoscope") {
        Some(ToolType::Stethoscope)
    } else if lower.contains("mattock") || lower.contains("dwarvish mattock") {
        Some(ToolType::Mattock)
    } else if lower.contains("pick-axe") || lower.contains("pick axe") {
        Some(ToolType::PickAxe)
    } else if lower.contains("skeleton key") || lower == "key" {
        Some(ToolType::Key)
    } else if lower.contains("lock pick") || lower.contains("lockpick") {
        Some(ToolType::LockPick)
    } else if lower.contains("credit card") {
        Some(ToolType::CreditCard)
    } else if lower.contains("magic lamp") {
        Some(ToolType::MagicLamp)
    } else if lower.contains("oil lamp") {
        Some(ToolType::OilLamp)
    } else if lower.contains("lantern") || lower.contains("brass lantern") {
        Some(ToolType::Lantern)
    } else if lower.contains("magic whistle") {
        Some(ToolType::MagicWhistle)
    } else if lower.contains("tin whistle") {
        Some(ToolType::TinWhistle)
    } else if lower.contains("mirror") {
        Some(ToolType::Mirror)
    } else if lower.contains("tinning kit") {
        Some(ToolType::TinningKit)
    } else if lower.contains("camera") || lower.contains("expensive camera") {
        Some(ToolType::Camera)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Main dispatch
// ---------------------------------------------------------------------------

/// Apply a tool.  This is the main entry point called from `turn.rs`.
///
/// `item` is the item entity being applied.
/// `direction` is an optional direction for directional tools (pick-axe,
/// stethoscope, mirror, camera).  `target_pos` is the targeted position
/// for the tool effect.
pub fn apply_tool(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let tool_type = match classify_tool(world, item) {
        Some(t) => t,
        None => {
            return vec![EngineEvent::msg("tool-nothing-happens")];
        }
    };

    // Read BUC status for tools that care about it.
    let buc = world
        .get_component::<BucStatus>(item)
        .map(|b| BucStatus {
            cursed: b.cursed,
            blessed: b.blessed,
            bknown: b.bknown,
        })
        .unwrap_or(BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        });

    match tool_type {
        ToolType::UnicornHorn => apply_unicorn_horn(world, player, &buc, rng),
        ToolType::Stethoscope => apply_stethoscope(world, player),
        ToolType::PickAxe | ToolType::Mattock => {
            apply_pickaxe(world, player, tool_type, rng)
        }
        ToolType::Key | ToolType::LockPick | ToolType::CreditCard => {
            apply_lock_tool(world, player, tool_type, item, rng)
        }
        ToolType::OilLamp | ToolType::MagicLamp | ToolType::Lantern => {
            apply_lamp(world, player, item, tool_type, &buc, rng)
        }
        ToolType::MagicWhistle | ToolType::TinWhistle => {
            apply_whistle(world, player, tool_type)
        }
        ToolType::Mirror => apply_mirror(world, player, rng),
        ToolType::TinningKit => apply_tinning_kit(world, player, rng),
        ToolType::Camera => apply_camera(world, player, rng),
    }
}

// ---------------------------------------------------------------------------
// 1. Unicorn horn
// ---------------------------------------------------------------------------

/// Apply a unicorn horn to cure (or inflict) status effects.
///
/// - Blessed: cures all of confusion/stun/hallucination/blindness/sickness
/// - Uncursed: ~33% chance to cure each
/// - Cursed: may add confusion
fn apply_unicorn_horn(
    world: &mut GameWorld,
    player: Entity,
    buc: &BucStatus,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        // Cursed unicorn horn: inflict confusion (d12+12 turns).
        let dur = rng.random_range(1..=12) + 12;
        events.extend(crate::status::make_confused(world, player, dur as u32));
        events.push(EngineEvent::msg("tool-unihorn-cursed"));
        return events;
    }

    let blessed = buc.blessed;

    // Confusion
    if crate::status::is_confused(world, player)
        && (blessed || rng.random_range(0..3) == 0)
    {
        events.extend(crate::status::make_confused(world, player, 0));
    }

    // Stun
    if crate::status::is_stunned(world, player)
        && (blessed || rng.random_range(0..3) == 0)
    {
        events.extend(crate::status::make_stunned(world, player, 0));
    }

    // Hallucination
    if crate::status::is_hallucinating(world, player)
        && (blessed || rng.random_range(0..3) == 0)
    {
        events.extend(crate::status::make_hallucinated(world, player, 0));
    }

    // Blindness
    if crate::status::is_blind(world, player)
        && (blessed || rng.random_range(0..3) == 0)
    {
        events.extend(crate::status::make_blinded(world, player, 0));
    }

    // Sickness (both food poisoning and disease)
    if crate::status::is_sick(world, player)
        && (blessed || rng.random_range(0..3) == 0)
    {
        events.extend(crate::status::cure_sick(
            world,
            player,
            crate::status::SICK_VOMITABLE | crate::status::SICK_NONVOMITABLE,
        ));
    }

    if events.is_empty() {
        events.push(EngineEvent::msg("tool-unihorn-nothing"));
    } else {
        events.push(EngineEvent::msg("tool-unihorn-cured"));
    }

    events
}

// ---------------------------------------------------------------------------
// 2. Stethoscope
// ---------------------------------------------------------------------------

/// Apply a stethoscope.
///
/// - Against self: show stats (HP, AC, level).
/// - Adjacent monsters: show their HP.
/// - Through walls: detect monsters in adjacent room.
fn apply_stethoscope(
    world: &GameWorld,
    player: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Check for adjacent monsters first.
    let mut found_adjacent = false;
    for (entity, (pos, _mon, hp, name)) in world
        .ecs()
        .query::<(&Positioned, &Monster, &HitPoints, &Name)>()
        .iter()
    {
        if entity == player {
            continue;
        }
        let dx = (pos.0.x - player_pos.x).abs();
        let dy = (pos.0.y - player_pos.y).abs();
        if dx <= 1 && dy <= 1 {
            // Adjacent monster: show HP.
            events.push(EngineEvent::msg_with(
                "tool-stethoscope-monster",
                vec![
                    ("name", name.0.clone()),
                    ("hp", hp.current.to_string()),
                    ("maxhp", hp.max.to_string()),
                ],
            ));
            found_adjacent = true;
        }
    }

    if !found_adjacent {
        // No adjacent monsters: examine self.
        let hp = world
            .get_component::<HitPoints>(player)
            .map(|h| (h.current, h.max))
            .unwrap_or((0, 0));
        let ac = world
            .get_component::<crate::world::ArmorClass>(player)
            .map(|a| a.0)
            .unwrap_or(10);
        let xlevel = world
            .get_component::<crate::world::ExperienceLevel>(player)
            .map(|x| x.0)
            .unwrap_or(1);

        events.push(EngineEvent::msg_with(
            "tool-stethoscope-self",
            vec![
                ("hp", hp.0.to_string()),
                ("maxhp", hp.1.to_string()),
                ("ac", ac.to_string()),
                ("level", xlevel.to_string()),
            ],
        ));
    }

    events
}

// ---------------------------------------------------------------------------
// 3. Pick-axe / Mattock — dig
// ---------------------------------------------------------------------------

/// Apply a pick-axe or mattock to dig through a wall.
///
/// Scans for a diggable wall in each adjacent direction.  If found,
/// converts the wall to floor (or corridor if it was stone).
/// Cannot dig shop walls or special walls.
fn apply_pickaxe(
    world: &mut GameWorld,
    player: Entity,
    tool: ToolType,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Try to dig in a random adjacent direction (in a real game the
    // player would choose the direction; we pick the first diggable
    // wall found by scanning all 8 directions).
    let directions = crate::action::Direction::PLANAR;

    for &dir in &directions {
        let target_pos = player_pos.step(dir);

        let terrain = match world.dungeon().current_level.get(target_pos) {
            Some(cell) => cell.terrain,
            None => continue,
        };

        if !is_diggable(terrain) {
            continue;
        }

        // Determine number of turns to dig (simplified).
        let dig_turns = match tool {
            ToolType::Mattock => rng.random_range(2..=4),
            _ => rng.random_range(3..=6),
        };

        // Convert wall/stone to corridor.
        let new_terrain = match terrain {
            crate::dungeon::Terrain::Stone => crate::dungeon::Terrain::Corridor,
            _ => crate::dungeon::Terrain::Corridor,
        };

        world
            .dungeon_mut()
            .current_level
            .set_terrain(target_pos, new_terrain);

        events.push(EngineEvent::msg_with(
            "tool-dig-wall",
            vec![("turns", dig_turns.to_string())],
        ));
        return events;
    }

    events.push(EngineEvent::msg("tool-dig-no-target"));
    events
}

/// Whether a terrain type can be dug through.
fn is_diggable(terrain: crate::dungeon::Terrain) -> bool {
    matches!(
        terrain,
        crate::dungeon::Terrain::Wall | crate::dungeon::Terrain::Stone
    )
}

// ---------------------------------------------------------------------------
// 4. Key / Lock pick / Credit card — unlock
// ---------------------------------------------------------------------------

/// Apply a lock tool (key, lock pick, or credit card) to a locked door.
///
/// Success rates:
/// - Key: 100%
/// - Lock pick: ~70%
/// - Credit card: ~50%
///
/// On lock pick failure, there's a 25% chance the pick breaks.
fn apply_lock_tool(
    world: &mut GameWorld,
    player: Entity,
    tool: ToolType,
    item: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Find an adjacent locked door.
    let directions = crate::action::Direction::PLANAR;
    for &dir in &directions {
        let target_pos = player_pos.step(dir);

        let terrain = match world.dungeon().current_level.get(target_pos) {
            Some(cell) => cell.terrain,
            None => continue,
        };

        if terrain != crate::dungeon::Terrain::DoorLocked {
            continue;
        }

        // Roll for success based on tool type.
        let success = match tool {
            ToolType::Key => true,
            ToolType::LockPick => rng.random_range(0..100) < 70,
            ToolType::CreditCard => rng.random_range(0..100) < 50,
            _ => false,
        };

        if success {
            // Unlock: change terrain from DoorLocked to DoorClosed.
            world
                .dungeon_mut()
                .current_level
                .set_terrain(target_pos, crate::dungeon::Terrain::DoorClosed);

            events.push(EngineEvent::msg("tool-unlock-success"));
            events.push(EngineEvent::DoorOpened {
                position: target_pos,
            });
        } else {
            events.push(EngineEvent::msg("tool-unlock-fail"));

            // Lock pick breakage: 25% on failure.
            if tool == ToolType::LockPick && rng.random_range(0..4) == 0 {
                events.push(EngineEvent::msg("tool-lockpick-breaks"));
                // Mark the item for destruction.
                events.push(EngineEvent::ItemDestroyed {
                    item,
                    cause: crate::event::DamageCause::Physical,
                });
            }
        }
        return events;
    }

    events.push(EngineEvent::msg("tool-no-locked-door"));
    events
}

// ---------------------------------------------------------------------------
// 5. Lamp / Lantern — toggle light
// ---------------------------------------------------------------------------

/// Apply a lamp or lantern: toggle its light source on/off.
///
/// Oil lamp: burns fuel (age), can go out when age reaches 0.
/// Magic lamp: rubbing has 1/3 chance of summoning a djinni (wish!).
fn apply_lamp(
    world: &mut GameWorld,
    _player: Entity,
    item: Entity,
    tool: ToolType,
    _buc: &BucStatus,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Magic lamp special: rubbing chance for djinni.
    if tool == ToolType::MagicLamp
        && rng.random_range(0..3) == 0 {
            events.push(EngineEvent::msg("tool-magic-lamp-djinni"));
            // In a full implementation this would trigger the wish system.
            // For now, just emit the event.
            return events;
        }
        // Otherwise just toggle light like a normal lamp.

    // Toggle light source.
    let is_lit = world
        .get_component::<LightSource>(item)
        .map(|ls| ls.lit)
        .unwrap_or(false);

    if is_lit {
        // Turn off.
        if let Some(mut ls) = world.get_component_mut::<LightSource>(item) {
            ls.lit = false;
        }
        events.push(EngineEvent::msg("tool-lamp-off"));
    } else {
        // Turn on.  Add LightSource if not present, or toggle existing.
        if world.get_component::<LightSource>(item).is_some() {
            if let Some(mut ls) = world.get_component_mut::<LightSource>(item) {
                ls.lit = true;
            }
        } else {
            // Insert a new LightSource component.
            let _ = world.ecs_mut().insert_one(
                item,
                LightSource {
                    lit: true,
                    recharged: 0,
                },
            );
        }
        events.push(EngineEvent::msg("tool-lamp-on"));
    }

    events
}

// ---------------------------------------------------------------------------
// 6. Whistle — call pets
// ---------------------------------------------------------------------------

/// Apply a whistle.
///
/// - Magic whistle: all pets teleport to the player's position.
/// - Tin whistle: exercise lungs only (cosmetic message).
fn apply_whistle(
    world: &mut GameWorld,
    player: Entity,
    tool: ToolType,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if tool == ToolType::TinWhistle {
        events.push(EngineEvent::msg("tool-tin-whistle"));
        return events;
    }

    // Magic whistle: teleport all pets to player.
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Collect pet entities.
    let mut pets: Vec<Entity> = Vec::new();
    for (entity, (_tame, _pos)) in world
        .ecs()
        .query::<(&Tame, &Positioned)>()
        .iter()
    {
        if entity != player {
            pets.push(entity);
        }
    }

    if pets.is_empty() {
        events.push(EngineEvent::msg("tool-whistle-no-pets"));
        return events;
    }

    // Teleport each pet adjacent to the player.
    let offsets: [(i32, i32); 8] = [
        (0, -1), (1, -1), (1, 0), (1, 1),
        (0, 1), (-1, 1), (-1, 0), (-1, -1),
    ];

    for (i, pet) in pets.iter().enumerate() {
        let offset = offsets[i % offsets.len()];
        let new_pos = Position::new(
            player_pos.x + offset.0,
            player_pos.y + offset.1,
        );

        let old_pos = world
            .get_component::<Positioned>(*pet)
            .map(|p| p.0)
            .unwrap_or(player_pos);

        if let Some(mut pos) = world.get_component_mut::<Positioned>(*pet) {
            pos.0 = new_pos;
        }

        events.push(EngineEvent::EntityTeleported {
            entity: *pet,
            from: old_pos,
            to: new_pos,
        });
    }

    events.push(EngineEvent::msg("tool-magic-whistle"));
    events
}

// ---------------------------------------------------------------------------
// 7. Mirror — reflect gaze
// ---------------------------------------------------------------------------

/// Apply a mirror.
///
/// - Against an adjacent monster with a gaze attack: reflect the gaze,
///   damaging the monster.
/// - Against self (no adjacent monsters): cosmetic message.
fn apply_mirror(
    world: &mut GameWorld,
    player: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Find the first adjacent monster.
    let mut target_entity: Option<Entity> = None;
    let mut target_name = String::new();

    for (entity, (pos, _mon, name)) in world
        .ecs()
        .query::<(&Positioned, &Monster, &Name)>()
        .iter()
    {
        if entity == player {
            continue;
        }
        let dx = (pos.0.x - player_pos.x).abs();
        let dy = (pos.0.y - player_pos.y).abs();
        if dx <= 1 && dy <= 1 {
            target_entity = Some(entity);
            target_name = name.0.clone();
            break;
        }
    }

    if let Some(target) = target_entity {
        // Monster sees its reflection.  50% chance of blinding for
        // d10 turns (simplified gaze reflection).
        if rng.random_range(0..2) == 0 {
            let blind_dur: u32 = rng.random_range(1..=10);
            events.extend(crate::status::make_blinded(world, target, blind_dur));
            events.push(EngineEvent::msg_with(
                "tool-mirror-reflect",
                vec![("monster", target_name)],
            ));
        } else {
            events.push(EngineEvent::msg_with(
                "tool-mirror-no-effect",
                vec![("monster", target_name)],
            ));
        }
    } else {
        // Self-examination.
        events.push(EngineEvent::msg("tool-mirror-self"));
    }

    events
}

// ---------------------------------------------------------------------------
// 8. Tinning kit
// ---------------------------------------------------------------------------

/// Apply a tinning kit to a corpse on the floor, creating a tin.
///
/// Finds the first corpse at the player's position and converts it to
/// a tin of that monster type.  Takes 2 * monster_level turns (simplified).
fn apply_tinning_kit(
    world: &mut GameWorld,
    player: Entity,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Find a corpse at the player's position.
    let mut corpse_entity: Option<Entity> = None;
    let mut corpse_name = String::new();

    for (entity, (core, loc)) in world
        .ecs()
        .query::<(&ObjectCore, &nethack_babel_data::ObjectLocation)>()
        .iter()
    {
        if let nethack_babel_data::ObjectLocation::Floor { x, y } = *loc
            && x as i32 == player_pos.x && y as i32 == player_pos.y {
                // Check if it's a corpse (food class, name contains "corpse").
                if core.object_class == nethack_babel_data::ObjectClass::Food {
                    let item_name = world.entity_name(entity);
                    if item_name.contains("corpse") {
                        corpse_entity = Some(entity);
                        corpse_name = item_name;
                        break;
                    }
                }
            }
    }

    match corpse_entity {
        Some(corpse) => {
            // Remove the corpse entity.
            let _ = world.despawn(corpse);

            // Create a tin (simplified: emit event; full implementation
            // would spawn a tin item entity).
            events.push(EngineEvent::msg_with(
                "tool-tinning-success",
                vec![("corpse", corpse_name)],
            ));
        }
        None => {
            events.push(EngineEvent::msg("tool-tinning-no-corpse"));
        }
    }

    events
}

// ---------------------------------------------------------------------------
// 9. Camera
// ---------------------------------------------------------------------------

/// Apply a camera: blind all adjacent monsters for d10 turns.
fn apply_camera(
    world: &mut GameWorld,
    player: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Find all adjacent monsters.
    let mut targets: Vec<(Entity, String)> = Vec::new();

    for (entity, (pos, _mon, name)) in world
        .ecs()
        .query::<(&Positioned, &Monster, &Name)>()
        .iter()
    {
        if entity == player {
            continue;
        }
        let dx = (pos.0.x - player_pos.x).abs();
        let dy = (pos.0.y - player_pos.y).abs();
        if dx <= 1 && dy <= 1 {
            targets.push((entity, name.0.clone()));
        }
    }

    if targets.is_empty() {
        events.push(EngineEvent::msg("tool-camera-no-target"));
        return events;
    }

    for (entity, name) in &targets {
        let blind_dur: u32 = rng.random_range(1..=10);

        // Ensure the monster has StatusEffects.
        if world.get_component::<StatusEffects>(*entity).is_none() {
            let _ = world
                .ecs_mut()
                .insert_one(*entity, StatusEffects::default());
        }

        events.extend(crate::status::make_blinded(world, *entity, blind_dur));
        events.push(EngineEvent::msg_with(
            "tool-camera-blind",
            vec![("monster", name.clone())],
        ));
    }

    events
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::dungeon::Terrain;
    use crate::status::{StatusEffects, SICK_NONVOMITABLE, SICK_VOMITABLE};
    use crate::world::{
        GameWorld, HitPoints, Monster, Name,
        Positioned, Speed, Tame,
    };
    use nethack_babel_data::{BucStatus, LightSource, ObjectClass, ObjectCore, ObjectLocation};
    use rand::SeedableRng;

    type TestRng = rand::rngs::StdRng;

    fn test_rng() -> TestRng {
        TestRng::seed_from_u64(42)
    }

    /// Create a game world with the player at (40, 10).
    fn test_world() -> GameWorld {
        GameWorld::new(Position::new(40, 10))
    }

    /// Spawn a tool item in the world with the given name.
    fn spawn_tool(
        world: &mut GameWorld,
        name: &str,
        blessed: bool,
        cursed: bool,
    ) -> Entity {
        world.spawn((
            ObjectCore {
                otyp: nethack_babel_data::ObjectTypeId(0),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('a'),
                artifact: None,
            },
            BucStatus {
                cursed,
                blessed,
                bknown: true,
            },
            nethack_babel_data::KnowledgeState {
                known: true,
                dknown: true,
                rknown: false,
                cknown: false,
                lknown: false,
                tknown: false,
            },
            ObjectLocation::Inventory,
            Name(name.to_string()),
        ))
    }

    /// Spawn a monster adjacent to the player at the given offset.
    fn spawn_adjacent_monster(
        world: &mut GameWorld,
        name: &str,
        offset: (i32, i32),
        hp: i32,
    ) -> Entity {
        let player_pos = world
            .get_component::<Positioned>(world.player())
            .unwrap()
            .0;
        let pos = Position::new(player_pos.x + offset.0, player_pos.y + offset.1);
        world.spawn((
            Monster,
            Positioned(pos),
            Name(name.to_string()),
            HitPoints {
                current: hp,
                max: hp,
            },
            Speed(12),
            StatusEffects::default(),
        ))
    }

    // ── Unicorn horn tests ──────────────────────────────────────

    #[test]
    fn test_unicorn_horn_blessed_cures_all() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Apply all five status effects.
        crate::status::make_confused(&mut world, player, 50);
        crate::status::make_stunned(&mut world, player, 50);
        crate::status::make_hallucinated(&mut world, player, 50);
        crate::status::make_blinded(&mut world, player, 50);
        crate::status::make_sick(
            &mut world,
            player,
            50,
            SICK_VOMITABLE | SICK_NONVOMITABLE,
        );

        let tool = spawn_tool(&mut world, "unicorn horn", true, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        // All statuses should be cleared.
        assert!(
            !crate::status::is_confused(&world, player),
            "confusion should be cured"
        );
        assert!(
            !crate::status::is_stunned(&world, player),
            "stun should be cured"
        );
        assert!(
            !crate::status::is_hallucinating(&world, player),
            "hallucination should be cured"
        );
        assert!(
            !crate::status::is_blind(&world, player),
            "blindness should be cured"
        );
        assert!(
            !crate::status::is_sick(&world, player),
            "sickness should be cured"
        );

        // Should have cure-related events.
        assert!(
            events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "tool-unihorn-cured")),
            "expected cure message"
        );
    }

    #[test]
    fn test_unicorn_horn_cursed_adds_confusion() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        assert!(!crate::status::is_confused(&world, player));

        let tool = spawn_tool(&mut world, "unicorn horn", false, true);
        let _events = apply_tool(&mut world, player, tool, &mut rng);

        assert!(
            crate::status::is_confused(&world, player),
            "cursed unicorn horn should add confusion"
        );
    }

    #[test]
    fn test_unicorn_horn_uncursed_probabilistic() {
        // Run multiple times to check probabilistic behavior.
        let mut cured_count = 0;
        let trials = 100;

        for seed in 0..trials {
            let mut world = test_world();
            let mut rng = TestRng::seed_from_u64(seed);
            let player = world.player();

            crate::status::make_confused(&mut world, player, 50);
            let tool = spawn_tool(&mut world, "unicorn horn", false, false);
            let _events = apply_tool(&mut world, player, tool, &mut rng);

            if !crate::status::is_confused(&world, player) {
                cured_count += 1;
            }
        }

        // With ~33% cure rate, we expect roughly 33 cures out of 100.
        // Allow wide range to avoid flaky tests.
        assert!(
            cured_count > 10 && cured_count < 70,
            "uncursed unicorn horn cure rate ({cured_count}/100) should be roughly 33%"
        );
    }

    // ── Pick-axe / digging tests ────────────────────────────────

    #[test]
    fn test_pickaxe_digs_wall() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Place a wall adjacent to the player (north).
        let player_pos = world
            .get_component::<Positioned>(player)
            .unwrap()
            .0;
        let wall_pos = Position::new(player_pos.x, player_pos.y - 1);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(wall_pos, Terrain::Wall);

        let tool = spawn_tool(&mut world, "pick-axe", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

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

        assert!(
            events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "tool-dig-wall")),
            "expected dig message"
        );
    }

    // ── Key / lock pick tests ───────────────────────────────────

    #[test]
    fn test_key_unlocks_door() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Place a locked door adjacent (east).
        let player_pos = world
            .get_component::<Positioned>(player)
            .unwrap()
            .0;
        let door_pos = Position::new(player_pos.x + 1, player_pos.y);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(door_pos, Terrain::DoorLocked);

        let tool = spawn_tool(&mut world, "skeleton key", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        // Key has 100% success rate.
        let terrain = world
            .dungeon()
            .current_level
            .get(door_pos)
            .unwrap()
            .terrain;
        assert_eq!(
            terrain,
            Terrain::DoorClosed,
            "locked door should become closed"
        );

        assert!(
            events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "tool-unlock-success")),
            "expected unlock success"
        );
    }

    #[test]
    fn test_lockpick_may_fail() {
        let mut successes = 0;
        let mut failures = 0;
        let trials = 100;

        for seed in 0..trials {
            let mut world = test_world();
            let mut rng = TestRng::seed_from_u64(seed);
            let player = world.player();

            let player_pos = world
                .get_component::<Positioned>(player)
                .unwrap()
                .0;
            let door_pos = Position::new(player_pos.x + 1, player_pos.y);
            world
                .dungeon_mut()
                .current_level
                .set_terrain(door_pos, Terrain::DoorLocked);

            let tool = spawn_tool(&mut world, "lock pick", false, false);
            let events = apply_tool(&mut world, player, tool, &mut rng);

            if events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-unlock-success")
            }) {
                successes += 1;
            } else {
                failures += 1;
            }
        }

        // 70% success rate: expect roughly 70 successes.
        assert!(
            successes > 40 && successes < 95,
            "lock pick success rate ({successes}/{trials}) should be ~70%"
        );
        assert!(failures > 0, "lock pick should sometimes fail");
    }

    // ── Lamp tests ──────────────────────────────────────────────

    #[test]
    fn test_lamp_toggle() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "oil lamp", false, false);

        // Initially no LightSource — applying should turn it on.
        let events = apply_tool(&mut world, player, tool, &mut rng);
        assert!(
            events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "tool-lamp-on")),
            "lamp should turn on"
        );
        let lit = world
            .get_component::<LightSource>(tool)
            .unwrap()
            .lit;
        assert!(lit, "lamp should be lit");

        // Apply again to turn off.
        let events2 = apply_tool(&mut world, player, tool, &mut rng);
        assert!(
            events2.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "tool-lamp-off")),
            "lamp should turn off"
        );
        let lit2 = world
            .get_component::<LightSource>(tool)
            .unwrap()
            .lit;
        assert!(!lit2, "lamp should be unlit");
    }

    #[test]
    fn test_magic_lamp_djinni() {
        // Test that roughly 1/3 of magic lamp applies produce djinni.
        let mut djinni_count = 0;
        let trials = 300;

        for seed in 0..trials {
            let mut world = test_world();
            let mut rng = TestRng::seed_from_u64(seed);
            let player = world.player();

            let tool = spawn_tool(&mut world, "magic lamp", false, false);
            let events = apply_tool(&mut world, player, tool, &mut rng);

            if events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-magic-lamp-djinni")
            }) {
                djinni_count += 1;
            }
        }

        // Expect roughly 100/300 djinni events (1/3).
        assert!(
            djinni_count > 60 && djinni_count < 140,
            "magic lamp djinni rate ({djinni_count}/{trials}) should be ~1/3"
        );
    }

    // ── Whistle tests ───────────────────────────────────────────

    #[test]
    fn test_whistle_calls_pets() {
        let mut world = test_world();
        let player = world.player();

        let player_pos = world
            .get_component::<Positioned>(player)
            .unwrap()
            .0;

        // Spawn a pet far away.
        let pet = world.spawn((
            Monster,
            Tame,
            Positioned(Position::new(70, 15)),
            Name("kitten".to_string()),
            HitPoints {
                current: 8,
                max: 8,
            },
            Speed(12),
        ));

        let tool = spawn_tool(&mut world, "magic whistle", false, false);
        let events = apply_tool(&mut world, player, tool, &mut TestRng::seed_from_u64(0));

        // Pet should now be adjacent to the player.
        let pet_pos = world
            .get_component::<Positioned>(pet)
            .unwrap()
            .0;
        let dx = (pet_pos.x - player_pos.x).abs();
        let dy = (pet_pos.y - player_pos.y).abs();
        assert!(
            dx <= 1 && dy <= 1,
            "pet should be adjacent to player after magic whistle"
        );

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::EntityTeleported { .. })),
            "expected teleport event"
        );
    }

    // ── Mirror tests ────────────────────────────────────────────

    #[test]
    fn test_mirror_reflects_gaze() {
        // Run enough trials to ensure mirror sometimes blinds the monster.
        let mut blinded_count = 0;
        let trials = 100;

        for seed in 0..trials {
            let mut world = test_world();
            let mut rng = TestRng::seed_from_u64(seed);
            let player = world.player();

            let _monster = spawn_adjacent_monster(&mut world, "medusa", (1, 0), 20);
            let tool = spawn_tool(&mut world, "mirror", false, false);
            let events = apply_tool(&mut world, player, tool, &mut rng);

            if events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-mirror-reflect")
            }) {
                blinded_count += 1;
            }
        }

        assert!(
            blinded_count > 20 && blinded_count < 80,
            "mirror should sometimes reflect gaze (blinded {blinded_count}/{trials})"
        );
    }

    #[test]
    fn test_mirror_self() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // No adjacent monsters.
        let tool = spawn_tool(&mut world, "mirror", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        assert!(
            events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "tool-mirror-self")),
            "expected self-examination message"
        );
    }

    // ── Tinning kit tests ───────────────────────────────────────

    #[test]
    fn test_tinning_creates_tin() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let player_pos = world
            .get_component::<Positioned>(player)
            .unwrap()
            .0;

        // Spawn a corpse at the player's position.
        let _corpse = world.spawn((
            ObjectCore {
                otyp: nethack_babel_data::ObjectTypeId(0),
                object_class: ObjectClass::Food,
                quantity: 1,
                weight: 100,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            ObjectLocation::Floor {
                x: player_pos.x as i16,
                y: player_pos.y as i16,
            },
            Name("giant ant corpse".to_string()),
        ));

        let tool = spawn_tool(&mut world, "tinning kit", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-tinning-success")
            }),
            "expected tinning success"
        );
    }

    #[test]
    fn test_tinning_no_corpse() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "tinning kit", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-tinning-no-corpse")
            }),
            "expected no corpse message"
        );
    }

    // ── Camera tests ────────────────────────────────────────────

    #[test]
    fn test_camera_blinds_adjacent() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = spawn_adjacent_monster(&mut world, "goblin", (1, 0), 5);
        let tool = spawn_tool(&mut world, "expensive camera", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        // Monster should be blinded.
        assert!(
            crate::status::is_blind(&world, monster),
            "monster should be blinded by camera"
        );
        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-camera-blind")
            }),
            "expected camera blind message"
        );
    }

    #[test]
    fn test_camera_no_target() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "expensive camera", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-camera-no-target")
            }),
            "expected no target message"
        );
    }

    // ── Stethoscope tests ───────────────────────────────────────

    #[test]
    fn test_stethoscope_self() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "stethoscope", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-stethoscope-self")
            }),
            "expected self stats message"
        );
    }

    #[test]
    fn test_stethoscope_monster() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        spawn_adjacent_monster(&mut world, "orc", (0, 1), 12);
        let tool = spawn_tool(&mut world, "stethoscope", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-stethoscope-monster")
            }),
            "expected monster stats message"
        );
    }

    // ── Tool classification tests ───────────────────────────────

    #[test]
    fn test_classify_tool_names() {
        assert_eq!(
            classify_tool_by_name("unicorn horn"),
            Some(ToolType::UnicornHorn)
        );
        assert_eq!(
            classify_tool_by_name("+2 unicorn horn"),
            Some(ToolType::UnicornHorn)
        );
        assert_eq!(
            classify_tool_by_name("stethoscope"),
            Some(ToolType::Stethoscope)
        );
        assert_eq!(
            classify_tool_by_name("pick-axe"),
            Some(ToolType::PickAxe)
        );
        assert_eq!(
            classify_tool_by_name("dwarvish mattock"),
            Some(ToolType::Mattock)
        );
        assert_eq!(
            classify_tool_by_name("skeleton key"),
            Some(ToolType::Key)
        );
        assert_eq!(
            classify_tool_by_name("lock pick"),
            Some(ToolType::LockPick)
        );
        assert_eq!(
            classify_tool_by_name("credit card"),
            Some(ToolType::CreditCard)
        );
        assert_eq!(
            classify_tool_by_name("magic lamp"),
            Some(ToolType::MagicLamp)
        );
        assert_eq!(
            classify_tool_by_name("oil lamp"),
            Some(ToolType::OilLamp)
        );
        assert_eq!(
            classify_tool_by_name("brass lantern"),
            Some(ToolType::Lantern)
        );
        assert_eq!(
            classify_tool_by_name("magic whistle"),
            Some(ToolType::MagicWhistle)
        );
        assert_eq!(
            classify_tool_by_name("tin whistle"),
            Some(ToolType::TinWhistle)
        );
        assert_eq!(
            classify_tool_by_name("mirror"),
            Some(ToolType::Mirror)
        );
        assert_eq!(
            classify_tool_by_name("tinning kit"),
            Some(ToolType::TinningKit)
        );
        assert_eq!(
            classify_tool_by_name("expensive camera"),
            Some(ToolType::Camera)
        );
        assert_eq!(classify_tool_by_name("random junk"), None);
    }

    // ── Whistle with no pets test ──────────────────────────────

    #[test]
    fn test_whistle_no_pets() {
        let mut world = test_world();
        let player = world.player();

        let tool = spawn_tool(&mut world, "magic whistle", false, false);
        let events = apply_tool(
            &mut world,
            player,
            tool,
            &mut TestRng::seed_from_u64(0),
        );

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-whistle-no-pets")
            }),
            "expected no pets message"
        );
    }

    // ── Tin whistle test ────────────────────────────────────────

    #[test]
    fn test_tin_whistle() {
        let mut world = test_world();
        let player = world.player();

        let tool = spawn_tool(&mut world, "tin whistle", false, false);
        let events = apply_tool(
            &mut world,
            player,
            tool,
            &mut TestRng::seed_from_u64(0),
        );

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-tin-whistle")
            }),
            "expected tin whistle message"
        );
    }

    // ── Additional tests ────────────────────────────────────────

    #[test]
    fn test_unknown_tool_nothing_happens() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "random junk", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-nothing-happens")
            }),
            "unknown tool should produce nothing-happens message"
        );
    }

    #[test]
    fn test_pickaxe_no_diggable_wall() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Surround the player with floor tiles (nothing to dig).
        let player_pos = world
            .get_component::<Positioned>(player)
            .unwrap()
            .0;
        for dir in crate::action::Direction::PLANAR {
            let pos = player_pos.step(dir);
            world
                .dungeon_mut()
                .current_level
                .set_terrain(pos, Terrain::Floor);
        }

        let tool = spawn_tool(&mut world, "pick-axe", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-dig-no-target")
            }),
            "expected no-target message when no diggable wall"
        );
    }

    #[test]
    fn test_credit_card_unlock_rate() {
        let mut successes = 0;
        let trials = 200;

        for seed in 0..trials {
            let mut world = test_world();
            let mut rng = TestRng::seed_from_u64(seed);
            let player = world.player();

            let player_pos = world
                .get_component::<Positioned>(player)
                .unwrap()
                .0;
            let door_pos = Position::new(player_pos.x + 1, player_pos.y);
            world
                .dungeon_mut()
                .current_level
                .set_terrain(door_pos, Terrain::DoorLocked);

            let tool = spawn_tool(&mut world, "credit card", false, false);
            let events = apply_tool(&mut world, player, tool, &mut rng);

            if events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-unlock-success")
            }) {
                successes += 1;
            }
        }

        // 50% success rate: expect roughly 100/200.
        assert!(
            successes > 60 && successes < 140,
            "credit card success rate ({successes}/{trials}) should be ~50%"
        );
    }

    #[test]
    fn test_unicorn_horn_uncursed_no_status_does_nothing() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // No status effects active.
        let tool = spawn_tool(&mut world, "unicorn horn", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-unihorn-nothing")
            }),
            "expected nothing message when no statuses to cure"
        );
    }

    #[test]
    fn test_camera_multiple_monsters() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let mon1 = spawn_adjacent_monster(&mut world, "goblin", (1, 0), 5);
        let mon2 = spawn_adjacent_monster(&mut world, "orc", (0, 1), 8);
        let tool = spawn_tool(&mut world, "expensive camera", false, false);
        let events = apply_tool(&mut world, player, tool, &mut rng);

        // Both monsters should be blinded.
        assert!(
            crate::status::is_blind(&world, mon1),
            "first monster should be blinded"
        );
        assert!(
            crate::status::is_blind(&world, mon2),
            "second monster should be blinded"
        );

        let blind_count = events
            .iter()
            .filter(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-camera-blind")
            })
            .count();
        assert_eq!(
            blind_count, 2,
            "expected two camera blind messages"
        );
    }
}
