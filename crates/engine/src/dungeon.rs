//! Dungeon map representation: terrain types, map cells, level maps,
//! dungeon-wide topology, and branch transition logic.

use std::collections::{HashMap, HashSet};

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::action::Position;
use crate::engrave::EngravingMap;
use crate::region::GasCloud;
use crate::traps::TrapMap;

// Re-export TerrainType from the canonical data crate definition.
pub use nethack_babel_data::TerrainType;

/// Which branch of the dungeon the player is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DungeonBranch {
    Main,
    Mines,
    Sokoban,
    Quest,
    FortLudios,
    Gehennom,
    VladsTower,
    Endgame,
}

/// Cached representation of a monster for level stashing.
///
/// When the player leaves a level, all monster entities are despawned and
/// their essential state is stored in this struct.  When the player returns,
/// the cached monsters are respawned as fresh ECS entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedMonster {
    pub position: Position,
    pub name: String,
    pub hp_current: i32,
    pub hp_max: i32,
    pub speed: u32,
    pub symbol: char,
    pub color: nethack_babel_data::Color,
}

/// What occupies a single map cell.
///
/// This is the engine-level terrain type used for map logic.  It wraps the
/// canonical `TerrainType` from the data crate and adds gameplay-oriented
/// query methods (`is_opaque`, `is_walkable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Terrain {
    Stone,
    Wall,
    Floor,
    Corridor,
    DoorOpen,
    DoorClosed,
    DoorLocked,
    StairsUp,
    StairsDown,
    Altar,
    Fountain,
    Throne,
    Sink,
    Grave,
    Pool,
    Moat,
    Lava,
    Ice,
    Air,
    Cloud,
    Water,
    Tree,
    IronBars,
    Drawbridge,
    MagicPortal,
}

impl Terrain {
    /// Whether the terrain blocks line of sight.
    #[inline]
    pub fn is_opaque(self) -> bool {
        matches!(
            self,
            Terrain::Stone | Terrain::Wall | Terrain::Tree | Terrain::DoorClosed
        )
    }

    /// Whether a creature can walk onto this terrain (ignoring levitation,
    /// swimming, etc.).
    #[inline]
    pub fn is_walkable(self) -> bool {
        matches!(
            self,
            Terrain::Floor
                | Terrain::Corridor
                | Terrain::DoorOpen
                | Terrain::StairsUp
                | Terrain::StairsDown
                | Terrain::Altar
                | Terrain::Fountain
                | Terrain::Throne
                | Terrain::Sink
                | Terrain::Grave
                | Terrain::Ice
                | Terrain::Air
                | Terrain::Cloud
                | Terrain::Drawbridge
                | Terrain::MagicPortal
        )
    }

    /// Convert from the canonical `TerrainType` (data crate) to the
    /// engine-level `Terrain` enum.
    pub fn from_terrain_type(tt: TerrainType) -> Self {
        match tt {
            TerrainType::Stone => Terrain::Stone,
            TerrainType::VWall
            | TerrainType::HWall
            | TerrainType::TLCorner
            | TerrainType::TRCorner
            | TerrainType::BLCorner
            | TerrainType::BRCorner
            | TerrainType::CrossWall
            | TerrainType::TUWall
            | TerrainType::TDWall
            | TerrainType::TLWall
            | TerrainType::TRWall
            | TerrainType::DbWall
            | TerrainType::LavaWall => Terrain::Wall,
            TerrainType::Tree => Terrain::Tree,
            TerrainType::SecretDoor | TerrainType::Door => Terrain::DoorClosed,
            TerrainType::SecretCorridor | TerrainType::Corridor => Terrain::Corridor,
            TerrainType::Pool => Terrain::Pool,
            TerrainType::Moat => Terrain::Moat,
            TerrainType::Water => Terrain::Water,
            TerrainType::DrawbridgeUp => Terrain::Drawbridge,
            TerrainType::LavaPool => Terrain::Lava,
            TerrainType::IronBars => Terrain::IronBars,
            TerrainType::Room => Terrain::Floor,
            TerrainType::Stairs | TerrainType::Ladder => Terrain::StairsUp,
            TerrainType::Fountain => Terrain::Fountain,
            TerrainType::Throne => Terrain::Throne,
            TerrainType::Sink => Terrain::Sink,
            TerrainType::Grave => Terrain::Grave,
            TerrainType::Altar => Terrain::Altar,
            TerrainType::Ice => Terrain::Ice,
            TerrainType::DrawbridgeDown => Terrain::Drawbridge,
            TerrainType::Air => Terrain::Air,
            TerrainType::Cloud => Terrain::Cloud,
        }
    }
}

/// A single cell on the level map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapCell {
    pub terrain: Terrain,
    /// Whether the player has ever seen this cell.
    pub explored: bool,
    /// Whether the cell is currently in the player's FOV.
    pub visible: bool,
}

impl Default for MapCell {
    fn default() -> Self {
        Self {
            terrain: Terrain::Stone,
            explored: false,
            visible: false,
        }
    }
}

/// A single dungeon level (80x21 standard NetHack size).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelMap {
    pub cells: Vec<Vec<MapCell>>,
    pub width: usize,
    pub height: usize,
}

impl LevelMap {
    /// NetHack standard dimensions.
    pub const DEFAULT_WIDTH: usize = 80;
    pub const DEFAULT_HEIGHT: usize = 21;

    /// Create a new level map filled with stone.
    pub fn new(width: usize, height: usize) -> Self {
        let cells = vec![vec![MapCell::default(); width]; height];
        Self {
            cells,
            width,
            height,
        }
    }

    /// Create a level with standard NetHack dimensions.
    pub fn new_standard() -> Self {
        Self::new(Self::DEFAULT_WIDTH, Self::DEFAULT_HEIGHT)
    }

    /// Get the terrain at a position, if in bounds.
    #[inline]
    pub fn get(&self, pos: Position) -> Option<&MapCell> {
        if self.in_bounds(pos) {
            Some(&self.cells[pos.y as usize][pos.x as usize])
        } else {
            None
        }
    }

    /// Get a mutable reference to the cell at a position, if in bounds.
    pub fn get_mut(&mut self, pos: Position) -> Option<&mut MapCell> {
        if self.in_bounds(pos) {
            Some(&mut self.cells[pos.y as usize][pos.x as usize])
        } else {
            None
        }
    }

    /// Set the terrain at a position.
    pub fn set_terrain(&mut self, pos: Position, terrain: Terrain) {
        if self.in_bounds(pos) {
            self.cells[pos.y as usize][pos.x as usize].terrain = terrain;
        }
    }

    /// Whether a position is inside the map boundaries.
    #[inline]
    pub fn in_bounds(&self, pos: Position) -> bool {
        pos.x >= 0 && pos.y >= 0 && (pos.x as usize) < self.width && (pos.y as usize) < self.height
    }

