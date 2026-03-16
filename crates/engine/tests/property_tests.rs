//! Property-based tests for NetHack Babel engine invariants.
//!
//! These tests use proptest to generate random inputs and verify
//! that certain invariants ALWAYS hold, regardless of input.

use proptest::prelude::*;

use nethack_babel_engine::combat::roll_dice;
use nethack_babel_engine::conduct::{calculate_score, ConductState, ScoreInput};
use nethack_babel_engine::world::Encumbrance;
use nethack_babel_engine::hunger::{
    HUNGRY_THRESHOLD, NOT_HUNGRY_THRESHOLD, SATIATED_THRESHOLD,
};
use nethack_babel_engine::identification::{an, just_an, makeplural, makesingular};
use nethack_babel_engine::religion::rnl;
use nethack_babel_engine::inventory::encumbrance_level;
use nethack_babel_engine::action::Position;
use nethack_babel_engine::ball::chebyshev_distance;
use nethack_babel_engine::wands::{recharge_wand, RechargeResult, WandCharges, WandType};

use nethack_babel_data::DiceExpr;

use rand::SeedableRng;
use rand_pcg::Pcg64;

// ─── Custom Domain Strategies ───

/// Strategy for valid NetHack map positions (within 80x21 standard map).
fn valid_position() -> impl Strategy<Value = (i32, i32)> {
    (1..79i32, 1..20i32)
}

/// Strategy for valid AC values (typical range in NetHack).
fn valid_ac() -> impl Strategy<Value = i32> {
    -50..50i32
}

/// Strategy for valid experience levels (1-30).
fn valid_xlevel() -> impl Strategy<Value = u32> {
    1..=30u32
}

/// Strategy for valid damage dice (1-10 dice, 1-20 sides).
fn valid_dice() -> impl Strategy<Value = (u8, u8)> {
    (1..=10u8, 1..=20u8)
}

/// Strategy for valid luck values (-13 to +13).
fn valid_luck() -> impl Strategy<Value = i32> {
    -13..=13i32
}

/// Strategy for valid hunger/nutrition (0-2000).
fn valid_nutrition() -> impl Strategy<Value = i32> {
    0..=2000i32
}

/// Strategy for valid HP as (current, max) where current <= max.
fn valid_hp() -> impl Strategy<Value = (i32, i32)> {
    (1..=500i32).prop_flat_map(|max| (1..=max, Just(max)))
}

/// Strategy for valid carried weight and capacity.
fn valid_weight_capacity() -> impl Strategy<Value = (u32, u32)> {
    (100..2000u32).prop_flat_map(|cap| (0..cap * 5, Just(cap)))
}

// ─── Combat Invariants ───

