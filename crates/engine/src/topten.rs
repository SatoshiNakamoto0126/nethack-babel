//! High score table and XLOGFILE format.
//!
//! Implements the NetHack top-ten list and extended log file format from
//! `topten.c`.  The XLOGFILE format is a tab-separated record written at
//! the end of each game, containing every detail needed by external score
//! trackers (NetHack scoreboard, Junethack, etc.).
//!
//! All functions are pure data transformations.  Actual file IO is handled
//! by the caller (TUI or persistence crate).

use serde::{Deserialize, Serialize};

use crate::conduct::{Conduct, ConductState};
use crate::end::EndHow;
use crate::role::Role;

// ---------------------------------------------------------------------------
// Top-ten entry
// ---------------------------------------------------------------------------

/// A single entry in the high score table.
///
/// Mirrors `struct toptenentry` from C NetHack's `topten.c`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopTenEntry {
    /// Final score (points).
    pub points: i64,
    /// Dungeon number where death occurred.
    pub death_dnum: i32,
    /// Level within dungeon where death occurred.
    pub death_lev: i32,
    /// Maximum dungeon level reached.
    pub max_lvl: i32,
    /// HP at death.
    pub hp: i32,
    /// Max HP at death.
    pub max_hp: i32,
    /// Number of previous deaths (bones encounters, etc.).
    pub deaths: i32,
    /// Death date (YYYYMMDD format).
    pub death_date: i64,
    /// Birth date (YYYYMMDD format).
    pub birth_date: i64,
    /// Player name (max 10 chars in C, unlimited here).
    pub name: String,
    /// Death reason string.
    pub death: String,
    /// Role code (e.g. "Val", "Wiz").
    pub role: String,
    /// Race code (e.g. "Hum", "Elf").
    pub race: String,
    /// Gender code ("Mal" or "Fem").
    pub gender: String,
    /// Alignment code ("Law", "Neu", "Cha").
    pub alignment: String,
}

impl TopTenEntry {
    /// Format this entry as a single-line record file entry.
    ///
    /// Matches the format written by `writeentry()` in C NetHack:
    /// `version points dnum lev maxlvl hp maxhp deaths deathdate birthdate uid role race gender align name,death`
    pub fn to_record_line(&self, version: &str, uid: i32) -> String {
        format!(
            "{} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {},{}",
            version,
            self.points,
            self.death_dnum,
            self.death_lev,
            self.max_lvl,
            self.hp,
            self.max_hp,
            self.deaths,
            self.death_date,
            self.birth_date,
            uid,
            self.role,
            self.race,
            self.gender,
            self.alignment,
            self.name,
            self.death,
        )
    }
}

// ---------------------------------------------------------------------------
// XLOGFILE entry
// ---------------------------------------------------------------------------

/// Extended log file entry containing all game details.
///
/// The XLOGFILE format uses tab-separated `key=value` pairs.
/// This is the format used by NetHack scoreboard sites and
/// analytics tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XlogEntry {
    /// Game version string (e.g. "0.1.0").
    pub version: String,
    /// Final score.
    pub points: i64,
    /// Dungeon number where death occurred.
    pub death_dnum: i32,
    /// Level within dungeon where death occurred.
    pub death_lev: i32,
    /// Maximum dungeon level reached.
    pub max_lvl: i32,
    /// HP at death.
    pub hp: i32,
    /// Max HP at death.
    pub max_hp: i32,
    /// Number of deaths (bones encounters).
    pub deaths: i32,
    /// Death date (YYYYMMDD).
    pub death_date: i64,
    /// Birth date (YYYYMMDD).
    pub birth_date: i64,
    /// Player name.
    pub name: String,
    /// Death reason.
    pub death: String,
    /// Role code.
    pub role: String,
    /// Race code.
    pub race: String,
    /// Gender code.
    pub gender: String,
    /// Alignment code.
    pub alignment: String,
    /// How the game ended (numeric code matching EndHow).
    pub how: i32,
    /// Conduct bitmask (see `encode_conduct`).
    pub conduct: i64,
    /// Total game turns.
    pub turns: u32,
    /// Achievement bitmask.
    pub achieve: i64,
    /// Real wall-clock time in seconds.
    pub realtime: i64,
    /// Starting gender code.
    pub gender0: String,
    /// Starting alignment code.
    pub align0: String,
    /// Game flags bitmask (wizard, discover, etc.).
    pub flags: i64,
    /// Gold carried at end of game.
    pub gold: i64,
    /// Number of wishes made.
    pub wish_cnt: i64,
    /// Number of artifact wishes.
    pub arti_wish_cnt: i64,
    /// Number of bones encounters.
    pub bones: i64,
}

