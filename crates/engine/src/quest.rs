//! Quest system: state tracking, eligibility checks, encounter logic,
//! dialogue/text system, and role-specific quest data.
//!
//! The quest is a role-specific dungeon branch where the player must meet
//! their quest leader, defeat the quest nemesis, and retrieve the quest
//! artifact.  Eligibility requires experience level 14+ and positive
//! alignment record.
//!
//! Reference: `specs/quest.md`, C sources `src/questpgr.c`, `src/quest.c`.

use nethack_babel_data::Alignment;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::event::EngineEvent;
use crate::role::Role;

// ---------------------------------------------------------------------------
// Quest status
// ---------------------------------------------------------------------------

/// Overall progress through the quest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuestStatus {
    /// The player has not yet been offered the quest.
    NotStarted,
    /// The quest leader has assigned the quest.
    Assigned,
    /// The player has entered the quest dungeon.
    InProgress,
    /// The player has defeated the nemesis and obtained the artifact.
    Completed,
}

/// Tracks the player's progress through the quest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestState {
    pub status: QuestStatus,
    pub leader_met: bool,
    pub nemesis_defeated: bool,
    pub artifact_obtained: bool,
    pub quest_dungeon_entered: bool,
    /// Number of times the leader has rejected the player.
    pub times_rejected: u32,
    /// Number of times the player has been expelled from the quest.
    pub times_expelled: u32,
    /// Whether the leader is angry (player attacked them).
    pub leader_angry: bool,
    /// Whether the nemesis is known to be dead (stinky corpse message).
    pub nemesis_stinky: bool,
    /// Gender index for the deity (0=male, 1=female, 2=neuter).
    pub god_gender: u8,
    /// Gender index for the leader.
    pub leader_gender: u8,
    /// Gender index for the nemesis.
    pub nemesis_gender: u8,
}

impl Default for QuestState {
    fn default() -> Self {
        Self {
            status: QuestStatus::NotStarted,
            leader_met: false,
            nemesis_defeated: false,
            artifact_obtained: false,
            quest_dungeon_entered: false,
            times_rejected: 0,
            times_expelled: 0,
            leader_angry: false,
            nemesis_stinky: false,
            god_gender: 0,
            leader_gender: 0,
            nemesis_gender: 0,
        }
    }
}

impl QuestState {
    /// Create a new quest state (not started).
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the quest leader as met.
    pub fn meet_leader(&mut self) {
        self.leader_met = true;
    }

    /// Assign the quest (leader offers it after eligibility check).
    pub fn assign(&mut self) {
        self.status = QuestStatus::Assigned;
    }

    /// Mark the quest dungeon as entered.
    pub fn enter_quest_dungeon(&mut self) {
        self.quest_dungeon_entered = true;
        if self.status == QuestStatus::Assigned {
            self.status = QuestStatus::InProgress;
        }
    }

    /// Mark the nemesis as defeated.
    pub fn defeat_nemesis(&mut self) {
        self.nemesis_defeated = true;
    }

    /// Mark the quest artifact as obtained.
    pub fn obtain_artifact(&mut self) {
        self.artifact_obtained = true;
    }

    /// Whether the player has done enough work to report back to the leader.
    pub fn ready_for_completion(&self) -> bool {
        self.nemesis_defeated && self.artifact_obtained
    }

    /// Mark the quest as completed after reporting back to the leader.
    pub fn complete(&mut self) {
        if self.ready_for_completion() {
            self.status = QuestStatus::Completed;
        }
    }

    /// Record that the leader rejected the player this visit.
    pub fn reject(&mut self) {
        self.times_rejected += 1;
    }

    /// Record that the player was expelled from the quest dungeon.
    pub fn expel(&mut self) {
        self.times_expelled += 1;
    }

    /// Mark the leader as angry (player attacked them).
    pub fn anger_leader(&mut self) {
        self.leader_angry = true;
    }
}

// ---------------------------------------------------------------------------
// Eligibility
// ---------------------------------------------------------------------------

/// Check whether the player is eligible to receive the quest.
///
/// Requirements (from NetHack 3.7):
/// - Experience level 14 or higher
/// - Positive alignment record (> 0)
///
/// Returns `true` if the player meets both criteria.
pub fn check_quest_eligibility(_role: Role, level: u8, alignment_record: i32) -> bool {
    level >= 14 && alignment_record > 0
}

// ---------------------------------------------------------------------------
// Role → quest artifact
// ---------------------------------------------------------------------------

/// Return the name of the quest artifact for the given role.
///
/// Every role has exactly one quest artifact.  These names match the
/// artifact table in `artifacts.rs`.
pub fn quest_artifact_for_role(role: Role) -> &'static str {
    match role {
        Role::Archeologist => "The Orb of Detection",
        Role::Barbarian => "The Heart of Ahriman",
        Role::Caveperson => "The Sceptre of Might",
        Role::Healer => "The Staff of Aesculapius",
        Role::Knight => "The Magic Mirror of Merlin",
        Role::Monk => "The Eyes of the Overworld",
        Role::Priest => "The Mitre of Holiness",
        Role::Ranger => "The Longbow of Diana",
        Role::Rogue => "The Master Key of Thievery",
        Role::Samurai => "The Tsurugi of Muramasa",
        Role::Tourist => "The Platinum Yendorian Express Card",
        Role::Valkyrie => "The Orb of Fate",
        Role::Wizard => "The Eye of the Aethiopica",
    }
}

