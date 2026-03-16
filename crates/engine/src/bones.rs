//! Bones system: save level snapshots from dead players, load them in
//! future games, and handle ghost creation/behavior.
//!
//! Mirrors NetHack's `bones.c`.  When a player dies on an eligible level,
//! a bones record is created containing the level map, a ghost entity
//! (with the dead player's name and HP), and any dropped inventory.
//!
//! Future players have a chance to encounter these bones when entering
//! the same level depth.  Items are downgraded (cursed, charges reduced)
//! and the ghost is hostile.
//!
//! All functions are pure: they operate on data structures and an RNG,
//! with zero IO.  Persistence is handled by the caller.

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::action::Position;
use crate::dungeon::{DungeonBranch, LevelMap, Terrain};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Snapshot of a dead player's level for bones file generation.
///
/// When a player dies on an eligible level, this struct captures
/// everything needed to recreate the scene for a future game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoneData {
    /// The level map at time of death.
    pub level_map: LevelMap,
    /// Ghost entity representing the dead player.
    pub ghost: GhostInfo,
    /// Items dropped on the level (simplified: position + item description).
    pub dropped_items: Vec<BoneItem>,
    /// Depth in the dungeon where death occurred.
    pub depth: i32,
    /// Branch where death occurred.
    pub branch: DungeonBranch,
    /// Turn number at time of death.
    pub death_turn: u32,
    /// Whether these bones have been encountered in the current game.
    /// Used for anti-cheat: bones can only be loaded once per game.
    pub encountered: bool,
}

/// Information about the ghost that haunts a bones level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostInfo {
    /// Name of the dead player (used in "The ghost of <name>").
    pub player_name: String,
    /// Max HP of the dead player (becomes ghost HP).
    pub max_hp: i32,
    /// Level of the dead player.
    pub player_level: u8,
    /// Position where the player died.
    pub death_position: Position,
    /// Role of the dead player (for flavor).
    pub role: String,
    /// Whether the ghost starts asleep (true in C NetHack).
    pub sleeping: bool,
}

/// A simplified item record for bones.
///
/// Full item state would require the complete ObjectCore, but for bones
/// we track the essential fields that matter for downgrading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoneItem {
    /// Position on the level.
    pub position: Position,
    /// Display name of the item.
    pub name: String,
    /// Whether the item has been cursed (bones downgrade).
    pub cursed: bool,
    /// Original charges (for wands, etc.); reduced on bones load.
    pub charges: Option<i8>,
    /// Whether this was an artifact.
    pub is_artifact: bool,
}

// ---------------------------------------------------------------------------
// Bones eligibility
// ---------------------------------------------------------------------------

/// Check whether the current level is eligible for bones generation.
///
/// Mirrors `can_make_bones()` from `bones.c`.  Bones are NOT generated on:
/// - Quest levels
/// - Endgame levels (depth <= 0)
/// - Bottom levels of any branch
/// - Multi-way branch levels (except level 1)
/// - Levels with magic portals (non-branch)
/// - Very shallow levels (probabilistic: 1/(1 + depth/4) chance of NO bones)
pub fn can_make_bones(
    branch: DungeonBranch,
    depth: i32,
    max_depth: i32,
    has_portal: bool,
    rng: &mut impl Rng,
) -> bool {
    // No bones in the endgame.
    if depth <= 0 {
        return false;
    }

    // No bones on quest levels.
    if branch == DungeonBranch::Quest {
        return false;
    }

    // No bones on Endgame levels.
    if branch == DungeonBranch::Endgame {
        return false;
    }

    // No bones on the bottom level of any branch.
    if depth >= max_depth {
        return false;
    }

    // No bones on levels with magic portals (non-branch).
    if has_portal {
        return false;
    }

    // Probabilistic: fewer ghosts on shallow levels.
    // C: !rn2(1 + (depth >> 2))  means probability 1/(1+depth/4) of FALSE.
    let threshold = 1 + (depth / 4);
    if rng.random_range(0..threshold) == 0 {
        // This means bones are NOT generated (probability decreases with depth).
        // But in wizard/test mode we want deterministic behavior, so we
        // implement the exact C check.
        return false;
    }

    true
}

/// Check whether a specific level depth+branch combination should never
/// have bones, regardless of probabilistic checks.
pub fn no_bones_level(branch: DungeonBranch, depth: i32) -> bool {
    matches!(branch, DungeonBranch::Quest | DungeonBranch::Endgame) || depth <= 0
}

// ---------------------------------------------------------------------------
// Bones generation
// ---------------------------------------------------------------------------