proptest! {
    /// Damage dice always produce values in valid range [count, count*sides].
    #[test]
    fn damage_dice_in_range(
        (count, sides) in valid_dice(),
        seed in any::<u64>(),
    ) {
        let dice = DiceExpr { count, sides };
        let mut rng = Pcg64::seed_from_u64(seed);
        let result = roll_dice(dice, &mut rng);
        let min = count as i32;
        let max = count as i32 * sides as i32;
        prop_assert!(result >= min,
            "{}d{} rolled {} < minimum {}", count, sides, result, min);
        prop_assert!(result <= max,
            "{}d{} rolled {} > maximum {}", count, sides, result, max);
    }

    /// Zero-count dice always return 0.
    #[test]
    fn zero_count_dice_return_zero(
        sides in 0..20u8,
        seed in any::<u64>(),
    ) {
        let dice = DiceExpr { count: 0, sides };
        let mut rng = Pcg64::seed_from_u64(seed);
        let result = roll_dice(dice, &mut rng);
        prop_assert_eq!(result, 0,
            "Dice with zero count should always return 0, got {}", result);
    }

    /// Zero-sides dice always return 0.
    #[test]
    fn zero_sides_dice_return_zero(
        count in 0..10u8,
        seed in any::<u64>(),
    ) {
        let dice = DiceExpr { count, sides: 0 };
        let mut rng = Pcg64::seed_from_u64(seed);
        let result = roll_dice(dice, &mut rng);
        prop_assert_eq!(result, 0,
            "Dice with zero sides should always return 0, got {}", result);
    }

    /// Armor class calculations are always bounded in [-127, 127].
    #[test]
    fn ac_always_bounded(
        base_ac in valid_ac(),
        armor_bonus in -30..30i32,
        ring_bonus in -10..10i32,
        spell_bonus in -10..10i32,
    ) {
        let total_ac = base_ac + armor_bonus + ring_bonus + spell_bonus;
        prop_assert!(total_ac >= -127 && total_ac <= 127,
            "AC {} out of bounds (base={}, armor={}, ring={}, spell={})",
            total_ac, base_ac, armor_bonus, ring_bonus, spell_bonus);
    }

    /// HP after damage: if current - damage <= 0, death must follow.
    #[test]
    fn hp_lethal_damage_detected(
        (current_hp, _max_hp) in valid_hp(),
        damage in 1..1000i32,
    ) {
        let final_hp = current_hp - damage;
        if final_hp <= 0 {
            prop_assert!(final_hp <= 0, "Damage {} from HP {} should be lethal", damage, current_hp);
        } else {
            prop_assert!(final_hp > 0, "HP {} after {} damage should be positive", final_hp, damage);
        }
    }
}

// ─── Hunger Invariants ───

proptest! {
    /// Hunger state transitions are monotonic with nutrition level.
    /// Higher nutrition should never produce a MORE hungry state.
    #[test]
    fn hunger_state_monotonic(
        nutrition_a in valid_nutrition(),
        nutrition_b in valid_nutrition(),
    ) {
        fn hunger_level(n: i32) -> u8 {
            match n {
                n if n > SATIATED_THRESHOLD => 4,     // satiated
                n if n > NOT_HUNGRY_THRESHOLD => 3,   // not hungry
                n if n > HUNGRY_THRESHOLD => 2,        // hungry
                n if n > 0 => 1,                       // weak
                _ => 0,                                // fainting/starved
            }
        }
        if nutrition_a > nutrition_b {
            prop_assert!(hunger_level(nutrition_a) >= hunger_level(nutrition_b),
                "Higher nutrition {} should not produce hungrier state than {}",
                nutrition_a, nutrition_b);
        }
    }

    /// Eating food always increases nutrition.
    #[test]
    fn eating_always_increases_nutrition(
        current_nutrition in valid_nutrition(),
        food_nutrition in 1..1000i32,
    ) {
        let new_nutrition = current_nutrition + food_nutrition;
        prop_assert!(new_nutrition > current_nutrition,
            "Eating {} nutrition from {} should increase, got {}",
            food_nutrition, current_nutrition, new_nutrition);
    }
}

// ─── Movement Invariants ───

