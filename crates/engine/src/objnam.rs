//! Object naming and display system.
//!
//! This module provides convenience functions for generating human-readable
//! item names, building on the core name pipeline in `identification`.
//! It corresponds to C NetHack's `objnam.c`.
//!
//! The primary name generation functions (`xname`, `doname`, `an`, `the`,
//! `makeplural`, `makesingular`, `typename`, `erosion_prefix`) live in
//! `crate::identification` and are re-exported here for ergonomic access.
//! This module adds higher-level wrappers (`upstart`, `the_xname`, `yname`,
//! `simple_typename`) and the erosion adjective system.

use hecs::Entity;

use nethack_babel_data::{Material, ObjectClass, ObjectDef, ObjectTypeId};

use crate::identification::{self, IdentificationState};
use crate::o_init::AppearanceTable;
use crate::world::GameWorld;

// ---------------------------------------------------------------------------
// Re-exports from identification
// ---------------------------------------------------------------------------

pub use crate::identification::{
    an, doname, erosion_prefix, just_an, makeplural, makesingular, the, typename, xname,
};

// ---------------------------------------------------------------------------
// Erosion types and adjectives
// ---------------------------------------------------------------------------

/// Erosion level (0–3), matching C NetHack's `oeroded`/`oeroded2` fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErosionLevel {
    None,
    Light,
    Medium,
    Severe,
}

impl ErosionLevel {
    /// Convert from the raw integer value (0–3) used in `Erosion.eroded`
    /// and `Erosion.eroded2`.
    pub fn from_raw(val: u8) -> Self {
        match val {
            0 => Self::None,
            1 => Self::Light,
            2 => Self::Medium,
            _ => Self::Severe,
        }
    }
}

/// The type of erosion damage an item has suffered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErosionType {
    /// Iron items: rust.
    Rust,
    /// Flammable items: fire/burn.
    Fire,
    /// Copper/iron items: corrosion.
    Corrode,
    /// Organic items: rot.
    Rot,
}

/// Return the erosion adjective for a given erosion type and level.
///
/// Returns `""` for `ErosionLevel::None`.
///
/// ```ignore
/// assert_eq!(erosion_adjective(ErosionType::Rust, ErosionLevel::Light), "rusty");
/// assert_eq!(erosion_adjective(ErosionType::Rust, ErosionLevel::Medium), "very rusty");
/// assert_eq!(erosion_adjective(ErosionType::Rust, ErosionLevel::Severe), "thoroughly rusty");
/// ```
pub fn erosion_adjective(erosion_type: ErosionType, level: ErosionLevel) -> &'static str {
    match (erosion_type, level) {
        (_, ErosionLevel::None) => "",
        (ErosionType::Rust, ErosionLevel::Light) => "rusty",
        (ErosionType::Rust, ErosionLevel::Medium) => "very rusty",
        (ErosionType::Rust, ErosionLevel::Severe) => "thoroughly rusty",
        (ErosionType::Fire, ErosionLevel::Light) => "burnt",
        (ErosionType::Fire, ErosionLevel::Medium) => "very burnt",
        (ErosionType::Fire, ErosionLevel::Severe) => "thoroughly burnt",
        (ErosionType::Corrode, ErosionLevel::Light) => "corroded",
        (ErosionType::Corrode, ErosionLevel::Medium) => "very corroded",
        (ErosionType::Corrode, ErosionLevel::Severe) => "thoroughly corroded",
        (ErosionType::Rot, ErosionLevel::Light) => "rotted",
        (ErosionType::Rot, ErosionLevel::Medium) => "very rotted",
        (ErosionType::Rot, ErosionLevel::Severe) => "thoroughly rotted",
    }
}

/// Determine the appropriate `ErosionType` for primary erosion (`oeroded`)
/// based on item material.
pub fn primary_erosion_type(mat: Material, class: ObjectClass) -> ErosionType {
    if identification::is_rustprone(mat) {
        ErosionType::Rust
    } else if identification::is_crackable(mat, class) {
        // Cracked glass uses the same slot as fire/burn in C.
        ErosionType::Fire
    } else {
        ErosionType::Fire
    }
}