    /// Return `(width, height)` as a tuple.
    #[inline]
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

// ── Branch transition configuration ──────────────────────────────────────

/// Describes a branch entrance: when descending on the Main branch at a
/// depth within [min_depth, max_depth], transition to the target branch.
#[derive(Debug, Clone, Copy)]
pub struct BranchEntrance {
    pub target: DungeonBranch,
    pub min_depth: i32,
    pub max_depth: i32,
    /// Starting depth in the target branch.
    pub target_start_depth: i32,
}

/// NetHack-standard branch entrance table.
pub const BRANCH_ENTRANCES: &[BranchEntrance] = &[
    BranchEntrance {
        target: DungeonBranch::Mines,
        min_depth: 3,
        max_depth: 5,
        target_start_depth: 1,
    },
    // Oracle level is around depth 5-9 but is on the Main branch;
    // it's a special room, not a branch.  We don't encode it here.
];

/// Sokoban entrance: from Mines depth 3-4, descending enters Sokoban.
pub const SOKOBAN_ENTRANCE: BranchEntrance = BranchEntrance {
    target: DungeonBranch::Sokoban,
    min_depth: 3,
    max_depth: 4,
    target_start_depth: 1,
};

/// Check if descending at the given branch+depth should enter a new
/// branch.  Returns `Some((target_branch, target_depth))` if a branch
/// transition occurs.
///
/// For the Mines entrance: the actual entrance depth is randomly chosen
/// once per game (from depth 3-5) and stored in `DungeonState`.
///
/// For Sokoban: entrance from Mines level 3-4.
///
/// For Gehennom: entering from depth >= 25 in the Main branch after the
/// Castle level.
pub fn check_branch_entrance(
    branch: DungeonBranch,
    depth: i32,
    mines_entrance_depth: i32,
    sokoban_entrance_depth: i32,
) -> Option<(DungeonBranch, i32)> {
    match branch {
        DungeonBranch::Main => {
            // Mines entrance
            if depth == mines_entrance_depth {
                return Some((DungeonBranch::Mines, 1));
            }
            // Castle is at depth ~25.  Below the Castle is Gehennom.
            if depth >= 26 {
                return Some((DungeonBranch::Gehennom, depth - 25));
            }
            None
        }
        DungeonBranch::Mines => {
            // Sokoban entrance from Mines depth 3-4.
            if depth == sokoban_entrance_depth {
                return Some((DungeonBranch::Sokoban, 1));
            }
            None
        }
        _ => None,
    }
}

/// Result of checking whether stairs can be used.
#[derive(Debug, Clone, PartialEq)]
pub enum StairsResult {
    Allowed,
    Blocked { reason: String },
    NeedsConfirmation { prompt: String },
}

/// Check if using stairs is allowed at this location.
/// Sokoban has special stair constraints.
pub fn stairs_allowed(
    branch: DungeonBranch,
    depth: i32,
    going_up: bool,
    carrying_boulder: bool,
) -> StairsResult {
    match branch {
        DungeonBranch::Sokoban => {
            if !going_up {
                // Can't go down in Sokoban (one-way up only)
                StairsResult::Blocked {
                    reason: "The stairs here lead only upward.".into(),
                }
            } else if carrying_boulder {
                StairsResult::Blocked {
                    reason: "You can't carry a boulder up the stairs!".into(),
                }
            } else {
                StairsResult::Allowed
            }
        }
        DungeonBranch::Quest => {
            if going_up && depth == 1 {
                // Leaving quest — ask confirmation
                StairsResult::NeedsConfirmation {
                    prompt: "Leave the quest? You may not return.".into(),
                }
            } else {
                StairsResult::Allowed
            }
        }
        DungeonBranch::Endgame => {
            // Can't go back in endgame
            if going_up {
                StairsResult::Blocked {
                    reason: "There is no turning back now!".into(),
                }
            } else {
                StairsResult::Allowed
            }
        }
        _ => StairsResult::Allowed,
    }
}

/// Absolute dungeon depth for a branch/level pair.
///
/// Maps branch-local depth to absolute depth in the main dungeon.
/// This is used for difficulty scaling, monster generation, and
/// level teleport destination display.
///
/// Formula follows NetHack's `depth()` in `dungeon.c`:
/// - Main: depth = level
/// - Mines: depth = mines_entrance_depth + level - 1
/// - Sokoban: depth = mines_entrance_depth + sokoban_entrance_depth + level - 2
/// - Quest: depth = quest_entrance_depth + level - 1
/// - FortLudios: depth = fort_ludios_depth
/// - Gehennom: depth = 25 + level
/// - VladsTower: depth = 25 + vlad_offset + level (but going up)
/// - Endgame: depth = 50 + level
pub fn absolute_depth(
    branch: DungeonBranch,
    local_depth: i32,
    mines_entrance_depth: i32,
    sokoban_entrance_depth: i32,
) -> i32 {
    match branch {
        DungeonBranch::Main => local_depth,
        DungeonBranch::Mines => mines_entrance_depth + local_depth - 1,
        DungeonBranch::Sokoban => mines_entrance_depth + sokoban_entrance_depth + local_depth - 2,
        DungeonBranch::Quest => {
            // Quest entrance is around depth 13-15.
            // Base: Oracle level (~5-9) + 6-8 = ~11-17.  Simplified: 14 + level - 1.
            14 + local_depth - 1
        }
        DungeonBranch::FortLudios => {
            // Fort Ludios is accessed via portal, typically around depth 18-22.
            20
        }
        DungeonBranch::Gehennom => 25 + local_depth,
        DungeonBranch::VladsTower => {
            // Vlad's Tower goes upward: bottom is around depth 35, top is 33.
            35 - local_depth + 1
        }
        DungeonBranch::Endgame => 50 + local_depth,
    }
}

/// Maximum depth within a branch (canonical level count).
pub fn branch_max_depth(branch: DungeonBranch) -> i32 {
    match branch {
        DungeonBranch::Main => 29,  // 25 + up to 4 random
        DungeonBranch::Mines => 10, // 8 + up to 2
        DungeonBranch::Sokoban => 4,
        DungeonBranch::Quest => 7, // 5 + up to 2
        DungeonBranch::FortLudios => 1,
        DungeonBranch::Gehennom => 25, // 20 + up to 5
        DungeonBranch::VladsTower => 3,
        DungeonBranch::Endgame => 6,
    }
}

/// Whether a branch is hellish (Gehennom properties: fire traps, no bones,
/// prayer restrictions, demon lord lairs).
pub fn branch_is_hellish(branch: DungeonBranch) -> bool {
    matches!(branch, DungeonBranch::Gehennom)
}

/// Whether the branch uses mazelike level generation.
pub fn branch_is_mazelike(branch: DungeonBranch) -> bool {
    matches!(
        branch,
        DungeonBranch::Gehennom
            | DungeonBranch::Mines
            | DungeonBranch::Sokoban
            | DungeonBranch::FortLudios
            | DungeonBranch::VladsTower
            | DungeonBranch::Endgame
    )
}

/// Dungeon level identifier: branch + local depth.
///
/// Used as a key for level caching, portal lookup, and visited tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DungeonLevel {
    pub branch: DungeonBranch,
    pub depth: i32,
}

impl DungeonLevel {
    pub fn new(branch: DungeonBranch, depth: i32) -> Self {
        Self { branch, depth }
    }

    /// Convert to absolute dungeon depth.
    pub fn absolute_depth(&self, mines_entrance_depth: i32, sokoban_entrance_depth: i32) -> i32 {
        absolute_depth(
            self.branch,
            self.depth,
            mines_entrance_depth,
            sokoban_entrance_depth,
        )
    }
}

impl std::fmt::Display for DungeonLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self.branch {
            DungeonBranch::Main => "Dungeons of Doom",
            DungeonBranch::Mines => "Gnomish Mines",
            DungeonBranch::Sokoban => "Sokoban",
            DungeonBranch::Quest => "Quest",
            DungeonBranch::FortLudios => "Fort Ludios",
            DungeonBranch::Gehennom => "Gehennom",
            DungeonBranch::VladsTower => "Vlad's Tower",
            DungeonBranch::Endgame => "Elemental Planes",
        };
        write!(f, "{}:{}", name, self.depth)
    }
}

// ── Portal mechanics ────────────────────────────────────────────────────

/// A portal link between two branch/depth pairs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalLink {
    pub from_branch: DungeonBranch,
    pub from_depth: i32,
    pub from_pos: Position,
    pub to_branch: DungeonBranch,
    pub to_depth: i32,
    pub to_pos: Position,
}

// ── Level feeling messages ──────────────────────────────────────────────

/// Serializable flags for the current level's special properties.
///
/// Mirrors `SpecialLevelFlags` from `special_levels` but with serde support
/// for inclusion in `DungeonState`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct CurrentLevelFlags {
    /// Digging is forbidden on this level (e.g. Sokoban).
    pub no_dig: bool,
    /// Teleporting is forbidden on this level.
    pub no_teleport: bool,
    /// Prayer does not work on this level (e.g. Gehennom, Sanctum).
    pub no_prayer: bool,
    /// This level is part of the endgame sequence.
    pub is_endgame: bool,
}

impl From<crate::special_levels::SpecialLevelFlags> for CurrentLevelFlags {
    fn from(f: crate::special_levels::SpecialLevelFlags) -> Self {
        Self {
            no_dig: f.no_dig,
            no_teleport: f.no_teleport,
            no_prayer: f.no_prayer,
            is_endgame: f.is_endgame,
        }
    }
}

/// Flags describing the current level's atmosphere, used to select
/// level-feeling messages.
#[derive(Debug, Clone, Copy, Default)]
pub struct LevelFlags {
    pub has_bones: bool,
    pub has_graveyard: bool,
    pub previously_visited: bool,
    pub has_demon_lord: bool,
}

