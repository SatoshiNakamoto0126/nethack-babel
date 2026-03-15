//! Schema types for TOML-based level definitions.
//!
//! These types define the declarative format for special dungeon levels.
//! Levels that need dynamic logic (conditional terrain, loops) use Rust
//! generators in `crates/engine/src/special_levels.rs` instead.

use serde::{Deserialize, Serialize};

/// A complete level definition loaded from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelDefinition {
    pub level: LevelHeader,
    #[serde(default)]
    pub map: Option<MapDefinition>,
    #[serde(default)]
    pub regions: Vec<RegionDefinition>,
    #[serde(default)]
    pub monsters: Vec<MonsterPlacement>,
    #[serde(default)]
    pub objects: Vec<ObjectPlacement>,
    #[serde(default)]
    pub traps: Vec<TrapPlacement>,
    #[serde(default)]
    pub doors: Vec<DoorPlacement>,
    #[serde(default)]
    pub stairs: Vec<StairsPlacement>,
    #[serde(default)]
    pub altars: Vec<AltarPlacement>,
    #[serde(default)]
    pub engraving: Vec<EngravingPlacement>,
    #[serde(default)]
    pub shuffle_groups: Vec<ShuffleGroup>,
}

/// Level header metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelHeader {
    pub name: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub depth: Option<i32>,
}

/// ASCII map definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapDefinition {
    #[serde(default = "default_halign")]
    pub halign: String,
    #[serde(default = "default_valign")]
    pub valign: String,
    pub data: String,
}

fn default_halign() -> String {
    "center".to_string()
}
fn default_valign() -> String {
    "center".to_string()
}

/// A named region within the level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionDefinition {
    /// [x1, y1, x2, y2] bounding box
    pub area: [i32; 4],
    #[serde(default)]
    pub lit: Option<bool>,
    #[serde(default)]
    pub region_type: Option<String>,
    #[serde(default)]
    pub irregular: bool,
}

/// Monster placement specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonsterPlacement {
    /// Specific monster name (e.g., "Orcus", "vampire lord")
    #[serde(default)]
    pub id: Option<String>,
    /// Monster class letter (e.g., "V" for vampires)
    #[serde(default)]
    pub class: Option<String>,
    /// Fixed x coordinate (random if absent)
    #[serde(default)]
    pub x: Option<i32>,
    /// Fixed y coordinate (random if absent)
    #[serde(default)]
    pub y: Option<i32>,
    /// Spawn probability as percentage (100 = always, 75 = 75% chance)
    #[serde(default = "default_chance")]
    pub chance: u32,
    /// Whether the monster is peaceful
    #[serde(default)]
    pub peaceful: Option<bool>,
    /// Whether the monster is asleep
    #[serde(default)]
    pub asleep: Option<bool>,
    /// Alignment override
    #[serde(default)]
    pub align: Option<String>,
}

/// Object placement specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPlacement {
    /// Specific object name (e.g., "wand of death")
    #[serde(default)]
    pub id: Option<String>,
    /// Object class character (e.g., "/" for wands)
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default = "default_chance")]
    pub chance: u32,
    /// BUC status override
    #[serde(default)]
    pub cursed: Option<bool>,
    #[serde(default)]
    pub blessed: Option<bool>,
    /// Quantity for stackable items
    #[serde(default)]
    pub quantity: Option<u32>,
    /// Enchantment value
    #[serde(default)]
    pub enchantment: Option<i32>,
    /// Whether the item is identified
    #[serde(default)]
    pub identified: Option<bool>,
    /// Container contents name
    #[serde(default)]
    pub contained_in: Option<String>,
}

/// Trap placement specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapPlacement {
    #[serde(rename = "type")]
    pub trap_type: String,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default = "default_chance")]
    pub chance: u32,
}

/// Door placement specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoorPlacement {
    pub state: String, // "open", "closed", "locked", "nodoor", "broken"
    pub x: i32,
    pub y: i32,
}

/// Stairs placement specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StairsPlacement {
    pub direction: String, // "up" or "down"
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
}

/// Altar placement specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AltarPlacement {
    pub align: String, // "lawful", "neutral", "chaotic", "noalign"
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub shrine: bool,
}

/// Engraving placement specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngravingPlacement {
    pub x: i32,
    pub y: i32,
    pub text: String,
    #[serde(default = "default_engrave_type")]
    pub engrave_type: String,
}

fn default_chance() -> u32 {
    100
}
fn default_engrave_type() -> String {
    "engrave".to_string()
}

/// Shuffle group for random variant selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShuffleGroup {
    pub name: String,
    pub choices: Vec<String>,
}

/// Dungeon topology definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DungeonTopology {
    pub branches: Vec<BranchDefinition>,
    #[serde(default)]
    pub connections: Vec<BranchConnection>,
}

/// A dungeon branch definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchDefinition {
    pub name: String,
    pub max_depth: i32,
    #[serde(default)]
    pub base_difficulty: Option<i32>,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub special_levels: Vec<SpecialLevelEntry>,
}

/// A special level assignment within a branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialLevelEntry {
    pub name: String,
    /// Fixed depth or None for randomized
    #[serde(default)]
    pub depth: Option<i32>,
    /// Depth range for randomized placement [min, max]
    #[serde(default)]
    pub depth_range: Option<[i32; 2]>,
    /// TOML file to load (relative path)
    #[serde(default)]
    pub file: Option<String>,
}

/// Connection between branches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchConnection {
    pub from_branch: String,
    pub to_branch: String,
    /// Fixed entrance depth or randomized range
    #[serde(default)]
    pub entrance_depth: Option<i32>,
    #[serde(default)]
    pub entrance_range: Option<[i32; 2]>,
    /// Connection type: "stairs", "portal", "magic_portal"
    #[serde(default = "default_connection_type")]
    pub connection_type: String,
}

fn default_connection_type() -> String {
    "stairs".to_string()
}
