//! Engraving system: writing on the dungeon floor + Elbereth mechanics.
//!
//! Players can engrave text on the floor using various methods (dust,
//! blade, fire, lightning, digging).  The most important use is writing
//! "Elbereth" to scare non-immune monsters away.
//!
//! Engravings are stored per-position in the `EngravingMap` on the
//! dungeon level.  Each turn, non-permanent engravings may degrade
//! when stepped on.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::action::Position;
use crate::conduct::{ConductAction, ConductState};
use crate::event::EngineEvent;

// ---------------------------------------------------------------------------
// Engraving data types
// ---------------------------------------------------------------------------

/// Method used to create an engraving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngraveMethod {
    /// Written in dust with a finger — very fragile.
    Dust,
    /// Scratched with a blade or hard gem — moderate durability.
    Blade,
    /// Burned by wand of fire — permanent.
    Fire,
    /// Burned by wand of lightning — permanent.
    Lightning,
    /// Dug with a pick-axe or mattock — permanent.
    Dig,
}

impl EngraveMethod {
    /// Initial durability for an engraving of this type.
    ///
    /// Dust: 1 step destroys it.
    /// Blade: ~5 steps per character written.
    /// Fire/Lightning/Dig: permanent (u32::MAX).
    pub fn base_durability(self) -> u32 {
        match self {
            EngraveMethod::Dust => 1,
            EngraveMethod::Blade => 5,
            EngraveMethod::Fire | EngraveMethod::Lightning | EngraveMethod::Dig => u32::MAX,
        }
    }

    /// Number of turns each character takes to engrave.
    pub fn turns_per_char(self) -> u32 {
        match self {
            EngraveMethod::Dust => 1,
            EngraveMethod::Blade => 2,
            EngraveMethod::Fire | EngraveMethod::Lightning | EngraveMethod::Dig => 0,
        }
    }

    /// Whether the engraving is permanent (never degrades from stepping).
    pub fn is_permanent(self) -> bool {
        self.base_durability() == u32::MAX
    }

    /// Convert to the conduct-system `EngravingMethod` (from `conduct.rs`).
    pub fn to_conduct_method(self) -> crate::conduct::EngravingMethod {
        match self {
            EngraveMethod::Dust => crate::conduct::EngravingMethod::Dust,
            EngraveMethod::Blade => crate::conduct::EngravingMethod::Engrave,
            EngraveMethod::Fire | EngraveMethod::Lightning | EngraveMethod::Dig => {
                crate::conduct::EngravingMethod::Burn
            }
        }
    }
}

/// A single engraving on the dungeon floor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Engraving {
    /// The text written on the floor.
    pub text: String,
    /// How the engraving was created.
    pub method: EngraveMethod,
    /// Remaining durability — decremented when stepped on.
    /// Permanent engravings use `u32::MAX`.
    pub durability: u32,
    /// Map position of this engraving.
    pub position: Position,
}

impl Engraving {
    /// Create a new engraving.
    pub fn new(text: String, method: EngraveMethod, position: Position) -> Self {
        Self {
            durability: method.base_durability(),
            text,
            method,
            position,
        }
    }

    /// Whether this engraving is still readable (durability > 0).
    pub fn is_readable(&self) -> bool {
        self.durability > 0
    }

    /// Whether this engraving contains the word "Elbereth" (case-sensitive,
    /// matching NetHack's behavior).
    pub fn has_elbereth(&self) -> bool {
        self.text.contains("Elbereth")
    }

    /// Whether this engraving currently repels monsters.
    pub fn repels_monsters(&self) -> bool {
        self.is_readable() && self.has_elbereth()
    }

    /// Degrade the engraving by one step (e.g., someone stepped on it).
    ///
    /// Returns `true` if the engraving is still readable after degradation.
    pub fn step_on(&mut self) -> bool {
        if self.method.is_permanent() {
            return true;
        }
        self.durability = self.durability.saturating_sub(1);
        self.is_readable()
    }
}

