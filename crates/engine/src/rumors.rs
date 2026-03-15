//! Game content delivery — rumors, epitaphs, oracle consultations,
//! bogus monster names, and random engravings.
//!
//! Content is loaded from plain text files in `data/content/` at startup.
//! Each module provides a random selection function.

use rand::Rng;

/// Loaded game content, stored in the game session.
#[derive(Debug, Clone, Default)]
pub struct GameContent {
    pub rumors_true: Vec<String>,
    pub rumors_false: Vec<String>,
    pub epitaphs: Vec<String>,
    pub bogusmon: Vec<String>,
    pub oracles: Vec<String>,
    pub engravings: Vec<String>,
}

impl GameContent {
    /// Load game content from a data directory path.
    ///
    /// Expects files in `{data_dir}/content/`:
    ///   rumors_true.txt, rumors_false.txt, epitaphs.txt,
    ///   bogusmon.txt, oracles.txt, engravings.txt
    pub fn load(data_dir: &std::path::Path) -> Self {
        let content_dir = data_dir.join("content");
        Self {
            rumors_true: load_lines(&content_dir.join("rumors_true.txt")),
            rumors_false: load_lines(&content_dir.join("rumors_false.txt")),
            epitaphs: load_lines(&content_dir.join("epitaphs.txt")),
            bogusmon: load_lines(&content_dir.join("bogusmon.txt")),
            oracles: load_sections(&content_dir.join("oracles.txt")),
            engravings: load_lines(&content_dir.join("engravings.txt")),
        }
    }

    /// Get a random rumor (true or false, weighted 2:1 toward true).
    pub fn random_rumor(&self, rng: &mut impl Rng) -> Option<&str> {
        let use_true = rng.random_range(0..3) != 0; // 2/3 true
        let source = if use_true {
            &self.rumors_true
        } else {
            &self.rumors_false
        };
        if source.is_empty() {
            return None;
        }
        let idx = rng.random_range(0..source.len());
        Some(&source[idx])
    }

    /// Get a random true rumor (for fortune cookies, blessed effect).
    pub fn random_true_rumor(&self, rng: &mut impl Rng) -> Option<&str> {
        if self.rumors_true.is_empty() {
            return None;
        }
        let idx = rng.random_range(0..self.rumors_true.len());
        Some(&self.rumors_true[idx])
    }

    /// Get a random false rumor (for cursed fortune cookies).
    pub fn random_false_rumor(&self, rng: &mut impl Rng) -> Option<&str> {
        if self.rumors_false.is_empty() {
            return None;
        }
        let idx = rng.random_range(0..self.rumors_false.len());
        Some(&self.rumors_false[idx])
    }

    /// Get a random epitaph for tombstones.
    pub fn random_epitaph(&self, rng: &mut impl Rng) -> Option<&str> {
        if self.epitaphs.is_empty() {
            return None;
        }
        let idx = rng.random_range(0..self.epitaphs.len());
        Some(&self.epitaphs[idx])
    }

    /// Get a random bogus monster name for hallucination.
    pub fn random_bogusmon(&self, rng: &mut impl Rng) -> Option<&str> {
        if self.bogusmon.is_empty() {
            return None;
        }
        let idx = rng.random_range(0..self.bogusmon.len());
        Some(&self.bogusmon[idx])
    }

    /// Get an oracle consultation by index (cycled).
    pub fn oracle_consultation(&self, index: usize) -> Option<&str> {
        if self.oracles.is_empty() {
            return None;
        }
        Some(&self.oracles[index % self.oracles.len()])
    }

    /// Get a random engraving for dungeon generation.
    pub fn random_engraving(&self, rng: &mut impl Rng) -> Option<&str> {
        if self.engravings.is_empty() {
            return None;
        }
        let idx = rng.random_range(0..self.engravings.len());
        Some(&self.engravings[idx])
    }
}

/// Load a text file as a vector of non-empty, trimmed lines.
fn load_lines(path: &std::path::Path) -> Vec<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => content
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Load a text file split by `---` separators into sections.
fn load_sections(path: &std::path::Path) -> Vec<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => content
            .split("\n---\n")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_lines_empty() {
        let lines = load_lines(std::path::Path::new("/nonexistent/path"));
        assert!(lines.is_empty());
    }

    #[test]
    fn test_game_content_default() {
        let content = GameContent::default();
        assert!(content.rumors_true.is_empty());
        assert!(content.rumors_false.is_empty());
        assert!(content.epitaphs.is_empty());
        assert!(content.bogusmon.is_empty());
        assert!(content.oracles.is_empty());
        assert!(content.engravings.is_empty());
    }

    #[test]
    fn test_random_from_empty() {
        let content = GameContent::default();
        let mut rng = rand::rng();
        assert!(content.random_rumor(&mut rng).is_none());
        assert!(content.random_epitaph(&mut rng).is_none());
        assert!(content.random_bogusmon(&mut rng).is_none());
        assert!(content.oracle_consultation(0).is_none());
        assert!(content.random_engraving(&mut rng).is_none());
    }

    #[test]
    fn test_random_from_populated() {
        let content = GameContent {
            rumors_true: vec!["true rumor".to_string()],
            rumors_false: vec!["false rumor".to_string()],
            epitaphs: vec!["RIP".to_string()],
            bogusmon: vec!["bogus".to_string()],
            oracles: vec!["oracle says".to_string()],
            engravings: vec!["engraving".to_string()],
        };
        let mut rng = rand::rng();
        assert!(content.random_rumor(&mut rng).is_some());
        assert!(content.random_epitaph(&mut rng).is_some());
        assert!(content.random_bogusmon(&mut rng).is_some());
        assert_eq!(content.oracle_consultation(0), Some("oracle says"));
        assert!(content.random_engraving(&mut rng).is_some());
    }

    #[test]
    fn test_oracle_cycling() {
        let content = GameContent {
            oracles: vec!["first".to_string(), "second".to_string()],
            ..Default::default()
        };
        assert_eq!(content.oracle_consultation(0), Some("first"));
        assert_eq!(content.oracle_consultation(1), Some("second"));
        assert_eq!(content.oracle_consultation(2), Some("first")); // wraps
    }

    #[test]
    fn test_content_loading() {
        // Integration test: load from actual data directory if available.
        let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data");
        if data_dir.join("content").exists() {
            let content = GameContent::load(&data_dir);
            assert!(
                !content.rumors_true.is_empty(),
                "Should load at least one true rumor"
            );
            assert!(
                !content.rumors_false.is_empty(),
                "Should load at least one false rumor"
            );
            assert!(
                !content.epitaphs.is_empty(),
                "Should load at least one epitaph"
            );
            assert!(
                !content.bogusmon.is_empty(),
                "Should load at least one bogus monster"
            );
            assert!(
                !content.oracles.is_empty(),
                "Should load at least one oracle"
            );
        }
    }
}
