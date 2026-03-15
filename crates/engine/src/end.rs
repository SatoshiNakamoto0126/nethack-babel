//! Game-over processing and end-of-game sequences.
//!
//! Implements the core logic from NetHack's `end.c`:
//! - Death processing (`done`)
//! - Score finalization
//! - Vanquished monster list
//! - "Do you want your possessions identified?" (DYWYPI)
//! - Tombstone data
//! - Ascension handling
//!
//! The engine module produces data and events; actual UI rendering of
//! the tombstone, disclosure menus, etc. is handled by the TUI crate.

use hecs::Entity;
use serde::{Deserialize, Serialize};

use crate::conduct::ConductState;
use crate::event::{DeathCause, EngineEvent};
use crate::exper::Experience;
use crate::role::Role;
use crate::world::{ExperienceLevel, GameWorld, HitPoints};

// ---------------------------------------------------------------------------
// How the game ended
// ---------------------------------------------------------------------------

/// How the game ended.  Matches the `deaths[]` array in `end.c`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EndHow {
    Died,
    Choked,
    Poisoned,
    Starvation,
    Drowning,
    Burning,
    Dissolved,
    Crushed,
    Stoning,
    Slimed,
    Genocided,
    Panicked,
    Trickery,
    Quit,
    Escaped,
    Ascended,
}

impl EndHow {
    /// Past-tense description matching the C `deaths[]` array.
    pub fn death_description(&self) -> &'static str {
        match self {
            EndHow::Died => "died",
            EndHow::Choked => "choked",
            EndHow::Poisoned => "poisoned",
            EndHow::Starvation => "starvation",
            EndHow::Drowning => "drowning",
            EndHow::Burning => "burning",
            EndHow::Dissolved => "dissolving under the heat and pressure",
            EndHow::Crushed => "crushed",
            EndHow::Stoning => "turned to stone",
            EndHow::Slimed => "turned into slime",
            EndHow::Genocided => "genocided",
            EndHow::Panicked => "panic",
            EndHow::Trickery => "trickery",
            EndHow::Quit => "quit",
            EndHow::Escaped => "escaped",
            EndHow::Ascended => "ascended",
        }
    }

    /// "when you %s" phrasing from the C `ends[]` array.
    pub fn ending_phrase(&self) -> &'static str {
        match self {
            EndHow::Died => "died",
            EndHow::Choked => "choked",
            EndHow::Poisoned => "were poisoned",
            EndHow::Starvation => "starved",
            EndHow::Drowning => "drowned",
            EndHow::Burning => "burned",
            EndHow::Dissolved => "dissolved in the lava",
            EndHow::Crushed => "were crushed",
            EndHow::Stoning => "turned to stone",
            EndHow::Slimed => "turned into slime",
            EndHow::Genocided => "were genocided",
            EndHow::Panicked => "panicked",
            EndHow::Trickery => "were tricked",
            EndHow::Quit => "quit",
            EndHow::Escaped => "escaped",
            EndHow::Ascended => "ascended",
        }
    }

    /// Whether the player is considered dead (for bones eligibility etc.).
    pub fn is_dead(&self) -> bool {
        !matches!(self, EndHow::Quit | EndHow::Escaped | EndHow::Ascended
                  | EndHow::Panicked | EndHow::Trickery)
    }

    /// Whether this ending allows bones file creation.
    pub fn allows_bones(&self) -> bool {
        matches!(
            self,
            EndHow::Died | EndHow::Choked | EndHow::Poisoned
            | EndHow::Starvation | EndHow::Drowning | EndHow::Burning
            | EndHow::Dissolved | EndHow::Crushed | EndHow::Stoning
            | EndHow::Slimed
        )
    }
}

// ---------------------------------------------------------------------------
// Vanquished monster entry
// ---------------------------------------------------------------------------

/// A single entry in the vanquished monster list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VanquishedEntry {
    /// Monster display name.
    pub name: String,
    /// Number killed.
    pub count: u32,
}

/// Sort vanquished list by count descending, then name ascending.
pub fn sort_vanquished(list: &mut [VanquishedEntry]) {
    list.sort_by(|a, b| {
        b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name))
    });
}

// ---------------------------------------------------------------------------
// Tombstone data
// ---------------------------------------------------------------------------

/// Data for rendering a tombstone (RIP screen).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tombstone {
    /// Player name.
    pub name: String,
    /// How they died / ended.
    pub how: EndHow,
    /// The killer's name or cause string.
    pub killer: String,
    /// Final score.
    pub score: u64,
    /// Player's role name.
    pub role: String,
    /// Experience level at death.
    pub level: u8,
    /// Dungeon depth at death (display string, e.g. "Dlvl:5").
    pub depth: String,
    /// Total game turns.
    pub turns: u32,
    /// Gold carried at death.
    pub gold: i64,
    /// HP at death.
    pub hp: i32,
    /// Max HP at death.
    pub maxhp: i32,
}

// ---------------------------------------------------------------------------
// Game over result
// ---------------------------------------------------------------------------

/// Complete result of a game ending, produced by `done()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameOverResult {
    /// How the game ended.
    pub how: EndHow,
    /// Final score.
    pub score: u64,
    /// Tombstone data for the RIP screen.
    pub tombstone: Tombstone,
    /// Vanquished monster list.
    pub vanquished: Vec<VanquishedEntry>,
    /// Conducts maintained.
    pub conducts_maintained: u32,
    /// All engine events from the death sequence.
    pub events: Vec<EngineEvent>,
}

// ---------------------------------------------------------------------------
// Game over processing
// ---------------------------------------------------------------------------

/// Parameters for `done()`.
pub struct DoneParams {
    /// How the game ended.
    pub how: EndHow,
    /// Killer name / cause of death.
    pub killer: String,
    /// Deepest dungeon level reached.
    pub deepest_level: i32,
    /// Gold carried.
    pub gold: i64,
    /// Starting gold (for net gain calculation).
    pub starting_gold: i64,
    /// Vanquished monster list.
    pub vanquished: Vec<VanquishedEntry>,
    /// Current conduct state.
    pub conducts: ConductState,
    /// Current dungeon depth display string.
    pub depth_string: String,
    /// Player's role.
    pub role: Role,
    /// Whether the player retained their original alignment (for ascension bonus).
    pub original_alignment: bool,
}

