//! Environment interaction systems: fountains, thrones, kicking, and
//! containers.
//!
//! These implement the classic NetHack terrain/feature interactions that
//! are not part of movement, combat, or the item lifecycle.

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use nethack_babel_data::{ObjectCore, ObjectLocation};

use crate::action::{Direction, Position};
use crate::dungeon::Terrain;
use crate::event::{EngineEvent, HpSource, StatusEffect};
use crate::world::{Attributes, GameWorld, HitPoints, Monster, Positioned};

// ── Fountain effects ────────────────────────────────────────────────────

/// Possible outcomes from quaffing a fountain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FountainEffect {
    /// Granted a wish (1/300 overall: rn2(30)==0 then rn2(10)==0).
    Wish,
    /// A water moccasin appears (1/5 chance, after wish check fails).
    WaterMoccasin,
    /// See invisible granted (1/20 of remaining).
    SeeInvisible,
    /// Poison (1/20 of remaining).
    Poison,
    /// Random attribute change (+1 or -1 to a random stat).
    AttributeChange { stat_index: usize, delta: i8 },
    /// Harmless splash (no effect).
    Nothing,
}

/// Determine what happens when the player drinks from a fountain.
///
/// NetHack's `drinkfountain()` logic (simplified):
///   - 1/30 chance to enter wish check; then 1/10 for actual wish = 1/300 total.
///   - Otherwise 1/5 water moccasin.
///   - Then 1/20 see invisible, 1/20 poison, 1/10 attribute change.
///   - Otherwise nothing.
pub fn quaff_fountain(
    world: &mut GameWorld,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let player = world.player();
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return vec![EngineEvent::msg("fountain-no-position")],
    };

    // Verify standing on a fountain.
    let terrain = match world.dungeon().current_level.get(player_pos) {
        Some(cell) => cell.terrain,
        None => return vec![],
    };
    if terrain != Terrain::Fountain {
        return vec![EngineEvent::msg("fountain-not-here")];
    }

    let mut events = vec![EngineEvent::FountainDrank {
        entity: player,
        position: player_pos,
    }];

    let effect = roll_fountain_effect(rng);

    match effect {
        FountainEffect::Wish => {
            events.push(EngineEvent::msg("fountain-wish"));
            // The wish granting itself is handled by the wish system;
            // we just emit the message here.
        }
        FountainEffect::WaterMoccasin => {
            events.push(EngineEvent::msg("fountain-water-moccasin"));
            // Monster spawning is handled by the caller/turn loop.
        }
        FountainEffect::SeeInvisible => {
            events.push(EngineEvent::StatusApplied {
                entity: player,
                status: StatusEffect::SeeInvisible,
                duration: None, // permanent
                source: None,
            });
            events.push(EngineEvent::msg("fountain-see-invisible"));
        }
        FountainEffect::Poison => {
            events.push(EngineEvent::HpChange {
                entity: player,
                amount: -3,
                new_hp: world
                    .get_component::<HitPoints>(player)
                    .map(|hp| (hp.current - 3).max(1))
                    .unwrap_or(1),
                source: HpSource::Poison,
            });
            events.push(EngineEvent::msg("fountain-poison"));
            // Apply HP loss.
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                hp.current = (hp.current - 3).max(1);
            }
        }
        FountainEffect::AttributeChange { stat_index, delta } => {
            let stat_name = match stat_index {
                0 => "strength",
                1 => "dexterity",
                2 => "constitution",
                3 => "intelligence",
                4 => "wisdom",
                _ => "charisma",
            };
            if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                match stat_index {
                    0 => attrs.strength = (attrs.strength as i16 + delta as i16).clamp(3, 25) as u8,
                    1 => attrs.dexterity = (attrs.dexterity as i16 + delta as i16).clamp(3, 25) as u8,
                    2 => attrs.constitution = (attrs.constitution as i16 + delta as i16).clamp(3, 25) as u8,
                    3 => attrs.intelligence = (attrs.intelligence as i16 + delta as i16).clamp(3, 25) as u8,
                    4 => attrs.wisdom = (attrs.wisdom as i16 + delta as i16).clamp(3, 25) as u8,
                    _ => attrs.charisma = (attrs.charisma as i16 + delta as i16).clamp(3, 25) as u8,
                }
            }
            let direction = if delta > 0 { "increases" } else { "decreases" };
            events.push(EngineEvent::msg_with(
                "fountain-attribute-change",
                vec![
                    ("stat", stat_name.to_string()),
                    ("direction", direction.to_string()),
                ],
            ));
        }
        FountainEffect::Nothing => {
            events.push(EngineEvent::msg("fountain-nothing"));
        }
    }

    // Fountain may dry up (1/3 chance).
    if rng.random_range(0..3) == 0 {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(player_pos, Terrain::Floor);
        events.push(EngineEvent::msg("fountain-dries-up"));
    }

    events
}

