use fluent::FluentArgs;

use crate::locale::LocaleManager;

/// Which article (or possessive) to use when rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Article {
    /// No article at all.
    None,
    /// Indefinite article: "a" / "an".
    A,
    /// Definite article: "the".
    The,
    /// Possessive: "your".
    Your,
    /// Bare name (same as None, but semantically distinct for CJK).
    Bare,
}

/// Blessed/Uncursed/Cursed status label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucLabel {
    Blessed,
    Uncursed,
    Cursed,
}

/// A structured item name that can be rendered in any language.
///
/// Use the builder pattern to set optional fields, then call `render()`
/// with a `LocaleManager` to get the final display string.
#[derive(Debug, Clone)]
pub struct NounPhrase {
    pub article: Article,
    pub buc: Option<BucLabel>,
    pub erosion: Option<String>,
    pub enchantment: Option<i8>,
    pub material: Option<String>,
    pub base_name: String,
    pub named: Option<String>,
    pub suffix: Option<String>,
    pub quantity: i32,
    pub classifier: Option<String>,
    pub gender: Option<String>,
    pub case: Option<String>,
    pub plural_override: Option<String>,
}

impl NounPhrase {
    /// Create a new NounPhrase with just a base name and sensible defaults.
    pub fn new(base_name: impl Into<String>) -> Self {
        Self {
            article: Article::None,
            buc: None,
            erosion: None,
            enchantment: None,
            material: None,
            base_name: base_name.into(),
            named: None,
            suffix: None,
            quantity: 1,
            classifier: None,
            gender: None,
            case: None,
            plural_override: None,
        }
    }

    /// Render the noun phrase for the current locale.
    ///
    /// Uses the Fluent-based rendering path when the locale has item-name
    /// templates loaded. Falls back to hardcoded rendering when no FTL
    /// templates are available (e.g. bare `LocaleManager::new()`).
    pub fn render(&self, locale: &LocaleManager) -> String {
        // Check if the locale has item-name FTL templates loaded by probing
        // for a known message id.
        if locale.translate("item-buc-blessed", None) != "item-buc-blessed" {
            self.render_via_fluent(locale)
        } else if locale.is_cjk() {
            self.render_cjk(locale)
        } else {
            self.render_english()
        }
    }

    // ── Builder methods ──────────────────────────────────────────

    pub fn with_article(mut self, article: Article) -> Self {
        self.article = article;
        self
    }

    pub fn with_buc(mut self, buc: BucLabel) -> Self {
        self.buc = Some(buc);
        self
    }

    pub fn with_erosion(mut self, erosion: impl Into<String>) -> Self {
        self.erosion = Some(erosion.into());
        self
    }

    pub fn with_enchantment(mut self, spe: i8) -> Self {
        self.enchantment = Some(spe);
        self
    }

    pub fn with_material(mut self, material: impl Into<String>) -> Self {
        self.material = Some(material.into());
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.named = Some(name.into());
        self
    }

    pub fn with_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    pub fn with_quantity(mut self, quantity: i32) -> Self {
        self.quantity = quantity;
        self
    }

    pub fn with_classifier(mut self, classifier: impl Into<String>) -> Self {
        self.classifier = Some(classifier.into());
        self
    }

    pub fn with_gender(mut self, gender: impl Into<String>) -> Self {
        self.gender = Some(gender.into());
        self
    }

    pub fn with_case(mut self, case: impl Into<String>) -> Self {
        self.case = Some(case.into());
        self
    }

    pub fn with_plural(mut self, plural: impl Into<String>) -> Self {
        self.plural_override = Some(plural.into());
        self
    }

    // ── Fluent-based rendering (unified path) ─────────────────────