/// Process end of game.
///
/// Calculates final score, builds tombstone, and emits GameOver event.
/// This is the engine-side equivalent of `really_done()` from `end.c`.
///
/// The actual UI interactions (DYWYPI, disclosure menus) are driven by the
/// TUI crate using the returned `GameOverResult`.
pub fn done(
    world: &GameWorld,
    player: Entity,
    params: DoneParams,
) -> GameOverResult {
    let mut events = Vec::new();

    // Gather player stats.
    let level = world
        .get_component::<ExperienceLevel>(player)
        .map(|l| l.0)
        .unwrap_or(1);

    let (hp, maxhp) = world
        .get_component::<HitPoints>(player)
        .map(|h| (h.current, h.max))
        .unwrap_or((0, 0));

    let player_name = world.entity_name(player);

    let turns = world.turn();

    let score_xp = world
        .get_component::<Experience>(player)
        .map(|e| e.score_xp)
        .unwrap_or(0);

    // Calculate score.
    let score = calculate_final_score(
        &params,
        score_xp,
    );

    // Build tombstone.
    let tombstone = Tombstone {
        name: player_name.clone(),
        how: params.how.clone(),
        killer: params.killer.clone(),
        score,
        role: params.role.name().to_string(),
        level,
        depth: params.depth_string.clone(),
        turns,
        gold: params.gold,
        hp,
        maxhp,
    };

    // Sort vanquished list.
    let mut vanquished = params.vanquished;
    sort_vanquished(&mut vanquished);

    let conducts_maintained = params.conducts.maintained_count();

    // Emit GameOver event.
    let death_cause = match &params.how {
        EndHow::Died => DeathCause::KilledBy {
            killer_name: params.killer.clone(),
        },
        EndHow::Choked | EndHow::Starvation => DeathCause::Starvation,
        EndHow::Poisoned => DeathCause::Poisoning,
        EndHow::Drowning => DeathCause::Drowning,
        EndHow::Burning => DeathCause::Burning,
        EndHow::Stoning => DeathCause::Petrification,
        EndHow::Slimed => DeathCause::Sickness,
        EndHow::Quit => DeathCause::Quit,
        EndHow::Escaped => DeathCause::Escaped,
        EndHow::Ascended => DeathCause::Ascended,
        _ => DeathCause::KilledBy {
            killer_name: params.killer,
        },
    };

    events.push(EngineEvent::GameOver {
        cause: death_cause,
        score,
    });

    // Early death message.
    if turns <= 1 && params.how.is_dead() {
        events.push(EngineEvent::msg("end-do-not-pass-go"));
    }

    // Goodbye message.
    events.push(EngineEvent::msg_with(
        "end-goodbye",
        vec![
            ("name", player_name),
            ("role", params.role.name().to_string()),
            ("how", params.how.death_description().to_string()),
        ],
    ));

    GameOverResult {
        how: params.how,
        score,
        tombstone,
        vanquished,
        conducts_maintained,
        events,
    }
}

// ---------------------------------------------------------------------------
// Score calculation
// ---------------------------------------------------------------------------

/// Calculate the final score.
///
/// Matches the score logic in `really_done()` from `end.c`:
/// - Base: accumulated score XP
/// - Net gold gain (minus 10% death tax if dead)
/// - Depth bonus: 50 * (deepest - 1)
/// - Deep bonus: 1000 * (deepest - 20) for levels > 20
/// - Ascension doubles score if original alignment retained
fn calculate_final_score(
    params: &DoneParams,
    score_xp: i64,
) -> u64 {
    let mut score = score_xp.max(0);

    // Net gold gain.
    let net_gold = (params.gold - params.starting_gold).max(0);
    let gold_score = if params.how.is_dead() {
        net_gold - net_gold / 10 // 10% death tax
    } else {
        net_gold
    };
    score = score.saturating_add(gold_score);

    // Depth bonus.
    let deepest = params.deepest_level.max(1);
    score = score.saturating_add(50 * (deepest as i64 - 1));

    // Extra deep bonus.
    if deepest > 20 {
        let deep_bonus = if deepest > 30 { 10 } else { deepest - 20 };
        score = score.saturating_add(1000 * deep_bonus as i64);
    }

    // Ascension bonus.
    if params.how == EndHow::Ascended && params.original_alignment {
        // Double the score.
        score = score.saturating_mul(2);
    } else if params.how == EndHow::Ascended {
        // Converted alignment: 1.5x.
        score = score.saturating_add(score / 2);
    }

    score.max(0) as u64
}

// ---------------------------------------------------------------------------
// Disclosure helpers
// ---------------------------------------------------------------------------

/// What to disclose at end of game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DisclosureOption {
    Inventory,
    Attributes,
    Vanquished,
    Genocided,
    Conducts,
    Overview,
}

impl DisclosureOption {
    /// All disclosure options in standard order.
    pub const ALL: [DisclosureOption; 6] = [
        DisclosureOption::Inventory,
        DisclosureOption::Attributes,
        DisclosureOption::Vanquished,
        DisclosureOption::Genocided,
        DisclosureOption::Conducts,
        DisclosureOption::Overview,
    ];

    /// Display prompt for this option.
    pub fn prompt(&self) -> &'static str {
        match self {
            DisclosureOption::Inventory => "Do you want your possessions identified?",
            DisclosureOption::Attributes => "Do you want to see your attributes?",
            DisclosureOption::Vanquished => "Do you want to see the vanquished list?",
            DisclosureOption::Genocided => "Do you want to see the genocided list?",
            DisclosureOption::Conducts => "Do you want to see your conduct?",
            DisclosureOption::Overview => "Do you want to see the dungeon overview?",
        }
    }
}

// ---------------------------------------------------------------------------
// Tombstone ASCII art
// ---------------------------------------------------------------------------

/// The classic NetHack tombstone template.
///
/// Matches the ASCII art from `win/tty/topl.c`.  Placeholders are filled in
/// by `render_tombstone()`.
const TOMBSTONE_TEMPLATE: &[&str] = &[
    "                       ----------",
    "                      /          \\",
    "                     /    REST    \\",
    "                    /      IN      \\",
    "                   /     PEACE      \\",
    "                  /                  \\",
    "                  |  {name}  |",
    "                  |  {score}  |",
    "                  |  {killer}  |",
    "                  |  {depth}  |",
    "                  |  {date}  |",
    "                  |                  |",
    "                 *|     *  *  *      |*",
    "        _________)/\\/\\/\\/\\.  .\\/.\\./\\_(/________",
];

/// Render a tombstone from game-over data.
///
/// Returns a vector of strings, one per line, ready for display.
pub fn render_tombstone(tombstone: &Tombstone) -> Vec<String> {
    let center = |s: &str, width: usize| -> String {
        if s.len() >= width {
            s[..width].to_string()
        } else {
            let pad = (width - s.len()) / 2;
            let mut result = " ".repeat(pad);
            result.push_str(s);
            result
        }
    };

    let name_line = center(&tombstone.name, 18);
    let score_line = center(&format!("{} Au", tombstone.score), 18);
    let killer_line = center(&tombstone.killer, 18);
    let depth_line = center(&tombstone.depth, 18);
    let date_line = center("2026", 18); // placeholder date

    let mut lines = Vec::new();
    for template_line in TOMBSTONE_TEMPLATE {
        let line = template_line
            .replace("{name}", &name_line)
            .replace("{score}", &score_line)
            .replace("{killer}", &killer_line)
            .replace("{depth}", &depth_line)
            .replace("{date}", &date_line);
        lines.push(line);
    }
    lines
}

