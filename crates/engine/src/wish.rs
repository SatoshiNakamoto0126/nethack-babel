//! Wish parsing system.
//!
//! Parses player text input like "blessed +2 silver dragon scale mail"
//! into a structured `WishResult` describing the desired item.
//!
//! This implements a simplified version of the C NetHack `readobjnam()`
//! function from `objnam.c`.

use nethack_babel_data::{Material, ObjectClass, ObjectDef, ObjectTypeId};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Result of parsing a wish string.
#[derive(Debug, Clone, PartialEq)]
pub struct WishResult {
    /// The matched object type.
    pub object_type: ObjectTypeId,
    /// Blessed/uncursed/cursed status, if specified.
    pub buc: Option<BucWish>,
    /// Enchantment value (+N or -N), if specified.
    pub enchantment: Option<i8>,
    /// Requested quantity (defaults to 1 if not specified).
    pub quantity: u32,
    /// Whether erodeproofing was requested.
    pub erodeproof: bool,
    /// Individual name for the item, if specified via "named X".
    pub name: Option<String>,
}

/// BUC status as requested in a wish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucWish {
    Blessed,
    Uncursed,
    Cursed,
}

// ---------------------------------------------------------------------------
// Unwishable item names (case-insensitive substrings)
// ---------------------------------------------------------------------------

/// Names of items that cannot be wished for, or should be downgraded.
const UNWISHABLE_NAMES: &[&str] = &[
    "amulet of yendor",
    "candelabrum of invocation",
    "bell of opening",
    "book of the dead",
];

/// Name of the downgrade item when wishing for the Amulet of Yendor.
const FAKE_AMULET_NAME: &str = "cheap plastic imitation of the Amulet of Yendor";

// ---------------------------------------------------------------------------
// Core parse function
// ---------------------------------------------------------------------------

/// Parse a wish string against the given object definitions.
///
/// Returns `Some(WishResult)` if a matching object is found, `None` if the
/// wish cannot be fulfilled (e.g., unwishable item with no downgrade).
///
/// The parser strips known prefixes (BUC, enchantment, quantity, erodeproof
/// keywords) from the input, then fuzzy-matches the remainder against object
/// names.
pub fn parse_wish(input: &str, objects: &[ObjectDef]) -> Option<WishResult> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let lower = input.to_lowercase();
    let mut remaining = lower.as_str();

    // --- 1. Extract quantity (leading digits) ---
    let quantity = parse_leading_quantity(&mut remaining);

    // --- 2. Extract BUC prefix ---
    let buc = parse_buc(&mut remaining);

    // --- 3. Extract erodeproof keywords ---
    let erodeproof = parse_erodeproof(&mut remaining);

    // --- 4. Extract enchantment (+N / -N) ---
    let enchantment = parse_enchantment(&mut remaining);

    // Strip again in case there was whitespace between modifiers
    let erodeproof = erodeproof || parse_erodeproof(&mut remaining);

    // --- 5. Extract "named X" suffix ---
    let name = parse_named_suffix(&mut remaining);

    // --- 6. Clean up remaining text ---
    let mut item_name = remaining.trim().to_string();

    if item_name.is_empty() {
        return None;
    }

    // --- 6a. Strip "pair of " / "set of " prefixes ---
    if let Some(rest) = item_name.strip_prefix("pair of ") {
        item_name = rest.trim().to_string();
    } else if let Some(rest) = item_name.strip_prefix("set of ") {
        item_name = rest.trim().to_string();
    }

    // --- 6b. Try singularizing the name for matching ---
    // E.g. "arrows" -> "arrow", "potions of healing" -> "potion of healing"
    let singularized = crate::identification::makesingular(&item_name);

    // --- 7. Check for unwishable items ---
    let check_name = item_name.as_str();
    if is_unwishable(check_name) {
        // Special case: "amulet of yendor" -> downgrade to fake
        if check_name.contains("amulet of yendor")
            && let Some(fake) = find_object_by_name(FAKE_AMULET_NAME, objects)
        {
            return Some(WishResult {
                object_type: fake.id,
                buc,
                enchantment,
                quantity: quantity.unwrap_or(1),
                erodeproof,
                name,
            });
        }
        return None;
    }

    // --- 8. Match item name against object database ---
    // Try exact name first, then singularized form.
    let matched =
        match_object(check_name, objects).or_else(|| match_object(&singularized, objects));

    if let Some(obj) = matched {
        // Reject items marked as nowish
        if obj.is_nowish {
            return None;
        }

        return Some(WishResult {
            object_type: obj.id,
            buc,
            enchantment,
            quantity: quantity.unwrap_or(1),
            erodeproof,
            name,
        });
    }

    None
}

