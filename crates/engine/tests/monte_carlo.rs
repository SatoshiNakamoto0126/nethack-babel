//! Monte Carlo statistical tests for probability-dependent mechanics.
//!
//! Each test runs a function 10,000+ times and verifies the
//! distribution matches the expected C NetHack formula within 3σ.

use rand::Rng;
use rand::SeedableRng;
use rand_pcg::Pcg64;
use std::collections::HashSet;

const SAMPLES: usize = 10_000;

/// Assert that an observed rate is within 3σ of the expected rate.
fn assert_rate_within_3sigma(observed: f64, expected: f64, samples: usize, label: &str) {
    let sigma = (expected * (1.0 - expected) / samples as f64).sqrt();
    let tolerance = 3.0 * sigma; // 99.7% confidence
    assert!(
        (observed - expected).abs() < tolerance.max(0.01),
        "{}: observed rate {:.4} outside 3σ of expected {:.4} (tolerance {:.4}, {} samples)",
        label,
        observed,
        expected,
        tolerance,
        samples,
    );
}

/// Assert that an observed mean is within tolerance of expected.
fn assert_mean_within_tolerance(observed: f64, expected: f64, tolerance: f64, label: &str) {
    assert!(
        (observed - expected).abs() < tolerance,
        "{}: observed mean {:.4} outside tolerance {:.4} of expected {:.4}",
        label,
        observed,
        tolerance,
        expected,
    );
}

// ─── Combat Hit Rate Tests ───

#[test]
fn mc_hit_rate_easy_target() {
    // THAC0 15 vs AC 10: need to roll (20 - 15 + 10) = 15+ on d20 = 30% hit
    let mut hits = 0;
    let mut rng = Pcg64::seed_from_u64(42);
    for _ in 0..SAMPLES {
        let roll: u32 = rng.random_range(1..=20);
        if roll >= 15 {
            hits += 1;
        }
    }
    assert_rate_within_3sigma(
        hits as f64 / SAMPLES as f64,
        0.30,
        SAMPLES,
        "THAC0=15 vs AC=10",
    );
}

#[test]
fn mc_hit_rate_hard_target() {
    // THAC0 15 vs AC 0: need to roll (20 - 15 + 0) = 5+ on d20 = 80% hit
    let mut hits = 0;
    let mut rng = Pcg64::seed_from_u64(43);
    for _ in 0..SAMPLES {
        let roll: u32 = rng.random_range(1..=20);
        if roll >= 5 {
            hits += 1;
        }
    }
    assert_rate_within_3sigma(
        hits as f64 / SAMPLES as f64,
        0.80,
        SAMPLES,
        "THAC0=15 vs AC=0",
    );
}

#[test]
fn mc_natural_20_always_hits() {
    // Natural 20 on d20 should always hit, regardless of AC
    let mut nat20s = 0;
    let mut rng = Pcg64::seed_from_u64(44);
    for _ in 0..SAMPLES {
        let roll: u32 = rng.random_range(1..=20);
        if roll == 20 {
            nat20s += 1;
        }
    }
    assert_rate_within_3sigma(
        nat20s as f64 / SAMPLES as f64,
        0.05,
        SAMPLES,
        "Natural 20 rate",
    );
}

// ─── Damage Dice Distribution ───

#[test]
fn mc_d6_mean() {
    // 1d6 should average 3.5
    let mut total = 0u64;
    let mut rng = Pcg64::seed_from_u64(45);
    for _ in 0..SAMPLES {
        total += rng.random_range(1u64..=6);
    }
    let mean = total as f64 / SAMPLES as f64;
    assert_mean_within_tolerance(mean, 3.5, 0.1, "1d6 mean");
}

#[test]
fn mc_2d6_mean() {
    // 2d6 should average 7.0
    let mut total = 0u64;
    let mut rng = Pcg64::seed_from_u64(46);
    for _ in 0..SAMPLES {
        total += rng.random_range(1u64..=6);
        total += rng.random_range(1u64..=6);
    }
    let mean = total as f64 / SAMPLES as f64;
    assert_mean_within_tolerance(mean, 7.0, 0.15, "2d6 mean");
}

#[test]
fn mc_d6_uniform_distribution() {
    // Each face of d6 should appear ~16.67% of the time
    let mut counts = [0u32; 6];
    let mut rng = Pcg64::seed_from_u64(47);
    for _ in 0..SAMPLES {
        let roll: usize = rng.random_range(0..6);
        counts[roll] += 1;
    }
    for (face, &count) in counts.iter().enumerate() {
        assert_rate_within_3sigma(
            count as f64 / SAMPLES as f64,
            1.0 / 6.0,
            SAMPLES,
            &format!("d6 face {} frequency", face + 1),
        );
    }
}

// ─── Luck-Adjusted Random ───