// ---------------------------------------------------------------------------
// Vanquished list formatting
// ---------------------------------------------------------------------------

/// Format the vanquished monster list for display.
///
/// Returns lines like:
/// - "10 orcs"
/// - "5 kobolds"
/// - "1 dragon"
///
/// Sorted by count descending (caller should sort first or use pre-sorted).
pub fn format_vanquished(vanquished: &[VanquishedEntry]) -> Vec<String> {
    if vanquished.is_empty() {
        return vec!["You vanquished no combatants.".to_string()];
    }

    let total: u32 = vanquished.iter().map(|v| v.count).sum();
    let mut lines = vec![format!(
        "Vanquished creatures ({} total):",
        total
    )];

    for entry in vanquished {
        if entry.count == 1 {
            lines.push(format!("  {}", entry.name));
        } else {
            lines.push(format!("  {} {}s", entry.count, entry.name));
        }
    }

    lines
}

/// Total number of monsters vanquished.
pub fn total_vanquished(vanquished: &[VanquishedEntry]) -> u32 {
    vanquished.iter().map(|v| v.count).sum()
}

// ---------------------------------------------------------------------------
// Conduct summary formatting
// ---------------------------------------------------------------------------

/// Format the conduct summary for end-of-game display.
///
/// Returns lines describing which conducts were maintained and which
/// were violated, matching C NetHack's conduct disclosure.
pub fn format_conduct_summary(conducts: &ConductState) -> Vec<String> {
    use crate::conduct::Conduct;

    let maintained = conducts.maintained_count();
    let mut lines = Vec::new();

    if maintained == 13 {
        lines.push("You maintained all 13 conducts.".to_string());
    } else if maintained == 0 {
        lines.push("You broke every single conduct.".to_string());
    } else {
        lines.push(format!(
            "You maintained {} conduct{}:",
            maintained,
            if maintained == 1 { "" } else { "s" }
        ));
    }

    let descriptions: &[(Conduct, &str, &str)] = &[
        (Conduct::Foodless, "ate no food", "ate food"),
        (Conduct::Vegan, "followed a strict vegan diet", "ate non-vegan food"),
        (Conduct::Vegetarian, "were a vegetarian", "ate meat"),
        (Conduct::Atheist, "were an atheist", "prayed or sacrificed"),
        (Conduct::Weaponless, "never hit with a wielded weapon", "hit with weapons"),
        (Conduct::Pacifist, "were a pacifist", "killed creatures"),
        (Conduct::Illiterate, "were illiterate", "read scrolls or spellbooks"),
        (Conduct::Polypileless, "never polymorphed an object", "polymorphed objects"),
        (Conduct::Polyselfless, "never polymorphed", "polymorphed self"),
        (Conduct::Wishless, "never made a wish", "made wishes"),
        (Conduct::ArtifactWishless, "never wished for an artifact", "wished for artifacts"),
        (Conduct::Genocideless, "never genocided anything", "committed genocide"),
        (Conduct::Petless, "never used a pet", "used pets"),
    ];

    for (conduct, maintained_desc, violated_desc) in descriptions {
        if conducts.is_maintained(*conduct) {
            lines.push(format!("  You {}.", maintained_desc));
        } else {
            let count = conducts.violation_count(*conduct);
            lines.push(format!("  You {} ({} time{}).", violated_desc, count,
                if count == 1 { "" } else { "s" }));
        }
    }

    // Elberethless bonus.
    if conducts.is_maintained(Conduct::Elberethless) {
        lines.push("  You never wrote Elbereth.".to_string());
    }

    lines
}

// ---------------------------------------------------------------------------
// Ascension handling
// ---------------------------------------------------------------------------

/// Check if the player qualifies for ascension.
///
/// Requirements (from C NetHack's `done()` ascension path):
/// - Player has the real Amulet of Yendor in inventory
/// - Player is on the Astral Plane
/// - Player offered the Amulet on an aligned altar
pub fn check_ascension_requirements(
    has_amulet: bool,
    on_astral_plane: bool,
    altar_aligned: bool,
) -> AscensionCheck {
    if !has_amulet {
        AscensionCheck::MissingAmulet
    } else if !on_astral_plane {
        AscensionCheck::NotOnAstralPlane
    } else if !altar_aligned {
        AscensionCheck::WrongAltar
    } else {
        AscensionCheck::Qualified
    }
}

/// Result of an ascension eligibility check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AscensionCheck {
    /// Player can ascend.
    Qualified,
    /// Player doesn't have the Amulet.
    MissingAmulet,
    /// Player isn't on the Astral Plane.
    NotOnAstralPlane,
    /// Player is at an altar of the wrong alignment.
    WrongAltar,
}

/// Generate the ascension messages and score modifiers.
///
/// When a player ascends, they receive special messages and a score bonus
/// depending on whether they maintained their original alignment.
pub fn ascension_sequence(
    player_name: &str,
    role: &Role,
    original_alignment: bool,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    events.push(EngineEvent::msg("end-ascension-offering"));

    if original_alignment {
        events.push(EngineEvent::msg_with(
            "end-ascension-demigod",
            vec![
                ("name", player_name.to_string()),
                ("role", role.name().to_string()),
            ],
        ));
    } else {
        events.push(EngineEvent::msg_with(
            "end-ascension-demigod-converted",
            vec![
                ("name", player_name.to_string()),
                ("role", role.name().to_string()),
            ],
        ));
    }

    events
}

// ---------------------------------------------------------------------------
// DYWYPI (Do You Want Your Possessions Identified?) sequence
// ---------------------------------------------------------------------------

/// Determine which disclosure options to show based on game state.
///
/// In C NetHack, the `#disclose` option controls which end-of-game
/// disclosures are shown.  This function provides the default set.
pub fn default_disclosure_options(_how: &EndHow) -> Vec<DisclosureOption> {
    let mut opts = vec![
        DisclosureOption::Inventory,
        DisclosureOption::Attributes,
        DisclosureOption::Vanquished,
    ];

    // Genocided list is only interesting if genocide occurred.
    opts.push(DisclosureOption::Genocided);

    // Conducts and overview are always available.
    opts.push(DisclosureOption::Conducts);
    opts.push(DisclosureOption::Overview);

    opts
}

// ---------------------------------------------------------------------------
// Disclosure data
// ---------------------------------------------------------------------------