/// Roll the fountain effect without side effects (for testing).
pub fn roll_fountain_effect(rng: &mut impl Rng) -> FountainEffect {
    // Wish: 1/30 * 1/10 = 1/300.
    if rng.random_range(0..30) == 0
        && rng.random_range(0..10) == 0 {
            return FountainEffect::Wish;
        }

    // Water moccasin: 1/5.
    if rng.random_range(0..5) == 0 {
        return FountainEffect::WaterMoccasin;
    }

    // See invisible: 1/20.
    if rng.random_range(0..20) == 0 {
        return FountainEffect::SeeInvisible;
    }

    // Poison: 1/20.
    if rng.random_range(0..20) == 0 {
        return FountainEffect::Poison;
    }

    // Attribute change: 1/10.
    if rng.random_range(0..10) == 0 {
        let stat_index = rng.random_range(0..6);
        let delta: i8 = if rng.random_bool(0.5) { 1 } else { -1 };
        return FountainEffect::AttributeChange { stat_index, delta };
    }

    FountainEffect::Nothing
}

// ── Throne effects ──────────────────────────────────────────────────────

/// Possible outcomes from sitting on a throne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThroneEffect {
    /// Granted a wish (1/13).
    Wish,
    /// Genocide (1/13).
    Genocide,
    /// Identify items (1/13).
    Identify,
    /// Gold appears (1/13).
    Gold,
    /// Nothing special (9/13).
    Nothing,
}

/// Determine what happens when the player sits on a throne.
///
/// NetHack's `dosit()` for thrones: each positive effect is ~1/13.
/// After a positive effect, the throne disappears.
pub fn sit_throne(
    world: &mut GameWorld,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let player = world.player();
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return vec![EngineEvent::msg("throne-no-position")],
    };

    // Verify standing on a throne.
    let terrain = match world.dungeon().current_level.get(player_pos) {
        Some(cell) => cell.terrain,
        None => return vec![],
    };
    if terrain != Terrain::Throne {
        return vec![EngineEvent::msg("throne-not-here")];
    }

    let mut events = Vec::new();
    events.push(EngineEvent::msg("throne-sit"));

    let effect = roll_throne_effect(rng);

    match effect {
        ThroneEffect::Wish => {
            events.push(EngineEvent::msg("throne-wish"));
        }
        ThroneEffect::Genocide => {
            events.push(EngineEvent::msg("throne-genocide"));
        }
        ThroneEffect::Identify => {
            events.push(EngineEvent::msg("throne-identify"));
        }
        ThroneEffect::Gold => {
            let amount = rng.random_range(10..=200);
            events.push(EngineEvent::msg_with(
                "throne-gold",
                vec![("amount", amount.to_string())],
            ));
        }
        ThroneEffect::Nothing => {
            events.push(EngineEvent::msg("throne-nothing"));
        }
    }

    // Throne disappears after any positive effect.
    if effect != ThroneEffect::Nothing {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(player_pos, Terrain::Floor);
        events.push(EngineEvent::msg("throne-vanishes"));
    }

    events
}

