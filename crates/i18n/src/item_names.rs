//! Item naming pipeline, equivalent to NetHack's xname()/doname().
//!
//! This module translates ECS components into display strings using the
//! [`NounPhrase`] builder from [`crate::noun_phrase`].

use fluent::FluentArgs;
use nethack_babel_data::{
    BucStatus, Enchantment, Erosion, KnowledgeState, Material, ObjectClass, ObjectCore, ObjectDef,
    ObjectExtra,
};

use crate::locale::LocaleManager;
use crate::noun_phrase::{Article, BucLabel, NounPhrase};

// ---------------------------------------------------------------------------
// Configuration / context flags
// ---------------------------------------------------------------------------

/// Options that influence how an item name is rendered.
/// Mirrors the various global flags that C NetHack inspects during doname().
#[derive(Debug, Clone)]
pub struct NamingContext {
    /// Whether the object type has been "discovered" (oc_name_known).
    pub type_known: bool,
    /// Player-assigned type name (oc_uname), e.g. "heal?" for an unidentified
    /// potion the player called something.
    pub type_called: Option<String>,
    /// Whether the `implicit_uncursed` option is on (default true).
    /// When true, "uncursed" is omitted in many situations.
    pub implicit_uncursed: bool,
    /// Whether the viewer is a Priest (always sees BUC).
    pub is_priest: bool,
    /// Equipment status suffix, e.g. "(weapon in hand)", "(being worn)".
    pub equip_status: Option<String>,
}

impl Default for NamingContext {
    fn default() -> Self {
        Self {
            type_known: true,
            type_called: None,
            implicit_uncursed: true,
            is_priest: false,
            equip_status: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build a display name for an item, equivalent to NetHack's doname().
///
/// Assembles the full inventory-style name including article, BUC prefix,
/// erosion adjectives, enchantment, base name, and suffixes.
///
/// When `locale` is `Some` and the language is CJK, the base name is
/// translated via `locale.translate_object_name()` and a classifier is
/// applied from `locale.classifier()`.
#[allow(clippy::too_many_arguments)]
pub fn doname(
    core: &ObjectCore,
    def: &ObjectDef,
    buc: Option<&BucStatus>,
    knowledge: Option<&KnowledgeState>,
    enchantment: Option<&Enchantment>,
    erosion: Option<&Erosion>,
    extra: Option<&ObjectExtra>,
    ctx: &NamingContext,
) -> String {
    doname_locale(
        core,
        def,
        buc,
        knowledge,
        enchantment,
        erosion,
        extra,
        ctx,
        None,
    )
}

/// Like [`doname`] but accepts an optional locale for i18n translation.
///
/// When a locale is provided, object names are translated, grammatical
/// metadata (gender, plural) is attached, and CJK classifiers are applied.
#[allow(clippy::too_many_arguments)]
pub fn doname_locale(
    core: &ObjectCore,
    def: &ObjectDef,
    buc: Option<&BucStatus>,
    knowledge: Option<&KnowledgeState>,
    enchantment: Option<&Enchantment>,
    erosion: Option<&Erosion>,
    extra: Option<&ObjectExtra>,
    ctx: &NamingContext,
    locale: Option<&LocaleManager>,
) -> String {
    let mut np = build_noun_phrase(
        core,
        def,
        buc,
        knowledge,
        enchantment,
        erosion,
        extra,
        ctx,
        locale,
    );

    if let Some(loc) = locale {
        // Translate the base name and retrieve grammatical metadata.
        let en_base = &np.base_name;
        let (translated, meta) = loc.translate_object_name_with_meta(en_base);
        if translated != en_base {
            np.base_name = translated.to_string();
        }

        // Attach gender and plural from translation metadata.
        if let Some(gender) = meta.gender {
            np = np.with_gender(gender);
        }
        if let Some(plural) = meta.plural {
            np = np.with_plural(plural);
        }

        // Set CJK classifier from locale's classifier table.
        if loc.is_cjk()
            && let Some(clf) = loc.classifier()
        {
            let class_name = format!("{:?}", def.class).to_lowercase();
            let classifier = clf.get(&class_name, &np.base_name);
            np = np.with_classifier(classifier);
        }
    }

    // Add equipment status suffix.
    if let Some(ref suffix) = ctx.equip_status {
        np = np.with_suffix(suffix.clone());
    }

    match locale {
        Some(loc) => np.render(loc),
        None => {
            let default_en = LocaleManager::new();
            np.render(&default_en)
        }
    }
}

/// Build an xname-equivalent: base name without article or quantity prefix.
///
/// This is the inner name used by other naming functions (cxname, yname, etc.)
pub fn xname(
    core: &ObjectCore,
    def: &ObjectDef,
    knowledge: Option<&KnowledgeState>,
    extra: Option<&ObjectExtra>,
    ctx: &NamingContext,
) -> String {
    let base = base_name(def, knowledge, ctx, None);

    // Pluralize if quantity > 1.
    let name = if core.quantity > 1 {
        crate::noun_phrase::english_plural_fallback(&base)
    } else {
        base
    };

    // Append "named X" if the object has an individual name.
    if let Some(extra) = extra
        && let Some(ref oname) = extra.name
        && knowledge.is_none_or(|k| k.dknown)
    {
        return format!("{} named {}", name, oname);
    }

    name
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Determine the base name string for an object, following the xname() logic
/// from the spec.  Handles type discovery, appearances, and "called" names.
///
/// When `locale` is `Some`, class patterns ("potion of X", "scroll labeled Y")
/// are resolved via Fluent templates instead of hardcoded `format!()`.
fn base_name(
    def: &ObjectDef,
    knowledge: Option<&KnowledgeState>,
    ctx: &NamingContext,
    locale: Option<&LocaleManager>,
) -> String {
    let dknown = knowledge.is_none_or(|k| k.dknown);

    match def.class {
        // Classes where unidentified items use a generic class name + appearance
        ObjectClass::Potion => potion_base_name(def, dknown, ctx, locale),
        ObjectClass::Scroll => scroll_base_name(def, dknown, ctx, locale),
        ObjectClass::Wand => wand_base_name(def, dknown, ctx, locale),
        ObjectClass::Ring => ring_base_name(def, dknown, ctx, locale),
        ObjectClass::Amulet => amulet_base_name(def, dknown, ctx, locale),
        ObjectClass::Spellbook => spellbook_base_name(def, dknown, ctx, locale),
        ObjectClass::Gem => gem_base_name(def, dknown, ctx, locale),

        // Classes that (almost) always use their actual name
        ObjectClass::Weapon
        | ObjectClass::Tool
        | ObjectClass::Venom
        | ObjectClass::Armor
        | ObjectClass::Food
        | ObjectClass::Coin
        | ObjectClass::Chain
        | ObjectClass::Ball
        | ObjectClass::Rock => {
            if !dknown && let Some(ref appearance) = def.appearance {
                return appearance.clone();
            }
            if ctx.type_known {
                def.name.clone()
            } else if let Some(ref called) = ctx.type_called {
                let dn = def.appearance.as_deref().unwrap_or(&def.name);
                if let Some(loc) = locale {
                    let mut args = FluentArgs::new();
                    args.set("base", dn.to_string());
                    args.set("called", called.clone());
                    loc.translate("item-generic-called", Some(&args))
                } else {
                    format!("{} called {}", dn, called)
                }
            } else if let Some(ref appearance) = def.appearance {
                appearance.clone()
            } else {
                def.name.clone()
            }
        }

        // Fallback
        _ => def.name.clone(),
    }
}

fn potion_base_name(
    def: &ObjectDef,
    dknown: bool,
    ctx: &NamingContext,
    locale: Option<&LocaleManager>,
) -> String {
    if !dknown {
        return locale_or_hardcoded(locale, "item-potion-generic", "potion");
    }
    if ctx.type_known {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("name", def.name.clone());
            loc.translate("item-potion-identified", Some(&args))
        } else {
            format!("potion of {}", def.name)
        }
    } else if let Some(ref called) = ctx.type_called {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("called", called.clone());
            loc.translate("item-potion-called", Some(&args))
        } else {
            format!("potion called {}", called)
        }
    } else if let Some(ref appearance) = def.appearance {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("appearance", appearance.clone());
            loc.translate("item-potion-appearance", Some(&args))
        } else {
            format!("{} potion", appearance)
        }
    } else {
        locale_or_hardcoded(locale, "item-potion-generic", "potion")
    }
}

fn scroll_base_name(
    def: &ObjectDef,
    dknown: bool,
    ctx: &NamingContext,
    locale: Option<&LocaleManager>,
) -> String {
    if !dknown {
        return locale_or_hardcoded(locale, "item-scroll-generic", "scroll");
    }
    if ctx.type_known {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("name", def.name.clone());
            loc.translate("item-scroll-identified", Some(&args))
        } else {
            format!("scroll of {}", def.name)
        }
    } else if let Some(ref called) = ctx.type_called {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("called", called.clone());
            loc.translate("item-scroll-called", Some(&args))
        } else {
            format!("scroll called {}", called)
        }
    } else if let Some(ref appearance) = def.appearance {
        if def.is_magic {
            if let Some(loc) = locale {
                let mut args = FluentArgs::new();
                args.set("label", appearance.clone());
                loc.translate("item-scroll-labeled", Some(&args))
            } else {
                format!("scroll labeled {}", appearance)
            }
        } else {
            if let Some(loc) = locale {
                let mut args = FluentArgs::new();
                args.set("appearance", appearance.clone());
                loc.translate("item-scroll-appearance", Some(&args))
            } else {
                format!("{} scroll", appearance)
            }
        }
    } else {
        locale_or_hardcoded(locale, "item-scroll-generic", "scroll")
    }
}

