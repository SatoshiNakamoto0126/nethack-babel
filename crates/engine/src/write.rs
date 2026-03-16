//! Magic marker writing system for NetHack Babel.
//!
//! Implements the ability to write scrolls and spellbooks using a
//! magic marker.  Writing consumes marker charges; higher-level items
//! cost more.  Success depends on the player's Luck stat.  On failure,
//! a gibberish scroll ("scroll labeled DAIYEN FANSEN") is produced.
//!
//! Prerequisite: the player must have already identified the target
//! scroll or spellbook type.
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::event::EngineEvent;
use crate::scrolls::ScrollType;
use crate::spells::SpellType;
use crate::world::{GameWorld, PlayerCombat};

// ---------------------------------------------------------------------------
// Dice helpers
// ---------------------------------------------------------------------------

/// rn2(x) = uniform in [0, x).
#[inline]
fn rn2<R: Rng>(rng: &mut R, x: u32) -> u32 {
    if x <= 1 {
        return 0;
    }
    rng.random_range(0..x)
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Component: charges remaining in a magic marker.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MarkerCharges(pub u8);

// ---------------------------------------------------------------------------
// Scroll difficulty classification
// ---------------------------------------------------------------------------

/// Return the "level" of a scroll type for charge cost calculation.
///
/// Level 1-2 scrolls: common utility scrolls (light, blank paper, etc.)
/// Level 3-4 scrolls: moderately powerful (enchant weapon/armor, etc.)
/// Level 5+  scrolls: powerful (genocide, charging, etc.)
fn scroll_level(scroll: ScrollType) -> u32 {
    match scroll {
        ScrollType::Light | ScrollType::BlankPaper | ScrollType::Mail => 1,

        ScrollType::ConfuseMonster
        | ScrollType::ScareMonster
        | ScrollType::FoodDetection
        | ScrollType::GoldDetection
        | ScrollType::CreateMonster
        | ScrollType::Earth
        | ScrollType::Fire => 2,

        ScrollType::Identify
        | ScrollType::Teleportation
        | ScrollType::Amnesia
        | ScrollType::Taming
        | ScrollType::Punishment
        | ScrollType::StinkingCloud => 3,

        ScrollType::EnchantWeapon
        | ScrollType::EnchantArmor
        | ScrollType::RemoveCurse
        | ScrollType::DestroyArmor
        | ScrollType::MagicMapping => 4,

        ScrollType::Genocide | ScrollType::Charging => 5,
    }
}

/// Charge cost for writing a scroll: depends on scroll level.
///
/// Level 1-2: 4-8 charges (uniform).
/// Level 3-4: 8-16 charges.
/// Level 5+:  16-24 charges.
fn scroll_charge_cost(scroll: ScrollType, rng: &mut impl Rng) -> u8 {
    let level = scroll_level(scroll);
    match level {
        1..=2 => rng.random_range(4..=8),
        3..=4 => rng.random_range(8..=16),
        _ => rng.random_range(16..=24),
    }
}

/// Charge cost for writing a spellbook: 2x the equivalent scroll cost.
fn spellbook_charge_cost(rng: &mut impl Rng) -> u8 {
    // Spellbooks are uniformly expensive: 16-32 charges.
    rng.random_range(16..=32)
}

// ---------------------------------------------------------------------------
// Writing logic
// ---------------------------------------------------------------------------

/// Check whether the Luck-based writing attempt succeeds.
///
/// Success if `rn2(luck + 20) > 10`, where luck is the player's
/// current luck value (clamped to effective range).
fn writing_succeeds(luck: i32, rng: &mut impl Rng) -> bool {
    let adjusted = (luck + 20).max(1) as u32;
    rn2(rng, adjusted) > 10
}

/// Write a scroll using a magic marker.
///
/// Consumes charges from the marker.  On success, emits an event
/// indicating the scroll was created.  On failure, emits an event
/// for the gibberish scroll.
///
/// Returns `Err` message if the marker lacks sufficient charges.
pub fn write_scroll(
    world: &mut GameWorld,
    player: Entity,
    marker: Entity,
    scroll_type: ScrollType,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Get current marker charges.
    let charges = match world.get_component::<MarkerCharges>(marker) {
        Some(mc) => mc.0,
        None => {
            events.push(EngineEvent::msg("write-no-marker"));
            return events;
        }
    };

    let cost = scroll_charge_cost(scroll_type, rng);
    if charges < cost {
        events.push(EngineEvent::msg("write-not-enough-charges"));
        return events;
    }

    // Consume charges.
    if let Some(mut mc) = world.get_component_mut::<MarkerCharges>(marker) {
        mc.0 = mc.0.saturating_sub(cost);
    }

    events.push(EngineEvent::ItemCharged {
        item: marker,
        new_charges: world
            .get_component::<MarkerCharges>(marker)
            .map(|mc| mc.0 as i8)
            .unwrap_or(0),
    });

    // Luck-based success check.
    let luck = world
        .get_component::<PlayerCombat>(player)
        .map(|pc| pc.luck)
        .unwrap_or(0);

    if writing_succeeds(luck, rng) {
        events.push(EngineEvent::msg_with(
            "write-scroll-success",
            vec![("scroll", format!("{:?}", scroll_type))],
        ));
    } else {
        // Failure: produce gibberish.
        events.push(EngineEvent::msg("write-scroll-fail-daiyen-fansen"));
    }

    events
}

/// Write a spellbook using a magic marker.
///
/// Similar to scroll writing but costs 2x charges.
pub fn write_spellbook(
    world: &mut GameWorld,
    player: Entity,
    marker: Entity,
    _spell_type: SpellType,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let charges = match world.get_component::<MarkerCharges>(marker) {
        Some(mc) => mc.0,
        None => {
            events.push(EngineEvent::msg("write-no-marker"));
            return events;
        }
    };

    let cost = spellbook_charge_cost(rng);
    if charges < cost {
        events.push(EngineEvent::msg("write-not-enough-charges"));
        return events;
    }

    // Consume charges.
    if let Some(mut mc) = world.get_component_mut::<MarkerCharges>(marker) {
        mc.0 = mc.0.saturating_sub(cost);
    }

    events.push(EngineEvent::ItemCharged {
        item: marker,
        new_charges: world
            .get_component::<MarkerCharges>(marker)
            .map(|mc| mc.0 as i8)
            .unwrap_or(0),
    });

    let luck = world
        .get_component::<PlayerCombat>(player)
        .map(|pc| pc.luck)
        .unwrap_or(0);

    if writing_succeeds(luck, rng) {
        events.push(EngineEvent::msg("write-spellbook-success"));
    } else {
        events.push(EngineEvent::msg("write-spellbook-fail"));
    }

    events
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn make_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    fn setup_marker(world: &mut GameWorld, charges: u8) -> Entity {
        world.spawn((MarkerCharges(charges),))
    }

    #[test]
    fn write_scroll_consumes_charges() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();
        let player = world.player();
        let marker = setup_marker(&mut world, 50);

        let events = write_scroll(&mut world, player, marker, ScrollType::Light, &mut rng);

        // Charges should have decreased.
        let remaining = world.get_component::<MarkerCharges>(marker).unwrap().0;
        assert!(remaining < 50);
        // Should have ItemCharged and either success or fail event.
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::ItemCharged { .. }))
        );
    }

    #[test]
    fn write_scroll_insufficient_charges() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();
        let player = world.player();
        let marker = setup_marker(&mut world, 1); // Not enough for any scroll.

        let events = write_scroll(&mut world, player, marker, ScrollType::Genocide, &mut rng);

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "write-not-enough-charges"
        )));
        // Charges untouched.
        let remaining = world.get_component::<MarkerCharges>(marker).unwrap().0;
        assert_eq!(remaining, 1);
    }

    #[test]
    fn write_scroll_no_marker() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();
        let player = world.player();
        // Entity without MarkerCharges.
        let fake = world.spawn(());

        let events = write_scroll(&mut world, player, fake, ScrollType::Light, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "write-no-marker"
        )));
    }

    #[test]
    fn write_spellbook_costs_more() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng1 = make_rng();
        let mut rng2 = make_rng();
        let player = world.player();

        let marker1 = setup_marker(&mut world, 100);
        write_scroll(&mut world, player, marker1, ScrollType::Light, &mut rng1);
        let scroll_remaining = world.get_component::<MarkerCharges>(marker1).unwrap().0;

        let marker2 = setup_marker(&mut world, 100);
        write_spellbook(&mut world, player, marker2, SpellType::Light, &mut rng2);
        let book_remaining = world.get_component::<MarkerCharges>(marker2).unwrap().0;

        // Spellbook should cost more (or equal in edge cases), so fewer
        // charges remain.  With the same seed the spellbook cost (16-32)
        // should be >= scroll cost (4-8).
        assert!(book_remaining <= scroll_remaining);
    }

    #[test]
    fn scroll_level_classification() {
        assert_eq!(scroll_level(ScrollType::Light), 1);
        assert_eq!(scroll_level(ScrollType::Identify), 3);
        assert_eq!(scroll_level(ScrollType::Genocide), 5);
        assert_eq!(scroll_level(ScrollType::EnchantWeapon), 4);
    }

    #[test]
    fn writing_success_depends_on_luck() {
        // With very high luck, should almost always succeed.
        let mut successes = 0;
        let mut rng = make_rng();
        for _ in 0..100 {
            if writing_succeeds(10, &mut rng) {
                successes += 1;
            }
        }
        // With luck=10, rn2(30) > 10 succeeds ~63% of the time.
        assert!(
            successes > 30,
            "expected many successes with luck 10, got {successes}"
        );

        // With very negative luck, should rarely succeed.
        let mut successes = 0;
        for _ in 0..100 {
            if writing_succeeds(-15, &mut rng) {
                successes += 1;
            }
        }
        // With luck=-15, rn2(5) > 10 never succeeds.
        assert_eq!(successes, 0, "expected no successes with luck -15");
    }
}
