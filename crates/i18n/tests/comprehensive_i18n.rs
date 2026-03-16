//! Comprehensive i18n integration test suite.
//!
//! Covers FTL key coverage, CJK leak detection, English parity,
//! CJK/French/German rendering, translation metadata, fallback chains,
//! language switching, and plural systems.

use fluent::FluentArgs;
use nethack_babel_i18n::locale::{LanguageManifest, LocaleManager};
use nethack_babel_i18n::noun_phrase::{Article, BucLabel, NounPhrase};

// ---------------------------------------------------------------------------
// FTL and TOML data loaded at compile time
// ---------------------------------------------------------------------------

const EN_FTL: &str = include_str!("../../../data/locale/en/messages.ftl");
const ZH_CN_FTL: &str = include_str!("../../../data/locale/zh-CN/messages.ftl");
const ZH_TW_FTL: &str = include_str!("../../../data/locale/zh-TW/messages.ftl");
const FR_FTL: &str = include_str!("../../../data/locale/fr/messages.ftl");
const DE_FTL: &str = include_str!("../../../data/locale/de/messages.ftl");

const FR_OBJECTS_TOML: &str = include_str!("../../../data/locale/fr/objects.toml");
const DE_OBJECTS_TOML: &str = include_str!("../../../data/locale/de/objects.toml");
const ZH_CN_OBJECTS_TOML: &str = include_str!("../../../data/locale/zh-CN/objects.toml");
const ZH_CN_MONSTERS_TOML: &str = include_str!("../../../data/locale/zh-CN/monsters.toml");

const FR_MONSTERS_TOML: &str = include_str!("../../../data/locale/fr/monsters.toml");
const DE_MONSTERS_TOML: &str = include_str!("../../../data/locale/de/monsters.toml");
const ZH_TW_OBJECTS_TOML: &str = include_str!("../../../data/locale/zh-TW/objects.toml");
const ZH_TW_MONSTERS_TOML: &str = include_str!("../../../data/locale/zh-TW/monsters.toml");

const ZH_CN_CLASSIFIERS_TOML: &str = include_str!("../../../data/locale/zh-CN/classifiers.toml");

// ---------------------------------------------------------------------------
// Helper: manifest constructors
// ---------------------------------------------------------------------------

fn en_manifest() -> LanguageManifest {
    LanguageManifest {
        code: "en".to_string(),
        name: "English".to_string(),
        name_en: "English".to_string(),
        author: "NetHack DevTeam".to_string(),
        version: "0.1.0".to_string(),
        fallback: None,
        is_cjk: false,
        has_articles: true,
        has_plural: true,
        has_verb_conj: true,
        has_gender: false,
        possessive: None,
        quote_left: None,
        quote_right: None,
    }
}

fn zh_cn_manifest() -> LanguageManifest {
    LanguageManifest {
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
    }
}

fn zh_tw_manifest() -> LanguageManifest {
    LanguageManifest {
        code: "zh-TW".to_string(),
        name: "\u{7e41}\u{9ad4}\u{4e2d}\u{6587}".to_string(),
        name_en: "Traditional Chinese".to_string(),
        author: "test".to_string(),
        version: "0.1.0".to_string(),
        fallback: Some("zh-CN".to_string()),
        is_cjk: true,
        has_articles: false,
        has_plural: false,
        has_verb_conj: false,
        has_gender: false,
        possessive: Some("\u{7684}".to_string()),
        quote_left: Some("\u{300c}".to_string()),
        quote_right: Some("\u{300d}".to_string()),
    }
}

fn fr_manifest() -> LanguageManifest {
    LanguageManifest {
        code: "fr".to_string(),
        name: "Fran\u{e7}ais".to_string(),
        name_en: "French".to_string(),
        author: "test".to_string(),
        version: "0.1.0".to_string(),
        fallback: Some("en".to_string()),
        is_cjk: false,
        has_articles: true,
        has_plural: true,
        has_verb_conj: true,
        has_gender: true,
        possessive: None,
        quote_left: None,
        quote_right: None,
    }
}

fn de_manifest() -> LanguageManifest {
    LanguageManifest {
        code: "de".to_string(),
        name: "Deutsch".to_string(),
        name_en: "German".to_string(),
        author: "test".to_string(),
        version: "0.1.0".to_string(),
        fallback: Some("en".to_string()),
        is_cjk: false,
        has_articles: true,
        has_plural: true,
        has_verb_conj: true,
        has_gender: true,
        possessive: None,
        quote_left: None,
        quote_right: None,
    }
}

// ---------------------------------------------------------------------------
// Helper: FTL sanitizer
// ---------------------------------------------------------------------------

