use std::collections::HashMap;

use fluent::FluentArgs;
use fluent::FluentBundle;
use fluent::FluentResource;
use serde::{Deserialize, Serialize};
use unic_langid::LanguageIdentifier;

use crate::classifier::Classifier;

/// Metadata about a language pack, loaded from its manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageManifest {
    pub code: String,
    pub name: String,
    pub name_en: String,
    pub author: String,
    pub version: String,
    pub fallback: Option<String>,
    pub is_cjk: bool,
    pub has_articles: bool,
    pub has_plural: bool,
    pub has_verb_conj: bool,
    #[serde(default)]
    pub has_gender: bool,
    pub possessive: Option<String>,
    pub quote_left: Option<String>,
    pub quote_right: Option<String>,
}

/// Grammatical metadata for a translated name.
#[derive(Debug, Clone, Default)]
pub struct TranslationMeta {
    pub gender: Option<String>,
    pub plural: Option<String>,
    pub genitive: Option<String>,
}

/// A translation entry that may be a simple string or a struct with metadata.
///
/// Supports both formats in TOML:
/// ```toml
/// "dagger" = "匕首"
/// "long sword" = { name = "épée longue", gender = "feminine" }
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum TranslationEntry {
    Simple(String),
    WithMeta {
        name: String,
        gender: Option<String>,
        plural: Option<String>,
        genitive: Option<String>,
    },
}

impl TranslationEntry {
    fn name(&self) -> &str {
        match self {
            TranslationEntry::Simple(s) => s,
            TranslationEntry::WithMeta { name, .. } => name,
        }
    }

    fn meta(&self) -> TranslationMeta {
        match self {
            TranslationEntry::Simple(_) => TranslationMeta::default(),
            TranslationEntry::WithMeta {
                gender,
                plural,
                genitive,
                ..
            } => TranslationMeta {
                gender: gender.clone(),
                plural: plural.clone(),
                genitive: genitive.clone(),
            },
        }
    }
}

/// TOML structure for entity name translation files (monsters.toml, objects.toml).
#[derive(Debug, Clone, Deserialize)]
struct EntityTranslations {
    translations: HashMap<String, TranslationEntry>,
}

/// Legacy TOML structure for simple string-only translations.
#[derive(Debug, Clone, Deserialize)]
struct SimpleEntityTranslations {
    translations: HashMap<String, String>,
}

/// Manages loaded locale bundles and provides translation lookups.
pub struct LocaleManager {
    bundles: HashMap<String, FluentBundle<FluentResource>>,
    manifests: HashMap<String, LanguageManifest>,
    current: String,
    /// Maps lang code -> (English monster name -> translated name).
    monster_names: HashMap<String, HashMap<String, String>>,
    /// Maps lang code -> (English object name -> translated name).
    object_names: HashMap<String, HashMap<String, String>>,
    /// Maps lang code -> (English object name -> grammatical metadata).
    object_meta: HashMap<String, HashMap<String, TranslationMeta>>,
    /// CJK classifier loaded from classifiers.toml.
    classifier: Option<Classifier>,
}

impl LocaleManager {
    /// Create a new LocaleManager with English as the default (empty) locale.
    pub fn new() -> Self {
        let mut manager = Self {
            bundles: HashMap::new(),
            manifests: HashMap::new(),
            current: "en".to_string(),
            monster_names: HashMap::new(),
            object_names: HashMap::new(),
            object_meta: HashMap::new(),
            classifier: None,
        };

        // Register English as the built-in default with an empty bundle.
        let en_manifest = LanguageManifest {
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
        };

        let langid: LanguageIdentifier = "en".parse().expect("valid langid");
        let bundle = FluentBundle::new(vec![langid]);
        manager.bundles.insert("en".to_string(), bundle);
        manager.manifests.insert("en".to_string(), en_manifest);
        manager
    }

    /// Load a locale from a manifest and a set of FTL source strings.
    ///
    /// Each entry in `ftl_sources` is `(filename, ftl_content)`.
    pub fn load_locale(
        &mut self,
        code: &str,
        manifest: LanguageManifest,
        ftl_sources: &[(&str, &str)],
    ) -> Result<(), LocaleError> {
        let langid: LanguageIdentifier = code
            .parse()
            .map_err(|e| LocaleError::ParseError(format!("invalid language id '{}': {}", code, e)))?;

        let mut bundle = FluentBundle::new(vec![langid]);
        bundle.set_use_isolating(false);

        for (filename, content) in ftl_sources {
            let resource = FluentResource::try_new(content.to_string()).map_err(|(_, errs)| {
                let msgs: Vec<String> = errs.iter().map(|e| format!("{}", e)).collect();
                LocaleError::ParseError(format!("{}:  {}", filename, msgs.join("; ")))
            })?;
            bundle.add_resource(resource).map_err(|errs| {
                let msgs: Vec<String> = errs.iter().map(|e| format!("{}", e)).collect();
                LocaleError::ParseError(format!(
                    "conflicting ids in {}: {}",
                    filename,
                    msgs.join("; ")
                ))
            })?;
        }

        self.bundles.insert(code.to_string(), bundle);
        self.manifests.insert(code.to_string(), manifest);
        Ok(())
    }

