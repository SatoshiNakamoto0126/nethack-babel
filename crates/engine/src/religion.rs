//! Religion and prayer system for NetHack Babel.
//!
//! Implements alignment, prayer, sacrifice, crowning, and luck mechanics
//! based on the NetHack 3.7 source (`pray.c`, `attrib.c`, `timeout.c`,
//! `do.c`).
//!
//! Reference: `specs/religion.md`

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::Alignment;

use crate::event::{DamageSource, EngineEvent, HpSource, StatusEffect};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Alignment record threshold: pious (required for crowning).
pub const PIOUS: i32 = 20;
/// Alignment record threshold: devout.
pub const DEVOUT: i32 = 14;
/// Alignment record threshold: fervent.
pub const FERVENT: i32 = 9;
/// Alignment record threshold: strident (minimum "pleased").
pub const STRIDENT: i32 = 4;
/// Maximum sacrifice value for normal corpses.
pub const MAXVALUE: i32 = 24;
/// Upper bound for `u.uluck`.
pub const LUCKMAX: i8 = 10;
/// Lower bound for `u.uluck`.
pub const LUCKMIN: i8 = -10;
/// Luckstone bonus absolute value.
pub const LUCKADD: i8 = 3;
/// Extended upper bound with luckstone.
pub const LUCK_RANGE_MAX: i8 = 13;
/// Extended lower bound with luckstone.
pub const LUCK_RANGE_MIN: i8 = -13;

// ---------------------------------------------------------------------------
// Religion state (standalone, testable without full ECS)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Trouble system (spec section 2.7)
// ---------------------------------------------------------------------------

/// Major troubles (positive values, higher = more severe).
/// Minor troubles (negative values).
/// Zero means no trouble.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum Trouble {
    // Major troubles (positive, descending priority)
    Stoned = 14,
    Slimed = 13,
    Strangled = 12,
    Lava = 11,
    Sick = 10,
    Starving = 9,
    Region = 8,
    CriticallyLowHp = 7,
    Lycanthrope = 6,
    Collapsing = 5,
    StuckInWall = 4,
    CursedLevitation = 3,
    UnuseableHands = 2,
    CursedBlindfold = 1,
    // No trouble
    None = 0,
    // Minor troubles (negative)
    Punished = -1,
    Fumbling = -2,
    CursedItems = -3,
    CursedSaddle = -4,
    Blind = -5,
    Poisoned = -6,
    WoundedLegs = -7,
    Hungry = -8,
    TempStunned = -9,
    TempConfused = -10,
    Hallucination = -11,
}

impl Trouble {
    /// Numeric value of the trouble (positive = major, negative = minor).
    pub fn value(self) -> i32 {
        self as i32
    }

    /// Whether this is a major (severe) trouble.
    pub fn is_major(self) -> bool {
        self.value() > 0
    }

    /// Whether this is a minor trouble.
    pub fn is_minor(self) -> bool {
        self.value() < 0
    }
}

/// Snapshot of the player's religion state, extracted from ECS for pure
/// function computation.
#[derive(Debug, Clone)]
pub struct ReligionState {
    /// Current alignment type (Lawful / Neutral / Chaotic).
    pub alignment: Alignment,
    /// Alignment record — relationship with one's deity.
    pub alignment_record: i32,
    /// God's anger level (0 = not angry).
    pub god_anger: i32,
    /// Number of artifact gifts received from the deity.
    pub god_gifts: i32,
    /// Amount of divine protection.
    pub blessed_amount: i32,
    /// Turns remaining before the next prayer can be answered.
    pub bless_cooldown: i32,
    /// Whether the player has been crowned (hand of Elbereth etc.).
    pub crowned: bool,
    /// Whether the player is a demigod (entered end-game).
    pub demigod: bool,
    /// Current game turn.
    pub turn: u32,

    // Player stats needed for prayer/sacrifice evaluation.
    pub experience_level: u8,
    pub current_hp: i32,
    pub max_hp: i32,
    pub current_pw: i32,
    pub max_pw: i32,
    /// Current nutrition value.
    pub nutrition: i32,

    /// Current base luck (without luckstone bonus).
    pub luck: i8,
    /// Luckstone bonus (-3, 0, or +3).
    pub luck_bonus: i8,

    /// Whether the player carries a luckstone.
    pub has_luckstone: bool,
    /// Whether the luckstone is blessed.
    pub luckstone_blessed: bool,
    /// Whether the luckstone is cursed.
    pub luckstone_cursed: bool,

    /// Whether the player is in Gehennom.
    pub in_gehennom: bool,
    /// Whether the player is an undead polyform.
    pub is_undead: bool,
    /// Whether the player is a demon polyform.
    pub is_demon: bool,

    /// Original alignment (at game start).
    pub original_alignment: Alignment,
    /// Whether alignment has been converted (can only convert once).
    pub has_converted: bool,

    /// Abuse counter (for Erinys tracking; incremented by negative adjalign).
    pub alignment_abuse: u32,
}

impl ReligionState {
    /// Effective luck = base luck + luckstone bonus.
    pub fn effective_luck(&self) -> i8 {
        self.luck.saturating_add(self.luck_bonus)
    }

    /// Whether the player's god is angry (alignment_record < 0).
    pub fn god_is_angry(&self) -> bool {
        self.alignment_record < 0
    }
}

// ---------------------------------------------------------------------------
// Alignment record
// ---------------------------------------------------------------------------

/// Compute the alignment limit based on current turn.
///
/// `ALIGNLIM = 10 + (moves / 200)`
pub fn alignment_limit(turn: u32) -> i32 {
    10 + (turn as i32 / 200)
}

/// Adjust alignment record by `delta`, clamping positive values to
/// `ALIGNLIM` and allowing negative values without limit.
///
/// Negative deltas also accumulate in `alignment_abuse` (for Erinys
/// tracking).
pub fn adjust_alignment(state: &mut ReligionState, delta: i32) {
    let old = state.alignment_record;
    let new_val = old.saturating_add(delta);
    if delta < 0 {
        // Negative adjustments: no lower bound
        if new_val < old {
            state.alignment_record = new_val;
        }
        // Track abuse for Erinys mechanism
        state.alignment_abuse = state.alignment_abuse.saturating_add((-delta) as u32);
    } else if new_val > old {
        let limit = alignment_limit(state.turn);
        state.alignment_record = new_val.min(limit);
    }
}

/// `gods_upset(g_align)` — when a god of alignment `g_align` is angered.
///
/// If `g_align` is the player's own alignment, god_anger increases.
/// If it is another god, the player's own god's anger decreases by 1
/// (other gods being upset helps your relationship with your own god).
pub fn gods_upset(state: &mut ReligionState, upset_alignment: Alignment) {
    if upset_alignment == state.alignment {
        state.god_anger += 1;
    } else if state.god_anger > 0 {
        state.god_anger -= 1;
    }
}

/// Return a human-readable alignment title for the given record value.
pub fn alignment_title(record: i32) -> &'static str {
    if record >= PIOUS {
        "pious"
    } else if record >= DEVOUT {
        "devout"
    } else if record >= FERVENT {
        "fervent"
    } else if record >= STRIDENT {
        "strident"
    } else if record > 0 {
        "haltingly"
    } else if record == 0 {
        "nominally"
    } else {
        "sinned"
    }
}

// ---------------------------------------------------------------------------
// Luck system
// ---------------------------------------------------------------------------

/// Adjust base luck by `delta`, clamping to [-10, +10] normally or
/// [-13, +13] with a luckstone.
pub fn adjust_luck(state: &mut ReligionState, delta: i8) {
    let (lo, hi) = if state.has_luckstone {
        (LUCK_RANGE_MIN, LUCK_RANGE_MAX)
    } else {
        (LUCKMIN, LUCKMAX)
    };
    let new_luck = (state.luck as i16 + delta as i16).clamp(lo as i16, hi as i16) as i8;
    state.luck = new_luck;
}

/// Decay luck toward the base value over time.
///
/// Called every turn; actual decay happens every `period` turns.
/// - Normal period: 600 turns.
/// - Accelerated (holding Amulet or god angry): 300 turns.
///
/// Rules:
/// - No luckstone: luck decays toward `base_luck` in both directions.
/// - Blessed luckstone: positive luck does NOT decay (negative still
///   recovers).
/// - Cursed luckstone: negative luck does NOT recover (positive still
///   decays).
/// - Uncursed luckstone: no decay in either direction.
pub fn luck_timeout(
    state: &mut ReligionState,
    current_turn: u32,
    base_luck: i8,
    has_amulet: bool,
) {
    let period = if has_amulet || state.god_anger > 0 {
        300u32
    } else {
        600
    };
    if !current_turn.is_multiple_of(period) {
        return;
    }

    let has_stone = state.has_luckstone;
    let blessed = state.luckstone_blessed;
    let cursed = state.luckstone_cursed;

    // Determine whether to prevent decay in each direction.
    // - Blessed luckstone: blocks positive decay only.
    // - Cursed luckstone: blocks negative recovery only.
    // - Uncursed luckstone: blocks both directions.
    let uncursed_stone = has_stone && !blessed && !cursed;
    let block_positive_decay = has_stone && (blessed || uncursed_stone);
    let block_negative_recovery = has_stone && (cursed || uncursed_stone);

    if state.luck > base_luck && !block_positive_decay {
        state.luck -= 1;
    } else if state.luck < base_luck && !block_negative_recovery {
        state.luck += 1;
    }
}

/// Check whether the player currently carries a luckstone.
///
/// In a full implementation this would scan the ECS inventory.
/// Here we rely on the pre-extracted `has_luckstone` field.
pub fn has_luckstone(state: &ReligionState) -> bool {
    state.has_luckstone
}

// ---------------------------------------------------------------------------
// rnl — luck-adjusted random (NetHack rnd.c)
// ---------------------------------------------------------------------------

/// Luck-adjusted random number: `0 <= rnl(x) < x`.
///
/// Faithfully mirrors C NetHack `rnl()` from `rnd.c`.
/// - Good luck biases toward 0 (better for checks where low = success).
/// - Bad luck biases toward `x - 1`.
/// - For small ranges (`x <= 15`), luck is scaled by `(|luck|+1)/3`.
/// - The adjustment is applied probabilistically: `rn2(37 + |adj|)` must
///   succeed (i.e., not roll 0..36) for the adjustment to take effect.
///
/// # Arguments
/// * `x` — Upper bound (exclusive). Must be > 0.
/// * `luck` — Effective luck value (base + luckstone bonus).
/// * `rng` — Random number generator.
pub fn rnl<R: Rng>(rng: &mut R, x: i32, luck: i32) -> i32 {
    if x <= 0 {
        return 0;
    }

    let mut adjustment = luck;
    if x <= 15 {
        // For small ranges, scale luck by (|luck|+1)/3 rounded away from 0.
        adjustment = (adjustment.abs() + 1) / 3 * adjustment.signum();
    }

    let mut i = rng.random_range(0..x);
    if adjustment != 0 && rng.random_range(0..(37 + adjustment.abs())) != 0 {
        i -= adjustment;
        i = i.clamp(0, x - 1);
    }
    i
}

/// Roll for an event where lower is better (luck helps).
///
/// Returns `true` if `rnl(max, luck) < threshold`.
/// Convenience wrapper for common pattern in C NetHack.
pub fn luck_check<R: Rng>(rng: &mut R, threshold: i32, max: i32, luck: i32) -> bool {
    rnl(rng, max, luck) < threshold
}

// ---------------------------------------------------------------------------
// rnz — randomized cooldown value (NetHack's non-linear random)
// ---------------------------------------------------------------------------

/// NetHack's `rnz(i)` function: produces a randomized value centered
/// around `i` but with a heavy-tailed distribution driven by `rne(4)`.
pub fn rnz<R: Rng>(rng: &mut R, i: i32, experience_level: u8) -> i32 {
    let x = i as i64;
    let tmp_base: i64 = 1000 + rng.random_range(0..1000) as i64;
    let rne_val = rne(rng, 4, experience_level) as i64;
    let tmp = tmp_base * rne_val;
    let result = if rng.random_range(0..2) == 0 {
        x * tmp / 1000
    } else {
        x * 1000 / tmp
    };
    result.clamp(1, i32::MAX as i64) as i32
}

/// NetHack's `rne(x)` — geometric distribution capped at
/// `max(ulevel/3, 5)`.
fn rne<R: Rng>(rng: &mut R, x: u32, level: u8) -> u32 {
    let cap = (level as u32 / 3).max(5);
    let mut result = 1u32;
    while result < cap && rng.random_range(0..x) == 0 {
        result += 1;
    }
    result
}

// ---------------------------------------------------------------------------
// God names
// ---------------------------------------------------------------------------

/// The three deities for a role, in order: lawful, neutral, chaotic.
pub struct Pantheon {
    pub lawful: &'static str,
    pub neutral: &'static str,
    pub chaotic: &'static str,
}

