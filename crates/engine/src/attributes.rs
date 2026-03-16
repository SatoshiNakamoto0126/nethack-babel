//! Attribute exercise/abuse system for NetHack Babel.
//!
//! Implements the NetHack 3.7 attribute exercise mechanics from `attrib.c`.
//! Attributes (STR, DEX, CON, INT, WIS, CHA) accumulate exercise points
//! through gameplay actions and are periodically adjusted based on those
//! points.
//!
//! The system also handles attribute draining (monster attacks), restoration
//! (restore ability potion, prayer), and racial attribute caps.

use serde::{Deserialize, Serialize};

use crate::event::EngineEvent;
use crate::world::Attributes;

// ---------------------------------------------------------------------------
// Attribute index
// ---------------------------------------------------------------------------

/// Which of the six attributes is being referenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Attr {
    Str = 0,
    Dex = 1,
    Con = 2,
    Int = 3,
    Wis = 4,
    Cha = 5,
}

// ---------------------------------------------------------------------------
// Hero race (re-used from hunger, but self-contained here for attribute caps)
// ---------------------------------------------------------------------------

/// Player race for racial attribute caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Race {
    Human,
    Elf,
    Dwarf,
    Gnome,
    Orc,
}

/// Player role, for determining who qualifies for STR 18/xx.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FighterRole {
    /// Valkyrie, Barbarian, Knight, Caveperson, Samurai — can have 18/xx STR.
    Fighter,
    /// All other roles — STR capped at 18 with no exceptional sub-value.
    NonFighter,
}

// ---------------------------------------------------------------------------
// Exercise tracking component
// ---------------------------------------------------------------------------

/// Accumulated exercise points for each attribute.
///
/// Positive values indicate the player has been exercising that attribute
/// (e.g., fighting exercises STR, dodging exercises DEX).  Negative values
/// indicate abuse (e.g., carrying too much abuses DEX).
///
/// Every `EXERCISE_INTERVAL` turns, the engine calls `apply_exercise` to
/// convert accumulated points into actual attribute changes.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AttributeExercise {
    pub str_exercise: i32,
    pub dex_exercise: i32,
    pub con_exercise: i32,
    pub int_exercise: i32,
    pub wis_exercise: i32,
    pub cha_exercise: i32,
}

/// Natural (un-drained) attribute maximums.
///
/// Stored alongside `Attributes` to track what the player's attributes
/// *should* be when fully restored.  Drain attacks reduce current attributes
/// below these values; restore ability brings them back.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NaturalAttributes {
    pub strength: u8,
    pub strength_extra: u8,
    pub dexterity: u8,
    pub constitution: u8,
    pub intelligence: u8,
    pub wisdom: u8,
    pub charisma: u8,
}

impl Default for NaturalAttributes {
    fn default() -> Self {
        Self {
            strength: 10,
            strength_extra: 0,
            dexterity: 10,
            constitution: 10,
            intelligence: 10,
            wisdom: 10,
            charisma: 10,
        }
    }
}