impl XlogEntry {
    /// Format this entry as a tab-separated XLOGFILE line.
    ///
    /// Matches the format written by `writexlentry()` in C NetHack.
    pub fn to_xlog_line(&self) -> String {
        let mut parts = Vec::new();
        parts.push(format!("version={}", self.version));
        parts.push(format!("points={}", self.points));
        parts.push(format!("deathdnum={}", self.death_dnum));
        parts.push(format!("deathlev={}", self.death_lev));
        parts.push(format!("maxlvl={}", self.max_lvl));
        parts.push(format!("hp={}", self.hp));
        parts.push(format!("maxhp={}", self.max_hp));
        parts.push(format!("deaths={}", self.deaths));
        parts.push(format!("deathdate={}", self.death_date));
        parts.push(format!("birthdate={}", self.birth_date));
        parts.push(format!("role={}", self.role));
        parts.push(format!("race={}", self.race));
        parts.push(format!("gender={}", self.gender));
        parts.push(format!("align={}", self.alignment));
        parts.push(format!("name={}", self.name));
        parts.push(format!("death={}", self.death));
        parts.push(format!("conduct=0x{:x}", self.conduct));
        parts.push(format!("turns={}", self.turns));
        parts.push(format!("achieve=0x{:x}", self.achieve));
        parts.push(format!("realtime={}", self.realtime));
        parts.push(format!("gender0={}", self.gender0));
        parts.push(format!("align0={}", self.align0));
        parts.push(format!("flags=0x{:x}", self.flags));
        parts.push(format!("gold={}", self.gold));
        parts.push(format!("wish_cnt={}", self.wish_cnt));
        parts.push(format!("arti_wish_cnt={}", self.arti_wish_cnt));
        parts.push(format!("bones={}", self.bones));
        parts.join("\t")
    }

    /// Parse an XLOGFILE line into an XlogEntry.
    ///
    /// The format is tab-separated `key=value` pairs.
    pub fn from_xlog_line(line: &str) -> Option<Self> {
        let mut entry = XlogEntry {
            version: String::new(),
            points: 0,
            death_dnum: 0,
            death_lev: 0,
            max_lvl: 0,
            hp: 0,
            max_hp: 0,
            deaths: 0,
            death_date: 0,
            birth_date: 0,
            name: String::new(),
            death: String::new(),
            role: String::new(),
            race: String::new(),
            gender: String::new(),
            alignment: String::new(),
            how: 0,
            conduct: 0,
            turns: 0,
            achieve: 0,
            realtime: 0,
            gender0: String::new(),
            align0: String::new(),
            flags: 0,
            gold: 0,
            wish_cnt: 0,
            arti_wish_cnt: 0,
            bones: 0,
        };

        for field in line.split('\t') {
            let (key, value) = field.split_once('=')?;
            match key {
                "version" => entry.version = value.to_string(),
                "points" => entry.points = value.parse().ok()?,
                "deathdnum" => entry.death_dnum = value.parse().ok()?,
                "deathlev" => entry.death_lev = value.parse().ok()?,
                "maxlvl" => entry.max_lvl = value.parse().ok()?,
                "hp" => entry.hp = value.parse().ok()?,
                "maxhp" => entry.max_hp = value.parse().ok()?,
                "deaths" => entry.deaths = value.parse().ok()?,
                "deathdate" => entry.death_date = value.parse().ok()?,
                "birthdate" => entry.birth_date = value.parse().ok()?,
                "name" => entry.name = value.to_string(),
                "death" => entry.death = value.to_string(),
                "role" => entry.role = value.to_string(),
                "race" => entry.race = value.to_string(),
                "gender" => entry.gender = value.to_string(),
                "align" => entry.alignment = value.to_string(),
                "conduct" => {
                    entry.conduct = parse_hex_or_dec(value)?;
                }
                "turns" => entry.turns = value.parse().ok()?,
                "achieve" => {
                    entry.achieve = parse_hex_or_dec(value)?;
                }
                "realtime" => entry.realtime = value.parse().ok()?,
                "gender0" => entry.gender0 = value.to_string(),
                "align0" => entry.align0 = value.to_string(),
                "flags" => {
                    entry.flags = parse_hex_or_dec(value)?;
                }
                "gold" => entry.gold = value.parse().ok()?,
                "wish_cnt" => entry.wish_cnt = value.parse().ok()?,
                "arti_wish_cnt" => entry.arti_wish_cnt = value.parse().ok()?,
                "bones" => entry.bones = value.parse().ok()?,
                _ => {} // ignore unknown fields for forward compat
            }
        }

        Some(entry)
    }
}

/// Parse a string that might be "0x..." hex or plain decimal.
fn parse_hex_or_dec(s: &str) -> Option<i64> {
    if let Some(hex) = s.strip_prefix("0x") {
        i64::from_str_radix(hex, 16).ok()
    } else {
        s.parse().ok()
    }
}

// ---------------------------------------------------------------------------
// Conduct encoding
// ---------------------------------------------------------------------------

