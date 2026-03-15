//! Fountain effects: drinking, dipping, and fountain depletion.
//!
//! Ported from C NetHack's `fountain.c`.  All functions operate on the ECS
//! `GameWorld` and return `Vec<EngineEvent>` — no IO, no global state.
//!
//! Fountains provide a random mix of beneficial and harmful effects when
//! quaffed from (30 outcomes) or dipped into (additional curse/uncurse and
//! Excalibur creation).  Each use has a chance to dry up the fountain.

use hecs::Entity;
use rand::Rng;

use crate::action::Position;
use crate::dungeon::Terrain;
use crate::event::{EngineEvent, HpSource, StatusEffect};
use crate::world::{Attributes, GameWorld, HitPoints};

// ---------------------------------------------------------------------------
// Fountain state
// ---------------------------------------------------------------------------

/// Per-fountain flags tracking what has been looted.
///
/// In NetHack, each fountain tile has `looted` and `blessedftn` flags
/// that track whether the gem has been found and whether the fountain
/// was blessed.  We store this per-position in the dungeon.
#[derive(Debug, Clone, Copy, Default)]
pub struct FountainState {
    /// Whether the gem has already been found at this fountain.
    pub gem_looted: bool,
    /// Whether this is a blessed/magic fountain.
    pub blessed: bool,
}

// ---------------------------------------------------------------------------
// Drink outcomes
// ---------------------------------------------------------------------------

/// Possible outcomes of drinking from a fountain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrinkOutcome {
    /// Nothing happens (default for many rolls).
    Nothing,
    /// Refreshing drink: restore some HP.
    Refresh,
    /// Self-knowledge: learn your stats.
    SelfKnowledge,
    /// Foul water: lose some HP.
    FoulWater { damage: u32 },
    /// Poisoned water: lose Str, potentially die.
    Poison,
    /// Water snakes appear.
    WaterSnakes { count: u32 },
    /// A water demon appears (may grant a wish).
    WaterDemon,
    /// All carried items get cursed.
    CurseItems,
    /// Gain see invisible.
    SeeInvisible,
    /// Gain monster detection (temporary).
    SeeMonsters { duration: u32 },
    /// Find a gem in the water.
    FindGem,
    /// A water nymph appears and steals.
    WaterNymph,
    /// Scared: gain frightened status temporarily.
    Scared { duration: u32 },
    /// Water gushes: creates a pool at the fountain location.
    WaterGush,
    /// Strange tingling.
    StrangeTingling,
    /// Sudden chill.
    SuddenChill,
    /// Urge to bathe: lose some gold.
    BathUrge { gold_lost: u32 },
    /// See coins in the water: gold appears on the ground.
    SeeCoins { amount: u32 },
}

/// Roll a fountain-drink outcome.
///
/// Mirrors C `drinkfountain()` cases 1-30.
pub fn roll_drink_outcome<R: Rng>(
    rng: &mut R,
    depth: u32,
    fountain_state: &FountainState,
) -> DrinkOutcome {
    let fate = rng.random_range(1..=30);

    match fate {
        1..=9 => DrinkOutcome::Nothing,
        10 => {
            // Refreshing: heal d8 (rolled at application time)
            DrinkOutcome::Refresh
        }
        11 => DrinkOutcome::SelfKnowledge,
        12 => {
            // Foul water: d4 damage
            let damage = rng.random_range(1..=4);
            DrinkOutcome::FoulWater { damage }
        }
        13 => DrinkOutcome::Poison,
        14 => {
            // Water snakes: 2-5
            let count = rng.random_range(2..=5);
            DrinkOutcome::WaterSnakes { count }
        }
        15 => DrinkOutcome::WaterDemon,
        16 => DrinkOutcome::CurseItems,
        17 => DrinkOutcome::SeeInvisible,
        18 => {
            let dur = rng.random_range(50..=150);
            DrinkOutcome::SeeMonsters { duration: dur }
        }
        19 => {
            if fountain_state.gem_looted {
                DrinkOutcome::WaterGush
            } else {
                DrinkOutcome::FindGem
            }
        }
        20 => DrinkOutcome::WaterNymph,
        21 => {
            let dur = rng.random_range(20..=60);
            DrinkOutcome::Scared { duration: dur }
        }
        22 => DrinkOutcome::WaterGush,
        23 => DrinkOutcome::StrangeTingling,
        24 => DrinkOutcome::SuddenChill,
        25 => {
            // Gold loss: somegold / 10 equivalent
            let lost = rng.random_range(1..=50);
            DrinkOutcome::BathUrge { gold_lost: lost }
        }
        26..=28 => {
            // See coins: depth-based amount
            let amount = rng.random_range(1..=depth.max(1) * 2 + 5);
            DrinkOutcome::SeeCoins { amount }
        }
        _ => DrinkOutcome::Nothing,
    }
}