impl From<&Attributes> for NaturalAttributes {
    fn from(a: &Attributes) -> Self {
        Self {
            strength: a.strength,
            strength_extra: a.strength_extra,
            dexterity: a.dexterity,
            constitution: a.constitution,
            intelligence: a.intelligence,
            wisdom: a.wisdom,
            charisma: a.charisma,
        }
    }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// How many turns between exercise checks.
pub const EXERCISE_INTERVAL: u32 = 800;

/// Minimum exercise points needed to trigger an attribute change.
/// In NetHack, the threshold is typically around 10-15 accumulated points.
pub const EXERCISE_THRESHOLD: i32 = 10;

/// Absolute minimum for any attribute (even when drained).
pub const ATTR_MIN: u8 = 3;

// ---------------------------------------------------------------------------
// Racial attribute caps
// ---------------------------------------------------------------------------

/// Racial attribute cap for a specific attribute.
///
/// Returns the maximum value the given race can achieve for `attr`.
/// For STR, the returned value is the cap on the base strength value;
/// the 18/xx sub-value has its own handling.
pub fn racial_cap(race: Race, attr: Attr) -> u8 {
    match (race, attr) {
        // Human: all 18
        (Race::Human, _) => 18,

        // Elf: STR 18, DEX 20, CON 16, INT 20, WIS 20, CHA 18
        (Race::Elf, Attr::Str) => 18,
        (Race::Elf, Attr::Dex) => 20,
        (Race::Elf, Attr::Con) => 16,
        (Race::Elf, Attr::Int) => 20,
        (Race::Elf, Attr::Wis) => 20,
        (Race::Elf, Attr::Cha) => 18,

        // Dwarf: STR 20, DEX 16, CON 20, INT 16, WIS 16, CHA 16
        (Race::Dwarf, Attr::Str) => 20,
        (Race::Dwarf, Attr::Dex) => 16,
        (Race::Dwarf, Attr::Con) => 20,
        (Race::Dwarf, Attr::Int) => 16,
        (Race::Dwarf, Attr::Wis) => 16,
        (Race::Dwarf, Attr::Cha) => 16,

        // Gnome: STR 18, DEX 18, CON 18, INT 19, WIS 18, CHA 18
        (Race::Gnome, Attr::Str) => 18,
        (Race::Gnome, Attr::Dex) => 18,
        (Race::Gnome, Attr::Con) => 18,
        (Race::Gnome, Attr::Int) => 19,
        (Race::Gnome, Attr::Wis) => 18,
        (Race::Gnome, Attr::Cha) => 18,

        // Orc: STR 20, DEX 16, CON 20, INT 16, WIS 16, CHA 16
        (Race::Orc, Attr::Str) => 20,
        (Race::Orc, Attr::Dex) => 16,
        (Race::Orc, Attr::Con) => 20,
        (Race::Orc, Attr::Int) => 16,
        (Race::Orc, Attr::Wis) => 16,
        (Race::Orc, Attr::Cha) => 16,
    }
}

// ---------------------------------------------------------------------------
// STR 18/xx formatting
// ---------------------------------------------------------------------------

/// Format a strength value for display.
///
/// Fighters (Valkyrie, Barbarian, Knight, Caveperson, Samurai) can have
/// exceptional strength 18/01 through 18/100.  When `strength_extra > 0`
/// and `strength == 18`, we display "18/xx" or "18/**" for 18/100.
///
/// Non-fighters and strengths other than 18 simply show the numeric value.
pub fn format_strength(strength: u8, strength_extra: u8) -> String {
    if strength == 18 && strength_extra > 0 {
        if strength_extra >= 100 {
            "18/**".to_string()
        } else {
            format!("18/{:02}", strength_extra)
        }
    } else {
        strength.to_string()
    }
}

// ---------------------------------------------------------------------------
// Exercise / abuse
// ---------------------------------------------------------------------------

/// Add exercise points to a specific attribute.
///
/// Called by various game systems when the player uses an attribute:
/// - STR: fighting in melee, forcing locks
/// - DEX: successful dodging, throwing, picking locks
/// - CON: recovering from sickness, regenerating HP
/// - INT: casting spells, reading spellbooks
/// - WIS: praying, successful divine intervention
/// - CHA: successful negotiations with shopkeepers
///
/// Negative `amount` indicates abuse.
pub fn exercise_attribute(exercise: &mut AttributeExercise, attr: Attr, amount: i32) {
    match attr {
        Attr::Str => exercise.str_exercise += amount,
        Attr::Dex => exercise.dex_exercise += amount,
        Attr::Con => exercise.con_exercise += amount,
        Attr::Int => exercise.int_exercise += amount,
        Attr::Wis => exercise.wis_exercise += amount,
        Attr::Cha => exercise.cha_exercise += amount,
    }
}

/// Get the exercise value for a specific attribute.
fn get_exercise(exercise: &AttributeExercise, attr: Attr) -> i32 {
    match attr {
        Attr::Str => exercise.str_exercise,
        Attr::Dex => exercise.dex_exercise,
        Attr::Con => exercise.con_exercise,
        Attr::Int => exercise.int_exercise,
        Attr::Wis => exercise.wis_exercise,
        Attr::Cha => exercise.cha_exercise,
    }
}

/// Reset the exercise counter for a specific attribute.
fn reset_exercise(exercise: &mut AttributeExercise, attr: Attr) {
    match attr {
        Attr::Str => exercise.str_exercise = 0,
        Attr::Dex => exercise.dex_exercise = 0,
        Attr::Con => exercise.con_exercise = 0,
        Attr::Int => exercise.int_exercise = 0,
        Attr::Wis => exercise.wis_exercise = 0,
        Attr::Cha => exercise.cha_exercise = 0,
    }
}

// ---------------------------------------------------------------------------
// Exercise actions
// ---------------------------------------------------------------------------

/// Actions that exercise (improve) or abuse (deteriorate) attributes.
///
/// Derived from callers of `exercise()` throughout the C NetHack source
/// (`attrib.c`, `dokick.c`, `mhitu.c`, `pray.c`, `potion.c`, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExerciseAction {
    // STR exercises
    /// Forcing a lock or door open.
    ForcedDoor,
    /// Picking up heavy objects.
    PickedUpHeavy,
    /// Successful melee hit.
    MeleeAttack,
    /// Throwing objects.
    Threw,

    // DEX exercises
    /// Successfully picking a lock.
    PickedLock,
    /// Disarming a trap.
    DisarmedTrap,
    /// Dodging an attack (good AC).
    DodgedAttack,
    /// Riding a steed.
    RidingSkill,

    // CON exercises
    /// Survived poisoning.
    SurvivedPoison,
    /// Ate food while hungry.
    AteFood,
    /// Moved while encumbered.
    ForcedMarch,

    // INT exercises
    /// Reading a spellbook.
    ReadBook,
    /// Successfully casting a spell.
    CastSpell,
    /// Identifying an item.
    IdentifiedItem,