// ---------------------------------------------------------------------------
// Engraving map (per-level storage)
// ---------------------------------------------------------------------------

/// Collection of engravings on a single dungeon level, keyed by position.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngravingMap {
    engravings: HashMap<Position, Engraving>,
}

impl EngravingMap {
    /// Create an empty engraving map.
    pub fn new() -> Self {
        Self {
            engravings: HashMap::new(),
        }
    }

    /// Get the engraving at a position, if any.
    pub fn get(&self, pos: Position) -> Option<&Engraving> {
        self.engravings.get(&pos)
    }

    /// Get a mutable reference to the engraving at a position.
    pub fn get_mut(&mut self, pos: Position) -> Option<&mut Engraving> {
        self.engravings.get_mut(&pos)
    }

    /// Insert or replace an engraving at a position.
    pub fn insert(&mut self, engraving: Engraving) {
        self.engravings.insert(engraving.position, engraving);
    }

    /// Remove an engraving at a position (e.g., fully degraded).
    pub fn remove(&mut self, pos: Position) -> Option<Engraving> {
        self.engravings.remove(&pos)
    }

    /// Iterate over all engravings.
    pub fn iter(&self) -> impl Iterator<Item = (&Position, &Engraving)> {
        self.engravings.iter()
    }

    /// Degrade the engraving at a position (if any) due to being stepped on.
    /// Removes the engraving if its durability reaches zero.
    ///
    /// Returns `true` if there was an engraving and it is still readable.
    pub fn step_on(&mut self, pos: Position) -> bool {
        if let Some(eng) = self.engravings.get_mut(&pos) {
            let still_readable = eng.step_on();
            if !still_readable {
                self.engravings.remove(&pos);
            }
            still_readable
        } else {
            false
        }
    }

