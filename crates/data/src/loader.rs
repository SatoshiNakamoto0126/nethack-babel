//! TOML data file loader.
//!
//! Loads static game data definitions (monsters, objects) from TOML files
//! on disk into the schema structs defined in [`crate::schema`].
//!
//! The TOML files use human-readable string enums and flat structures.
//! This module provides intermediate deserialization types that match
//! the TOML format, then converts them into the canonical schema types.

use std::path::Path;

use arrayvec::ArrayVec;
use serde::Deserialize;

use crate::schema::{
    ArmorCategory, ArmorInfo, AttackDef, AttackMethod, Color, DamageType, DiceExpr, GenoFlags,
    Material, MonsterDef, MonsterFlags, MonsterNames, MonsterSize, MonsterSound, MonsterId,
    ObjectClass, ObjectDef, ObjectTypeId, Property, ResistanceSet, SpellDirection, SpellbookInfo,
    StrikeMode, WeaponInfo, WeaponSkill,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// All loaded game data.
#[derive(Debug, Clone)]
pub struct GameData {
    /// All monster definitions, ordered by id.
    pub monsters: Vec<MonsterDef>,
    /// All object definitions, ordered by id.
    pub objects: Vec<ObjectDef>,
}

/// Error type for data loading operations.
#[derive(Debug)]
pub enum LoadError {
    /// The file could not be read from disk.
    Io(std::io::Error),
    /// The file contents could not be parsed as valid TOML.
    Parse(toml::de::Error),
    /// A value could not be converted to the expected schema type.
    Convert(String),
    /// A required file or directory was not found.
    NotFound(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "I/O error loading data file: {e}"),
            LoadError::Parse(e) => write!(f, "TOML parse error: {e}"),
            LoadError::Convert(msg) => write!(f, "data conversion error: {msg}"),
            LoadError::NotFound(path) => write!(f, "data path not found: {path}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::Io(e) => Some(e),
            LoadError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        LoadError::Io(e)
    }
}

impl From<toml::de::Error> for LoadError {
    fn from(e: toml::de::Error) -> Self {
        LoadError::Parse(e)
    }
}

/// Load all game data from the given data directory.
///
/// Expects the directory to contain:
/// - `items/weapons.toml`, `items/armor.toml`, etc.
/// - `monsters/base.toml`
///
/// # Errors
///
/// Returns [`LoadError`] if any file cannot be read or parsed, or if
/// TOML values cannot be converted to the expected schema types.
pub fn load_game_data(data_dir: &Path) -> Result<GameData, LoadError> {
    let items_dir = data_dir.join("items");
    let monsters_dir = data_dir.join("monsters");

    if !items_dir.is_dir() {
        return Err(LoadError::NotFound(items_dir.display().to_string()));
    }
    if !monsters_dir.is_dir() {
        return Err(LoadError::NotFound(monsters_dir.display().to_string()));
    }

    // Load all objects from item category files
    let item_files = [
        "weapons.toml",
        "armor.toml",
        "rings.toml",
        "amulets.toml",
        "tools.toml",
        "food.toml",
        "potions.toml",
        "scrolls.toml",
        "wands.toml",
        "spellbooks.toml",
        "gems.toml",
    ];

    let mut objects = Vec::new();
    for filename in &item_files {
        let path = items_dir.join(filename);
        if path.exists() {
            let loaded = load_objects(&path)?;
            objects.extend(loaded);
        }
    }

    // Load monsters
    let monsters_path = monsters_dir.join("base.toml");
    let monsters = load_monsters(&monsters_path)?;

    Ok(GameData { monsters, objects })
}

/// Load monster definitions from a TOML file.
///
/// The file is expected to contain a top-level `[[monster]]` array where each
/// element matches the intermediate [`TomlMonster`] schema.
///
/// # Errors
///
/// Returns [`LoadError::Io`] if the file cannot be read, [`LoadError::Parse`]
/// if the TOML content is malformed, or [`LoadError::Convert`] if values
/// cannot be mapped to schema enums.
pub fn load_monsters(path: &Path) -> Result<Vec<MonsterDef>, LoadError> {
    let contents = std::fs::read_to_string(path)?;
    let file: TomlMonsterFile = toml::from_str(&contents)?;
    file.monster
        .into_iter()
        .map(convert_monster)
        .collect()
}

/// Load object definitions from a TOML file.
///
/// The file is expected to contain a top-level `[[object]]` array where each
/// element matches the intermediate [`TomlObject`] schema.
///
/// # Errors
///
/// Returns [`LoadError::Io`] if the file cannot be read, [`LoadError::Parse`]
/// if the TOML content is malformed, or [`LoadError::Convert`] if values
/// cannot be mapped to schema enums.
pub fn load_objects(path: &Path) -> Result<Vec<ObjectDef>, LoadError> {
    let contents = std::fs::read_to_string(path)?;
    let file: TomlObjectFile = toml::from_str(&contents)?;
    file.object
        .into_iter()
        .map(convert_object)
        .collect()
}

// ---------------------------------------------------------------------------
// Intermediate TOML types — Monsters
// ---------------------------------------------------------------------------

/// Top-level wrapper for a monster TOML file.
#[derive(Deserialize)]
struct TomlMonsterFile {
    monster: Vec<TomlMonster>,
}

/// Intermediate monster definition matching the TOML format.
#[derive(Deserialize)]
struct TomlMonster {
    id: u16,
    name: String,
    #[serde(default)]
    name_female: Option<String>,
    symbol: String,
    color: String,
    base_level: i8,
    speed: i8,
    armor_class: i8,
    magic_resistance: u8,
    alignment: i8,
    difficulty: u8,
    #[serde(default)]
    attacks: Vec<TomlAttack>,
    #[serde(default)]
    generation_flags: Vec<String>,
    #[serde(default)]
    frequency: u8,
    corpse_weight: u16,
    corpse_nutrition: u16,
    sound: String,
    size: String,
    #[serde(default)]
    resistances: Vec<String>,
    #[serde(default)]
    conveyed: Vec<String>,
    #[serde(default)]
    flags: Vec<String>,
}

/// Intermediate attack definition matching the TOML format.
#[derive(Deserialize)]
struct TomlAttack {
    #[serde(rename = "type")]
    attack_type: String,
    damage_type: String,
    dice: String,
}

// ---------------------------------------------------------------------------
// Intermediate TOML types — Objects
// ---------------------------------------------------------------------------

/// Top-level wrapper for an object TOML file.
#[derive(Deserialize)]
struct TomlObjectFile {
    object: Vec<TomlObject>,
}

/// Intermediate object definition matching the TOML format.
/// Uses a flat structure with all possible fields as `Option`.
#[derive(Deserialize)]
struct TomlObject {
    id: u16,
    name: String,
    class: String,
    #[serde(default)]
    sub: Option<String>,
    color: String,
    material: Option<String>,
    weight: u16,
    cost: i16,
    #[serde(default)]
    prob: u16,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    is_magic: bool,
    #[serde(default)]
    is_mergeable: bool,
    #[serde(default)]
    is_charged: bool,
    #[serde(default)]
    is_unique: bool,
    #[serde(default)]
    is_nowish: bool,
    #[serde(default)]
    is_bulky: bool,
    #[serde(default)]
    two_handed: bool,
    #[serde(default)]
    is_tough: bool,
    #[serde(default)]
    #[allow(dead_code)]
    is_launcher: bool,
    #[serde(default)]
    nutrition: u16,
    #[serde(default)]
    use_delay: i8,

    // Weapon-specific fields
    #[serde(default)]
    damage_small: Option<String>,
    #[serde(default)]
    damage_large: Option<String>,
    #[serde(default)]
    to_hit_bonus: Option<i8>,
    #[serde(default)]
    skill: Option<String>,
    #[serde(default)]
    strike_mode: Option<String>,

    // Armor-specific fields
    #[serde(default)]
    ac_bonus: Option<i8>,
    #[serde(default)]
    magic_cancel: Option<i8>,

    // Spellbook-specific fields
    #[serde(default)]
    spell_level: Option<i8>,
    #[serde(default)]
    direction: Option<String>,

    // Property fields
    #[serde(default)]
    conferred_property: Option<String>,
}

// ---------------------------------------------------------------------------
// Conversion: TOML -> Schema types
// ---------------------------------------------------------------------------

fn convert_monster(m: TomlMonster) -> Result<MonsterDef, LoadError> {
    let symbol = m
        .symbol
        .chars()
        .next()
        .ok_or_else(|| LoadError::Convert(format!("empty symbol for monster {}", m.name)))?;

    let color = parse_color(&m.color)?;
    let sound = parse_monster_sound(&m.sound)?;
    let size = parse_monster_size(&m.size)?;

    let mut attacks: ArrayVec<AttackDef, 6> = ArrayVec::new();
    for a in m.attacks {
        if attacks.len() >= 6 {
            break;
        }
        attacks.push(convert_attack(a)?);
    }

    let geno_flags = parse_geno_flags(&m.generation_flags)?;
    let resistances = parse_resistance_set(&m.resistances)?;
    let conveys = parse_resistance_set(&m.conveyed)?;
    let flags = parse_monster_flags(&m.flags)?;

    Ok(MonsterDef {
        id: MonsterId(m.id),
        names: MonsterNames {
            male: m.name,
            female: m.name_female,
        },
        symbol,
        color,
        base_level: m.base_level,
        speed: m.speed,
        armor_class: m.armor_class,
        magic_resistance: m.magic_resistance,
        alignment: m.alignment,
        difficulty: m.difficulty,
        attacks,
        geno_flags,
        frequency: m.frequency,
        corpse_weight: m.corpse_weight,
        corpse_nutrition: m.corpse_nutrition,
        sound,
        size,
        resistances,
        conveys,
        flags,
    })
}

fn convert_attack(a: TomlAttack) -> Result<AttackDef, LoadError> {
    let method = parse_attack_method(&a.attack_type)?;
    let damage_type = parse_damage_type(&a.damage_type)?;
    let dice = parse_dice_expr(&a.dice)?;
    Ok(AttackDef {
        method,
        damage_type,
        dice,
    })
}

fn convert_object(o: TomlObject) -> Result<ObjectDef, LoadError> {
    let class = parse_object_class(&o.class)?;
    let color = parse_color(&o.color)?;
    let material = match &o.material {
        Some(m) => parse_material(m)?,
        None => Material::NoMaterial,
    };

    // Normalize empty description strings to None
    let appearance = match o.description {
        Some(ref s) if s.is_empty() => None,
        Some(s) => Some(s),
        None => None,
    };

    // Build weapon info if weapon-specific fields are present
    let weapon = if o.damage_small.is_some() || o.damage_large.is_some() {
        let ds = o
            .damage_small
            .as_deref()
            .unwrap_or("0d0");
        let dl = o
            .damage_large
            .as_deref()
            .unwrap_or("0d0");
        let ds_dice = parse_dice_expr(ds)?;
        let dl_dice = parse_dice_expr(dl)?;

        // Weapon skill: try to parse from the skill field
        let skill = match &o.skill {
            Some(s) => parse_weapon_skill(s)?,
            None => WeaponSkill::None,
        };

        // Strike mode from string
        let strike_mode = match &o.strike_mode {
            Some(s) => parse_strike_mode(s)?,
            None => StrikeMode::empty(),
        };

        Some(WeaponInfo {
            skill,
            hit_bonus: o.to_hit_bonus.unwrap_or(0),
            damage_small: ds_dice.count as i8 * ds_dice.sides as i8,
            damage_large: dl_dice.count as i8 * dl_dice.sides as i8,
            strike_mode,
        })
    } else {
        None
    };

    // Build armor info if armor-specific fields are present
    let armor = if o.ac_bonus.is_some() {
        let category = match &o.sub {
            Some(s) => parse_armor_category(s)?,
            None => ArmorCategory::Suit,
        };
        Some(ArmorInfo {
            category,
            ac_bonus: o.ac_bonus.unwrap_or(0),
            magic_cancel: o.magic_cancel.unwrap_or(0),
        })
    } else {
        None
    };

    // Build spellbook info if spell-specific fields are present
    let spellbook = if o.spell_level.is_some() {
        let direction = match &o.direction {
            Some(s) => parse_spell_direction(s)?,
            None => SpellDirection::NoDir,
        };
        Some(SpellbookInfo {
            spell_level: o.spell_level.unwrap_or(0),
            direction,
        })
    } else {
        // Wands also have direction but no spell_level
        None
    };

    // Parse conferred property
    let conferred_property = match &o.conferred_property {
        Some(s) if s.is_empty() => None,
        Some(s) => Some(parse_property(s)?),
        None => None,
    };

    Ok(ObjectDef {
        id: ObjectTypeId(o.id),
        name: o.name,
        appearance,
        class,
        color,
        material,
        weight: o.weight,
        cost: o.cost,
        nutrition: o.nutrition,
        prob: o.prob,
        is_magic: o.is_magic,
        is_mergeable: o.is_mergeable,
        is_charged: o.is_charged,
        is_unique: o.is_unique,
        is_nowish: o.is_nowish,
        is_bimanual: o.two_handed,
        is_bulky: o.is_bulky,
        is_tough: o.is_tough,
        weapon,
        armor,
        spellbook,
        conferred_property,
        use_delay: o.use_delay,
    })
}

// ---------------------------------------------------------------------------
// String -> Enum parsers
// ---------------------------------------------------------------------------

fn parse_color(s: &str) -> Result<Color, LoadError> {
    match s {
        "Black" => Ok(Color::Black),
        "Red" => Ok(Color::Red),
        "Green" => Ok(Color::Green),
        "Brown" => Ok(Color::Brown),
        "Blue" => Ok(Color::Blue),
        "Magenta" => Ok(Color::Magenta),
        "Cyan" => Ok(Color::Cyan),
        "Gray" | "Grey" => Ok(Color::Gray),
        "NoColor" => Ok(Color::NoColor),
        "Orange" => Ok(Color::Orange),
        "BrightGreen" => Ok(Color::BrightGreen),
        "Yellow" => Ok(Color::Yellow),
        "BrightBlue" => Ok(Color::BrightBlue),
        "BrightMagenta" => Ok(Color::BrightMagenta),
        "BrightCyan" => Ok(Color::BrightCyan),
        "White" => Ok(Color::White),
        // HI_xxx aliases used in the C source for "material highlight" colors
        "HI_METAL" => Ok(Color::Cyan),
        "HI_WOOD" | "HI_LEATHER" => Ok(Color::Brown),
        "HI_PAPER" => Ok(Color::White),
        "HI_GLASS" => Ok(Color::BrightCyan),
        "HI_MINERAL" => Ok(Color::Gray),
        "HI_COPPER" => Ok(Color::Yellow),
        "HI_SILVER" => Ok(Color::BrightCyan),
        "HI_GOLD" => Ok(Color::Yellow),
        "HI_CLOTH" => Ok(Color::Brown),
        _ => Err(LoadError::Convert(format!("unknown color: {s}"))),
    }
}

fn parse_monster_sound(s: &str) -> Result<MonsterSound, LoadError> {
    match s {
        "Silent" | "0" => Ok(MonsterSound::Silent),
        "Bark" => Ok(MonsterSound::Bark),
        "Mew" => Ok(MonsterSound::Mew),
        "Roar" => Ok(MonsterSound::Roar),
        "Bellow" => Ok(MonsterSound::Bellow),
        "Growl" => Ok(MonsterSound::Growl),
        "Squeak" | "Sqeek" => Ok(MonsterSound::Sqeek),
        "Squawk" | "Sqawk" => Ok(MonsterSound::Sqawk),
        "Chirp" => Ok(MonsterSound::Chirp),
        "Hiss" => Ok(MonsterSound::Hiss),
        "Buzz" => Ok(MonsterSound::Buzz),
        "Grunt" => Ok(MonsterSound::Grunt),
        "Neigh" => Ok(MonsterSound::Neigh),
        "Moo" => Ok(MonsterSound::Moo),
        "Wail" => Ok(MonsterSound::Wail),
        "Gurgle" => Ok(MonsterSound::Gurgle),
        "Burble" => Ok(MonsterSound::Burble),
        "Trumpet" => Ok(MonsterSound::Trumpet),
        "Shriek" => Ok(MonsterSound::Shriek),
        "Bones" => Ok(MonsterSound::Bones),
        "Laugh" => Ok(MonsterSound::Laugh),
        "Mumble" => Ok(MonsterSound::Mumble),
        "Imitate" => Ok(MonsterSound::Imitate),
        "Were" => Ok(MonsterSound::Were),
        "Orc" => Ok(MonsterSound::Orc),
        "Humanoid" => Ok(MonsterSound::Humanoid),
        "Arrest" => Ok(MonsterSound::Arrest),
        "Soldier" => Ok(MonsterSound::Soldier),
        "Guard" => Ok(MonsterSound::Guard),
        "Djinni" => Ok(MonsterSound::Djinni),
        "Nurse" => Ok(MonsterSound::Nurse),
        "Seduce" => Ok(MonsterSound::Seduce),
        "Vampire" => Ok(MonsterSound::Vampire),
        "Bribe" => Ok(MonsterSound::Bribe),
        "Cuss" => Ok(MonsterSound::Cuss),
        "Rider" => Ok(MonsterSound::Rider),
        "Leader" => Ok(MonsterSound::Leader),
        "Nemesis" => Ok(MonsterSound::Nemesis),
        "Guardian" => Ok(MonsterSound::Guardian),
        "Sell" => Ok(MonsterSound::Sell),
        "Oracle" => Ok(MonsterSound::Oracle),
        "Priest" => Ok(MonsterSound::Priest),
        "Spell" => Ok(MonsterSound::Spell),
        "Boast" => Ok(MonsterSound::Boast),
        "Groan" => Ok(MonsterSound::Groan),
        _ => Err(LoadError::Convert(format!("unknown monster sound: {s}"))),
    }
}

fn parse_monster_size(s: &str) -> Result<MonsterSize, LoadError> {
    match s {
        "Tiny" | "0" => Ok(MonsterSize::Tiny),
        "Small" => Ok(MonsterSize::Small),
        "Medium" => Ok(MonsterSize::Medium),
        "Large" => Ok(MonsterSize::Large),
        "Huge" => Ok(MonsterSize::Huge),
        "Enormous" => Ok(MonsterSize::Enormous),
        "Colossal" => Ok(MonsterSize::Colossal),
        "Gigantic" => Ok(MonsterSize::Gigantic),
        _ => Err(LoadError::Convert(format!("unknown monster size: {s}"))),
    }
}

fn parse_attack_method(s: &str) -> Result<AttackMethod, LoadError> {
    match s {
        "Passive" | "None" => Ok(AttackMethod::None),
        "Claw" => Ok(AttackMethod::Claw),
        "Bite" => Ok(AttackMethod::Bite),
        "Kick" => Ok(AttackMethod::Kick),
        "Butt" => Ok(AttackMethod::Butt),
        "Touch" => Ok(AttackMethod::Touch),
        "Sting" => Ok(AttackMethod::Sting),
        "Hug" => Ok(AttackMethod::Hug),
        "Spit" => Ok(AttackMethod::Spit),
        "Engulf" => Ok(AttackMethod::Engulf),
        "Breath" => Ok(AttackMethod::Breath),
        "Explode" => Ok(AttackMethod::Explode),
        "ExplodeOnDeath" => Ok(AttackMethod::Boom),
        "Gaze" => Ok(AttackMethod::Gaze),
        "Tentacle" => Ok(AttackMethod::Tentacle),
        "Weapon" => Ok(AttackMethod::Weapon),
        "Magic" | "MagicMissile" => Ok(AttackMethod::MagicMissile),
        // Also handle attack-type strings that match damage-type names
        // when used as both (e.g., in TOML "type" = "Acid")
        "Acid" | "Cold" | "Fire" | "Electricity"
        | "Blind" | "Confuse" | "Corrode" | "Curse"
        | "Death" | "Decay" | "Digest" | "Disease"
        | "Disenchant" | "Disintegration" | "DrainCon"
        | "DrainDex" | "DrainInt" | "DrainLife" | "DrainMana"
        | "Famine" | "Hallucinate" | "Heal" | "Legs"
        | "Lycanthropy" | "Paralyze" | "Pestilence"
        | "Petrify" | "Physical" | "Poison" | "Polymorph"
        | "RandomBreath" | "Rust" | "SedExtended" | "Seduce"
        | "Sleep" | "Slime" | "Slow" | "StealAmulet"
        | "StealGold" | "StealItem" | "Stick" | "Stun"
        | "Teleport" | "Wrap" | "ClericalSpell" | "MagicSpell" => {
            // These are damage type names appearing as attack type.
            // In this context, the attack method is AT_NONE (passive).
            Ok(AttackMethod::None)
        }
        _ => Err(LoadError::Convert(format!("unknown attack method: {s}"))),
    }
}

fn parse_damage_type(s: &str) -> Result<DamageType, LoadError> {
    match s {
        "Physical" => Ok(DamageType::Physical),
        "MagicMissile" => Ok(DamageType::MagicMissile),
        "Fire" => Ok(DamageType::Fire),
        "Cold" => Ok(DamageType::Cold),
        "Sleep" => Ok(DamageType::Sleep),
        "Disintegration" => Ok(DamageType::Disintegrate),
        "Electricity" => Ok(DamageType::Electricity),
        "Poison" => Ok(DamageType::Poison),
        "Acid" => Ok(DamageType::Acid),
        "Blind" => Ok(DamageType::Blind),
        "Stun" => Ok(DamageType::Stun),
        "Slow" => Ok(DamageType::Slow),
        "Paralyze" => Ok(DamageType::Paralyze),
        "DrainLife" => Ok(DamageType::DrainLife),
        "DrainMana" => Ok(DamageType::DrainMagic),
        "Legs" => Ok(DamageType::Legs),
        "Petrify" => Ok(DamageType::Stone),
        "Stick" => Ok(DamageType::Sticking),
        "StealGold" => Ok(DamageType::GoldSteal),
        "StealItem" => Ok(DamageType::ItemSteal),
        "Seduce" => Ok(DamageType::Seduce),
        "Teleport" => Ok(DamageType::Teleport),
        "Rust" => Ok(DamageType::Rust),
        "Confuse" => Ok(DamageType::Confuse),
        "Digest" => Ok(DamageType::Digest),
        "Heal" => Ok(DamageType::Heal),
        "Wrap" => Ok(DamageType::Wrap),
        "Lycanthropy" => Ok(DamageType::Lycanthropy),
        "DrainDex" => Ok(DamageType::DrainDex),
        "DrainCon" => Ok(DamageType::DrainCon),
        "DrainInt" => Ok(DamageType::DrainInt),
        "Disease" => Ok(DamageType::Disease),
        "Decay" => Ok(DamageType::Decay),
        "SedExtended" => Ok(DamageType::SSuccubus),
        "Hallucinate" => Ok(DamageType::Hallucinate),
        "Death" => Ok(DamageType::Death),
        "Pestilence" => Ok(DamageType::Pestilence),
        "Famine" => Ok(DamageType::Famine),
        "Slime" => Ok(DamageType::Slime),
        "Disenchant" => Ok(DamageType::Disenchant),
        "Corrode" => Ok(DamageType::Corrode),
        "Polymorph" => Ok(DamageType::Polymorph),
        "ClericalSpell" => Ok(DamageType::ClericSpell),
        "MagicSpell" => Ok(DamageType::MagicSpell),
        "RandomBreath" => Ok(DamageType::RandomBreath),
        "StealAmulet" => Ok(DamageType::StealAmulet),
        "Curse" => Ok(DamageType::Curse),
        _ => Err(LoadError::Convert(format!("unknown damage type: {s}"))),
    }
}

fn parse_dice_expr(s: &str) -> Result<DiceExpr, LoadError> {
    let parts: Vec<&str> = s.split('d').collect();
    if parts.len() != 2 {
        return Err(LoadError::Convert(format!("invalid dice expr: {s}")));
    }
    let count: u8 = parts[0]
        .parse()
        .map_err(|_| LoadError::Convert(format!("invalid dice count in: {s}")))?;
    let sides: u8 = parts[1]
        .parse()
        .map_err(|_| LoadError::Convert(format!("invalid dice sides in: {s}")))?;
    Ok(DiceExpr { count, sides })
}

fn parse_geno_flags(flags: &[String]) -> Result<GenoFlags, LoadError> {
    let mut result = GenoFlags::empty();
    for f in flags {
        let flag = match f.as_str() {
            "Genocidable" => GenoFlags::G_GENO,
            "SmallGroups" => GenoFlags::G_SGROUP,
            "LargeGroups" => GenoFlags::G_LGROUP,
            "NoCorpse" => GenoFlags::G_NOCORPSE,
            "NoGen" => GenoFlags::G_NOGEN,
            "Hell" => GenoFlags::G_HELL,
            "NoHell" => GenoFlags::G_NOHELL,
            "Unique" => GenoFlags::G_UNIQ,
            _ => {
                return Err(LoadError::Convert(format!(
                    "unknown generation flag: {f}"
                )))
            }
        };
        result |= flag;
    }
    Ok(result)
}

fn parse_resistance_set(resists: &[String]) -> Result<ResistanceSet, LoadError> {
    let mut result = ResistanceSet::empty();
    for r in resists {
        let flag = match r.as_str() {
            "Fire" => ResistanceSet::FIRE,
            "Cold" => ResistanceSet::COLD,
            "Sleep" => ResistanceSet::SLEEP,
            "Disintegration" => ResistanceSet::DISINTEGRATE,
            "Electricity" => ResistanceSet::SHOCK,
            "Poison" => ResistanceSet::POISON,
            "Acid" => ResistanceSet::ACID,
            "Petrification" => ResistanceSet::STONE,
            _ => {
                return Err(LoadError::Convert(format!(
                    "unknown resistance: {r}"
                )))
            }
        };
        result |= flag;
    }
    Ok(result)
}

fn parse_monster_flags(flags: &[String]) -> Result<MonsterFlags, LoadError> {
    let mut result = MonsterFlags::empty();
    for f in flags {
        let flag = match f.as_str() {
            // M1 flags
            "Fly" => MonsterFlags::FLY,
            "Swim" => MonsterFlags::SWIM,
            "Amorphous" => MonsterFlags::AMORPHOUS,
            "WallWalk" => MonsterFlags::WALLWALK,
            "Cling" => MonsterFlags::CLING,
            "Tunnel" => MonsterFlags::TUNNEL,
            "NeedPick" => MonsterFlags::NEEDPICK,
            "Conceal" => MonsterFlags::CONCEAL,
            "Hide" => MonsterFlags::HIDE,
            "Amphibious" => MonsterFlags::AMPHIBIOUS,
            "Breathless" => MonsterFlags::BREATHLESS,
            "NoTake" => MonsterFlags::NOTAKE,
            "NoEyes" => MonsterFlags::NOEYES,
            "NoHands" => MonsterFlags::NOHANDS,
            "NoLimbs" => MonsterFlags::NOLIMBS,
            "NoHead" => MonsterFlags::NOHEAD,
            "Mindless" => MonsterFlags::MINDLESS,
            "Humanoid" => MonsterFlags::HUMANOID,
            "Animal" => MonsterFlags::ANIMAL,
            "Slithy" => MonsterFlags::SLITHY,
            "Unsolid" => MonsterFlags::UNSOLID,
            "ThickHide" => MonsterFlags::THICK_HIDE,
            "Oviparous" => MonsterFlags::OVIPAROUS,
            "Regeneration" => MonsterFlags::REGEN,
            "SeeInvisible" => MonsterFlags::SEE_INVIS,
            "Teleport" => MonsterFlags::TPORT,
            "TeleportControl" => MonsterFlags::TPORT_CNTRL,
            "Acidic" => MonsterFlags::ACID,
            "Poisonous" => MonsterFlags::POIS,
            "Carnivore" => MonsterFlags::CARNIVORE,
            "Herbivore" => MonsterFlags::HERBIVORE,
            "Omnivore" => MonsterFlags::OMNIVORE,
            "Metallivore" => MonsterFlags::METALLIVORE,
            // M2 flags
            "NoPoly" => MonsterFlags::NOPOLY,
            "Undead" => MonsterFlags::UNDEAD,
            "Were" => MonsterFlags::WERE,
            "Human" => MonsterFlags::HUMAN,
            "Elf" => MonsterFlags::ELF,
            "Dwarf" => MonsterFlags::DWARF,
            "Gnome" => MonsterFlags::GNOME,
            "Orc" => MonsterFlags::ORC,
            "Demon" => MonsterFlags::DEMON,
            "Merc" => MonsterFlags::MERC,
            "Lord" => MonsterFlags::LORD,
            "Prince" => MonsterFlags::PRINCE,
            "Minion" => MonsterFlags::MINION,
            "Giant" => MonsterFlags::GIANT,
            "Shapeshifter" => MonsterFlags::SHAPESHIFTER,
            "Male" => MonsterFlags::MALE,
            "Female" => MonsterFlags::FEMALE,
            "Neuter" => MonsterFlags::NEUTER,
            "ProperName" => MonsterFlags::PNAME,
            "Hostile" => MonsterFlags::HOSTILE,
            "Peaceful" => MonsterFlags::PEACEFUL,
            "Domestic" => MonsterFlags::DOMESTIC,
            "Wander" => MonsterFlags::WANDER,
            "Stalk" => MonsterFlags::STALK,
            "Nasty" => MonsterFlags::NASTY,
            "Strong" => MonsterFlags::STRONG,
            "RockThrow" => MonsterFlags::ROCKTHROW,
            "Greedy" => MonsterFlags::GREEDY,
            "Jewels" => MonsterFlags::JEWELS,
            "Collect" => MonsterFlags::COLLECT,
            "Magic" => MonsterFlags::MAGIC,
            // M3 flags
            "WantsAmulet" => MonsterFlags::WANTSAMUL,
            "WantsBook" => MonsterFlags::WANTSBOOK,
            "WantsCandelabrum" => MonsterFlags::WANTSCAND,
            "WantsArtifact" => MonsterFlags::WANTSARTI,
            "WaitForYou" => MonsterFlags::WAITFORU,
            "Close" => MonsterFlags::CLOSE,
            "Covetous" => MonsterFlags::COVETOUS,
            "InfraVision" | "Infravision" => MonsterFlags::INFRAVISION,
            "InfraVisible" | "Infravisible" => MonsterFlags::INFRAVISIBLE,
            "Displaces" => MonsterFlags::DISPLACES,
            // Geno-related flags appearing in the flags array should be ignored
            // since they belong in generation_flags. But handle gracefully.
            "Genocidable" | "SmallGroups" | "LargeGroups" | "NoCorpse" | "NoGen" | "Hell"
            | "NoHell" | "Unique" => {
                // These are generation flags; skip them in monster flags
                continue;
            }
            _ => {
                return Err(LoadError::Convert(format!(
                    "unknown monster flag: {f}"
                )))
            }
        };
        result |= flag;
    }
    Ok(result)
}

fn parse_object_class(s: &str) -> Result<ObjectClass, LoadError> {
    match s {
        "Weapon" => Ok(ObjectClass::Weapon),
        "Armor" => Ok(ObjectClass::Armor),
        "Ring" => Ok(ObjectClass::Ring),
        "Amulet" => Ok(ObjectClass::Amulet),
        "Tool" => Ok(ObjectClass::Tool),
        "Food" => Ok(ObjectClass::Food),
        "Potion" => Ok(ObjectClass::Potion),
        "Scroll" => Ok(ObjectClass::Scroll),
        "Spellbook" => Ok(ObjectClass::Spellbook),
        "Wand" => Ok(ObjectClass::Wand),
        "Coin" => Ok(ObjectClass::Coin),
        "Gem" => Ok(ObjectClass::Gem),
        "Rock" => Ok(ObjectClass::Rock),
        "Ball" => Ok(ObjectClass::Ball),
        "Chain" => Ok(ObjectClass::Chain),
        "Venom" => Ok(ObjectClass::Venom),
        _ => Err(LoadError::Convert(format!("unknown object class: {s}"))),
    }
}

fn parse_material(s: &str) -> Result<Material, LoadError> {
    match s {
        "" | "NoMaterial" => Ok(Material::NoMaterial),
        "Liquid" => Ok(Material::Liquid),
        "Wax" => Ok(Material::Wax),
        "Veggy" => Ok(Material::Veggy),
        "Flesh" => Ok(Material::Flesh),
        "Paper" => Ok(Material::Paper),
        "Cloth" => Ok(Material::Cloth),
        "Leather" => Ok(Material::Leather),
        "Wood" => Ok(Material::Wood),
        "Bone" => Ok(Material::Bone),
        "DragonHide" => Ok(Material::DragonHide),
        "Iron" => Ok(Material::Iron),
        "Metal" => Ok(Material::Metal),
        "Copper" => Ok(Material::Copper),
        "Silver" => Ok(Material::Silver),
        "Gold" => Ok(Material::Gold),
        "Platinum" => Ok(Material::Platinum),
        "Mithril" => Ok(Material::Mithril),
        "Plastic" => Ok(Material::Plastic),
        "Glass" => Ok(Material::Glass),
        "Gemstone" => Ok(Material::Gemstone),
        "Mineral" => Ok(Material::Mineral),
        _ => Err(LoadError::Convert(format!("unknown material: {s}"))),
    }
}

fn parse_weapon_skill(s: &str) -> Result<WeaponSkill, LoadError> {
    match s {
        "None" => Ok(WeaponSkill::None),
        "Dagger" => Ok(WeaponSkill::Dagger),
        "Knife" => Ok(WeaponSkill::Knife),
        "Axe" => Ok(WeaponSkill::Axe),
        "PickAxe" => Ok(WeaponSkill::PickAxe),
        "ShortSword" => Ok(WeaponSkill::ShortSword),
        "BroadSword" => Ok(WeaponSkill::BroadSword),
        "LongSword" => Ok(WeaponSkill::LongSword),
        "TwoHandedSword" => Ok(WeaponSkill::TwoHandedSword),
        "Saber" => Ok(WeaponSkill::Saber),
        "Club" => Ok(WeaponSkill::Club),
        "Mace" => Ok(WeaponSkill::Mace),
        "MorningStar" => Ok(WeaponSkill::MorningStar),
        "Flail" => Ok(WeaponSkill::Flail),
        "Hammer" => Ok(WeaponSkill::Hammer),
        "Quarterstaff" => Ok(WeaponSkill::Quarterstaff),
        "Polearms" => Ok(WeaponSkill::Polearms),
        "Spear" | "Javelin" => Ok(WeaponSkill::Spear),
        "Trident" => Ok(WeaponSkill::Trident),
        "Lance" => Ok(WeaponSkill::Lance),
        "Bow" => Ok(WeaponSkill::Bow),
        "Sling" => Ok(WeaponSkill::Sling),
        "Crossbow" => Ok(WeaponSkill::Crossbow),
        "Dart" => Ok(WeaponSkill::Dart),
        "Shuriken" => Ok(WeaponSkill::Shuriken),
        "Boomerang" => Ok(WeaponSkill::Boomerang),
        "Whip" => Ok(WeaponSkill::Whip),
        "UnicornHorn" => Ok(WeaponSkill::UnicornHorn),
        "AttackSpell" => Ok(WeaponSkill::AttackSpell),
        "HealingSpell" => Ok(WeaponSkill::HealingSpell),
        "DivineSpell" => Ok(WeaponSkill::DivineSpell),
        "EnchantSpell" => Ok(WeaponSkill::EnchantSpell),
        "ClericSpell" => Ok(WeaponSkill::ClericSpell),
        "EscapeSpell" => Ok(WeaponSkill::EscapeSpell),
        "MatterSpell" => Ok(WeaponSkill::MatterSpell),
        "BareHanded" => Ok(WeaponSkill::BareHanded),
        "TwoWeapon" => Ok(WeaponSkill::TwoWeapon),
        "Riding" => Ok(WeaponSkill::Riding),
        _ => Err(LoadError::Convert(format!("unknown weapon skill: {s}"))),
    }
}

fn parse_armor_category(s: &str) -> Result<ArmorCategory, LoadError> {
    match s {
        "Suit" => Ok(ArmorCategory::Suit),
        "Shield" => Ok(ArmorCategory::Shield),
        "Helm" => Ok(ArmorCategory::Helm),
        "Gloves" => Ok(ArmorCategory::Gloves),
        "Boots" => Ok(ArmorCategory::Boots),
        "Cloak" => Ok(ArmorCategory::Cloak),
        "Shirt" => Ok(ArmorCategory::Shirt),
        _ => Err(LoadError::Convert(format!(
            "unknown armor category: {s}"
        ))),
    }
}

fn parse_strike_mode(s: &str) -> Result<StrikeMode, LoadError> {
    // Can be a single value or could be combined; handle single values.
    let mut mode = StrikeMode::empty();
    for part in s.split('|') {
        let part = part.trim();
        let flag = match part {
            "Pierce" => StrikeMode::PIERCE,
            "Slash" => StrikeMode::SLASH,
            "Whack" => StrikeMode::WHACK,
            _ => {
                return Err(LoadError::Convert(format!(
                    "unknown strike mode: {part}"
                )))
            }
        };
        mode |= flag;
    }
    Ok(mode)
}

fn parse_spell_direction(s: &str) -> Result<SpellDirection, LoadError> {
    // Handle both PascalCase (spellbooks) and lowercase (wands)
    match s.to_ascii_lowercase().as_str() {
        "nodir" => Ok(SpellDirection::NoDir),
        "immediate" => Ok(SpellDirection::Immediate),
        "ray" => Ok(SpellDirection::Ray),
        _ => Err(LoadError::Convert(format!(
            "unknown spell direction: {s}"
        ))),
    }
}

fn parse_property(s: &str) -> Result<Property, LoadError> {
    match s {
        "FireRes" => Ok(Property::FireRes),
        "ColdRes" => Ok(Property::ColdRes),
        "SleepRes" => Ok(Property::SleepRes),
        "DisintRes" => Ok(Property::DisintRes),
        "ShockRes" => Ok(Property::ShockRes),
        "PoisonRes" => Ok(Property::PoisonRes),
        "AcidRes" => Ok(Property::AcidRes),
        "StoneRes" => Ok(Property::StoneRes),
        "DrainRes" => Ok(Property::DrainRes),
        "SickRes" => Ok(Property::SickRes),
        "Invulnerable" => Ok(Property::Invulnerable),
        "Antimagic" => Ok(Property::Antimagic),
        "Stunned" => Ok(Property::Stunned),
        "Confusion" => Ok(Property::Confusion),
        "Blinded" => Ok(Property::Blinded),
        "Deaf" => Ok(Property::Deaf),
        "Sick" => Ok(Property::Sick),
        "Stoned" => Ok(Property::Stoned),
        "Strangled" => Ok(Property::Strangled),
        "Vomiting" => Ok(Property::Vomiting),
        "Glib" => Ok(Property::Glib),
        "Slimed" => Ok(Property::Slimed),
        "Halluc" => Ok(Property::Halluc),
        "HallucRes" => Ok(Property::HallucRes),
        "Fumbling" => Ok(Property::Fumbling),
        "WoundedLegs" => Ok(Property::WoundedLegs),
        "Sleepy" => Ok(Property::Sleepy),
        "Hunger" => Ok(Property::Hunger),
        "SeeInvis" => Ok(Property::SeeInvis),
        "Telepat" => Ok(Property::Telepat),
        "Warning" => Ok(Property::Warning),
        "WarnOfMon" => Ok(Property::WarnOfMon),
        "WarnUndead" => Ok(Property::WarnUndead),
        "Searching" => Ok(Property::Searching),
        "Clairvoyant" => Ok(Property::Clairvoyant),
        "Infravision" => Ok(Property::Infravision),
        "DetectMonsters" => Ok(Property::DetectMonsters),
        "BlindRes" => Ok(Property::BlindRes),
        "Adorned" => Ok(Property::Adorned),
        "Invis" => Ok(Property::Invis),
        "Displaced" => Ok(Property::Displaced),
        "Stealth" => Ok(Property::Stealth),
        "AggravateMonster" => Ok(Property::AggravateMonster),
        "Conflict" => Ok(Property::Conflict),
        "Jumping" => Ok(Property::Jumping),
        "Teleport" => Ok(Property::Teleport),
        "TeleportControl" => Ok(Property::TeleportControl),
        "Levitation" => Ok(Property::Levitation),
        "Flying" => Ok(Property::Flying),
        "Wwalking" => Ok(Property::Wwalking),
        "Swimming" => Ok(Property::Swimming),
        "MagicalBreathing" => Ok(Property::MagicalBreathing),
        "PassesWalls" => Ok(Property::PassesWalls),
        "SlowDigestion" => Ok(Property::SlowDigestion),
        "HalfSpdam" => Ok(Property::HalfSpdam),
        "HalfPhdam" => Ok(Property::HalfPhdam),
        "Regeneration" => Ok(Property::Regeneration),
        "EnergyRegeneration" => Ok(Property::EnergyRegeneration),
        "Protection" => Ok(Property::Protection),
        "ProtFromShapeChangers" => Ok(Property::ProtFromShapeChangers),
        "Polymorph" => Ok(Property::Polymorph),
        "PolymorphControl" => Ok(Property::PolymorphControl),
        "Unchanging" => Ok(Property::Unchanging),
        "Fast" => Ok(Property::Fast),
        "Reflecting" => Ok(Property::Reflecting),
        "FreeAction" => Ok(Property::FreeAction),
        "FixedAbil" => Ok(Property::FixedAbil),
        "Lifesaved" => Ok(Property::Lifesaved),
        _ => Err(LoadError::Convert(format!("unknown property: {s}"))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Get the project root data directory.
    fn data_dir() -> PathBuf {
        // Tests run from the crate root (crates/data/), so we go up two levels.
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.join("../../data")
    }

    #[test]
    fn test_load_weapons() {
        let path = data_dir().join("items/weapons.toml");
        let objects = load_objects(&path).expect("failed to load weapons.toml");
        assert_eq!(objects.len(), 71, "expected 71 weapons");
        // First weapon is "arrow"
        assert_eq!(objects[0].name, "arrow");
        assert_eq!(objects[0].class, ObjectClass::Weapon);
        assert!(objects[0].weapon.is_some());
    }

    #[test]
    fn test_load_armor() {
        let path = data_dir().join("items/armor.toml");
        let objects = load_objects(&path).expect("failed to load armor.toml");
        assert_eq!(objects.len(), 82, "expected 82 armor items");
        // First armor is "elven leather helm"
        assert_eq!(objects[0].name, "elven leather helm");
        assert_eq!(objects[0].class, ObjectClass::Armor);
        assert!(objects[0].armor.is_some());
        let armor = objects[0].armor.as_ref().unwrap();
        assert_eq!(armor.category, ArmorCategory::Helm);
        assert_eq!(armor.ac_bonus, 1);
    }

    #[test]
    fn test_load_monsters() {
        let path = data_dir().join("monsters/base.toml");
        let monsters = load_monsters(&path).expect("failed to load monsters/base.toml");
        assert_eq!(monsters.len(), 394, "expected 394 monsters");
        // First monster is "giant ant"
        assert_eq!(monsters[0].names.male, "giant ant");
        assert_eq!(monsters[0].symbol, 'a');
        assert_eq!(monsters[0].color, Color::Brown);
        assert_eq!(monsters[0].attacks.len(), 1);
        assert_eq!(monsters[0].attacks[0].method, AttackMethod::Bite);
    }

    #[test]
    fn test_load_potions() {
        let path = data_dir().join("items/potions.toml");
        let objects = load_objects(&path).expect("failed to load potions.toml");
        assert_eq!(objects.len(), 26, "expected 26 potions");
        assert_eq!(objects[0].class, ObjectClass::Potion);
    }

    #[test]
    fn test_load_scrolls() {
        let path = data_dir().join("items/scrolls.toml");
        let objects = load_objects(&path).expect("failed to load scrolls.toml");
        assert_eq!(objects.len(), 23, "expected 23 scrolls");
        assert_eq!(objects[0].class, ObjectClass::Scroll);
    }

    #[test]
    fn test_load_spellbooks() {
        let path = data_dir().join("items/spellbooks.toml");
        let objects = load_objects(&path).expect("failed to load spellbooks.toml");
        assert_eq!(objects.len(), 44, "expected 44 spellbooks");
        assert_eq!(objects[0].class, ObjectClass::Spellbook);
        assert!(objects[0].spellbook.is_some());
    }

    #[test]
    fn test_load_wands() {
        let path = data_dir().join("items/wands.toml");
        let objects = load_objects(&path).expect("failed to load wands.toml");
        assert_eq!(objects.len(), 24, "expected 24 wands");
        assert_eq!(objects[0].class, ObjectClass::Wand);
    }

    #[test]
    fn test_load_rings() {
        let path = data_dir().join("items/rings.toml");
        let objects = load_objects(&path).expect("failed to load rings.toml");
        assert_eq!(objects.len(), 28, "expected 28 rings");
        assert_eq!(objects[0].class, ObjectClass::Ring);
    }

    #[test]
    fn test_load_amulets() {
        let path = data_dir().join("items/amulets.toml");
        let objects = load_objects(&path).expect("failed to load amulets.toml");
        assert_eq!(objects.len(), 13, "expected 13 amulets");
        assert_eq!(objects[0].class, ObjectClass::Amulet);
    }

    #[test]
    fn test_load_tools() {
        let path = data_dir().join("items/tools.toml");
        let objects = load_objects(&path).expect("failed to load tools.toml");
        assert_eq!(objects.len(), 50, "expected 50 tools");
        assert_eq!(objects[0].class, ObjectClass::Tool);
    }

    #[test]
    fn test_load_food() {
        let path = data_dir().join("items/food.toml");
        let objects = load_objects(&path).expect("failed to load food.toml");
        assert_eq!(objects.len(), 33, "expected 33 food items");
        assert_eq!(objects[0].class, ObjectClass::Food);
    }

    #[test]
    fn test_load_gems() {
        let path = data_dir().join("items/gems.toml");
        let objects = load_objects(&path).expect("failed to load gems.toml");
        assert_eq!(objects.len(), 36, "expected 36 gems");
        assert_eq!(objects[0].class, ObjectClass::Gem);
    }

    #[test]
    fn test_load_all_game_data() {
        let dir = data_dir();
        let data = load_game_data(&dir).expect("failed to load all game data");
        assert_eq!(data.monsters.len(), 394, "expected 394 monsters");
        // Total objects across all categories
        let expected_total = 71 + 82 + 28 + 13 + 50 + 33 + 26 + 23 + 24 + 44 + 36;
        assert_eq!(
            data.objects.len(),
            expected_total,
            "expected {expected_total} total objects"
        );
    }

    #[test]
    fn test_monster_with_female_name() {
        let path = data_dir().join("monsters/base.toml");
        let monsters = load_monsters(&path).expect("failed to load monsters");
        // Find a monster that has a female name variant
        let has_female = monsters.iter().any(|m| m.names.female.is_some());
        assert!(has_female, "expected at least one monster with a female name");
    }

    #[test]
    fn test_monster_with_resistances() {
        let path = data_dir().join("monsters/base.toml");
        let monsters = load_monsters(&path).expect("failed to load monsters");
        // killer bee (id=1) should have Poison resistance
        let bee = &monsters[1];
        assert_eq!(bee.names.male, "killer bee");
        assert!(bee.resistances.contains(ResistanceSet::POISON));
        assert!(bee.conveys.contains(ResistanceSet::POISON));
    }

    #[test]
    fn test_dice_expr_parsing() {
        let d = parse_dice_expr("2d6").unwrap();
        assert_eq!(d.count, 2);
        assert_eq!(d.sides, 6);

        let d = parse_dice_expr("0d0").unwrap();
        assert_eq!(d.count, 0);
        assert_eq!(d.sides, 0);

        assert!(parse_dice_expr("bad").is_err());
    }

    #[test]
    fn test_armor_conferred_property() {
        let path = data_dir().join("items/armor.toml");
        let objects = load_objects(&path).expect("failed to load armor");
        // cornuthaum (id=4) should confer Clairvoyant
        let cornuthaum = objects.iter().find(|o| o.name == "cornuthaum");
        assert!(cornuthaum.is_some());
        assert_eq!(
            cornuthaum.unwrap().conferred_property,
            Some(Property::Clairvoyant)
        );
    }

    #[test]
    fn test_empty_description_becomes_none() {
        let path = data_dir().join("items/weapons.toml");
        let objects = load_objects(&path).expect("failed to load weapons");
        // "arrow" has description = "" which should become None
        let arrow = &objects[0];
        assert_eq!(arrow.name, "arrow");
        assert!(arrow.appearance.is_none());
    }
}