/// Roll the throne effect without side effects (for testing).
pub fn roll_throne_effect(rng: &mut impl Rng) -> ThroneEffect {
    let roll = rng.random_range(0..13);
    match roll {
        0 => ThroneEffect::Wish,
        1 => ThroneEffect::Genocide,
        2 => ThroneEffect::Identify,
        3 => ThroneEffect::Gold,
        _ => ThroneEffect::Nothing,
    }
}

// ── Kicking ─────────────────────────────────────────────────────────────

/// Possible outcomes from kicking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KickResult {
    /// Kicked a door open successfully.
    DoorBroken,
    /// Failed to kick a door open.
    DoorHeld,
    /// Kicked a monster.
    KickedMonster { damage: u32 },
    /// Kicked a sink: a ring pops out.
    SinkRing,
    /// Kicked an item: it moves one tile.
    ItemMoved,
    /// Nothing to kick / empty space.
    Nothing,
    /// Kicked a wall/stone: hurt yourself.
    HurtFoot,
}

/// Attempt to kick in the given direction.
///
/// Implements NetHack's `dokick()` with simplified rules:
/// - Kicking a locked/closed door: STR-based chance to break it.
/// - Kicking a monster: d(1,4) base damage, monks get martial arts bonus.
/// - Kicking a sink: a ring pops out underneath.
/// - Kicking an item on the floor: moves it one tile in the kick direction.
/// - Kicking a wall/stone: you hurt your foot.
pub fn kick(
    world: &mut GameWorld,
    direction: Direction,
    is_monk: bool,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let player = world.player();
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return vec![],
    };

    let target_pos = player_pos.step(direction);

    // Check bounds.
    if !world.dungeon().current_level.in_bounds(target_pos) {
        return vec![EngineEvent::msg("kick-nothing")];
    }

    let terrain = match world.dungeon().current_level.get(target_pos) {
        Some(cell) => cell.terrain,
        None => return vec![EngineEvent::msg("kick-nothing")],
    };

    let mut events = Vec::new();

    // Check for monster at target position.
    if let Some(monster_entity) = find_monster_at(world, target_pos) {
        let base_damage = rng.random_range(1..=4);
        let bonus = if is_monk { rng.random_range(1..=6) } else { 0 };
        let total_damage = base_damage + bonus;

        // Apply damage to the monster.
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(monster_entity) {
            hp.current -= total_damage;
        }

        events.push(EngineEvent::msg_with(
            "kick-monster",
            vec![("damage", total_damage.to_string())],
        ));
        return events;
    }

    match terrain {
        Terrain::DoorClosed | Terrain::DoorLocked => {
            let strength = world
                .get_component::<Attributes>(player)
                .map(|a| a.strength)
                .unwrap_or(10);

            let result = try_kick_door(strength, rng);
            match result {
                KickResult::DoorBroken => {
                    world
                        .dungeon_mut()
                        .current_level
                        .set_terrain(target_pos, Terrain::DoorOpen);
                    events.push(EngineEvent::DoorBroken {
                        position: target_pos,
                    });
                    events.push(EngineEvent::msg("kick-door-open"));
                }
                KickResult::DoorHeld => {
                    events.push(EngineEvent::msg("kick-door-held"));
                }
                _ => unreachable!(),
            }
        }
        Terrain::Sink => {
            events.push(EngineEvent::msg("kick-sink-ring"));
            // The actual ring spawning is handled by the caller/item system.
        }
        Terrain::Stone | Terrain::Wall | Terrain::Tree | Terrain::IronBars => {
            events.push(EngineEvent::msg("kick-hurt-foot"));
            // Minor HP loss.
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                hp.current = (hp.current - 1).max(1);
            }
            events.push(EngineEvent::HpChange {
                entity: player,
                amount: -1,
                new_hp: world
                    .get_component::<HitPoints>(player)
                    .map(|hp| hp.current)
                    .unwrap_or(1),
                source: HpSource::Environment,
            });
        }
        _ => {
            // Check for items on the tile.
            if let Some(item_entity) = find_floor_item_at(world, target_pos) {
                let beyond = target_pos.step(direction);
                if world.dungeon().current_level.in_bounds(beyond)
                    && world
                        .dungeon()
                        .current_level
                        .get(beyond)
                        .is_some_and(|c| c.terrain.is_walkable())
                {
                    // Move the item one tile.
                    if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item_entity) {
                        *loc = ObjectLocation::Floor {
                            x: beyond.x as i16,
                            y: beyond.y as i16,
                        };
                    }
                    events.push(EngineEvent::msg("kick-item-moved"));
                } else {
                    events.push(EngineEvent::msg("kick-item-blocked"));
                }
            } else {
                events.push(EngineEvent::msg("kick-nothing"));
            }
        }
    }

    events
}

