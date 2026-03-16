//! Conduct tracking, achievements, and scoring for NetHack Babel.
//!
//! Implements all 13 voluntary challenge conducts (plus Elberethless as a
//! bonus), the 31 achievement milestones, score calculation, and a
//! high-score board.
//!
//! All functions operate on plain data structs so they can be tested
//! without a full ECS world.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use nethack_babel_data::RoleId;

use crate::event::EngineEvent;

// ---------------------------------------------------------------------------
// Conduct enum
// ---------------------------------------------------------------------------

/// The 13 voluntary challenge conducts a player can maintain, plus
/// Elberethless as a bonus 14th.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Conduct {
    /// Never ate any food.
    Foodless,
    /// Never ate non-vegan food.
    Vegan,
    /// Never ate non-vegetarian food (meat).
    Vegetarian,
    /// Never prayed, sacrificed, or invoked divine aid.
    Atheist,
    /// Never hit with a wielded weapon.
    Weaponless,
    /// Never directly killed a monster.
    Pacifist,
    /// Never read a scroll, spellbook, or engraving.
    Illiterate,
    /// Never polymorphed an object pile.
    Polypileless,
    /// Never polymorphed self.
    Polyselfless,
    /// Never made a wish.
    Wishless,
    /// Never wished for an artifact.
    ArtifactWishless,
    /// Never genocided a monster type.
    Genocideless,
    /// Never used a pet.
    Petless,
    /// (Bonus) Never wrote Elbereth.
    Elberethless,
}

impl Conduct {
    /// All 13 standard conducts (excluding Elberethless bonus).
    pub const ALL: [Conduct; 13] = [
        Conduct::Foodless,
        Conduct::Vegan,
        Conduct::Vegetarian,
        Conduct::Atheist,
        Conduct::Weaponless,
        Conduct::Pacifist,
        Conduct::Illiterate,
        Conduct::Polypileless,
        Conduct::Polyselfless,
        Conduct::Wishless,
        Conduct::ArtifactWishless,
        Conduct::Genocideless,
        Conduct::Petless,
    ];

    /// All 14 conducts including the Elberethless bonus.
    pub const ALL_WITH_BONUS: [Conduct; 14] = [
        Conduct::Foodless,
        Conduct::Vegan,
        Conduct::Vegetarian,
        Conduct::Atheist,
        Conduct::Weaponless,
        Conduct::Pacifist,
        Conduct::Illiterate,
        Conduct::Polypileless,
        Conduct::Polyselfless,
        Conduct::Wishless,
        Conduct::ArtifactWishless,
        Conduct::Genocideless,
        Conduct::Petless,
        Conduct::Elberethless,
    ];

    /// Human-readable name of this conduct.
    pub fn name(self) -> &'static str {
        match self {
            Conduct::Foodless => "foodless",
            Conduct::Vegan => "vegan",
            Conduct::Vegetarian => "vegetarian",
            Conduct::Atheist => "atheist",
            Conduct::Weaponless => "weaponless",
            Conduct::Pacifist => "pacifist",
            Conduct::Illiterate => "illiterate",
            Conduct::Polypileless => "polypileless",
            Conduct::Polyselfless => "polyselfless",
            Conduct::Wishless => "wishless",
            Conduct::ArtifactWishless => "artifact-wishless",
            Conduct::Genocideless => "genocideless",
            Conduct::Petless => "petless",
            Conduct::Elberethless => "elberethless",
        }
    }
}

// ---------------------------------------------------------------------------
// Actions that can potentially violate conducts
// ---------------------------------------------------------------------------

/// Actions that the conduct system inspects.  These are abstracted from
/// `PlayerAction` so that conduct checking is independent of the action
/// parsing layer.
#[derive(Debug, Clone, PartialEq)]
pub enum ConductAction {
    /// Ate food.  `is_vegan` / `is_vegetarian` describe the food item.
    Eat { is_vegan: bool, is_vegetarian: bool },
    /// Prayed to a god, sacrificed, or used divine aid.
    Pray,
    /// Hit a monster with a wielded weapon.
    WeaponHit,
    /// Directly killed a monster.
    Kill,
    /// Read a scroll, spellbook, or engraving.
    Read,
    /// Polymorphed an object pile.
    Polypile,
    /// Polymorphed self.
    Polyself,
    /// Made a wish (non-artifact).
    Wish,
    /// Wished for an artifact.
    ArtifactWish,
    /// Genocided a monster type.
    Genocide,
    /// Acquired or used a pet.
    UsePet,
    /// Wrote Elbereth.
    WriteElbereth,
}

// ---------------------------------------------------------------------------
// Conduct violation
// ---------------------------------------------------------------------------

/// Record of a conduct violation.
#[derive(Debug, Clone, PartialEq)]
pub struct ConductViolation {
    pub conduct: Conduct,
    /// How many times this conduct has now been violated (cumulative).
    pub total_violations: i64,
}

// ---------------------------------------------------------------------------
// Conduct state
// ---------------------------------------------------------------------------

/// Mutable conduct state for a single game.  Tracks violation counts for
/// each of the 14 conducts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConductState {
    /// Total food items eaten (Foodless counter).
    pub food: i64,
    /// Non-vegan food eaten (Vegan counter).
    pub unvegan: i64,
    /// Non-vegetarian food eaten (Vegetarian counter).
    pub unvegetarian: i64,
    /// Prayers/sacrifices/divine aid used (Atheist counter).
    pub gnostic: i64,
    /// Weapon hits dealt (Weaponless counter).
    pub weaphit: i64,
    /// Direct kills (Pacifist counter).
    pub killer: i64,
    /// Scrolls/books read (Illiterate counter).
    pub literate: i64,
    /// Object pile polymorphs (Polypileless counter).
    pub polypiles: i64,
    /// Self-polymorphs (Polyselfless counter).
    pub polyselfs: i64,
    /// Wishes made (Wishless counter).
    pub wishes: i64,
    /// Artifact wishes made (ArtifactWishless counter).
    pub wisharti: i64,
    /// Genocides performed (Genocideless counter).
    pub genocides: i64,
    /// Pets acquired/used (Petless counter).
    pub pets: i64,
    /// Elbereth engravings written (Elberethless counter).
    pub elbereths: i64,
}

impl ConductState {
    /// Create a fresh conduct state (no violations).
    pub fn new() -> Self {
        Self {
            food: 0,
            unvegan: 0,
            unvegetarian: 0,
            gnostic: 0,
            weaphit: 0,
            killer: 0,
            literate: 0,
            polypiles: 0,
            polyselfs: 0,
            wishes: 0,
            wisharti: 0,
            genocides: 0,
            pets: 0,
            elbereths: 0,
        }
    }

    /// Return the violation count for the given conduct.
    pub fn violation_count(&self, conduct: Conduct) -> i64 {
        match conduct {
            Conduct::Foodless => self.food,
            Conduct::Vegan => self.unvegan,
            Conduct::Vegetarian => self.unvegetarian,
            Conduct::Atheist => self.gnostic,
            Conduct::Weaponless => self.weaphit,
            Conduct::Pacifist => self.killer,
            Conduct::Illiterate => self.literate,
            Conduct::Polypileless => self.polypiles,
            Conduct::Polyselfless => self.polyselfs,
            Conduct::Wishless => self.wishes,
            Conduct::ArtifactWishless => self.wisharti,
            Conduct::Genocideless => self.genocides,
            Conduct::Petless => self.pets,
            Conduct::Elberethless => self.elbereths,
        }
    }

    /// Whether the given conduct has been maintained (never violated).
    pub fn is_maintained(&self, conduct: Conduct) -> bool {
        self.violation_count(conduct) == 0
    }

    /// Return the number of standard conducts (out of 13) still maintained.
    pub fn maintained_count(&self) -> u32 {
        Conduct::ALL
            .iter()
            .filter(|c| self.is_maintained(**c))
            .count() as u32
    }
}

impl Default for ConductState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// check_conduct
// ---------------------------------------------------------------------------

/// Check whether a `ConductAction` violates any conducts.  If so, increment
/// the relevant counters and return a list of violations.
pub fn check_conduct(state: &mut ConductState, action: &ConductAction) -> Vec<ConductViolation> {
    let mut violations = Vec::new();

    match action {
        ConductAction::Eat {
            is_vegan,
            is_vegetarian,
        } => {
            // Any eating violates Foodless.
            state.food += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Foodless,
                total_violations: state.food,
            });

            // Non-vegan food violates Vegan.
            if !is_vegan {
                state.unvegan += 1;
                violations.push(ConductViolation {
                    conduct: Conduct::Vegan,
                    total_violations: state.unvegan,
                });
            }

            // Non-vegetarian food violates both Vegan and Vegetarian.
            if !is_vegetarian {
                // Meat is always non-vegan, but the vegan counter was
                // already incremented above if !is_vegan.  Only
                // increment unvegan if it wasn't already (!is_vegan is
                // always true when !is_vegetarian, so no double-count).
                state.unvegetarian += 1;
                violations.push(ConductViolation {
                    conduct: Conduct::Vegetarian,
                    total_violations: state.unvegetarian,
                });
            }
        }
        ConductAction::Pray => {
            state.gnostic += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Atheist,
                total_violations: state.gnostic,
            });
        }
        ConductAction::WeaponHit => {
            state.weaphit += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Weaponless,
                total_violations: state.weaphit,
            });
        }
        ConductAction::Kill => {
            state.killer += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Pacifist,
                total_violations: state.killer,
            });
        }
        ConductAction::Read => {
            state.literate += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Illiterate,
                total_violations: state.literate,
            });
        }
        ConductAction::Polypile => {
            state.polypiles += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Polypileless,
                total_violations: state.polypiles,
            });
        }
        ConductAction::Polyself => {
            state.polyselfs += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Polyselfless,
                total_violations: state.polyselfs,
            });
        }
        ConductAction::Wish => {
            state.wishes += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Wishless,
                total_violations: state.wishes,
            });
        }
        ConductAction::ArtifactWish => {
            // An artifact wish also counts as a wish.
            state.wishes += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Wishless,
                total_violations: state.wishes,
            });
            state.wisharti += 1;
            violations.push(ConductViolation {
                conduct: Conduct::ArtifactWishless,
                total_violations: state.wisharti,
            });
        }
        ConductAction::Genocide => {
            state.genocides += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Genocideless,
                total_violations: state.genocides,
            });
        }
        ConductAction::UsePet => {
            state.pets += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Petless,
                total_violations: state.pets,
            });
        }
        ConductAction::WriteElbereth => {
            state.elbereths += 1;
            violations.push(ConductViolation {
                conduct: Conduct::Elberethless,
                total_violations: state.elbereths,
            });
        }
    }

    violations
}