    // WIS exercises
    /// Successful prayer.
    Prayed,
    /// Donated gold at a temple.
    DonatedToTemple,
    /// Turned undead.
    TurnedUndead,
    /// Detected a trap by searching.
    SensedTrap,

    // CHA exercises
    /// Taming a monster.
    TamedMonster,
    /// Buying or selling in a shop.
    ShopTransaction,
    /// Chatting with NPCs.
    Chatted,
}

impl ExerciseAction {
    /// Which attribute this action exercises.
    pub fn attribute(self) -> Attr {
        match self {
            ExerciseAction::ForcedDoor
            | ExerciseAction::PickedUpHeavy
            | ExerciseAction::MeleeAttack
            | ExerciseAction::Threw => Attr::Str,

            ExerciseAction::PickedLock
            | ExerciseAction::DisarmedTrap
            | ExerciseAction::DodgedAttack
            | ExerciseAction::RidingSkill => Attr::Dex,

            ExerciseAction::SurvivedPoison
            | ExerciseAction::AteFood
            | ExerciseAction::ForcedMarch => Attr::Con,

            ExerciseAction::ReadBook
            | ExerciseAction::CastSpell
            | ExerciseAction::IdentifiedItem => Attr::Int,

            ExerciseAction::Prayed
            | ExerciseAction::DonatedToTemple
            | ExerciseAction::TurnedUndead
            | ExerciseAction::SensedTrap => Attr::Wis,

            ExerciseAction::TamedMonster
            | ExerciseAction::ShopTransaction
            | ExerciseAction::Chatted => Attr::Cha,
        }
    }
}

/// Record an exercise action (+1 to the corresponding attribute's counter).
pub fn exercise_action(exercise: &mut AttributeExercise, action: ExerciseAction) {
    exercise_attribute(exercise, action.attribute(), 1);
}

// ---------------------------------------------------------------------------
// Attribute change messages
// ---------------------------------------------------------------------------

/// Message shown when an attribute increases through exercise.
pub fn gain_message(attr: Attr) -> &'static str {
    match attr {
        Attr::Str => "You feel stronger!",
        Attr::Dex => "You feel more agile!",
        Attr::Con => "You feel tougher!",
        Attr::Int => "You feel smarter!",
        Attr::Wis => "You feel wiser!",
        Attr::Cha => "You feel more attractive!",
    }
}

/// Message shown when an attribute decreases through abuse.
/// (For drain messages, see `drain_message`.)
pub fn loss_message(attr: Attr) -> &'static str {
    match attr {
        Attr::Str => "You feel weaker!",
        Attr::Dex => "You feel clumsy!",
        Attr::Con => "You feel fragile!",
        Attr::Int => "You feel stupid!",
        Attr::Wis => "You feel foolish!",
        Attr::Cha => "You feel ugly!",
    }
}

// ---------------------------------------------------------------------------
// Attribute helpers (continued)
// ---------------------------------------------------------------------------

/// Get the current value of a specific attribute.
fn get_attr(attrs: &Attributes, attr: Attr) -> u8 {
    match attr {
        Attr::Str => attrs.strength,
        Attr::Dex => attrs.dexterity,
        Attr::Con => attrs.constitution,
        Attr::Int => attrs.intelligence,
        Attr::Wis => attrs.wisdom,
        Attr::Cha => attrs.charisma,
    }
}

/// Set a specific attribute value.
fn set_attr(attrs: &mut Attributes, attr: Attr, val: u8) {
    match attr {
        Attr::Str => attrs.strength = val,
        Attr::Dex => attrs.dexterity = val,
        Attr::Con => attrs.constitution = val,
        Attr::Int => attrs.intelligence = val,
        Attr::Wis => attrs.wisdom = val,
        Attr::Cha => attrs.charisma = val,
    }
}

/// Get the natural (un-drained) value of a specific attribute.
fn get_natural(natural: &NaturalAttributes, attr: Attr) -> u8 {
    match attr {
        Attr::Str => natural.strength,
        Attr::Dex => natural.dexterity,
        Attr::Con => natural.constitution,
        Attr::Int => natural.intelligence,
        Attr::Wis => natural.wisdom,
        Attr::Cha => natural.charisma,
    }
}

/// Set the natural value of a specific attribute.
fn set_natural(natural: &mut NaturalAttributes, attr: Attr, val: u8) {
    match attr {
        Attr::Str => natural.strength = val,
        Attr::Dex => natural.dexterity = val,
        Attr::Con => natural.constitution = val,
        Attr::Int => natural.intelligence = val,
        Attr::Wis => natural.wisdom = val,
        Attr::Cha => natural.charisma = val,
    }
}