/// Determine if a kick breaks a door based on strength.
///
/// NetHack uses a STR-based threshold: higher STR = higher chance.
/// Simplified: chance = STR * 4 out of 100.  So STR 18 = 72% success.
pub fn try_kick_door(strength: u8, rng: &mut impl Rng) -> KickResult {
    let chance = (strength as u32) * 4;
    if rng.random_range(0..100) < chance {
        KickResult::DoorBroken
    } else {
        KickResult::DoorHeld
    }
}

/// Find a monster entity at the given position (excluding the player).
fn find_monster_at(world: &GameWorld, pos: Position) -> Option<Entity> {
    let player = world.player();
    for (entity, (positioned, _monster)) in world
        .ecs()
        .query::<(&Positioned, &Monster)>()
        .iter()
    {
        if entity != player && positioned.0 == pos {
            return Some(entity);
        }
    }
    None
}

/// Find an item entity on the floor at the given position.
fn find_floor_item_at(world: &GameWorld, pos: Position) -> Option<Entity> {
    for (entity, loc) in world.ecs().query::<&ObjectLocation>().iter() {
        if let ObjectLocation::Floor { x, y } = *loc
            && x == pos.x as i16 && y == pos.y as i16 {
                return Some(entity);
            }
    }
    None
}

// ── Containers ──────────────────────────────────────────────────────────

/// Container type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContainerType {
    Sack,
    BagOfHolding,
    Chest,
    LargeBox,
}

/// ECS component marking an entity as a container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub container_type: ContainerType,
    /// Whether the container is locked.
    pub locked: bool,
    /// Whether the container is trapped.
    pub trapped: bool,
}

impl Container {
    /// Weight reduction factor for the contents of this container.
    ///
    /// Bag of holding: contents weigh 1/4 of their actual weight.
    /// Other containers: no weight reduction (factor = 1.0, stored as 4/4).
    ///
    /// Returns (numerator, denominator) for integer arithmetic.
    pub fn weight_factor(&self) -> (u32, u32) {
        match self.container_type {
            ContainerType::BagOfHolding => (1, 4),
            _ => (1, 1),
        }
    }
}

/// List the contents of a container.
///
/// Returns all items whose `ObjectLocation` is `Contained` with the
/// container's entity id.
pub fn container_contents(
    world: &GameWorld,
    container_entity: Entity,
) -> Vec<(Entity, ObjectCore)> {
    let container_id = container_entity.to_bits().get() as u32;
    let mut items = Vec::new();

    for (entity, core) in world.query::<ObjectCore>().iter() {
        if let Some(loc) = world.get_component::<ObjectLocation>(entity)
            && let ObjectLocation::Contained {
                container_id: cid,
            } = *loc
                && cid == container_id {
                    items.push((entity, core.clone()));
                }
    }
    items
}