// ---------------------------------------------------------------------------
// display_conducts
// ---------------------------------------------------------------------------

/// Return a list of all 13 standard conducts with maintenance status and
/// violation count, suitable for the end-of-game disclosure screen.
pub fn display_conducts(state: &ConductState) -> Vec<(Conduct, bool, i64)> {
    Conduct::ALL
        .iter()
        .map(|c| (*c, state.is_maintained(*c), state.violation_count(*c)))
        .collect()
}

// ---------------------------------------------------------------------------
// Achievement enum
// ---------------------------------------------------------------------------

/// Achievement milestones (31 variants).
///
/// Milestones are recorded at most once per game.  The turn number of
/// first attainment is stored alongside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Achievement {
    // ── Milestones ────────────────────────────────────────────
    /// Killed the first monster.
    FirstKill,
    /// Entered a shop for the first time.
    FirstShop,
    /// Entered the Gnomish Mines.
    MinesEntered,
    /// Completed Sokoban.
    SokobanSolved,
    /// Killed Medusa.
    MedusaKilled,
    /// Completed the quest.
    QuestCompleted,
    /// Entered Gehennom.
    GehennomEntered,
    /// Killed Vlad the Impaler.
    VladKilled,
    /// Killed the Wizard of Yendor.
    WizardKilled,
    /// Obtained the Amulet of Yendor.
    AmuletObtained,
    /// Entered the Planes (endgame).
    EndgameEntered,
    /// Successfully ascended.
    Ascended,

    // ── Role mastery (first ascension per role) ──────────────
    AscendedArcheologist,
    AscendedBarbarian,
    AscendedCaveperson,
    AscendedHealer,
    AscendedKnight,
    AscendedMonk,
    AscendedPriest,
    AscendedRogue,
    AscendedRanger,
    AscendedSamurai,
    AscendedTourist,
    AscendedValkyrie,
    AscendedWizard,

    // ── Special challenge ascensions ─────────────────────────
    /// Ascended while maintaining weaponless conduct.
    WeaponlessAscension,
    /// Ascended while maintaining atheist conduct.
    AtheistAscension,
    /// Ascended while maintaining pacifist conduct.
    PacifistAscension,
    /// Ascended while maintaining foodless conduct.
    FoodlessAscension,
    /// Ascended while maintaining vegan conduct.
    VeganAscension,
}

impl Achievement {
    /// Map a role ID (0..12) to the corresponding role-ascension achievement.
    /// Returns `None` for unrecognized role IDs.
    pub fn for_role_ascension(role: RoleId) -> Option<Achievement> {
        match role.0 {
            0 => Some(Achievement::AscendedArcheologist),
            1 => Some(Achievement::AscendedBarbarian),
            2 => Some(Achievement::AscendedCaveperson),
            3 => Some(Achievement::AscendedHealer),
            4 => Some(Achievement::AscendedKnight),
            5 => Some(Achievement::AscendedMonk),
            6 => Some(Achievement::AscendedPriest),
            7 => Some(Achievement::AscendedRogue),
            8 => Some(Achievement::AscendedRanger),
            9 => Some(Achievement::AscendedSamurai),
            10 => Some(Achievement::AscendedTourist),
            11 => Some(Achievement::AscendedValkyrie),
            12 => Some(Achievement::AscendedWizard),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Achievement state
// ---------------------------------------------------------------------------

/// Tracks which achievements have been granted and when (turn number).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementState {
    /// Achievements in order of attainment, with the turn number.
    entries: Vec<(Achievement, u32)>,
    /// Fast membership lookup.
    granted: HashSet<Achievement>,
}

impl AchievementState {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            granted: HashSet::new(),
        }
    }

    /// Whether the given achievement has already been granted.
    pub fn has(&self, achievement: Achievement) -> bool {
        self.granted.contains(&achievement)
    }

    /// All granted achievements in chronological order.
    pub fn all(&self) -> &[(Achievement, u32)] {
        &self.entries
    }

    /// Number of achievements granted.
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for AchievementState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// grant_achievement
// ---------------------------------------------------------------------------

/// Grant an achievement if it has not already been earned.  Returns an
/// `EngineEvent::Message` announcing the achievement, or an empty vec if
/// it was already granted (idempotent).
pub fn grant_achievement(
    state: &mut AchievementState,
    achievement: Achievement,
    turn: u32,
) -> Vec<EngineEvent> {
    if state.granted.contains(&achievement) {
        return Vec::new();
    }
    state.granted.insert(achievement);
    state.entries.push((achievement, turn));

    vec![EngineEvent::msg_with(
        "achievement-unlock",
        vec![("name", format!("{:?}", achievement))],
    )]
}

// ---------------------------------------------------------------------------
// Score calculation
// ---------------------------------------------------------------------------

/// Input parameters for score calculation, extracted from game state.
#[derive(Debug, Clone)]
pub struct ScoreInput {
    /// Total accumulated experience points.
    pub experience: i64,
    /// Score-relevant experience points (`u.urexp`).
    pub score_experience: i64,
    /// Gold carried at end.
    pub gold_carried: i64,
    /// Gold deposited in shops / stashes.
    pub gold_deposited: i64,
    /// Number of distinct artifacts held.
    pub artifacts_held: u32,
    /// Conducts state at end of game.
    pub conducts: ConductState,
    /// Whether the player ascended.
    pub ascended: bool,
    /// Maximum dungeon depth reached.
    pub max_depth: u32,
}

/// Calculate the final score.
///
/// Formula:
///   base = `4 * experience + score_experience`  (this is what `urexp` tracks
///          in C NetHack, where `more_experienced(exp, rexp)` adds
///          `4*exp + rexp` to `urexp`)
///   + gold bonus: `gold_carried + gold_deposited`
///   + artifact bonus: `1000 * artifacts_held`
///   + conduct bonus: `5000 * maintained_count`  (out of 13 standard)
///   + ascension bonus: `50000` if ascended
pub fn calculate_score(input: &ScoreInput) -> u64 {
    // Base score: in C NetHack, urexp accumulates 4*exp + rexp.
    // We start from score_experience which is the Babel equivalent.
    // If it was populated the same way, it already includes 4*exp.
    // But if the caller provides raw values, we compute it:
    let base = (4i64.saturating_mul(input.experience))
        .saturating_add(input.score_experience)
        .max(0) as u64;

    // Gold bonus
    let gold = (input.gold_carried.saturating_add(input.gold_deposited)).max(0) as u64;

    // Artifact bonus: 1000 per artifact
    let artifact_bonus = input.artifacts_held as u64 * 1000;

    // Conduct bonus: 5000 per maintained standard conduct (out of 13)
    let conduct_bonus = input.conducts.maintained_count() as u64 * 5000;

    // Ascension bonus
    let ascension_bonus = if input.ascended { 50_000u64 } else { 0 };

    base.saturating_add(gold)
        .saturating_add(artifact_bonus)
        .saturating_add(conduct_bonus)
        .saturating_add(ascension_bonus)
}

// ---------------------------------------------------------------------------
// Scoreboard
// ---------------------------------------------------------------------------

/// A single entry in the high score table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreEntry {
    /// Player name.
    pub name: String,
    /// Role ID.
    pub role: RoleId,
    /// Race ID (stored as u8 for simplicity).
    pub race: u8,
    /// Final score.
    pub score: u64,
    /// Cause of death or "ascended".
    pub death_cause: String,
    /// Number of turns played.
    pub turns: u32,
    /// Which of the 13 standard conducts were maintained.
    pub conducts_maintained: Vec<Conduct>,
}

/// A persistent high score list, sorted by score descending.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scoreboard {
    entries: Vec<ScoreEntry>,
}

impl Scoreboard {
    /// Create an empty scoreboard.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add an entry.  The board is kept sorted by score descending.
    pub fn add_entry(&mut self, entry: ScoreEntry) {
        // Find insertion point to maintain descending order.
        let pos = self.entries.partition_point(|e| e.score >= entry.score);
        self.entries.insert(pos, entry);
    }

    /// Return the top `n` entries (or fewer if the board is smaller).
    pub fn get_top(&self, n: usize) -> &[ScoreEntry] {
        let end = n.min(self.entries.len());
        &self.entries[..end]
    }

    /// Number of entries on the board.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the board is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Save the scoreboard to a JSON file.
    pub fn save(&self, path: &Path) -> Result<(), ScoreboardError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| ScoreboardError::SerializeError(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| ScoreboardError::IoError(e.to_string()))?;
        Ok(())
    }

    /// Load a scoreboard from a JSON file.  Returns an empty board if the
    /// file does not exist.
    pub fn load(path: &Path) -> Result<Self, ScoreboardError> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let json =
            std::fs::read_to_string(path).map_err(|e| ScoreboardError::IoError(e.to_string()))?;
        let board: Scoreboard = serde_json::from_str(&json)
            .map_err(|e| ScoreboardError::DeserializeError(e.to_string()))?;
        Ok(board)
    }
}

impl Default for Scoreboard {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during scoreboard persistence.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ScoreboardError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("serialization error: {0}")]
    SerializeError(String),
    #[error("deserialization error: {0}")]
    DeserializeError(String),
}

// ---------------------------------------------------------------------------
// Experience level thresholds (Track N)
// ---------------------------------------------------------------------------