    /// Whether there is an effective Elbereth engraving at the position.
    pub fn is_elbereth_at(&self, pos: Position) -> bool {
        self.engravings
            .get(&pos)
            .map(|e| e.repels_monsters())
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Public API: engrave text at a position
// ---------------------------------------------------------------------------

/// Write an engraving at the given position.
///
/// If there is already an engraving at the position, it is replaced.
/// Writing "Elbereth" increments the conduct counter and breaks the
/// Elberethless conduct.
///
/// Returns the events generated and the number of turns the engraving
/// takes (so the caller can deduct movement points or occupy the player).
pub fn engrave(
    engraving_map: &mut EngravingMap,
    conduct: &mut ConductState,
    pos: Position,
    text: &str,
    method: EngraveMethod,
) -> (Vec<EngineEvent>, u32) {
    let mut events = Vec::new();

    // Calculate how many turns this takes.
    let turns = text.len() as u32 * method.turns_per_char();

    // Create the engraving.
    let engraving = Engraving::new(text.to_string(), method, pos);

    // Track Elbereth in conduct system.
    if engraving.has_elbereth() {
        let violations = crate::conduct::check_conduct(
            conduct,
            &ConductAction::WriteElbereth,
        );
        for v in &violations {
            events.push(EngineEvent::msg_with(
                "conduct-broken",
                vec![("conduct", v.conduct.name().to_string())],
            ));
        }
    }

    // Insert (replacing any existing engraving at this position).
    engraving_map.insert(engraving);

    events.push(EngineEvent::msg_with(
        "engrave-write",
        vec![
            ("text", text.to_string()),
            ("method", format!("{:?}", method)),
        ],
    ));

    (events, turns)
}

/// Degrade engravings at a position when stepped on.
///
/// Called when any entity moves onto a tile that may have an engraving.
pub fn degrade_engravings(engraving_map: &mut EngravingMap, pos: Position) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    if let Some(eng) = engraving_map.get(pos) {
        if eng.method.is_permanent() {
            return events;
        }
        let text = eng.text.clone();
        let still_readable = engraving_map.step_on(pos);
        if !still_readable {
            events.push(EngineEvent::msg_with(
                "engrave-smudged",
                vec![("text", text)],
            ));
        }
    }
    events
}

/// Check if there is an effective Elbereth engraving at a position.
pub fn is_elbereth_at(engraving_map: &EngravingMap, pos: Position) -> bool {
    engraving_map.is_elbereth_at(pos)
}

/// Get the engraving at a position, if any.
pub fn get_engraving_at(engraving_map: &EngravingMap, pos: Position) -> Option<&Engraving> {
    engraving_map.get(pos)
}

// ---------------------------------------------------------------------------
// Text wipeout (wipeout_text from engrave.c)
// ---------------------------------------------------------------------------

/// Wipe out characters from engraving text, simulating degradation.
///
/// Replaces `count` random characters with spaces.  Used when engravings
/// degrade from foot traffic, water, or monster stepping.
pub fn wipeout_text(text: &str, count: u32, rng: &mut impl rand::Rng) -> String {
    if text.is_empty() || count == 0 {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    let non_space: Vec<usize> = chars
        .iter()
        .enumerate()
        .filter(|(_, c)| **c != ' ')
        .map(|(i, _)| i)
        .collect();

    let wipe_count = (count as usize).min(non_space.len());
    // Fisher-Yates partial shuffle to pick which chars to wipe.
    let mut indices = non_space;
    for i in 0..wipe_count {
        let j = rng.random_range(i..indices.len());
        indices.swap(i, j);
    }
    for &idx in &indices[..wipe_count] {
        chars[idx] = ' ';
    }

    chars.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Random engraving generation (random_engraving from engrave.c)
// ---------------------------------------------------------------------------

/// Stock random engravings found on dungeon floors.
const RANDOM_ENGRAVINGS: &[&str] = &[
    "ad ae um",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
    "ad aerarium",
    "They say that no dwarf is born alive without a beard.",
    "Strstransen Tilansen",
    "Langstransen Tilansen",
    "closed for strstransen",
    "You steal, we zap.",
    "Langstransen Tilansen",
    "Langstransen Tilansen",
];

/// Pick a random stock engraving.
pub fn random_engraving(rng: &mut impl rand::Rng) -> &'static str {
    let idx = rng.random_range(0..RANDOM_ENGRAVINGS.len());
    RANDOM_ENGRAVINGS[idx]
}

// ---------------------------------------------------------------------------
// Headstone / epitaph engraving (for graves)
// ---------------------------------------------------------------------------

/// Stock epitaphs for headstones.
const EPITAPHS: &[&str] = &[
    "Rest in peace",
    "R.I.P.",
    "Gone but not forgotten",
    "Here lies a very model adventurer",
    "Here lies the finest warrior who ever drew sword",
    "At last... peace",
    "A valiant warrior",
    "Stranded... shipwrecked...",
    "Langstransen Tilansen... rest ye",
    "Langstransen Tilansen",
    "He always said it would end like this",
    "She always said it would end like this",
    "I told you I was sick!",
    "This looks interesting...",
];

/// Pick a random headstone epitaph.
pub fn random_epitaph(rng: &mut impl rand::Rng) -> &'static str {
    let idx = rng.random_range(0..EPITAPHS.len());
    EPITAPHS[idx]
}

// ---------------------------------------------------------------------------
// Engraving tool classification
// ---------------------------------------------------------------------------

/// What kind of engraving a tool produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngraveTool {
    /// Fingers: produces dust engraving.
    Fingers,
    /// Edged weapon or hard gem: produces blade (scratched) engraving.
    Blade,
    /// Wand of fire.
    WandFire,
    /// Wand of lightning.
    WandLightning,
    /// Wand of digging (engraves by digging into floor).
    WandDigging,
    /// Athame (special: writes instantly like wands).
    Athame,
    /// Unsuitable tool.
    Unsuitable,
}