// ---------------------------------------------------------------------------
// Role → quest nemesis
// ---------------------------------------------------------------------------

/// Return the name of the quest nemesis for the given role.
pub fn quest_nemesis_for_role(role: Role) -> &'static str {
    match role {
        Role::Archeologist => "the Minion of Huhetotl",
        Role::Barbarian => "Thoth Amon",
        Role::Caveperson => "Chromatic Dragon",
        Role::Healer => "Cyclops",
        Role::Knight => "Ixoth",
        Role::Monk => "Master Kaen",
        Role::Priest => "Nalzok",
        Role::Ranger => "Scorpius",
        Role::Rogue => "Master Assassin",
        Role::Samurai => "Ashikaga Takauji",
        Role::Tourist => "Master of Thieves",
        Role::Valkyrie => "Lord Surtur",
        Role::Wizard => "Dark One",
    }
}

// ---------------------------------------------------------------------------
// Role → quest leader
// ---------------------------------------------------------------------------

/// Return the name of the quest leader for the given role.
pub fn quest_leader_for_role(role: Role) -> &'static str {
    match role {
        Role::Archeologist => "Lord Carnarvon",
        Role::Barbarian => "Pelias",
        Role::Caveperson => "Shaman Karnov",
        Role::Healer => "Hippocrates",
        Role::Knight => "King Arthur",
        Role::Monk => "Grand Master",
        Role::Priest => "Arch Priest",
        Role::Ranger => "Orion",
        Role::Rogue => "Master of Thieves",
        Role::Samurai => "Lord Sato",
        Role::Tourist => "Twoflower",
        Role::Valkyrie => "Norn",
        Role::Wizard => "Neferet the Green",
    }
}

// ---------------------------------------------------------------------------
// Role → quest guardian
// ---------------------------------------------------------------------------

/// Return the name of the quest guardian monster for the given role.
pub fn quest_guardian_for_role(role: Role) -> &'static str {
    match role {
        Role::Archeologist => "human",
        Role::Barbarian => "chieftain",
        Role::Caveperson => "caveman",
        Role::Healer => "attendant",
        Role::Knight => "page",
        Role::Monk => "abbot",
        Role::Priest => "acolyte",
        Role::Ranger => "hunter",
        Role::Rogue => "guide",
        Role::Samurai => "roshi",
        Role::Tourist => "guide",
        Role::Valkyrie => "warrior",
        Role::Wizard => "apprentice",
    }
}

// ---------------------------------------------------------------------------
// Role → quest homebase
// ---------------------------------------------------------------------------

/// Return the quest home level name for the given role.
pub fn quest_homebase_for_role(role: Role) -> &'static str {
    match role {
        Role::Archeologist => "the Camp",
        Role::Barbarian => "the Camp",
        Role::Caveperson => "the Caves of the Ancestors",
        Role::Healer => "the Temple of Epidaurus",
        Role::Knight => "Camelot Castle",
        Role::Monk => "the Monastery of Chan-Sune",
        Role::Priest => "the Great Temple",
        Role::Ranger => "Orion's camp",
        Role::Rogue => "the Thieves' Guild Hall",
        Role::Samurai => "the Castle of the Taro Clan",
        Role::Tourist => "Stratos Airlines",
        Role::Valkyrie => "the Shrine of Destiny",
        Role::Wizard => "the Tower of Darkness",
    }
}

// ---------------------------------------------------------------------------
// Role → intermediate quest target
// ---------------------------------------------------------------------------

/// Return the intermediate location string for the given role's quest.
pub fn quest_intermed_for_role(role: Role) -> &'static str {
    match role {
        Role::Archeologist => "the Tomb of the Toltec Kings",
        Role::Barbarian => "the Duali Forest",
        Role::Caveperson => "the Dragon's Lair",
        Role::Healer => "the Isle of the Oracle",
        Role::Knight => "the Questing Beast's lair",
        Role::Monk => "the Monastery of the Earth-Lord",
        Role::Priest => "the Temple of Nalzok",
        Role::Ranger => "Scorpius's lair",
        Role::Rogue => "the Assassins' Guild Hall",
        Role::Samurai => "the Shogun's Castle",
        Role::Tourist => "Thieves' Guild Hall",
        Role::Valkyrie => "the cave of Surtur",
        Role::Wizard => "the Tower of Darkness",
    }
}

// ---------------------------------------------------------------------------
// Role → filecode (for quest text lookup)
// ---------------------------------------------------------------------------

/// Return the quest text file section key for the given role.
///
/// In C NetHack, this is `urole.filecode`, used for `quest.lua` lookup.
pub fn quest_filecode_for_role(role: Role) -> &'static str {
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

// ---------------------------------------------------------------------------
// Role → enemy types
// ---------------------------------------------------------------------------

/// Primary and secondary quest enemy types for a role.
#[derive(Debug, Clone)]
pub struct QuestEnemies {
    /// Primary enemy name (80% chance to encounter).
    pub enemy1: &'static str,
    /// Secondary enemy name (20% chance to encounter).
    pub enemy2: &'static str,
}

