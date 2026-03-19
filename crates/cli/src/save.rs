use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nethack_babel_data::components::{
    BucStatus, Enchantment, Erosion, KnowledgeState, ObjectCore, ObjectLocation, ShopState,
};
use nethack_babel_data::{
    GameData, MonsterId, PlayerEvents, PlayerIdentity, PlayerQuestItems, PlayerSkills,
    loader::load_game_data,
};
use nethack_babel_engine::action::Position;
use nethack_babel_engine::attributes::{AttributeExercise, NaturalAttributes};
use nethack_babel_engine::conduct::ConductState;
use nethack_babel_engine::dungeon::DungeonState;
use nethack_babel_engine::equipment::EquipmentSlots;
use nethack_babel_engine::inventory::Inventory;
use nethack_babel_engine::o_init::AppearanceTable;
use nethack_babel_engine::pets::PetState;
use nethack_babel_engine::polyself::{
    OriginalForm, PolymorphAbilities, PolymorphState, PolymorphTimer,
};
use nethack_babel_engine::quest::{QuestNpcRole, QuestState};
use nethack_babel_engine::religion::ReligionState;
use nethack_babel_engine::spells::SpellBook;
use nethack_babel_engine::status::{Intrinsics, SpellProtection, StatusEffects};
use nethack_babel_engine::world::Attributes as EngineAttributes;
use nethack_babel_engine::world::{
    ArmorClass, Encumbrance, EncumbranceLevel, ExperienceLevel, GameWorld, HeroSpeed,
    HeroSpeedBonus, HitPoints, Monster, MonsterIdentity, MovementPoints, Name, Nutrition,
    PlayerCombat, Positioned, Power, Speed, Tame,
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

type RecoverResult = anyhow::Result<Option<(GameWorld, u32, i32, [u8; 32])>>;

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
    #[serde(default)]
    pub spell_protection: Option<SpellProtection>,
    #[serde(default)]
    pub original_form: Option<OriginalForm>,
    #[serde(default)]
    pub polymorph_timer: Option<PolymorphTimer>,
    #[serde(default)]
    pub polymorph_abilities: Option<PolymorphAbilities>,
    #[serde(default)]
    pub polymorph_state: Option<PolymorphState>,
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
    #[serde(default)]
    pub serial_id: u32,
    pub position: Position,
    pub hp_current: i32,
    pub hp_max: i32,
    pub speed: u32,
    pub movement_points: i32,
    pub name: String,
    #[serde(default)]
    pub monster_id: Option<MonsterId>,
    #[serde(default)]
    pub species_flags: Option<nethack_babel_data::schema::MonsterFlags>,
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
    #[serde(default)]
    pub pet_state: Option<PetState>,
    #[serde(default)]
    pub trapped: Option<nethack_babel_engine::traps::Trapped>,
    #[serde(default)]
    pub status_effects: StatusEffects,
}