/// Map a tool to its engraving method.
pub fn tool_to_engrave_method(tool: EngraveTool) -> Option<EngraveMethod> {
    match tool {
        EngraveTool::Fingers => Some(EngraveMethod::Dust),
        EngraveTool::Blade => Some(EngraveMethod::Blade),
        EngraveTool::WandFire => Some(EngraveMethod::Fire),
        EngraveTool::WandLightning => Some(EngraveMethod::Lightning),
        EngraveTool::WandDigging | EngraveTool::Athame => Some(EngraveMethod::Dig),
        EngraveTool::Unsuitable => None,
    }
}

/// Whether the tool writes instantly (wands and athame).
pub fn is_instant_engrave(tool: EngraveTool) -> bool {
    matches!(
        tool,
        EngraveTool::WandFire
            | EngraveTool::WandLightning
            | EngraveTool::WandDigging
            | EngraveTool::Athame
    )
}

// ---------------------------------------------------------------------------
// Wand-specific engraving side effects
// ---------------------------------------------------------------------------

/// Side effects when engraving with a wand (based on engrave.c doengrave_sfx_item_WAN).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WandEngraveEffect {
    /// Wand of fire: burns the engraving, permanent.
    Burns,
    /// Wand of lightning: electrocutes, permanent. Also blinds if not resistant.
    Electrocutes { blinds: bool },
    /// Wand of digging: carves into floor, permanent.
    Carves,
    /// Wand of polymorph: changes existing engraving text randomly.
    Polymorphs,
    /// Wand of make invisible: makes the engraving invisible.
    MakesInvisible,
    /// Wand of cancellation / teleportation: removes existing engraving.
    Removes,
    /// Wand of cold: freezes the dust (no real effect, flavor only).
    Freezes,
    /// Unrecognized wand: just dust-writes using wand as stick.
    NoEffect,
}

/// Determine the side effect of engraving with a specific wand type.
pub fn wand_engrave_effect(wand_type: &str, player_blind_resistant: bool) -> WandEngraveEffect {
    match wand_type {
        "fire" => WandEngraveEffect::Burns,
        "lightning" => WandEngraveEffect::Electrocutes {
            blinds: !player_blind_resistant,
        },
        "digging" => WandEngraveEffect::Carves,
        "polymorph" => WandEngraveEffect::Polymorphs,
        "make invisible" => WandEngraveEffect::MakesInvisible,
        "cancellation" | "teleportation" => WandEngraveEffect::Removes,
        "cold" => WandEngraveEffect::Freezes,
        _ => WandEngraveEffect::NoEffect,
    }
}

// ---------------------------------------------------------------------------
// Reachability check
// ---------------------------------------------------------------------------