/// Encode conduct state as a bitmask matching C NetHack's `encodeconduct()`.
///
/// Each bit is 1 if the conduct was **maintained** (NOT violated).
///
/// Bit layout:
/// - 0: foodless
/// - 1: vegan
/// - 2: vegetarian
/// - 3: atheist
/// - 4: weaponless
/// - 5: pacifist
/// - 6: illiterate
/// - 7: polypileless
/// - 8: polyselfless
/// - 9: wishless
/// - 10: artifact-wishless
/// - 11: genocideless
/// - 13: petless (bit 12 is sokoban, skipped for simplicity)
pub fn encode_conduct(conducts: &ConductState) -> i64 {
    let mut e: i64 = 0;
    if conducts.is_maintained(Conduct::Foodless) {
        e |= 1 << 0;
    }
    if conducts.is_maintained(Conduct::Vegan) {
        e |= 1 << 1;
    }
    if conducts.is_maintained(Conduct::Vegetarian) {
        e |= 1 << 2;
    }
    if conducts.is_maintained(Conduct::Atheist) {
        e |= 1 << 3;
    }
    if conducts.is_maintained(Conduct::Weaponless) {
        e |= 1 << 4;
    }
    if conducts.is_maintained(Conduct::Pacifist) {
        e |= 1 << 5;
    }
    if conducts.is_maintained(Conduct::Illiterate) {
        e |= 1 << 6;
    }
    if conducts.is_maintained(Conduct::Polypileless) {
        e |= 1 << 7;
    }
    if conducts.is_maintained(Conduct::Polyselfless) {
        e |= 1 << 8;
    }
    if conducts.is_maintained(Conduct::Wishless) {
        e |= 1 << 9;
    }
    if conducts.is_maintained(Conduct::ArtifactWishless) {
        e |= 1 << 10;
    }
    if conducts.is_maintained(Conduct::Genocideless) {
        e |= 1 << 11;
    }
    // Bit 12: sokoban (not tracked yet, skip)
    if conducts.is_maintained(Conduct::Petless) {
        e |= 1 << 13;
    }
    e
}

/// Decode a conduct bitmask back to a list of maintained conducts.
pub fn decode_conduct(mask: i64) -> Vec<Conduct> {
    let mut result = Vec::new();
    let mapping: &[(i32, Conduct)] = &[
        (0, Conduct::Foodless),
        (1, Conduct::Vegan),
        (2, Conduct::Vegetarian),
        (3, Conduct::Atheist),
        (4, Conduct::Weaponless),
        (5, Conduct::Pacifist),
        (6, Conduct::Illiterate),
        (7, Conduct::Polypileless),
        (8, Conduct::Polyselfless),
        (9, Conduct::Wishless),
        (10, Conduct::ArtifactWishless),
        (11, Conduct::Genocideless),
        (13, Conduct::Petless),
    ];
    for &(bit, conduct) in mapping {
        if mask & (1i64 << bit) != 0 {
            result.push(conduct);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Game flags encoding
// ---------------------------------------------------------------------------

/// Encode game flags as a bitmask matching C NetHack's `encodexlogflags()`.
///
/// - Bit 0: wizard mode
/// - Bit 1: discover (explore) mode
/// - Bit 2: no bones encountered
/// - Bit 3: character was rerolled
pub fn encode_flags(wizard: bool, discover: bool, no_bones: bool, rerolled: bool) -> i64 {
    let mut e: i64 = 0;
    if wizard {
        e |= 1 << 0;
    }
    if discover {
        e |= 1 << 1;
    }
    if no_bones {
        e |= 1 << 2;
    }
    if rerolled {
        e |= 1 << 3;
    }
    e
}

// ---------------------------------------------------------------------------
// Killer format
// ---------------------------------------------------------------------------

/// Prefix used before the killer name, matching C `killed_by_prefix[]`.
pub fn killed_by_prefix(how: &EndHow) -> &'static str {
    match how {
        EndHow::Died => "killed by ",
        EndHow::Choked => "choked on ",
        EndHow::Poisoned => "poisoned by ",
        EndHow::Starvation => "died of ",
        EndHow::Drowning => "drowned in ",
        EndHow::Burning => "burned by ",
        EndHow::Dissolved => "dissolved in ",
        EndHow::Crushed => "crushed to death by ",
        EndHow::Stoning => "petrified by ",
        EndHow::Slimed => "turned to slime by ",
        EndHow::Genocided => "killed by ",
        EndHow::Panicked | EndHow::Trickery | EndHow::Quit | EndHow::Escaped | EndHow::Ascended => {
            ""
        }
    }
}

/// Format a full death reason string ("killed by a gnome", "ascended", etc.).
///
/// Mirrors `formatkiller()` from C NetHack, but without the mutable buffer
/// gymnastics.
pub fn format_killer(how: &EndHow, killer_name: &str) -> String {
    let prefix = killed_by_prefix(how);
    if prefix.is_empty() {
        // Non-death endings use the ending phrase directly.
        how.death_description().to_string()
    } else {
        format!("{}{}", prefix, sanitize_killer_name(killer_name))
    }
}

/// Sanitize a killer name for record/xlogfile output.
///
/// Replaces commas with semicolons, equals with underscores, and tabs
/// with spaces (matching C NetHack's formatkiller).
fn sanitize_killer_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            ',' => ';',
            '=' => '_',
            '\t' => ' ',
            _ => c,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Top-ten list management
// ---------------------------------------------------------------------------

/// A managed top-ten list with configurable maximum size.
///
/// Entries are kept sorted by score descending.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopTenList {
    entries: Vec<TopTenEntry>,
    /// Maximum number of entries to keep (default: 100).
    max_entries: usize,
}

impl TopTenList {
    /// Create an empty top-ten list with the given max size.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Add an entry to the list, maintaining sorted order and max size.
    /// Returns the rank (1-based) of the inserted entry, or None if it
    /// didn't make the cut.
    pub fn add_entry(&mut self, entry: TopTenEntry) -> Option<usize> {
        let rank = self.entries.partition_point(|e| e.points >= entry.points);

        // If full and would be placed beyond the end, it doesn't make it.
        if rank >= self.max_entries {
            return None;
        }

        self.entries.insert(rank, entry);

        // Trim excess entries.
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }

        Some(rank + 1) // 1-based rank
    }

    /// Get the top `n` entries.
    pub fn top(&self, n: usize) -> &[TopTenEntry] {
        let end = n.min(self.entries.len());
        &self.entries[..end]
    }

    /// All entries.
    pub fn all(&self) -> &[TopTenEntry] {
        &self.entries
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check if a given score would make it into the list.
    pub fn would_qualify(&self, points: i64) -> bool {
        if self.entries.len() < self.max_entries {
            return true;
        }
        if let Some(last) = self.entries.last() {
            points > last.points
        } else {
            true
        }
    }

    /// Find all entries matching a given player name.
    pub fn entries_for_player(&self, name: &str) -> Vec<(usize, &TopTenEntry)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.name == name)
            .map(|(i, e)| (i + 1, e)) // 1-based rank
            .collect()
    }

    /// Find all entries matching a given role code.
    pub fn entries_for_role(&self, role: &str) -> Vec<(usize, &TopTenEntry)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.role == role)
            .map(|(i, e)| (i + 1, e))
            .collect()
    }
}