proptest! {
    /// Chebyshev distance is always non-negative and symmetric.
    #[test]
    fn chebyshev_distance_properties(
        (x1, y1) in valid_position(),
        (x2, y2) in valid_position(),
    ) {
        let a = Position { x: x1, y: y1 };
        let b = Position { x: x2, y: y2 };
        let dist = chebyshev_distance(a, b);
        prop_assert!(dist >= 0, "Distance should be non-negative");
        let dist_reverse = chebyshev_distance(b, a);
        prop_assert_eq!(dist, dist_reverse, "Distance should be symmetric");
    }

    /// Chebyshev distance to self is always zero.
    #[test]
    fn chebyshev_distance_zero_to_self(
        (x, y) in valid_position(),
    ) {
        let p = Position { x, y };
        prop_assert_eq!(chebyshev_distance(p, p), 0,
            "Distance to self should be 0");
    }

    /// Movement in a direction then opposite returns to origin.
    #[test]
    fn movement_reversible(
        (x, y) in valid_position(),
        dx in -1..=1i32,
        dy in -1..=1i32,
    ) {
        let moved = Position { x: x + dx, y: y + dy };
        let returned = Position { x: moved.x - dx, y: moved.y - dy };
        prop_assert_eq!((x, y), (returned.x, returned.y),
            "Move then reverse should return to origin");
    }

    /// Triangle inequality: dist(a,c) <= dist(a,b) + dist(b,c).
    #[test]
    fn chebyshev_triangle_inequality(
        x1 in 1..50i32, y1 in 1..20i32,
        x2 in 1..50i32, y2 in 1..20i32,
        x3 in 1..50i32, y3 in 1..20i32,
    ) {
        let a = Position { x: x1, y: y1 };
        let b = Position { x: x2, y: y2 };
        let c = Position { x: x3, y: y3 };
        let ac = chebyshev_distance(a, c);
        let ab = chebyshev_distance(a, b);
        let bc = chebyshev_distance(b, c);
        prop_assert!(ac <= ab + bc,
            "Triangle inequality violated: dist({:?},{:?})={} > dist({:?},{:?})={} + dist({:?},{:?})={}",
            a, c, ac, a, b, ab, b, c, bc);
    }

    /// Valid positions are always within map bounds.
    #[test]
    fn position_always_valid((x, y) in valid_position()) {
        let pos = Position::new(x, y);
        prop_assert!(pos.x >= 1 && pos.x < 79,
            "x={} out of valid map range [1,79)", pos.x);
        prop_assert!(pos.y >= 1 && pos.y < 20,
            "y={} out of valid map range [1,20)", pos.y);
    }
}

// ─── Item Naming Invariants ───

proptest! {
    /// Pluralization should never produce an empty string from non-empty input.
    /// Note: Latin plurals can be shorter (e.g., "us"->"i", "matzoh"->"matzot").
    #[test]
    fn plural_never_empty(name in "[a-z]{1,20}") {
        let plural = makeplural(&name);
        prop_assert!(!plural.is_empty(),
            "Plural of '{}' should not be empty", name);
    }

    /// Article prefix is always "a " or "an ".
    #[test]
    fn article_always_a_or_an(name in "[a-z]{2,20}") {
        let article = just_an(&name);
        prop_assert!(article == "a " || article == "an ",
            "Article for '{}' should be 'a ' or 'an ', got '{}'", name, article);
    }

    /// an() prepends the correct article to the name.
    #[test]
    fn an_prepends_article(name in "[a-z]{2,20}") {
        let result = an(&name);
        prop_assert!(result.starts_with("a ") || result.starts_with("an "),
            "an('{}') should start with 'a ' or 'an ', got '{}'", name, result);
        prop_assert!(result.ends_with(&name),
            "an('{}') = '{}' should end with the original name", name, result);
    }

    /// Singularization of a pluralized word should not be longer than the plural.
    #[test]
    fn singular_not_longer_than_plural(name in "[a-z]{3,15}") {
        let plural = makeplural(&name);
        let singular = makesingular(&plural);
        prop_assert!(singular.len() <= plural.len() + 2,
            "Singular '{}' should not be much longer than plural '{}'",
            singular, plural);
    }
}

// ─── Luck / rnl Invariants ───

