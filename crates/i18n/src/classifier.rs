use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Chinese counter word (量词) system.
///
/// Maps object classes and specific object names to the appropriate
/// classifier/measure word used for counting in CJK languages.
#[derive(Debug, Clone)]
pub struct Classifier {
    /// Specific object name overrides (checked first).
    name_map: HashMap<String, String>,
    /// Object class defaults (checked second).
    class_map: HashMap<String, String>,
    /// Fallback classifier if nothing matches.
    default: String,
}

/// TOML representation for deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClassifierToml {
    #[serde(default = "default_classifier_str")]
    default: String,
    #[serde(default)]
    class: HashMap<String, String>,
    #[serde(default)]
    name: HashMap<String, String>,
}

fn default_classifier_str() -> String {
    "\u{4e2a}".to_string() // 个
}

impl Classifier {
    /// Create a Classifier with built-in Chinese defaults.
    pub fn new() -> Self {
        let mut class_map = HashMap::new();
        // Weapon subtypes are handled in name_map; general weapon class.
        class_map.insert("weapon".to_string(), "\u{628a}".to_string()); // 把
        class_map.insert("potion".to_string(), "\u{74f6}".to_string()); // 瓶
        class_map.insert("scroll".to_string(), "\u{5f20}".to_string()); // 张
        class_map.insert("ring".to_string(), "\u{679a}".to_string()); // 枚
        class_map.insert("amulet".to_string(), "\u{4e2a}".to_string()); // 个
        class_map.insert("armor".to_string(), "\u{4ef6}".to_string()); // 件
        class_map.insert("food".to_string(), "\u{4e2a}".to_string()); // 个
        class_map.insert("tool".to_string(), "\u{4e2a}".to_string()); // 个
        class_map.insert("coin".to_string(), "\u{679a}".to_string()); // 枚
        class_map.insert("gem".to_string(), "\u{9897}".to_string()); // 颗
        class_map.insert("rock".to_string(), "\u{5757}".to_string()); // 块
        class_map.insert("wand".to_string(), "\u{652f}".to_string()); // 支
        class_map.insert("spellbook".to_string(), "\u{672c}".to_string()); // 本

        let mut name_map = HashMap::new();
        // Arrows, darts, and other projectiles use 支.
        name_map.insert("arrow".to_string(), "\u{652f}".to_string()); // 支
        name_map.insert("elven arrow".to_string(), "\u{652f}".to_string());
        name_map.insert("orcish arrow".to_string(), "\u{652f}".to_string());
        name_map.insert("silver arrow".to_string(), "\u{652f}".to_string());
        name_map.insert("ya".to_string(), "\u{652f}".to_string());
        name_map.insert("dart".to_string(), "\u{652f}".to_string());
        name_map.insert("shuriken".to_string(), "\u{679a}".to_string()); // 枚

        // Bows use 张 (sheet/flat object).
        name_map.insert("bow".to_string(), "\u{5f20}".to_string()); // 张
        name_map.insert("elven bow".to_string(), "\u{5f20}".to_string());
        name_map.insert("orcish bow".to_string(), "\u{5f20}".to_string());
        name_map.insert("yumi".to_string(), "\u{5f20}".to_string());
        name_map.insert("crossbow".to_string(), "\u{5f20}".to_string());

        // Shields also use 面.
        name_map.insert("shield".to_string(), "\u{9762}".to_string()); // 面
        name_map.insert("small shield".to_string(), "\u{9762}".to_string());
        name_map.insert("large shield".to_string(), "\u{9762}".to_string());
        name_map.insert("shield of reflection".to_string(), "\u{9762}".to_string());

        // Boots use 双 (pair).
        name_map.insert("boots".to_string(), "\u{53cc}".to_string()); // 双
        name_map.insert("elven boots".to_string(), "\u{53cc}".to_string());
        name_map.insert("iron shoes".to_string(), "\u{53cc}".to_string());
        name_map.insert("speed boots".to_string(), "\u{53cc}".to_string());
        name_map.insert("water walking boots".to_string(), "\u{53cc}".to_string());
        name_map.insert("jumping boots".to_string(), "\u{53cc}".to_string());
        name_map.insert("levitation boots".to_string(), "\u{53cc}".to_string());
        name_map.insert("fumble boots".to_string(), "\u{53cc}".to_string());

        // Gloves use 双.
        name_map.insert("gloves".to_string(), "\u{53cc}".to_string());
        name_map.insert("gauntlets".to_string(), "\u{53cc}".to_string());
        name_map.insert("gauntlets of power".to_string(), "\u{53cc}".to_string());
        name_map.insert("gauntlets of dexterity".to_string(), "\u{53cc}".to_string());
        name_map.insert("gauntlets of fumbling".to_string(), "\u{53cc}".to_string());

        Self {
            name_map,
            class_map,
            default: "\u{4e2a}".to_string(), // 个
        }
    }

    /// Load classifier mappings from a TOML string.
    ///
    /// Expected format:
    /// ```toml
    /// default = "个"
    ///
    /// [class]
    /// weapon = "把"
    /// potion = "瓶"
    ///
    /// [name]
    /// arrow = "支"
    /// bow = "张"
    /// ```
    pub fn load_from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        let parsed: ClassifierToml = toml::from_str(toml_str)?;
        Ok(Self {
            name_map: parsed.name,
            class_map: parsed.class,
            default: parsed.default,
        })
    }

    /// Get the classifier for an object, checking specific name first, then
    /// class, then falling back to the default.
    pub fn get(&self, object_class: &str, object_name: &str) -> &str {
        if let Some(clf) = self.name_map.get(object_name) {
            return clf;
        }
        if let Some(clf) = self.class_map.get(object_class) {
            return clf;
        }
        &self.default
    }
}

impl Default for Classifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let clf = Classifier::new();
        assert_eq!(clf.get("weapon", "long sword"), "\u{628a}"); // 把
        assert_eq!(clf.get("weapon", "arrow"), "\u{652f}"); // 支 (name override)
        assert_eq!(clf.get("weapon", "bow"), "\u{5f20}"); // 张 (name override)
        assert_eq!(clf.get("potion", "potion of healing"), "\u{74f6}"); // 瓶
        assert_eq!(clf.get("scroll", "scroll of identify"), "\u{5f20}"); // 张
        assert_eq!(clf.get("ring", "ring of protection"), "\u{679a}"); // 枚
        assert_eq!(clf.get("armor", "plate mail"), "\u{4ef6}"); // 件
        assert_eq!(clf.get("coin", "gold piece"), "\u{679a}"); // 枚
        assert_eq!(clf.get("unknown", "mystery thing"), "\u{4e2a}"); // 个 (default)
    }

    #[test]
    fn test_load_from_toml() {
        let toml_str = r#"
default = "\u4e2a"

[class]
weapon = "\u628a"
potion = "\u74f6"

[name]
arrow = "\u652f"
"#;
        let clf = Classifier::load_from_toml(toml_str).unwrap();
        assert_eq!(clf.get("weapon", "arrow"), "\u{652f}");
        assert_eq!(clf.get("weapon", "long sword"), "\u{628a}");
        assert_eq!(clf.get("potion", "healing potion"), "\u{74f6}");
        assert_eq!(clf.get("food", "apple"), "\u{4e2a}");
    }

    #[test]
    fn test_boots_pair() {
        let clf = Classifier::new();
        assert_eq!(clf.get("armor", "speed boots"), "\u{53cc}"); // 双 (name override)
        assert_eq!(clf.get("armor", "plate mail"), "\u{4ef6}"); // 件 (class default)
    }
}