    /// Render via Fluent templates. Resolves each component through the
    /// locale's FTL messages, then assembles the final string.
    pub fn render_via_fluent(&self, locale: &LocaleManager) -> String {
        let is_cjk = locale.is_cjk();
        let gender = self.gender.as_deref().unwrap_or("none");
        let case = self.case.as_deref().unwrap_or("nominative");
        let is_en = locale.current_language() == "en";

        let mut parts: Vec<String> = Vec::new();

        // 1. Article / quantity prefix
        if is_cjk {
            // CJK: quantity + classifier prefix (e.g. "3把")
            if self.quantity > 1 {
                let clf = self.classifier.as_deref().unwrap_or("\u{4e2a}"); // 个
                parts.push(format!("{}{}", self.quantity, clf));
            }
        } else {
            match self.article {
                Article::The => {
                    let the = locale.translate("item-article-the", None);
                    parts.push(the);
                }
                Article::Your => {
                    if is_en {
                        parts.push("your".to_string());
                    } else {
                        let mut args = FluentArgs::new();
                        args.set("gender", gender.to_string());
                        args.set("case", case.to_string());
                        parts.push(locale.translate("item-article-your", Some(&args)));
                    }
                }
                Article::A => {
                    if self.quantity == 1 {
                        // Defer a/an for English; non-English uses Fluent
                        if !is_en {
                            let mut args = FluentArgs::new();
                            args.set("gender", gender.to_string());
                            args.set("case", case.to_string());
                            parts.push(locale.translate("item-article-indefinite", Some(&args)));
                        }
                        // For English: defer a/an until we know the next word
                    } else {
                        parts.push(self.quantity.to_string());
                    }
                }
                Article::None | Article::Bare => {
                    if self.quantity > 1 {
                        parts.push(self.quantity.to_string());
                    }
                }
            }
        }

        // 2. BUC label (via Fluent)
        if let Some(buc) = self.buc {
            let msg_id = match buc {
                BucLabel::Blessed => "item-buc-blessed",
                BucLabel::Uncursed => "item-buc-uncursed",
                BucLabel::Cursed => "item-buc-cursed",
            };
            let mut args = FluentArgs::new();
            args.set("gender", gender.to_string());
            args.set("case", case.to_string());
            parts.push(locale.translate(msg_id, Some(&args)));
        }

        // 3. Erosion (via Fluent)
        if let Some(ref erosion) = self.erosion {
            let mut args = FluentArgs::new();
            args.set("gender", gender.to_string());
            args.set("case", case.to_string());
            // The erosion string is already an English adjective like "rusty"
            // or "very rusty corroded". We need to translate it via Fluent.
            let translated = self.translate_erosion_string(erosion, locale, &args);
            parts.push(translated);
        }

        // 4. Enchantment
        if let Some(spe) = self.enchantment {
            parts.push(format!("{:+}", spe));
        }

        // 5. Material override
        if let Some(ref material) = self.material {
            parts.push(material.clone());
        }

        // 6. Base name (with pluralization for non-CJK)
        let name = if !is_cjk && self.quantity != 1 && self.article != Article::The {
            self.pluralize_via_fluent(locale)
        } else {
            self.base_name.clone()
        };
        parts.push(name);

        // 7. Named suffix
        if let Some(ref named) = self.named {
            let mut args = FluentArgs::new();
            args.set("name", named.clone());
            args.set("gender", gender.to_string());
            parts.push(locale.translate("item-named-suffix", Some(&args)));
        }

        // Assembly
        if is_cjk {
            let mut result = parts.join("");
            if let Some(ref suffix) = self.suffix {
                result.push_str(suffix);
            }
            result
        } else {
            // For English: deferred a/an article logic
            let mut result = if is_en && self.article == Article::A && self.quantity == 1 {
                let rest = parts.join(" ");
                let article = if starts_with_vowel_sound(&rest) {
                    "an"
                } else {
                    "a"
                };
                format!("{} {}", article, rest)
            } else {
                parts.join(" ")
            };

            if let Some(ref suffix) = self.suffix {
                result.push(' ');
                result.push_str(suffix);
            }
            result
        }
    }

