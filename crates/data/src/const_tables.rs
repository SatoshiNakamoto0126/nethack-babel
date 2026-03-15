//! Compile-time constant lookup tables for critical game data.
//!
//! These tables replace runtime match chains and TOML lookups for the most
//! performance-sensitive game data (combat bonuses, experience thresholds,
//! terrain properties).  All tables are validated at compile time via
//! const assertions.
//!
//! ## Strength encoding
//!
//! NetHack encodes strength as a single index 0..=125:
//! - Index 0..=18: raw STR value (0 through 18 with no exceptional)
//! - Index 19..=118: STR 18/01 through 18/100  (index = 18 + extra)
//! - Index 119..=125: STR 19 through 25 (gauntlets of power, etc.)
//!
//! The `encode_strength` helper converts the (strength, strength_extra)
//! pair used elsewhere in the codebase into this flat index.

// ---------------------------------------------------------------------------
// Strength bonus tables
// ---------------------------------------------------------------------------

/// Encode a (strength, strength_extra) pair into a flat index 0..=125
/// for use with [`STR_TO_HIT`] and [`STR_DAMAGE`].
///
/// - strength 0..=17: index = strength
/// - strength 18, extra 0: index = 18
/// - strength 18, extra 1..=100: index = 18 + extra  (19..=118)
/// - strength 19..=25: index = 100 + strength  (119..=125)
/// - strength > 25: clamped to index 125
#[inline]
pub const fn encode_strength(strength: u8, strength_extra: u8) -> usize {
    if strength < 18 {
        strength as usize
    } else if strength == 18 {
        if strength_extra == 0 {
            18
        } else if strength_extra <= 100 {
            18 + strength_extra as usize
        } else {
            // Extra > 100 treated as 18/100
            118
        }
    } else {
        // STR 19..=25 maps to indices 119..=125; clamp above 25
        let clamped = if strength > 25 { 25 } else { strength };
        100 + clamped as usize
    }
}

/// STR to-hit bonus table, indexed by encoded strength value.
/// Matches NetHack's `sbon()` exactly (weapon.c:950-984).
///
/// Use [`encode_strength`] to convert (strength, strength_extra) to an index.
pub const STR_TO_HIT: [i8; 126] = compute_str_to_hit();

const fn compute_str_to_hit() -> [i8; 126] {
    let mut table = [0i8; 126];
    let mut i: usize = 0;
    while i < 126 {
        table[i] = str_to_hit_for_index(i);
        i += 1;
    }
    table
}

const fn str_to_hit_for_index(idx: usize) -> i8 {
    // Decode back to (str, extra) semantics for clarity
    if idx <= 5 {
        -2 // STR 0..=5
    } else if idx <= 7 {
        -1 // STR 6..=7
    } else if idx <= 16 {
        0 // STR 8..=16
    } else if idx == 17 {
        1 // STR 17
    } else if idx == 18 {
        // STR 18/00
        1
    } else if idx <= 68 {
        // STR 18/01..18/50 (indices 19..=68)
        1
    } else if idx <= 117 {
        // STR 18/51..18/99 (indices 69..=117)
        2
    } else {
        // STR 18/100 (index 118) or STR 19+ (indices 119..=125)
        3
    }
}

/// STR damage bonus table, indexed by encoded strength value.
/// Matches NetHack's `dbon()` exactly (weapon.c:988-1011).
///
/// Use [`encode_strength`] to convert (strength, strength_extra) to an index.
pub const STR_DAMAGE: [i8; 126] = compute_str_damage();

const fn compute_str_damage() -> [i8; 126] {
    let mut table = [0i8; 126];
    let mut i: usize = 0;
    while i < 126 {
        table[i] = str_damage_for_index(i);
        i += 1;
    }
    table
}

