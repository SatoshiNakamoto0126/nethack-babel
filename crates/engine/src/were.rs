//! Lycanthropy system: infection, transformation, full moon effects,
//! and human/animal form mapping.
//!
//! Ported from C NetHack's `were.c`.  All functions operate on the ECS
//! `GameWorld` and return `Vec<EngineEvent>` — no IO, no global state.
//!
//! In NetHack, werecreatures come in pairs: a human form and an animal
//! form.  When the player contracts lycanthropy (AD_WERE attack), they
//! can transform between forms.  Full moon nights force transformation.

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use nethack_babel_data::MonsterId;

use crate::event::{EngineEvent, StatusEffect};
use crate::world::GameWorld;

// ---------------------------------------------------------------------------
// WereType enum
// ---------------------------------------------------------------------------

/// The three types of lycanthropy in NetHack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WereType {
    Wererat,
    Werejackal,
    Werewolf,
}

impl WereType {
    /// Get the MonsterId of the human form for this were-type.
    pub fn human_form(self) -> MonsterId {
        match self {
            WereType::Wererat => WERE_PAIRS[0].human,
            WereType::Werejackal => WERE_PAIRS[1].human,
            WereType::Werewolf => WERE_PAIRS[2].human,
        }
    }

    /// Get the MonsterId of the animal form for this were-type.
    pub fn animal_form(self) -> MonsterId {
        match self {
            WereType::Wererat => WERE_PAIRS[0].animal,
            WereType::Werejackal => WERE_PAIRS[1].animal,
            WereType::Werewolf => WERE_PAIRS[2].animal,
        }
    }