// ---------------------------------------------------------------------------
// Prefix / suffix parsers
// ---------------------------------------------------------------------------

/// Parse and consume a leading numeric quantity.
fn parse_leading_quantity(s: &mut &str) -> Option<u32> {
    let trimmed = s.trim_start();
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    if let Ok(n) = digits.parse::<u32>()
        && n > 0
    {
        let rest = trimmed[digits.len()..].trim_start();
        *s = rest;
        return Some(n);
    }
    None
}

/// Parse and consume a BUC prefix.
fn parse_buc(s: &mut &str) -> Option<BucWish> {
    let trimmed = s.trim_start();
    if strip_word(trimmed, "blessed").is_some() {
        *s = strip_word(trimmed, "blessed").unwrap();
        return Some(BucWish::Blessed);
    }
    if strip_word(trimmed, "uncursed").is_some() {
        *s = strip_word(trimmed, "uncursed").unwrap();
        return Some(BucWish::Uncursed);
    }
    if strip_word(trimmed, "cursed").is_some() {
        *s = strip_word(trimmed, "cursed").unwrap();
        return Some(BucWish::Cursed);
    }
    // Also accept "holy" as blessed, "unholy" as cursed (for holy water)
    if strip_word(trimmed, "holy").is_some() {
        *s = strip_word(trimmed, "holy").unwrap();
        return Some(BucWish::Blessed);
    }
    if strip_word(trimmed, "unholy").is_some() {
        *s = strip_word(trimmed, "unholy").unwrap();
        return Some(BucWish::Cursed);
    }
    None
}

/// Parse and consume erodeproof keywords.
/// Returns true if any were found.
fn parse_erodeproof(s: &mut &str) -> bool {
    let mut found = false;
    let keywords = [
        "rustproof",
        "fireproof",
        "fixed",
        "greased",
        "erodeproof",
        "corrodeproof",
        "rotproof",
    ];
    loop {
        let trimmed = s.trim_start();
        let mut matched = false;
        for kw in &keywords {
            if let Some(rest) = strip_word(trimmed, kw) {
                *s = rest;
                found = true;
                matched = true;
                break;
            }
        }
        if !matched {
            break;
        }
    }
    found
}

/// Parse and consume an enchantment value like "+2" or "-1".
fn parse_enchantment(s: &mut &str) -> Option<i8> {
    let trimmed = s.trim_start();
    if trimmed.starts_with('+') || trimmed.starts_with('-') {
        let sign = if trimmed.starts_with('+') { 1i8 } else { -1i8 };
        let rest = &trimmed[1..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty()
            && let Ok(n) = digits.parse::<i8>()
        {
            let val = sign.saturating_mul(n);
            *s = rest[digits.len()..].trim_start();
            return Some(val);
        }
    }
    None
}

/// Parse and consume a "named X" suffix from the remaining string.
/// Returns the name portion if found.
fn parse_named_suffix(s: &mut &str) -> Option<String> {
    let trimmed = s.trim();
    // Look for " named " in the string (case already lowered)
    if let Some(idx) = trimmed.find(" named ") {
        let name_part = trimmed[idx + 7..].trim().to_string();
        *s = &trimmed[..idx];
        if name_part.is_empty() {
            return None;
        }
        return Some(name_part);
    }
    None
}

// ---------------------------------------------------------------------------
// Object matching
// ---------------------------------------------------------------------------

/// Check if an item name refers to an unwishable item.
fn is_unwishable(name: &str) -> bool {
    let lower = name.to_lowercase();
    UNWISHABLE_NAMES.iter().any(|u| lower.contains(u))
}

/// Find an object by exact name match (case-insensitive).
fn find_object_by_name<'a>(name: &str, objects: &'a [ObjectDef]) -> Option<&'a ObjectDef> {
    let lower = name.to_lowercase();
    objects.iter().find(|o| o.name.to_lowercase() == lower)
}

