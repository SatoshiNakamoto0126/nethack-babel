//! Touchstone i18n tests: verify that English locale output never leaks
//! CJK characters into item names or engine event messages.
//!
//! Scenario 10: i18n No Leak

use nethack_babel_engine::action::Position;
use nethack_babel_engine::event::{DeathCause, EngineEvent, StatusEffect};
use nethack_babel_engine::world::{GameWorld, Name};
use nethack_babel_data::{
    Material, ObjectClass, ObjectCore, ObjectDef, ObjectTypeId,
};
use nethack_babel_i18n::item_names::{doname, NamingContext};
use nethack_babel_i18n::locale::LocaleManager;
use nethack_babel_i18n::MessageComposer;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns true if any character in `s` is in the CJK Unified Ideographs
/// block (U+4E00..U+9FFF) or CJK extensions / compatibility ranges.
fn contains_cjk(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(c,
            '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
            | '\u{3400}'..='\u{4DBF}' // CJK Extension A
            | '\u{F900}'..='\u{FAFF}' // CJK Compatibility Ideographs
            | '\u{2E80}'..='\u{2EFF}' // CJK Radicals Supplement
            | '\u{3000}'..='\u{303F}' // CJK Symbols and Punctuation
            | '\u{31C0}'..='\u{31EF}' // CJK Strokes
            | '\u{3200}'..='\u{32FF}' // Enclosed CJK Letters and Months
            | '\u{FE30}'..='\u{FE4F}' // CJK Compatibility Forms
        )
    })
}