/// Disclosure categories that can be shown after game ends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisclosureCategory {
    /// Full inventory with identification.
    Inventory { items: Vec<DisclosureItem> },
    /// Player attributes and intrinsics.
    Attributes {
        stats: Vec<(String, String)>,
        intrinsics: Vec<String>,
    },
    /// Monsters vanquished during the game.
    Vanquished { kills: Vec<(String, u32)> },
    /// Genocided species.
    Genocided { species: Vec<String> },
    /// Conducts maintained and broken.
    Conducts {
        maintained: Vec<String>,
        broken: Vec<String>,
    },
}

/// A single item in the inventory disclosure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisclosureItem {
    /// Inventory letter.
    pub letter: char,
    /// Full identified name.
    pub name: String,
    /// What the player saw (before identification).
    pub unidentified_name: String,
    /// BUC status (blessed/uncursed/cursed).
    pub buc: String,
}

/// Generate all disclosure data for end-of-game display.
pub fn generate_disclosure(
    inventory: &[(char, String, String, String)],
    stats: &[(String, String)],
    intrinsics: &[String],
    kill_counts: &[(String, u32)],
    genocided: &[String],
    conducts_maintained: &[String],
    conducts_broken: &[String],
) -> Vec<DisclosureCategory> {
    vec![
        DisclosureCategory::Inventory {
            items: inventory
                .iter()
                .map(|(l, n, u, b)| DisclosureItem {
                    letter: *l,
                    name: n.clone(),
                    unidentified_name: u.clone(),
                    buc: b.clone(),
                })
                .collect(),
        },
        DisclosureCategory::Attributes {
            stats: stats.to_vec(),
            intrinsics: intrinsics.to_vec(),
        },
        DisclosureCategory::Vanquished {
            kills: kill_counts.to_vec(),
        },
        DisclosureCategory::Genocided {
            species: genocided.to_vec(),
        },
        DisclosureCategory::Conducts {
            maintained: conducts_maintained.to_vec(),
            broken: conducts_broken.to_vec(),
        },
    ]
}

/// Format a disclosure category for text display.
pub fn format_disclosure(category: &DisclosureCategory) -> Vec<String> {
    match category {
        DisclosureCategory::Inventory { items } => {
            let mut lines = vec!["Your inventory:".to_string()];
            for item in items {
                lines.push(format!("  {} - {} ({})", item.letter, item.name, item.buc));
            }
            lines
        }
        DisclosureCategory::Attributes { stats, intrinsics } => {
            let mut lines = vec!["Final attributes:".to_string()];
            for (key, val) in stats {
                lines.push(format!("  {}: {}", key, val));
            }
            if !intrinsics.is_empty() {
                lines.push("  Intrinsics:".to_string());
                for intr in intrinsics {
                    lines.push(format!("    {}", intr));
                }
            }
            lines
        }
        DisclosureCategory::Vanquished { kills } => {
            let total: u32 = kills.iter().map(|(_, c)| c).sum();
            let mut lines = vec![format!("Vanquished creatures ({} total):", total)];
            let mut sorted = kills.clone();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            for (name, count) in &sorted {
                if *count == 1 {
                    lines.push(format!("  {}", name));
                } else {
                    lines.push(format!("  {} (x{})", name, count));
                }
            }
            lines
        }
        DisclosureCategory::Genocided { species } => {
            let mut lines = vec!["Genocided species:".to_string()];
            if species.is_empty() {
                lines.push("  None.".to_string());
            } else {
                for sp in species {
                    lines.push(format!("  {}", sp));
                }
            }
            lines
        }
        DisclosureCategory::Conducts {
            maintained,
            broken: _,
        } => {
            let mut lines = vec!["Voluntary challenges:".to_string()];
            if maintained.is_empty() {
                lines.push("  None maintained.".to_string());
            } else {
                for conduct in maintained {
                    lines.push(format!("  You maintained the {} conduct.", conduct));
                }
            }
            lines
        }
    }
}