/// Determine the appropriate `ErosionType` for secondary erosion (`oeroded2`)
/// based on item material.
pub fn secondary_erosion_type(mat: Material) -> ErosionType {
    if identification::is_corrodeable(mat) {
        ErosionType::Corrode
    } else {
        ErosionType::Rot
    }
}

// ---------------------------------------------------------------------------
// String utilities
// ---------------------------------------------------------------------------

/// Capitalize the first character of a string.
///
/// Mirrors C `upstart()` from hacklib.c.
///
/// ```ignore
/// assert_eq!(upstart("the sword"), "The sword");
/// assert_eq!(upstart(""), "");
/// ```
pub fn upstart(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let mut result = String::with_capacity(s.len());
            for upper in c.to_uppercase() {
                result.push(upper);
            }
            result.extend(chars);
            result
        }
    }
}

/// Lowercase the first character of a string.
///
/// Mirrors C `lowc()` usage pattern.
pub fn lcase(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let mut result = String::with_capacity(s.len());
            for lower in c.to_lowercase() {
                result.push(lower);
            }
            result.extend(chars);
            result
        }
    }
}

// ---------------------------------------------------------------------------
// Name wrappers
// ---------------------------------------------------------------------------

/// Return the base type name for an object type.
///
/// This is a convenience wrapper around `identification::typename` that
/// matches the C `simple_typename()` interface.
pub fn simple_typename(type_id: ObjectTypeId, obj_defs: &[ObjectDef]) -> String {
    identification::typename(type_id, obj_defs)
}

/// Return `"the <xname>"` for an item — used in combat messages like
/// "The long sword hits!".
///
/// Mirrors C `the(xname(obj))` pattern.
pub fn the_xname(
    item: Entity,
    world: &GameWorld,
    id_state: &IdentificationState,
    obj_defs: &[ObjectDef],
) -> String {
    let base = identification::xname(item, world, id_state, obj_defs);
    identification::the(&base)
}

/// Return `"The <xname>"` with capitalized "The" — for sentence-initial
/// position.
///
/// Mirrors C `The(xname(obj))` pattern.
pub fn the_xname_upper(
    item: Entity,
    world: &GameWorld,
    id_state: &IdentificationState,
    obj_defs: &[ObjectDef],
) -> String {
    upstart(&the_xname(item, world, id_state, obj_defs))
}

/// Return `"your <xname>"` — possessive form for the player's items.
///
/// Mirrors C `yname()` from objnam.c.
pub fn yname(
    item: Entity,
    world: &GameWorld,
    id_state: &IdentificationState,
    obj_defs: &[ObjectDef],
) -> String {
    let base = identification::xname(item, world, id_state, obj_defs);
    format!("your {}", base)
}

/// Return `"Your <xname>"` — capitalized possessive for sentence starts.
///
/// Mirrors C `Yname2()` from objnam.c.
pub fn yname_upper(
    item: Entity,
    world: &GameWorld,
    id_state: &IdentificationState,
    obj_defs: &[ObjectDef],
) -> String {
    let base = identification::xname(item, world, id_state, obj_defs);
    format!("Your {}", base)
}

/// Return `"an <xname>"` — with article, for messages like
/// "You see an arrow here."
pub fn an_xname(
    item: Entity,
    world: &GameWorld,
    id_state: &IdentificationState,
    obj_defs: &[ObjectDef],
) -> String {
    let base = identification::xname(item, world, id_state, obj_defs);
    identification::an(&base)
}

// ---------------------------------------------------------------------------
// Appearance-based naming
// ---------------------------------------------------------------------------