/// Return the quest enemy types for the given role.
pub fn quest_enemies_for_role(role: Role) -> QuestEnemies {
    match role {
        Role::Archeologist => QuestEnemies {
            enemy1: "snake",
            enemy2: "human mummy",
        },
        Role::Barbarian => QuestEnemies {
            enemy1: "ogre",
            enemy2: "troll",
        },
        Role::Caveperson => QuestEnemies {
            enemy1: "jaguar",
            enemy2: "bugbear",
        },
        Role::Healer => QuestEnemies {
            enemy1: "giant rat",
            enemy2: "snake",
        },
        Role::Knight => QuestEnemies {
            enemy1: "quasit",
            enemy2: "ochre jelly",
        },
        Role::Monk => QuestEnemies {
            enemy1: "earth elemental",
            enemy2: "xorn",
        },
        Role::Priest => QuestEnemies {
            enemy1: "human zombie",
            enemy2: "wraith",
        },
        Role::Ranger => QuestEnemies {
            enemy1: "centaur",
            enemy2: "scorpion",
        },
        Role::Rogue => QuestEnemies {
            enemy1: "leprechaun",
            enemy2: "guardian naga",
        },
        Role::Samurai => QuestEnemies {
            enemy1: "ninja",
            enemy2: "ronin",
        },
        Role::Tourist => QuestEnemies {
            enemy1: "giant spider",
            enemy2: "forest centaur",
        },
        Role::Valkyrie => QuestEnemies {
            enemy1: "fire giant",
            enemy2: "frost giant",
        },
        Role::Wizard => QuestEnemies {
            enemy1: "vampire bat",
            enemy2: "xorn",
        },
    }
}

/// Select a quest monster type name: 80% primary, 20% secondary.
pub fn select_quest_monster<R: Rng>(role: Role, rng: &mut R) -> &'static str {
    let enemies = quest_enemies_for_role(role);
    if rng.random_range(0..5) != 0 {
        enemies.enemy1
    } else {
        enemies.enemy2
    }
}

// ---------------------------------------------------------------------------
// Quest text placeholder expansion
// ---------------------------------------------------------------------------

/// Context for expanding quest text placeholders.
#[derive(Debug, Clone)]
pub struct QuestTextContext {
    pub player_name: String,
    pub role: Role,
    pub role_name: String,
    pub rank: String,
    pub is_female: bool,
    pub alignment: Alignment,
    pub deity_name: String,
}

/// Expand quest text placeholders (`%p`, `%c`, `%l`, `%n`, etc.)
///
/// Mirrors `convert_arg()` from `questpgr.c`.
pub fn expand_quest_placeholder(placeholder: char, ctx: &QuestTextContext) -> String {
    match placeholder {
        'p' => ctx.player_name.clone(),
        'c' => ctx.role_name.clone(),
        'r' => ctx.rank.clone(),
        's' => {
            if ctx.is_female {
                "sister".to_string()
            } else {
                "brother".to_string()
            }
        }
        'S' => {
            if ctx.is_female {
                "daughter".to_string()
            } else {
                "son".to_string()
            }
        }
        'l' => quest_leader_for_role(ctx.role).to_string(),
        'n' => quest_nemesis_for_role(ctx.role).to_string(),
        'g' => quest_guardian_for_role(ctx.role).to_string(),
        'o' => quest_artifact_for_role(ctx.role).to_string(),
        'H' => quest_homebase_for_role(ctx.role).to_string(),
        'i' => quest_intermed_for_role(ctx.role).to_string(),
        'a' => match ctx.alignment {
            Alignment::Lawful => "lawful".to_string(),
            Alignment::Neutral => "neutral".to_string(),
            Alignment::Chaotic => "chaotic".to_string(),
        },
        'd' => ctx.deity_name.clone(),
        'L' => "lawful".to_string(),
        'N' => "neutral".to_string(),
        'C' => "chaotic".to_string(),
        '%' => "%".to_string(),
        _ => String::new(),
    }
}

/// Expand all `%X` placeholders in a quest text string.
pub fn expand_quest_text(text: &str, ctx: &QuestTextContext) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            if let Some(&next) = chars.peek() {
                let expanded = expand_quest_placeholder(next, ctx);
                result.push_str(&expanded);
                chars.next();
            } else {
                result.push('%');
            }
        } else {
            result.push(ch);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Quest encounters
// ---------------------------------------------------------------------------

/// The type of NPC encounter in the quest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestEncounterType {
    /// Meeting the quest leader for the first time.
    LeaderFirst,
    /// Meeting the quest leader after being rejected.
    LeaderNext,
    /// Meeting the quest leader after being assigned the quest.
    LeaderAssigned,
    /// Meeting the quest leader after nemesis is defeated.
    LeaderNemesisDead,
    /// Encountering quest guardians.
    Guardian,
    /// First encounter with the quest nemesis.
    NemesisFirst,
    /// Subsequent encounters with the quest nemesis.
    NemesisNext,
    /// Final encounter with the quest nemesis (carrying artifact).
    NemesisWithArtifact,
    /// The nemesis has been killed.
    NemesisDead,
}