/// Whether the player can reach the floor to engrave.
///
/// Can't engrave when: levitating (without Lev_at_will), swallowed,
/// stuck in a pit and can't reach, riding without dismounting.
pub fn can_reach_floor(
    is_levitating: bool,
    has_lev_at_will: bool,
    is_swallowed: bool,
    is_in_pit: bool,
) -> bool {
    if is_swallowed {
        return false;
    }
    if is_levitating && !has_lev_at_will {
        return false;
    }
    if is_in_pit {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Count engravings
// ---------------------------------------------------------------------------

/// Count the number of readable engravings on the level.
pub fn count_engravings(map: &EngravingMap) -> usize {
    map.iter().filter(|(_, e)| e.is_readable()).count()
}

// ---------------------------------------------------------------------------
// Elbereth immunity check
// ---------------------------------------------------------------------------

/// Whether a monster is immune to Elbereth's scare effect.
///
/// In NetHack, the following are immune:
/// - @-class (player, shopkeeper): identified by being human-like with MERC
///   or being the player
/// - Wizard of Yendor (covetous + unique)
/// - Riders (astral plane bosses)
/// - Blind monsters (cannot read)
/// - Mindless monsters
///
/// We approximate this with flags: a monster is immune if it is covetous
/// (Wizard, Riders), or mindless, or a tame pet (won't scare your own pet).
pub fn is_elbereth_immune(
    flags: nethack_babel_data::MonsterFlags,
    is_blind: bool,
    is_covetous: bool,
    is_peaceful: bool,
) -> bool {
    use nethack_babel_data::MonsterFlags;

    // Covetous monsters (Wizard of Yendor, Riders) ignore Elbereth.
    if is_covetous {
        return true;
    }

    // Blind monsters cannot read the engraving.
    if is_blind {
        return true;
    }

    // Mindless monsters (golems, etc.) don't comprehend Elbereth.
    if flags.contains(MonsterFlags::MINDLESS) {
        return true;
    }

    // Peaceful monsters aren't attacking anyway, so immunity is moot,
    // but in NetHack they are also not scared by Elbereth.
    if is_peaceful {
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conduct::ConductState;

    fn make_pos(x: i32, y: i32) -> Position {
        Position::new(x, y)
    }

    // ── test_engrave_dust_degrades ──────────────────────────────

    #[test]
    fn test_engrave_dust_degrades() {
        let mut map = EngravingMap::new();
        let pos = make_pos(5, 5);
        map.insert(Engraving::new(
            "hello".to_string(),
            EngraveMethod::Dust,
            pos,
        ));

        assert!(map.get(pos).unwrap().is_readable());

        // One step destroys dust engraving (durability = 1).
        let still = map.step_on(pos);
        assert!(!still, "dust engraving should be gone after one step");
        assert!(
            map.get(pos).is_none(),
            "fully degraded engraving should be removed"
        );
    }

    // ── test_engrave_fire_permanent ─────────────────────────────

    #[test]
    fn test_engrave_fire_permanent() {
        let mut map = EngravingMap::new();
        let pos = make_pos(3, 3);
        map.insert(Engraving::new(
            "Elbereth".to_string(),
            EngraveMethod::Fire,
            pos,
        ));

        // 100 steps should not degrade a fire engraving.
        for _ in 0..100 {
            map.step_on(pos);
        }
        assert!(
            map.get(pos).unwrap().is_readable(),
            "fire engraving should be permanent"
        );
    }

    // ── test_elbereth_scares_monster ─────────────────────────────

    #[test]
    fn test_elbereth_scares_monster() {
        let map = {
            let mut m = EngravingMap::new();
            let pos = make_pos(8, 8);
            m.insert(Engraving::new(
                "Elbereth".to_string(),
                EngraveMethod::Dust,
                pos,
            ));
            m
        };

        assert!(
            is_elbereth_at(&map, make_pos(8, 8)),
            "Elbereth should be detected at the engraved position"
        );

        // A non-immune monster should be scared.
        let flags = nethack_babel_data::MonsterFlags::empty();
        assert!(
            !is_elbereth_immune(flags, false, false, false),
            "normal monster should not be immune to Elbereth"
        );
    }

    // ── test_elbereth_immune_monster ─────────────────────────────

    #[test]
    fn test_elbereth_immune_monster() {
        // Covetous (Wizard of Yendor / Riders) ignores Elbereth.
        let flags = nethack_babel_data::MonsterFlags::COVETOUS;
        assert!(
            is_elbereth_immune(flags, false, true, false),
            "covetous monster should be immune to Elbereth"
        );

        // Blind monster ignores Elbereth.
        let flags = nethack_babel_data::MonsterFlags::empty();
        assert!(
            is_elbereth_immune(flags, true, false, false),
            "blind monster should be immune to Elbereth"
        );

        // Mindless monster ignores Elbereth.
        let flags = nethack_babel_data::MonsterFlags::MINDLESS;
        assert!(
            is_elbereth_immune(flags, false, false, false),
            "mindless monster should be immune to Elbereth"
        );
    }

    // ── test_elbereth_dust_degrades_fast ─────────────────────────

    #[test]
    fn test_elbereth_dust_degrades_fast() {
        let mut map = EngravingMap::new();
        let pos = make_pos(5, 5);
        map.insert(Engraving::new(
            "Elbereth".to_string(),
            EngraveMethod::Dust,
            pos,
        ));

        assert!(map.is_elbereth_at(pos));

        // Dust: 1 step destroys.
        map.step_on(pos);
        assert!(
            !map.is_elbereth_at(pos),
            "dust Elbereth should be gone after 1 step"
        );
    }

    // ── test_elbereth_blade_lasts_longer ─────────────────────────

    #[test]
    fn test_elbereth_blade_lasts_longer() {
        let mut map = EngravingMap::new();
        let pos = make_pos(5, 5);
        map.insert(Engraving::new(
            "Elbereth".to_string(),
            EngraveMethod::Blade,
            pos,
        ));

        assert!(map.is_elbereth_at(pos));

        // Blade: survives 4 steps, dies on 5th.
        for i in 0..4 {
            map.step_on(pos);
            assert!(
                map.is_elbereth_at(pos),
                "blade Elbereth should survive step {}",
                i + 1
            );
        }
        map.step_on(pos);
        assert!(
            !map.is_elbereth_at(pos),
            "blade Elbereth should be gone after 5 steps"
        );
    }

    // ── test_engrave_conduct_tracked ─────────────────────────────

    #[test]
    fn test_engrave_conduct_tracked() {
        let mut map = EngravingMap::new();
        let mut conduct = ConductState::new();
        let pos = make_pos(5, 5);

        assert_eq!(
            conduct.elbereths, 0,
            "elbereth count should start at 0"
        );

        let (events, _turns) = engrave(
            &mut map,
            &mut conduct,
            pos,
            "Elbereth",
            EngraveMethod::Dust,
        );

        assert_eq!(
            conduct.elbereths, 1,
            "writing Elbereth should increment conduct counter"
        );
        // Should produce at least a conduct-broken message and an
        // engrave-write message.
        assert!(
            events.len() >= 2,
            "should emit conduct violation and engrave events"
        );
    }

    // ── test_engrave_overwrites_existing ──────────────────────────

    #[test]
    fn test_engrave_overwrites_existing() {
        let mut map = EngravingMap::new();
        let mut conduct = ConductState::new();
        let pos = make_pos(5, 5);

        engrave(&mut map, &mut conduct, pos, "hello", EngraveMethod::Dust);
        assert_eq!(map.get(pos).unwrap().text, "hello");

        engrave(
            &mut map,
            &mut conduct,
            pos,
            "Elbereth",
            EngraveMethod::Fire,
        );
        let eng = map.get(pos).unwrap();
        assert_eq!(eng.text, "Elbereth");
        assert_eq!(eng.method, EngraveMethod::Fire);
        assert!(eng.is_readable());
    }

    // ── test_engrave_non_elbereth_no_conduct ─────────────────────

    #[test]
    fn test_engrave_non_elbereth_no_conduct() {
        let mut map = EngravingMap::new();
        let mut conduct = ConductState::new();
        let pos = make_pos(5, 5);

        engrave(&mut map, &mut conduct, pos, "hello world", EngraveMethod::Dust);
        assert_eq!(
            conduct.elbereths, 0,
            "writing non-Elbereth text should not affect conduct"
        );
    }

    // ── test_degrade_engravings ──────────────────────────────────

    #[test]
    fn test_degrade_engravings() {
        let mut map = EngravingMap::new();
        let pos = make_pos(5, 5);
        map.insert(Engraving::new(
            "test".to_string(),
            EngraveMethod::Dust,
            pos,
        ));

        let events = degrade_engravings(&mut map, pos);
        assert!(
            !events.is_empty(),
            "degrading a dust engraving should emit an event"
        );
        assert!(
            map.get(pos).is_none(),
            "dust engraving should be gone after degradation"
        );
    }

    // ── test_engrave_turns_calculation ────────────────────────────

    #[test]
    fn test_engrave_turns_calculation() {
        assert_eq!(EngraveMethod::Dust.turns_per_char(), 1);
        assert_eq!(EngraveMethod::Blade.turns_per_char(), 2);
        assert_eq!(EngraveMethod::Fire.turns_per_char(), 0);
        assert_eq!(EngraveMethod::Lightning.turns_per_char(), 0);
        assert_eq!(EngraveMethod::Dig.turns_per_char(), 0);

        let mut map = EngravingMap::new();
        let mut conduct = ConductState::new();
        let pos = make_pos(5, 5);

        // "Elbereth" = 8 chars, dust = 1 turn/char => 8 turns.
        let (_events, turns) = engrave(
            &mut map,
            &mut conduct,
            pos,
            "Elbereth",
            EngraveMethod::Dust,
        );
        assert_eq!(turns, 8, "dust Elbereth should take 8 turns");

        // "Elbereth" = 8 chars, blade = 2 turns/char => 16 turns.
        let (_events, turns) = engrave(
            &mut map,
            &mut conduct,
            pos,
            "Elbereth",
            EngraveMethod::Blade,
        );
        assert_eq!(turns, 16, "blade Elbereth should take 16 turns");

        // Fire = 0 turns/char => 0 turns.
        let (_events, turns) = engrave(
            &mut map,
            &mut conduct,
            pos,
            "Elbereth",
            EngraveMethod::Fire,
        );
        assert_eq!(turns, 0, "fire Elbereth should be instant");
    }

    // ── test_wipeout_text ─────────────────────────────────────────

    #[test]
    fn test_wipeout_text_replaces_chars() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let result = wipeout_text("Elbereth", 3, &mut rng);
        // Should have exactly 3 spaces where there were letters.
        let spaces_added = result.chars().filter(|&c| c == ' ').count();
        assert_eq!(spaces_added, 3, "should have 3 wiped characters");
        assert_eq!(result.len(), "Elbereth".len());
    }

    #[test]
    fn test_wipeout_text_empty() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        assert_eq!(wipeout_text("", 5, &mut rng), "");
    }

    #[test]
    fn test_wipeout_text_all_chars() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let result = wipeout_text("abc", 10, &mut rng); // more wipes than chars
        assert_eq!(result, "   "); // all spaces
    }

    // ── test_random_engraving ───────────────────────────────────

    #[test]
    fn test_random_engraving_returns_something() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let text = random_engraving(&mut rng);
        assert!(!text.is_empty());
    }

    // ── test_random_epitaph ─────────────────────────────────────

    #[test]
    fn test_random_epitaph_returns_something() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let text = random_epitaph(&mut rng);
        assert!(!text.is_empty());
    }

    // ── test_engrave_tool_classification ─────────────────────────

    #[test]
    fn test_tool_to_method() {
        assert_eq!(tool_to_engrave_method(EngraveTool::Fingers), Some(EngraveMethod::Dust));
        assert_eq!(tool_to_engrave_method(EngraveTool::Blade), Some(EngraveMethod::Blade));
        assert_eq!(tool_to_engrave_method(EngraveTool::WandFire), Some(EngraveMethod::Fire));
        assert_eq!(tool_to_engrave_method(EngraveTool::WandLightning), Some(EngraveMethod::Lightning));
        assert_eq!(tool_to_engrave_method(EngraveTool::WandDigging), Some(EngraveMethod::Dig));
        assert_eq!(tool_to_engrave_method(EngraveTool::Athame), Some(EngraveMethod::Dig));
        assert_eq!(tool_to_engrave_method(EngraveTool::Unsuitable), None);
    }

    #[test]
    fn test_instant_engrave() {
        assert!(is_instant_engrave(EngraveTool::WandFire));
        assert!(is_instant_engrave(EngraveTool::Athame));
        assert!(!is_instant_engrave(EngraveTool::Fingers));
        assert!(!is_instant_engrave(EngraveTool::Blade));
    }

    // ── test_wand_engrave_effects ───────────────────────────────

    #[test]
    fn test_wand_fire_burns() {
        assert_eq!(wand_engrave_effect("fire", false), WandEngraveEffect::Burns);
    }

    #[test]
    fn test_wand_lightning_electrocutes_and_blinds() {
        match wand_engrave_effect("lightning", false) {
            WandEngraveEffect::Electrocutes { blinds } => assert!(blinds),
            other => panic!("expected Electrocutes, got {:?}", other),
        }
    }

    #[test]
    fn test_wand_lightning_no_blind_if_resistant() {
        match wand_engrave_effect("lightning", true) {
            WandEngraveEffect::Electrocutes { blinds } => assert!(!blinds),
            other => panic!("expected Electrocutes, got {:?}", other),
        }
    }

    #[test]
    fn test_wand_polymorph() {
        assert_eq!(wand_engrave_effect("polymorph", false), WandEngraveEffect::Polymorphs);
    }

    #[test]
    fn test_wand_cancellation_removes() {
        assert_eq!(wand_engrave_effect("cancellation", false), WandEngraveEffect::Removes);
    }

    #[test]
    fn test_wand_unknown_no_effect() {
        assert_eq!(wand_engrave_effect("sleep", false), WandEngraveEffect::NoEffect);
    }

    // ── test_can_reach_floor ────────────────────────────────────

    #[test]
    fn test_can_reach_floor_normal() {
        assert!(can_reach_floor(false, false, false, false));
    }

    #[test]
    fn test_cannot_reach_floor_levitating() {
        assert!(!can_reach_floor(true, false, false, false));
    }

    #[test]
    fn test_can_reach_floor_lev_at_will() {
        assert!(can_reach_floor(true, true, false, false));
    }

    #[test]
    fn test_cannot_reach_floor_swallowed() {
        assert!(!can_reach_floor(false, false, true, false));
    }

    #[test]
    fn test_cannot_reach_floor_in_pit() {
        assert!(!can_reach_floor(false, false, false, true));
    }

    // ── test_count_engravings ───────────────────────────────────

    #[test]
    fn test_count_engravings_empty() {
        let map = EngravingMap::new();
        assert_eq!(count_engravings(&map), 0);
    }

    #[test]
    fn test_count_engravings_mixed() {
        let mut map = EngravingMap::new();
        map.insert(Engraving::new("hello".to_string(), EngraveMethod::Fire, make_pos(1, 1)));
        map.insert(Engraving::new("world".to_string(), EngraveMethod::Dust, make_pos(2, 2)));
        assert_eq!(count_engravings(&map), 2);

        // Degrade the dust one until unreadable.
        map.step_on(make_pos(2, 2));
        assert_eq!(count_engravings(&map), 1);
    }

    // ── test_lightning_and_dig_permanent ──────────────────────────

    #[test]
    fn test_lightning_and_dig_permanent() {
        let mut map = EngravingMap::new();
        let pos_l = make_pos(1, 1);
        let pos_d = make_pos(2, 2);

        map.insert(Engraving::new(
            "Elbereth".to_string(),
            EngraveMethod::Lightning,
            pos_l,
        ));
        map.insert(Engraving::new(
            "Elbereth".to_string(),
            EngraveMethod::Dig,
            pos_d,
        ));

        for _ in 0..100 {
            map.step_on(pos_l);
            map.step_on(pos_d);
        }

        assert!(
            map.is_elbereth_at(pos_l),
            "lightning Elbereth should be permanent"
        );
        assert!(
            map.is_elbereth_at(pos_d),
            "dig Elbereth should be permanent"
        );
    }
}