/// Return a level-feeling message based on the level's flags.
///
/// Returns `None` if no special feeling applies.  In NetHack, feelings
/// are shown upon first entering a level.
pub fn level_feeling(flags: &LevelFlags, _depth: i32, rng: &mut impl Rng) -> Option<&'static str> {
    // Priority order matches NetHack's `arrive()` logic.
    if flags.has_demon_lord {
        return Some("You sense the presence of evil...");
    }
    if flags.has_bones {
        // Classic bones feeling (50% chance).
        if rng.random_bool(0.5) {
            return Some("You have a strange feeling...");
        }
    }
    if flags.has_graveyard {
        return Some("You feel a cold shiver...");
    }
    if flags.previously_visited {
        return Some("This place seems familiar...");
    }
    None
}

// ── Dungeon topology ────────────────────────────────────────────────────

/// Tracks randomized special level depth assignments for this game session.
///
/// Each game randomizes where certain special levels appear within their
/// allowed depth range (matching NetHack's `init_dungeons()` behavior).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DungeonTopology {
    /// Oracle level depth within Main (randomized 5-9).
    pub oracle_depth: i32,
    /// Rogue level depth within Main (randomized 15-18).
    pub rogue_depth: i32,
    /// Big Room depth within Main (randomized 10-13).
    pub bigroom_depth: i32,
    /// Whether this game has a Big Room (50% chance).
    pub has_bigroom: bool,
}

impl DungeonTopology {
    /// Create a new topology with randomized depths.
    pub fn new(rng: &mut impl Rng) -> Self {
        Self {
            oracle_depth: rng.random_range(5..=9),
            rogue_depth: rng.random_range(15..=18),
            bigroom_depth: rng.random_range(10..=13),
            has_bigroom: rng.random_bool(0.5),
        }
    }

    /// Create a topology with fixed values (for testing).
    pub fn fixed(oracle: i32, rogue: i32, bigroom: i32, has_bigroom: bool) -> Self {
        Self {
            oracle_depth: oracle,
            rogue_depth: rogue,
            bigroom_depth: bigroom,
            has_bigroom,
        }
    }
}

impl Default for DungeonTopology {
    fn default() -> Self {
        Self {
            oracle_depth: 7,
            rogue_depth: 16,
            bigroom_depth: 11,
            has_bigroom: false,
        }
    }
}

// ── Dungeon state ───────────────────────────────────────────────────────

/// Top-level dungeon state across all levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DungeonState {
    pub current_level: LevelMap,
    pub depth: i32,
    pub branch: DungeonBranch,
    /// Cache of visited levels, keyed by (branch, depth).
    pub levels: HashMap<(DungeonBranch, i32), LevelMap>,
    /// Cache of monster state per level, keyed by (branch, depth).
    pub monster_cache: HashMap<(DungeonBranch, i32), Vec<CachedMonster>>,
    /// Traps on the current level.
    pub trap_map: TrapMap,
    /// Engravings on the current level.
    pub engraving_map: EngravingMap,
    /// Set of (branch, depth) pairs the player has entered at least once.
    pub visited_set: HashSet<(DungeonBranch, i32)>,
    /// Randomly determined Mines entrance depth (3-5), set once per game.
    pub mines_entrance_depth: i32,
    /// Randomly determined Sokoban entrance depth (3-4 in Mines), set once per game.
    pub sokoban_entrance_depth: i32,
    /// Portal links between branches (e.g. Fort Ludios, Quest, Endgame).
    pub portals: Vec<PortalLink>,
    /// Maximum dungeon depth for level teleport clamping.
    max_depth_value: Option<i32>,
    /// Randomized dungeon topology (special level depth assignments).
    pub topology: DungeonTopology,
    /// Flags for the current level (no_dig, no_teleport, etc.).
    pub current_level_flags: CurrentLevelFlags,
    /// Player annotations keyed by (branch, depth), equivalent to NetHack's
    /// per-level notes set by `#annotate`.
    pub level_annotations: HashMap<(DungeonBranch, i32), String>,
    /// Player "called" names for item classes, keyed by class character.
    pub called_item_classes: HashMap<char, String>,
    /// Vault rooms on the current level (for guard spawning).
    pub vault_rooms: Vec<crate::vault::VaultRoom>,
    /// Whether a vault guard is currently active on this level.
    pub vault_guard_present: bool,
    /// Active gas clouds on the current level.
    pub gas_clouds: Vec<GasCloud>,
    /// Runtime autopickup toggle (wired from CLI options).
    pub autopickup_enabled: bool,
    /// Runtime autopickup object classes (wired from CLI options).
    pub autopickup_classes: Vec<nethack_babel_data::ObjectClass>,
}

impl DungeonState {
    /// Create a new dungeon starting on level 1 of the main dungeon.
    pub fn new() -> Self {
        Self::with_rng(&mut rand::rng())
    }

    /// Create a new dungeon with deterministic branch entrance depths.
    pub fn with_rng(rng: &mut impl Rng) -> Self {
        let mines_entrance_depth = rng.random_range(3..=5);
        let sokoban_entrance_depth = rng.random_range(3..=4);
        let topology = DungeonTopology::new(rng);
        Self {
            current_level: LevelMap::new_standard(),
            depth: 1,
            branch: DungeonBranch::Main,
            levels: HashMap::new(),
            monster_cache: HashMap::new(),
            trap_map: TrapMap::new(),
            engraving_map: EngravingMap::new(),
            visited_set: HashSet::new(),
            mines_entrance_depth,
            sokoban_entrance_depth,
            portals: Vec::new(),
            max_depth_value: None,
            topology,
            current_level_flags: CurrentLevelFlags::default(),
            level_annotations: HashMap::new(),
            called_item_classes: HashMap::new(),
            vault_rooms: Vec::new(),
            vault_guard_present: false,
            gas_clouds: Vec::new(),
            autopickup_enabled: true,
            autopickup_classes: Vec::new(),
        }
    }

    /// Create a dungeon with specific branch entrance depths (for testing).
    pub fn with_entrance_depths(mines_depth: i32, sokoban_depth: i32) -> Self {
        Self {
            current_level: LevelMap::new_standard(),
            depth: 1,
            branch: DungeonBranch::Main,
            levels: HashMap::new(),
            monster_cache: HashMap::new(),
            trap_map: TrapMap::new(),
            engraving_map: EngravingMap::new(),
            visited_set: HashSet::new(),
            mines_entrance_depth: mines_depth,
            sokoban_entrance_depth: sokoban_depth,
            portals: Vec::new(),
            max_depth_value: None,
            topology: DungeonTopology::default(),
            current_level_flags: CurrentLevelFlags::default(),
            level_annotations: HashMap::new(),
            called_item_classes: HashMap::new(),
            vault_rooms: Vec::new(),
            vault_guard_present: false,
            gas_clouds: Vec::new(),
            autopickup_enabled: true,
            autopickup_classes: Vec::new(),
        }
    }

    /// Configure runtime autopickup behavior.
    pub fn set_autopickup(&mut self, enabled: bool, classes: Vec<nethack_babel_data::ObjectClass>) {
        self.autopickup_enabled = enabled;
        self.autopickup_classes = classes;
    }

    /// Save the current level and its monsters into the cache.
    pub fn cache_current_level(&mut self, monsters: Vec<CachedMonster>) {
        let key = (self.branch, self.depth);
        self.levels.insert(key, self.current_level.clone());
        self.monster_cache.insert(key, monsters);
    }

    /// Check if a level has been visited before.
    pub fn has_visited(&self, branch: DungeonBranch, depth: i32) -> bool {
        self.levels.contains_key(&(branch, depth))
    }

    /// Load a cached level, returning the map and cached monsters.
    /// Returns None if the level hasn't been visited.
    pub fn load_cached_level(
        &mut self,
        branch: DungeonBranch,
        depth: i32,
    ) -> Option<(LevelMap, Vec<CachedMonster>)> {
        let key = (branch, depth);
        let map = self.levels.get(&key)?.clone();
        let monsters = self.monster_cache.get(&key).cloned().unwrap_or_default();
        Some((map, monsters))
    }

    /// Mark the current (branch, depth) as visited by the player.
    pub fn mark_visited(&mut self) {
        self.visited_set.insert((self.branch, self.depth));
    }

    /// Whether the player has previously entered this (branch, depth).
    pub fn was_visited(&self, branch: DungeonBranch, depth: i32) -> bool {
        self.visited_set.contains(&(branch, depth))
    }