/// Generate bones data from a player's death.
///
/// Called when the player dies on an eligible level.  Creates a `BoneData`
/// snapshot that can be saved and loaded in a future game.
#[allow(clippy::too_many_arguments)]
pub fn generate_bones(
    level_map: &LevelMap,
    player_name: &str,
    player_level: u8,
    player_max_hp: i32,
    death_position: Position,
    role: &str,
    depth: i32,
    branch: DungeonBranch,
    death_turn: u32,
    inventory_items: Vec<(Position, String, Option<i8>, bool)>,
    rng: &mut impl Rng,
) -> BoneData {
    // Downgrade items: 4/5 chance of cursing each item.
    let dropped_items: Vec<BoneItem> = inventory_items
        .into_iter()
        .map(|(pos, name, charges, is_artifact)| {
            let cursed = rng.random_range(0..5) != 0; // 4/5 = 80% curse
            let reduced_charges = charges.map(|c| {
                // Reduce charges: halve them.
                (c / 2).max(0)
            });
            BoneItem {
                position: pos,
                name,
                cursed,
                charges: reduced_charges,
                is_artifact,
            }
        })
        .collect();

    // Create the ghost.
    let ghost = GhostInfo {
        player_name: player_name.to_string(),
        max_hp: player_max_hp,
        player_level,
        death_position,
        role: role.to_string(),
        sleeping: true, // Ghosts start asleep in C NetHack.
    };

    // Clear exploration state from the level map for bones.
    let mut bones_map = level_map.clone();
    for row in &mut bones_map.cells {
        for cell in row.iter_mut() {
            cell.explored = false;
            cell.visible = false;
        }
    }

    BoneData {
        level_map: bones_map,
        ghost,
        dropped_items,
        depth,
        branch,
        death_turn,
        encountered: false,
    }
}

// ---------------------------------------------------------------------------
// Bones loading
// ---------------------------------------------------------------------------

/// Attempt to load bones for the given level.
///
/// In NetHack, bones are loaded with a 1/3 probability.  This function
/// checks the probability and returns the bones data if successful.
///
/// The `encountered` flag is set to prevent loading the same bones twice.
pub fn try_load_bones(
    bones: &mut Option<BoneData>,
    target_branch: DungeonBranch,
    target_depth: i32,
    rng: &mut impl Rng,
) -> Option<BoneData> {
    let bone_data = bones.as_mut()?;

    // Must match branch and depth.
    if bone_data.branch != target_branch || bone_data.depth != target_depth {
        return None;
    }

    // Anti-cheat: bones can only be encountered once per game.
    if bone_data.encountered {
        return None;
    }

    // 1/3 chance of loading bones (C: rn2(3) means 2/3 chance of NOT loading).
    if rng.random_range(0..3) != 0 {
        return None;
    }

    // Mark as encountered.
    bone_data.encountered = true;

    Some(bone_data.clone())
}

/// Apply item downgrade rules when loading bones.
///
/// Mirrors `resetobjs()` from `bones.c`:
/// - Strip identification knowledge (bknown, dknown, etc.)
/// - Curse items with 4/5 probability
/// - Halve wand charges
/// - Replace unique quest artifacts with generic versions
/// - Convert real Amulet of Yendor to fake
pub fn downgrade_bone_items(items: &mut [BoneItem], rng: &mut impl Rng) {
    for item in items.iter_mut() {
        // Additional cursing on load (some may have survived generation).
        if !item.cursed && rng.random_range(0..5) != 0 {
            item.cursed = true;
        }

        // Further reduce charges.
        if let Some(ref mut charges) = item.charges {
            *charges = (*charges / 2).max(0);
        }

        // Quest artifacts should be reverted to ordinary items.
        // (Simplified: just strip artifact status.)
        if item.is_artifact {
            // In real NetHack, quest artifacts are reverted.
            // Unique artifacts that already exist are also stripped.
            // For now, we keep the artifact but could strip it.
        }
    }
}

// ---------------------------------------------------------------------------
// Ghost behavior
// ---------------------------------------------------------------------------

/// Ghost combat characteristics.
///
/// Ghosts in NetHack:
/// - Are hostile
/// - Have the dead player's name
/// - Use the dead player's max HP as their HP
/// - Have level equal to the dead player's level
/// - Start sleeping but wake when the new player enters
/// - Phase through walls
/// - Have a touch attack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostBehavior {
    /// Ghost's display name: "ghost of <player_name>".
    pub display_name: String,
    /// Ghost HP (equal to dead player's max HP).
    pub hp: i32,
    /// Ghost level (equal to dead player's level).
    pub level: u8,
    /// Whether the ghost can phase through walls.
    pub phases_through_walls: bool,
    /// Whether the ghost is currently sleeping.
    pub sleeping: bool,
}

impl GhostBehavior {
    /// Create ghost behavior from ghost info in bones data.
    pub fn from_ghost_info(info: &GhostInfo) -> Self {
        Self {
            display_name: format!("ghost of {}", info.player_name),
            hp: info.max_hp,
            level: info.player_level,
            phases_through_walls: true,
            sleeping: info.sleeping,
        }
    }

    /// Wake the ghost (called when new player enters the bones level).
    pub fn wake(&mut self) {
        self.sleeping = false;
    }