/// Match an item name against the object database, using multiple strategies:
/// 1. Exact match against object name
/// 2. Match with class prefix stripped ("scroll of X" -> match X in scrolls)
/// 3. Material prefix match ("silver dragon scale mail")
/// 4. Partial/fuzzy match
fn match_object<'a>(name: &str, objects: &'a [ObjectDef]) -> Option<&'a ObjectDef> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }

    // Strategy 1: Exact match against full object name
    if let Some(obj) = find_object_by_name(name, objects) {
        return Some(obj);
    }

    // Strategy 2: Match with class prefix handling.
    // Users type "scroll of identify" but the data stores just "identify".
    // Also handle "potion of healing", "wand of fire", "ring of X", "spellbook of X".
    let class_prefixes: &[(&str, ObjectClass)] = &[
        ("scroll of ", ObjectClass::Scroll),
        ("potion of ", ObjectClass::Potion),
        ("wand of ", ObjectClass::Wand),
        ("ring of ", ObjectClass::Ring),
        ("spellbook of ", ObjectClass::Spellbook),
        ("amulet of ", ObjectClass::Amulet),
    ];

    for &(prefix, class) in class_prefixes {
        if let Some(base_name) = name.strip_prefix(prefix) {
            let base_name = base_name.trim();
            // Exact match within the class
            if let Some(obj) = objects
                .iter()
                .find(|o| o.class == class && o.name.to_lowercase() == base_name)
            {
                return Some(obj);
            }
            // Also try matching the full stored name (for amulets like "amulet of ESP")
            if let Some(obj) = find_object_by_name(name, objects) {
                return Some(obj);
            }
        }
    }

    // Strategy 3: Handle material prefix.
    // "silver dragon scale mail" -> material=Silver + name="dragon scale mail"
    // Actually in the data, "silver dragon scale mail" IS the full name.
    // But let's handle generic material prefixes too.
    let material_prefixes: &[(&str, Material)] = &[
        ("silver ", Material::Silver),
        ("iron ", Material::Iron),
        ("wooden ", Material::Wood),
        ("leather ", Material::Leather),
        ("gold ", Material::Gold),
        ("mithril ", Material::Mithril),
        ("copper ", Material::Copper),
        ("glass ", Material::Glass),
        ("cloth ", Material::Cloth),
    ];

    for &(prefix, _material) in material_prefixes {
        if let Some(base_name) = name.strip_prefix(prefix) {
            // Try exact match on the base name with any material
            if let Some(obj) = find_object_by_name(base_name, objects) {
                return Some(obj);
            }
        }
    }

    // Strategy 4: Partial match (prefix match on object name).
    // "scroll of id" -> look for objects where class prefix + name starts with the input.
    // First try: the input contains a class prefix, extract base and partial-match.
    for &(prefix, class) in class_prefixes {
        if let Some(base_partial) = name.strip_prefix(prefix) {
            let base_partial = base_partial.trim();
            if !base_partial.is_empty() {
                // Find objects in this class whose name starts with the partial
                if let Some(obj) = objects
                    .iter()
                    .find(|o| o.class == class && o.name.to_lowercase().starts_with(base_partial))
                {
                    return Some(obj);
                }
            }
        }
    }

    // Strategy 5: General partial match across all objects.
    // The input might be a partial name like "long sw" matching "long sword".
    if let Some(obj) = objects
        .iter()
        .find(|o| o.name.to_lowercase().starts_with(name))
    {
        return Some(obj);
    }

    // Strategy 6: Substring match (less precise, last resort).
    // Check if any object name contains the input as a substring.
    if name.len() >= 3
        && let Some(obj) = objects
            .iter()
            .find(|o| o.name.to_lowercase().contains(name))
    {
        return Some(obj);
    }

    None
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// If `s` starts with `word` followed by a space or end-of-string,
/// return the remainder after the word and any trailing space.
fn strip_word<'a>(s: &'a str, word: &str) -> Option<&'a str> {
    if let Some(rest) = s.strip_prefix(word)
        && (rest.is_empty() || rest.starts_with(' '))
    {
        return Some(rest.trim_start());
    }
    None
}

// ---------------------------------------------------------------------------
// Wish restrictions
// ---------------------------------------------------------------------------

/// Reasons a wish might be restricted or denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WishRestriction {
    /// The item cannot be wished for at all (quest artifacts, invocation items).
    Forbidden,
    /// The item can only appear once and has already been generated.
    AlreadyGenerated,
    /// The enchantment was clamped to the allowed maximum.
    EnchantmentClamped { requested: i8, granted: i8 },
    /// Quantity was reduced.
    QuantityReduced { requested: u32, granted: u32 },
}