    /// Set or clear an annotation for a specific level.
    ///
    /// Empty/whitespace-only text clears the annotation.
    pub fn set_level_annotation(&mut self, branch: DungeonBranch, depth: i32, text: String) {
        let key = (branch, depth);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            self.level_annotations.remove(&key);
        } else {
            self.level_annotations.insert(key, trimmed.to_string());
        }
    }

    /// Set or clear annotation for the current level.
    pub fn set_current_level_annotation(&mut self, text: String) {
        self.set_level_annotation(self.branch, self.depth, text);
    }

    /// Get annotation for a level, if any.
    pub fn level_annotation(&self, branch: DungeonBranch, depth: i32) -> Option<&str> {
        self.level_annotations
            .get(&(branch, depth))
            .map(String::as_str)
    }

    /// Get annotation for current level, if any.
    pub fn current_level_annotation(&self) -> Option<&str> {
        self.level_annotation(self.branch, self.depth)
    }

    /// Set or clear a "called" name for an item class.
    ///
    /// Empty/whitespace-only text clears the called name.
    pub fn set_called_item_class(&mut self, class: char, name: String) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            self.called_item_classes.remove(&class);
        } else {
            self.called_item_classes.insert(class, trimmed.to_string());
        }
    }

    /// Get the called name for an item class, if present.
    pub fn called_item_class(&self, class: char) -> Option<&str> {
        self.called_item_classes.get(&class).map(String::as_str)
    }

    /// Check if descending at the current branch+depth should enter a
    /// new branch.  Delegates to [`check_branch_entrance`].
    pub fn check_branch_transition(&self) -> Option<(DungeonBranch, i32)> {
        check_branch_entrance(
            self.branch,
            self.depth,
            self.mines_entrance_depth,
            self.sokoban_entrance_depth,
        )
    }

    /// Add a portal link between two branch/depth/position pairs.
    pub fn add_portal(&mut self, link: PortalLink) {
        self.portals.push(link);
    }

    /// Look up a portal at the given branch, depth, and position.
    /// Returns the destination (branch, depth, position) if found.
    pub fn find_portal(
        &self,
        branch: DungeonBranch,
        depth: i32,
        pos: Position,
    ) -> Option<(DungeonBranch, i32, Position)> {
        for link in &self.portals {
            if link.from_branch == branch && link.from_depth == depth && link.from_pos == pos {
                return Some((link.to_branch, link.to_depth, link.to_pos));
            }
            // Portals are bidirectional.
            if link.to_branch == branch && link.to_depth == depth && link.to_pos == pos {
                return Some((link.from_branch, link.from_depth, link.from_pos));
            }
        }
        None
    }

    /// Return the current dungeon depth.
    #[inline]
    pub fn current_depth(&self) -> i32 {
        self.depth
    }

    /// Return the maximum depth the player can reach in the main dungeon.
    ///
    /// If no max has been set, returns the current depth.
    #[inline]
    pub fn max_depth(&self) -> i32 {
        self.max_depth_value.unwrap_or(self.depth)
    }

    /// Set the maximum dungeon depth (for level teleport clamping).
    pub fn set_max_depth(&mut self, depth: i32) {
        self.max_depth_value = Some(depth);
    }

    /// Replace the current level with a new map and change depth.
    pub fn set_current_level(&mut self, level: LevelMap, new_depth: i32) {
        self.current_level = level;
        self.depth = new_depth;
    }

    /// Compute the absolute dungeon depth of the current location.
    pub fn absolute_depth(&self) -> i32 {
        absolute_depth(
            self.branch,
            self.depth,
            self.mines_entrance_depth,
            self.sokoban_entrance_depth,
        )
    }

    /// Return a `DungeonLevel` for the current location.
    pub fn current_dungeon_level(&self) -> DungeonLevel {
        DungeonLevel::new(self.branch, self.depth)
    }

    /// Whether the current branch is hellish.
    pub fn is_hellish(&self) -> bool {
        branch_is_hellish(self.branch)
    }

    /// Whether the current branch uses mazelike generation.
    pub fn is_mazelike(&self) -> bool {
        branch_is_mazelike(self.branch)
    }

    /// Move to a different branch and depth.
    pub fn change_branch(&mut self, branch: DungeonBranch, depth: i32) {
        self.branch = branch;
        self.depth = depth;
    }

    /// Maximum depth in the current branch.
    pub fn current_branch_max_depth(&self) -> i32 {
        branch_max_depth(self.branch)
    }

    /// Check if the given (branch, depth) is a topology-assigned special level.
    ///
    /// For the Main branch, uses the per-game randomized topology depths
    /// (oracle, rogue, bigroom) instead of the hardcoded defaults in
    /// `identify_special_level`. For other branches, delegates to
    /// `identify_special_level` directly.
    pub fn check_topology_special(
        &self,
        branch: &DungeonBranch,
        depth: i32,
    ) -> Option<crate::special_levels::SpecialLevelId> {
        use crate::special_levels::SpecialLevelId;
        match branch {
            DungeonBranch::Main => {
                if depth == self.topology.oracle_depth {
                    return Some(SpecialLevelId::OracleLevel);
                }
                if depth == self.topology.rogue_depth {
                    return Some(SpecialLevelId::Rogue);
                }
                if self.topology.has_bigroom && depth == self.topology.bigroom_depth {
                    return Some(SpecialLevelId::BigRoom(0));
                }
                // Castle and Medusa remain at fixed depths.
                match depth {
                    25 => Some(SpecialLevelId::Castle),
                    24 => Some(SpecialLevelId::Medusa(0)),
                    _ => None,
                }
            }
            // For other branches, delegate to identify_special_level.
            _ => crate::special_levels::identify_special_level(*branch, depth),
        }
    }
}

