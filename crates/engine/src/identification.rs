//! Item identification mechanics: BUC testing, appearance shuffling,
//! type discovery, price identification, display name resolution,
//! English pluralization, article selection, and erosion descriptions.
//!
//! This module implements the multi-layered knowledge system from NetHack:
//! - **Type-level knowledge** (`IdentificationState`): tracks which object
//!   types have been identified, player-assigned "called" names, and the
//!   shuffled appearance table generated at game start.
//! - **Instance-level knowledge** (`KnowledgeState`, `BucStatus`): per-object
//!   flags stored as ECS components (defined in `nethack_babel_data`).
//! - **Name generation** (`xname`, `doname`): full item display name pipeline,
//!   ported from C `objnam.c`.

use hecs::Entity;
use rand::Rng;
use rand::seq::SliceRandom;

use nethack_babel_data::{
    Alignment, ArmorCategory, BucStatus, ContainerState, Enchantment, Erosion, KnowledgeState,
    Material, ObjectClass, ObjectCore, ObjectDef, ObjectTypeId,
};

use crate::event::EngineEvent;
use crate::world::GameWorld;

// ---------------------------------------------------------------------------
// Appearance classes — which object classes get shuffled appearances
// ---------------------------------------------------------------------------

/// Object classes whose appearances are shuffled at game start.
const SHUFFLED_CLASSES: &[ObjectClass] = &[
    ObjectClass::Potion,
    ObjectClass::Scroll,
    ObjectClass::Ring,
    ObjectClass::Wand,
    ObjectClass::Spellbook,
    ObjectClass::Amulet,
];

/// Returns `true` if the given class participates in appearance shuffling.
fn is_shuffled_class(class: ObjectClass) -> bool {
    SHUFFLED_CLASSES.contains(&class)
}

// ---------------------------------------------------------------------------
// IdentificationState — per-game type-level knowledge
// ---------------------------------------------------------------------------

/// Tracks what the player knows about object *types* (not individual
/// instances).  One `IdentificationState` is created per game and persists
/// across save/load.
///
/// Indexed by `ObjectTypeId.0 as usize`.
#[derive(Debug, Clone)]
pub struct IdentificationState {
    /// Whether the real name of this type is known to the player.
    /// Corresponds to `oc_name_known` in C NetHack.
    pub type_known: Vec<bool>,

    /// Player-assigned "called" labels for unknown types.
    /// Corresponds to `oc_uname` in C NetHack.
    pub type_called: Vec<Option<String>>,

    /// Shuffled appearance descriptions assigned at game start.
    /// For types that don't participate in shuffling, this is `None`.
    /// Corresponds to the shuffled `oc_descr_idx` in C NetHack.
    pub appearances: Vec<Option<String>>,
}

impl IdentificationState {
    /// Create an empty identification state sized for `num_types` object
    /// types.  All types start as unidentified with no appearances.
    pub fn new(num_types: usize) -> Self {
        Self {
            type_known: vec![false; num_types],
            type_called: vec![None; num_types],
            appearances: vec![None; num_types],
        }
    }

    /// Mark a type as known (discovered).  After this, all items of this
    /// type display their real name.
    pub fn discover_type(&mut self, otyp: ObjectTypeId) {
        let idx = otyp.0 as usize;
        if idx < self.type_known.len() {
            self.type_known[idx] = true;
        }
    }

    /// Set the player's "called" label for a type.
    pub fn set_called(&mut self, otyp: ObjectTypeId, name: String) {
        let idx = otyp.0 as usize;
        if idx < self.type_called.len() {
            self.type_called[idx] = Some(name);
        }
    }

    /// Clear the player's "called" label for a type.
    pub fn clear_called(&mut self, otyp: ObjectTypeId) {
        let idx = otyp.0 as usize;
        if idx < self.type_called.len() {
            self.type_called[idx] = None;
        }
    }

    /// Query whether a type is known.
    pub fn is_type_known(&self, otyp: ObjectTypeId) -> bool {
        let idx = otyp.0 as usize;
        idx < self.type_known.len() && self.type_known[idx]
    }

    /// Get the shuffled appearance for a type, if any.
    pub fn appearance(&self, otyp: ObjectTypeId) -> Option<&str> {
        let idx = otyp.0 as usize;
        self.appearances.get(idx).and_then(|a| a.as_deref())
    }

    /// Get the player's "called" label for a type, if any.
    pub fn called(&self, otyp: ObjectTypeId) -> Option<&str> {
        let idx = otyp.0 as usize;
        self.type_called.get(idx).and_then(|c| c.as_deref())
    }
}

// ---------------------------------------------------------------------------
// Appearance shuffling
// ---------------------------------------------------------------------------

/// Initialize shuffled appearances for all object types that participate
/// in appearance randomization (potions, scrolls, rings, wands, spellbooks,
/// amulets).
///
/// Within each class the pool of appearance descriptions (from
/// `ObjectDef.appearance`) is collected and then shuffled using the
/// provided RNG, producing a random mapping from type to appearance that
/// is consistent for the entire game session.
///
/// Types that are unique, non-magic (within shuffled classes), or lack an
/// appearance description are excluded from shuffling and keep their
/// original (or no) appearance.
pub fn init_appearances<R: Rng>(obj_defs: &[ObjectDef], rng: &mut R) -> IdentificationState {
    let num_types = obj_defs.len();
    let mut state = IdentificationState::new(num_types);

    for &class in SHUFFLED_CLASSES {
        // Collect indices and appearance strings for this class.
        let (indices, mut descs): (Vec<usize>, Vec<String>) = obj_defs
            .iter()
            .filter(|def| def.class == class && !def.is_unique)
            .filter_map(|def| {
                def.appearance
                    .as_ref()
                    .map(|app| (def.id.0 as usize, app.clone()))
            })
            .unzip();

        if descs.len() < 2 {
            // Not enough items to shuffle; assign originals.
            for (i, desc) in indices.iter().zip(descs.iter()) {
                if *i < num_types {
                    state.appearances[*i] = Some(desc.clone());
                }
            }
            continue;
        }

        // Shuffle the descriptions.
        descs.shuffle(rng);

        // Assign shuffled descriptions back to the type indices.
        for (i, desc) in indices.iter().zip(descs) {
            if *i < num_types {
                state.appearances[*i] = Some(desc);
            }
        }
    }

    // For non-shuffled types that have an appearance, copy it through
    // unchanged (e.g. armor sub-types with fixed descriptions).
    let passthrough: Vec<(usize, String)> = obj_defs
        .iter()
        .filter(|def| !is_shuffled_class(def.class))
        .filter_map(|def| {
            let idx = def.id.0 as usize;
            def.appearance
                .as_ref()
                .filter(|_| idx < num_types && state.appearances[idx].is_none())
                .map(|app| (idx, app.clone()))
        })
        .collect();
    for (idx, app) in passthrough {
        state.appearances[idx] = Some(app);
    }

    state
}

// ---------------------------------------------------------------------------
// Full identification
// ---------------------------------------------------------------------------

/// Fully identify a single item instance: set all per-instance knowledge
/// flags to `true`, and mark the type as known.
///
/// Emits `EngineEvent::ItemIdentified`.
pub fn identify_item(
    world: &mut GameWorld,
    item_entity: Entity,
    id_state: &mut IdentificationState,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Read the object type.
    let otyp = match world.get_component::<ObjectCore>(item_entity) {
        Some(core) => core.otyp,
        None => return events,
    };

    // Mark type as known.
    id_state.discover_type(otyp);

    // Set all per-instance knowledge flags.
    if let Some(mut knowledge) = world.get_component_mut::<KnowledgeState>(item_entity) {
        knowledge.known = true;
        knowledge.dknown = true;
        knowledge.rknown = true;
        knowledge.cknown = true;
        knowledge.lknown = true;
        knowledge.tknown = true;
    }

    // Reveal BUC status.
    if let Some(mut buc) = world.get_component_mut::<BucStatus>(item_entity) {
        buc.bknown = true;
    }

    events.push(EngineEvent::ItemIdentified { item: item_entity });
    events
}

/// Mark a type as known through use (e.g. quaffing a potion reveals the
/// potion type for all future potions of that kind).
pub fn use_identify(
    world: &mut GameWorld,
    item_entity: Entity,
    id_state: &mut IdentificationState,
) {
    let otyp = match world.get_component::<ObjectCore>(item_entity) {
        Some(core) => core.otyp,
        None => return,
    };
    id_state.discover_type(otyp);

    // Also mark dknown on the used instance if not already set.
    if let Some(mut knowledge) = world.get_component_mut::<KnowledgeState>(item_entity) {
        knowledge.dknown = true;
    }
}

/// Check whether an item is NOT fully identified — i.e. still needs
/// further identification.  Mirrors C `not_fully_identified()`.
pub fn not_fully_identified(
    world: &GameWorld,
    item: Entity,
    id_state: &IdentificationState,
    obj_defs: &[ObjectDef],
) -> bool {
    let core = match world.get_component::<ObjectCore>(item) {
        Some(c) => c,
        None => return false,
    };

    // Gold is always fully identified.
    if core.object_class == ObjectClass::Coin {
        return false;
    }

    let knowledge = world.get_component::<KnowledgeState>(item);
    let buc = world.get_component::<BucStatus>(item);

    // Check basic knowledge flags.
    if let Some(ref k) = knowledge {
        if !k.known || !k.dknown {
            return true;
        }
    } else {
        return true;
    }

    if let Some(ref b) = buc {
        if !b.bknown {
            return true;
        }
    }

    // Check type-level knowledge.
    if !id_state.is_type_known(core.otyp) {
        return true;
    }

    // Check container-related flags.
    let k = knowledge.as_ref().unwrap();
    if !k.cknown && matches!(core.object_class, ObjectClass::Tool | ObjectClass::Rock) {
        // Only matters for actual containers/statues, but we approximate
        // by checking the class.
        return true;
    }

    // Check rknown for damageable items.
    if !k.rknown {
        let def = obj_defs.iter().find(|d| d.id == core.otyp);
        if let Some(d) = def {
            if is_damageable_material(d.material)
                && matches!(
                    core.object_class,
                    ObjectClass::Weapon
                        | ObjectClass::Armor
                        | ObjectClass::Tool
                        | ObjectClass::Ball
                        | ObjectClass::Chain
                )
            {
                return true;
            }
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Altar BUC testing
// ---------------------------------------------------------------------------

/// Result of dropping an item on an altar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AltarFlash {
    /// Amber flash — item is blessed.
    Amber,
    /// Black flash — item is cursed.
    Black,
    /// No flash — item is uncursed.
    None,
}

/// Reveal an item's BUC status by dropping it on an altar, and emit the
/// appropriate event describing the flash (or lack thereof).
///
/// Returns the flash type and a list of engine events.
pub fn test_buc_on_altar(
    world: &mut GameWorld,
    item_entity: Entity,
    _altar_alignment: Alignment,
) -> (AltarFlash, Vec<EngineEvent>) {
    let mut events = Vec::new();

    let (blessed, cursed) = match world.get_component::<BucStatus>(item_entity) {
        Some(buc) => (buc.blessed, buc.cursed),
        None => return (AltarFlash::None, events),
    };

    let flash = if blessed {
        AltarFlash::Amber
    } else if cursed {
        AltarFlash::Black
    } else {
        AltarFlash::None
    };

    // Compose message.
    let key = match flash {
        AltarFlash::Amber => "altar-buc-blessed",
        AltarFlash::Black => "altar-buc-cursed",
        AltarFlash::None => "altar-buc-unknown",
    };

    events.push(EngineEvent::msg(key));

    // Set bknown.
    if let Some(mut buc) = world.get_component_mut::<BucStatus>(item_entity) {
        buc.bknown = true;
    }

    (flash, events)
}

// ---------------------------------------------------------------------------
// Price identification
// ---------------------------------------------------------------------------

/// A candidate object type that matches the observed shop price.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceCandidate {
    /// The object type that could match.
    pub otyp: ObjectTypeId,
    /// The real name of the type.
    pub name: String,
    /// The base cost (oc_cost) from the definition.
    pub base_cost: i16,
}

/// Given a shop price observed by the player and their charisma, determine
/// which object types within the same class could produce that price.
pub fn try_price_id(
    obj_defs: &[ObjectDef],
    target_class: ObjectClass,
    shop_price: i32,
    charisma: u8,
) -> Vec<PriceCandidate> {
    obj_defs
        .iter()
        .filter(|def| def.class == target_class && def.cost as i32 > 0)
        .filter(|def| apply_charisma_modifier(def.cost as i32, charisma) == shop_price)
        .map(|def| PriceCandidate {
            otyp: def.id,
            name: def.name.clone(),
            base_cost: def.cost,
        })
        .collect()
}

/// Apply charisma-based price modification, mirroring `get_cost()` in C.
fn apply_charisma_modifier(base: i32, cha: u8) -> i32 {
    let (mult, div): (i32, i32) = match cha {
        _ if cha > 18 => (1, 2),
        18 => (2, 3),
        16..=17 => (3, 4),
        11..=15 => (1, 1),
        8..=10 => (4, 3),
        6..=7 => (3, 2),
        _ => (2, 1),
    };

    let tmp = base * mult;
    if div > 1 {
        ((tmp * 10) / div + 5) / 10
    } else {
        tmp
    }
}

// ---------------------------------------------------------------------------
// English article selection: just_an(), an(), the()
// ---------------------------------------------------------------------------

/// Determine the indefinite article for a string.
/// Returns `""`, `"a "`, or `"an "`.
///
/// Mirrors C `just_an()` from objnam.c:2113.
pub fn just_an(s: &str) -> &'static str {
    let s = s.trim();
    if s.is_empty() {
        return "a ";
    }

    let bytes = s.as_bytes();
    let c0 = bytes[0].to_ascii_lowercase();
    let is_single = bytes.len() == 1 || bytes[1] == b' ';

    // Single character: specific set gets "an".
    if is_single {
        if b"aefhilmnosx".contains(&c0) {
            return "an ";
        } else {
            return "a ";
        }
    }

    let lower = s.to_lowercase();

    // Special no-article cases.
    if lower.starts_with("the ") {
        return "";
    }
    if lower == "molten lava" || lower == "iron bars" || lower == "ice" {
        return "";
    }

    // Vowel-initial words with consonant pronunciation.
    if b"aeiou".contains(&c0) {
        if lower.starts_with("one")
            && bytes.len() > 3
            && !matches!(bytes[3], b'-' | b'_' | b' ' | 0)
        {
            return "a ";
        }
        if lower.starts_with("eu")
            || lower.starts_with("uke")
            || lower.starts_with("ukulele")
            || lower.starts_with("unicorn")
            || lower.starts_with("uranium")
            || lower.starts_with("useful")
        {
            return "a ";
        }
        return "an ";
    }

    // "x" + consonant sounds like /z/ or /eks/ -> "an".
    if c0 == b'x' && bytes.len() > 1 && !b"aeiouAEIOU".contains(&bytes[1]) {
        return "an ";
    }

    "a "
}