fn wand_base_name(
    def: &ObjectDef,
    dknown: bool,
    ctx: &NamingContext,
    locale: Option<&LocaleManager>,
) -> String {
    if !dknown {
        return locale_or_hardcoded(locale, "item-wand-generic", "wand");
    }
    if ctx.type_known {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("name", def.name.clone());
            loc.translate("item-wand-identified", Some(&args))
        } else {
            format!("wand of {}", def.name)
        }
    } else if let Some(ref called) = ctx.type_called {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("called", called.clone());
            loc.translate("item-wand-called", Some(&args))
        } else {
            format!("wand called {}", called)
        }
    } else if let Some(ref appearance) = def.appearance {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("appearance", appearance.clone());
            loc.translate("item-wand-appearance", Some(&args))
        } else {
            format!("{} wand", appearance)
        }
    } else {
        locale_or_hardcoded(locale, "item-wand-generic", "wand")
    }
}

fn ring_base_name(
    def: &ObjectDef,
    dknown: bool,
    ctx: &NamingContext,
    locale: Option<&LocaleManager>,
) -> String {
    if !dknown {
        return locale_or_hardcoded(locale, "item-ring-generic", "ring");
    }
    if ctx.type_known {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("name", def.name.clone());
            loc.translate("item-ring-identified", Some(&args))
        } else {
            format!("ring of {}", def.name)
        }
    } else if let Some(ref called) = ctx.type_called {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("called", called.clone());
            loc.translate("item-ring-called", Some(&args))
        } else {
            format!("ring called {}", called)
        }
    } else if let Some(ref appearance) = def.appearance {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("appearance", appearance.clone());
            loc.translate("item-ring-appearance", Some(&args))
        } else {
            format!("{} ring", appearance)
        }
    } else {
        locale_or_hardcoded(locale, "item-ring-generic", "ring")
    }
}

fn amulet_base_name(
    def: &ObjectDef,
    dknown: bool,
    ctx: &NamingContext,
    locale: Option<&LocaleManager>,
) -> String {
    if !dknown {
        return locale_or_hardcoded(locale, "item-amulet-generic", "amulet");
    }
    if ctx.type_known {
        // Use the actual name directly (e.g. "Amulet of Yendor")
        def.name.clone()
    } else if let Some(ref called) = ctx.type_called {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("called", called.clone());
            loc.translate("item-amulet-called", Some(&args))
        } else {
            format!("amulet called {}", called)
        }
    } else if let Some(ref appearance) = def.appearance {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("appearance", appearance.clone());
            loc.translate("item-amulet-appearance", Some(&args))
        } else {
            format!("{} amulet", appearance)
        }
    } else {
        locale_or_hardcoded(locale, "item-amulet-generic", "amulet")
    }
}

fn spellbook_base_name(
    def: &ObjectDef,
    dknown: bool,
    ctx: &NamingContext,
    locale: Option<&LocaleManager>,
) -> String {
    if !dknown {
        return locale_or_hardcoded(locale, "item-spellbook-generic", "spellbook");
    }
    if ctx.type_known {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("name", def.name.clone());
            loc.translate("item-spellbook-identified", Some(&args))
        } else {
            format!("spellbook of {}", def.name)
        }
    } else if let Some(ref called) = ctx.type_called {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("called", called.clone());
            loc.translate("item-spellbook-called", Some(&args))
        } else {
            format!("spellbook called {}", called)
        }
    } else if let Some(ref appearance) = def.appearance {
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("appearance", appearance.clone());
            loc.translate("item-spellbook-appearance", Some(&args))
        } else {
            format!("{} spellbook", appearance)
        }
    } else {
        locale_or_hardcoded(locale, "item-spellbook-generic", "spellbook")
    }
}

fn gem_base_name(
    def: &ObjectDef,
    dknown: bool,
    ctx: &NamingContext,
    locale: Option<&LocaleManager>,
) -> String {
    let is_stone = def.material == Material::Mineral;

    if !dknown {
        let (msg_id, fallback) = if is_stone {
            ("item-gem-stone", "stone")
        } else {
            ("item-gem-gem", "gem")
        };
        return locale_or_hardcoded(locale, msg_id, fallback);
    }
    if ctx.type_known {
        def.name.clone()
    } else if let Some(ref called) = ctx.type_called {
        let msg_id = if is_stone {
            "item-gem-called-stone"
        } else {
            "item-gem-called-gem"
        };
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("called", called.clone());
            loc.translate(msg_id, Some(&args))
        } else {
            let rock_or_gem = if is_stone { "stone" } else { "gem" };
            format!("{} called {}", rock_or_gem, called)
        }
    } else if let Some(ref appearance) = def.appearance {
        let msg_id = if is_stone {
            "item-gem-appearance-stone"
        } else {
            "item-gem-appearance-gem"
        };
        if let Some(loc) = locale {
            let mut args = FluentArgs::new();
            args.set("appearance", appearance.clone());
            loc.translate(msg_id, Some(&args))
        } else {
            let rock_or_gem = if is_stone { "stone" } else { "gem" };
            format!("{} {}", appearance, rock_or_gem)
        }
    } else {
        let (msg_id, fallback) = if is_stone {
            ("item-gem-stone", "stone")
        } else {
            ("item-gem-gem", "gem")
        };
        locale_or_hardcoded(locale, msg_id, fallback)
    }
}