impl Default for DungeonState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    #[test]
    fn test_cache_and_restore_level() {
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        // Mark a distinctive terrain on the current level.
        ds.current_level
            .set_terrain(Position::new(5, 5), Terrain::StairsDown);

        let monsters = vec![CachedMonster {
            position: Position::new(3, 3),
            name: "grid bug".to_string(),
            hp_current: 4,
            hp_max: 4,
            speed: 12,
            symbol: 'x',
            color: nethack_babel_data::Color::Magenta,
        }];
        ds.cache_current_level(monsters);

        // Overwrite the current level to simulate switching.
        ds.current_level = LevelMap::new_standard();
        ds.depth = 2;

        // Restore cached level 1.
        let (map, mons) = ds
            .load_cached_level(DungeonBranch::Main, 1)
            .expect("level 1 should be cached");
        assert_eq!(
            map.get(Position::new(5, 5)).unwrap().terrain,
            Terrain::StairsDown,
        );
        assert_eq!(mons.len(), 1);
        assert_eq!(mons[0].name, "grid bug");
        assert_eq!(mons[0].hp_current, 4);
    }

    #[test]
    fn test_has_visited() {
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        assert!(
            !ds.has_visited(DungeonBranch::Main, 1),
            "no levels cached yet"
        );

        ds.cache_current_level(vec![]);
        assert!(
            ds.has_visited(DungeonBranch::Main, 1),
            "level 1 should be cached"
        );
        assert!(
            !ds.has_visited(DungeonBranch::Main, 2),
            "level 2 not visited"
        );
        assert!(
            !ds.has_visited(DungeonBranch::Mines, 1),
            "mines not visited"
        );
    }

    // ── Branch transition tests ─────────────────────────────────────

    #[test]
    fn test_branch_entrance_mines_depth() {
        // Mines entrance depth is randomly chosen in 3-5 range.
        for mines_depth in 3..=5 {
            let result = check_branch_entrance(
                DungeonBranch::Main,
                mines_depth,
                mines_depth, // mines entrance is at this depth
                3,           // sokoban doesn't matter here
            );
            assert_eq!(
                result,
                Some((DungeonBranch::Mines, 1)),
                "descending at Main depth {} with mines entrance at {} should enter Mines",
                mines_depth,
                mines_depth,
            );
        }

        // Depth 2 should NOT enter Mines even if mines entrance is 3.
        assert_eq!(check_branch_entrance(DungeonBranch::Main, 2, 3, 3), None,);

        // Depth 4 with mines entrance at 3 should NOT enter Mines.
        assert_eq!(check_branch_entrance(DungeonBranch::Main, 4, 3, 3), None,);
    }

    #[test]
    fn test_branch_entrance_sokoban() {
        // Sokoban entrance from Mines depth 3 or 4.
        for sok_depth in 3..=4 {
            let result = check_branch_entrance(
                DungeonBranch::Mines,
                sok_depth,
                3, // mines entrance depth (irrelevant)
                sok_depth,
            );
            assert_eq!(
                result,
                Some((DungeonBranch::Sokoban, 1)),
                "descending at Mines depth {} with sokoban entrance at {} should enter Sokoban",
                sok_depth,
                sok_depth,
            );
        }
    }

    #[test]
    fn test_branch_entrance_gehennom() {
        // Gehennom begins below depth 25 (Castle).
        let result = check_branch_entrance(DungeonBranch::Main, 26, 3, 3);
        assert_eq!(result, Some((DungeonBranch::Gehennom, 1)));

        let result = check_branch_entrance(DungeonBranch::Main, 30, 3, 3);
        assert_eq!(result, Some((DungeonBranch::Gehennom, 5)));

        // Depth 25 is the Castle, not yet Gehennom.
        assert_eq!(check_branch_entrance(DungeonBranch::Main, 25, 3, 3), None,);
    }

    #[test]
    fn test_branch_entrance_random_mines_depth() {
        // Verify the DungeonState mines_entrance_depth falls in 3-5.
        for seed in 0..100 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let ds = DungeonState::with_rng(&mut rng);
            assert!(
                (3..=5).contains(&ds.mines_entrance_depth),
                "mines_entrance_depth {} not in 3..=5 for seed {}",
                ds.mines_entrance_depth,
                seed,
            );
            assert!(
                (3..=4).contains(&ds.sokoban_entrance_depth),
                "sokoban_entrance_depth {} not in 3..=4 for seed {}",
                ds.sokoban_entrance_depth,
                seed,
            );
        }
    }

    #[test]
    fn test_dungeon_state_check_branch_transition() {
        let mut ds = DungeonState::with_entrance_depths(4, 3);
        ds.branch = DungeonBranch::Main;
        ds.depth = 4;
        assert_eq!(
            ds.check_branch_transition(),
            Some((DungeonBranch::Mines, 1)),
        );

        ds.depth = 3;
        assert_eq!(ds.check_branch_transition(), None);

        ds.branch = DungeonBranch::Mines;
        ds.depth = 3;
        assert_eq!(
            ds.check_branch_transition(),
            Some((DungeonBranch::Sokoban, 1)),
        );
    }

    // ── Level feeling tests ─────────────────────────────────────────

    #[test]
    fn test_level_feeling_bones() {
        let flags = LevelFlags {
            has_bones: true,
            ..Default::default()
        };
        // Over many trials, roughly half should produce the bones message.
        let mut bone_count = 0;
        let trials = 1000;
        for seed in 0..trials {
            let mut rng = Pcg64::seed_from_u64(seed);
            if let Some(msg) = level_feeling(&flags, 5, &mut rng) {
                assert_eq!(msg, "You have a strange feeling...");
                bone_count += 1;
            }
        }
        // Expect ~50%, allow generous margin.
        assert!(
            bone_count > 300 && bone_count < 700,
            "Expected ~50% bones feeling, got {}/{}",
            bone_count,
            trials,
        );
    }

    #[test]
    fn test_level_feeling_demon_lord() {
        let flags = LevelFlags {
            has_demon_lord: true,
            has_bones: true, // demon lord takes priority
            ..Default::default()
        };
        let mut rng = Pcg64::seed_from_u64(42);
        let msg = level_feeling(&flags, 30, &mut rng);
        assert_eq!(msg, Some("You sense the presence of evil..."));
    }

    #[test]
    fn test_level_feeling_graveyard() {
        let flags = LevelFlags {
            has_graveyard: true,
            ..Default::default()
        };
        let mut rng = Pcg64::seed_from_u64(42);
        let msg = level_feeling(&flags, 10, &mut rng);
        assert_eq!(msg, Some("You feel a cold shiver..."));
    }

    #[test]
    fn test_level_feeling_familiar() {
        let flags = LevelFlags {
            previously_visited: true,
            ..Default::default()
        };
        let mut rng = Pcg64::seed_from_u64(42);
        let msg = level_feeling(&flags, 5, &mut rng);
        assert_eq!(msg, Some("This place seems familiar..."));
    }

    #[test]
    fn test_level_feeling_no_flags() {
        let flags = LevelFlags::default();
        let mut rng = Pcg64::seed_from_u64(42);
        assert_eq!(level_feeling(&flags, 5, &mut rng), None);
    }

    // ── Portal tests ────────────────────────────────────────────────

    #[test]
    fn test_portal_bidirectional_lookup() {
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        ds.add_portal(PortalLink {
            from_branch: DungeonBranch::Main,
            from_depth: 15,
            from_pos: Position::new(10, 5),
            to_branch: DungeonBranch::FortLudios,
            to_depth: 1,
            to_pos: Position::new(40, 10),
        });

        // Forward lookup.
        let dest = ds.find_portal(DungeonBranch::Main, 15, Position::new(10, 5));
        assert_eq!(
            dest,
            Some((DungeonBranch::FortLudios, 1, Position::new(40, 10))),
        );

        // Reverse lookup.
        let dest = ds.find_portal(DungeonBranch::FortLudios, 1, Position::new(40, 10));
        assert_eq!(dest, Some((DungeonBranch::Main, 15, Position::new(10, 5))),);

        // Non-existent portal.
        assert_eq!(
            ds.find_portal(DungeonBranch::Main, 15, Position::new(20, 5)),
            None,
        );
    }

    #[test]
    fn test_visited_set() {
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        assert!(!ds.was_visited(DungeonBranch::Main, 1));

        ds.mark_visited();
        assert!(ds.was_visited(DungeonBranch::Main, 1));
        assert!(!ds.was_visited(DungeonBranch::Main, 2));
    }

    // ── Absolute depth tests ─────────────────────────────────────

    #[test]
    fn test_absolute_depth_main() {
        assert_eq!(absolute_depth(DungeonBranch::Main, 1, 3, 3), 1);
        assert_eq!(absolute_depth(DungeonBranch::Main, 10, 3, 3), 10);
        assert_eq!(absolute_depth(DungeonBranch::Main, 25, 3, 3), 25);
    }

    #[test]
    fn test_absolute_depth_mines() {
        // Mines: mines_entrance + level - 1.
        assert_eq!(absolute_depth(DungeonBranch::Mines, 1, 3, 3), 3);
        assert_eq!(absolute_depth(DungeonBranch::Mines, 5, 3, 3), 7);
        assert_eq!(absolute_depth(DungeonBranch::Mines, 1, 5, 3), 5);
    }

    #[test]
    fn test_absolute_depth_sokoban() {
        // Sokoban: mines + sokoban + level - 2.
        // mines=3, sokoban=3, level=1: 3+3+1-2 = 5
        assert_eq!(absolute_depth(DungeonBranch::Sokoban, 1, 3, 3), 5);
        // mines=3, sokoban=3, level=4: 3+3+4-2 = 8
        assert_eq!(absolute_depth(DungeonBranch::Sokoban, 4, 3, 3), 8);
        // mines=5, sokoban=4, level=1: 5+4+1-2 = 8
        assert_eq!(absolute_depth(DungeonBranch::Sokoban, 1, 5, 4), 8);
    }

    #[test]
    fn test_absolute_depth_gehennom() {
        assert_eq!(absolute_depth(DungeonBranch::Gehennom, 1, 3, 3), 26);
        assert_eq!(absolute_depth(DungeonBranch::Gehennom, 10, 3, 3), 35);
    }

    #[test]
    fn test_absolute_depth_quest() {
        assert_eq!(absolute_depth(DungeonBranch::Quest, 1, 3, 3), 14);
        assert_eq!(absolute_depth(DungeonBranch::Quest, 5, 3, 3), 18);
    }

    #[test]
    fn test_absolute_depth_vlads_tower() {
        // Vlad's Tower goes upward.
        let bottom = absolute_depth(DungeonBranch::VladsTower, 1, 3, 3);
        let top = absolute_depth(DungeonBranch::VladsTower, 3, 3, 3);
        assert!(
            bottom > top,
            "bottom ({}) should be deeper than top ({})",
            bottom,
            top
        );
        assert_eq!(bottom, 35);
        assert_eq!(top, 33);
    }

    #[test]
    fn test_absolute_depth_endgame() {
        assert_eq!(absolute_depth(DungeonBranch::Endgame, 1, 3, 3), 51);
        assert_eq!(absolute_depth(DungeonBranch::Endgame, 6, 3, 3), 56);
    }

    #[test]
    fn test_absolute_depth_fort_ludios() {
        assert_eq!(absolute_depth(DungeonBranch::FortLudios, 1, 3, 3), 20);
    }

    // ── Branch max depth tests ───────────────────────────────────

    #[test]
    fn test_branch_max_depth_values() {
        assert_eq!(branch_max_depth(DungeonBranch::Main), 29);
        assert_eq!(branch_max_depth(DungeonBranch::Mines), 10);
        assert_eq!(branch_max_depth(DungeonBranch::Sokoban), 4);
        assert_eq!(branch_max_depth(DungeonBranch::Quest), 7);
        assert_eq!(branch_max_depth(DungeonBranch::FortLudios), 1);
        assert_eq!(branch_max_depth(DungeonBranch::Gehennom), 25);
        assert_eq!(branch_max_depth(DungeonBranch::VladsTower), 3);
        assert_eq!(branch_max_depth(DungeonBranch::Endgame), 6);
    }

    // ── Branch property tests ────────────────────────────────────

    #[test]
    fn test_branch_is_hellish() {
        assert!(branch_is_hellish(DungeonBranch::Gehennom));
        assert!(!branch_is_hellish(DungeonBranch::Main));
        assert!(!branch_is_hellish(DungeonBranch::Mines));
        assert!(!branch_is_hellish(DungeonBranch::VladsTower));
        assert!(!branch_is_hellish(DungeonBranch::Endgame));
    }

    #[test]
    fn test_branch_is_mazelike() {
        assert!(!branch_is_mazelike(DungeonBranch::Main));
        assert!(!branch_is_mazelike(DungeonBranch::Quest));
        assert!(branch_is_mazelike(DungeonBranch::Gehennom));
        assert!(branch_is_mazelike(DungeonBranch::Mines));
        assert!(branch_is_mazelike(DungeonBranch::Sokoban));
        assert!(branch_is_mazelike(DungeonBranch::FortLudios));
        assert!(branch_is_mazelike(DungeonBranch::VladsTower));
        assert!(branch_is_mazelike(DungeonBranch::Endgame));
    }

    // ── DungeonLevel tests ───────────────────────────────────────

    #[test]
    fn test_dungeon_level_display() {
        let level = DungeonLevel::new(DungeonBranch::Main, 5);
        assert_eq!(format!("{}", level), "Dungeons of Doom:5");

        let level = DungeonLevel::new(DungeonBranch::Mines, 3);
        assert_eq!(format!("{}", level), "Gnomish Mines:3");

        let level = DungeonLevel::new(DungeonBranch::Gehennom, 10);
        assert_eq!(format!("{}", level), "Gehennom:10");

        let level = DungeonLevel::new(DungeonBranch::VladsTower, 2);
        assert_eq!(format!("{}", level), "Vlad's Tower:2");

        let level = DungeonLevel::new(DungeonBranch::Endgame, 1);
        assert_eq!(format!("{}", level), "Elemental Planes:1");
    }

    #[test]
    fn test_dungeon_level_absolute_depth() {
        let level = DungeonLevel::new(DungeonBranch::Mines, 3);
        assert_eq!(level.absolute_depth(4, 3), 6); // 4 + 3 - 1
    }

    #[test]
    fn test_dungeon_level_equality() {
        let a = DungeonLevel::new(DungeonBranch::Main, 5);
        let b = DungeonLevel::new(DungeonBranch::Main, 5);
        let c = DungeonLevel::new(DungeonBranch::Main, 6);
        let d = DungeonLevel::new(DungeonBranch::Mines, 5);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn test_dungeon_level_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(DungeonLevel::new(DungeonBranch::Main, 1));
        set.insert(DungeonLevel::new(DungeonBranch::Main, 1));
        set.insert(DungeonLevel::new(DungeonBranch::Mines, 1));
        assert_eq!(set.len(), 2);
    }

    // ── DungeonState method tests ────────────────────────────────

    #[test]
    fn test_dungeon_state_absolute_depth() {
        let mut ds = DungeonState::with_entrance_depths(4, 3);
        ds.branch = DungeonBranch::Main;
        ds.depth = 10;
        assert_eq!(ds.absolute_depth(), 10);

        ds.branch = DungeonBranch::Mines;
        ds.depth = 3;
        assert_eq!(ds.absolute_depth(), 6); // 4 + 3 - 1
    }

    #[test]
    fn test_dungeon_state_current_level() {
        let ds = DungeonState::with_entrance_depths(3, 3);
        let level = ds.current_dungeon_level();
        assert_eq!(level.branch, DungeonBranch::Main);
        assert_eq!(level.depth, 1);
    }

    #[test]
    fn test_dungeon_state_is_hellish() {
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        assert!(!ds.is_hellish());

        ds.branch = DungeonBranch::Gehennom;
        assert!(ds.is_hellish());
    }

    #[test]
    fn test_dungeon_state_is_mazelike() {
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        assert!(!ds.is_mazelike()); // Main

        ds.branch = DungeonBranch::Gehennom;
        assert!(ds.is_mazelike());
    }

    #[test]
    fn test_dungeon_state_change_branch() {
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        ds.change_branch(DungeonBranch::Mines, 1);
        assert_eq!(ds.branch, DungeonBranch::Mines);
        assert_eq!(ds.depth, 1);
    }

    #[test]
    fn test_dungeon_state_branch_max_depth() {
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        assert_eq!(ds.current_branch_max_depth(), 29); // Main

        ds.branch = DungeonBranch::Sokoban;
        assert_eq!(ds.current_branch_max_depth(), 4);
    }

    #[test]
    fn test_dungeon_state_max_depth() {
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        // Initially, max_depth == current depth (1).
        assert_eq!(ds.max_depth(), 1);

        ds.set_max_depth(30);
        assert_eq!(ds.max_depth(), 30);
    }

    // ── Terrain tests ────────────────────────────────────────────

    #[test]
    fn test_terrain_opaque() {
        assert!(Terrain::Stone.is_opaque());
        assert!(Terrain::Wall.is_opaque());
        assert!(Terrain::Tree.is_opaque());
        assert!(Terrain::DoorClosed.is_opaque());
        assert!(!Terrain::Floor.is_opaque());
        assert!(!Terrain::Corridor.is_opaque());
        assert!(!Terrain::DoorOpen.is_opaque());
        assert!(!Terrain::Air.is_opaque());
    }

    #[test]
    fn test_terrain_walkable() {
        assert!(Terrain::Floor.is_walkable());
        assert!(Terrain::Corridor.is_walkable());
        assert!(Terrain::DoorOpen.is_walkable());
        assert!(Terrain::StairsUp.is_walkable());
        assert!(Terrain::StairsDown.is_walkable());
        assert!(Terrain::Altar.is_walkable());
        assert!(Terrain::Fountain.is_walkable());
        assert!(Terrain::Throne.is_walkable());
        assert!(Terrain::Sink.is_walkable());
        assert!(Terrain::Grave.is_walkable());
        assert!(Terrain::Ice.is_walkable());
        assert!(Terrain::Air.is_walkable());
        assert!(Terrain::Cloud.is_walkable());
        assert!(Terrain::Drawbridge.is_walkable());
        assert!(Terrain::MagicPortal.is_walkable());

        assert!(!Terrain::Stone.is_walkable());
        assert!(!Terrain::Wall.is_walkable());
        assert!(!Terrain::DoorClosed.is_walkable());
        assert!(!Terrain::DoorLocked.is_walkable());
        assert!(!Terrain::Pool.is_walkable());
        assert!(!Terrain::Moat.is_walkable());
        assert!(!Terrain::Lava.is_walkable());
        assert!(!Terrain::Water.is_walkable());
        assert!(!Terrain::Tree.is_walkable());
        assert!(!Terrain::IronBars.is_walkable());
    }

    // ── LevelMap tests ───────────────────────────────────────────

    #[test]
    fn test_level_map_dimensions() {
        let map = LevelMap::new_standard();
        assert_eq!(map.width, 80);
        assert_eq!(map.height, 21);
        assert_eq!(map.dimensions(), (80, 21));
    }

    #[test]
    fn test_level_map_default_stone() {
        let map = LevelMap::new_standard();
        for y in 0..map.height {
            for x in 0..map.width {
                let cell = map.get(Position::new(x as i32, y as i32)).unwrap();
                assert_eq!(cell.terrain, Terrain::Stone);
                assert!(!cell.explored);
                assert!(!cell.visible);
            }
        }
    }

    #[test]
    fn test_level_map_set_and_get() {
        let mut map = LevelMap::new(10, 10);
        let pos = Position::new(5, 5);
        map.set_terrain(pos, Terrain::Floor);
        assert_eq!(map.get(pos).unwrap().terrain, Terrain::Floor);
    }

    #[test]
    fn test_level_map_bounds() {
        let map = LevelMap::new(10, 10);
        assert!(map.in_bounds(Position::new(0, 0)));
        assert!(map.in_bounds(Position::new(9, 9)));
        assert!(!map.in_bounds(Position::new(-1, 0)));
        assert!(!map.in_bounds(Position::new(0, -1)));
        assert!(!map.in_bounds(Position::new(10, 0)));
        assert!(!map.in_bounds(Position::new(0, 10)));
    }

    #[test]
    fn test_level_map_get_mut() {
        let mut map = LevelMap::new(10, 10);
        let pos = Position::new(3, 3);
        if let Some(cell) = map.get_mut(pos) {
            cell.terrain = Terrain::Fountain;
            cell.explored = true;
        }
        let cell = map.get(pos).unwrap();
        assert_eq!(cell.terrain, Terrain::Fountain);
        assert!(cell.explored);
    }

    #[test]
    fn test_level_map_out_of_bounds_returns_none() {
        let map = LevelMap::new(10, 10);
        assert!(map.get(Position::new(-1, 5)).is_none());
        assert!(map.get(Position::new(10, 5)).is_none());
        assert!(map.get(Position::new(5, 10)).is_none());
    }

    // ── Multiple portal tests ────────────────────────────────────

    #[test]
    fn test_multiple_portals() {
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        ds.add_portal(PortalLink {
            from_branch: DungeonBranch::Main,
            from_depth: 10,
            from_pos: Position::new(5, 5),
            to_branch: DungeonBranch::Quest,
            to_depth: 1,
            to_pos: Position::new(20, 10),
        });
        ds.add_portal(PortalLink {
            from_branch: DungeonBranch::Main,
            from_depth: 18,
            from_pos: Position::new(30, 8),
            to_branch: DungeonBranch::FortLudios,
            to_depth: 1,
            to_pos: Position::new(40, 10),
        });

        // Quest portal.
        let dest = ds.find_portal(DungeonBranch::Main, 10, Position::new(5, 5));
        assert_eq!(dest, Some((DungeonBranch::Quest, 1, Position::new(20, 10))));

        // Fort Ludios portal.
        let dest = ds.find_portal(DungeonBranch::Main, 18, Position::new(30, 8));
        assert_eq!(
            dest,
            Some((DungeonBranch::FortLudios, 1, Position::new(40, 10)))
        );

        // Wrong position.
        let dest = ds.find_portal(DungeonBranch::Main, 10, Position::new(6, 5));
        assert_eq!(dest, None);
    }

    // ── Cache multiple levels test ───────────────────────────────

    #[test]
    fn test_cache_multiple_levels() {
        let mut ds = DungeonState::with_entrance_depths(3, 3);

        // Cache Main:1
        ds.current_level
            .set_terrain(Position::new(1, 1), Terrain::Floor);
        ds.cache_current_level(vec![]);

        // Move to depth 2 and cache it too.
        ds.current_level = LevelMap::new_standard();
        ds.depth = 2;
        ds.current_level
            .set_terrain(Position::new(2, 2), Terrain::Altar);
        ds.cache_current_level(vec![]);

        // Verify both are cached.
        let (map1, _) = ds.load_cached_level(DungeonBranch::Main, 1).unwrap();
        assert_eq!(
            map1.get(Position::new(1, 1)).unwrap().terrain,
            Terrain::Floor
        );

        let (map2, _) = ds.load_cached_level(DungeonBranch::Main, 2).unwrap();
        assert_eq!(
            map2.get(Position::new(2, 2)).unwrap().terrain,
            Terrain::Altar
        );
    }

    // ── DungeonTopology tests ───────────────────────────────────

    #[test]
    fn test_topology_randomization() {
        use rand::SeedableRng;
        let mut rng = Pcg64::seed_from_u64(42);
        let topo = DungeonTopology::new(&mut rng);
        assert!(
            (5..=9).contains(&topo.oracle_depth),
            "oracle_depth {} out of range 5..=9",
            topo.oracle_depth
        );
        assert!(
            (15..=18).contains(&topo.rogue_depth),
            "rogue_depth {} out of range 15..=18",
            topo.rogue_depth
        );
        assert!(
            (10..=13).contains(&topo.bigroom_depth),
            "bigroom_depth {} out of range 10..=13",
            topo.bigroom_depth
        );
    }

    #[test]
    fn test_topology_randomization_many_seeds() {
        use rand::SeedableRng;
        for seed in 0..200 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let topo = DungeonTopology::new(&mut rng);
            assert!((5..=9).contains(&topo.oracle_depth));
            assert!((15..=18).contains(&topo.rogue_depth));
            assert!((10..=13).contains(&topo.bigroom_depth));
        }
    }

    #[test]
    fn test_topology_has_bigroom_variance() {
        use rand::SeedableRng;
        let mut has_count = 0;
        let trials = 200;
        for seed in 0..trials {
            let mut rng = Pcg64::seed_from_u64(seed);
            let topo = DungeonTopology::new(&mut rng);
            if topo.has_bigroom {
                has_count += 1;
            }
        }
        // Expect ~50% with generous margin.
        assert!(
            has_count > 50 && has_count < 150,
            "Expected ~50% has_bigroom, got {}/{}",
            has_count,
            trials
        );
    }

    #[test]
    fn test_topology_fixed() {
        let topo = DungeonTopology::fixed(7, 16, 11, false);
        assert_eq!(topo.oracle_depth, 7);
        assert_eq!(topo.rogue_depth, 16);
        assert_eq!(topo.bigroom_depth, 11);
        assert!(!topo.has_bigroom);
    }

    #[test]
    fn test_topology_default() {
        let topo = DungeonTopology::default();
        assert_eq!(topo.oracle_depth, 7);
        assert_eq!(topo.rogue_depth, 16);
        assert_eq!(topo.bigroom_depth, 11);
        assert!(!topo.has_bigroom);
    }

    // ── check_topology_special tests ────────────────────────────

    #[test]
    fn test_topology_oracle_at_randomized_depth() {
        use crate::special_levels::SpecialLevelId;
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        ds.topology = DungeonTopology::fixed(6, 16, 11, false);
        // Oracle should be at depth 6.
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Main, 6),
            Some(SpecialLevelId::OracleLevel),
        );
        // Not at depth 7 (the default).
        assert_eq!(ds.check_topology_special(&DungeonBranch::Main, 7), None,);
    }

    #[test]
    fn test_topology_rogue_at_randomized_depth() {
        use crate::special_levels::SpecialLevelId;
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        ds.topology = DungeonTopology::fixed(7, 17, 11, false);
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Main, 17),
            Some(SpecialLevelId::Rogue),
        );
        assert_eq!(ds.check_topology_special(&DungeonBranch::Main, 16), None,);
    }

    #[test]
    fn test_topology_bigroom_with_flag() {
        use crate::special_levels::SpecialLevelId;
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        // With bigroom enabled.
        ds.topology = DungeonTopology::fixed(7, 16, 12, true);
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Main, 12),
            Some(SpecialLevelId::BigRoom(0)),
        );
        // With bigroom disabled, depth 12 should be None.
        ds.topology = DungeonTopology::fixed(7, 16, 12, false);
        assert_eq!(ds.check_topology_special(&DungeonBranch::Main, 12), None,);
    }

    #[test]
    fn test_topology_castle_always_at_25() {
        use crate::special_levels::SpecialLevelId;
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        ds.topology = DungeonTopology::fixed(7, 16, 11, false);
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Main, 25),
            Some(SpecialLevelId::Castle),
        );
    }

    #[test]
    fn test_topology_medusa_always_at_24() {
        use crate::special_levels::SpecialLevelId;
        let mut ds = DungeonState::with_entrance_depths(3, 3);
        ds.topology = DungeonTopology::fixed(7, 16, 11, false);
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Main, 24),
            Some(SpecialLevelId::Medusa(0)),
        );
    }

    #[test]
    fn test_topology_gehennom_uses_identify() {
        use crate::special_levels::SpecialLevelId;
        let ds = DungeonState::with_entrance_depths(3, 3);
        // Gehennom delegates to identify_special_level.
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Gehennom, 1),
            Some(SpecialLevelId::Valley),
        );
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Gehennom, 17),
            Some(SpecialLevelId::WizardTower),
        );
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Gehennom, 20),
            Some(SpecialLevelId::Sanctum),
        );
    }

    #[test]
    fn test_topology_sokoban_delegates() {
        use crate::special_levels::SpecialLevelId;
        let ds = DungeonState::with_entrance_depths(3, 3);
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Sokoban, 1),
            Some(SpecialLevelId::Sokoban(1)),
        );
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Sokoban, 4),
            Some(SpecialLevelId::Sokoban(4)),
        );
    }

    // ── CurrentLevelFlags tests ─────────────────────────────────

    #[test]
    fn test_current_level_flags_default() {
        let flags = CurrentLevelFlags::default();
        assert!(!flags.no_dig);
        assert!(!flags.no_teleport);
        assert!(!flags.no_prayer);
        assert!(!flags.is_endgame);
    }

    #[test]
    fn test_current_level_flags_from_special() {
        let special = crate::special_levels::SpecialLevelFlags {
            no_dig: true,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        };
        let flags: CurrentLevelFlags = special.into();
        assert!(flags.no_dig);
        assert!(flags.no_teleport);
        assert!(!flags.no_prayer);
        assert!(!flags.is_endgame);
    }

    // ── Integration: full dungeon traversal tests ───────────────

    #[test]
    fn test_full_dungeon_traversal_main_to_castle() {
        use crate::special_levels::SpecialLevelId;
        let mut ds = DungeonState::with_entrance_depths(4, 3);
        ds.topology = DungeonTopology::fixed(7, 16, 11, false);
        // Simulate descending from depth 1 to 25.
        for depth in 1..=25 {
            let id = ds.check_topology_special(&DungeonBranch::Main, depth);
            match depth {
                7 => assert_eq!(id, Some(SpecialLevelId::OracleLevel)),
                16 => assert_eq!(id, Some(SpecialLevelId::Rogue)),
                24 => assert_eq!(id, Some(SpecialLevelId::Medusa(0))),
                25 => assert_eq!(id, Some(SpecialLevelId::Castle)),
                _ => assert_eq!(id, None, "unexpected special at depth {}", depth),
            }
        }
    }

    #[test]
    fn test_mines_branch_entry_and_sokoban() {
        let ds = DungeonState::with_entrance_depths(4, 3);
        // Mines entrance at depth 4.
        assert_eq!(
            check_branch_entrance(DungeonBranch::Main, 4, 4, 3),
            Some((DungeonBranch::Mines, 1)),
        );
        // Sokoban entrance from Mines depth 3.
        assert_eq!(
            check_branch_entrance(DungeonBranch::Mines, 3, 4, 3),
            Some((DungeonBranch::Sokoban, 1)),
        );
        // All 4 Sokoban levels are special.
        for d in 1..=4 {
            assert!(
                ds.check_topology_special(&DungeonBranch::Sokoban, d)
                    .is_some(),
                "Sokoban depth {} should be special",
                d,
            );
        }
    }

    #[test]
    fn test_gehennom_traversal() {
        use crate::special_levels::SpecialLevelId;
        let ds = DungeonState::with_entrance_depths(3, 3);
        // Verify all known Gehennom special levels.
        let expected = vec![
            (1, SpecialLevelId::Valley),
            (5, SpecialLevelId::Juiblex),
            (7, SpecialLevelId::Asmodeus),
            (10, SpecialLevelId::Baalzebub),
            (12, SpecialLevelId::Orcus),
            (14, SpecialLevelId::FakeWizard(1)),
            (15, SpecialLevelId::FakeWizard(2)),
            (17, SpecialLevelId::WizardTower),
            (18, SpecialLevelId::WizardTower2),
            (19, SpecialLevelId::WizardTower3),
            (20, SpecialLevelId::Sanctum),
        ];
        for (depth, expected_id) in &expected {
            assert_eq!(
                ds.check_topology_special(&DungeonBranch::Gehennom, *depth),
                Some(*expected_id),
                "Gehennom depth {} should be {:?}",
                depth,
                expected_id,
            );
        }
        // Non-special depths in Gehennom.
        for depth in [2, 3, 4, 6, 8, 9, 11, 13, 16] {
            assert_eq!(
                ds.check_topology_special(&DungeonBranch::Gehennom, depth),
                None,
                "Gehennom depth {} should not be special",
                depth,
            );
        }
    }

    #[test]
    fn test_endgame_planes() {
        use crate::special_levels::SpecialLevelId;
        let ds = DungeonState::with_entrance_depths(3, 3);
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Endgame, 1),
            Some(SpecialLevelId::EarthPlane),
        );
        assert_eq!(
            ds.check_topology_special(&DungeonBranch::Endgame, 5),
            Some(SpecialLevelId::AstralPlane),
        );
    }

    #[test]
    fn test_topology_stored_in_dungeon_state() {
        use rand::SeedableRng;
        let mut rng = Pcg64::seed_from_u64(99);
        let ds = DungeonState::with_rng(&mut rng);
        // Topology should be initialized.
        assert!((5..=9).contains(&ds.topology.oracle_depth));
        assert!((15..=18).contains(&ds.topology.rogue_depth));
        assert!((10..=13).contains(&ds.topology.bigroom_depth));
    }

    // ── Stairs constraints ──────────────────────────────────

    #[test]
    fn stairs_allowed_normal_dungeon() {
        assert_eq!(
            stairs_allowed(DungeonBranch::Main, 5, true, false),
            StairsResult::Allowed,
        );
        assert_eq!(
            stairs_allowed(DungeonBranch::Main, 5, false, false),
            StairsResult::Allowed,
        );
        assert_eq!(
            stairs_allowed(DungeonBranch::Mines, 3, true, false),
            StairsResult::Allowed,
        );
    }

    #[test]
    fn stairs_blocked_sokoban_down() {
        let result = stairs_allowed(DungeonBranch::Sokoban, 2, false, false);
        assert!(matches!(result, StairsResult::Blocked { .. }));
        if let StairsResult::Blocked { reason } = result {
            assert!(reason.contains("upward"));
        }
    }

    #[test]
    fn stairs_blocked_sokoban_boulder() {
        let result = stairs_allowed(DungeonBranch::Sokoban, 2, true, true);
        assert!(matches!(result, StairsResult::Blocked { .. }));
        if let StairsResult::Blocked { reason } = result {
            assert!(reason.contains("boulder"));
        }
    }

    #[test]
    fn stairs_allowed_sokoban_up_no_boulder() {
        assert_eq!(
            stairs_allowed(DungeonBranch::Sokoban, 2, true, false),
            StairsResult::Allowed,
        );
    }

    #[test]
    fn stairs_quest_exit_confirmation() {
        let result = stairs_allowed(DungeonBranch::Quest, 1, true, false);
        assert!(matches!(result, StairsResult::NeedsConfirmation { .. }));
    }

    #[test]
    fn stairs_quest_deeper_allowed() {
        assert_eq!(
            stairs_allowed(DungeonBranch::Quest, 2, false, false),
            StairsResult::Allowed,
        );
        assert_eq!(
            stairs_allowed(DungeonBranch::Quest, 3, true, false),
            StairsResult::Allowed,
        );
    }

    #[test]
    fn stairs_endgame_no_going_back() {
        let result = stairs_allowed(DungeonBranch::Endgame, 1, true, false);
        assert!(matches!(result, StairsResult::Blocked { .. }));
        if let StairsResult::Blocked { reason } = result {
            assert!(reason.contains("no turning back"));
        }
    }

    #[test]
    fn stairs_endgame_forward_allowed() {
        assert_eq!(
            stairs_allowed(DungeonBranch::Endgame, 1, false, false),
            StairsResult::Allowed,
        );
    }
}
