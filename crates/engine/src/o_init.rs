//! Object appearance shuffling for unidentified items.
//!
//! In C NetHack, each new game randomizes the appearance of unidentified
//! items — potions get random colors, scrolls get random labels, rings
//! and wands get random materials, etc.  Without this, every game uses
//! the same descriptions and the identification mini-game is destroyed.
//!
//! This module provides a standalone `AppearanceTable` that holds the
//! shuffled mapping, generated once per game from the RNG seed.
//! Reference: C NetHack `src/o_init.c` — `shuffle_all()`, `init_objects()`.

use rand::Rng;
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------
// Appearance pools (from C NetHack o_init.c / objnam.c)
// -----------------------------------------------------------------------

/// Potion color pool.
const POTION_COLORS: &[&str] = &[
    "ruby",
    "pink",
    "orange",
    "yellow",
    "emerald",
    "dark green",
    "cyan",
    "sky blue",
    "brilliant blue",
    "magenta",
    "purple-red",
    "puce",
    "milky",
    "swirly",
    "bubbly",
    "smoky",
    "cloudy",
    "effervescent",
    "black",
    "golden",
    "brown",
    "fizzy",
    "dark",
    "white",
    "murky",
    "clear",
];

/// Scroll label pool.
const SCROLL_LABELS: &[&str] = &[
    "ZELGO MER",
    "JUYED AWK YACC",
    "NR 9",
    "XIXAXA XOXAXA XUXAXA",
    "PRATYAVAYAH",
    "DAIYEN FANSEN",
    "HAPAX LEGOMENON",
    "ELBIB YLANSEN",
    "KERNOD WEL",
    "ELAM EANSEN",
    "NIHIL ESTNE",
    "FOOBIE BLETCH",
    "TEMOV",
    "GARVEN DEH",
    "READ ME",
    "ETAOIN SHRDLU",
    "LOREM IPSUM",
    "FNORD",
    "KO BANSEN",
    "DUAM XNAHT",
    "STRC PRANSEN",
    "HACKEM MUCHE",
    "VELOX NEB",
    "PRIRUTSENIE",
    "ANDOVA BEGARIN",
    "KIRJE",
    "VE FORBRULL",
    "VERR ULL SUULL",
    "THARR",
    "YUM YUM",
    "AQUE BRAGH",
    "HZLRC ANSEN",
];

/// Ring material pool.
const RING_MATERIALS: &[&str] = &[
    "wooden",
    "granite",
    "opal",
    "clay",
    "coral",
    "black onyx",
    "moonstone",
    "tiger eye",
    "jade",
    "bronze",
    "agate",
    "topaz",
    "sapphire",
    "ruby",
    "diamond",
    "pearl",
    "iron",
    "brass",
    "copper",
    "twisted",
    "steel",
    "wire",
    "engagement",
    "shiny",
    "gold",
    "silver",
];

/// Wand material pool.
const WAND_MATERIALS: &[&str] = &[
    "glass",
    "balsa",
    "crystal",
    "maple",
    "pine",
    "oak",
    "ebony",
    "marble",
    "tin",
    "brass",
    "copper",
    "silver",
    "platinum",
    "iridium",
    "zinc",
    "aluminum",
    "uranium",
    "iron",
    "steel",
    "hexagonal",
    "short",
    "runed",
    "long",
    "curved",
    "forked",
    "spiked",
    "jeweled",
];

/// Spellbook color pool.
const SPELLBOOK_COLORS: &[&str] = &[
    "parchment",
    "vellum",
    "ragged",
    "dog eared",
    "mottled",
    "stained",
    "cloth",
    "leather",
    "white",
    "pink",
    "red",
    "orange",
    "yellow",
    "velvet",
    "light green",
    "dark green",
    "turquoise",
    "cyan",
    "light blue",
    "dark blue",
    "indigo",
    "magenta",
    "purple",
    "violet",
    "tan",
    "plaid",
    "light brown",
    "dark brown",
    "gray",
    "wrinkled",
    "dusty",
    "bronze",
    "copper",
    "silver",
    "gold",
    "glittering",
    "shining",
    "dull",
    "thin",
    "thick",
];