    /// Try to determine the WereType from a MonsterId.
    pub fn from_monster_id(id: MonsterId) -> Option<Self> {
        for (i, pair) in WERE_PAIRS.iter().enumerate() {
            if id == pair.human || id == pair.animal {
                return Some(match i {
                    0 => WereType::Wererat,
                    1 => WereType::Werejackal,
                    _ => WereType::Werewolf,
                });
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Were-creature form pairs
// ---------------------------------------------------------------------------

/// A pair of monster IDs representing human and animal forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WerePair {
    /// The human form (e.g. werewolf in human form).
    pub human: MonsterId,
    /// The animal form (e.g. wolf).
    pub animal: MonsterId,
}

/// Hardcoded were-creature form pairs from the C source.
///
/// In NetHack: werewolf ↔ wolf, werejackal ↔ jackal, wererat ↔ rat.
/// Monster IDs here are placeholders — the data crate assigns final
/// values from monsters.toml.  We use well-known conventional IDs.
///
/// The caller should resolve these against the actual loaded MonsterDef table.
pub const WERE_PAIRS: &[WerePair] = &[
    // PM_WERERAT ↔ PM_SEWER_RAT (using conventional IDs)
    WerePair {
        human: MonsterId(100),
        animal: MonsterId(101),
    },
    // PM_WEREJACKAL ↔ PM_JACKAL
    WerePair {
        human: MonsterId(102),
        animal: MonsterId(103),
    },
    // PM_WEREWOLF ↔ PM_WOLF
    WerePair {
        human: MonsterId(104),
        animal: MonsterId(105),
    },
];

/// Look up the counter-form for a given were-creature monster ID.
///
/// If the ID is a human-form werecreature, returns the animal form.
/// If the ID is an animal-form, returns the human form.
/// Returns `None` if the ID is not part of any were-pair.
///
/// Mirrors C `counter_were(mndx)`.
pub fn counter_were(monster_id: MonsterId) -> Option<MonsterId> {
    for pair in WERE_PAIRS {
        if monster_id == pair.human {
            return Some(pair.animal);
        }
        if monster_id == pair.animal {
            return Some(pair.human);
        }
    }
    None
}

/// Given a were-creature (either form), return the animal form.
///
/// Returns `None` if not a were-creature.
/// Mirrors C `were_beastie(mndx)`.
pub fn were_beastie(monster_id: MonsterId) -> Option<MonsterId> {
    for pair in WERE_PAIRS {
        if monster_id == pair.human || monster_id == pair.animal {
            return Some(pair.animal);
        }
    }
    None
}

/// Given a were-creature (either form), return the human form.
///
/// Returns `None` if not a were-creature.
pub fn were_human(monster_id: MonsterId) -> Option<MonsterId> {
    for pair in WERE_PAIRS {
        if monster_id == pair.human || monster_id == pair.animal {
            return Some(pair.human);
        }
    }
    None
}

/// Check whether a monster ID represents a were-creature (either form).
pub fn is_were(monster_id: MonsterId) -> bool {
    WERE_PAIRS
        .iter()
        .any(|p| p.human == monster_id || p.animal == monster_id)
}

// ---------------------------------------------------------------------------
// Player lycanthropy component
// ---------------------------------------------------------------------------

/// Component: lycanthropy infection state for the player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LycanthropyState {
    /// Which type of lycanthropy the player has (human-form MonsterId).
    pub were_type: MonsterId,
    /// Whether the player is currently in animal form.
    pub in_animal_form: bool,
}

// ---------------------------------------------------------------------------
// Lycanthropy infection
// ---------------------------------------------------------------------------

/// Infect the player with lycanthropy from a were-creature attack.
///
/// Returns events describing the infection.  If the player already has
/// lycanthropy of the same type, no additional effect.
pub fn infect_lycanthropy(
    world: &mut GameWorld,
    victim: Entity,
    were_type: MonsterId,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Resolve to the human form for storage
    let human_form = match were_human(were_type) {
        Some(h) => h,
        None => return events,
    };

    // Check if already infected with same type
    if let Some(state) = world.get_component::<LycanthropyState>(victim)
        && state.were_type == human_form
    {
        return events;
    }

    // Apply infection
    let state = LycanthropyState {
        were_type: human_form,
        in_animal_form: false,
    };
    let _ = world.ecs_mut().insert_one(victim, state);

    events.push(EngineEvent::StatusApplied {
        entity: victim,
        status: StatusEffect::Lycanthropy,
        duration: None,
        source: None,
    });
    events.push(EngineEvent::msg("lycanthropy-infected"));

    events
}

/// Cure lycanthropy from an entity.
///
/// Returns events describing the cure.
pub fn cure_lycanthropy(world: &mut GameWorld, entity: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let had_it = world.get_component::<LycanthropyState>(entity).is_some();
    if !had_it {
        return events;
    }

    let _ = world.ecs_mut().remove_one::<LycanthropyState>(entity);

    events.push(EngineEvent::StatusRemoved {
        entity,
        status: StatusEffect::Lycanthropy,
    });
    events.push(EngineEvent::msg("lycanthropy-cured"));

    events
}

// ---------------------------------------------------------------------------
// Transformation
// ---------------------------------------------------------------------------

/// Attempt to transform a were-creature (monster) to its other form.
///
/// Mirrors C `new_were(mon)`.  In the full game this changes the monster's
/// species, stats, and display.  Here we record the transformation and
/// emit events.  The caller handles actual component updates.
///
/// Returns `Some(new_monster_id)` on success, `None` if not a were-creature.
pub fn transform_were(
    world: &GameWorld,
    entity: Entity,
    current_id: MonsterId,
) -> Option<(MonsterId, Vec<EngineEvent>)> {
    let new_id = counter_were(current_id)?;

    let mut events = Vec::new();
    let name = world.entity_name(entity);
    events.push(EngineEvent::msg_with(
        "were-transform",
        vec![("name", name)],
    ));

    Some((new_id, events))
}

/// Check if it's a full moon night.
///
/// In NetHack, the moon phase drives forced were-transformations.
/// This is a simplified check — the full implementation would use
/// real moon phase calculation from the system clock.
pub fn is_full_moon(turn: u32) -> bool {
    // Simplified: full moon every 80 turns (NetHack's night_time()
    // checks actual system date; we use a game-turn approximation).
    (turn % 80) < 10
}

/// Check whether the current turn falls during night hours.
///
/// NetHack uses wall-clock time. We approximate that cycle over 24 turns:
/// 00:00-05:59 and 22:00-23:59 count as night.
pub fn is_night(turn: u32) -> bool {
    let hour = turn % 24;
    !(6..=21).contains(&hour)
}

/// Check whether the current turn falls exactly at midnight.
pub fn is_midnight(turn: u32) -> bool {
    turn.is_multiple_of(24)
}

/// Decide whether a were-creature should change form this turn.
///
/// Mirrors C `were_change(mon)`.
/// Returns `true` if the creature should transform.
pub fn should_were_change<R: Rng>(rng: &mut R, turn: u32, is_animal_form: bool) -> bool {
    let full_moon = is_full_moon(turn);

    if full_moon && !is_animal_form {
        // During full moon, human form always transforms to animal
        true
    } else if !full_moon && is_animal_form {
        // Outside full moon, 1/60 chance per turn to revert
        rng.random_range(0..60) == 0
    } else {
        false
    }
}

/// Process a were-creature's potential transformation this turn.
///
/// Checks full moon status and random reversion, then transforms if needed.
/// Mirrors C `were_change(mon)`.
pub fn were_change<R: Rng>(
    rng: &mut R,
    world: &mut GameWorld,
    entity: Entity,
    current_id: MonsterId,
) -> Vec<EngineEvent> {
    let is_animal = were_human(current_id).is_some_and(|h| h != current_id);
    let turn = world.turn();

    if !should_were_change(rng, turn, is_animal) {
        return Vec::new();
    }

    match transform_were(world, entity, current_id) {
        Some((_new_id, events)) => events,
        None => Vec::new(),
    }
}

/// Check if full moon forces a player transformation.
///
/// If the player has lycanthropy and it's a full moon, force transformation
/// to animal form.  Returns events if transformation occurs.
pub fn check_full_moon<R: Rng>(
    rng: &mut R,
    world: &mut GameWorld,
    entity: Entity,
) -> Vec<EngineEvent> {
    let turn = world.turn();
    if !is_full_moon(turn) {
        return Vec::new();
    }

    let state = match world.get_component::<LycanthropyState>(entity) {
        Some(s) => (s.were_type, s.in_animal_form),
        None => return Vec::new(),
    };

    let (were_type, in_animal_form) = state;

    if in_animal_form {
        return Vec::new();
    }

    // Force transformation to animal form (validate it's a valid were-type)
    let _animal = match were_beastie(were_type) {
        Some(a) => a,
        None => return Vec::new(),
    };

    let mut events = Vec::new();
    events.push(EngineEvent::msg("lycanthropy-full-moon-transform"));

    // Update the lycanthropy state
    if let Some(mut ls) = world.get_component_mut::<LycanthropyState>(entity) {
        ls.in_animal_form = true;
    }

    // Summon pack animals (1-5)
    let summon_count = were_summon_count(rng);
    events.push(EngineEvent::msg_with(
        "were-summon-pack",
        vec![("count", summon_count.to_string())],
    ));

    events
}

// ---------------------------------------------------------------------------
// Were summoning
// ---------------------------------------------------------------------------

/// Determine how many pack animals a were-creature summons.
///
/// Mirrors C `were_summon(ptr, ...)`.  The were-creature summons
/// animal allies when it transforms to animal form.
pub fn were_summon_count<R: Rng>(rng: &mut R) -> u32 {
    // NetHack: rnd(5), so 1-5 animals
    rng.random_range(1..=5)
}

/// Get the monster ID of the animal allies summoned by a were-creature.
///
/// Returns `None` if not a were-creature.
pub fn were_summon_type(were_type: MonsterId) -> Option<MonsterId> {
    were_beastie(were_type)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::world::GameWorld;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn test_world() -> GameWorld {
        GameWorld::new(Position::new(5, 5))
    }

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    // ── Form lookups ──────────────────────────────────────────

    #[test]
    fn counter_were_returns_opposite_form() {
        // Werewolf (human) → wolf (animal)
        assert_eq!(counter_were(MonsterId(104)), Some(MonsterId(105)));
        // Wolf (animal) → werewolf (human)
        assert_eq!(counter_were(MonsterId(105)), Some(MonsterId(104)));
    }

    #[test]
    fn counter_were_nonwere_returns_none() {
        assert_eq!(counter_were(MonsterId(999)), None);
    }

    #[test]
    fn were_beastie_returns_animal_form() {
        // Werewolf human → wolf
        assert_eq!(were_beastie(MonsterId(104)), Some(MonsterId(105)));
        // Wolf → wolf (already animal)
        assert_eq!(were_beastie(MonsterId(105)), Some(MonsterId(105)));
    }

    #[test]
    fn were_beastie_nonwere_returns_none() {
        assert_eq!(were_beastie(MonsterId(0)), None);
    }

    #[test]
    fn were_human_returns_human_form() {
        // Wolf → werewolf
        assert_eq!(were_human(MonsterId(105)), Some(MonsterId(104)));
        // Werewolf → werewolf (already human)
        assert_eq!(were_human(MonsterId(104)), Some(MonsterId(104)));
    }

    #[test]
    fn is_were_checks_both_forms() {
        assert!(is_were(MonsterId(100))); // wererat human
        assert!(is_were(MonsterId(101))); // sewer rat (animal)
        assert!(!is_were(MonsterId(999)));
    }

    // ── All three pairs work ──────────────────────────────────

    #[test]
    fn all_pairs_have_bidirectional_mapping() {
        for pair in WERE_PAIRS {
            assert_eq!(counter_were(pair.human), Some(pair.animal));
            assert_eq!(counter_were(pair.animal), Some(pair.human));
            assert_eq!(were_beastie(pair.human), Some(pair.animal));
            assert_eq!(were_beastie(pair.animal), Some(pair.animal));
            assert_eq!(were_human(pair.human), Some(pair.human));
            assert_eq!(were_human(pair.animal), Some(pair.human));
        }
    }

    // ── Infection ─────────────────────────────────────────────

    #[test]
    fn infect_lycanthropy_adds_component() {
        let mut world = test_world();
        let player = world.player();

        let events = infect_lycanthropy(&mut world, player, MonsterId(104));
        assert_eq!(events.len(), 2);

        let state = world
            .get_component::<LycanthropyState>(player)
            .expect("should have lycanthropy");
        assert_eq!(state.were_type, MonsterId(104));
        assert!(!state.in_animal_form);
    }

    #[test]
    fn infect_lycanthropy_same_type_no_duplicate() {
        let mut world = test_world();
        let player = world.player();

        infect_lycanthropy(&mut world, player, MonsterId(104));
        let events = infect_lycanthropy(&mut world, player, MonsterId(104));
        assert!(events.is_empty());
    }

    #[test]
    fn infect_lycanthropy_nonwere_does_nothing() {
        let mut world = test_world();
        let player = world.player();

        let events = infect_lycanthropy(&mut world, player, MonsterId(999));
        assert!(events.is_empty());
    }

    // ── Cure ──────────────────────────────────────────────────

    #[test]
    fn cure_lycanthropy_removes_component() {
        let mut world = test_world();
        let player = world.player();

        infect_lycanthropy(&mut world, player, MonsterId(100));
        let events = cure_lycanthropy(&mut world, player);
        assert_eq!(events.len(), 2);
        assert!(world.get_component::<LycanthropyState>(player).is_none());
    }

    #[test]
    fn cure_lycanthropy_when_not_infected() {
        let mut world = test_world();
        let player = world.player();

        let events = cure_lycanthropy(&mut world, player);
        assert!(events.is_empty());
    }

    // ── Transformation ────────────────────────────────────────

    #[test]
    fn transform_were_returns_counter_form() {
        let world = test_world();
        // Just test the lookup; entity doesn't matter much
        let result = transform_were(&world, world.player(), MonsterId(104));
        let (new_id, events) = result.unwrap();
        assert_eq!(new_id, MonsterId(105));
        assert!(!events.is_empty());
    }

    #[test]
    fn transform_nonwere_returns_none() {
        let world = test_world();
        let result = transform_were(&world, world.player(), MonsterId(999));
        assert!(result.is_none());
    }

    // ── Full moon ─────────────────────────────────────────────

    #[test]
    fn full_moon_periodic() {
        // Turns 0-9 should be full moon
        for t in 0..10 {
            assert!(is_full_moon(t), "turn {} should be full moon", t);
        }
        // Turns 10-79 should not
        for t in 10..80 {
            assert!(!is_full_moon(t), "turn {} should not be full moon", t);
        }
        // Cycle repeats
        assert!(is_full_moon(80));
        assert!(is_full_moon(89));
        assert!(!is_full_moon(90));
    }

    // ── should_were_change ────────────────────────────────────

    #[test]
    fn should_change_full_moon_human_form() {
        let mut rng = test_rng();
        // Full moon + human form → always change
        assert!(should_were_change(&mut rng, 0, false));
    }

    #[test]
    fn should_change_full_moon_already_animal() {
        let mut rng = test_rng();
        // Full moon + already animal → no change
        assert!(!should_were_change(&mut rng, 0, true));
    }

    #[test]
    fn should_change_no_moon_human_stays() {
        let mut rng = test_rng();
        // Not full moon + human form → no change (unless very lucky)
        // Test over many trials — mostly false
        let mut changes = 0;
        for _ in 0..1000 {
            if should_were_change(&mut rng, 50, false) {
                changes += 1;
            }
        }
        assert_eq!(changes, 0);
    }

    #[test]
    fn should_change_no_moon_animal_sometimes_reverts() {
        let mut rng = test_rng();
        // Not full moon + animal form → ~1/60 chance per turn
        let mut reverts = 0;
        for _ in 0..6000 {
            if should_were_change(&mut rng, 50, true) {
                reverts += 1;
            }
        }
        // Expect ~100 reverts (6000/60), allow wide range
        assert!(
            reverts > 50 && reverts < 200,
            "expected ~100 reverts, got {}",
            reverts
        );
    }

    // ── Summoning ─────────────────────────────────────────────

    #[test]
    fn were_summon_count_range() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let count = were_summon_count(&mut rng);
            assert!(count >= 1 && count <= 5);
        }
    }

