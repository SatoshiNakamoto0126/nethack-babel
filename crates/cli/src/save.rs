use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nethack_babel_data::components::{
    BucStatus, Enchantment, Erosion, KnowledgeState, ObjectCore, ObjectLocation,
};
use nethack_babel_data::{PlayerIdentity, PlayerSkills};
use nethack_babel_engine::action::Position;
use nethack_babel_engine::attributes::{AttributeExercise, NaturalAttributes};
use nethack_babel_engine::conduct::ConductState;
use nethack_babel_engine::dungeon::DungeonState;
use nethack_babel_engine::equipment::EquipmentSlots;
use nethack_babel_engine::inventory::Inventory;
use nethack_babel_engine::spells::SpellBook;
use nethack_babel_engine::status::{Intrinsics, StatusEffects};
use nethack_babel_engine::world::Attributes as EngineAttributes;
use nethack_babel_engine::world::{
    ArmorClass, Encumbrance, EncumbranceLevel, ExperienceLevel, GameWorld, HeroSpeed,
    HeroSpeedBonus, HitPoints, Monster, MovementPoints, Name, Nutrition, PlayerCombat, Positioned,
    Power, Speed, Tame,
};

// =========================================================================
// Save format version
// =========================================================================

/// Current save format version.  Bump minor for backward-compatible changes,
/// major for breaking changes.
const SAVE_VERSION: [u8; 3] = [0, 3, 1];

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
    pub is_tame: bool,
    pub creation_order: u64,
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
        let creation_order = world
            .get_component::<nethack_babel_engine::world::CreationOrder>(entity)
            .map(|c| c.0)
            .unwrap_or(0);

        monsters.push(SerializableMonster {
            position,
            hp_current,
            hp_max,
            speed,
            movement_points,
            name,
            is_tame,
            creation_order,
        });
    }
    monsters
}

/// Rebuild a `GameWorld` from deserialized `SaveData`.
fn rebuild_world(data: &SaveData) -> GameWorld {
    let p = &data.player;
    let mut world = GameWorld::new(p.position);

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

    // Set the turn counter.
    while world.turn() < data.turn {
        world.advance_turn();
    }

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
    }

    world
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
pub fn load_game(path: &Path) -> anyhow::Result<(GameWorld, u32, i32, [u8; 32])> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;

    // Read and validate magic bytes.
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if magic != SAVE_MAGIC {
        anyhow::bail!(
            "Invalid save file: expected magic {:?}, got {:?}",
            SAVE_MAGIC,
            magic
        );
    }

    // Read and validate version.
    let mut version = [0u8; 3];
    file.read_exact(&mut version)?;
    if !is_compatible_version(version) {
        anyhow::bail!(
            "Save file version mismatch: file has {}.{}.{}, \
             binary expects {}.{}.{}",
            version[0],
            version[1],
            version[2],
            SAVE_VERSION[0],
            SAVE_VERSION[1],
            SAVE_VERSION[2],
        );
    }

    // Read payload length.
    let mut len_buf = [0u8; 8];
    file.read_exact(&mut len_buf)?;
    let payload_len = u64::from_le_bytes(len_buf) as usize;

    // Sanity check payload size (max 256 MB).
    if payload_len > 256 * 1024 * 1024 {
        anyhow::bail!(
            "Save file payload too large: {} bytes (max 256 MB)",
            payload_len
        );
    }

    // Read payload.
    let mut payload = vec![0u8; payload_len];
    file.read_exact(&mut payload)?;

    // Deserialize.
    let (save_data, _): (SaveData, _) =
        bincode::serde::decode_from_slice(&payload, bincode::config::standard())?;

    // Validate embedded header magic (defense in depth).
    if save_data.header.magic != SAVE_MAGIC {
        anyhow::bail!("Corrupt save: embedded header has wrong magic bytes");
    }

    // Rebuild the world.
    let world = rebuild_world(&save_data);
    let turn = save_data.turn;
    let depth = save_data.dungeon.depth;
    let rng_state = save_data.rng_state;

    // Delete save file after successful load (anti-savescumming).
    drop(file);
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
pub fn try_recover(player_name: &str) -> anyhow::Result<Option<(GameWorld, u32, i32, [u8; 32])>> {
    let panic_path = save_file_path(player_name).with_extension("panic.nbsv");

    if !panic_path.exists() {
        return Ok(None);
    }

    // Load without deleting (we'll rename instead).
    use std::io::Read;

    let mut file = std::fs::File::open(&panic_path)?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if magic != SAVE_MAGIC {
        anyhow::bail!("Panic save file has invalid magic bytes");
    }

    let mut version = [0u8; 3];
    file.read_exact(&mut version)?;
    if !is_compatible_version(version) {
        anyhow::bail!(
            "Panic save version mismatch: {}.{}.{}",
            version[0],
            version[1],
            version[2]
        );
    }

    let mut len_buf = [0u8; 8];
    file.read_exact(&mut len_buf)?;
    let payload_len = u64::from_le_bytes(len_buf) as usize;

    if payload_len > 256 * 1024 * 1024 {
        anyhow::bail!("Panic save payload too large");
    }

    let mut payload = vec![0u8; payload_len];
    file.read_exact(&mut payload)?;

    let (save_data, _): (SaveData, _) =
        bincode::serde::decode_from_slice(&payload, bincode::config::standard())?;

    if save_data.header.magic != SAVE_MAGIC {
        anyhow::bail!("Corrupt panic save: embedded header has wrong magic");
    }

    let world = rebuild_world(&save_data);
    let turn = save_data.turn;
    let depth = save_data.dungeon.depth;
    let rng_state = save_data.rng_state;

    // Rename to .recovered.nbsv instead of deleting.
    drop(file);
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
        Alignment, GameData, Gender, Handedness, RaceId, loader::load_game_data,
    };
    use nethack_babel_engine::{
        action::PlayerAction,
        artifacts::find_artifact_by_name,
        dungeon::{DungeonBranch, LevelMap, Terrain},
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
        let (mut loaded, _turn, _depth, loaded_rng) = load_game(&path).unwrap();
        install_test_catalogs(&mut loaded);
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
    fn version_compatibility_older_minor() {
        // Older minor version (same major) should be compatible.
        let older = [SAVE_VERSION[0], SAVE_VERSION[1].saturating_sub(1), 0];
        assert!(is_compatible_version(older));
    }

    #[test]
    fn version_compatibility_different_major() {
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
}