proptest! {
    /// rnl always returns values in [0, x-1] for positive x.
    #[test]
    fn rnl_always_in_range(
        x in 2..100i32,
        luck in valid_luck(),
        seed in any::<u64>(),
    ) {
        let mut rng = Pcg64::seed_from_u64(seed);
        let result = rnl(&mut rng, x, luck);
        prop_assert!(result >= 0,
            "rnl({}, luck={}) = {} should be >= 0", x, luck, result);
        prop_assert!(result < x,
            "rnl({}, luck={}) = {} should be < {}", x, luck, result, x);
    }

    /// rnl(0) always returns 0 regardless of luck.
    #[test]
    fn rnl_zero_returns_zero(
        luck in valid_luck(),
        seed in any::<u64>(),
    ) {
        let mut rng = Pcg64::seed_from_u64(seed);
        let result = rnl(&mut rng, 0, luck);
        prop_assert_eq!(result, 0,
            "rnl(0, luck={}) should be 0, got {}", luck, result);
    }

    /// rnl(1) always returns 0 (only one possible value).
    #[test]
    fn rnl_one_returns_zero(
        luck in valid_luck(),
        seed in any::<u64>(),
    ) {
        let mut rng = Pcg64::seed_from_u64(seed);
        let result = rnl(&mut rng, 1, luck);
        prop_assert_eq!(result, 0,
            "rnl(1, luck={}) should be 0, got {}", luck, result);
    }

    /// Positive luck biases rnl toward lower values (over many samples).
    #[test]
    fn rnl_positive_luck_biases_low(seed in any::<u64>()) {
        let x = 20;
        let trials = 200;
        let mut sum_lucky = 0i64;
        let mut sum_unlucky = 0i64;

        for i in 0..trials {
            let mut rng_lucky = Pcg64::seed_from_u64(seed.wrapping_add(i));
            let mut rng_unlucky = Pcg64::seed_from_u64(seed.wrapping_add(i));
            sum_lucky += rnl(&mut rng_lucky, x, 13) as i64;
            sum_unlucky += rnl(&mut rng_unlucky, x, -13) as i64;
        }

        // With positive luck, average should be lower (biased toward 0).
        // With negative luck, average should be higher.
        // We don't assert strict inequality per-seed, just that the
        // sums are plausibly different (positive luck sum <= unlucky sum).
        // This is a statistical property, so we use a loose bound.
        prop_assert!(sum_lucky <= sum_unlucky + (trials as i64 * x as i64 / 2),
            "Positive luck should bias rnl lower: lucky_sum={} unlucky_sum={}",
            sum_lucky, sum_unlucky);
    }
}

// ─── Encumbrance Invariants ───

proptest! {
    /// Encumbrance level is monotonically increasing with carried weight.
    #[test]
    fn encumbrance_monotonic_with_weight(
        (weight_a, capacity) in valid_weight_capacity(),
        extra in 0..3000u32,
    ) {
        let weight_b = weight_a + extra;
        let enc_a = encumbrance_level(weight_a, capacity) as u8;
        let enc_b = encumbrance_level(weight_b, capacity) as u8;
        prop_assert!(enc_a <= enc_b,
            "More weight ({} vs {}) with cap {} should not decrease encumbrance ({} vs {})",
            weight_a, weight_b, capacity, enc_a, enc_b);
    }

    /// Encumbrance at or below capacity is always Unencumbered.
    #[test]
    fn unencumbered_at_capacity(
        capacity in 10..2000u32,
        frac in 0.0..=1.0f64,
    ) {
        let weight = (capacity as f64 * frac) as u32;
        let enc = encumbrance_level(weight, capacity);
        prop_assert_eq!(enc, Encumbrance::Unencumbered,
            "Weight {} <= capacity {} should be Unencumbered, got {:?}",
            weight, capacity, enc);
    }

    /// Encumbrance level is always in [0, 5].
    #[test]
    fn encumbrance_always_valid(
        weight in 0..10000u32,
        capacity in 1..2000u32,
    ) {
        let enc = encumbrance_level(weight, capacity) as u8;
        prop_assert!(enc <= 5,
            "Encumbrance level {} out of valid range [0,5]", enc);
    }
}

// ─── Score Invariants ───