/// Process a drink-from-fountain action.
///
/// Rolls the outcome and generates appropriate events.  Does NOT apply
/// all side effects directly (e.g., monster spawning, inventory cursing) —
/// returns events that the turn loop interprets.
pub fn drink_fountain<R: Rng>(
    rng: &mut R,
    world: &mut GameWorld,
    entity: Entity,
    pos: Position,
    fountain_state: &FountainState,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Verify there's a fountain here
    if world.dungeon().current_level.get(pos).map(|c| c.terrain) != Some(Terrain::Fountain) {
        events.push(EngineEvent::msg("no-fountain-here"));
        return events;
    }

    events.push(EngineEvent::FountainDrank {
        entity,
        position: pos,
    });

    let depth = world.dungeon().depth as u32;
    let outcome = roll_drink_outcome(rng, depth, fountain_state);

    match outcome {
        DrinkOutcome::Nothing => {
            events.push(EngineEvent::msg("fountain-nothing"));
        }
        DrinkOutcome::Refresh => {
            let heal = rng.random_range(1..=8_i32);
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
                let old = hp.current;
                hp.current = (hp.current + heal).min(hp.max);
                let actual = hp.current - old;
                if actual > 0 {
                    events.push(EngineEvent::HpChange {
                        entity,
                        amount: actual,
                        new_hp: hp.current,
                        source: HpSource::Environment,
                    });
                }
            }
            events.push(EngineEvent::msg("fountain-refresh"));
        }
        DrinkOutcome::SelfKnowledge => {
            events.push(EngineEvent::msg("fountain-self-knowledge"));
        }
        DrinkOutcome::FoulWater { damage } => {
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
                hp.current -= damage as i32;
                events.push(EngineEvent::HpChange {
                    entity,
                    amount: -(damage as i32),
                    new_hp: hp.current,
                    source: HpSource::Environment,
                });
            }
            events.push(EngineEvent::msg("fountain-foul"));
        }
        DrinkOutcome::Poison => {
            // Reduce strength by 1 (min 3)
            if let Some(mut attrs) = world.get_component_mut::<Attributes>(entity)
                && attrs.strength > 3
            {
                attrs.strength -= 1;
            }
            events.push(EngineEvent::msg("fountain-poison"));
        }
        DrinkOutcome::WaterSnakes { count } => {
            events.push(EngineEvent::msg_with(
                "fountain-water-snakes",
                vec![("count", count.to_string())],
            ));
        }
        DrinkOutcome::WaterDemon => {
            events.push(EngineEvent::msg("fountain-water-demon"));
        }
        DrinkOutcome::CurseItems => {
            events.push(EngineEvent::msg("fountain-curse-items"));
        }
        DrinkOutcome::SeeInvisible => {
            events.push(EngineEvent::StatusApplied {
                entity,
                status: StatusEffect::SeeInvisible,
                duration: None,
                source: None,
            });
            events.push(EngineEvent::msg("fountain-see-invisible"));
        }
        DrinkOutcome::SeeMonsters { duration } => {
            events.push(EngineEvent::StatusApplied {
                entity,
                status: StatusEffect::Telepathy,
                duration: Some(duration),
                source: None,
            });
            events.push(EngineEvent::msg("fountain-see-monsters"));
        }
        DrinkOutcome::FindGem => {
            events.push(EngineEvent::msg("fountain-find-gem"));
        }
        DrinkOutcome::WaterNymph => {
            events.push(EngineEvent::msg("fountain-water-nymph"));
        }
        DrinkOutcome::Scared { duration } => {
            events.push(EngineEvent::msg_with(
                "fountain-scared",
                vec![("duration", duration.to_string())],
            ));
        }
        DrinkOutcome::WaterGush => {
            events.push(EngineEvent::msg("fountain-gush"));
        }
        DrinkOutcome::StrangeTingling => {
            events.push(EngineEvent::msg("fountain-tingling"));
        }
        DrinkOutcome::SuddenChill => {
            events.push(EngineEvent::msg("fountain-chill"));
        }
        DrinkOutcome::BathUrge { gold_lost } => {
            events.push(EngineEvent::msg_with(
                "fountain-bath",
                vec![("gold", gold_lost.to_string())],
            ));
        }
        DrinkOutcome::SeeCoins { amount } => {
            events.push(EngineEvent::msg_with(
                "fountain-see-coins",
                vec![("amount", amount.to_string())],
            ));
        }
    }

    // Chance to dry up
    if maybe_dry_up(rng) {
        events.extend(dry_up(world, pos));
    }

    events
}