    #[test]
    fn were_summon_type_returns_animal() {
        // Werewolf → wolf
        assert_eq!(were_summon_type(MonsterId(104)), Some(MonsterId(105)));
        // Nonwere → None
        assert_eq!(were_summon_type(MonsterId(999)), None);
    }

    // ── WereType enum ─────────────────────────────────────────

    #[test]
    fn were_type_human_form() {
        assert_eq!(WereType::Wererat.human_form(), MonsterId(100));
        assert_eq!(WereType::Werejackal.human_form(), MonsterId(102));
        assert_eq!(WereType::Werewolf.human_form(), MonsterId(104));
    }

    #[test]
    fn were_type_animal_form() {
        assert_eq!(WereType::Wererat.animal_form(), MonsterId(101));
        assert_eq!(WereType::Werejackal.animal_form(), MonsterId(103));
        assert_eq!(WereType::Werewolf.animal_form(), MonsterId(105));
    }

    #[test]
    fn were_type_from_monster_id() {
        assert_eq!(
            WereType::from_monster_id(MonsterId(100)),
            Some(WereType::Wererat)
        );
        assert_eq!(
            WereType::from_monster_id(MonsterId(101)),
            Some(WereType::Wererat)
        );
        assert_eq!(
            WereType::from_monster_id(MonsterId(104)),
            Some(WereType::Werewolf)
        );
        assert_eq!(
            WereType::from_monster_id(MonsterId(105)),
            Some(WereType::Werewolf)
        );
        assert_eq!(WereType::from_monster_id(MonsterId(999)), None);
    }