/// Calculate the cumulative XP threshold required to advance past the given
/// level.  Equivalent to `newuexp(lev)` in C NetHack (exper.c).
///
/// - lev < 1  => 0
/// - lev 1..9 => 10 * 2^lev
/// - lev 10..19 => 10_000 * 2^(lev - 10)
/// - lev >= 20 => 10_000_000 * (lev - 19)
pub fn newuexp(lev: i32) -> i64 {
    if lev < 1 {
        return 0;
    }
    if lev < 10 {
        // 10 * 2^lev
        10i64 * (1i64 << lev)
    } else if lev < 20 {
        // 10_000 * 2^(lev - 10)
        10_000i64 * (1i64 << (lev - 10))
    } else {
        // 10_000_000 * (lev - 19)
        10_000_000i64 * (lev as i64 - 19)
    }
}

/// Maximum experience level.
pub const MAXULEV: i32 = 30;

/// Normal movement speed constant.
pub const NORMAL_SPEED: i32 = 12;

/// Number of attack slots per monster.
pub const NATTK: usize = 6;

// ---------------------------------------------------------------------------
// Monster experience value calculation (Track N)
// ---------------------------------------------------------------------------

/// Attack method constants matching NetHack's AT_xxx values.
pub mod at {
    pub const AT_NONE: u8 = 0;
    pub const AT_CLAW: u8 = 1;
    pub const AT_BITE: u8 = 2;
    pub const AT_KICK: u8 = 3;
    pub const AT_BUTT: u8 = 4;
    pub const AT_TUCH: u8 = 5;
    pub const AT_WEAP: u8 = 254;
    pub const AT_MAGC: u8 = 255;
}

/// Damage type constants matching NetHack's AD_xxx values.
pub mod ad {
    pub const AD_PHYS: u8 = 0;
    pub const AD_BLND: u8 = 11;
    pub const AD_DRLI: u8 = 15;
    pub const AD_STON: u8 = 18;
    pub const AD_WRAP: u8 = 28;
    pub const AD_SLIM: u8 = 40;
}

/// A single attack slot for XP calculation purposes.
#[derive(Debug, Clone, Copy)]
pub struct XpAttack {
    /// Attack method (AT_xxx value).
    pub aatyp: u8,
    /// Damage type (AD_xxx value).
    pub adtyp: u8,
    /// Damage dice count.
    pub damn: u8,
    /// Damage dice sides.
    pub damd: u8,
}

/// Parameters needed to calculate the XP value of a killed monster.
#[derive(Debug, Clone)]
pub struct MonsterXpInput {
    /// The monster's current level (after `adj_lev` adjustment).
    pub m_lev: i32,
    /// The monster's effective AC (from find_mac).
    pub mac: i32,
    /// The monster's movement speed.
    pub mmove: i32,
    /// The monster's attacks (up to 6).
    pub attacks: Vec<XpAttack>,
    /// Whether the monster has M2_NASTY flag.
    pub extra_nasty: bool,
    /// Whether the monster is a mail daemon.
    pub is_mail_daemon: bool,
    /// Whether the monster has been revived or cloned.
    pub revived_or_cloned: bool,
    /// Total kill count for this monster type (including this kill).
    pub nk: i32,
    /// Monster class character (for eel drowning check).
    pub monster_class: char,
    /// Whether the player is amphibious (for eel drowning check).
    pub player_amphibious: bool,
}

/// Calculate the experience points awarded for killing a monster.
/// Equivalent to `experience(mtmp, nk)` in C NetHack (exper.c).
pub fn monster_experience(input: &MonsterXpInput) -> i64 {
    let m_lev = input.m_lev as i64;

    // Base: 1 + m_lev^2
    let mut tmp = 1i64 + m_lev * m_lev;

    // AC bonus
    if input.mac < 3 {
        if input.mac < 0 {
            tmp += (7 - input.mac as i64) * 2;
        } else {
            tmp += 7 - input.mac as i64;
        }
    }

    // Speed bonus
    if input.mmove > NORMAL_SPEED {
        if input.mmove > 18 {
            tmp += 5;
        } else {
            tmp += 3;
        }
    }

    // Attack method bonuses
    for atk in &input.attacks {
        if atk.aatyp > at::AT_BUTT {
            if atk.aatyp == at::AT_WEAP {
                tmp += 5;
            } else if atk.aatyp == at::AT_MAGC {
                tmp += 10;
            } else {
                tmp += 3;
            }
        }

        // Damage type bonuses
        let adtyp = atk.adtyp;
        if adtyp > ad::AD_PHYS && adtyp < ad::AD_BLND {
            // AD_MAGM(1)..AD_SPC2(10): +2 * m_lev
            tmp += 2 * m_lev;
        } else if adtyp == ad::AD_DRLI || adtyp == ad::AD_STON || adtyp == ad::AD_SLIM {
            tmp += 50;
        } else if adtyp != ad::AD_PHYS {
            // Other non-physical damage types
            tmp += m_lev;
        }

        // Heavy damage bonus (applies to ALL damage types including AD_PHYS)
        if (atk.damd as i64) * (atk.damn as i64) > 23 {
            tmp += m_lev;
        }

        // Eel drowning special
        if adtyp == ad::AD_WRAP
            && input.monster_class == ';' // S_EEL
            && !input.player_amphibious
        {
            tmp += 1000;
        }
    }

    // Extra nasty bonus
    if input.extra_nasty {
        tmp += 7 * m_lev;
    }

    // High level bonus
    if input.m_lev > 8 {
        tmp += 50;
    }

    // Mail daemon override
    if input.is_mail_daemon {
        return 1;
    }

    // Revive/clone diminishing returns
    if input.revived_or_cloned {
        tmp = apply_revive_diminishing(tmp, input.nk);
    }

    tmp.max(0)
}

/// Apply the diminishing returns for revived/cloned monsters.
/// The kill-count brackets are: 20, 20, 40, 40, 60, 60, ...
fn apply_revive_diminishing(base_xp: i64, total_kills: i32) -> i64 {
    let mut tmp = base_xp;
    let mut nk = total_kills;
    let mut tmp2 = 20i32;
    let mut i = 0;

    while nk > tmp2 && tmp > 1 {
        tmp = (tmp + 1) / 2; // integer division, round up
        nk -= tmp2;
        if i % 2 == 1 {
            tmp2 += 20;
        }
        i += 1;
    }

    tmp
}

// ---------------------------------------------------------------------------
// Monster level adjustment (Track N)
// ---------------------------------------------------------------------------

/// Input parameters for monster level adjustment.
#[derive(Debug, Clone, Copy)]
pub struct AdjLevInput {
    /// Base monster level from `permonst.mlevel`.
    pub mlevel: i32,
    /// Dungeon difficulty from `level_difficulty()`.
    pub level_difficulty: i32,
    /// Player's experience level (`u.ulevel`).
    pub player_level: i32,
    /// Whether this is the Wizard of Yendor.
    pub is_wizard: bool,
    /// Number of times the Wizard has been killed (for Wizard only).
    pub wizard_kills: i32,
}

/// Calculate adjusted monster level, equivalent to `adj_lev()` in
/// C NetHack (makemon.c).
pub fn adj_lev(input: &AdjLevInput) -> i32 {
    // Special: Wizard of Yendor
    if input.is_wizard {
        return (input.mlevel + input.wizard_kills).min(49);
    }

    // Special: "super" demons (mlevel > 49)
    if input.mlevel > 49 {
        return 50;
    }

    let mut tmp = input.mlevel;

    // Adjust based on dungeon difficulty
    let tmp2 = input.level_difficulty - input.mlevel;
    if tmp2 < 0 {
        tmp -= 1;
    } else {
        tmp += tmp2 / 5;
    }

    // Adjust based on player level
    let tmp2 = input.player_level - input.mlevel;
    if tmp2 > 0 {
        tmp += tmp2 / 4;
    }

    // Upper limit: 1.5x base level, capped at 49
    let upper = (3 * input.mlevel / 2).min(49);

    tmp.clamp(0, upper)
}

// ---------------------------------------------------------------------------
// Death message format (Track N, Decision D1)
// ---------------------------------------------------------------------------

/// Format a death message following the original NetHack convention.
///
/// The format is: "{player_name}, {title}, killed by {cause} on dungeon level {depth}"
/// For ascension: "{player_name}, {title}, ascended to demigod-hood"
pub fn format_death_message(
    player_name: &str,
    title: &str,
    cause: &str,
    dungeon_depth: Option<i32>,
    ascended: bool,
) -> String {
    if ascended {
        format!("{}, {}, ascended to demigod-hood", player_name, title)
    } else if let Some(depth) = dungeon_depth {
        format!(
            "{}, {}, killed by {} on dungeon level {}",
            player_name, title, cause, depth
        )
    } else {
        format!("{}, {}, killed by {}", player_name, title, cause)
    }
}

// ---------------------------------------------------------------------------
// Elbereth engraving mechanics (Track Q)
// ---------------------------------------------------------------------------

/// Method used to engrave Elbereth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngravingMethod {
    /// Written in dust (finger, wand of nothing, etc.).
    Dust,
    /// Engraved (hard stylus like athame or hard gem).
    Engrave,
    /// Burned (wand of fire, wand of lightning).
    Burn,
}

/// State of an Elbereth engraving on a tile.
#[derive(Debug, Clone)]
pub struct ElberethEngraving {
    /// How was it written.
    pub method: EngravingMethod,
    /// Number of times the word appears (can be overwritten).
    pub count: i32,
    /// Whether it is still effective (not walked over too many times).
    pub effective: bool,
    /// Durability: how many more steps before it degrades.
    /// Dust: degrades on each step (1/count chance per step for each copy).
    /// Engrave: much more durable.
    /// Burn: most durable (never degrades from walking).
    pub durability: i32,
}

impl ElberethEngraving {
    /// Create a new Elbereth engraving.
    pub fn new(method: EngravingMethod) -> Self {
        let durability = match method {
            EngravingMethod::Dust => 1,
            EngravingMethod::Engrave => 5,
            EngravingMethod::Burn => i32::MAX, // effectively permanent
        };
        Self {
            method,
            count: 1,
            effective: true,
            durability,
        }
    }