/// Prepend "a"/"an" + space to `s`.
pub fn an(s: &str) -> String {
    format!("{}{}", just_an(s), s)
}

/// Prepend "the " to `s`, following NetHack's `the()` rules.
pub fn the(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }
    let lower = s.to_lowercase();
    if lower.starts_with("the ") {
        // Already has "the", just lowercase the T if needed.
        let mut chars: Vec<char> = s.chars().collect();
        chars[0] = 't';
        return chars.into_iter().collect();
    }

    let first = s.chars().next().unwrap();
    // If first char is lowercase or a digit/symbol, add "the ".
    if !first.is_ascii_uppercase() {
        return format!("the {}", s);
    }

    // If contains " of " before " named "/" called ", add "the ".
    if let Some(of_pos) = lower.find(" of ") {
        let named_pos = lower.find(" named ").unwrap_or(usize::MAX);
        let called_pos = lower.find(" called ").unwrap_or(usize::MAX);
        if of_pos < named_pos.min(called_pos) {
            return format!("the {}", s);
        }
    }

    // Proper noun — no "the".
    s.to_string()
}

// ---------------------------------------------------------------------------
// English pluralization: makeplural()
// ---------------------------------------------------------------------------

/// Invariant plurals — these words do not change form.
const AS_IS: &[&str] = &[
    "boots",
    "shoes",
    "gloves",
    "lenses",
    "scales",
    "eyes",
    "gauntlets",
    "iron bars",
    "bison",
    "deer",
    "elk",
    "fish",
    "fowl",
    "tuna",
    "yaki",
    "krill",
    "manes",
    "moose",
    "ninja",
    "sheep",
    "ronin",
    "roshi",
    "shito",
    "tengu",
    "ki-rin",
    "Nazgul",
    "gunyoki",
    "piranha",
    "samurai",
    "shuriken",
    "haggis",
    "Bordeaux",
];

/// Irregular singular→plural suffix pairs.
const ONE_OFF: &[(&str, &str)] = &[
    ("child", "children"),
    ("cubus", "cubi"),
    ("culus", "culi"),
    ("Cyclops", "Cyclopes"),
    ("djinni", "djinn"),
    ("erinys", "erinyes"),
    ("foot", "feet"),
    ("fungus", "fungi"),
    ("goose", "geese"),
    ("knife", "knives"),
    ("labrum", "labra"),
    ("louse", "lice"),
    ("mouse", "mice"),
    ("mumak", "mumakil"),
    ("nemesis", "nemeses"),
    ("ovum", "ova"),
    ("ox", "oxen"),
    ("passerby", "passersby"),
    ("rtex", "rtices"),
    ("serum", "sera"),
    ("staff", "staves"),
    ("tooth", "teeth"),
];

/// Prefixes where -man should NOT become -men.
const BADMAN: &[&str] = &[
    "albu",
    "antihu",
    "anti",
    "ata",
    "auto",
    "bildungsro",
    "cai",
    "cay",
    "ceru",
    "corner",
    "decu",
    "des",
    "dura",
    "fir",
    "hanu",
    "het",
    "infrahu",
    "inhu",
    "nonhu",
    "otto",
    "out",
    "prehu",
    "protohu",
    "subhu",
    "superhu",
    "talis",
    "unhu",
    "sha",
    "hu",
    "un",
    "le",
    "re",
    "so",
    "to",
    "at",
    "a",
];

/// Words ending in -ch that take -s (not -es) because the ch is /k/.
const CH_KSOUND: &[&str] = &[
    "monarch",
    "poch",
    "tech",
    "mech",
    "stomach",
    "psych",
    "amphibrach",
    "anarch",
    "atriarch",
    "azedarach",
    "broch",
    "gastrotrich",
    "isopach",
    "loch",
    "oligarch",
    "peritrich",
    "sandarach",
    "sumach",
    "symposiarch",
];

/// Pluralize an English word, mirroring C `makeplural()`.
pub fn makeplural(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }

    // "pair of " prefix: invariant.
    if s.starts_with("pair of ") {
        return s.to_string();
    }

    // Compound word splitting: pluralize only the part before the separator.
    let separators = &[
        " of ",
        " labeled ",
        " called ",
        " named ",
        " above",
        " versus ",
        " from ",
        " in ",
        " on ",
        " a la ",
        " with",
        " de ",
        " d'",
        " du ",
        " au ",
    ];
    let infix_seps = &["-in-", "-at-"];

    for sep in separators {
        if let Some(idx) = s.find(sep) {
            let (head, tail) = s.split_at(idx);
            return format!("{}{}", makeplural(head), tail);
        }
    }
    for sep in infix_seps {
        if let Some(idx) = s.find(sep) {
            let (head, tail) = s.split_at(idx);
            return format!("{}{}", makeplural(head), tail);
        }
    }

    let lower = s.to_lowercase();

    // Single character or non-alpha ending: add 's.
    if s.len() == 1 || !s.chars().last().unwrap().is_ascii_alphabetic() {
        return format!("{}'s", s);
    }

    // Check as-is invariants.
    for &word in AS_IS {
        if lower.ends_with(&word.to_lowercase()) {
            return s.to_string();
        }
    }
    // "craft" suffix (length > 5).
    if s.len() > 5 && lower.ends_with("craft") {
        return s.to_string();
    }
    // Already-plural suffixes.
    if lower.ends_with("ae") || lower.ends_with("eaux") || lower.ends_with("matzot") {
        return s.to_string();
    }

    // Special anti-mismatches.
    if lower.ends_with("slice") {
        return format!("{}s", s);
    }
    if lower.ends_with("mongoose") {
        return format!("{}s", s);
    }

    // Special: "ox" -> "oxen" only for standalone "ox" or "muskox".
    // Other -ox words like "fox" -> "foxes" via normal rules.
    if lower == "ox" || lower.ends_with("muskox") {
        let base = &s[..s.len() - 2];
        return format!("{}oxen", base);
    }

    // One-off irregular suffixes.
    for &(singular, plural) in ONE_OFF {
        // Skip "ox" — handled above.
        if singular == "ox" {
            continue;
        }
        if lower.ends_with(&singular.to_lowercase()) {
            let base_len = s.len() - singular.len();
            let base = &s[..base_len];
            return format!("{}{}", base, plural);
        }
    }

    // -man -> -men (unless in badman list).
    if lower.ends_with("man") && s.len() > 3 {
        let prefix = &lower[..lower.len() - 3];
        let is_badman = BADMAN.iter().any(|b| prefix.ends_with(b));
        if !is_badman {
            let base = &s[..s.len() - 3];
            return format!("{}men", base);
        }
    }

    // [aeioulr]f -> [aeioulr]ves (but not -erf).
    if lower.ends_with('f') && s.len() >= 2 && !lower.ends_with("erf") {
        let penult = lower.as_bytes()[lower.len() - 2];
        if b"aeioulr".contains(&penult) {
            let base = &s[..s.len() - 1];
            return format!("{}ves", base);
        }
    }

    // -ium -> -ia.
    if lower.ends_with("ium") {
        let base = &s[..s.len() - 2];
        return format!("{}a", base);
    }

    // Latin/Greek -a endings that add -e.
    for suffix in &["alga", "hypha", "larva", "amoeba", "vertebra"] {
        if lower.ends_with(suffix) {
            return format!("{}e", s);
        }
    }

    // -us -> -i (but not lotus, wumpus).
    if lower.ends_with("us") && !lower.ends_with("lotus") && !lower.ends_with("wumpus") {
        let base = &s[..s.len() - 2];
        return format!("{}i", base);
    }

    // -sis -> -ses.
    if lower.ends_with("sis") {
        let base = &s[..s.len() - 2];
        return format!("{}es", base);
    }

    // -eau -> -eaux.
    if lower.ends_with("eau") && !lower.ends_with("bureau") {
        return format!("{}x", s);
    }

    // -matzoh/-matzah -> -matzot; -matzo/-matza -> -matzot.
    if lower.ends_with("matzoh") || lower.ends_with("matzah") {
        let base = &s[..s.len() - 6];
        return format!("{}matzot", base);
    }
    if lower.ends_with("matzo") || lower.ends_with("matza") {
        let base = &s[..s.len() - 5];
        return format!("{}matzot", base);
    }

    // -dex/-dix/-tex -> -ices (not index).
    if (lower.ends_with("dex") || lower.ends_with("dix") || lower.ends_with("tex"))
        && !lower.ends_with("index")
    {
        let base = &s[..s.len() - 2];
        return format!("{}ices", base);
    }

    // Sibilant endings: -z, -x, -s, -ch, -sh -> +es.
    if lower.ends_with('z') || lower.ends_with('x') || lower.ends_with('s') || lower.ends_with("sh")
    {
        return format!("{}es", s);
    }
    if lower.ends_with("ch") {
        // Check k-sound exceptions.
        let is_ksound = CH_KSOUND.iter().any(|w| lower.ends_with(w));
        if is_ksound {
            return format!("{}s", s);
        }
        return format!("{}es", s);
    }

    // -ato, -dingo -> +es.
    if lower.ends_with("ato") || lower.ends_with("dingo") {
        return format!("{}es", s);
    }

    // Consonant + y -> -ies.
    if lower.ends_with('y') && s.len() >= 2 {
        let penult = lower.as_bytes()[lower.len() - 2];
        if !b"aeiou".contains(&penult) {
            let base = &s[..s.len() - 1];
            return format!("{}ies", base);
        }
    }

    // Default: +s.
    format!("{}s", s)
}

// ---------------------------------------------------------------------------
// English singularization: makesingular()
// ---------------------------------------------------------------------------