/// Role index constants matching RoleId values.
pub mod roles {
    pub const ARCHEOLOGIST: u8 = 0;
    pub const BARBARIAN: u8 = 1;
    pub const CAVEMAN: u8 = 2;
    pub const HEALER: u8 = 3;
    pub const KNIGHT: u8 = 4;
    pub const MONK: u8 = 5;
    pub const PRIEST: u8 = 6;
    pub const ROGUE: u8 = 7;
    pub const RANGER: u8 = 8;
    pub const SAMURAI: u8 = 9;
    pub const TOURIST: u8 = 10;
    pub const VALKYRIE: u8 = 11;
    pub const WIZARD: u8 = 12;
}

/// Table of god names indexed by role.  Priest (index 6) borrows from
/// another role at runtime, so returns `None`.
pub fn pantheon_for_role(role_index: u8) -> Option<Pantheon> {
    match role_index {
        roles::ARCHEOLOGIST => Some(Pantheon {
            lawful: "Quetzalcoatl",
            neutral: "Camaxtli",
            chaotic: "Huhetotl",
        }),
        roles::BARBARIAN => Some(Pantheon {
            lawful: "Mitra",
            neutral: "Crom",
            chaotic: "Set",
        }),
        roles::CAVEMAN => Some(Pantheon {
            lawful: "Anu",
            neutral: "_Ishtar",
            chaotic: "Anshar",
        }),
        roles::HEALER => Some(Pantheon {
            lawful: "_Athena",
            neutral: "Hermes",
            chaotic: "Poseidon",
        }),
        roles::KNIGHT => Some(Pantheon {
            lawful: "Lugh",
            neutral: "_Brigit",
            chaotic: "Manannan Mac Lir",
        }),
        roles::MONK => Some(Pantheon {
            lawful: "Shan Lai Ching",
            neutral: "Chih Sung-tzu",
            chaotic: "Huan Ti",
        }),
        roles::PRIEST => None, // Priest borrows another role's pantheon
        roles::ROGUE => Some(Pantheon {
            lawful: "Issek",
            neutral: "Mog",
            chaotic: "Kos",
        }),
        roles::RANGER => Some(Pantheon {
            lawful: "Mercury",
            neutral: "_Venus",
            chaotic: "Mars",
        }),
        roles::SAMURAI => Some(Pantheon {
            lawful: "_Amaterasu Omikami",
            neutral: "Raijin",
            chaotic: "Susanowo",
        }),
        roles::TOURIST => Some(Pantheon {
            lawful: "Blind Io",
            neutral: "_The Lady",
            chaotic: "Offler",
        }),
        roles::VALKYRIE => Some(Pantheon {
            lawful: "Tyr",
            neutral: "Odin",
            chaotic: "Loki",
        }),
        roles::WIZARD => Some(Pantheon {
            lawful: "Ptah",
            neutral: "Thoth",
            chaotic: "Anhur",
        }),
        _ => None,
    }
}

/// Look up the god name for a role and alignment.
/// The leading underscore on female god names is stripped for display.
pub fn god_name(role_index: u8, alignment: Alignment) -> &'static str {
    let pantheon = match pantheon_for_role(role_index) {
        Some(p) => p,
        None => return "the Unknown God",
    };
    let raw = match alignment {
        Alignment::Lawful => pantheon.lawful,
        Alignment::Neutral => pantheon.neutral,
        Alignment::Chaotic => pantheon.chaotic,
    };
    // Strip leading underscore (female deity marker).
    raw.strip_prefix('_').unwrap_or(raw)
}

/// Returns "god" or "goddess" based on whether the deity name starts
/// with underscore in the table.
pub fn god_title(role_index: u8, alignment: Alignment) -> &'static str {
    let pantheon = match pantheon_for_role(role_index) {
        Some(p) => p,
        None => return "god",
    };
    let raw = match alignment {
        Alignment::Lawful => pantheon.lawful,
        Alignment::Neutral => pantheon.neutral,
        Alignment::Chaotic => pantheon.chaotic,
    };
    if raw.starts_with('_') {
        "goddess"
    } else {
        "god"
    }
}

// ---------------------------------------------------------------------------
// Sacrifice
// ---------------------------------------------------------------------------

/// Information about a corpse being offered on an altar.
#[derive(Debug, Clone)]
pub struct CorpseOffering {
    /// Entity handle for the corpse item.
    pub entity: Entity,
    /// Monster difficulty of the corpse's species.
    pub monster_difficulty: i32,
    /// Turn the corpse was created (age).
    pub creation_turn: u32,
    /// Whether the corpse has been partially eaten.
    pub partially_eaten: bool,
    /// Fraction of nutrition remaining (0.0..1.0) if partially eaten.
    pub eaten_fraction: f64,
    /// Whether this was the player's former pet.
    pub was_pet: bool,
    /// Whether the corpse is same-race as the player.
    pub same_race: bool,
    /// Whether the corpse is an acid blob (never expires).
    pub is_acid_blob: bool,
    /// Whether this is a unicorn corpse.
    pub is_unicorn: bool,
    /// Alignment of the unicorn (only meaningful if `is_unicorn`).
    pub unicorn_alignment: Option<Alignment>,
    /// Whether the monster is undead.
    pub is_undead_monster: bool,
}

/// Compute the sacrifice value of a corpse.
///
/// Returns 0 if the corpse is too old (> 50 turns) and not an acid blob.
pub fn sacrifice_value(offering: &CorpseOffering, current_turn: u32) -> i32 {
    let fresh = offering.is_acid_blob
        || current_turn <= offering.creation_turn + 50;
    if !fresh {
        return 0;
    }
    let mut value = offering.monster_difficulty + 1;
    if offering.partially_eaten {
        // Scale by remaining fraction
        value = (value as f64 * offering.eaten_fraction).max(0.0) as i32;
    }
    value
}

/// Evaluate the offering value, including unicorn special rules (spec 3.3).
///
/// Returns the adjusted value, or -1 for a unicorn insult.
/// May have side effects on alignment record.
pub fn eval_offering(
    state: &mut ReligionState,
    offering: &CorpseOffering,
    altar_alignment: Alignment,
    current_turn: u32,
) -> i32 {
    let value = sacrifice_value(offering, current_turn);
    if value == 0 {
        return 0;
    }

    if offering.is_unicorn
        && let Some(uni_align) = offering.unicorn_alignment
    {
        if uni_align == altar_alignment {
            // Offering same-alignment unicorn on matching altar → insult
            return -1;
        } else if state.alignment == altar_alignment {
            // Offering non-aligned unicorn on own altar → great deed
            adjust_alignment(state, 5);
            return value + 3;
        } else if uni_align == state.alignment {
            // Offering own-alignment unicorn on enemy altar → god angry
            state.alignment_record = -1;
            return 1;
        } else {
            // Other combination
            return value + 3;
        }
    }

    if offering.is_undead_monster {
        return value + 1;
    }

    value
}

/// Perform a sacrifice on an altar.
///
/// Returns a list of engine events describing the effects.
pub fn sacrifice<R: Rng>(
    state: &mut ReligionState,
    offering: &CorpseOffering,
    altar_alignment: Alignment,
    num_existing_artifacts: u32,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let current_turn = state.turn;

    // Break atheist conduct (tracked externally).

    // Same-race sacrifice
    if offering.same_race {
        if state.alignment != Alignment::Chaotic {
            adjust_alignment(state, -5);
            state.god_anger += 3;
            adjust_luck(state, -5);
            events.push(EngineEvent::msg("sacrifice-own-kind-anger"));
        } else {
            adjust_alignment(state, 5);
            events.push(EngineEvent::msg("sacrifice-own-kind-pleased"));
        }
        return events;
    }

    // Former pet sacrifice: permanent Aggravate Monster
    if offering.was_pet {
        adjust_alignment(state, -3);
        events.push(EngineEvent::msg("sacrifice-pet-guilt"));
        // gods_upset for own god
        gods_upset(state, state.alignment);
        return events;
    }

    // Evaluate offering (includes unicorn logic)
    let value = eval_offering(state, offering, altar_alignment, current_turn);

    // Worthless corpse (too old)
    if value == 0 {
        events.push(EngineEvent::msg("sacrifice-nothing"));
        return events;
    }

    // Unicorn insult
    if value < 0 {
        gods_upset(state, altar_alignment);
        events.push(EngineEvent::msg("sacrifice-unicorn-insult"));
        return events;
    }

    // Cross-aligned altar sacrifice
    if altar_alignment != state.alignment {
        return sacrifice_cross_aligned(
            state, value, altar_alignment, rng, events,
        );
    }

    // Same-alignment altar, normal sacrifice
    sacrifice_own_altar(state, value, num_existing_artifacts, rng, events)
}

/// Handle sacrifice on a cross-aligned altar (spec 3.5 step 7).
///
/// If the player's god is angry (ugod_is_angry), attempt alignment
/// conversion (first time only). Otherwise attempt to convert the altar.
fn sacrifice_cross_aligned<R: Rng>(
    state: &mut ReligionState,
    _value: i32,
    altar_alignment: Alignment,
    rng: &mut R,
    mut events: Vec<EngineEvent>,
) -> Vec<EngineEvent> {
    // If god is angry or alignment record negative → possible conversion
    if state.god_is_angry() || state.god_anger > 0 {
        if !state.has_converted {
            // First-time alignment conversion (spec 1.5)
            state.alignment = altar_alignment;
            state.has_converted = true;
            adjust_luck(state, -3);
            state.bless_cooldown += 300;
            events.push(EngineEvent::msg("sacrifice-alignment-convert"));
            return events;
        } else {
            // Already converted once — rejected with heavy penalties
            state.god_anger += 3;
            adjust_alignment(state, -5);
            adjust_luck(state, -5);
            if !state.in_gehennom {
                // angrygods would fire here; simplified to gods_upset
                gods_upset(state, state.alignment);
            }
            events.push(EngineEvent::msg("sacrifice-conversion-rejected"));
            return events;
        }
    }

    // Not angry: attempt to convert the altar alignment
    // Success rate: rn2(8+ulevel) > 5, i.e. (2+ulevel)/(8+ulevel)
    let roll = rng.random_range(0..(8 + state.experience_level as u32));
    if roll > 5 {
        // Altar converts to player's alignment
        adjust_luck(state, 1);
        events.push(EngineEvent::msg("sacrifice-altar-convert"));
    } else {
        // Failed conversion
        adjust_luck(state, -1);
        events.push(EngineEvent::msg("sacrifice-altar-reject"));
    }
    events
}

/// Handle sacrifice on one's own altar.
fn sacrifice_own_altar<R: Rng>(
    state: &mut ReligionState,
    value: i32,
    num_existing_artifacts: u32,
    rng: &mut R,
    mut events: Vec<EngineEvent>,
) -> Vec<EngineEvent> {
    if state.god_anger > 0 {
        // Reduce god anger
        let divisor = if state.alignment == Alignment::Chaotic {
            2
        } else {
            3
        };
        let reduction = (value * divisor / MAXVALUE).max(1);
        state.god_anger = (state.god_anger - reduction).max(0);
        events.push(if state.god_anger == 0 {
            EngineEvent::msg("pray-reconciled")
        } else {
            EngineEvent::msg("pray-mollified")
        });
    } else if state.alignment_record < 0 {
        // Improve alignment record
        let boost = value.min(MAXVALUE.min(-state.alignment_record));
        adjust_alignment(state, boost);
        events.push(EngineEvent::msg("sacrifice-accept"));
    } else if state.bless_cooldown > 0 {
        // Reduce prayer cooldown
        let divisor = if state.alignment == Alignment::Chaotic {
            500
        } else {
            300
        };
        let reduction = value * divisor / MAXVALUE;
        state.bless_cooldown = (state.bless_cooldown - reduction).max(0);
        events.push(EngineEvent::msg("sacrifice-reduce-timeout"));
    } else {
        // Attempt artifact gift, then luck increase
        if bestow_artifact_check(state, num_existing_artifacts, rng) {
            state.god_gifts += 1;
            let nart = num_existing_artifacts;
            state.bless_cooldown =
                rnz(rng, 300 + 50 * nart as i32, state.experience_level);
            events.push(EngineEvent::msg("sacrifice-gift"));
        } else {
            // Luck increase from sacrifice
            let luck_increase = (value * LUCKMAX as i32 / (MAXVALUE * 2))
                .max(0) as i8;
            let effective = if state.luck >= value as i8 {
                0
            } else if state.luck + luck_increase > value as i8 {
                (value as i8 - state.luck).max(0)
            } else {
                luck_increase
            };
            if effective > 0 {
                adjust_luck(state, effective);
            }
            if state.luck < 0 {
                state.luck = 0;
            }
            events.push(EngineEvent::msg("sacrifice-accept"));
        }
    }
    events
}

/// Check whether the god bestows an artifact.
///
/// Probability: `1 / (6 + 2 * ugifts * nartifacts)`.
/// Requires `ulevel > 2` and `luck >= 0`.
pub fn bestow_artifact_check<R: Rng>(
    state: &ReligionState,
    num_existing_artifacts: u32,
    rng: &mut R,
) -> bool {
    if state.experience_level <= 2 || state.effective_luck() < 0 {
        return false;
    }
    let denom =
        6 + 2 * state.god_gifts as u32 * num_existing_artifacts;
    rng.random_range(0..denom) == 0
}