    /// Switch the active language. The language must have been loaded first.
    pub fn set_language(&mut self, code: &str) -> Result<(), LocaleError> {
        if self.bundles.contains_key(code) {
            self.current = code.to_string();
            Ok(())
        } else {
            Err(LocaleError::NotFound(code.to_string()))
        }
    }

    /// The code of the currently active language.
    pub fn current_language(&self) -> &str {
        &self.current
    }

    /// Translate a message id with optional Fluent arguments.
    ///
    /// Lookup order:
    /// 1. Current language bundle
    /// 2. Fallback language (if manifest specifies one)
    /// 3. English bundle
    /// 4. Raw `msg_id` as-is
    pub fn translate(&self, msg_id: &str, args: Option<&FluentArgs>) -> String {
        // Try current language.
        if let Some(result) = self.try_format(&self.current, msg_id, args) {
            return result;
        }

        // Try fallback from manifest.
        if let Some(manifest) = self.manifests.get(&self.current)
            && let Some(ref fallback) = manifest.fallback
                && fallback != &self.current
                    && let Some(result) = self.try_format(fallback, msg_id, args) {
                        return result;
                    }

        // Try English.
        if self.current != "en"
            && let Some(result) = self.try_format("en", msg_id, args) {
                return result;
            }

        // Last resort: return the raw message id.
        msg_id.to_string()
    }

    /// Get the manifest for the currently active language.
    pub fn manifest(&self) -> &LanguageManifest {
        self.manifests
            .get(&self.current)
            .expect("current language always has a manifest")
    }

    /// Get the manifest for a specific language code, if loaded.
    pub fn manifest_for(&self, code: &str) -> Option<&LanguageManifest> {
        self.manifests.get(code)
    }

    /// Whether the current language is CJK.
    pub fn is_cjk(&self) -> bool {
        self.manifest().is_cjk
    }

    /// Whether the current language uses articles (a/an/the).
    pub fn has_articles(&self) -> bool {
        self.manifest().has_articles
    }

    /// Whether the current language has grammatical plurals.
    pub fn has_plural(&self) -> bool {
        self.manifest().has_plural
    }

    /// List all loaded locale codes.
    pub fn available_languages(&self) -> Vec<&str> {
        self.manifests.keys().map(|s| s.as_str()).collect()
    }

    /// Load entity name translations (monsters and objects) for a language.
    ///
    /// Supports both simple and metadata formats in TOML:
    /// ```toml
    /// [translations]
    /// "giant ant" = "巨蚁"
    /// "long sword" = { name = "épée longue", gender = "feminine" }
    /// ```
    pub fn load_entity_translations(
        &mut self,
        code: &str,
        monsters_toml: &str,
        objects_toml: &str,
    ) -> Result<(), LocaleError> {
        // Monsters are always simple strings (no gender metadata needed).
        let monsters: SimpleEntityTranslations = toml::from_str(monsters_toml).map_err(|e| {
            LocaleError::ParseError(format!("monsters.toml for '{}': {}", code, e))
        })?;

        // Objects support both simple strings and metadata entries.
        let objects: EntityTranslations = toml::from_str(objects_toml).map_err(|e| {
            LocaleError::ParseError(format!("objects.toml for '{}': {}", code, e))
        })?;

        // Extract name strings and metadata from object entries.
        let mut obj_names = HashMap::new();
        let mut obj_meta = HashMap::new();
        for (en_name, entry) in &objects.translations {
            obj_names.insert(en_name.clone(), entry.name().to_string());
            let meta = entry.meta();
            if meta.gender.is_some() || meta.plural.is_some() || meta.genitive.is_some() {
                obj_meta.insert(en_name.clone(), meta);
            }
        }

        self.monster_names
            .insert(code.to_string(), monsters.translations);
        self.object_names.insert(code.to_string(), obj_names);
        if !obj_meta.is_empty() {
            self.object_meta.insert(code.to_string(), obj_meta);
        }
        Ok(())
    }

    /// Whether the current language has grammatical gender.
    pub fn has_gender(&self) -> bool {
        self.manifest().has_gender
    }