const fn str_damage_for_index(idx: usize) -> i8 {
    if idx <= 5 {
        -1 // STR 0..=5
    } else if idx <= 15 {
        0 // STR 6..=15
    } else if idx <= 17 {
        1 // STR 16..=17
    } else if idx == 18 {
        // STR 18/00 (exactly 18, no exceptional)
        2
    } else if idx <= 68 {
        // STR 18/01..18/50 (indices 19..=68)
        3
    } else if idx <= 93 {
        // STR 18/51..18/75 (indices 69..=93)
        4
    } else if idx <= 108 {
        // STR 18/76..18/90 (indices 94..=108)
        5
    } else if idx <= 117 {
        // STR 18/91..18/99 (indices 109..=117)
        6
    } else if idx == 118 {
        // STR 18/100 (**)
        7
    } else {
        // STR 19+ (indices 119..=125)
        6
    }
}

// ---------------------------------------------------------------------------
// Experience level thresholds
// ---------------------------------------------------------------------------

/// XP thresholds for levels 0..=29.
///
/// `XP_THRESHOLDS[lev]` is the cumulative XP needed to advance *past*
/// level `lev` (i.e. `newuexp(lev)`).  When `u.uexp >= XP_THRESHOLDS[u.ulevel]`,
/// the player gains a level.
///
/// Formula from exper.c `newuexp()`:
/// - lev < 1:  0
/// - lev < 10: 10 * 2^lev
/// - lev < 20: 10_000 * 2^(lev - 10)
/// - lev >= 20: 10_000_000 * (lev - 19)
pub const XP_THRESHOLDS: [i64; 30] = compute_xp_thresholds();

const fn compute_xp_thresholds() -> [i64; 30] {
    let mut table = [0i64; 30];
    let mut i: usize = 0;
    while i < 30 {
        table[i] = newuexp(i);
        i += 1;
    }
    table
}

/// Compute `newuexp(lev)` at compile time.
const fn newuexp(lev: usize) -> i64 {
    if lev < 1 {
        0
    } else if lev < 10 {
        // 10 * 2^lev
        10 * (1i64 << lev)
    } else if lev < 20 {
        // 10_000 * 2^(lev - 10)
        10_000 * (1i64 << (lev - 10))
    } else {
        // 10_000_000 * (lev - 19)
        10_000_000 * (lev as i64 - 19)
    }
}

// ---------------------------------------------------------------------------
// Terrain property lookups
// ---------------------------------------------------------------------------

/// Whether a terrain type (as `u8` discriminant) is passable by walking.
///
/// Passable terrains: Door (23), Corridor (24), Room (25), Stairs (26),
/// Ladder (27), Fountain (28), Throne (29), Sink (30), Grave (31),
/// Altar (32), Ice (33), DrawbridgeDown (34), Air (35), Cloud (36).
///
/// Note: Pool/Moat/Water/LavaPool are *not* passable by normal walking
/// (require levitation, water walking, etc.).
pub const fn is_terrain_passable(t: u8) -> bool {
    // Use a 37-element lookup table (one bool per terrain value)
    const TABLE: [bool; 37] = compute_passable_table();
    if t < 37 { TABLE[t as usize] } else { false }
}

const fn compute_passable_table() -> [bool; 37] {
    let mut table = [false; 37];
    // Door (23) - passable when open/unlocked (conservatively true)
    table[23] = true;
    // Corridor (24)
    table[24] = true;
    // Room (25)
    table[25] = true;
    // Stairs (26)
    table[26] = true;
    // Ladder (27)
    table[27] = true;
    // Fountain (28)
    table[28] = true;
    // Throne (29)
    table[29] = true;
    // Sink (30)
    table[30] = true;
    // Grave (31)
    table[31] = true;
    // Altar (32)
    table[32] = true;
    // Ice (33)
    table[33] = true;
    // DrawbridgeDown (34)
    table[34] = true;
    // Air (35)
    table[35] = true;
    // Cloud (36)
    table[36] = true;
    table
}

/// Whether a terrain type (as `u8` discriminant) blocks line of sight.
///
/// Opaque terrains: Stone (0), all Wall variants (1..=12), Tree (13),
/// SecretDoor (14), SecretCorridor (15), DrawbridgeUp (19),
/// LavaWall (21), IronBars (22 — blocks movement but see-through,
/// so NOT opaque), Cloud (36 — treated as opaque for FOV).
pub const fn is_terrain_opaque(t: u8) -> bool {
    const TABLE: [bool; 37] = compute_opaque_table();
    if t < 37 { TABLE[t as usize] } else { true }
}