/// Amulet shape pool.
const AMULET_SHAPES: &[&str] = &[
    "circular",
    "spherical",
    "oval",
    "triangular",
    "pyramidal",
    "square",
    "concave",
    "hexagonal",
    "octagonal",
];

/// Default gem color names (before identification).
/// Gems share color-based descriptions — multiple gem types can appear
/// as the same color, unlike other shuffled classes.
const GEM_DESCRIPTIONS: &[&str] = &[
    "white", "white", "red", "red", "orange", "orange", "yellow", "yellow", "green", "green",
    "blue", "blue", "violet", "violet", "black", "black",
];

// -----------------------------------------------------------------------
// AppearanceTable
// -----------------------------------------------------------------------

/// Shuffled appearance tables for unidentified items.
/// Generated once per game using the game's RNG seed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppearanceTable {
    /// Potion color assignments: type_index -> color_name.
    pub potion_colors: Vec<String>,
    /// Scroll label assignments: type_index -> label.
    pub scroll_labels: Vec<String>,
    /// Ring material assignments: type_index -> material_name.
    pub ring_materials: Vec<String>,
    /// Wand material assignments: type_index -> material_name.
    pub wand_materials: Vec<String>,
    /// Spellbook color assignments: type_index -> color_name.
    pub spellbook_colors: Vec<String>,
    /// Amulet shape assignments: type_index -> shape_name.
    pub amulet_shapes: Vec<String>,
    /// Gem descriptions (before identification).
    pub gem_appearances: Vec<String>,
}

impl AppearanceTable {
    /// Generate a new shuffled appearance table for a new game.
    ///
    /// The `*_count` parameters specify how many types exist in each class,
    /// which determines how many entries from each pool are drawn.
    /// Use `Self::new_default(rng)` for standard NetHack type counts.
    pub fn new_with_counts(
        rng: &mut impl Rng,
        potion_count: usize,
        scroll_count: usize,
        ring_count: usize,
        wand_count: usize,
        spellbook_count: usize,
        amulet_count: usize,
    ) -> Self {
        Self {
            potion_colors: shuffle_and_assign(POTION_COLORS, potion_count, rng),
            scroll_labels: shuffle_and_assign(SCROLL_LABELS, scroll_count, rng),
            ring_materials: shuffle_and_assign(RING_MATERIALS, ring_count, rng),
            wand_materials: shuffle_and_assign(WAND_MATERIALS, wand_count, rng),
            spellbook_colors: shuffle_and_assign(SPELLBOOK_COLORS, spellbook_count, rng),
            amulet_shapes: shuffle_and_assign(AMULET_SHAPES, amulet_count, rng),
            gem_appearances: default_gem_appearances(),
        }
    }

    /// Generate a new appearance table using standard NetHack 3.7 type counts.
    pub fn new(rng: &mut impl Rng) -> Self {
        Self::new_with_counts(rng, 26, 23, 28, 24, 44, 9)
    }

    /// Get the unidentified appearance for an item type.
    ///
    /// `class` is the object class symbol:
    /// - `'!'` potion, `'?'` scroll, `'='` ring, `'/'` wand,
    /// - `'+'` spellbook, `'"'` amulet, `'*'` gem.
    pub fn appearance(&self, class: char, type_index: usize) -> Option<&str> {
        let table = match class {
            '!' => &self.potion_colors,
            '?' => &self.scroll_labels,
            '=' => &self.ring_materials,
            '/' => &self.wand_materials,
            '+' => &self.spellbook_colors,
            '"' => &self.amulet_shapes,
            '*' => &self.gem_appearances,
            _ => return None,
        };
        table.get(type_index).map(|s| s.as_str())
    }