fn test_object_def(name: &str, class: ObjectClass, material: Material) -> ObjectDef {
    ObjectDef {
        id: ObjectTypeId(0),
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
        is_charged: true,
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

// ==========================================================================
// Touchstone 10.1 -- doname() in English locale never contains CJK chars
// ==========================================================================

/// Call doname() for a wide variety of items in the default English locale
/// and verify that no CJK characters appear in any output string.
#[test]
fn touchstone_10_doname_en_us_no_cjk() {
    let test_items: Vec<(&str, ObjectClass, Material)> = vec![
        // Weapons
        ("long sword", ObjectClass::Weapon, Material::Iron),
        ("dagger", ObjectClass::Weapon, Material::Iron),
        ("spear", ObjectClass::Weapon, Material::Iron),
        ("mace", ObjectClass::Weapon, Material::Iron),
        ("battle-axe", ObjectClass::Weapon, Material::Iron),
        ("katana", ObjectClass::Weapon, Material::Iron),
        ("arrow", ObjectClass::Weapon, Material::Iron),
        // Armor
        ("plate mail", ObjectClass::Armor, Material::Iron),
        ("leather armor", ObjectClass::Armor, Material::Leather),
        ("shield of reflection", ObjectClass::Armor, Material::Silver),
        // Food
        ("food ration", ObjectClass::Food, Material::Flesh),
        ("apple", ObjectClass::Food, Material::Veggy),
        ("tripe ration", ObjectClass::Food, Material::Flesh),
        // Tools
        ("skeleton key", ObjectClass::Tool, Material::Bone),
        ("tinning kit", ObjectClass::Tool, Material::Iron),
        ("lamp", ObjectClass::Tool, Material::Copper),
        // Coins
        ("gold piece", ObjectClass::Coin, Material::Gold),
        // Rocks
        ("boulder", ObjectClass::Rock, Material::Mineral),
    ];

    for (name, class, material) in &test_items {
        for qty in [1, 3, 10] {
            let c = core(qty);
            let def = test_object_def(name, *class, *material);
            let ctx = default_ctx();

            let result = doname(&c, &def, None, None, None, None, None, &ctx);

            assert!(
                !contains_cjk(&result),
                "CJK characters leaked into English doname() for '{}' qty={}: \"{}\"",
                name, qty, result
            );
        }
    }

    // Also test unidentified potions, scrolls, wands, rings (with appearances)
    let magic_items: Vec<(&str, ObjectClass, Material, &str)> = vec![
        ("healing", ObjectClass::Potion, Material::Glass, "milky"),
        ("identify", ObjectClass::Scroll, Material::Paper, "ZELGO MER"),
        ("fire", ObjectClass::Wand, Material::Wood, "oak"),
        ("teleportation", ObjectClass::Ring, Material::Gemstone, "jade"),
        ("force bolt", ObjectClass::Spellbook, Material::Paper, "vellum"),
        ("emerald", ObjectClass::Gem, Material::Gemstone, "green"),
    ];

    for (name, class, material, appearance) in &magic_items {
        for qty in [1, 2] {
            let c = core(qty);
            let mut def = test_object_def(name, *class, *material);
            def.appearance = Some(appearance.to_string());
            if *class == ObjectClass::Potion
                || *class == ObjectClass::Scroll
                || *class == ObjectClass::Spellbook
            {
                def.is_charged = false;
            }

            // Test identified
            let ctx = NamingContext {
                type_known: true,
                ..default_ctx()
            };
            let result = doname(&c, &def, None, None, None, None, None, &ctx);
            assert!(
                !contains_cjk(&result),
                "CJK leaked (identified) for '{}' qty={}: \"{}\"",
                name, qty, result
            );

            // Test unidentified
            let ctx2 = NamingContext {
                type_known: false,
                ..default_ctx()
            };
            let result2 = doname(&c, &def, None, None, None, None, None, &ctx2);
            assert!(
                !contains_cjk(&result2),
                "CJK leaked (unidentified) for '{}' qty={}: \"{}\"",
                name, qty, result2
            );
        }
    }
}

// ==========================================================================
// Touchstone 10.2 -- Engine events composed to English messages have no CJK
// ==========================================================================

/// Create various engine events, compose them via MessageComposer with the
/// default English locale, and verify no CJK characters appear.
#[test]
fn touchstone_10_message_no_cjk() {
    let locale = LocaleManager::new();
    let composer = MessageComposer::new(&locale);
    let mut world = GameWorld::new(Position::new(5, 5));

    // Spawn some named entities.
    let goblin = world.spawn((Name("goblin".to_string()),));
    let dragon = world.spawn((Name("red dragon".to_string()),));

    let events: Vec<EngineEvent> = vec![
        // Melee combat
        EngineEvent::MeleeHit {
            attacker: world.player(),
            defender: goblin,
            weapon: None,
            damage: 5,
        },
        EngineEvent::MeleeMiss {
            attacker: goblin,
            defender: world.player(),
        },
        EngineEvent::MeleeHit {
            attacker: world.player(),
            defender: dragon,
            weapon: None,
            damage: 12,
        },
        // Entity death
        EngineEvent::EntityDied {
            entity: goblin,
            killer: Some(world.player()),
            cause: DeathCause::KilledBy {
                killer_name: "a long sword".to_string(),
            },
        },
        // Item events
        EngineEvent::ItemPickedUp {
            actor: world.player(),
            item: goblin, // re-using entity for simplicity
            quantity: 1,
        },
        // HP change
        EngineEvent::HpChange {
            entity: world.player(),
            amount: -3,
            new_hp: 13,
            source: nethack_babel_engine::event::HpSource::Combat,
        },
        // Level up
        EngineEvent::LevelUp {
            entity: world.player(),
            new_level: 5,
        },
        // Status effects
        EngineEvent::StatusApplied {
            entity: world.player(),
            status: StatusEffect::Blind,
            duration: Some(10),
            source: None,
        },
        // Message events (raw Fluent key)
        EngineEvent::msg("wand-nothing"),
        EngineEvent::msg("wand-light"),
        EngineEvent::msg("wand-wrested"),
    ];

    let messages = composer.compose_all(&events, &world);

    assert!(
        !messages.is_empty(),
        "At least some events should produce messages"
    );

    for msg in &messages {
        assert!(
            !contains_cjk(msg),
            "CJK characters leaked into English message: \"{}\"",
            msg
        );
    }
}

// ==========================================================================
// Touchstone 10.3 -- doname() format matches expected English output
// ==========================================================================

/// Verify that doname() produces the expected English strings for specific
/// items.  This catches regressions in the naming pipeline.
#[test]
fn touchstone_10_doname_format_matches_original() {
    let cases: Vec<(&str, ObjectClass, Material, i32, &str)> = vec![
        // (item_name, class, material, quantity, expected_output)
        ("long sword", ObjectClass::Weapon, Material::Iron, 1, "a long sword"),
        ("dagger", ObjectClass::Weapon, Material::Iron, 3, "3 daggers"),
        ("food ration", ObjectClass::Food, Material::Flesh, 1, "a food ration"),
        ("gold piece", ObjectClass::Coin, Material::Gold, 1, "a gold piece"),
        ("gold piece", ObjectClass::Coin, Material::Gold, 42, "42 gold pieces"),
    ];

    for (name, class, material, qty, expected) in &cases {
        let c = core(*qty);
        let def = test_object_def(name, *class, *material);
        let ctx = default_ctx();

        let result = doname(&c, &def, None, None, None, None, None, &ctx);
        assert_eq!(
            &result, expected,
            "doname() for '{}' qty={} should be \"{}\" but got \"{}\"",
            name, qty, expected, result
        );
    }
}
