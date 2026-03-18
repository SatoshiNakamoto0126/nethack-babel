//! Central game state: the `GameWorld` wrapper and ECS component definitions.
//!
//! `GameWorld` owns the hecs ECS world, the dungeon topology, the turn
//! counter, and the player entity handle.  All game logic operates on
//! this struct.

use hecs::{Entity, World};
use nethack_babel_data::{MonsterDef, MonsterId, ObjectDef, PlayerEvents, PlayerQuestItems};
use serde::{Deserialize, Serialize};

use crate::action::Position;
use crate::attributes::{AttributeExercise, NaturalAttributes};
use crate::dungeon::DungeonState;
use crate::inventory::Inventory;
use crate::o_init::AppearanceTable;
use crate::status::{Intrinsics, StatusEffects};

/// The baseline speed value in NetHack's movement point system.
/// Each action costs this many movement points.
pub const NORMAL_SPEED: u32 = 12;

/// Marker component for the player entity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Player;

/// Component: position on the current level map.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Positioned(pub Position);

/// Component: hit points.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HitPoints {
    pub current: i32,
    pub max: i32,
}

/// Component: power / mana.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Power {
    pub current: i32,
    pub max: i32,
}

/// Component: experience level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ExperienceLevel(pub u8);

/// Component: base attributes (Str, Dex, Con, Int, Wis, Cha).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Attributes {
    pub strength: u8,
    /// NetHack's "exceptional" strength for fighters (e.g. 18/50).
    pub strength_extra: u8,
    pub dexterity: u8,
    pub constitution: u8,
    pub intelligence: u8,
    pub wisdom: u8,
    pub charisma: u8,
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            strength: 10,
            strength_extra: 0,
            dexterity: 10,
            constitution: 10,
            intelligence: 10,
            wisdom: 10,
            charisma: 10,
        }
    }
}

/// Component: armor class.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ArmorClass(pub i32);

/// Component: movement speed (NetHack normal = 12).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Speed(pub u32);

/// Component: player combat bonuses (luck, hit/damage adjustments).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlayerCombat {
    pub luck: i32,
    /// To-hit bonus from equipment/rings.
    pub uhitinc: i32,
    /// Damage bonus from equipment/rings.
    pub udaminc: i32,
}

/// Marker component for monster entities.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Monster;

/// Stable monster catalog identity for runtime lookups.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MonsterIdentity(pub MonsterId);

/// Marker component for boulder entities.
///
/// Boulders are large rocks that can be pushed by walking into them
/// (Sokoban mechanic).  When pushed into a pit or hole, the boulder
/// fills the pit and both are removed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Boulder;

/// Marker component for tame (pet) entities.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Tame;

/// Marker component for peaceful non-hostile monsters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Peaceful;

/// Component: monotonically increasing creation order for deterministic
/// turn ordering (Decision D2 from the spec).
///
/// When multiple monsters have the same speed, they act in creation order
/// (ascending).  This ensures reproducible gameplay given the same RNG seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CreationOrder(pub u64);

/// Component: total carried weight (for diagonal squeeze checks, etc.).
///
/// Weight 0 means the entity carries nothing significant.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CarryWeight(pub u32);

/// Component: accumulated movement points.
///
/// Entities accumulate movement points each turn based on their speed.
/// An action costs `NORMAL_SPEED` (12) points.  An entity can act when
/// it has at least `NORMAL_SPEED` points.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MovementPoints(pub i32);

/// Encumbrance level, mirroring NetHack's weight-capacity tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Encumbrance {
    Unencumbered = 0,
    Burdened = 1,
    Stressed = 2,
    Strained = 3,
    Overtaxed = 4,
    Overloaded = 5,
}

/// Component: current encumbrance level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EncumbranceLevel(pub Encumbrance);

/// Speed modifier for monsters (slow / normal / fast).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeedModifier {
    Slow,
    Normal,
    Fast,
}

/// Component: speed modifier applied on top of base `Speed`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MonsterSpeedMod(pub SpeedModifier);

/// Hero speed intrinsic -- whether the player has Fast or Very_fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeroSpeed {
    /// No speed bonus.
    Normal,
    /// Intrinsic fast (e.g., eaten tengu): 1/3 chance of +12 per turn.
    Fast,
    /// Extrinsic fast (boots of speed, potion, spell): 2/3 chance of +12.
    VeryFast,
}

/// Component: hero speed intrinsic (only on the player entity).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HeroSpeedBonus(pub HeroSpeed);