    /// Get the full unidentified name for display.
    ///
    /// Examples: `"ruby potion"`, `"scroll labeled ZELGO MER"`,
    /// `"granite ring"`, `"glass wand"`, `"parchment spellbook"`,
    /// `"circular amulet"`.
    pub fn unidentified_name(&self, class: char, type_index: usize) -> Option<String> {
        let appearance = self.appearance(class, type_index)?;
        Some(match class {
            '!' => format!("{} potion", appearance),
            '?' => format!("scroll labeled {}", appearance),
            '=' => format!("{} ring", appearance),
            '/' => format!("{} wand", appearance),
            '+' => format!("{} spellbook", appearance),
            '"' => format!("{} amulet", appearance),
            '*' => appearance.to_string(),
            _ => appearance.to_string(),
        })
    }

    /// Return the pool of all possible appearances for a class.
    pub fn pool(class: char) -> &'static [&'static str] {
        match class {
            '!' => POTION_COLORS,
            '?' => SCROLL_LABELS,
            '=' => RING_MATERIALS,
            '/' => WAND_MATERIALS,
            '+' => SPELLBOOK_COLORS,
            '"' => AMULET_SHAPES,
            '*' => GEM_DESCRIPTIONS,
            _ => &[],
        }
    }
}

impl Default for AppearanceTable {
    fn default() -> Self {
        use rand::SeedableRng;
        let mut rng = rand_pcg::Pcg64::seed_from_u64(0);
        Self::new(&mut rng)
    }
}

/// Verify two games with different seeds produce different appearances.
pub fn appearances_differ(a: &AppearanceTable, b: &AppearanceTable) -> bool {
    a.potion_colors != b.potion_colors
        || a.scroll_labels != b.scroll_labels
        || a.ring_materials != b.ring_materials
        || a.wand_materials != b.wand_materials
        || a.spellbook_colors != b.spellbook_colors
        || a.amulet_shapes != b.amulet_shapes
}

// -----------------------------------------------------------------------
// Internal helpers
// -----------------------------------------------------------------------

/// Shuffle a pool and take the first `count` entries.
/// Uses Fisher-Yates shuffle to match C NetHack's approach.
fn shuffle_and_assign(pool: &[&str], count: usize, rng: &mut impl Rng) -> Vec<String> {
    let mut shuffled: Vec<String> = pool.iter().map(|s| s.to_string()).collect();
    // Fisher-Yates shuffle (matching C's shuffle() in o_init.c).
    for i in (1..shuffled.len()).rev() {
        let j = rng.random_range(0..=i);
        shuffled.swap(i, j);
    }
    shuffled.truncate(count.min(shuffled.len()));
    shuffled
}