impl Default for TopTenList {
    fn default() -> Self {
        Self::new(100)
    }
}

// ---------------------------------------------------------------------------
// Leaderboard with JSON persistence
// ---------------------------------------------------------------------------

/// A leaderboard that persists high scores to a JSON file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Leaderboard {
    pub entries: Vec<LeaderboardEntry>,
}

/// A single leaderboard entry with display-friendly fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: u32,
    pub score: i64,
    pub player_name: String,
    pub role: String,
    pub race: String,
    pub gender: String,
    pub alignment: String,
    pub death_cause: String,
    pub dungeon_level: String,
    pub experience_level: u32,
    pub turns: u64,
    pub timestamp: String,
}

impl Leaderboard {
    /// Load leaderboard from a JSON file, or return empty if not found.
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save leaderboard to a JSON file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize: {}", e))?;
        std::fs::write(path, json).map_err(|e| format!("Failed to write: {}", e))
    }

    /// Add a new entry and keep only the top 100.
    pub fn add_entry(&mut self, entry: LeaderboardEntry) {
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.score.cmp(&a.score));
        self.entries.truncate(100);
        for (i, e) in self.entries.iter_mut().enumerate() {
            e.rank = (i + 1) as u32;
        }
    }

    /// Format top N entries for display.
    pub fn format_top(&self, n: usize) -> Vec<String> {
        let mut lines = vec![" No  Points     Name".to_string()];
        for entry in self.entries.iter().take(n) {
            lines.push(format!(
                "{:>3}. {:>10}  {} the {} ({} {} {}), {} on {}",
                entry.rank,
                entry.score,
                entry.player_name,
                entry.role,
                entry.gender,
                entry.race,
                entry.alignment,
                entry.death_cause,
                entry.dungeon_level,
            ));
        }
        lines
    }

    /// Get the player's best rank if they are on the leaderboard.
    pub fn player_rank(&self, player_name: &str) -> Option<u32> {
        self.entries
            .iter()
            .find(|e| e.player_name == player_name)
            .map(|e| e.rank)
    }
}

/// Default leaderboard file path.
pub fn default_leaderboard_path() -> std::path::PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push("nethack-babel");
    path.push("topten.json");
    path
}

// ---------------------------------------------------------------------------
// Display formatting
// ---------------------------------------------------------------------------

/// Format a top-ten entry for display, matching C NetHack's `outentry()`.
///
/// Format: ` N  1234567  Name-Role-Race-Gen-Ali  died on level 5, killed by a gnome`
pub fn format_entry(rank: usize, entry: &TopTenEntry, highlight: bool) -> String {
    let prefix = if highlight { " *" } else { "  " };
    format!(
        "{}{:>3} {:>9}  {}-{}-{}-{}-{}  {}",
        prefix,
        rank,
        entry.points,
        entry.name,
        entry.role,
        entry.race,
        entry.gender,
        entry.alignment,
        entry.death,
    )
}

/// Format the scoreboard header line.
pub fn format_header() -> String {
    "       Pts  Name".to_string()
}