/// Component: nutrition counter for hunger tracking.
///
/// Starts at 900.  Depleted by 1 each turn (plus accessory costs).
/// Thresholds: >1000 Satiated, >150 NotHungry, >50 Hungry, >0 Weak,
/// <=0 Fainting.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Nutrition(pub i32);

/// Component: display name for entities (monsters, items, the player).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Name(pub String);

/// Component: display symbol and color for rendering on the map.
///
/// Monsters get their symbol and color from `MonsterDef`; the player is
/// always `@` white.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DisplaySymbol {
    pub symbol: char,
    pub color: nethack_babel_data::Color,
}

/// The central game-state wrapper around `hecs::World`.
///
/// All game logic operates on this struct.  It owns the ECS world, dungeon
/// topology, and turn counter.  It has ZERO IO.
pub struct GameWorld {
    world: World,
    turn: u32,
    player: Entity,
    dungeon: DungeonState,
    /// Monotonically increasing counter for assigning `CreationOrder`.
    next_creation_order: u64,
    /// Shuffled appearance table for unidentified items (generated once
    /// per game from the RNG seed).
    pub appearance_table: AppearanceTable,
    /// Static monster catalog used by systems that need `MonsterDef` lookup
    /// during runtime world transitions (e.g. special-level population).
    monster_catalog: Vec<MonsterDef>,
    /// Static object catalog used by systems that need `ObjectDef` lookup
    /// during runtime world transitions (e.g. special-level population).
    object_catalog: Vec<ObjectDef>,
}

impl GameWorld {
    /// Create a new game world with a freshly spawned player entity at the
    /// given starting position.
    pub fn new(player_start: Position) -> Self {
        use rand::SeedableRng;
        let mut rng = rand_pcg::Pcg64::seed_from_u64(0);
        Self::new_with_rng(player_start, &mut rng)
    }

    /// Create a new game world using the provided RNG for appearance
    /// table shuffling and any other seed-dependent initialization.
    pub fn new_with_rng(player_start: Position, rng: &mut impl rand::Rng) -> Self {
        let appearance_table = AppearanceTable::new(rng);
        let mut world = World::new();
        let player = world.spawn((
            Player,
            Positioned(player_start),
            HitPoints {
                current: 16,
                max: 16,
            },
            Power { current: 4, max: 4 },
            ExperienceLevel(1),
            Attributes::default(),
            ArmorClass(10),
            Speed(12),
            MovementPoints(NORMAL_SPEED as i32),
            EncumbranceLevel(Encumbrance::Unencumbered),
            HeroSpeedBonus(HeroSpeed::Normal),
            Nutrition(900),
        ));
        // hecs limits spawn tuples to 16 elements, so we insert the
        // remaining components individually.
        let _ = world.insert_one(
            player,
            PlayerCombat {
                luck: 0,
                uhitinc: 0,
                udaminc: 0,
            },
        );
        let _ = world.insert_one(player, Name("you".to_string()));
        let _ = world.insert_one(player, CreationOrder(0));
        let _ = world.insert_one(player, StatusEffects::default());
        let _ = world.insert_one(player, Intrinsics::default());
        let _ = world.insert_one(player, Inventory::new());
        let _ = world.insert_one(player, crate::equipment::EquipmentSlots::default());
        let _ = world.insert_one(player, AttributeExercise::default());
        let _ = world.insert_one(player, NaturalAttributes::default());
        let _ = world.insert_one(player, crate::spells::SpellBook::default());
        let _ = world.insert_one(player, PlayerQuestItems::default());
        let _ = world.insert_one(player, PlayerEvents::default());
        Self {
            world,
            turn: 1,
            player,
            dungeon: DungeonState::with_rng(rng),
            next_creation_order: 1,
            appearance_table,
            monster_catalog: Vec::new(),
            object_catalog: Vec::new(),
        }
    }

    // ── Accessors ─────────────────────────────────────────────

    /// The player entity handle.
    #[inline]
    pub fn player(&self) -> Entity {
        self.player
    }

    /// Current game turn.
    #[inline]
    pub fn turn(&self) -> u32 {
        self.turn
    }

    /// Read-only access to the dungeon state.
    #[inline]
    pub fn dungeon(&self) -> &DungeonState {
        &self.dungeon
    }

    /// Mutable access to the dungeon state.
    #[inline]
    pub fn dungeon_mut(&mut self) -> &mut DungeonState {
        &mut self.dungeon
    }