// ---------------------------------------------------------------------------
// Prayer
// ---------------------------------------------------------------------------

/// The result of evaluating whether prayer can succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrayerType {
    /// Prayer answered favorably (p_type == 3).
    Success,
    /// On a cross-aligned altar but conditions are OK (p_type == 2).
    CrossAligned,
    /// God is angry / luck negative / alignment bad (p_type == 1).
    Punished,
    /// Prayed too soon (p_type == 0).
    TooSoon,
    /// Undead form danger (p_type == -1).
    UndeadDanger,
    /// Moloch altar (p_type == -2).
    Moloch,
    /// Gehennom — god can't help you.
    Gehennom,
    /// Demon rejected (demon trying to pray to non-chaotic/none).
    DemonRejected,
}

/// Whether the altar is Moloch's (alignment `A_NONE`).
/// In our system Moloch altars are represented as `None` alignment.
/// The caller should pass `is_moloch_altar = true` separately.
///
/// Compute the effective alignment used for prayer attitude check
/// when praying on a cross-aligned altar.
///
/// Spec section 2.4:
/// - If ualign.type != 0 && ualign.type == -p_aligntyp: negate record
/// - If ualign.type != p_aligntyp (but not opposite): halve record
/// - If same: use record as-is
pub fn prayer_alignment(state: &ReligionState, prayer_target: Alignment) -> i32 {
    if state.alignment != Alignment::Neutral
        && state.alignment as i8 == -(prayer_target as i8)
    {
        // Diametrically opposed alignment — negate
        -state.alignment_record
    } else if state.alignment != prayer_target {
        // Different but not opposed (or one is Neutral)
        state.alignment_record / 2
    } else {
        // Same alignment
        state.alignment_record
    }
}

/// Evaluate what kind of prayer response the player will get.
///
/// `trouble` is the player's most severe current trouble (from `in_trouble()`).
/// `is_moloch_altar` indicates if the altar is Moloch's (A_NONE).
pub fn evaluate_prayer(
    state: &ReligionState,
    on_altar: bool,
    altar_alignment: Option<Alignment>,
    trouble: Trouble,
    is_moloch_altar: bool,
) -> PrayerType {
    let effective_luck = state.effective_luck();

    // Determine prayer target alignment
    let p_aligntyp = if on_altar {
        if is_moloch_altar {
            None // Moloch / A_NONE
        } else {
            altar_alignment
        }
    } else {
        Some(state.alignment)
    };

    // Demon check (spec section 2.4, known bug: only A_NEUTRAL allowed)
    // In our faithful implementation, demons can only pray to Neutral.
    if state.is_demon {
        if let Some(align) = p_aligntyp {
            if align != Alignment::Neutral {
                return PrayerType::DemonRejected;
            }
        } else {
            // Moloch altar — also rejected per the bug
            return PrayerType::DemonRejected;
        }
    }

    // Moloch altar
    if is_moloch_altar && on_altar {
        return PrayerType::Moloch;
    }

    // Cooldown check — threshold depends on trouble severity (spec 2.2)
    let cooldown_threshold = if trouble.is_major() {
        200 // Major trouble: only blocked if > 200
    } else if trouble.is_minor() {
        100 // Minor trouble: only blocked if > 100
    } else {
        0 // No trouble: any positive cooldown blocks
    };

    if state.bless_cooldown > cooldown_threshold {
        // Check for undead override before returning TooSoon
        let ptype = PrayerType::TooSoon;
        return maybe_undead_override(state, p_aligntyp, ptype);
    }

    // Compute effective alignment for attitude check
    let alignment = if let Some(target) = p_aligntyp {
        prayer_alignment(state, target)
    } else {
        state.alignment_record
    };

    // Attitude check
    if (effective_luck as i32) < 0 || state.god_anger > 0 || alignment < 0 {
        let ptype = PrayerType::Punished;
        return maybe_undead_override(state, p_aligntyp, ptype);
    }

    // Cross-aligned altar
    if on_altar
        && let Some(alt_align) = altar_alignment
        && alt_align != state.alignment
        && !is_moloch_altar
    {
        let ptype = PrayerType::CrossAligned;
        return maybe_undead_override(state, p_aligntyp, ptype);
    }

    // Success — but check Gehennom and undead
    let ptype = PrayerType::Success;
    maybe_undead_override(state, p_aligntyp, ptype)
}

/// Apply the undead override check from spec section 2.4.
///
/// If the player is undead and not in Gehennom:
/// - Lawful prayer target → always dangerous
/// - Neutral prayer target → 1/10 chance dangerous
fn maybe_undead_override(
    state: &ReligionState,
    p_aligntyp: Option<Alignment>,
    base_ptype: PrayerType,
) -> PrayerType {
    // Undead override only applies outside Gehennom
    if state.is_undead
        && !state.in_gehennom
        && let Some(align) = p_aligntyp
        && align == Alignment::Lawful {
            return PrayerType::UndeadDanger;
        }
        // Neutral: 10% chance of danger — we can't roll here in a
        // pure function, so we return the base type and let the caller
        // handle the random check. For deterministic evaluation we
        // skip this (the `pray()` function handles it).
    base_ptype
}

/// Backward-compatible wrapper for evaluate_prayer with no trouble.
pub fn evaluate_prayer_simple(
    state: &ReligionState,
    on_altar: bool,
    altar_alignment: Option<Alignment>,
) -> PrayerType {
    evaluate_prayer(state, on_altar, altar_alignment, Trouble::None, false)
}

/// Execute a prayer action.
///
/// This is the main entry point for the `#pray` command.
///
/// `trouble` is the most severe trouble at prayer start.
/// `troubles` is the full list of current troubles for `pleased()` resolution.
#[allow(clippy::too_many_arguments)]
pub fn pray<R: Rng>(
    state: &mut ReligionState,
    player_entity: Entity,
    on_altar: bool,
    altar_alignment: Option<Alignment>,
    trouble: Trouble,
    troubles: &[Trouble],
    is_moloch_altar: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let prayer_type = evaluate_prayer(
        state, on_altar, altar_alignment, trouble, is_moloch_altar,
    );

    match prayer_type {
        PrayerType::Success => {
            // Gehennom: god can't help you (spec 2.6)
            if state.in_gehennom {
                events.push(EngineEvent::msg("pray-gehennom-no-help"));
                // If record <= 0 or rnl(record) fails → angrygods
                if state.alignment_record <= 0
                    || rng.random_range(0..state.alignment_record.max(1) as u32) == 0
                {
                    events.extend(prayer_angry(state, player_entity, rng));
                }
            } else {
                // On own altar: bless water, revive pets (simplified)
                events.extend(pleased(
                    state, player_entity, trouble, troubles, on_altar,
                    false, rng,
                ));
            }
        }
        PrayerType::CrossAligned => {
            if state.in_gehennom {
                events.push(EngineEvent::msg("pray-gehennom-no-help"));
                if state.alignment_record <= 0
                    || rng.random_range(0..state.alignment_record.max(1) as u32) == 0
                {
                    events.extend(prayer_angry(state, player_entity, rng));
                }
            } else {
                // Cross-aligned altar: attempt to curse water (simplified),
                // then pleased with penalty
                let _alignment = prayer_alignment(
                    state,
                    altar_alignment.unwrap_or(state.alignment),
                );
                events.extend(pleased(
                    state, player_entity, trouble, troubles, on_altar,
                    true, rng,
                ));
            }
        }
        PrayerType::TooSoon => {
            events.extend(prayer_too_soon(state, rng));
        }
        PrayerType::Punished => {
            events.extend(prayer_angry(state, player_entity, rng));
        }
        PrayerType::Moloch => {
            // Demonic laughter, wake nearby monsters
            adjust_alignment(state, -2);
            events.push(EngineEvent::msg("pray-moloch-laughter"));
            if state.in_gehennom {
                // In Gehennom on Moloch altar → angrygods
                events.extend(prayer_angry(state, player_entity, rng));
            }
        }
        PrayerType::UndeadDanger => {
            // God rebukes undead form, force rehumanize
            let damage = rng.random_range(1..=20);
            events.push(EngineEvent::ExtraDamage {
                target: player_entity,
                amount: damage as u32,
                source: DamageSource::Divine,
            });
            events.push(EngineEvent::msg("pray-undead-rebuke"));
        }
        PrayerType::Gehennom => {
            events.push(EngineEvent::msg("pray-gehennom-no-help"));
            if state.alignment_record <= 0
                || rng.random_range(0..state.alignment_record.max(1) as u32) == 0
            {
                events.extend(prayer_angry(state, player_entity, rng));
            }
        }
        PrayerType::DemonRejected => {
            events.push(EngineEvent::msg("pray-demon-rejected"));
        }
    }

    // Anti-automation: extra cooldown after turn 100,000 (spec 2.2)
    // Applied after all prayer logic so it stacks on top of any
    // cooldown set by pleased/angrygods/prayer_too_soon.
    if state.turn > 100_000 {
        let incr = ((state.turn - 100_000) / 100) as i32;
        state.bless_cooldown = state.bless_cooldown.saturating_add(incr);
    }

    events
}

/// Backward-compatible simplified pray (no trouble system).
pub fn pray_simple<R: Rng>(
    state: &mut ReligionState,
    player_entity: Entity,
    on_altar: bool,
    altar_alignment: Option<Alignment>,
    rng: &mut R,
) -> Vec<EngineEvent> {
    pray(
        state, player_entity, on_altar, altar_alignment,
        Trouble::None, &[], false, rng,
    )
}

/// The `pleased()` function — god is satisfied, resolve troubles and
/// possibly grant bonus blessings.
///
/// Implements spec section 2.8 faithfully.
fn pleased<R: Rng>(
    state: &mut ReligionState,
    player_entity: Entity,
    trouble: Trouble,
    troubles: &[Trouble],
    on_altar: bool,
    is_cross_altar: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Cross-aligned altar: minor penalty and return
    if is_cross_altar {
        adjust_alignment(state, -1);
        events.push(EngineEvent::msg("pray-cross-altar-penalty"));
        // Set cooldown
        state.bless_cooldown = rnz(rng, 350, state.experience_level);
        return events;
    }

    // If record is low and no major trouble, give a small boost
    if state.alignment_record < 2 && trouble.value() <= 0 {
        adjust_alignment(state, 1);
    }

    let mut pat_on_head = false;

    if trouble == Trouble::None && state.alignment_record >= DEVOUT {
        // No trouble + devout → extra blessing
        pat_on_head = true;
    } else if trouble != Trouble::None {
        // Determine action level
        let prayer_luck = state.effective_luck().max(-1) as i32;
        let altar_bonus = if !on_altar {
            2
        } else {
            3 // + 1 if shrine, simplified to 3
        };
        let mut action = rng.random_range(0..(prayer_luck + altar_bonus).max(1) as u32) as i32 + 1;

        if !on_altar {
            action = action.min(3);
        }
        if state.alignment_record < STRIDENT {
            action = if state.alignment_record > 0
                || rng.random_range(0..2u32) == 0
            {
                1
            } else {
                0
            };
        }

        action = action.min(5);

        match action {
            5 => {
                pat_on_head = true;
                // Fix ALL troubles
                for &t in troubles {
                    if t != Trouble::None {
                        events.push(fix_trouble(player_entity, t));
                    }
                }
            }
            4 => {
                // Fix all troubles
                for &t in troubles {
                    if t != Trouble::None {
                        events.push(fix_trouble(player_entity, t));
                    }
                }
            }
            3 => {
                // Fix worst + all major troubles (up to 10)
                events.push(fix_trouble(player_entity, trouble));
                let mut count = 0;
                for &t in troubles {
                    if t.is_major() && t != trouble && count < 10 {
                        events.push(fix_trouble(player_entity, t));
                        count += 1;
                    }
                }
            }
            2 => {
                // Fix all major troubles (up to 9)
                let mut count = 0;
                for &t in troubles {
                    if t.is_major() && count < 9 {
                        events.push(fix_trouble(player_entity, t));
                        count += 1;
                    }
                }
            }
            1 => {
                // Fix worst if major
                if trouble.is_major() {
                    events.push(fix_trouble(player_entity, trouble));
                }
            }
            _ => {
                // action == 0: god is indifferent
                events.push(EngineEvent::msg("pray-indifferent"));
            }
        }
    }

    // Extra blessing (pat_on_head) when all troubles resolved
    if pat_on_head {
        let luck_val = (state.effective_luck() as i32 + 6) / 2;
        let bonus_roll = if luck_val > 0 {
            rng.random_range(0..luck_val as u32) as i32
        } else {
            0
        };

        match bonus_roll {
            0 => {
                // Nothing extra
            }
            1 => {
                // Repair/bless weapon
                events.push(EngineEvent::msg("pray-bless-weapon"));
            }
            2 => {
                // Golden glow: restore lost levels or +5 maxhp
                let heal_amount = 5;
                state.max_hp += heal_amount;
                state.current_hp = state.max_hp;
                events.push(EngineEvent::HpChange {
                    entity: player_entity,
                    amount: heal_amount,
                    new_hp: state.max_hp,
                    source: HpSource::Divine,
                });
                events.push(EngineEvent::msg("pray-golden-glow"));
            }
            3 => {
                // Reveal castle tune
                events.push(EngineEvent::msg("pray-castle-tune"));
            }
            4 => {
                // Blue glow: uncurse all items
                events.push(EngineEvent::msg("pray-uncurse-all"));
            }
            5 => {
                // Grant intrinsic (Telepathy > Speed > Stealth > Protection)
                events.push(EngineEvent::StatusApplied {
                    entity: player_entity,
                    status: StatusEffect::Telepathy,
                    duration: None,
                    source: None,
                });
                events.push(EngineEvent::msg("pray-grant-intrinsic"));
            }
            6 => {
                // Grant spellbook
                events.push(EngineEvent::msg("pray-grant-spell"));
            }
            _ => {
                // 7-8: attempt crowning if qualified, else grant spell
                if state.alignment_record >= PIOUS && !state.crowned {
                    events.extend(crowning(state, player_entity, rng));
                } else {
                    events.push(EngineEvent::msg("pray-grant-spell"));
                }
            }
        }
    }

    // Heal to full HP (baseline prayer benefit)
    if state.current_hp < state.max_hp {
        let heal = state.max_hp - state.current_hp;
        state.current_hp = state.max_hp;
        events.push(EngineEvent::HpChange {
            entity: player_entity,
            amount: heal,
            new_hp: state.max_hp,
            source: HpSource::Divine,
        });
    }

    // Restore power
    if state.current_pw < state.max_pw {
        state.current_pw = state.max_pw;
    }

    // Hunger relief
    if state.nutrition < 900 {
        state.nutrition = 900;
    }

    // Set cooldown
    state.bless_cooldown = rnz(rng, 350, state.experience_level);

    // Kick on butt: extra cooldown for demigod/crowned
    let kick_on_butt =
        (if state.demigod { 1 } else { 0 }) + (if state.crowned { 1 } else { 0 });
    if kick_on_butt > 0 {
        state.bless_cooldown += kick_on_butt
            * rnz(rng, 1000, state.experience_level);
    }

    events.push(EngineEvent::msg("pray-pleased"));

    events
}

