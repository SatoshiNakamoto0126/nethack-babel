use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nethack_babel_data::components::{
    BucStatus, Enchantment, Erosion, KnowledgeState, ObjectCore, ObjectLocation,
};
use nethack_babel_data::{
    GameData, PlayerEvents, PlayerIdentity, PlayerQuestItems, PlayerSkills, loader::load_game_data,
};
use nethack_babel_engine::action::Position;
use nethack_babel_engine::attributes::{AttributeExercise, NaturalAttributes};
use nethack_babel_engine::conduct::ConductState;
use nethack_babel_engine::dungeon::DungeonState;
use nethack_babel_engine::equipment::EquipmentSlots;
use nethack_babel_engine::inventory::Inventory;
use nethack_babel_engine::o_init::AppearanceTable;
use nethack_babel_engine::quest::{QuestNpcRole, QuestState};
use nethack_babel_engine::religion::ReligionState;
use nethack_babel_engine::spells::SpellBook;
use nethack_babel_engine::status::{Intrinsics, StatusEffects};
use nethack_babel_engine::world::Attributes as EngineAttributes;
use nethack_babel_engine::world::{
    ArmorClass, Encumbrance, EncumbranceLevel, ExperienceLevel, GameWorld, HeroSpeed,
    HeroSpeedBonus, HitPoints, Monster, MovementPoints, Name, Nutrition, PlayerCombat, Positioned,
    Power, Speed, Tame,
};
use nethack_babel_engine::{
    npc::{Priest, Shopkeeper},
    turn::sync_current_level_npc_state,
};

// =========================================================================
// Save format version
// =========================================================================

/// Current save format version.  Bump minor for backward-compatible changes,
/// major for breaking changes.
const SAVE_VERSION: [u8; 3] = [1, 0, 0];

/// Magic bytes identifying a NetHack Babel save file.
const SAVE_MAGIC: [u8; 4] = *b"NBSV";

/// Default checkpoint interval in turns.
pub const DEFAULT_CHECKPOINT_INTERVAL: u32 = 100;

// =========================================================================
// SaveReason
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveReason {
    Quit,
    Checkpoint,
    Panic,
}

// =========================================================================
// SaveHeader
// =========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveHeader {
    pub magic: [u8; 4],
    pub version: [u8; 3],
    pub player_name: String,
    pub role: String,
    pub race: String,
    pub depth: String,
    pub turn: u32,
    pub reason: SaveReason,
}

impl SaveHeader {
    /// Create a new save header with the magic number pre-filled.
    pub fn new(
        player_name: String,
        role: String,
        race: String,
        depth: String,
        turn: u32,
        reason: SaveReason,
    ) -> Self {
        Self {
            magic: SAVE_MAGIC,
            version: SAVE_VERSION,
            player_name,
            role,
            race,
            depth,
            turn,
            reason,
        }
    }
}

// =========================================================================
// Serializable snapshot types
// =========================================================================

/// Complete serializable game state.
#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub header: SaveHeader,
    pub turn: u32,
    pub dungeon: DungeonState,
    #[serde(default)]
    pub appearance_table: AppearanceTable,
    pub player: SerializablePlayer,
    pub monsters: Vec<SerializableMonster>,
    pub items: Vec<SerializableItem>,
    pub rng_state: [u8; 32],
    /// Monotonically increasing entity counter, so new entities after load
    /// don't collide with saved creation order values.
    pub next_creation_order: u64,
}

/// Flattened player state, extracted from ECS components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializablePlayer {
    pub position: Position,
    pub hp_current: i32,
    pub hp_max: i32,
    pub pw_current: i32,
    pub pw_max: i32,
    pub experience_level: u8,
    pub strength: u8,
    pub strength_extra: u8,
    pub dexterity: u8,
    pub constitution: u8,
    pub intelligence: u8,
    pub wisdom: u8,
    pub charisma: u8,
    pub armor_class: i32,
    pub speed: u32,
    pub movement_points: i32,
    pub encumbrance: Encumbrance,
    pub hero_speed: HeroSpeed,
    pub nutrition: i32,
    pub name: String,
    // New in v0.3.0:
    pub status_effects: StatusEffects,
    pub intrinsics: Intrinsics,
    pub combat: PlayerCombat,
    pub attribute_exercise: AttributeExercise,
    pub natural_attributes: NaturalAttributes,
    pub spell_book: SpellBook,
    /// Optional role/race/gender/alignment identity component.
    #[serde(default)]
    pub identity: Option<PlayerIdentity>,
    /// Optional weapon skill/practice component.
    #[serde(default)]
    pub skills: Option<PlayerSkills>,
    /// Voluntary challenge conduct counters.
    #[serde(default)]
    pub conduct: ConductState,
    /// Persistent religion/prayer state.
    #[serde(default)]
    pub religion: Option<ReligionState>,
    /// Persistent quest progression state.
    #[serde(default)]
    pub quest: Option<QuestState>,
    /// Cached possession flags for quest / invocation-critical items.
    #[serde(default)]
    pub quest_items: PlayerQuestItems,
    /// Sticky runtime milestone flags.
    #[serde(default)]
    pub player_events: PlayerEvents,
    /// Indices into `SaveData::items` for the player's inventory, in order.
    pub inventory_item_indices: Vec<u32>,
    /// Equipment slot assignments, as indices into `SaveData::items`.
    /// Order: weapon, off_hand, helmet, cloak, body_armor, shield, gloves,
    /// boots, shirt, ring_left, ring_right, amulet.
    pub equipment_indices: [Option<u32>; 12],
}

/// Flattened monster state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableMonster {
    pub position: Position,
    pub hp_current: i32,
    pub hp_max: i32,
    pub speed: u32,
    pub movement_points: i32,
    pub name: String,
    #[serde(default)]
    pub is_tame: bool,
    #[serde(default)]
    pub is_peaceful: bool,
    #[serde(default)]
    pub creation_order: u64,
    #[serde(default)]
    pub priest: Option<Priest>,
    #[serde(default)]
    pub shopkeeper: Option<Shopkeeper>,
    #[serde(default)]
    pub quest_npc_role: Option<QuestNpcRole>,
}

/// Flattened item state with full component data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableItem {
    pub core: ObjectCore,
    pub buc: BucStatus,
    pub knowledge: KnowledgeState,
    pub location: ObjectLocation,
    pub enchantment: Option<i8>,
    pub erosion: Option<SerializableErosion>,
}

/// Erosion data for serialization (avoids depending on Erosion's exact layout).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableErosion {
    pub eroded: u8,
    pub eroded2: u8,
    pub erodeproof: bool,
    pub greased: bool,
}

// =========================================================================
// Checkpoint configuration
// =========================================================================

/// Configuration for automatic checkpoint saves.
#[derive(Debug, Clone)]
pub struct CheckpointConfig {
    /// Save a checkpoint every N turns.
    pub interval: u32,
    /// Whether checkpointing is enabled.
    pub enabled: bool,
    /// Last turn a checkpoint was saved.
    pub last_checkpoint_turn: u32,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            interval: DEFAULT_CHECKPOINT_INTERVAL,
            enabled: true,
            last_checkpoint_turn: 0,
        }
    }
}

impl CheckpointConfig {
    /// Create a config with checkpointing disabled.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Create a config with a custom interval.
    pub fn with_interval(interval: u32) -> Self {
        Self {
            interval,
            ..Self::default()
        }
    }
}

// =========================================================================
// Extract / rebuild helpers
// =========================================================================

/// Extract a `SerializablePlayer` from the game world.
/// `item_index_map` maps ECS Entity → index in the items vec for cross-referencing.
fn extract_player(
    world: &GameWorld,
    item_index_map: &std::collections::HashMap<hecs::Entity, u32>,
) -> SerializablePlayer {
    let entity = world.player();

    let position = world
        .get_component::<Positioned>(entity)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));
    let (hp_current, hp_max) = world
        .get_component::<HitPoints>(entity)
        .map(|h| (h.current, h.max))
        .unwrap_or((1, 1));
    let (pw_current, pw_max) = world
        .get_component::<Power>(entity)
        .map(|p| (p.current, p.max))
        .unwrap_or((0, 0));
    let experience_level = world
        .get_component::<ExperienceLevel>(entity)
        .map(|e| e.0)
        .unwrap_or(1);
    let attrs = world
        .get_component::<EngineAttributes>(entity)
        .map(|a| *a)
        .unwrap_or_default();
    let armor_class = world
        .get_component::<ArmorClass>(entity)
        .map(|a| a.0)
        .unwrap_or(10);
    let speed = world
        .get_component::<Speed>(entity)
        .map(|s| s.0)
        .unwrap_or(12);
    let movement_points = world
        .get_component::<MovementPoints>(entity)
        .map(|m| m.0)
        .unwrap_or(12);
    let encumbrance = world
        .get_component::<EncumbranceLevel>(entity)
        .map(|e| e.0)
        .unwrap_or(Encumbrance::Unencumbered);
    let hero_speed = world
        .get_component::<HeroSpeedBonus>(entity)
        .map(|h| h.0)
        .unwrap_or(HeroSpeed::Normal);
    let nutrition = world
        .get_component::<Nutrition>(entity)
        .map(|n| n.0)
        .unwrap_or(900);
    let name = world.entity_name(entity);

    // New components (v0.3.0).
    // Note: get_component returns Option<hecs::Ref<T>>, so we must
    // dereference explicitly before cloning to avoid cloning the Ref wrapper.
    let status_effects = world
        .get_component::<StatusEffects>(entity)
        .map(|s| (*s).clone())
        .unwrap_or_default();
    let intrinsics = world
        .get_component::<Intrinsics>(entity)
        .map(|i| (*i).clone())
        .unwrap_or_default();
    let combat = world
        .get_component::<PlayerCombat>(entity)
        .map(|c| *c)
        .unwrap_or(PlayerCombat {
            luck: 0,
            uhitinc: 0,
            udaminc: 0,
        });
    let attribute_exercise = world
        .get_component::<AttributeExercise>(entity)
        .map(|a| *a)
        .unwrap_or_default();
    let natural_attributes = world
        .get_component::<NaturalAttributes>(entity)
        .map(|a| *a)
        .unwrap_or_default();
    let spell_book = world
        .get_component::<SpellBook>(entity)
        .map(|s| (*s).clone())
        .unwrap_or_default();
    let identity = world
        .get_component::<PlayerIdentity>(entity)
        .map(|id| (*id).clone());
    let skills = world
        .get_component::<PlayerSkills>(entity)
        .map(|s| (*s).clone());
    let conduct = world
        .get_component::<ConductState>(entity)
        .map(|c| (*c).clone())
        .unwrap_or_default();
    let religion = world
        .get_component::<ReligionState>(entity)
        .map(|state| (*state).clone());
    let quest = world
        .get_component::<QuestState>(entity)
        .map(|state| (*state).clone());
    let quest_items = world
        .get_component::<PlayerQuestItems>(entity)
        .map(|items| (*items).clone())
        .unwrap_or_default();
    let player_events = world
        .get_component::<PlayerEvents>(entity)
        .map(|events| (*events).clone())
        .unwrap_or_default();

    // Map inventory entity refs to item indices.
    let inventory_item_indices: Vec<u32> = match world.get_component::<Inventory>(entity) {
        Some(inv) => inv
            .items
            .iter()
            .filter_map(|e| item_index_map.get(e).copied())
            .collect(),
        None => Vec::new(),
    };

    // Map equipment entity refs to item indices.
    let equip = world.get_component::<EquipmentSlots>(entity);
    let equipment_indices = if let Some(eq) = equip {
        [
            eq.weapon.and_then(|e| item_index_map.get(&e).copied()),
            eq.off_hand.and_then(|e| item_index_map.get(&e).copied()),
            eq.helmet.and_then(|e| item_index_map.get(&e).copied()),
            eq.cloak.and_then(|e| item_index_map.get(&e).copied()),
            eq.body_armor.and_then(|e| item_index_map.get(&e).copied()),
            eq.shield.and_then(|e| item_index_map.get(&e).copied()),
            eq.gloves.and_then(|e| item_index_map.get(&e).copied()),
            eq.boots.and_then(|e| item_index_map.get(&e).copied()),
            eq.shirt.and_then(|e| item_index_map.get(&e).copied()),
            eq.ring_left.and_then(|e| item_index_map.get(&e).copied()),
            eq.ring_right.and_then(|e| item_index_map.get(&e).copied()),
            eq.amulet.and_then(|e| item_index_map.get(&e).copied()),
        ]
    } else {
        [None; 12]
    };

    SerializablePlayer {
        position,
        hp_current,
        hp_max,
        pw_current,
        pw_max,
        experience_level,
        strength: attrs.strength,
        strength_extra: attrs.strength_extra,
        dexterity: attrs.dexterity,
        constitution: attrs.constitution,
        intelligence: attrs.intelligence,
        wisdom: attrs.wisdom,
        charisma: attrs.charisma,
        armor_class,
        speed,
        movement_points,
        encumbrance,
        hero_speed,
        nutrition,
        name,
        status_effects,
        intrinsics,
        combat,
        attribute_exercise,
        natural_attributes,
        spell_book,
        identity,
        skills,
        conduct,
        religion,
        quest,
        quest_items,
        player_events,
        inventory_item_indices,
        equipment_indices,
    }
}