    /// Translate an erosion string through Fluent.
    ///
    /// The input is the pre-composed English erosion string (e.g. "very rusty corroded").
    /// For non-English locales, each component is resolved via Fluent.
    fn translate_erosion_string(
        &self,
        erosion: &str,
        locale: &LocaleManager,
        args: &FluentArgs,
    ) -> String {
        // Map known erosion adjectives to FTL message ids.
        // The erosion string may contain multiple adjectives space-separated.
        let is_cjk = locale.is_cjk();
        let mut translated_parts = Vec::new();

        for part in split_erosion_parts(erosion) {
            let msg_id = match part {
                "rusty" => "item-erosion-rusty",
                "very rusty" => "item-erosion-very-rusty",
                "thoroughly rusty" => "item-erosion-thoroughly-rusty",
                "corroded" => "item-erosion-corroded",
                "very corroded" => "item-erosion-very-corroded",
                "thoroughly corroded" => "item-erosion-thoroughly-corroded",
                "burnt" => "item-erosion-burnt",
                "very burnt" => "item-erosion-very-burnt",
                "thoroughly burnt" => "item-erosion-thoroughly-burnt",
                "rotted" => "item-erosion-rotted",
                "very rotted" => "item-erosion-very-rotted",
                "thoroughly rotted" => "item-erosion-thoroughly-rotted",
                "rustproof" => "item-erosion-rustproof",
                "fireproof" => "item-erosion-fireproof",
                "corrodeproof" => "item-erosion-corrodeproof",
                "rotproof" => "item-erosion-rotproof",
                _ => {
                    translated_parts.push(part.to_string());
                    continue;
                }
            };
            translated_parts.push(locale.translate(msg_id, Some(args)));
        }

        if is_cjk {
            translated_parts.join("")
        } else {
            translated_parts.join(" ")
        }
    }

    /// Pluralize the base name using Fluent and/or the plural_override.
    fn pluralize_via_fluent(&self, locale: &LocaleManager) -> String {
        let plural_form = if let Some(ref ovr) = self.plural_override {
            ovr.clone()
        } else {
            // Fall back to rule-based English pluralization
            english_plural_fallback(&self.base_name)
        };

        let mut args = FluentArgs::new();
        args.set("count", self.quantity as i64);
        args.set("singular", self.base_name.clone());
        args.set("plural", plural_form.clone());
        let result = locale.translate("item-count-name", Some(&args));

        // If the Fluent message wasn't found, fall back to the plural form
        if result == "item-count-name" {
            plural_form
        } else {
            result
        }
    }

    // ── Legacy English rendering (used when FTL not loaded) ───────

    fn render_english(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        match self.article {
            Article::The => parts.push("the".to_string()),
            Article::Your => parts.push("your".to_string()),
            Article::A => {
                if self.quantity == 1 {
                    // Defer "a"/"an" until we know the next word.
                } else {
                    parts.push(self.quantity.to_string());
                }
            }
            Article::None | Article::Bare => {
                if self.quantity > 1 {
                    parts.push(self.quantity.to_string());
                }
            }
        }

        if let Some(buc) = self.buc {
            parts.push(buc_label_en(buc).to_string());
        }

        if let Some(ref erosion) = self.erosion {
            parts.push(erosion.clone());
        }

        if let Some(spe) = self.enchantment {
            parts.push(format!("{:+}", spe));
        }

        if let Some(ref material) = self.material {
            parts.push(material.clone());
        }

        let name = if self.quantity != 1 && self.article != Article::The {
            english_plural_fallback(&self.base_name)
        } else {
            self.base_name.clone()
        };
        parts.push(name);

        if let Some(ref named) = self.named {
            parts.push(format!("named {}", named));
        }

        let mut result = if self.article == Article::A && self.quantity == 1 {
            let rest = parts.join(" ");
            let article = if starts_with_vowel_sound(&rest) {
                "an"
            } else {
                "a"
            };
            format!("{} {}", article, rest)
        } else {
            parts.join(" ")
        };

        if let Some(ref suffix) = self.suffix {
            result.push(' ');
            result.push_str(suffix);
        }

        result
    }