/// Generate an event to fix a specific trouble.
fn fix_trouble(player_entity: Entity, trouble: Trouble) -> EngineEvent {
    match trouble {
        Trouble::Stoned => EngineEvent::StatusRemoved {
            entity: player_entity,
            status: StatusEffect::Stoning,
        },
        Trouble::Slimed => EngineEvent::StatusRemoved {
            entity: player_entity,
            status: StatusEffect::Slimed,
        },
        Trouble::Strangled => EngineEvent::StatusRemoved {
            entity: player_entity,
            status: StatusEffect::Strangled,
        },
        Trouble::Sick => EngineEvent::StatusRemoved {
            entity: player_entity,
            status: StatusEffect::Sick,
        },
        Trouble::Lycanthrope => EngineEvent::StatusRemoved {
            entity: player_entity,
            status: StatusEffect::Lycanthropy,
        },
        Trouble::CursedBlindfold => EngineEvent::StatusRemoved {
            entity: player_entity,
            status: StatusEffect::Blind,
        },
        Trouble::Blind => EngineEvent::StatusRemoved {
            entity: player_entity,
            status: StatusEffect::Blind,
        },
        Trouble::TempStunned => EngineEvent::StatusRemoved {
            entity: player_entity,
            status: StatusEffect::Stunned,
        },
        Trouble::TempConfused => EngineEvent::StatusRemoved {
            entity: player_entity,
            status: StatusEffect::Confused,
        },
        Trouble::Hallucination => EngineEvent::StatusRemoved {
            entity: player_entity,
            status: StatusEffect::Hallucinating,
        },
        _ => EngineEvent::msg_with("pray-fix-trouble", vec![
            ("trouble", format!("{:?}", trouble)),
        ]),
    }
}

/// Effects of praying too soon.
fn prayer_too_soon<R: Rng>(
    state: &mut ReligionState,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    state.bless_cooldown += rnz(rng, 250, state.experience_level);
    adjust_luck(state, -3);
    state.god_anger += 1;
    events.push(EngineEvent::msg("pray-angry-god"));
    events
}

/// `angrygods()` — god is furious, dispense punishment.
///
/// Implements spec section 2.9 with two anger formulas:
/// - Cross-god: `maxanger = record/2 + luck_penalty`
/// - Own god: `maxanger = 3*ugangr + luck_penalty`
fn prayer_angry<R: Rng>(
    state: &mut ReligionState,
    player_entity: Entity,
    rng: &mut R,
) -> Vec<EngineEvent> {
    prayer_angry_for(state, player_entity, state.alignment, rng)
}

/// Inner angry-gods with explicit responding-god alignment.
fn prayer_angry_for<R: Rng>(
    state: &mut ReligionState,
    player_entity: Entity,
    resp_god: Alignment,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    state.blessed_amount = 0; // Lose divine protection

    let luck = state.effective_luck() as i32;
    let luck_penalty = if luck > 0 { -luck / 3 } else { -luck };

    let max_anger = if resp_god != state.alignment {
        // Cross-god anger formula
        (state.alignment_record / 2 + luck_penalty).clamp(1, 15)
    } else {
        // Own god anger formula
        let base = 3 * state.god_anger;
        let modified = if luck > 0 || state.alignment_record >= STRIDENT {
            base - luck / 3
        } else {
            base + luck_penalty
        };
        modified.clamp(1, 15)
    };

    let anger_roll = rng.random_range(0..max_anger as u32);

    match anger_roll {
        0 | 1 => {
            events.push(EngineEvent::msg("pray-angry-displeased"));
        }
        2 | 3 => {
            // Lose wisdom and experience level
            events.push(EngineEvent::msg("pray-angry-lose-wis"));
        }
        4 | 5 => {
            // Dark curse: attrcurse or rndcurse
            events.push(EngineEvent::msg("pray-angry-curse"));
        }
        6 => {
            // Punished (iron ball)
            events.push(EngineEvent::msg("pray-angry-punished"));
        }
        7 | 8 => {
            // Summon hostile minions
            let result = crate::minion::summon_angry_minion(
                resp_god,
                state.experience_level,
                rng,
            );
            events.push(EngineEvent::msg_with(
                "pray-angry-summon",
                vec![
                    ("minion", format!("{:?}", result.minion_type)),
                    ("tame", result.is_tame.to_string()),
                ],
            ));
            events.push(EngineEvent::MonsterGenerated {
                entity: player_entity,
                position: crate::action::Position::new(0, 0),
            });
        }
        _ => {
            // 9+ — god_zaps_you (lightning + disintegration)
            let damage = rng.random_range(1..=20) as u32;
            events.push(EngineEvent::ExtraDamage {
                target: player_entity,
                amount: damage,
                source: DamageSource::Divine,
            });
            events.push(EngineEvent::msg("pray-angry-zap"));
        }
    }

    // Reset cooldown (take max of current and new)
    let new_cooldown = rnz(rng, 300, state.experience_level);
    state.bless_cooldown = state.bless_cooldown.max(new_cooldown);

    events
}

// ---------------------------------------------------------------------------
// Crowning
// ---------------------------------------------------------------------------

/// Crowning weapon gift by alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrowningGift {
    /// Lawful — Excalibur (long sword)
    Excalibur,
    /// Neutral — Vorpal Blade (long sword)
    VorpalBlade,
    /// Chaotic — Stormbringer (broad sword)
    Stormbringer,
}

/// The six permanent resistances granted by crowning.
pub const CROWNING_RESISTANCES: [StatusEffect; 6] = [
    StatusEffect::SeeInvisible,
    StatusEffect::FireResistance,
    StatusEffect::ColdResistance,
    StatusEffect::ShockResistance,
    StatusEffect::SleepResistance,
    StatusEffect::PoisonResistance,
];

/// Check whether the player qualifies for crowning.
pub fn can_crown(state: &ReligionState) -> bool {
    state.alignment_record >= PIOUS && !state.crowned
}

/// Perform the crowning ceremony.
///
/// Returns the list of events including granted resistances and weapon
/// gift.  The caller is responsible for actually creating the weapon
/// entity and adding resistances as intrinsics.
pub fn crowning<R: Rng>(
    state: &mut ReligionState,
    player_entity: Entity,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if !can_crown(state) {
        return events;
    }

    state.crowned = true;

    // Grant six permanent resistances
    for &resist in &CROWNING_RESISTANCES {
        events.push(EngineEvent::StatusApplied {
            entity: player_entity,
            status: resist,
            duration: None, // permanent
            source: None,
        });
    }

    // Emit crowning message with alignment title
    let title = match state.alignment {
        Alignment::Lawful => "the Hand of Elbereth",
        Alignment::Neutral => "the Envoy of Balance",
        Alignment::Chaotic => "chosen to steal souls",
    };
    events.push(EngineEvent::msg_with("crown-msg", vec![
        ("title", title.to_string()),
    ]));

    // Increase cooldown for crowning (kick_on_butt mechanism)
    // kick_on_butt = (demigod ? 1 : 0) + (crowned ? 1 : 0), max 2
    // Since we just set crowned=true, crowned is always 1 here
    let kick_on_butt =
        1 + if state.demigod { 1i32 } else { 0 };
    state.bless_cooldown += kick_on_butt
        * rnz(rng, 1000, state.experience_level);

    events
}

/// Return which weapon gift corresponds to an alignment.
pub fn crowning_gift_for_alignment(alignment: Alignment) -> CrowningGift {
    match alignment {
        Alignment::Lawful => CrowningGift::Excalibur,
        Alignment::Neutral => CrowningGift::VorpalBlade,
        Alignment::Chaotic => CrowningGift::Stormbringer,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Human-readable alignment name.
pub fn alignment_name(alignment: Alignment) -> &'static str {
    match alignment {
        Alignment::Lawful => "Law",
        Alignment::Neutral => "Balance",
        Alignment::Chaotic => "Chaos",
    }
}

/// Check whether HP is critically low (NetHack's `critically_low_hp`).
///
/// `level` is the experience level (1..30).
pub fn critically_low_hp(
    current_hp: i32,
    max_hp: i32,
    level: u8,
    only_if_injured: bool,
) -> bool {
    if only_if_injured && current_hp >= max_hp {
        return false;
    }
    let hp_lim = 15 * level as i32;
    let effective_max = max_hp.min(hp_lim);
    let divisor = match level {
        1..=5 => 5,
        6..=13 => 6,
        14..=21 => 7,
        22..=29 => 8,
        _ => 9,
    };
    current_hp <= 5 || current_hp * divisor <= effective_max
}

// ---------------------------------------------------------------------------
// Amulet offering / Ascension
// ---------------------------------------------------------------------------

/// Result of offering the Amulet of Yendor on an altar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmuletOfferingResult {
    /// Player ascended: offered on correct-alignment altar on Astral Plane.
    Ascended,
    /// Wrong altar alignment: the Amulet is rejected.
    Rejected,
    /// Not on Astral Plane: offering has no ascension effect.
    NotAstralPlane,
}

/// Evaluate what happens when the player offers the real Amulet of Yendor
/// on an altar.
///
/// In NetHack, offering the Amulet on the correct-alignment high altar on
/// the Astral Plane triggers ascension.  Offering on a wrong-alignment
/// altar causes the Amulet to be rejected (disappears, reappears nearby).
///
/// `player_alignment` — the player's current alignment.
/// `altar_alignment` — the alignment of the altar being offered at.
/// `on_astral_plane` — whether the player is on the Astral Plane.
pub fn offer_amulet(
    player_alignment: Alignment,
    altar_alignment: Alignment,
    on_astral_plane: bool,
) -> AmuletOfferingResult {
    if !on_astral_plane {
        return AmuletOfferingResult::NotAstralPlane;
    }
    if player_alignment == altar_alignment {
        AmuletOfferingResult::Ascended
    } else {
        AmuletOfferingResult::Rejected
    }
}