/// Extract all items from the ECS world into serializable form.
/// Returns `(items, entity→index map)` so that inventory/equipment can
/// reference items by index.
fn extract_items(
    world: &GameWorld,
) -> (
    Vec<SerializableItem>,
    std::collections::HashMap<hecs::Entity, u32>,
) {
    let mut items = Vec::new();
    let mut index_map = std::collections::HashMap::new();

    for (entity, (core,)) in world.ecs().query::<(&ObjectCore,)>().iter() {
        let idx = items.len() as u32;
        index_map.insert(entity, idx);

        let buc = world
            .get_component::<BucStatus>(entity)
            .map(|b| (*b).clone())
            .unwrap_or(BucStatus {
                cursed: false,
                blessed: false,
                bknown: false,
            });

        let knowledge = world
            .get_component::<KnowledgeState>(entity)
            .map(|k| (*k).clone())
            .unwrap_or(KnowledgeState {
                known: false,
                dknown: false,
                rknown: false,
                cknown: false,
                lknown: false,
                tknown: false,
            });

        let location = world
            .get_component::<ObjectLocation>(entity)
            .map(|l| (*l).clone())
            .unwrap_or(ObjectLocation::Free);

        let enchantment = world.get_component::<Enchantment>(entity).map(|e| e.spe);

        let erosion = world
            .get_component::<Erosion>(entity)
            .map(|e| SerializableErosion {
                eroded: e.eroded,
                eroded2: e.eroded2,
                erodeproof: e.erodeproof,
                greased: e.greased,
            });

        items.push(SerializableItem {
            core: core.clone(),
            buc,
            knowledge,
            location,
            enchantment,
            erosion,
        });
    }

    (items, index_map)
}

/// Extract all monsters from the ECS world into serializable form.
fn extract_monsters(world: &GameWorld) -> Vec<SerializableMonster> {
    let mut monsters = Vec::new();
    for (entity, (_marker,)) in world.ecs().query::<(&Monster,)>().iter() {
        let position = world
            .get_component::<Positioned>(entity)
            .map(|p| p.0)
            .unwrap_or(Position::new(0, 0));
        let (hp_current, hp_max) = world
            .get_component::<HitPoints>(entity)
            .map(|h| (h.current, h.max))
            .unwrap_or((1, 1));
        let speed = world
            .get_component::<Speed>(entity)
            .map(|s| s.0)
            .unwrap_or(12);
        let movement_points = world
            .get_component::<MovementPoints>(entity)
            .map(|m| m.0)
            .unwrap_or(12);
        let name = world.entity_name(entity);
        let is_tame = world.get_component::<Tame>(entity).is_some();
        let is_peaceful = world
            .get_component::<nethack_babel_engine::world::Peaceful>(entity)
            .is_some();
        let creation_order = world
            .get_component::<nethack_babel_engine::world::CreationOrder>(entity)
            .map(|c| c.0)
            .unwrap_or(0);
        let priest = world.get_component::<Priest>(entity).map(|priest| *priest);
        let shopkeeper = world
            .get_component::<Shopkeeper>(entity)
            .map(|shopkeeper| (*shopkeeper).clone());
        let quest_npc_role = world
            .get_component::<QuestNpcRole>(entity)
            .map(|role| *role);

        monsters.push(SerializableMonster {
            position,
            hp_current,
            hp_max,
            speed,
            movement_points,
            name,
            is_tame,
            is_peaceful,
            creation_order,
            priest,
            shopkeeper,
            quest_npc_role,
        });
    }
    monsters
}

/// Rebuild a `GameWorld` from deserialized `SaveData`.
fn rebuild_world(data: &SaveData) -> GameWorld {
    let p = &data.player;
    let mut world = GameWorld::new(p.position);
    world.appearance_table = data.appearance_table.clone();

    // Overwrite the player's components.
    let player = world.player();
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
        hp.current = p.hp_current;
        hp.max = p.hp_max;
    }
    if let Some(mut pw) = world.get_component_mut::<Power>(player) {
        pw.current = p.pw_current;
        pw.max = p.pw_max;
    }
    if let Some(mut el) = world.get_component_mut::<ExperienceLevel>(player) {
        el.0 = p.experience_level;
    }
    if let Some(mut attrs) = world.get_component_mut::<EngineAttributes>(player) {
        attrs.strength = p.strength;
        attrs.strength_extra = p.strength_extra;
        attrs.dexterity = p.dexterity;
        attrs.constitution = p.constitution;
        attrs.intelligence = p.intelligence;
        attrs.wisdom = p.wisdom;
        attrs.charisma = p.charisma;
    }
    if let Some(mut ac) = world.get_component_mut::<ArmorClass>(player) {
        ac.0 = p.armor_class;
    }
    if let Some(mut spd) = world.get_component_mut::<Speed>(player) {
        spd.0 = p.speed;
    }
    if let Some(mut mp) = world.get_component_mut::<MovementPoints>(player) {
        mp.0 = p.movement_points;
    }
    if let Some(mut enc) = world.get_component_mut::<EncumbranceLevel>(player) {
        enc.0 = p.encumbrance;
    }
    if let Some(mut hsb) = world.get_component_mut::<HeroSpeedBonus>(player) {
        hsb.0 = p.hero_speed;
    }
    if let Some(mut nut) = world.get_component_mut::<Nutrition>(player) {
        nut.0 = p.nutrition;
    }
    if let Some(mut nm) = world.get_component_mut::<Name>(player) {
        nm.0 = p.name.clone();
    }
    // Restore new v0.3.0 components.
    if let Some(mut se) = world.get_component_mut::<StatusEffects>(player) {
        *se = p.status_effects.clone();
    }
    if let Some(mut intr) = world.get_component_mut::<Intrinsics>(player) {
        *intr = p.intrinsics.clone();
    }
    if let Some(mut pc) = world.get_component_mut::<PlayerCombat>(player) {
        *pc = p.combat;
    }
    if let Some(mut ae) = world.get_component_mut::<AttributeExercise>(player) {
        *ae = p.attribute_exercise;
    }
    if let Some(mut na) = world.get_component_mut::<NaturalAttributes>(player) {
        *na = p.natural_attributes;
    }
    if let Some(mut sb) = world.get_component_mut::<SpellBook>(player) {
        *sb = p.spell_book.clone();
    }
    if let Some(identity) = &p.identity {
        if let Some(mut live_identity) = world.get_component_mut::<PlayerIdentity>(player) {
            *live_identity = identity.clone();
        } else {
            let _ = world.ecs_mut().insert_one(player, identity.clone());
        }
    }
    if let Some(skills) = &p.skills {
        if let Some(mut live_skills) = world.get_component_mut::<PlayerSkills>(player) {
            *live_skills = skills.clone();
        } else {
            let _ = world.ecs_mut().insert_one(player, skills.clone());
        }
    }
    if let Some(mut conduct) = world.get_component_mut::<ConductState>(player) {
        *conduct = p.conduct.clone();
    } else {
        let _ = world.ecs_mut().insert_one(player, p.conduct.clone());
    }
    if let Some(religion) = &p.religion {
        if let Some(mut live_religion) = world.get_component_mut::<ReligionState>(player) {
            *live_religion = religion.clone();
        } else {
            let _ = world.ecs_mut().insert_one(player, religion.clone());
        }
    }
    if let Some(quest) = &p.quest {
        if let Some(mut live_quest) = world.get_component_mut::<QuestState>(player) {
            *live_quest = quest.clone();
        } else {
            let _ = world.ecs_mut().insert_one(player, quest.clone());
        }
    }
    if let Some(mut live_items) = world.get_component_mut::<PlayerQuestItems>(player) {
        *live_items = p.quest_items.clone();
    } else {
        let _ = world.ecs_mut().insert_one(player, p.quest_items.clone());
    }
    if let Some(mut live_events) = world.get_component_mut::<PlayerEvents>(player) {
        *live_events = p.player_events.clone();
    } else {
        let _ = world.ecs_mut().insert_one(player, p.player_events.clone());
    }

    // Set the turn counter.
    while world.turn() < data.turn {
        world.advance_turn();
    }
    world.set_next_creation_order_value(data.next_creation_order);

    // Restore dungeon state.
    *world.dungeon_mut() = data.dungeon.clone();

    // Spawn items first so we can build an index→Entity map.
    let mut item_entities: Vec<hecs::Entity> = Vec::with_capacity(data.items.len());
    for item_data in &data.items {
        let mut entity_builder = hecs::EntityBuilder::new();
        entity_builder.add(item_data.core.clone());
        entity_builder.add(item_data.buc.clone());
        entity_builder.add(item_data.knowledge.clone());
        entity_builder.add(item_data.location.clone());
        if let Some(spe) = item_data.enchantment {
            entity_builder.add(Enchantment { spe });
        }
        if let Some(ref ero) = item_data.erosion {
            entity_builder.add(Erosion {
                eroded: ero.eroded,
                eroded2: ero.eroded2,
                erodeproof: ero.erodeproof,
                greased: ero.greased,
            });
        }
        let entity = world.ecs_mut().spawn(entity_builder.build());
        item_entities.push(entity);
    }

    // Rebuild player inventory from saved indices.
    {
        let inv_entities: Vec<hecs::Entity> = p
            .inventory_item_indices
            .iter()
            .filter_map(|&idx| item_entities.get(idx as usize).copied())
            .collect();
        if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
            inv.items = inv_entities;
        }
    }

    // Rebuild player equipment from saved indices.
    {
        let slot_entities: [Option<hecs::Entity>; 12] = std::array::from_fn(|i| {
            p.equipment_indices[i].and_then(|idx| item_entities.get(idx as usize).copied())
        });
        if let Some(mut eq) = world.get_component_mut::<EquipmentSlots>(player) {
            eq.weapon = slot_entities[0];
            eq.off_hand = slot_entities[1];
            eq.helmet = slot_entities[2];
            eq.cloak = slot_entities[3];
            eq.body_armor = slot_entities[4];
            eq.shield = slot_entities[5];
            eq.gloves = slot_entities[6];
            eq.boots = slot_entities[7];
            eq.shirt = slot_entities[8];
            eq.ring_left = slot_entities[9];
            eq.ring_right = slot_entities[10];
            eq.amulet = slot_entities[11];
        }
    }

    // Spawn monsters.
    for m in &data.monsters {
        let entity = world.spawn((
            Monster,
            Positioned(m.position),
            HitPoints {
                current: m.hp_current,
                max: m.hp_max,
            },
            Speed(m.speed),
            MovementPoints(m.movement_points),
            Name(m.name.clone()),
            nethack_babel_engine::world::CreationOrder(m.creation_order),
        ));

        if m.is_tame {
            let _ = world.ecs_mut().insert_one(entity, Tame);
        }
        if m.is_peaceful {
            let _ = world
                .ecs_mut()
                .insert_one(entity, nethack_babel_engine::world::Peaceful);
        }
        if let Some(priest) = m.priest {
            let _ = world.ecs_mut().insert_one(entity, priest);
        }
        if let Some(shopkeeper) = &m.shopkeeper {
            let _ = world.ecs_mut().insert_one(entity, shopkeeper.clone());
        }
        if let Some(role) = m.quest_npc_role {
            let _ = world.ecs_mut().insert_one(entity, role);
        }
    }

    sync_current_level_npc_state(&mut world);

    world
}

fn restore_runtime_catalogs(world: &mut GameWorld, data: &GameData) {
    world.set_spawn_catalogs(data.monsters.clone(), data.objects.clone());
}

#[allow(dead_code)]
fn resolved_runtime_data_dir() -> anyhow::Result<PathBuf> {
    if let Ok(path) = std::env::var("NETHACK_BABEL_DATA_DIR") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Ok(path);
        }
    }

    let manifest_data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    if manifest_data.is_dir() {
        return Ok(manifest_data);
    }

    let cwd_data = Path::new("data");
    if cwd_data.is_dir() {
        return Ok(cwd_data.to_path_buf());
    }

    if let Ok(home) = std::env::var("HOME") {
        let user_data = PathBuf::from(home).join(".nethack-babel").join("data");
        if user_data.is_dir() {
            return Ok(user_data);
        }
    }

    for shared in [
        Path::new("/usr/local/share/nethack-babel/data"),
        Path::new("/usr/share/nethack-babel/data"),
    ] {
        if shared.is_dir() {
            return Ok(shared.to_path_buf());
        }
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        for candidate in [
            exe_dir.join("data"),
            exe_dir
                .parent()
                .map(|parent| parent.join("data"))
                .unwrap_or_default(),
        ] {
            if candidate.is_dir() {
                return Ok(candidate);
            }
        }
    }

    anyhow::bail!(
        "Data directory not found while restoring save. Set NETHACK_BABEL_DATA_DIR or install data files."
    )
}

#[allow(dead_code)]
fn load_runtime_game_data() -> anyhow::Result<GameData> {
    let data_dir = resolved_runtime_data_dir()?;
    load_game_data(&data_dir).map_err(|e| {
        anyhow::anyhow!(
            "Failed to load game data from {}: {}",
            data_dir.display(),
            e
        )
    })
}

fn read_save_data(path: &Path, label: &str) -> anyhow::Result<SaveData> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;

    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if magic != SAVE_MAGIC {
        anyhow::bail!(
            "Invalid {label}: expected magic {:?}, got {:?}",
            SAVE_MAGIC,
            magic
        );
    }

    let mut version = [0u8; 3];
    file.read_exact(&mut version)?;
    if !is_compatible_version(version) {
        anyhow::bail!(
            "{label} version mismatch: file has {}.{}.{}, binary expects {}.{}.{}",
            version[0],
            version[1],
            version[2],
            SAVE_VERSION[0],
            SAVE_VERSION[1],
            SAVE_VERSION[2],
        );
    }

    let mut len_buf = [0u8; 8];
    file.read_exact(&mut len_buf)?;
    let payload_len = u64::from_le_bytes(len_buf) as usize;
    if payload_len > 256 * 1024 * 1024 {
        anyhow::bail!(
            "{label} payload too large: {} bytes (max 256 MB)",
            payload_len
        );
    }

    let mut payload = vec![0u8; payload_len];
    file.read_exact(&mut payload)?;

    let (save_data, _): (SaveData, _) =
        bincode::serde::decode_from_slice(&payload, bincode::config::standard())?;

    if save_data.header.magic != SAVE_MAGIC {
        anyhow::bail!("Corrupt {label}: embedded header has wrong magic bytes");
    }

    Ok(save_data)
}