    // ── Legacy CJK rendering (used when FTL not loaded) ──────────

    fn render_cjk(&self, _locale: &LocaleManager) -> String {
        let mut parts: Vec<String> = Vec::new();

        if self.quantity > 1 {
            let clf = self.classifier.as_deref().unwrap_or("\u{4e2a}"); // 个
            parts.push(format!("{}{}", self.quantity, clf));
        }

        if let Some(buc) = self.buc {
            parts.push(buc_label_cjk(buc).to_string());
        }

        if let Some(ref erosion) = self.erosion {
            parts.push(erosion.clone());
        }

        if let Some(spe) = self.enchantment {
            parts.push(format!("{:+}", spe));
        }

        if let Some(ref material) = self.material {
            parts.push(material.clone());
        }

        parts.push(self.base_name.clone());

        if let Some(ref named) = self.named {
            parts.push(format!("\u{300c}{}\u{300d}", named)); // 「name」
        }

        let mut result = parts.join("");

        if let Some(ref suffix) = self.suffix {
            result.push_str(suffix);
        }

        result
    }
}

/// English BUC label (legacy fallback).
fn buc_label_en(buc: BucLabel) -> &'static str {
    match buc {
        BucLabel::Blessed => "blessed",
        BucLabel::Uncursed => "uncursed",
        BucLabel::Cursed => "cursed",
    }
}

/// CJK BUC label (legacy fallback).
fn buc_label_cjk(buc: BucLabel) -> &'static str {
    match buc {
        BucLabel::Blessed => "\u{795d}\u{798f}\u{7684}", // 祝福的
        BucLabel::Uncursed => "\u{672a}\u{8bc5}\u{5492}\u{7684}", // 未诅咒的
        BucLabel::Cursed => "\u{88ab}\u{8bc5}\u{5492}\u{7684}", // 被诅咒的
    }
}

/// Simple heuristic: does the string start with a vowel sound?
fn starts_with_vowel_sound(s: &str) -> bool {
    let trimmed =
        s.trim_start_matches(|c: char| c == '+' || c == '-' || c.is_ascii_digit() || c == ' ');
    match trimmed.chars().next() {
        Some(c) => matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u'),
        None => false,
    }
}

/// Split an erosion string into its component adjective phrases.
///
/// Handles compound erosion like "very rusty corroded" → ["very rusty", "corroded"]
/// and "thoroughly rusty corroded rustproof" → ["thoroughly rusty", "corroded", "rustproof"].
fn split_erosion_parts(erosion: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut remaining = erosion;

    while !remaining.is_empty() {
        // Check for degree-qualified adjectives first
        let found = [
            "thoroughly rusty",
            "very rusty",
            "thoroughly corroded",
            "very corroded",
            "thoroughly burnt",
            "very burnt",
            "thoroughly rotted",
            "very rotted",
        ]
        .iter()
        .find(|&&prefix| remaining.starts_with(prefix));

        if let Some(&prefix) = found {
            parts.push(prefix);
            remaining = remaining[prefix.len()..].trim_start();
        } else {
            // Single-word adjective
            let end = remaining.find(' ').unwrap_or(remaining.len());
            parts.push(&remaining[..end]);
            remaining = remaining[end..].trim_start();
        }
    }
    parts
}