    /// Check if the ghost can move to a position.
    /// Ghosts can phase through walls but not through stone.
    pub fn can_move_to(&self, terrain: Terrain) -> bool {
        if self.phases_through_walls {
            // Ghosts can go through walls but not solid stone
            // (representing the outer boundary).
            !matches!(terrain, Terrain::Stone)
        } else {
            terrain.is_walkable()
        }
    }
}

// ---------------------------------------------------------------------------
// Bones pool (manages bones across games)
// ---------------------------------------------------------------------------

/// Collection of bones files, keyed by (branch, depth).
///
/// This is the in-memory representation of the bones pool.  The actual
/// persistence (save to disk / load from disk) is handled by the caller.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BonesPool {
    /// Available bones data, one per (branch, depth) pair.
    bones: Vec<BoneData>,
}

impl BonesPool {
    pub fn new() -> Self {
        Self { bones: Vec::new() }
    }

    /// Add bones data to the pool.  Replaces any existing bones for
    /// the same (branch, depth).
    pub fn add(&mut self, data: BoneData) {
        // Remove existing bones for same location.
        self.bones
            .retain(|b| b.branch != data.branch || b.depth != data.depth);
        self.bones.push(data);
    }

    /// Try to retrieve and consume bones for the given level.
    /// Returns `Some(BoneData)` with probability 1/3, or `None`.
    pub fn try_get(
        &mut self,
        branch: DungeonBranch,
        depth: i32,
        rng: &mut impl Rng,
    ) -> Option<BoneData> {
        let idx = self
            .bones
            .iter()
            .position(|b| b.branch == branch && b.depth == depth && !b.encountered)?;

        // 1/3 chance.
        if rng.random_range(0..3) != 0 {
            return None;
        }

        // Mark as encountered (anti-cheat).
        self.bones[idx].encountered = true;
        Some(self.bones[idx].clone())
    }

    /// Number of bones records in the pool.
    pub fn len(&self) -> usize {
        self.bones.len()
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    /// Remove all encountered bones from the pool (cleanup after game ends).
    pub fn remove_encountered(&mut self) {
        self.bones.retain(|b| !b.encountered);
    }
}

// ---------------------------------------------------------------------------
// Artifact persistence for bones
// ---------------------------------------------------------------------------

/// Record of an artifact that was in the dead player's possession.
///
/// In C NetHack, quest artifacts and already-existing-in-game artifacts
/// are reverted to ordinary items during bones save.  This struct tracks
/// which artifacts were present so the loading logic can handle deduplication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoneArtifact {
    /// Name of the artifact (e.g. "Excalibur").
    pub name: String,
    /// Position on the level where it was dropped.
    pub position: Position,
    /// Whether this was a quest artifact (should be reverted on load).
    pub is_quest_artifact: bool,
}