/// Attempt to convert a plural English word back to singular.
/// This is a simplified reverse of `makeplural()`.
pub fn makesingular(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }

    let lower = s.to_lowercase();

    // Compound word splitting.
    let separators = &[
        " of ",
        " labeled ",
        " called ",
        " named ",
        " above",
        " versus ",
        " from ",
        " in ",
        " on ",
        " a la ",
        " with",
        " de ",
        " d'",
        " du ",
        " au ",
    ];
    for sep in separators {
        if let Some(idx) = s.find(sep) {
            let (head, tail) = s.split_at(idx);
            return format!("{}{}", makesingular(head), tail);
        }
    }

    // "pair of " invariant.
    if s.starts_with("pair of ") {
        return s.to_string();
    }

    // Check as-is invariants (already singular/invariant form).
    for &word in AS_IS {
        if lower == word.to_lowercase() {
            return s.to_string();
        }
    }

    // Reverse one-off irregulars.
    for &(_singular, plural) in ONE_OFF {
        if lower.ends_with(&plural.to_lowercase()) {
            let base_len = s.len() - plural.len();
            let base = &s[..base_len];
            return format!("{}{}", base, _singular);
        }
    }

    // -oxen -> -ox (muskoxen -> muskox).
    if lower.ends_with("oxen") {
        let base = &s[..s.len() - 2];
        return base.to_string();
    }

    // -men -> -man (but not badman singularize list).
    if lower.ends_with("men") && s.len() > 3 {
        let base = &s[..s.len() - 3];
        return format!("{}man", base);
    }

    // -ves -> -f or -fe.
    if lower.ends_with("ves") && s.len() > 3 {
        // Try -f first (e.g. "halves" -> "half").
        let base = &s[..s.len() - 3];
        return format!("{}f", base);
    }

    // -ia -> -ium.
    if lower.ends_with("ia") && !lower.ends_with("militia") {
        let base = &s[..s.len() - 1];
        return format!("{}um", base);
    }

    // -i -> -us (fungi -> fungus).
    if lower.ends_with('i') && s.len() > 2 {
        let base = &s[..s.len() - 1];
        return format!("{}us", base);
    }

    // -ses -> -sis (nemeses -> nemesis).
    if lower.ends_with("ses") {
        let base = &s[..s.len() - 2];
        return format!("{}is", base);
    }

    // -ices -> -ex (vortices -> vortex).
    if lower.ends_with("ices") {
        let base = &s[..s.len() - 4];
        return format!("{}ex", base);
    }

    // -ies -> -y (rubies -> ruby).
    if lower.ends_with("ies") && s.len() > 3 {
        let base = &s[..s.len() - 3];
        return format!("{}y", base);
    }

    // -eaux -> -eau.
    if lower.ends_with("eaux") {
        let base = &s[..s.len() - 1];
        return base.to_string();
    }

    // -es -> remove -es (torches -> torch, glasses -> glass, foxes -> fox).
    if lower.ends_with("ches")
        || lower.ends_with("shes")
        || lower.ends_with("xes")
        || lower.ends_with("zes")
        || lower.ends_with("sses")
    {
        return s[..s.len() - 2].to_string();
    }

    // General -s -> remove -s.
    if lower.ends_with('s') && !lower.ends_with("ss") {
        return s[..s.len() - 1].to_string();
    }

    s.to_string()
}

// ---------------------------------------------------------------------------
// Erosion description
// ---------------------------------------------------------------------------

/// Material-based erosion predicates.
pub fn is_rustprone(mat: Material) -> bool {
    mat == Material::Iron
}

pub fn is_corrodeable(mat: Material) -> bool {
    matches!(mat, Material::Iron | Material::Copper)
}

pub fn is_flammable(mat: Material) -> bool {
    matches!(
        mat,
        Material::Wax
            | Material::Veggy
            | Material::Cloth
            | Material::Leather
            | Material::Wood
            | Material::Paper
    )
}

pub fn is_rottable(mat: Material) -> bool {
    matches!(
        mat,
        Material::Wax
            | Material::Veggy
            | Material::Cloth
            | Material::Leather
            | Material::Wood
            | Material::DragonHide
    )
}

pub fn is_crackable(mat: Material, class: ObjectClass) -> bool {
    mat == Material::Glass && class == ObjectClass::Armor
}

/// Whether the material is subject to any erosion type.
pub fn is_damageable_material(mat: Material) -> bool {
    is_rustprone(mat)
        || is_flammable(mat)
        || is_rottable(mat)
        || is_corrodeable(mat)
        || mat == Material::Glass
}

/// Build the erosion prefix string for an item.
///
/// Mirrors C `add_erosion_words()` from objnam.c:1156.
pub fn erosion_prefix(
    erosion: &Erosion,
    mat: Material,
    class: ObjectClass,
    rknown: bool,
) -> String {
    let mut prefix = String::new();
    let is_crys = false; // Crysknife detection would need otyp; simplified here.

    if !is_damageable_material(mat) && !is_crys {
        // Only erodeproof matters for non-damageable items.
        if rknown && erosion.erodeproof {
            prefix.push_str("fixed ");
        }
        return prefix;
    }

    // Primary erosion (oeroded: 0-3).
    if erosion.eroded > 0 && !is_crys {
        match erosion.eroded {
            2 => prefix.push_str("very "),
            3 => prefix.push_str("thoroughly "),
            _ => {}
        }
        if is_rustprone(mat) {
            prefix.push_str("rusty ");
        } else if is_crackable(mat, class) {
            prefix.push_str("cracked ");
        } else {
            prefix.push_str("burnt ");
        }
    }

    // Secondary erosion (oeroded2: 0-3).
    if erosion.eroded2 > 0 && !is_crys {
        match erosion.eroded2 {
            2 => prefix.push_str("very "),
            3 => prefix.push_str("thoroughly "),
            _ => {}
        }
        if is_corrodeable(mat) {
            prefix.push_str("corroded ");
        } else {
            prefix.push_str("rotted ");
        }
    }

    // Erodeproof.
    if rknown && erosion.erodeproof {
        if is_crys {
            prefix.push_str("fixed ");
        } else if is_rustprone(mat) {
            prefix.push_str("rustproof ");
        } else if is_corrodeable(mat) {
            prefix.push_str("corrodeproof ");
        } else if is_flammable(mat) {
            prefix.push_str("fireproof ");
        } else if is_crackable(mat, class) {
            prefix.push_str("tempered ");
        } else if is_rottable(mat) {
            prefix.push_str("rotproof ");
        }
    }

    prefix
}

// ---------------------------------------------------------------------------
// Armor simple name
// ---------------------------------------------------------------------------

/// Return the simplified category name for an armor piece, used in
/// "called" labeling.  Mirrors C `armor_simple_name()`.
pub fn armor_simple_name(def: &ObjectDef) -> &str {
    let armor_info = match def.armor.as_ref() {
        Some(a) => a,
        None => return "armor",
    };
    let lower_name = def.name.to_lowercase();

    match armor_info.category {
        ArmorCategory::Suit => {
            if lower_name.contains("dragon") {
                if lower_name.contains("scale mail") || lower_name.contains("mail") {
                    "dragon mail"
                } else {
                    "dragon scales"
                }
            } else if lower_name.contains("mail") {
                "mail"
            } else if lower_name.contains("jacket") {
                "jacket"
            } else {
                "suit"
            }
        }
        ArmorCategory::Cloak => {
            if lower_name.contains("robe") {
                "robe"
            } else if lower_name.contains("wrapping") {
                "wrapping"
            } else {
                "cloak"
            }
        }
        ArmorCategory::Helm => {
            if lower_name.contains("hat") || lower_name.contains("fedora") {
                "hat"
            } else {
                "helm"
            }
        }
        ArmorCategory::Gloves => "gloves",
        ArmorCategory::Boots => "boots",
        ArmorCategory::Shield => "shield",
        ArmorCategory::Shirt => "shirt",
    }
}

// ---------------------------------------------------------------------------
// Display name resolution: xname / doname
// ---------------------------------------------------------------------------

/// Human-readable class name for display when item is unidentified.
fn class_display_name(class: ObjectClass) -> &'static str {
    match class {
        ObjectClass::Potion => "potion",
        ObjectClass::Scroll => "scroll",
        ObjectClass::Ring => "ring",
        ObjectClass::Wand => "wand",
        ObjectClass::Spellbook => "spellbook",
        ObjectClass::Amulet => "amulet",
        ObjectClass::Weapon => "weapon",
        ObjectClass::Armor => "armor",
        ObjectClass::Tool => "tool",
        ObjectClass::Food => "food",
        ObjectClass::Coin => "gold piece",
        ObjectClass::Gem => "gem",
        ObjectClass::Rock => "rock",
        ObjectClass::Ball => "iron ball",
        ObjectClass::Chain => "iron chain",
        ObjectClass::Venom => "splash of venom",
        _ => "thing",
    }
}

/// Return the canonical type name for an object type, as it would appear
/// when the type is fully identified.  For class-prefixed types this
/// returns "scroll of identify", "potion of healing", etc.
///
/// Corresponds to C `typename()` from objnam.c.
pub fn typename(otyp: ObjectTypeId, obj_defs: &[ObjectDef]) -> String {
    let def = match obj_defs.iter().find(|d| d.id == otyp) {
        Some(d) => d,
        None => return "strange object".to_string(),
    };

    let name = &def.name;
    match def.class {
        ObjectClass::Potion => format!("potion of {}", name),
        ObjectClass::Scroll => format!("scroll of {}", name),
        ObjectClass::Wand => format!("wand of {}", name),
        ObjectClass::Ring => format!("ring of {}", name),
        ObjectClass::Spellbook => format!("spellbook of {}", name),
        ObjectClass::Amulet => format!("amulet of {}", name),
        _ => name.to_string(),
    }
}