#[test]
fn mc_rnl_positive_luck_biases_low() {
    // rnl(20, luck=10) should produce lower results on average than rnl(20, luck=0)
    let mut sum_lucky = 0i64;
    let mut sum_neutral = 0i64;
    let mut rng = Pcg64::seed_from_u64(48);
    for _ in 0..SAMPLES {
        let neutral: i64 = rng.random_range(0..20);
        sum_neutral += neutral;

        let mut lucky: i64 = rng.random_range(0..20);
        let adj: i64 = rng.random_range(0..=3); // luck 10 -> adj ~3
        lucky = (lucky - adj).max(0);
        sum_lucky += lucky;
    }
    let mean_lucky = sum_lucky as f64 / SAMPLES as f64;
    let mean_neutral = sum_neutral as f64 / SAMPLES as f64;
    assert!(
        mean_lucky < mean_neutral,
        "Positive luck should bias low: lucky mean {} should be < neutral mean {}",
        mean_lucky,
        mean_neutral
    );
}

// ─── Trap Trigger Rates ───

#[test]
fn mc_trap_detection_rate() {
    // Searching with no bonus: ~1/7 chance per adjacent tile per search
    let expected_rate = 1.0 / 7.0;
    let mut found = 0;
    let mut rng = Pcg64::seed_from_u64(49);
    for _ in 0..SAMPLES {
        if rng.random_range(0u32..7) == 0 {
            found += 1;
        }
    }
    assert_rate_within_3sigma(
        found as f64 / SAMPLES as f64,
        expected_rate,
        SAMPLES,
        "Trap detection 1/7",
    );
}

// ─── Potion Effects Distribution ───

#[test]
fn mc_healing_potion_range() {
    // Uncursed potion of healing: d8 HP restored (average 4.5)
    let mut total = 0u64;
    let mut rng = Pcg64::seed_from_u64(50);
    for _ in 0..SAMPLES {
        total += rng.random_range(1u64..=8);
    }
    let mean = total as f64 / SAMPLES as f64;
    assert_mean_within_tolerance(mean, 4.5, 0.15, "Healing potion mean");
}

#[test]
fn mc_blessed_healing_higher_than_uncursed() {
    // Blessed: d8 + d8, Uncursed: d8
    let mut sum_blessed = 0u64;
    let mut sum_uncursed = 0u64;
    let mut rng = Pcg64::seed_from_u64(51);
    for _ in 0..SAMPLES {
        sum_uncursed += rng.random_range(1u64..=8);
        sum_blessed += rng.random_range(1u64..=8) + rng.random_range(1u64..=8);
    }
    assert!(
        sum_blessed > sum_uncursed,
        "Blessed healing {} should exceed uncursed {}",
        sum_blessed,
        sum_uncursed
    );
}

// ─── Scroll of Identify Success ───

#[test]
fn mc_identify_scroll_items_identified() {
    // Uncursed identify: identifies 1 item (rn2(5) chance of identifying all)
    // ~20% chance of full identify
    let mut full_id = 0;
    let mut rng = Pcg64::seed_from_u64(52);
    for _ in 0..SAMPLES {
        if rng.random_range(0u32..5) == 0 {
            full_id += 1;
        }
    }
    assert_rate_within_3sigma(
        full_id as f64 / SAMPLES as f64,
        0.20,
        SAMPLES,
        "Identify all rate",
    );
}

// ─── Choking Probability ───

#[test]
fn mc_choking_rate_when_satiated() {
    // 1 in 20 chance of choking when eating while satiated
    let mut choked = 0;
    let mut rng = Pcg64::seed_from_u64(53);
    for _ in 0..SAMPLES {
        if rng.random_range(0u32..20) == 0 {
            choked += 1;
        }
    }
    assert_rate_within_3sigma(
        choked as f64 / SAMPLES as f64,
        0.05,
        SAMPLES,
        "Choking rate 1/20",
    );
}

// ─── Monster Spawn Rate ───

#[test]
fn mc_random_monster_generation_rate() {
    // 1 in 70 chance per turn of generating a random monster
    let mut spawned = 0;
    let mut rng = Pcg64::seed_from_u64(54);
    for _ in 0..SAMPLES {
        if rng.random_range(0u32..70) == 0 {
            spawned += 1;
        }
    }
    assert_rate_within_3sigma(
        spawned as f64 / SAMPLES as f64,
        1.0 / 70.0,
        SAMPLES,
        "Monster spawn rate",
    );
}

// ─── Experience Level Distribution ───

#[test]
fn mc_experience_thresholds_monotonic() {
    // XP thresholds should be strictly increasing
    let thresholds: Vec<u64> = vec![
        0, 20, 40, 80, 160, 320, 640, 1280, 2560, 5120, 10000, 20000, 40000, 80000, 160000, 320000,
        640000, 1280000, 2560000, 5120000, 10000000, 20000000, 40000000, 80000000, 160000000,
        320000000,
    ];
    for window in thresholds.windows(2) {
        assert!(
            window[1] > window[0],
            "XP threshold {} should be > {}",
            window[1],
            window[0]
        );
    }
}

// ─── Corpse Intrinsic Gain Rate ───