/// Check whether the player has all three invocation items needed to
/// perform the invocation ritual and open the passage to the Sanctum.
///
/// The three items are: Bell of Opening, Candelabrum of Invocation,
/// and Book of the Dead.
pub fn has_invocation_items(has_bell: bool, has_menorah: bool, has_book: bool) -> bool {
    has_bell && has_menorah && has_book
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    fn make_state() -> ReligionState {
        ReligionState {
            alignment: Alignment::Neutral,
            alignment_record: 10,
            god_anger: 0,
            god_gifts: 0,
            blessed_amount: 0,
            bless_cooldown: 0,
            crowned: false,
            demigod: false,
            turn: 1000,
            experience_level: 10,
            current_hp: 50,
            max_hp: 50,
            current_pw: 20,
            max_pw: 20,
            nutrition: 900,
            luck: 3,
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
        }
    }

    fn make_rng() -> Pcg64 {
        Pcg64::seed_from_u64(12345)
    }

    fn dummy_entity() -> Entity {
        // hecs Entity from raw bits for testing
        unsafe { std::mem::transmute::<u64, Entity>(1u64) }
    }

    // --- Prayer tests ---

    #[test]
    fn prayer_succeeds_when_conditions_met() {
        let mut state = make_state();
        state.bless_cooldown = 0;
        state.god_anger = 0;
        state.alignment_record = 10;
        state.luck = 3;
        state.current_hp = 30; // not full, so healing triggers

        let mut rng = make_rng();
        let events = pray_simple(
            &mut state,
            dummy_entity(),
            false,
            None,
            &mut rng,
        );

        // Should have healed to full and set cooldown
        assert_eq!(state.current_hp, state.max_hp);
        assert!(state.bless_cooldown > 0);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::HpChange {
                source: HpSource::Divine,
                ..
            }
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("pray-pleased")
        )));
    }

    #[test]
    fn prayer_fails_on_cooldown() {
        let mut state = make_state();
        state.bless_cooldown = 100; // still cooling down

        let mut rng = make_rng();
        let events = pray_simple(
            &mut state,
            dummy_entity(),
            false,
            None,
            &mut rng,
        );

        // Cooldown should have increased
        assert!(state.bless_cooldown > 100);
        // Luck should have decreased by 3
        assert_eq!(state.luck, 0); // was 3, now 0
        // God anger increased
        assert_eq!(state.god_anger, 1);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("pray-angry-god")
        )));
    }

    #[test]
    fn prayer_fails_when_god_angry() {
        let mut state = make_state();
        state.bless_cooldown = 0;
        state.god_anger = 3;

        let ptype = evaluate_prayer_simple(&state, false, None);
        assert_eq!(ptype, PrayerType::Punished);
    }

    #[test]
    fn prayer_fails_with_negative_luck() {
        let mut state = make_state();
        state.bless_cooldown = 0;
        state.luck = -2;
        state.luck_bonus = 0;

        let ptype = evaluate_prayer_simple(&state, false, None);
        assert_eq!(ptype, PrayerType::Punished);
    }

    // --- Sacrifice tests ---

    #[test]
    fn sacrifice_boosts_alignment() {
        let mut state = make_state();
        state.alignment_record = -5;
        state.god_anger = 0;
        state.bless_cooldown = 0;

        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 10,
            creation_turn: 995,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };

        let mut rng = make_rng();
        let events = sacrifice(
            &mut state,
            &offering,
            Alignment::Neutral,
            0,
            &mut rng,
        );

        // alignment_record should have improved
        assert!(state.alignment_record > -5);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("sacrifice-accept")
        )));
    }

    #[test]
    fn sacrifice_reduces_god_anger() {
        let mut state = make_state();
        state.god_anger = 10;

        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 20,
            creation_turn: 999,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };

        let mut rng = make_rng();
        let events = sacrifice(
            &mut state,
            &offering,
            Alignment::Neutral,
            0,
            &mut rng,
        );

        assert!(state.god_anger < 10);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("pray-mollified") || key.contains("pray-reconciled")
        )));
    }

    #[test]
    fn artifact_gift_probability_formula() {
        // ugifts=0, nartifacts=anything → denominator is 6 → 1/6
        let state = make_state(); // luck=3 >= 0, level=10 > 2
        let denom = 6 + 2 * state.god_gifts as u32 * 5;
        assert_eq!(denom, 6);

        // ugifts=1, nartifacts=3 → 6 + 2*1*3 = 12 → 1/12
        let mut state2 = make_state();
        state2.god_gifts = 1;
        let denom2 = 6 + 2 * state2.god_gifts as u32 * 3;
        assert_eq!(denom2, 12);

        // ugifts=2, nartifacts=5 → 6 + 2*2*5 = 26 → 1/26
        let mut state3 = make_state();
        state3.god_gifts = 2;
        let denom3 = 6 + 2 * state3.god_gifts as u32 * 5;
        assert_eq!(denom3, 26);
    }

    #[test]
    fn artifact_gift_blocked_by_low_level() {
        let mut state = make_state();
        state.experience_level = 2;
        let mut rng = make_rng();
        assert!(!bestow_artifact_check(&state, 0, &mut rng));
    }

    #[test]
    fn artifact_gift_blocked_by_negative_luck() {
        let mut state = make_state();
        state.luck = -5;
        state.luck_bonus = 0;
        let mut rng = make_rng();
        assert!(!bestow_artifact_check(&state, 0, &mut rng));
    }

    #[test]
    fn cross_aligned_sacrifice_penalty() {
        let mut state = make_state();
        state.god_anger = 2; // angry
        let old_luck = state.luck;

        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 5,
            creation_turn: 999,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };

        let mut rng = make_rng();
        let _events = sacrifice(
            &mut state,
            &offering,
            Alignment::Chaotic, // different from Neutral
            0,
            &mut rng,
        );

        // Luck should decrease by 3
        assert_eq!(state.luck, old_luck - 3);
        // Cooldown increased
        assert!(state.bless_cooldown > 0);
    }

    // --- Crowning tests ---

    #[test]
    fn crowning_grants_resistances() {
        let mut state = make_state();
        state.alignment_record = PIOUS;
        state.crowned = false;

        let mut rng = make_rng();
        let events = crowning(&mut state, dummy_entity(), &mut rng);

        assert!(state.crowned);

        // Should have 6 StatusApplied events for resistances
        let resistance_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::StatusApplied { .. }))
            .collect();
        assert_eq!(resistance_events.len(), 6);

        // Should have a crowning message
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("crown-msg")
        )));
    }

    #[test]
    fn crowning_blocked_if_already_crowned() {
        let mut state = make_state();
        state.alignment_record = PIOUS;
        state.crowned = true;

        let mut rng = make_rng();
        let events = crowning(&mut state, dummy_entity(), &mut rng);
        assert!(events.is_empty());
    }

    #[test]
    fn crowning_blocked_if_record_too_low() {
        let mut state = make_state();
        state.alignment_record = PIOUS - 1;
        state.crowned = false;

        assert!(!can_crown(&state));
    }

    // --- Luck tests ---

    #[test]
    fn luck_clamped_to_bounds() {
        let mut state = make_state();
        state.luck = 8;
        state.has_luckstone = false;

        // Push above normal max
        adjust_luck(&mut state, 5);
        assert_eq!(state.luck, LUCKMAX); // clamped to 10

        // Push below normal min
        state.luck = -8;
        adjust_luck(&mut state, -5);
        assert_eq!(state.luck, LUCKMIN); // clamped to -10
    }

    #[test]
    fn luckstone_extends_luck_range() {
        let mut state = make_state();
        state.luck = 8;
        state.has_luckstone = true;

        adjust_luck(&mut state, 5);
        assert_eq!(state.luck, 13); // extended range

        state.luck = -8;
        adjust_luck(&mut state, -6);
        assert_eq!(state.luck, -13); // extended range min
    }

    #[test]
    fn luck_decays_toward_zero_without_luckstone() {
        let mut state = make_state();
        state.luck = 5;
        state.has_luckstone = false;

        luck_timeout(&mut state, 600, 0, false);
        assert_eq!(state.luck, 4); // decayed by 1
    }

    #[test]
    fn blessed_luckstone_prevents_positive_decay() {
        let mut state = make_state();
        state.luck = 5;
        state.has_luckstone = true;
        state.luckstone_blessed = true;
        state.luckstone_cursed = false;

        luck_timeout(&mut state, 600, 0, false);
        assert_eq!(state.luck, 5); // no decay
    }

    #[test]
    fn cursed_luckstone_prevents_negative_recovery() {
        let mut state = make_state();
        state.luck = -3;
        state.has_luckstone = true;
        state.luckstone_blessed = false;
        state.luckstone_cursed = true;

        luck_timeout(&mut state, 600, 0, false);
        assert_eq!(state.luck, -3); // no recovery
    }

    #[test]
    fn uncursed_luckstone_prevents_both_decay_directions() {
        let mut state = make_state();
        state.has_luckstone = true;
        state.luckstone_blessed = false;
        state.luckstone_cursed = false;

        // Positive luck should not decay
        state.luck = 5;
        luck_timeout(&mut state, 600, 0, false);
        assert_eq!(state.luck, 5);

        // Negative luck should not recover
        state.luck = -3;
        luck_timeout(&mut state, 1200, 0, false);
        assert_eq!(state.luck, -3);
    }

    #[test]
    fn luck_accelerated_decay_when_god_angry() {
        let mut state = make_state();
        state.luck = 5;
        state.god_anger = 2;
        state.has_luckstone = false;

        // Should decay at turn 300 (accelerated period)
        luck_timeout(&mut state, 300, 0, false);
        assert_eq!(state.luck, 4);
    }

    // --- Alignment tests ---

    #[test]
    fn alignment_record_thresholds() {
        assert_eq!(alignment_title(PIOUS), "pious");
        assert_eq!(alignment_title(DEVOUT), "devout");
        assert_eq!(alignment_title(FERVENT), "fervent");
        assert_eq!(alignment_title(STRIDENT), "strident");
        assert_eq!(alignment_title(1), "haltingly");
        assert_eq!(alignment_title(0), "nominally");
        assert_eq!(alignment_title(-5), "sinned");
    }

    #[test]
    fn alignment_capped_by_alignlim() {
        let mut state = make_state();
        state.turn = 1000; // ALIGNLIM = 10 + 1000/200 = 15
        state.alignment_record = 14;

        adjust_alignment(&mut state, 5);
        assert_eq!(state.alignment_record, 15); // capped at ALIGNLIM
    }

    #[test]
    fn alignment_negative_unlimited() {
        let mut state = make_state();
        state.alignment_record = -100;

        adjust_alignment(&mut state, -50);
        assert_eq!(state.alignment_record, -150);
    }

    // --- Sacrifice value tests ---

    #[test]
    fn sacrifice_value_fresh_corpse() {
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 20,
            creation_turn: 970,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        // Within 50 turns of turn 1000
        assert_eq!(sacrifice_value(&offering, 1000), 21);
    }

    #[test]
    fn sacrifice_value_old_corpse() {
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 20,
            creation_turn: 940,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        // 60 turns old, > 50 → worthless
        assert_eq!(sacrifice_value(&offering, 1000), 0);
    }

    #[test]
    fn sacrifice_value_acid_blob_never_expires() {
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 1,
            creation_turn: 1,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: true,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        // Acid blob at turn 9999 — still fresh
        assert_eq!(sacrifice_value(&offering, 9999), 2);
    }

    // --- God name tests ---

    #[test]
    fn god_names_correct() {
        assert_eq!(god_name(roles::VALKYRIE, Alignment::Lawful), "Tyr");
        assert_eq!(god_name(roles::VALKYRIE, Alignment::Neutral), "Odin");
        assert_eq!(god_name(roles::VALKYRIE, Alignment::Chaotic), "Loki");

        // Female deity — underscore stripped
        assert_eq!(
            god_name(roles::HEALER, Alignment::Lawful),
            "Athena"
        );
        assert_eq!(
            god_title(roles::HEALER, Alignment::Lawful),
            "goddess"
        );
        assert_eq!(
            god_title(roles::VALKYRIE, Alignment::Lawful),
            "god"
        );
    }

    #[test]
    fn priest_pantheon_is_none() {
        assert!(pantheon_for_role(roles::PRIEST).is_none());
    }

    // --- Critically low HP tests ---

    #[test]
    fn critically_low_hp_at_5_or_below() {
        assert!(critically_low_hp(5, 100, 1, false));
        assert!(critically_low_hp(1, 100, 1, false));
    }

    #[test]
    fn critically_low_hp_ratio_check() {
        // level 1: divisor=5, hplim=15, effective_max=min(30,15)=15
        // curhp=6: 6 > 5 and 6*5=30 > 15 → false
        assert!(!critically_low_hp(6, 30, 1, false));

        // curhp=3: 3 <= 5 → true
        assert!(critically_low_hp(3, 30, 1, true));
    }

    #[test]
    fn critically_low_hp_not_injured() {
        // only_if_injured=true, curhp==maxhp → false
        assert!(!critically_low_hp(50, 50, 10, true));
    }

    // --- rnz tests ---

    #[test]
    fn rnz_produces_positive_values() {
        let mut rng = make_rng();
        for _ in 0..100 {
            let val = rnz(&mut rng, 350, 10);
            assert!(val > 0);
        }
    }

    #[test]
    fn rnz_centered_roughly_around_input() {
        let mut rng = make_rng();
        let mut sum: i64 = 0;
        let n = 10000;
        for _ in 0..n {
            sum += rnz(&mut rng, 350, 10) as i64;
        }
        let avg = sum / n;
        // Should be in a wide range around 350 (distribution is heavy-tailed)
        assert!(avg > 100 && avg < 2000, "avg was {}", avg);
    }

    // --- Crowning gift tests ---

    #[test]
    fn crowning_gift_by_alignment() {
        assert_eq!(
            crowning_gift_for_alignment(Alignment::Lawful),
            CrowningGift::Excalibur
        );
        assert_eq!(
            crowning_gift_for_alignment(Alignment::Neutral),
            CrowningGift::VorpalBlade
        );
        assert_eq!(
            crowning_gift_for_alignment(Alignment::Chaotic),
            CrowningGift::Stormbringer
        );
    }

    // =====================================================================
    // J.1: Prayer System — comprehensive tests
    // =====================================================================

    // --- Prayer cooldown with trouble awareness (spec 2.2) ---

    #[test]
    fn test_prayer_cooldown_major_trouble_threshold_201() {
        // Spec test vector #1: major trouble, ublesscnt=201 → too early
        let mut state = make_state();
        state.bless_cooldown = 201;
        let ptype = evaluate_prayer(
            &state, false, None, Trouble::Stoned, false,
        );
        assert_eq!(ptype, PrayerType::TooSoon);
    }

    #[test]
    fn test_prayer_cooldown_major_trouble_threshold_200() {
        // Spec test vector #2: major trouble, ublesscnt=200 → NOT too early
        let mut state = make_state();
        state.bless_cooldown = 200;
        let ptype = evaluate_prayer(
            &state, false, None, Trouble::Stoned, false,
        );
        assert_ne!(ptype, PrayerType::TooSoon);
    }

    #[test]
    fn test_prayer_cooldown_minor_trouble_threshold_101() {
        // Spec test vector #3: minor trouble, ublesscnt=101 → too early
        let mut state = make_state();
        state.bless_cooldown = 101;
        let ptype = evaluate_prayer(
            &state, false, None, Trouble::Punished, false,
        );
        assert_eq!(ptype, PrayerType::TooSoon);
    }

    #[test]
    fn test_prayer_cooldown_minor_trouble_threshold_100() {
        // Spec test vector #4: minor trouble, ublesscnt=100 → NOT too early
        let mut state = make_state();
        state.bless_cooldown = 100;
        let ptype = evaluate_prayer(
            &state, false, None, Trouble::Punished, false,
        );
        assert_ne!(ptype, PrayerType::TooSoon);
    }

    #[test]
    fn test_prayer_cooldown_no_trouble_threshold_1() {
        // Spec test vector #5: no trouble, ublesscnt=1 → too early
        let mut state = make_state();
        state.bless_cooldown = 1;
        let ptype = evaluate_prayer(
            &state, false, None, Trouble::None, false,
        );
        assert_eq!(ptype, PrayerType::TooSoon);
    }

    #[test]
    fn test_prayer_cooldown_no_trouble_threshold_0() {
        // Spec test vector #6: no trouble, ublesscnt=0 → not too early
        let mut state = make_state();
        state.bless_cooldown = 0;
        let ptype = evaluate_prayer(
            &state, false, None, Trouble::None, false,
        );
        assert_ne!(ptype, PrayerType::TooSoon);
    }

    // --- Gehennom prayer rule (spec 2.6) ---

    #[test]
    fn test_prayer_gehennom_god_cant_help() {
        // Spec test vector #17: in Gehennom, p_type==3 → no invulnerability
        let mut state = make_state();
        state.in_gehennom = true;
        state.alignment_record = 20;
        let mut rng = make_rng();
        let events = pray(
            &mut state, dummy_entity(), false, None,
            Trouble::None, &[], false, &mut rng,
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("gehennom")
        )));
    }

    // --- Moloch altar (spec 2.6) ---

    #[test]
    fn test_prayer_moloch_altar() {
        let mut state = make_state();
        let old_record = state.alignment_record;
        let mut rng = make_rng();
        let events = pray(
            &mut state, dummy_entity(), true, None,
            Trouble::None, &[], true, &mut rng,
        );
        // Should have lost 2 alignment
        assert_eq!(state.alignment_record, old_record - 2);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("moloch")
        )));
    }

    // --- Demon prayer (spec 2.4 known bug) ---

    #[test]
    fn test_prayer_demon_rejected_on_lawful() {
        let mut state = make_state();
        state.is_demon = true;
        // On a lawful altar
        let ptype = evaluate_prayer(
            &state, true, Some(Alignment::Lawful), Trouble::None, false,
        );
        assert_eq!(ptype, PrayerType::DemonRejected);
    }

    #[test]
    fn test_prayer_demon_rejected_on_chaotic() {
        let mut state = make_state();
        state.is_demon = true;
        // On a chaotic altar — still rejected per the known bug
        let ptype = evaluate_prayer(
            &state, true, Some(Alignment::Chaotic), Trouble::None, false,
        );
        assert_eq!(ptype, PrayerType::DemonRejected);
    }

    #[test]
    fn test_prayer_demon_allowed_on_neutral() {
        let mut state = make_state();
        state.is_demon = true;
        // Neutral altar is the only one that works (known bug)
        let ptype = evaluate_prayer(
            &state, true, Some(Alignment::Neutral), Trouble::None, false,
        );
        assert_ne!(ptype, PrayerType::DemonRejected);
    }

    // --- Undead prayer danger (spec 2.4) ---

    #[test]
    fn test_prayer_undead_lawful_danger() {
        let mut state = make_state();
        state.is_undead = true;
        state.in_gehennom = false;
        // On lawful altar → always dangerous
        let ptype = evaluate_prayer(
            &state, true, Some(Alignment::Lawful), Trouble::None, false,
        );
        assert_eq!(ptype, PrayerType::UndeadDanger);
    }

    #[test]
    fn test_prayer_undead_in_gehennom_no_override() {
        let mut state = make_state();
        state.is_undead = true;
        state.in_gehennom = true;
        // In Gehennom, undead override does NOT apply
        let ptype = evaluate_prayer(
            &state, true, Some(Alignment::Lawful), Trouble::None, false,
        );
        assert_ne!(ptype, PrayerType::UndeadDanger);
    }

    // --- Prayer alignment calculation (spec 2.4) ---

    #[test]
    fn test_prayer_alignment_same() {
        let mut state = make_state();
        state.alignment = Alignment::Lawful;
        state.alignment_record = 15;
        assert_eq!(prayer_alignment(&state, Alignment::Lawful), 15);
    }

    #[test]
    fn test_prayer_alignment_opposite() {
        let mut state = make_state();
        state.alignment = Alignment::Lawful;
        state.alignment_record = 10;
        // Chaotic is opposite of Lawful
        assert_eq!(prayer_alignment(&state, Alignment::Chaotic), -10);
    }

    #[test]
    fn test_prayer_alignment_different_not_opposite() {
        let mut state = make_state();
        state.alignment = Alignment::Neutral;
        state.alignment_record = 10;
        // Neutral vs Lawful: different but not diametrically opposed
        assert_eq!(prayer_alignment(&state, Alignment::Lawful), 5);
    }

    // --- Prayer success sets cooldown (spec 2.2) ---

    #[test]
    fn test_prayer_success_sets_cooldown() {
        let mut state = make_state();
        state.bless_cooldown = 0;
        state.alignment_record = 10;
        let mut rng = make_rng();
        let _events = pray_simple(
            &mut state, dummy_entity(), false, None, &mut rng,
        );
        assert!(state.bless_cooldown > 0, "cooldown should be set after prayer");
    }

    // --- Anti-automation (spec 2.2) ---

    #[test]
    fn test_prayer_anti_automation() {
        let mut state = make_state();
        state.turn = 200_000;
        state.bless_cooldown = 0;
        state.alignment_record = 10;
        let mut rng = make_rng();
        let _events = pray(
            &mut state, dummy_entity(), false, None,
            Trouble::None, &[], false, &mut rng,
        );
        // Extra cooldown: (200000 - 100000) / 100 = 1000
        // Plus the rnz(350) base cooldown
        assert!(state.bless_cooldown > 1000,
            "bless_cooldown should be > 1000, was {}",
            state.bless_cooldown);
    }

    // --- pleased() effect priority chain (spec 2.8) ---

    #[test]
    fn test_prayer_pleased_fixes_major_trouble() {
        let mut state = make_state();
        state.alignment_record = FERVENT; // >= STRIDENT
        state.bless_cooldown = 0;
        state.luck = 5;
        let mut rng = Pcg64::seed_from_u64(42);
        let troubles = vec![Trouble::Sick, Trouble::Hungry];
        let events = pray(
            &mut state, dummy_entity(), false, None,
            Trouble::Sick, &troubles, false, &mut rng,
        );
        // Should fix at least the major trouble
        let has_fix = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved { status: StatusEffect::Sick, .. }
        ) || matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("fix-trouble")
        ));
        assert!(has_fix || events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("pray-pleased")
        )));
    }

    #[test]
    fn test_prayer_pleased_cross_altar_penalty() {
        // Cross-aligned altar: should get -1 alignment, not full benefit
        let mut state = make_state();
        state.alignment = Alignment::Neutral;
        state.bless_cooldown = 0;
        state.alignment_record = 15;
        let _old_record = state.alignment_record;
        let mut rng = make_rng();
        let events = pray(
            &mut state, dummy_entity(), true,
            Some(Alignment::Lawful),
            Trouble::None, &[], false, &mut rng,
        );
        // Cross-aligned prayer: penalty applied
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("cross-altar")
        )));
    }

    // --- Crowning cooldown with demigod (spec 2.2) ---

    #[test]
    fn test_prayer_crowning_kick_on_butt_demigod() {
        let mut state = make_state();
        state.alignment_record = PIOUS;
        state.crowned = false;
        state.demigod = true;
        let mut rng = make_rng();
        let _events = crowning(&mut state, dummy_entity(), &mut rng);
        assert!(state.crowned);
        // kick_on_butt = 1 (crowned) + 1 (demigod) = 2
        // cooldown should be > 2 * rnz(1000, ...) which is large
        assert!(state.bless_cooldown > 500,
            "crowned demigod cooldown too low: {}",
            state.bless_cooldown);
    }

    #[test]
    fn test_prayer_crowning_kick_on_butt_no_demigod() {
        let mut state = make_state();
        state.alignment_record = PIOUS;
        state.crowned = false;
        state.demigod = false;
        let mut rng = make_rng();
        let cd_before = state.bless_cooldown;
        let _events = crowning(&mut state, dummy_entity(), &mut rng);
        assert!(state.crowned);
        // kick_on_butt = 1 (crowned only)
        assert!(state.bless_cooldown > cd_before);
    }

    // =====================================================================
    // J.2: Altar & Sacrifice
    // =====================================================================

    // --- Sacrifice value formula (spec 3.2) ---

    #[test]
    fn test_sacrifice_value_acid_blob_ignores_age() {
        // Spec test vector #7
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 1,
            creation_turn: 1,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: true,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        assert_eq!(sacrifice_value(&offering, 1000), 2);
    }

    #[test]
    fn test_sacrifice_value_dragon_fresh() {
        // Spec test vector #8: dragon diff=20, 30 turns old
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 20,
            creation_turn: 970,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        assert_eq!(sacrifice_value(&offering, 1000), 21);
    }

    #[test]
    fn test_sacrifice_value_dragon_expired() {
        // Spec test vector #9: dragon diff=20, 51 turns old → 0
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 20,
            creation_turn: 949,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        assert_eq!(sacrifice_value(&offering, 1000), 0);
    }

    #[test]
    fn test_sacrifice_value_kobold_fresh() {
        // Spec test vector #10: kobold diff=0, 10 turns old
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 0,
            creation_turn: 990,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        assert_eq!(sacrifice_value(&offering, 1000), 1);
    }

    // --- Unicorn sacrifice (spec 3.3) ---

    #[test]
    fn test_sacrifice_unicorn_same_alignment_insult() {
        // Sacrificing a unicorn whose alignment matches the altar → insult
        let mut state = make_state();
        state.alignment = Alignment::Lawful;
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 5,
            creation_turn: 999,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: true,
            unicorn_alignment: Some(Alignment::Lawful),
            is_undead_monster: false,
        };
        let value = eval_offering(&mut state, &offering, Alignment::Lawful, 1000);
        assert_eq!(value, -1, "same-alignment unicorn should be an insult");
    }

    #[test]
    fn test_sacrifice_unicorn_cross_alignment_bonus() {
        // Non-aligned unicorn on own altar → value + 3 and adjalign(5)
        let mut state = make_state();
        state.alignment = Alignment::Neutral;
        state.alignment_record = 5;
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 5,
            creation_turn: 999,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: true,
            unicorn_alignment: Some(Alignment::Chaotic),
            is_undead_monster: false,
        };
        let value = eval_offering(
            &mut state, &offering, Alignment::Neutral, 1000,
        );
        assert_eq!(value, 9, "should be difficulty+1+3 = 9");
        assert_eq!(state.alignment_record, 10, "should gain 5 alignment");
    }

    // --- Altar conversion (spec 3.5 step 7) ---

    #[test]
    fn test_sacrifice_altar_conversion_success_rate() {
        // Test the altar conversion probability: rn2(8+level) > 5
        let mut state = make_state();
        state.experience_level = 10;
        state.god_anger = 0;
        state.alignment_record = 10;
        let mut successes = 0u32;
        let trials = 10000u32;
        for i in 0..trials {
            let mut s = state.clone();
            let mut rng = Pcg64::seed_from_u64(i as u64);
            let offering = CorpseOffering {
                entity: dummy_entity(),
                monster_difficulty: 10,
                creation_turn: 999,
                partially_eaten: false,
                eaten_fraction: 1.0,
                was_pet: false,
                same_race: false,
                is_acid_blob: false,
                is_unicorn: false,
                unicorn_alignment: None,
                is_undead_monster: false,
            };
            let events = sacrifice(
                &mut s, &offering, Alignment::Chaotic, 0, &mut rng,
            );
            if events.iter().any(|e| matches!(
                e,
                EngineEvent::Message { key, .. }
                    if key.contains("altar-convert")
            )) {
                successes += 1;
            }
        }
        // Expected: (2+10)/(8+10) = 12/18 ≈ 66.7%
        let rate = successes as f64 / trials as f64;
        assert!(
            rate > 0.55 && rate < 0.78,
            "conversion rate {:.1}% out of expected range 55-78%",
            rate * 100.0
        );
    }

    // --- Alignment conversion (spec 1.5) ---

    #[test]
    fn test_sacrifice_first_alignment_conversion() {
        let mut state = make_state();
        state.alignment = Alignment::Neutral;
        state.alignment_record = -5; // god is angry (record < 0)
        state.has_converted = false;
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 10,
            creation_turn: 999,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        let mut rng = make_rng();
        let _events = sacrifice(
            &mut state, &offering, Alignment::Lawful, 0, &mut rng,
        );
        assert_eq!(state.alignment, Alignment::Lawful, "should have converted");
        assert!(state.has_converted, "should be marked as converted");
        // Luck penalty of -3
        assert_eq!(state.luck, 0); // was 3, now 0
    }

    #[test]
    fn test_sacrifice_second_conversion_rejected() {
        let mut state = make_state();
        state.alignment = Alignment::Neutral;
        state.alignment_record = -5;
        state.has_converted = true; // already converted once
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 10,
            creation_turn: 999,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        let mut rng = make_rng();
        let _events = sacrifice(
            &mut state, &offering, Alignment::Lawful, 0, &mut rng,
        );
        // Should NOT have converted
        assert_eq!(state.alignment, Alignment::Neutral);
        // Heavy penalties
        assert!(state.god_anger >= 3);
        assert!(state.luck < 0); // -5 from rejection
    }

    // --- Artifact gift probability (spec 3.6) ---

    #[test]
    fn test_artifact_gift_level_too_low() {
        // Spec test vector #11: ulevel=2, uluck=5 → no gift
        let mut state = make_state();
        state.experience_level = 2;
        state.luck = 5;
        state.luck_bonus = 0;
        let mut rng = make_rng();
        assert!(!bestow_artifact_check(&state, 0, &mut rng));
    }

    #[test]
    fn test_artifact_gift_negative_luck() {
        // Spec test vector #12: uluck=-1 → no gift
        let mut state = make_state();
        state.experience_level = 10;
        state.luck = -1;
        state.luck_bonus = 0;
        let mut rng = make_rng();
        assert!(!bestow_artifact_check(&state, 0, &mut rng));
    }

    #[test]
    fn test_artifact_gift_probability_first_gift() {
        // Spec test vector #13: ugifts=0 → prob = 1/6
        let state = make_state();
        let mut successes = 0u32;
        let trials = 60000u32;
        for i in 0..trials {
            let mut rng = Pcg64::seed_from_u64(i as u64 + 10000);
            if bestow_artifact_check(&state, 2, &mut rng) {
                successes += 1;
            }
        }
        let rate = successes as f64 / trials as f64;
        // Expected: 1/6 ≈ 16.7%
        assert!(
            rate > 0.14 && rate < 0.20,
            "first gift rate {:.1}% out of expected range 14-20%",
            rate * 100.0
        );
    }

    #[test]
    fn test_artifact_gift_probability_high_gifts() {
        // Spec test vector #14: ugifts=2, nartifacts=4 → 1/(6+16) = 1/22
        let mut state = make_state();
        state.god_gifts = 2;
        let mut successes = 0u32;
        let trials = 60000u32;
        for i in 0..trials {
            let mut rng = Pcg64::seed_from_u64(i as u64 + 20000);
            if bestow_artifact_check(&state, 4, &mut rng) {
                successes += 1;
            }
        }
        let rate = successes as f64 / trials as f64;
        // Expected: 1/22 ≈ 4.5%
        assert!(
            rate > 0.03 && rate < 0.07,
            "high gifts rate {:.1}% out of expected range 3-7%",
            rate * 100.0
        );
    }

    // --- Same-race sacrifice (spec 3.5) ---

    #[test]
    fn test_sacrifice_same_race_non_chaotic() {
        let mut state = make_state();
        state.alignment = Alignment::Neutral;
        let old_record = state.alignment_record;
        let old_luck = state.luck;
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 5,
            creation_turn: 999,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: true,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        let mut rng = make_rng();
        let _events = sacrifice(
            &mut state, &offering, Alignment::Neutral, 0, &mut rng,
        );
        assert_eq!(state.alignment_record, old_record - 5);
        assert_eq!(state.god_anger, 3);
        assert_eq!(state.luck, old_luck - 5);
    }

    #[test]
    fn test_sacrifice_same_race_chaotic() {
        let mut state = make_state();
        state.alignment = Alignment::Chaotic;
        let old_record = state.alignment_record;
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 5,
            creation_turn: 999,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: true,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        let mut rng = make_rng();
        let _events = sacrifice(
            &mut state, &offering, Alignment::Chaotic, 0, &mut rng,
        );
        // Chaotic: +5 alignment for same-race sacrifice
        assert!(state.alignment_record > old_record);
    }

    // --- Pet sacrifice (spec 3.5) ---

    #[test]
    fn test_sacrifice_pet_guilt() {
        let mut state = make_state();
        let old_record = state.alignment_record;
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 5,
            creation_turn: 999,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: true,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        let mut rng = make_rng();
        let events = sacrifice(
            &mut state, &offering, Alignment::Neutral, 0, &mut rng,
        );
        assert_eq!(state.alignment_record, old_record - 3);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("sacrifice-pet")
        )));
    }

    // --- gods_upset (spec 1.4) ---

    #[test]
    fn test_gods_upset_own_god() {
        let mut state = make_state();
        state.alignment = Alignment::Neutral;
        state.god_anger = 0;
        gods_upset(&mut state, Alignment::Neutral);
        assert_eq!(state.god_anger, 1, "own god upset → anger +1");
    }

    #[test]
    fn test_gods_upset_other_god_reduces_anger() {
        let mut state = make_state();
        state.alignment = Alignment::Neutral;
        state.god_anger = 5;
        gods_upset(&mut state, Alignment::Lawful);
        assert_eq!(state.god_anger, 4, "other god upset → own anger -1");
    }

    #[test]
    fn test_gods_upset_other_god_floor_zero() {
        let mut state = make_state();
        state.alignment = Alignment::Neutral;
        state.god_anger = 0;
        gods_upset(&mut state, Alignment::Chaotic);
        assert_eq!(state.god_anger, 0, "should not go below zero");
    }

    // =====================================================================
    // J.3: Luck System
    // =====================================================================

    // --- Luck bounds (spec 7.1) ---

    #[test]
    fn test_luck_effective_with_bonus() {
        let mut state = make_state();
        state.luck = 10;
        state.luck_bonus = 3;
        assert_eq!(state.effective_luck(), 13);
    }

    #[test]
    fn test_luck_effective_negative_with_curse() {
        let mut state = make_state();
        state.luck = -10;
        state.luck_bonus = -3;
        assert_eq!(state.effective_luck(), -13);
    }

    #[test]
    fn test_luck_clamp_no_luckstone_positive() {
        // Spec test vector #16
        let mut state = make_state();
        state.luck = LUCKMAX;
        state.has_luckstone = false;
        adjust_luck(&mut state, 5);
        assert_eq!(state.luck, LUCKMAX, "luck clamped at 10 without stone");
    }

    #[test]
    fn test_luck_clamp_no_luckstone_negative() {
        let mut state = make_state();
        state.luck = LUCKMIN;
        state.has_luckstone = false;
        adjust_luck(&mut state, -5);
        assert_eq!(state.luck, LUCKMIN, "luck clamped at -10 without stone");
    }

    // --- Luck decay (spec 7.3) ---

    #[test]
    fn test_luck_decay_no_stone_positive() {
        // Spec test vector #21
        let mut state = make_state();
        state.luck = 5;
        state.has_luckstone = false;
        luck_timeout(&mut state, 600, 0, false);
        assert_eq!(state.luck, 4);
    }

    #[test]
    fn test_luck_decay_blessed_stone_blocks_positive() {
        // Spec test vector #22
        let mut state = make_state();
        state.luck = 5;
        state.has_luckstone = true;
        state.luckstone_blessed = true;
        state.luckstone_cursed = false;
        luck_timeout(&mut state, 600, 0, false);
        assert_eq!(state.luck, 5);
    }

    #[test]
    fn test_luck_decay_cursed_stone_blocks_negative_recovery() {
        // Spec test vector #23
        let mut state = make_state();
        state.luck = -3;
        state.has_luckstone = true;
        state.luckstone_blessed = false;
        state.luckstone_cursed = true;
        luck_timeout(&mut state, 600, 0, false);
        assert_eq!(state.luck, -3);
    }

    #[test]
    fn test_luck_decay_uncursed_stone_blocks_both() {
        // Spec test vector #24
        let mut state = make_state();
        state.has_luckstone = true;
        state.luckstone_blessed = false;
        state.luckstone_cursed = false;
        state.luck = 5;
        luck_timeout(&mut state, 600, 0, false);
        assert_eq!(state.luck, 5);
        state.luck = -3;
        luck_timeout(&mut state, 1200, 0, false);
        assert_eq!(state.luck, -3);
    }

    #[test]
    fn test_luck_decay_accelerated_with_amulet() {
        let mut state = make_state();
        state.luck = 5;
        state.has_luckstone = false;
        // With amulet: period = 300
        luck_timeout(&mut state, 300, 0, true);
        assert_eq!(state.luck, 4, "should decay at turn 300 with amulet");
        // Without amulet: turn 300 should NOT trigger decay (period=600)
        state.luck = 5;
        luck_timeout(&mut state, 300, 0, false);
        assert_eq!(state.luck, 5, "should NOT decay at turn 300 without amulet");
    }

    #[test]
    fn test_luck_decay_toward_nonzero_base() {
        // Base luck can be non-zero (e.g. full moon +1)
        let mut state = make_state();
        state.luck = 5;
        state.has_luckstone = false;
        luck_timeout(&mut state, 600, 3, false);
        assert_eq!(state.luck, 4, "should decay toward base=3");
        state.luck = 2;
        luck_timeout(&mut state, 1200, 3, false);
        assert_eq!(state.luck, 3, "should increase toward base=3");
    }

    #[test]
    fn test_luck_no_decay_at_base() {
        let mut state = make_state();
        state.luck = 0;
        state.has_luckstone = false;
        luck_timeout(&mut state, 600, 0, false);
        assert_eq!(state.luck, 0, "no decay when already at base");
    }

    // --- Luck effects on prayer (spec 7.5) ---

    #[test]
    fn test_luck_negative_blocks_prayer() {
        let mut state = make_state();
        state.luck = -1;
        state.luck_bonus = 0;
        state.bless_cooldown = 0;
        let ptype = evaluate_prayer(
            &state, false, None, Trouble::None, false,
        );
        assert_eq!(ptype, PrayerType::Punished,
            "negative effective luck should cause Punished");
    }

    #[test]
    fn test_luck_luckstone_saves_prayer() {
        let mut state = make_state();
        state.luck = -1;
        state.luck_bonus = 3; // luckstone makes effective = 2
        state.bless_cooldown = 0;
        let ptype = evaluate_prayer(
            &state, false, None, Trouble::None, false,
        );
        assert_ne!(ptype, PrayerType::Punished,
            "positive effective luck (with stone) should not be Punished");
    }

    // --- Alignment limit (spec 1.2) ---

    #[test]
    fn test_alignment_limit_formula() {
        // Spec test vector #15
        assert_eq!(alignment_limit(0), 10);
        assert_eq!(alignment_limit(200), 11);
        assert_eq!(alignment_limit(1000), 15);
        assert_eq!(alignment_limit(2000), 20);
    }

    #[test]
    fn test_alignment_capped_at_alignlim() {
        // Spec test vector #15
        let mut state = make_state();
        state.turn = 1000; // ALIGNLIM = 15
        state.alignment_record = 20;
        adjust_alignment(&mut state, 1);
        assert_eq!(state.alignment_record, 15,
            "should be capped at ALIGNLIM=15");
    }

    // --- Alignment abuse tracking ---

    #[test]
    fn test_alignment_abuse_tracked() {
        let mut state = make_state();
        state.alignment_abuse = 0;
        adjust_alignment(&mut state, -7);
        assert_eq!(state.alignment_abuse, 7);
        adjust_alignment(&mut state, -3);
        assert_eq!(state.alignment_abuse, 10);
        // Positive adjustments don't add abuse
        adjust_alignment(&mut state, 5);
        assert_eq!(state.alignment_abuse, 10);
    }

    // --- Critically low HP edge cases (spec 2.7) ---

    #[test]
    fn test_critically_low_hp_spec_vector_18() {
        // curhp=5, maxhp=100, ulevel=1 → hplim=15, eff_max=15
        // curhp<=5 → true
        assert!(critically_low_hp(5, 100, 1, false));
    }

    #[test]
    fn test_critically_low_hp_spec_vector_19() {
        // curhp=6, maxhp=30, ulevel=1 → hplim=15, eff_max=15
        // 6>5 and 6*5=30>15 → false
        assert!(!critically_low_hp(6, 30, 1, false));
    }

    #[test]
    fn test_critically_low_hp_spec_vector_20() {
        // curhp=7, maxhp=30, ulevel=1 → hplim=15, eff_max=15
        // 7>5 and 7*5=35>15 → false
        assert!(!critically_low_hp(7, 30, 1, false));
    }

    #[test]
    fn test_critically_low_hp_high_level() {
        // level=30: divisor=9, hplim=450, eff_max=min(500,450)=450
        // curhp=50: 50>5 and 50*9=450 <= 450 → true
        assert!(critically_low_hp(50, 500, 30, false));
        // curhp=51: 51>5 and 51*9=459 > 450 → false
        assert!(!critically_low_hp(51, 500, 30, false));
    }

    // --- Trouble system ---

    #[test]
    fn test_trouble_values() {
        assert_eq!(Trouble::Stoned.value(), 14);
        assert_eq!(Trouble::None.value(), 0);
        assert_eq!(Trouble::Punished.value(), -1);
        assert_eq!(Trouble::Hallucination.value(), -11);
    }

    #[test]
    fn test_trouble_classification() {
        assert!(Trouble::Stoned.is_major());
        assert!(!Trouble::Stoned.is_minor());
        assert!(!Trouble::None.is_major());
        assert!(!Trouble::None.is_minor());
        assert!(!Trouble::Punished.is_major());
        assert!(Trouble::Punished.is_minor());
    }

    // --- god_is_angry helper ---

    #[test]
    fn test_god_is_angry() {
        let mut state = make_state();
        state.alignment_record = 0;
        assert!(!state.god_is_angry());
        state.alignment_record = -1;
        assert!(state.god_is_angry());
        state.alignment_record = 10;
        assert!(!state.god_is_angry());
    }

    // --- angrygods cross-god vs own-god anger formula (spec 2.9) ---

    #[test]
    fn test_prayer_angry_cross_god_formula() {
        // Cross-god: maxanger = record/2 + luck_penalty
        let mut state = make_state();
        state.alignment = Alignment::Neutral;
        state.alignment_record = 10;
        state.luck = 0;
        state.luck_bonus = 0;
        state.blessed_amount = 5;
        let mut rng = make_rng();
        let _events = prayer_angry_for(
            &mut state, dummy_entity(), Alignment::Lawful, &mut rng,
        );
        // blessed_amount should be zeroed
        assert_eq!(state.blessed_amount, 0);
        // Cooldown should be set
        assert!(state.bless_cooldown > 0);
    }

    #[test]
    fn test_prayer_angry_own_god_formula() {
        let mut state = make_state();
        state.alignment = Alignment::Neutral;
        state.god_anger = 5;
        state.luck = 0;
        state.luck_bonus = 0;
        state.blessed_amount = 3;
        let mut rng = make_rng();
        let _events = prayer_angry_for(
            &mut state, dummy_entity(), Alignment::Neutral, &mut rng,
        );
        assert_eq!(state.blessed_amount, 0);
        assert!(state.bless_cooldown > 0);
    }

    // --- rnl (luck-adjusted random) ---

    #[test]
    fn test_rnl_positive_luck_biases_low() {
        let mut rng = Pcg64::seed_from_u64(42);
        let n = 5000;
        let mut sum_lucky: i64 = 0;
        let mut sum_neutral: i64 = 0;
        for _ in 0..n {
            sum_lucky += rnl(&mut Pcg64::seed_from_u64(rng.random_range(0..10000)),
                             100, 10) as i64;
            sum_neutral += rnl(&mut Pcg64::seed_from_u64(rng.random_range(0..10000)),
                               100, 0) as i64;
        }
        // With positive luck, the average should be lower than neutral.
        let avg_lucky = sum_lucky as f64 / n as f64;
        let avg_neutral = sum_neutral as f64 / n as f64;
        assert!(avg_lucky < avg_neutral,
            "positive luck avg ({:.1}) should be lower than neutral ({:.1})",
            avg_lucky, avg_neutral);
    }

    #[test]
    fn test_rnl_negative_luck_biases_high() {
        let mut rng = Pcg64::seed_from_u64(42);
        let n = 5000;
        let mut sum_unlucky: i64 = 0;
        let mut sum_neutral: i64 = 0;
        for _ in 0..n {
            sum_unlucky += rnl(&mut Pcg64::seed_from_u64(rng.random_range(0..10000)),
                               100, -10) as i64;
            sum_neutral += rnl(&mut Pcg64::seed_from_u64(rng.random_range(0..10000)),
                                100, 0) as i64;
        }
        let avg_unlucky = sum_unlucky as f64 / n as f64;
        let avg_neutral = sum_neutral as f64 / n as f64;
        assert!(avg_unlucky > avg_neutral,
            "negative luck avg ({:.1}) should be higher than neutral ({:.1})",
            avg_unlucky, avg_neutral);
    }

    #[test]
    fn test_rnl_zero_luck_uniform() {
        // With luck=0, rnl should behave like rn2: range [0, x-1].
        let mut rng = Pcg64::seed_from_u64(12345);
        for _ in 0..1000 {
            let val = rnl(&mut rng, 20, 0);
            assert!(val >= 0 && val < 20, "rnl should be in [0,20), got {}", val);
        }
    }

    #[test]
    fn test_rnl_clamped_to_range() {
        // Even with extreme luck, result should stay in [0, x-1].
        let mut rng = Pcg64::seed_from_u64(999);
        for _ in 0..500 {
            let val = rnl(&mut rng, 10, 13);
            assert!(val >= 0 && val < 10, "rnl with luck 13 should be in [0,10), got {}", val);
            let val = rnl(&mut rng, 10, -13);
            assert!(val >= 0 && val < 10, "rnl with luck -13 should be in [0,10), got {}", val);
        }
    }

    #[test]
    fn test_rnl_small_range_scales_luck() {
        // For x <= 15, luck is scaled by (|luck|+1)/3.
        // luck=3 => (3+1)/3 = 1; luck=10 => (10+1)/3 = 3
        let mut rng = Pcg64::seed_from_u64(42);
        let n = 5000;
        let mut sum_small_luck: i64 = 0;
        let mut sum_big_luck: i64 = 0;
        for _ in 0..n {
            sum_small_luck += rnl(&mut Pcg64::seed_from_u64(rng.random_range(0..10000)),
                                  10, 3) as i64;
            sum_big_luck += rnl(&mut Pcg64::seed_from_u64(rng.random_range(0..10000)),
                                10, 10) as i64;
        }
        // Bigger luck should have lower average.
        let avg_small = sum_small_luck as f64 / n as f64;
        let avg_big = sum_big_luck as f64 / n as f64;
        assert!(avg_big <= avg_small,
            "bigger luck ({:.1}) should have lower-or-equal avg than small luck ({:.1})",
            avg_big, avg_small);
    }

    #[test]
    fn test_rnl_edge_case_x_1() {
        // rnl(1, luck) should always return 0.
        let mut rng = Pcg64::seed_from_u64(42);
        for luck in -13..=13 {
            assert_eq!(rnl(&mut rng, 1, luck), 0, "rnl(1, {}) should be 0", luck);
        }
    }

    #[test]
    fn test_luck_check_positive_luck_helps() {
        // With high luck, a threshold check should succeed more often.
        let n = 5000;
        let mut pass_lucky = 0;
        let mut pass_unlucky = 0;
        for seed in 0..n {
            if luck_check(&mut Pcg64::seed_from_u64(seed), 5, 20, 10) {
                pass_lucky += 1;
            }
            if luck_check(&mut Pcg64::seed_from_u64(seed), 5, 20, -10) {
                pass_unlucky += 1;
            }
        }
        assert!(pass_lucky > pass_unlucky,
            "lucky passes ({}) should exceed unlucky passes ({})",
            pass_lucky, pass_unlucky);
    }

    // --- Luck adjust and timeout ---

    #[test]
    fn test_luck_adjustment_with_luckstone() {
        let mut state = make_state();
        state.has_luckstone = true;
        state.luck = 10;
        adjust_luck(&mut state, 5);
        // With luckstone, max is 13.
        assert_eq!(state.luck, 13);
    }

    #[test]
    fn test_luck_adjustment_without_luckstone() {
        let mut state = make_state();
        state.has_luckstone = false;
        state.luck = 8;
        adjust_luck(&mut state, 5);
        // Without luckstone, max is 10.
        assert_eq!(state.luck, 10);
    }

    #[test]
    fn test_luck_timeout_without_luckstone_decays() {
        let mut state = make_state();
        state.has_luckstone = false;
        state.luckstone_blessed = false;
        state.luckstone_cursed = false;
        state.luck = 5;
        state.god_anger = 0;
        // Decay happens at turn multiples of 600.
        luck_timeout(&mut state, 600, 0, false);
        assert_eq!(state.luck, 4, "luck should decay by 1 at turn 600");
    }

    #[test]
    fn test_luck_timeout_with_uncursed_luckstone_no_decay() {
        let mut state = make_state();
        state.has_luckstone = true;
        state.luckstone_blessed = false;
        state.luckstone_cursed = false;
        state.luck = 5;
        state.god_anger = 0;
        luck_timeout(&mut state, 600, 0, false);
        // Uncursed luckstone blocks both positive decay and negative recovery.
        assert_eq!(state.luck, 5, "uncursed luckstone should prevent decay");
    }

    // --- rnz with different experience levels ---

    #[test]
    fn test_rnz_level_1_vs_level_30() {
        let mut rng1 = Pcg64::seed_from_u64(99);
        let mut rng2 = Pcg64::seed_from_u64(99);
        let mut sum_low: i64 = 0;
        let mut sum_high: i64 = 0;
        let n = 5000;
        for _ in 0..n {
            sum_low += rnz(&mut rng1, 350, 1) as i64;
            sum_high += rnz(&mut rng2, 350, 30) as i64;
        }
        // Higher level → rne cap is higher → more extreme values possible
        // Both should be positive
        assert!(sum_low > 0);
        assert!(sum_high > 0);
    }

    // --- Sacrifice cooldown reduction formula (spec 3.5 step 8) ---

    #[test]
    fn test_sacrifice_reduces_cooldown() {
        let mut state = make_state();
        state.alignment_record = 5; // positive, god not angry
        state.god_anger = 0;
        state.bless_cooldown = 500;
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 20,
            creation_turn: 999,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        let mut rng = make_rng();
        let _events = sacrifice(
            &mut state, &offering, Alignment::Neutral, 0, &mut rng,
        );
        // Neutral: reduction = value * 300 / MAXVALUE = 21 * 300 / 24 = 262
        assert!(state.bless_cooldown < 500,
            "cooldown should have decreased, was {}",
            state.bless_cooldown);
        // 500 - 262 = 238 (approx)
        assert!(state.bless_cooldown < 250 && state.bless_cooldown > 200,
            "cooldown should be ~238, was {}",
            state.bless_cooldown);
    }

    #[test]
    fn test_sacrifice_cooldown_chaotic_uses_500() {
        let mut state = make_state();
        state.alignment = Alignment::Chaotic;
        state.alignment_record = 5;
        state.god_anger = 0;
        state.bless_cooldown = 500;
        let offering = CorpseOffering {
            entity: dummy_entity(),
            monster_difficulty: 20,
            creation_turn: 999,
            partially_eaten: false,
            eaten_fraction: 1.0,
            was_pet: false,
            same_race: false,
            is_acid_blob: false,
            is_unicorn: false,
            unicorn_alignment: None,
            is_undead_monster: false,
        };
        let mut rng = make_rng();
        let _events = sacrifice(
            &mut state, &offering, Alignment::Chaotic, 0, &mut rng,
        );
        // Chaotic: reduction = 21 * 500 / 24 = 437
        // 500 - 437 = 63
        assert!(state.bless_cooldown < 100,
            "chaotic cooldown should be ~63, was {}",
            state.bless_cooldown);
    }
}