/// Very simple English pluralization (internal fallback).
///
/// This handles the common cases; nethack item names are mostly regular.
/// Used as fallback when no explicit plural form is provided in translation data.
///
/// Not part of the public API — callers should use `NounPhrase::with_plural()`
/// with data from `TranslationMeta`, or rely on `pluralize_via_fluent()`.
pub(crate) fn english_plural_fallback(word: &str) -> String {
    if word.is_empty() {
        return word.to_string();
    }

    // Special cases for common NetHack items.
    if let Some(prefix) = word.strip_suffix("staff") {
        return format!("{prefix}staves");
    }
    if word == "foot" {
        return "feet".to_string();
    }
    if word == "tooth" {
        return "teeth".to_string();
    }
    if let Some(prefix) = word.strip_suffix("mouse") {
        return format!("{prefix}mice");
    }
    if let Some(prefix) = word.strip_suffix("goose") {
        return format!("{prefix}geese");
    }
    if let Some(prefix) = word.strip_suffix("child") {
        return format!("{prefix}children");
    }

    // Words ending in s, x, z, ch, sh -> add "es".
    if word.ends_with('s')
        || word.ends_with('x')
        || word.ends_with('z')
        || word.ends_with("ch")
        || word.ends_with("sh")
    {
        return format!("{}es", word);
    }

    // Words ending in consonant + y -> change y to ies.
    if word.ends_with('y')
        && let Some(prev) = word.chars().rev().nth(1)
        && !"aeiou".contains(prev)
    {
        return format!("{}ies", &word[..word.len() - 1]);
    }

    // Words ending in f/fe -> ves (knife -> knives, wolf -> wolves).
    if let Some(prefix) = word.strip_suffix("fe") {
        return format!("{prefix}ves");
    }
    if word.ends_with('f') && !word.ends_with("ff") {
        return format!("{}ves", &word[..word.len() - 1]);
    }

    // Default: add "s".
    format!("{}s", word)
}