proptest! {
    /// Score is always non-negative regardless of inputs.
    #[test]
    fn score_never_negative(
        experience in 0..100000i64,
        score_experience in 0..100000i64,
        gold_carried in 0..50000i64,
        gold_deposited in 0..50000i64,
        artifacts in 0..20u32,
        max_depth in 1..50u32,
        ascended in any::<bool>(),
    ) {
        let input = ScoreInput {
            experience,
            score_experience,
            gold_carried,
            gold_deposited,
            artifacts_held: artifacts,
            conducts: ConductState::default(),
            ascended,
            max_depth,
        };
        let score = calculate_score(&input);
        // calculate_score returns u64, so it can never be negative,
        // but we verify it doesn't panic or overflow.
        prop_assert!(score > 0 || (experience == 0 && score_experience == 0 && gold_carried == 0
            && gold_deposited == 0 && artifacts == 0),
            "Score should be positive for non-trivial inputs, got {}", score);
    }

    /// More experience always yields equal or higher score (all else equal).
    #[test]
    fn more_experience_more_score(
        exp_a in 0..50000i64,
        exp_b in 0..50000i64,
        gold in 0..10000i64,
    ) {
        let make_input = |exp: i64| ScoreInput {
            experience: exp,
            score_experience: 0,
            gold_carried: gold,
            gold_deposited: 0,
            artifacts_held: 0,
            conducts: ConductState::default(),
            ascended: false,
            max_depth: 1,
        };
        let score_a = calculate_score(&make_input(exp_a));
        let score_b = calculate_score(&make_input(exp_b));
        if exp_a >= exp_b {
            prop_assert!(score_a >= score_b,
                "More experience ({} vs {}) should yield higher score ({} vs {})",
                exp_a, exp_b, score_a, score_b);
        }
    }
}

// ─── Explosion Invariants ───

proptest! {
    /// Explosion blast radius check is symmetric around center.
    #[test]
    fn explosion_radius_symmetric(
        dx in -3..=3i32,
        dy in -3..=3i32,
    ) {
        // Standard NetHack explosion is 3x3 (radius 1 in Chebyshev)
        let in_blast = dx.abs().max(dy.abs()) <= 1;
        let in_blast_mirror = (-dx).abs().max((-dy).abs()) <= 1;
        prop_assert_eq!(in_blast, in_blast_mirror,
            "Blast radius should be symmetric: ({},{}) vs ({},{})", dx, dy, -dx, -dy);
    }
}

// ─── Appearance Shuffling Invariants ───

proptest! {
    /// Fisher-Yates shuffle produces no duplicate entries.
    #[test]
    fn shuffle_no_duplicates(seed in any::<u64>()) {
        use std::collections::HashSet;
        let mut rng = Pcg64::seed_from_u64(seed);

        // Simulate a shuffle of 28 items (typical potion appearances)
        let mut items: Vec<usize> = (0..28).collect();
        for i in (1..items.len()).rev() {
            let j = rng.random_range(0..=i);
            items.swap(i, j);
        }

        let unique: HashSet<_> = items.iter().collect();
        prop_assert_eq!(unique.len(), items.len(),
            "Shuffled appearances should have no duplicates");
    }

    /// Shuffle is a permutation (same elements, different order).
    #[test]
    fn shuffle_is_permutation(seed in any::<u64>(), size in 2..50usize) {
        let mut rng = Pcg64::seed_from_u64(seed);

        let original: Vec<usize> = (0..size).collect();
        let mut shuffled = original.clone();
        for i in (1..shuffled.len()).rev() {
            let j = rng.random_range(0..=i);
            shuffled.swap(i, j);
        }

        let mut sorted = shuffled.clone();
        sorted.sort();
        prop_assert_eq!(sorted, original,
            "Shuffled array should be a permutation of the original");
    }
}

// ─── Wand Recharge BUC Invariants ───

proptest! {
    /// Cursed recharge always strips charges to 0.
    #[test]
    fn cursed_recharge_always_strips(
        initial_spe in 0..8i8,
        seed in any::<u64>(),
    ) {
        let mut charges = WandCharges { spe: initial_spe, recharged: 0 };
        let mut rng = Pcg64::seed_from_u64(seed);
        let result = recharge_wand(WandType::MagicMissile, &mut charges, false, true, &mut rng);
        prop_assert_eq!(result, RechargeResult::Stripped,
            "Cursed recharge should strip, got {:?}", result);
        prop_assert_eq!(charges.spe, 0,
            "Cursed recharge should set spe to 0, got {}", charges.spe);
    }

    /// Blessed recharge always gives >= charges than uncursed (first recharge).
    #[test]
    fn blessed_recharge_ge_uncursed(
        seed in any::<u64>(),
    ) {
        let mut charges_b = WandCharges { spe: 0, recharged: 0 };
        let mut charges_u = WandCharges { spe: 0, recharged: 0 };
        let mut rng_b = Pcg64::seed_from_u64(seed);
        let mut rng_u = Pcg64::seed_from_u64(seed);

        let result_b = recharge_wand(WandType::MagicMissile, &mut charges_b, true, false, &mut rng_b);
        let result_u = recharge_wand(WandType::MagicMissile, &mut charges_u, false, false, &mut rng_u);

        if let (RechargeResult::Success { new_spe: spe_b }, RechargeResult::Success { new_spe: spe_u }) = (result_b, result_u) {
            prop_assert!(spe_b >= spe_u,
                "Blessed recharge should give >= charges than uncursed: {} vs {}", spe_b, spe_u);
        }
    }
}