/// Determine the appropriate encounter type based on quest state and NPC.
pub fn determine_encounter(
    quest_state: &QuestState,
    is_leader: bool,
    is_nemesis: bool,
) -> QuestEncounterType {
    if is_leader {
        if quest_state.status == QuestStatus::Completed || quest_state.ready_for_completion() {
            QuestEncounterType::LeaderNemesisDead
        } else if quest_state.status == QuestStatus::Assigned
            || quest_state.status == QuestStatus::InProgress
        {
            QuestEncounterType::LeaderAssigned
        } else if quest_state.leader_met {
            QuestEncounterType::LeaderNext
        } else {
            QuestEncounterType::LeaderFirst
        }
    } else if is_nemesis {
        if quest_state.nemesis_defeated {
            QuestEncounterType::NemesisDead
        } else if quest_state.artifact_obtained {
            QuestEncounterType::NemesisWithArtifact
        } else if quest_state.quest_dungeon_entered {
            QuestEncounterType::NemesisNext
        } else {
            QuestEncounterType::NemesisFirst
        }
    } else {
        QuestEncounterType::Guardian
    }
}

/// Generate the quest text message key for a given encounter type.
///
/// Returns the key name used for quest dialogue lookup, e.g.
/// `"quest-leader-first"`, `"quest-nemesis-dead"`.
pub fn quest_message_key(encounter: QuestEncounterType) -> &'static str {
    match encounter {
        QuestEncounterType::LeaderFirst => "quest-leader-first",
        QuestEncounterType::LeaderNext => "quest-leader-next",
        QuestEncounterType::LeaderAssigned => "quest-leader-assigned",
        QuestEncounterType::LeaderNemesisDead => "quest-leader-nemesis-dead",
        QuestEncounterType::Guardian => "quest-guardian",
        QuestEncounterType::NemesisFirst => "quest-nemesis-first",
        QuestEncounterType::NemesisNext => "quest-nemesis-next",
        QuestEncounterType::NemesisWithArtifact => "quest-nemesis-artifact",
        QuestEncounterType::NemesisDead => "quest-nemesis-dead",
    }
}