/// Role short code from a Role enum.
pub fn role_code(role: Role) -> &'static str {
    match role {
        Role::Archeologist => "Arc",
        Role::Barbarian => "Bar",
        Role::Caveperson => "Cav",
        Role::Healer => "Hea",
        Role::Knight => "Kni",
        Role::Monk => "Mon",
        Role::Priest => "Pri",
        Role::Ranger => "Ran",
        Role::Rogue => "Rog",
        Role::Samurai => "Sam",
        Role::Tourist => "Tou",
        Role::Valkyrie => "Val",
        Role::Wizard => "Wiz",
    }
}

/// Role from a short code string.
pub fn role_from_code(code: &str) -> Option<Role> {
    match code {
        "Arc" => Some(Role::Archeologist),
        "Bar" => Some(Role::Barbarian),
        "Cav" => Some(Role::Caveperson),
        "Hea" => Some(Role::Healer),
        "Kni" => Some(Role::Knight),
        "Mon" => Some(Role::Monk),
        "Pri" => Some(Role::Priest),
        "Ran" => Some(Role::Ranger),
        "Rog" => Some(Role::Rogue),
        "Sam" => Some(Role::Samurai),
        "Tou" => Some(Role::Tourist),
        "Val" => Some(Role::Valkyrie),
        "Wiz" => Some(Role::Wizard),
        _ => None,
    }
}

/// Alignment display code from alignment string.
pub fn alignment_code(alignment: &str) -> &'static str {
    match alignment.to_lowercase().as_str() {
        "lawful" | "law" => "Law",
        "neutral" | "neu" => "Neu",
        "chaotic" | "cha" => "Cha",
        _ => "???",
    }
}

// ---------------------------------------------------------------------------
// Observable depth
// ---------------------------------------------------------------------------

/// Calculate the display depth for a level, matching C's `observable_depth()`.
///
/// For endgame levels, returns negative values:
/// - Astral: -5
/// - Water: -4
/// - Fire: -3
/// - Air: -2
/// - Earth: -1
///
/// For regular levels, returns the dungeon level number.
pub fn observable_depth(depth: i32, is_endgame: bool, endgame_level: Option<&str>) -> i32 {
    if is_endgame {
        match endgame_level {
            Some("astral") => -5,
            Some("water") => -4,
            Some("fire") => -3,
            Some("air") => -2,
            Some("earth") => -1,
            _ => 0,
        }
    } else {
        depth
    }
}

// ---------------------------------------------------------------------------
// Score filtering
// ---------------------------------------------------------------------------

/// Filter criteria for viewing scores.
#[derive(Debug, Clone, Default)]
pub struct ScoreFilter {
    /// Show only entries for this player name.
    pub player_name: Option<String>,
    /// Show only entries for this role code.
    pub role_code: Option<String>,
    /// Show only entries for this race code.
    pub race_code: Option<String>,
    /// Maximum number of entries to show.
    pub max_entries: Option<usize>,
    /// Show only the current player's entry and surrounding entries.
    pub around_rank: Option<usize>,
    /// Number of entries to show above and below the target rank.
    pub around_count: usize,
}