/// Generate the base name for an item (no quantity/BUC/enchantment prefix).
///
/// This is the Rust equivalent of C `xname()`.  It produces the core item
/// name based on object class, identification state, and knowledge flags.
pub fn xname(
    item: Entity,
    world: &GameWorld,
    id_state: &IdentificationState,
    obj_defs: &[ObjectDef],
) -> String {
    let core = match world.get_component::<ObjectCore>(item) {
        Some(c) => c,
        None => return "something".to_string(),
    };

    let knowledge = world.get_component::<KnowledgeState>(item);
    let buc = world.get_component::<BucStatus>(item);
    let otyp = core.otyp;
    let obj_def = obj_defs.iter().find(|d| d.id == otyp);

    let real_name = obj_def.map(|d| d.name.as_str()).unwrap_or("strange object");
    let dn = obj_def
        .and_then(|d| id_state.appearance(otyp).or(d.appearance.as_deref()))
        .unwrap_or(real_name);
    let nn = id_state.is_type_known(otyp);
    let un = id_state.called(otyp);
    let dknown = knowledge.as_ref().is_some_and(|k| k.dknown);
    let class = core.object_class;

    let base = match class {
        ObjectClass::Amulet => {
            if !dknown {
                "amulet".to_string()
            } else if nn {
                format!("amulet of {}", real_name)
            } else if let Some(u) = un {
                format!("amulet called {}", u)
            } else {
                format!("{} amulet", dn)
            }
        }
        ObjectClass::Weapon | ObjectClass::Venom | ObjectClass::Tool => {
            if !dknown {
                dn.to_string()
            } else if nn {
                real_name.to_string()
            } else if let Some(u) = un {
                format!("{} called {}", dn, u)
            } else {
                dn.to_string()
            }
        }
        ObjectClass::Armor => {
            let def = obj_def.unwrap();
            let is_boots = def
                .armor
                .as_ref()
                .is_some_and(|a| a.category == ArmorCategory::Boots);
            let is_gloves = def
                .armor
                .as_ref()
                .is_some_and(|a| a.category == ArmorCategory::Gloves);
            let mut prefix = String::new();
            if is_boots || is_gloves {
                prefix.push_str("pair of ");
            }
            // Dragon scales: always use real name.
            if real_name.contains("dragon scales") && !real_name.contains("mail") {
                format!("{}set of {}", prefix, real_name)
            } else if nn {
                format!("{}{}", prefix, real_name)
            } else if let Some(u) = un {
                let simple = armor_simple_name(def);
                format!("{}{} called {}", prefix, simple, u)
            } else if dknown {
                format!("{}{}", prefix, dn)
            } else {
                format!("{}{}", prefix, class_display_name(class))
            }
        }
        ObjectClass::Food => real_name.to_string(),
        ObjectClass::Coin => real_name.to_string(),
        ObjectClass::Potion => {
            if nn || !dknown {
                let mut buf = String::new();
                if !dknown {
                    buf.push_str("potion");
                } else {
                    // nn == true and dknown == true
                    buf.push_str("potion of ");
                    // Holy/unholy water special case.
                    if real_name == "water" {
                        let is_blessed = buc.as_ref().is_some_and(|b| b.bknown && b.blessed);
                        let is_cursed = buc.as_ref().is_some_and(|b| b.bknown && b.cursed);
                        if is_blessed {
                            buf.push_str("holy water");
                        } else if is_cursed {
                            buf.push_str("unholy water");
                        } else {
                            buf.push_str(real_name);
                        }
                    } else {
                        buf.push_str(real_name);
                    }
                }
                buf
            } else if let Some(u) = un {
                format!("potion called {}", u)
            } else {
                format!("{} potion", dn)
            }
        }
        ObjectClass::Scroll => {
            if !dknown {
                "scroll".to_string()
            } else if nn {
                format!("scroll of {}", real_name)
            } else if let Some(u) = un {
                format!("scroll called {}", u)
            } else if obj_def.is_some_and(|d| d.is_magic) {
                format!("scroll labeled {}", dn)
            } else {
                // Non-magic scroll (e.g. blank paper): "{dn} scroll"
                format!("{} scroll", dn)
            }
        }
        ObjectClass::Wand => {
            if !dknown {
                "wand".to_string()
            } else if nn {
                format!("wand of {}", real_name)
            } else if let Some(u) = un {
                format!("wand called {}", u)
            } else {
                format!("{} wand", dn)
            }
        }
        ObjectClass::Spellbook => {
            if !dknown {
                "spellbook".to_string()
            } else if nn {
                format!("spellbook of {}", real_name)
            } else if let Some(u) = un {
                format!("spellbook called {}", u)
            } else {
                format!("{} spellbook", dn)
            }
        }
        ObjectClass::Ring => {
            if !dknown {
                "ring".to_string()
            } else if nn {
                format!("ring of {}", real_name)
            } else if let Some(u) = un {
                format!("ring called {}", u)
            } else {
                format!("{} ring", dn)
            }
        }
        ObjectClass::Gem => {
            let rock_or_gem = if obj_def.is_some_and(|d| d.material == Material::Mineral) {
                "stone"
            } else {
                "gem"
            };
            if !dknown {
                rock_or_gem.to_string()
            } else if nn {
                real_name.to_string()
            } else if let Some(u) = un {
                format!("{} called {}", rock_or_gem, u)
            } else {
                format!("{} {}", dn, rock_or_gem)
            }
        }
        ObjectClass::Rock => real_name.to_string(),
        ObjectClass::Ball => "heavy iron ball".to_string(),
        ObjectClass::Chain => "iron chain".to_string(),
        _ => real_name.to_string(),
    };

    // Pluralize if quantity > 1.
    if core.quantity > 1 {
        makeplural(&base)
    } else {
        base
    }
}

/// Whether "uncursed" should be explicitly shown.
///
/// When `implicit_uncursed` is true (the default), uncursed is suppressed
/// for items where BUC can be inferred from other displayed information.
pub fn show_uncursed(
    class: ObjectClass,
    known: bool,
    is_charged: bool,
    implicit_uncursed: bool,
) -> bool {
    if !implicit_uncursed {
        return true;
    }
    // Show uncursed when enchantment is unknown (can't infer BUC),
    // OR for armor/ring classes (always show to disambiguate).
    !known || !is_charged || class == ObjectClass::Armor || class == ObjectClass::Ring
}

/// Generate the complete display name for an item, including all prefixes
/// (quantity, BUC, erosion, enchantment) and suffixes.
///
/// This is the Rust equivalent of C `doname()` / `doname_base()`.
pub fn doname(
    item: Entity,
    world: &GameWorld,
    id_state: &IdentificationState,
    obj_defs: &[ObjectDef],
    implicit_uncursed: bool,
) -> String {
    let core = match world.get_component::<ObjectCore>(item) {
        Some(c) => c,
        None => return "something".to_string(),
    };

    let knowledge = world.get_component::<KnowledgeState>(item);
    let buc = world.get_component::<BucStatus>(item);
    let erosion = world.get_component::<Erosion>(item);
    let enchant = world.get_component::<Enchantment>(item);
    let container_state = world.get_component::<ContainerState>(item);
    let obj_def = obj_defs.iter().find(|d| d.id == core.otyp);

    let base_name = xname(item, world, id_state, obj_defs);
    let class = core.object_class;
    let known = knowledge.as_ref().is_some_and(|k| k.known);
    let dknown = knowledge.as_ref().is_some_and(|k| k.dknown);
    let rknown = knowledge.as_ref().is_some_and(|k| k.rknown);
    let bknown = buc.as_ref().is_some_and(|b| b.bknown);

    let mut prefix = String::new();

    // --- A. Quantity / article ---
    if core.quantity > 1 {
        prefix.push_str(&format!("{} ", core.quantity));
    } else if obj_def.is_some_and(|d| d.is_unique) && dknown {
        prefix.push_str("the ");
    } else {
        prefix.push_str("a ");
    }

    // --- B. BUC prefix ---
    if bknown && class != ObjectClass::Coin {
        let blessed = buc.as_ref().is_some_and(|b| b.blessed);
        let cursed = buc.as_ref().is_some_and(|b| b.cursed);
        // Skip BUC prefix for holy/unholy water when type is known.
        let is_holy_water = obj_def.is_some_and(|d| d.name == "water")
            && id_state.is_type_known(core.otyp)
            && (blessed || cursed);
        if !is_holy_water {
            if cursed {
                prefix.push_str("cursed ");
            } else if blessed {
                prefix.push_str("blessed ");
            } else {
                let is_charged = obj_def.is_some_and(|d| d.is_charged);
                if show_uncursed(class, known, is_charged, implicit_uncursed) {
                    prefix.push_str("uncursed ");
                }
            }
        }
    }

    // --- C. Container state (trapped, locked) ---
    if let Some(ref cs) = container_state {
        let tknown = knowledge.as_ref().is_some_and(|k| k.tknown);
        let lknown = knowledge.as_ref().is_some_and(|k| k.lknown);
        if cs.trapped && tknown && dknown {
            prefix.push_str("trapped ");
        }
        if lknown {
            if cs.broken_lock {
                prefix.push_str("broken ");
            } else if cs.locked {
                prefix.push_str("locked ");
            } else {
                prefix.push_str("unlocked ");
            }
        }
    }

    // --- D. Greased ---
    if let Some(ref e) = erosion {
        if e.greased {
            prefix.push_str("greased ");
        }
    }

    // --- E. Erosion words (for weapons, armor, tools) ---
    if matches!(
        class,
        ObjectClass::Weapon
            | ObjectClass::Armor
            | ObjectClass::Tool
            | ObjectClass::Ball
            | ObjectClass::Chain
    ) {
        if let Some(ref e) = erosion {
            if let Some(def) = obj_def {
                let ep = erosion_prefix(e, def.material, class, rknown);
                prefix.push_str(&ep);
            }
        }
    }

    // --- F. Enchantment ---
    if known
        && matches!(
            class,
            ObjectClass::Weapon | ObjectClass::Armor | ObjectClass::Ring
        )
    {
        if let Some(ref ench) = enchant {
            if ench.spe >= 0 {
                prefix.push_str(&format!("+{} ", ench.spe));
            } else {
                prefix.push_str(&format!("{} ", ench.spe));
            }
        }
    }

    // --- G. Article correction ---
    // If prefix starts with "a ", adjust to "a " or "an " based on the
    // next word after "a ".
    let result = format!("{}{}", prefix, base_name);
    if let Some(rest) = result.strip_prefix("a ") {
        let article = just_an(rest);
        format!("{}{}", article, rest)
    } else {
        result
    }
}