    /// Advance the turn counter by one.
    #[inline]
    pub fn advance_turn(&mut self) {
        self.turn += 1;
    }

    /// Check whether the given entity is the player.
    #[inline]
    pub fn is_player(&self, entity: Entity) -> bool {
        entity == self.player
    }

    /// Allocate the next creation order value.
    ///
    /// Used when spawning monsters to assign a deterministic ordering
    /// for turn processing (speed desc, then creation order asc).
    #[inline]
    pub fn next_creation_order(&mut self) -> CreationOrder {
        let order = CreationOrder(self.next_creation_order);
        self.next_creation_order += 1;
        order
    }

    /// Read the current creation order counter without incrementing.
    /// Used by the save system to persist and restore the counter.
    #[inline]
    pub fn next_creation_order_value(&self) -> u64 {
        self.next_creation_order
    }

    /// Restore the next creation order counter after loading a save.
    #[inline]
    pub fn set_next_creation_order_value(&mut self, next_creation_order: u64) {
        self.next_creation_order = next_creation_order.max(1);
    }

    /// Install runtime catalogs for monster/object lookup.
    pub fn set_spawn_catalogs(&mut self, monsters: Vec<MonsterDef>, objects: Vec<ObjectDef>) {
        self.monster_catalog = monsters;
        self.object_catalog = objects;
    }

    /// Read-only monster catalog.
    #[inline]
    pub fn monster_catalog(&self) -> &[MonsterDef] {
        &self.monster_catalog
    }

    /// Read-only object catalog.
    #[inline]
    pub fn object_catalog(&self) -> &[ObjectDef] {
        &self.object_catalog
    }

    /// Look up the display name for an entity.
    /// Returns "something" if the entity has no Name component.
    pub fn entity_name(&self, entity: Entity) -> String {
        self.get_component::<Name>(entity)
            .map(|n| n.0.clone())
            .unwrap_or_else(|| "something".to_string())
    }

    // ── ECS helpers ───────────────────────────────────────────

    /// Get a component for an entity.
    #[inline]
    pub fn get_component<T: Send + Sync + 'static>(
        &self,
        entity: Entity,
    ) -> Option<hecs::Ref<'_, T>> {
        self.world.get::<&T>(entity).ok()
    }

    /// Get a mutable component for an entity.
    #[inline]
    pub fn get_component_mut<T: Send + Sync + 'static>(
        &mut self,
        entity: Entity,
    ) -> Option<hecs::RefMut<'_, T>> {
        self.world.get::<&mut T>(entity).ok()
    }

    /// Iterate over all entities having component T.
    pub fn query<T: Send + Sync + 'static>(&self) -> hecs::QueryBorrow<'_, &T> {
        self.world.query::<&T>()
    }

    /// Spawn a new entity with the given components.
    pub fn spawn(&mut self, components: impl hecs::DynamicBundle) -> Entity {
        self.world.spawn(components)
    }

    /// Despawn an entity.
    pub fn despawn(&mut self, entity: Entity) -> Result<(), hecs::NoSuchEntity> {
        self.world.despawn(entity)
    }

    /// Raw access to the underlying hecs::World (escape hatch for complex queries).
    pub fn ecs(&self) -> &World {
        &self.world
    }

    /// Mutable raw access to the underlying hecs::World.
    pub fn ecs_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_world_has_player() {
        let world = GameWorld::new(Position::new(40, 10));
        let pos = world
            .get_component::<Positioned>(world.player())
            .expect("player should have position");
        assert_eq!(pos.0, Position::new(40, 10));
    }

    #[test]
    fn advance_turn_increments() {
        let mut world = GameWorld::new(Position::new(0, 0));
        assert_eq!(world.turn(), 1);
        world.advance_turn();
        assert_eq!(world.turn(), 2);
    }

    #[test]
    fn spawn_and_query() {
        let mut world = GameWorld::new(Position::new(0, 0));
        let e = world.spawn((Positioned(Position::new(5, 5)), Speed(8)));
        let spd = world
            .get_component::<Speed>(e)
            .expect("entity should have Speed");
        assert_eq!(spd.0, 8);
    }

    #[test]
    fn entity_name_fallback() {
        let mut world = GameWorld::new(Position::new(0, 0));
        let unnamed = world.spawn((Positioned(Position::new(1, 1)),));
        assert_eq!(world.entity_name(unnamed), "something");
        assert_eq!(world.entity_name(world.player()), "you");
    }
}