/// Open a container: list its contents and emit events.
pub fn open_container(
    world: &GameWorld,
    container_entity: Entity,
) -> Vec<EngineEvent> {
    // Check if locked.
    if let Some(container) = world.get_component::<Container>(container_entity)
        && container.locked {
            return vec![EngineEvent::msg("container-locked")];
        }

    let contents = container_contents(world, container_entity);
    let mut events = Vec::new();

    if contents.is_empty() {
        events.push(EngineEvent::msg("container-empty"));
    } else {
        events.push(EngineEvent::msg_with(
            "container-contents",
            vec![("count", contents.len().to_string())],
        ));
    }

    events
}

/// Put an item from the player's inventory into a container.
///
/// Changes the item's `ObjectLocation` from `Inventory` to `Contained`.
/// Returns events describing what happened.
pub fn put_in_container(
    world: &mut GameWorld,
    item_entity: Entity,
    container_entity: Entity,
) -> Vec<EngineEvent> {
    // Verify the container is not locked.
    if let Some(container) = world.get_component::<Container>(container_entity)
        && container.locked {
            return vec![EngineEvent::msg("container-locked")];
        }

    // Update item location.
    let container_id = container_entity.to_bits().get() as u32;
    if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item_entity) {
        *loc = ObjectLocation::Contained { container_id };
    }

    // Remove from player's Inventory component.
    let player = world.player();
    if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
        inv.remove(item_entity);
    }

    vec![EngineEvent::msg("container-put-in")]
}

/// Take an item from a container into the player's inventory.
///
/// Changes the item's `ObjectLocation` from `Contained` to `Inventory`.
pub fn take_from_container(
    world: &mut GameWorld,
    item_entity: Entity,
    _container_entity: Entity,
) -> Vec<EngineEvent> {
    // Update item location.
    if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item_entity) {
        *loc = ObjectLocation::Inventory;
    }

    // Add to player's Inventory component.
    let player = world.player();
    if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
        inv.add(item_entity);
    }

    vec![EngineEvent::msg("container-take-out")]
}