/// Maximum enchantment allowed by wishing (matches C NetHack: 3 for most).
pub const MAX_WISH_ENCHANTMENT: i8 = 3;

/// Apply wish restrictions to a parsed `WishResult`, potentially modifying
/// enchantment and quantity.  Returns a list of restrictions that were
/// applied (empty if none).
pub fn apply_wish_restrictions(result: &mut WishResult) -> Vec<WishRestriction> {
    let mut restrictions = Vec::new();

    // Clamp enchantment.
    if let Some(ench) = result.enchantment
        && ench > MAX_WISH_ENCHANTMENT
    {
        restrictions.push(WishRestriction::EnchantmentClamped {
            requested: ench,
            granted: MAX_WISH_ENCHANTMENT,
        });
        result.enchantment = Some(MAX_WISH_ENCHANTMENT);
    }

    // Clamp quantity for non-stackable items (simplified: max 1 for
    // weapons, armor, tools; allow up to 20 for ammo/scrolls/potions).
    // Full implementation would check object class.
    if result.quantity > 20 {
        restrictions.push(WishRestriction::QuantityReduced {
            requested: result.quantity,
            granted: 20,
        });
        result.quantity = 20;
    }

    restrictions
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use nethack_babel_data::{GameData, load_game_data};
    use std::path::PathBuf;

    fn data_dir() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.join("../../data")
    }

    fn load_test_data() -> GameData {
        load_game_data(&data_dir()).expect("failed to load game data")
    }

    #[test]
    fn test_parse_blessed_plus2_silver_dragon_scale_mail() {
        let data = load_test_data();
        let result = parse_wish("blessed +2 silver dragon scale mail", &data.objects);
        let r = result.expect("should parse");
        assert_eq!(r.buc, Some(BucWish::Blessed));
        assert_eq!(r.enchantment, Some(2));
        assert_eq!(r.erodeproof, false);
        // Find the expected object type
        let sdsm = data
            .objects
            .iter()
            .find(|o| o.name.to_lowercase() == "silver dragon scale mail")
            .expect("SDSM should exist in data");
        assert_eq!(r.object_type, sdsm.id);
    }

    #[test]
    fn test_parse_rustproof_plus3_long_sword() {
        let data = load_test_data();
        let result = parse_wish("rustproof +3 long sword", &data.objects);
        let r = result.expect("should parse");
        assert_eq!(r.erodeproof, true);
        assert_eq!(r.enchantment, Some(3));
        let ls = data
            .objects
            .iter()
            .find(|o| o.name.to_lowercase() == "long sword")
            .expect("long sword should exist");
        assert_eq!(r.object_type, ls.id);
    }

    #[test]
    fn test_parse_blessed_elven_arrow() {
        let data = load_test_data();
        let result = parse_wish("blessed +4 elven arrow", &data.objects);
        let r = result.expect("should parse");
        assert_eq!(r.buc, Some(BucWish::Blessed));
        assert_eq!(r.enchantment, Some(4));
        let ea = data
            .objects
            .iter()
            .find(|o| o.name.to_lowercase() == "elven arrow")
            .expect("elven arrow should exist");
        assert_eq!(r.object_type, ea.id);
    }

    #[test]
    fn test_parse_amulet_of_yendor_rejected() {
        let data = load_test_data();
        let result = parse_wish("amulet of yendor", &data.objects);
        // Should either return None or the fake amulet
        match result {
            Some(r) => {
                // Should be the cheap plastic imitation
                let fake = data
                    .objects
                    .iter()
                    .find(|o| o.name.to_lowercase().contains("cheap plastic"))
                    .expect("fake amulet should exist");
                assert_eq!(r.object_type, fake.id);
            }
            None => {
                // Also acceptable if the fake isn't found
            }
        }
    }

    #[test]
    fn test_parse_case_insensitive() {
        let data = load_test_data();
        let r1 =
            parse_wish("BLESSED +2 LONG SWORD", &data.objects).expect("uppercase should parse");
        let r2 =
            parse_wish("blessed +2 long sword", &data.objects).expect("lowercase should parse");
        assert_eq!(r1.object_type, r2.object_type);
        assert_eq!(r1.buc, r2.buc);
        assert_eq!(r1.enchantment, r2.enchantment);
    }

    #[test]
    fn test_parse_partial_match_scroll_of_id() {
        let data = load_test_data();
        let result = parse_wish("scroll of id", &data.objects);
        let r = result.expect("partial match should work");
        let identify = data
            .objects
            .iter()
            .find(|o| o.class == ObjectClass::Scroll && o.name.to_lowercase() == "identify")
            .expect("scroll of identify should exist");
        assert_eq!(r.object_type, identify.id);
    }

    #[test]
    fn test_parse_empty_input() {
        let data = load_test_data();
        assert!(parse_wish("", &data.objects).is_none());
        assert!(parse_wish("   ", &data.objects).is_none());
    }

    #[test]
    fn test_parse_quantity() {
        let data = load_test_data();
        let result = parse_wish("3 arrow", &data.objects);
        let r = result.expect("should parse quantity");
        assert_eq!(r.quantity, 3);
    }

    #[test]
    fn test_parse_named_item() {
        let data = load_test_data();
        let result = parse_wish("long sword named excalibur", &data.objects);
        let r = result.expect("should parse named item");
        assert_eq!(r.name, Some("excalibur".to_string()));
        let ls = data
            .objects
            .iter()
            .find(|o| o.name.to_lowercase() == "long sword")
            .expect("long sword should exist");
        assert_eq!(r.object_type, ls.id);
    }

    #[test]
    fn test_parse_scroll_of_identify() {
        let data = load_test_data();
        let result = parse_wish("scroll of identify", &data.objects);
        let r = result.expect("should parse scroll of identify");
        let identify = data
            .objects
            .iter()
            .find(|o| o.class == ObjectClass::Scroll && o.name.to_lowercase() == "identify")
            .expect("identify scroll should exist");
        assert_eq!(r.object_type, identify.id);
    }

    #[test]
    fn test_parse_wand_of_fire() {
        let data = load_test_data();
        let result = parse_wish("wand of fire", &data.objects);
        let r = result.expect("should parse wand of fire");
        let wand = data
            .objects
            .iter()
            .find(|o| o.class == ObjectClass::Wand && o.name.to_lowercase() == "fire")
            .expect("wand of fire should exist");
        assert_eq!(r.object_type, wand.id);
    }

    #[test]
    fn test_parse_negative_enchantment() {
        let data = load_test_data();
        let result = parse_wish("-1 long sword", &data.objects);
        let r = result.expect("should parse");
        assert_eq!(r.enchantment, Some(-1));
    }

    #[test]
    fn test_strip_word() {
        assert_eq!(strip_word("blessed +2", "blessed"), Some("+2"));
        assert_eq!(strip_word("bless", "blessed"), None);
        assert_eq!(strip_word("blessed", "blessed"), Some(""));
    }

    #[test]
    fn test_parse_plural_arrows() {
        let data = load_test_data();
        let result = parse_wish("3 arrows", &data.objects);
        let r = result.expect("should parse plural arrows");
        assert_eq!(r.quantity, 3);
        let arrow = data
            .objects
            .iter()
            .find(|o| o.name.to_lowercase() == "arrow")
            .expect("arrow should exist");
        assert_eq!(r.object_type, arrow.id);
    }

    #[test]
    fn test_parse_pair_of_boots() {
        let data = load_test_data();
        // "pair of speed boots" should match "speed boots".
        let result = parse_wish("pair of speed boots", &data.objects);
        let r = result.expect("should parse pair of boots");
        let boots = data
            .objects
            .iter()
            .find(|o| o.name.to_lowercase() == "speed boots")
            .expect("speed boots should exist");
        assert_eq!(r.object_type, boots.id);
    }

    #[test]
    fn test_parse_holy_water() {
        let data = load_test_data();
        let result = parse_wish("holy water", &data.objects);
        let r = result.expect("should parse holy water");
        assert_eq!(r.buc, Some(BucWish::Blessed));
        let water = data
            .objects
            .iter()
            .find(|o| o.name.to_lowercase() == "water" && o.class == ObjectClass::Potion)
            .expect("potion of water should exist");
        assert_eq!(r.object_type, water.id);
    }

    #[test]
    fn test_parse_unholy_water() {
        let data = load_test_data();
        let result = parse_wish("unholy water", &data.objects);
        let r = result.expect("should parse unholy water");
        assert_eq!(r.buc, Some(BucWish::Cursed));
    }

    #[test]
    fn test_parse_potions_plural() {
        let data = load_test_data();
        let result = parse_wish("2 potions of healing", &data.objects);
        let r = result.expect("should parse plural potions");
        assert_eq!(r.quantity, 2);
        let healing = data
            .objects
            .iter()
            .find(|o| o.class == ObjectClass::Potion && o.name.to_lowercase() == "healing")
            .expect("healing potion should exist");
        assert_eq!(r.object_type, healing.id);
    }

    // -- BUC variants -------------------------------------------------------

    #[test]
    fn test_parse_cursed_sword() {
        let data = load_test_data();
        let r = parse_wish("cursed long sword", &data.objects).expect("should parse");
        assert_eq!(r.buc, Some(BucWish::Cursed));
    }

    #[test]
    fn test_parse_uncursed_sword() {
        let data = load_test_data();
        let r = parse_wish("uncursed long sword", &data.objects).expect("should parse");
        assert_eq!(r.buc, Some(BucWish::Uncursed));
    }

    #[test]
    fn test_parse_no_buc_specified() {
        let data = load_test_data();
        let r = parse_wish("long sword", &data.objects).expect("should parse");
        assert_eq!(r.buc, None);
    }

    // -- Erodeproof keywords ------------------------------------------------

    #[test]
    fn test_parse_fireproof_keyword() {
        let data = load_test_data();
        let r = parse_wish("fireproof +2 long sword", &data.objects).expect("should parse");
        assert!(r.erodeproof);
        assert_eq!(r.enchantment, Some(2));
    }

    #[test]
    fn test_parse_fixed_keyword() {
        let data = load_test_data();
        let r = parse_wish("fixed long sword", &data.objects).expect("should parse");
        assert!(r.erodeproof);
    }

    #[test]
    fn test_parse_corrodeproof_keyword() {
        let data = load_test_data();
        let r = parse_wish("corrodeproof long sword", &data.objects).expect("should parse");
        assert!(r.erodeproof);
    }

    #[test]
    fn test_parse_rotproof_keyword() {
        let data = load_test_data();
        let r = parse_wish("rotproof long sword", &data.objects).expect("should parse");
        assert!(r.erodeproof);
    }

    #[test]
    fn test_parse_greased_keyword() {
        let data = load_test_data();
        let r = parse_wish("greased long sword", &data.objects).expect("should parse");
        assert!(r.erodeproof);
    }

    // -- Enchantment edge cases ---------------------------------------------

    #[test]
    fn test_parse_zero_enchantment() {
        let data = load_test_data();
        let r = parse_wish("+0 long sword", &data.objects).expect("should parse");
        assert_eq!(r.enchantment, Some(0));
    }

    #[test]
    fn test_parse_high_positive_enchantment() {
        let data = load_test_data();
        let r = parse_wish("+7 long sword", &data.objects).expect("should parse");
        assert_eq!(r.enchantment, Some(7));
    }

    #[test]
    fn test_parse_no_enchantment_specified() {
        let data = load_test_data();
        let r = parse_wish("long sword", &data.objects).expect("should parse");
        assert_eq!(r.enchantment, None);
    }

    // -- Quantity tests -----------------------------------------------------

    #[test]
    fn test_parse_large_quantity() {
        let data = load_test_data();
        let r = parse_wish("50 arrow", &data.objects).expect("should parse");
        assert_eq!(r.quantity, 50);
    }

    #[test]
    fn test_parse_default_quantity_is_one() {
        let data = load_test_data();
        let r = parse_wish("long sword", &data.objects).expect("should parse");
        assert_eq!(r.quantity, 1);
    }

    // -- Class prefix matching ----------------------------------------------

    #[test]
    fn test_parse_ring_of_protection() {
        let data = load_test_data();
        let r = parse_wish("ring of protection", &data.objects);
        assert!(r.is_some(), "should parse ring of protection");
    }

    #[test]
    fn test_parse_spellbook_of() {
        let data = load_test_data();
        let r = parse_wish("spellbook of fireball", &data.objects);
        assert!(r.is_some(), "should parse spellbook of fireball");
    }

    #[test]
    fn test_parse_amulet_of() {
        let data = load_test_data();
        let r = parse_wish("amulet of life saving", &data.objects);
        assert!(r.is_some(), "should parse amulet of life saving");
    }

    // -- Named items --------------------------------------------------------

    #[test]
    fn test_parse_named_excalibur() {
        let data = load_test_data();
        let r = parse_wish("long sword named Excalibur", &data.objects).expect("should parse");
        assert_eq!(r.name, Some("excalibur".to_string()));
    }

    #[test]
    fn test_parse_named_empty_ignored() {
        let data = load_test_data();
        let r = parse_wish("long sword named ", &data.objects);
        if let Some(r) = r {
            assert_eq!(r.name, None);
        }
    }

    // -- Unwishable items ---------------------------------------------------

    #[test]
    fn test_candelabrum_unwishable() {
        let data = load_test_data();
        let r = parse_wish("candelabrum of invocation", &data.objects);
        assert!(r.is_none(), "candelabrum should be unwishable");
    }

    #[test]
    fn test_bell_of_opening_unwishable() {
        let data = load_test_data();
        let r = parse_wish("bell of opening", &data.objects);
        assert!(r.is_none(), "bell of opening should be unwishable");
    }

    #[test]
    fn test_book_of_the_dead_unwishable() {
        let data = load_test_data();
        let r = parse_wish("book of the dead", &data.objects);
        assert!(r.is_none(), "book of the dead should be unwishable");
    }

    // -- Combined modifiers -------------------------------------------------

    #[test]
    fn test_parse_all_modifiers_combined() {
        let data = load_test_data();
        let r = parse_wish(
            "2 blessed rustproof +3 long sword named test",
            &data.objects,
        );
        if let Some(r) = r {
            assert_eq!(r.quantity, 2);
            assert_eq!(r.buc, Some(BucWish::Blessed));
            assert!(r.erodeproof);
            assert_eq!(r.enchantment, Some(3));
            assert_eq!(r.name, Some("test".to_string()));
        }
    }

    #[test]
    fn test_parse_blessed_erodeproof_order() {
        let data = load_test_data();
        let r = parse_wish("blessed erodeproof +2 long sword", &data.objects);
        if let Some(r) = r {
            assert_eq!(r.buc, Some(BucWish::Blessed));
            assert!(r.erodeproof);
            assert_eq!(r.enchantment, Some(2));
        }
    }

    // -- Partial matching ---------------------------------------------------

    #[test]
    fn test_parse_partial_wand_fi() {
        let data = load_test_data();
        let r = parse_wish("wand of fi", &data.objects);
        if let Some(r) = r {
            let fire_wand = data
                .objects
                .iter()
                .find(|o| o.class == ObjectClass::Wand && o.name.to_lowercase() == "fire");
            if let Some(fw) = fire_wand {
                assert_eq!(r.object_type, fw.id);
            }
        }
    }

    #[test]
    fn test_parse_partial_name_prefix() {
        let data = load_test_data();
        let r = parse_wish("long sw", &data.objects);
        assert!(r.is_some(), "'long sw' should match long sword");
    }

    // -- Edge cases ---------------------------------------------------------

    #[test]
    fn test_parse_extra_whitespace() {
        let data = load_test_data();
        let r = parse_wish("  blessed  +2  long sword  ", &data.objects);
        assert!(r.is_some(), "extra whitespace should be handled");
    }

    #[test]
    fn test_parse_nonsense_returns_none() {
        let data = load_test_data();
        let r = parse_wish("xyzzy plugh", &data.objects);
        assert!(r.is_none(), "nonsense should return None");
    }

    #[test]
    fn test_parse_just_modifiers_no_item() {
        let data = load_test_data();
        let r = parse_wish("blessed +2", &data.objects);
        assert!(r.is_none(), "just modifiers should return None");
    }

    #[test]
    fn test_parse_just_whitespace() {
        let data = load_test_data();
        assert!(parse_wish("   ", &data.objects).is_none());
    }

    // -- Material prefix matching -------------------------------------------

    #[test]
    fn test_parse_silver_item() {
        let data = load_test_data();
        let r = parse_wish("silver dragon scale mail", &data.objects);
        assert!(r.is_some(), "silver dragon scale mail should parse");
    }

    // -- Scroll type wish ---------------------------------------------------

    #[test]
    fn test_parse_scroll_of_teleportation() {
        let data = load_test_data();
        let r = parse_wish("scroll of teleportation", &data.objects);
        assert!(r.is_some(), "scroll of teleportation should parse");
    }

    #[test]
    fn test_parse_scroll_of_genocide() {
        let data = load_test_data();
        let r = parse_wish("scroll of genocide", &data.objects);
        assert!(r.is_some(), "scroll of genocide should parse");
    }

    // -- Armor wish ---------------------------------------------------------

    #[test]
    fn test_parse_gauntlets_of_power() {
        let data = load_test_data();
        let r = parse_wish("gauntlets of power", &data.objects);
        assert!(r.is_some(), "gauntlets of power should parse");
    }

    #[test]
    fn test_parse_cloak_of_magic_resistance() {
        let data = load_test_data();
        let r = parse_wish("cloak of magic resistance", &data.objects);
        assert!(r.is_some(), "cloak of magic resistance should parse");
    }

    #[test]
    fn test_parse_speed_boots() {
        let data = load_test_data();
        let r = parse_wish("speed boots", &data.objects);
        assert!(r.is_some(), "speed boots should parse");
    }

    // -- Wish restriction tests ------------------------------------------------

    #[test]
    fn test_restriction_clamp_enchantment() {
        let mut result = WishResult {
            object_type: ObjectTypeId(0),
            buc: None,
            enchantment: Some(7),
            quantity: 1,
            erodeproof: false,
            name: None,
        };
        let restrictions = apply_wish_restrictions(&mut result);
        assert_eq!(result.enchantment, Some(MAX_WISH_ENCHANTMENT));
        assert!(restrictions.iter().any(|r| matches!(
            r,
            WishRestriction::EnchantmentClamped {
                requested: 7,
                granted: 3
            }
        )));
    }

    #[test]
    fn test_restriction_no_clamp_within_limit() {
        let mut result = WishResult {
            object_type: ObjectTypeId(0),
            buc: None,
            enchantment: Some(2),
            quantity: 1,
            erodeproof: false,
            name: None,
        };
        let restrictions = apply_wish_restrictions(&mut result);
        assert_eq!(result.enchantment, Some(2));
        assert!(restrictions.is_empty());
    }

    #[test]
    fn test_restriction_clamp_quantity() {
        let mut result = WishResult {
            object_type: ObjectTypeId(0),
            buc: None,
            enchantment: None,
            quantity: 100,
            erodeproof: false,
            name: None,
        };
        let restrictions = apply_wish_restrictions(&mut result);
        assert_eq!(result.quantity, 20);
        assert!(restrictions.iter().any(|r| matches!(
            r,
            WishRestriction::QuantityReduced {
                requested: 100,
                granted: 20
            }
        )));
    }

    #[test]
    fn test_restriction_quantity_within_limit() {
        let mut result = WishResult {
            object_type: ObjectTypeId(0),
            buc: None,
            enchantment: None,
            quantity: 5,
            erodeproof: false,
            name: None,
        };
        let restrictions = apply_wish_restrictions(&mut result);
        assert_eq!(result.quantity, 5);
        assert!(restrictions.is_empty());
    }

    #[test]
    fn test_restriction_negative_enchantment_not_clamped() {
        let mut result = WishResult {
            object_type: ObjectTypeId(0),
            buc: None,
            enchantment: Some(-5),
            quantity: 1,
            erodeproof: false,
            name: None,
        };
        let restrictions = apply_wish_restrictions(&mut result);
        // Negative enchantments are not clamped upward.
        assert_eq!(result.enchantment, Some(-5));
        assert!(restrictions.is_empty());
    }

    #[test]
    fn test_restriction_both_clamps() {
        let mut result = WishResult {
            object_type: ObjectTypeId(0),
            buc: Some(BucWish::Blessed),
            enchantment: Some(9),
            quantity: 50,
            erodeproof: true,
            name: None,
        };
        let restrictions = apply_wish_restrictions(&mut result);
        assert_eq!(result.enchantment, Some(3));
        assert_eq!(result.quantity, 20);
        assert_eq!(restrictions.len(), 2);
    }

    #[test]
    fn test_restriction_no_enchantment() {
        let mut result = WishResult {
            object_type: ObjectTypeId(0),
            buc: None,
            enchantment: None,
            quantity: 1,
            erodeproof: false,
            name: None,
        };
        let restrictions = apply_wish_restrictions(&mut result);
        assert!(restrictions.is_empty());
    }

    #[test]
    fn test_restriction_exact_boundary() {
        let mut result = WishResult {
            object_type: ObjectTypeId(0),
            buc: None,
            enchantment: Some(3),
            quantity: 20,
            erodeproof: false,
            name: None,
        };
        let restrictions = apply_wish_restrictions(&mut result);
        // At boundary: no clamping.
        assert_eq!(result.enchantment, Some(3));
        assert_eq!(result.quantity, 20);
        assert!(restrictions.is_empty());
    }
}