/// Public wrapper for tests only. Production code should not use this.
#[cfg(test)]
pub fn simple_pluralize(word: &str) -> String {
    english_plural_fallback(word)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::{LanguageManifest, LocaleManager};

    /// English FTL content for item-name templates (included at compile time).
    const EN_ITEM_FTL: &str = include_str!("../../../data/locale/en/messages.ftl");
    const ZH_CN_ITEM_FTL: &str = include_str!("../../../data/locale/zh-CN/messages.ftl");
    const FR_ITEM_FTL: &str = include_str!("../../../data/locale/fr/messages.ftl");
    const DE_ITEM_FTL: &str = include_str!("../../../data/locale/de/messages.ftl");

    /// Create a LocaleManager with English FTL loaded.
    fn locale_with_en_ftl() -> LocaleManager {
        let mut locale = LocaleManager::new();
        let manifest = LanguageManifest {
            code: "en".to_string(),
            name: "English".to_string(),
            name_en: "English".to_string(),
            author: "test".to_string(),
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
        };
        locale
            .load_locale("en", manifest, &[("messages.ftl", EN_ITEM_FTL)])
            .unwrap();
        locale
    }

    fn locale_with_zh_cn_ftl() -> LocaleManager {
        let mut locale = locale_with_en_ftl();
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
        locale
            .load_locale("zh-CN", manifest, &[("messages.ftl", ZH_CN_ITEM_FTL)])
            .unwrap();
        locale.set_language("zh-CN").unwrap();
        locale
    }

    fn locale_with_fr_ftl() -> LocaleManager {
        let mut locale = locale_with_en_ftl();
        let manifest = LanguageManifest {
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
        };
        locale
            .load_locale("fr", manifest, &[("messages.ftl", FR_ITEM_FTL)])
            .unwrap();
        locale.set_language("fr").unwrap();
        locale
    }

    fn locale_with_de_ftl() -> LocaleManager {
        let mut locale = locale_with_en_ftl();
        let manifest = LanguageManifest {
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
        };
        locale
            .load_locale("de", manifest, &[("messages.ftl", DE_ITEM_FTL)])
            .unwrap();
        locale.set_language("de").unwrap();
        locale
    }

    // ── Legacy tests (no FTL loaded, should still work) ────────

    #[test]
    fn test_simple_english() {
        let locale = LocaleManager::new();
        let np = NounPhrase::new("long sword").with_article(Article::A);
        assert_eq!(np.render(&locale), "a long sword");
    }

    #[test]
    fn test_english_with_enchantment_and_buc() {
        let locale = LocaleManager::new();
        let np = NounPhrase::new("long sword")
            .with_article(Article::A)
            .with_buc(BucLabel::Blessed)
            .with_enchantment(2);
        assert_eq!(np.render(&locale), "a blessed +2 long sword");
    }

    #[test]
    fn test_english_plural() {
        let locale = LocaleManager::new();
        let np = NounPhrase::new("arrow")
            .with_article(Article::A)
            .with_quantity(5);
        assert_eq!(np.render(&locale), "5 arrows");
    }

    #[test]
    fn test_english_an() {
        let locale = LocaleManager::new();
        let np = NounPhrase::new("amulet").with_article(Article::A);
        assert_eq!(np.render(&locale), "an amulet");
    }

    #[test]
    fn test_english_named() {
        let locale = LocaleManager::new();
        let np = NounPhrase::new("long sword")
            .with_article(Article::The)
            .with_name("Excalibur");
        assert_eq!(np.render(&locale), "the long sword named Excalibur");
    }

    #[test]
    fn test_cjk_basic() {
        let mut locale = LocaleManager::new();
        let manifest = LanguageManifest {
            code: "zh-CN".to_string(),
            name: "\u{7b80}\u{4f53}\u{4e2d}\u{6587}".to_string(),
            name_en: "Simplified Chinese".to_string(),
            author: "test".to_string(),
            version: "0.1.0".to_string(),
            fallback: None,
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
        locale.set_language("zh-CN").unwrap();

        let np = NounPhrase::new("\u{957f}\u{5251}") // 长剑
            .with_buc(BucLabel::Blessed)
            .with_enchantment(2);
        let rendered = np.render(&locale);
        assert_eq!(rendered, "\u{795d}\u{798f}\u{7684}+2\u{957f}\u{5251}");
        // 祝福的+2长剑
    }

    #[test]
    fn test_cjk_quantity_with_classifier() {
        let mut locale = LocaleManager::new();
        let manifest = LanguageManifest {
            code: "zh-CN".to_string(),
            name: "\u{7b80}\u{4f53}\u{4e2d}\u{6587}".to_string(),
            name_en: "Simplified Chinese".to_string(),
            author: "test".to_string(),
            version: "0.1.0".to_string(),
            fallback: None,
            is_cjk: true,
            has_articles: false,
            has_plural: false,
            has_verb_conj: false,
            has_gender: false,
            possessive: None,
            quote_left: None,
            quote_right: None,
        };
        locale.load_locale("zh-CN", manifest, &[]).unwrap();
        locale.set_language("zh-CN").unwrap();

        let np = NounPhrase::new("\u{7bad}") // 箭
            .with_quantity(5)
            .with_classifier("\u{652f}"); // 支
        let rendered = np.render(&locale);
        assert_eq!(rendered, "5\u{652f}\u{7bad}"); // 5支箭
    }

    #[test]
    fn test_pluralize() {
        assert_eq!(simple_pluralize("sword"), "swords");
        assert_eq!(simple_pluralize("box"), "boxes");
        assert_eq!(simple_pluralize("potion"), "potions");
        assert_eq!(simple_pluralize("knife"), "knives");
        assert_eq!(simple_pluralize("staff"), "staves");
        assert_eq!(simple_pluralize("berry"), "berries");
        assert_eq!(simple_pluralize("key"), "keys");
    }

    // ── Fluent rendering tests (FTL loaded) ────────────────────

    #[test]
    fn test_fluent_english_simple() {
        let locale = locale_with_en_ftl();
        let np = NounPhrase::new("long sword").with_article(Article::A);
        assert_eq!(np.render(&locale), "a long sword");
    }

    #[test]
    fn test_fluent_english_buc_enchantment() {
        let locale = locale_with_en_ftl();
        let np = NounPhrase::new("long sword")
            .with_article(Article::A)
            .with_buc(BucLabel::Blessed)
            .with_enchantment(2);
        assert_eq!(np.render(&locale), "a blessed +2 long sword");
    }

    #[test]
    fn test_fluent_english_plural() {
        let locale = locale_with_en_ftl();
        let np = NounPhrase::new("arrow")
            .with_article(Article::A)
            .with_quantity(5);
        assert_eq!(np.render(&locale), "5 arrows");
    }

    #[test]
    fn test_fluent_english_an() {
        let locale = locale_with_en_ftl();
        let np = NounPhrase::new("amulet").with_article(Article::A);
        assert_eq!(np.render(&locale), "an amulet");
    }

    #[test]
    fn test_fluent_english_named() {
        let locale = locale_with_en_ftl();
        let np = NounPhrase::new("long sword")
            .with_article(Article::The)
            .with_name("Excalibur");
        assert_eq!(np.render(&locale), "the long sword named Excalibur");
    }

    #[test]
    fn test_fluent_english_erosion() {
        let locale = locale_with_en_ftl();
        let np = NounPhrase::new("long sword")
            .with_article(Article::A)
            .with_erosion("very rusty corroded")
            .with_enchantment(0);
        assert_eq!(np.render(&locale), "a very rusty corroded +0 long sword");
    }

    #[test]
    fn test_fluent_english_plural_override() {
        let locale = locale_with_en_ftl();
        let np = NounPhrase::new("staff")
            .with_article(Article::A)
            .with_quantity(3)
            .with_plural("staves");
        assert_eq!(np.render(&locale), "3 staves");
    }

    #[test]
    fn test_fluent_english_your_article() {
        let locale = locale_with_en_ftl();
        let np = NounPhrase::new("long sword").with_article(Article::Your);
        assert_eq!(np.render(&locale), "your long sword");
    }

    #[test]
    fn test_fluent_english_uncursed_enchanted() {
        let locale = locale_with_en_ftl();
        let np = NounPhrase::new("long sword")
            .with_article(Article::A)
            .with_buc(BucLabel::Uncursed)
            .with_enchantment(-1);
        assert_eq!(np.render(&locale), "an uncursed -1 long sword");
    }

    // ── Fluent parity: verify render_via_fluent matches render_english ──

    #[test]
    fn test_fluent_en_parity_basic() {
        let locale_bare = LocaleManager::new();
        let locale_ftl = locale_with_en_ftl();

        let cases = vec![
            NounPhrase::new("long sword").with_article(Article::A),
            NounPhrase::new("long sword")
                .with_article(Article::A)
                .with_buc(BucLabel::Blessed)
                .with_enchantment(2),
            NounPhrase::new("arrow")
                .with_article(Article::A)
                .with_quantity(5),
            NounPhrase::new("amulet").with_article(Article::A),
            NounPhrase::new("long sword")
                .with_article(Article::The)
                .with_name("Excalibur"),
            NounPhrase::new("long sword")
                .with_article(Article::A)
                .with_erosion("rusty")
                .with_enchantment(1),
            NounPhrase::new("long sword")
                .with_article(Article::A)
                .with_buc(BucLabel::Cursed)
                .with_enchantment(-1),
            NounPhrase::new("elven broadsword").with_article(Article::A),
            NounPhrase::new("dart")
                .with_article(Article::A)
                .with_quantity(3)
                .with_enchantment(0),
        ];

        for np in &cases {
            let english = np.render(&locale_bare);
            let fluent = np.render(&locale_ftl);
            assert_eq!(english, fluent, "Parity mismatch for {:?}", np.base_name);
        }
    }

    // ── CJK Fluent rendering ──────────────────────────────────

    #[test]
    fn test_fluent_cjk_basic() {
        let locale = locale_with_zh_cn_ftl();
        let np = NounPhrase::new("\u{957f}\u{5251}") // 长剑
            .with_buc(BucLabel::Blessed)
            .with_enchantment(2);
        assert_eq!(
            np.render(&locale),
            "\u{795d}\u{798f}\u{7684}+2\u{957f}\u{5251}"
        );
    }

    #[test]
    fn test_fluent_cjk_quantity() {
        let locale = locale_with_zh_cn_ftl();
        let np = NounPhrase::new("\u{7bad}") // 箭
            .with_quantity(5)
            .with_classifier("\u{652f}"); // 支
        assert_eq!(np.render(&locale), "5\u{652f}\u{7bad}"); // 5支箭
    }

    #[test]
    fn test_fluent_cjk_named() {
        let locale = locale_with_zh_cn_ftl();
        let np = NounPhrase::new("\u{957f}\u{5251}") // 长剑
            .with_name("Excalibur");
        assert_eq!(
            np.render(&locale),
            "\u{957f}\u{5251}\u{300c}Excalibur\u{300d}"
        ); // 长剑「Excalibur」
    }

    // ── French rendering tests ─────────────────────────────────

    #[test]
    fn test_french_blessed_feminine() {
        let locale = locale_with_fr_ftl();
        let np = NounPhrase::new("\u{e9}p\u{e9}e longue") // épée longue
            .with_article(Article::A)
            .with_buc(BucLabel::Blessed)
            .with_gender("feminine");
        assert_eq!(np.render(&locale), "une b\u{e9}nie \u{e9}p\u{e9}e longue");
    }

    #[test]
    fn test_french_blessed_masculine() {
        let locale = locale_with_fr_ftl();
        let np = NounPhrase::new("arc")
            .with_article(Article::A)
            .with_buc(BucLabel::Blessed)
            .with_gender("masculine");
        assert_eq!(np.render(&locale), "un b\u{e9}ni arc");
    }

    #[test]
    fn test_french_quantity() {
        let locale = locale_with_fr_ftl();
        let np = NounPhrase::new("dague")
            .with_article(Article::A)
            .with_quantity(3)
            .with_gender("feminine")
            .with_plural("dagues");
        assert_eq!(np.render(&locale), "3 dagues");
    }

    // ── German rendering tests ─────────────────────────────────

    #[test]
    fn test_german_neuter_article() {
        let locale = locale_with_de_ftl();
        let np = NounPhrase::new("Langschwert")
            .with_article(Article::A)
            .with_gender("neuter");
        assert_eq!(np.render(&locale), "ein Langschwert");
    }

    #[test]
    fn test_german_masculine_buc() {
        let locale = locale_with_de_ftl();
        let np = NounPhrase::new("Dolch")
            .with_article(Article::A)
            .with_buc(BucLabel::Blessed)
            .with_gender("masculine");
        assert_eq!(np.render(&locale), "ein gesegneter Dolch");
    }

    #[test]
    fn test_german_feminine_buc() {
        let locale = locale_with_de_ftl();
        let np = NounPhrase::new("Keule")
            .with_article(Article::A)
            .with_buc(BucLabel::Blessed)
            .with_gender("feminine");
        assert_eq!(np.render(&locale), "eine gesegnete Keule");
    }

    #[test]
    fn test_german_accusative_article() {
        let locale = locale_with_de_ftl();
        let np = NounPhrase::new("Dolch")
            .with_article(Article::A)
            .with_gender("masculine")
            .with_case("accusative");
        assert_eq!(np.render(&locale), "einen Dolch");
    }

    #[test]
    fn test_german_neuter_buc() {
        let locale = locale_with_de_ftl();
        let np = NounPhrase::new("Langschwert")
            .with_article(Article::A)
            .with_buc(BucLabel::Blessed)
            .with_gender("neuter");
        assert_eq!(np.render(&locale), "ein gesegnetes Langschwert");
    }

    // ── Erosion parsing tests ──────────────────────────────────

    #[test]
    fn test_split_erosion_parts() {
        assert_eq!(split_erosion_parts("rusty"), vec!["rusty"]);
        assert_eq!(split_erosion_parts("very rusty"), vec!["very rusty"]);
        assert_eq!(
            split_erosion_parts("very rusty corroded"),
            vec!["very rusty", "corroded"]
        );
        assert_eq!(
            split_erosion_parts("thoroughly rusty corroded rustproof"),
            vec!["thoroughly rusty", "corroded", "rustproof"]
        );
        assert_eq!(split_erosion_parts("fireproof"), vec!["fireproof"]);
    }
}