/// Helper: translate via locale if available, else return hardcoded fallback.
fn locale_or_hardcoded(locale: Option<&LocaleManager>, msg_id: &str, fallback: &str) -> String {
    if let Some(loc) = locale {
        let result = loc.translate(msg_id, None);
        if result == msg_id {
            fallback.to_string()
        } else {
            result
        }
    } else {
        fallback.to_string()
    }
}

/// Build erosion adjectives string from an Erosion component.
///
/// Follows the add_erosion_words() logic from objnam.c:
/// - eroded 1 = "rusty", 2 = "very rusty", 3 = "thoroughly rusty" (for iron)
/// - eroded2 1 = "corroded", 2 = "very corroded", 3 = "thoroughly corroded"
fn erosion_string(erosion: &Erosion, material: Material) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();

    // Primary erosion
    if erosion.eroded > 0 {
        let degree = match erosion.eroded {
            2 => "very ",
            3 => "thoroughly ",
            _ => "",
        };
        let word = if is_rustprone(material) {
            "rusty"
        } else {
            "burnt"
        };
        parts.push(degree);
        parts.push(word);
    }

    // Secondary erosion
    if erosion.eroded2 > 0 {
        // Add space separator if we already have primary erosion words
        if !parts.is_empty() {
            parts.push(" ");
        }
        let degree = match erosion.eroded2 {
            2 => "very ",
            3 => "thoroughly ",
            _ => "",
        };
        let word = if is_corrodeable(material) {
            "corroded"
        } else {
            "rotted"
        };
        parts.push(degree);
        parts.push(word);
    }

    // Erodeproof
    if erosion.erodeproof {
        if !parts.is_empty() {
            parts.push(" ");
        }
        let word = if is_rustprone(material) {
            "rustproof"
        } else if is_corrodeable(material) {
            "corrodeproof"
        } else if is_flammable(material) {
            "fireproof"
        } else {
            "rotproof"
        };
        parts.push(word);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.concat())
    }
}

/// Whether the material is susceptible to rust (iron/steel).
fn is_rustprone(m: Material) -> bool {
    m == Material::Iron
}

/// Whether the material is susceptible to corrosion (iron or copper).
fn is_corrodeable(m: Material) -> bool {
    matches!(m, Material::Iron | Material::Copper)
}

/// Whether the material is flammable.
fn is_flammable(m: Material) -> bool {
    matches!(
        m,
        Material::Wax
            | Material::Veggy
            | Material::Flesh
            | Material::Paper
            | Material::Cloth
            | Material::Leather
            | Material::Wood
            | Material::Plastic
    )
}

/// Whether the material is damageable (can have erosion words displayed).
fn is_damageable(m: Material) -> bool {
    is_rustprone(m) || is_flammable(m) || is_corrodeable(m) || is_rottable(m)
}

/// Whether the material can rot.
fn is_rottable(m: Material) -> bool {
    matches!(
        m,
        Material::Wax
            | Material::Veggy
            | Material::Flesh
            | Material::Paper
            | Material::Cloth
            | Material::Leather
            | Material::Wood
            | Material::DragonHide
    )
}

/// Determine whether "uncursed" should be shown given the context.
///
/// Follows the show_uncursed() logic from objnam.c (section 3.3 of spec):
/// - If `implicit_uncursed` is false, always show it.
/// - If priest, never show it.
/// - For armor and rings, always show it (even when known+charged).
/// - For known+charged items of other classes, omit it.
/// - Otherwise show it.
fn should_show_uncursed(
    def: &ObjectDef,
    knowledge: Option<&KnowledgeState>,
    ctx: &NamingContext,
) -> bool {
    if !ctx.implicit_uncursed {
        return true;
    }
    if ctx.is_priest {
        return false;
    }
    let known = knowledge.is_some_and(|k| k.known);
    if known && def.is_charged {
        // Armor and Ring classes always show uncursed even when known+charged
        matches!(def.class, ObjectClass::Armor | ObjectClass::Ring)
    } else {
        // Not fully identified or not charged -- show uncursed
        true
    }
}

/// Build a [`NounPhrase`] from ECS components.
#[allow(clippy::too_many_arguments)]
fn build_noun_phrase(
    core: &ObjectCore,
    def: &ObjectDef,
    buc: Option<&BucStatus>,
    knowledge: Option<&KnowledgeState>,
    enchantment: Option<&Enchantment>,
    erosion: Option<&Erosion>,
    extra: Option<&ObjectExtra>,
    ctx: &NamingContext,
    locale: Option<&LocaleManager>,
) -> NounPhrase {
    // 1. Determine base name
    let base = base_name(def, knowledge, ctx, locale);

    let mut np = NounPhrase::new(&base);

    // 2. Quantity
    np = np.with_quantity(core.quantity);

    // 3. Article
    if core.quantity == 1 {
        np = np.with_article(Article::A);
    } else {
        np = np.with_article(Article::A); // triggers quantity prefix in render
    }

    // 4. BUC prefix
    if let Some(buc) = buc
        && buc.bknown
    {
        if buc.blessed {
            np = np.with_buc(BucLabel::Blessed);
        } else if buc.cursed {
            np = np.with_buc(BucLabel::Cursed);
        } else if should_show_uncursed(def, knowledge, ctx) {
            np = np.with_buc(BucLabel::Uncursed);
        }
    }

    // 5. Erosion adjectives (only for damageable items)
    if let Some(erosion) = erosion
        && is_damageable(def.material)
        && let Some(erosion_str) = erosion_string(erosion, def.material)
    {
        np = np.with_erosion(erosion_str);
    }

    // 6. Enchantment (only if known)
    if let Some(ench) = enchantment {
        let known = knowledge.is_some_and(|k| k.known);
        if known {
            np = np.with_enchantment(ench.spe);
        }
    }

    // 7. Individual name ("named X")
    if let Some(extra) = extra
        && let Some(ref oname) = extra.name
    {
        let dknown = knowledge.is_none_or(|k| k.dknown);
        if dknown {
            np = np.with_name(oname.clone());
        }
    }

    np
}

// =========================================================================
// Convenience constructors for tests
// =========================================================================