/// Flattened item state with full component data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableItem {
    #[serde(default)]
    pub serial_id: u32,
    pub core: ObjectCore,
    #[serde(default)]
    pub name: Option<String>,
    pub buc: BucStatus,
    pub knowledge: KnowledgeState,
    pub location: ObjectLocation,
    pub enchantment: Option<i8>,
    pub erosion: Option<SerializableErosion>,
    #[serde(default)]
    pub container: Option<nethack_babel_engine::environment::Container>,
    #[serde(default)]
    pub wand_type: Option<nethack_babel_engine::wands::WandType>,
    #[serde(default)]
    pub wand_charges: Option<nethack_babel_engine::wands::WandCharges>,
    #[serde(default)]
    pub shop_state: Option<ShopState>,
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
    let original_form = world
        .get_component::<OriginalForm>(entity)
        .map(|state| (*state).clone());
    let polymorph_timer = world
        .get_component::<PolymorphTimer>(entity)
        .map(|timer| *timer);
    let polymorph_abilities = world
        .get_component::<PolymorphAbilities>(entity)
        .map(|abilities| *abilities);
    let polymorph_state = world
        .get_component::<PolymorphState>(entity)
        .map(|state| *state);
    let intrinsics = world
        .get_component::<Intrinsics>(entity)
        .map(|i| (*i).clone())
        .unwrap_or_default();
    let spell_protection = world
        .get_component::<SpellProtection>(entity)
        .map(|protection| (*protection).clone());
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
        spell_protection,
        original_form,
        polymorph_timer,
        polymorph_abilities,
        polymorph_state,
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
        let name = world
            .get_component::<Name>(entity)
            .map(|name| name.0.clone());

        let enchantment = world.get_component::<Enchantment>(entity).map(|e| e.spe);

        let erosion = world
            .get_component::<Erosion>(entity)
            .map(|e| SerializableErosion {
                eroded: e.eroded,
                eroded2: e.eroded2,
                erodeproof: e.erodeproof,
                greased: e.greased,
            });
        let container = world
            .get_component::<nethack_babel_engine::environment::Container>(entity)
            .map(|container| (*container).clone());
        let wand_type = world
            .get_component::<nethack_babel_engine::monster_ai::WandTypeTag>(entity)
            .map(|tag| tag.0);
        let wand_charges = world
            .get_component::<nethack_babel_engine::wands::WandCharges>(entity)
            .map(|charges| *charges);
        let shop_state = world
            .get_component::<ShopState>(entity)
            .map(|state| (*state).clone());

        items.push(SerializableItem {
            serial_id: entity.to_bits().get() as u32,
            core: core.clone(),
            name,
            buc,
            knowledge,
            location,
            enchantment,
            erosion,
            container,
            wand_type,
            wand_charges,
            shop_state,
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
        let monster_id = world
            .get_component::<MonsterIdentity>(entity)
            .map(|identity| identity.0)
            .or_else(|| {
                world
                    .monster_catalog()
                    .iter()
                    .find(|def| def.names.male.eq_ignore_ascii_case(&name))
                    .map(|def| def.id)
            });
        let species_flags = world
            .get_component::<nethack_babel_engine::monster_ai::MonsterSpeciesFlags>(entity)
            .map(|flags| flags.0);
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
        let pet_state = world
            .get_component::<PetState>(entity)
            .map(|pet_state| (*pet_state).clone());
        let trapped = world
            .get_component::<nethack_babel_engine::traps::Trapped>(entity)
            .map(|trapped| *trapped);
        let status_effects = world
            .get_component::<StatusEffects>(entity)
            .map(|status| (*status).clone())
            .unwrap_or_default();

        monsters.push(SerializableMonster {
            serial_id: entity.to_bits().get() as u32,
            position,
            hp_current,
            hp_max,
            speed,
            movement_points,
            name,
            monster_id,
            species_flags,
            is_tame,
            is_peaceful,
            creation_order,
            priest,
            shopkeeper,
            quest_npc_role,
            pet_state,
            trapped,
            status_effects,
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
    match &p.spell_protection {
        Some(spell_protection) => {
            if let Some(mut live_protection) = world.get_component_mut::<SpellProtection>(player) {
                *live_protection = spell_protection.clone();
            } else {
                let _ = world.ecs_mut().insert_one(player, spell_protection.clone());
            }
        }
        None => {
            let _ = world.ecs_mut().remove_one::<SpellProtection>(player);
        }
    }
    match &p.original_form {
        Some(original_form) => {
            if let Some(mut live_original) = world.get_component_mut::<OriginalForm>(player) {
                *live_original = original_form.clone();
            } else {
                let _ = world.ecs_mut().insert_one(player, original_form.clone());
            }
        }
        None => {
            let _ = world.ecs_mut().remove_one::<OriginalForm>(player);
        }
    }
    match p.polymorph_timer {
        Some(polymorph_timer) => {
            if let Some(mut live_timer) = world.get_component_mut::<PolymorphTimer>(player) {
                *live_timer = polymorph_timer;
            } else {
                let _ = world.ecs_mut().insert_one(player, polymorph_timer);
            }
        }
        None => {
            let _ = world.ecs_mut().remove_one::<PolymorphTimer>(player);
        }
    }
    match p.polymorph_abilities {
        Some(polymorph_abilities) => {
            if let Some(mut live_abilities) = world.get_component_mut::<PolymorphAbilities>(player)
            {
                *live_abilities = polymorph_abilities;
            } else {
                let _ = world.ecs_mut().insert_one(player, polymorph_abilities);
            }
        }
        None => {
            let _ = world.ecs_mut().remove_one::<PolymorphAbilities>(player);
        }
    }
    match p.polymorph_state {
        Some(polymorph_state) => {
            if let Some(mut live_state) = world.get_component_mut::<PolymorphState>(player) {
                *live_state = polymorph_state;
            } else {
                let _ = world.ecs_mut().insert_one(player, polymorph_state);
            }
        }
        None => {
            let _ = world.ecs_mut().remove_one::<PolymorphState>(player);
        }
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
        if let Some(name) = &item_data.name {
            entity_builder.add(Name(name.clone()));
        }
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
        if let Some(container) = &item_data.container {
            entity_builder.add(container.clone());
        }
        if let Some(wand_type) = item_data.wand_type {
            entity_builder.add(nethack_babel_engine::monster_ai::WandTypeTag(wand_type));
        }
        if let Some(wand_charges) = item_data.wand_charges {
            entity_builder.add(wand_charges);
        }
        if let Some(shop_state) = &item_data.shop_state {
            entity_builder.add(shop_state.clone());
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

    let mut item_id_map = std::collections::HashMap::new();
    for (idx, item_data) in data.items.iter().enumerate() {
        if let Some(item) = item_entities.get(idx).copied()
            && item_data.serial_id != 0
        {
            item_id_map.insert(item_data.serial_id, item);
        }
    }

    let mut monster_id_map = std::collections::HashMap::new();

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
        monster_id_map.insert(m.serial_id, entity.to_bits().get() as u32);

        if let Some(monster_id) = m.monster_id {
            let _ = world
                .ecs_mut()
                .insert_one(entity, MonsterIdentity(monster_id));
        }
        if let Some(species_flags) = m.species_flags {
            let _ = world.ecs_mut().insert_one(
                entity,
                nethack_babel_engine::monster_ai::MonsterSpeciesFlags(species_flags),
            );
        }

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
        if let Some(pet_state) = &m.pet_state {
            let _ = world.ecs_mut().insert_one(entity, pet_state.clone());
        }
        if let Some(trapped) = m.trapped {
            let _ = world.ecs_mut().insert_one(entity, trapped);
        }
        let _ = world.ecs_mut().insert_one(entity, m.status_effects.clone());
    }

    for (idx, item_data) in data.items.iter().enumerate() {
        if let ObjectLocation::Contained { container_id } = item_data.location
            && let Some(&remapped_container) = item_id_map.get(&container_id)
            && let Some(item) = item_entities.get(idx)
            && let Some(mut loc) = world.get_component_mut::<ObjectLocation>(*item)
        {
            *loc = ObjectLocation::Contained {
                container_id: remapped_container.to_bits().get() as u32,
            };
        }
        if let ObjectLocation::MonsterInventory { carrier_id } = item_data.location
            && let Some(&remapped_carrier_id) = monster_id_map.get(&carrier_id)
            && let Some(item) = item_entities.get(idx)
            && let Some(mut loc) = world.get_component_mut::<ObjectLocation>(*item)
        {
            *loc = ObjectLocation::MonsterInventory {
                carrier_id: remapped_carrier_id,
            };
        }
    }

    for shop in &mut world.dungeon_mut().shop_rooms {
        for entry in shop.bill.entries_mut() {
            if let Some(&remapped_item) = item_id_map.get(&(entry.item.to_bits().get() as u32)) {
                entry.item = remapped_item;
            }
        }
    }

    sync_current_level_npc_state(&mut world);
    nethack_babel_engine::shop::sync_item_shop_states(&mut world);

    world
}

fn restore_runtime_catalogs(world: &mut GameWorld, data: &GameData) {
    world.set_spawn_catalogs(data.monsters.clone(), data.objects.clone());
    if let Ok(data_dir) = resolved_runtime_data_dir() {
        world.set_game_content(nethack_babel_engine::rumors::GameContent::load(&data_dir));
    }
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
        if let Ok(guard) = PANIC_STATE.lock()
            && let Some(ref state) = *guard
        {
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
pub fn try_recover(player_name: &str) -> RecoverResult {
    let data = load_runtime_game_data()?;
    try_recover_with_data(player_name, &data)
}

pub fn try_recover_with_data(player_name: &str, data: &GameData) -> RecoverResult {
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
        schema::{MonsterSound, ObjectClass, ObjectTypeId},
    };
    use nethack_babel_engine::{
        action::{Direction, PlayerAction},
        artifacts::find_artifact_by_name,
        dungeon::{DungeonBranch, LevelMap, PortalLink, Terrain},
        event::{DeathCause, EngineEvent, HpSource},
        pets::PetState,
        role::Role,
        teleport::handle_magic_portal,
        turn::resolve_turn,
        world::Peaceful,
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

    fn monster_name_with_sound(world: &GameWorld, sound: MonsterSound) -> String {
        world
            .monster_catalog()
            .iter()
            .find(|def| def.sound == sound)
            .map(|def| def.names.male.clone())
            .unwrap_or_else(|| panic!("test catalog should contain a monster with sound {sound:?}"))
    }

    fn monster_name_and_id_matching(
        world: &GameWorld,
        predicate: impl Fn(&nethack_babel_data::schema::MonsterDef) -> bool,
    ) -> (String, nethack_babel_data::MonsterId) {
        world
            .monster_catalog()
            .iter()
            .find(|def| predicate(def))
            .map(|def| (def.names.male.clone(), def.id))
            .unwrap_or_else(|| panic!("test catalog should contain a monster matching predicate"))
    }

    fn monster_name_with_sound_excluding(
        world: &GameWorld,
        sound: MonsterSound,
        excluded: &[&str],
    ) -> String {
        world
            .monster_catalog()
            .iter()
            .find(|def| {
                def.sound == sound
                    && !excluded
                        .iter()
                        .any(|name| def.names.male.eq_ignore_ascii_case(name))
            })
            .map(|def| def.names.male.clone())
            .unwrap_or_else(|| panic!("test catalog should contain a monster with sound {sound:?}"))
    }

    fn make_tame_pet_state(
        world: &mut GameWorld,
        monster: hecs::Entity,
        tameness: u8,
        hungrytime: u32,
    ) {
        world
            .ecs_mut()
            .insert_one(monster, Tame)
            .expect("monster should accept tame marker");
        let mut pet_state = PetState::new(10, world.turn());
        pet_state.tameness = tameness;
        pet_state.hungrytime = hungrytime;
        world
            .ecs_mut()
            .insert_one(monster, pet_state)
            .expect("monster should accept pet state");
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

    fn spawn_floor_coin(world: &mut GameWorld, pos: Position) -> hecs::Entity {
        let creation_order = world.next_creation_order();
        world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(1),
                object_class: ObjectClass::Coin,
                quantity: 100,
                weight: 1,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            ObjectLocation::Floor {
                x: pos.x as i16,
                y: pos.y as i16,
                level: nethack_babel_data::schema::DungeonLevel {
                    branch: nethack_babel_engine::dungeon::data_branch_id(world.dungeon().branch),
                    depth: world.dungeon().depth as i16,
                },
            },
            creation_order,
        ))
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

    fn monster_carries_named_item(world: &GameWorld, monster: hecs::Entity, name: &str) -> bool {
        let carrier_id = monster.to_bits().get() as u32;
        let expected_artifact_id = nethack_babel_engine::artifacts::find_artifact_by_name(name)
            .map(|artifact| artifact.id);
        world
            .ecs()
            .query::<(&ObjectCore, &ObjectLocation)>()
            .iter()
            .any(|(item, (core, loc))| {
                matches!(
                    *loc,
                    ObjectLocation::MonsterInventory { carrier_id: cid } if cid == carrier_id
                ) && (nethack_babel_engine::turn::force_item_display_name(world, item)
                    .is_some_and(|item_name| item_name.eq_ignore_ascii_case(name))
                    || expected_artifact_id
                        .is_some_and(|artifact_id| core.artifact == Some(artifact_id)))
            })
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

    fn resolve_object_type_by_spec(world: &GameWorld, spec: &str) -> Option<ObjectTypeId> {
        let normalized = spec.trim().to_ascii_lowercase();
        world.object_catalog().iter().find_map(|def| {
            let canonical = def.name.to_ascii_lowercase();
            let prefixed = match def.class {
                ObjectClass::Wand => format!("wand of {canonical}"),
                ObjectClass::Scroll => format!("scroll of {canonical}"),
                ObjectClass::Potion => format!("potion of {canonical}"),
                ObjectClass::Ring => format!("ring of {canonical}"),
                ObjectClass::Spellbook => format!("spellbook of {canonical}"),
                ObjectClass::Amulet => format!("amulet of {canonical}"),
                _ => canonical.clone(),
            };
            (normalized == canonical || normalized == prefixed).then_some(def.id)
        })
    }

    fn monster_id_with_sound_excluding(
        world: &GameWorld,
        sound: MonsterSound,
        excluded: &[&str],
    ) -> MonsterId {
        world
            .monster_catalog()
            .iter()
            .find(|def| {
                def.sound == sound
                    && !excluded
                        .iter()
                        .any(|name| def.names.male.eq_ignore_ascii_case(name))
            })
            .map(|def| def.id)
            .unwrap_or_else(|| panic!("test catalog should contain a monster with sound {sound:?}"))
    }

    fn count_objects_named(world: &GameWorld, name: &str) -> usize {
        let Some(object_type) = resolve_object_type_by_spec(world, name) else {
            return 0;
        };
        world
            .ecs()
            .query::<&ObjectCore>()
            .iter()
            .filter(|(_, core)| core.otyp == object_type)
            .count()
    }

    fn spawn_inventory_object_by_name(
        world: &mut GameWorld,
        name: &str,
        letter: char,
    ) -> hecs::Entity {
        let data = test_game_data();
        let normalized = name.trim().to_ascii_lowercase();
        let object_def = data
            .objects
            .iter()
            .find_map(|def| {
                let canonical = def.name.to_ascii_lowercase();
                let prefixed = match def.class {
                    ObjectClass::Wand => format!("wand of {canonical}"),
                    ObjectClass::Scroll => format!("scroll of {canonical}"),
                    ObjectClass::Potion => format!("potion of {canonical}"),
                    ObjectClass::Ring => format!("ring of {canonical}"),
                    ObjectClass::Spellbook => format!("spellbook of {canonical}"),
                    ObjectClass::Amulet => format!("amulet of {canonical}"),
                    _ => canonical.clone(),
                };
                (normalized == canonical || normalized == prefixed).then_some(def)
            })
            .unwrap_or_else(|| panic!("{name} should resolve against the object catalog"));
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
        let entity = world.spawn((
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
        ));
        if let Some(monster_id) = world
            .monster_catalog()
            .iter()
            .find(|def| def.names.male.eq_ignore_ascii_case(name))
            .map(|def| def.id)
        {
            let _ = world
                .ecs_mut()
                .insert_one(entity, MonsterIdentity(monster_id));
        }
        entity
    }

    fn spawn_monster_with_symbol(
        world: &mut GameWorld,
        pos: Position,
        name: &str,
        hp: i32,
        symbol: char,
        sleeping: u32,
    ) -> hecs::Entity {
        let entity = spawn_full_monster(world, pos, name, hp);
        if let Some(mut display) =
            world.get_component_mut::<nethack_babel_engine::world::DisplaySymbol>(entity)
        {
            display.symbol = symbol;
        }
        if sleeping > 0 {
            world
                .ecs_mut()
                .insert_one(
                    entity,
                    nethack_babel_engine::status::StatusEffects {
                        sleeping,
                        ..Default::default()
                    },
                )
                .expect("monster should accept sleeping status");
        }
        entity
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
        MedusaRevisit,
        CastleRevisit,
        OrcusRevisit,
        FortLudiosRevisit,
        VladTopEntry,
        InvocationPortalRevisit,
        ShopEntry,
        ShopEntryWelcomeBack,
        ShopEntryRobbed,
        ShopkeeperFollow,
        ShopkeeperPayoff,
        ShopkeeperCredit,
        ShopCreditCovers,
        ShopPartialPayment,
        ShopNoMoney,
        ShopWandUsageFee,
        ShopGreaseUsageFee,
        ShopLampUsageFee,
        ShopCameraUsageFee,
        ShopTinningUsageFee,
        ShopSpellbookUsageFee,
        ShopkeeperSell,
        ShopChatPriceQuote,
        ShopContainerPickup,
        DemonBribe,
        ShopRepair,
        ShopkeeperDeath,
        ShopRobbery,
        ShopRestitution,
        TempleWrongAlignment,
        TempleAleGift,
        TempleVirtuesOfPoverty,
        TempleDonationThanks,
        TemplePious,
        TempleDonation,
        TempleBlessing,
        TempleCleansing,
        TempleSelflessGenerosity,
        TempleWrath,
        TempleCalm,
        OracleConsultation,
        UntendedTempleGhost,
        SanctumRevisit,
        WizardHarassment,
        WizardTaunt,
        WizardIntervention,
        WizardAmuletWake,
        WizardBlackGlowBlind,
        WizardCovetousGroundAmulet,
        WizardCovetousMonsterTool,
        WizardCovetousQuestArtifact,
        WizardRetreatHeal,
        HumanoidAlohaChat,
        HobbitComplaintChat,
        VampireKindredChat,
        VampireNightChat,
        VampireMidnightChat,
        WereFullMoonChat,
        WereDaytimeMoonChat,
        ChatPreconditionBlocks,
        WizardLevelTeleport,
        EndgameAscension,
    }

    impl SaveStoryTraversalScenario {
        fn label(self) -> &'static str {
            match self {
                SaveStoryTraversalScenario::QuestClosure => "quest-closure",
                SaveStoryTraversalScenario::QuestLeaderAnger => "quest-leader-anger",
                SaveStoryTraversalScenario::MedusaRevisit => "medusa-revisit",
                SaveStoryTraversalScenario::CastleRevisit => "castle-revisit",
                SaveStoryTraversalScenario::OrcusRevisit => "orcus-revisit",
                SaveStoryTraversalScenario::FortLudiosRevisit => "fort-ludios-revisit",
                SaveStoryTraversalScenario::VladTopEntry => "vlad-top-entry",
                SaveStoryTraversalScenario::InvocationPortalRevisit => "invocation-portal-revisit",
                SaveStoryTraversalScenario::ShopEntry => "shop-entry",
                SaveStoryTraversalScenario::ShopEntryWelcomeBack => "shop-entry-welcome-back",
                SaveStoryTraversalScenario::ShopEntryRobbed => "shop-entry-robbed",
                SaveStoryTraversalScenario::ShopkeeperFollow => "shopkeeper-follow",
                SaveStoryTraversalScenario::ShopkeeperPayoff => "shopkeeper-payoff",
                SaveStoryTraversalScenario::ShopkeeperCredit => "shopkeeper-credit",
                SaveStoryTraversalScenario::ShopCreditCovers => "shop-credit-covers",
                SaveStoryTraversalScenario::ShopPartialPayment => "shop-partial-payment",
                SaveStoryTraversalScenario::ShopNoMoney => "shop-no-money",
                SaveStoryTraversalScenario::ShopWandUsageFee => "shop-wand-usage-fee",
                SaveStoryTraversalScenario::ShopGreaseUsageFee => "shop-grease-usage-fee",
                SaveStoryTraversalScenario::ShopLampUsageFee => "shop-lamp-usage-fee",
                SaveStoryTraversalScenario::ShopCameraUsageFee => "shop-camera-usage-fee",
                SaveStoryTraversalScenario::ShopTinningUsageFee => "shop-tinning-usage-fee",
                SaveStoryTraversalScenario::ShopSpellbookUsageFee => "shop-spellbook-usage-fee",
                SaveStoryTraversalScenario::ShopkeeperSell => "shopkeeper-sell",
                SaveStoryTraversalScenario::ShopChatPriceQuote => "shop-chat-price-quote",
                SaveStoryTraversalScenario::ShopContainerPickup => "shop-container-pickup",
                SaveStoryTraversalScenario::DemonBribe => "demon-bribe",
                SaveStoryTraversalScenario::ShopRepair => "shop-repair",
                SaveStoryTraversalScenario::ShopkeeperDeath => "shopkeeper-death",
                SaveStoryTraversalScenario::ShopRobbery => "shop-robbery",
                SaveStoryTraversalScenario::ShopRestitution => "shop-restitution",
                SaveStoryTraversalScenario::TempleWrongAlignment => "temple-wrong-alignment",
                SaveStoryTraversalScenario::TempleAleGift => "temple-ale-gift",
                SaveStoryTraversalScenario::TempleVirtuesOfPoverty => "temple-virtues-of-poverty",
                SaveStoryTraversalScenario::TempleDonationThanks => "temple-donation-thanks",
                SaveStoryTraversalScenario::TemplePious => "temple-pious",
                SaveStoryTraversalScenario::TempleDonation => "temple-donation",
                SaveStoryTraversalScenario::TempleBlessing => "temple-blessing",
                SaveStoryTraversalScenario::TempleCleansing => "temple-cleansing",
                SaveStoryTraversalScenario::TempleSelflessGenerosity => {
                    "temple-selfless-generosity"
                }
                SaveStoryTraversalScenario::TempleWrath => "temple-wrath",
                SaveStoryTraversalScenario::TempleCalm => "temple-calm",
                SaveStoryTraversalScenario::OracleConsultation => "oracle-consultation",
                SaveStoryTraversalScenario::UntendedTempleGhost => "untended-temple-ghost",
                SaveStoryTraversalScenario::SanctumRevisit => "sanctum-revisit",
                SaveStoryTraversalScenario::WizardHarassment => "wizard-harassment",
                SaveStoryTraversalScenario::WizardTaunt => "wizard-taunt",
                SaveStoryTraversalScenario::WizardIntervention => "wizard-intervention",
                SaveStoryTraversalScenario::WizardAmuletWake => "wizard-amulet-wake",
                SaveStoryTraversalScenario::WizardBlackGlowBlind => "wizard-black-glow-blind",
                SaveStoryTraversalScenario::WizardCovetousGroundAmulet => {
                    "wizard-covetous-ground-amulet"
                }
                SaveStoryTraversalScenario::WizardCovetousMonsterTool => {
                    "wizard-covetous-monster-tool"
                }
                SaveStoryTraversalScenario::WizardCovetousQuestArtifact => {
                    "wizard-covetous-quest-artifact"
                }
                SaveStoryTraversalScenario::WizardRetreatHeal => "wizard-retreat-heal",
                SaveStoryTraversalScenario::HumanoidAlohaChat => "humanoid-aloha-chat",
                SaveStoryTraversalScenario::HobbitComplaintChat => "hobbit-complaint-chat",
                SaveStoryTraversalScenario::VampireKindredChat => "vampire-kindred-chat",
                SaveStoryTraversalScenario::VampireNightChat => "vampire-night-chat",
                SaveStoryTraversalScenario::VampireMidnightChat => "vampire-midnight-chat",
                SaveStoryTraversalScenario::WereFullMoonChat => "were-full-moon-chat",
                SaveStoryTraversalScenario::WereDaytimeMoonChat => "were-daytime-moon-chat",
                SaveStoryTraversalScenario::ChatPreconditionBlocks => "chat-precondition-blocks",
                SaveStoryTraversalScenario::WizardLevelTeleport => "wizard-level-teleport",
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
            SaveStoryTraversalScenario::MedusaRevisit => {
                let mut world = make_stair_world(DungeonBranch::Main, 23, Terrain::StairsDown);
                let mut rng = Pcg64::seed_from_u64(7107);

                let enter_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );
                assert_eq!(count_monsters_named(&world, "medusa"), 1);

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-medusa-revisit", &world, [56u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);

                let medusa_down =
                    find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
                        .expect("Medusa level should preserve stairs down after save/load");
                set_player_position(&mut loaded, medusa_down);
                let descend_events = resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);
                assert!(
                    descend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );

                let castle_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
                    .expect("Castle should preserve stairs up after save/load");
                set_player_position(&mut loaded, castle_up);
                let revisit_events = resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);
                (loaded, revisit_events)
            }
            SaveStoryTraversalScenario::CastleRevisit => {
                let mut world = make_stair_world(DungeonBranch::Main, 24, Terrain::StairsDown);
                let mut rng = Pcg64::seed_from_u64(7154);

                let enter_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );
                assert_eq!(count_objects_named(&world, "wand of wishing"), 1);

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-castle-revisit", &world, [59u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);

                let castle_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
                    .expect("Castle should preserve stairs up after save/load");
                set_player_position(&mut loaded, castle_up);
                let ascend_events = resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);
                assert!(
                    ascend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );

                let medusa_down =
                    find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
                        .expect("Medusa should preserve stairs down after save/load");
                set_player_position(&mut loaded, medusa_down);
                let revisit_events = resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);
                (loaded, revisit_events)
            }
            SaveStoryTraversalScenario::OrcusRevisit => {
                let mut world = make_stair_world(DungeonBranch::Gehennom, 11, Terrain::StairsDown);
                let mut rng = Pcg64::seed_from_u64(7108);

                let enter_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );
                assert_eq!(count_monsters_named(&world, "orcus"), 1);

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-orcus-revisit", &world, [57u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);

                let orcus_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
                    .expect("Orcus level should preserve stairs up after save/load");
                set_player_position(&mut loaded, orcus_up);
                let ascend_events = resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);
                assert!(
                    ascend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );

                let gehennom_down =
                    find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
                        .expect("cached Gehennom level should preserve stairs down");
                set_player_position(&mut loaded, gehennom_down);
                let revisit_events = resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);
                (loaded, revisit_events)
            }
            SaveStoryTraversalScenario::FortLudiosRevisit => {
                let mut world = make_stair_world(DungeonBranch::Main, 20, Terrain::MagicPortal);
                let mut rng = Pcg64::seed_from_u64(7110);
                let player = world.player();
                let portal_pos = Position::new(5, 5);
                world.dungeon_mut().add_portal(PortalLink {
                    from_branch: DungeonBranch::Main,
                    from_depth: 20,
                    from_pos: portal_pos,
                    to_branch: DungeonBranch::FortLudios,
                    to_depth: 1,
                    to_pos: portal_pos,
                });
                world.dungeon_mut().add_portal(PortalLink {
                    from_branch: DungeonBranch::FortLudios,
                    from_depth: 1,
                    from_pos: portal_pos,
                    to_branch: DungeonBranch::Main,
                    to_depth: 20,
                    to_pos: portal_pos,
                });

                let enter_events = handle_magic_portal(&mut world, player, &mut rng);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );
                assert_eq!(count_monsters_named(&world, "soldier"), 2);
                assert_eq!(count_monsters_named(&world, "lieutenant"), 1);
                assert_eq!(count_monsters_named(&world, "captain"), 1);

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-fort-ludios-revisit", &world, [60u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let loaded_player = loaded.player();

                loaded
                    .dungeon_mut()
                    .current_level
                    .set_terrain(portal_pos, Terrain::MagicPortal);
                set_player_position(&mut loaded, portal_pos);
                let return_events = handle_magic_portal(&mut loaded, loaded_player, &mut rng);
                assert!(
                    return_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );

                loaded
                    .dungeon_mut()
                    .current_level
                    .set_terrain(portal_pos, Terrain::MagicPortal);
                set_player_position(&mut loaded, portal_pos);
                let revisit_events = handle_magic_portal(&mut loaded, loaded_player, &mut rng);
                (loaded, revisit_events)
            }
            SaveStoryTraversalScenario::VladTopEntry => {
                let mut world = make_stair_world(DungeonBranch::VladsTower, 2, Terrain::StairsDown);
                install_test_catalogs(&mut world);
                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-vlad-top-entry", &world, [61u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let candelabrum_name = "Candelabrum of Invocation";

                let enter_events = resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );
                assert_eq!(count_monsters_named(&loaded, "Vlad the Impaler"), 1);
                assert_eq!(count_objects_named(&loaded, candelabrum_name), 1);
                (loaded, enter_events)
            }
            SaveStoryTraversalScenario::InvocationPortalRevisit => {
                let mut world = make_stair_world(DungeonBranch::Gehennom, 20, Terrain::StairsDown);
                let mut rng = Pcg64::seed_from_u64(7109);

                let enter_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );

                let player = world.player();
                if let Some(mut flags) = world.get_component_mut::<PlayerEvents>(player) {
                    flags.invoked = true;
                    flags.found_vibrating_square = true;
                }

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-invocation-portal-revisit",
                    &world,
                    [58u8; 32],
                );
                let mut rng = Pcg64::from_seed(loaded_rng);

                let current_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
                    .expect("Gehennom 21 should preserve stairs up after save/load");
                set_player_position(&mut loaded, current_up);
                let ascend_events = resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);
                assert!(
                    ascend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                );

                let cached_down =
                    find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
                        .expect("Gehennom 20 should preserve stairs down after save/load");
                set_player_position(&mut loaded, cached_down);
                let revisit_events = resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);
                (loaded, revisit_events)
            }
            SaveStoryTraversalScenario::ShopEntry => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 20);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, nethack_babel_engine::world::Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
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

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-entry", &world, [53u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Move {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopEntryWelcomeBack => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 20);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, nethack_babel_engine::world::Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
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
                world.dungeon_mut().shop_rooms[0].surcharge = true;

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-shop-entry-welcome-back",
                    &world,
                    [54u8; 32],
                );
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Move {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopEntryRobbed => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 20);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, nethack_babel_engine::world::Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
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
                world.dungeon_mut().shop_rooms[0].robbed = 75;

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-entry-robbed", &world, [55u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Move {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (loaded, events)
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
                let warning_events = resolve_turn(
                    &mut world,
                    PlayerAction::Move {
                        direction: Direction::West,
                    },
                    &mut rng,
                );
                assert!(warning_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "shop-leave-warning"
                )));

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shopkeeper-follow", &world, [27u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Move {
                        direction: Direction::West,
                    },
                    &mut rng,
                );
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
            SaveStoryTraversalScenario::ShopkeeperCredit => {
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
                world.dungeon_mut().shop_rooms[0].debit = 50;
                world.dungeon_mut().shop_rooms[0].angry = true;
                world.dungeon_mut().shop_rooms[0].surcharge = true;

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shopkeeper-credit", &world, [39u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let loaded_gold = spawn_inventory_gold(&mut loaded, 150, 'g');
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Drop { item: loaded_gold },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopCreditCovers => {
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
                world.dungeon_mut().shop_rooms[0].credit = 150;
                world.dungeon_mut().shop_rooms[0].debit = 20;
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
                    "shop bill should accept a credited entry"
                );

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-credit-covers", &world, [61u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(&mut loaded, PlayerAction::Pay, &mut rng);
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopPartialPayment => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let _gold = spawn_inventory_gold(&mut world, 50, 'g');
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
                    "shop bill should accept an underfunded entry"
                );

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-partial-payment", &world, [62u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(&mut loaded, PlayerAction::Pay, &mut rng);
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopNoMoney => {
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
                    "shop bill should accept a zero-funds entry"
                );

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-no-money", &world, [63u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(&mut loaded, PlayerAction::Pay, &mut rng);
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopWandUsageFee => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
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
                let wand = world.spawn((
                    ObjectCore {
                        otyp: ObjectTypeId(0),
                        object_class: ObjectClass::Wand,
                        quantity: 1,
                        weight: 7,
                        age: 0,
                        inv_letter: Some('z'),
                        artifact: None,
                    },
                    ObjectLocation::Inventory,
                    nethack_babel_engine::monster_ai::WandTypeTag(
                        nethack_babel_engine::wands::WandType::Light,
                    ),
                    nethack_babel_engine::wands::WandCharges {
                        spe: 2,
                        recharged: 0,
                    },
                ));
                if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
                    inv.items.push(wand);
                }
                assert!(
                    world.dungeon_mut().shop_rooms[0].bill.add(wand, 100, 1),
                    "shop bill should accept an unpaid wand"
                );

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-wand-usage-fee", &world, [64u8; 32]);
                let loaded_wand = loaded
                    .get_component::<Inventory>(loaded.player())
                    .and_then(|inv| {
                        inv.items.iter().copied().find(|item| {
                            loaded
                                .get_component::<ObjectCore>(*item)
                                .is_some_and(|core| {
                                    core.object_class == ObjectClass::Wand
                                        && core.inv_letter == Some('z')
                                })
                        })
                    })
                    .expect("reloaded inventory should preserve the unpaid wand");
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::ZapWand {
                        item: loaded_wand,
                        direction: None,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopGreaseUsageFee => {
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
                let grease = spawn_inventory_object_by_name(&mut world, "can of grease", 'g');
                world
                    .ecs_mut()
                    .insert_one(grease, Enchantment { spe: 2 })
                    .expect("grease should accept charges");
                assert!(
                    world.dungeon_mut().shop_rooms[0].bill.add(grease, 100, 1),
                    "shop bill should accept unpaid grease"
                );

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-grease-usage-fee", &world, [65u8; 32]);
                let loaded_grease = loaded
                    .get_component::<Inventory>(loaded.player())
                    .and_then(|inv| {
                        inv.items.iter().copied().find(|item| {
                            loaded
                                .get_component::<ObjectCore>(*item)
                                .is_some_and(|core| {
                                    core.object_class == ObjectClass::Tool
                                        && core.inv_letter == Some('g')
                                })
                        })
                    })
                    .expect("reloaded inventory should preserve the unpaid grease");
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Apply {
                        item: loaded_grease,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopLampUsageFee => {
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
                let lamp = spawn_inventory_object_by_name(&mut world, "oil lamp", 'l');
                assert!(
                    world.dungeon_mut().shop_rooms[0].bill.add(lamp, 100, 1),
                    "shop bill should accept unpaid lamp"
                );

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-lamp-usage-fee", &world, [66u8; 32]);
                let loaded_lamp = loaded
                    .get_component::<Inventory>(loaded.player())
                    .and_then(|inv| {
                        inv.items.iter().copied().find(|item| {
                            loaded
                                .get_component::<ObjectCore>(*item)
                                .is_some_and(|core| {
                                    core.object_class == ObjectClass::Tool
                                        && core.inv_letter == Some('l')
                                })
                        })
                    })
                    .expect("reloaded inventory should preserve the unpaid lamp");
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Apply { item: loaded_lamp },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopCameraUsageFee => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player_pos = world
                    .get_component::<Positioned>(world.player())
                    .expect("player should have a position")
                    .0;
                let shopkeeper = spawn_full_monster(
                    &mut world,
                    Position::new(player_pos.x + 2, player_pos.y),
                    "Izchak",
                    20,
                );
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, nethack_babel_engine::world::Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(nethack_babel_engine::shop::ShopRoom::new(
                        Position::new(player_pos.x, player_pos.y - 1),
                        Position::new(player_pos.x + 2, player_pos.y + 1),
                        nethack_babel_engine::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                spawn_full_monster(
                    &mut world,
                    Position::new(player_pos.x + 1, player_pos.y),
                    "goblin",
                    5,
                );
                let camera = spawn_inventory_object_by_name(&mut world, "expensive camera", 'c');
                world
                    .ecs_mut()
                    .insert_one(camera, Enchantment { spe: 2 })
                    .expect("camera should accept charges");
                assert!(
                    world.dungeon_mut().shop_rooms[0].bill.add(camera, 100, 1),
                    "shop bill should accept unpaid camera"
                );

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-camera-usage-fee", &world, [67u8; 32]);
                let loaded_camera = loaded
                    .get_component::<Inventory>(loaded.player())
                    .and_then(|inv| {
                        inv.items.iter().copied().find(|item| {
                            loaded
                                .get_component::<ObjectCore>(*item)
                                .is_some_and(|core| {
                                    core.object_class == ObjectClass::Tool
                                        && core.inv_letter == Some('c')
                                })
                        })
                    })
                    .expect("reloaded inventory should preserve the unpaid camera");
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Apply {
                        item: loaded_camera,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopTinningUsageFee => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player_pos = world
                    .get_component::<Positioned>(world.player())
                    .expect("player should have a position")
                    .0;
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
                world.spawn((
                    ObjectCore {
                        otyp: ObjectTypeId(0),
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
                        level: world.dungeon().current_data_dungeon_level(),
                    },
                    Name("giant ant corpse".to_string()),
                ));
                let tinning_kit = spawn_inventory_object_by_name(&mut world, "tinning kit", 't');
                world
                    .ecs_mut()
                    .insert_one(tinning_kit, Enchantment { spe: 2 })
                    .expect("tinning kit should accept charges");
                assert!(
                    world.dungeon_mut().shop_rooms[0]
                        .bill
                        .add(tinning_kit, 100, 1),
                    "shop bill should accept unpaid tinning kit"
                );

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-shop-tinning-usage-fee",
                    &world,
                    [68u8; 32],
                );
                let loaded_tinning_kit = loaded
                    .get_component::<Inventory>(loaded.player())
                    .and_then(|inv| {
                        inv.items.iter().copied().find(|item| {
                            loaded
                                .get_component::<ObjectCore>(*item)
                                .is_some_and(|core| {
                                    core.object_class == ObjectClass::Tool
                                        && core.inv_letter == Some('t')
                                })
                        })
                    })
                    .expect("reloaded inventory should preserve the unpaid tinning kit");
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Apply {
                        item: loaded_tinning_kit,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopSpellbookUsageFee => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                install_test_catalogs(&mut world);
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
                        nethack_babel_engine::shop::ShopType::Book,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                let book = spawn_inventory_object_by_name(&mut world, "spellbook of light", 'b');
                assert!(
                    world.dungeon_mut().shop_rooms[0].bill.add(book, 100, 1),
                    "shop bill should accept unpaid spellbook"
                );

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-shop-spellbook-usage-fee",
                    &world,
                    [69u8; 32],
                );
                let loaded_book = loaded
                    .get_component::<Inventory>(loaded.player())
                    .and_then(|inv| {
                        inv.items.iter().copied().find(|item| {
                            loaded
                                .get_component::<ObjectCore>(*item)
                                .is_some_and(|core| {
                                    core.object_class == ObjectClass::Spellbook
                                        && core.inv_letter == Some('b')
                                })
                        })
                    })
                    .expect("reloaded inventory should preserve the unpaid spellbook");
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Read {
                        item: Some(loaded_book),
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopkeeperSell => {
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
                world.dungeon_mut().shop_rooms[0].shopkeeper_gold = 80;

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shopkeeper-sell", &world, [41u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let loaded_item = spawn_inventory_object_by_name(&mut loaded, "pick-axe", 'p');
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Drop { item: loaded_item },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopChatPriceQuote => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 6), "Izchak", 20);
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
                let item = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
                let second_item = spawn_inventory_object_by_name(&mut world, "lock pick", 'q');
                if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
                    inv.items.retain(|entry| *entry != item);
                    inv.items.retain(|entry| *entry != second_item);
                }
                let current_level = world.dungeon().current_data_dungeon_level();
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item) {
                    *loc = ObjectLocation::Floor {
                        x: 5,
                        y: 5,
                        level: current_level,
                    };
                }
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(second_item) {
                    *loc = ObjectLocation::Floor {
                        x: 5,
                        y: 5,
                        level: current_level,
                    };
                }

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-chat-price-quote", &world, [64u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Chat {
                        direction: Direction::North,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopContainerPickup => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 6), "Izchak", 20);
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

                let sack = spawn_inventory_object_by_name(&mut world, "sack", 's');
                let pick_axe = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
                let lock_pick = spawn_inventory_object_by_name(&mut world, "lock pick", 'q');
                if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
                    inv.items.retain(|entry| !matches!(*entry, e if e == sack || e == pick_axe || e == lock_pick));
                }
                let current_level = world.dungeon().current_data_dungeon_level();
                let sack_id = sack.to_bits().get() as u32;
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(sack) {
                    *loc = ObjectLocation::Floor {
                        x: 5,
                        y: 5,
                        level: current_level.clone(),
                    };
                }
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(pick_axe) {
                    *loc = ObjectLocation::Contained {
                        container_id: sack_id,
                    };
                }
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(lock_pick) {
                    *loc = ObjectLocation::Contained {
                        container_id: sack_id,
                    };
                }
                world
                    .ecs_mut()
                    .insert_one(
                        sack,
                        nethack_babel_engine::environment::Container {
                            container_type: nethack_babel_engine::environment::ContainerType::Sack,
                            locked: false,
                            trapped: false,
                        },
                    )
                    .expect("sack should accept container component");

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-container-pickup", &world, [66u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(&mut loaded, PlayerAction::PickUp, &mut rng);
                (loaded, events)
            }
            SaveStoryTraversalScenario::DemonBribe => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let (demon_name, demon_id) = monster_name_and_id_matching(&world, |def| {
                    def.sound == MonsterSound::Bribe
                        && def
                            .flags
                            .contains(nethack_babel_data::schema::MonsterFlags::DEMON)
                });
                let demon = spawn_full_monster(&mut world, Position::new(6, 5), &demon_name, 12);
                world
                    .ecs_mut()
                    .insert(
                        demon,
                        (
                            nethack_babel_engine::world::MonsterIdentity(demon_id),
                            nethack_babel_engine::world::Peaceful,
                        ),
                    )
                    .expect("demon should accept identity and peaceful state");
                spawn_inventory_gold(&mut world, 500, '$');

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-demon-bribe", &world, [66u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::BribeDemon {
                        direction: Direction::East,
                        amount: 500,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopRepair => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                set_player_position(&mut world, Position::new(6, 6));
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
                let damaged_pos = Position::new(5, 5);
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(damaged_pos, Terrain::Floor);
                nethack_babel_engine::shop::record_shop_damage(
                    &mut world.dungeon_mut().shop_rooms[0],
                    damaged_pos,
                    nethack_babel_engine::shop::ShopDamageType::DoorBroken,
                );
                sync_current_level_npc_state(&mut world);

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-repair", &world, [37u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(&mut loaded, PlayerAction::Rest, &mut rng);
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopkeeperDeath => {
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
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(shopkeeper) {
                    hp.current = 1;
                    hp.max = 1;
                }
                if let Some(mut mp) = world.get_component_mut::<MovementPoints>(shopkeeper) {
                    mp.0 = 0;
                }

                let mut rng = Pcg64::seed_from_u64(7133);
                let mut death_events = Vec::new();
                for _ in 0..8 {
                    if let Some(mut mp) = world.get_component_mut::<MovementPoints>(shopkeeper) {
                        mp.0 = 0;
                    }
                    death_events.extend(resolve_turn(
                        &mut world,
                        PlayerAction::FightDirection {
                            direction: Direction::East,
                        },
                        &mut rng,
                    ));
                    if death_events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "shop-keeper-dead"
                        )
                    }) {
                        break;
                    }
                }
                assert!(death_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "shop-keeper-dead"
                )));

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shopkeeper-death", &world, [61u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Move {
                        direction: Direction::West,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopRobbery => {
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

                let mut rng = Pcg64::seed_from_u64(7105);
                let warning_events = resolve_turn(
                    &mut world,
                    PlayerAction::Move {
                        direction: Direction::West,
                    },
                    &mut rng,
                );
                assert!(warning_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "shop-leave-warning"
                )));

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-robbery", &world, [31u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Move {
                        direction: Direction::West,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::ShopRestitution => {
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
                world.dungeon_mut().shop_rooms[0].robbed = 5;
                world.dungeon_mut().shop_rooms[0].angry = true;
                world.dungeon_mut().shop_rooms[0].surcharge = true;
                sync_current_level_npc_state(&mut world);

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-shop-restitution", &world, [33u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let loaded_item = spawn_inventory_object_by_name(&mut loaded, "pick-axe", 'p');
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::Drop { item: loaded_item },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::TempleWrongAlignment => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 500, 'g');
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(priest, nethack_babel_engine::world::Peaceful)
                    .expect("priest should accept peaceful marker");
                world
                    .ecs_mut()
                    .insert_one(
                        priest,
                        Priest {
                            alignment: Alignment::Chaotic,
                            has_shrine: true,
                            is_high_priest: false,
                            angry: false,
                        },
                    )
                    .expect("priest should accept explicit runtime state");

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-temple-wrong-alignment",
                    &world,
                    [60u8; 32],
                );
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
            SaveStoryTraversalScenario::TempleAleGift => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(priest, nethack_babel_engine::world::Peaceful)
                    .expect("priest should accept peaceful marker");

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-temple-ale-gift", &world, [34u8; 32]);
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
            SaveStoryTraversalScenario::TempleVirtuesOfPoverty => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(priest, nethack_babel_engine::world::Peaceful)
                    .expect("priest should accept peaceful marker");
                world
                    .ecs_mut()
                    .insert_one(
                        priest,
                        Priest {
                            alignment: Alignment::Lawful,
                            has_shrine: false,
                            is_high_priest: false,
                            angry: false,
                        },
                    )
                    .expect("priest should accept explicit runtime state");

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-temple-virtues-of-poverty",
                    &world,
                    [57u8; 32],
                );
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
            SaveStoryTraversalScenario::TempleDonationThanks => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 100, 'g');
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(priest, nethack_babel_engine::world::Peaceful)
                    .expect("priest should accept peaceful marker");

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-temple-donation-thanks",
                    &world,
                    [35u8; 32],
                );
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
            SaveStoryTraversalScenario::TemplePious => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 300, 'g');
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(priest, nethack_babel_engine::world::Peaceful)
                    .expect("priest should accept peaceful marker");

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-temple-pious", &world, [58u8; 32]);
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
            SaveStoryTraversalScenario::TempleDonation => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 1_000, 'g');
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(priest, nethack_babel_engine::world::Peaceful)
                    .expect("priest should accept peaceful marker");

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-temple-donation", &world, [32u8; 32]);
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
            SaveStoryTraversalScenario::TempleBlessing => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 300, 'g');
                let mut religion = wizard_story_religion(&world, player);
                religion.alignment = Alignment::Lawful;
                religion.original_alignment = Alignment::Lawful;
                religion.alignment_record = -5;
                world
                    .ecs_mut()
                    .insert_one(player, religion)
                    .expect("player should accept religion state");
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(priest, nethack_babel_engine::world::Peaceful)
                    .expect("priest should accept peaceful marker");

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-temple-blessing", &world, [36u8; 32]);
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
            SaveStoryTraversalScenario::TempleCleansing => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 700, 'g');
                let mut religion = wizard_story_religion(&world, player);
                religion.alignment = Alignment::Lawful;
                religion.original_alignment = Alignment::Lawful;
                religion.alignment_record = -5;
                world
                    .ecs_mut()
                    .insert_one(player, religion)
                    .expect("player should accept religion state");
                world
                    .ecs_mut()
                    .insert_one(
                        player,
                        nethack_babel_engine::status::SpellProtection {
                            layers: 1,
                            countdown: 10,
                            interval: 10,
                        },
                    )
                    .expect("player should accept spell protection");
                while world.turn() <= 5001 {
                    world.advance_turn();
                }
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(priest, nethack_babel_engine::world::Peaceful)
                    .expect("priest should accept peaceful marker");

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-temple-cleansing", &world, [37u8; 32]);
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
            SaveStoryTraversalScenario::TempleSelflessGenerosity => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 700, 'g');
                world
                    .ecs_mut()
                    .insert_one(
                        player,
                        nethack_babel_engine::status::SpellProtection {
                            layers: 1,
                            countdown: 10,
                            interval: 10,
                        },
                    )
                    .expect("player should accept spell protection");
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(priest, nethack_babel_engine::world::Peaceful)
                    .expect("priest should accept peaceful marker");

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-temple-selfless-generosity",
                    &world,
                    [59u8; 32],
                );
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
            SaveStoryTraversalScenario::TempleWrath => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, wizard_identity())
                    .expect("player should accept wizard identity");
                if let Some(mut hp) =
                    world.get_component_mut::<nethack_babel_engine::world::HitPoints>(player)
                {
                    hp.current = 40;
                    hp.max = 40;
                }
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
                world
                    .ecs_mut()
                    .insert_one(priest, nethack_babel_engine::world::Peaceful)
                    .expect("priest should accept peaceful marker");
                if let Some(mut mp) =
                    world.get_component_mut::<nethack_babel_engine::world::MovementPoints>(priest)
                {
                    mp.0 = 0;
                }

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
            SaveStoryTraversalScenario::OracleConsultation => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                world.set_game_content(nethack_babel_engine::rumors::GameContent {
                    oracles: vec!["The first consultation.".to_string()],
                    ..nethack_babel_engine::rumors::GameContent::default()
                });
                let oracle = spawn_full_monster(&mut world, Position::new(6, 5), "oracle", 12);
                world
                    .ecs_mut()
                    .insert_one(oracle, nethack_babel_engine::world::Peaceful)
                    .expect("oracle should accept peaceful marker");
                spawn_inventory_gold(&mut world, 200, '$');

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-oracle-consultation", &world, [67u8; 32]);
                loaded.set_game_content(world.game_content().clone());
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(
                    &mut loaded,
                    PlayerAction::ConsultOracle {
                        direction: Direction::East,
                        major: true,
                    },
                    &mut rng,
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::UntendedTempleGhost => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                spawn_full_monster(&mut world, Position::new(7, 5), "ghost", 12);

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-untended-temple-ghost", &world, [62u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = resolve_turn(&mut loaded, PlayerAction::Rest, &mut rng);
                (loaded, events)
            }
            SaveStoryTraversalScenario::SanctumRevisit => {
                let mut world = make_stair_world(DungeonBranch::Gehennom, 19, Terrain::StairsDown);
                let mut rng = Pcg64::seed_from_u64(7106);

                let first_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(first_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "sanctum-infidel"
                )));
                assert!(first_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "sanctum-be-gone"
                )));
                assert_eq!(count_monsters_named(&world, "high priest"), 1);

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-sanctum-revisit", &world, [52u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);

                let sanctum_up = find_terrain(&loaded.dungeon().current_level, Terrain::StairsUp)
                    .expect("Sanctum should preserve stairs up after save/load");
                set_player_position(&mut loaded, sanctum_up);
                let ascend_events = resolve_turn(&mut loaded, PlayerAction::GoUp, &mut rng);
                assert!(
                    ascend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should leave Sanctum after save/load",
                    scenario.label()
                );

                let gehennom_down =
                    find_terrain(&loaded.dungeon().current_level, Terrain::StairsDown)
                        .expect("Gehennom 19 should preserve stairs down to Sanctum");
                set_player_position(&mut loaded, gehennom_down);
                let revisit_events = resolve_turn(&mut loaded, PlayerAction::GoDown, &mut rng);
                (loaded, revisit_events)
            }
            SaveStoryTraversalScenario::WizardHarassment => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
                let wizard =
                    spawn_full_monster(&mut world, Position::new(14, 14), "Wizard of Yendor", 20);
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
                    hp.current = 20;
                    hp.max = 20;
                }
                let sword = spawn_inventory_object_by_name(&mut world, "long sword", 'b');
                world
                    .ecs_mut()
                    .insert_one(
                        sword,
                        BucStatus {
                            cursed: false,
                            blessed: false,
                            bknown: false,
                        },
                    )
                    .expect("inventory item should accept BUC state");
                if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
                    player_events.invoked = true;
                }

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-wizard-harassment", &world, [42u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let mut final_events = Vec::new();
                for _ in 0..256 {
                    let events = resolve_turn(&mut loaded, PlayerAction::Rest, &mut rng);
                    if events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. }
                                if key == "wizard-curse-items"
                                    || key == "wizard-summon-nasties"
                                    || key == "wizard-double-trouble"
                        )
                    }) {
                        final_events = events;
                        break;
                    }
                }
                (loaded, final_events)
            }
            SaveStoryTraversalScenario::WizardTaunt => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                let wizard =
                    spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 20);
                world
                    .ecs_mut()
                    .insert_one(wizard, nethack_babel_engine::world::Peaceful)
                    .expect("wizard should accept Peaceful in wizard taunt save scenario");
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
                    hp.current = 12;
                    hp.max = 20;
                }
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current = 4;
                    hp.max = 20;
                }
                if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
                    player_events.invoked = true;
                }

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-wizard-taunt", &world, [62u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let mut final_events = Vec::new();
                for _ in 0..256 {
                    let events = resolve_turn(&mut loaded, PlayerAction::Rest, &mut rng);
                    if events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. }
                                if key == "wizard-taunt-laughs"
                                    || key == "wizard-taunt-relinquish"
                                    || key == "wizard-taunt-panic"
                                    || key == "wizard-taunt-return"
                                    || key == "wizard-taunt-general"
                        )
                    }) {
                        final_events = events;
                        break;
                    }
                }
                (loaded, final_events)
            }
            SaveStoryTraversalScenario::WizardIntervention => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "goblin", 6);
                let _ = nethack_babel_engine::status::make_sleeping(&mut world, sleeper, 10);
                let sword = spawn_inventory_object_by_name(&mut world, "long sword", 'b');
                world
                    .ecs_mut()
                    .insert_one(
                        sword,
                        BucStatus {
                            cursed: false,
                            blessed: false,
                            bknown: false,
                        },
                    )
                    .expect("inventory item should accept BUC state");
                let current_turn = world.turn();
                if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
                    player_events.killed_wizard = true;
                    player_events.wizard_times_killed = 1;
                    player_events.wizard_last_killed_turn = current_turn;
                    player_events.wizard_intervention_cooldown = 1;
                }

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-wizard-intervention", &world, [61u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let mut final_events = Vec::new();
                for _ in 0..40 {
                    let events = resolve_turn(&mut loaded, PlayerAction::Rest, &mut rng);
                    if events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. }
                                if key == "wizard-vague-nervous"
                                    || key == "wizard-black-glow"
                                    || key == "wizard-aggravate"
                                    || key == "wizard-summon-nasties"
                                    || key == "wizard-respawned"
                        )
                    }) {
                        final_events = events;
                        break;
                    }
                }
                (loaded, final_events)
            }
            SaveStoryTraversalScenario::WizardAmuletWake => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
                let wizard =
                    spawn_full_monster(&mut world, Position::new(14, 14), "Wizard of Yendor", 20);
                let _ = nethack_babel_engine::status::make_sleeping(&mut world, wizard, 10_000);

                let (mut loaded, _loaded_rng) =
                    save_and_reload_world("story-matrix-wizard-amulet-wake", &world, [63u8; 32]);
                let mut rng = Pcg64::seed_from_u64(42);
                let mut final_events = Vec::new();
                for _ in 0..4096 {
                    let events =
                        nethack_babel_engine::turn::force_amulet_wake_check(&mut loaded, &mut rng);
                    let restored = find_monster_named(&loaded, "Wizard of Yendor")
                        .expect("wizard amulet wake matrix should keep a live Wizard");
                    if !nethack_babel_engine::status::is_sleeping(&loaded, restored) {
                        final_events = events;
                        break;
                    }
                }
                (loaded, final_events)
            }
            SaveStoryTraversalScenario::WizardBlackGlowBlind => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                let sword = spawn_inventory_object_by_name(&mut world, "long sword", 'b');
                world
                    .ecs_mut()
                    .insert_one(
                        sword,
                        BucStatus {
                            cursed: false,
                            blessed: false,
                            bknown: false,
                        },
                    )
                    .expect("inventory item should accept BUC state");
                let _ = nethack_babel_engine::status::make_blinded(&mut world, player, 20);

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-wizard-black-glow-blind",
                    &world,
                    [65u8; 32],
                );
                let mut rng = Pcg64::from_seed(loaded_rng);
                let events = nethack_babel_engine::turn::force_wizard_harassment_action(
                    &mut loaded,
                    None,
                    player,
                    nethack_babel_engine::npc::WizardAction::BlackGlowCurse,
                    &mut rng,
                );
                assert!(
                    loaded
                        .get_component::<BucStatus>(sword)
                        .is_some_and(|status| status.cursed),
                    "deterministic black-glow harness should still curse the tracked item"
                );
                (loaded, events)
            }
            SaveStoryTraversalScenario::WizardCovetousGroundAmulet => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                let player_pos = world
                    .get_component::<Positioned>(player)
                    .expect("player should keep a position")
                    .0;
                let amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
                if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
                    inv.remove(amulet);
                }
                let ground_level = (world.dungeon().branch, world.dungeon().depth);
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(amulet) {
                    *loc = nethack_babel_engine::dungeon::floor_object_location(
                        ground_level.0,
                        ground_level.1,
                        player_pos,
                    );
                }
                let _wizard =
                    spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 20);

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-wizard-covetous-ground-amulet",
                    &world,
                    [66u8; 32],
                );
                let mut rng = Pcg64::from_seed(loaded_rng);
                let loaded_player = loaded.player();
                let wizard = find_monster_named(&loaded, "Wizard of Yendor")
                    .expect("wizard ground-covetous save matrix should keep a live Wizard");
                let action = nethack_babel_engine::npc::choose_wizard_action(
                    &loaded, wizard, true, false, false, false, &mut rng,
                );
                let final_events = nethack_babel_engine::turn::force_wizard_harassment_action(
                    &mut loaded,
                    Some(wizard),
                    loaded_player,
                    action,
                    &mut rng,
                );
                (loaded, final_events)
            }
            SaveStoryTraversalScenario::WizardCovetousMonsterTool => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let player = world.player();
                let carrier = spawn_full_monster(&mut world, Position::new(12, 12), "goblin", 6);
                let book = spawn_inventory_object_by_name(&mut world, "Book of the Dead", 'a');
                if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
                    inv.remove(book);
                }
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(book) {
                    *loc = ObjectLocation::MonsterInventory {
                        carrier_id: carrier.to_bits().get() as u32,
                    };
                }
                let _wizard =
                    spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 20);

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-wizard-covetous-monster-tool",
                    &world,
                    [67u8; 32],
                );
                let mut rng = Pcg64::from_seed(loaded_rng);
                let loaded_player = loaded.player();
                let wizard = find_monster_named(&loaded, "Wizard of Yendor")
                    .expect("wizard monster-covetous save matrix should keep a live Wizard");
                let action = nethack_babel_engine::npc::choose_wizard_action(
                    &loaded, wizard, false, true, false, false, &mut rng,
                );
                let final_events = nethack_babel_engine::turn::force_wizard_harassment_action(
                    &mut loaded,
                    Some(wizard),
                    loaded_player,
                    action,
                    &mut rng,
                );
                (loaded, final_events)
            }
            SaveStoryTraversalScenario::WizardCovetousQuestArtifact => {
                let mut world = make_stair_world(DungeonBranch::Quest, 6, Terrain::StairsDown);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, wizard_identity())
                    .expect("player should accept wizard identity");
                let mut rng = Pcg64::seed_from_u64(7013);
                let stairs_down = find_terrain(&world.dungeon().current_level, Terrain::StairsDown)
                    .expect("wizard covetous save matrix should preserve quest stairs down");
                set_player_position(&mut world, stairs_down);
                let descend_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    descend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "wizard covetous save matrix should descend into the quest goal"
                );
                spawn_inventory_object_by_name(&mut world, "Book of the Dead", 'b');
                let eye = find_artifact_by_name("The Eye of the Aethiopica")
                    .expect("wizard covetous save matrix should use the real quest artifact");
                let eye = world
                    .ecs()
                    .query::<&ObjectCore>()
                    .iter()
                    .find_map(|(entity, core)| (core.artifact == Some(eye.id)).then_some(entity))
                    .expect("wizard covetous save matrix should find the quest artifact entity");
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(eye) {
                    *loc = ObjectLocation::Inventory;
                }
                if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
                    inv.items.push(eye);
                }
                let player_pos = world
                    .get_component::<Positioned>(player)
                    .expect("wizard covetous save matrix player should keep a position")
                    .0;
                let _wizard = spawn_full_monster(
                    &mut world,
                    Position::new(player_pos.x + 1, player_pos.y),
                    "Wizard of Yendor",
                    20,
                );
                if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
                    player_events.invoked = true;
                }

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-wizard-covetous-quest-artifact",
                    &world,
                    [64u8; 32],
                );
                let mut rng = Pcg64::from_seed(loaded_rng);
                let loaded_player = loaded.player();
                let wizard = find_monster_named(&loaded, "Wizard of Yendor")
                    .expect("wizard covetous save matrix should keep a live Wizard");
                let action = nethack_babel_engine::npc::choose_wizard_action(
                    &loaded, wizard, false, true, true, true, &mut rng,
                );
                let final_events = nethack_babel_engine::turn::force_wizard_harassment_action(
                    &mut loaded,
                    Some(wizard),
                    loaded_player,
                    action,
                    &mut rng,
                );
                (loaded, final_events)
            }
            SaveStoryTraversalScenario::WizardRetreatHeal => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                for y in 3..=21 {
                    for x in 3..=21 {
                        world
                            .dungeon_mut()
                            .current_level
                            .set_terrain(Position::new(x, y), Terrain::Floor);
                    }
                }
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(3, 3), Terrain::StairsUp);
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(21, 20), Terrain::StairsDown);
                let player = world.player();
                set_player_position(&mut world, Position::new(20, 20));
                let wizard =
                    spawn_full_monster(&mut world, Position::new(7, 7), "Wizard of Yendor", 20);
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
                    hp.current = 12;
                    hp.max = 20;
                }
                if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
                    player_events.invoked = true;
                }
                let mut pre_save_rng = Pcg64::seed_from_u64(7017);
                let _teleport_events = nethack_babel_engine::turn::force_wizard_retreat_and_heal(
                    &mut world,
                    wizard,
                    player,
                    &mut pre_save_rng,
                );
                let expected_retreat_pos = world
                    .get_component::<Positioned>(wizard)
                    .expect("wizard retreat save matrix should keep pre-save wizard position")
                    .0;

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-wizard-retreat-heal", &world, [73u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let loaded_player = loaded.player();
                let loaded_wizard = find_monster_named(&loaded, "Wizard of Yendor")
                    .expect("wizard retreat save matrix should keep a live Wizard");
                let loaded_pos = loaded
                    .get_component::<Positioned>(loaded_wizard)
                    .expect("wizard retreat save matrix should restore wizard position")
                    .0;
                assert_eq!(
                    loaded_pos, expected_retreat_pos,
                    "save/load wizard retreat matrix should preserve the directional retreat target"
                );
                let final_events = nethack_babel_engine::turn::force_wizard_retreat_and_heal(
                    &mut loaded,
                    loaded_wizard,
                    loaded_player,
                    &mut rng,
                );
                (loaded, final_events)
            }
            SaveStoryTraversalScenario::HumanoidAlohaChat => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let tourist = spawn_full_monster(&mut world, Position::new(6, 5), "tourist", 10);
                world
                    .ecs_mut()
                    .insert_one(tourist, nethack_babel_engine::world::Peaceful)
                    .expect("tourist should accept peaceful marker");

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-humanoid-aloha-chat", &world, [67u8; 32]);
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
            SaveStoryTraversalScenario::HobbitComplaintChat => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let hobbit = spawn_full_monster(&mut world, Position::new(6, 5), "hobbit", 10);
                world
                    .ecs_mut()
                    .insert_one(hobbit, nethack_babel_engine::world::Peaceful)
                    .expect("hobbit should accept peaceful marker");
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(hobbit) {
                    hp.current = (hp.max - 1).max(1);
                }

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-hobbit-complaint-chat", &world, [68u8; 32]);
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
            SaveStoryTraversalScenario::VampireKindredChat => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                install_test_catalogs(&mut world);
                while world.turn() < 313 {
                    world.advance_turn();
                }
                let player = world.player();
                let vampire_form_id = world
                    .monster_catalog()
                    .iter()
                    .find(|monster| monster.names.male == "vampire")
                    .map(|monster| monster.id)
                    .expect("vampire should resolve");
                let player_attributes = *world
                    .get_component::<nethack_babel_engine::world::Attributes>(player)
                    .expect("player should have attributes");
                let player_hp = *world
                    .get_component::<HitPoints>(player)
                    .expect("player should have hp");
                let player_speed = world
                    .get_component::<Speed>(player)
                    .map(|speed| speed.0)
                    .unwrap_or(12);
                world
                    .ecs_mut()
                    .insert_one(
                        player,
                        nethack_babel_engine::polyself::OriginalForm {
                            attributes: player_attributes,
                            hp: player_hp,
                            speed: player_speed,
                            display_symbol: '@',
                            display_color: nethack_babel_data::Color::White,
                            monster_id: vampire_form_id,
                        },
                    )
                    .expect("player should accept original form");
                world
                    .ecs_mut()
                    .insert_one(player, nethack_babel_engine::polyself::PolymorphTimer(500))
                    .expect("player should accept polymorph timer");
                world
                    .ecs_mut()
                    .insert_one(
                        player,
                        nethack_babel_engine::polyself::PolymorphState {
                            original_hp: 12,
                            original_max_hp: 12,
                            original_level: 1,
                            monster_form_id: vampire_form_id,
                            timer: 500,
                        },
                    )
                    .expect("player should accept polymorph state");
                let vampire = spawn_full_monster(&mut world, Position::new(6, 5), "vampire", 10);
                let current_turn = world.turn();
                make_tame_pet_state(&mut world, vampire, 10, current_turn.saturating_add(100));

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-vampire-kindred-chat", &world, [170u8; 32]);
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
            SaveStoryTraversalScenario::VampireNightChat => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                while world.turn() < 313 {
                    world.advance_turn();
                }
                let vampire = spawn_full_monster(&mut world, Position::new(6, 5), "vampire", 10);
                let current_turn = world.turn();
                make_tame_pet_state(&mut world, vampire, 10, current_turn.saturating_sub(300));

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-vampire-night-chat", &world, [171u8; 32]);
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
            SaveStoryTraversalScenario::VampireMidnightChat => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                while world.turn() < 312 {
                    world.advance_turn();
                }
                let vampire = spawn_full_monster(&mut world, Position::new(6, 5), "vampire", 10);
                let current_turn = world.turn();
                make_tame_pet_state(&mut world, vampire, 10, current_turn.saturating_sub(300));

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-vampire-midnight-chat",
                    &world,
                    [172u8; 32],
                );
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
            SaveStoryTraversalScenario::WereFullMoonChat => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                let were_name =
                    monster_name_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
                let were = spawn_full_monster(&mut world, Position::new(6, 5), &were_name, 10);
                let were_id =
                    monster_id_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
                world
                    .ecs_mut()
                    .insert_one(were, MonsterIdentity(were_id))
                    .expect("were full moon save scenario should accept explicit monster identity");
                let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "kobold", 8);
                let _ = nethack_babel_engine::status::make_sleeping(&mut world, sleeper, 20);

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-were-full-moon-chat", &world, [66u8; 32]);
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
            SaveStoryTraversalScenario::WereDaytimeMoonChat => {
                let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                for _ in 0..8 {
                    world.advance_turn();
                }
                let were_name =
                    monster_name_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
                let were = spawn_full_monster(&mut world, Position::new(6, 5), &were_name, 10);
                let were_id =
                    monster_id_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
                world
                    .ecs_mut()
                    .insert_one(were, MonsterIdentity(were_id))
                    .expect("daytime were save scenario should accept explicit monster identity");
                let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "kobold", 8);
                let _ = nethack_babel_engine::status::make_sleeping(&mut world, sleeper, 20);

                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-were-daytime-moon-chat",
                    &world,
                    [167u8; 32],
                );
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
            SaveStoryTraversalScenario::ChatPreconditionBlocks => {
                let mut silent_world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                install_test_catalogs(&mut silent_world);
                let player = silent_world.player();
                let giant_ant = silent_world
                    .monster_catalog()
                    .iter()
                    .find(|monster| monster.names.male == "giant ant")
                    .map(|monster| monster.id)
                    .expect("giant ant should resolve");
                silent_world
                    .ecs_mut()
                    .insert_one(
                        player,
                        nethack_babel_engine::polyself::PolymorphState {
                            original_hp: 12,
                            original_max_hp: 12,
                            original_level: 1,
                            monster_form_id: giant_ant,
                            timer: 20,
                        },
                    )
                    .expect("player should accept polymorph state");
                let (mut silent_loaded, silent_rng) = save_and_reload_world(
                    "story-matrix-chat-silent-polyform",
                    &silent_world,
                    [180u8; 32],
                );
                let mut silent_rng = Pcg64::from_seed(silent_rng);
                let silent_events = resolve_turn(
                    &mut silent_loaded,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut silent_rng,
                );
                assert!(silent_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "chat-cannot-speak"
                )));

                let mut strangled_world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
                if let Some(mut status) = strangled_world
                    .get_component_mut::<nethack_babel_engine::status::StatusEffects>(
                        strangled_world.player(),
                    )
                {
                    status.strangled = 3;
                }
                let (mut strangled_loaded, strangled_rng) = save_and_reload_world(
                    "story-matrix-chat-strangled",
                    &strangled_world,
                    [181u8; 32],
                );
                let mut strangled_rng = Pcg64::from_seed(strangled_rng);
                let strangled_events = resolve_turn(
                    &mut strangled_loaded,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut strangled_rng,
                );
                assert!(strangled_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "chat-strangled"
                )));

                let underwater_world = make_stair_world(DungeonBranch::Endgame, 4, Terrain::Floor);
                let (mut loaded, loaded_rng) = save_and_reload_world(
                    "story-matrix-chat-underwater",
                    &underwater_world,
                    [182u8; 32],
                );
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
            SaveStoryTraversalScenario::WizardLevelTeleport => {
                let mut world = make_stair_world(DungeonBranch::Main, 10, Terrain::Floor);
                let player = world.player();
                let wizard =
                    spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 20);
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
                    hp.current = 20;
                    hp.max = 20;
                }
                if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
                    player_events.invoked = true;
                }

                let (mut loaded, loaded_rng) =
                    save_and_reload_world("story-matrix-wizard-level-teleport", &world, [60u8; 32]);
                let mut rng = Pcg64::from_seed(loaded_rng);
                let mut final_events = Vec::new();
                for _ in 0..256 {
                    let events = resolve_turn(&mut loaded, PlayerAction::Rest, &mut rng);
                    if events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-level-teleport"
                        )
                    }) {
                        final_events = events;
                        break;
                    }
                }
                (loaded, final_events)
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

    #[test]
    fn round_trip_spell_protection() {
        let dir = test_dir("spell_protection");
        let path = dir.join("spell_protection.nbsv");

        let mut world = make_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(
                player,
                SpellProtection {
                    layers: 2,
                    countdown: 17,
                    interval: 10,
                },
            )
            .expect("player should accept spell protection");

        save_game(&world, &path, SaveReason::Checkpoint, [0u8; 32]).unwrap();
        let (loaded, _, _, _) = load_game(&path).unwrap();

        let protection = loaded
            .get_component::<SpellProtection>(loaded.player())
            .expect("spell protection should survive round-trip");
        assert_eq!(protection.layers, 2);
        assert_eq!(protection.countdown, 17);
        assert_eq!(protection.interval, 10);

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
        spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'b');
        let wizard = spawn_full_monster(&mut world, Position::new(14, 14), "Wizard of Yendor", 20);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
            hp.current = 20;
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
        let mut saw_double_trouble = false;
        let mut saw_level_teleport = false;
        let mut saw_remote_theft = false;

        for _ in 0..256 {
            let events = resolve_turn(&mut loaded, PlayerAction::Rest, &mut rng);
            let turn_saw_theft = events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "wizard-steal-amulet"
                )
            });
            let turn_saw_curse = events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "wizard-curse-items"
                )
            });
            let turn_saw_summon = events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "wizard-summon-nasties"
                )
            });
            let turn_saw_double_trouble = events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "wizard-double-trouble"
                )
            });
            let turn_saw_level_teleport = events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "wizard-level-teleport"
                )
            });

            saw_remote_theft |= turn_saw_theft;
            saw_curse |= turn_saw_curse;
            saw_summon |= turn_saw_summon;
            saw_double_trouble |= turn_saw_double_trouble;
            saw_level_teleport |= turn_saw_level_teleport;
            saw_harassment |= turn_saw_theft
                || turn_saw_curse
                || turn_saw_summon
                || turn_saw_double_trouble
                || turn_saw_level_teleport;

            if turn_saw_level_teleport {
                assert!(
                    events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "wizard level teleport after reload should really move the player"
                );
            }

            if saw_remote_theft
                && (saw_curse || saw_summon || saw_double_trouble || saw_level_teleport)
            {
                break;
            }
        }

        assert!(
            saw_harassment,
            "a live wizard should eventually keep harassing the player after save/load"
        );
        assert!(
            saw_remote_theft,
            "a distant live wizard should eventually covetously steal the Amulet after save/load"
        );
        assert!(
            saw_curse || saw_summon || saw_double_trouble || saw_level_teleport,
            "a live wizard should keep applying non-theft harassment after the covetous steal"
        );
        assert!(
            resolve_object_type_by_spec(&loaded, "Amulet of Yendor").is_some_and(|amulet_type| {
                loaded
                    .get_component::<Inventory>(loaded.player())
                    .is_some_and(|inv| {
                        !inv.items.iter().any(|item| {
                            loaded
                                .get_component::<ObjectCore>(*item)
                                .is_some_and(|core| core.otyp == amulet_type)
                                && loaded
                                    .get_component::<ObjectLocation>(*item)
                                    .is_some_and(|loc| matches!(*loc, ObjectLocation::Inventory))
                        })
                    })
            }),
            "save/load should let the wizard remove the Amulet from the player"
        );
        let wizard = find_monster_named(&loaded, "Wizard of Yendor")
            .expect("save/load harassment test should keep a live wizard");
        assert!(
            monster_carries_named_item(&loaded, wizard, "Amulet of Yendor"),
            "save/load should keep the stolen Amulet in the Wizard's inventory"
        );
        if saw_curse {
            assert!(
                resolve_object_type_by_spec(&loaded, "long sword").is_some_and(|sword_type| {
                    loaded
                        .get_component::<Inventory>(loaded.player())
                        .is_some_and(|inv| {
                            inv.items.iter().any(|item| {
                                loaded
                                    .get_component::<ObjectCore>(*item)
                                    .is_some_and(|core| core.otyp == sword_type)
                                    && loaded
                                        .get_component::<BucStatus>(*item)
                                        .is_some_and(|status| status.cursed)
                            })
                        })
                }),
                "curse harassment after reload should mutate live inventory state"
            );
        }
        if saw_summon {
            assert!(
                count_monsters_named(&loaded, "Wizard of Yendor") >= 1,
                "summon harassment after reload should keep the wizard present"
            );
        }
        if saw_level_teleport {
            assert_ne!(
                loaded.dungeon().depth,
                1,
                "wizard level teleport after reload should change the current depth"
            );
        }
    }

    #[test]
    fn round_trip_loaded_non_wizard_covetous_monster_keeps_ground_target_priority() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(3, 3), Terrain::StairsDown);

        let arch_lich = spawn_full_monster(&mut world, Position::new(14, 14), "arch-lich", 30);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(arch_lich) {
            hp.current = 20;
            hp.max = 30;
        }
        let arch_lich_flags = world
            .monster_catalog()
            .iter()
            .find(|def| def.names.male.eq_ignore_ascii_case("arch-lich"))
            .map(|def| def.flags)
            .expect("arch-lich should exist in the monster catalog");
        world
            .ecs_mut()
            .insert_one(
                arch_lich,
                nethack_babel_engine::monster_ai::MonsterSpeciesFlags(arch_lich_flags),
            )
            .expect("arch-lich should accept species flags");

        let book_def = test_game_data()
            .objects
            .iter()
            .find(|def| def.name.eq_ignore_ascii_case("Book of the Dead"))
            .expect("Book of the Dead should exist in the object catalog");
        let _book = world.spawn((
            ObjectCore {
                otyp: book_def.id,
                object_class: book_def.class,
                quantity: 1,
                weight: book_def.weight as u32,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            ObjectLocation::Floor {
                x: 6,
                y: 8,
                level: world.dungeon().current_data_dungeon_level(),
            },
            Name("Book of the Dead".to_string()),
        ));

        let (mut loaded, loaded_rng) =
            save_and_reload_world("covetous-ground-target-round-trip", &world, [91u8; 32]);
        let arch_lich = find_monster_named(&loaded, "arch-lich")
            .expect("round-trip covetous test should keep the arch-lich");
        let mut rng = Pcg64::from_seed(loaded_rng);
        let events = nethack_babel_engine::monster_ai::resolve_monster_turn(
            &mut loaded,
            arch_lich,
            &mut rng,
        );

        assert!(
            events
                .iter()
                .any(|event| matches!(event, EngineEvent::ItemPickedUp { .. })),
            "after save/load, a non-Wizard covetous monster should still prioritize the ground target"
        );
        assert!(
            monster_carries_named_item(&loaded, arch_lich, "Book of the Dead"),
            "after save/load, the covetous monster should keep the stolen ground target in inventory"
        );
        assert_eq!(
            loaded
                .get_component::<Positioned>(arch_lich)
                .map(|pos| pos.0),
            Some(Position::new(6, 8)),
            "after save/load, the covetous monster should still teleport onto the ground target tile"
        );
    }

    #[test]
    fn round_trip_loaded_non_wizard_covetous_monster_keeps_directional_retreat_choice() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(2, 2), Terrain::StairsUp);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(8, 8), Terrain::StairsDown);

        let arch_lich = spawn_full_monster(&mut world, Position::new(7, 7), "arch-lich", 20);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(arch_lich) {
            hp.current = 10;
            hp.max = 30;
        }
        let arch_lich_flags = world
            .monster_catalog()
            .iter()
            .find(|def| def.names.male.eq_ignore_ascii_case("arch-lich"))
            .map(|def| def.flags)
            .expect("arch-lich should exist in the monster catalog");
        world
            .ecs_mut()
            .insert_one(
                arch_lich,
                nethack_babel_engine::monster_ai::MonsterSpeciesFlags(arch_lich_flags),
            )
            .expect("arch-lich should accept species flags");

        let (mut loaded, loaded_rng) =
            save_and_reload_world("covetous-retreat-direction-round-trip", &world, [92u8; 32]);
        let arch_lich = find_monster_named(&loaded, "arch-lich")
            .expect("round-trip covetous retreat test should keep the arch-lich");
        let expected = if arch_lich.to_bits().get().is_multiple_of(2) {
            Position::new(8, 8)
        } else {
            Position::new(2, 2)
        };
        let mut rng = Pcg64::from_seed(loaded_rng);
        let events = nethack_babel_engine::monster_ai::resolve_monster_turn(
            &mut loaded,
            arch_lich,
            &mut rng,
        );

        assert!(
            events.iter().any(|event| matches!(
                event,
                EngineEvent::EntityTeleported { to, .. } if *to == expected
            )),
            "after save/load, a non-Wizard covetous monster should keep the original directional retreat stair choice"
        );
        assert_eq!(
            loaded
                .get_component::<Positioned>(arch_lich)
                .map(|pos| pos.0),
            Some(expected),
            "after save/load, the covetous monster should still land on the preferred retreat stair"
        );
    }

    #[test]
    fn round_trip_loaded_shop_damage_repairs_when_keeper_is_home() {
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
        let damaged_pos = Position::new(5, 5);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(damaged_pos, Terrain::Floor);
        nethack_babel_engine::shop::record_shop_damage(
            &mut world.dungeon_mut().shop_rooms[0],
            damaged_pos,
            nethack_babel_engine::shop::ShopDamageType::DoorBroken,
        );
        sync_current_level_npc_state(&mut world);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("shop-repair-round-trip", &world, [31u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);
        let events = resolve_turn(&mut loaded, PlayerAction::Rest, &mut rng);

        assert!(
            loaded.dungeon().shop_rooms[0].damage_list.is_empty(),
            "shop damage queue should continue repairing after save/load"
        );
        assert_eq!(
            loaded
                .dungeon()
                .current_level
                .get(damaged_pos)
                .map(|cell| cell.terrain),
            Some(Terrain::DoorClosed)
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-repair"
        )));
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
    fn round_trip_loaded_preserves_sleeping_monsters_on_current_level() {
        let mut world = make_stair_world(DungeonBranch::Main, 3, Terrain::Floor);
        let sleeper = world.spawn((
            Monster,
            Positioned(Position::new(6, 5)),
            Name("Goblin".to_string()),
            HitPoints { current: 8, max: 8 },
            Speed(12),
            MovementPoints(12),
        ));
        let _ = nethack_babel_engine::status::make_sleeping(&mut world, sleeper, 7);

        let (loaded, _loaded_rng) =
            save_and_reload_world("sleeping-current-level", &world, [23u8; 32]);
        let restored = loaded
            .ecs()
            .query::<(&Monster, &Name)>()
            .iter()
            .find_map(|(entity, (_monster, name))| {
                name.0.eq_ignore_ascii_case("Goblin").then_some(entity)
            })
            .expect("sleeping goblin should survive round-trip");

        assert!(
            nethack_babel_engine::status::is_sleeping(&loaded, restored),
            "live sleeping monsters should keep their sleep timer across save/load"
        );
    }

    #[test]
    fn round_trip_loaded_amulet_wakes_sleeping_wizard() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
        let wizard = spawn_full_monster(&mut world, Position::new(14, 14), "Wizard of Yendor", 20);
        let _ = nethack_babel_engine::status::make_sleeping(&mut world, wizard, 10_000);

        let (mut loaded, _loaded_rng) =
            save_and_reload_world("wizard-amulet-wake-round-trip", &world, [65u8; 32]);
        let restored = find_monster_named(&loaded, "Wizard of Yendor")
            .expect("sleeping Wizard of Yendor should survive round-trip");
        assert!(
            nethack_babel_engine::status::is_sleeping(&loaded, restored),
            "round-trip should preserve the sleeping Wizard before the Amulet wake check"
        );
        assert!(
            resolve_object_type_by_spec(&loaded, "Amulet of Yendor").is_some_and(|amulet_type| {
                loaded
                    .get_component::<Inventory>(loaded.player())
                    .is_some_and(|inv| {
                        inv.items.iter().any(|item| {
                            loaded
                                .get_component::<ObjectCore>(*item)
                                .is_some_and(|core| core.otyp == amulet_type)
                        })
                    })
            }),
            "round-trip should preserve the Amulet in the player's live inventory"
        );
        let amulet_name =
            resolve_object_type_by_spec(&loaded, "Amulet of Yendor").and_then(|amulet_type| {
                loaded
                    .get_component::<Inventory>(loaded.player())
                    .and_then(|inv| {
                        inv.items.iter().find_map(|item| {
                            loaded
                                .get_component::<ObjectCore>(*item)
                                .is_some_and(|core| core.otyp == amulet_type)
                                .then(|| {
                                    nethack_babel_engine::turn::force_item_display_name(
                                        &loaded, *item,
                                    )
                                })
                                .flatten()
                        })
                    })
            });
        assert_eq!(
            amulet_name.as_deref(),
            Some("Amulet of Yendor"),
            "round-trip should preserve the display name used by wizard amulet wake logic"
        );
        assert!(
            nethack_babel_engine::turn::force_player_has_named_item(
                &loaded,
                loaded.player(),
                "Amulet of Yendor",
            ),
            "round-trip should preserve carried Amulet lookup for wizard wake logic"
        );
        assert_eq!(
            nethack_babel_engine::turn::force_live_wizard_count(&loaded),
            1,
            "round-trip should preserve one live Wizard for the amulet wake check"
        );

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events = nethack_babel_engine::turn::force_amulet_wake_check(&mut loaded, &mut rng);
            if !nethack_babel_engine::status::is_sleeping(&loaded, restored) {
                assert!(
                    events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "wizard-vague-nervous"
                    )),
                    "waking a distant Wizard with the carried Amulet should emit the nervous warning"
                );
                return;
            }
        }

        panic!("carried Amulet should eventually wake the sleeping Wizard after save/load");
    }

    #[test]
    fn round_trip_loaded_vault_ambient_preserves_runtime_conditions() {
        let mut world = make_stair_world(DungeonBranch::Main, 5, Terrain::Floor);
        world
            .dungeon_mut()
            .vault_rooms
            .push(nethack_babel_engine::vault::VaultRoom {
                top_left: Position::new(6, 5),
                bottom_right: Position::new(7, 6),
            });
        spawn_floor_coin(&mut world, Position::new(6, 5));
        set_player_position(&mut world, Position::new(2, 2));

        let (loaded, _loaded_rng) =
            save_and_reload_world("vault-ambient-round-trip", &world, [66u8; 32]);
        assert_eq!(loaded.dungeon().vault_rooms.len(), 1);

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events =
                nethack_babel_engine::turn::force_emit_ambient_dungeon_sound(&loaded, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "ambient-vault-counting"
                )
            }) {
                return;
            }
        }

        panic!("vault counting ambience should survive save/load round-trip");
    }

    #[test]
    fn round_trip_loaded_hallucinating_vault_can_emit_scrooge() {
        let mut world = make_stair_world(DungeonBranch::Main, 5, Terrain::Floor);
        world
            .dungeon_mut()
            .vault_rooms
            .push(nethack_babel_engine::vault::VaultRoom {
                top_left: Position::new(6, 5),
                bottom_right: Position::new(7, 6),
            });
        world.dungeon_mut().vault_guard_present = true;
        if let Some(mut status) =
            world.get_component_mut::<nethack_babel_engine::status::StatusEffects>(world.player())
        {
            status.hallucination = 20;
        }
        set_player_position(&mut world, Position::new(2, 2));

        let (loaded, _loaded_rng) =
            save_and_reload_world("vault-scrooge-round-trip", &world, [74u8; 32]);

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events =
                nethack_babel_engine::turn::force_emit_ambient_dungeon_sound(&loaded, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "ambient-vault-scrooge"
                )
            }) {
                return;
            }
        }

        panic!("hallucinating vault ambience should survive save/load round-trip");
    }

    #[test]
    fn round_trip_loaded_hallucinating_gold_vault_can_emit_quarterback() {
        let mut world = make_stair_world(DungeonBranch::Main, 5, Terrain::Floor);
        world
            .dungeon_mut()
            .vault_rooms
            .push(nethack_babel_engine::vault::VaultRoom {
                top_left: Position::new(6, 5),
                bottom_right: Position::new(7, 6),
            });
        spawn_floor_coin(&mut world, Position::new(6, 5));
        if let Some(mut status) =
            world.get_component_mut::<nethack_babel_engine::status::StatusEffects>(world.player())
        {
            status.hallucination = 20;
        }
        set_player_position(&mut world, Position::new(2, 2));

        let (loaded, _loaded_rng) =
            save_and_reload_world("vault-quarterback-round-trip", &world, [75u8; 32]);

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events =
                nethack_babel_engine::turn::force_emit_ambient_dungeon_sound(&loaded, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "ambient-vault-quarterback"
                )
            }) {
                return;
            }
        }

        panic!("hallucinating gold-vault ambience should survive save/load round-trip");
    }

    #[test]
    fn round_trip_loaded_fountain_ambient_preserves_runtime_conditions() {
        let mut world = make_stair_world(DungeonBranch::Main, 5, Terrain::Floor);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Fountain);
        set_player_position(&mut world, Position::new(2, 2));

        let (loaded, _loaded_rng) =
            save_and_reload_world("fountain-ambient-round-trip", &world, [67u8; 32]);

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events =
                nethack_babel_engine::turn::force_emit_ambient_dungeon_sound(&loaded, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-fountain-")
                )
            }) {
                return;
            }
        }

        panic!("fountain ambience should survive save/load round-trip");
    }

    #[test]
    fn round_trip_loaded_court_ambient_preserves_runtime_conditions() {
        let mut world = make_stair_world(DungeonBranch::Main, 12, Terrain::Floor);
        spawn_full_monster(&mut world, Position::new(7, 7), "kobold lord", 12);
        set_player_position(&mut world, Position::new(2, 2));

        let (loaded, _loaded_rng) =
            save_and_reload_world("court-ambient-round-trip", &world, [70u8; 32]);

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events =
                nethack_babel_engine::turn::force_emit_ambient_dungeon_sound(&loaded, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-court-")
                )
            }) {
                return;
            }
        }

        panic!("court ambience should survive save/load round-trip");
    }

    #[test]
    fn round_trip_loaded_swamp_ambient_preserves_runtime_conditions() {
        let mut world = make_stair_world(DungeonBranch::Main, 18, Terrain::Floor);
        for y in 5..9 {
            for x in 5..11 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Pool);
            }
        }
        spawn_monster_with_symbol(&mut world, Position::new(7, 7), "giant eel", 12, ';', 0);
        set_player_position(&mut world, Position::new(2, 2));

        let (loaded, _loaded_rng) =
            save_and_reload_world("swamp-ambient-round-trip", &world, [68u8; 32]);

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events =
                nethack_babel_engine::turn::force_emit_ambient_dungeon_sound(&loaded, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-swamp-")
                )
            }) {
                return;
            }
        }

        panic!("swamp ambience should survive save/load round-trip");
    }

    #[test]
    fn round_trip_loaded_beehive_ambient_preserves_runtime_conditions() {
        let mut world = make_stair_world(DungeonBranch::Main, 12, Terrain::Floor);
        spawn_full_monster(&mut world, Position::new(7, 7), "killer bee", 8);
        set_player_position(&mut world, Position::new(2, 2));

        let (loaded, _loaded_rng) =
            save_and_reload_world("beehive-ambient-round-trip", &world, [71u8; 32]);

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events =
                nethack_babel_engine::turn::force_emit_ambient_dungeon_sound(&loaded, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-beehive-")
                )
            }) {
                return;
            }
        }

        panic!("beehive ambience should survive save/load round-trip");
    }

    #[test]
    fn round_trip_loaded_morgue_ambient_preserves_runtime_conditions() {
        let mut world = make_stair_world(DungeonBranch::Main, 12, Terrain::Floor);
        spawn_full_monster(&mut world, Position::new(7, 7), "ghost", 8);
        set_player_position(&mut world, Position::new(2, 2));

        let (loaded, _loaded_rng) =
            save_and_reload_world("morgue-ambient-round-trip", &world, [72u8; 32]);

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events =
                nethack_babel_engine::turn::force_emit_ambient_dungeon_sound(&loaded, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-morgue-")
                )
            }) {
                return;
            }
        }

        panic!("morgue ambience should survive save/load round-trip");
    }

    #[test]
    fn round_trip_loaded_barracks_ambient_preserves_runtime_conditions() {
        let mut world = make_stair_world(DungeonBranch::Main, 18, Terrain::Floor);
        for idx in 0..6 {
            let sleeping = if idx == 0 { 20 } else { 0 };
            spawn_monster_with_symbol(
                &mut world,
                Position::new(5 + idx, 5),
                "soldier",
                12,
                '@',
                sleeping,
            );
        }
        set_player_position(&mut world, Position::new(2, 2));

        let (loaded, _loaded_rng) =
            save_and_reload_world("barracks-ambient-round-trip", &world, [69u8; 32]);

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events =
                nethack_babel_engine::turn::force_emit_ambient_dungeon_sound(&loaded, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-barracks-")
                )
            }) {
                return;
            }
        }

        panic!("barracks ambience should survive save/load round-trip");
    }

    #[test]
    fn round_trip_loaded_hallucinating_shop_can_emit_neiman_marcus() {
        let mut world = make_stair_world(DungeonBranch::Main, 5, Terrain::Floor);
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
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
        if let Some(mut status) =
            world.get_component_mut::<nethack_babel_engine::status::StatusEffects>(world.player())
        {
            status.hallucination = 20;
        }
        set_player_position(&mut world, Position::new(2, 2));

        let (loaded, _loaded_rng) =
            save_and_reload_world("shop-neiman-round-trip", &world, [75u8; 32]);

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events =
                nethack_babel_engine::turn::force_emit_ambient_dungeon_sound(&loaded, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "ambient-shop-neiman-marcus"
                )
            }) {
                return;
            }
        }

        panic!("hallucinating shop ambience should survive save/load round-trip");
    }

    #[test]
    fn round_trip_loaded_hallucinating_shopkeeper_chat_can_emit_geico_pitch() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
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
        if let Some(mut status) =
            world.get_component_mut::<nethack_babel_engine::status::StatusEffects>(player)
        {
            status.hallucination = 200;
        }

        let (mut loaded, loaded_rng) =
            save_and_reload_world("hallu-shopkeeper-chat-round-trip", &world, [76u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        for _ in 0..64 {
            let events = resolve_turn(
                &mut loaded,
                PlayerAction::Chat {
                    direction: Direction::East,
                },
                &mut rng,
            );
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "shk-geico-pitch"
                )
            }) {
                return;
            }
        }

        panic!("hallucinating shopkeeper chat should survive save/load round-trip");
    }

    #[test]
    fn round_trip_loaded_chatting_with_laughing_monster_keeps_laughter_line() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        spawn_full_monster(&mut world, Position::new(6, 5), "leprechaun", 12);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("laughing-monster-chat-round-trip", &world, [77u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. }
                    if matches!(
                        key.as_str(),
                        "npc-laugh-giggles"
                            | "npc-laugh-chuckles"
                            | "npc-laugh-snickers"
                            | "npc-laugh-laughs"
                    )
            )
        }));
    }

    #[test]
    fn round_trip_loaded_hallucinating_chatting_with_gecko_keeps_fake_shop_pitch() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let player = world.player();
        spawn_full_monster(&mut world, Position::new(6, 5), "gecko", 8);
        if let Some(mut status) =
            world.get_component_mut::<nethack_babel_engine::status::StatusEffects>(player)
        {
            status.hallucination = 200;
        }

        let (mut loaded, loaded_rng) =
            save_and_reload_world("hallu-gecko-chat-round-trip", &world, [78u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-gecko-geico-pitch"
            )
        }));
    }

    #[test]
    fn round_trip_loaded_chatting_with_skeleton_keeps_rattle_and_paralysis() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let player = world.player();
        spawn_full_monster(&mut world, Position::new(6, 5), "skeleton", 12);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("skeleton-chat-round-trip", &world, [79u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-bones-rattle"
            )
        }));
        assert!(
            loaded
                .get_component::<nethack_babel_engine::status::StatusEffects>(player)
                .is_some_and(|status| status.paralysis > 0),
            "skeleton chat paralysis should survive the round-trip setup and apply after load"
        );
    }

    #[test]
    fn round_trip_loaded_chatting_with_shrieker_keeps_wake_effect() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        spawn_full_monster(&mut world, Position::new(6, 5), "shrieker", 8);
        let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "kobold", 8);
        let _ = nethack_babel_engine::status::make_sleeping(&mut world, sleeper, 20);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("shrieker-chat-round-trip", &world, [80u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-shriek")
        }));
        let woken = loaded
            .ecs()
            .query::<(
                &nethack_babel_engine::world::Monster,
                &nethack_babel_engine::world::Name,
            )>()
            .iter()
            .find_map(|(entity, (_, name))| (name.0 == "kobold").then_some(entity))
            .and_then(|entity| {
                loaded.get_component::<nethack_babel_engine::status::StatusEffects>(entity)
            })
            .is_none_or(|status| status.sleeping == 0);
        assert!(
            woken,
            "shrieker chat should still wake sleeping monsters after load"
        );
    }

    #[test]
    fn round_trip_loaded_chatting_with_full_moon_werewolf_keeps_howl_and_wake() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let were_name = monster_name_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
        let were = spawn_full_monster(&mut world, Position::new(6, 5), &were_name, 8);
        let were_id = monster_id_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
        world
            .ecs_mut()
            .insert_one(were, MonsterIdentity(were_id))
            .expect("were chat round-trip should accept explicit monster identity");
        let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "kobold", 8);
        let _ = nethack_babel_engine::status::make_sleeping(&mut world, sleeper, 20);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("were-chat-round-trip", &world, [84u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-were-howls")
        }));
        let woken = loaded
            .ecs()
            .query::<(
                &nethack_babel_engine::world::Monster,
                &nethack_babel_engine::world::Name,
            )>()
            .iter()
            .find_map(|(entity, (_, name))| (name.0 == "kobold").then_some(entity))
            .and_then(|entity| {
                loaded.get_component::<nethack_babel_engine::status::StatusEffects>(entity)
            })
            .is_none_or(|status| status.sleeping == 0);
        assert!(
            woken,
            "full moon were chat should still wake nearby sleeping monsters after load"
        );
    }

    #[test]
    fn round_trip_loaded_chatting_with_peaceful_buzzing_monster_keeps_drone_line() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let buzz_name = monster_name_with_sound(&world, MonsterSound::Buzz);
        let monster = spawn_full_monster(&mut world, Position::new(6, 5), &buzz_name, 8);
        world
            .ecs_mut()
            .insert_one(monster, Peaceful)
            .expect("buzzing monster should accept peaceful marker");

        let (mut loaded, loaded_rng) =
            save_and_reload_world("buzz-chat-round-trip", &world, [81u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-buzz-drones")
        }));
    }

    #[test]
    fn round_trip_loaded_chatting_with_peaceful_vampire_mentions_potions() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let vampire_name = monster_name_with_sound(&world, MonsterSound::Vampire);
        let vampire = spawn_full_monster(&mut world, Position::new(6, 5), &vampire_name, 8);
        world
            .ecs_mut()
            .insert_one(vampire, Peaceful)
            .expect("vampire should accept peaceful marker");

        let (mut loaded, loaded_rng) =
            save_and_reload_world("vampire-chat-round-trip", &world, [86u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-vampire-peaceful"
            )
        }));
    }

    #[test]
    fn round_trip_loaded_chatting_with_hungry_tame_vampire_at_night_begs_for_craving() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        while world.turn() < 313 {
            world.advance_turn();
        }
        let vampire = spawn_full_monster(&mut world, Position::new(6, 5), "vampire", 8);
        let current_turn = world.turn();
        assert!(nethack_babel_engine::were::is_night(current_turn));
        assert!(!nethack_babel_engine::were::is_midnight(current_turn));
        make_tame_pet_state(&mut world, vampire, 10, current_turn.saturating_sub(300));

        let (mut loaded, loaded_rng) =
            save_and_reload_world("tame-vampire-night-chat-round-trip", &world, [190u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);
        let restored = find_monster_named(&loaded, "vampire").expect("vampire should survive load");
        assert!(loaded.get_component::<Tame>(restored).is_some());
        assert!(
            loaded
                .get_component::<PetState>(restored)
                .is_some_and(|pet_state| pet_state.is_hungry(loaded.turn()))
        );
        assert!(nethack_babel_engine::were::is_night(loaded.turn()));
        assert!(!nethack_babel_engine::were::is_midnight(loaded.turn()));

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-vampire-tame-night-craving"
            )
        }));
    }

    #[test]
    fn round_trip_loaded_tame_vampire_kindred_chat_preserves_polymorph_state() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        install_test_catalogs(&mut world);
        while world.turn() < 313 {
            world.advance_turn();
        }
        let player = world.player();
        let vampire_form_id = world
            .monster_catalog()
            .iter()
            .find(|monster| monster.names.male == "vampire")
            .map(|monster| monster.id)
            .expect("vampire should resolve");
        let player_attributes = *world
            .get_component::<nethack_babel_engine::world::Attributes>(player)
            .expect("player should have attributes");
        let player_hp = *world
            .get_component::<HitPoints>(player)
            .expect("player should have hp");
        let player_speed = world
            .get_component::<Speed>(player)
            .map(|speed| speed.0)
            .unwrap_or(12);
        world
            .ecs_mut()
            .insert_one(
                player,
                nethack_babel_engine::polyself::OriginalForm {
                    attributes: player_attributes,
                    hp: player_hp,
                    speed: player_speed,
                    display_symbol: '@',
                    display_color: nethack_babel_data::Color::White,
                    monster_id: vampire_form_id,
                },
            )
            .expect("player should accept original form");
        world
            .ecs_mut()
            .insert_one(player, nethack_babel_engine::polyself::PolymorphTimer(500))
            .expect("player should accept polymorph timer");
        world
            .ecs_mut()
            .insert_one(
                player,
                nethack_babel_engine::polyself::PolymorphState {
                    original_hp: 12,
                    original_max_hp: 12,
                    original_level: 1,
                    monster_form_id: vampire_form_id,
                    timer: 500,
                },
            )
            .expect("player should accept polymorph state");
        let vampire = spawn_full_monster(&mut world, Position::new(6, 5), "vampire", 8);
        let current_turn = world.turn();
        make_tame_pet_state(&mut world, vampire, 10, current_turn.saturating_add(100));

        let (mut loaded, loaded_rng) =
            save_and_reload_world("tame-vampire-kindred-chat-round-trip", &world, [191u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        assert!(
            loaded
                .get_component::<nethack_babel_engine::polyself::PolymorphState>(loaded.player())
                .is_some_and(|state| {
                    loaded
                        .monster_catalog()
                        .iter()
                        .find(|monster| monster.names.male == "vampire")
                        .map(|monster| monster.id)
                        .is_some_and(|vampire_id| state.monster_form_id == vampire_id)
                })
        );

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-vampire-tame-kindred-evening"
            )
        }));
    }

    #[test]
    fn round_trip_loaded_chatting_with_trumpeting_monster_keeps_wake_effect() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let trumpet_name = monster_name_with_sound(&world, MonsterSound::Trumpet);
        spawn_full_monster(&mut world, Position::new(6, 5), &trumpet_name, 8);
        let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "kobold", 8);
        let _ = nethack_babel_engine::status::make_sleeping(&mut world, sleeper, 20);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("trumpet-chat-round-trip", &world, [87u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-trumpet-trumpets"
            )
        }));
        let woken = loaded
            .ecs()
            .query::<(
                &nethack_babel_engine::world::Monster,
                &nethack_babel_engine::world::Name,
            )>()
            .iter()
            .find_map(|(entity, (_, name))| (name.0 == "kobold").then_some(entity))
            .and_then(|entity| {
                loaded.get_component::<nethack_babel_engine::status::StatusEffects>(entity)
            })
            .is_none_or(|status| status.sleeping == 0);
        assert!(
            woken,
            "trumpet chat should still wake nearby sleeping monsters after load"
        );
    }

    #[test]
    fn round_trip_loaded_chatting_with_untamed_mooing_monster_bellows() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let moo_name = monster_name_with_sound(&world, MonsterSound::Moo);
        spawn_full_monster(&mut world, Position::new(6, 5), &moo_name, 8);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("moo-chat-round-trip", &world, [92u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-bellow-bellows"
            )
        }));
    }

    #[test]
    fn round_trip_loaded_chatting_with_death_keeps_rider_line() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        spawn_full_monster(&mut world, Position::new(6, 5), "Death", 20);

        let (mut loaded, loaded_rng) =
            save_and_reload_world("death-chat-round-trip", &world, [88u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. }
                    if key == "npc-rider-war" || key == "npc-rider-sandman"
            )
        }));
    }

    #[test]
    fn round_trip_loaded_consulting_oracle_keeps_consultation_text() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        world.set_game_content(nethack_babel_engine::rumors::GameContent {
            rumors_true: vec!["A true rumor.".to_string()],
            oracles: vec!["The first consultation.".to_string()],
            ..nethack_babel_engine::rumors::GameContent::default()
        });
        let oracle = spawn_full_monster(&mut world, Position::new(6, 5), "oracle", 12);
        world
            .ecs_mut()
            .insert_one(oracle, nethack_babel_engine::world::Peaceful)
            .expect("oracle should accept peaceful marker");
        spawn_inventory_gold(&mut world, 200, '$');

        let (mut loaded, loaded_rng) =
            save_and_reload_world("oracle-chat-round-trip", &world, [89u8; 32]);
        loaded.set_game_content(world.game_content().clone());
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::ConsultOracle {
                direction: Direction::East,
                major: false,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, args }
                if key == "oracle-consultation"
                    && args.iter().any(|(k, v)| k == "text" && v == "A true rumor.")
        )));
        assert!(
            loaded
                .get_component::<nethack_babel_data::components::PlayerEvents>(loaded.player())
                .is_some_and(|flags| flags.minor_oracle),
            "oracle consultation after load should still set the minor_oracle flag"
        );
        let loaded_gold = loaded
            .get_component::<nethack_babel_engine::inventory::Inventory>(loaded.player())
            .map(|inv| {
                inv.items
                    .iter()
                    .filter_map(|item| loaded.get_component::<ObjectCore>(*item))
                    .filter(|core| core.object_class == ObjectClass::Coin)
                    .map(|core| core.quantity.max(0))
                    .sum::<i32>()
            })
            .unwrap_or(0);
        assert_eq!(loaded_gold, 150);
    }

    #[test]
    fn round_trip_loaded_bribing_demon_buys_safe_passage() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let demon_name = monster_name_with_sound(&world, MonsterSound::Bribe);
        let demon = spawn_full_monster(&mut world, Position::new(6, 5), &demon_name, 12);
        world
            .ecs_mut()
            .insert_one(demon, nethack_babel_engine::world::Peaceful)
            .expect("demon should accept peaceful marker");
        spawn_inventory_gold(&mut world, 500, '$');

        let (mut loaded, loaded_rng) =
            save_and_reload_world("demon-bribe-round-trip", &world, [90u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::BribeDemon {
                direction: Direction::East,
                amount: 500,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "demon-vanishes-laughing"
        )));
        assert!(
            loaded.get_component::<Monster>(demon).is_none(),
            "accepted bribe should still remove the demon after save/load"
        );
        let loaded_gold = loaded
            .get_component::<nethack_babel_engine::inventory::Inventory>(loaded.player())
            .map(|inv| {
                inv.items
                    .iter()
                    .filter_map(|item| loaded.get_component::<ObjectCore>(*item))
                    .filter(|core| core.object_class == ObjectClass::Coin)
                    .map(|core| core.quantity.max(0))
                    .sum::<i32>()
            })
            .unwrap_or(0);
        assert_eq!(loaded_gold, 0);
    }

    #[test]
    fn round_trip_loaded_chatting_with_satiated_tame_cat_keeps_purr_line() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let mew_name = monster_name_with_sound(&world, MonsterSound::Mew);
        let monster = spawn_full_monster(&mut world, Position::new(6, 5), &mew_name, 8);
        let current_turn = world.turn();
        make_tame_pet_state(&mut world, monster, 10, current_turn.saturating_add(1500));

        let (mut loaded, loaded_rng) =
            save_and_reload_world("mew-chat-round-trip", &world, [82u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-mew-purrs")
        }));
    }

    #[test]
    fn round_trip_loaded_chatting_with_trapped_tame_cat_keeps_yowl_line() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let mew_name = monster_name_with_sound(&world, MonsterSound::Mew);
        let monster = spawn_full_monster(&mut world, Position::new(6, 5), &mew_name, 8);
        let current_turn = world.turn();
        make_tame_pet_state(&mut world, monster, 10, current_turn.saturating_add(1500));
        world
            .ecs_mut()
            .insert_one(
                monster,
                nethack_babel_engine::traps::Trapped {
                    kind: nethack_babel_engine::traps::TrappedIn::BearTrap,
                    turns_remaining: 5,
                },
            )
            .expect("cat should accept trapped state");

        let (mut loaded, loaded_rng) =
            save_and_reload_world("mew-trapped-chat-round-trip", &world, [84u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-mew-yowls")
        }));
    }

    #[test]
    fn round_trip_loaded_chatting_with_peaceful_dingo_stays_silent() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        while nethack_babel_engine::were::is_full_moon(world.turn()) {
            world.advance_turn();
        }
        let dingo = spawn_full_monster(&mut world, Position::new(6, 5), "dingo", 8);
        world
            .ecs_mut()
            .insert_one(dingo, nethack_babel_engine::world::Peaceful)
            .expect("dingo should accept peaceful marker");

        let (mut loaded, loaded_rng) =
            save_and_reload_world("dingo-chat-round-trip", &world, [85u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-chat-no-response")
        }));
    }

    #[test]
    fn round_trip_loaded_chatting_with_tame_content_dingo_stays_silent() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        while nethack_babel_engine::were::is_full_moon(world.turn()) {
            world.advance_turn();
        }
        let dingo = spawn_full_monster(&mut world, Position::new(6, 5), "dingo", 8);
        world
            .ecs_mut()
            .insert_one(dingo, nethack_babel_engine::world::Peaceful)
            .expect("dingo should accept peaceful marker");
        world
            .ecs_mut()
            .insert_one(dingo, Tame)
            .expect("dingo should accept tame marker");
        let current_turn = world.turn();
        world
            .ecs_mut()
            .insert_one(
                dingo,
                PetState {
                    tameness: 10,
                    apport: 10,
                    hungrytime: current_turn.saturating_add(100),
                    abuse: 0,
                    revivals: 0,
                    whistletime: 0,
                    droptime: 0,
                    dropdist: 0,
                    mhpmax_penalty: 0,
                    leashed: false,
                    last_seen_turn: current_turn,
                    killed_by_u: false,
                },
            )
            .expect("dingo should accept pet state");

        let (mut loaded, loaded_rng) =
            save_and_reload_world("tame-dingo-chat-round-trip", &world, [188u8; 32]);
        let mut rng = Pcg64::from_seed(loaded_rng);

        let events = resolve_turn(
            &mut loaded,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-chat-no-response")
        }));
    }

    #[test]
    fn round_trip_loaded_zoo_ambient_preserves_runtime_conditions() {
        let mut world = make_stair_world(DungeonBranch::Main, 12, Terrain::Floor);
        spawn_monster_with_symbol(&mut world, Position::new(7, 7), "jackal", 8, 'd', 20);
        set_player_position(&mut world, Position::new(2, 2));

        let (loaded, _loaded_rng) =
            save_and_reload_world("zoo-ambient-round-trip", &world, [73u8; 32]);

        let mut rng = Pcg64::seed_from_u64(42);
        for _ in 0..4096 {
            let events =
                nethack_babel_engine::turn::force_emit_ambient_dungeon_sound(&loaded, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-zoo-")
                )
            }) {
                return;
            }
        }

        panic!("zoo ambience should survive save/load round-trip");
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
            EngineEvent::Message { key, .. } if key == "shk-angry-rude"
        )));
    }

    #[test]
    fn round_trip_loaded_preserves_partially_paid_shop_bill_progress() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let _gold = spawn_inventory_gold(&mut world, 50, 'g');
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
            "shop bill should accept a partial-payment entry"
        );

        let mut rng = Pcg64::seed_from_u64(7011);
        let pay_events = resolve_turn(&mut world, PlayerAction::Pay, &mut rng);
        assert!(pay_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-pay-success"
        )));
        assert!(pay_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-owe"
        )));

        let (loaded, _) =
            save_and_reload_world("shop-partial-payment-round-trip", &world, [64u8; 32]);
        let loaded_player = loaded.player();
        let gold_total: i32 = loaded
            .get_component::<Inventory>(loaded_player)
            .map(|inv| {
                inv.items
                    .iter()
                    .filter_map(|item| loaded.get_component::<ObjectCore>(*item))
                    .filter(|core| core.object_class == ObjectClass::Coin)
                    .map(|core| core.quantity)
                    .sum()
            })
            .unwrap_or(0);
        let shop = &loaded.dungeon().shop_rooms[0];

        assert_eq!(gold_total, 0);
        assert_eq!(shop.bill.total(), 50);
        assert_eq!(shop.bill.entries().len(), 1);
        assert_eq!(shop.bill.entries()[0].paid_amount, 50);
        assert_eq!(shop.bill.entries()[0].original_quantity, 1);
        assert_eq!(shop.credit, 0);
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
    fn round_trip_loaded_preserves_untended_temple_ghost_on_current_level() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        spawn_full_monster(&mut world, Position::new(7, 5), "ghost", 12);

        let (loaded, _loaded_rng) =
            save_and_reload_world("untended-temple-ghost-round-trip", &world, [61u8; 32]);

        assert_eq!(
            count_monsters_named(&loaded, "ghost"),
            1,
            "untended temple ghost should survive round-trip on the current level"
        );
        assert_eq!(
            loaded
                .dungeon()
                .current_level
                .get(Position::new(6, 5))
                .map(|cell| cell.terrain),
            Some(Terrain::Altar)
        );
    }

    #[test]
    fn round_trip_loaded_preserves_temple_wrath_hp_and_blindness() {
        let mut world = make_stair_world(DungeonBranch::Main, 1, Terrain::Floor);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        if let Some(mut hp) =
            world.get_component_mut::<nethack_babel_engine::world::HitPoints>(player)
        {
            hp.current = 40;
            hp.max = 40;
        }
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 20);
        world
            .ecs_mut()
            .insert_one(priest, nethack_babel_engine::world::Peaceful)
            .expect("priest should accept peaceful marker");
        if let Some(mut mp) =
            world.get_component_mut::<nethack_babel_engine::world::MovementPoints>(priest)
        {
            mp.0 = 0;
        }

        let mut rng = Pcg64::seed_from_u64(7105);
        let attack_events = resolve_turn(
            &mut world,
            PlayerAction::FightDirection {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(attack_events.iter().any(|event| matches!(
            event,
            EngineEvent::HpChange {
                entity,
                amount,
                source: nethack_babel_engine::event::HpSource::Divine,
                ..
            } if *entity == player && *amount < 0
        )));
        assert!(
            world
                .get_component::<nethack_babel_engine::status::StatusEffects>(player)
                .is_some_and(|status| status.blindness > 0),
            "pre-save wrath should blind the player"
        );

        let (loaded, _loaded_rng) =
            save_and_reload_world("temple-wrath-runtime-round-trip", &world, [49u8; 32]);
        let loaded_player = loaded.player();
        assert!(
            loaded
                .get_component::<nethack_babel_engine::world::HitPoints>(loaded_player)
                .is_some_and(|hp| hp.current < 40),
            "divine wrath HP loss should survive load"
        );
        assert!(
            loaded
                .get_component::<nethack_babel_engine::status::StatusEffects>(loaded_player)
                .is_some_and(|status| status.blindness > 0),
            "divine wrath blindness should survive load"
        );
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
            SaveStoryTraversalScenario::MedusaRevisit,
            SaveStoryTraversalScenario::CastleRevisit,
            SaveStoryTraversalScenario::OrcusRevisit,
            SaveStoryTraversalScenario::FortLudiosRevisit,
            SaveStoryTraversalScenario::VladTopEntry,
            SaveStoryTraversalScenario::InvocationPortalRevisit,
            SaveStoryTraversalScenario::ShopEntry,
            SaveStoryTraversalScenario::ShopEntryWelcomeBack,
            SaveStoryTraversalScenario::ShopEntryRobbed,
            SaveStoryTraversalScenario::ShopkeeperFollow,
            SaveStoryTraversalScenario::ShopkeeperPayoff,
            SaveStoryTraversalScenario::ShopkeeperCredit,
            SaveStoryTraversalScenario::ShopCreditCovers,
            SaveStoryTraversalScenario::ShopPartialPayment,
            SaveStoryTraversalScenario::ShopNoMoney,
            SaveStoryTraversalScenario::ShopWandUsageFee,
            SaveStoryTraversalScenario::ShopGreaseUsageFee,
            SaveStoryTraversalScenario::ShopLampUsageFee,
            SaveStoryTraversalScenario::ShopCameraUsageFee,
            SaveStoryTraversalScenario::ShopTinningUsageFee,
            SaveStoryTraversalScenario::ShopSpellbookUsageFee,
            SaveStoryTraversalScenario::ShopkeeperSell,
            SaveStoryTraversalScenario::ShopChatPriceQuote,
            SaveStoryTraversalScenario::ShopContainerPickup,
            SaveStoryTraversalScenario::DemonBribe,
            SaveStoryTraversalScenario::ShopRepair,
            SaveStoryTraversalScenario::ShopkeeperDeath,
            SaveStoryTraversalScenario::ShopRobbery,
            SaveStoryTraversalScenario::ShopRestitution,
            SaveStoryTraversalScenario::TempleWrongAlignment,
            SaveStoryTraversalScenario::TempleAleGift,
            SaveStoryTraversalScenario::TempleVirtuesOfPoverty,
            SaveStoryTraversalScenario::TempleDonationThanks,
            SaveStoryTraversalScenario::TemplePious,
            SaveStoryTraversalScenario::TempleDonation,
            SaveStoryTraversalScenario::TempleBlessing,
            SaveStoryTraversalScenario::TempleCleansing,
            SaveStoryTraversalScenario::TempleSelflessGenerosity,
            SaveStoryTraversalScenario::TempleWrath,
            SaveStoryTraversalScenario::TempleCalm,
            SaveStoryTraversalScenario::OracleConsultation,
            SaveStoryTraversalScenario::UntendedTempleGhost,
            SaveStoryTraversalScenario::SanctumRevisit,
            SaveStoryTraversalScenario::WizardHarassment,
            SaveStoryTraversalScenario::WizardTaunt,
            SaveStoryTraversalScenario::WizardIntervention,
            SaveStoryTraversalScenario::WizardAmuletWake,
            SaveStoryTraversalScenario::WizardBlackGlowBlind,
            SaveStoryTraversalScenario::WizardCovetousGroundAmulet,
            SaveStoryTraversalScenario::WizardCovetousMonsterTool,
            SaveStoryTraversalScenario::WizardCovetousQuestArtifact,
            SaveStoryTraversalScenario::WizardRetreatHeal,
            SaveStoryTraversalScenario::HumanoidAlohaChat,
            SaveStoryTraversalScenario::HobbitComplaintChat,
            SaveStoryTraversalScenario::VampireKindredChat,
            SaveStoryTraversalScenario::VampireNightChat,
            SaveStoryTraversalScenario::VampireMidnightChat,
            SaveStoryTraversalScenario::WereFullMoonChat,
            SaveStoryTraversalScenario::WereDaytimeMoonChat,
            SaveStoryTraversalScenario::ChatPreconditionBlocks,
            SaveStoryTraversalScenario::WizardLevelTeleport,
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
                SaveStoryTraversalScenario::MedusaRevisit => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::Main);
                    assert_eq!(world.dungeon().depth, 24);
                    assert_eq!(count_monsters_named(&world, "medusa"), 1);
                }
                SaveStoryTraversalScenario::CastleRevisit => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::Main);
                    assert_eq!(world.dungeon().depth, 25);
                    assert_eq!(count_objects_named(&world, "wand of wishing"), 1);
                }
                SaveStoryTraversalScenario::OrcusRevisit => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::Gehennom);
                    assert_eq!(world.dungeon().depth, 12);
                    assert_eq!(count_monsters_named(&world, "orcus"), 1);
                }
                SaveStoryTraversalScenario::FortLudiosRevisit => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::FortLudios);
                    assert_eq!(world.dungeon().depth, 1);
                    assert_eq!(count_monsters_named(&world, "soldier"), 2);
                    assert_eq!(count_monsters_named(&world, "lieutenant"), 1);
                    assert_eq!(count_monsters_named(&world, "captain"), 1);
                }
                SaveStoryTraversalScenario::VladTopEntry => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::VladsTower);
                    assert_eq!(world.dungeon().depth, 3);
                    assert_eq!(count_monsters_named(&world, "Vlad the Impaler"), 1);
                    assert_eq!(count_objects_named(&world, "Candelabrum of Invocation"), 1);
                }
                SaveStoryTraversalScenario::InvocationPortalRevisit => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::Gehennom);
                    assert_eq!(world.dungeon().depth, 21);
                    assert!(
                        find_terrain(&world.dungeon().current_level, Terrain::MagicPortal)
                            .is_some(),
                        "invoked Gehennom revisit should reopen the endgame portal after save/load"
                    );
                }
                SaveStoryTraversalScenario::ShopEntry => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-enter"
                    )));
                }
                SaveStoryTraversalScenario::ShopEntryWelcomeBack => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-welcome-back"
                    )));
                }
                SaveStoryTraversalScenario::ShopEntryRobbed => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-stolen"
                    )));
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
                        pos.x <= 6,
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
                SaveStoryTraversalScenario::ShopkeeperCredit => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-pay-success"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-credit"
                    )));
                    assert_eq!(shop.debit, 0);
                    assert_eq!(shop.credit, 100);
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
                    assert_eq!(gold_total, 0);
                }
                SaveStoryTraversalScenario::ShopCreditCovers => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-credit-covers"
                    )));
                    assert!(!final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-pay-success" || key == "shop-owe"
                    )));
                    assert!(shop.bill.is_empty());
                    assert_eq!(shop.debit, 0);
                    assert_eq!(shop.credit, 30);
                }
                SaveStoryTraversalScenario::ShopPartialPayment => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-pay-success"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-owe"
                    )));
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
                    assert_eq!(gold_total, 0);
                    assert_eq!(shop.bill.total(), 50);
                    assert_eq!(shop.bill.entries().len(), 1);
                    assert_eq!(shop.bill.entries()[0].paid_amount, 50);
                    assert_eq!(shop.credit, 0);
                }
                SaveStoryTraversalScenario::ShopNoMoney => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-no-money"
                    )));
                    assert!(!final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-pay-success" || key == "shop-owe"
                    )));
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
                    assert_eq!(gold_total, 0);
                    assert_eq!(shop.bill.total(), 100);
                    assert_eq!(shop.bill.entries().len(), 1);
                    assert_eq!(shop.bill.entries()[0].paid_amount, 0);
                    assert_eq!(shop.credit, 0);
                }
                SaveStoryTraversalScenario::ShopWandUsageFee => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-usage-fee"
                    )));
                    assert_eq!(shop.bill.total(), 100);
                    assert_eq!(shop.debit, 25);
                }
                SaveStoryTraversalScenario::ShopGreaseUsageFee => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-usage-fee"
                    )));
                    assert_eq!(shop.bill.total(), 100);
                    assert_eq!(shop.debit, 10);
                }
                SaveStoryTraversalScenario::ShopLampUsageFee => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "tool-lamp-on"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-usage-fee"
                    )));
                    assert_eq!(shop.bill.total(), 100);
                    assert_eq!(shop.debit, 25);
                }
                SaveStoryTraversalScenario::ShopCameraUsageFee => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "tool-camera-blind"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-usage-fee"
                    )));
                    assert!(!shop.bill.is_empty());
                    assert_eq!(shop.debit, 10);
                }
                SaveStoryTraversalScenario::ShopTinningUsageFee => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "tool-tinning-success"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-usage-fee"
                    )));
                    assert_eq!(shop.bill.total(), 100);
                    assert_eq!(shop.debit, 10);
                }
                SaveStoryTraversalScenario::ShopSpellbookUsageFee => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "spell-learned"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-usage-fee"
                    )));
                    assert_eq!(shop.bill.total(), 100);
                    assert_eq!(shop.debit, 80);
                }
                SaveStoryTraversalScenario::ShopkeeperSell => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-sell"
                    )));
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
                    assert_eq!(gold_total, 5);
                    assert_eq!(shop.shopkeeper_gold, 75);
                }
                SaveStoryTraversalScenario::ShopChatPriceQuote => {
                    let quote_count = final_events
                        .iter()
                        .filter(|event| {
                            matches!(
                                event,
                                EngineEvent::Message { key, .. } if key.starts_with("shop-price")
                            )
                        })
                        .count();
                    assert_eq!(quote_count, 2);
                }
                SaveStoryTraversalScenario::ShopContainerPickup => {
                    assert!(
                        final_events
                            .iter()
                            .any(|event| matches!(event, EngineEvent::ItemPickedUp { .. }))
                    );
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key.starts_with("shop-price")
                    )));
                    let shop = &world.dungeon().shop_rooms[0];
                    assert_eq!(shop.bill.len(), 3);
                    let sack = world
                        .get_component::<Inventory>(player)
                        .and_then(|inv| {
                            inv.items.iter().copied().find(|item| {
                                world
                                    .get_component::<Name>(*item)
                                    .is_some_and(|name| name.0.as_str() == "sack")
                            })
                        })
                        .expect("picked up sack should be in inventory");
                    assert_eq!(
                        nethack_babel_engine::environment::container_contents(&world, sack).len(),
                        2
                    );
                    assert!(
                        world
                            .get_component::<ShopState>(sack)
                            .is_some_and(|state| state.unpaid),
                        "picked up sack should remain marked unpaid after save/load"
                    );
                }
                SaveStoryTraversalScenario::DemonBribe => {
                    let demon_name = monster_name_and_id_matching(&world, |def| {
                        def.sound == MonsterSound::Bribe
                            && def
                                .flags
                                .contains(nethack_babel_data::schema::MonsterFlags::DEMON)
                    })
                    .0;
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "demon-demand-safe-passage"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "demon-vanishes-laughing"
                    )));
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
                    assert_eq!(gold_total, 0);
                    assert_eq!(count_monsters_named(&world, &demon_name), 0);
                }
                SaveStoryTraversalScenario::ShopRepair => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-repair"
                    )));
                    assert!(shop.damage_list.is_empty());
                    assert_eq!(
                        world
                            .dungeon()
                            .current_level
                            .get(Position::new(5, 5))
                            .map(|cell| cell.terrain),
                        Some(Terrain::DoorClosed)
                    );
                }
                SaveStoryTraversalScenario::ShopkeeperDeath => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(
                        !final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "shop-shoplift"
                        )),
                        "deserted shops should not keep charging robbery after save/load"
                    );
                    assert!(shop.bill.is_empty());
                    assert_eq!(shop.debit, 0);
                    assert_eq!(shop.credit, 0);
                    assert!(!shop.angry);
                    assert!(!shop.surcharge);
                    assert!(
                        world.get_component::<Shopkeeper>(shop.shopkeeper).is_none(),
                        "dead shopkeepers should not regain explicit runtime state after save/load"
                    );
                }
                SaveStoryTraversalScenario::ShopRobbery => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-shoplift"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-stolen-amount"
                    )));
                    assert_eq!(shop.robbed, 100);
                    assert!(shop.bill.is_empty());
                    assert!(shop.angry);
                }
                SaveStoryTraversalScenario::ShopRestitution => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-restock"
                    )));
                    assert_eq!(shop.robbed, 0);
                    assert!(!shop.angry);
                    assert!(!shop.surcharge);
                }
                SaveStoryTraversalScenario::TempleWrongAlignment => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-wrong-alignment"
                    )));
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
                    assert_eq!(gold_total, 500);
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::status::SpellProtection>(player)
                            .is_none()
                    );
                }
                SaveStoryTraversalScenario::TempleAleGift => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-ale-gift"
                    )));
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
                    assert_eq!(gold_total, 2);
                }
                SaveStoryTraversalScenario::TempleVirtuesOfPoverty => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-virtues-of-poverty"
                    )));
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
                    assert_eq!(gold_total, 0);
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::status::SpellProtection>(player)
                            .is_none()
                    );
                }
                SaveStoryTraversalScenario::TempleDonationThanks => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-small-thanks"
                    )));
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
                    assert_eq!(gold_total, 0);
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::status::SpellProtection>(player)
                            .is_none()
                    );
                }
                SaveStoryTraversalScenario::TemplePious => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-pious"
                    )));
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
                    assert_eq!(gold_total, 0);
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::status::SpellProtection>(player)
                            .is_none()
                    );
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::status::StatusEffects>(player)
                            .is_none_or(|status| status.clairvoyance == 0)
                    );
                }
                SaveStoryTraversalScenario::TempleDonation => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-protection-granted"
                    )));
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
                    assert_eq!(gold_total, 600);
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::status::SpellProtection>(player)
                            .is_some_and(|protection| protection.layers == 1)
                    );
                }
                SaveStoryTraversalScenario::TempleSelflessGenerosity => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. }
                            if key == "priest-selfless-generosity"
                    )));
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
                    assert_eq!(gold_total, 0);
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::status::SpellProtection>(player)
                            .is_some_and(|protection| protection.layers == 1)
                    );
                }
                SaveStoryTraversalScenario::TempleBlessing => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-clairvoyance"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. }
                            if key == "clairvoyance-reveal" || key == "clairvoyance-nothing-new"
                    )));
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::status::StatusEffects>(player)
                            .is_some_and(|status| status.clairvoyance > 0)
                    );
                }
                SaveStoryTraversalScenario::TempleCleansing => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-cleansing"
                    )));
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
                    assert_eq!(gold_total, 0);
                    assert!(
                        world
                            .get_component::<ReligionState>(player)
                            .is_some_and(|state| state.alignment_record >= 0)
                    );
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::status::SpellProtection>(player)
                            .is_some_and(|protection| protection.layers == 1)
                    );
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
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. }
                            if matches!(
                                key.as_str(),
                                "priest-cranky-1" | "priest-cranky-2" | "priest-cranky-3"
                            )
                    )));
                    assert!(
                        !final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "priest-protection-granted"
                        )),
                        "hostile priest should not grant protection after save/load"
                    );
                    let player = world.player();
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::world::HitPoints>(player)
                            .is_some_and(|hp| hp.current < 40),
                        "divine wrath HP loss should survive save/load"
                    );
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::status::StatusEffects>(player)
                            .is_some_and(|status| status.blindness > 0),
                        "divine wrath blindness should survive save/load"
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
                SaveStoryTraversalScenario::OracleConsultation => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, args }
                            if key == "oracle-consultation"
                                && args
                                    .iter()
                                    .any(|(arg_key, value)| arg_key == "text"
                                        && value == "The first consultation.")
                    )));
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|flags| flags.major_oracle)
                    );
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
                    assert_eq!(gold_total, 0);
                }
                SaveStoryTraversalScenario::UntendedTempleGhost => {
                    assert_eq!(
                        count_monsters_named(&world, "ghost"),
                        1,
                        "untended temple ghost should survive the save/load matrix"
                    );
                    assert_eq!(
                        world
                            .dungeon()
                            .current_level
                            .get(Position::new(6, 5))
                            .map(|cell| cell.terrain),
                        Some(Terrain::Altar)
                    );
                }
                SaveStoryTraversalScenario::SanctumRevisit => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::Gehennom);
                    assert_eq!(world.dungeon().depth, 20);
                    assert_eq!(count_monsters_named(&world, "high priest"), 1);
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "sanctum-desecrate"
                    )));
                    assert!(
                        !final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. }
                                if key == "sanctum-infidel" || key == "sanctum-be-gone"
                        )),
                        "Sanctum revisit after save/load should not replay first-entry messaging"
                    );
                }
                SaveStoryTraversalScenario::WizardHarassment => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. }
                            if key == "wizard-curse-items"
                                || key == "wizard-summon-nasties"
                                || key == "wizard-double-trouble"
                    )));
                    let cursed = world
                        .get_component::<Inventory>(player)
                        .map(|inv| {
                            inv.items.iter().any(|item| {
                                world
                                    .get_component::<BucStatus>(*item)
                                    .is_some_and(|status| status.cursed)
                            })
                        })
                        .unwrap_or(false);
                    let summoned = final_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. }));
                    assert!(cursed || summoned);
                    assert!(
                        resolve_object_type_by_spec(&world, "Amulet of Yendor").is_some_and(
                            |amulet_type| world.get_component::<Inventory>(player).is_some_and(
                                |inv| !inv.items.iter().any(|item| {
                                    world
                                        .get_component::<ObjectCore>(*item)
                                        .is_some_and(|core| core.otyp == amulet_type)
                                        && world.get_component::<ObjectLocation>(*item).is_some_and(
                                            |loc| matches!(*loc, ObjectLocation::Inventory),
                                        )
                                })
                            )
                        ),
                        "save/load wizard matrix should let a covetous wizard remove the Amulet from the player"
                    );
                    let wizard = find_monster_named(&world, "Wizard of Yendor")
                        .expect("save/load wizard matrix should keep a live Wizard");
                    assert!(
                        monster_carries_named_item(&world, wizard, "Amulet of Yendor"),
                        "save/load wizard matrix should keep the stolen Amulet in the Wizard inventory"
                    );
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.invoked),
                        "save/load wizard matrix should preserve the invoked harassment trigger"
                    );
                }
                SaveStoryTraversalScenario::WizardTaunt => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. }
                            if key == "wizard-taunt-laughs"
                                || key == "wizard-taunt-relinquish"
                                || key == "wizard-taunt-panic"
                                || key == "wizard-taunt-return"
                                || key == "wizard-taunt-general"
                    )));
                    assert_eq!(count_monsters_named(&world, "Wizard of Yendor"), 1);
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.invoked),
                        "save/load wizard matrix should preserve the live taunt trigger"
                    );
                }
                SaveStoryTraversalScenario::WizardIntervention => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. }
                            if key == "wizard-vague-nervous"
                                || key == "wizard-black-glow"
                                || key == "wizard-aggravate"
                                || key == "wizard-summon-nasties"
                                || key == "wizard-respawned"
                    )));
                    let cursed = world
                        .get_component::<Inventory>(player)
                        .map(|inv| {
                            inv.items.iter().any(|item| {
                                world
                                    .get_component::<BucStatus>(*item)
                                    .is_some_and(|status| status.cursed)
                            })
                        })
                        .unwrap_or(false);
                    let summoned = final_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. }));
                    let black_glow = final_events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-black-glow"
                        )
                    });
                    let summon_msg = final_events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-summon-nasties"
                        )
                    });
                    let aggravate = final_events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-aggravate"
                        )
                    });
                    let respawned = final_events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-respawned"
                        )
                    });
                    if black_glow {
                        assert!(cursed);
                    }
                    if summon_msg {
                        assert!(summoned);
                    }
                    if aggravate {
                        let sleeper = world
                            .ecs()
                            .query::<(&Monster, &Name)>()
                            .iter()
                            .find_map(|(entity, (_, name))| (name.0 == "goblin").then_some(entity))
                            .expect(
                                "wizard save/load intervention should keep the sleeping goblin",
                            );
                        assert!(!nethack_babel_engine::status::is_sleeping(&world, sleeper));
                    }
                    if respawned {
                        assert_eq!(count_monsters_named(&world, "Wizard of Yendor"), 1);
                        assert!(
                            final_events
                                .iter()
                                .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. }))
                        );
                    } else {
                        assert_eq!(count_monsters_named(&world, "Wizard of Yendor"), 0);
                    }
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.killed_wizard),
                        "save/load wizard matrix should preserve the off-screen intervention trigger"
                    );
                }
                SaveStoryTraversalScenario::WizardAmuletWake => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "wizard-vague-nervous"
                    )));
                    let restored = find_monster_named(&world, "Wizard of Yendor")
                        .expect("wizard amulet wake matrix should keep a live Wizard");
                    assert!(
                        !nethack_babel_engine::status::is_sleeping(&world, restored),
                        "save/load wizard matrix should wake the sleeping Wizard when the player carries the Amulet"
                    );
                }
                SaveStoryTraversalScenario::WizardBlackGlowBlind => {
                    assert!(
                        !final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-black-glow"
                        )),
                        "blind hero should not see the black glow after save/load"
                    );
                    let cursed = world
                        .get_component::<Inventory>(player)
                        .map(|inv| {
                            inv.items.iter().any(|item| {
                                world
                                    .get_component::<BucStatus>(*item)
                                    .is_some_and(|status| status.cursed)
                            })
                        })
                        .unwrap_or(false);
                    assert!(
                        cursed,
                        "blind hero should still suffer the black-glow curse after save/load"
                    );
                    assert!(
                        world
                            .get_component::<StatusEffects>(player)
                            .is_some_and(|status| status.blindness > 0),
                        "black-glow blind scenario should preserve blindness through round-trip"
                    );
                }
                SaveStoryTraversalScenario::WizardCovetousGroundAmulet => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "wizard-steal-amulet"
                    )));
                    assert!(
                        !nethack_babel_engine::turn::force_player_has_named_item(
                            &world,
                            player,
                            "Amulet of Yendor",
                        ),
                        "save/load ground-covetous matrix should keep the Amulet off the player"
                    );
                    let wizard = find_monster_named(&world, "Wizard of Yendor")
                        .expect("wizard ground-covetous save matrix should keep a live Wizard");
                    assert!(
                        monster_carries_named_item(&world, wizard, "Amulet of Yendor"),
                        "save/load ground-covetous matrix should keep the Amulet in the Wizard inventory"
                    );
                    let wizard_pos = world
                        .get_component::<Positioned>(wizard)
                        .expect(
                            "wizard should keep a position after save/load ground-covetous replay",
                        )
                        .0;
                    let player_pos = world
                        .get_component::<Positioned>(player)
                        .expect(
                            "player should keep a position after save/load ground-covetous replay",
                        )
                        .0;
                    assert_eq!(
                        nethack_babel_engine::ball::chebyshev_distance(wizard_pos, player_pos),
                        1,
                        "save/load ground-covetous matrix should leave the Wizard adjacent to the player after stealing from the hero tile"
                    );
                }
                SaveStoryTraversalScenario::WizardCovetousMonsterTool => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. }
                            if key == "wizard-steal-invocation-tool"
                    )));
                    let wizard = find_monster_named(&world, "Wizard of Yendor")
                        .expect("wizard monster-covetous save matrix should keep a live Wizard");
                    let carrier = find_monster_named(&world, "goblin")
                        .expect("monster-covetous save matrix should keep the original carrier");
                    assert!(
                        monster_carries_named_item(&world, wizard, "Book of the Dead"),
                        "save/load monster-covetous matrix should keep the invocation tool in the Wizard inventory"
                    );
                    assert!(
                        !monster_carries_named_item(&world, carrier, "Book of the Dead"),
                        "save/load monster-covetous matrix should strip the original carrier inventory"
                    );
                }
                SaveStoryTraversalScenario::WizardCovetousQuestArtifact => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "wizard-steal-quest-artifact"
                    )));
                    let player_keeps_book = resolve_object_type_by_spec(&world, "Book of the Dead")
                        .is_some_and(|book_type| {
                            world.get_component::<Inventory>(player).is_some_and(|inv| {
                                inv.items.iter().any(|item| {
                                    world
                                        .get_component::<ObjectCore>(*item)
                                        .is_some_and(|core| core.otyp == book_type)
                                        && world.get_component::<ObjectLocation>(*item).is_some_and(
                                            |loc| matches!(*loc, ObjectLocation::Inventory),
                                        )
                                })
                            })
                        });
                    assert!(
                        player_keeps_book,
                        "save/load covetous matrix should leave lower-priority invocation tools on the player"
                    );
                    let eye_artifact = find_artifact_by_name("The Eye of the Aethiopica")
                        .expect("wizard covetous save matrix should know the quest artifact id");
                    let player_keeps_eye =
                        world.get_component::<Inventory>(player).is_some_and(|inv| {
                            inv.items.iter().any(|item| {
                                world
                                    .get_component::<ObjectCore>(*item)
                                    .is_some_and(|core| core.artifact == Some(eye_artifact.id))
                                    && world.get_component::<ObjectLocation>(*item).is_some_and(
                                        |loc| matches!(*loc, ObjectLocation::Inventory),
                                    )
                            })
                        });
                    assert!(
                        !player_keeps_eye,
                        "save/load covetous matrix should remove the quest artifact from the player"
                    );
                    let wizard = find_monster_named(&world, "Wizard of Yendor")
                        .expect("wizard covetous save matrix should keep a live Wizard");
                    assert!(
                        monster_carries_named_item(&world, wizard, "The Eye of the Aethiopica"),
                        "save/load covetous matrix should keep the quest artifact in the Wizard inventory"
                    );
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.invoked),
                        "save/load covetous matrix should preserve the invoked priority trigger"
                    );
                }
                SaveStoryTraversalScenario::WizardRetreatHeal => {
                    let wizard = find_monster_named(&world, "Wizard of Yendor")
                        .expect("wizard retreat save matrix should keep a live Wizard");
                    assert!(
                        final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::HpChange {
                                entity,
                                amount,
                                source: HpSource::Regeneration,
                                ..
                            } if *entity == wizard && *amount > 0
                        )),
                        "save/load wizard retreat matrix should really heal the wounded Wizard"
                    );
                    let wizard_pos = world
                        .get_component::<Positioned>(wizard)
                        .expect("wizard retreat save matrix should keep wizard position")
                        .0;
                    assert!(
                        wizard_pos == Position::new(3, 3) || wizard_pos == Position::new(21, 20),
                        "save/load wizard retreat matrix should leave the Wizard on one of the retreat stairs"
                    );
                    let hp = world
                        .get_component::<HitPoints>(wizard)
                        .expect("wizard retreat save matrix should keep wizard HP");
                    assert!(hp.current > 12);
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.invoked),
                        "save/load wizard retreat matrix should preserve the invoked trigger"
                    );
                }
                SaveStoryTraversalScenario::VampireNightChat => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "npc-vampire-tame-night-craving"
                    )));
                }
                SaveStoryTraversalScenario::VampireKindredChat => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "npc-vampire-tame-kindred-evening"
                    )));
                    assert!(
                        world
                            .get_component::<nethack_babel_engine::polyself::PolymorphState>(player)
                            .is_some(),
                        "vampire kindred round-trip should preserve polymorph state"
                    );
                }
                SaveStoryTraversalScenario::VampireMidnightChat => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "npc-vampire-tame-craving"
                    )));
                }
                SaveStoryTraversalScenario::WereFullMoonChat => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "npc-were-howls"
                    )));
                    let sleeper = world
                        .ecs()
                        .query::<(
                            &nethack_babel_engine::world::Monster,
                            &nethack_babel_engine::world::Name,
                        )>()
                        .iter()
                        .find_map(|(entity, (_, name))| (name.0 == "kobold").then_some(entity))
                        .expect("were full moon save matrix should keep the sleeping kobold");
                    assert!(
                        !nethack_babel_engine::status::is_sleeping(&world, sleeper),
                        "full moon were chat should still wake nearby sleeping monsters after save/load"
                    );
                }
                SaveStoryTraversalScenario::WereDaytimeMoonChat => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "npc-were-moon"
                    )));
                    let sleeper = world
                        .ecs()
                        .query::<(
                            &nethack_babel_engine::world::Monster,
                            &nethack_babel_engine::world::Name,
                        )>()
                        .iter()
                        .find_map(|(entity, (_, name))| (name.0 == "kobold").then_some(entity))
                        .expect("daytime were save matrix should keep the sleeping kobold");
                    assert!(
                        nethack_babel_engine::status::is_sleeping(&world, sleeper),
                        "daytime full moon were chat should keep nearby sleepers asleep after save/load"
                    );
                }
                SaveStoryTraversalScenario::ChatPreconditionBlocks => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "chat-underwater"
                    )));
                    assert_eq!(world.dungeon().branch, DungeonBranch::Endgame);
                    assert_eq!(world.dungeon().depth, 4);
                }
                SaveStoryTraversalScenario::HumanoidAlohaChat => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "npc-humanoid-aloha"
                    )));
                }
                SaveStoryTraversalScenario::HobbitComplaintChat => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "npc-humanoid-hobbit-complains"
                    )));
                }
                SaveStoryTraversalScenario::WizardLevelTeleport => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "wizard-level-teleport"
                    )));
                    assert!(
                        final_events
                            .iter()
                            .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                    );
                    assert_eq!(world.dungeon().branch, DungeonBranch::Main);
                    assert_ne!(world.dungeon().depth, 10);
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.invoked),
                        "save/load wizard matrix should preserve the invoked teleport trigger"
                    );
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