/// Resolve a quest encounter: emit appropriate events and update state.
///
/// Returns events to display dialogue and potentially change quest state.
pub fn resolve_encounter(
    quest_state: &mut QuestState,
    role: Role,
    encounter: QuestEncounterType,
    level: u8,
    alignment_record: i32,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if matches!(
        encounter,
        QuestEncounterType::LeaderFirst
            | QuestEncounterType::LeaderNext
            | QuestEncounterType::LeaderAssigned
            | QuestEncounterType::LeaderNemesisDead
    ) && quest_state.leader_angry
    {
        events.push(EngineEvent::msg_with(
            "quest-leader-reject",
            vec![
                ("leader", quest_leader_for_role(role).to_string()),
                ("reason", "angry with you".to_string()),
            ],
        ));
        return events;
    }

    let key = quest_message_key(encounter);

    match encounter {
        QuestEncounterType::LeaderFirst => {
            quest_state.meet_leader();

            // Check eligibility.
            if check_quest_eligibility(role, level, alignment_record) {
                quest_state.assign();
                events.push(EngineEvent::msg_with(
                    key,
                    vec![
                        ("leader", quest_leader_for_role(role).to_string()),
                        ("nemesis", quest_nemesis_for_role(role).to_string()),
                        ("artifact", quest_artifact_for_role(role).to_string()),
                    ],
                ));
                events.push(EngineEvent::msg("quest-assigned"));
            } else {
                quest_state.reject();
                events.push(EngineEvent::msg_with(
                    "quest-leader-reject",
                    vec![
                        ("leader", quest_leader_for_role(role).to_string()),
                        (
                            "reason",
                            if level < 14 {
                                "too inexperienced".to_string()
                            } else {
                                "not worthy".to_string()
                            },
                        ),
                    ],
                ));
            }
        }
        QuestEncounterType::LeaderNext => {
            if check_quest_eligibility(role, level, alignment_record) {
                quest_state.assign();
                events.push(EngineEvent::msg_with(
                    key,
                    vec![("leader", quest_leader_for_role(role).to_string())],
                ));
                events.push(EngineEvent::msg("quest-assigned"));
            } else {
                quest_state.reject();
                events.push(EngineEvent::msg_with(
                    "quest-leader-reject",
                    vec![
                        ("leader", quest_leader_for_role(role).to_string()),
                        (
                            "reason",
                            if level < 14 {
                                "too inexperienced".to_string()
                            } else {
                                "not worthy".to_string()
                            },
                        ),
                    ],
                ));
            }
        }
        QuestEncounterType::LeaderAssigned => {
            events.push(EngineEvent::msg_with(
                key,
                vec![
                    ("leader", quest_leader_for_role(role).to_string()),
                    ("nemesis", quest_nemesis_for_role(role).to_string()),
                ],
            ));
        }
        QuestEncounterType::LeaderNemesisDead => {
            if quest_state.status != QuestStatus::Completed && quest_state.ready_for_completion() {
                quest_state.complete();
                events.push(EngineEvent::msg("quest-completed"));
            }
            events.push(EngineEvent::msg_with(
                key,
                vec![
                    ("leader", quest_leader_for_role(role).to_string()),
                    ("artifact", quest_artifact_for_role(role).to_string()),
                ],
            ));
        }
        QuestEncounterType::Guardian => {
            events.push(EngineEvent::msg_with(
                key,
                vec![("guardian", quest_guardian_for_role(role).to_string())],
            ));
        }
        QuestEncounterType::NemesisFirst
        | QuestEncounterType::NemesisNext
        | QuestEncounterType::NemesisWithArtifact => {
            events.push(EngineEvent::msg_with(
                key,
                vec![("nemesis", quest_nemesis_for_role(role).to_string())],
            ));
        }
        QuestEncounterType::NemesisDead => {
            quest_state.nemesis_stinky = true;
            events.push(EngineEvent::msg_with(
                key,
                vec![("nemesis", quest_nemesis_for_role(role).to_string())],
            ));
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Quest level checks
// ---------------------------------------------------------------------------

/// Minimum player level required to be assigned the quest.
pub const MIN_QUEST_LEVEL: u8 = 14;

/// Check if the player should be expelled from the quest dungeon.
///
/// Expulsion occurs if quest is not assigned and the player tries to descend.
pub fn should_expel_from_quest(quest_state: &QuestState) -> bool {
    quest_state.status == QuestStatus::NotStarted || quest_state.leader_angry
}

/// Check if the player's alignment is sufficient for the quest.
///
/// Returns `true` if alignment record is positive.
pub fn alignment_sufficient(alignment_record: i32) -> bool {
    alignment_record > 0
}

/// Return the alignment that the given role's quest is associated with.
pub fn quest_alignment_for_role(role: Role) -> Alignment {
    match role {
        Role::Archeologist => Alignment::Lawful,
        Role::Barbarian => Alignment::Neutral,
        Role::Caveperson => Alignment::Neutral,
        Role::Healer => Alignment::Neutral,
        Role::Knight => Alignment::Lawful,
        Role::Monk => Alignment::Neutral,
        Role::Priest => Alignment::Neutral,
        Role::Ranger => Alignment::Neutral,
        Role::Rogue => Alignment::Chaotic,
        Role::Samurai => Alignment::Lawful,
        Role::Tourist => Alignment::Neutral,
        Role::Valkyrie => Alignment::Neutral,
        Role::Wizard => Alignment::Neutral,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Eligibility tests ─────────────────────────────────────────

    #[test]
    fn test_quest_eligibility_level14() {
        // Level 14 with positive alignment → eligible.
        assert!(check_quest_eligibility(Role::Valkyrie, 14, 10));
    }

    #[test]
    fn test_quest_eligibility_high_level() {
        // Level 30 with positive alignment → eligible.
        assert!(check_quest_eligibility(Role::Wizard, 30, 5));
    }

    #[test]
    fn test_quest_eligibility_low_level() {
        // Level 10 → ineligible (need 14+).
        assert!(!check_quest_eligibility(Role::Valkyrie, 10, 10));
    }

    #[test]
    fn test_quest_eligibility_level13() {
        // Level 13 → ineligible.
        assert!(!check_quest_eligibility(Role::Knight, 13, 50));
    }

    #[test]
    fn test_quest_eligibility_zero_alignment() {
        // Level 14 but alignment = 0 → ineligible.
        assert!(!check_quest_eligibility(Role::Monk, 14, 0));
    }

    #[test]
    fn test_quest_eligibility_negative_alignment() {
        // Level 20 but negative alignment → ineligible.
        assert!(!check_quest_eligibility(Role::Rogue, 20, -5));
    }

    // ── Artifact mapping tests ────────────────────────────────────

    #[test]
    fn test_quest_artifact_per_role() {
        // All 13 roles must have unique, non-empty artifact names.
        let mut names = std::collections::HashSet::new();
        for &role in &Role::ALL {
            let name = quest_artifact_for_role(role);
            assert!(!name.is_empty(), "{:?} should have an artifact", role);
            assert!(names.insert(name), "duplicate artifact name: {name}");
        }
        assert_eq!(names.len(), 13, "should have 13 unique artifacts");
    }

    #[test]
    fn test_quest_artifact_valkyrie() {
        assert_eq!(quest_artifact_for_role(Role::Valkyrie), "The Orb of Fate");
    }

    #[test]
    fn test_quest_artifact_wizard() {
        assert_eq!(
            quest_artifact_for_role(Role::Wizard),
            "The Eye of the Aethiopica"
        );
    }

    // ── Nemesis mapping tests ─────────────────────────────────────

    #[test]
    fn test_quest_nemesis_per_role() {
        // All 13 roles must have unique, non-empty nemesis names.
        let mut names = std::collections::HashSet::new();
        for &role in &Role::ALL {
            let name = quest_nemesis_for_role(role);
            assert!(!name.is_empty(), "{:?} should have a nemesis", role);
            assert!(names.insert(name), "duplicate nemesis name: {name}");
        }
        assert_eq!(names.len(), 13, "should have 13 unique nemeses");
    }

    #[test]
    fn test_quest_nemesis_valkyrie() {
        assert_eq!(quest_nemesis_for_role(Role::Valkyrie), "Lord Surtur");
    }

    #[test]
    fn test_quest_nemesis_wizard() {
        assert_eq!(quest_nemesis_for_role(Role::Wizard), "Dark One");
    }

    // ── Leader mapping tests ──────────────────────────────────────

    #[test]
    fn test_quest_leader_per_role() {
        // All 13 roles must have non-empty leader names.
        for &role in &Role::ALL {
            let name = quest_leader_for_role(role);
            assert!(!name.is_empty(), "{:?} should have a leader", role);
        }
    }

    #[test]
    fn test_quest_leader_valkyrie() {
        assert_eq!(quest_leader_for_role(Role::Valkyrie), "Norn");
    }

    #[test]
    fn test_quest_leader_wizard() {
        assert_eq!(quest_leader_for_role(Role::Wizard), "Neferet the Green");
    }

    // ── Quest state transitions ───────────────────────────────────

    #[test]
    fn test_quest_state_default() {
        let qs = QuestState::new();
        assert_eq!(qs.status, QuestStatus::NotStarted);
        assert!(!qs.leader_met);
        assert!(!qs.nemesis_defeated);
        assert!(!qs.artifact_obtained);
        assert!(!qs.quest_dungeon_entered);
    }

    #[test]
    fn test_quest_state_full_progression() {
        let mut qs = QuestState::new();

        qs.meet_leader();
        assert!(qs.leader_met);
        assert_eq!(qs.status, QuestStatus::NotStarted);

        qs.assign();
        assert_eq!(qs.status, QuestStatus::Assigned);

        qs.enter_quest_dungeon();
        assert!(qs.quest_dungeon_entered);
        assert_eq!(qs.status, QuestStatus::InProgress);

        qs.defeat_nemesis();
        assert!(qs.nemesis_defeated);
        // Not completed yet — still need the artifact.
        assert_eq!(qs.status, QuestStatus::InProgress);

        qs.obtain_artifact();
        assert!(qs.artifact_obtained);
        assert!(qs.ready_for_completion());
        assert_eq!(qs.status, QuestStatus::InProgress);

        qs.complete();
        assert_eq!(qs.status, QuestStatus::Completed);
    }

    #[test]
    fn test_quest_state_artifact_before_nemesis() {
        // If the player gets the artifact before killing the nemesis,
        // quest is not complete until nemesis is defeated.
        let mut qs = QuestState::new();
        qs.assign();
        qs.enter_quest_dungeon();

        qs.obtain_artifact();
        assert_eq!(qs.status, QuestStatus::InProgress);

        qs.defeat_nemesis();
        assert!(qs.ready_for_completion());
        assert_eq!(qs.status, QuestStatus::InProgress);

        qs.complete();
        assert_eq!(qs.status, QuestStatus::Completed);
    }

    // ── Rejection and expulsion tracking ─────────────────────────

    #[test]
    fn test_quest_rejection_tracking() {
        let mut qs = QuestState::new();
        assert_eq!(qs.times_rejected, 0);
        qs.reject();
        assert_eq!(qs.times_rejected, 1);
        qs.reject();
        assert_eq!(qs.times_rejected, 2);
    }

    #[test]
    fn test_quest_expulsion_tracking() {
        let mut qs = QuestState::new();
        assert_eq!(qs.times_expelled, 0);
        qs.expel();
        assert_eq!(qs.times_expelled, 1);
    }

    #[test]
    fn test_quest_leader_angry() {
        let mut qs = QuestState::new();
        assert!(!qs.leader_angry);
        qs.anger_leader();
        assert!(qs.leader_angry);
    }

    // ── Guardian mapping tests ────────────────────────────────────

    #[test]
    fn test_quest_guardian_per_role() {
        for &role in &Role::ALL {
            let name = quest_guardian_for_role(role);
            assert!(!name.is_empty(), "{:?} should have a guardian", role);
        }
    }

    #[test]
    fn test_quest_guardian_knight() {
        assert_eq!(quest_guardian_for_role(Role::Knight), "page");
    }

    // ── Homebase mapping tests ────────────────────────────────────

    #[test]
    fn test_quest_homebase_per_role() {
        for &role in &Role::ALL {
            let name = quest_homebase_for_role(role);
            assert!(!name.is_empty(), "{:?} should have a homebase", role);
        }
    }

    #[test]
    fn test_quest_homebase_samurai() {
        assert_eq!(
            quest_homebase_for_role(Role::Samurai),
            "the Castle of the Taro Clan"
        );
    }

    // ── Intermediate target tests ─────────────────────────────────

    #[test]
    fn test_quest_intermed_per_role() {
        for &role in &Role::ALL {
            let name = quest_intermed_for_role(role);
            assert!(!name.is_empty(), "{:?} should have intermediate", role);
        }
    }

    // ── Filecode tests ────────────────────────────────────────────

    #[test]
    fn test_quest_filecode_per_role() {
        for &role in &Role::ALL {
            let code = quest_filecode_for_role(role);
            assert_eq!(code.len(), 3, "{:?} filecode should be 3 chars", role);
        }
    }

    #[test]
    fn test_quest_filecode_valkyrie() {
        assert_eq!(quest_filecode_for_role(Role::Valkyrie), "Val");
    }

    // ── Enemy type tests ──────────────────────────────────────────

    #[test]
    fn test_quest_enemies_per_role() {
        for &role in &Role::ALL {
            let enemies = quest_enemies_for_role(role);
            assert!(!enemies.enemy1.is_empty(), "{:?} needs enemy1", role);
            assert!(!enemies.enemy2.is_empty(), "{:?} needs enemy2", role);
        }
    }

    #[test]
    fn test_quest_monster_selection() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        let mut primary = 0;
        let mut secondary = 0;
        let enemies = quest_enemies_for_role(Role::Valkyrie);
        for _ in 0..1000 {
            let monster = select_quest_monster(Role::Valkyrie, &mut rng);
            if monster == enemies.enemy1 {
                primary += 1;
            } else {
                secondary += 1;
            }
        }
        // ~80% primary, ~20% secondary.
        assert!(primary > 700, "primary count {} too low", primary);
        assert!(secondary > 100, "secondary count {} too low", secondary);
    }

    // ── Quest text expansion tests ────────────────────────────────

    #[test]
    fn test_expand_player_name() {
        let ctx = QuestTextContext {
            player_name: "Aragorn".to_string(),
            role: Role::Knight,
            role_name: "Knight".to_string(),
            rank: "Gallant".to_string(),
            is_female: false,
            alignment: Alignment::Lawful,
            deity_name: "Lugh".to_string(),
        };
        assert_eq!(expand_quest_placeholder('p', &ctx), "Aragorn");
    }

    #[test]
    fn test_expand_role_name() {
        let ctx = QuestTextContext {
            player_name: "test".to_string(),
            role: Role::Wizard,
            role_name: "Wizard".to_string(),
            rank: "Thaumaturge".to_string(),
            is_female: false,
            alignment: Alignment::Neutral,
            deity_name: "Thoth".to_string(),
        };
        assert_eq!(expand_quest_placeholder('c', &ctx), "Wizard");
    }

    #[test]
    fn test_expand_sibling_gendered() {
        let mut ctx = QuestTextContext {
            player_name: "test".to_string(),
            role: Role::Valkyrie,
            role_name: "Valkyrie".to_string(),
            rank: "Warrior".to_string(),
            is_female: true,
            alignment: Alignment::Neutral,
            deity_name: "Odin".to_string(),
        };
        assert_eq!(expand_quest_placeholder('s', &ctx), "sister");
        assert_eq!(expand_quest_placeholder('S', &ctx), "daughter");
        ctx.is_female = false;
        assert_eq!(expand_quest_placeholder('s', &ctx), "brother");
        assert_eq!(expand_quest_placeholder('S', &ctx), "son");
    }

    #[test]
    fn test_expand_leader_nemesis_artifact() {
        let ctx = QuestTextContext {
            player_name: "test".to_string(),
            role: Role::Knight,
            role_name: "Knight".to_string(),
            rank: "Gallant".to_string(),
            is_female: false,
            alignment: Alignment::Lawful,
            deity_name: "Lugh".to_string(),
        };
        assert_eq!(expand_quest_placeholder('l', &ctx), "King Arthur");
        assert_eq!(expand_quest_placeholder('n', &ctx), "Ixoth");
        assert_eq!(
            expand_quest_placeholder('o', &ctx),
            "The Magic Mirror of Merlin"
        );
    }

    #[test]
    fn test_expand_full_text() {
        let ctx = QuestTextContext {
            player_name: "Gandalf".to_string(),
            role: Role::Wizard,
            role_name: "Wizard".to_string(),
            rank: "Thaumaturge".to_string(),
            is_female: false,
            alignment: Alignment::Neutral,
            deity_name: "Thoth".to_string(),
        };
        let text = "Greetings, %p the %c! Your leader %l awaits.";
        let expanded = expand_quest_text(text, &ctx);
        assert_eq!(
            expanded,
            "Greetings, Gandalf the Wizard! Your leader Neferet the Green awaits."
        );
    }

    #[test]
    fn test_expand_percent_escape() {
        let ctx = QuestTextContext {
            player_name: "test".to_string(),
            role: Role::Rogue,
            role_name: "Rogue".to_string(),
            rank: "Footpad".to_string(),
            is_female: false,
            alignment: Alignment::Chaotic,
            deity_name: "Issek".to_string(),
        };
        assert_eq!(expand_quest_text("100%%", &ctx), "100%");
    }

    #[test]
    fn test_expand_alignment() {
        let ctx = QuestTextContext {
            player_name: "test".to_string(),
            role: Role::Knight,
            role_name: "Knight".to_string(),
            rank: "Gallant".to_string(),
            is_female: false,
            alignment: Alignment::Lawful,
            deity_name: "Lugh".to_string(),
        };
        assert_eq!(expand_quest_placeholder('a', &ctx), "lawful");
    }

    // ── Encounter determination tests ─────────────────────────────

    #[test]
    fn test_encounter_leader_first() {
        let qs = QuestState::new();
        let enc = determine_encounter(&qs, true, false);
        assert_eq!(enc, QuestEncounterType::LeaderFirst);
    }

    #[test]
    fn test_encounter_leader_after_rejection() {
        let mut qs = QuestState::new();
        qs.meet_leader();
        qs.reject();
        let enc = determine_encounter(&qs, true, false);
        assert_eq!(enc, QuestEncounterType::LeaderNext);
    }

    #[test]
    fn test_encounter_leader_after_assignment() {
        let mut qs = QuestState::new();
        qs.meet_leader();
        qs.assign();
        let enc = determine_encounter(&qs, true, false);
        assert_eq!(enc, QuestEncounterType::LeaderAssigned);
    }

    #[test]
    fn test_encounter_leader_nemesis_dead() {
        let mut qs = QuestState::new();
        qs.meet_leader();
        qs.assign();
        qs.enter_quest_dungeon();
        qs.defeat_nemesis();
        let enc = determine_encounter(&qs, true, false);
        assert_eq!(enc, QuestEncounterType::LeaderNemesisDead);
    }

    #[test]
    fn test_encounter_guardian() {
        let qs = QuestState::new();
        let enc = determine_encounter(&qs, false, false);
        assert_eq!(enc, QuestEncounterType::Guardian);
    }

    #[test]
    fn test_encounter_nemesis_first() {
        let qs = QuestState::new();
        let enc = determine_encounter(&qs, false, true);
        assert_eq!(enc, QuestEncounterType::NemesisFirst);
    }

    #[test]
    fn test_encounter_nemesis_with_artifact() {
        let mut qs = QuestState::new();
        qs.assign();
        qs.enter_quest_dungeon();
        qs.obtain_artifact();
        let enc = determine_encounter(&qs, false, true);
        assert_eq!(enc, QuestEncounterType::NemesisWithArtifact);
    }

    #[test]
    fn test_encounter_nemesis_dead() {
        let mut qs = QuestState::new();
        qs.assign();
        qs.enter_quest_dungeon();
        qs.defeat_nemesis();
        qs.obtain_artifact();
        let enc = determine_encounter(&qs, false, true);
        assert_eq!(enc, QuestEncounterType::NemesisDead);
    }

    // ── Resolve encounter tests ───────────────────────────────────

    #[test]
    fn test_resolve_leader_first_eligible() {
        let mut qs = QuestState::new();
        let events = resolve_encounter(
            &mut qs,
            Role::Valkyrie,
            QuestEncounterType::LeaderFirst,
            14,
            10,
        );
        assert!(qs.leader_met);
        assert_eq!(qs.status, QuestStatus::Assigned);
        assert!(events.len() >= 2); // leader greeting + assignment
    }

    #[test]
    fn test_resolve_leader_first_ineligible() {
        let mut qs = QuestState::new();
        let events = resolve_encounter(
            &mut qs,
            Role::Valkyrie,
            QuestEncounterType::LeaderFirst,
            10, // too low
            10,
        );
        assert!(qs.leader_met);
        assert_eq!(qs.status, QuestStatus::NotStarted);
        assert_eq!(qs.times_rejected, 1);
        let has_reject = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "quest-leader-reject"));
        assert!(has_reject);
    }

    #[test]
    fn test_resolve_nemesis_dead() {
        let mut qs = QuestState::new();
        qs.assign();
        qs.enter_quest_dungeon();
        qs.defeat_nemesis();
        qs.obtain_artifact();
        let events = resolve_encounter(
            &mut qs,
            Role::Knight,
            QuestEncounterType::NemesisDead,
            20,
            15,
        );
        assert!(qs.nemesis_stinky);
        assert!(!events.is_empty());
    }

    #[test]
    fn test_resolve_leader_completion_marks_completed() {
        let mut qs = QuestState::new();
        qs.assign();
        qs.enter_quest_dungeon();
        qs.defeat_nemesis();
        qs.obtain_artifact();

        let events = resolve_encounter(
            &mut qs,
            Role::Wizard,
            QuestEncounterType::LeaderNemesisDead,
            20,
            15,
        );

        assert_eq!(qs.status, QuestStatus::Completed);
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-completed"
        )));
    }

    #[test]
    fn test_angry_leader_rejects_even_eligible_player() {
        let mut qs = QuestState::new();
        qs.anger_leader();

        let events = resolve_encounter(
            &mut qs,
            Role::Wizard,
            QuestEncounterType::LeaderFirst,
            20,
            15,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-leader-reject"
        )));
        assert_eq!(qs.status, QuestStatus::NotStarted);
    }

    // ── Quest message key tests ───────────────────────────────────

    #[test]
    fn test_quest_message_keys_unique() {
        let encounters = [
            QuestEncounterType::LeaderFirst,
            QuestEncounterType::LeaderNext,
            QuestEncounterType::LeaderAssigned,
            QuestEncounterType::LeaderNemesisDead,
            QuestEncounterType::Guardian,
            QuestEncounterType::NemesisFirst,
            QuestEncounterType::NemesisNext,
            QuestEncounterType::NemesisWithArtifact,
            QuestEncounterType::NemesisDead,
        ];
        let mut keys = std::collections::HashSet::new();
        for enc in &encounters {
            let key = quest_message_key(*enc);
            assert!(keys.insert(key), "duplicate key: {key}");
        }
    }

    // ── Quest level check tests ───────────────────────────────────

    #[test]
    fn test_should_expel_not_started() {
        let qs = QuestState::new();
        assert!(should_expel_from_quest(&qs));
    }

    #[test]
    fn test_should_still_expel_after_meeting_leader_without_assignment() {
        let mut qs = QuestState::new();
        qs.meet_leader();
        assert!(should_expel_from_quest(&qs));
    }

    #[test]
    fn test_should_not_expel_after_assignment() {
        let mut qs = QuestState::new();
        qs.assign();
        assert!(!should_expel_from_quest(&qs));
    }

    #[test]
    fn test_should_expel_after_angering_leader() {
        let mut qs = QuestState::new();
        qs.assign();
        qs.anger_leader();
        assert!(should_expel_from_quest(&qs));
    }

    #[test]
    fn test_alignment_sufficient() {
        assert!(alignment_sufficient(1));
        assert!(alignment_sufficient(100));
        assert!(!alignment_sufficient(0));
        assert!(!alignment_sufficient(-5));
    }

    // ── Quest alignment for role tests ────────────────────────────

    #[test]
    fn test_quest_alignment_per_role() {
        assert_eq!(quest_alignment_for_role(Role::Knight), Alignment::Lawful,);
        assert_eq!(quest_alignment_for_role(Role::Rogue), Alignment::Chaotic,);
        assert_eq!(quest_alignment_for_role(Role::Healer), Alignment::Neutral,);
    }
}