/// Create a minimal ObjectDef for testing.
#[cfg(test)]
fn test_object_def(name: &str, class: ObjectClass, material: Material) -> ObjectDef {
    ObjectDef {
        id: nethack_babel_data::ObjectTypeId(0),
        name: name.to_string(),
        appearance: None,
        class,
        color: nethack_babel_data::Color::Gray,
        material,
        weight: 40,
        cost: 10,
        nutrition: 0,
        prob: 0,
        is_magic: false,
        is_mergeable: false,
        is_charged: true, // weapons/armor default
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

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use nethack_babel_data::{ArmorCategory, ObjectTypeId};

    /// Helper: create a basic ObjectCore with given quantity.
    fn core(quantity: i32) -> ObjectCore {
        ObjectCore {
            otyp: ObjectTypeId(0),
            object_class: ObjectClass::Weapon,
            quantity,
            weight: 40,
            age: 0,
            inv_letter: None,
            artifact: None,
        }
    }

    fn default_ctx() -> NamingContext {
        NamingContext {
            type_known: true,
            type_called: None,
            implicit_uncursed: true,
            is_priest: false,
            equip_status: None,
        }
    }

    // -- Test 1: basic unidentified weapon (type_known=true gives actual name)
    #[test]
    fn test_basic_long_sword() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();

        let result = doname(&c, &def, None, None, None, None, None, &ctx);
        assert_eq!(result, "a long sword");
    }

    // -- Test 2: known enchantment
    #[test]
    fn test_known_enchantment() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();
        let ench = Enchantment { spe: 2 };
        let k = KnowledgeState {
            known: true,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };

        let result = doname(&c, &def, None, Some(&k), Some(&ench), None, None, &ctx);
        assert_eq!(result, "a +2 long sword");
    }

    // -- Test 3: blessed with known enchantment
    #[test]
    fn test_blessed_enchanted() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();
        let ench = Enchantment { spe: 2 };
        let k = KnowledgeState {
            known: true,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };
        let buc = BucStatus {
            blessed: true,
            cursed: false,
            bknown: true,
        };

        let result = doname(
            &c,
            &def,
            Some(&buc),
            Some(&k),
            Some(&ench),
            None,
            None,
            &ctx,
        );
        assert_eq!(result, "a blessed +2 long sword");
    }

    // -- Test 4: quantity + plural + enchantment
    #[test]
    fn test_quantity_plural() {
        let c = core(3);
        let mut def = test_object_def("dart", ObjectClass::Weapon, Material::Iron);
        def.is_charged = true;
        let ctx = default_ctx();
        let ench = Enchantment { spe: 0 };
        let k = KnowledgeState {
            known: true,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };

        let result = doname(&c, &def, None, Some(&k), Some(&ench), None, None, &ctx);
        assert_eq!(result, "3 +0 darts");
    }

    // -- Test 5: erosion (rusty)
    #[test]
    fn test_rusty_weapon() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();
        let ench = Enchantment { spe: 1 };
        let k = KnowledgeState {
            known: true,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };
        let erosion = Erosion {
            eroded: 1,
            eroded2: 0,
            erodeproof: false,
            greased: false,
        };

        let result = doname(
            &c,
            &def,
            None,
            Some(&k),
            Some(&ench),
            Some(&erosion),
            None,
            &ctx,
        );
        assert_eq!(result, "a rusty +1 long sword");
    }

    // -- Test 6: double erosion (very rusty + corroded)
    #[test]
    fn test_double_erosion() {
        let c = core(1);
        let mut def = test_object_def("iron chain mail", ObjectClass::Armor, Material::Iron);
        def.armor = Some(nethack_babel_data::ArmorInfo {
            category: ArmorCategory::Suit,
            ac_bonus: 5,
            magic_cancel: 0,
        });
        let ctx = default_ctx();
        let erosion = Erosion {
            eroded: 2,
            eroded2: 1,
            erodeproof: false,
            greased: false,
        };

        let result = doname(&c, &def, None, None, None, Some(&erosion), None, &ctx);
        assert_eq!(result, "a very rusty corroded iron chain mail");
    }

    // -- Test 7: article "an" before vowel (elven broadsword)
    #[test]
    fn test_article_an_vowel() {
        let c = core(1);
        let def = test_object_def("elven broadsword", ObjectClass::Weapon, Material::Wood);
        let ctx = default_ctx();

        let result = doname(&c, &def, None, None, None, None, None, &ctx);
        assert_eq!(result, "an elven broadsword");
    }

    // -- Test 8: potion identified vs not identified
    #[test]
    fn test_potion_identified() {
        let c = core(1);
        let mut def = test_object_def("healing", ObjectClass::Potion, Material::Glass);
        def.appearance = Some("milky".to_string());
        def.is_charged = false;

        // Identified
        let ctx = NamingContext {
            type_known: true,
            ..default_ctx()
        };
        let result = doname(&c, &def, None, None, None, None, None, &ctx);
        assert_eq!(result, "a potion of healing");
    }

    #[test]
    fn test_potion_unidentified() {
        let c = core(1);
        let mut def = test_object_def("healing", ObjectClass::Potion, Material::Glass);
        def.appearance = Some("milky".to_string());
        def.is_charged = false;

        let ctx = NamingContext {
            type_known: false,
            ..default_ctx()
        };
        let result = doname(&c, &def, None, None, None, None, None, &ctx);
        assert_eq!(result, "a milky potion");
    }

    // -- Test 9: scroll "called" by player
    #[test]
    fn test_scroll_called() {
        let c = core(1);
        let mut def = test_object_def("identify", ObjectClass::Scroll, Material::Paper);
        def.appearance = Some("ZELGO MER".to_string());
        def.is_magic = true;
        def.is_charged = false;

        let ctx = NamingContext {
            type_known: false,
            type_called: Some("FOOBAR".to_string()),
            ..default_ctx()
        };
        let result = doname(&c, &def, None, None, None, None, None, &ctx);
        assert_eq!(result, "a scroll called FOOBAR");
    }

    // -- Test 10: implicit uncursed for armor (always shown)
    #[test]
    fn test_implicit_uncursed_armor() {
        let c = core(1);
        let mut def = test_object_def("plate mail", ObjectClass::Armor, Material::Iron);
        def.armor = Some(nethack_babel_data::ArmorInfo {
            category: ArmorCategory::Suit,
            ac_bonus: 7,
            magic_cancel: 0,
        });
        let ctx = NamingContext {
            implicit_uncursed: true,
            ..default_ctx()
        };
        let ench = Enchantment { spe: 0 };
        let k = KnowledgeState {
            known: true,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };
        let buc = BucStatus {
            blessed: false,
            cursed: false,
            bknown: true,
        };

        let result = doname(
            &c,
            &def,
            Some(&buc),
            Some(&k),
            Some(&ench),
            None,
            None,
            &ctx,
        );
        assert_eq!(result, "an uncursed +0 plate mail");
    }

    // -- Test 11: implicit uncursed omitted for known weapon
    #[test]
    fn test_implicit_uncursed_weapon_omitted() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = NamingContext {
            implicit_uncursed: true,
            ..default_ctx()
        };
        let ench = Enchantment { spe: 1 };
        let k = KnowledgeState {
            known: true,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };
        let buc = BucStatus {
            blessed: false,
            cursed: false,
            bknown: true,
        };

        let result = doname(
            &c,
            &def,
            Some(&buc),
            Some(&k),
            Some(&ench),
            None,
            None,
            &ctx,
        );
        // known + charged + WEAPON -> omit "uncursed"
        assert_eq!(result, "a +1 long sword");
    }

    // -- Test 12: uncursed shown for weapon when enchantment NOT known
    #[test]
    fn test_uncursed_shown_unknown_enchantment() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = NamingContext {
            implicit_uncursed: true,
            ..default_ctx()
        };
        let k = KnowledgeState {
            known: false,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };
        let buc = BucStatus {
            blessed: false,
            cursed: false,
            bknown: true,
        };

        let result = doname(&c, &def, Some(&buc), Some(&k), None, None, None, &ctx);
        assert_eq!(result, "an uncursed long sword");
    }

    // -- Test 13: wand unidentified with appearance
    #[test]
    fn test_wand_unidentified() {
        let c = core(1);
        let mut def = test_object_def("fire", ObjectClass::Wand, Material::Wood);
        def.appearance = Some("oak".to_string());
        def.is_charged = true;

        let ctx = NamingContext {
            type_known: false,
            ..default_ctx()
        };
        let result = doname(&c, &def, None, None, None, None, None, &ctx);
        assert_eq!(result, "an oak wand");
    }

    // -- Test 14: ring with appearance
    #[test]
    fn test_ring_unidentified() {
        let c = core(1);
        let mut def = test_object_def("teleportation", ObjectClass::Ring, Material::Gemstone);
        def.appearance = Some("jade".to_string());
        def.is_charged = true;

        let ctx = NamingContext {
            type_known: false,
            ..default_ctx()
        };
        let result = doname(&c, &def, None, None, None, None, None, &ctx);
        assert_eq!(result, "a jade ring");
    }

    // -- Test 15: named item
    #[test]
    fn test_named_item() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();
        let extra = ObjectExtra {
            name: Some("Sting".to_string()),
            contained_monster: None,
        };

        let result = doname(&c, &def, None, None, None, None, Some(&extra), &ctx);
        assert_eq!(result, "a long sword named Sting");
    }

    // -- Test 16: cursed item
    #[test]
    fn test_cursed_item() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();
        let buc = BucStatus {
            blessed: false,
            cursed: true,
            bknown: true,
        };

        let result = doname(&c, &def, Some(&buc), None, None, None, None, &ctx);
        assert_eq!(result, "a cursed long sword");
    }

    // -- Test 17: erodeproof (rustproof)
    #[test]
    fn test_erodeproof() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();
        let erosion = Erosion {
            eroded: 0,
            eroded2: 0,
            erodeproof: true,
            greased: false,
        };
        let k = KnowledgeState {
            known: true,
            dknown: true,
            rknown: true,
            cknown: false,
            lknown: false,
            tknown: false,
        };

        let result = doname(&c, &def, None, Some(&k), None, Some(&erosion), None, &ctx);
        assert_eq!(result, "a rustproof long sword");
    }

    // -- Test 18: gem unidentified with appearance
    #[test]
    fn test_gem_unidentified() {
        let c = core(1);
        let mut def = test_object_def("emerald", ObjectClass::Gem, Material::Gemstone);
        def.appearance = Some("green".to_string());

        let ctx = NamingContext {
            type_known: false,
            ..default_ctx()
        };
        let result = doname(&c, &def, None, None, None, None, None, &ctx);
        assert_eq!(result, "a green gem");
    }

    // -- Test 19: equip status suffix
    #[test]
    fn test_equip_suffix() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = NamingContext {
            equip_status: Some("(weapon in hand)".to_string()),
            ..default_ctx()
        };

        let result = doname(&c, &def, None, None, None, None, None, &ctx);
        assert_eq!(result, "a long sword (weapon in hand)");
    }

    // -- Test 20: all segments combined
    #[test]
    fn test_full_combination() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = NamingContext {
            equip_status: Some("(weapon in hand)".to_string()),
            ..default_ctx()
        };
        let buc = BucStatus {
            blessed: true,
            cursed: false,
            bknown: true,
        };
        let ench = Enchantment { spe: 5 };
        let k = KnowledgeState {
            known: true,
            dknown: true,
            rknown: true,
            cknown: false,
            lknown: false,
            tknown: false,
        };
        let erosion = Erosion {
            eroded: 0,
            eroded2: 0,
            erodeproof: true,
            greased: false,
        };

        let result = doname(
            &c,
            &def,
            Some(&buc),
            Some(&k),
            Some(&ench),
            Some(&erosion),
            None,
            &ctx,
        );
        assert_eq!(result, "a blessed rustproof +5 long sword (weapon in hand)");
    }

    // -- Test 21: xname standalone
    #[test]
    fn test_xname_basic() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();

        let result = xname(&c, &def, None, None, &ctx);
        assert_eq!(result, "long sword");
    }

    // -- Test 22: xname with quantity (pluralized)
    #[test]
    fn test_xname_plural() {
        let c = core(3);
        let def = test_object_def("arrow", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();

        let result = xname(&c, &def, None, None, &ctx);
        assert_eq!(result, "arrows");
    }

    // -- Test 23: implicit_uncursed=false always shows uncursed
    #[test]
    fn test_no_implicit_uncursed() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = NamingContext {
            implicit_uncursed: false,
            ..default_ctx()
        };
        let ench = Enchantment { spe: 1 };
        let k = KnowledgeState {
            known: true,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };
        let buc = BucStatus {
            blessed: false,
            cursed: false,
            bknown: true,
        };

        let result = doname(
            &c,
            &def,
            Some(&buc),
            Some(&k),
            Some(&ench),
            None,
            None,
            &ctx,
        );
        assert_eq!(result, "an uncursed +1 long sword");
    }

    // -- Test 24: priest never shows uncursed
    #[test]
    fn test_priest_no_uncursed() {
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = NamingContext {
            implicit_uncursed: true,
            is_priest: true,
            ..default_ctx()
        };
        let k = KnowledgeState {
            known: false,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };
        let buc = BucStatus {
            blessed: false,
            cursed: false,
            bknown: true,
        };

        let result = doname(&c, &def, Some(&buc), Some(&k), None, None, None, &ctx);
        // Priest + implicit_uncursed -> no "uncursed"
        assert_eq!(result, "a long sword");
    }

    // ── CJK doname tests ─────────────────────────────────────────

    fn setup_cjk_locale() -> crate::locale::LocaleManager {
        use crate::locale::LanguageManifest;
        let mut locale = crate::locale::LocaleManager::new();
        let manifest = LanguageManifest {
            code: "zh-CN".to_string(),
            name: "\u{7b80}\u{4f53}\u{4e2d}\u{6587}".to_string(),
            name_en: "Simplified Chinese".to_string(),
            author: "test".to_string(),
            version: "0.1.0".to_string(),
            fallback: Some("en".to_string()),
            is_cjk: true,
            has_articles: false,
            has_plural: false,
            has_verb_conj: false,
            has_gender: false,
            possessive: Some("\u{7684}".to_string()),
            quote_left: Some("\u{300c}".to_string()),
            quote_right: Some("\u{300d}".to_string()),
        };
        locale.load_locale("zh-CN", manifest, &[]).unwrap();
        // Load object translations.
        let monsters_toml = "[translations]\n";
        let objects_toml = concat!(
            "[translations]\n",
            "\"long sword\" = \"\u{957f}\u{5251}\"\n",
            "\"dagger\" = \"\u{5315}\u{9996}\"\n",
        );
        locale
            .load_entity_translations("zh-CN", monsters_toml, objects_toml)
            .unwrap();
        // Load classifier.
        let clf_toml = "default = \"\u{4e2a}\"\n\n[class]\nweapon = \"\u{628a}\"\n\n[name]\n";
        locale.load_classifier(clf_toml).unwrap();
        locale.set_language("zh-CN").unwrap();
        locale
    }

    #[test]
    fn test_doname_cjk() {
        let locale = setup_cjk_locale();
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();

        let result = doname_locale(&c, &def, None, None, None, None, None, &ctx, Some(&locale));
        assert_eq!(result, "\u{957f}\u{5251}"); // 长剑 (no article in CJK)
    }

    #[test]
    fn test_doname_cjk_buc() {
        let locale = setup_cjk_locale();
        let c = core(1);
        let def = test_object_def("long sword", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();
        let buc = BucStatus {
            blessed: true,
            cursed: false,
            bknown: true,
        };
        let ench = Enchantment { spe: 2 };
        let k = KnowledgeState {
            known: true,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        };

        let result = doname_locale(
            &c,
            &def,
            Some(&buc),
            Some(&k),
            Some(&ench),
            None,
            None,
            &ctx,
            Some(&locale),
        );
        // 祝福的+2长剑
        assert_eq!(result, "\u{795d}\u{798f}\u{7684}+2\u{957f}\u{5251}");
    }

    #[test]
    fn test_doname_cjk_quantity() {
        let locale = setup_cjk_locale();
        let c = core(3);
        let def = test_object_def("dagger", ObjectClass::Weapon, Material::Iron);
        let ctx = default_ctx();

        let result = doname_locale(&c, &def, None, None, None, None, None, &ctx, Some(&locale));
        // 3把匕首
        assert_eq!(result, "3\u{628a}\u{5315}\u{9996}");
    }

    // ======================================================================
    // O.2 — Comprehensive doname() insta snapshot tests
    // ======================================================================

    /// BUC state configuration for snapshot tests.
    struct BucConfig {
        label: &'static str,
        buc: Option<BucStatus>,
    }

    fn buc_configs() -> Vec<BucConfig> {
        vec![
            BucConfig {
                label: "blessed",
                buc: Some(BucStatus {
                    blessed: true,
                    cursed: false,
                    bknown: true,
                }),
            },
            BucConfig {
                label: "uncursed",
                buc: Some(BucStatus {
                    blessed: false,
                    cursed: false,
                    bknown: true,
                }),
            },
            BucConfig {
                label: "cursed",
                buc: Some(BucStatus {
                    blessed: false,
                    cursed: true,
                    bknown: true,
                }),
            },
            BucConfig {
                label: "buc_unknown",
                buc: Some(BucStatus {
                    blessed: false,
                    cursed: false,
                    bknown: false,
                }),
            },
        ]
    }

    fn knowledge_known() -> KnowledgeState {
        KnowledgeState {
            known: true,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        }
    }

    fn knowledge_unknown() -> KnowledgeState {
        KnowledgeState {
            known: false,
            dknown: true,
            rknown: false,
            cknown: false,
            lknown: false,
            tknown: false,
        }
    }

    // -- Weapons snapshot --

    #[test]
    fn test_doname_snapshot_weapons() {
        use std::fmt::Write;
        let mut output = String::new();

        let weapons = [
            ("long sword", Material::Iron),
            ("dagger", Material::Iron),
            ("arrow", Material::Iron),
            ("elven broadsword", Material::Wood),
        ];

        for (name, material) in &weapons {
            writeln!(output, "=== {} ===", name).unwrap();

            for quantity in [1, 3] {
                let c = core(quantity);

                for bc in buc_configs() {
                    for ench in [None, Some(-1i8), Some(0), Some(2), Some(5)] {
                        let ench_comp = ench.map(|e| Enchantment { spe: e });
                        let k = if ench.is_some() {
                            Some(knowledge_known())
                        } else {
                            Some(knowledge_unknown())
                        };
                        let def = test_object_def(name, ObjectClass::Weapon, *material);
                        let ctx = default_ctx();

                        let result = doname(
                            &c,
                            &def,
                            bc.buc.as_ref(),
                            k.as_ref(),
                            ench_comp.as_ref(),
                            None,
                            None,
                            &ctx,
                        );
                        writeln!(
                            output,
                            "qty={} buc={} ench={:?} -> {}",
                            quantity, bc.label, ench, result
                        )
                        .unwrap();
                    }
                }

                // Erosion variants (only for iron).
                if *material == Material::Iron {
                    let def = test_object_def(name, ObjectClass::Weapon, *material);
                    let ctx = default_ctx();
                    let k = knowledge_known();

                    for (eroded, eroded2, label) in [
                        (1, 0, "rusty"),
                        (2, 0, "very_rusty"),
                        (0, 1, "corroded"),
                        (2, 1, "very_rusty+corroded"),
                    ] {
                        let erosion = Erosion {
                            eroded,
                            eroded2,
                            erodeproof: false,
                            greased: false,
                        };
                        let ench = Enchantment { spe: 0 };
                        let result = doname(
                            &c,
                            &def,
                            None,
                            Some(&k),
                            Some(&ench),
                            Some(&erosion),
                            None,
                            &ctx,
                        );
                        writeln!(output, "qty={} erosion={} -> {}", quantity, label, result)
                            .unwrap();
                    }
                }

                // Named variant.
                {
                    let def = test_object_def(name, ObjectClass::Weapon, *material);
                    let ctx = default_ctx();
                    let extra = ObjectExtra {
                        name: Some("Sting".to_string()),
                        contained_monster: None,
                    };
                    let result = doname(&c, &def, None, None, None, None, Some(&extra), &ctx);
                    writeln!(output, "qty={} named=Sting -> {}", quantity, result).unwrap();
                }

                // "called" variant.
                {
                    let def = test_object_def(name, ObjectClass::Weapon, *material);
                    let ctx = NamingContext {
                        type_known: false,
                        type_called: Some("stabby".to_string()),
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} called=stabby -> {}", quantity, result).unwrap();
                }
            }
        }

        insta::assert_snapshot!("weapons", output);
    }

    // -- Armor snapshot --

    #[test]
    fn test_doname_snapshot_armor() {
        use std::fmt::Write;
        let mut output = String::new();

        let armors = [
            ("plate mail", Material::Iron),
            ("leather armor", Material::Leather),
            ("elven cloak", Material::Cloth),
        ];

        for (name, material) in &armors {
            writeln!(output, "=== {} ===", name).unwrap();
            let mut def = test_object_def(name, ObjectClass::Armor, *material);
            def.armor = Some(nethack_babel_data::ArmorInfo {
                category: ArmorCategory::Suit,
                ac_bonus: 5,
                magic_cancel: 0,
            });

            for quantity in [1, 3] {
                let c = core(quantity);

                for bc in buc_configs() {
                    for ench in [None, Some(0i8), Some(2), Some(5)] {
                        let ench_comp = ench.map(|e| Enchantment { spe: e });
                        let k = if ench.is_some() {
                            Some(knowledge_known())
                        } else {
                            Some(knowledge_unknown())
                        };
                        let ctx = default_ctx();

                        let result = doname(
                            &c,
                            &def,
                            bc.buc.as_ref(),
                            k.as_ref(),
                            ench_comp.as_ref(),
                            None,
                            None,
                            &ctx,
                        );
                        writeln!(
                            output,
                            "qty={} buc={} ench={:?} -> {}",
                            quantity, bc.label, ench, result
                        )
                        .unwrap();
                    }
                }
            }
        }

        insta::assert_snapshot!("armor", output);
    }

    // -- Potions snapshot --

    #[test]
    fn test_doname_snapshot_potions() {
        use std::fmt::Write;
        let mut output = String::new();

        let potions = [
            ("healing", Some("milky")),
            ("extra healing", Some("pink")),
            ("speed", Some("amber")),
        ];

        for (name, appearance) in &potions {
            writeln!(output, "=== potion of {} ===", name).unwrap();
            let mut def = test_object_def(name, ObjectClass::Potion, Material::Glass);
            def.appearance = appearance.map(|s| s.to_string());
            def.is_charged = false;

            for quantity in [1, 3] {
                let c = core(quantity);

                // Identified.
                for bc in buc_configs() {
                    let ctx = NamingContext {
                        type_known: true,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, bc.buc.as_ref(), None, None, None, None, &ctx);
                    writeln!(
                        output,
                        "qty={} identified buc={} -> {}",
                        quantity, bc.label, result
                    )
                    .unwrap();
                }

                // Unidentified (shows appearance).
                {
                    let ctx = NamingContext {
                        type_known: false,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} unidentified -> {}", quantity, result).unwrap();
                }

                // Called.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        type_called: Some("heal?".to_string()),
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} called=heal? -> {}", quantity, result).unwrap();
                }
            }
        }

        insta::assert_snapshot!("potions", output);
    }

    // -- Scrolls snapshot --

    #[test]
    fn test_doname_snapshot_scrolls() {
        use std::fmt::Write;
        let mut output = String::new();

        let scrolls = [
            ("identify", Some("ZELGO MER"), true),
            ("enchant weapon", Some("DAIYEN FANSEN"), true),
        ];

        for (name, appearance, is_magic) in &scrolls {
            writeln!(output, "=== scroll of {} ===", name).unwrap();
            let mut def = test_object_def(name, ObjectClass::Scroll, Material::Paper);
            def.appearance = appearance.map(|s| s.to_string());
            def.is_magic = *is_magic;
            def.is_charged = false;

            for quantity in [1, 3] {
                let c = core(quantity);

                // Identified.
                for bc in buc_configs() {
                    let ctx = NamingContext {
                        type_known: true,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, bc.buc.as_ref(), None, None, None, None, &ctx);
                    writeln!(
                        output,
                        "qty={} identified buc={} -> {}",
                        quantity, bc.label, result
                    )
                    .unwrap();
                }

                // Unidentified.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} unidentified -> {}", quantity, result).unwrap();
                }

                // Called.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        type_called: Some("id?".to_string()),
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} called=id? -> {}", quantity, result).unwrap();
                }
            }
        }

        insta::assert_snapshot!("scrolls", output);
    }

    // -- Wands snapshot --

    #[test]
    fn test_doname_snapshot_wands() {
        use std::fmt::Write;
        let mut output = String::new();

        let wands = [("fire", Some("oak")), ("death", Some("ebony"))];

        for (name, appearance) in &wands {
            writeln!(output, "=== wand of {} ===", name).unwrap();
            let mut def = test_object_def(name, ObjectClass::Wand, Material::Wood);
            def.appearance = appearance.map(|s| s.to_string());
            def.is_charged = true;

            for quantity in [1, 3] {
                let c = core(quantity);

                // Identified.
                for bc in buc_configs() {
                    let ctx = NamingContext {
                        type_known: true,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, bc.buc.as_ref(), None, None, None, None, &ctx);
                    writeln!(
                        output,
                        "qty={} identified buc={} -> {}",
                        quantity, bc.label, result
                    )
                    .unwrap();
                }

                // Unidentified.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} unidentified -> {}", quantity, result).unwrap();
                }

                // Called.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        type_called: Some("zap!".to_string()),
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} called=zap! -> {}", quantity, result).unwrap();
                }
            }
        }

        insta::assert_snapshot!("wands", output);
    }

    // -- Rings snapshot --

    #[test]
    fn test_doname_snapshot_rings() {
        use std::fmt::Write;
        let mut output = String::new();

        let rings = [
            ("teleportation", Some("jade")),
            ("protection", Some("iron")),
        ];

        for (name, appearance) in &rings {
            writeln!(output, "=== ring of {} ===", name).unwrap();
            let mut def = test_object_def(name, ObjectClass::Ring, Material::Gemstone);
            def.appearance = appearance.map(|s| s.to_string());
            def.is_charged = true;

            for quantity in [1, 3] {
                let c = core(quantity);

                // Identified + various BUC + enchantments.
                for bc in buc_configs() {
                    for ench in [None, Some(0i8), Some(2)] {
                        let ench_comp = ench.map(|e| Enchantment { spe: e });
                        let k = if ench.is_some() {
                            Some(knowledge_known())
                        } else {
                            Some(knowledge_unknown())
                        };
                        let ctx = NamingContext {
                            type_known: true,
                            ..default_ctx()
                        };
                        let result = doname(
                            &c,
                            &def,
                            bc.buc.as_ref(),
                            k.as_ref(),
                            ench_comp.as_ref(),
                            None,
                            None,
                            &ctx,
                        );
                        writeln!(
                            output,
                            "qty={} identified buc={} ench={:?} -> {}",
                            quantity, bc.label, ench, result
                        )
                        .unwrap();
                    }
                }

                // Unidentified.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} unidentified -> {}", quantity, result).unwrap();
                }
            }
        }

        insta::assert_snapshot!("rings", output);
    }

    // -- Amulets snapshot --

    #[test]
    fn test_doname_snapshot_amulets() {
        use std::fmt::Write;
        let mut output = String::new();

        let amulets = [
            ("amulet of life saving", Some("circular")),
            ("amulet of reflection", Some("triangular")),
        ];

        for (name, appearance) in &amulets {
            writeln!(output, "=== {} ===", name).unwrap();
            let mut def = test_object_def(name, ObjectClass::Amulet, Material::Metal);
            def.appearance = appearance.map(|s| s.to_string());
            def.is_charged = false;

            for quantity in [1, 3] {
                let c = core(quantity);

                // Identified.
                for bc in buc_configs() {
                    let ctx = NamingContext {
                        type_known: true,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, bc.buc.as_ref(), None, None, None, None, &ctx);
                    writeln!(
                        output,
                        "qty={} identified buc={} -> {}",
                        quantity, bc.label, result
                    )
                    .unwrap();
                }

                // Unidentified.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} unidentified -> {}", quantity, result).unwrap();
                }

                // Called.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        type_called: Some("lifesave?".to_string()),
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} called=lifesave? -> {}", quantity, result).unwrap();
                }
            }
        }

        insta::assert_snapshot!("amulets", output);
    }

    // -- Food snapshot --

    #[test]
    fn test_doname_snapshot_food() {
        use std::fmt::Write;
        let mut output = String::new();

        let foods = ["food ration", "apple", "tripe ration"];

        for name in &foods {
            writeln!(output, "=== {} ===", name).unwrap();
            let def = test_object_def(name, ObjectClass::Food, Material::Flesh);

            for quantity in [1, 3] {
                let c = core(quantity);

                for bc in buc_configs() {
                    let ctx = default_ctx();
                    let result = doname(&c, &def, bc.buc.as_ref(), None, None, None, None, &ctx);
                    writeln!(output, "qty={} buc={} -> {}", quantity, bc.label, result).unwrap();
                }
            }
        }

        insta::assert_snapshot!("food", output);
    }

    // -- Tools snapshot --

    #[test]
    fn test_doname_snapshot_tools() {
        use std::fmt::Write;
        let mut output = String::new();

        let tools = [
            ("skeleton key", Material::Bone),
            ("tinning kit", Material::Iron),
            ("lamp", Material::Copper),
        ];

        for (name, material) in &tools {
            writeln!(output, "=== {} ===", name).unwrap();
            let def = test_object_def(name, ObjectClass::Tool, *material);

            for quantity in [1, 3] {
                let c = core(quantity);

                for bc in buc_configs() {
                    let ctx = default_ctx();
                    let result = doname(&c, &def, bc.buc.as_ref(), None, None, None, None, &ctx);
                    writeln!(output, "qty={} buc={} -> {}", quantity, bc.label, result).unwrap();
                }

                // Named tool.
                {
                    let ctx = default_ctx();
                    let extra = ObjectExtra {
                        name: Some("MyKey".to_string()),
                        contained_monster: None,
                    };
                    let result = doname(&c, &def, None, None, None, None, Some(&extra), &ctx);
                    writeln!(output, "qty={} named=MyKey -> {}", quantity, result).unwrap();
                }
            }
        }

        insta::assert_snapshot!("tools", output);
    }

    // -- Gems snapshot --

    #[test]
    fn test_doname_snapshot_gems() {
        use std::fmt::Write;
        let mut output = String::new();

        let gems = [
            ("emerald", Material::Gemstone, Some("green")),
            ("diamond", Material::Gemstone, Some("white")),
            ("flint", Material::Mineral, Some("gray")),
        ];

        for (name, material, appearance) in &gems {
            writeln!(output, "=== {} ===", name).unwrap();
            let mut def = test_object_def(name, ObjectClass::Gem, *material);
            def.appearance = appearance.map(|s| s.to_string());

            for quantity in [1, 3] {
                let c = core(quantity);

                // Identified.
                for bc in buc_configs() {
                    let ctx = NamingContext {
                        type_known: true,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, bc.buc.as_ref(), None, None, None, None, &ctx);
                    writeln!(
                        output,
                        "qty={} identified buc={} -> {}",
                        quantity, bc.label, result
                    )
                    .unwrap();
                }

                // Unidentified.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} unidentified -> {}", quantity, result).unwrap();
                }

                // Called.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        type_called: Some("valuable?".to_string()),
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} called=valuable? -> {}", quantity, result).unwrap();
                }
            }
        }

        insta::assert_snapshot!("gems", output);
    }

    // -- Spellbooks snapshot --

    #[test]
    fn test_doname_snapshot_spellbooks() {
        use std::fmt::Write;
        let mut output = String::new();

        let spellbooks = [
            ("force bolt", Some("vellum")),
            ("healing", Some("parchment")),
        ];

        for (name, appearance) in &spellbooks {
            writeln!(output, "=== spellbook of {} ===", name).unwrap();
            let mut def = test_object_def(name, ObjectClass::Spellbook, Material::Paper);
            def.appearance = appearance.map(|s| s.to_string());
            def.is_charged = false;

            for quantity in [1, 3] {
                let c = core(quantity);

                // Identified.
                for bc in buc_configs() {
                    let ctx = NamingContext {
                        type_known: true,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, bc.buc.as_ref(), None, None, None, None, &ctx);
                    writeln!(
                        output,
                        "qty={} identified buc={} -> {}",
                        quantity, bc.label, result
                    )
                    .unwrap();
                }

                // Unidentified.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} unidentified -> {}", quantity, result).unwrap();
                }

                // Called.
                {
                    let ctx = NamingContext {
                        type_known: false,
                        type_called: Some("attack?".to_string()),
                        ..default_ctx()
                    };
                    let result = doname(&c, &def, None, None, None, None, None, &ctx);
                    writeln!(output, "qty={} called=attack? -> {}", quantity, result).unwrap();
                }
            }
        }

        insta::assert_snapshot!("spellbooks", output);
    }

    // -- Coins snapshot --

    #[test]
    fn test_doname_snapshot_coins() {
        use std::fmt::Write;
        let mut output = String::new();

        let def = test_object_def("gold piece", ObjectClass::Coin, Material::Gold);

        for quantity in [1, 3, 100] {
            let c = core(quantity);
            let ctx = default_ctx();
            let result = doname(&c, &def, None, None, None, None, None, &ctx);
            writeln!(output, "qty={} -> {}", quantity, result).unwrap();
        }

        insta::assert_snapshot!("coins", output);
    }

    // ======================================================================
    // O.3 — CJK classifier leak guard test
    // ======================================================================

    /// Verify that NO Chinese classifier characters leak into English locale output.
    /// This is a critical regression guard: CJK classifiers like 把/瓶/张/枚/件/支/个/本/颗/块
    /// must NEVER appear when rendering in English.
    #[test]
    fn test_doname_en_us_no_classifier_leak() {
        let cjk_classifiers = [
            '\u{628a}', // 把
            '\u{74f6}', // 瓶
            '\u{5f20}', // 张
            '\u{679a}', // 枚
            '\u{4ef6}', // 件
            '\u{652f}', // 支
            '\u{4e2a}', // 个
            '\u{672c}', // 本
            '\u{9897}', // 颗
            '\u{5757}', // 块
        ];

        // Items across multiple classes, all with quantity > 1 to exercise
        // the code path where classifiers could leak.
        let test_items: Vec<(
            &str,
            ObjectClass,
            Material,
            bool,         // type_known
            Option<&str>, // appearance
        )> = vec![
            ("dagger", ObjectClass::Weapon, Material::Iron, true, None),
            ("arrow", ObjectClass::Weapon, Material::Iron, true, None),
            ("plate mail", ObjectClass::Armor, Material::Iron, true, None),
            (
                "healing",
                ObjectClass::Potion,
                Material::Glass,
                true,
                Some("milky"),
            ),
            (
                "identify",
                ObjectClass::Scroll,
                Material::Paper,
                true,
                Some("ZELGO MER"),
            ),
            ("fire", ObjectClass::Wand, Material::Wood, true, Some("oak")),
            (
                "teleportation",
                ObjectClass::Ring,
                Material::Gemstone,
                true,
                Some("jade"),
            ),
            (
                "food ration",
                ObjectClass::Food,
                Material::Flesh,
                true,
                None,
            ),
            (
                "skeleton key",
                ObjectClass::Tool,
                Material::Bone,
                true,
                None,
            ),
            (
                "emerald",
                ObjectClass::Gem,
                Material::Gemstone,
                true,
                Some("green"),
            ),
            (
                "force bolt",
                ObjectClass::Spellbook,
                Material::Paper,
                true,
                Some("vellum"),
            ),
            ("gold piece", ObjectClass::Coin, Material::Gold, true, None),
        ];

        for (name, class, material, type_known, appearance) in &test_items {
            let c = core(3);
            let mut def = test_object_def(name, *class, *material);
            if let Some(app) = appearance {
                def.appearance = Some(app.to_string());
            }
            if *class == ObjectClass::Potion
                || *class == ObjectClass::Scroll
                || *class == ObjectClass::Spellbook
            {
                def.is_charged = false;
            }
            let ctx = NamingContext {
                type_known: *type_known,
                ..default_ctx()
            };

            // English locale (default, no locale parameter).
            let result = doname(&c, &def, None, None, None, None, None, &ctx);

            // Must not contain ANY CJK classifier characters.
            for clf in &cjk_classifiers {
                assert!(
                    !result.contains(*clf),
                    "CJK classifier '{}' (U+{:04X}) leaked into English output for {}: \"{}\"",
                    clf,
                    *clf as u32,
                    name,
                    result
                );
            }

            // English quantity format: must start with "3 " (space after digit).
            assert!(
                result.starts_with("3 "),
                "English quantity format wrong for {}: expected '3 ...' but got \"{}\"",
                name,
                result
            );
        }
    }

    /// Additional CJK leak guard: test that doname_locale with locale=None
    /// produces the same output as doname (both English), and neither leaks.
    #[test]
    fn test_doname_locale_none_matches_doname() {
        let items = [
            ("long sword", ObjectClass::Weapon, Material::Iron),
            ("dagger", ObjectClass::Weapon, Material::Iron),
            ("healing", ObjectClass::Potion, Material::Glass),
        ];

        for (name, class, material) in &items {
            for qty in [1, 3] {
                let c = core(qty);
                let mut def = test_object_def(name, *class, *material);
                if *class == ObjectClass::Potion {
                    def.is_charged = false;
                }
                let ctx = default_ctx();

                let via_doname = doname(&c, &def, None, None, None, None, None, &ctx);
                let via_locale = doname_locale(&c, &def, None, None, None, None, None, &ctx, None);

                assert_eq!(
                    via_doname, via_locale,
                    "doname vs doname_locale(None) mismatch for {} qty={}",
                    name, qty
                );
            }
        }
    }
}