const fn compute_opaque_table() -> [bool; 37] {
    let mut table = [false; 37];
    // Stone (0)
    table[0] = true;
    // Walls (1..=12)
    table[1] = true;
    table[2] = true;
    table[3] = true;
    table[4] = true;
    table[5] = true;
    table[6] = true;
    table[7] = true;
    table[8] = true;
    table[9] = true;
    table[10] = true;
    table[11] = true;
    table[12] = true;
    // Tree (13)
    table[13] = true;
    // SecretDoor (14) — looks like wall
    table[14] = true;
    // SecretCorridor (15) — looks like stone
    table[15] = true;
    // DrawbridgeUp (19) — raised drawbridge blocks sight
    table[19] = true;
    // LavaWall (21) — opaque
    table[21] = true;
    // Cloud (36) — blocks sight
    table[36] = true;
    // IronBars (22) — see-through, NOT opaque
    // Everything else: transparent
    table
}

// ---------------------------------------------------------------------------
// Compile-time validation
// ---------------------------------------------------------------------------

// Verify XP table is strictly monotonically increasing for levels 1..29
const _: () = {
    let mut i = 1;
    while i < 30 {
        assert!(XP_THRESHOLDS[i] > XP_THRESHOLDS[i - 1]);
        i += 1;
    }
};

// Verify XP_THRESHOLDS[0] == 0 (newuexp(0))
const _: () = assert!(XP_THRESHOLDS[0] == 0);

// Verify specific known values from the spec table
const _: () = {
    assert!(XP_THRESHOLDS[1] == 20);
    assert!(XP_THRESHOLDS[2] == 40);
    assert!(XP_THRESHOLDS[9] == 5120);
    assert!(XP_THRESHOLDS[10] == 10_000);
    assert!(XP_THRESHOLDS[20] == 10_000_000);
    assert!(XP_THRESHOLDS[29] == 100_000_000);
};

// Verify strength table consistency: encode_strength round-trips correctly
// for the key breakpoints
const _: () = {
    // STR 3 -> index 3
    assert!(STR_TO_HIT[encode_strength(3, 0)] == -2);
    assert!(STR_DAMAGE[encode_strength(3, 0)] == -1);

    // STR 18/00
    assert!(STR_TO_HIT[encode_strength(18, 0)] == 1);
    assert!(STR_DAMAGE[encode_strength(18, 0)] == 2);

    // STR 18/50
    assert!(STR_TO_HIT[encode_strength(18, 50)] == 1);
    assert!(STR_DAMAGE[encode_strength(18, 50)] == 3);

    // STR 18/51
    assert!(STR_TO_HIT[encode_strength(18, 51)] == 2);
    assert!(STR_DAMAGE[encode_strength(18, 51)] == 4);

    // STR 18/100
    assert!(STR_TO_HIT[encode_strength(18, 100)] == 3);
    assert!(STR_DAMAGE[encode_strength(18, 100)] == 7);

    // STR 19+
    assert!(STR_TO_HIT[encode_strength(19, 0)] == 3);
    assert!(STR_DAMAGE[encode_strength(19, 0)] == 6);
    assert!(STR_DAMAGE[encode_strength(25, 0)] == 6);
};

// Verify terrain passability for key types
const _: () = {
    assert!(!is_terrain_passable(0));  // Stone
    assert!(!is_terrain_passable(1));  // VWall
    assert!(!is_terrain_passable(16)); // Pool
    assert!(!is_terrain_passable(20)); // LavaPool
    assert!(is_terrain_passable(23));  // Door
    assert!(is_terrain_passable(24));  // Corridor
    assert!(is_terrain_passable(25));  // Room
    assert!(is_terrain_passable(26));  // Stairs
    assert!(is_terrain_passable(33));  // Ice
    assert!(is_terrain_passable(35));  // Air
};