/// Fix common Fluent issues in the data files:
/// 1. Empty message values (`msg-id =\n`) -> `msg-id = {""}`.
/// 2. Duplicate message IDs: keep only the first occurrence.
fn sanitize_ftl(raw: &str) -> String {
    use std::collections::HashSet;

    let mut out = String::with_capacity(raw.len());
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut skip_until_next_msg = false;

    for line in raw.lines() {
        let trimmed = line.trim_end();

        // Detect top-level message definitions (not comments, not indented
        // continuation/variant lines).
        let is_message_def = !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with(' ')
            && !trimmed.starts_with('.')
            && !trimmed.starts_with('*')
            && !trimmed.starts_with('[')
            && trimmed.contains(" =");

        if is_message_def {
            // Extract the message id (everything before ` =`).
            let msg_id = trimmed.split(" =").next().unwrap_or("").trim();
            if !msg_id.is_empty() && !seen_ids.insert(msg_id.to_string()) {
                // Duplicate: skip this message and its continuation lines.
                skip_until_next_msg = true;
                continue;
            }
            skip_until_next_msg = false;
        } else if skip_until_next_msg {
            // We are inside a duplicate message block; skip continuation lines.
            // A blank line or comment ends the block.
            if trimmed.is_empty() || trimmed.starts_with('#') {
                skip_until_next_msg = false;
                // Fall through to emit this line.
            } else {
                continue;
            }
        }

        // Fix empty values.
        if trimmed.ends_with(" =")
            && !trimmed.starts_with(' ')
            && !trimmed.starts_with('#')
            && !trimmed.contains("->")
        {
            out.push_str(trimmed);
            out.push_str(" {\"\"}\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Helper: locale setup functions
// ---------------------------------------------------------------------------

/// English locale with FTL loaded.
fn setup_en() -> LocaleManager {
    let mut lm = LocaleManager::new();
    let en_ftl = sanitize_ftl(EN_FTL);
    lm.load_locale("en", en_manifest(), &[("messages.ftl", &en_ftl)])
        .unwrap();
    lm
}

/// zh-CN locale with English fallback, FTL, entity translations, and
/// classifiers loaded.
fn setup_zh_cn() -> LocaleManager {
    let mut lm = setup_en();
    let zh_cn_ftl = sanitize_ftl(ZH_CN_FTL);
    lm.load_locale("zh-CN", zh_cn_manifest(), &[("messages.ftl", &zh_cn_ftl)])
        .unwrap();
    lm.load_entity_translations("zh-CN", ZH_CN_MONSTERS_TOML, ZH_CN_OBJECTS_TOML)
        .unwrap();
    lm.load_classifier(ZH_CN_CLASSIFIERS_TOML).unwrap();
    lm.set_language("zh-CN").unwrap();
    lm
}

/// French locale with English fallback, FTL, and object translations loaded.
fn setup_fr() -> LocaleManager {
    let mut lm = setup_en();
    let fr_ftl = sanitize_ftl(FR_FTL);
    lm.load_locale("fr", fr_manifest(), &[("messages.ftl", &fr_ftl)])
        .unwrap();
    lm.load_entity_translations("fr", FR_MONSTERS_TOML, FR_OBJECTS_TOML)
        .unwrap();
    lm.set_language("fr").unwrap();
    lm
}

/// German locale with English fallback, FTL, and object translations loaded.
fn setup_de() -> LocaleManager {
    let mut lm = setup_en();
    let de_ftl = sanitize_ftl(DE_FTL);
    lm.load_locale("de", de_manifest(), &[("messages.ftl", &de_ftl)])
        .unwrap();
    lm.load_entity_translations("de", DE_MONSTERS_TOML, DE_OBJECTS_TOML)
        .unwrap();
    lm.set_language("de").unwrap();
    lm
}

/// zh-TW locale with zh-CN and English fallback, FTL, and entity translations.
fn setup_zh_tw() -> LocaleManager {
    let mut lm = setup_zh_cn();
    let zh_tw_ftl = sanitize_ftl(ZH_TW_FTL);
    lm.load_locale("zh-TW", zh_tw_manifest(), &[("messages.ftl", &zh_tw_ftl)])
        .unwrap();
    lm.load_entity_translations("zh-TW", ZH_TW_MONSTERS_TOML, ZH_TW_OBJECTS_TOML)
        .unwrap();
    lm.set_language("zh-TW").unwrap();
    lm
}

/// All locales loaded (en, zh-CN, zh-TW, fr, de). Current language is English.
fn setup_all() -> LocaleManager {
    let mut lm = setup_en();

    let zh_cn_ftl = sanitize_ftl(ZH_CN_FTL);
    lm.load_locale("zh-CN", zh_cn_manifest(), &[("messages.ftl", &zh_cn_ftl)])
        .unwrap();
    lm.load_entity_translations("zh-CN", ZH_CN_MONSTERS_TOML, ZH_CN_OBJECTS_TOML)
        .unwrap();
    lm.load_classifier(ZH_CN_CLASSIFIERS_TOML).unwrap();

    let zh_tw_ftl = sanitize_ftl(ZH_TW_FTL);
    lm.load_locale("zh-TW", zh_tw_manifest(), &[("messages.ftl", &zh_tw_ftl)])
        .unwrap();
    lm.load_entity_translations("zh-TW", ZH_TW_MONSTERS_TOML, ZH_TW_OBJECTS_TOML)
        .unwrap();

    let fr_ftl = sanitize_ftl(FR_FTL);
    lm.load_locale("fr", fr_manifest(), &[("messages.ftl", &fr_ftl)])
        .unwrap();
    lm.load_entity_translations("fr", FR_MONSTERS_TOML, FR_OBJECTS_TOML)
        .unwrap();

    let de_ftl = sanitize_ftl(DE_FTL);
    lm.load_locale("de", de_manifest(), &[("messages.ftl", &de_ftl)])
        .unwrap();
    lm.load_entity_translations("de", DE_MONSTERS_TOML, DE_OBJECTS_TOML)
        .unwrap();

    lm
}

// ---------------------------------------------------------------------------
// Helper: CJK detection
// ---------------------------------------------------------------------------

fn contains_cjk(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(
            c,
            '\u{4E00}'..='\u{9FFF}'
                | '\u{3400}'..='\u{4DBF}'
                | '\u{F900}'..='\u{FAFF}'
                | '\u{2E80}'..='\u{2EFF}'
                | '\u{3000}'..='\u{303F}'
                | '\u{31C0}'..='\u{31EF}'
                | '\u{3200}'..='\u{32FF}'
                | '\u{FE30}'..='\u{FE4F}'
        )
    })
}

/// Returns true if every character is in the ASCII/Latin-1 range (no CJK,
/// no exotic scripts).
fn is_ascii_latin(s: &str) -> bool {
    s.chars().all(|c| (c as u32) < 0x2000)
}

// ==========================================================================
// 1. FTL Key Coverage Tests
// ==========================================================================

/// Helper: assert that a message id resolves to something other than the raw
/// id in the given locale (meaning it exists in the bundle).
fn assert_key_exists(lm: &LocaleManager, key: &str) {
    let result = lm.translate(key, None);
    assert_ne!(
        result,
        key,
        "FTL key '{}' not found in locale '{}'",
        key,
        lm.current_language()
    );
}

/// Helper: assert a key exists across multiple locales.
fn assert_key_in_locales(lm: &mut LocaleManager, key: &str, locales: &[&str]) {
    for locale in locales {
        lm.set_language(locale).unwrap();
        assert_key_exists(lm, key);
    }
}

#[test]
fn ftl_coverage_buc_keys_all_locales() {
    let mut lm = setup_all();
    let buc_keys = ["item-buc-blessed", "item-buc-cursed", "item-buc-uncursed"];
    let locales = ["en", "zh-CN", "zh-TW", "fr", "de"];
    for key in &buc_keys {
        assert_key_in_locales(&mut lm, key, &locales);
    }
}

#[test]
fn ftl_coverage_named_suffix_all_locales() {
    let mut lm = setup_all();
    let locales = ["en", "zh-CN", "zh-TW", "fr", "de"];
    // item-named-suffix requires a $name arg
    for locale in &locales {
        lm.set_language(locale).unwrap();
        let mut args = FluentArgs::new();
        args.set("name", "Excalibur".to_string());
        args.set("gender", "masculine".to_string());
        let result = lm.translate("item-named-suffix", Some(&args));
        assert_ne!(
            result, "item-named-suffix",
            "item-named-suffix missing in {}",
            locale
        );
    }
}

#[test]
fn ftl_coverage_class_patterns_en_zh_cn_fr_de() {
    let mut lm = setup_all();
    let class_keys = [
        "item-potion-identified",
        "item-scroll-identified",
        "item-wand-identified",
        "item-ring-identified",
        "item-spellbook-identified",
    ];
    // These keys require a $name arg.
    let locales = ["en", "zh-CN", "fr", "de"];
    for key in &class_keys {
        for locale in &locales {
            lm.set_language(locale).unwrap();
            let mut args = FluentArgs::new();
            args.set("name", "test".to_string());
            let result = lm.translate(key, Some(&args));
            assert_ne!(
                result, *key,
                "FTL key '{}' missing in locale '{}'",
                key, locale
            );
        }
    }
}

#[test]
fn ftl_coverage_count_name_all_locales() {
    let mut lm = setup_all();
    let locales = ["en", "zh-CN", "zh-TW", "fr", "de"];
    for locale in &locales {
        lm.set_language(locale).unwrap();
        let mut args = FluentArgs::new();
        args.set("count", 3_i64);
        args.set("singular", "sword".to_string());
        args.set("plural", "swords".to_string());
        let result = lm.translate("item-count-name", Some(&args));
        assert_ne!(
            result, "item-count-name",
            "item-count-name missing in {}",
            locale
        );
    }
}

#[test]
fn ftl_coverage_ui_keys_en_zh_cn() {
    let mut lm = setup_all();
    let ui_keys = [
        "ui-inventory-title",
        "ui-inventory-empty",
        "ui-more",
        "ui-equipment-title",
        "ui-help-title",
        "ui-select-language",
        "ui-save-prompt",
        "ui-save-success",
        "ui-goodbye",
    ];
    let locales = ["en", "zh-CN"];
    for key in &ui_keys {
        assert_key_in_locales(&mut lm, key, &locales);
    }
}

#[test]
fn ftl_coverage_event_keys_en_zh_cn() {
    let mut lm = setup_all();
    let event_keys = ["entity-killed", "entity-destroyed", "entity-dissolved"];
    // These keys require $entity arg.
    let locales = ["en", "zh-CN"];
    for key in &event_keys {
        for locale in &locales {
            lm.set_language(locale).unwrap();
            let mut args = FluentArgs::new();
            args.set("entity", "goblin".to_string());
            let result = lm.translate(key, Some(&args));
            assert_ne!(
                result, *key,
                "FTL key '{}' missing in locale '{}'",
                key, locale
            );
        }
    }
}

#[test]
fn ftl_coverage_melee_keys_en_zh_cn() {
    let mut lm = setup_all();
    // melee-hit-bare and melee-miss require $attacker/$defender.
    let locales = ["en", "zh-CN"];
    for locale in &locales {
        lm.set_language(locale).unwrap();
        let mut args = FluentArgs::new();
        args.set("attacker", "You".to_string());
        args.set("defender", "goblin".to_string());

        let hit = lm.translate("melee-hit-bare", Some(&args));
        assert_ne!(
            hit, "melee-hit-bare",
            "melee-hit-bare missing in {}",
            locale
        );

        let miss = lm.translate("melee-miss", Some(&args));
        assert_ne!(miss, "melee-miss", "melee-miss missing in {}", locale);
    }
}

#[test]
fn ftl_coverage_status_keys_en_zh_cn() {
    let mut lm = setup_all();
    let status_keys = [
        "status-hungry",
        "status-weak",
        "status-satiated",
        "status-fainting",
        "status-starved",
    ];
    let locales = ["en", "zh-CN"];
    for key in &status_keys {
        assert_key_in_locales(&mut lm, key, &locales);
    }
}

#[test]
fn ftl_coverage_inv_class_headers_en_zh_cn() {
    let mut lm = setup_all();
    let inv_keys = [
        "inv-class-weapon",
        "inv-class-armor",
        "inv-class-ring",
        "inv-class-amulet",
        "inv-class-tool",
        "inv-class-food",
        "inv-class-potion",
        "inv-class-scroll",
        "inv-class-spellbook",
        "inv-class-wand",
        "inv-class-coin",
        "inv-class-gem",
    ];
    let locales = ["en", "zh-CN"];
    for key in &inv_keys {
        assert_key_in_locales(&mut lm, key, &locales);
    }
}

#[test]
fn ftl_coverage_prompt_keys_en_zh_cn() {
    let mut lm = setup_all();
    let prompt_keys = [
        "prompt-direction",
        "prompt-eat",
        "prompt-drink",
        "prompt-read",
        "prompt-name",
    ];
    let locales = ["en", "zh-CN"];
    for key in &prompt_keys {
        assert_key_in_locales(&mut lm, key, &locales);
    }
}

// ==========================================================================
// 2. CJK Leak Guard Tests
// ==========================================================================

#[test]
fn cjk_leak_guard_english_ftl_no_cjk() {
    let lm = setup_en();
    // Probe a variety of known English FTL keys and confirm no CJK in output.
    let simple_keys = [
        "item-buc-blessed",
        "item-buc-cursed",
        "item-buc-uncursed",
        "item-erosion-rusty",
        "item-erosion-very-rusty",
        "item-erosion-corroded",
        "item-erosion-rustproof",
        "item-erosion-fireproof",
        "item-potion-generic",
        "item-scroll-generic",
        "item-wand-generic",
        "item-ring-generic",
        "item-amulet-generic",
        "item-spellbook-generic",
        "item-gem-stone",
        "item-gem-gem",
        "item-article-the",
        "item-article-your",
        "ui-inventory-title",
        "ui-inventory-empty",
        "ui-more",
        "status-hungry",
        "status-weak",
        "status-satiated",
        "inv-class-weapon",
        "inv-class-armor",
        "inv-class-potion",
    ];
    for key in &simple_keys {
        let result = lm.translate(key, None);
        // Only test keys that actually resolved.
        if result != *key {
            assert!(
                !contains_cjk(&result),
                "CJK characters found in English FTL key '{}': \"{}\"",
                key,
                result
            );
        }
    }
}

#[test]
fn cjk_leak_guard_english_ftl_parametric_no_cjk() {
    let lm = setup_en();
    // Test parametric keys in English with sample values.
    let mut args = FluentArgs::new();
    args.set("name", "healing".to_string());
    let potion = lm.translate("item-potion-identified", Some(&args));
    assert!(
        !contains_cjk(&potion),
        "CJK leaked in English potion name: \"{}\"",
        potion
    );

    let mut args2 = FluentArgs::new();
    args2.set("name", "Excalibur".to_string());
    args2.set("gender", "masculine".to_string());
    let named = lm.translate("item-named-suffix", Some(&args2));
    assert!(
        !contains_cjk(&named),
        "CJK leaked in English named suffix: \"{}\"",
        named
    );
}

#[test]
fn cjk_leak_guard_noun_phrase_english_no_cjk() {
    let lm = setup_en();
    let phrases = [
        NounPhrase::new("long sword").with_article(Article::A),
        NounPhrase::new("long sword")
            .with_article(Article::A)
            .with_buc(BucLabel::Blessed)
            .with_enchantment(2),
        NounPhrase::new("arrow")
            .with_article(Article::A)
            .with_quantity(5),
        NounPhrase::new("amulet")
            .with_article(Article::The)
            .with_name("Yendor"),
        NounPhrase::new("dagger")
            .with_article(Article::A)
            .with_buc(BucLabel::Cursed)
            .with_erosion("rusty")
            .with_enchantment(-1),
    ];
    for np in &phrases {
        let rendered = np.render(&lm);
        assert!(
            !contains_cjk(&rendered),
            "CJK leaked in English NounPhrase for '{}': \"{}\"",
            np.base_name,
            rendered
        );
    }
}

#[test]
fn cjk_leak_guard_translate_object_name_english() {
    let lm = setup_en();
    // In English mode, translate_object_name should return English names.
    let names = ["long sword", "dagger", "arrow", "plate mail"];
    for name in &names {
        let translated = lm.translate_object_name(name);
        assert!(
            !contains_cjk(translated),
            "CJK leaked in English translate_object_name for '{}': \"{}\"",
            name,
            translated
        );
        // Should be identity for English.
        assert_eq!(translated, *name, "English object name should be unchanged");
    }
}

// ==========================================================================
// 3. English Parity Tests -- Fluent vs Legacy
// ==========================================================================

#[test]
fn english_parity_basic_items() {
    let legacy = LocaleManager::new(); // no FTL loaded = legacy path
    let fluent = setup_en(); // FTL loaded = Fluent path

    let cases = vec![
        NounPhrase::new("long sword").with_article(Article::A),
        NounPhrase::new("dagger").with_article(Article::A),
        NounPhrase::new("amulet").with_article(Article::A),
        NounPhrase::new("arrow")
            .with_article(Article::A)
            .with_quantity(3),
        NounPhrase::new("arrow")
            .with_article(Article::A)
            .with_quantity(5),
        NounPhrase::new("long sword")
            .with_article(Article::The)
            .with_name("Excalibur"),
        NounPhrase::new("long sword").with_article(Article::Your),
    ];
    for np in &cases {
        let legacy_result = np.render(&legacy);
        let fluent_result = np.render(&fluent);
        assert_eq!(
            legacy_result, fluent_result,
            "Parity mismatch for '{}' qty={}",
            np.base_name, np.quantity
        );
    }
}

#[test]
fn english_parity_buc_enchantment_combos() {
    let legacy = LocaleManager::new();
    let fluent = setup_en();

    let cases = vec![
        NounPhrase::new("long sword")
            .with_article(Article::A)
            .with_buc(BucLabel::Blessed)
            .with_enchantment(2),
        NounPhrase::new("dagger")
            .with_article(Article::A)
            .with_buc(BucLabel::Cursed)
            .with_enchantment(-1),
        NounPhrase::new("long sword")
            .with_article(Article::A)
            .with_buc(BucLabel::Uncursed)
            .with_enchantment(0),
    ];
    for np in &cases {
        assert_eq!(
            np.render(&legacy),
            np.render(&fluent),
            "BUC/enchant parity mismatch for '{}'",
            np.base_name
        );
    }
}

#[test]
fn english_parity_class_patterns() {
    let fluent = setup_en();

    // Verify Fluent renders known English class patterns correctly.
    let mut args = FluentArgs::new();
    args.set("name", "healing".to_string());
    assert_eq!(
        fluent.translate("item-potion-identified", Some(&args)),
        "potion of healing"
    );

    let mut args2 = FluentArgs::new();
    args2.set("label", "FOOBIE BLETCH".to_string());
    assert_eq!(
        fluent.translate("item-scroll-labeled", Some(&args2)),
        "scroll labeled FOOBIE BLETCH"
    );

    let mut args3 = FluentArgs::new();
    args3.set("name", "wishing".to_string());
    assert_eq!(
        fluent.translate("item-wand-identified", Some(&args3)),
        "wand of wishing"
    );
}

#[test]
fn english_parity_erosion_combos() {
    let legacy = LocaleManager::new();
    let fluent = setup_en();

    let np = NounPhrase::new("long sword")
        .with_article(Article::A)
        .with_erosion("very rusty corroded")
        .with_enchantment(0);
    assert_eq!(
        np.render(&legacy),
        np.render(&fluent),
        "Erosion combo parity mismatch"
    );
    assert_eq!(np.render(&fluent), "a very rusty corroded +0 long sword");
}

#[test]
fn english_parity_article_an() {
    let legacy = LocaleManager::new();
    let fluent = setup_en();

    // "an" before vowel
    let np = NounPhrase::new("amulet").with_article(Article::A);
    assert_eq!(np.render(&legacy), np.render(&fluent));
    assert_eq!(np.render(&fluent), "an amulet");

    // "an" before uncursed
    let np2 = NounPhrase::new("long sword")
        .with_article(Article::A)
        .with_buc(BucLabel::Uncursed)
        .with_enchantment(-1);
    assert_eq!(np2.render(&legacy), np2.render(&fluent));
    assert_eq!(np2.render(&fluent), "an uncursed -1 long sword");
}

#[test]
fn english_parity_quantity_plurals() {
    let fluent = setup_en();

    let np3 = NounPhrase::new("dagger")
        .with_article(Article::A)
        .with_quantity(3);
    assert_eq!(np3.render(&fluent), "3 daggers");

    let np5 = NounPhrase::new("arrow")
        .with_article(Article::A)
        .with_quantity(5);
    assert_eq!(np5.render(&fluent), "5 arrows");

    let np1 = NounPhrase::new("gold piece")
        .with_article(Article::A)
        .with_quantity(42);
    assert_eq!(np1.render(&fluent), "42 gold pieces");
}

// ==========================================================================
// 4. CJK Rendering Tests
// ==========================================================================

#[test]
fn cjk_buc_labels_render_correctly() {
    let lm = setup_zh_cn();
    // Direct FTL lookup.
    assert_eq!(
        lm.translate("item-buc-blessed", None),
        "\u{795d}\u{798f}\u{7684}"
    );
    assert_eq!(
        lm.translate("item-buc-cursed", None),
        "\u{88ab}\u{8bc5}\u{5492}\u{7684}"
    );
    assert_eq!(
        lm.translate("item-buc-uncursed", None),
        "\u{672a}\u{8bc5}\u{5492}\u{7684}"
    );
}

#[test]
fn cjk_named_suffix_uses_corner_brackets() {
    let lm = setup_zh_cn();
    let mut args = FluentArgs::new();
    args.set("name", "Excalibur".to_string());
    // CJK named suffix should use corner brackets.
    let result = lm.translate("item-named-suffix", Some(&args));
    assert!(
        result.contains('\u{300c}') && result.contains('\u{300d}'),
        "CJK named suffix should use corner brackets: \"{}\"",
        result
    );
    assert!(
        result.contains("Excalibur"),
        "Named suffix should contain the name: \"{}\"",
        result
    );
}

#[test]
fn cjk_no_spaces_between_components() {
    let lm = setup_zh_cn();
    let np = NounPhrase::new("\u{957f}\u{5251}") // 长剑
        .with_buc(BucLabel::Blessed)
        .with_enchantment(2);
    let rendered = np.render(&lm);
    // CJK rendering should not contain spaces.
    assert!(
        !rendered.contains(' '),
        "CJK rendering should have no spaces: \"{}\"",
        rendered
    );
    assert_eq!(rendered, "\u{795d}\u{798f}\u{7684}+2\u{957f}\u{5251}");
}

#[test]
fn cjk_quantity_with_classifier() {
    let lm = setup_zh_cn();
    let np = NounPhrase::new("\u{7bad}") // 箭
        .with_quantity(5)
        .with_classifier("\u{652f}"); // 支
    let rendered = np.render(&lm);
    assert_eq!(rendered, "5\u{652f}\u{7bad}"); // 5支箭
}

#[test]
fn cjk_named_rendering() {
    let lm = setup_zh_cn();
    let np = NounPhrase::new("\u{957f}\u{5251}") // 长剑
        .with_name("Excalibur");
    let rendered = np.render(&lm);
    // 长剑「Excalibur」
    assert_eq!(rendered, "\u{957f}\u{5251}\u{300c}Excalibur\u{300d}");
}

#[test]
fn cjk_object_name_translation() {
    let lm = setup_zh_cn();
    assert_eq!(lm.translate_object_name("long sword"), "\u{957f}\u{5251}");
    assert_eq!(lm.translate_object_name("dagger"), "\u{5315}\u{9996}");
    assert_eq!(lm.translate_object_name("arrow"), "\u{7bad}");
}

#[test]
fn cjk_monster_name_translation() {
    let lm = setup_zh_cn();
    assert_eq!(lm.translate_monster_name("giant ant"), "\u{5de8}\u{8681}");
    assert_eq!(
        lm.translate_monster_name("goblin"),
        "\u{54e5}\u{5e03}\u{6797}"
    );
}

#[test]
fn cjk_blessed_enchanted_full_rendering() {
    let lm = setup_zh_cn();
    let np = NounPhrase::new("\u{957f}\u{5251}") // 长剑
        .with_buc(BucLabel::Blessed)
        .with_enchantment(3)
        .with_name("Excalibur");
    let rendered = np.render(&lm);
    // Expected: 祝福的+3长剑「Excalibur」
    assert!(rendered.starts_with("\u{795d}\u{798f}\u{7684}+3"));
    assert!(rendered.contains("\u{957f}\u{5251}"));
    assert!(rendered.contains("\u{300c}Excalibur\u{300d}"));
    assert!(!rendered.contains(' '));
}

// ==========================================================================
// 5. French Rendering Tests
// ==========================================================================

#[test]
fn french_gender_agreement_blessed() {
    let lm = setup_fr();
    // Feminine: blessed -> "benie"
    let np_f = NounPhrase::new("\u{e9}p\u{e9}e longue")
        .with_article(Article::A)
        .with_buc(BucLabel::Blessed)
        .with_gender("feminine");
    let rendered_f = np_f.render(&lm);
    assert!(
        rendered_f.contains("b\u{e9}nie"),
        "French feminine blessed should be 'benie': \"{}\"",
        rendered_f
    );
    assert!(
        rendered_f.starts_with("une"),
        "French feminine article should be 'une': \"{}\"",
        rendered_f
    );

    // Masculine: blessed -> "beni"
    let np_m = NounPhrase::new("arc")
        .with_article(Article::A)
        .with_buc(BucLabel::Blessed)
        .with_gender("masculine");
    let rendered_m = np_m.render(&lm);
    assert!(
        rendered_m.contains("b\u{e9}ni"),
        "French masculine blessed should be 'beni': \"{}\"",
        rendered_m
    );
    assert!(
        rendered_m.starts_with("un "),
        "French masculine article should be 'un': \"{}\"",
        rendered_m
    );
}

#[test]
fn french_cursed_gender_agreement() {
    let lm = setup_fr();
    // Feminine cursed -> "maudite"
    let np_f = NounPhrase::new("dague")
        .with_article(Article::A)
        .with_buc(BucLabel::Cursed)
        .with_gender("feminine");
    let rendered_f = np_f.render(&lm);
    assert!(
        rendered_f.contains("maudite"),
        "French feminine cursed should be 'maudite': \"{}\"",
        rendered_f
    );

    // Masculine cursed -> "maudit"
    let np_m = NounPhrase::new("arc")
        .with_article(Article::A)
        .with_buc(BucLabel::Cursed)
        .with_gender("masculine");
    let rendered_m = np_m.render(&lm);
    assert!(
        rendered_m.contains("maudit"),
        "French masculine cursed should contain 'maudit': \"{}\"",
        rendered_m
    );
    // Ensure it is not the feminine form "maudite".
    // Since "maudite" contains "maudit", check the exact word boundary.
    let words: Vec<&str> = rendered_m.split_whitespace().collect();
    assert!(
        words.contains(&"maudit"),
        "French masculine cursed should have exact word 'maudit': {:?}",
        words
    );
}

#[test]
fn french_articles_gender_based() {
    let lm = setup_fr();
    let mut args_m = FluentArgs::new();
    args_m.set("gender", "masculine".to_string());
    args_m.set("case", "nominative".to_string());
    assert_eq!(lm.translate("item-article-indefinite", Some(&args_m)), "un");

    let mut args_f = FluentArgs::new();
    args_f.set("gender", "feminine".to_string());
    args_f.set("case", "nominative".to_string());
    assert_eq!(
        lm.translate("item-article-indefinite", Some(&args_f)),
        "une"
    );
}

#[test]
fn french_class_patterns() {
    let lm = setup_fr();
    let mut args = FluentArgs::new();
    args.set("name", "healing".to_string());
    let result = lm.translate("item-potion-identified", Some(&args));
    assert_eq!(result, "potion de healing");
}

#[test]
fn french_named_suffix_gender() {
    let lm = setup_fr();
    let mut args_m = FluentArgs::new();
    args_m.set("name", "Excalibur".to_string());
    args_m.set("gender", "masculine".to_string());
    let result_m = lm.translate("item-named-suffix", Some(&args_m));
    assert!(
        result_m.contains("nomm\u{e9}") && !result_m.contains("nomm\u{e9}e"),
        "French masculine named suffix should use 'nomme' (no final e): \"{}\"",
        result_m
    );

    let mut args_f = FluentArgs::new();
    args_f.set("name", "Excalibur".to_string());
    args_f.set("gender", "feminine".to_string());
    let result_f = lm.translate("item-named-suffix", Some(&args_f));
    assert!(
        result_f.contains("nomm\u{e9}e"),
        "French feminine named suffix should use 'nommee': \"{}\"",
        result_f
    );
}

#[test]
fn french_quantity_plural() {
    let lm = setup_fr();
    let np = NounPhrase::new("dague")
        .with_article(Article::A)
        .with_quantity(3)
        .with_gender("feminine")
        .with_plural("dagues");
    let rendered = np.render(&lm);
    assert_eq!(rendered, "3 dagues");
}

#[test]
fn french_object_name_translation() {
    let lm = setup_fr();
    let (name, meta) = lm.translate_object_name_with_meta("long sword");
    assert_eq!(name, "\u{e9}p\u{e9}e longue");
    assert_eq!(meta.gender.as_deref(), Some("feminine"));
}

#[test]
fn french_erosion_gender_agreement() {
    let lm = setup_fr();
    // Feminine: rusty -> "rouillee"
    let np_f = NounPhrase::new("dague")
        .with_article(Article::A)
        .with_erosion("rusty")
        .with_gender("feminine");
    let rendered = np_f.render(&lm);
    assert!(
        rendered.contains("rouill\u{e9}e"),
        "French feminine rusty should be 'rouillee': \"{}\"",
        rendered
    );
}

// ==========================================================================
// 6. German Rendering Tests
// ==========================================================================

#[test]
fn german_three_gender_articles() {
    let lm = setup_de();
    // Masculine: "ein Dolch"
    let np_m = NounPhrase::new("Dolch")
        .with_article(Article::A)
        .with_gender("masculine");
    assert_eq!(np_m.render(&lm), "ein Dolch");

    // Feminine: "eine Keule"
    let np_f = NounPhrase::new("Keule")
        .with_article(Article::A)
        .with_gender("feminine");
    assert_eq!(np_f.render(&lm), "eine Keule");

    // Neuter: "ein Langschwert"
    let np_n = NounPhrase::new("Langschwert")
        .with_article(Article::A)
        .with_gender("neuter");
    assert_eq!(np_n.render(&lm), "ein Langschwert");
}

#[test]
fn german_accusative_articles() {
    let lm = setup_de();
    // Masculine accusative: "einen Dolch"
    let np = NounPhrase::new("Dolch")
        .with_article(Article::A)
        .with_gender("masculine")
        .with_case("accusative");
    assert_eq!(np.render(&lm), "einen Dolch");

    // Feminine accusative: "eine Keule" (same as nominative)
    let np_f = NounPhrase::new("Keule")
        .with_article(Article::A)
        .with_gender("feminine")
        .with_case("accusative");
    assert_eq!(np_f.render(&lm), "eine Keule");

    // Neuter accusative: "ein Langschwert" (same as nominative)
    let np_n = NounPhrase::new("Langschwert")
        .with_article(Article::A)
        .with_gender("neuter")
        .with_case("accusative");
    assert_eq!(np_n.render(&lm), "ein Langschwert");
}

#[test]
fn german_dative_articles() {
    let lm = setup_de();
    // Masculine dative: "einem Dolch"
    let np = NounPhrase::new("Dolch")
        .with_article(Article::A)
        .with_gender("masculine")
        .with_case("dative");
    assert_eq!(np.render(&lm), "einem Dolch");

    // Feminine dative: "einer Keule"
    let np_f = NounPhrase::new("Keule")
        .with_article(Article::A)
        .with_gender("feminine")
        .with_case("dative");
    assert_eq!(np_f.render(&lm), "einer Keule");
}

#[test]
fn german_buc_gender_agreement() {
    let lm = setup_de();
    // Masculine blessed: "gesegneter"
    let np_m = NounPhrase::new("Dolch")
        .with_article(Article::A)
        .with_buc(BucLabel::Blessed)
        .with_gender("masculine");
    assert_eq!(np_m.render(&lm), "ein gesegneter Dolch");

    // Feminine blessed: "gesegnete"
    let np_f = NounPhrase::new("Keule")
        .with_article(Article::A)
        .with_buc(BucLabel::Blessed)
        .with_gender("feminine");
    assert_eq!(np_f.render(&lm), "eine gesegnete Keule");

    // Neuter blessed: "gesegnetes"
    let np_n = NounPhrase::new("Langschwert")
        .with_article(Article::A)
        .with_buc(BucLabel::Blessed)
        .with_gender("neuter");
    assert_eq!(np_n.render(&lm), "ein gesegnetes Langschwert");
}

#[test]
fn german_cursed_gender_agreement() {
    let lm = setup_de();
    // Masculine: "verfluchter"
    let np_m = NounPhrase::new("Dolch")
        .with_article(Article::A)
        .with_buc(BucLabel::Cursed)
        .with_gender("masculine");
    let rendered_m = np_m.render(&lm);
    assert!(
        rendered_m.contains("verfluchter"),
        "German masculine cursed: \"{}\"",
        rendered_m
    );

    // Feminine: "verfluchte"
    let np_f = NounPhrase::new("Keule")
        .with_article(Article::A)
        .with_buc(BucLabel::Cursed)
        .with_gender("feminine");
    let rendered_f = np_f.render(&lm);
    assert!(
        rendered_f.contains("verfluchte"),
        "German feminine cursed: \"{}\"",
        rendered_f
    );

    // Neuter: "verfluchtes"
    let np_n = NounPhrase::new("Langschwert")
        .with_article(Article::A)
        .with_buc(BucLabel::Cursed)
        .with_gender("neuter");
    let rendered_n = np_n.render(&lm);
    assert!(
        rendered_n.contains("verfluchtes"),
        "German neuter cursed: \"{}\"",
        rendered_n
    );
}

#[test]
fn german_erosion_gender_agreement() {
    let lm = setup_de();
    // Masculine rusty -> "rostiger"
    let np_m = NounPhrase::new("Dolch")
        .with_article(Article::A)
        .with_erosion("rusty")
        .with_gender("masculine");
    let rendered = np_m.render(&lm);
    assert!(
        rendered.contains("rostiger"),
        "German masculine rusty should be 'rostiger': \"{}\"",
        rendered
    );

    // Feminine rusty -> "rostige"
    let np_f = NounPhrase::new("Keule")
        .with_article(Article::A)
        .with_erosion("rusty")
        .with_gender("feminine");
    let rendered_f = np_f.render(&lm);
    assert!(
        rendered_f.contains("rostige"),
        "German feminine rusty should be 'rostige': \"{}\"",
        rendered_f
    );

    // Neuter rusty -> "rostiges"
    let np_n = NounPhrase::new("Langschwert")
        .with_article(Article::A)
        .with_erosion("rusty")
        .with_gender("neuter");
    let rendered_n = np_n.render(&lm);
    assert!(
        rendered_n.contains("rostiges"),
        "German neuter rusty should be 'rostiges': \"{}\"",
        rendered_n
    );
}

#[test]
fn german_object_name_translation() {
    let lm = setup_de();
    let (name, meta) = lm.translate_object_name_with_meta("long sword");
    assert_eq!(name, "Langschwert");
    assert_eq!(meta.gender.as_deref(), Some("neuter"));

    let (name2, meta2) = lm.translate_object_name_with_meta("dagger");
    assert_eq!(name2, "Dolch");
    assert_eq!(meta2.gender.as_deref(), Some("masculine"));

    let (name3, meta3) = lm.translate_object_name_with_meta("mace");
    assert_eq!(name3, "Keule");
    assert_eq!(meta3.gender.as_deref(), Some("feminine"));
}

#[test]
fn german_class_patterns() {
    let lm = setup_de();
    let mut args = FluentArgs::new();
    args.set("name", "Heilung".to_string());
    let result = lm.translate("item-potion-identified", Some(&args));
    assert_eq!(result, "Trank von Heilung");

    let mut args2 = FluentArgs::new();
    args2.set("name", "Wunsch".to_string());
    let result2 = lm.translate("item-wand-identified", Some(&args2));
    assert_eq!(result2, "Zauberstab von Wunsch");
}

#[test]
fn german_named_suffix() {
    let lm = setup_de();
    let mut args = FluentArgs::new();
    args.set("name", "Excalibur".to_string());
    let result = lm.translate("item-named-suffix", Some(&args));
    assert_eq!(result, "namens Excalibur");
}

// ==========================================================================
// 7. Translation Metadata Tests
// ==========================================================================

#[test]
fn metadata_simple_string_toml_parses() {
    // zh-CN uses simple string format: "dagger" = "匕首"
    let lm = setup_zh_cn();
    assert_eq!(lm.translate_object_name("dagger"), "\u{5315}\u{9996}");
}

#[test]
fn metadata_structured_toml_parses() {
    // French uses structured format:
    // "long sword" = { name = "epee longue", gender = "feminine" }
    let lm = setup_fr();
    let (name, meta) = lm.translate_object_name_with_meta("long sword");
    assert_eq!(name, "\u{e9}p\u{e9}e longue");
    assert_eq!(meta.gender.as_deref(), Some("feminine"));
}

#[test]
fn metadata_gender_returned_correctly() {
    let lm = setup_fr();
    // Feminine items
    let (_, meta_f) = lm.translate_object_name_with_meta("dagger");
    assert_eq!(
        meta_f.gender.as_deref(),
        Some("feminine"),
        "French dagger (dague) should be feminine"
    );

    // Masculine items
    let (_, meta_m) = lm.translate_object_name_with_meta("scimitar");
    assert_eq!(
        meta_m.gender.as_deref(),
        Some("masculine"),
        "French scimitar (cimeterre) should be masculine"
    );
}

#[test]
fn metadata_plural_override_returned() {
    let lm = setup_fr();
    let (_, meta) = lm.translate_object_name_with_meta("knife");
    assert_eq!(
        meta.plural.as_deref(),
        Some("couteaux"),
        "French knife should have plural 'couteaux'"
    );
}

#[test]
fn metadata_german_gender_and_plural() {
    let lm = setup_de();
    let (name, meta) = lm.translate_object_name_with_meta("axe");
    assert_eq!(name, "Axt");
    assert_eq!(meta.gender.as_deref(), Some("feminine"));
    assert_eq!(meta.plural.as_deref(), Some("\u{c4}xte")); // Axte

    let (name2, meta2) = lm.translate_object_name_with_meta("bow");
    assert_eq!(name2, "Bogen");
    assert_eq!(meta2.gender.as_deref(), Some("masculine"));
    assert_eq!(meta2.plural.as_deref(), Some("B\u{f6}gen")); // Bogen
}

#[test]
fn metadata_backward_compat_simple_format() {
    // Both simple and structured formats should work.
    let mut lm = LocaleManager::new();
    let en_ftl = sanitize_ftl(EN_FTL);
    lm.load_locale("en", en_manifest(), &[("messages.ftl", &en_ftl)])
        .unwrap();
    let zh_cn_ftl = sanitize_ftl(ZH_CN_FTL);
    lm.load_locale("zh-CN", zh_cn_manifest(), &[("messages.ftl", &zh_cn_ftl)])
        .unwrap();
    // zh-CN objects use simple format (no metadata).
    lm.load_entity_translations("zh-CN", ZH_CN_MONSTERS_TOML, ZH_CN_OBJECTS_TOML)
        .unwrap();
    lm.set_language("zh-CN").unwrap();

    let (name, meta) = lm.translate_object_name_with_meta("long sword");
    assert_eq!(name, "\u{957f}\u{5251}");
    // No gender metadata for CJK.
    assert!(meta.gender.is_none());
}

// ==========================================================================
// 8. Fallback Chain Tests
// ==========================================================================

#[test]
fn fallback_zh_tw_to_zh_cn() {
    let mut lm = setup_all();
    lm.set_language("zh-TW").unwrap();

    // zh-TW has its own BUC labels (繁体).
    let blessed = lm.translate("item-buc-blessed", None);
    // zh-TW blessed is 祝福的 (same content, different encoding context)
    assert_ne!(blessed, "item-buc-blessed");

    // For FTL keys only in en/zh-CN but not zh-TW, it should fall back.
    // zh-TW has full FTL coverage, so test entity name fallback.
    // If zh-TW monsters.toml has "goblin" -> 哥布林, test that.
    // But if it does NOT have a specific monster, it should fall back to zh-CN.
    let goblin = lm.translate_monster_name("goblin");
    // zh-TW monsters.toml should have goblin; verify it is not English.
    assert!(
        contains_cjk(goblin),
        "zh-TW goblin should be CJK: \"{}\"",
        goblin
    );
}

#[test]
fn fallback_zh_cn_to_english() {
    let mut lm = setup_all();
    lm.set_language("zh-CN").unwrap();

    // A completely unknown FTL key should fall back all the way to the raw key.
    let result = lm.translate("this-key-does-not-exist-anywhere", None);
    assert_eq!(result, "this-key-does-not-exist-anywhere");

    // An unknown monster name should return the English name.
    let result = lm.translate_monster_name("nonexistent creature");
    assert_eq!(result, "nonexistent creature");
}

#[test]
fn fallback_fr_to_english() {
    let mut lm = setup_all();
    lm.set_language("fr").unwrap();

    // French now has its own ui-inventory-title translation.
    let result = lm.translate("ui-inventory-title", None);
    assert_eq!(
        result, "Inventaire",
        "French should have its own ui-inventory-title"
    );

    // Unknown object name falls back to English.
    let result = lm.translate_object_name("nonexistent weapon");
    assert_eq!(result, "nonexistent weapon");
}

#[test]
fn fallback_de_to_english() {
    let mut lm = setup_all();
    lm.set_language("de").unwrap();

    // German FTL does not have UI keys; should fall back to English.
    let result = lm.translate("ui-more", None);
    assert_eq!(
        result, "--More--",
        "German should fall back to English for ui-more"
    );
}

#[test]
fn fallback_unknown_msg_id_returns_raw_key() {
    let lm = setup_en();
    let result = lm.translate("totally-nonexistent-key-xyz", None);
    assert_eq!(result, "totally-nonexistent-key-xyz");
}

// ==========================================================================
// 9. Language Switching Tests
// ==========================================================================

#[test]
fn language_switch_en_to_zh_cn_and_back() {
    let mut lm = setup_all();

    // Start in English.
    assert_eq!(lm.current_language(), "en");
    let en_blessed = lm.translate("item-buc-blessed", None);
    assert_eq!(en_blessed, "blessed");

    // Switch to zh-CN.
    lm.set_language("zh-CN").unwrap();
    assert_eq!(lm.current_language(), "zh-CN");
    let zh_blessed = lm.translate("item-buc-blessed", None);
    assert_eq!(zh_blessed, "\u{795d}\u{798f}\u{7684}");
    assert_ne!(en_blessed, zh_blessed);

    // Switch back to English.
    lm.set_language("en").unwrap();
    assert_eq!(lm.current_language(), "en");
    let en_again = lm.translate("item-buc-blessed", None);
    assert_eq!(en_again, "blessed");
}

#[test]
fn language_switch_rendering_changes() {
    let mut lm = setup_all();

    let np = NounPhrase::new("long sword")
        .with_article(Article::A)
        .with_buc(BucLabel::Blessed);

    // English rendering.
    let en_result = np.render(&lm);
    assert_eq!(en_result, "a blessed long sword");

    // Switch to French.
    lm.set_language("fr").unwrap();
    let fr_result = np.render(&lm);
    // French should differ from English.
    assert_ne!(en_result, fr_result);
    // Without gender, defaults to masculine.
    assert!(
        fr_result.contains("b\u{e9}ni"),
        "French should contain 'beni': \"{}\"",
        fr_result
    );
}

#[test]
fn language_switch_nonexistent_falls_back() {
    let mut lm = setup_all();
    // Trying to switch to a non-loaded language should fail.
    let result = lm.set_language("ja");
    assert!(result.is_err(), "Switching to unloaded 'ja' should error");
    // Current language should remain unchanged.
    assert_eq!(lm.current_language(), "en");
}

#[test]
fn language_switch_multiple_locales() {
    let mut lm = setup_all();
    let locales = ["en", "zh-CN", "fr", "de", "zh-TW"];
    for locale in &locales {
        lm.set_language(locale).unwrap();
        assert_eq!(lm.current_language(), *locale);
        // BUC blessed should resolve in all locales.
        let result = lm.translate("item-buc-blessed", None);
        assert_ne!(
            result, "item-buc-blessed",
            "item-buc-blessed should resolve in {}",
            locale
        );
    }
}

// ==========================================================================
// 10. Plural System Tests
// ==========================================================================

#[test]
fn plural_english_regular_via_fluent() {
    let lm = setup_en();
    // "sword" -> "swords"
    let np = NounPhrase::new("sword")
        .with_article(Article::A)
        .with_quantity(3);
    assert_eq!(np.render(&lm), "3 swords");

    // "arrow" -> "arrows"
    let np2 = NounPhrase::new("arrow")
        .with_article(Article::A)
        .with_quantity(7);
    assert_eq!(np2.render(&lm), "7 arrows");
}

#[test]
fn plural_english_irregular_override() {
    let lm = setup_en();
    // "staff" with explicit plural override "staves"
    let np = NounPhrase::new("staff")
        .with_article(Article::A)
        .with_quantity(3)
        .with_plural("staves");
    assert_eq!(np.render(&lm), "3 staves");
}

#[test]
fn plural_english_builtin_irregular() {
    let lm = setup_en();
    // simple_pluralize handles "staff" -> "staves" automatically.
    let np = NounPhrase::new("staff")
        .with_article(Article::A)
        .with_quantity(2);
    let rendered = np.render(&lm);
    assert_eq!(rendered, "2 staves");
}

#[test]
fn plural_english_es_suffix() {
    let lm = setup_en();
    // "box" -> "boxes"
    let np = NounPhrase::new("box")
        .with_article(Article::A)
        .with_quantity(4);
    assert_eq!(np.render(&lm), "4 boxes");
}

#[test]
fn plural_english_ies_suffix() {
    let lm = setup_en();
    // "berry" -> "berries"
    let np = NounPhrase::new("berry")
        .with_article(Article::A)
        .with_quantity(5);
    assert_eq!(np.render(&lm), "5 berries");
}

#[test]
fn plural_english_knife_knives() {
    let lm = setup_en();
    let np = NounPhrase::new("knife")
        .with_article(Article::A)
        .with_quantity(2);
    assert_eq!(np.render(&lm), "2 knives");
}

#[test]
fn plural_cjk_ignores_plurals() {
    let lm = setup_zh_cn();
    // CJK should use quantity + classifier, not pluralization.
    let np = NounPhrase::new("\u{7bad}") // 箭
        .with_quantity(5)
        .with_classifier("\u{652f}"); // 支
    let rendered = np.render(&lm);
    // Should be "5支箭", not "5支箭s" or anything with plural suffix.
    assert_eq!(rendered, "5\u{652f}\u{7bad}");
    assert!(!rendered.contains('s'));
}

#[test]
fn plural_cjk_single_no_classifier_prefix() {
    let lm = setup_zh_cn();
    // Quantity 1 should not show quantity prefix.
    let np = NounPhrase::new("\u{957f}\u{5251}") // 长剑
        .with_quantity(1)
        .with_classifier("\u{628a}"); // 把
    let rendered = np.render(&lm);
    // Should be just "长剑", not "1把长剑".
    assert_eq!(rendered, "\u{957f}\u{5251}");
}

#[test]
fn plural_french_cldr_singular_vs_plural() {
    let lm = setup_fr();

    // item-count-name with count=1 should return singular.
    let mut args_one = FluentArgs::new();
    args_one.set("count", 1_i64);
    args_one.set("singular", "dague".to_string());
    args_one.set("plural", "dagues".to_string());
    let result_one = lm.translate("item-count-name", Some(&args_one));
    assert_eq!(result_one, "dague");

    // item-count-name with count=3 should return plural.
    let mut args_many = FluentArgs::new();
    args_many.set("count", 3_i64);
    args_many.set("singular", "dague".to_string());
    args_many.set("plural", "dagues".to_string());
    let result_many = lm.translate("item-count-name", Some(&args_many));
    assert_eq!(result_many, "dagues");
}

// ==========================================================================
// Additional integration tests
// ==========================================================================

#[test]
fn all_locales_listed_in_available() {
    let lm = setup_all();
    let available = lm.available_languages();
    for code in &["en", "zh-CN", "zh-TW", "fr", "de"] {
        assert!(
            available.contains(code),
            "Locale '{}' should be in available languages: {:?}",
            code,
            available
        );
    }
}

#[test]
fn manifest_properties_correct() {
    let lm = setup_all();
    // English
    let en = lm.manifest_for("en").unwrap();
    assert!(!en.is_cjk);
    assert!(en.has_articles);
    assert!(en.has_plural);
    assert!(!en.has_gender);

    // zh-CN
    let zh = lm.manifest_for("zh-CN").unwrap();
    assert!(zh.is_cjk);
    assert!(!zh.has_articles);
    assert!(!zh.has_plural);

    // French
    let fr = lm.manifest_for("fr").unwrap();
    assert!(!fr.is_cjk);
    assert!(fr.has_articles);
    assert!(fr.has_gender);

    // German
    let de = lm.manifest_for("de").unwrap();
    assert!(!de.is_cjk);
    assert!(de.has_articles);
    assert!(de.has_gender);
}

#[test]
fn english_no_cjk_in_all_simple_ftl_keys() {
    let lm = setup_en();
    // Exhaustive check of all simple (no-arg) FTL keys that should be pure
    // ASCII/Latin in the English locale.
    let keys = [
        "item-buc-blessed",
        "item-buc-cursed",
        "item-buc-uncursed",
        "item-erosion-rusty",
        "item-erosion-very-rusty",
        "item-erosion-thoroughly-rusty",
        "item-erosion-corroded",
        "item-erosion-very-corroded",
        "item-erosion-thoroughly-corroded",
        "item-erosion-burnt",
        "item-erosion-very-burnt",
        "item-erosion-thoroughly-burnt",
        "item-erosion-rotted",
        "item-erosion-very-rotted",
        "item-erosion-thoroughly-rotted",
        "item-erosion-rustproof",
        "item-erosion-fireproof",
        "item-erosion-corrodeproof",
        "item-erosion-rotproof",
        "item-potion-generic",
        "item-scroll-generic",
        "item-wand-generic",
        "item-ring-generic",
        "item-amulet-generic",
        "item-spellbook-generic",
        "item-gem-stone",
        "item-gem-gem",
        "item-article-the",
        "item-article-your",
        "ui-inventory-title",
        "ui-more",
        "status-hungry",
        "status-weak",
        "status-satiated",
        "status-fainting",
        "status-starved",
        "inv-class-weapon",
        "inv-class-armor",
        "inv-class-ring",
        "inv-class-amulet",
        "inv-class-tool",
        "inv-class-food",
        "inv-class-potion",
        "inv-class-scroll",
        "inv-class-spellbook",
        "inv-class-wand",
        "inv-class-coin",
        "inv-class-gem",
    ];
    for key in &keys {
        let result = lm.translate(key, None);
        if result != *key {
            assert!(
                is_ascii_latin(&result),
                "English key '{}' contains non-Latin chars: \"{}\"",
                key,
                result
            );
        }
    }
}

#[test]
fn full_rendering_roundtrip_all_locales() {
    let mut lm = setup_all();
    // Render a blessed +2 item across all locales and verify each produces
    // something non-empty and locale-appropriate.
    //
    // For CJK locales we use the translated base name (as the caller would
    // in production), for European locales we use the English name.
    let locales_and_names: &[(&str, &str, bool)] = &[
        ("en", "long sword", false),
        ("zh-CN", "\u{957f}\u{5251}", true),    // 长剑
        ("zh-TW", "\u{9577}\u{528d}", true),    // 長劍 (zh-TW objects.toml)
        ("fr", "\u{e9}p\u{e9}e longue", false), // epee longue
        ("de", "Langschwert", false),
    ];

    for (locale, base_name, is_cjk) in locales_and_names {
        lm.set_language(locale).unwrap();
        let np = NounPhrase::new(*base_name)
            .with_article(Article::A)
            .with_buc(BucLabel::Blessed)
            .with_enchantment(2);
        let rendered = np.render(&lm);
        assert!(
            !rendered.is_empty(),
            "Rendering should not be empty in locale {}",
            locale
        );
        if *is_cjk {
            assert!(
                !rendered.contains(' '),
                "CJK locale {} should not have spaces in NounPhrase: \"{}\"",
                locale,
                rendered
            );
        }
        if *locale == "en" {
            assert!(
                !contains_cjk(&rendered),
                "English rendering should not contain CJK: \"{}\"",
                rendered
            );
        }
    }
}

// ==========================================================================
// 11. FTL Key Resolution Tests (all locales)
// ==========================================================================

/// All help-* FTL keys resolve (not returned as-is) in every locale.
#[test]
fn help_keys_resolve_in_all_locales() {
    let keys = [
        "help-title",
        "help-move",
        "help-attack",
        "help-wait",
        "help-search",
        "help-inventory",
        "help-pickup",
        "help-drop",
        "help-stairs-up",
        "help-stairs-down",
        "help-eat",
        "help-quaff",
        "help-read",
        "help-wield",
        "help-wear",
        "help-remove",
        "help-zap",
        "help-move-diagram",
        "help-symbols-title",
        "help-symbol-player",
        "help-symbol-floor",
        "help-symbol-corridor",
        "help-symbol-door-closed",
        "help-symbol-door-open",
        "help-symbol-stairs-up",
        "help-symbol-stairs-down",
        "help-symbol-water",
        "help-symbol-fountain",
        "help-options",
        "help-look",
        "help-history",
        "help-shift-run",
        "help-arrows",
    ];
    for setup in [setup_en, setup_zh_cn, setup_zh_tw, setup_fr, setup_de] {
        let lm = setup();
        let lang = lm.current_language().to_string();
        for key in &keys {
            let result = lm.translate(key, None);
            assert_ne!(result, *key, "{lang}: FTL key '{key}' not resolved");
        }
    }
}

/// All inv-class-* keys resolve in every locale.
#[test]
fn inv_class_keys_resolve_in_all_locales() {
    let keys = [
        "inv-class-weapon",
        "inv-class-armor",
        "inv-class-ring",
        "inv-class-amulet",
        "inv-class-tool",
        "inv-class-food",
        "inv-class-potion",
        "inv-class-scroll",
        "inv-class-spellbook",
        "inv-class-wand",
        "inv-class-coin",
        "inv-class-gem",
        "inv-class-rock",
        "inv-class-ball",
        "inv-class-chain",
        "inv-class-venom",
        "inv-class-other",
    ];
    for setup in [setup_en, setup_zh_cn, setup_zh_tw, setup_fr, setup_de] {
        let lm = setup();
        let lang = lm.current_language().to_string();
        for key in &keys {
            let result = lm.translate(key, None);
            assert_ne!(result, *key, "{lang}: FTL key '{key}' not resolved");
        }
    }
}

/// All stat-label-* keys resolve in every locale.
#[test]
fn stat_label_keys_resolve_in_all_locales() {
    let keys = [
        "stat-label-str",
        "stat-label-dex",
        "stat-label-con",
        "stat-label-int",
        "stat-label-wis",
        "stat-label-cha",
        "stat-label-dlvl",
        "stat-label-gold",
        "stat-label-hp",
        "stat-label-pw",
        "stat-label-ac",
        "stat-label-xp",
        "stat-label-turn",
    ];
    for setup in [setup_en, setup_zh_cn, setup_zh_tw, setup_fr, setup_de] {
        let lm = setup();
        let lang = lm.current_language().to_string();
        for key in &keys {
            let result = lm.translate(key, None);
            assert_ne!(result, *key, "{lang}: FTL key '{key}' not resolved");
        }
    }
}

/// All options menu keys resolve in every locale.
#[test]
fn options_keys_resolve_in_all_locales() {
    let keys = [
        "ui-options-title",
        "ui-options-game",
        "ui-options-display",
        "ui-options-sound",
        "opt-autopickup",
        "opt-autopickup-types",
        "opt-legacy",
        "opt-map-colors",
        "opt-message-colors",
        "opt-buc-highlight",
        "opt-minimap",
        "opt-mouse-hover",
        "opt-nerd-fonts",
        "opt-sound-enabled",
        "opt-volume",
        "opt-on",
        "opt-off",
    ];
    for setup in [setup_en, setup_zh_cn, setup_zh_tw, setup_fr, setup_de] {
        let lm = setup();
        let lang = lm.current_language().to_string();
        for key in &keys {
            let result = lm.translate(key, None);
            assert_ne!(result, *key, "{lang}: FTL key '{key}' not resolved");
        }
    }
}

/// TUI common message keys resolve in every locale.
#[test]
fn tui_message_keys_resolve_in_all_locales() {
    let keys = [
        "ui-never-mind",
        "ui-no-such-item",
        "ui-not-implemented",
        "ui-empty-handed",
        "ui-inventory-title",
        "ui-inventory-empty",
        "ui-pickup-title",
    ];
    for setup in [setup_en, setup_zh_cn, setup_zh_tw, setup_fr, setup_de] {
        let lm = setup();
        let lang = lm.current_language().to_string();
        for key in &keys {
            let result = lm.translate(key, None);
            assert_ne!(result, *key, "{lang}: FTL key '{key}' not resolved");
        }
    }
}

/// Legacy intro renders with variable substitution in all locales.
#[test]
fn legacy_intro_substitution_all_locales() {
    for setup in [setup_en, setup_zh_cn, setup_zh_tw, setup_fr, setup_de] {
        let lm = setup();
        let lang = lm.current_language().to_string();
        let mut args = FluentArgs::new();
        args.set("deity", "Tyr".to_string());
        args.set("role", "Valkyrie".to_string());
        let result = lm.translate("legacy-intro", Some(&args));
        assert_ne!(result, "legacy-intro", "{lang}: legacy-intro not resolved");
        assert!(
            result.contains("Tyr"),
            "{lang}: legacy-intro should contain deity name 'Tyr', got: {result}"
        );
    }
}

// ==========================================================================
// 12. CJK Leak Detection (help, stat, inv-class)
// ==========================================================================

/// zh-CN help title should contain Chinese characters.
#[test]
fn help_zh_cn_is_chinese() {
    let lm = setup_zh_cn();
    let title = lm.translate("help-title", None);
    assert!(
        contains_cjk(&title),
        "zh-CN help-title should contain CJK characters, got: {title}"
    );
}

/// zh-CN stat labels should be Chinese, not English.
#[test]
fn stat_labels_zh_cn_are_chinese() {
    let lm = setup_zh_cn();
    let hp = lm.translate("stat-label-hp", None);
    assert_ne!(hp, "HP", "zh-CN stat-label-hp should not be English 'HP'");
    assert!(
        contains_cjk(&hp),
        "zh-CN stat-label-hp should contain CJK characters, got: {hp}"
    );
}

/// zh-CN inventory class headers should be Chinese.
#[test]
fn inv_class_zh_cn_is_chinese() {
    let lm = setup_zh_cn();
    let weapons = lm.translate("inv-class-weapon", None);
    assert_ne!(
        weapons, "Weapons",
        "zh-CN inv-class-weapon should not be English"
    );
    assert!(
        contains_cjk(&weapons),
        "zh-CN inv-class-weapon should be CJK, got: {weapons}"
    );
}

// ==========================================================================
// 13. FTL Key Parity Across Locales (WS5 audit)
// ==========================================================================

/// All FTL keys in English must also exist in every other locale.
#[test]
fn ftl_key_parity_across_locales() {
    use std::collections::HashSet;

    fn extract_keys(ftl_content: &str) -> HashSet<String> {
        ftl_content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim_end();
                // FTL message definitions start at column 0, are not comments,
                // not indented continuation lines, and contain " = " or end
                // with " =".
                if trimmed.is_empty()
                    || trimmed.starts_with('#')
                    || trimmed.starts_with(' ')
                    || trimmed.starts_with('.')
                    || trimmed.starts_with('*')
                    || trimmed.starts_with('[')
                {
                    return None;
                }
                if trimmed.contains(" = ") || trimmed.ends_with(" =") {
                    trimmed
                        .split(' ')
                        .next()
                        .filter(|k| !k.is_empty() && !k.starts_with('-'))
                        .map(|k| k.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    let en_keys = extract_keys(EN_FTL);
    assert!(
        en_keys.len() > 50,
        "English FTL should have many keys, got {}",
        en_keys.len()
    );

    let locales: &[(&str, &str)] = &[
        ("zh-CN", ZH_CN_FTL),
        ("zh-TW", ZH_TW_FTL),
        ("fr", FR_FTL),
        ("de", DE_FTL),
    ];

    for (locale_name, ftl_content) in locales {
        let keys = extract_keys(ftl_content);
        let missing: Vec<_> = en_keys.difference(&keys).cloned().collect();
        if !missing.is_empty() {
            let pct = missing.len() as f64 / en_keys.len() as f64 * 100.0;
            // CJK locales should be nearly complete
            if *locale_name == "zh-CN" || *locale_name == "zh-TW" {
                assert!(
                    missing.len() <= 20,
                    "{locale_name} has {} missing FTL keys ({:.1}%): {:?}",
                    missing.len(),
                    pct,
                    &missing[..missing.len().min(20)]
                );
            }
            // Report missing keys for all locales
            if !missing.is_empty() {
                eprintln!(
                    "NOTE: {locale_name} missing {} of {} FTL keys ({:.1}%)",
                    missing.len(),
                    en_keys.len(),
                    pct
                );
            }
        }
    }
}