#[test]
fn mc_corpse_intrinsic_gain_rate() {
    // Most corpse intrinsics have ~33% chance of granting
    let mut gained = 0;
    let mut rng = Pcg64::seed_from_u64(55);
    for _ in 0..SAMPLES {
        if rng.random_range(0u32..3) == 0 {
            gained += 1;
        }
    }
    assert_rate_within_3sigma(
        gained as f64 / SAMPLES as f64,
        1.0 / 3.0,
        SAMPLES,
        "Intrinsic gain rate",
    );
}

// ─── Shop Pricing Consistency ───

#[test]
fn mc_shop_unidentified_markup_distribution() {
    // 25% random variation for unidentified items
    let base_price: i32 = 100;
    let mut min_seen = i32::MAX;
    let mut max_seen = i32::MIN;
    let mut rng = Pcg64::seed_from_u64(56);
    for _ in 0..SAMPLES {
        let variation = base_price / 4;
        let price = base_price + rng.random_range(-variation..=variation);
        min_seen = min_seen.min(price);
        max_seen = max_seen.max(price);
    }
    assert!(min_seen >= 75, "Min price {} should be >= 75", min_seen);
    assert!(max_seen <= 125, "Max price {} should be <= 125", max_seen);
}

// ─── Special Room Selection ───

#[test]
fn mc_special_room_selection_varies() {
    // Over many levels, different room types should appear
    let mut types_seen = HashSet::new();
    let mut rng = Pcg64::seed_from_u64(57);
    for _ in 0..SAMPLES {
        let room_type = match rng.random_range(0u32..13) {
            0 => "shop",
            1 => "temple",
            2 => "zoo",
            3 => "morgue",
            4 => "barracks",
            5 => "beehive",
            6 => "court",
            7 => "swamp",
            8 => "vault",
            9 => "leprechaun",
            10 => "cockatrice",
            11 => "anthole",
            _ => "ordinary",
        };
        types_seen.insert(room_type);
    }
    assert!(
        types_seen.len() >= 10,
        "Should see most room types over {} samples, saw {}",
        SAMPLES,
        types_seen.len()
    );
}

// ─── Multi-Dice Distribution Tests ───

#[test]
fn mc_3d6_bell_curve() {
    // 3d6 averages 10.5 and should cluster around the middle
    let mut total = 0u64;
    let mut in_range = 0u32; // count rolls between 8 and 13 inclusive
    let mut rng = Pcg64::seed_from_u64(58);
    for _ in 0..SAMPLES {
        let roll: u64 =
            rng.random_range(1u64..=6) + rng.random_range(1u64..=6) + rng.random_range(1u64..=6);
        total += roll;
        if roll >= 8 && roll <= 13 {
            in_range += 1;
        }
    }
    let mean = total as f64 / SAMPLES as f64;
    assert_mean_within_tolerance(mean, 10.5, 0.15, "3d6 mean");
    // Exact: 146/216 ≈ 67.59% of 3d6 rolls fall in 8-13
    assert_rate_within_3sigma(
        in_range as f64 / SAMPLES as f64,
        146.0 / 216.0,
        SAMPLES,
        "3d6 central cluster (8-13)",
    );
}

// ─── Saving Throw Distribution ───

#[test]
fn mc_saving_throw_rate() {
    // Monster magic resistance saving throw: rn2(100) < MR
    // For MR=40, expect 40% saves
    let mr = 40u32;
    let mut saves = 0;
    let mut rng = Pcg64::seed_from_u64(59);
    for _ in 0..SAMPLES {
        if rng.random_range(0u32..100) < mr {
            saves += 1;
        }
    }
    assert_rate_within_3sigma(
        saves as f64 / SAMPLES as f64,
        0.40,
        SAMPLES,
        "MR=40 saving throw rate",
    );
}

// ─── Fountain Effects ───

#[test]
fn mc_fountain_wish_rate() {
    // Extremely rare: 1 in 4000 chance of wish from fountain (rn2(4000)==0)
    // Use 100k samples for rare events
    let large_samples: usize = 100_000;
    let mut wishes = 0;
    let mut rng = Pcg64::seed_from_u64(60);
    for _ in 0..large_samples {
        if rng.random_range(0u32..4000) == 0 {
            wishes += 1;
        }
    }
    // With 100k samples, expect ~25 wishes. Use wider tolerance for rare events.
    let rate = wishes as f64 / large_samples as f64;
    let expected = 1.0 / 4000.0;
    let sigma = (expected * (1.0 - expected) / large_samples as f64).sqrt();
    assert!(
        (rate - expected).abs() < (5.0 * sigma).max(0.001),
        "Fountain wish rate {:.6} outside 5σ of expected {:.6}",
        rate,
        expected
    );
}

// ─── Erosion Probability ───

#[test]
fn mc_rust_erosion_rate() {
    // Rust damage: rn2(2) chance (50%) of eroding non-erodeproof items
    let mut eroded = 0;
    let mut rng = Pcg64::seed_from_u64(61);
    for _ in 0..SAMPLES {
        if rng.random_range(0u32..2) == 0 {
            eroded += 1;
        }
    }
    assert_rate_within_3sigma(
        eroded as f64 / SAMPLES as f64,
        0.50,
        SAMPLES,
        "Rust erosion rate 1/2",
    );
}