// Verify terrain opacity for key types
const _: () = {
    assert!(is_terrain_opaque(0));   // Stone
    assert!(is_terrain_opaque(1));   // VWall
    assert!(is_terrain_opaque(13));  // Tree
    assert!(is_terrain_opaque(14));  // SecretDoor
    assert!(!is_terrain_opaque(22)); // IronBars (see-through)
    assert!(!is_terrain_opaque(23)); // Door
    assert!(!is_terrain_opaque(24)); // Corridor
    assert!(!is_terrain_opaque(25)); // Room
    assert!(is_terrain_opaque(36));  // Cloud
};

// ===========================================================================
// Runtime tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_to_hit_matches_spec() {
        // STR < 6: -2
        for s in 0..=5u8 {
            assert_eq!(
                STR_TO_HIT[encode_strength(s, 0)], -2,
                "STR_TO_HIT for STR {s}"
            );
        }
        // STR 6-7: -1
        assert_eq!(STR_TO_HIT[encode_strength(6, 0)], -1);
        assert_eq!(STR_TO_HIT[encode_strength(7, 0)], -1);
        // STR 8-16: 0
        for s in 8..=16u8 {
            assert_eq!(
                STR_TO_HIT[encode_strength(s, 0)], 0,
                "STR_TO_HIT for STR {s}"
            );
        }
        // STR 17: +1
        assert_eq!(STR_TO_HIT[encode_strength(17, 0)], 1);
        // STR 18/00..18/50: +1
        assert_eq!(STR_TO_HIT[encode_strength(18, 0)], 1);
        assert_eq!(STR_TO_HIT[encode_strength(18, 50)], 1);
        // STR 18/51..18/99: +2
        assert_eq!(STR_TO_HIT[encode_strength(18, 51)], 2);
        assert_eq!(STR_TO_HIT[encode_strength(18, 99)], 2);
        // STR 18/100: +3
        assert_eq!(STR_TO_HIT[encode_strength(18, 100)], 3);
        // STR 19+: +3
        assert_eq!(STR_TO_HIT[encode_strength(19, 0)], 3);
        assert_eq!(STR_TO_HIT[encode_strength(25, 0)], 3);
    }

    #[test]
    fn str_damage_matches_spec() {
        // STR < 6: -1
        for s in 0..=5u8 {
            assert_eq!(
                STR_DAMAGE[encode_strength(s, 0)], -1,
                "STR_DAMAGE for STR {s}"
            );
        }
        // STR 6-15: 0
        for s in 6..=15u8 {
            assert_eq!(
                STR_DAMAGE[encode_strength(s, 0)], 0,
                "STR_DAMAGE for STR {s}"
            );
        }
        // STR 16-17: +1
        assert_eq!(STR_DAMAGE[encode_strength(16, 0)], 1);
        assert_eq!(STR_DAMAGE[encode_strength(17, 0)], 1);
        // STR 18/00: +2
        assert_eq!(STR_DAMAGE[encode_strength(18, 0)], 2);
        // STR 18/01..18/50: +3
        assert_eq!(STR_DAMAGE[encode_strength(18, 1)], 3);
        assert_eq!(STR_DAMAGE[encode_strength(18, 50)], 3);
        // STR 18/51..18/75: +4
        assert_eq!(STR_DAMAGE[encode_strength(18, 51)], 4);
        assert_eq!(STR_DAMAGE[encode_strength(18, 75)], 4);
        // STR 18/76..18/90: +5
        assert_eq!(STR_DAMAGE[encode_strength(18, 76)], 5);
        assert_eq!(STR_DAMAGE[encode_strength(18, 90)], 5);
        // STR 18/91..18/99: +6
        assert_eq!(STR_DAMAGE[encode_strength(18, 91)], 6);
        assert_eq!(STR_DAMAGE[encode_strength(18, 99)], 6);
        // STR 18/100: +7
        assert_eq!(STR_DAMAGE[encode_strength(18, 100)], 7);
        // STR 19+: +6
        assert_eq!(STR_DAMAGE[encode_strength(19, 0)], 6);
        assert_eq!(STR_DAMAGE[encode_strength(25, 0)], 6);
    }

    #[test]
    fn xp_thresholds_match_spec_table() {
        // From the spec table in specs/experience.md
        assert_eq!(XP_THRESHOLDS[0], 0);
        assert_eq!(XP_THRESHOLDS[1], 20);
        assert_eq!(XP_THRESHOLDS[2], 40);
        assert_eq!(XP_THRESHOLDS[3], 80);
        assert_eq!(XP_THRESHOLDS[4], 160);
        assert_eq!(XP_THRESHOLDS[5], 320);
        assert_eq!(XP_THRESHOLDS[6], 640);
        assert_eq!(XP_THRESHOLDS[7], 1_280);
        assert_eq!(XP_THRESHOLDS[8], 2_560);
        assert_eq!(XP_THRESHOLDS[9], 5_120);
        assert_eq!(XP_THRESHOLDS[10], 10_000);
        assert_eq!(XP_THRESHOLDS[11], 20_000);
        assert_eq!(XP_THRESHOLDS[12], 40_000);
        assert_eq!(XP_THRESHOLDS[13], 80_000);
        assert_eq!(XP_THRESHOLDS[14], 160_000);
        assert_eq!(XP_THRESHOLDS[15], 320_000);
        assert_eq!(XP_THRESHOLDS[16], 640_000);
        assert_eq!(XP_THRESHOLDS[17], 1_280_000);
        assert_eq!(XP_THRESHOLDS[18], 2_560_000);
        assert_eq!(XP_THRESHOLDS[19], 5_120_000);
        assert_eq!(XP_THRESHOLDS[20], 10_000_000);
        assert_eq!(XP_THRESHOLDS[21], 20_000_000);
        assert_eq!(XP_THRESHOLDS[22], 30_000_000);
        assert_eq!(XP_THRESHOLDS[23], 40_000_000);
        assert_eq!(XP_THRESHOLDS[24], 50_000_000);
        assert_eq!(XP_THRESHOLDS[25], 60_000_000);
        assert_eq!(XP_THRESHOLDS[26], 70_000_000);
        assert_eq!(XP_THRESHOLDS[27], 80_000_000);
        assert_eq!(XP_THRESHOLDS[28], 90_000_000);
        assert_eq!(XP_THRESHOLDS[29], 100_000_000);
    }

    #[test]
    fn terrain_passable_coverage() {
        // All wall types should be impassable
        for t in 0..=12u8 {
            assert!(
                !is_terrain_passable(t),
                "terrain {t} should not be passable"
            );
        }
        // Tree, secret door, secret corridor
        assert!(!is_terrain_passable(13));
        assert!(!is_terrain_passable(14));
        assert!(!is_terrain_passable(15));
        // Water types
        assert!(!is_terrain_passable(16)); // Pool
        assert!(!is_terrain_passable(17)); // Moat
        assert!(!is_terrain_passable(18)); // Water
        assert!(!is_terrain_passable(20)); // LavaPool
        // All floor-like types
        for t in 23..=36u8 {
            assert!(
                is_terrain_passable(t),
                "terrain {t} should be passable"
            );
        }
    }

    #[test]
    fn encode_strength_boundary_cases() {
        assert_eq!(encode_strength(0, 0), 0);
        assert_eq!(encode_strength(17, 0), 17);
        assert_eq!(encode_strength(18, 0), 18);
        assert_eq!(encode_strength(18, 1), 19);
        assert_eq!(encode_strength(18, 100), 118);
        assert_eq!(encode_strength(19, 0), 119);        // 100 + 19
        assert_eq!(encode_strength(25, 0), 125);       // 100 + 25
        // Clamping: STR > 25 treated as 25
        assert_eq!(encode_strength(30, 0), encode_strength(25, 0));
        // Extra > 100 treated as 18/100
        assert_eq!(encode_strength(18, 200), 118);
    }
}