// ---------------------------------------------------------------------------
// Dip outcomes
// ---------------------------------------------------------------------------

/// Possible outcomes of dipping into a fountain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DipOutcome {
    /// Nothing happens.
    Nothing,
    /// The item gets cursed.
    CurseItem,
    /// The item gets uncursed.
    UncurseItem,
    /// Water demon appears.
    WaterDemon,
    /// Water nymph appears.
    WaterNymph,
    /// Water snakes appear.
    WaterSnakes { count: u32 },
    /// Find a gem.
    FindGem,
    /// Water gushes forth.
    WaterGush,
    /// Strange tingling.
    StrangeTingling,
    /// Sudden chill.
    SuddenChill,
    /// Lost gold in fountain.
    LostGold { amount: u32 },
    /// See coins in the water.
    SeeCoins { amount: u32 },
}

/// Roll a dip-into-fountain outcome.
///
/// Mirrors C `dipfountain()` cases 16-30 (after water damage check).
pub fn roll_dip_outcome<R: Rng>(
    rng: &mut R,
    depth: u32,
    fountain_state: &FountainState,
) -> DipOutcome {
    let fate = rng.random_range(1..=30);

    match fate {
        1..=15 => DipOutcome::Nothing,
        16 => DipOutcome::CurseItem,
        17..=20 => DipOutcome::UncurseItem,
        21 => DipOutcome::WaterDemon,
        22 => DipOutcome::WaterNymph,
        23 => {
            let count = rng.random_range(2..=5);
            DipOutcome::WaterSnakes { count }
        }
        24 => {
            if fountain_state.gem_looted {
                DipOutcome::WaterGush
            } else {
                DipOutcome::FindGem
            }
        }
        25 => DipOutcome::WaterGush,
        26 => DipOutcome::StrangeTingling,
        27 => DipOutcome::SuddenChill,
        28 => {
            let amount = rng.random_range(1..=50);
            DipOutcome::LostGold { amount }
        }
        29..=30 => {
            let amount = rng.random_range(1..=depth.max(1) * 2 + 5);
            DipOutcome::SeeCoins { amount }
        }
        _ => DipOutcome::Nothing,
    }
}