// =========================================================================
// Public API
// =========================================================================

/// Save the full game state to a file at `path`.
///
/// Extracts all entities from the hecs `World` into a flat `SaveData`,
/// serializes with bincode v2 (via serde), and writes to disk with a
/// length-prefixed header for fast validation on load.
pub fn save_game(
    world: &GameWorld,
    path: &Path,
    reason: SaveReason,
    rng_state: [u8; 32],
) -> anyhow::Result<()> {
    use std::io::Write;

    let (items, item_index_map) = extract_items(world);
    let player_data = extract_player(world, &item_index_map);
    let monsters = extract_monsters(world);

    let header = SaveHeader::new(
        player_data.name.clone(),
        String::new(), // role — not yet tracked in simple ECS
        String::new(), // race — not yet tracked in simple ECS
        format!("Dlvl:{}", world.dungeon().depth),
        world.turn(),
        reason,
    );

    let save_data = SaveData {
        header: header.clone(),
        turn: world.turn(),
        dungeon: world.dungeon().clone(),
        appearance_table: world.appearance_table.clone(),
        player: player_data,
        monsters,
        items,
        rng_state,
        next_creation_order: world.next_creation_order_value(),
    };

    // Serialize with bincode v2 using serde compat layer.
    let encoded = bincode::serde::encode_to_vec(&save_data, bincode::config::standard())?;

    // Atomic write: write to a temp file first, then rename.
    let tmp_path = path.with_extension("nbsv.tmp");
    {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(&SAVE_MAGIC)?;
        file.write_all(&SAVE_VERSION)?;
        file.write_all(&(encoded.len() as u64).to_le_bytes())?;
        file.write_all(&encoded)?;
        file.flush()?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp_path, path)?;

    tracing::info!("Game saved to {}", path.display());
    Ok(())
}

/// Load the full game state from a file at `path`.
///
/// Reads and validates the header (magic bytes, version), deserializes the
/// `SaveData` payload, and rebuilds a `GameWorld` from it.
///
/// On success the save file is deleted to prevent save-scumming.
///
/// Returns `(GameWorld, turn, depth, rng_state)`.
#[allow(dead_code)]
pub fn load_game(path: &Path) -> anyhow::Result<(GameWorld, u32, i32, [u8; 32])> {
    let data = load_runtime_game_data()?;
    load_game_with_data(path, &data)
}

/// Load a save using already-loaded runtime data.
pub fn load_game_with_data(
    path: &Path,
    data: &GameData,
) -> anyhow::Result<(GameWorld, u32, i32, [u8; 32])> {
    let save_data = read_save_data(path, "save file")?;
    let mut world = rebuild_world(&save_data);
    restore_runtime_catalogs(&mut world, data);
    let turn = save_data.turn;
    let depth = save_data.dungeon.depth;
    let rng_state = save_data.rng_state;

    if let Err(e) = std::fs::remove_file(path) {
        tracing::warn!("Failed to delete save file after load: {e}");
    }

    tracing::info!("Game loaded from {}", path.display());
    Ok((world, turn, depth, rng_state))
}

/// Check whether a save file exists and has valid magic bytes without loading.
pub fn save_file_exists(path: &Path) -> bool {
    use std::io::Read;

    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic).is_ok() && magic == SAVE_MAGIC
}

/// Return the default save file path for a given player name.
pub fn save_file_path(player_name: &str) -> PathBuf {
    let mut path = dirs_or_default();
    path.push(format!("{player_name}.nbsv"));
    path
}

/// Return the checkpoint file path for a given player name.
pub fn checkpoint_path(player_name: &str) -> PathBuf {
    let mut path = dirs_or_default();
    path.push(format!("{player_name}.ckpt.nbsv"));
    path
}

/// Determine the save directory, preferring XDG conventions.
fn dirs_or_default() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        let mut p = PathBuf::from(dir);
        p.push("nethack-babel");
        p
    } else if let Ok(home) = std::env::var("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".local/share/nethack-babel");
        p
    } else {
        PathBuf::from(".")
    }
}

// =========================================================================
// Version compatibility
// =========================================================================

/// Check if a file version is compatible with the current binary.
/// Same major version is required; minor version of file must be <= current.
fn is_compatible_version(file_version: [u8; 3]) -> bool {
    file_version[0] == SAVE_VERSION[0] && file_version[1] <= SAVE_VERSION[1]
}

// =========================================================================
// Checkpoint system
// =========================================================================

/// Check if a checkpoint save is due and perform it if so.
///
/// Returns `true` if a checkpoint was written.
pub fn maybe_checkpoint(
    world: &GameWorld,
    config: &mut CheckpointConfig,
    player_name: &str,
    rng_state: [u8; 32],
) -> bool {
    if !config.enabled {
        return false;
    }

    let current_turn = world.turn();
    if current_turn < config.last_checkpoint_turn + config.interval {
        return false;
    }

    let path = checkpoint_path(player_name);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match save_game(world, &path, SaveReason::Checkpoint, rng_state) {
        Ok(()) => {
            config.last_checkpoint_turn = current_turn;
            tracing::info!(
                "Checkpoint saved at turn {} to {}",
                current_turn,
                path.display()
            );
            true
        }
        Err(e) => {
            tracing::warn!("Checkpoint save failed: {e}");
            false
        }
    }
}

// =========================================================================
// Crash recovery
// =========================================================================

/// Shared state for the panic hook to access.
struct PanicSaveState {
    world_ptr: *const GameWorld,
    player_name: String,
    rng_state: [u8; 32],
}

// SAFETY: The pointer is only read during a panic (process is about to exit).
unsafe impl Send for PanicSaveState {}
unsafe impl Sync for PanicSaveState {}

static PANIC_STATE: Mutex<Option<PanicSaveState>> = Mutex::new(None);

/// Register the game state for emergency save on panic.
///
/// The `world` reference must remain valid for the duration of the game.
/// Call `unregister_panic_save()` before dropping the world.
///
/// # Safety
///
/// The caller must ensure `world` outlives any potential panic.
pub unsafe fn register_panic_save(world: &GameWorld, player_name: &str, rng_state: [u8; 32]) {
    let state = PanicSaveState {
        world_ptr: world as *const GameWorld,
        player_name: player_name.to_string(),
        rng_state,
    };
    *PANIC_STATE.lock().unwrap() = Some(state);
}

/// Unregister the panic save hook.
pub fn unregister_panic_save() {
    *PANIC_STATE.lock().unwrap() = None;
}

/// Install a panic hook that attempts an emergency save before aborting.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Attempt emergency save.
        if let Ok(guard) = PANIC_STATE.lock() {
            if let Some(ref state) = *guard {
                // SAFETY: We trust the caller of register_panic_save
                // to keep the world alive.
                let world = unsafe { &*state.world_ptr };
                let path = save_file_path(&state.player_name).with_extension("panic.nbsv");
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = save_game(world, &path, SaveReason::Panic, state.rng_state);
                eprintln!("Emergency save written to {}", path.display());
            }
        }
        // Continue with the default panic handler.
        default_hook(info);
    }));
}

/// Attempt to recover from a panic save file.
///
/// Looks for `<player_name>.panic.nbsv` and loads it if found.
/// Unlike normal load, the panic file is renamed to `.recovered.nbsv`
/// instead of deleted, so the player can inspect it.
#[allow(dead_code)]
pub fn try_recover(player_name: &str) -> anyhow::Result<Option<(GameWorld, u32, i32, [u8; 32])>> {
    let data = load_runtime_game_data()?;
    try_recover_with_data(player_name, &data)
}