/// Get the display name for an unidentified item using the appearance table.
///
/// If `is_identified` is true, returns `item_name` as-is.
/// If not identified and the player has named the type (`is_called`),
/// returns e.g. `"ruby potion called heal"`.
/// Otherwise returns the shuffled appearance name from the table.
pub fn display_name_with_appearance(
    item_name: &str,
    item_class: char,
    type_index: usize,
    is_identified: bool,
    is_called: Option<&str>,
    appearance_table: &AppearanceTable,
) -> String {
    if is_identified {
        return item_name.to_string();
    }

    if let Some(called) = is_called
        && let Some(appearance) = appearance_table.unidentified_name(item_class, type_index)
    {
        return format!("{} called {}", appearance, called);
    }

    if let Some(appearance) = appearance_table.unidentified_name(item_class, type_index) {
        appearance
    } else {
        item_name.to_string()
    }
}

/// Get just the appearance string for a given item type.
///
/// Returns the raw appearance descriptor (e.g. `"ruby"` for a potion,
/// `"ZELGO MER"` for a scroll) without the class suffix.
pub fn item_appearance(
    item_class: char,
    type_index: usize,
    appearance_table: &AppearanceTable,
) -> Option<String> {
    appearance_table
        .appearance(item_class, type_index)
        .map(String::from)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::identification::IdentificationState;
    use crate::items::{SpawnLocation, spawn_item};
    use crate::world::GameWorld;
    use nethack_babel_data::{Color, Material, ObjectClass, ObjectDef, ObjectTypeId};

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

    // -- upstart tests ------------------------------------------------------

    #[test]
    fn test_upstart_basic() {
        assert_eq!(upstart("the sword"), "The sword");
    }

    #[test]
    fn test_upstart_already_upper() {
        assert_eq!(upstart("The sword"), "The sword");
    }

    #[test]
    fn test_upstart_empty() {
        assert_eq!(upstart(""), "");
    }

    #[test]
    fn test_upstart_single_char() {
        assert_eq!(upstart("a"), "A");
    }

    // -- lcase tests --------------------------------------------------------

    #[test]
    fn test_lcase_basic() {
        assert_eq!(lcase("The sword"), "the sword");
    }

    #[test]
    fn test_lcase_empty() {
        assert_eq!(lcase(""), "");
    }

    // -- an() / the() re-export tests ---------------------------------------

    #[test]
    fn test_an_vowel() {
        assert_eq!(an("amulet"), "an amulet");
    }

    #[test]
    fn test_an_consonant() {
        assert_eq!(an("sword"), "a sword");
    }

    #[test]
    fn test_the_basic() {
        assert_eq!(the("Excalibur"), "Excalibur");
    }

    #[test]
    fn test_the_lowercase() {
        assert_eq!(the("long sword"), "the long sword");
    }

    #[test]
    fn test_the_of_pattern() {
        assert_eq!(the("Amulet of Yendor"), "the Amulet of Yendor");
    }

    // -- makeplural re-export tests -----------------------------------------

    #[test]
    fn test_makeplural_basic() {
        assert_eq!(makeplural("arrow"), "arrows");
    }

    #[test]
    fn test_makeplural_es() {
        assert_eq!(makeplural("torch"), "torches");
    }

    #[test]
    fn test_makeplural_ies() {
        assert_eq!(makeplural("ruby"), "rubies");
    }

    #[test]
    fn test_makeplural_ves() {
        assert_eq!(makeplural("loaf"), "loaves");
    }

    #[test]
    fn test_makeplural_special() {
        assert_eq!(makeplural("tooth"), "teeth");
    }

    #[test]
    fn test_makeplural_of() {
        assert_eq!(makeplural("potion of healing"), "potions of healing");
    }

    // -- makesingular re-export tests ---------------------------------------

    #[test]
    fn test_makesingular_basic() {
        assert_eq!(makesingular("arrows"), "arrow");
    }

    #[test]
    fn test_makesingular_teeth() {
        assert_eq!(makesingular("teeth"), "tooth");
    }

    // -- erosion_adjective tests --------------------------------------------

    #[test]
    fn test_erosion_adjective_none() {
        assert_eq!(erosion_adjective(ErosionType::Rust, ErosionLevel::None), "");
    }

    #[test]
    fn test_erosion_adjective_rusty() {
        assert_eq!(
            erosion_adjective(ErosionType::Rust, ErosionLevel::Light),
            "rusty"
        );
        assert_eq!(
            erosion_adjective(ErosionType::Rust, ErosionLevel::Medium),
            "very rusty"
        );
        assert_eq!(
            erosion_adjective(ErosionType::Rust, ErosionLevel::Severe),
            "thoroughly rusty"
        );
    }

    #[test]
    fn test_erosion_adjective_burnt() {
        assert_eq!(
            erosion_adjective(ErosionType::Fire, ErosionLevel::Light),
            "burnt"
        );
        assert_eq!(
            erosion_adjective(ErosionType::Fire, ErosionLevel::Medium),
            "very burnt"
        );
    }

    #[test]
    fn test_erosion_adjective_corroded() {
        assert_eq!(
            erosion_adjective(ErosionType::Corrode, ErosionLevel::Light),
            "corroded"
        );
        assert_eq!(
            erosion_adjective(ErosionType::Corrode, ErosionLevel::Severe),
            "thoroughly corroded"
        );
    }

    #[test]
    fn test_erosion_adjective_rotted() {
        assert_eq!(
            erosion_adjective(ErosionType::Rot, ErosionLevel::Light),
            "rotted"
        );
        assert_eq!(
            erosion_adjective(ErosionType::Rot, ErosionLevel::Severe),
            "thoroughly rotted"
        );
    }

    // -- ErosionLevel::from_raw tests ---------------------------------------

    #[test]
    fn test_erosion_level_from_raw() {
        assert_eq!(ErosionLevel::from_raw(0), ErosionLevel::None);
        assert_eq!(ErosionLevel::from_raw(1), ErosionLevel::Light);
        assert_eq!(ErosionLevel::from_raw(2), ErosionLevel::Medium);
        assert_eq!(ErosionLevel::from_raw(3), ErosionLevel::Severe);
        assert_eq!(ErosionLevel::from_raw(255), ErosionLevel::Severe);
    }

    // -- the_xname / yname tests (require world) ---------------------------

    #[test]
    fn test_the_xname() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "long sword", ObjectClass::Weapon, None, 15);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        {
            let mut k = world
                .get_component_mut::<nethack_babel_data::KnowledgeState>(item)
                .unwrap();
            k.dknown = true;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = the_xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "the long sword");
    }

    #[test]
    fn test_the_xname_upper() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "long sword", ObjectClass::Weapon, None, 15);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        {
            let mut k = world
                .get_component_mut::<nethack_babel_data::KnowledgeState>(item)
                .unwrap();
            k.dknown = true;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = the_xname_upper(item, &world, &id_state, &[def]);
        assert_eq!(name, "The long sword");
    }

    #[test]
    fn test_yname() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "long sword", ObjectClass::Weapon, None, 15);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        {
            let mut k = world
                .get_component_mut::<nethack_babel_data::KnowledgeState>(item)
                .unwrap();
            k.dknown = true;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = yname(item, &world, &id_state, &[def]);
        assert_eq!(name, "your long sword");
    }

    #[test]
    fn test_yname_upper() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "long sword", ObjectClass::Weapon, None, 15);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        {
            let mut k = world
                .get_component_mut::<nethack_babel_data::KnowledgeState>(item)
                .unwrap();
            k.dknown = true;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = yname_upper(item, &world, &id_state, &[def]);
        assert_eq!(name, "Your long sword");
    }

    // -- simple_typename test -----------------------------------------------

    #[test]
    fn test_simple_typename_weapon() {
        let defs = vec![make_def(0, "long sword", ObjectClass::Weapon, None, 15)];
        let name = simple_typename(ObjectTypeId(0), &defs);
        assert_eq!(name, "long sword");
    }

    #[test]
    fn test_simple_typename_potion() {
        let defs = vec![make_def(
            0,
            "healing",
            ObjectClass::Potion,
            Some("ruby"),
            20,
        )];
        let name = simple_typename(ObjectTypeId(0), &defs);
        assert_eq!(name, "potion of healing");
    }

    #[test]
    fn test_simple_typename_scroll() {
        let defs = vec![make_def(
            0,
            "identify",
            ObjectClass::Scroll,
            Some("ZELGO MER"),
            20,
        )];
        let name = simple_typename(ObjectTypeId(0), &defs);
        assert_eq!(name, "scroll of identify");
    }

    // -- an_xname test ------------------------------------------------------

    #[test]
    fn test_an_xname() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let def = make_def(0, "arrow", ObjectClass::Weapon, None, 2);
        let item = spawn_item(&mut world, &def, SpawnLocation::Floor(5, 5), None);
        {
            let mut k = world
                .get_component_mut::<nethack_babel_data::KnowledgeState>(item)
                .unwrap();
            k.dknown = true;
        }

        let mut id_state = IdentificationState::new(1);
        id_state.discover_type(ObjectTypeId(0));

        let name = an_xname(item, &world, &id_state, &[def]);
        assert_eq!(name, "an arrow");
    }

    // -- primary/secondary erosion type tests --------------------------------

    #[test]
    fn test_primary_erosion_type_iron() {
        assert_eq!(
            primary_erosion_type(Material::Iron, ObjectClass::Weapon),
            ErosionType::Rust
        );
    }

    #[test]
    fn test_primary_erosion_type_wood() {
        assert_eq!(
            primary_erosion_type(Material::Wood, ObjectClass::Weapon),
            ErosionType::Fire
        );
    }

    #[test]
    fn test_secondary_erosion_type_iron() {
        assert_eq!(secondary_erosion_type(Material::Iron), ErosionType::Corrode);
    }

    #[test]
    fn test_secondary_erosion_type_leather() {
        assert_eq!(secondary_erosion_type(Material::Leather), ErosionType::Rot);
    }

    // -- display_name_with_appearance tests ----------------------------------

    fn make_appearance_table() -> AppearanceTable {
        use rand::SeedableRng;
        use rand_pcg::Pcg64;
        let mut rng = Pcg64::seed_from_u64(42);
        AppearanceTable::new(&mut rng)
    }

    #[test]
    fn test_display_name_identified() {
        let table = make_appearance_table();
        let name = display_name_with_appearance("healing", '!', 0, true, None, &table);
        assert_eq!(name, "healing");
    }

    #[test]
    fn test_display_name_unidentified_uses_appearance() {
        let table = make_appearance_table();
        let name = display_name_with_appearance("healing", '!', 0, false, None, &table);
        // Should be "<color> potion", not "healing"
        assert!(name.ends_with(" potion"), "got: {}", name);
        assert_ne!(name, "healing");
    }

    #[test]
    fn test_display_name_called() {
        let table = make_appearance_table();
        let name = display_name_with_appearance("healing", '!', 0, false, Some("heal"), &table);
        // Should be "<color> potion called heal"
        assert!(name.ends_with(" called heal"), "got: {}", name);
        assert!(name.contains("potion"), "got: {}", name);
    }

    #[test]
    fn test_display_name_scroll_unidentified() {
        let table = make_appearance_table();
        let name = display_name_with_appearance("identify", '?', 0, false, None, &table);
        assert!(name.starts_with("scroll labeled "), "got: {}", name);
    }

    #[test]
    fn test_item_appearance_lookup() {
        let table = make_appearance_table();
        // Potion appearance exists
        let app = item_appearance('!', 0, &table);
        assert!(app.is_some());
        // Scroll appearance exists
        let app = item_appearance('?', 0, &table);
        assert!(app.is_some());
        // Ring appearance exists
        let app = item_appearance('=', 0, &table);
        assert!(app.is_some());
        // Wand appearance exists
        let app = item_appearance('/', 0, &table);
        assert!(app.is_some());
        // Unknown class returns None
        let app = item_appearance('X', 0, &table);
        assert!(app.is_none());
    }

    #[test]
    fn test_item_appearance_out_of_bounds() {
        let table = make_appearance_table();
        assert!(item_appearance('!', 999, &table).is_none());
    }
}