/// Process a dip-in-fountain action (non-Excalibur path).
///
/// Returns events for the dip outcome.
pub fn dip_fountain<R: Rng>(
    rng: &mut R,
    world: &mut GameWorld,
    _entity: Entity,
    pos: Position,
    fountain_state: &FountainState,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if world.dungeon().current_level.get(pos).map(|c| c.terrain) != Some(Terrain::Fountain) {
        events.push(EngineEvent::msg("no-fountain-here"));
        return events;
    }

    let depth = world.dungeon().depth as u32;
    let outcome = roll_dip_outcome(rng, depth, fountain_state);

    match outcome {
        DipOutcome::Nothing => {
            events.push(EngineEvent::msg("fountain-dip-nothing"));
        }
        DipOutcome::CurseItem => {
            events.push(EngineEvent::msg("fountain-dip-curse"));
        }
        DipOutcome::UncurseItem => {
            events.push(EngineEvent::msg("fountain-dip-uncurse"));
        }
        DipOutcome::WaterDemon => {
            events.push(EngineEvent::msg("fountain-water-demon"));
        }
        DipOutcome::WaterNymph => {
            events.push(EngineEvent::msg("fountain-water-nymph"));
        }
        DipOutcome::WaterSnakes { count } => {
            events.push(EngineEvent::msg_with(
                "fountain-water-snakes",
                vec![("count", count.to_string())],
            ));
        }
        DipOutcome::FindGem => {
            events.push(EngineEvent::msg("fountain-find-gem"));
        }
        DipOutcome::WaterGush => {
            events.push(EngineEvent::msg("fountain-gush"));
        }
        DipOutcome::StrangeTingling => {
            events.push(EngineEvent::msg("fountain-tingling"));
        }
        DipOutcome::SuddenChill => {
            events.push(EngineEvent::msg("fountain-chill"));
        }
        DipOutcome::LostGold { amount } => {
            events.push(EngineEvent::msg_with(
                "fountain-lost-gold",
                vec![("amount", amount.to_string())],
            ));
        }
        DipOutcome::SeeCoins { amount } => {
            events.push(EngineEvent::msg_with(
                "fountain-see-coins",
                vec![("amount", amount.to_string())],
            ));
        }
    }

    if maybe_dry_up(rng) {
        events.extend(dry_up(world, pos));
    }

    events
}

// ---------------------------------------------------------------------------
// Fountain depletion
// ---------------------------------------------------------------------------

/// Check whether a fountain should dry up after use.
///
/// In NetHack, fountains dry up with probability 1/3 per use.
pub fn maybe_dry_up<R: Rng>(rng: &mut R) -> bool {
    rng.random_range(0..3) == 0
}

/// Dry up a fountain: convert the terrain to a room floor.
///
/// Returns events about the fountain drying up.
pub fn dry_up(world: &mut GameWorld, pos: Position) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if world.dungeon().current_level.get(pos).map(|c| c.terrain) == Some(Terrain::Fountain) {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(pos, Terrain::Floor);
        events.push(EngineEvent::msg("fountain-dried-up"));
    }

    events
}

// ---------------------------------------------------------------------------
// Helpers for spawning fountain creatures
// ---------------------------------------------------------------------------