/// Calculate the total weight of a container including its contents.
///
/// For a bag of holding, contents weigh 1/4.  For other containers,
/// the weight is the container's own weight plus the full weight of
/// its contents.
pub fn container_weight(
    world: &GameWorld,
    container_entity: Entity,
) -> u32 {
    let own_weight = world
        .get_component::<ObjectCore>(container_entity)
        .map(|c| c.weight)
        .unwrap_or(0);

    let (num, den) = world
        .get_component::<Container>(container_entity)
        .map(|c| c.weight_factor())
        .unwrap_or((1, 1));

    let contents = container_contents(world, container_entity);
    let contents_weight: u32 = contents
        .iter()
        .map(|(_, core)| core.weight.saturating_mul(core.quantity as u32))
        .sum();

    // Apply weight reduction: contents_weight * num / den.
    let effective_contents = contents_weight.saturating_mul(num) / den;

    own_weight.saturating_add(effective_contents)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    use nethack_babel_data::{BucStatus, KnowledgeState, ObjectClass, ObjectTypeId};

    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(42)
    }

    fn setup_world_at(pos: Position) -> GameWorld {
        GameWorld::new(pos)
    }

    // ── Fountain tests ──────────────────────────────────────────────

    #[test]
    fn test_fountain_wish_probability() {
        // Over 10,000 trials, wish should occur ~1/300 times.
        let mut wish_count = 0;
        let trials = 30_000;

        for seed in 0..trials {
            let mut rng = Pcg64::seed_from_u64(seed);
            let effect = roll_fountain_effect(&mut rng);
            if effect == FountainEffect::Wish {
                wish_count += 1;
            }
        }

        // Expected: ~100 wishes out of 30,000 (1/300).
        // Allow generous range: 30-250.
        assert!(
            wish_count > 30 && wish_count < 250,
            "Expected ~100 wishes from 30,000 rolls (1/300), got {}",
            wish_count,
        );
    }

    #[test]
    fn test_fountain_water_moccasin_probability() {
        let mut moccasin_count = 0;
        let trials = 10_000;

        for seed in 0..trials {
            let mut rng = Pcg64::seed_from_u64(seed);
            let effect = roll_fountain_effect(&mut rng);
            if effect == FountainEffect::WaterMoccasin {
                moccasin_count += 1;
            }
        }

        // After wish check (1/30 consumed), remaining 29/30 * 1/5 ~= 19.3%.
        // Allow range 1000-2500.
        assert!(
            moccasin_count > 1000 && moccasin_count < 2500,
            "Expected ~1930 moccasins from 10,000 rolls, got {}",
            moccasin_count,
        );
    }

    #[test]
    fn test_fountain_quaff_on_fountain() {
        let mut world = setup_world_at(Position::new(10, 5));
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(10, 5), Terrain::Fountain);

        let mut rng = test_rng();
        let events = quaff_fountain(&mut world, &mut rng);

        // Should at least have the FountainDrank event.
        assert!(events.iter().any(|e| matches!(e, EngineEvent::FountainDrank { .. })));
    }

    #[test]
    fn test_fountain_not_on_fountain() {
        let mut world = setup_world_at(Position::new(10, 5));
        // Floor, not fountain.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(10, 5), Terrain::Floor);

        let mut rng = test_rng();
        let events = quaff_fountain(&mut world, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "fountain-not-here"
        )));
    }

    // ── Throne tests ────────────────────────────────────────────────

    #[test]
    fn test_throne_wish_probability() {
        let mut wish_count = 0;
        let trials = 13_000;

        for seed in 0..trials {
            let mut rng = Pcg64::seed_from_u64(seed);
            let effect = roll_throne_effect(&mut rng);
            if effect == ThroneEffect::Wish {
                wish_count += 1;
            }
        }

        // Expected: ~1000 wishes from 13,000 (1/13).
        // Allow range 500-1500.
        assert!(
            wish_count > 500 && wish_count < 1500,
            "Expected ~1000 wishes from 13,000 rolls (1/13), got {}",
            wish_count,
        );
    }

    #[test]
    fn test_throne_effect_distribution() {
        let mut counts = [0u32; 5]; // wish, genocide, identify, gold, nothing
        let trials = 13_000;

        for seed in 0..trials {
            let mut rng = Pcg64::seed_from_u64(seed);
            let effect = roll_throne_effect(&mut rng);
            match effect {
                ThroneEffect::Wish => counts[0] += 1,
                ThroneEffect::Genocide => counts[1] += 1,
                ThroneEffect::Identify => counts[2] += 1,
                ThroneEffect::Gold => counts[3] += 1,
                ThroneEffect::Nothing => counts[4] += 1,
            }
        }

        // Each of wish/genocide/identify/gold should be ~1/13 = ~1000.
        for (i, &c) in counts[0..4].iter().enumerate() {
            assert!(
                c > 500 && c < 1500,
                "Throne effect {} count {} not in expected range",
                i,
                c,
            );
        }
        // Nothing should be ~9/13 = ~9000.
        assert!(
            counts[4] > 7500 && counts[4] < 10500,
            "Throne nothing count {} not in expected range",
            counts[4],
        );
    }

    #[test]
    fn test_throne_vanishes_on_wish() {
        let mut world = setup_world_at(Position::new(10, 5));
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(10, 5), Terrain::Throne);

        // Find a seed that gives a Wish.
        let mut wish_seed = None;
        for seed in 0..1000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            if roll_throne_effect(&mut rng) == ThroneEffect::Wish {
                wish_seed = Some(seed);
                break;
            }
        }
        let seed = wish_seed.expect("should find a wish seed in 1000 tries");
        let mut rng = Pcg64::seed_from_u64(seed);
        let _events = sit_throne(&mut world, &mut rng);

        // Throne should have been replaced with floor.
        let terrain = world
            .dungeon()
            .current_level
            .get(Position::new(10, 5))
            .unwrap()
            .terrain;
        assert_eq!(terrain, Terrain::Floor, "Throne should vanish after wish");
    }

    // ── Kick tests ──────────────────────────────────────────────────

    #[test]
    fn test_kick_door_breaks() {
        // STR 18: 72% chance = 72/100.  With enough seeds, one should succeed.
        let mut world = setup_world_at(Position::new(5, 5));
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::DoorClosed);

        // Set player STR to 18.
        {
            let player = world.player();
            if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
                attrs.strength = 18;
            }
        }

        // Try multiple seeds until we find one that breaks the door.
        let mut broke = false;
        for seed in 0..100 {
            // Reset the door each attempt.
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(6, 5), Terrain::DoorClosed);

            let mut rng = Pcg64::seed_from_u64(seed);
            let events = kick(&mut world, Direction::East, false, &mut rng);

            if events.iter().any(|e| matches!(e, EngineEvent::DoorBroken { .. })) {
                broke = true;
                break;
            }
        }
        assert!(broke, "STR 18 should break a door within 100 attempts");
    }

    #[test]
    fn test_kick_door_weak_fails() {
        // STR 6: 24% chance.  Over 100 attempts, most should fail.
        let mut successes = 0;
        for seed in 0..100 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let result = try_kick_door(6, &mut rng);
            if result == KickResult::DoorBroken {
                successes += 1;
            }
        }
        // 24% = ~24 successes.  Should be significantly less than 50.
        assert!(
            successes < 50,
            "STR 6 should fail more often than succeed, got {} successes/100",
            successes,
        );
    }

    #[test]
    fn test_kick_wall_hurts() {
        let mut world = setup_world_at(Position::new(5, 5));
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Wall);

        let mut rng = test_rng();
        let events = kick(&mut world, Direction::East, false, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "kick-hurt-foot"
        )));
    }

    #[test]
    fn test_kick_nothing() {
        let mut world = setup_world_at(Position::new(5, 5));
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Floor);

        let mut rng = test_rng();
        let events = kick(&mut world, Direction::East, false, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "kick-nothing"
        )));
    }

    #[test]
    fn test_kick_door_strength_scaling() {
        // Verify that higher strength gives more successes.
        let mut low_str_successes = 0;
        let mut high_str_successes = 0;
        let trials = 1000;

        for seed in 0..trials {
            let mut rng = Pcg64::seed_from_u64(seed);
            if try_kick_door(6, &mut rng) == KickResult::DoorBroken {
                low_str_successes += 1;
            }

            let mut rng = Pcg64::seed_from_u64(seed);
            if try_kick_door(18, &mut rng) == KickResult::DoorBroken {
                high_str_successes += 1;
            }
        }

        assert!(
            high_str_successes > low_str_successes,
            "STR 18 ({}) should break doors more often than STR 6 ({})",
            high_str_successes,
            low_str_successes,
        );
    }

    // ── Container tests ─────────────────────────────────────────────

    fn spawn_container(
        world: &mut GameWorld,
        ctype: ContainerType,
    ) -> Entity {
        let core = ObjectCore {
            otyp: ObjectTypeId(200), // dummy
            object_class: ObjectClass::Tool,
            quantity: 1,
            weight: 15,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let buc = BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        };
        let knowledge = KnowledgeState {
            known: false,
            dknown: false,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };
        let loc = ObjectLocation::Inventory;
        let container = Container {
            container_type: ctype,
            locked: false,
            trapped: false,
        };
        let entity = world.spawn((core, buc, knowledge, loc, container));
        entity
    }

    fn spawn_item_in_inventory(world: &mut GameWorld, weight: u32) -> Entity {
        let core = ObjectCore {
            otyp: ObjectTypeId(100), // dummy
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight,
            age: 0,
            inv_letter: Some('a'),
            artifact: None,
        };
        let buc = BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        };
        let knowledge = KnowledgeState {
            known: false,
            dknown: false,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };
        let loc = ObjectLocation::Inventory;
        let entity = world.spawn((core, buc, knowledge, loc));

        // Also add to player's Inventory component.
        let player = world.player();
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.add(entity);
        }

        entity
    }

    #[test]
    fn test_container_put_take() {
        let mut world = setup_world_at(Position::new(5, 5));
        let container = spawn_container(&mut world, ContainerType::Sack);
        let item = spawn_item_in_inventory(&mut world, 100);

        // Put item in container.
        let events = put_in_container(&mut world, item, container);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "container-put-in"
        )));

        // Verify item is now in the container.
        let contents = container_contents(&world, container);
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].0, item);

        // Verify item is no longer in player inventory.
        let player = world.player();
        {
            let inv = world.get_component::<crate::inventory::Inventory>(player).unwrap();
            assert!(!inv.items.contains(&item));
        }

        // Take item from container.
        let events = take_from_container(&mut world, item, container);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "container-take-out"
        )));

        // Verify item is back in player inventory.
        {
            let inv = world.get_component::<crate::inventory::Inventory>(player).unwrap();
            assert!(inv.items.contains(&item));
        }

        // Verify container is now empty.
        let contents = container_contents(&world, container);
        assert_eq!(contents.len(), 0);
    }

    #[test]
    fn test_bag_of_holding_weight() {
        let mut world = setup_world_at(Position::new(5, 5));
        let boh = spawn_container(&mut world, ContainerType::BagOfHolding);

        // Put a 100-weight item in it.
        let item = spawn_item_in_inventory(&mut world, 100);
        put_in_container(&mut world, item, boh);

        // Container weight: own (15) + contents (100/4 = 25) = 40.
        let weight = container_weight(&world, boh);
        assert_eq!(weight, 40, "Bag of holding should reduce contents weight to 1/4");

        // Compare with a regular sack.
        let mut world2 = setup_world_at(Position::new(5, 5));
        let sack = spawn_container(&mut world2, ContainerType::Sack);
        let item2 = spawn_item_in_inventory(&mut world2, 100);
        put_in_container(&mut world2, item2, sack);

        let sack_weight = container_weight(&world2, sack);
        assert_eq!(sack_weight, 115, "Regular sack: own (15) + contents (100) = 115");
    }

    #[test]
    fn test_open_locked_container() {
        let mut world = setup_world_at(Position::new(5, 5));
        let container = spawn_container(&mut world, ContainerType::Chest);

        // Lock the container.
        if let Some(mut c) = world.get_component_mut::<Container>(container) {
            c.locked = true;
        }

        let events = open_container(&world, container);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "container-locked"
        )));
    }

    #[test]
    fn test_open_empty_container() {
        let mut world = setup_world_at(Position::new(5, 5));
        let container = spawn_container(&mut world, ContainerType::LargeBox);

        let events = open_container(&world, container);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "container-empty"
        )));
    }

    #[test]
    fn test_container_multiple_items() {
        let mut world = setup_world_at(Position::new(5, 5));
        let container = spawn_container(&mut world, ContainerType::Sack);

        let item1 = spawn_item_in_inventory(&mut world, 50);
        let item2 = spawn_item_in_inventory(&mut world, 30);

        put_in_container(&mut world, item1, container);
        put_in_container(&mut world, item2, container);

        let contents = container_contents(&world, container);
        assert_eq!(contents.len(), 2);

        let weight = container_weight(&world, container);
        // sack weight (15) + 50 + 30 = 95
        assert_eq!(weight, 95);
    }

    #[test]
    fn test_bag_of_holding_weight_factor() {
        let sack = Container {
            container_type: ContainerType::Sack,
            locked: false,
            trapped: false,
        };
        assert_eq!(sack.weight_factor(), (1, 1));

        let boh = Container {
            container_type: ContainerType::BagOfHolding,
            locked: false,
            trapped: false,
        };
        assert_eq!(boh.weight_factor(), (1, 4));
    }
}