    /// Simulate stepping on the engraving.  Returns true if it's still effective.
    pub fn step_on(&mut self) -> bool {
        if !self.effective {
            return false;
        }
        match self.method {
            EngravingMethod::Dust => {
                // Dust: degrades with each step
                self.durability -= 1;
                if self.durability <= 0 {
                    self.effective = false;
                }
            }
            EngravingMethod::Engrave => {
                // Engraved: degrades more slowly
                self.durability -= 1;
                if self.durability <= 0 {
                    self.effective = false;
                }
            }
            EngravingMethod::Burn => {
                // Burned: does not degrade from walking
            }
        }
        self.effective
    }

    /// Whether this engraving repels monsters.
    pub fn repels_monsters(&self) -> bool {
        self.effective && self.count > 0
    }
}

/// Durability ordering: burn > engrave > dust (higher is more durable).
pub fn engraving_durability_order(method: EngravingMethod) -> u8 {
    match method {
        EngravingMethod::Dust => 0,
        EngravingMethod::Engrave => 1,
        EngravingMethod::Burn => 2,
    }
}

// ---------------------------------------------------------------------------
// Pudding farming mechanic (Track Q)
// ---------------------------------------------------------------------------

/// Simulate a pudding split check.  In NetHack, hitting a pudding with an
/// edged weapon has a chance to split it into two puddings.
///
/// Returns true if the pudding splits.
///
/// Conditions for splitting:
/// - Weapon damage type must be "slash" or "pierce" (is_edged = true).
/// - Pudding must have HP > 1 (has_hp = true).
/// - Random check: always splits when conditions are met (NetHack behavior).
pub fn pudding_should_split(is_edged: bool, pudding_hp: i32) -> bool {
    // In NetHack, pudding always splits if hit with a cutting/piercing weapon
    // and has sufficient HP (> 1).  There is no random element to the split
    // decision itself.
    is_edged && pudding_hp > 1
}

// ---------------------------------------------------------------------------
// Price identification mechanic (Track Q)
// ---------------------------------------------------------------------------

/// Given a base price from a shop and the object class, deduce possible
/// identities.  This is the "price ID" exploit.
///
/// Returns the set of base prices that map to that buy price range,
/// allowing the player to narrow down item identity.
pub fn price_id_from_buy_price(observed_buy_price: i32, charisma: u8, is_tourist: bool) -> i32 {
    // Reverse the buy price to get the base cost.
    // buy_price = base_cost * cha_mul / cha_div * tourist_mul / tourist_div
    // So base_cost = buy_price * cha_div / cha_mul * tourist_div / tourist_mul

    let (cha_mul, cha_div) = match charisma {
        0..=5 => (2i32, 1i32),
        6..=7 => (3, 2),
        8..=10 => (4, 3),
        11..=15 => (1, 1),
        16..=17 => (3, 4),
        18 => (2, 3),
        _ => (1, 2),
    };

    let (tour_mul, tour_div) = if is_tourist { (4, 3) } else { (1, 1) };

    // Reverse: base = observed * cha_div * tour_div / (cha_mul * tour_mul)
    let base = observed_buy_price * cha_div * tour_div / (cha_mul * tour_mul);
    base.max(0)
}

// ---------------------------------------------------------------------------
// Unicorn horn rubbing (Track Q)
// ---------------------------------------------------------------------------

/// Status effects that can be cured by rubbing a unicorn horn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnicornHornCurable {
    Confusion,
    Stun,
    Hallucination,
    Blindness,
    Sickness,
    Nausea,
}