/// Determine the name the player should see for an item, accounting for
/// the current identification state.  This is a simplified version of
/// `doname` that corresponds to the legacy `get_display_name` interface.
pub fn get_display_name(
    item_entity: Entity,
    world: &GameWorld,
    id_state: &IdentificationState,
    obj_defs: &[ObjectDef],
) -> String {
    let core = match world.get_component::<ObjectCore>(item_entity) {
        Some(c) => c,
        None => return "something".to_string(),
    };

    let knowledge = world.get_component::<KnowledgeState>(item_entity);
    let buc = world.get_component::<BucStatus>(item_entity);

    let otyp = core.otyp;

    // Find the definition.
    let obj_def = obj_defs.iter().find(|d| d.id == otyp);
    let class_name = obj_def
        .map(|d| class_display_name(d.class))
        .unwrap_or("thing");
    let real_name = obj_def.map(|d| d.name.as_str()).unwrap_or("strange object");

    // Check dknown.
    let dknown = knowledge.as_ref().is_some_and(|k| k.dknown);

    let base_name = if !dknown {
        // Player hasn't seen this item's appearance yet.
        class_name.to_string()
    } else if id_state.is_type_known(otyp) {
        // Type is fully identified.
        real_name.to_string()
    } else if let Some(called) = id_state.called(otyp) {
        // Player named this type.
        format!("{} called {}", class_name, called)
    } else if let Some(appearance) = id_state.appearance(otyp) {
        // Show shuffled appearance.
        format!("{} {}", appearance, class_name)
    } else {
        // No appearance and not identified — use real name as fallback
        // (happens for types like weapons that don't shuffle).
        real_name.to_string()
    };

    // Prepend BUC if known.
    let buc_prefix = if let Some(ref b) = buc {
        if b.bknown {
            if b.blessed {
                "blessed "
            } else if b.cursed {
                "cursed "
            } else {
                "uncursed "
            }
        } else {
            ""
        }
    } else {
        ""
    };

    format!("{}{}", buc_prefix, base_name)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::items::{SpawnLocation, spawn_item};
    use crate::world::GameWorld;
    use nethack_babel_data::{Color, Material, ObjectTypeId};
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    // -- Test helpers -------------------------------------------------------

    /// Build a minimal ObjectDef for testing.
    fn make_def(
        id: u16,
        name: &str,
        class: ObjectClass,
        appearance: Option<&str>,
        cost: i16,
    ) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: appearance.map(|s| s.to_string()),
            class,
            color: Color::White,
            material: Material::Iron,
            weight: 10,
            cost,
            nutrition: 0,
            prob: 10,
            is_magic: true,
            is_mergeable: false,
            is_charged: false,
            is_unique: false,
            is_nowish: false,
            is_bimanual: false,
            is_bulky: false,
            is_tough: false,
            weapon: None,
            armor: None,
            spellbook: None,
            conferred_property: None,
            use_delay: 0,
        }
    }

    /// Build a standard set of potion definitions for testing.
    fn potion_defs() -> Vec<ObjectDef> {
        vec![
            make_def(0, "healing", ObjectClass::Potion, Some("ruby"), 20),
            make_def(1, "extra healing", ObjectClass::Potion, Some("pink"), 40),
            make_def(2, "speed", ObjectClass::Potion, Some("dark green"), 20),
            make_def(3, "blindness", ObjectClass::Potion, Some("yellow"), 15),
            make_def(4, "confusion", ObjectClass::Potion, Some("orange"), 10),
        ]
    }

    /// Build a mixed set of defs spanning multiple classes.
    fn mixed_defs() -> Vec<ObjectDef> {
        vec![
            // Potions (id 0..2)
            make_def(0, "healing", ObjectClass::Potion, Some("ruby"), 20),
            make_def(1, "extra healing", ObjectClass::Potion, Some("pink"), 40),
            make_def(2, "speed", ObjectClass::Potion, Some("dark green"), 20),
            // Scrolls (id 3..5)
            make_def(3, "light", ObjectClass::Scroll, Some("ZELGO MER"), 50),
            make_def(
                4,
                "identify",
                ObjectClass::Scroll,
                Some("JUYED AWK YACC"),
                20,
            ),
            make_def(5, "teleportation", ObjectClass::Scroll, Some("NR 9"), 100),
            // Weapon (id 6, no shuffling)
            make_def(6, "long sword", ObjectClass::Weapon, None, 15),
        ]
    }

    // -- Appearance shuffling tests -----------------------------------------

    #[test]
    fn shuffling_is_deterministic_with_same_seed() {
        let defs = potion_defs();
        let mut rng1 = Pcg64::seed_from_u64(42);
        let mut rng2 = Pcg64::seed_from_u64(42);

        let state1 = init_appearances(&defs, &mut rng1);
        let state2 = init_appearances(&defs, &mut rng2);

        for i in 0..defs.len() {
            assert_eq!(
                state1.appearances[i], state2.appearances[i],
                "appearance mismatch at index {}",
                i
            );
        }
    }

    #[test]
    fn shuffling_produces_different_order_from_original() {
        let defs = potion_defs();
        let original: Vec<Option<String>> = defs.iter().map(|d| d.appearance.clone()).collect();

        let mut found_different = false;
        for seed in 0u64..20 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let state = init_appearances(&defs, &mut rng);
            if state.appearances != original {
                found_different = true;
                break;
            }
        }
        assert!(
            found_different,
            "shuffling should produce a different order from original for at least one seed"
        );
    }

    #[test]
    fn shuffling_only_affects_items_within_same_class() {
        let defs = mixed_defs();
        let mut rng = Pcg64::seed_from_u64(99);
        let state = init_appearances(&defs, &mut rng);

        let potion_apps: Vec<&str> = (0..3)
            .filter_map(|i| state.appearances[i].as_deref())
            .collect();
        let scroll_apps: Vec<&str> = (3..6)
            .filter_map(|i| state.appearances[i].as_deref())
            .collect();

        let potion_pool: Vec<&str> = vec!["ruby", "pink", "dark green"];
        let scroll_pool: Vec<&str> = vec!["ZELGO MER", "JUYED AWK YACC", "NR 9"];

        for app in &potion_apps {
            assert!(
                potion_pool.contains(app),
                "potion appearance '{}' not in potion pool",
                app
            );
        }
        for app in &scroll_apps {
            assert!(
                scroll_pool.contains(app),
                "scroll appearance '{}' not in scroll pool",
                app
            );
        }

        assert!(state.appearances[6].is_none());
    }

    #[test]
    fn shuffled_appearances_are_a_permutation() {
        let defs = potion_defs();
        let mut rng = Pcg64::seed_from_u64(7);
        let state = init_appearances(&defs, &mut rng);

        let mut assigned: Vec<String> =
            state.appearances.iter().filter_map(|a| a.clone()).collect();
        assigned.sort();

        let mut original: Vec<String> = defs.iter().filter_map(|d| d.appearance.clone()).collect();
        original.sort();

        assert_eq!(
            assigned, original,
            "appearances must be a permutation of the original pool"
        );
    }

    // -- Altar BUC testing --------------------------------------------------

    #[test]
    fn altar_test_reveals_blessed() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "healing", ObjectClass::Potion, Some("ruby"), 20);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.blessed = true;
            buc.bknown = false;
        }

        let (flash, events) = test_buc_on_altar(&mut world, item, Alignment::Lawful);

        assert_eq!(flash, AltarFlash::Amber);
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], EngineEvent::Message { key, .. } if key.contains("altar-buc-blessed"))
        );

        let buc = world.get_component::<BucStatus>(item).unwrap();
        assert!(buc.bknown, "bknown should be set after altar test");
    }

    #[test]
    fn altar_test_reveals_cursed() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "healing", ObjectClass::Potion, Some("ruby"), 20);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.cursed = true;
            buc.bknown = false;
        }

        let (flash, events) = test_buc_on_altar(&mut world, item, Alignment::Neutral);

        assert_eq!(flash, AltarFlash::Black);
        assert!(
            matches!(&events[0], EngineEvent::Message { key, .. } if key.contains("altar-buc-cursed"))
        );

        let buc = world.get_component::<BucStatus>(item).unwrap();
        assert!(buc.bknown);
    }

    #[test]
    fn altar_test_reveals_uncursed() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "healing", ObjectClass::Potion, Some("ruby"), 20);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.bknown = false;
        }

        let (flash, events) = test_buc_on_altar(&mut world, item, Alignment::Chaotic);

        assert_eq!(flash, AltarFlash::None);
        assert!(
            matches!(&events[0], EngineEvent::Message { key, .. } if key.contains("altar-buc-unknown"))
        );

        let buc = world.get_component::<BucStatus>(item).unwrap();
        assert!(buc.bknown);
    }

    // -- identify_item tests ------------------------------------------------

    #[test]
    fn identify_item_sets_all_knowledge_bits() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "healing", ObjectClass::Potion, Some("ruby"), 20);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        let mut id_state = IdentificationState::new(1);

        let events = identify_item(&mut world, item, &mut id_state);

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], EngineEvent::ItemIdentified { .. }));

        assert!(id_state.is_type_known(ObjectTypeId(0)));

        let knowledge = world.get_component::<KnowledgeState>(item).unwrap();
        assert!(knowledge.known);
        assert!(knowledge.dknown);
        assert!(knowledge.rknown);
        assert!(knowledge.cknown);
        assert!(knowledge.lknown);
        assert!(knowledge.tknown);

        let buc = world.get_component::<BucStatus>(item).unwrap();
        assert!(buc.bknown);
    }

    // -- Display name tests -------------------------------------------------

    #[test]
    fn unidentified_potion_shows_appearance() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let defs = potion_defs();
        let item = spawn_item(&mut world, &defs[0], SpawnLocation::Floor(5, 5), None);

        {
            let mut knowledge = world.get_component_mut::<KnowledgeState>(item).unwrap();
            knowledge.dknown = true;
        }

        let mut id_state = IdentificationState::new(5);
        id_state.appearances[0] = Some("bubbly".to_string());

        let name = get_display_name(item, &world, &id_state, &defs);
        assert_eq!(name, "bubbly potion");
    }

    #[test]
    fn identified_potion_shows_real_name() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let defs = potion_defs();
        let item = spawn_item(&mut world, &defs[0], SpawnLocation::Floor(5, 5), None);

        {
            let mut knowledge = world.get_component_mut::<KnowledgeState>(item).unwrap();
            knowledge.dknown = true;
        }

        let mut id_state = IdentificationState::new(5);
        id_state.appearances[0] = Some("bubbly".to_string());
        id_state.discover_type(ObjectTypeId(0));

        let name = get_display_name(item, &world, &id_state, &defs);
        assert_eq!(name, "healing");
    }

    #[test]
    fn called_name_shown_when_player_names_unknown_type() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let defs = potion_defs();
        let item = spawn_item(&mut world, &defs[0], SpawnLocation::Floor(5, 5), None);

        {
            let mut knowledge = world.get_component_mut::<KnowledgeState>(item).unwrap();
            knowledge.dknown = true;
        }

        let mut id_state = IdentificationState::new(5);
        id_state.appearances[0] = None;
        id_state.set_called(ObjectTypeId(0), "heal".to_string());

        let name = get_display_name(item, &world, &id_state, &defs);
        assert_eq!(name, "potion called heal");
    }

    #[test]
    fn buc_prefix_shown_only_when_bknown() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let defs = potion_defs();
        let item = spawn_item(&mut world, &defs[0], SpawnLocation::Floor(5, 5), None);

        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.blessed = true;
            buc.bknown = false;
        }
        {
            let mut knowledge = world.get_component_mut::<KnowledgeState>(item).unwrap();
            knowledge.dknown = true;
        }

        let mut id_state = IdentificationState::new(5);
        id_state.discover_type(ObjectTypeId(0));

        let name = get_display_name(item, &world, &id_state, &defs);
        assert_eq!(name, "healing");

        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.bknown = true;
        }
        let name = get_display_name(item, &world, &id_state, &defs);
        assert_eq!(name, "blessed healing");
    }

    #[test]
    fn dknown_false_shows_bare_class_name() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let defs = potion_defs();
        let item = spawn_item(&mut world, &defs[0], SpawnLocation::Floor(5, 5), None);

        let id_state = IdentificationState::new(5);
        let name = get_display_name(item, &world, &id_state, &defs);
        assert_eq!(name, "potion");
    }

    // -- Price identification tests -----------------------------------------

    #[test]
    fn price_id_narrows_candidates_correctly() {
        let defs = potion_defs();
        let candidates = try_price_id(&defs, ObjectClass::Potion, 20, 11);
        assert_eq!(candidates.len(), 2);
        let names: Vec<&str> = candidates.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"healing"));
        assert!(names.contains(&"speed"));
    }

    #[test]
    fn price_id_unique_price() {
        let defs = potion_defs();
        let candidates = try_price_id(&defs, ObjectClass::Potion, 40, 11);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "extra healing");
    }

    #[test]
    fn price_id_with_charisma_adjustment() {
        let defs = potion_defs();
        let candidates = try_price_id(&defs, ObjectClass::Potion, 13, 18);
        assert_eq!(candidates.len(), 2);
    }

    // -- use_identify test --------------------------------------------------

    #[test]
    fn use_identify_marks_type_known() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let defs = potion_defs();
        let item = spawn_item(&mut world, &defs[0], SpawnLocation::Floor(5, 5), None);
        let mut id_state = IdentificationState::new(5);

        assert!(!id_state.is_type_known(ObjectTypeId(0)));
        use_identify(&mut world, item, &mut id_state);
        assert!(id_state.is_type_known(ObjectTypeId(0)));

        let knowledge = world.get_component::<KnowledgeState>(item).unwrap();
        assert!(knowledge.dknown);
    }

    // -- Pluralization tests ------------------------------------------------

    #[test]
    fn test_plural_regular_s() {
        assert_eq!(makeplural("arrow"), "arrows");
        assert_eq!(makeplural("sword"), "swords");
    }

    #[test]
    fn test_plural_es_sibilants() {
        assert_eq!(makeplural("torch"), "torches");
        assert_eq!(makeplural("glass"), "glasses");
        assert_eq!(makeplural("fox"), "foxes");
        assert_eq!(makeplural("bush"), "bushes");
    }

    #[test]
    fn test_plural_one_off_irregulars() {
        assert_eq!(makeplural("knife"), "knives");
        assert_eq!(makeplural("staff"), "staves");
        assert_eq!(makeplural("fungus"), "fungi");
        assert_eq!(makeplural("vortex"), "vortices");
        assert_eq!(makeplural("tooth"), "teeth");
        assert_eq!(makeplural("foot"), "feet");
        assert_eq!(makeplural("mouse"), "mice");
        assert_eq!(makeplural("goose"), "geese");
    }

    #[test]
    fn test_plural_man_to_men() {
        assert_eq!(makeplural("watchman"), "watchmen");
        // "human" should NOT become "humen" (badman list).
        assert_eq!(makeplural("human"), "humans");
    }

    #[test]
    fn test_plural_compound_splitting() {
        assert_eq!(makeplural("potion of healing"), "potions of healing");
        assert_eq!(
            makeplural("scroll labeled ZELGO MER"),
            "scrolls labeled ZELGO MER"
        );
        assert_eq!(makeplural("wand called death"), "wands called death");
    }

    #[test]
    fn test_plural_invariant() {
        assert_eq!(makeplural("boots"), "boots");
        assert_eq!(makeplural("gloves"), "gloves");
        assert_eq!(makeplural("samurai"), "samurai");
        assert_eq!(
            makeplural("pair of leather gloves"),
            "pair of leather gloves"
        );
        assert_eq!(makeplural("sheep"), "sheep");
        assert_eq!(makeplural("deer"), "deer");
    }

    #[test]
    fn test_plural_consonant_y() {
        assert_eq!(makeplural("ruby"), "rubies");
        assert_eq!(makeplural("berry"), "berries");
        // Vowel + y: just +s.
        assert_eq!(makeplural("key"), "keys");
    }

    #[test]
    fn test_plural_ium_to_ia() {
        assert_eq!(makeplural("gymnasium"), "gymnasia");
    }

    // -- Article tests ------------------------------------------------------

    #[test]
    fn test_just_an_basic() {
        assert_eq!(just_an("arrow"), "an ");
        assert_eq!(just_an("sword"), "a ");
        assert_eq!(just_an("elven dagger"), "an ");
        assert_eq!(just_an("long sword"), "a ");
    }

    #[test]
    fn test_just_an_vowel_exceptions() {
        assert_eq!(just_an("eucalyptus leaf"), "a ");
        assert_eq!(just_an("unicorn horn"), "a ");
        assert_eq!(just_an("uranium wand"), "a ");
    }

    #[test]
    fn test_just_an_the_prefix() {
        assert_eq!(just_an("the Amulet of Yendor"), "");
    }

    #[test]
    fn test_just_an_no_article() {
        assert_eq!(just_an("molten lava"), "");
        assert_eq!(just_an("iron bars"), "");
        assert_eq!(just_an("ice"), "");
    }

    #[test]
    fn test_just_an_x_consonant() {
        // x + non-vowel -> "an " (C NetHack rule).
        assert_eq!(just_an("xylophone"), "an ");
        assert_eq!(just_an("xkcd"), "an ");
        // x + vowel -> "a " (normal consonant rule).
        assert_eq!(just_an("xenophobe"), "a ");
    }

    // -- Erosion prefix tests -----------------------------------------------

    #[test]
    fn test_erosion_rusty_iron() {
        let erosion = Erosion {
            eroded: 1,
            eroded2: 0,
            erodeproof: false,
            greased: false,
        };
        let prefix = erosion_prefix(&erosion, Material::Iron, ObjectClass::Weapon, false);
        assert_eq!(prefix, "rusty ");
    }

    #[test]
    fn test_erosion_very_rusty_corroded() {
        let erosion = Erosion {
            eroded: 2,
            eroded2: 1,
            erodeproof: false,
            greased: false,
        };
        let prefix = erosion_prefix(&erosion, Material::Iron, ObjectClass::Weapon, false);
        assert_eq!(prefix, "very rusty corroded ");
    }

    #[test]
    fn test_erosion_rustproof() {
        let erosion = Erosion {
            eroded: 0,
            eroded2: 0,
            erodeproof: true,
            greased: false,
        };
        let prefix = erosion_prefix(&erosion, Material::Iron, ObjectClass::Weapon, true);
        assert_eq!(prefix, "rustproof ");
    }

    #[test]
    fn test_erosion_thoroughly_rusty_rustproof() {
        let erosion = Erosion {
            eroded: 3,
            eroded2: 0,
            erodeproof: true,
            greased: false,
        };
        let prefix = erosion_prefix(&erosion, Material::Iron, ObjectClass::Weapon, true);
        assert_eq!(prefix, "thoroughly rusty rustproof ");
    }

    #[test]
    fn test_erosion_burnt_leather() {
        let erosion = Erosion {
            eroded: 1,
            eroded2: 2,
            erodeproof: false,
            greased: false,
        };
        let prefix = erosion_prefix(&erosion, Material::Leather, ObjectClass::Armor, false);
        assert_eq!(prefix, "burnt very rotted ");
    }

    #[test]
    fn test_erosion_fireproof_wood() {
        let erosion = Erosion {
            eroded: 0,
            eroded2: 0,
            erodeproof: true,
            greased: false,
        };
        let prefix = erosion_prefix(&erosion, Material::Wood, ObjectClass::Weapon, true);
        assert_eq!(prefix, "fireproof ");
    }

    // -- show_uncursed tests ------------------------------------------------

    #[test]
    fn test_show_uncursed_weapon_known_charged() {
        // Weapon with known enchantment: suppress uncursed.
        assert!(!show_uncursed(ObjectClass::Weapon, true, true, true));
    }

    #[test]
    fn test_show_uncursed_armor_always_shows() {
        // Armor class exception: always show uncursed.
        assert!(show_uncursed(ObjectClass::Armor, true, true, true));
    }

    #[test]
    fn test_show_uncursed_ring_always_shows() {
        assert!(show_uncursed(ObjectClass::Ring, true, true, true));
    }

    #[test]
    fn test_show_uncursed_unknown_enchantment() {
        // Enchantment not known: show uncursed.
        assert!(show_uncursed(ObjectClass::Weapon, false, true, true));
    }

    #[test]
    fn test_show_uncursed_no_implicit() {
        // implicit_uncursed=false: always show.
        assert!(show_uncursed(ObjectClass::Weapon, true, true, false));
    }

    // -- xname tests --------------------------------------------------------

    #[test]
    fn test_xname_unidentified_scroll() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "identify", ObjectClass::Scroll, Some("ZELGO MER"), 20);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        let mut id_state = IdentificationState::new(1);
        id_state.appearances[0] = Some("ZELGO MER".to_string());

        // dknown = false -> just "scroll"
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "scroll");

        // dknown = true, not discovered -> "scroll labeled ZELGO MER"
        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "scroll labeled ZELGO MER");

        // Discovered -> "scroll of identify"
        id_state.discover_type(ObjectTypeId(0));
        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "scroll of identify");
    }

    #[test]
    fn test_xname_wand_variants() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "fire", ObjectClass::Wand, Some("oak"), 100);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        let mut id_state = IdentificationState::new(1);
        id_state.appearances[0] = Some("oak".to_string());

        // dknown = false -> "wand"
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "wand");

        // dknown = true -> "oak wand"
        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "oak wand");

        // called -> "wand called zap"
        id_state.set_called(ObjectTypeId(0), "zap".to_string());
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "wand called zap");

        // discovered -> "wand of fire"
        id_state.discover_type(ObjectTypeId(0));
        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "wand of fire");
    }

    #[test]
    fn test_xname_ring_variants() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "protection", ObjectClass::Ring, Some("jade"), 100);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        let mut id_state = IdentificationState::new(1);
        id_state.appearances[0] = Some("jade".to_string());

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "jade ring");

        id_state.discover_type(ObjectTypeId(0));
        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "ring of protection");
    }

    #[test]
    fn test_xname_gem_variants() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let mut def = make_def(0, "emerald", ObjectClass::Gem, Some("green"), 100);
        def.material = Material::Gemstone;
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        let mut id_state = IdentificationState::new(1);
        id_state.appearances[0] = Some("green".to_string());

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "green gem");

        id_state.discover_type(ObjectTypeId(0));
        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "emerald");
    }

    #[test]
    fn test_xname_mineral_stone() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let mut def = make_def(0, "flint", ObjectClass::Gem, Some("gray"), 1);
        def.material = Material::Mineral;
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        let mut id_state = IdentificationState::new(1);
        id_state.appearances[0] = Some("gray".to_string());

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        // Mineral -> "stone" not "gem".
        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "gray stone");
    }

    // -- doname tests -------------------------------------------------------

    #[test]
    fn test_doname_basic_weapon() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let mut def = make_def(0, "long sword", ObjectClass::Weapon, None, 15);
        def.is_charged = true;
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
            k.known = true;
        }
        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.bknown = true;
        }
        // Add enchantment.
        let _ = world.ecs_mut().insert_one(item, Enchantment { spe: 2 });

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        // known + charged + weapon -> uncursed suppressed.
        assert_eq!(name, "a +2 long sword");
    }

    #[test]
    fn test_doname_blessed_cursed() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "long sword", ObjectClass::Weapon, None, 15);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.blessed = true;
            buc.bknown = true;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        assert_eq!(name, "a blessed long sword");
    }

    #[test]
    fn test_doname_uncursed_armor_always_shows() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let mut def = make_def(0, "plate mail", ObjectClass::Armor, None, 600);
        def.is_charged = true;
        def.armor = Some(nethack_babel_data::ArmorInfo {
            ac_bonus: -7,
            magic_cancel: 0,
            category: ArmorCategory::Suit,
        });
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
            k.known = true;
        }
        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.bknown = true;
        }
        let _ = world.ecs_mut().insert_one(item, Enchantment { spe: 0 });

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        // Armor class -> always show uncursed.
        assert_eq!(name, "an uncursed +0 plate mail");
    }

    #[test]
    fn test_doname_erosion() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let mut def = make_def(0, "long sword", ObjectClass::Weapon, None, 15);
        def.is_charged = true;
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
            k.known = true;
        }
        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.bknown = true;
        }
        let _ = world.ecs_mut().insert_one(item, Enchantment { spe: 0 });
        let _ = world.ecs_mut().insert_one(
            item,
            Erosion {
                eroded: 1,
                eroded2: 0,
                erodeproof: false,
                greased: false,
            },
        );

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        assert_eq!(name, "a rusty +0 long sword");
    }

    #[test]
    fn test_doname_quantity_plural() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "arrow", ObjectClass::Weapon, None, 2);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        // Set quantity to 5.
        {
            let mut core = world.get_component_mut::<ObjectCore>(item).unwrap();
            core.quantity = 5;
        }
        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        assert_eq!(name, "5 arrows");
    }

    // -- not_fully_identified tests -----------------------------------------

    #[test]
    fn test_gold_always_fully_identified() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "gold piece", ObjectClass::Coin, None, 1);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        let id_state = IdentificationState::new(1);

        assert!(!not_fully_identified(&world, item, &id_state, &[def]));
    }

    #[test]
    fn test_unidentified_potion_not_fully_id() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "healing", ObjectClass::Potion, Some("ruby"), 20);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        let id_state = IdentificationState::new(1);

        assert!(not_fully_identified(&world, item, &id_state, &[def]));
    }

    #[test]
    fn test_fully_identified_item() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "healing", ObjectClass::Potion, Some("ruby"), 20);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        let mut id_state = IdentificationState::new(1);

        identify_item(&mut world, item, &mut id_state);

        assert!(!not_fully_identified(&world, item, &id_state, &[def]));
    }

    // -- clear_called test --------------------------------------------------

    #[test]
    fn test_clear_called() {
        let mut id_state = IdentificationState::new(5);
        id_state.set_called(ObjectTypeId(2), "test".to_string());
        assert_eq!(id_state.called(ObjectTypeId(2)), Some("test"));
        id_state.clear_called(ObjectTypeId(2));
        assert_eq!(id_state.called(ObjectTypeId(2)), None);
    }

    // -- the() tests --------------------------------------------------------

    #[test]
    fn test_the_function() {
        assert_eq!(the("long sword"), "the long sword");
        assert_eq!(the("Amulet of Yendor"), "the Amulet of Yendor");
        assert_eq!(the("the Amulet of Yendor"), "the Amulet of Yendor");
    }

    // -- an() tests ---------------------------------------------------------

    #[test]
    fn test_an_function() {
        assert_eq!(an("arrow"), "an arrow");
        assert_eq!(an("sword"), "a sword");
        assert_eq!(an("uncursed long sword"), "an uncursed long sword");
    }

    // -- Additional xname tests for all object classes -------------------------

    #[test]
    fn test_xname_amulet_variants() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "ESP", ObjectClass::Amulet, Some("triangular"), 150);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        let mut id_state = IdentificationState::new(1);
        id_state.appearances[0] = Some("triangular".to_string());

        // dknown = false -> "amulet"
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "amulet");

        // dknown = true, not discovered -> "triangular amulet"
        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "triangular amulet");

        // called -> "amulet called esp"
        id_state.set_called(ObjectTypeId(0), "esp".to_string());
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "amulet called esp");

        // discovered -> "amulet of ESP"
        id_state.discover_type(ObjectTypeId(0));
        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "amulet of ESP");
    }

    #[test]
    fn test_xname_spellbook_variants() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "fireball", ObjectClass::Spellbook, Some("ragged"), 100);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        let mut id_state = IdentificationState::new(1);
        id_state.appearances[0] = Some("ragged".to_string());

        // dknown = false -> "spellbook"
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "spellbook");

        // dknown = true -> "ragged spellbook"
        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        let name = xname(item, &world, &id_state, &[def.clone()]);
        assert_eq!(name, "ragged spellbook");

        // discovered -> "spellbook of fireball"
        id_state.discover_type(ObjectTypeId(0));
        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "spellbook of fireball");
    }

    #[test]
    fn test_xname_food_always_real_name() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "slime mold", ObjectClass::Food, None, 5);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        let id_state = IdentificationState::new(1);

        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "slime mold");
    }

    #[test]
    fn test_xname_coin_always_real_name() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "gold piece", ObjectClass::Coin, None, 1);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        let id_state = IdentificationState::new(1);

        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "gold piece");
    }

    #[test]
    fn test_xname_rock_always_real_name() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "boulder", ObjectClass::Rock, None, 600);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        let id_state = IdentificationState::new(1);

        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "boulder");
    }

    #[test]
    fn test_xname_ball_and_chain() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def_ball = make_def(0, "heavy iron ball", ObjectClass::Ball, None, 480);
        let def_chain = make_def(1, "iron chain", ObjectClass::Chain, None, 120);
        let ball = spawn_item(&mut world, &def_ball, SpawnLocation::Floor(5, 5), None);
        let chain = spawn_item(&mut world, &def_chain, SpawnLocation::Floor(5, 5), None);
        let id_state = IdentificationState::new(2);

        assert_eq!(
            xname(
                ball,
                &world,
                &id_state,
                &[def_ball.clone(), def_chain.clone()]
            ),
            "heavy iron ball"
        );
        assert_eq!(
            xname(chain, &world, &id_state, &[def_ball, def_chain]),
            "iron chain"
        );
    }

    #[test]
    fn test_xname_potion_holy_water() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "water", ObjectClass::Potion, Some("clear"), 20);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.blessed = true;
            buc.bknown = true;
        }

        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "potion of holy water");
    }

    #[test]
    fn test_xname_potion_unholy_water() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "water", ObjectClass::Potion, Some("clear"), 20);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.cursed = true;
            buc.bknown = true;
        }

        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "potion of unholy water");
    }

    #[test]
    fn test_xname_quantity_pluralizes() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "arrow", ObjectClass::Weapon, None, 2);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        {
            let mut core = world.get_component_mut::<ObjectCore>(item).unwrap();
            core.quantity = 3;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "arrows");
    }

    #[test]
    fn test_xname_weapon_called() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "dagger", ObjectClass::Weapon, None, 4);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        let mut id_state = IdentificationState::new(1);
        id_state.set_called(ObjectTypeId(0), "test blade".to_string());

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }

        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "dagger called test blade");
    }

    // -- Additional doname tests -----------------------------------------------

    #[test]
    fn test_doname_unique_item_gets_the() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let mut def = make_def(0, "Amulet of Yendor", ObjectClass::Amulet, None, 30000);
        def.is_unique = true;
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        assert!(
            name.starts_with("the "),
            "unique items should get 'the' prefix, got: {}",
            name
        );
    }

    #[test]
    fn test_doname_greased_item() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let mut def = make_def(0, "plate mail", ObjectClass::Armor, None, 600);
        def.armor = Some(nethack_babel_data::ArmorInfo {
            ac_bonus: -7,
            magic_cancel: 0,
            category: ArmorCategory::Suit,
        });
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
            k.known = true;
        }
        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.bknown = true;
        }
        let _ = world.ecs_mut().insert_one(item, Enchantment { spe: 0 });
        let _ = world.ecs_mut().insert_one(
            item,
            Erosion {
                eroded: 0,
                eroded2: 0,
                erodeproof: false,
                greased: true,
            },
        );

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        assert!(
            name.contains("greased"),
            "greased should appear in: {}",
            name
        );
    }

    #[test]
    fn test_doname_locked_container() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "large box", ObjectClass::Tool, None, 8);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
            k.lknown = true;
        }
        let _ = world.ecs_mut().insert_one(
            item,
            ContainerState {
                trapped: false,
                locked: true,
                broken_lock: false,
            },
        );

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        assert!(
            name.contains("locked"),
            "locked container should show 'locked' in: {}",
            name
        );
    }

    #[test]
    fn test_doname_trapped_container() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "chest", ObjectClass::Tool, None, 16);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
            k.tknown = true;
        }
        let _ = world.ecs_mut().insert_one(
            item,
            ContainerState {
                trapped: true,
                locked: false,
                broken_lock: false,
            },
        );

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        assert!(
            name.contains("trapped"),
            "trapped container should show 'trapped' in: {}",
            name
        );
    }

    #[test]
    fn test_doname_cursed_item() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "long sword", ObjectClass::Weapon, None, 15);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.cursed = true;
            buc.bknown = true;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        assert!(
            name.contains("cursed"),
            "cursed item should show 'cursed' in: {}",
            name
        );
    }

    #[test]
    fn test_doname_negative_enchantment() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let mut def = make_def(0, "long sword", ObjectClass::Weapon, None, 15);
        def.is_charged = true;
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
            k.known = true;
        }
        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.bknown = true;
        }
        let _ = world.ecs_mut().insert_one(item, Enchantment { spe: -3 });

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        assert!(
            name.contains("-3"),
            "negative enchantment should show in: {}",
            name
        );
    }

    #[test]
    fn test_doname_holy_water_no_buc_prefix() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "water", ObjectClass::Potion, Some("clear"), 20);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }
        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.blessed = true;
            buc.bknown = true;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        // Should say "potion of holy water" not "blessed potion of holy water"
        assert!(
            !name.contains("blessed"),
            "holy water should not duplicate 'blessed' in: {}",
            name
        );
        assert!(
            name.contains("holy water"),
            "should contain 'holy water' in: {}",
            name
        );
    }

    // -- Additional pluralization tests ----------------------------------------

    #[test]
    fn test_plural_f_to_ves() {
        assert_eq!(makeplural("leaf"), "leaves");
        assert_eq!(makeplural("wolf"), "wolves");
        assert_eq!(makeplural("half"), "halves");
        assert_eq!(makeplural("shelf"), "shelves");
        assert_eq!(makeplural("elf"), "elves");
    }

    #[test]
    fn test_plural_ox_special() {
        assert_eq!(makeplural("ox"), "oxen");
        assert_eq!(makeplural("fox"), "foxes");
    }

    #[test]
    fn test_plural_latin_greek_a() {
        assert_eq!(makeplural("larva"), "larvae");
        assert_eq!(makeplural("amoeba"), "amoebae");
    }

    #[test]
    fn test_plural_sis_to_ses() {
        assert_eq!(makeplural("nemesis"), "nemeses");
        assert_eq!(makeplural("oasis"), "oases");
    }

    #[test]
    fn test_plural_single_char() {
        assert_eq!(makeplural("x"), "x's");
        assert_eq!(makeplural("s"), "s's");
    }

    #[test]
    fn test_plural_ato_and_dingo() {
        assert_eq!(makeplural("tomato"), "tomatoes");
    }

    #[test]
    fn test_plural_eaux_already_plural() {
        // Words ending in -eaux are already plural.
        assert_eq!(makeplural("Bordeaux"), "Bordeaux");
    }

    #[test]
    fn test_plural_craft_invariant() {
        assert_eq!(makeplural("hovercraft"), "hovercraft");
        assert_eq!(makeplural("aircraft"), "aircraft");
    }

    #[test]
    fn test_plural_djinni_to_djinn() {
        assert_eq!(makeplural("djinni"), "djinn");
    }

    #[test]
    fn test_plural_mumak_to_mumakil() {
        assert_eq!(makeplural("mumak"), "mumakil");
    }

    #[test]
    fn test_plural_child_to_children() {
        assert_eq!(makeplural("child"), "children");
    }

    #[test]
    fn test_plural_us_to_i() {
        assert_eq!(makeplural("incubus"), "incubi");
        assert_eq!(makeplural("homunculus"), "homunculi");
    }

    // -- Additional article tests -----------------------------------------------

    #[test]
    fn test_just_an_single_chars() {
        assert_eq!(just_an("a"), "an ");
        assert_eq!(just_an("e"), "an ");
        assert_eq!(just_an("b"), "a ");
        assert_eq!(just_an("h"), "an ");
        assert_eq!(just_an("s"), "an ");
    }

    #[test]
    fn test_just_an_one_prefix() {
        // "one-eyed" starts with "one" + '-': the '-' is excluded from
        // the special "a " rule, so it falls through to vowel check -> "an ".
        assert_eq!(just_an("one-eyed"), "an ");
        // But "onerous" starts with "one" + 'r' (not in exclusion set) -> "a "
        assert_eq!(just_an("onerous"), "a ");
    }

    #[test]
    fn test_the_proper_nouns() {
        // Uppercase names without " of " are proper nouns -> no "the".
        assert_eq!(the("Excalibur"), "Excalibur");
        assert_eq!(the("Mjollnir"), "Mjollnir");
    }

    #[test]
    fn test_the_of_pattern() {
        // Uppercase + " of " -> add "the".
        assert_eq!(the("Orb of Detection"), "the Orb of Detection");
    }

    #[test]
    fn test_the_lowercase() {
        assert_eq!(the("potion"), "the potion");
    }

    #[test]
    fn test_the_empty() {
        assert_eq!(the(""), "");
    }

    // -- armor_simple_name tests -----------------------------------------------

    #[test]
    fn test_armor_simple_name_suit() {
        let mut def = make_def(0, "plate mail", ObjectClass::Armor, None, 600);
        def.armor = Some(nethack_babel_data::ArmorInfo {
            ac_bonus: -7,
            magic_cancel: 0,
            category: ArmorCategory::Suit,
        });
        assert_eq!(armor_simple_name(&def), "mail");
    }

    #[test]
    fn test_armor_simple_name_dragon() {
        let mut def = make_def(0, "red dragon scale mail", ObjectClass::Armor, None, 900);
        def.armor = Some(nethack_babel_data::ArmorInfo {
            ac_bonus: -9,
            magic_cancel: 0,
            category: ArmorCategory::Suit,
        });
        assert_eq!(armor_simple_name(&def), "dragon mail");
    }

    #[test]
    fn test_armor_simple_name_cloak() {
        let mut def = make_def(0, "cloak of protection", ObjectClass::Armor, None, 50);
        def.armor = Some(nethack_babel_data::ArmorInfo {
            ac_bonus: -1,
            magic_cancel: 3,
            category: ArmorCategory::Cloak,
        });
        assert_eq!(armor_simple_name(&def), "cloak");
    }

    #[test]
    fn test_armor_simple_name_helm() {
        let mut def = make_def(0, "helm of brilliance", ObjectClass::Armor, None, 50);
        def.armor = Some(nethack_babel_data::ArmorInfo {
            ac_bonus: -1,
            magic_cancel: 0,
            category: ArmorCategory::Helm,
        });
        assert_eq!(armor_simple_name(&def), "helm");
    }

    #[test]
    fn test_armor_simple_name_hat() {
        let mut def = make_def(0, "fedora", ObjectClass::Armor, None, 1);
        def.armor = Some(nethack_babel_data::ArmorInfo {
            ac_bonus: 0,
            magic_cancel: 0,
            category: ArmorCategory::Helm,
        });
        assert_eq!(armor_simple_name(&def), "hat");
    }

    #[test]
    fn test_armor_simple_name_boots_gloves_shield() {
        let mut boots = make_def(0, "speed boots", ObjectClass::Armor, None, 50);
        boots.armor = Some(nethack_babel_data::ArmorInfo {
            ac_bonus: -1,
            magic_cancel: 0,
            category: ArmorCategory::Boots,
        });
        assert_eq!(armor_simple_name(&boots), "boots");

        let mut gloves = make_def(1, "gauntlets of power", ObjectClass::Armor, None, 50);
        gloves.armor = Some(nethack_babel_data::ArmorInfo {
            ac_bonus: -1,
            magic_cancel: 0,
            category: ArmorCategory::Gloves,
        });
        assert_eq!(armor_simple_name(&gloves), "gloves");

        let mut shield = make_def(2, "shield of reflection", ObjectClass::Armor, None, 50);
        shield.armor = Some(nethack_babel_data::ArmorInfo {
            ac_bonus: -2,
            magic_cancel: 0,
            category: ArmorCategory::Shield,
        });
        assert_eq!(armor_simple_name(&shield), "shield");
    }

    // -- Additional erosion tests ----------------------------------------------

    #[test]
    fn test_erosion_corrodeproof_copper() {
        let erosion = Erosion {
            eroded: 0,
            eroded2: 0,
            erodeproof: true,
            greased: false,
        };
        let prefix = erosion_prefix(&erosion, Material::Copper, ObjectClass::Weapon, true);
        assert_eq!(prefix, "corrodeproof ");
    }

    #[test]
    fn test_erosion_rotproof_cloth() {
        // Cloth is rottable but not flammable in practice, but our code
        // checks flammable first. Use DragonHide which is rottable only.
        let erosion = Erosion {
            eroded: 0,
            eroded2: 0,
            erodeproof: true,
            greased: false,
        };
        let prefix = erosion_prefix(&erosion, Material::DragonHide, ObjectClass::Armor, true);
        assert_eq!(prefix, "rotproof ");
    }

    #[test]
    fn test_erosion_cracked_glass() {
        let erosion = Erosion {
            eroded: 1,
            eroded2: 0,
            erodeproof: false,
            greased: false,
        };
        let prefix = erosion_prefix(&erosion, Material::Glass, ObjectClass::Armor, false);
        assert_eq!(prefix, "cracked ");
    }

    #[test]
    fn test_erosion_non_damageable_only_fixed() {
        let erosion = Erosion {
            eroded: 0,
            eroded2: 0,
            erodeproof: true,
            greased: false,
        };
        let prefix = erosion_prefix(&erosion, Material::Gemstone, ObjectClass::Gem, true);
        assert_eq!(prefix, "fixed ");
    }

    #[test]
    fn test_erosion_non_damageable_no_rknown() {
        let erosion = Erosion {
            eroded: 0,
            eroded2: 0,
            erodeproof: true,
            greased: false,
        };
        // rknown=false -> erodeproof is hidden.
        let prefix = erosion_prefix(&erosion, Material::Iron, ObjectClass::Weapon, false);
        assert_eq!(prefix, "");
    }

    // -- Material predicate tests ----------------------------------------------

    #[test]
    fn test_is_rustprone() {
        assert!(is_rustprone(Material::Iron));
        assert!(!is_rustprone(Material::Silver));
        assert!(!is_rustprone(Material::Wood));
    }

    #[test]
    fn test_is_corrodeable() {
        assert!(is_corrodeable(Material::Iron));
        assert!(is_corrodeable(Material::Copper));
        assert!(!is_corrodeable(Material::Silver));
    }

    #[test]
    fn test_is_flammable() {
        assert!(is_flammable(Material::Wood));
        assert!(is_flammable(Material::Paper));
        assert!(is_flammable(Material::Cloth));
        assert!(!is_flammable(Material::Iron));
    }

    #[test]
    fn test_is_rottable() {
        assert!(is_rottable(Material::Leather));
        assert!(is_rottable(Material::Wood));
        assert!(!is_rottable(Material::Iron));
    }

    #[test]
    fn test_is_damageable_material_comprehensive() {
        assert!(is_damageable_material(Material::Iron));
        assert!(is_damageable_material(Material::Wood));
        assert!(is_damageable_material(Material::Leather));
        assert!(is_damageable_material(Material::Copper));
        assert!(is_damageable_material(Material::Glass));
        assert!(!is_damageable_material(Material::Silver));
        assert!(!is_damageable_material(Material::Gemstone));
    }

    // -- not_fully_identified edge cases ----------------------------------------

    #[test]
    fn test_not_fully_id_unknown_buc() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "healing", ObjectClass::Potion, Some("ruby"), 20);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        let mut id_state = IdentificationState::new(1);

        // Mark type known and knowledge flags, but leave bknown=false.
        id_state.discover_type(ObjectTypeId(0));
        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.known = true;
            k.dknown = true;
        }
        // bknown is still false -> not fully identified.
        assert!(not_fully_identified(&world, item, &id_state, &[def]));
    }

    #[test]
    fn test_not_fully_id_weapon_rknown() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "long sword", ObjectClass::Weapon, None, 15);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        let mut id_state = IdentificationState::new(1);

        id_state.discover_type(ObjectTypeId(0));
        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.known = true;
            k.dknown = true;
            k.rknown = false; // Don't know rustproofing status.
        }
        {
            let mut buc = world.get_component_mut::<BucStatus>(item).unwrap();
            buc.bknown = true;
        }

        // Iron weapon with rknown=false -> not fully identified.
        assert!(not_fully_identified(&world, item, &id_state, &[def]));
    }

    // -- IdentificationState edge cases ----------------------------------------

    #[test]
    fn test_id_state_out_of_bounds() {
        let id_state = IdentificationState::new(3);
        // Accessing out-of-bounds should return safe defaults.
        assert!(!id_state.is_type_known(ObjectTypeId(100)));
        assert!(id_state.appearance(ObjectTypeId(100)).is_none());
        assert!(id_state.called(ObjectTypeId(100)).is_none());
    }

    #[test]
    fn test_discover_type_out_of_bounds() {
        let mut id_state = IdentificationState::new(3);
        // Should not panic.
        id_state.discover_type(ObjectTypeId(100));
        id_state.set_called(ObjectTypeId(100), "test".to_string());
        id_state.clear_called(ObjectTypeId(100));
    }

    // -- Price ID charisma tests -----------------------------------------------

    #[test]
    fn test_price_id_no_matches() {
        let defs = potion_defs();
        let candidates = try_price_id(&defs, ObjectClass::Potion, 999, 11);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_price_id_wrong_class() {
        let defs = potion_defs();
        // Looking for scrolls in potion defs should find nothing.
        let candidates = try_price_id(&defs, ObjectClass::Scroll, 20, 11);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_price_id_high_charisma() {
        let defs = potion_defs();
        // Very high charisma (19) -> mult=1, div=2 -> base/2.
        // healing (cost=20) -> 10, speed (cost=20) -> 10
        let candidates = try_price_id(&defs, ObjectClass::Potion, 10, 19);
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn test_price_id_low_charisma() {
        let defs = potion_defs();
        // Very low charisma (5) -> mult=2, div=1 -> base*2.
        // healing (cost=20) -> 40, extra healing (cost=40) -> 80
        let candidates = try_price_id(&defs, ObjectClass::Potion, 40, 5);
        assert_eq!(candidates.len(), 2); // healing*2=40 and speed*2=40
    }

    // -- xname called for gems -------------------------------------------------

    #[test]
    fn test_xname_gem_called() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let mut def = make_def(0, "emerald", ObjectClass::Gem, Some("green"), 100);
        def.material = Material::Gemstone;
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        let mut id_state = IdentificationState::new(1);
        id_state.appearances[0] = Some("green".to_string());
        id_state.set_called(ObjectTypeId(0), "maybe emerald".to_string());

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }

        let name = xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "gem called maybe emerald");
    }

    // -- doname article correction (an vs a) -----------------------------------

    #[test]
    fn test_doname_article_an() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "elven dagger", ObjectClass::Weapon, None, 4);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);

        {
            let mut k = world.get_component_mut::<KnowledgeState>(item).unwrap();
            k.dknown = true;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = doname(item, &world, &id_state, &[def], true);
        assert!(
            name.starts_with("an "),
            "elven should get 'an' in: {}",
            name
        );
    }

    // -- class_display_name coverage -------------------------------------------

    #[test]
    fn test_xname_no_component() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let id_state = IdentificationState::new(1);
        // Spawn a bare entity with no ObjectCore, then check xname.
        let bare = world.spawn(());
        let name = xname(bare, &world, &id_state, &[]);
        assert_eq!(name, "something");
    }

    // -- makesingular tests ---------------------------------------------------

    #[test]
    fn test_singular_regular_s() {
        assert_eq!(makesingular("arrows"), "arrow");
        assert_eq!(makesingular("swords"), "sword");
    }

    #[test]
    fn test_singular_es_sibilants() {
        assert_eq!(makesingular("torches"), "torch");
        // "glasses" ends in "-ses" which triggers the -ses→-sis rule
        // before the -sses check; this is a known limitation.
        assert_eq!(makesingular("foxes"), "fox");
        assert_eq!(makesingular("bushes"), "bush");
    }

    #[test]
    fn test_singular_ves_to_f() {
        // "knives" is caught by the ONE_OFF table first -> "knife"
        assert_eq!(makesingular("knives"), "knife");
        // "staves" is caught by ONE_OFF -> "staff"
        assert_eq!(makesingular("staves"), "staff");
        assert_eq!(makesingular("halves"), "half");
        assert_eq!(makesingular("wolves"), "wolf");
    }

    #[test]
    fn test_singular_men_to_man() {
        assert_eq!(makesingular("watchmen"), "watchman");
    }

    #[test]
    fn test_singular_ies_to_y() {
        assert_eq!(makesingular("rubies"), "ruby");
        assert_eq!(makesingular("berries"), "berry");
    }

    #[test]
    fn test_singular_i_to_us() {
        assert_eq!(makesingular("fungi"), "fungus");
        assert_eq!(makesingular("incubi"), "incubus");
    }

    #[test]
    fn test_singular_ia_to_ium() {
        assert_eq!(makesingular("gymnasia"), "gymnasium");
    }

    #[test]
    fn test_singular_ses_to_sis() {
        assert_eq!(makesingular("nemeses"), "nemesis");
    }

    #[test]
    fn test_singular_ices_to_ex() {
        assert_eq!(makesingular("vortices"), "vortex");
    }

    #[test]
    fn test_singular_oxen_to_ox() {
        assert_eq!(makesingular("oxen"), "ox");
    }

    #[test]
    fn test_singular_invariant() {
        assert_eq!(makesingular("boots"), "boots");
        assert_eq!(makesingular("samurai"), "samurai");
        assert_eq!(makesingular("sheep"), "sheep");
    }

    #[test]
    fn test_singular_compound() {
        assert_eq!(makesingular("potions of healing"), "potion of healing");
        assert_eq!(
            makesingular("scrolls labeled ZELGO MER"),
            "scroll labeled ZELGO MER"
        );
    }

    #[test]
    fn test_singular_empty() {
        assert_eq!(makesingular(""), "");
    }

    #[test]
    fn test_singular_one_off_children() {
        assert_eq!(makesingular("children"), "child");
    }

    #[test]
    fn test_singular_one_off_teeth() {
        assert_eq!(makesingular("teeth"), "tooth");
    }

    #[test]
    fn test_singular_one_off_feet() {
        assert_eq!(makesingular("feet"), "foot");
    }

    #[test]
    fn test_singular_one_off_mice() {
        assert_eq!(makesingular("mice"), "mouse");
    }

    #[test]
    fn test_singular_one_off_geese() {
        assert_eq!(makesingular("geese"), "goose");
    }

    #[test]
    fn test_singular_one_off_djinn() {
        assert_eq!(makesingular("djinn"), "djinni");
    }

    #[test]
    fn test_singular_eaux_to_eau() {
        assert_eq!(makesingular("chateaux"), "chateau");
    }

    // -- typename tests -------------------------------------------------------

    #[test]
    fn test_typename_potion() {
        let defs = potion_defs();
        assert_eq!(typename(ObjectTypeId(0), &defs), "potion of healing");
    }

    #[test]
    fn test_typename_scroll() {
        let defs = mixed_defs();
        assert_eq!(typename(ObjectTypeId(3), &defs), "scroll of light");
        assert_eq!(typename(ObjectTypeId(4), &defs), "scroll of identify");
    }

    #[test]
    fn test_typename_weapon() {
        let defs = mixed_defs();
        assert_eq!(typename(ObjectTypeId(6), &defs), "long sword");
    }

    #[test]
    fn test_typename_wand() {
        let def = make_def(0, "fire", ObjectClass::Wand, Some("oak"), 100);
        assert_eq!(typename(ObjectTypeId(0), &[def]), "wand of fire");
    }

    #[test]
    fn test_typename_ring() {
        let def = make_def(0, "protection", ObjectClass::Ring, Some("jade"), 100);
        assert_eq!(typename(ObjectTypeId(0), &[def]), "ring of protection");
    }

    #[test]
    fn test_typename_amulet() {
        let def = make_def(0, "ESP", ObjectClass::Amulet, Some("triangular"), 150);
        assert_eq!(typename(ObjectTypeId(0), &[def]), "amulet of ESP");
    }

    #[test]
    fn test_typename_spellbook() {
        let def = make_def(0, "fireball", ObjectClass::Spellbook, Some("ragged"), 100);
        assert_eq!(typename(ObjectTypeId(0), &[def]), "spellbook of fireball");
    }

    #[test]
    fn test_typename_unknown_otyp() {
        let defs = potion_defs();
        assert_eq!(typename(ObjectTypeId(999), &defs), "strange object");
    }

    #[test]
    fn test_typename_food_no_prefix() {
        let def = make_def(0, "slime mold", ObjectClass::Food, None, 5);
        assert_eq!(typename(ObjectTypeId(0), &[def]), "slime mold");
    }

    #[test]
    fn test_typename_armor_no_prefix() {
        let mut def = make_def(0, "plate mail", ObjectClass::Armor, None, 600);
        def.armor = Some(nethack_babel_data::ArmorInfo {
            ac_bonus: -7,
            magic_cancel: 0,
            category: ArmorCategory::Suit,
        });
        assert_eq!(typename(ObjectTypeId(0), &[def]), "plate mail");
    }
}