/// Apply accumulated exercise points to attributes.
///
/// Called every `EXERCISE_INTERVAL` turns.  For each attribute:
/// - If exercise >= `EXERCISE_THRESHOLD`: increase attribute by 1
///   (up to racial cap for the attribute).
/// - If exercise <= -`EXERCISE_THRESHOLD`: decrease attribute by 1
///   (down to `ATTR_MIN`).
/// - After processing, reset the exercise counter.
///
/// Returns a list of events describing what changed.
pub fn apply_exercise(
    attrs: &mut Attributes,
    natural: &mut NaturalAttributes,
    exercise: &mut AttributeExercise,
    race: Race,
    fighter_role: FighterRole,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    for &attr in &[
        Attr::Str,
        Attr::Dex,
        Attr::Con,
        Attr::Int,
        Attr::Wis,
        Attr::Cha,
    ] {
        let ex = get_exercise(exercise, attr);
        let current = get_attr(attrs, attr);
        let cap = racial_cap(race, attr);

        if ex >= EXERCISE_THRESHOLD {
            // Try to increase
            if current < cap {
                let new_val = current + 1;
                set_attr(attrs, attr, new_val);
                set_natural(natural, attr, new_val);
                events.push(EngineEvent::msg_with(
                    "attribute-increased",
                    vec![("attr", attr_name(attr).to_string())],
                ));
            } else if attr == Attr::Str && current == 18 && fighter_role == FighterRole::Fighter {
                // Increase STR 18/xx sub-value
                let new_extra = (attrs.strength_extra + 10).min(100);
                if new_extra > attrs.strength_extra {
                    attrs.strength_extra = new_extra;
                    natural.strength_extra = new_extra;
                    events.push(EngineEvent::msg_with(
                        "attribute-increased",
                        vec![("attr", "strength".to_string())],
                    ));
                }
            }
            reset_exercise(exercise, attr);
        } else if ex <= -EXERCISE_THRESHOLD {
            // Try to decrease
            if current > ATTR_MIN {
                let new_val = current - 1;
                set_attr(attrs, attr, new_val);
                set_natural(natural, attr, new_val);
                events.push(EngineEvent::msg_with(
                    "attribute-decreased",
                    vec![("attr", attr_name(attr).to_string())],
                ));
            }
            reset_exercise(exercise, attr);
        }
        // If exercise is between -threshold and +threshold, keep accumulating
    }

    events
}

// ---------------------------------------------------------------------------
// Drain / Restore
// ---------------------------------------------------------------------------

/// Drain an attribute by `amount` due to a monster drain attack.
///
/// Reduces the current attribute value (but NOT the natural max), so
/// `restore_attribute` can recover it later.  The attribute cannot go
/// below `ATTR_MIN`.
///
/// For STR, draining first reduces `strength_extra` (if any), then base
/// strength.
///
/// Returns events describing the drain.
pub fn drain_attribute(attrs: &mut Attributes, attr: Attr, amount: u8) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if attr == Attr::Str {
        // Drain exceptional strength first
        let mut remaining = amount;
        if attrs.strength_extra > 0 {
            if remaining as u16 * 10 <= attrs.strength_extra as u16 {
                attrs.strength_extra -= remaining * 10;
                remaining = 0;
            } else {
                remaining -= (attrs.strength_extra / 10).max(1);
                attrs.strength_extra = 0;
            }
        }
        if remaining > 0 {
            let new_val = attrs.strength.saturating_sub(remaining).max(ATTR_MIN);
            attrs.strength = new_val;
        }
    } else {
        let current = get_attr(attrs, attr);
        let new_val = current.saturating_sub(amount).max(ATTR_MIN);
        set_attr(attrs, attr, new_val);
    }

    events.push(EngineEvent::msg_with(
        "attribute-drained",
        vec![("attr", attr_name(attr).to_string())],
    ));

    events
}

/// Restore a drained attribute to its natural maximum.
///
/// Used by restore ability potion, prayer, etc.  If the current attribute
/// is already at or above the natural max, this is a no-op.
///
/// Returns events describing what was restored.
pub fn restore_attribute(
    attrs: &mut Attributes,
    natural: &NaturalAttributes,
    attr: Attr,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let current = get_attr(attrs, attr);
    let max = get_natural(natural, attr);

    let str_extra_drained = attr == Attr::Str && attrs.strength_extra < natural.strength_extra;

    if current < max || str_extra_drained {
        set_attr(attrs, attr, max);
        if attr == Attr::Str {
            attrs.strength_extra = natural.strength_extra;
        }
        events.push(EngineEvent::msg_with(
            "attribute-restored",
            vec![("attr", attr_name(attr).to_string())],
        ));
    }

    events
}

/// Restore all drained attributes to their natural maximums.
///
/// Returns events describing what was restored.
pub fn restore_all_attributes(
    attrs: &mut Attributes,
    natural: &NaturalAttributes,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    for &attr in &[
        Attr::Str,
        Attr::Dex,
        Attr::Con,
        Attr::Int,
        Attr::Wis,
        Attr::Cha,
    ] {
        events.extend(restore_attribute(attrs, natural, attr));
    }
    events
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Human-readable name for an attribute.
fn attr_name(attr: Attr) -> &'static str {
    match attr {
        Attr::Str => "strength",
        Attr::Dex => "dexterity",
        Attr::Con => "constitution",
        Attr::Int => "intelligence",
        Attr::Wis => "wisdom",
        Attr::Cha => "charisma",
    }
}