    // ── were_change ───────────────────────────────────────────

    #[test]
    fn were_change_at_full_moon() {
        let mut rng = test_rng();
        let mut world = test_world();
        // Set turn to full moon (turn 0 is full moon)
        // World starts at turn 1, but is_full_moon(1) is true (1 < 10)
        let entity = world.player();

        // Werewolf human form at full moon should transform
        let events = were_change(&mut rng, &mut world, entity, MonsterId(104));
        assert!(!events.is_empty());
    }

    #[test]
    fn were_change_nonwere_does_nothing() {
        let mut rng = test_rng();
        let mut world = test_world();
        let entity = world.player();

        let events = were_change(&mut rng, &mut world, entity, MonsterId(999));
        assert!(events.is_empty());
    }

    // ── check_full_moon ───────────────────────────────────────

    #[test]
    fn check_full_moon_infects_player() {
        let mut rng = test_rng();
        let mut world = test_world();
        let player = world.player();

        // Infect player first
        infect_lycanthropy(&mut world, player, MonsterId(104));

        // World starts at turn 1, which is full moon
        let events = check_full_moon(&mut rng, &mut world, player);
        assert!(!events.is_empty());

        // Should now be in animal form
        let state = world.get_component::<LycanthropyState>(player).unwrap();
        assert!(state.in_animal_form);
    }

    #[test]
    fn check_full_moon_no_lycanthropy() {
        let mut rng = test_rng();
        let mut world = test_world();
        let player = world.player();

        let events = check_full_moon(&mut rng, &mut world, player);
        assert!(events.is_empty());
    }

    #[test]
    fn check_full_moon_already_animal() {
        let mut rng = test_rng();
        let mut world = test_world();
        let player = world.player();

        // Infect and set to animal form
        infect_lycanthropy(&mut world, player, MonsterId(104));
        if let Some(mut state) = world.get_component_mut::<LycanthropyState>(player) {
            state.in_animal_form = true;
        }

        let events = check_full_moon(&mut rng, &mut world, player);
        assert!(events.is_empty());
    }
}