/// Whether to automatically show a disclosure option (no prompt needed).
///
/// Some endings auto-show certain disclosures:
/// - Ascension always shows inventory identified.
/// - Death always prompts.
pub fn auto_disclose(how: &EndHow, option: DisclosureOption) -> bool {
    match how {
        EndHow::Ascended => {
            matches!(option, DisclosureOption::Inventory | DisclosureOption::Conducts)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Artifact score calculation
// ---------------------------------------------------------------------------

/// Calculate score bonus from artifacts.
///
/// From C's `end.c` artifact scoring:
/// - Amulet of Yendor carried at game end: +5000
/// - Bell of Opening, Book of the Dead, Candelabrum: +2500 each
/// - Other named artifacts ever possessed: +1000 each
pub fn calculate_artifact_score(
    artifacts_carried: &[String],
    artifacts_ever_had: &[String],
) -> i64 {
    let mut score: i64 = 0;

    // Amulet of Yendor (must be carried at game end).
    if artifacts_carried.iter().any(|a| a == "Amulet of Yendor") {
        score += 5000;
    }

    // Bell, Book, Candelabrum (must be carried at game end).
    const QUEST_ITEMS: [&str; 3] = [
        "Bell of Opening",
        "Book of the Dead",
        "Candelabrum of Invocation",
    ];
    for item in &QUEST_ITEMS {
        if artifacts_carried.iter().any(|a| a == item) {
            score += 2500;
        }
    }

    // Other named artifacts ever possessed (not counting the above).
    for art in artifacts_ever_had {
        if art == "Amulet of Yendor" {
            continue;
        }
        if QUEST_ITEMS.contains(&art.as_str()) {
            continue;
        }
        score += 1000;
    }

    score
}

// ---------------------------------------------------------------------------
// Life saving check
// ---------------------------------------------------------------------------

/// Check for amulet of life saving.
///
/// If the player has life saving, restore HP to half of max and return true.
/// Otherwise return false.
pub fn save_life_check(
    has_life_saving: bool,
    hp: &mut i32,
    max_hp: i32,
) -> bool {
    if has_life_saving {
        *hp = max_hp / 2;
        if *hp < 1 {
            *hp = 1;
        }
        true
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Death message formatting
// ---------------------------------------------------------------------------

/// Format a death message for display.
///
/// Matches C NetHack's `killer_format()` / `done()` death reason strings.
pub fn fixup_death_message(killer: &str, how: &EndHow) -> String {
    match how {
        EndHow::Died => format!("killed by {}", add_death_article(killer)),
        EndHow::Drowning => format!("drowned by {}", add_death_article(killer)),
        EndHow::Stoning => format!("turned to stone by {}", add_death_article(killer)),
        EndHow::Choked => format!("choked on {}", killer),
        EndHow::Poisoned => format!("poisoned by {}", add_death_article(killer)),
        EndHow::Burning => format!("burned by {}", add_death_article(killer)),
        EndHow::Crushed => format!("crushed to death by {}", add_death_article(killer)),
        EndHow::Starvation => "starved to death".to_string(),
        EndHow::Quit => "quit".to_string(),
        EndHow::Escaped => "escaped".to_string(),
        EndHow::Ascended => "ascended".to_string(),
        _ => format!("{} by {}", how.death_description(), killer),
    }
}

/// Add "a" or "an" article to a killer name if it doesn't already have one.
fn add_death_article(name: &str) -> String {
    // Don't add article if it already starts with an article or is a proper noun.
    if name.starts_with("a ") || name.starts_with("an ")
        || name.starts_with("the ")
        || name.starts_with("A ") || name.starts_with("An ")
        || name.starts_with("The ")
    {
        return name.to_string();
    }

    if name.starts_with(|c: char| "aeiouAEIOU".contains(c)) {
        format!("an {}", name)
    } else {
        format!("a {}", name)
    }
}

// ---------------------------------------------------------------------------
// EndHow numeric conversion
// ---------------------------------------------------------------------------

impl EndHow {
    /// Convert to a numeric code (for XLOGFILE).
    pub fn to_code(&self) -> i32 {
        match self {
            EndHow::Died => 0,
            EndHow::Choked => 1,
            EndHow::Poisoned => 2,
            EndHow::Starvation => 3,
            EndHow::Drowning => 4,
            EndHow::Burning => 5,
            EndHow::Dissolved => 6,
            EndHow::Crushed => 7,
            EndHow::Stoning => 8,
            EndHow::Slimed => 9,
            EndHow::Genocided => 10,
            EndHow::Panicked => 11,
            EndHow::Trickery => 12,
            EndHow::Quit => 13,
            EndHow::Escaped => 14,
            EndHow::Ascended => 15,
        }
    }

    /// Convert from a numeric code.
    pub fn from_code(code: i32) -> Option<EndHow> {
        match code {
            0 => Some(EndHow::Died),
            1 => Some(EndHow::Choked),
            2 => Some(EndHow::Poisoned),
            3 => Some(EndHow::Starvation),
            4 => Some(EndHow::Drowning),
            5 => Some(EndHow::Burning),
            6 => Some(EndHow::Dissolved),
            7 => Some(EndHow::Crushed),
            8 => Some(EndHow::Stoning),
            9 => Some(EndHow::Slimed),
            10 => Some(EndHow::Genocided),
            11 => Some(EndHow::Panicked),
            12 => Some(EndHow::Trickery),
            13 => Some(EndHow::Quit),
            14 => Some(EndHow::Escaped),
            15 => Some(EndHow::Ascended),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Game summary for external tools
// ---------------------------------------------------------------------------

/// A complete game summary suitable for logging/analytics.
///
/// This is built from `GameOverResult` and additional context, providing
/// everything needed for the XLOGFILE entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSummary {
    /// Player name.
    pub name: String,
    /// Role name.
    pub role: String,
    /// Race name.
    pub race: String,
    /// Gender ("male" or "female").
    pub gender: String,
    /// Alignment ("lawful", "neutral", "chaotic").
    pub alignment: String,
    /// How the game ended.
    pub how: EndHow,
    /// Final score.
    pub score: u64,
    /// Death reason string.
    pub death_reason: String,
    /// Dungeon level at death.
    pub death_level: i32,
    /// Maximum dungeon level reached.
    pub max_level: i32,
    /// HP at death.
    pub hp: i32,
    /// Max HP.
    pub max_hp: i32,
    /// Total game turns.
    pub turns: u32,
    /// Conducts maintained count.
    pub conducts_maintained: u32,
    /// Total monsters vanquished.
    pub monsters_vanquished: u32,
    /// Gold at end.
    pub gold: i64,
}

impl GameSummary {
    /// Build a summary from a GameOverResult and context.
    pub fn from_result(
        result: &GameOverResult,
        race: &str,
        gender: &str,
        alignment: &str,
        death_level: i32,
        max_level: i32,
    ) -> Self {
        Self {
            name: result.tombstone.name.clone(),
            role: result.tombstone.role.clone(),
            race: race.to_string(),
            gender: gender.to_string(),
            alignment: alignment.to_string(),
            how: result.how.clone(),
            score: result.score,
            death_reason: result.tombstone.killer.clone(),
            death_level,
            max_level,
            hp: result.tombstone.hp,
            max_hp: result.tombstone.maxhp,
            turns: result.tombstone.turns,
            conducts_maintained: result.conducts_maintained,
            monsters_vanquished: total_vanquished(&result.vanquished),
            gold: result.tombstone.gold,
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::conduct::ConductState;
    use crate::world::GameWorld;

    fn make_test_world() -> GameWorld {
        GameWorld::new(Position::new(5, 5))
    }

    // --- Test 1: EndHow descriptions ---

    #[test]
    fn end_how_descriptions() {
        assert_eq!(EndHow::Died.death_description(), "died");
        assert_eq!(EndHow::Ascended.death_description(), "ascended");
        assert_eq!(EndHow::Stoning.death_description(), "turned to stone");
        assert_eq!(EndHow::Quit.ending_phrase(), "quit");
        assert_eq!(EndHow::Drowning.ending_phrase(), "drowned");
    }

    // --- Test 2: is_dead classification ---

    #[test]
    fn end_how_is_dead() {
        assert!(EndHow::Died.is_dead());
        assert!(EndHow::Poisoned.is_dead());
        assert!(EndHow::Stoning.is_dead());
        assert!(!EndHow::Quit.is_dead());
        assert!(!EndHow::Escaped.is_dead());
        assert!(!EndHow::Ascended.is_dead());
    }

    // --- Test 3: allows_bones ---

    #[test]
    fn end_how_allows_bones() {
        assert!(EndHow::Died.allows_bones());
        assert!(EndHow::Stoning.allows_bones());
        assert!(!EndHow::Quit.allows_bones());
        assert!(!EndHow::Ascended.allows_bones());
        assert!(!EndHow::Genocided.allows_bones());
    }

    // --- Test 4: Vanquished sorting ---

    #[test]
    fn vanquished_sorting() {
        let mut list = vec![
            VanquishedEntry { name: "kobold".to_string(), count: 5 },
            VanquishedEntry { name: "orc".to_string(), count: 10 },
            VanquishedEntry { name: "ant".to_string(), count: 10 },
            VanquishedEntry { name: "dragon".to_string(), count: 1 },
        ];
        sort_vanquished(&mut list);
        assert_eq!(list[0].name, "ant");     // 10, alphabetically first
        assert_eq!(list[1].name, "orc");     // 10, alphabetically second
        assert_eq!(list[2].name, "kobold");  // 5
        assert_eq!(list[3].name, "dragon");  // 1
    }

    // --- Test 5: Score calculation basic ---

    #[test]
    fn score_basic() {
        let params = DoneParams {
            how: EndHow::Died,
            killer: "a jackal".to_string(),
            deepest_level: 5,
            gold: 100,
            starting_gold: 0,
            vanquished: vec![],
            conducts: ConductState::new(),
            depth_string: "Dlvl:5".to_string(),
            role: Role::Wizard,
            original_alignment: true,
        };
        let score = calculate_final_score(&params, 500);
        // 500 + (100 - 10) + 50*(5-1) = 500 + 90 + 200 = 790
        assert_eq!(score, 790);
    }

    // --- Test 6: Score with depth > 20 ---

    #[test]
    fn score_deep_dungeon() {
        let params = DoneParams {
            how: EndHow::Died,
            killer: "a dragon".to_string(),
            deepest_level: 25,
            gold: 0,
            starting_gold: 0,
            vanquished: vec![],
            conducts: ConductState::new(),
            depth_string: "Dlvl:25".to_string(),
            role: Role::Valkyrie,
            original_alignment: true,
        };
        let score = calculate_final_score(&params, 1000);
        // 1000 + 0 + 50*24 + 1000*5 = 1000 + 1200 + 5000 = 7200
        assert_eq!(score, 7200);
    }

    // --- Test 7: Ascension score bonus (original alignment) ---

    #[test]
    fn score_ascension_original() {
        let params = DoneParams {
            how: EndHow::Ascended,
            killer: String::new(),
            deepest_level: 30,
            gold: 50000,
            starting_gold: 0,
            vanquished: vec![],
            conducts: ConductState::new(),
            depth_string: "Astral".to_string(),
            role: Role::Valkyrie,
            original_alignment: true,
        };
        let score = calculate_final_score(&params, 100_000);
        // base: 100000 + 50000 + 50*29 + 1000*10 = 100000+50000+1450+10000 = 161450
        // ascension (original): *2 = 322900
        assert_eq!(score, 322900);
    }

    // --- Test 8: Ascension score bonus (converted alignment) ---

    #[test]
    fn score_ascension_converted() {
        let params = DoneParams {
            how: EndHow::Ascended,
            killer: String::new(),
            deepest_level: 30,
            gold: 50000,
            starting_gold: 0,
            vanquished: vec![],
            conducts: ConductState::new(),
            depth_string: "Astral".to_string(),
            role: Role::Valkyrie,
            original_alignment: false,
        };
        let score = calculate_final_score(&params, 100_000);
        // base: 161450 (same as above)
        // ascension (converted): *1.5 = 161450 + 80725 = 242175
        assert_eq!(score, 242175);
    }

    // --- Test 9: done() produces GameOver event ---

    #[test]
    fn done_produces_gameover() {
        let world = make_test_world();
        let player = world.player();

        // Need Experience component.
        // Since we can't mutate world (it's immutable in done()),
        // we test with default values.
        let params = DoneParams {
            how: EndHow::Died,
            killer: "a gnome".to_string(),
            deepest_level: 3,
            gold: 50,
            starting_gold: 0,
            vanquished: vec![
                VanquishedEntry { name: "gnome".to_string(), count: 3 },
            ],
            conducts: ConductState::new(),
            depth_string: "Dlvl:3".to_string(),
            role: Role::Archeologist,
            original_alignment: true,
        };

        let result = done(&world, player, params);

        assert_eq!(result.how, EndHow::Died);
        assert!(result.score > 0, "score should be positive");

        let has_gameover = result.events.iter().any(|e| matches!(
            e,
            EngineEvent::GameOver { .. }
        ));
        assert!(has_gameover, "should emit GameOver event");
    }

    // --- Test 10: Tombstone has correct data ---

    #[test]
    fn tombstone_data() {
        let world = make_test_world();
        let player = world.player();

        let params = DoneParams {
            how: EndHow::Stoning,
            killer: "a cockatrice".to_string(),
            deepest_level: 7,
            gold: 200,
            starting_gold: 0,
            vanquished: vec![],
            conducts: ConductState::new(),
            depth_string: "Dlvl:7".to_string(),
            role: Role::Rogue,
            original_alignment: true,
        };

        let result = done(&world, player, params);
        let tomb = &result.tombstone;

        assert_eq!(tomb.how, EndHow::Stoning);
        assert_eq!(tomb.killer, "a cockatrice");
        assert_eq!(tomb.role, "Rogue");
        assert_eq!(tomb.level, 1);
        assert_eq!(tomb.gold, 200);
        assert_eq!(tomb.depth, "Dlvl:7");
    }

    // --- Test 11: Early death message ---

    #[test]
    fn early_death_message() {
        let mut world = make_test_world();
        let player = world.player();

        // Turn 1 death.
        let params = DoneParams {
            how: EndHow::Died,
            killer: "a falling rock trap".to_string(),
            deepest_level: 1,
            gold: 0,
            starting_gold: 0,
            vanquished: vec![],
            conducts: ConductState::new(),
            depth_string: "Dlvl:1".to_string(),
            role: Role::Tourist,
            original_alignment: true,
        };

        let result = done(&world, player, params);

        let has_early_msg = result.events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "end-do-not-pass-go"
        ));
        assert!(has_early_msg, "turn 1 death should get early death message");
    }

    // --- Test 12: Quit is not dead ---

    #[test]
    fn quit_not_dead() {
        let world = make_test_world();
        let player = world.player();

        let params = DoneParams {
            how: EndHow::Quit,
            killer: String::new(),
            deepest_level: 1,
            gold: 0,
            starting_gold: 0,
            vanquished: vec![],
            conducts: ConductState::new(),
            depth_string: "Dlvl:1".to_string(),
            role: Role::Wizard,
            original_alignment: true,
        };

        let result = done(&world, player, params);
        assert!(!result.how.is_dead());
    }

    // --- Test 13: Disclosure options ---

    #[test]
    fn disclosure_options() {
        assert_eq!(DisclosureOption::ALL.len(), 6);
        assert!(DisclosureOption::Inventory.prompt().contains("possessions"));
        assert!(DisclosureOption::Vanquished.prompt().contains("vanquished"));
    }

    // --- Test 14: Death tax on gold ---

    #[test]
    fn death_tax() {
        let dead_params = DoneParams {
            how: EndHow::Died,
            killer: "test".to_string(),
            deepest_level: 1,
            gold: 1000,
            starting_gold: 0,
            vanquished: vec![],
            conducts: ConductState::new(),
            depth_string: "Dlvl:1".to_string(),
            role: Role::Wizard,
            original_alignment: true,
        };
        let dead_score = calculate_final_score(&dead_params, 0);

        let alive_params = DoneParams {
            how: EndHow::Escaped,
            gold: 1000,
            ..DoneParams {
                how: EndHow::Escaped,
                killer: "test".to_string(),
                deepest_level: 1,
                gold: 1000,
                starting_gold: 0,
                vanquished: vec![],
                conducts: ConductState::new(),
                depth_string: "Dlvl:1".to_string(),
                role: Role::Wizard,
                original_alignment: true,
            }
        };
        let alive_score = calculate_final_score(&alive_params, 0);

        assert!(dead_score < alive_score, "death should incur gold tax");
    }

    // --- Test 15: Conducts maintained count ---

    #[test]
    fn conducts_maintained() {
        let conducts = ConductState::new();
        // Fresh game: all 13 conducts maintained.
        assert_eq!(conducts.maintained_count(), 13);

        let world = make_test_world();
        let player = world.player();

        let params = DoneParams {
            how: EndHow::Ascended,
            killer: String::new(),
            deepest_level: 30,
            gold: 0,
            starting_gold: 0,
            vanquished: vec![],
            conducts,
            depth_string: "Astral".to_string(),
            role: Role::Monk,
            original_alignment: true,
        };

        let result = done(&world, player, params);
        assert_eq!(result.conducts_maintained, 13);
    }

    // --- Test 16: Tombstone rendering ---

    #[test]
    fn tombstone_rendering() {
        let tomb = Tombstone {
            name: "TestPlayer".to_string(),
            how: EndHow::Died,
            killer: "a gnome".to_string(),
            score: 1234,
            role: "Wizard".to_string(),
            level: 5,
            depth: "Dlvl:3".to_string(),
            turns: 100,
            gold: 50,
            hp: -3,
            maxhp: 42,
        };

        let lines = render_tombstone(&tomb);
        assert!(!lines.is_empty());
        // Check that at least one line contains the player name.
        assert!(
            lines.iter().any(|l| l.contains("TestPlayer")),
            "tombstone should contain player name"
        );
        // Check score is present.
        assert!(
            lines.iter().any(|l| l.contains("1234")),
            "tombstone should contain score"
        );
    }

    // --- Test 17: Vanquished formatting ---

    #[test]
    fn vanquished_format_empty() {
        let lines = format_vanquished(&[]);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("no combatants"));
    }

    #[test]
    fn vanquished_format_entries() {
        let entries = vec![
            VanquishedEntry { name: "orc".to_string(), count: 10 },
            VanquishedEntry { name: "dragon".to_string(), count: 1 },
        ];
        let lines = format_vanquished(&entries);
        assert!(lines[0].contains("11 total"));
        assert!(lines[1].contains("10 orcs"));
        assert!(lines[2].contains("dragon"));
        assert!(!lines[2].contains("dragons")); // singular
    }

    #[test]
    fn test_total_vanquished() {
        let entries = vec![
            VanquishedEntry { name: "a".to_string(), count: 5 },
            VanquishedEntry { name: "b".to_string(), count: 3 },
        ];
        assert_eq!(total_vanquished(&entries), 8);
    }

    // --- Test 18: Conduct summary ---

    #[test]
    fn conduct_summary_all_maintained() {
        let conducts = ConductState::new();
        let lines = format_conduct_summary(&conducts);
        assert!(lines[0].contains("all 13"));
    }

    #[test]
    fn conduct_summary_some_violated() {
        let mut conducts = ConductState::new();
        conducts.food = 5;
        conducts.killer = 3;
        let lines = format_conduct_summary(&conducts);
        assert!(lines[0].contains("11 conduct"));
        // Should have entries for maintained and violated.
        let ate_line = lines.iter().find(|l| l.contains("ate food")).unwrap();
        assert!(ate_line.contains("5 time"));
    }

    // --- Test 19: Ascension check ---

    #[test]
    fn ascension_check_qualified() {
        assert_eq!(
            check_ascension_requirements(true, true, true),
            AscensionCheck::Qualified
        );
    }

    #[test]
    fn ascension_check_missing_amulet() {
        assert_eq!(
            check_ascension_requirements(false, true, true),
            AscensionCheck::MissingAmulet
        );
    }

    #[test]
    fn ascension_check_wrong_plane() {
        assert_eq!(
            check_ascension_requirements(true, false, true),
            AscensionCheck::NotOnAstralPlane
        );
    }

    #[test]
    fn ascension_check_wrong_altar() {
        assert_eq!(
            check_ascension_requirements(true, true, false),
            AscensionCheck::WrongAltar
        );
    }

    // --- Test 20: Ascension sequence ---

    #[test]
    fn ascension_events() {
        let events = ascension_sequence("Hero", &Role::Valkyrie, true);
        assert!(!events.is_empty());
        let has_offering = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "end-ascension-offering"
        ));
        assert!(has_offering);

        let has_demigod = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "end-ascension-demigod"
        ));
        assert!(has_demigod);
    }

    #[test]
    fn ascension_events_converted() {
        let events = ascension_sequence("Hero", &Role::Wizard, false);
        let has_converted = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "end-ascension-demigod-converted"
        ));
        assert!(has_converted);
    }

    // --- Test 21: EndHow code roundtrip ---

    #[test]
    fn end_how_code_roundtrip() {
        for code in 0..=15 {
            let how = EndHow::from_code(code).unwrap();
            assert_eq!(how.to_code(), code);
        }
        assert!(EndHow::from_code(16).is_none());
        assert!(EndHow::from_code(-1).is_none());
    }

    // --- Test 22: DYWYPI defaults ---

    #[test]
    fn dywypi_defaults() {
        let opts = default_disclosure_options(&EndHow::Died);
        assert_eq!(opts.len(), 6);
        assert!(opts.contains(&DisclosureOption::Inventory));
        assert!(opts.contains(&DisclosureOption::Conducts));
    }

    #[test]
    fn auto_disclose_ascension() {
        assert!(auto_disclose(&EndHow::Ascended, DisclosureOption::Inventory));
        assert!(auto_disclose(&EndHow::Ascended, DisclosureOption::Conducts));
        assert!(!auto_disclose(&EndHow::Ascended, DisclosureOption::Overview));
        assert!(!auto_disclose(&EndHow::Died, DisclosureOption::Inventory));
    }

    // --- Test 23: Artifact score calculation ---

    #[test]
    fn artifact_score_amulet_only() {
        let carried = vec!["Amulet of Yendor".to_string()];
        let ever_had = vec![];
        assert_eq!(calculate_artifact_score(&carried, &ever_had), 5000);
    }

    #[test]
    fn artifact_score_quest_items() {
        let carried = vec![
            "Amulet of Yendor".to_string(),
            "Bell of Opening".to_string(),
            "Book of the Dead".to_string(),
            "Candelabrum of Invocation".to_string(),
        ];
        let ever_had = vec![];
        // 5000 + 2500*3 = 12500
        assert_eq!(calculate_artifact_score(&carried, &ever_had), 12500);
    }

    #[test]
    fn artifact_score_with_named_artifacts() {
        let carried = vec!["Amulet of Yendor".to_string()];
        let ever_had = vec![
            "Excalibur".to_string(),
            "Mjollnir".to_string(),
            "Magicbane".to_string(),
        ];
        // 5000 + 3*1000 = 8000
        assert_eq!(calculate_artifact_score(&carried, &ever_had), 8000);
    }

    #[test]
    fn artifact_score_no_double_counting_quest_items() {
        let carried = vec![
            "Bell of Opening".to_string(),
        ];
        let ever_had = vec![
            "Bell of Opening".to_string(),
            "Excalibur".to_string(),
        ];
        // carried: 2500 for Bell
        // ever_had: Bell is quest item (skip), Excalibur = 1000
        // total: 2500 + 1000 = 3500
        assert_eq!(calculate_artifact_score(&carried, &ever_had), 3500);
    }

    #[test]
    fn artifact_score_empty() {
        let carried: Vec<String> = vec![];
        let ever_had: Vec<String> = vec![];
        assert_eq!(calculate_artifact_score(&carried, &ever_had), 0);
    }

    // --- Test 24: Life saving check ---

    #[test]
    fn life_saving_restores_hp() {
        let mut hp = -5;
        let saved = save_life_check(true, &mut hp, 50);
        assert!(saved);
        assert_eq!(hp, 25); // max_hp / 2
    }

    #[test]
    fn life_saving_minimum_1_hp() {
        let mut hp = -10;
        let saved = save_life_check(true, &mut hp, 1);
        assert!(saved);
        assert_eq!(hp, 1); // max_hp/2 = 0, clamped to 1
    }

    #[test]
    fn no_life_saving() {
        let mut hp = -5;
        let saved = save_life_check(false, &mut hp, 50);
        assert!(!saved);
        assert_eq!(hp, -5); // unchanged
    }

    // --- Test 25: Death message formatting ---

    #[test]
    fn death_message_killed_by() {
        let msg = fixup_death_message("gnome", &EndHow::Died);
        assert_eq!(msg, "killed by a gnome");
    }

    #[test]
    fn death_message_killed_by_vowel() {
        let msg = fixup_death_message("orc", &EndHow::Died);
        assert_eq!(msg, "killed by an orc");
    }

    #[test]
    fn death_message_existing_article() {
        let msg = fixup_death_message("the Wizard of Yendor", &EndHow::Died);
        assert_eq!(msg, "killed by the Wizard of Yendor");
    }

    #[test]
    fn death_message_stoned() {
        let msg = fixup_death_message("cockatrice", &EndHow::Stoning);
        assert_eq!(msg, "turned to stone by a cockatrice");
    }

    #[test]
    fn death_message_choked() {
        let msg = fixup_death_message("food ration", &EndHow::Choked);
        assert_eq!(msg, "choked on food ration");
    }

    #[test]
    fn death_message_starvation() {
        let msg = fixup_death_message("", &EndHow::Starvation);
        assert_eq!(msg, "starved to death");
    }

    #[test]
    fn death_message_ascended() {
        let msg = fixup_death_message("", &EndHow::Ascended);
        assert_eq!(msg, "ascended");
    }

    // --- Test 26: Disclosure generation ---

    #[test]
    fn disclosure_generation_inventory() {
        let inventory = vec![
            ('a', "long sword".to_string(), "a sword".to_string(), "uncursed".to_string()),
            ('b', "scroll of identify".to_string(), "scroll labeled LOREM".to_string(), "blessed".to_string()),
        ];
        let disclosure = generate_disclosure(
            &inventory, &[], &[], &[], &[], &[], &[],
        );
        assert_eq!(disclosure.len(), 5);
        match &disclosure[0] {
            DisclosureCategory::Inventory { items } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].letter, 'a');
                assert_eq!(items[0].name, "long sword");
                assert_eq!(items[0].unidentified_name, "a sword");
                assert_eq!(items[0].buc, "uncursed");
            }
            _ => panic!("expected Inventory"),
        }
    }

    #[test]
    fn disclosure_format_vanquished() {
        let kills = vec![
            ("orc".to_string(), 10u32),
            ("dragon".to_string(), 1u32),
            ("gnome".to_string(), 5u32),
        ];
        let category = DisclosureCategory::Vanquished { kills };
        let lines = format_disclosure(&category);
        assert!(lines[0].contains("16 total"));
        // Should be sorted by count descending.
        assert!(lines[1].contains("orc"));
        assert!(lines[1].contains("x10"));
        assert!(lines[2].contains("gnome"));
        assert!(lines[2].contains("x5"));
        assert!(lines[3].contains("dragon"));
        assert!(!lines[3].contains("x1")); // singular has no count
    }

    #[test]
    fn disclosure_format_conducts() {
        let category = DisclosureCategory::Conducts {
            maintained: vec!["pacifist".to_string(), "atheist".to_string()],
            broken: vec!["foodless".to_string()],
        };
        let lines = format_disclosure(&category);
        assert!(lines[0].contains("Voluntary challenges"));
        assert!(lines[1].contains("pacifist"));
        assert!(lines[2].contains("atheist"));
    }

    #[test]
    fn disclosure_format_inventory() {
        let category = DisclosureCategory::Inventory {
            items: vec![DisclosureItem {
                letter: 'a',
                name: "long sword".to_string(),
                unidentified_name: "a sword".to_string(),
                buc: "blessed".to_string(),
            }],
        };
        let lines = format_disclosure(&category);
        assert!(lines[0].contains("inventory"));
        assert!(lines[1].contains("a - long sword (blessed)"));
    }

    #[test]
    fn disclosure_format_empty_conducts() {
        let category = DisclosureCategory::Conducts {
            maintained: vec![],
            broken: vec![],
        };
        let lines = format_disclosure(&category);
        assert!(lines[1].contains("None maintained"));
    }

    // --- Test 27: Game summary ---

    #[test]
    fn game_summary_from_result() {
        let world = make_test_world();
        let player = world.player();

        let params = DoneParams {
            how: EndHow::Died,
            killer: "a gnome".to_string(),
            deepest_level: 5,
            gold: 100,
            starting_gold: 0,
            vanquished: vec![
                VanquishedEntry { name: "gnome".to_string(), count: 3 },
                VanquishedEntry { name: "orc".to_string(), count: 2 },
            ],
            conducts: ConductState::new(),
            depth_string: "Dlvl:5".to_string(),
            role: Role::Wizard,
            original_alignment: true,
        };

        let result = done(&world, player, params);
        let summary = GameSummary::from_result(
            &result,
            "Human",
            "male",
            "neutral",
            5,
            5,
        );

        assert_eq!(summary.name, result.tombstone.name);
        assert_eq!(summary.role, "Wizard");
        assert_eq!(summary.race, "Human");
        assert_eq!(summary.monsters_vanquished, 5);
        assert_eq!(summary.gold, 100);
    }
}