/// Default gem appearances — gems share color-based descriptions.
fn default_gem_appearances() -> Vec<String> {
    GEM_DESCRIPTIONS.iter().map(|s| s.to_string()).collect()
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    fn make_rng(seed: u64) -> Pcg64 {
        Pcg64::seed_from_u64(seed)
    }

    #[test]
    fn new_table_has_correct_counts() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        // Counts are min(requested, pool_size).
        assert_eq!(table.potion_colors.len(), 26); // min(26, 26)
        assert_eq!(table.scroll_labels.len(), 23); // min(23, 32)
        assert_eq!(table.ring_materials.len(), 26); // min(28, 26)
        assert_eq!(table.wand_materials.len(), 24); // min(24, 27)
        assert_eq!(table.spellbook_colors.len(), 40); // min(44, 40)
        assert_eq!(table.amulet_shapes.len(), 9); // min(9, 9)
        assert_eq!(table.gem_appearances.len(), GEM_DESCRIPTIONS.len());
    }

    #[test]
    fn different_seeds_different_shuffles() {
        let mut rng1 = make_rng(1);
        let mut rng2 = make_rng(2);
        let t1 = AppearanceTable::new(&mut rng1);
        let t2 = AppearanceTable::new(&mut rng2);
        assert!(appearances_differ(&t1, &t2));
    }

    #[test]
    fn same_seed_same_shuffle() {
        let mut rng1 = make_rng(12345);
        let mut rng2 = make_rng(12345);
        let t1 = AppearanceTable::new(&mut rng1);
        let t2 = AppearanceTable::new(&mut rng2);
        assert!(!appearances_differ(&t1, &t2));
        assert_eq!(t1.potion_colors, t2.potion_colors);
        assert_eq!(t1.scroll_labels, t2.scroll_labels);
        assert_eq!(t1.ring_materials, t2.ring_materials);
        assert_eq!(t1.wand_materials, t2.wand_materials);
        assert_eq!(t1.spellbook_colors, t2.spellbook_colors);
        assert_eq!(t1.amulet_shapes, t2.amulet_shapes);
    }

    #[test]
    fn appearance_lookup_potion() {
        let mut rng = make_rng(99);
        let table = AppearanceTable::new(&mut rng);
        for i in 0..table.potion_colors.len() {
            let app = table.appearance('!', i);
            assert!(app.is_some());
            assert!(POTION_COLORS.contains(&app.unwrap()));
        }
        // Out of bounds returns None.
        assert!(table.appearance('!', 100).is_none());
    }

    #[test]
    fn appearance_lookup_scroll() {
        let mut rng = make_rng(99);
        let table = AppearanceTable::new(&mut rng);
        for i in 0..table.scroll_labels.len() {
            let app = table.appearance('?', i);
            assert!(app.is_some());
            assert!(SCROLL_LABELS.contains(&app.unwrap()));
        }
    }

    #[test]
    fn appearance_lookup_ring() {
        let mut rng = make_rng(99);
        let table = AppearanceTable::new(&mut rng);
        for i in 0..table.ring_materials.len() {
            let app = table.appearance('=', i);
            assert!(app.is_some());
            assert!(RING_MATERIALS.contains(&app.unwrap()));
        }
    }

    #[test]
    fn appearance_lookup_wand() {
        let mut rng = make_rng(99);
        let table = AppearanceTable::new(&mut rng);
        for i in 0..table.wand_materials.len() {
            let app = table.appearance('/', i);
            assert!(app.is_some());
            assert!(WAND_MATERIALS.contains(&app.unwrap()));
        }
    }

    #[test]
    fn appearance_lookup_spellbook() {
        let mut rng = make_rng(99);
        let table = AppearanceTable::new(&mut rng);
        for i in 0..table.spellbook_colors.len() {
            let app = table.appearance('+', i);
            assert!(app.is_some());
            assert!(SPELLBOOK_COLORS.contains(&app.unwrap()));
        }
    }

    #[test]
    fn appearance_lookup_amulet() {
        let mut rng = make_rng(99);
        let table = AppearanceTable::new(&mut rng);
        for i in 0..table.amulet_shapes.len() {
            let app = table.appearance('"', i);
            assert!(app.is_some());
            assert!(AMULET_SHAPES.contains(&app.unwrap()));
        }
    }

    #[test]
    fn appearance_lookup_gem() {
        let mut rng = make_rng(99);
        let table = AppearanceTable::new(&mut rng);
        for i in 0..table.gem_appearances.len() {
            assert!(table.appearance('*', i).is_some());
        }
    }

    #[test]
    fn appearance_unknown_class_returns_none() {
        let mut rng = make_rng(99);
        let table = AppearanceTable::new(&mut rng);
        assert!(table.appearance('X', 0).is_none());
        assert!(table.appearance('$', 0).is_none());
        assert!(table.appearance('[', 0).is_none());
    }

    #[test]
    fn unidentified_name_potion() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        let name = table.unidentified_name('!', 0).unwrap();
        assert!(name.ends_with(" potion"), "got: {}", name);
    }

    #[test]
    fn unidentified_name_scroll() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        let name = table.unidentified_name('?', 0).unwrap();
        assert!(name.starts_with("scroll labeled "), "got: {}", name);
    }

    #[test]
    fn unidentified_name_ring() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        let name = table.unidentified_name('=', 0).unwrap();
        assert!(name.ends_with(" ring"), "got: {}", name);
    }

    #[test]
    fn unidentified_name_wand() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        let name = table.unidentified_name('/', 0).unwrap();
        assert!(name.ends_with(" wand"), "got: {}", name);
    }

    #[test]
    fn unidentified_name_spellbook() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        let name = table.unidentified_name('+', 0).unwrap();
        assert!(name.ends_with(" spellbook"), "got: {}", name);
    }

    #[test]
    fn unidentified_name_amulet() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        let name = table.unidentified_name('"', 0).unwrap();
        assert!(name.ends_with(" amulet"), "got: {}", name);
    }

    #[test]
    fn unidentified_name_out_of_bounds() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        assert!(table.unidentified_name('!', 999).is_none());
    }

    #[test]
    fn no_duplicate_appearances_potion() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        let mut seen = std::collections::HashSet::new();
        for color in &table.potion_colors {
            assert!(seen.insert(color), "duplicate potion color: {}", color);
        }
    }

    #[test]
    fn no_duplicate_appearances_scroll() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        let mut seen = std::collections::HashSet::new();
        for label in &table.scroll_labels {
            assert!(seen.insert(label), "duplicate scroll label: {}", label);
        }
    }

    #[test]
    fn no_duplicate_appearances_ring() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        let mut seen = std::collections::HashSet::new();
        for mat in &table.ring_materials {
            assert!(seen.insert(mat), "duplicate ring material: {}", mat);
        }
    }

    #[test]
    fn no_duplicate_appearances_wand() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        let mut seen = std::collections::HashSet::new();
        for mat in &table.wand_materials {
            assert!(seen.insert(mat), "duplicate wand material: {}", mat);
        }
    }

    #[test]
    fn pool_sizes_sufficient() {
        // The pool for each class must be >= the requested type count.
        assert!(POTION_COLORS.len() >= 26, "not enough potion colors");
        assert!(SCROLL_LABELS.len() >= 23, "not enough scroll labels");
        assert!(RING_MATERIALS.len() >= 26, "not enough ring materials");
        assert!(WAND_MATERIALS.len() >= 24, "not enough wand materials");
        assert!(SPELLBOOK_COLORS.len() >= 40, "not enough spellbook colors");
        assert!(AMULET_SHAPES.len() >= 9, "not enough amulet shapes");
    }

    #[test]
    fn gem_defaults_correct() {
        let gems = default_gem_appearances();
        assert_eq!(gems.len(), 16);
        assert_eq!(gems[0], "white");
        assert_eq!(gems[2], "red");
        assert_eq!(gems[14], "black");
    }

    #[test]
    fn custom_counts() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new_with_counts(&mut rng, 5, 5, 5, 5, 5, 5);
        assert_eq!(table.potion_colors.len(), 5);
        assert_eq!(table.scroll_labels.len(), 5);
        assert_eq!(table.ring_materials.len(), 5);
        assert_eq!(table.wand_materials.len(), 5);
        assert_eq!(table.spellbook_colors.len(), 5);
        assert_eq!(table.amulet_shapes.len(), 5);
    }

    #[test]
    fn pool_accessor() {
        assert_eq!(AppearanceTable::pool('!').len(), POTION_COLORS.len());
        assert_eq!(AppearanceTable::pool('?').len(), SCROLL_LABELS.len());
        assert_eq!(AppearanceTable::pool('=').len(), RING_MATERIALS.len());
        assert_eq!(AppearanceTable::pool('/').len(), WAND_MATERIALS.len());
        assert_eq!(AppearanceTable::pool('+').len(), SPELLBOOK_COLORS.len());
        assert_eq!(AppearanceTable::pool('"').len(), AMULET_SHAPES.len());
        assert_eq!(AppearanceTable::pool('*').len(), GEM_DESCRIPTIONS.len());
        assert!(AppearanceTable::pool('X').is_empty());
    }

    #[test]
    fn serde_roundtrip() {
        let mut rng = make_rng(42);
        let table = AppearanceTable::new(&mut rng);
        let json = serde_json::to_string(&table).unwrap();
        let restored: AppearanceTable = serde_json::from_str(&json).unwrap();
        assert_eq!(table.potion_colors, restored.potion_colors);
        assert_eq!(table.scroll_labels, restored.scroll_labels);
        assert_eq!(table.ring_materials, restored.ring_materials);
        assert_eq!(table.wand_materials, restored.wand_materials);
        assert_eq!(table.spellbook_colors, restored.spellbook_colors);
        assert_eq!(table.amulet_shapes, restored.amulet_shapes);
        assert_eq!(table.gem_appearances, restored.gem_appearances);
    }
}