/// Drain message for each attribute (NetHack style).
pub fn drain_message(attr: Attr) -> &'static str {
    match attr {
        Attr::Str => "You feel weaker.",
        Attr::Dex => "You feel clumsy.",
        Attr::Con => "You feel fragile.",
        Attr::Int => "You feel stupid.",
        Attr::Wis => "You feel foolish.",
        Attr::Cha => "You feel ugly.",
    }
}

/// Check whether a turn number is an exercise checkpoint.
pub fn is_exercise_turn(turn: u32) -> bool {
    turn > 0 && turn.is_multiple_of(EXERCISE_INTERVAL)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create default attrs at STR 16.
    fn attrs_str16() -> Attributes {
        Attributes {
            strength: 16,
            strength_extra: 0,
            ..Attributes::default()
        }
    }

    // ------------------------------------------------------------------
    // 1. Exercise increases attribute
    // ------------------------------------------------------------------
    #[test]
    fn test_exercise_increases_attribute() {
        let mut attrs = attrs_str16();
        let mut natural = NaturalAttributes::from(&attrs);
        let mut exercise = AttributeExercise::default();

        // Accumulate enough exercise for STR
        exercise_attribute(&mut exercise, Attr::Str, EXERCISE_THRESHOLD);
        assert_eq!(exercise.str_exercise, EXERCISE_THRESHOLD);

        let events = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Human,
            FighterRole::NonFighter,
        );

        assert_eq!(attrs.strength, 17);
        assert_eq!(natural.strength, 17);
        assert!(!events.is_empty());
        // Exercise counter should be reset
        assert_eq!(exercise.str_exercise, 0);
    }

    // ------------------------------------------------------------------
    // 2. Abuse decreases attribute
    // ------------------------------------------------------------------
    #[test]
    fn test_abuse_decreases_attribute() {
        let mut attrs = attrs_str16();
        let mut natural = NaturalAttributes::from(&attrs);
        let mut exercise = AttributeExercise::default();

        exercise_attribute(&mut exercise, Attr::Str, -EXERCISE_THRESHOLD);

        let events = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Human,
            FighterRole::NonFighter,
        );

        assert_eq!(attrs.strength, 15);
        assert_eq!(natural.strength, 15);
        assert!(!events.is_empty());
    }

    // ------------------------------------------------------------------
    // 3. Racial cap enforced
    // ------------------------------------------------------------------
    #[test]
    fn test_racial_cap_enforced() {
        // Elf STR cap is 18; start at 18 — should not increase further
        let mut attrs = Attributes {
            strength: 18,
            strength_extra: 0,
            ..Attributes::default()
        };
        let mut natural = NaturalAttributes::from(&attrs);
        let mut exercise = AttributeExercise::default();

        exercise_attribute(&mut exercise, Attr::Str, EXERCISE_THRESHOLD);

        let events = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Elf,
            FighterRole::NonFighter,
        );

        // STR should remain at 18 for elf (non-fighter, no 18/xx)
        assert_eq!(attrs.strength, 18);
        assert_eq!(attrs.strength_extra, 0);
        // No attribute-increased event since nothing changed
        assert!(events.is_empty());
    }

    // ------------------------------------------------------------------
    // 3b. Racial cap allows higher for dwarves
    // ------------------------------------------------------------------
    #[test]
    fn test_racial_cap_dwarf_str() {
        // Dwarf STR cap is 20
        let mut attrs = Attributes {
            strength: 18,
            strength_extra: 0,
            ..Attributes::default()
        };
        let mut natural = NaturalAttributes::from(&attrs);
        let mut exercise = AttributeExercise::default();

        exercise_attribute(&mut exercise, Attr::Str, EXERCISE_THRESHOLD);

        let events = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Dwarf,
            FighterRole::NonFighter,
        );

        assert_eq!(attrs.strength, 19);
        assert!(!events.is_empty());
    }

    // ------------------------------------------------------------------
    // 4. Drain reduces attribute
    // ------------------------------------------------------------------
    #[test]
    fn test_drain_reduces_attribute() {
        let mut attrs = attrs_str16();

        let events = drain_attribute(&mut attrs, Attr::Str, 2);

        assert_eq!(attrs.strength, 14);
        assert!(!events.is_empty());
    }

    // ------------------------------------------------------------------
    // 4b. Drain respects minimum
    // ------------------------------------------------------------------
    #[test]
    fn test_drain_respects_minimum() {
        let mut attrs = Attributes {
            strength: 4,
            ..Attributes::default()
        };

        drain_attribute(&mut attrs, Attr::Str, 5);
        assert_eq!(attrs.strength, ATTR_MIN); // 3
    }

    // ------------------------------------------------------------------
    // 5. Restore recovers drained attribute
    // ------------------------------------------------------------------
    #[test]
    fn test_restore_recovers_drained() {
        let mut attrs = attrs_str16();
        let natural = NaturalAttributes::from(&attrs);

        // Drain it
        drain_attribute(&mut attrs, Attr::Str, 3);
        assert_eq!(attrs.strength, 13);

        // Restore it
        let events = restore_attribute(&mut attrs, &natural, Attr::Str);

        assert_eq!(attrs.strength, 16);
        assert!(!events.is_empty());
    }

    // ------------------------------------------------------------------
    // 6. STR 18/xx formatting
    // ------------------------------------------------------------------
    #[test]
    fn test_str_18_xx_formatting() {
        assert_eq!(format_strength(18, 0), "18");
        assert_eq!(format_strength(18, 1), "18/01");
        assert_eq!(format_strength(18, 50), "18/50");
        assert_eq!(format_strength(18, 99), "18/99");
        assert_eq!(format_strength(18, 100), "18/**");
        assert_eq!(format_strength(17, 0), "17");
        assert_eq!(format_strength(19, 0), "19");
        // Extra is ignored when STR != 18
        assert_eq!(format_strength(16, 50), "16");
    }

    // ------------------------------------------------------------------
    // 7. Exercise threshold — small amounts don't trigger change
    // ------------------------------------------------------------------
    #[test]
    fn test_exercise_threshold() {
        let mut attrs = attrs_str16();
        let mut natural = NaturalAttributes::from(&attrs);
        let mut exercise = AttributeExercise::default();

        // Exercise less than threshold
        exercise_attribute(&mut exercise, Attr::Str, EXERCISE_THRESHOLD - 1);

        let events = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Human,
            FighterRole::NonFighter,
        );

        // Should NOT change — below threshold
        assert_eq!(attrs.strength, 16);
        assert!(events.is_empty());
        // Exercise counter should NOT be reset (still accumulating)
        assert_eq!(exercise.str_exercise, EXERCISE_THRESHOLD - 1);
    }

    // ------------------------------------------------------------------
    // 8. Fighter 18/xx exercise
    // ------------------------------------------------------------------
    #[test]
    fn test_fighter_str_18_xx_exercise() {
        let mut attrs = Attributes {
            strength: 18,
            strength_extra: 50,
            ..Attributes::default()
        };
        let mut natural = NaturalAttributes::from(&attrs);
        let mut exercise = AttributeExercise::default();

        exercise_attribute(&mut exercise, Attr::Str, EXERCISE_THRESHOLD);

        let events = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Human,
            FighterRole::Fighter,
        );

        // 18/50 should go to 18/60
        assert_eq!(attrs.strength, 18);
        assert_eq!(attrs.strength_extra, 60);
        assert!(!events.is_empty());
    }

    // ------------------------------------------------------------------
    // 9. Fighter 18/xx caps at 18/100
    // ------------------------------------------------------------------
    #[test]
    fn test_fighter_str_18_100_cap() {
        let mut attrs = Attributes {
            strength: 18,
            strength_extra: 95,
            ..Attributes::default()
        };
        let mut natural = NaturalAttributes::from(&attrs);
        let mut exercise = AttributeExercise::default();

        exercise_attribute(&mut exercise, Attr::Str, EXERCISE_THRESHOLD);

        let events = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Human,
            FighterRole::Fighter,
        );

        assert_eq!(attrs.strength_extra, 100);
        assert!(!events.is_empty());

        // Try again — should be a no-op now
        exercise_attribute(&mut exercise, Attr::Str, EXERCISE_THRESHOLD);
        let events2 = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Human,
            FighterRole::Fighter,
        );
        assert_eq!(attrs.strength_extra, 100);
        assert!(events2.is_empty());
    }

    // ------------------------------------------------------------------
    // 10. Drain exceptional STR
    // ------------------------------------------------------------------
    #[test]
    fn test_drain_exceptional_strength() {
        let mut attrs = Attributes {
            strength: 18,
            strength_extra: 50,
            ..Attributes::default()
        };

        drain_attribute(&mut attrs, Attr::Str, 1);

        // Should reduce strength_extra first
        assert_eq!(attrs.strength, 18);
        assert_eq!(attrs.strength_extra, 40);
    }

    // ------------------------------------------------------------------
    // 11. Restore exceptional STR
    // ------------------------------------------------------------------
    #[test]
    fn test_restore_exceptional_strength() {
        let mut attrs = Attributes {
            strength: 18,
            strength_extra: 50,
            ..Attributes::default()
        };
        let natural = NaturalAttributes::from(&attrs);

        // Drain it
        drain_attribute(&mut attrs, Attr::Str, 2);
        assert_eq!(attrs.strength_extra, 30);

        // Restore
        let events = restore_attribute(&mut attrs, &natural, Attr::Str);
        assert_eq!(attrs.strength, 18);
        assert_eq!(attrs.strength_extra, 50);
        assert!(!events.is_empty());
    }

    // ------------------------------------------------------------------
    // 12. is_exercise_turn
    // ------------------------------------------------------------------
    #[test]
    fn test_is_exercise_turn() {
        assert!(!is_exercise_turn(0));
        assert!(!is_exercise_turn(1));
        assert!(!is_exercise_turn(799));
        assert!(is_exercise_turn(800));
        assert!(!is_exercise_turn(801));
        assert!(is_exercise_turn(1600));
    }

    // ------------------------------------------------------------------
    // 13. Abuse can't go below ATTR_MIN
    // ------------------------------------------------------------------
    #[test]
    fn test_abuse_minimum_floor() {
        let mut attrs = Attributes {
            strength: 4,
            ..Attributes::default()
        };
        let mut natural = NaturalAttributes::from(&attrs);
        let mut exercise = AttributeExercise::default();

        exercise_attribute(&mut exercise, Attr::Str, -EXERCISE_THRESHOLD);

        let events = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Human,
            FighterRole::NonFighter,
        );

        assert_eq!(attrs.strength, 3);
        assert!(!events.is_empty());

        // One more abuse — should stay at 3
        exercise_attribute(&mut exercise, Attr::Str, -EXERCISE_THRESHOLD);
        let events2 = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Human,
            FighterRole::NonFighter,
        );
        assert_eq!(attrs.strength, ATTR_MIN);
        assert!(events2.is_empty());
    }

    // ------------------------------------------------------------------
    // 14. Restore all attributes
    // ------------------------------------------------------------------
    #[test]
    fn test_restore_all_attributes() {
        let mut attrs = Attributes {
            strength: 16,
            dexterity: 14,
            constitution: 12,
            intelligence: 10,
            wisdom: 10,
            charisma: 10,
            ..Attributes::default()
        };
        let natural = NaturalAttributes::from(&attrs);

        // Drain STR and DEX
        drain_attribute(&mut attrs, Attr::Str, 2);
        drain_attribute(&mut attrs, Attr::Dex, 3);

        let events = restore_all_attributes(&mut attrs, &natural);

        assert_eq!(attrs.strength, 16);
        assert_eq!(attrs.dexterity, 14);
        assert_eq!(events.len(), 2); // STR and DEX restored
    }

    // ------------------------------------------------------------------
    // 15. Multiple attributes exercised simultaneously
    // ------------------------------------------------------------------
    #[test]
    fn test_multiple_attributes_exercise() {
        let mut attrs = Attributes::default(); // all 10
        let mut natural = NaturalAttributes::from(&attrs);
        let mut exercise = AttributeExercise::default();

        exercise_attribute(&mut exercise, Attr::Str, EXERCISE_THRESHOLD);
        exercise_attribute(&mut exercise, Attr::Dex, EXERCISE_THRESHOLD);
        exercise_attribute(&mut exercise, Attr::Int, -EXERCISE_THRESHOLD);

        let events = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Human,
            FighterRole::NonFighter,
        );

        assert_eq!(attrs.strength, 11);
        assert_eq!(attrs.dexterity, 11);
        assert_eq!(attrs.intelligence, 9);
        assert_eq!(events.len(), 3);
    }

    // ------------------------------------------------------------------
    // 16. ExerciseAction maps to correct attribute
    // ------------------------------------------------------------------
    #[test]
    fn test_exercise_action_str_mapping() {
        assert_eq!(ExerciseAction::ForcedDoor.attribute(), Attr::Str);
        assert_eq!(ExerciseAction::PickedUpHeavy.attribute(), Attr::Str);
        assert_eq!(ExerciseAction::MeleeAttack.attribute(), Attr::Str);
        assert_eq!(ExerciseAction::Threw.attribute(), Attr::Str);
    }

    #[test]
    fn test_exercise_action_dex_mapping() {
        assert_eq!(ExerciseAction::PickedLock.attribute(), Attr::Dex);
        assert_eq!(ExerciseAction::DisarmedTrap.attribute(), Attr::Dex);
        assert_eq!(ExerciseAction::DodgedAttack.attribute(), Attr::Dex);
        assert_eq!(ExerciseAction::RidingSkill.attribute(), Attr::Dex);
    }

    #[test]
    fn test_exercise_action_con_mapping() {
        assert_eq!(ExerciseAction::SurvivedPoison.attribute(), Attr::Con);
        assert_eq!(ExerciseAction::AteFood.attribute(), Attr::Con);
        assert_eq!(ExerciseAction::ForcedMarch.attribute(), Attr::Con);
    }

    #[test]
    fn test_exercise_action_int_mapping() {
        assert_eq!(ExerciseAction::ReadBook.attribute(), Attr::Int);
        assert_eq!(ExerciseAction::CastSpell.attribute(), Attr::Int);
        assert_eq!(ExerciseAction::IdentifiedItem.attribute(), Attr::Int);
    }

    #[test]
    fn test_exercise_action_wis_mapping() {
        assert_eq!(ExerciseAction::Prayed.attribute(), Attr::Wis);
        assert_eq!(ExerciseAction::DonatedToTemple.attribute(), Attr::Wis);
        assert_eq!(ExerciseAction::TurnedUndead.attribute(), Attr::Wis);
        assert_eq!(ExerciseAction::SensedTrap.attribute(), Attr::Wis);
    }

    #[test]
    fn test_exercise_action_cha_mapping() {
        assert_eq!(ExerciseAction::TamedMonster.attribute(), Attr::Cha);
        assert_eq!(ExerciseAction::ShopTransaction.attribute(), Attr::Cha);
        assert_eq!(ExerciseAction::Chatted.attribute(), Attr::Cha);
    }

    // ------------------------------------------------------------------
    // 17. exercise_action convenience function
    // ------------------------------------------------------------------
    #[test]
    fn test_exercise_action_adds_to_counter() {
        let mut exercise = AttributeExercise::default();

        exercise_action(&mut exercise, ExerciseAction::MeleeAttack);
        assert_eq!(exercise.str_exercise, 1);

        exercise_action(&mut exercise, ExerciseAction::MeleeAttack);
        assert_eq!(exercise.str_exercise, 2);

        exercise_action(&mut exercise, ExerciseAction::PickedLock);
        assert_eq!(exercise.dex_exercise, 1);
    }

    // ------------------------------------------------------------------
    // 18. Multiple exercise actions accumulate before periodic check
    // ------------------------------------------------------------------
    #[test]
    fn test_multiple_actions_accumulate() {
        let mut attrs = Attributes::default(); // all 10
        let mut natural = NaturalAttributes::from(&attrs);
        let mut exercise = AttributeExercise::default();

        // Exercise STR via 10 melee attacks to hit threshold
        for _ in 0..EXERCISE_THRESHOLD {
            exercise_action(&mut exercise, ExerciseAction::MeleeAttack);
        }
        assert_eq!(exercise.str_exercise, EXERCISE_THRESHOLD);

        let events = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Human,
            FighterRole::NonFighter,
        );

        assert_eq!(attrs.strength, 11);
        assert!(!events.is_empty());
        assert_eq!(exercise.str_exercise, 0);
    }

    // ------------------------------------------------------------------
    // 19. Gain messages
    // ------------------------------------------------------------------
    #[test]
    fn test_gain_messages() {
        assert_eq!(gain_message(Attr::Str), "You feel stronger!");
        assert_eq!(gain_message(Attr::Dex), "You feel more agile!");
        assert_eq!(gain_message(Attr::Con), "You feel tougher!");
        assert_eq!(gain_message(Attr::Int), "You feel smarter!");
        assert_eq!(gain_message(Attr::Wis), "You feel wiser!");
        assert_eq!(gain_message(Attr::Cha), "You feel more attractive!");
    }

    // ------------------------------------------------------------------
    // 20. Loss messages
    // ------------------------------------------------------------------
    #[test]
    fn test_loss_messages() {
        assert_eq!(loss_message(Attr::Str), "You feel weaker!");
        assert_eq!(loss_message(Attr::Dex), "You feel clumsy!");
        assert_eq!(loss_message(Attr::Con), "You feel fragile!");
        assert_eq!(loss_message(Attr::Int), "You feel stupid!");
        assert_eq!(loss_message(Attr::Wis), "You feel foolish!");
        assert_eq!(loss_message(Attr::Cha), "You feel ugly!");
    }

    // ------------------------------------------------------------------
    // 21. Racial caps for all races
    // ------------------------------------------------------------------
    #[test]
    fn test_racial_caps_all_races() {
        // Human: all 18
        for attr in &[
            Attr::Str,
            Attr::Dex,
            Attr::Con,
            Attr::Int,
            Attr::Wis,
            Attr::Cha,
        ] {
            assert_eq!(racial_cap(Race::Human, *attr), 18);
        }

        // Elf highlights
        assert_eq!(racial_cap(Race::Elf, Attr::Dex), 20);
        assert_eq!(racial_cap(Race::Elf, Attr::Con), 16);
        assert_eq!(racial_cap(Race::Elf, Attr::Int), 20);

        // Dwarf highlights
        assert_eq!(racial_cap(Race::Dwarf, Attr::Str), 20);
        assert_eq!(racial_cap(Race::Dwarf, Attr::Con), 20);
        assert_eq!(racial_cap(Race::Dwarf, Attr::Cha), 16);

        // Gnome
        assert_eq!(racial_cap(Race::Gnome, Attr::Int), 19);

        // Orc
        assert_eq!(racial_cap(Race::Orc, Attr::Str), 20);
        assert_eq!(racial_cap(Race::Orc, Attr::Int), 16);
        assert_eq!(racial_cap(Race::Orc, Attr::Cha), 16);
    }

    // ------------------------------------------------------------------
    // 22. Attribute adjustment clamping
    // ------------------------------------------------------------------
    #[test]
    fn test_attribute_adjustment_clamping() {
        // Can't exceed racial cap via exercise
        let mut attrs = Attributes {
            dexterity: 20,
            ..Attributes::default()
        };
        let mut natural = NaturalAttributes::from(&attrs);
        let mut exercise = AttributeExercise::default();

        exercise_attribute(&mut exercise, Attr::Dex, EXERCISE_THRESHOLD);

        let events = apply_exercise(
            &mut attrs,
            &mut natural,
            &mut exercise,
            Race::Elf, // Elf DEX cap = 20
            FighterRole::NonFighter,
        );

        // Already at cap, no change
        assert_eq!(attrs.dexterity, 20);
        assert!(events.is_empty());
    }
}