/// Spawn water snakes near a fountain.
///
/// Returns the number actually created and events.
/// The actual monster creation should use `makemon`.
pub fn spawn_water_snakes_count<R: Rng>(rng: &mut R) -> u32 {
    rng.random_range(2..=5)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::dungeon::Terrain;
    use crate::world::GameWorld;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    fn test_world_with_fountain() -> GameWorld {
        let mut world = GameWorld::new(Position::new(5, 5));
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(5, 5), Terrain::Fountain);
        world
    }

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    fn default_fountain_state() -> FountainState {
        FountainState::default()
    }

    // ── DrinkOutcome distribution ─────────────────────────────

    #[test]
    fn roll_drink_outcome_covers_all_fates() {
        let mut rng = test_rng();
        let fs = default_fountain_state();

        let mut outcomes = Vec::new();
        for _ in 0..1000 {
            outcomes.push(roll_drink_outcome(&mut rng, 5, &fs));
        }

        // Should get a mix of outcomes (not all Nothing)
        let non_nothing = outcomes
            .iter()
            .filter(|o| !matches!(o, DrinkOutcome::Nothing))
            .count();
        assert!(
            non_nothing > 100,
            "expected variety, got {} non-nothing out of 1000",
            non_nothing
        );
    }

    #[test]
    fn roll_drink_outcome_nothing_most_common() {
        let mut rng = test_rng();
        let fs = default_fountain_state();

        let mut nothing_count = 0;
        for _ in 0..3000 {
            if matches!(roll_drink_outcome(&mut rng, 5, &fs), DrinkOutcome::Nothing) {
                nothing_count += 1;
            }
        }
        // Cases 1-9 + 29-30 = 11/30 ≈ 37% are Nothing
        // Allow wide range: 25%-50%
        assert!(
            nothing_count > 750 && nothing_count < 1500,
            "expected ~1100 nothing outcomes, got {}",
            nothing_count
        );
    }

    #[test]
    fn find_gem_blocked_when_looted() {
        let looted = FountainState {
            gem_looted: true,
            blessed: false,
        };

        // Run many rolls; if fate=19 with gem_looted, should be WaterGush
        let mut found_gem = false;
        for seed in 0..10000u64 {
            let mut r = SmallRng::seed_from_u64(seed);
            let outcome = roll_drink_outcome(&mut r, 5, &looted);
            if matches!(outcome, DrinkOutcome::FindGem) {
                found_gem = true;
                break;
            }
        }
        assert!(!found_gem, "should never find gem when looted");
    }

    // ── drink_fountain ────────────────────────────────────────

    #[test]
    fn drink_fountain_at_fountain_succeeds() {
        let mut rng = test_rng();
        let mut world = test_world_with_fountain();
        let player = world.player();
        let fs = default_fountain_state();

        let events =
            drink_fountain(&mut rng, &mut world, player, Position::new(5, 5), &fs);
        // Should have at least the FountainDrank event
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::FountainDrank { .. })),
            "should emit FountainDrank event"
        );
    }

    #[test]
    fn drink_fountain_no_fountain_fails() {
        let mut rng = test_rng();
        let mut world = GameWorld::new(Position::new(5, 5));
        let player = world.player();
        let fs = default_fountain_state();

        let events =
            drink_fountain(&mut rng, &mut world, player, Position::new(5, 5), &fs);
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::Message { key, .. } if key == "no-fountain-here"
            )),
            "should report no fountain"
        );
    }

    // ── DipOutcome distribution ───────────────────────────────

    #[test]
    fn roll_dip_outcome_covers_range() {
        let mut rng = test_rng();
        let fs = default_fountain_state();

        let mut outcomes = Vec::new();
        for _ in 0..1000 {
            outcomes.push(roll_dip_outcome(&mut rng, 5, &fs));
        }

        let non_nothing = outcomes
            .iter()
            .filter(|o| !matches!(o, DipOutcome::Nothing))
            .count();
        assert!(
            non_nothing > 100,
            "expected variety in dip outcomes, got {} non-nothing",
            non_nothing
        );
    }

    // ── dip_fountain ──────────────────────────────────────────

    #[test]
    fn dip_fountain_at_fountain_succeeds() {
        let mut rng = test_rng();
        let mut world = test_world_with_fountain();
        let player = world.player();
        let fs = default_fountain_state();

        let events =
            dip_fountain(&mut rng, &mut world, player, Position::new(5, 5), &fs);
        assert!(!events.is_empty());
    }

    #[test]
    fn dip_fountain_no_fountain_fails() {
        let mut rng = test_rng();
        let mut world = GameWorld::new(Position::new(5, 5));
        let player = world.player();
        let fs = default_fountain_state();

        let events =
            dip_fountain(&mut rng, &mut world, player, Position::new(5, 5), &fs);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "no-fountain-here"
        )));
    }

    // ── dry_up ────────────────────────────────────────────────

    #[test]
    fn dry_up_converts_to_floor() {
        let mut world = test_world_with_fountain();
        let pos = Position::new(5, 5);

        assert_eq!(world.dungeon().current_level.get(pos).map(|c| c.terrain), Some(Terrain::Fountain));

        let events = dry_up(&mut world, pos);
        assert!(!events.is_empty());
        assert_eq!(world.dungeon().current_level.get(pos).map(|c| c.terrain), Some(Terrain::Floor));
    }

    #[test]
    fn dry_up_no_fountain_no_events() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let pos = Position::new(5, 5);

        let events = dry_up(&mut world, pos);
        assert!(events.is_empty());
    }

    // ── maybe_dry_up ──────────────────────────────────────────

    #[test]
    fn maybe_dry_up_probability() {
        let mut rng = test_rng();
        let mut dried = 0;
        for _ in 0..3000 {
            if maybe_dry_up(&mut rng) {
                dried += 1;
            }
        }
        // 1/3 ≈ 33%, expect ~1000, allow 800-1200
        assert!(
            dried > 800 && dried < 1200,
            "expected ~1000 dry-ups, got {}",
            dried
        );
    }

    // ── spawn_water_snakes_count ──────────────────────────────

    #[test]
    fn water_snakes_count_range() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let count = spawn_water_snakes_count(&mut rng);
            assert!(count >= 2 && count <= 5);
        }
    }

    // ── Fountain refresh heals HP ─────────────────────────────

    #[test]
    fn drink_refresh_heals() {
        // Use a seed that produces a Refresh outcome (fate=10)
        // We'll just check that HpChange events can be emitted
        let mut world = test_world_with_fountain();
        let player = world.player();

        // Lower player HP so healing is visible
        {
            let mut hp = world.get_component_mut::<HitPoints>(player).unwrap();
            hp.current = 8;
        }

        let fs = default_fountain_state();
        // Try many seeds to find one that gives Refresh
        for seed in 0..1000u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            // Reset fountain terrain (may have dried up)
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(5, 5), Terrain::Fountain);
            // Reset HP
            {
                let mut hp = world.get_component_mut::<HitPoints>(player).unwrap();
                hp.current = 8;
            }

            let events = drink_fountain(
                &mut rng,
                &mut world,
                player,
                Position::new(5, 5),
                &fs,
            );
            let has_heal = events
                .iter()
                .any(|e| matches!(e, EngineEvent::HpChange { amount, .. } if *amount > 0));
            if has_heal {
                // Found a healing outcome — test passes
                return;
            }
        }
        panic!("could not find a seed that produces a Refresh outcome");
    }

    // ── Fountain poison reduces strength ──────────────────────

    #[test]
    fn drink_poison_reduces_strength() {
        let mut world = test_world_with_fountain();
        let player = world.player();
        let fs = default_fountain_state();

        let original_str = {
            let attrs = world.get_component::<Attributes>(player).unwrap();
            attrs.strength
        };

        // Find a seed that produces Poison outcome
        for seed in 0..10000u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            // Reset
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(5, 5), Terrain::Fountain);
            {
                let mut attrs = world.get_component_mut::<Attributes>(player).unwrap();
                attrs.strength = original_str;
            }

            let events = drink_fountain(
                &mut rng,
                &mut world,
                player,
                Position::new(5, 5),
                &fs,
            );
            let has_poison = events.iter().any(|e| matches!(
                e,
                EngineEvent::Message { key, .. } if key == "fountain-poison"
            ));
            if has_poison {
                let new_str = {
                    let attrs = world.get_component::<Attributes>(player).unwrap();
                    attrs.strength
                };
                assert!(
                    new_str < original_str,
                    "strength should decrease from {} to less, got {}",
                    original_str,
                    new_str
                );
                return;
            }
        }
        panic!("could not find a seed that produces a Poison outcome");
    }
}