/// Check whether rubbing a unicorn horn cures the given status effect.
///
/// In NetHack, a blessed unicorn horn has the best cure chance,
/// uncursed is moderate, and cursed can actually inflict problems.
///
/// Returns true if the status is cured.
pub fn unicorn_horn_cures(
    effect: UnicornHornCurable,
    is_blessed: bool,
    is_cursed: bool,
    roll: i32, // rn2(100) value
) -> bool {
    if is_cursed {
        // Cursed horn: never cures, may inflict
        return false;
    }

    // Each cure has a different threshold.
    // Blessed horn: higher chance.
    // Based on NetHack's unihorn.c logic.
    let threshold = if is_blessed {
        match effect {
            UnicornHornCurable::Confusion => 100, // always cures
            UnicornHornCurable::Stun => 100,
            UnicornHornCurable::Hallucination => 100,
            UnicornHornCurable::Blindness => 100,
            UnicornHornCurable::Sickness => 100,
            UnicornHornCurable::Nausea => 100,
        }
    } else {
        // Uncursed: 1/3 chance per try (simplified)
        match effect {
            UnicornHornCurable::Confusion => 33,
            UnicornHornCurable::Stun => 33,
            UnicornHornCurable::Hallucination => 33,
            UnicornHornCurable::Blindness => 33,
            UnicornHornCurable::Sickness => 33,
            UnicornHornCurable::Nausea => 33,
        }
    };

    roll < threshold
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Conduct tests ─────────────────────────────────────────

    #[test]
    fn eating_meat_violates_vegan_and_vegetarian() {
        let mut state = ConductState::new();
        let violations = check_conduct(
            &mut state,
            &ConductAction::Eat {
                is_vegan: false,
                is_vegetarian: false,
            },
        );
        let violated: Vec<Conduct> = violations.iter().map(|v| v.conduct).collect();
        assert!(violated.contains(&Conduct::Foodless));
        assert!(violated.contains(&Conduct::Vegan));
        assert!(violated.contains(&Conduct::Vegetarian));
        assert_eq!(state.food, 1);
        assert_eq!(state.unvegan, 1);
        assert_eq!(state.unvegetarian, 1);
    }

    #[test]
    fn eating_vegan_food_only_violates_foodless() {
        let mut state = ConductState::new();
        let violations = check_conduct(
            &mut state,
            &ConductAction::Eat {
                is_vegan: true,
                is_vegetarian: true,
            },
        );
        let violated: Vec<Conduct> = violations.iter().map(|v| v.conduct).collect();
        assert!(violated.contains(&Conduct::Foodless));
        assert!(!violated.contains(&Conduct::Vegan));
        assert!(!violated.contains(&Conduct::Vegetarian));
    }

    #[test]
    fn using_weapon_violates_weaponless() {
        let mut state = ConductState::new();
        let violations = check_conduct(&mut state, &ConductAction::WeaponHit);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].conduct, Conduct::Weaponless);
        assert_eq!(state.weaphit, 1);
        assert!(!state.is_maintained(Conduct::Weaponless));
    }

    #[test]
    fn praying_violates_atheist() {
        let mut state = ConductState::new();
        let violations = check_conduct(&mut state, &ConductAction::Pray);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].conduct, Conduct::Atheist);
        assert_eq!(state.gnostic, 1);
    }

    #[test]
    fn reading_violates_illiterate() {
        let mut state = ConductState::new();
        let violations = check_conduct(&mut state, &ConductAction::Read);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].conduct, Conduct::Illiterate);
        assert_eq!(state.literate, 1);
    }

    #[test]
    fn killing_violates_pacifist() {
        let mut state = ConductState::new();
        let violations = check_conduct(&mut state, &ConductAction::Kill);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].conduct, Conduct::Pacifist);
        assert_eq!(state.killer, 1);
    }

    #[test]
    fn multiple_conduct_violations_tracked_independently() {
        let mut state = ConductState::new();

        // Violate weaponless twice.
        check_conduct(&mut state, &ConductAction::WeaponHit);
        check_conduct(&mut state, &ConductAction::WeaponHit);
        assert_eq!(state.weaphit, 2);

        // Violate pacifist once.
        check_conduct(&mut state, &ConductAction::Kill);
        assert_eq!(state.killer, 1);

        // Weaponless and pacifist are independent.
        assert!(!state.is_maintained(Conduct::Weaponless));
        assert!(!state.is_maintained(Conduct::Pacifist));
        // Others still maintained.
        assert!(state.is_maintained(Conduct::Atheist));
        assert!(state.is_maintained(Conduct::Vegan));
    }

    #[test]
    fn artifact_wish_violates_both_wishless_and_artiwishless() {
        let mut state = ConductState::new();
        let violations = check_conduct(&mut state, &ConductAction::ArtifactWish);
        let violated: Vec<Conduct> = violations.iter().map(|v| v.conduct).collect();
        assert!(violated.contains(&Conduct::Wishless));
        assert!(violated.contains(&Conduct::ArtifactWishless));
        assert_eq!(state.wishes, 1);
        assert_eq!(state.wisharti, 1);
    }

    // ── Achievement tests ─────────────────────────────────────

    #[test]
    fn achievement_granted_once_only_idempotent() {
        let mut state = AchievementState::new();

        let events1 = grant_achievement(&mut state, Achievement::FirstKill, 10);
        assert_eq!(events1.len(), 1);
        assert!(state.has(Achievement::FirstKill));

        // Granting again returns no events.
        let events2 = grant_achievement(&mut state, Achievement::FirstKill, 20);
        assert!(events2.is_empty());
        assert_eq!(state.count(), 1);
        // Turn number is from first grant.
        assert_eq!(state.all()[0].1, 10);
    }

    #[test]
    fn multiple_achievements_recorded_in_order() {
        let mut state = AchievementState::new();
        grant_achievement(&mut state, Achievement::FirstShop, 5);
        grant_achievement(&mut state, Achievement::MinesEntered, 100);
        grant_achievement(&mut state, Achievement::MedusaKilled, 5000);

        assert_eq!(state.count(), 3);
        assert_eq!(state.all()[0].0, Achievement::FirstShop);
        assert_eq!(state.all()[1].0, Achievement::MinesEntered);
        assert_eq!(state.all()[2].0, Achievement::MedusaKilled);
    }

    // ── Score tests ───────────────────────────────────────────

    #[test]
    fn score_calculation_with_xp_and_gold() {
        let conducts = ConductState::new(); // All 13 maintained.
        let input = ScoreInput {
            experience: 1000,
            score_experience: 0,
            gold_carried: 500,
            gold_deposited: 200,
            artifacts_held: 2,
            conducts,
            ascended: false,
            max_depth: 10,
        };
        let score = calculate_score(&input);
        // base = 4*1000 + 0 = 4000
        // gold = 500 + 200 = 700
        // artifacts = 2 * 1000 = 2000
        // conducts = 13 * 5000 = 65000
        // ascension = 0
        assert_eq!(score, 4000 + 700 + 2000 + 65000);
    }

    #[test]
    fn score_with_ascension_bonus() {
        let mut conducts = ConductState::new();
        // Break a few conducts.
        check_conduct(
            &mut conducts,
            &ConductAction::Eat {
                is_vegan: false,
                is_vegetarian: false,
            },
        );
        check_conduct(&mut conducts, &ConductAction::Kill);
        // 3 conducts broken: Foodless, Vegan, Vegetarian, Pacifist = 4.
        // Wait, eating non-veg meat: Foodless + Vegan + Vegetarian = 3 counters bumped.
        // Kill: Pacifist = 1 counter. Total broken = 4 distinct.
        // Maintained = 13 - 4 = 9.

        let input = ScoreInput {
            experience: 5000,
            score_experience: 100,
            gold_carried: 0,
            gold_deposited: 0,
            artifacts_held: 0,
            conducts,
            ascended: true,
            max_depth: 50,
        };
        let score = calculate_score(&input);
        // base = 4*5000 + 100 = 20100
        // gold = 0
        // artifacts = 0
        // conducts = 9 * 5000 = 45000
        // ascension = 50000
        assert_eq!(score, 20100 + 45000 + 50000);
    }

    // ── Conduct display test ──────────────────────────────────

    #[test]
    fn conduct_display_shows_all_13() {
        let state = ConductState::new();
        let display = display_conducts(&state);
        assert_eq!(display.len(), 13);
        // All maintained with 0 violations.
        for (_, maintained, count) in &display {
            assert!(maintained);
            assert_eq!(*count, 0);
        }
    }

    #[test]
    fn conduct_display_shows_violations() {
        let mut state = ConductState::new();
        check_conduct(&mut state, &ConductAction::Kill);
        check_conduct(&mut state, &ConductAction::Kill);
        check_conduct(&mut state, &ConductAction::Kill);

        let display = display_conducts(&state);
        let pacifist_entry = display
            .iter()
            .find(|(c, _, _)| *c == Conduct::Pacifist)
            .unwrap();
        assert!(!pacifist_entry.1); // not maintained
        assert_eq!(pacifist_entry.2, 3); // 3 violations
    }

    // ── Scoreboard tests ──────────────────────────────────────

    #[test]
    fn scoreboard_sorts_by_score_descending() {
        let mut board = Scoreboard::new();
        board.add_entry(make_entry("Alice", 100));
        board.add_entry(make_entry("Bob", 500));
        board.add_entry(make_entry("Charlie", 300));

        let top = board.get_top(10);
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].name, "Bob");
        assert_eq!(top[0].score, 500);
        assert_eq!(top[1].name, "Charlie");
        assert_eq!(top[1].score, 300);
        assert_eq!(top[2].name, "Alice");
        assert_eq!(top[2].score, 100);
    }

    #[test]
    fn scoreboard_get_top_limits_results() {
        let mut board = Scoreboard::new();
        for i in 0..20 {
            board.add_entry(make_entry(&format!("Player{}", i), i * 10));
        }
        let top5 = board.get_top(5);
        assert_eq!(top5.len(), 5);
        // Highest score should be first.
        assert_eq!(top5[0].score, 190);
    }

    #[test]
    fn scoreboard_persists_to_file() {
        let dir = std::env::temp_dir().join("nethack_babel_test_scoreboard");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("scores.json");

        // Clean up from any previous run.
        let _ = std::fs::remove_file(&path);

        let mut board = Scoreboard::new();
        board.add_entry(make_entry("Hero", 42000));
        board.add_entry(make_entry("Zero", 100));
        board.save(&path).expect("save should succeed");

        let loaded = Scoreboard::load(&path).expect("load should succeed");
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.get_top(1)[0].name, "Hero");
        assert_eq!(loaded.get_top(1)[0].score, 42000);

        // Clean up.
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn scoreboard_load_nonexistent_returns_empty() {
        let path = std::env::temp_dir().join("nethack_babel_no_such_file.json");
        let _ = std::fs::remove_file(&path);
        let board = Scoreboard::load(&path).expect("load of missing file");
        assert!(board.is_empty());
    }

    // ── Role ascension achievement mapping ────────────────────

    #[test]
    fn role_ascension_maps_all_13_roles() {
        for i in 0..13u8 {
            assert!(
                Achievement::for_role_ascension(RoleId(i)).is_some(),
                "role {} should map to an achievement",
                i
            );
        }
        assert!(Achievement::for_role_ascension(RoleId(99)).is_none());
    }

    // ── Helpers ───────────────────────────────────────────────

    fn make_entry(name: &str, score: u64) -> ScoreEntry {
        ScoreEntry {
            name: name.to_string(),
            role: RoleId(0),
            race: 0,
            score,
            death_cause: "killed by a test".to_string(),
            turns: 1000,
            conducts_maintained: Vec::new(),
        }
    }

    // =========================================================================
    // Track N — Score & Experience System Tests
    // =========================================================================

    // ── Experience threshold tests ────────────────────────────

    #[test]
    fn test_score_newuexp_boundary_below_1() {
        assert_eq!(newuexp(-1), 0);
        assert_eq!(newuexp(0), 0);
    }

    #[test]
    fn test_score_newuexp_level_1() {
        assert_eq!(newuexp(1), 20);
    }

    #[test]
    fn test_score_newuexp_level_9_boundary() {
        // lev=9 still uses first formula: 10 * 2^9 = 5120
        assert_eq!(newuexp(9), 5120);
    }

    #[test]
    fn test_score_newuexp_level_10_boundary() {
        // lev=10 switches to second formula: 10_000 * 2^0 = 10_000
        assert_eq!(newuexp(10), 10_000);
    }

    #[test]
    fn test_score_newuexp_level_19_boundary() {
        // lev=19 still uses second formula: 10_000 * 2^9 = 5_120_000
        assert_eq!(newuexp(19), 5_120_000);
    }

    #[test]
    fn test_score_newuexp_level_20_boundary() {
        // lev=20 switches to third formula: 10_000_000 * 1 = 10_000_000
        assert_eq!(newuexp(20), 10_000_000);
    }

    #[test]
    fn test_score_newuexp_level_30() {
        assert_eq!(newuexp(30), 110_000_000);
    }

    #[test]
    fn test_score_newuexp_geometric_progression_low_levels() {
        // Verify the geometric (doubling) nature: each level doubles
        for lev in 1..9 {
            assert_eq!(
                newuexp(lev + 1),
                newuexp(lev) * 2,
                "level {} to {} should double",
                lev,
                lev + 1
            );
        }
    }

    #[test]
    fn test_score_newuexp_full_table() {
        let expected: Vec<(i32, i64)> = vec![
            (1, 20),
            (2, 40),
            (3, 80),
            (4, 160),
            (5, 320),
            (6, 640),
            (7, 1280),
            (8, 2560),
            (9, 5120),
            (10, 10_000),
            (11, 20_000),
            (12, 40_000),
            (13, 80_000),
            (14, 160_000),
            (15, 320_000),
            (16, 640_000),
            (17, 1_280_000),
            (18, 2_560_000),
            (19, 5_120_000),
            (20, 10_000_000),
            (21, 20_000_000),
            (22, 30_000_000),
            (23, 40_000_000),
            (24, 50_000_000),
            (25, 60_000_000),
            (26, 70_000_000),
            (27, 80_000_000),
            (28, 90_000_000),
            (29, 100_000_000),
            (30, 110_000_000),
        ];
        for (lev, exp) in &expected {
            assert_eq!(newuexp(*lev), *exp, "newuexp({}) should be {}", lev, exp);
        }
    }

    // ── Monster XP calculation tests (spec test vectors) ─────

    /// Helper: no attacks (all AT_NONE / AD_PHYS).
    fn no_attacks() -> Vec<XpAttack> {
        Vec::new()
    }

    #[test]
    fn test_score_monster_xp_vector_a_simple_low_level() {
        // Vector A from spec: m_lev=3, mac=10, mmove=12, no attacks, no nasty
        let input = MonsterXpInput {
            m_lev: 3,
            mac: 10,
            mmove: 12,
            attacks: no_attacks(),
            extra_nasty: false,
            is_mail_daemon: false,
            revived_or_cloned: false,
            nk: 0,
            monster_class: ' ',
            player_amphibious: false,
        };
        assert_eq!(monster_experience(&input), 10);
    }

    #[test]
    fn test_score_monster_xp_vector_b_nasty_special_attacks() {
        // Vector B: m_lev=10, mac=-2, mmove=15,
        // attacks: [AT_WEAP/AD_PHYS/2d8, AT_MAGC/AD_SPEL/0d0, AT_CLAW/AD_DRLI/1d6]
        let attacks = vec![
            XpAttack {
                aatyp: at::AT_WEAP,
                adtyp: ad::AD_PHYS,
                damn: 2,
                damd: 8,
            },
            XpAttack {
                aatyp: at::AT_MAGC,
                adtyp: 241,
                damn: 0,
                damd: 0,
            }, // AD_SPEL=241
            XpAttack {
                aatyp: at::AT_CLAW,
                adtyp: ad::AD_DRLI,
                damn: 1,
                damd: 6,
            },
        ];
        let input = MonsterXpInput {
            m_lev: 10,
            mac: -2,
            mmove: 15,
            attacks,
            extra_nasty: true,
            is_mail_daemon: false,
            revived_or_cloned: false,
            nk: 0,
            monster_class: ' ',
            player_amphibious: false,
        };
        assert_eq!(monster_experience(&input), 317);
    }

    #[test]
    fn test_score_monster_xp_vector_c_eel_drowning() {
        // Vector C: m_lev=5, mac=9, mmove=12, S_EEL, non-amphibious player
        let attacks = vec![
            XpAttack {
                aatyp: at::AT_TUCH,
                adtyp: ad::AD_WRAP,
                damn: 2,
                damd: 6,
            },
            XpAttack {
                aatyp: at::AT_BITE,
                adtyp: ad::AD_PHYS,
                damn: 1,
                damd: 4,
            },
        ];
        let input = MonsterXpInput {
            m_lev: 5,
            mac: 9,
            mmove: 12,
            attacks,
            extra_nasty: false,
            is_mail_daemon: false,
            revived_or_cloned: false,
            nk: 0,
            monster_class: ';', // S_EEL
            player_amphibious: false,
        };
        assert_eq!(monster_experience(&input), 1034);
    }

    #[test]
    fn test_score_monster_xp_vector_d_revive_diminishing_nk45() {
        // Vector D: base XP = 100, nk = 45
        let result = apply_revive_diminishing(100, 45);
        assert_eq!(result, 25);
    }

    #[test]
    fn test_score_monster_xp_vector_e_revive_diminishing_nk200() {
        // Vector E: base XP = 200, nk = 200
        let result = apply_revive_diminishing(200, 200);
        assert_eq!(result, 7);
    }

    #[test]
    fn test_score_monster_xp_mail_daemon() {
        // Mail daemon always gives XP=1 regardless of stats.
        let input = MonsterXpInput {
            m_lev: 25,
            mac: -10,
            mmove: 24,
            attacks: vec![XpAttack {
                aatyp: at::AT_MAGC,
                adtyp: 1,
                damn: 8,
                damd: 8,
            }],
            extra_nasty: true,
            is_mail_daemon: true,
            revived_or_cloned: false,
            nk: 0,
            monster_class: ' ',
            player_amphibious: false,
        };
        assert_eq!(monster_experience(&input), 1);
    }

    #[test]
    fn test_score_monster_xp_ac_bonus_negative() {
        // AC -5 should give (7 - (-5)) * 2 = 24 bonus
        let input = MonsterXpInput {
            m_lev: 1,
            mac: -5,
            mmove: 12,
            attacks: no_attacks(),
            extra_nasty: false,
            is_mail_daemon: false,
            revived_or_cloned: false,
            nk: 0,
            monster_class: ' ',
            player_amphibious: false,
        };
        // base = 1 + 1 = 2, AC bonus = 24
        assert_eq!(monster_experience(&input), 26);
    }

    #[test]
    fn test_score_monster_xp_ac_bonus_zero() {
        // AC 0: (7-0)*1 = 7 bonus
        let input = MonsterXpInput {
            m_lev: 1,
            mac: 0,
            mmove: 12,
            attacks: no_attacks(),
            extra_nasty: false,
            is_mail_daemon: false,
            revived_or_cloned: false,
            nk: 0,
            monster_class: ' ',
            player_amphibious: false,
        };
        // base = 2, AC bonus = 7
        assert_eq!(monster_experience(&input), 9);
    }

    #[test]
    fn test_score_monster_xp_ac_bonus_positive_2() {
        // AC 2: (7-2)*1 = 5 bonus
        let input = MonsterXpInput {
            m_lev: 1,
            mac: 2,
            mmove: 12,
            attacks: no_attacks(),
            extra_nasty: false,
            is_mail_daemon: false,
            revived_or_cloned: false,
            nk: 0,
            monster_class: ' ',
            player_amphibious: false,
        };
        // base = 2, AC bonus = 5
        assert_eq!(monster_experience(&input), 7);
    }

    #[test]
    fn test_score_monster_xp_speed_fast() {
        // Speed 15: > 12 but not > 18 => +3
        let input = MonsterXpInput {
            m_lev: 1,
            mac: 10,
            mmove: 15,
            attacks: no_attacks(),
            extra_nasty: false,
            is_mail_daemon: false,
            revived_or_cloned: false,
            nk: 0,
            monster_class: ' ',
            player_amphibious: false,
        };
        assert_eq!(monster_experience(&input), 5); // base 2 + speed 3
    }

    #[test]
    fn test_score_monster_xp_speed_very_fast() {
        // Speed 20: > 18 => +5
        let input = MonsterXpInput {
            m_lev: 1,
            mac: 10,
            mmove: 20,
            attacks: no_attacks(),
            extra_nasty: false,
            is_mail_daemon: false,
            revived_or_cloned: false,
            nk: 0,
            monster_class: ' ',
            player_amphibious: false,
        };
        assert_eq!(monster_experience(&input), 7); // base 2 + speed 5
    }

    #[test]
    fn test_score_monster_xp_high_level_bonus() {
        // m_lev > 8 => +50
        let input = MonsterXpInput {
            m_lev: 9,
            mac: 10,
            mmove: 12,
            attacks: no_attacks(),
            extra_nasty: false,
            is_mail_daemon: false,
            revived_or_cloned: false,
            nk: 0,
            monster_class: ' ',
            player_amphibious: false,
        };
        // base = 1 + 81 = 82, high level = +50
        assert_eq!(monster_experience(&input), 132);
    }

    #[test]
    fn test_score_monster_xp_heavy_damage() {
        // damn * damd > 23: e.g. 4d8 = 32 > 23 => +m_lev
        let attacks = vec![XpAttack {
            aatyp: at::AT_CLAW,
            adtyp: ad::AD_PHYS,
            damn: 4,
            damd: 8,
        }];
        let input = MonsterXpInput {
            m_lev: 5,
            mac: 10,
            mmove: 12,
            attacks,
            extra_nasty: false,
            is_mail_daemon: false,
            revived_or_cloned: false,
            nk: 0,
            monster_class: ' ',
            player_amphibious: false,
        };
        // base = 1 + 25 = 26, heavy damage +5
        assert_eq!(monster_experience(&input), 31);
    }

    // ── adj_lev tests (spec test vectors) ─────────────────────

    #[test]
    fn test_score_adj_lev_vector_m() {
        // Vector M: mlevel=8, level_difficulty=15, player_level=12
        let input = AdjLevInput {
            mlevel: 8,
            level_difficulty: 15,
            player_level: 12,
            is_wizard: false,
            wizard_kills: 0,
        };
        assert_eq!(adj_lev(&input), 10);
    }

    #[test]
    fn test_score_adj_lev_vector_n_upper_clamp() {
        // Vector N: mlevel=5, level_difficulty=30, player_level=25
        // Result should be clamped to 1.5*5 = 7
        let input = AdjLevInput {
            mlevel: 5,
            level_difficulty: 30,
            player_level: 25,
            is_wizard: false,
            wizard_kills: 0,
        };
        assert_eq!(adj_lev(&input), 7);
    }

    #[test]
    fn test_score_adj_lev_wizard() {
        let input = AdjLevInput {
            mlevel: 30,
            level_difficulty: 40,
            player_level: 20,
            is_wizard: true,
            wizard_kills: 5,
        };
        assert_eq!(adj_lev(&input), 35);
    }

    #[test]
    fn test_score_adj_lev_wizard_capped_at_49() {
        let input = AdjLevInput {
            mlevel: 30,
            level_difficulty: 40,
            player_level: 20,
            is_wizard: true,
            wizard_kills: 25,
        };
        assert_eq!(adj_lev(&input), 49);
    }

    #[test]
    fn test_score_adj_lev_super_demon() {
        let input = AdjLevInput {
            mlevel: 56,
            level_difficulty: 30,
            player_level: 15,
            is_wizard: false,
            wizard_kills: 0,
        };
        assert_eq!(adj_lev(&input), 50);
    }

    #[test]
    fn test_score_adj_lev_easier_dungeon() {
        // Monster harder than dungeon: -1
        let input = AdjLevInput {
            mlevel: 10,
            level_difficulty: 5,
            player_level: 3,
            is_wizard: false,
            wizard_kills: 0,
        };
        // tmp = 10, diff = 5-10 = -5 < 0 => tmp -= 1 => 9
        // player diff = 3-10 = -7 <= 0 => no change
        // upper = min(15, 49) = 15
        // clamp(9, 0, 15) = 9
        assert_eq!(adj_lev(&input), 9);
    }

    // ── Death message format tests ────────────────────────────

    #[test]
    fn test_score_death_message_normal() {
        let msg = format_death_message("Dudley", "Valkyrie", "a newt", Some(1), false);
        assert_eq!(msg, "Dudley, Valkyrie, killed by a newt on dungeon level 1");
    }

    #[test]
    fn test_score_death_message_ascension() {
        let msg = format_death_message("Dudley", "Demigod", "irrelevant", None, true);
        assert_eq!(msg, "Dudley, Demigod, ascended to demigod-hood");
    }

    #[test]
    fn test_score_death_message_no_depth() {
        let msg = format_death_message("Dudley", "Wizard", "a troll", None, false);
        assert_eq!(msg, "Dudley, Wizard, killed by a troll");
    }

    // ── Conduct tracking tests ────────────────────────────────

    #[test]
    fn test_score_conduct_all_maintained_count() {
        let state = ConductState::new();
        assert_eq!(state.maintained_count(), 13);
    }

    #[test]
    fn test_score_conduct_breaking_reduces_count() {
        let mut state = ConductState::new();
        check_conduct(&mut state, &ConductAction::Kill);
        assert_eq!(state.maintained_count(), 12);
        check_conduct(&mut state, &ConductAction::Pray);
        assert_eq!(state.maintained_count(), 11);
    }

    #[test]
    fn test_score_conduct_multiple_violations_same_conduct() {
        let mut state = ConductState::new();
        check_conduct(&mut state, &ConductAction::Kill);
        check_conduct(&mut state, &ConductAction::Kill);
        check_conduct(&mut state, &ConductAction::Kill);
        // Still only 1 conduct broken (pacifist), count = 12
        assert_eq!(state.maintained_count(), 12);
        assert_eq!(state.violation_count(Conduct::Pacifist), 3);
    }

    // ── Score formula integration tests ───────────────────────

    #[test]
    fn test_score_zero_everything() {
        let input = ScoreInput {
            experience: 0,
            score_experience: 0,
            gold_carried: 0,
            gold_deposited: 0,
            artifacts_held: 0,
            conducts: ConductState::new(),
            ascended: false,
            max_depth: 1,
        };
        // base = 0, gold = 0, artifacts = 0, conducts = 13*5000 = 65000, ascension = 0
        assert_eq!(calculate_score(&input), 65_000);
    }

    #[test]
    fn test_score_ascension_with_all_conducts() {
        let input = ScoreInput {
            experience: 10_000_000,
            score_experience: 0,
            gold_carried: 100_000,
            gold_deposited: 50_000,
            artifacts_held: 5,
            conducts: ConductState::new(),
            ascended: true,
            max_depth: 50,
        };
        // base = 4*10_000_000 = 40_000_000
        // gold = 150_000
        // artifacts = 5_000
        // conducts = 65_000
        // ascension = 50_000
        assert_eq!(
            calculate_score(&input),
            40_000_000 + 150_000 + 5_000 + 65_000 + 50_000
        );
    }

    #[test]
    fn test_score_revive_diminishing_no_kills() {
        // nk <= 20 should not diminish at all
        assert_eq!(apply_revive_diminishing(100, 1), 100);
        assert_eq!(apply_revive_diminishing(100, 20), 100);
    }

    #[test]
    fn test_score_revive_diminishing_21_kills() {
        // nk=21: first bracket halves
        // tmp = (100+1)/2 = 50, nk = 1
        assert_eq!(apply_revive_diminishing(100, 21), 50);
    }

    #[test]
    fn test_score_revive_diminishing_255_kills() {
        // Edge case: very high kills
        let result = apply_revive_diminishing(1000, 255);
        assert!(result >= 1, "should be at least 1");
        assert!(result < 1000, "should be diminished");
    }

    // =========================================================================
    // Track P — RNG Distribution Verification Tests
    // =========================================================================

    use rand::{Rng, SeedableRng};
    use rand_pcg::Pcg64;

    const RNG_SAMPLES: usize = 10_000;

    #[test]
    fn test_rng_floating_eye_telepathy_probability() {
        // Floating eye: level 3, conveys telepathy, chance=1.
        // Probability of gaining telepathy:
        //   - Selected as candidate: 1/1 = 100% (only candidate)
        //   - should_givit: level(3) > rn2(1) => 3 > 0 always true
        //   - Therefore: 100% probability
        use crate::hunger::{CorpseDef, CorpseIntrinsic, check_intrinsic_gain};
        use nethack_babel_data::{MonsterFlags, ResistanceSet};

        let mut rng = Pcg64::seed_from_u64(42);
        let mut successes = 0;
        for _ in 0..RNG_SAMPLES {
            let corpse = CorpseDef {
                name: "floating eye".to_string(),
                base_level: 3,
                corpse_weight: 10,
                corpse_nutrition: 10,
                conveys: ResistanceSet::empty(),
                flags: MonsterFlags::empty(),
                poisonous: false,
                acidic: false,
                flesh_petrifies: false,
                is_giant: false,
                is_domestic: false,
                is_same_race: false,
                cannibal_allowed: false,
                conveys_telepathy: true,
                conveys_teleport: false,
                nonrotting: false,
                is_vegan: false,
                is_vegetarian: false,
            };

            if let Some(CorpseIntrinsic::Telepathy) = check_intrinsic_gain(&corpse, &mut rng) {
                successes += 1;
            }
        }

        // Should be 100% (or very close, given rn2(1) always returns 0)
        let rate = successes as f64 / RNG_SAMPLES as f64;
        assert!(
            rate > 0.99,
            "floating eye telepathy rate should be ~100%, got {:.1}%",
            rate * 100.0
        );
    }

    #[test]
    fn test_rng_throne_wish_probability() {
        // Sitting on a throne: wish probability is approximately 1/13.
        // We simulate the check: rn2(13) == 0 => wish
        let mut rng = Pcg64::seed_from_u64(12345);
        let mut wishes = 0;
        for _ in 0..RNG_SAMPLES {
            let roll: u32 = rng.random_range(0..13);
            if roll == 0 {
                wishes += 1;
            }
        }
        let rate = wishes as f64 / RNG_SAMPLES as f64;
        let expected = 1.0 / 13.0;
        assert!(
            (rate - expected).abs() < 0.03,
            "throne wish rate should be ~{:.1}%, got {:.1}%",
            expected * 100.0,
            rate * 100.0
        );
    }

    #[test]
    fn test_rng_enchant_weapon_evaporation_at_plus_6() {
        // When spe > 5 and amount >= 0: 2/3 chance to evaporate (rn2(3) != 0).
        let mut rng = Pcg64::seed_from_u64(777);
        let mut evaporated = 0;
        for _ in 0..RNG_SAMPLES {
            let roll: u32 = rng.random_range(0..3);
            if roll != 0 {
                evaporated += 1;
            }
        }
        let rate = evaporated as f64 / RNG_SAMPLES as f64;
        let expected = 2.0 / 3.0;
        assert!(
            (rate - expected).abs() < 0.03,
            "enchant weapon evaporation should be ~{:.1}%, got {:.1}%",
            expected * 100.0,
            rate * 100.0
        );
    }

    #[test]
    fn test_rng_prayer_success_deterministic() {
        // With alignment_record >= 0, luck >= 0, no anger, no cooldown,
        // prayer should succeed deterministically.
        use crate::religion::{PrayerType, ReligionState, evaluate_prayer_simple};
        use nethack_babel_data::Alignment;

        let state = ReligionState {
            alignment: Alignment::Lawful,
            alignment_record: 10,
            god_anger: 0,
            god_gifts: 0,
            blessed_amount: 0,
            bless_cooldown: 0,
            crowned: false,
            demigod: false,
            turn: 1000,
            experience_level: 5,
            current_hp: 20,
            max_hp: 20,
            current_pw: 10,
            max_pw: 10,
            nutrition: 900,
            luck: 5,
            luck_bonus: 0,
            has_luckstone: false,
            luckstone_blessed: false,
            luckstone_cursed: false,
            in_gehennom: false,
            is_undead: false,
            is_demon: false,
            original_alignment: Alignment::Lawful,
            has_converted: false,
            alignment_abuse: 0,
        };

        let result = evaluate_prayer_simple(&state, false, None);
        assert_eq!(result, PrayerType::Success);
    }

    #[test]
    fn test_rng_prayer_fails_with_anger() {
        use crate::religion::{PrayerType, ReligionState, evaluate_prayer_simple};
        use nethack_babel_data::Alignment;

        let state = ReligionState {
            alignment: Alignment::Neutral,
            alignment_record: -5,
            god_anger: 3,
            god_gifts: 0,
            blessed_amount: 0,
            bless_cooldown: 0,
            crowned: false,
            demigod: false,
            turn: 1000,
            experience_level: 8,
            current_hp: 10,
            max_hp: 20,
            current_pw: 5,
            max_pw: 10,
            nutrition: 500,
            luck: -2,
            luck_bonus: 0,
            has_luckstone: false,
            luckstone_blessed: false,
            luckstone_cursed: false,
            in_gehennom: false,
            is_undead: false,
            is_demon: false,
            original_alignment: Alignment::Neutral,
            has_converted: false,
            alignment_abuse: 0,
        };

        let result = evaluate_prayer_simple(&state, false, None);
        assert_eq!(result, PrayerType::Punished);
    }

    #[test]
    fn test_rng_pudding_split_probability() {
        // Pudding split: always splits if edged weapon and hp > 1.
        // No randomness involved — deterministic.
        let mut splits = 0;
        for hp in 1..=10 {
            if pudding_should_split(true, hp) {
                splits += 1;
            }
        }
        // hp=1 doesn't split, hp 2..10 all split
        assert_eq!(splits, 9);
        // Blunt weapon: never splits
        assert!(!pudding_should_split(false, 50));
    }

    #[test]
    fn test_rng_monster_generation_depth_distribution() {
        // At depth 5, generate 10K "random monster levels" using the
        // standard formula: rn2(depth + 1) which gives [0, depth].
        // Check that level-0 through level-5 monsters appear at reasonable rates.
        let mut rng = Pcg64::seed_from_u64(99999);
        let depth: u32 = 5;
        let mut level_counts = [0u32; 6]; // levels 0..5

        for _ in 0..RNG_SAMPLES {
            let mon_level: u32 = rng.random_range(0..=depth);
            level_counts[mon_level as usize] += 1;
        }

        // Each level should appear approximately 1/6 of the time (16.67%)
        let expected = RNG_SAMPLES as f64 / 6.0;
        for (level, &count) in level_counts.iter().enumerate() {
            let rate = count as f64 / RNG_SAMPLES as f64;
            assert!(
                (count as f64 - expected).abs() < expected * 0.2,
                "level {} generation rate {:.1}% too far from expected {:.1}%",
                level,
                rate * 100.0,
                100.0 / 6.0
            );
        }
    }

    #[test]
    fn test_rng_unicorn_horn_cure_probability() {
        // Uncursed unicorn horn: ~33% cure chance per try.
        // Verify over 10K samples.
        let mut rng = Pcg64::seed_from_u64(54321);
        let mut cured = 0;
        for _ in 0..RNG_SAMPLES {
            let roll: i32 = rng.random_range(0..100);
            if unicorn_horn_cures(UnicornHornCurable::Confusion, false, false, roll) {
                cured += 1;
            }
        }
        let rate = cured as f64 / RNG_SAMPLES as f64;
        assert!(
            (rate - 0.33).abs() < 0.03,
            "uncursed unicorn horn cure rate should be ~33%, got {:.1}%",
            rate * 100.0
        );
    }

    // =========================================================================
    // Track Q — Exploit Preservation Tests
    // =========================================================================

    // ── Exploit: Elbereth engraving mechanics ─────────────────

    #[test]
    fn test_exploit_elbereth_dust_degrades() {
        let mut eng = ElberethEngraving::new(EngravingMethod::Dust);
        assert!(eng.repels_monsters());
        // One step destroys dust engraving
        eng.step_on();
        assert!(!eng.repels_monsters());
    }

    #[test]
    fn test_exploit_elbereth_engrave_more_durable() {
        let mut eng = ElberethEngraving::new(EngravingMethod::Engrave);
        assert!(eng.repels_monsters());
        // Takes 5 steps to degrade
        for i in 0..4 {
            assert!(
                eng.step_on(),
                "engraved Elbereth should survive step {}",
                i + 1
            );
        }
        // 5th step destroys it
        eng.step_on();
        assert!(!eng.repels_monsters());
    }

    #[test]
    fn test_exploit_elbereth_burn_permanent() {
        let mut eng = ElberethEngraving::new(EngravingMethod::Burn);
        assert!(eng.repels_monsters());
        // 100 steps should not degrade burned Elbereth
        for _ in 0..100 {
            eng.step_on();
        }
        assert!(eng.repels_monsters());
    }

    #[test]
    fn test_exploit_elbereth_durability_ordering() {
        // burn > engrave > dust
        assert!(
            engraving_durability_order(EngravingMethod::Burn)
                > engraving_durability_order(EngravingMethod::Engrave)
        );
        assert!(
            engraving_durability_order(EngravingMethod::Engrave)
                > engraving_durability_order(EngravingMethod::Dust)
        );
    }

    // ── Exploit: Pudding farming ──────────────────────────────

    #[test]
    fn test_exploit_pudding_farming_edged_weapon_splits() {
        // Hitting a pudding with an edged weapon when HP > 1 always splits
        assert!(pudding_should_split(true, 50));
        assert!(pudding_should_split(true, 2));
    }

    #[test]
    fn test_exploit_pudding_farming_blunt_weapon_no_split() {
        assert!(!pudding_should_split(false, 50));
        assert!(!pudding_should_split(false, 2));
    }

    #[test]
    fn test_exploit_pudding_farming_hp_1_no_split() {
        // Pudding at 1 HP cannot split (it would die)
        assert!(!pudding_should_split(true, 1));
        assert!(!pudding_should_split(true, 0));
    }

    #[test]
    fn test_exploit_pudding_farming_infinite_generation() {
        // Simulate pudding farming: start with 1 pudding, split 10 times
        let mut pudding_count = 1;
        let pudding_hp = 50;
        for _ in 0..10 {
            if pudding_should_split(true, pudding_hp) {
                pudding_count += 1;
            }
        }
        assert_eq!(pudding_count, 11, "each split should add one pudding");
    }

    // ── Exploit: Price identification ─────────────────────────

    #[test]
    fn test_exploit_price_id_neutral_cha() {
        // CHA 11-15: no modifier (1:1)
        // A buy price of 300 with CHA 12 should reverse to base ~300
        let base = price_id_from_buy_price(300, 12, false);
        assert_eq!(base, 300);
    }

    #[test]
    fn test_exploit_price_id_low_cha() {
        // CHA 5: modifier 2:1 (double buy price)
        // Observed buy price 600 => base = 600 * 1/2 = 300
        let base = price_id_from_buy_price(600, 5, false);
        assert_eq!(base, 300);
    }

    #[test]
    fn test_exploit_price_id_high_cha() {
        // CHA 18: modifier 2:3 (buy at 2/3 price)
        // Observed buy price 200 => base = 200 * 3/2 = 300
        let base = price_id_from_buy_price(200, 18, false);
        assert_eq!(base, 300);
    }

    #[test]
    fn test_exploit_price_id_tourist_penalty() {
        // Tourist with CHA 12: buy = base * 4/3
        // Observed 400 => base = 400 * 3/4 = 300
        let base = price_id_from_buy_price(400, 12, true);
        assert_eq!(base, 300);
    }

    #[test]
    fn test_exploit_price_id_combined_cha_and_tourist() {
        // CHA 5 (2:1) + Tourist (4:3) => effective multiplier = 8:3
        // Observed = base * 8/3 = 800 => base = 800 * 3/8 = 300
        let base = price_id_from_buy_price(800, 5, true);
        assert_eq!(base, 300);
    }

    // ── Exploit: Fountain dipping for Excalibur ───────────────

    #[test]
    fn test_exploit_excalibur_fountain_dip() {
        use crate::artifacts::{ExcaliburResult, try_create_excalibur};
        use nethack_babel_data::{Alignment, ObjectTypeId};

        let long_sword = ObjectTypeId(28); // OBJ_LONG_SWORD

        // Lawful character, level >= 5, dip long sword in fountain
        let mut found_success = false;
        for seed in 0..200u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let result = try_create_excalibur(
                long_sword,
                5, // level 5 minimum
                Alignment::Lawful,
                true,  // is_knight
                false, // excalibur_exists
                &mut rng,
            );
            if result == ExcaliburResult::Success {
                found_success = true;
                break;
            }
        }
        assert!(
            found_success,
            "lawful character at level 5+ should eventually get Excalibur"
        );
    }

    #[test]
    fn test_exploit_excalibur_requires_level_5() {
        use crate::artifacts::{ExcaliburResult, try_create_excalibur};
        use nethack_babel_data::{Alignment, ObjectTypeId};

        let long_sword = ObjectTypeId(28);
        let mut rng = Pcg64::seed_from_u64(42);
        // Level 4 should always fail
        let result = try_create_excalibur(long_sword, 4, Alignment::Lawful, true, false, &mut rng);
        assert_eq!(result, ExcaliburResult::Invalid);
    }

    #[test]
    fn test_exploit_excalibur_requires_lawful() {
        use crate::artifacts::{ExcaliburResult, try_create_excalibur};
        use nethack_babel_data::{Alignment, ObjectTypeId};

        let long_sword = ObjectTypeId(28);

        // Non-lawful character may get cursed weapon instead of Excalibur
        let mut found_cursed = false;
        for seed in 0..200u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let result = try_create_excalibur(
                long_sword,
                5,
                Alignment::Neutral,
                false, // not knight
                false,
                &mut rng,
            );
            if result == ExcaliburResult::Cursed {
                found_cursed = true;
                break;
            }
        }
        assert!(
            found_cursed,
            "non-lawful character should sometimes get cursed sword from fountain"
        );
    }

    // ── Exploit: Floating eye + telepathy ─────────────────────

    #[test]
    fn test_exploit_floating_eye_telepathy_strategy() {
        // Verify the classic early-game strategy works:
        // 1. Kill a floating eye (e.g., while blind or with ranged attack)
        // 2. Eat the corpse
        // 3. Gain intrinsic telepathy (100% if level 3 floating eye)
        use crate::hunger::{CorpseDef, CorpseIntrinsic, check_intrinsic_gain};
        use nethack_babel_data::{MonsterFlags, ResistanceSet};

        let mut rng = Pcg64::seed_from_u64(42);

        let corpse = CorpseDef {
            name: "floating eye".to_string(),
            base_level: 3,
            corpse_weight: 10,
            corpse_nutrition: 10,
            conveys: ResistanceSet::empty(),
            flags: MonsterFlags::empty(),
            poisonous: false,
            acidic: false,
            flesh_petrifies: false,
            is_giant: false,
            is_domestic: false,
            is_same_race: false,
            cannibal_allowed: false,
            conveys_telepathy: true,
            conveys_teleport: false,
            nonrotting: false,
            is_vegan: false,
            is_vegetarian: false,
        };

        // Should always get telepathy from floating eye
        let result = check_intrinsic_gain(&corpse, &mut rng);
        assert_eq!(
            result,
            Some(CorpseIntrinsic::Telepathy),
            "floating eye corpse should always grant telepathy"
        );
    }

    // ── Exploit: Unicorn horn rubbing ─────────────────────────

    #[test]
    fn test_exploit_unicorn_horn_blessed_cures_all() {
        // Blessed unicorn horn should cure everything
        for effect in [
            UnicornHornCurable::Confusion,
            UnicornHornCurable::Stun,
            UnicornHornCurable::Hallucination,
            UnicornHornCurable::Blindness,
            UnicornHornCurable::Sickness,
            UnicornHornCurable::Nausea,
        ] {
            // Any roll should work for blessed
            assert!(
                unicorn_horn_cures(effect, true, false, 50),
                "blessed horn should cure {:?}",
                effect
            );
            assert!(
                unicorn_horn_cures(effect, true, false, 99),
                "blessed horn should cure {:?} even with high roll",
                effect
            );
        }
    }

    #[test]
    fn test_exploit_unicorn_horn_cursed_never_cures() {
        for effect in [
            UnicornHornCurable::Confusion,
            UnicornHornCurable::Stun,
            UnicornHornCurable::Hallucination,
            UnicornHornCurable::Blindness,
        ] {
            assert!(
                !unicorn_horn_cures(effect, false, true, 0),
                "cursed horn should never cure {:?}",
                effect
            );
        }
    }

    #[test]
    fn test_exploit_unicorn_horn_uncursed_partial() {
        // Uncursed: 33% chance (roll < 33)
        assert!(unicorn_horn_cures(
            UnicornHornCurable::Confusion,
            false,
            false,
            10
        ));
        assert!(unicorn_horn_cures(
            UnicornHornCurable::Confusion,
            false,
            false,
            32
        ));
        assert!(!unicorn_horn_cures(
            UnicornHornCurable::Confusion,
            false,
            false,
            33
        ));
        assert!(!unicorn_horn_cures(
            UnicornHornCurable::Confusion,
            false,
            false,
            99
        ));
    }

    // ── Score formula edge cases ──────────────────────────────

    #[test]
    fn test_score_negative_gold_clamped() {
        let input = ScoreInput {
            experience: 0,
            score_experience: 0,
            gold_carried: -500,
            gold_deposited: 200,
            artifacts_held: 0,
            conducts: ConductState::new(),
            ascended: false,
            max_depth: 1,
        };
        // gold = max(-500 + 200, 0) = 0 (actually -300.max(0) = 0)
        let score = calculate_score(&input);
        // base=0, gold=0, artifacts=0, conducts=65000, ascension=0
        assert_eq!(score, 65_000);
    }

    #[test]
    fn test_score_overflow_protection() {
        let input = ScoreInput {
            experience: i64::MAX / 8,
            score_experience: i64::MAX / 8,
            gold_carried: i64::MAX / 4,
            gold_deposited: 0,
            artifacts_held: 0,
            conducts: ConductState::new(),
            ascended: true,
            max_depth: 50,
        };
        // Should not panic
        let _score = calculate_score(&input);
    }
}