    /// Translate an object name and return its grammatical metadata.
    ///
    /// Returns `(translated_name, TranslationMeta)`. The meta contains
    /// gender, plural, and genitive forms when available.
    pub fn translate_object_name_with_meta<'a>(
        &'a self,
        en_name: &'a str,
    ) -> (&'a str, TranslationMeta) {
        let translated = self.translate_object_name(en_name);
        let meta = self.lookup_object_meta(en_name);
        (translated, meta)
    }

    fn lookup_object_meta(&self, en_name: &str) -> TranslationMeta {
        // Try current language.
        if let Some(meta) = self
            .object_meta
            .get(&self.current)
            .and_then(|map| map.get(en_name))
        {
            return meta.clone();
        }
        // Try fallback.
        if let Some(manifest) = self.manifests.get(&self.current)
            && let Some(ref fallback) = manifest.fallback
            && fallback != &self.current
            && let Some(meta) = self
                .object_meta
                .get(fallback.as_str())
                .and_then(|map| map.get(en_name))
        {
            return meta.clone();
        }
        TranslationMeta::default()
    }

    /// Load a CJK classifier from TOML.
    pub fn load_classifier(&mut self, toml_str: &str) -> Result<(), LocaleError> {
        let clf = Classifier::load_from_toml(toml_str).map_err(|e| {
            LocaleError::ParseError(format!("classifiers.toml: {}", e))
        })?;
        self.classifier = Some(clf);
        Ok(())
    }

    /// Translate a monster name from English to the current language.
    ///
    /// Lookup order: current language, then fallback, then returns `en_name`.
    pub fn translate_monster_name<'a>(&'a self, en_name: &'a str) -> &'a str {
        // Try current language.
        if let Some(translated) = self.monster_names.get(&self.current)
            .and_then(|map| map.get(en_name))
        {
            return translated;
        }
        // Try fallback from manifest.
        if let Some(manifest) = self.manifests.get(&self.current)
            && let Some(ref fallback) = manifest.fallback
            && fallback != &self.current
            && let Some(translated) = self.monster_names
                .get(fallback.as_str())
                .and_then(|map| map.get(en_name))
        {
            return translated;
        }
        en_name
    }

    /// Translate an object name from English to the current language.
    ///
    /// Lookup order: current language, then fallback, then returns `en_name`.
    pub fn translate_object_name<'a>(&'a self, en_name: &'a str) -> &'a str {
        // Try current language.
        if let Some(translated) = self.object_names.get(&self.current)
            .and_then(|map| map.get(en_name))
        {
            return translated;
        }
        // Try fallback from manifest.
        if let Some(manifest) = self.manifests.get(&self.current)
            && let Some(ref fallback) = manifest.fallback
            && fallback != &self.current
            && let Some(translated) = self.object_names
                .get(fallback.as_str())
                .and_then(|map| map.get(en_name))
        {
            return translated;
        }
        en_name
    }

    /// Access the loaded classifier, if any.
    pub fn classifier(&self) -> Option<&Classifier> {
        self.classifier.as_ref()
    }

    // ── private ──────────────────────────────────────────────────

    fn try_format(&self, lang: &str, msg_id: &str, args: Option<&FluentArgs>) -> Option<String> {
        let bundle = self.bundles.get(lang)?;
        let message = bundle.get_message(msg_id)?;
        let pattern = message.value()?;
        let mut errors = vec![];
        let result = bundle.format_pattern(pattern, args, &mut errors);
        Some(result.into_owned())
    }
}

impl Default for LocaleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during locale operations.
#[derive(Debug, Clone)]
pub enum LocaleError {
    NotFound(String),
    ParseError(String),
    MissingField(String),
}

impl std::fmt::Display for LocaleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocaleError::NotFound(code) => write!(f, "Language pack not found: {}", code),
            LocaleError::ParseError(msg) => write!(f, "Failed to parse FTL: {}", msg),
            LocaleError::MissingField(field) => write!(f, "Missing manifest field: {}", field),
        }
    }
}