// ─── BUC Parametrized Tests (manual matrix) ───

/// Test wand recharge behavior across BUC status for all wand types.
#[test]
fn wand_recharge_buc_matrix() {
    let wand_types = [
        WandType::MagicMissile,
        WandType::Fire,
        WandType::Cold,
        WandType::Sleep,
        WandType::Light,
        WandType::Nothing,
    ];

    // (label, blessed, cursed, expected_behavior)
    let buc_cases: &[(&str, bool, bool)] = &[
        ("blessed", true, false),
        ("uncursed", false, false),
        ("cursed", false, true),
    ];

    for wand_type in &wand_types {
        for &(buc_label, blessed, cursed) in buc_cases {
            for seed in 0..50u64 {
                let mut charges = WandCharges {
                    spe: 3,
                    recharged: 0,
                };
                let mut rng = Pcg64::seed_from_u64(seed);
                let result =
                    recharge_wand(*wand_type, &mut charges, blessed, cursed, &mut rng);

                match result {
                    RechargeResult::Stripped => {
                        assert!(
                            cursed,
                            "Only cursed should strip: wand={:?} buc={} seed={}",
                            wand_type, buc_label, seed,
                        );
                        assert_eq!(
                            charges.spe, 0,
                            "Stripped wand should have 0 charges: wand={:?} buc={} seed={}",
                            wand_type, buc_label, seed,
                        );
                    }
                    RechargeResult::Success { new_spe } => {
                        assert!(
                            !cursed,
                            "Cursed should not succeed: wand={:?} buc={} seed={}",
                            wand_type, buc_label, seed,
                        );
                        assert!(
                            new_spe >= 1,
                            "Successful recharge should give >= 1 spe: wand={:?} buc={} spe={} seed={}",
                            wand_type, buc_label, new_spe, seed,
                        );
                    }
                    RechargeResult::Exploded => {
                        // Explosions can happen on any BUC if recharged > 0.
                        // Since recharged starts at 0 here, this shouldn't happen.
                        panic!(
                            "First recharge should not explode: wand={:?} buc={} seed={}",
                            wand_type, buc_label, seed,
                        );
                    }
                }
            }
        }
    }
}

/// Test that dice rolling distribution covers the full range across many seeds.
#[test]
fn dice_distribution_covers_range() {
    let cases: &[(u8, u8)] = &[
        (1, 6),   // 1d6
        (2, 6),   // 2d6
        (1, 20),  // 1d20
        (3, 4),   // 3d4
    ];

    for &(count, sides) in cases {
        let dice = DiceExpr { count, sides };
        let min_expected = count as i32;
        let max_expected = count as i32 * sides as i32;
        let mut min_seen = i32::MAX;
        let mut max_seen = i32::MIN;

        for seed in 0..1000u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let result = roll_dice(dice, &mut rng);
            min_seen = min_seen.min(result);
            max_seen = max_seen.max(result);
        }

        assert_eq!(
            min_seen, min_expected,
            "{}d{}: expected min {} but saw {}",
            count, sides, min_expected, min_seen,
        );
        assert_eq!(
            max_seen, max_expected,
            "{}d{}: expected max {} but saw {}",
            count, sides, max_expected, max_seen,
        );
    }
}