pub fn try_recover_with_data(
    player_name: &str,
    data: &GameData,
) -> anyhow::Result<Option<(GameWorld, u32, i32, [u8; 32])>> {
    let panic_path = save_file_path(player_name).with_extension("panic.nbsv");

    if !panic_path.exists() {
        return Ok(None);
    }

    let save_data = read_save_data(&panic_path, "panic save file")?;
    let mut world = rebuild_world(&save_data);
    restore_runtime_catalogs(&mut world, data);
    let turn = save_data.turn;
    let depth = save_data.dungeon.depth;
    let rng_state = save_data.rng_state;

    // Rename to .recovered.nbsv instead of deleting.
    let recovered_path = panic_path.with_extension("recovered.nbsv");
    let _ = std::fs::rename(&panic_path, &recovered_path);
    tracing::info!(
        "Recovered from panic save (renamed to {})",
        recovered_path.display()
    );

    Ok(Some((world, turn, depth, rng_state)))
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    };

    use nethack_babel_data::{
        Alignment, GameData, Gender, Handedness, RaceId,
        loader::load_game_data,
        schema::{ObjectClass, ObjectTypeId},
    };
    use nethack_babel_engine::{
        action::{Direction, PlayerAction},
        artifacts::find_artifact_by_name,
        dungeon::{DungeonBranch, LevelMap, Terrain},
        event::{DeathCause, EngineEvent},
        role::Role,
        turn::resolve_turn,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    /// Helper: create a unique temp directory for test save files.
    fn test_dir(name: &str) -> PathBuf {
        let unique = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir()
            .join("nethack-babel-save-test")
            .join(format!("{name}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Helper: create a default game world.
    fn make_world() -> GameWorld {
        GameWorld::new(Position::new(40, 10))
    }

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn test_game_data() -> &'static GameData {
        static DATA: OnceLock<GameData> = OnceLock::new();
        DATA.get_or_init(|| {
            load_game_data(&data_dir())
                .unwrap_or_else(|e| panic!("failed to load test game data: {}", e))
        })
    }

    fn install_test_catalogs(world: &mut GameWorld) {
        let data = test_game_data();
        world.set_spawn_catalogs(data.monsters.clone(), data.objects.clone());
    }

    fn make_stair_world(branch: DungeonBranch, depth: i32, player_terrain: Terrain) -> GameWorld {
        let mut world = GameWorld::new(Position::new(5, 5));
        install_test_catalogs(&mut world);
        world.dungeon_mut().branch = branch;
        world.dungeon_mut().depth = depth;
        for y in 3..=7 {
            for x in 3..=7 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(5, 5), player_terrain);
        world
    }

    fn wizard_identity() -> PlayerIdentity {
        PlayerIdentity {
            name: "tester".to_string(),
            role: Role::Wizard.to_id(),
            race: RaceId(0),
            gender: Gender::Male,
            alignment: Alignment::Chaotic,
            alignment_base: [Alignment::Chaotic, Alignment::Chaotic],
            handedness: Handedness::RightHanded,
        }
    }

    fn save_and_reload_world(
        label: &str,
        world: &GameWorld,
        rng_state: [u8; 32],
    ) -> (GameWorld, [u8; 32]) {
        let dir = test_dir(label);
        let path = dir.join(format!("{label}.nbsv"));
        save_game(world, &path, SaveReason::Checkpoint, rng_state).unwrap();
        let (loaded, _turn, _depth, loaded_rng) = load_game(&path).unwrap();
        assert!(
            !loaded.monster_catalog().is_empty(),
            "loaded worlds should restore the monster catalog automatically"
        );
        assert!(
            !loaded.object_catalog().is_empty(),
            "loaded worlds should restore the object catalog automatically"
        );
        let _ = std::fs::remove_dir_all(&dir);
        (loaded, loaded_rng)
    }

    fn set_player_position(world: &mut GameWorld, pos: Position) {
        if let Some(mut player_pos) = world.get_component_mut::<Positioned>(world.player()) {
            player_pos.0 = pos;
        }
    }

    fn find_terrain(map: &LevelMap, terrain: Terrain) -> Option<Position> {
        for y in 0..map.height {
            for x in 0..map.width {
                let pos = Position::new(x as i32, y as i32);
                if map.get(pos).is_some_and(|cell| cell.terrain == terrain) {
                    return Some(pos);
                }
            }
        }
        None
    }

    fn count_monsters_named(world: &GameWorld, name: &str) -> usize {
        world
            .ecs()
            .query::<(&Monster, &Name)>()
            .iter()
            .filter(|(_, (_monster, monster_name))| monster_name.0.eq_ignore_ascii_case(name))
            .count()
    }

    fn find_monster_named(world: &GameWorld, name: &str) -> Option<hecs::Entity> {
        world.ecs().query::<(&Monster, &Name)>().iter().find_map(
            |(entity, (_monster, monster_name))| {
                monster_name.0.eq_ignore_ascii_case(name).then_some(entity)
            },
        )
    }

    fn count_objects_with_artifact_name(world: &GameWorld, artifact_name: &str) -> usize {
        let artifact = find_artifact_by_name(artifact_name)
            .unwrap_or_else(|| panic!("missing artifact definition for {}", artifact_name));
        world
            .ecs()
            .query::<&ObjectCore>()
            .iter()
            .filter(|(_, core)| core.artifact == Some(artifact.id))
            .count()
    }

    fn spawn_inventory_object_by_name(
        world: &mut GameWorld,
        name: &str,
        letter: char,
    ) -> hecs::Entity {
        let data = test_game_data();
        let object_def = data
            .objects
            .iter()
            .find(|def| def.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("{name} should exist in the object catalog"));
        let item = world.spawn((
            ObjectCore {
                otyp: object_def.id,
                object_class: object_def.class,
                quantity: 1,
                weight: object_def.weight as u32,
                age: 0,
                inv_letter: Some(letter),
                artifact: None,
            },
            ObjectLocation::Inventory,
            Name(name.to_string()),
        ));
        let player = world.player();
        if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
            inv.items.push(item);
        }
        item
    }

    fn spawn_inventory_gold(world: &mut GameWorld, amount: u32, letter: char) -> hecs::Entity {
        let item = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Coin,
                quantity: amount as i32,
                weight: 1,
                age: 0,
                inv_letter: Some(letter),
                artifact: None,
            },
            ObjectLocation::Inventory,
        ));
        let player = world.player();
        if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
            inv.items.push(item);
        }
        item
    }

    fn spawn_full_monster(
        world: &mut GameWorld,
        pos: Position,
        name: &str,
        hp: i32,
    ) -> hecs::Entity {
        world.spawn((
            Monster,
            Positioned(pos),
            Name(name.to_string()),
            HitPoints {
                current: hp,
                max: hp,
            },
            Speed(12),
            MovementPoints(0),
            nethack_babel_engine::world::DisplaySymbol {
                symbol: '@',
                color: nethack_babel_data::Color::Green,
            },
        ))
    }

    fn adjacent_walkable_step(map: &LevelMap, target: Position) -> Option<(Position, Direction)> {
        for direction in [
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
            Direction::NorthWest,
            Direction::NorthEast,
            Direction::SouthWest,
            Direction::SouthEast,
        ] {
            let origin = match direction {
                Direction::North => Position::new(target.x, target.y + 1),
                Direction::South => Position::new(target.x, target.y - 1),
                Direction::West => Position::new(target.x + 1, target.y),
                Direction::East => Position::new(target.x - 1, target.y),
                Direction::NorthWest => Position::new(target.x + 1, target.y + 1),
                Direction::NorthEast => Position::new(target.x - 1, target.y + 1),
                Direction::SouthWest => Position::new(target.x + 1, target.y - 1),
                Direction::SouthEast => Position::new(target.x - 1, target.y - 1),
                Direction::Up | Direction::Down | Direction::Self_ => continue,
            };
            if map
                .get(origin)
                .is_some_and(|cell| cell.terrain.is_walkable())
            {
                return Some((origin, direction));
            }
        }
        None
    }

    fn move_player_onto_magic_portal(
        world: &mut GameWorld,
        rng: &mut impl rand::Rng,
    ) -> Vec<EngineEvent> {
        let portal_pos = find_terrain(&world.dungeon().current_level, Terrain::MagicPortal)
            .expect("current level should expose a magic portal");
        let (entry_pos, direction) =
            adjacent_walkable_step(&world.dungeon().current_level, portal_pos)
                .expect("magic portal should have an adjacent walkable entry tile");
        set_player_position(world, entry_pos);
        resolve_turn(world, PlayerAction::Move { direction }, rng)
    }

    fn wizard_story_religion(world: &GameWorld, player: hecs::Entity) -> ReligionState {
        let hp = world
            .get_component::<HitPoints>(player)
            .map(|hp| *hp)
            .unwrap_or(HitPoints {
                current: 16,
                max: 16,
            });
        let pw = world
            .get_component::<Power>(player)
            .map(|pw| *pw)
            .unwrap_or(Power { current: 4, max: 4 });
        ReligionState {
            alignment: Alignment::Chaotic,
            alignment_record: 10,
            god_anger: 0,
            god_gifts: 0,
            blessed_amount: 0,
            bless_cooldown: 0,
            crowned: false,
            demigod: false,
            turn: world.turn(),
            experience_level: 14,
            current_hp: hp.current,
            max_hp: hp.max,
            current_pw: pw.current,
            max_pw: pw.max,
            nutrition: 900,
            luck: 0,
            luck_bonus: 0,
            has_luckstone: false,
            luckstone_blessed: false,
            luckstone_cursed: false,
            in_gehennom: false,
            is_undead: false,
            is_demon: false,
            original_alignment: Alignment::Chaotic,
            has_converted: false,
            alignment_abuse: 0,
        }
    }

    fn monk_identity() -> PlayerIdentity {
        PlayerIdentity {
            name: "tester".to_string(),
            role: nethack_babel_data::RoleId(nethack_babel_engine::religion::roles::MONK),
            race: RaceId(0),
            gender: Gender::Male,
            alignment: Alignment::Lawful,
            alignment_base: [Alignment::Lawful, Alignment::Lawful],
            handedness: Handedness::RightHanded,
        }
    }

    #[derive(Clone, Copy)]
    enum SaveStoryTraversalScenario {
        QuestClosure,
        QuestLeaderAnger,
        ShopkeeperFollow,
        ShopkeeperPayoff,
        TempleWrath,
        TempleCalm,
        EndgameAscension,
    }

    impl SaveStoryTraversalScenario {
        fn label(self) -> &'static str {
            match self {
                SaveStoryTraversalScenario::QuestClosure => "quest-closure",
                SaveStoryTraversalScenario::QuestLeaderAnger => "quest-leader-anger",
                SaveStoryTraversalScenario::ShopkeeperFollow => "shopkeeper-follow",
                SaveStoryTraversalScenario::ShopkeeperPayoff => "shopkeeper-payoff",
                SaveStoryTraversalScenario::TempleWrath => "temple-wrath",
                SaveStoryTraversalScenario::TempleCalm => "temple-calm",
                SaveStoryTraversalScenario::EndgameAscension => "endgame-ascension",
            }
        }
    }

    fn run_round_trip_story_traversal_scenario(
        scenario: SaveStoryTraversalScenario,
    ) -> (GameWorld, Vec<EngineEvent>) {
        match scenario {
            SaveStoryTraversalScenario::QuestClosure => {
                let mut world = make_stair_world(DungeonBranch::Quest, 1, Terrain::StairsDown);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, wizard_identity())
                    .expect("player should accept wizard identity");
                let religion = wizard_story_religion(&world, player);
                world
                    .ecs_mut()
                    .insert_one(player, religion)
                    .expect("player should accept religion state");
                if let Some(mut level) = world.get_component_mut::<ExperienceLevel>(player) {
                    level.0 = 14;
                }
                world.spawn((
                    Monster,
                    Positioned(Position::new(6, 5)),
                    Name("Neferet the Green".to_string()),
                    HitPoints {
                        current: 30,
                        max: 30,
                    },
                    Speed(12),
                    nethack_babel_engine::world::DisplaySymbol {
                        symbol: '@',
                        color: nethack_babel_data::Color::Green,
                    },
                    MovementPoints(12),
                ));

                let mut rng = Pcg64::seed_from_u64(7101);
                let assign_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                assert!(assign_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "quest-assigned"
                )));

                for expected_depth in 2..=7 {
                    if expected_depth > 2 {
                        let stairs_down =
                            find_terrain(&world.dungeon().current_level, Terrain::StairsDown)
                                .expect("quest traversal should preserve stairs down");
                        set_player_position(&mut world, stairs_down);
                    }
                    let events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                    assert!(
                        events
                            .iter()
                            .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                        "{} should descend into quest depth {} before save/load",
                        scenario.label(),
                        expected_depth
                    );
                }

                let eye = find_artifact_by_name("The Eye of the Aethiopica")
                    .expect("wizard quest artifact should exist");
                let artifact_entity = world
                    .ecs()
                    .query::<&ObjectCore>()
                    .iter()
                    .find_map(|(entity, core)| (core.artifact == Some(eye.id)).then_some(entity))
                    .expect("quest goal should place the Eye artifact");
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(artifact_entity) {
                    *loc = ObjectLocation::Inventory;
                }
                if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
                    inv.items.push(artifact_entity);
                }

                let dark_one = world
                    .ecs()
                    .query::<(&Monster, &Name)>()
                    .iter()
                    .find_map(|(entity, (_monster, name))| {
                        name.0.eq_ignore_ascii_case("Dark One").then_some(entity)
                    })
                    .expect("quest goal should spawn the Dark One");
                world
                    .despawn(dark_one)
                    .expect("nemesis should despawn cleanly");
                let _ = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-quest", &world, [21u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);

                for expected_depth in (1..=6).rev() {
                    let stairs_up =
                        find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
                            .expect("quest traversal should preserve stairs up after save/load");
                    set_player_position(&mut loaded, stairs_up);
                    let events = resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);
                    assert!(
                        events
                            .iter()
                            .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                        "{} should ascend into quest depth {} after save/load",
                        scenario.label(),
                        expected_depth
                    );
                }

                let leader_events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (loaded, leader_events)
            }
            SaveStoryTraversalScenario::QuestLeaderAnger => {
                let mut world = make_stair_world(DungeonBranch::Quest, 1, Terrain::StairsDown);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, wizard_identity())
                    .expect("player should accept wizard identity");
                let leader =
                    spawn_full_monster(&mut world, Position::new(6, 5), "Neferet the Green", 40);
                world
                    .ecs_mut()
                    .insert_one(leader, nethack_babel_engine::world::Peaceful)
                    .expect("leader should accept peaceful marker");

                let mut rng = Pcg64::seed_from_u64(7103);
                let attack_events = resolve_turn(
                    &mut world,
                    PlayerAction::FightDirection {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                assert!(attack_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::MeleeHit { defender, .. }
                        | EngineEvent::MeleeMiss { defender, .. }
                        if *defender == leader
                )));

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-quest-anger", &world, [23u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let blocked_events = resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);
                (loaded, blocked_events)
            }
            SaveStoryTraversalScenario::ShopkeeperFollow => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 20);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, nethack_babel_engine::world::Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(nethack_babel_engine::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        nethack_babel_engine::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                let unpaid_item = world.spawn((
                    ObjectCore {
                        otyp: ObjectTypeId(0),
                        object_class: ObjectClass::Tool,
                        quantity: 1,
                        weight: 10,
                        age: 0,
                        inv_letter: Some('u'),
                        artifact: None,
                    },
                    ObjectLocation::Floor {
                        x: 6,
                        y: 5,
                        level: world.dungeon().current_data_dungeon_level(),
                    },
                ));
                assert!(
                    world.dungeon_mut().shop_rooms[0]
                        .bill
                        .add(unpaid_item, 100, 1),
                    "shop bill should accept an unpaid entry"
                );

                let mut rng = Pcg64::seed_from_u64(7104);
                let _leave_events = resolve_turn(
                    &mut world,
                    PlayerAction::Move {
                        direction: Direction::West,
                    },
                    &mut rng,
                );

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shopkeeper-follow", &world, [27u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(&mut loaded, PlayerAction::Rest, &mut rng);
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopkeeperPayoff => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let _gold = spawn_inventory_gold(&mut world, 150, 'g');
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 20);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, nethack_babel_engine::world::Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(nethack_babel_engine::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        nethack_babel_engine::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                world.dungeon_mut().shop_rooms[0].angry = true;
                world.dungeon_mut().shop_rooms[0].surcharge = true;
                let unpaid_item = world.spawn((
                    ObjectCore {
                        otyp: ObjectTypeId(0),
                        object_class: ObjectClass::Tool,
                        quantity: 1,
                        weight: 10,
                        age: 0,
                        inv_letter: Some('u'),
                        artifact: None,
                    },
                    ObjectLocation::Floor {
                        x: 6,
                        y: 5,
                        level: world.dungeon().current_data_dungeon_level(),
                    },
                ));
                assert!(
                    world.dungeon_mut().shop_rooms[0]
                        .bill
                        .add(unpaid_item, 100, 1),
                    "shop bill should accept a payable entry"
                );

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shopkeeper-payoff", &world, [29u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(&mut loaded, PlayerAction::Pay, &mut rng);
                (loaded, events)
            }
            SaveStoryTraversalScenario::TempleWrath => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, wizard_identity())
                    .expect("player should accept wizard identity");
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(priest, nethack_babel_engine::world::Peaceful)
                    .expect("priest should accept peaceful marker");

                let mut rng = Pcg64::seed_from_u64(7105);
                let _attack_events = resolve_turn(
                    &mut world,
                    PlayerAction::FightDirection {
                        direction: Direction::East,
                    },
                    &mut rng,
                );

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-temple-wrath", &world, [28u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::TempleCalm => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let mut religion = wizard_story_religion(&world, player);
                religion.alignment = Alignment::Lawful;
                religion.original_alignment = Alignment::Lawful;
                religion.alignment_record = 10;
                religion.bless_cooldown = 0;
                world
                    .ecs_mut()
                    .insert_one(player, religion)
                    .expect("player should accept religion state");
                let altar_pos = Position::new(5, 5);
                set_player_position(&mut world, altar_pos);
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(altar_pos, Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(
                        priest,
                        Priest {
                            alignment: Alignment::Lawful,
                            has_shrine: true,
                            is_high_priest: false,
                            angry: true,
                        },
                    )
                    .expect("priest should accept explicit runtime state");

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-temple-calm", &world, [30u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(&mut loaded, PlayerAction::Pray, &mut rng);
                (loaded, events)
            }
            SaveStoryTraversalScenario::EndgameAscension => {
                let mut world = make_stair_world(DungeonBranch::Gehennom, 20, Terrain::StairsDown);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, wizard_identity())
                    .expect("player should accept wizard identity");
                let religion = wizard_story_religion(&world, player);
                world
                    .ecs_mut()
                    .insert_one(player, religion)
                    .expect("player should accept religion state");

                let mut rng = Pcg64::seed_from_u64(7102);
                let descend_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    descend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );
                assert_eq!(world.dungeon().depth, 21);

                let invocation_pos = world
                    .dungeon()
                    .trap_map
                    .traps
                    .iter()
                    .find(|trap| trap.trap_type == nethack_babel_data::TrapType::VibratingSquare)
                    .map(|trap| trap.pos)
                    .expect("Gehennom 21 should expose a vibrating square before invocation");
                set_player_position(&mut world, invocation_pos);

                let bell = spawn_inventory_object_by_name(&mut world, "Bell of Opening", 'b');
                let candelabrum =
                    spawn_inventory_object_by_name(&mut world, "Candelabrum of Invocation", 'c');
                let book = spawn_inventory_object_by_name(&mut world, "Book of the Dead", 'd');
                let current_turn = world.turn() as i64;
                if let Some(mut core) = world.get_component_mut::<ObjectCore>(bell) {
                    core.age = current_turn;
                }
                world
                    .ecs_mut()
                    .insert_one(candelabrum, Enchantment { spe: 7 })
                    .expect("candelabrum should accept candle count");
                world
                    .ecs_mut()
                    .insert_one(
                        candelabrum,
                        nethack_babel_data::LightSource {
                            lit: true,
                            recharged: 0,
                        },
                    )
                    .expect("candelabrum should accept light state");

                let invocation_events = resolve_turn(
                    &mut world,
                    PlayerAction::Read { item: Some(book) },
                    &mut rng,
                );
                assert!(invocation_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "invocation-complete"
                )));

                for expected_depth in 1..=5 {
                    let portal_events = move_player_onto_magic_portal(&mut world, &mut rng);
                    assert!(
                        portal_events
                            .iter()
                            .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                        "{} should traverse into Endgame depth {} before save/load",
                        scenario.label(),
                        expected_depth
                    );
                }
                assert_eq!(world.dungeon().branch, DungeonBranch::Endgame);
                assert_eq!(world.dungeon().depth, 5);
                let angel_count = count_monsters_named(&world, "Angel");
                assert_eq!(
                    angel_count, 1,
                    "Astral entry should spawn one guardian angel"
                );

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-endgame", &world, [22u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                assert_eq!(count_monsters_named(&loaded, "Angel"), angel_count);

                let mut altar_positions = Vec::new();
                for y in 0..loaded.dungeon().current_level.height {
                    for x in 0..loaded.dungeon().current_level.width {
                        let pos = Position::new(x as i32, y as i32);
                        if loaded
                            .dungeon()
                            .current_level
                            .get(pos)
                            .is_some_and(|cell| cell.terrain == Terrain::Altar)
                        {
                            altar_positions.push(pos);
                        }
                    }
                }
                altar_positions.sort_by_key(|pos| pos.x);
                let chaotic_altar = *altar_positions
                    .last()
                    .expect("Astral Plane should have a chaotic altar");
                set_player_position(&mut loaded, chaotic_altar);
                let amulet = spawn_inventory_object_by_name(&mut loaded, "Amulet of Yendor", 'a');

                let offer_events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Offer { item: Some(amulet) },
                    &mut rng,
                );
                (loaded, offer_events)
            }
        }
    }

    // ----- Basic round-trip --------------------------------------------------

    #[test]
    fn round_trip_empty_world() {
        let dir = test_dir("empty_v3");
        let path = dir.join("empty.nbsv");

        let world = make_world();
        let rng_state = [42u8; 32];

        save_game(&world, &path, SaveReason::Checkpoint, rng_state).unwrap();
        assert!(path.exists());

        let (loaded, turn, depth, loaded_rng) = load_game(&path).unwrap();

        // File deleted after load.
        assert!(!path.exists());

        // Turn and depth match.
        assert_eq!(turn, 1);
        assert_eq!(depth, 1);
        assert_eq!(loaded_rng, rng_state);

        // Player position preserved.
        let pos = loaded.get_component::<Positioned>(loaded.player()).unwrap();
        assert_eq!(pos.0, Position::new(40, 10));

        // Player HP preserved.
        let hp = loaded.get_component::<HitPoints>(loaded.player()).unwrap();
        assert_eq!(hp.current, 16);
        assert_eq!(hp.max, 16);
        assert!(
            !loaded.monster_catalog().is_empty(),
            "plain load_game should restore runtime monster catalogs"
        );
        assert!(
            !loaded.object_catalog().is_empty(),
            "plain load_game should restore runtime object catalogs"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trip_restores_next_creation_order_counter() {
        let dir = test_dir("creation_order_v3");
        let path = dir.join("creation_order.nbsv");

        let mut world = make_world();
        let _ = world.next_creation_order();
        let _ = world.next_creation_order();
        let expected_next = world.next_creation_order_value();

        save_game(&world, &path, SaveReason::Checkpoint, [11u8; 32]).unwrap();

        let (loaded, _, _, _) = load_game(&path).unwrap();
        assert_eq!(
            loaded.next_creation_order_value(),
            expected_next,
            "loading a save should restore the next creation order counter"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trip_restores_appearance_table() {
        let dir = test_dir("appearance_table_v3");
        let path = dir.join("appearance_table.nbsv");

        let mut rng = Pcg64::seed_from_u64(0xA11CE);
        let world = GameWorld::new_with_rng(Position::new(40, 10), &mut rng);
        let expected = world.appearance_table.clone();

        save_game(&world, &path, SaveReason::Checkpoint, [12u8; 32]).unwrap();

        let (loaded, _, _, _) = load_game(&path).unwrap();
        assert_eq!(
            loaded.appearance_table, expected,
            "loading a save should preserve the per-game appearance table"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trip_restores_religion_and_quest_state() {
        let dir = test_dir("story_state_v3");
        let path = dir.join("story_state.nbsv");

        let mut world = make_world();
        let player = world.player();

        let religion = ReligionState {
            alignment: Alignment::Chaotic,
            alignment_record: 17,
            god_anger: 2,
            god_gifts: 1,
            blessed_amount: 3,
            bless_cooldown: 42,
            crowned: true,
            demigod: false,
            turn: 777,
            experience_level: 14,
            current_hp: 22,
            max_hp: 30,
            current_pw: 18,
            max_pw: 24,
            nutrition: 650,
            luck: 4,
            luck_bonus: 0,
            has_luckstone: false,
            luckstone_blessed: false,
            luckstone_cursed: false,
            in_gehennom: false,
            is_undead: false,
            is_demon: false,
            original_alignment: Alignment::Chaotic,
            has_converted: false,
            alignment_abuse: 0,
        };
        let mut quest = QuestState::new();
        quest.meet_leader();
        quest.assign();
        quest.enter_quest_dungeon();
        quest.defeat_nemesis();
        quest.obtain_artifact();
        quest.complete();
        let quest_items = PlayerQuestItems {
            has_amulet: true,
            has_bell: true,
            has_book: false,
            has_menorah: false,
            has_quest_artifact: true,
        };
        let player_events = PlayerEvents {
            quest_called: true,
            quest_completed: true,
            gehennom_entered: true,
            ..PlayerEvents::default()
        };

        world
            .ecs_mut()
            .insert_one(player, religion.clone())
            .expect("player should accept religion state");
        world
            .ecs_mut()
            .insert_one(player, quest.clone())
            .expect("player should accept quest state");
        if let Some(mut live_items) = world.get_component_mut::<PlayerQuestItems>(player) {
            *live_items = quest_items.clone();
        }
        if let Some(mut live_events) = world.get_component_mut::<PlayerEvents>(player) {
            *live_events = player_events.clone();
        }

        save_game(&world, &path, SaveReason::Checkpoint, [17u8; 32]).unwrap();

        let (loaded, _, _, _) = load_game(&path).unwrap();
        assert_eq!(
            loaded
                .get_component::<ReligionState>(loaded.player())
                .map(|state| (*state).clone()),
            Some(religion),
            "loading a save should restore religion state"
        );
        assert_eq!(
            loaded
                .get_component::<QuestState>(loaded.player())
                .map(|state| (*state).clone()),
            Some(quest),
            "loading a save should restore quest state"
        );
        assert_eq!(
            loaded
                .get_component::<PlayerQuestItems>(loaded.player())
                .map(|state| (*state).clone()),
            Some(quest_items),
            "loading a save should restore player quest item flags"
        );
        assert_eq!(
            loaded
                .get_component::<PlayerEvents>(loaded.player())
                .map(|state| (*state).clone()),
            Some(player_events),
            "loading a save should restore player milestone flags"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Round-trip with player + monsters --------------------------------

    #[test]
    fn round_trip_with_monsters() {
        let dir = test_dir("monsters_v3");
        let path = dir.join("monsters.nbsv");

        let mut world = make_world();

        // Modify player state.
        {
            let player = world.player();
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                hp.current = 10;
                hp.max = 20;
            }
            if let Some(mut nut) = world.get_component_mut::<Nutrition>(player) {
                nut.0 = 500;
            }
        }

        // Advance turns.
        for _ in 0..9 {
            world.advance_turn();
        }
        assert_eq!(world.turn(), 10);

        // Spawn two monsters.
        world.spawn((
            Monster,
            Positioned(Position::new(5, 5)),
            HitPoints {
                current: 8,
                max: 12,
            },
            Speed(10),
            MovementPoints(12),
            Name("goblin".to_string()),
        ));
        world.spawn((
            Monster,
            Positioned(Position::new(20, 15)),
            HitPoints {
                current: 15,
                max: 15,
            },
            Speed(6),
            MovementPoints(6),
            Name("orc".to_string()),
        ));

        let rng_state = [7u8; 32];
        save_game(&world, &path, SaveReason::Quit, rng_state).unwrap();

        let (loaded, turn, _depth, loaded_rng) = load_game(&path).unwrap();
        assert_eq!(turn, 10);
        assert_eq!(loaded_rng, rng_state);

        // Player HP matches modified values.
        let hp = loaded.get_component::<HitPoints>(loaded.player()).unwrap();
        assert_eq!(hp.current, 10);
        assert_eq!(hp.max, 20);

        // Nutrition preserved.
        let nut = loaded.get_component::<Nutrition>(loaded.player()).unwrap();
        assert_eq!(nut.0, 500);

        // Two monsters exist.
        let monster_count = loaded.ecs().query::<(&Monster,)>().iter().count();
        assert_eq!(monster_count, 2);

        // Check monster names are present.
        let mut names: Vec<String> = loaded
            .ecs()
            .query::<(&Monster, &Name)>()
            .iter()
            .map(|(_, (_, n))| n.0.clone())
            .collect();
        names.sort();
        assert_eq!(names, vec!["goblin", "orc"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Round-trip with tame monster -------------------------------------

    #[test]
    fn round_trip_tame_monster() {
        let dir = test_dir("tame_v3");
        let path = dir.join("tame.nbsv");

        let mut world = make_world();
        let _pet = world.spawn((
            Monster,
            Positioned(Position::new(41, 10)),
            HitPoints { current: 6, max: 6 },
            Speed(12),
            MovementPoints(12),
            Name("kitten".to_string()),
            Tame,
        ));

        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        let (loaded, _, _, _) = load_game(&path).unwrap();

        // Find the tame monster.
        let tame_count = loaded.ecs().query::<(&Monster, &Tame)>().iter().count();
        assert_eq!(tame_count, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Invalid magic bytes rejected ------------------------------------

    #[test]
    fn invalid_magic_rejected() {
        let dir = test_dir("bad_magic_v3");
        let path = dir.join("bad_magic.nbsv");

        // Write garbage.
        std::fs::write(&path, b"FAKE0000deadbeef").unwrap();

        let err_msg = match load_game(&path) {
            Err(e) => format!("{e}"),
            Ok(_) => panic!("Expected error for invalid magic"),
        };
        assert!(
            err_msg.contains("magic"),
            "Error should mention magic bytes: {err_msg}"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Version mismatch detected ---------------------------------------

    #[test]
    fn version_mismatch_detected() {
        let dir = test_dir("bad_version_v3");
        let path = dir.join("bad_version.nbsv");

        // Write correct magic but wrong version (major mismatch).
        let mut data = Vec::new();
        data.extend_from_slice(b"NBSV");
        data.extend_from_slice(&[99, 99, 99]); // wrong version
        data.extend_from_slice(&0u64.to_le_bytes()); // zero-length payload
        std::fs::write(&path, &data).unwrap();

        let err_msg = match load_game(&path) {
            Err(e) => format!("{e}"),
            Ok(_) => panic!("Expected error for version mismatch"),
        };
        assert!(
            err_msg.contains("version mismatch")
                || err_msg.contains("Version")
                || err_msg.contains("99"),
            "Error should mention version mismatch: {err_msg}"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Truncated file handled gracefully --------------------------------

    #[test]
    fn truncated_file_fails() {
        let dir = test_dir("truncated_v3");
        let path = dir.join("truncated.nbsv");

        // Write just magic + version, no payload.
        let mut data = Vec::new();
        data.extend_from_slice(&SAVE_MAGIC);
        data.extend_from_slice(&SAVE_VERSION);
        std::fs::write(&path, &data).unwrap();

        let result = load_game(&path);
        assert!(result.is_err());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Player attributes round-trip ------------------------------------

    #[test]
    fn round_trip_player_attributes() {
        let dir = test_dir("attrs_v3");
        let path = dir.join("attrs.nbsv");

        let mut world = make_world();
        let player = world.player();
        if let Some(mut attrs) = world.get_component_mut::<EngineAttributes>(player) {
            attrs.strength = 18;
            attrs.strength_extra = 50;
            attrs.dexterity = 15;
            attrs.constitution = 14;
            attrs.intelligence = 12;
            attrs.wisdom = 11;
            attrs.charisma = 8;
        }

        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        let (loaded, _, _, _) = load_game(&path).unwrap();

        let attrs = loaded
            .get_component::<EngineAttributes>(loaded.player())
            .unwrap();
        assert_eq!(attrs.strength, 18);
        assert_eq!(attrs.strength_extra, 50);
        assert_eq!(attrs.dexterity, 15);
        assert_eq!(attrs.constitution, 14);
        assert_eq!(attrs.intelligence, 12);
        assert_eq!(attrs.wisdom, 11);
        assert_eq!(attrs.charisma, 8);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- save_file_path contains player name -----------------------------

    #[test]
    fn save_file_path_contains_player_name() {
        let path = save_file_path("wizard");
        assert!(path.to_str().unwrap().ends_with("wizard.nbsv"));
    }

    // ----- SaveHeader magic pre-filled -------------------------------------

    #[test]
    fn save_header_magic() {
        let h = SaveHeader::new(
            "test".into(),
            "Valkyrie".into(),
            "Human".into(),
            "Dlvl:1".into(),
            1,
            SaveReason::Checkpoint,
        );
        assert_eq!(&h.magic, b"NBSV");
        assert_eq!(h.version, SAVE_VERSION);
    }

    // ----- Dungeon state round-trip ----------------------------------------

    #[test]
    fn round_trip_dungeon_state() {
        use nethack_babel_engine::dungeon::{DungeonBranch, Terrain};

        let dir = test_dir("dungeon_v3");
        let path = dir.join("dungeon.nbsv");

        let mut world = make_world();
        world.dungeon_mut().depth = 5;
        world.dungeon_mut().branch = DungeonBranch::Mines;
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(10, 10), Terrain::Fountain);

        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        let (loaded, _, depth, _) = load_game(&path).unwrap();

        assert_eq!(depth, 5);
        assert_eq!(loaded.dungeon().branch, DungeonBranch::Mines);

        let cell = loaded
            .dungeon()
            .current_level
            .get(Position::new(10, 10))
            .unwrap();
        assert_eq!(cell.terrain, Terrain::Fountain);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ===================================================================
    // Required wiring tests
    // ===================================================================

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = test_dir("roundtrip_wiring_v3");
        let path = dir.join("roundtrip.nbsv");

        let mut world = make_world();

        for _ in 0..24 {
            world.advance_turn();
        }
        assert_eq!(world.turn(), 25);

        world.dungeon_mut().depth = 3;

        {
            let player = world.player();
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                hp.current = 7;
                hp.max = 30;
            }
        }

        let rng_state = [0xABu8; 32];
        save_game(&world, &path, SaveReason::Quit, rng_state).unwrap();

        let (loaded, turn, depth, loaded_rng) = load_game(&path).unwrap();

        assert_eq!(turn, 25, "turn number must survive round-trip");
        assert_eq!(depth, 3, "depth must survive round-trip");
        assert_eq!(loaded_rng, rng_state, "rng state must survive round-trip");

        let hp = loaded.get_component::<HitPoints>(loaded.player()).unwrap();
        assert_eq!(hp.current, 7, "player HP current must survive round-trip");
        assert_eq!(hp.max, 30, "player HP max must survive round-trip");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_file_deleted_on_load() {
        let dir = test_dir("deleted_on_load_v3");
        let path = dir.join("deleted.nbsv");

        let world = make_world();
        save_game(&world, &path, SaveReason::Quit, [0u8; 32]).unwrap();
        assert!(path.exists(), "save file must exist after save");

        let _loaded = load_game(&path).unwrap();
        assert!(
            !path.exists(),
            "save file must be deleted after successful load"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_nonexistent_returns_none() {
        let path = std::path::PathBuf::from("/tmp/nethack-babel-test-nonexistent-12345.nbsv");
        let _ = std::fs::remove_file(&path);

        let result = load_game(&path);
        assert!(
            result.is_err(),
            "loading a nonexistent save file must return Err"
        );
    }

    #[test]
    fn test_save_file_has_magic_header() {
        let dir = test_dir("magic_header_v3");
        let path = dir.join("magic.nbsv");

        let world = make_world();
        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();

        let raw = std::fs::read(&path).unwrap();
        assert!(raw.len() >= 7, "save file must be at least 7 bytes");
        assert_eq!(
            &raw[0..4],
            b"NBSV",
            "first 4 bytes must be the NBSV magic number"
        );
        assert_eq!(
            &raw[4..7],
            &SAVE_VERSION,
            "bytes 4..7 must be the save format version"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_version_mismatch_rejected() {
        let dir = test_dir("version_reject_v3");
        let path = dir.join("version_reject.nbsv");

        let world = make_world();
        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();

        let mut raw = std::fs::read(&path).unwrap();
        assert!(raw.len() >= 7);
        raw[4] = 255;
        raw[5] = 255;
        raw[6] = 255;
        std::fs::write(&path, &raw).unwrap();

        let result = load_game(&path);
        assert!(result.is_err(), "tampered version must cause load to fail");
        let err_msg = format!("{}", result.err().unwrap());
        assert!(
            err_msg.contains("version") || err_msg.contains("255"),
            "error message should mention version mismatch: {err_msg}"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ===================================================================
    // New v0.3.0 tests
    // ===================================================================

    // ----- Status effects round-trip -------------------------------------

    #[test]
    fn round_trip_status_effects() {
        let dir = test_dir("status_fx");
        let path = dir.join("status.nbsv");

        let mut world = make_world();
        let player = world.player();
        if let Some(mut se) = world.get_component_mut::<StatusEffects>(player) {
            se.confusion = 15;
            se.stun = 10;
            se.blindness = 200;
            se.hallucination = 50;
            se.stoning = 5;
        }

        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        let (loaded, _, _, _) = load_game(&path).unwrap();

        let se = loaded
            .get_component::<StatusEffects>(loaded.player())
            .unwrap();
        assert_eq!(se.confusion, 15);
        assert_eq!(se.stun, 10);
        assert_eq!(se.blindness, 200);
        assert_eq!(se.hallucination, 50);
        assert_eq!(se.stoning, 5);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Intrinsics round-trip -----------------------------------------

    #[test]
    fn round_trip_intrinsics() {
        let dir = test_dir("intrinsics");
        let path = dir.join("intr.nbsv");

        let mut world = make_world();
        let player = world.player();
        if let Some(mut intr) = world.get_component_mut::<Intrinsics>(player) {
            intr.fire_resistance = true;
            intr.cold_resistance = true;
            intr.telepathy = true;
            intr.see_invisible = true;
        }

        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        let (loaded, _, _, _) = load_game(&path).unwrap();

        let intr = loaded.get_component::<Intrinsics>(loaded.player()).unwrap();
        assert!(intr.fire_resistance);
        assert!(intr.cold_resistance);
        assert!(intr.telepathy);
        assert!(intr.see_invisible);
        assert!(!intr.poison_resistance);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Combat bonuses round-trip -------------------------------------

    #[test]
    fn round_trip_combat() {
        let dir = test_dir("combat");
        let path = dir.join("combat.nbsv");

        let mut world = make_world();
        let player = world.player();
        if let Some(mut pc) = world.get_component_mut::<PlayerCombat>(player) {
            pc.luck = 5;
            pc.uhitinc = 3;
            pc.udaminc = 2;
        }

        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        let (loaded, _, _, _) = load_game(&path).unwrap();

        let pc = loaded
            .get_component::<PlayerCombat>(loaded.player())
            .unwrap();
        assert_eq!(pc.luck, 5);
        assert_eq!(pc.uhitinc, 3);
        assert_eq!(pc.udaminc, 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- SpellBook round-trip ------------------------------------------

    #[test]
    fn round_trip_spellbook() {
        use nethack_babel_engine::spells::{KnownSpell, SpellType};

        let dir = test_dir("spellbook");
        let path = dir.join("spellbook.nbsv");

        let mut world = make_world();
        let player = world.player();
        if let Some(mut sb) = world.get_component_mut::<SpellBook>(player) {
            sb.spells.push(KnownSpell {
                spell_type: SpellType::MagicMissile,
                memory: 20000,
                level: 2,
            });
            sb.spells.push(KnownSpell {
                spell_type: SpellType::HealingSpell,
                memory: 15000,
                level: 1,
            });
        }

        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        let (loaded, _, _, _) = load_game(&path).unwrap();

        let sb = loaded.get_component::<SpellBook>(loaded.player()).unwrap();
        assert_eq!(sb.spells.len(), 2);
        assert_eq!(sb.spells[0].spell_type, SpellType::MagicMissile);
        assert_eq!(sb.spells[0].memory, 20000);
        assert_eq!(sb.spells[1].spell_type, SpellType::HealingSpell);
        assert_eq!(sb.spells[1].memory, 15000);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Item round-trip -----------------------------------------------

    #[test]
    fn round_trip_items() {
        use nethack_babel_data::schema::{ObjectClass, ObjectTypeId};

        let dir = test_dir("items");
        let path = dir.join("items.nbsv");

        let mut world = make_world();

        // Spawn an item directly using the ECS.
        let core = ObjectCore {
            otyp: ObjectTypeId(5),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 30,
            age: 0,
            inv_letter: Some('a'),
            artifact: None,
        };
        let buc = BucStatus {
            cursed: false,
            blessed: true,
            bknown: true,
        };
        let knowledge = KnowledgeState {
            known: true,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };
        let location = ObjectLocation::Inventory;
        let enchantment = Enchantment { spe: 3 };

        let item = world.ecs_mut().spawn((
            core.clone(),
            buc.clone(),
            knowledge.clone(),
            location.clone(),
            enchantment,
        ));

        // Add the item to the player's inventory.
        let player = world.player();
        if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
            inv.items.push(item);
        }

        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        let (loaded, _, _, _) = load_game(&path).unwrap();

        // Verify item was restored.
        let item_count = loaded.ecs().query::<(&ObjectCore,)>().iter().count();
        assert_eq!(item_count, 1);

        // Verify item components.
        for (_, (c,)) in loaded.ecs().query::<(&ObjectCore,)>().iter() {
            assert_eq!(c.otyp, ObjectTypeId(5));
            assert_eq!(c.object_class, ObjectClass::Weapon);
            assert!(c.inv_letter == Some('a'));
        }

        // Verify BUC status.
        for (_, (b,)) in loaded.ecs().query::<(&BucStatus,)>().iter() {
            assert!(b.blessed);
            assert!(!b.cursed);
            assert!(b.bknown);
        }

        // Verify enchantment.
        for (_, (e,)) in loaded.ecs().query::<(&Enchantment,)>().iter() {
            assert_eq!(e.spe, 3);
        }

        // Verify inventory has the item.
        let loaded_player = loaded.player();
        let inv = loaded.get_component::<Inventory>(loaded_player).unwrap();
        assert_eq!(inv.items.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Checkpoint config --------------------------------------------

    #[test]
    fn checkpoint_config_default() {
        let config = CheckpointConfig::default();
        assert!(config.enabled);
        assert_eq!(config.interval, DEFAULT_CHECKPOINT_INTERVAL);
        assert_eq!(config.last_checkpoint_turn, 0);
    }

    #[test]
    fn checkpoint_config_disabled() {
        let config = CheckpointConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn checkpoint_skips_when_disabled() {
        let world = make_world();
        let mut config = CheckpointConfig::disabled();
        let result = maybe_checkpoint(&world, &mut config, "test", [0u8; 32]);
        assert!(!result);
    }

    #[test]
    fn checkpoint_skips_when_too_early() {
        let world = make_world();
        let mut config = CheckpointConfig::with_interval(100);
        // Turn 1, interval 100 — too early.
        let result = maybe_checkpoint(&world, &mut config, "test", [0u8; 32]);
        assert!(!result);
    }

    #[test]
    fn checkpoint_triggers_at_interval() {
        let dir = test_dir("checkpoint_trigger");
        let ckpt_path = dir.join("ckpt_test.ckpt.nbsv");

        let mut world = make_world();
        // Advance to turn 101.
        for _ in 0..100 {
            world.advance_turn();
        }
        assert_eq!(world.turn(), 101);

        let mut config = CheckpointConfig::with_interval(100);

        // Manually test the checkpoint logic: config says it's time.
        assert!(config.enabled);
        assert!(world.turn() >= config.last_checkpoint_turn + config.interval);

        // Save directly to verify the path works.
        save_game(&world, &ckpt_path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        assert!(ckpt_path.exists());
        config.last_checkpoint_turn = world.turn();

        // Next checkpoint should not trigger immediately.
        assert!(world.turn() < config.last_checkpoint_turn + config.interval);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Version compatibility ----------------------------------------

    #[test]
    fn version_compatibility_same() {
        assert!(is_compatible_version(SAVE_VERSION));
    }

    #[test]
    fn version_compatibility_older_major_rejected() {
        let older = [SAVE_VERSION[0] - 1, 9, 9];
        assert!(!is_compatible_version(older));
    }

    #[test]
    fn version_compatibility_newer_major_rejected() {
        let diff = [SAVE_VERSION[0] + 1, 0, 0];
        assert!(!is_compatible_version(diff));
    }

    #[test]
    fn version_compatibility_newer_minor_rejected() {
        let newer = [SAVE_VERSION[0], SAVE_VERSION[1] + 1, 0];
        assert!(!is_compatible_version(newer));
    }

    // ----- save_file_exists ----------------------------------------------

    #[test]
    fn save_file_exists_true() {
        let dir = test_dir("exists_check");
        let path = dir.join("exists.nbsv");

        let world = make_world();
        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();

        assert!(save_file_exists(&path));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_file_exists_false() {
        let path = PathBuf::from("/tmp/nethack-babel-nonexist-xyz.nbsv");
        assert!(!save_file_exists(&path));
    }

    #[test]
    fn save_file_exists_bad_magic() {
        let dir = test_dir("bad_magic_exists");
        let path = dir.join("bad.nbsv");
        std::fs::write(&path, b"NOPE1234").unwrap();

        assert!(!save_file_exists(&path));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Payload size limit -------------------------------------------

    #[test]
    fn oversized_payload_rejected() {
        let dir = test_dir("oversized");
        let path = dir.join("oversized.nbsv");

        let mut data = Vec::new();
        data.extend_from_slice(&SAVE_MAGIC);
        data.extend_from_slice(&SAVE_VERSION);
        // Claim a 512 MB payload.
        data.extend_from_slice(&(512u64 * 1024 * 1024).to_le_bytes());
        std::fs::write(&path, &data).unwrap();

        let result = load_game(&path);
        assert!(result.is_err());
        let err = format!("{}", result.err().unwrap());
        assert!(err.contains("too large"), "should mention size: {err}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Crash recovery -----------------------------------------------

    #[test]
    fn try_recover_no_panic_file() {
        let result = try_recover("nonexistent_player_xyz").unwrap();
        assert!(result.is_none());
    }

    // ----- Natural attributes round-trip ---------------------------------

    #[test]
    fn round_trip_natural_attributes() {
        let dir = test_dir("nat_attrs");
        let path = dir.join("nat_attrs.nbsv");

        let mut world = make_world();
        let player = world.player();
        if let Some(mut na) = world.get_component_mut::<NaturalAttributes>(player) {
            na.strength = 18;
            na.dexterity = 16;
            na.constitution = 15;
        }

        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        let (loaded, _, _, _) = load_game(&path).unwrap();

        let na = loaded
            .get_component::<NaturalAttributes>(loaded.player())
            .unwrap();
        assert_eq!(na.strength, 18);
        assert_eq!(na.dexterity, 16);
        assert_eq!(na.constitution, 15);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- Attribute exercise round-trip ---------------------------------

    #[test]
    fn round_trip_attribute_exercise() {
        let dir = test_dir("attr_exercise");
        let path = dir.join("exercise.nbsv");

        let mut world = make_world();
        let player = world.player();
        if let Some(mut ae) = world.get_component_mut::<AttributeExercise>(player) {
            ae.str_exercise = 50;
            ae.dex_exercise = -10;
            ae.con_exercise = 25;
        }

        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        let (loaded, _, _, _) = load_game(&path).unwrap();

        let ae = loaded
            .get_component::<AttributeExercise>(loaded.player())
            .unwrap();
        assert_eq!(ae.str_exercise, 50);
        assert_eq!(ae.dex_exercise, -10);
        assert_eq!(ae.con_exercise, 25);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trip_loaded_medusa_revisit_does_not_duplicate_boss() {
        let mut world = make_stair_world(DungeonBranch::Main, 23, Terrain::StairsDown);
        let mut rng = Pcg64::seed_from_u64(7001);

        resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(world.dungeon().depth, 24);
        assert_eq!(count_monsters_named(&world, "medusa"), 1);

        let (mut loaded, loaded_rng) = save_and_reload_world("medusa-revisit", &world, [11u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let medusa_down = find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
            .expect("Medusa level should have stairs down");
        set_player_position(&mut loaded, medusa_down);
        resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);

        let castle_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
            .expect("Castle should have stairs up");
        set_player_position(&mut loaded, castle_up);
        resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);

        assert_eq!(loaded.dungeon().depth, 24);
        assert_eq!(
            count_monsters_named(&loaded, "medusa"),
            1,
            "Medusa should not duplicate after save/load and revisit"
        );
    }

    #[test]
    fn round_trip_loaded_orcus_revisit_does_not_duplicate_boss() {
        let mut world = make_stair_world(DungeonBranch::Gehennom, 11, Terrain::StairsDown);
        let mut rng = Pcg64::seed_from_u64(7002);

        resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(world.dungeon().depth, 12);
        assert_eq!(count_monsters_named(&world, "orcus"), 1);

        let (mut loaded, loaded_rng) = save_and_reload_world("orcus-revisit", &world, [12u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let orcus_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
            .expect("Orcus level should have stairs up");
        set_player_position(&mut loaded, orcus_up);
        resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);

        let gehennom_down = find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
            .expect("cached Gehennom entry level should preserve stairs down");
        set_player_position(&mut loaded, gehennom_down);
        resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);

        assert_eq!(loaded.dungeon().depth, 12);
        assert_eq!(
            count_monsters_named(&loaded, "orcus"),
            1,
            "Orcus should not duplicate after save/load and revisit"
        );
    }

    #[test]
    fn round_trip_loaded_wizard_quest_start_revisit_preserves_population() {
        let mut world = make_stair_world(DungeonBranch::Quest, 2, Terrain::StairsUp);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept identity");
        let mut rng = Pcg64::seed_from_u64(7003);

        resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);
        assert_eq!(world.dungeon().depth, 1);
        let apprentices = count_monsters_named(&world, "apprentice");
        assert_eq!(count_monsters_named(&world, "Neferet the Green"), 1);
        assert!(apprentices >= 1);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("quest-start-revisit", &world, [13u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let start_down = find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
            .expect("Quest start should have stairs down");
        set_player_position(&mut loaded, start_down);
        resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);

        let cached_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
            .expect("cached quest depth 2 should preserve stairs up");
        set_player_position(&mut loaded, cached_up);
        resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);

        assert_eq!(loaded.dungeon().depth, 1);
        assert_eq!(count_monsters_named(&loaded, "Neferet the Green"), 1);
        assert_eq!(
            count_monsters_named(&loaded, "apprentice"),
            apprentices,
            "quest start guardians should not duplicate after save/load"
        );
    }

    #[test]
    fn round_trip_loaded_wizard_quest_locator_revisit_preserves_population() {
        let mut world = make_stair_world(DungeonBranch::Quest, 3, Terrain::StairsDown);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept identity");
        let mut rng = Pcg64::seed_from_u64(7004);

        resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(world.dungeon().depth, 4);
        let xorns = count_monsters_named(&world, "xorn");
        let vampire_bats = count_monsters_named(&world, "vampire bat");
        assert!(xorns >= 1);
        assert!(vampire_bats >= 1);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("quest-locator-revisit", &world, [14u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let locator_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
            .expect("Quest locator should have stairs up");
        set_player_position(&mut loaded, locator_up);
        resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);

        let cached_down = find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
            .expect("cached quest depth 3 should preserve stairs down");
        set_player_position(&mut loaded, cached_down);
        resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);

        assert_eq!(loaded.dungeon().depth, 4);
        assert_eq!(count_monsters_named(&loaded, "xorn"), xorns);
        assert_eq!(
            count_monsters_named(&loaded, "vampire bat"),
            vampire_bats,
            "quest locator enemies should not duplicate after save/load"
        );
    }

    #[test]
    fn round_trip_loaded_wizard_quest_filler_revisit_preserves_population() {
        let mut world = make_stair_world(DungeonBranch::Quest, 2, Terrain::StairsDown);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept identity");
        let mut rng = Pcg64::seed_from_u64(7005);

        resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(world.dungeon().depth, 3);
        let xorns = count_monsters_named(&world, "xorn");
        let vampire_bats = count_monsters_named(&world, "vampire bat");
        assert!(xorns >= 1);
        assert!(vampire_bats >= 1);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("quest-filler-revisit", &world, [15u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let filler_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
            .expect("Quest filler should have stairs up");
        set_player_position(&mut loaded, filler_up);
        resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);

        let cached_down = find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
            .expect("cached quest depth 2 should preserve stairs down");
        set_player_position(&mut loaded, cached_down);
        resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);

        assert_eq!(loaded.dungeon().depth, 3);
        assert_eq!(count_monsters_named(&loaded, "xorn"), xorns);
        assert_eq!(
            count_monsters_named(&loaded, "vampire bat"),
            vampire_bats,
            "quest filler enemies should not duplicate after save/load"
        );
    }

    #[test]
    fn round_trip_loaded_wizard_quest_goal_revisit_preserves_population_and_artifact() {
        let mut world = make_stair_world(DungeonBranch::Quest, 6, Terrain::StairsDown);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept identity");
        let mut rng = Pcg64::seed_from_u64(7006);

        resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(world.dungeon().depth, 7);
        let xorns = count_monsters_named(&world, "xorn");
        let vampire_bats = count_monsters_named(&world, "vampire bat");
        assert_eq!(count_monsters_named(&world, "Dark One"), 1);
        assert!(xorns >= 1);
        assert!(vampire_bats >= 1);
        assert_eq!(
            count_objects_with_artifact_name(&world, "The Eye of the Aethiopica"),
            1
        );

        let (mut loaded, loaded_rng) =
            save_and_reload_world("quest-goal-revisit", &world, [16u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let goal_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
            .expect("Quest goal should have stairs up");
        set_player_position(&mut loaded, goal_up);
        resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);

        let cached_down = find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
            .expect("cached quest depth 6 should preserve stairs down");
        set_player_position(&mut loaded, cached_down);
        resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);

        assert_eq!(loaded.dungeon().depth, 7);
        assert_eq!(count_monsters_named(&loaded, "Dark One"), 1);
        assert_eq!(count_monsters_named(&loaded, "xorn"), xorns);
        assert_eq!(count_monsters_named(&loaded, "vampire bat"), vampire_bats);
        assert_eq!(
            count_objects_with_artifact_name(&loaded, "The Eye of the Aethiopica"),
            1,
            "quest goal artifact should not duplicate after save/load"
        );
    }

    #[test]
    fn round_trip_loaded_invoked_gehennom_portal_reopens_on_revisit() {
        let mut world = make_stair_world(DungeonBranch::Gehennom, 20, Terrain::StairsDown);
        let mut rng = Pcg64::seed_from_u64(7007);

        resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(world.dungeon().depth, 21);
        assert!(
            find_terrain(&world.dungeon().current_level, Terrain::MagicPortal).is_none(),
            "before invocation the Gehennom 21 portal should stay closed"
        );

        let player = world.player();
        if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
            player_events.invoked = true;
            player_events.found_vibrating_square = true;
        }

        let (mut loaded, loaded_rng) =
            save_and_reload_world("invoked-gehennom-portal", &world, [18u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let current_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
            .expect("Gehennom 21 should have stairs up");
        set_player_position(&mut loaded, current_up);
        resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);

        let cached_down = find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
            .expect("cached Gehennom 20 should preserve stairs down");
        set_player_position(&mut loaded, cached_down);
        resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);

        assert_eq!(loaded.dungeon().depth, 21);
        assert!(
            find_terrain(&loaded.dungeon().current_level, Terrain::MagicPortal).is_some(),
            "after save/load, revisiting Gehennom 21 with invoked=true should reopen the endgame portal"
        );
        assert!(
            !loaded
                .dungeon()
                .trap_map
                .traps
                .iter()
                .any(|trap| trap.trap_type == nethack_babel_data::TrapType::VibratingSquare),
            "the vibrating square marker should be gone once invocation has been recorded"
        );
    }

    #[test]
    fn round_trip_loaded_wizard_respawn_state_survives_and_reanimates() {
        let mut world = make_stair_world(DungeonBranch::Gehennom, 10, Terrain::Floor);
        let player = world.player();
        if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
            player_events.killed_wizard = true;
            player_events.wizard_times_killed = 1;
            player_events.wizard_last_killed_turn = 0;
        }
        while world.turn() < 99 {
            world.advance_turn();
        }

        let (mut loaded, loaded_rng) =
            save_and_reload_world("wizard-respawn-state", &world, [21u8; 32]);
        let restored_events = {
            let events = loaded
                .get_component::<PlayerEvents>(loaded.player())
                .expect("player events should survive save/load");
            (*events).clone()
        };
        assert!(restored_events.killed_wizard);
        assert_eq!(restored_events.wizard_times_killed, 1);
        assert_eq!(restored_events.wizard_last_killed_turn, 0);

        let mut rng = Pcg64::from_seed(loaded_rng);
        let events = resolve_turn(&mut loaded, PlayerAction::Rest, &mut rng);

        assert_eq!(count_monsters_named(&loaded, "Wizard of Yendor"), 1);
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "wizard-respawned"
        )));
    }

    #[test]
    fn round_trip_loaded_live_wizard_still_harasses_player() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let wizard = spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 20);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
            hp.current = 12;
            hp.max = 20;
        }
        let cursed_target = spawn_inventory_object_by_name(&mut world, "long sword", 'a');
        world
            .ecs_mut()
            .insert_one(
                cursed_target,
                BucStatus {
                    cursed: false,
                    blessed: false,
                    bknown: false,
                },
            )
            .expect("inventory item should accept BUC state");
        if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(world.player()) {
            player_events.invoked = true;
        }

        let (mut loaded, loaded_rng) =
            save_and_reload_world("wizard-live-harassment", &world, [22u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);
        let mut saw_harassment = false;
        let mut saw_curse = false;
        let mut saw_summon = false;

        for _ in 0..256 {
            let events = resolve_turn(&mut loaded, PlayerAction::Rest, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. }
                        if key == "wizard-curse-items" || key == "wizard-summon-nasties"
                )
            }) {
                saw_harassment = true;
                saw_curse = events.iter().any(|event| {
                    matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "wizard-curse-items"
                    )
                });
                saw_summon = events.iter().any(|event| {
                    matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "wizard-summon-nasties"
                    )
                });
                break;
            }
        }

        assert!(
            saw_harassment,
            "a live wizard should eventually keep harassing the player after save/load"
        );
        if saw_curse {
            assert!(
                loaded
                    .get_component::<BucStatus>(cursed_target)
                    .is_some_and(|status| status.cursed),
                "curse harassment after reload should mutate live inventory state"
            );
        }
        if saw_summon {
            assert!(
                count_monsters_named(&loaded, "Wizard of Yendor") >= 1,
                "summon harassment after reload should keep the wizard present"
            );
        }
    }

    #[test]
    fn round_trip_loaded_preserves_peaceful_monsters_on_current_level() {
        let mut world = make_stair_world(DungeonBranch::Main, 3, Terrain::Floor);
        let angel = world.spawn((
            Monster,
            Positioned(Position::new(6, 5)),
            Name("Angel".to_string()),
            HitPoints {
                current: 18,
                max: 18,
            },
            Speed(12),
            MovementPoints(12),
            nethack_babel_engine::world::Peaceful,
        ));
        let _ = angel;

        let (loaded, _loaded_rng) =
            save_and_reload_world("peaceful-current-level", &world, [22u8; 32]);
        let restored_angel = loaded
            .ecs()
            .query::<(&Monster, &Name)>()
            .iter()
            .find_map(|(entity, (_monster, name))| {
                name.0.eq_ignore_ascii_case("Angel").then_some(entity)
            })
            .expect("peaceful angel should survive round-trip");

        assert!(
            loaded
                .get_component::<nethack_babel_engine::world::Peaceful>(restored_angel)
                .is_some(),
            "live peaceful monsters should keep their peaceful marker across save/load"
        );
    }

    #[test]
    fn round_trip_loaded_revisit_restores_cached_runtime_state() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::StairsDown);
        let shopkeeper = world.spawn((
            Monster,
            Positioned(Position::new(6, 5)),
            Name("Izchak".to_string()),
            HitPoints {
                current: 20,
                max: 20,
            },
            Speed(12),
            nethack_babel_engine::world::DisplaySymbol {
                symbol: '@',
                color: nethack_babel_data::Color::White,
            },
            MovementPoints(12),
        ));
        world
            .dungeon_mut()
            .trap_map
            .traps
            .push(nethack_babel_engine::traps::TrapInstance {
                pos: Position::new(4, 4),
                trap_type: nethack_babel_data::TrapType::Pit,
                detected: true,
                triggered_count: 1,
            });
        world
            .dungeon_mut()
            .engraving_map
            .insert(nethack_babel_engine::engrave::Engraving::new(
                "Elbereth".to_string(),
                nethack_babel_engine::engrave::EngraveMethod::Blade,
                Position::new(4, 5),
            ));
        world.dungeon_mut().current_level_flags.no_prayer = true;
        world.dungeon_mut().current_level_flags.no_teleport = true;
        world
            .dungeon_mut()
            .vault_rooms
            .push(nethack_babel_engine::vault::VaultRoom {
                top_left: Position::new(6, 6),
                bottom_right: Position::new(7, 7),
            });
        world
            .dungeon_mut()
            .shop_rooms
            .push(nethack_babel_engine::shop::ShopRoom::new(
                Position::new(3, 3),
                Position::new(7, 7),
                nethack_babel_engine::shop::ShopType::General,
                shopkeeper,
                "Izchak".to_string(),
            ));
        world.dungeon_mut().vault_guard_present = true;
        world
            .dungeon_mut()
            .gas_clouds
            .push(nethack_babel_engine::region::GasCloud {
                position: Position::new(5, 6),
                radius: 1,
                turns_remaining: 3,
                damage_type: nethack_babel_engine::region::GasCloudType::Poison,
                damage_per_turn: 6,
            });

        let mut rng = Pcg64::seed_from_u64(7008);
        resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(world.dungeon().depth, 2);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("runtime-cache-revisit", &world, [19u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let level2_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
            .expect("generated level 2 should have stairs up");
        set_player_position(&mut loaded, level2_up);
        resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);

        assert_eq!(loaded.dungeon().depth, 1);
        assert!(
            loaded
                .dungeon()
                .trap_map
                .trap_at(Position::new(4, 4))
                .is_some()
        );
        assert!(
            loaded
                .dungeon()
                .engraving_map
                .is_elbereth_at(Position::new(4, 5))
        );
        assert!(loaded.dungeon().current_level_flags.no_prayer);
        assert!(loaded.dungeon().current_level_flags.no_teleport);
        assert_eq!(loaded.dungeon().vault_rooms.len(), 1);
        assert_eq!(loaded.dungeon().shop_rooms.len(), 1);
        assert!(loaded.dungeon().vault_guard_present);
        assert_eq!(loaded.dungeon().gas_clouds.len(), 1);
        assert_eq!(loaded.dungeon().gas_clouds[0].turns_remaining, 2);

        let shopkeeper_name = loaded
            .get_component::<Name>(loaded.dungeon().shop_rooms[0].shopkeeper)
            .expect("restored shopkeeper should be rebound to a live entity");
        assert_eq!(shopkeeper_name.0, "Izchak");
    }

    #[test]
    fn round_trip_loaded_angry_shopkeeper_keeps_angry_greeting() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 40);
        world
            .dungeon_mut()
            .shop_rooms
            .push(nethack_babel_engine::shop::ShopRoom::new(
                Position::new(6, 4),
                Position::new(7, 6),
                nethack_babel_engine::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));

        let mut rng = Pcg64::seed_from_u64(7010);
        let _ = resolve_turn(
            &mut world,
            PlayerAction::FightDirection {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(world.dungeon().shop_rooms[0].angry);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("angry-shopkeeper-round-trip", &world, [24u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);
        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(loaded.dungeon().shop_rooms[0].angry);
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shk-angry-greeting"
        )));
    }

    #[test]
    fn round_trip_loaded_restores_explicit_npc_components_on_current_level() {
        let mut world = make_stair_world(DungeonBranch::Quest, 1, Terrain::Floor);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        if let Some(mut level) = world.get_component_mut::<ExperienceLevel>(player) {
            level.0 = 14;
        }
        let religion = wizard_story_religion(&world, player);
        world
            .ecs_mut()
            .insert_one(player, religion)
            .expect("player should accept religion state");

        let leader = spawn_full_monster(&mut world, Position::new(6, 5), "mysterious sage", 20);
        world
            .ecs_mut()
            .insert_one(leader, nethack_babel_engine::quest::QuestNpcRole::Leader)
            .expect("leader should accept explicit quest role");

        let priest = spawn_full_monster(&mut world, Position::new(4, 5), "oracle", 18);
        world
            .ecs_mut()
            .insert_one(priest, nethack_babel_engine::world::Peaceful)
            .expect("priest should accept peaceful marker");
        world
            .ecs_mut()
            .insert_one(
                priest,
                nethack_babel_engine::npc::Priest {
                    alignment: Alignment::Chaotic,
                    has_shrine: false,
                    is_high_priest: false,
                    angry: false,
                },
            )
            .expect("priest should accept explicit priest component");

        let (mut loaded, loaded_rng) =
            save_and_reload_world("explicit-npc-components-round-trip", &world, [26u8; 32]);

        let loaded_leader =
            find_monster_named(&loaded, "mysterious sage").expect("leader should survive load");
        assert_eq!(
            loaded
                .get_component::<nethack_babel_engine::quest::QuestNpcRole>(loaded_leader)
                .map(|role| *role),
            Some(nethack_babel_engine::quest::QuestNpcRole::Leader)
        );

        let loaded_priest =
            find_monster_named(&loaded, "oracle").expect("priest should survive load");
        let loaded_priest_data = loaded
            .get_component::<nethack_babel_engine::npc::Priest>(loaded_priest)
            .map(|priest| *priest)
            .expect("explicit priest component should survive load");
        assert_eq!(loaded_priest_data.alignment, Alignment::Chaotic);
        assert!(!loaded_priest_data.has_shrine);

        let mut rng = Pcg64::from_seed(loaded_rng);
        let quest_events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(quest_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-assigned"
        )));
    }

    #[test]
    fn round_trip_loaded_floor_items_stay_on_original_level() {
        use nethack_babel_data::schema::{ObjectClass, ObjectTypeId};

        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::StairsDown);
        let coin_pos = Position::new(4, 4);
        let level_one = world.dungeon().current_data_dungeon_level();
        world.ecs_mut().spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Coin,
                quantity: 42,
                weight: 1,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            BucStatus {
                cursed: false,
                blessed: false,
                bknown: false,
            },
            KnowledgeState {
                known: false,
                dknown: false,
                rknown: false,
                cknown: false,
                lknown: false,
                tknown: false,
            },
            ObjectLocation::Floor {
                x: coin_pos.x as i16,
                y: coin_pos.y as i16,
                level: level_one,
            },
        ));

        assert_eq!(
            nethack_babel_engine::inventory::items_at_position(&world, coin_pos).len(),
            1
        );

        let mut rng = Pcg64::seed_from_u64(7009);
        resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(world.dungeon().depth, 2);
        assert!(
            nethack_babel_engine::inventory::items_at_position(&world, coin_pos).is_empty(),
            "depth-1 floor item should not be visible on depth 2 before save"
        );

        let (mut loaded, loaded_rng) =
            save_and_reload_world("floor-item-level-scope", &world, [20u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        assert_eq!(loaded.dungeon().depth, 2);
        assert!(
            nethack_babel_engine::inventory::items_at_position(&loaded, coin_pos).is_empty(),
            "depth-1 floor item should still be hidden after save/load on depth 2"
        );

        let mut saw_saved_coin = false;
        for (_entity, (core, loc)) in loaded
            .ecs()
            .query::<(&ObjectCore, &ObjectLocation)>()
            .iter()
        {
            if core.object_class == ObjectClass::Coin
                && core.quantity == 42
                && matches!(
                    *loc,
                    ObjectLocation::Floor { x: 4, y: 4, level }
                        if level
                            == nethack_babel_engine::dungeon::data_dungeon_level(
                                DungeonBranch::Main,
                                1
                            )
                )
            {
                saw_saved_coin = true;
                break;
            }
        }
        assert!(
            saw_saved_coin,
            "serialized floor item should retain its original branch/depth metadata"
        );

        let level2_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
            .expect("generated level 2 should have stairs up");
        set_player_position(&mut loaded, level2_up);
        resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);

        assert_eq!(loaded.dungeon().depth, 1);
        assert_eq!(
            nethack_babel_engine::inventory::items_at_position(&loaded, coin_pos).len(),
            1,
            "revisiting depth 1 after save/load should reveal the original floor item again"
        );
    }

    #[test]
    fn round_trip_story_traversal_matrix() {
        for scenario in [
            SaveStoryTraversalScenario::QuestClosure,
            SaveStoryTraversalScenario::QuestLeaderAnger,
            SaveStoryTraversalScenario::ShopkeeperFollow,
            SaveStoryTraversalScenario::ShopkeeperPayoff,
            SaveStoryTraversalScenario::TempleWrath,
            SaveStoryTraversalScenario::TempleCalm,
            SaveStoryTraversalScenario::EndgameAscension,
        ] {
            let (world, final_events) = run_round_trip_story_traversal_scenario(scenario);
            let player = world.player();

            match scenario {
                SaveStoryTraversalScenario::QuestClosure => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "quest-completed"
                    )));
                    assert_eq!(world.dungeon().branch, DungeonBranch::Quest);
                    assert_eq!(world.dungeon().depth, 1);
                    assert!(
                        world
                            .get_component::<QuestState>(player)
                            .is_some_and(|state| state.status
                                == nethack_babel_engine::quest::QuestStatus::Completed)
                    );
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.quest_completed)
                    );
                }
                SaveStoryTraversalScenario::QuestLeaderAnger => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "quest-expelled"
                    )));
                    assert_eq!(world.dungeon().branch, DungeonBranch::Quest);
                    assert_eq!(world.dungeon().depth, 1);
                    assert!(
                        world
                            .get_component::<QuestState>(player)
                            .is_some_and(|state| state.leader_angry)
                    );
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.quest_expelled)
                    );
                }
                SaveStoryTraversalScenario::ShopkeeperFollow => {
                    let shopkeeper =
                        find_monster_named(&world, "Izchak").expect("shopkeeper should exist");
                    let shopkeeper_state = world
                        .get_component::<Shopkeeper>(shopkeeper)
                        .map(|state| (*state).clone())
                        .expect("shopkeeper should keep explicit runtime state");
                    assert!(shopkeeper_state.following);
                    let pos = world
                        .get_component::<Positioned>(shopkeeper)
                        .map(|pos| pos.0)
                        .expect("shopkeeper should still have a position");
                    assert!(
                        pos.x <= 5,
                        "shopkeeper should keep advancing toward the hero after save/load, got {:?}",
                        pos
                    );
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::world::Peaceful>(shopkeeper)
                            .is_some(),
                        "non-hostile follow behavior should not require losing Peaceful status"
                    );
                }
                SaveStoryTraversalScenario::ShopkeeperPayoff => {
                    let shopkeeper = world.dungeon().shop_rooms[0].shopkeeper;
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-pay-success"
                    )));
                    assert!(shop.bill.is_empty());
                    assert_eq!(shop.debit, 0);
                    assert!(!shop.angry);
                    let gold_total: i32 = world
                        .get_component::<Inventory>(player)
                        .map(|inv| {
                            inv.items
                                .iter()
                                .filter_map(|item| world.get_component::<ObjectCore>(*item))
                                .filter(|core| core.object_class == ObjectClass::Coin)
                                .map(|core| core.quantity)
                                .sum()
                        })
                        .unwrap_or(0);
                    assert_eq!(gold_total, 50);
                    let shopkeeper_state = world
                        .get_component::<Shopkeeper>(shopkeeper)
                        .map(|state| (*state).clone())
                        .expect("shopkeeper should keep explicit runtime state");
                    assert!(!shopkeeper_state.following);
                }
                SaveStoryTraversalScenario::TempleWrath => {
                    let priest = find_monster_named(&world, "priest").expect("priest should exist");
                    let priest_state = world
                        .get_component::<Priest>(priest)
                        .map(|state| *state)
                        .expect("priest should keep explicit runtime state");
                    assert!(priest_state.angry);
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::world::Peaceful>(priest)
                            .is_none(),
                        "angered priest should stay hostile after save/load"
                    );
                    assert!(
                        !final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "priest-protection-granted"
                        )),
                        "hostile priest should not grant protection after save/load"
                    );
                }
                SaveStoryTraversalScenario::TempleCalm => {
                    let priest = find_monster_named(&world, "priest").expect("priest should exist");
                    let priest_state = world
                        .get_component::<Priest>(priest)
                        .map(|state| *state)
                        .expect("priest should keep explicit runtime state");
                    assert!(!priest_state.angry);
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::world::Peaceful>(priest)
                            .is_some(),
                        "calmed priest should stay peaceful after save/load"
                    );
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-calmed"
                    )));
                }
                SaveStoryTraversalScenario::EndgameAscension => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::GameOver {
                            cause: DeathCause::Ascended,
                            ..
                        }
                    )));
                    assert_eq!(world.dungeon().branch, DungeonBranch::Endgame);
                    assert_eq!(world.dungeon().depth, 5);
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.ascended)
                    );
                }
            }
        }
    }
}