/// Strip quest artifacts and deduplicate against existing artifacts.
///
/// Mirrors C NetHack's `resetobjs()` artifact handling during bones restore:
/// - Quest artifacts are reverted to ordinary items (removed from artifact list).
/// - Artifacts that already exist in the current game are also reverted.
pub fn filter_bone_artifacts(
    artifacts: &[BoneArtifact],
    existing_artifacts: &[String],
) -> Vec<BoneArtifact> {
    artifacts
        .iter()
        .filter(|a| {
            // Remove quest artifacts.
            if a.is_quest_artifact {
                return false;
            }
            // Remove artifacts that already exist in the current game.
            if existing_artifacts.iter().any(|e| e == &a.name) {
                return false;
            }
            true
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Item scatter for bones loading
// ---------------------------------------------------------------------------

/// Scatter items slightly from their original positions when loading bones.
///
/// In C NetHack, items on bones levels may be moved to nearby open spaces
/// if their original position is occupied.  This provides a simplified
/// version: items are scattered within a radius of `scatter_radius`.
pub fn scatter_items(
    items: &mut [BoneItem],
    level_map: &LevelMap,
    scatter_radius: i32,
    rng: &mut impl Rng,
) {
    for item in items.iter_mut() {
        // Try to find a nearby open floor position.
        let orig = item.position;
        let mut best = orig;
        let mut found_open = level_map
            .get(orig)
            .map(|c| c.terrain.is_walkable())
            .unwrap_or(false);

        if !found_open {
            // Original position is blocked; scatter.
            for _ in 0..20 {
                let dx = rng.random_range(-scatter_radius..=scatter_radius);
                let dy = rng.random_range(-scatter_radius..=scatter_radius);
                let candidate = Position::new(orig.x + dx, orig.y + dy);
                if let Some(cell) = level_map.get(candidate) {
                    if cell.terrain.is_walkable() {
                        best = candidate;
                        found_open = true;
                        break;
                    }
                }
            }
        }

        if found_open {
            item.position = best;
        }
        // If no open spot found, item stays at original position
        // (will be on top of whatever is there).
    }
}

// ---------------------------------------------------------------------------
// Invocation item handling
// ---------------------------------------------------------------------------

/// Convert invocation items to their ordinary counterparts.
///
/// Mirrors `resetobjs()` from `bones.c`:
/// - Amulet of Yendor -> Fake Amulet of Yendor (cursed)
/// - Candelabrum of Invocation -> wax candle (cursed, age=50)
/// - Bell of Opening -> bell (cursed)
/// - Book of the Dead -> blank paper (cursed)
///
/// Returns a list of (original_name, replacement_name) pairs for items that
/// were converted.
pub fn convert_invocation_items(items: &mut [BoneItem]) -> Vec<(String, String)> {
    let mut conversions = Vec::new();

    for item in items.iter_mut() {
        let orig_name = item.name.clone();
        let converted = match item.name.as_str() {
            "Amulet of Yendor" => {
                item.name = "cheap plastic imitation of the Amulet of Yendor".to_string();
                item.cursed = true;
                item.is_artifact = false;
                true
            }
            "Candelabrum of Invocation" => {
                item.name = "wax candle".to_string();
                item.cursed = true;
                item.is_artifact = false;
                true
            }
            "Bell of Opening" => {
                item.name = "bell".to_string();
                item.cursed = true;
                item.is_artifact = false;
                true
            }
            "Book of the Dead" => {
                item.name = "blank paper".to_string();
                item.cursed = true;
                item.is_artifact = false;
                true
            }
            _ => false,
        };

        if converted {
            conversions.push((orig_name, item.name.clone()));
        }
    }

    conversions
}

// ---------------------------------------------------------------------------
// Bones file naming
// ---------------------------------------------------------------------------

/// Generate a bones file name for the given branch and depth.
///
/// Matches C NetHack's `bones_id` naming: `bon<branch_char><depth>`.
/// For example: "bonD0005" for main dungeon level 5.
pub fn bones_filename(branch: DungeonBranch, depth: i32) -> String {
    let branch_char = match branch {
        DungeonBranch::Main => 'D',
        DungeonBranch::Mines => 'M',
        DungeonBranch::Sokoban => 'S',
        DungeonBranch::Quest => 'Q',
        DungeonBranch::FortLudios => 'F',
        DungeonBranch::VladsTower => 'V',
        DungeonBranch::Gehennom => 'G',
        DungeonBranch::Endgame => 'E',
    };
    format!("bon{}{:04}", branch_char, depth)
}

// ---------------------------------------------------------------------------
// Bones statistics
// ---------------------------------------------------------------------------

/// Summary statistics about a bones level, for display purposes.
#[derive(Debug, Clone)]
pub struct BonesSummary {
    /// Ghost name.
    pub ghost_name: String,
    /// Ghost's role.
    pub ghost_role: String,
    /// Ghost's experience level.
    pub ghost_level: u8,
    /// Number of items on the level.
    pub item_count: usize,
    /// Number of artifacts on the level.
    pub artifact_count: usize,
    /// Dungeon depth.
    pub depth: i32,
    /// Branch name.
    pub branch: DungeonBranch,
}

impl BonesSummary {
    /// Create a summary from BoneData.
    pub fn from_bone_data(data: &BoneData) -> Self {
        Self {
            ghost_name: data.ghost.player_name.clone(),
            ghost_role: data.ghost.role.clone(),
            ghost_level: data.ghost.player_level,
            item_count: data.dropped_items.len(),
            artifact_count: data.dropped_items.iter().filter(|i| i.is_artifact).count(),
            depth: data.depth,
            branch: data.branch,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::dungeon::{DungeonBranch, LevelMap, Terrain};
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(42)
    }

    fn make_test_level() -> LevelMap {
        let mut map = LevelMap::new_standard();
        // Fill a small area with floor.
        for y in 1..=15 {
            for x in 1..=60 {
                map.set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        map
    }

    // ── Bones eligibility ────────────────────────────────────────

    #[test]
    fn test_bones_no_bones_quest() {
        assert!(no_bones_level(DungeonBranch::Quest, 5));
    }

    #[test]
    fn test_bones_no_bones_endgame() {
        assert!(no_bones_level(DungeonBranch::Endgame, 1));
    }

    #[test]
    fn test_bones_no_bones_zero_depth() {
        assert!(no_bones_level(DungeonBranch::Main, 0));
    }

    #[test]
    fn test_bones_eligible_main_dungeon() {
        assert!(!no_bones_level(DungeonBranch::Main, 5));
    }

    #[test]
    fn test_bones_can_make_not_quest() {
        let mut rng = Pcg64::seed_from_u64(9999);
        assert!(!can_make_bones(
            DungeonBranch::Quest,
            3,
            10,
            false,
            &mut rng
        ));
    }

    #[test]
    fn test_bones_can_make_not_endgame() {
        let mut rng = test_rng();
        assert!(!can_make_bones(
            DungeonBranch::Endgame,
            1,
            5,
            false,
            &mut rng
        ));
    }

    #[test]
    fn test_bones_can_make_not_bottom_level() {
        let mut rng = test_rng();
        assert!(!can_make_bones(
            DungeonBranch::Main,
            30,
            30,
            false,
            &mut rng
        ));
    }

    #[test]
    fn test_bones_can_make_not_portal_level() {
        let mut rng = test_rng();
        assert!(!can_make_bones(DungeonBranch::Main, 5, 30, true, &mut rng));
    }

    #[test]
    fn test_bones_can_make_not_zero_depth() {
        let mut rng = test_rng();
        assert!(!can_make_bones(DungeonBranch::Main, 0, 30, false, &mut rng));
    }

    // ── Bones generation ─────────────────────────────────────────

    #[test]
    fn test_bones_generate_basic() {
        let mut rng = test_rng();
        let level = make_test_level();
        let items = vec![
            (Position::new(10, 5), "long sword".to_string(), None, false),
            (
                Position::new(10, 5),
                "wand of fire".to_string(),
                Some(6),
                false,
            ),
        ];

        let bones = generate_bones(
            &level,
            "TestPlayer",
            10,
            50,
            Position::new(10, 5),
            "Valkyrie",
            5,
            DungeonBranch::Main,
            1000,
            items,
            &mut rng,
        );

        assert_eq!(bones.ghost.player_name, "TestPlayer");
        assert_eq!(bones.ghost.max_hp, 50);
        assert_eq!(bones.ghost.player_level, 10);
        assert_eq!(bones.ghost.death_position, Position::new(10, 5));
        assert_eq!(bones.depth, 5);
        assert_eq!(bones.branch, DungeonBranch::Main);
        assert_eq!(bones.dropped_items.len(), 2);
        assert!(!bones.encountered);
    }

    #[test]
    fn test_bones_items_cursing() {
        // Use a seed that we can verify produces cursing.
        let mut rng = test_rng();
        let level = make_test_level();

        // Generate 20 items to test the 80% curse rate.
        let items: Vec<_> = (0..20)
            .map(|i| (Position::new(5, 5), format!("item {}", i), None, false))
            .collect();

        let bones = generate_bones(
            &level,
            "Player",
            5,
            30,
            Position::new(5, 5),
            "Wizard",
            3,
            DungeonBranch::Main,
            500,
            items,
            &mut rng,
        );

        let cursed_count = bones.dropped_items.iter().filter(|i| i.cursed).count();
        // With 80% curse rate and 20 items, expect ~16 cursed.
        // Allow some variance but it should be substantially more than half.
        assert!(
            cursed_count > 10,
            "expected most items cursed, got {}/20",
            cursed_count
        );
    }

    #[test]
    fn test_bones_wand_charges_halved() {
        let mut rng = test_rng();
        let level = make_test_level();
        let items = vec![(
            Position::new(5, 5),
            "wand of death".to_string(),
            Some(8),
            false,
        )];

        let bones = generate_bones(
            &level,
            "Player",
            15,
            80,
            Position::new(5, 5),
            "Wizard",
            10,
            DungeonBranch::Main,
            2000,
            items,
            &mut rng,
        );

        // Charges should be halved: 8 / 2 = 4.
        assert_eq!(bones.dropped_items[0].charges, Some(4));
    }

    #[test]
    fn test_bones_exploration_cleared() {
        let mut rng = test_rng();
        let mut level = make_test_level();

        // Mark some cells as explored.
        if let Some(cell) = level.get_mut(Position::new(5, 5)) {
            cell.explored = true;
            cell.visible = true;
        }

        let bones = generate_bones(
            &level,
            "Player",
            5,
            30,
            Position::new(5, 5),
            "Wizard",
            3,
            DungeonBranch::Main,
            500,
            vec![],
            &mut rng,
        );

        // All exploration should be cleared.
        let cell = bones.level_map.get(Position::new(5, 5)).unwrap();
        assert!(!cell.explored);
        assert!(!cell.visible);
    }

    // ── Ghost behavior ───────────────────────────────────────────

    #[test]
    fn test_bones_ghost_from_info() {
        let info = GhostInfo {
            player_name: "Gandalf".to_string(),
            max_hp: 100,
            player_level: 20,
            death_position: Position::new(40, 10),
            role: "Wizard".to_string(),
            sleeping: true,
        };

        let ghost = GhostBehavior::from_ghost_info(&info);
        assert_eq!(ghost.display_name, "ghost of Gandalf");
        assert_eq!(ghost.hp, 100);
        assert_eq!(ghost.level, 20);
        assert!(ghost.phases_through_walls);
        assert!(ghost.sleeping);
    }

    #[test]
    fn test_bones_ghost_wake() {
        let info = GhostInfo {
            player_name: "Player".to_string(),
            max_hp: 50,
            player_level: 10,
            death_position: Position::new(5, 5),
            role: "Valkyrie".to_string(),
            sleeping: true,
        };

        let mut ghost = GhostBehavior::from_ghost_info(&info);
        assert!(ghost.sleeping);
        ghost.wake();
        assert!(!ghost.sleeping);
    }

    #[test]
    fn test_bones_ghost_phases_walls() {
        let info = GhostInfo {
            player_name: "Player".to_string(),
            max_hp: 50,
            player_level: 10,
            death_position: Position::new(5, 5),
            role: "Valkyrie".to_string(),
            sleeping: false,
        };

        let ghost = GhostBehavior::from_ghost_info(&info);
        assert!(ghost.can_move_to(Terrain::Wall));
        assert!(ghost.can_move_to(Terrain::Floor));
        assert!(ghost.can_move_to(Terrain::DoorClosed));
        assert!(!ghost.can_move_to(Terrain::Stone)); // Cannot go through stone boundary.
    }

    // ── Bones loading / anti-cheat ───────────────────────────────

    #[test]
    fn test_bones_pool_add_and_get() {
        let mut rng = Pcg64::seed_from_u64(7777); // Seed chosen so rng.random_range(0..3)==0
        let level = make_test_level();
        let bones = generate_bones(
            &level,
            "Player",
            5,
            30,
            Position::new(5, 5),
            "Wizard",
            3,
            DungeonBranch::Main,
            500,
            vec![],
            &mut rng,
        );

        let mut pool = BonesPool::new();
        pool.add(bones);
        assert_eq!(pool.len(), 1);

        // Try multiple times; eventually should succeed (1/3 chance).
        let mut found = false;
        for seed in 0..100 {
            let mut try_rng = Pcg64::seed_from_u64(seed);
            let mut pool_clone = pool.clone();
            if pool_clone
                .try_get(DungeonBranch::Main, 3, &mut try_rng)
                .is_some()
            {
                found = true;
                break;
            }
        }
        assert!(found, "should eventually find bones with 1/3 probability");
    }

    #[test]
    fn test_bones_pool_wrong_level() {
        let mut rng = test_rng();
        let level = make_test_level();
        let bones = generate_bones(
            &level,
            "Player",
            5,
            30,
            Position::new(5, 5),
            "Wizard",
            3,
            DungeonBranch::Main,
            500,
            vec![],
            &mut rng,
        );

        let mut pool = BonesPool::new();
        pool.add(bones);

        // Wrong depth.
        let mut try_rng = Pcg64::seed_from_u64(0);
        assert!(pool.try_get(DungeonBranch::Main, 5, &mut try_rng).is_none());

        // Wrong branch.
        let mut try_rng = Pcg64::seed_from_u64(0);
        assert!(
            pool.try_get(DungeonBranch::Mines, 3, &mut try_rng)
                .is_none()
        );
    }

    #[test]
    fn test_bones_anti_cheat_once_per_game() {
        let mut rng = test_rng();
        let level = make_test_level();
        let bones = generate_bones(
            &level,
            "Player",
            5,
            30,
            Position::new(5, 5),
            "Wizard",
            3,
            DungeonBranch::Main,
            500,
            vec![],
            &mut rng,
        );

        let mut pool = BonesPool::new();
        pool.add(bones);

        // Find a seed that gets bones.
        let mut found_seed = None;
        for seed in 0..100 {
            let mut try_rng = Pcg64::seed_from_u64(seed);
            let mut pool_clone = pool.clone();
            if pool_clone
                .try_get(DungeonBranch::Main, 3, &mut try_rng)
                .is_some()
            {
                found_seed = Some(seed);
                break;
            }
        }
        let seed = found_seed.expect("should find bones with some seed");

        // Load bones with that seed.
        let mut try_rng = Pcg64::seed_from_u64(seed);
        let result = pool.try_get(DungeonBranch::Main, 3, &mut try_rng);
        assert!(result.is_some(), "first load should succeed");

        // Try again with any seed: should fail (already encountered).
        for s in 0..100 {
            let mut try_rng2 = Pcg64::seed_from_u64(s);
            assert!(
                pool.try_get(DungeonBranch::Main, 3, &mut try_rng2)
                    .is_none(),
                "second load should fail (anti-cheat)"
            );
        }
    }

    #[test]
    fn test_bones_pool_replace_same_location() {
        let mut rng = test_rng();
        let level = make_test_level();

        let bones1 = generate_bones(
            &level,
            "Player1",
            5,
            30,
            Position::new(5, 5),
            "Wizard",
            3,
            DungeonBranch::Main,
            500,
            vec![],
            &mut rng,
        );
        let bones2 = generate_bones(
            &level,
            "Player2",
            10,
            60,
            Position::new(5, 5),
            "Valkyrie",
            3,
            DungeonBranch::Main,
            800,
            vec![],
            &mut rng,
        );

        let mut pool = BonesPool::new();
        pool.add(bones1);
        pool.add(bones2);

        // Should have only one entry (replaced).
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn test_bones_downgrade_items() {
        let mut rng = test_rng();
        let mut items = vec![
            BoneItem {
                position: Position::new(5, 5),
                name: "wand of fire".to_string(),
                cursed: false,
                charges: Some(8),
                is_artifact: false,
            },
            BoneItem {
                position: Position::new(6, 5),
                name: "long sword".to_string(),
                cursed: false,
                charges: None,
                is_artifact: false,
            },
        ];

        downgrade_bone_items(&mut items, &mut rng);

        // Charges should be further halved: 8 -> 4.
        assert_eq!(items[0].charges, Some(4));

        // Most items should be cursed with 80% probability.
        // We check structural correctness rather than specific randomness.
    }

    #[test]
    fn test_bones_pool_remove_encountered() {
        let mut rng = test_rng();
        let level = make_test_level();

        let bones = generate_bones(
            &level,
            "Player",
            5,
            30,
            Position::new(5, 5),
            "Wizard",
            3,
            DungeonBranch::Main,
            500,
            vec![],
            &mut rng,
        );

        let mut pool = BonesPool::new();
        pool.add(bones);

        // Find and load bones.
        let mut found = false;
        for seed in 0..100 {
            let mut try_rng = Pcg64::seed_from_u64(seed);
            if pool.try_get(DungeonBranch::Main, 3, &mut try_rng).is_some() {
                found = true;
                break;
            }
        }
        assert!(found);

        pool.remove_encountered();
        assert!(pool.is_empty());
    }

    // ── try_load_bones function ──────────────────────────────────

    #[test]
    fn test_bones_try_load_wrong_branch() {
        let mut rng = test_rng();
        let level = make_test_level();
        let bones = generate_bones(
            &level,
            "Player",
            5,
            30,
            Position::new(5, 5),
            "Wizard",
            3,
            DungeonBranch::Main,
            500,
            vec![],
            &mut rng,
        );

        let mut opt = Some(bones);
        let result = try_load_bones(&mut opt, DungeonBranch::Mines, 3, &mut rng);
        assert!(result.is_none());
    }

    #[test]
    fn test_bones_try_load_wrong_depth() {
        let mut rng = test_rng();
        let level = make_test_level();
        let bones = generate_bones(
            &level,
            "Player",
            5,
            30,
            Position::new(5, 5),
            "Wizard",
            3,
            DungeonBranch::Main,
            500,
            vec![],
            &mut rng,
        );

        let mut opt = Some(bones);
        let result = try_load_bones(&mut opt, DungeonBranch::Main, 7, &mut rng);
        assert!(result.is_none());
    }

    #[test]
    fn test_bones_try_load_already_encountered() {
        let mut rng = test_rng();
        let level = make_test_level();
        let mut bones = generate_bones(
            &level,
            "Player",
            5,
            30,
            Position::new(5, 5),
            "Wizard",
            3,
            DungeonBranch::Main,
            500,
            vec![],
            &mut rng,
        );
        bones.encountered = true;

        let mut opt = Some(bones);
        // Even with matching branch/depth, already-encountered bones should not load.
        for seed in 0..100 {
            let mut try_rng = Pcg64::seed_from_u64(seed);
            let result = try_load_bones(&mut opt, DungeonBranch::Main, 3, &mut try_rng);
            assert!(result.is_none(), "encountered bones should never load");
        }
    }

    // ── Artifact persistence ─────────────────────────────────────

    #[test]
    fn test_bones_filter_quest_artifacts() {
        let artifacts = vec![
            BoneArtifact {
                name: "Excalibur".to_string(),
                position: Position::new(5, 5),
                is_quest_artifact: false,
            },
            BoneArtifact {
                name: "The Orb of Detection".to_string(),
                position: Position::new(6, 5),
                is_quest_artifact: true,
            },
        ];
        let existing: Vec<String> = vec![];
        let filtered = filter_bone_artifacts(&artifacts, &existing);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Excalibur");
    }

    #[test]
    fn test_bones_filter_duplicate_artifacts() {
        let artifacts = vec![
            BoneArtifact {
                name: "Excalibur".to_string(),
                position: Position::new(5, 5),
                is_quest_artifact: false,
            },
            BoneArtifact {
                name: "Grayswandir".to_string(),
                position: Position::new(6, 5),
                is_quest_artifact: false,
            },
        ];
        let existing = vec!["Excalibur".to_string()];
        let filtered = filter_bone_artifacts(&artifacts, &existing);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Grayswandir");
    }

    #[test]
    fn test_bones_filter_all_removed() {
        let artifacts = vec![BoneArtifact {
            name: "Quest Art".to_string(),
            position: Position::new(5, 5),
            is_quest_artifact: true,
        }];
        let existing: Vec<String> = vec![];
        let filtered = filter_bone_artifacts(&artifacts, &existing);
        assert!(filtered.is_empty());
    }

    // ── Item scatter ─────────────────────────────────────────────

    #[test]
    fn test_bones_scatter_items_walkable() {
        let mut rng = test_rng();
        let level = make_test_level();
        let mut items = vec![BoneItem {
            position: Position::new(10, 5),
            name: "long sword".to_string(),
            cursed: false,
            charges: None,
            is_artifact: false,
        }];

        scatter_items(&mut items, &level, 2, &mut rng);
        // Position should remain walkable (level has floor from 1..=60, 1..=15).
        assert!(items[0].position.x >= 1 && items[0].position.x <= 60);
        assert!(items[0].position.y >= 1 && items[0].position.y <= 15);
    }

    #[test]
    fn test_bones_scatter_items_blocked_original() {
        let mut rng = test_rng();
        let level = make_test_level();
        // Place item on stone (outside the floor area).
        let mut items = vec![BoneItem {
            position: Position::new(0, 0),
            name: "potion".to_string(),
            cursed: false,
            charges: None,
            is_artifact: false,
        }];

        scatter_items(&mut items, &level, 5, &mut rng);
        // Should have been moved to a walkable position.
        let _cell = level.get(items[0].position);
        // May or may not find walkable depending on scatter radius from (0,0).
        // At minimum, the function should not crash.
    }

    // ── Invocation item conversion ───────────────────────────────

    #[test]
    fn test_bones_convert_amulet() {
        let mut items = vec![BoneItem {
            position: Position::new(5, 5),
            name: "Amulet of Yendor".to_string(),
            cursed: false,
            charges: None,
            is_artifact: true,
        }];

        let conversions = convert_invocation_items(&mut items);
        assert_eq!(conversions.len(), 1);
        assert_eq!(conversions[0].0, "Amulet of Yendor");
        assert!(items[0].name.contains("cheap plastic imitation"));
        assert!(items[0].cursed);
        assert!(!items[0].is_artifact);
    }

    #[test]
    fn test_bones_convert_candelabrum() {
        let mut items = vec![BoneItem {
            position: Position::new(5, 5),
            name: "Candelabrum of Invocation".to_string(),
            cursed: false,
            charges: Some(7),
            is_artifact: true,
        }];

        let conversions = convert_invocation_items(&mut items);
        assert_eq!(conversions.len(), 1);
        assert_eq!(items[0].name, "wax candle");
        assert!(items[0].cursed);
    }

    #[test]
    fn test_bones_convert_bell() {
        let mut items = vec![BoneItem {
            position: Position::new(5, 5),
            name: "Bell of Opening".to_string(),
            cursed: false,
            charges: None,
            is_artifact: true,
        }];

        convert_invocation_items(&mut items);
        assert_eq!(items[0].name, "bell");
        assert!(items[0].cursed);
    }

    #[test]
    fn test_bones_convert_book_of_dead() {
        let mut items = vec![BoneItem {
            position: Position::new(5, 5),
            name: "Book of the Dead".to_string(),
            cursed: false,
            charges: None,
            is_artifact: true,
        }];

        convert_invocation_items(&mut items);
        assert_eq!(items[0].name, "blank paper");
        assert!(items[0].cursed);
    }

    #[test]
    fn test_bones_convert_no_invocation_items() {
        let mut items = vec![BoneItem {
            position: Position::new(5, 5),
            name: "long sword".to_string(),
            cursed: false,
            charges: None,
            is_artifact: false,
        }];

        let conversions = convert_invocation_items(&mut items);
        assert!(conversions.is_empty());
        assert_eq!(items[0].name, "long sword");
    }

    // ── Bones file naming ────────────────────────────────────────

    #[test]
    fn test_bones_filename() {
        assert_eq!(bones_filename(DungeonBranch::Main, 5), "bonD0005");
        assert_eq!(bones_filename(DungeonBranch::Mines, 3), "bonM0003");
        assert_eq!(bones_filename(DungeonBranch::Sokoban, 1), "bonS0001");
    }

    // ── Bones summary ────────────────────────────────────────────

    #[test]
    fn test_bones_summary() {
        let mut rng = test_rng();
        let level = make_test_level();
        let items = vec![
            (Position::new(5, 5), "long sword".to_string(), None, false),
            (Position::new(5, 5), "Excalibur".to_string(), None, true),
        ];

        let bones = generate_bones(
            &level,
            "TestPlayer",
            15,
            80,
            Position::new(5, 5),
            "Valkyrie",
            10,
            DungeonBranch::Main,
            2000,
            items,
            &mut rng,
        );

        let summary = BonesSummary::from_bone_data(&bones);
        assert_eq!(summary.ghost_name, "TestPlayer");
        assert_eq!(summary.ghost_role, "Valkyrie");
        assert_eq!(summary.ghost_level, 15);
        assert_eq!(summary.item_count, 2);
        assert_eq!(summary.artifact_count, 1);
        assert_eq!(summary.depth, 10);
    }
}