impl std::error::Error for LocaleError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_english() {
        let locale = LocaleManager::new();
        assert_eq!(locale.current_language(), "en");
        assert!(locale.has_articles());
        assert!(locale.has_plural());
        assert!(!locale.is_cjk());
    }

    #[test]
    fn test_fallback_to_msg_id() {
        let locale = LocaleManager::new();
        assert_eq!(locale.translate("nonexistent-msg", None), "nonexistent-msg");
    }

    #[test]
    fn test_load_and_translate() {
        let mut locale = LocaleManager::new();
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
        let ftl = "hello = \u{4f60}\u{597d}\n";
        locale
            .load_locale("zh-CN", manifest, &[("messages.ftl", ftl)])
            .unwrap();
        locale.set_language("zh-CN").unwrap();
        assert_eq!(locale.translate("hello", None), "\u{4f60}\u{597d}");
        assert!(locale.is_cjk());
    }

    #[test]
    fn test_set_language_not_loaded() {
        let mut locale = LocaleManager::new();
        assert!(locale.set_language("fr").is_err());
    }

    fn setup_zh_cn_locale() -> LocaleManager {
        let mut locale = LocaleManager::new();
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
            .load_locale("zh-CN", manifest, &[])
            .unwrap();
        locale.set_language("zh-CN").unwrap();
        locale
    }

    #[test]
    fn test_translate_monster_name() {
        let mut locale = setup_zh_cn_locale();
        let monsters_toml = r#"
[translations]
"giant ant" = "巨蚁"
"killer bee" = "杀人蜂"
"goblin" = "哥布林"
"#;
        let objects_toml = "[translations]\n";
        locale
            .load_entity_translations("zh-CN", monsters_toml, objects_toml)
            .unwrap();

        assert_eq!(locale.translate_monster_name("giant ant"), "巨蚁");
        assert_eq!(locale.translate_monster_name("killer bee"), "杀人蜂");
        assert_eq!(locale.translate_monster_name("goblin"), "哥布林");
        // Unknown monster returns English name.
        assert_eq!(
            locale.translate_monster_name("unknown creature"),
            "unknown creature"
        );
    }

    #[test]
    fn test_translate_object_name() {
        let mut locale = setup_zh_cn_locale();
        let monsters_toml = "[translations]\n";
        let objects_toml = r#"
[translations]
"long sword" = "长剑"
"short sword" = "短剑"
"dagger" = "匕首"
"#;
        locale
            .load_entity_translations("zh-CN", monsters_toml, objects_toml)
            .unwrap();

        assert_eq!(locale.translate_object_name("long sword"), "长剑");
        assert_eq!(locale.translate_object_name("short sword"), "短剑");
        assert_eq!(locale.translate_object_name("dagger"), "匕首");
        // Unknown object returns English name.
        assert_eq!(
            locale.translate_object_name("mystery item"),
            "mystery item"
        );
    }

    #[test]
    fn test_translate_monster_name_english() {
        // When language is English, all names stay English.
        let locale = LocaleManager::new();
        assert_eq!(locale.translate_monster_name("giant ant"), "giant ant");
        assert_eq!(locale.translate_object_name("long sword"), "long sword");
    }

    #[test]
    fn test_ftl_loads_many_keys() {
        // Verify the locale system handles FTL files with 1000+ keys.
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
        // Generate a large FTL with 1200 keys.
        let mut ftl = String::new();
        for i in 0..1200 {
            ftl.push_str(&format!("key-{} = value {}\n", i, i));
        }
        locale
            .load_locale("en", manifest, &[("messages.ftl", &ftl)])
            .unwrap();
        locale.set_language("en").unwrap();

        assert_eq!(locale.translate("key-0", None), "value 0");
        assert_eq!(locale.translate("key-999", None), "value 999");
        assert_eq!(locale.translate("key-1199", None), "value 1199");
    }

    #[test]
    fn test_translate_with_args() {
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
        let ftl = "greeting = Hello, { $name }!\nitem-hit = { $attacker } hits { $defender }!\n";
        locale
            .load_locale("en", manifest, &[("messages.ftl", ftl)])
            .unwrap();
        locale.set_language("en").unwrap();

        let mut args = FluentArgs::new();
        args.set("name", "Player");
        assert_eq!(locale.translate("greeting", Some(&args)), "Hello, Player!");

        let mut args2 = FluentArgs::new();
        args2.set("attacker", "goblin");
        args2.set("defender", "you");
        assert_eq!(
            locale.translate("item-hit", Some(&args2)),
            "goblin hits you!"
        );
    }

    #[test]
    fn test_missing_key_returns_key_name() {
        let locale = LocaleManager::new();
        assert_eq!(
            locale.translate("this-key-does-not-exist", None),
            "this-key-does-not-exist"
        );
    }

    #[test]
    fn test_classifier_integration() {
        let mut locale = setup_zh_cn_locale();
        let clf_toml = r#"
default = "个"

[class]
weapon = "把"
potion = "瓶"

[name]
arrow = "支"
"#;
        locale.load_classifier(clf_toml).unwrap();

        let clf = locale.classifier().unwrap();
        assert_eq!(clf.get("weapon", "long sword"), "把");
        assert_eq!(clf.get("weapon", "arrow"), "支");
        assert_eq!(clf.get("potion", "healing"), "瓶");
        assert_eq!(clf.get("unknown", "thing"), "个");
    }
}