/// Apply a filter to a top-ten list and return matching entries with ranks.
pub fn filter_scores<'a>(
    list: &'a TopTenList,
    filter: &ScoreFilter,
) -> Vec<(usize, &'a TopTenEntry)> {
    let mut results: Vec<(usize, &TopTenEntry)> = list
        .all()
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            if let Some(ref name) = filter.player_name {
                if e.name != *name {
                    return false;
                }
            }
            if let Some(ref role) = filter.role_code {
                if e.role != *role {
                    return false;
                }
            }
            if let Some(ref race) = filter.race_code {
                if e.race != *race {
                    return false;
                }
            }
            true
        })
        .map(|(i, e)| (i + 1, e))
        .collect();

    // If showing around a rank, filter to nearby entries.
    if let Some(target) = filter.around_rank {
        let count = filter.around_count;
        let start = target.saturating_sub(count);
        let end = target + count;
        results.retain(|(rank, _)| *rank >= start && *rank <= end);
    }

    // Apply max entries limit.
    if let Some(max) = filter.max_entries {
        results.truncate(max);
    }

    results
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conduct::ConductState;

    // ── XLOGFILE format ──────────────────────────────────────────

    #[test]
    fn test_xlog_roundtrip() {
        let entry = XlogEntry {
            version: "0.1.0".to_string(),
            points: 12345,
            death_dnum: 0,
            death_lev: 5,
            max_lvl: 8,
            hp: -3,
            max_hp: 42,
            deaths: 0,
            death_date: 20260314,
            birth_date: 20260314,
            name: "TestPlayer".to_string(),
            death: "killed by a gnome".to_string(),
            role: "Wiz".to_string(),
            race: "Elf".to_string(),
            gender: "Fem".to_string(),
            alignment: "Cha".to_string(),
            how: 0,
            conduct: 0x3fff,
            turns: 500,
            achieve: 0,
            realtime: 3600,
            gender0: "Fem".to_string(),
            align0: "Cha".to_string(),
            flags: 0,
            gold: 200,
            wish_cnt: 0,
            arti_wish_cnt: 0,
            bones: 0,
        };

        let line = entry.to_xlog_line();
        assert!(line.contains("version=0.1.0"));
        assert!(line.contains("points=12345"));
        assert!(line.contains("name=TestPlayer"));
        assert!(line.contains("death=killed by a gnome"));
        assert!(line.contains("conduct=0x3fff"));

        // Parse it back.
        let parsed = XlogEntry::from_xlog_line(&line).unwrap();
        assert_eq!(parsed.version, "0.1.0");
        assert_eq!(parsed.points, 12345);
        assert_eq!(parsed.name, "TestPlayer");
        assert_eq!(parsed.death, "killed by a gnome");
        assert_eq!(parsed.conduct, 0x3fff);
        assert_eq!(parsed.turns, 500);
        assert_eq!(parsed.gold, 200);
    }

    #[test]
    fn test_xlog_parse_unknown_fields() {
        let line = "version=0.1.0\tpoints=100\tname=Test\tdeath=quit\tfuture_field=hello";
        let parsed = XlogEntry::from_xlog_line(line).unwrap();
        assert_eq!(parsed.version, "0.1.0");
        assert_eq!(parsed.points, 100);
        assert_eq!(parsed.name, "Test");
    }

    // ── Conduct encoding ─────────────────────────────────────────

    #[test]
    fn test_encode_conduct_fresh() {
        let conducts = ConductState::new();
        let encoded = encode_conduct(&conducts);
        // All 13 conducts maintained: bits 0-11 + bit 13.
        assert_eq!(encoded & (1 << 0), 1 << 0); // foodless
        assert_eq!(encoded & (1 << 5), 1 << 5); // pacifist
        assert_eq!(encoded & (1 << 9), 1 << 9); // wishless
        assert_eq!(encoded & (1 << 13), 1 << 13); // petless
    }

    #[test]
    fn test_encode_conduct_violated() {
        let mut conducts = ConductState::new();
        conducts.food = 1; // violated foodless
        conducts.killer = 3; // violated pacifist
        let encoded = encode_conduct(&conducts);
        assert_eq!(encoded & (1 << 0), 0); // foodless violated
        assert_eq!(encoded & (1 << 5), 0); // pacifist violated
        assert_eq!(encoded & (1 << 1), 1 << 1); // vegan still maintained
    }

    #[test]
    fn test_decode_conduct_roundtrip() {
        let conducts = ConductState::new();
        let encoded = encode_conduct(&conducts);
        let decoded = decode_conduct(encoded);
        assert_eq!(decoded.len(), 13); // all 13 standard conducts
        assert!(decoded.contains(&Conduct::Foodless));
        assert!(decoded.contains(&Conduct::Petless));
    }

    // ── Game flags ───────────────────────────────────────────────

    #[test]
    fn test_encode_flags() {
        let flags = encode_flags(true, false, true, false);
        assert_eq!(flags & 1, 1); // wizard
        assert_eq!(flags & 2, 0); // not discover
        assert_eq!(flags & 4, 4); // no bones
        assert_eq!(flags & 8, 0); // not rerolled
    }

    // ── Killer formatting ────────────────────────────────────────

    #[test]
    fn test_format_killer_died() {
        let result = format_killer(&EndHow::Died, "a gnome");
        assert_eq!(result, "killed by a gnome");
    }

    #[test]
    fn test_format_killer_choked() {
        let result = format_killer(&EndHow::Choked, "a food ration");
        assert_eq!(result, "choked on a food ration");
    }

    #[test]
    fn test_format_killer_stoning() {
        let result = format_killer(&EndHow::Stoning, "a cockatrice");
        assert_eq!(result, "petrified by a cockatrice");
    }

    #[test]
    fn test_format_killer_ascended() {
        let result = format_killer(&EndHow::Ascended, "");
        assert_eq!(result, "ascended");
    }

    #[test]
    fn test_format_killer_quit() {
        let result = format_killer(&EndHow::Quit, "");
        assert_eq!(result, "quit");
    }

    #[test]
    fn test_sanitize_killer_commas() {
        let result = format_killer(&EndHow::Died, "a monster, called evil");
        assert_eq!(result, "killed by a monster; called evil");
    }

    // ── Top-ten list ─────────────────────────────────────────────

    #[test]
    fn test_topten_add_and_rank() {
        let mut list = TopTenList::new(10);
        let entry1 = make_entry("Alice", 5000);
        let entry2 = make_entry("Bob", 10000);
        let entry3 = make_entry("Charlie", 3000);

        let rank1 = list.add_entry(entry1);
        assert_eq!(rank1, Some(1)); // first entry = rank 1

        let rank2 = list.add_entry(entry2);
        assert_eq!(rank2, Some(1)); // Bob has higher score = rank 1

        let rank3 = list.add_entry(entry3);
        assert_eq!(rank3, Some(3)); // lowest score = rank 3

        assert_eq!(list.len(), 3);
        assert_eq!(list.all()[0].name, "Bob");
        assert_eq!(list.all()[1].name, "Alice");
        assert_eq!(list.all()[2].name, "Charlie");
    }

    #[test]
    fn test_topten_max_entries() {
        let mut list = TopTenList::new(3);
        for i in 0..5 {
            let entry = make_entry(&format!("Player{}", i), (i + 1) as i64 * 1000);
            list.add_entry(entry);
        }
        assert_eq!(list.len(), 3);
        // Top 3 should be Player4(5000), Player3(4000), Player2(3000).
        assert_eq!(list.all()[0].points, 5000);
        assert_eq!(list.all()[2].points, 3000);
    }

    #[test]
    fn test_topten_would_qualify() {
        let mut list = TopTenList::new(2);
        list.add_entry(make_entry("A", 5000));
        list.add_entry(make_entry("B", 3000));

        assert!(list.would_qualify(4000)); // would fit at rank 2
        assert!(!list.would_qualify(2000)); // lower than both
    }

    #[test]
    fn test_topten_doesnt_qualify() {
        let mut list = TopTenList::new(2);
        list.add_entry(make_entry("A", 5000));
        list.add_entry(make_entry("B", 3000));

        let rank = list.add_entry(make_entry("C", 1000));
        assert_eq!(rank, None); // doesn't make the list
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_topten_entries_for_player() {
        let mut list = TopTenList::new(10);
        list.add_entry(make_entry("Alice", 5000));
        list.add_entry(make_entry("Bob", 4000));
        list.add_entry(make_entry("Alice", 3000));

        let alice = list.entries_for_player("Alice");
        assert_eq!(alice.len(), 2);
        assert_eq!(alice[0].1.points, 5000);
        assert_eq!(alice[1].1.points, 3000);
    }

    // ── Format display ───────────────────────────────────────────

    #[test]
    fn test_format_entry() {
        let entry = make_entry("Alice", 12345);
        let formatted = format_entry(1, &entry, false);
        assert!(formatted.contains("12345"));
        assert!(formatted.contains("Alice"));
    }

    #[test]
    fn test_format_entry_highlighted() {
        let entry = make_entry("Alice", 12345);
        let formatted = format_entry(1, &entry, true);
        assert!(formatted.starts_with(" *"));
    }

    // ── Role codes ───────────────────────────────────────────────

    #[test]
    fn test_role_code() {
        assert_eq!(role_code(Role::Wizard), "Wiz");
        assert_eq!(role_code(Role::Valkyrie), "Val");
        assert_eq!(role_code(Role::Samurai), "Sam");
    }

    #[test]
    fn test_role_from_code() {
        assert_eq!(role_from_code("Wiz"), Some(Role::Wizard));
        assert_eq!(role_from_code("Val"), Some(Role::Valkyrie));
        assert_eq!(role_from_code("???"), None);
    }

    // ── Observable depth ─────────────────────────────────────────

    #[test]
    fn test_observable_depth_normal() {
        assert_eq!(observable_depth(5, false, None), 5);
        assert_eq!(observable_depth(1, false, None), 1);
    }

    #[test]
    fn test_observable_depth_endgame() {
        assert_eq!(observable_depth(0, true, Some("astral")), -5);
        assert_eq!(observable_depth(0, true, Some("water")), -4);
        assert_eq!(observable_depth(0, true, Some("earth")), -1);
    }

    // ── Score filter ─────────────────────────────────────────────

    #[test]
    fn test_filter_by_player() {
        let mut list = TopTenList::new(10);
        list.add_entry(make_entry("Alice", 5000));
        list.add_entry(make_entry("Bob", 4000));
        list.add_entry(make_entry("Alice", 3000));

        let filter = ScoreFilter {
            player_name: Some("Alice".to_string()),
            ..Default::default()
        };
        let results = filter_scores(&list, &filter);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_filter_by_role() {
        let mut list = TopTenList::new(10);
        let mut e1 = make_entry("Alice", 5000);
        e1.role = "Wiz".to_string();
        let mut e2 = make_entry("Bob", 4000);
        e2.role = "Val".to_string();
        list.add_entry(e1);
        list.add_entry(e2);

        let filter = ScoreFilter {
            role_code: Some("Wiz".to_string()),
            ..Default::default()
        };
        let results = filter_scores(&list, &filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.name, "Alice");
    }

    #[test]
    fn test_filter_max_entries() {
        let mut list = TopTenList::new(10);
        for i in 0..5 {
            list.add_entry(make_entry(&format!("P{}", i), (5 - i) as i64 * 1000));
        }

        let filter = ScoreFilter {
            max_entries: Some(3),
            ..Default::default()
        };
        let results = filter_scores(&list, &filter);
        assert_eq!(results.len(), 3);
    }

    // ── Record line format ───────────────────────────────────────

    #[test]
    fn test_record_line() {
        let entry = make_entry("TestPlayer", 9999);
        let line = entry.to_record_line("0.1.0", 1000);
        assert!(line.contains("0.1.0"));
        assert!(line.contains("9999"));
        assert!(line.contains("TestPlayer"));
    }

    // ── Alignment code ───────────────────────────────────────────

    #[test]
    fn test_alignment_code() {
        assert_eq!(alignment_code("lawful"), "Law");
        assert_eq!(alignment_code("neutral"), "Neu");
        assert_eq!(alignment_code("chaotic"), "Cha");
        assert_eq!(alignment_code("Law"), "Law");
        assert_eq!(alignment_code("unknown"), "???");
    }

    // ── Hex parsing ──────────────────────────────────────────────

    #[test]
    fn test_parse_hex() {
        assert_eq!(parse_hex_or_dec("0x1f"), Some(31));
        assert_eq!(parse_hex_or_dec("42"), Some(42));
        assert_eq!(parse_hex_or_dec("0x3fff"), Some(0x3fff));
    }

    // ── Leaderboard ───────────────────────────────────────────────

    fn make_lb_entry(name: &str, score: i64) -> LeaderboardEntry {
        LeaderboardEntry {
            rank: 0,
            score,
            player_name: name.to_string(),
            role: "Wizard".to_string(),
            race: "Elf".to_string(),
            gender: "Female".to_string(),
            alignment: "Chaotic".to_string(),
            death_cause: "killed by a gnome".to_string(),
            dungeon_level: "Dlvl:5".to_string(),
            experience_level: 10,
            turns: 500,
            timestamp: "2026-03-15T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn leaderboard_add_and_sort() {
        let mut lb = Leaderboard::default();
        lb.add_entry(make_lb_entry("Alice", 5000));
        lb.add_entry(make_lb_entry("Bob", 10000));
        lb.add_entry(make_lb_entry("Charlie", 3000));

        assert_eq!(lb.entries.len(), 3);
        assert_eq!(lb.entries[0].player_name, "Bob");
        assert_eq!(lb.entries[0].rank, 1);
        assert_eq!(lb.entries[1].player_name, "Alice");
        assert_eq!(lb.entries[1].rank, 2);
        assert_eq!(lb.entries[2].player_name, "Charlie");
        assert_eq!(lb.entries[2].rank, 3);
    }

    #[test]
    fn leaderboard_truncate_top_100() {
        let mut lb = Leaderboard::default();
        for i in 0..110 {
            lb.add_entry(make_lb_entry(&format!("Player{}", i), (i + 1) as i64 * 100));
        }
        assert_eq!(lb.entries.len(), 100);
        assert_eq!(lb.entries[0].score, 11000); // Player109
        assert_eq!(lb.entries[99].score, 1100); // Player10
    }

    #[test]
    fn leaderboard_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("topten.json");

        let mut lb = Leaderboard::default();
        lb.add_entry(make_lb_entry("Alice", 5000));
        lb.add_entry(make_lb_entry("Bob", 10000));
        lb.save(&path).unwrap();

        let loaded = Leaderboard::load(&path);
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0].player_name, "Bob");
        assert_eq!(loaded.entries[0].score, 10000);
        assert_eq!(loaded.entries[1].player_name, "Alice");
        assert_eq!(loaded.entries[1].score, 5000);
    }

    #[test]
    fn leaderboard_load_missing_file() {
        let lb = Leaderboard::load(std::path::Path::new("/nonexistent/topten.json"));
        assert!(lb.entries.is_empty());
    }

    #[test]
    fn leaderboard_format_display() {
        let mut lb = Leaderboard::default();
        lb.add_entry(make_lb_entry("Alice", 5000));
        lb.add_entry(make_lb_entry("Bob", 10000));

        let lines = lb.format_top(10);
        assert!(lines[0].contains("No"));
        assert!(lines[0].contains("Points"));
        assert!(lines[1].contains("Bob"));
        assert!(lines[1].contains("10000"));
        assert!(lines[2].contains("Alice"));
        assert!(lines[2].contains("5000"));
    }

    #[test]
    fn leaderboard_player_rank() {
        let mut lb = Leaderboard::default();
        lb.add_entry(make_lb_entry("Alice", 5000));
        lb.add_entry(make_lb_entry("Bob", 10000));

        assert_eq!(lb.player_rank("Bob"), Some(1));
        assert_eq!(lb.player_rank("Alice"), Some(2));
        assert_eq!(lb.player_rank("Charlie"), None);
    }

    #[test]
    fn leaderboard_empty_format() {
        let lb = Leaderboard::default();
        let lines = lb.format_top(10);
        assert_eq!(lines.len(), 1); // header only
    }

    #[test]
    fn leaderboard_default_path_exists() {
        let path = default_leaderboard_path();
        assert!(path.to_str().unwrap().contains("nethack-babel"));
        assert!(path.to_str().unwrap().contains("topten.json"));
    }

    // ── Helper ───────────────────────────────────────────────────

    fn make_entry(name: &str, points: i64) -> TopTenEntry {
        TopTenEntry {
            points,
            death_dnum: 0,
            death_lev: 5,
            max_lvl: 8,
            hp: -3,
            max_hp: 42,
            deaths: 0,
            death_date: 20260314,
            birth_date: 20260314,
            name: name.to_string(),
            death: "killed by a gnome".to_string(),
            role: "Wiz".to_string(),
            race: "Elf".to_string(),
            gender: "Fem".to_string(),
            alignment: "Cha".to_string(),
        }
    }
}
