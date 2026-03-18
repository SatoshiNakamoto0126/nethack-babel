//! Minion summoning system for NetHack Babel.
//!
//! Implements divine minion summoning, guardian angels, and demon
//! hierarchy summoning.
//!
//! Reference: C source `src/minion.c` (566 lines).

use rand::Rng;
use serde::{Deserialize, Serialize};

use nethack_babel_data::Alignment;

use crate::event::EngineEvent;

// ---------------------------------------------------------------------------
// Minion types
// ---------------------------------------------------------------------------

/// Types of religious minions that can be summoned.
///
/// Mirrors the minion selection logic in C's `summon_minion()`,
/// `lminion()`, `ndemon()`, and `gain_guardian_angel()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MinionType {
    // Lawful minions
    /// Standard lawful minion (from S_ANGEL class).
    Angel,
    /// Archon — powerful lawful lord (`llord()` in C).
    Archon,

    // Neutral minions (elementals)
    /// Air elemental.
    AirElemental,
    /// Fire elemental.
    FireElemental,
    /// Earth elemental.
    EarthElemental,
    /// Water elemental.
    WaterElemental,

    // Chaotic minions (demons)
    /// Generic chaotic demon (`ndemon()` in C).
    Demon,
    /// Succubus (chaotic).
    Succubus,
    /// Incubus (chaotic).
    Incubus,
}

// ---------------------------------------------------------------------------
// Summoning result
// ---------------------------------------------------------------------------

/// Result of a minion summoning attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummonResult {
    pub minion_type: MinionType,
    /// Whether the minion is tame (friendly) to the player.
    pub is_tame: bool,
}

// ---------------------------------------------------------------------------
// Summoning logic
// ---------------------------------------------------------------------------

/// Select and summon a minion based on alignment.
///
/// Mirrors `summon_minion()` from C:
/// - Lawful: picks from S_ANGEL class via `lminion()`.  At high levels
///   an Archon may appear (`llord()`).
/// - Neutral: picks a random elemental from the four basic types.
/// - Chaotic: picks a demon via `ndemon()`.  May also get succubus/incubus.
pub fn summon_minion(alignment: Alignment, player_level: u8, rng: &mut impl Rng) -> SummonResult {
    let minion_type = match alignment {
        Alignment::Lawful => {
            if player_level >= 20 && rng.random_range(0..20) == 0 {
                MinionType::Archon
            } else {
                MinionType::Angel
            }
        }
        Alignment::Neutral => {
            // Random elemental, mirroring `ROLL_FROM(elementals)`.
            match rng.random_range(0..4) {
                0 => MinionType::AirElemental,
                1 => MinionType::FireElemental,
                2 => MinionType::EarthElemental,
                _ => MinionType::WaterElemental,
            }
        }
        Alignment::Chaotic => {
            // Chaotic minions: demons, succubi, or incubi.
            match rng.random_range(0..4) {
                0 => MinionType::Succubus,
                1 => MinionType::Incubus,
                _ => MinionType::Demon,
            }
        }
    };

    // Lawful and neutral minions are always tame when summoned as divine aid.
    // Chaotic minions may or may not be tame (50/50 chance).
    let is_tame = match alignment {
        Alignment::Chaotic => rng.random_range(0..2) == 0,
        _ => true,
    };

    SummonResult {
        minion_type,
        is_tame,
    }
}

/// Summon a punitive minion (angry god sends one to attack).
///
/// Mirrors `summon_minion(alignment, TRUE)` in C.
/// The minion is always hostile regardless of alignment.
pub fn summon_angry_minion(
    alignment: Alignment,
    player_level: u8,
    rng: &mut impl Rng,
) -> SummonResult {
    let mut result = summon_minion(alignment, player_level, rng);
    result.is_tame = false;
    result
}

// ---------------------------------------------------------------------------
// Guardian angel
// ---------------------------------------------------------------------------

/// Guardian angel data — summoned on the Astral Plane when worthy.
///
/// Mirrors `gain_guardian_angel()` from C.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardianAngel {
    pub alignment: Alignment,
    pub is_tame: bool,
    /// Whether this angel carries an amulet of reflection.
    pub has_reflection: bool,
    /// Combat level (15-22 in C: `rn1(8, 15)`).
    pub level: u8,
}

/// Determine whether the player is worthy of a guardian angel.
///
/// In C, the player gets a guardian angel on entering the Astral Plane
/// if `u.ualign.record > 8` (fervent) and no Conflict is active.
pub fn worthy_of_guardian(alignment_record: i32, has_conflict: bool) -> bool {
    !has_conflict && alignment_record > 8
}

/// Gain a guardian angel upon entering the Astral Plane.
///
/// The angel is tame only if the player has already had pets
/// (petless conduct check — `u.uconduct.pets` in C).
pub fn gain_guardian_angel(
    alignment: Alignment,
    has_had_pets: bool,
    rng: &mut impl Rng,
) -> GuardianAngel {
    let level = rng.random_range(15..=22);
    GuardianAngel {
        alignment,
        is_tame: has_had_pets,
        has_reflection: true,
        level,
    }
}

/// Generate events when the player loses their guardian angel
/// (e.g., due to Conflict).
///
/// In C, `lose_guardian_angel()` removes the angel and spawns
/// 2-4 hostile angels as replacements.
pub fn lose_guardian_angel(rng: &mut impl Rng) -> (Vec<EngineEvent>, u8) {
    let hostile_count = rng.random_range(2..=4);
    let events = vec![
        EngineEvent::msg("guardian-angel-rebukes"),
        EngineEvent::msg_with(
            "guardian-angel-replaced",
            vec![("count", hostile_count.to_string())],
        ),
    ];
    (events, hostile_count)
}

// ---------------------------------------------------------------------------
// Demon negotiation
// ---------------------------------------------------------------------------

/// Result of demon negotiation (`demon_talk()` in C).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemonTalkResult {
    /// Demon is angry (wielding Excalibur/Demonbane, or offer rejected).
    Angry,
    /// Demon recognizes hero as fellow demon.
    FellowDemon,
    /// Demon vanishes after accepting the bribe.
    Bribed { amount: i64 },
    /// Demon scowls but vanishes (close enough offer + charisma).
    ReluctantlyLeaves,
    /// Demon cannot be bribed (has Amulet, hero is Deaf, etc.).
    Unbribable,
}

#[derive(Debug, Clone, Copy)]
pub struct DemonTalkContext {
    pub player_gold: i64,
    pub offer: i64,
    pub demand: i64,
    pub charisma: i32,
    pub wielding_excalibur_or_demonbane: bool,
    pub player_is_demon: bool,
    pub demon_has_amulet: bool,
}

/// Calculate the demon's gold demand.
///
/// Mirrors C: `demand = (cash * (rnd(80) + 20*Athome)) / (100 * (1 + same_align))`.
pub fn demon_demand(
    player_gold: i64,
    is_at_home: bool,
    same_alignment: bool,
    rng: &mut impl Rng,
) -> i64 {
    if player_gold <= 0 {
        return 0;
    }
    let bonus = if is_at_home { 20 } else { 0 };
    let roll = rng.random_range(1..=80) + bonus;
    let divisor = 100 * if same_alignment { 2 } else { 1 };
    (player_gold * roll as i64) / divisor as i64
}

/// Resolve a demon talk encounter.
pub fn demon_talk(context: DemonTalkContext, rng: &mut impl Rng) -> DemonTalkResult {
    if context.wielding_excalibur_or_demonbane {
        return DemonTalkResult::Angry;
    }
    if context.player_is_demon {
        return DemonTalkResult::FellowDemon;
    }
    if context.demon_has_amulet {
        return DemonTalkResult::Unbribable;
    }
    if context.player_gold <= 0 {
        return DemonTalkResult::Angry;
    }
    if context.offer >= context.demand {
        return DemonTalkResult::Bribed {
            amount: context.offer,
        };
    }
    // Charisma check: `rnd(5 * CHA) > (demand - offer)`.
    if context.offer > 0 {
        let cha_roll = rng.random_range(1..=(5 * context.charisma).max(1));
        if cha_roll as i64 > context.demand - context.offer {
            return DemonTalkResult::ReluctantlyLeaves;
        }
    }
    DemonTalkResult::Angry
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(12345)
    }

    #[test]
    fn test_summon_lawful_angel() {
        let mut rng = test_rng();
        // At low level, should always get Angel (not Archon).
        let result = summon_minion(Alignment::Lawful, 5, &mut rng);
        assert_eq!(result.minion_type, MinionType::Angel);
        assert!(result.is_tame);
    }

    #[test]
    fn test_summon_lawful_high_level_may_get_archon() {
        // Run many attempts to verify Archon is possible at level 20+.
        let mut got_archon = false;
        for seed in 0..200 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let result = summon_minion(Alignment::Lawful, 25, &mut rng);
            if result.minion_type == MinionType::Archon {
                got_archon = true;
                assert!(result.is_tame);
                break;
            }
        }
        assert!(got_archon, "Archon should be possible at high levels");
    }

    #[test]
    fn test_summon_neutral_elemental() {
        let mut rng = test_rng();
        let result = summon_minion(Alignment::Neutral, 10, &mut rng);
        assert!(matches!(
            result.minion_type,
            MinionType::AirElemental
                | MinionType::FireElemental
                | MinionType::EarthElemental
                | MinionType::WaterElemental
        ));
        assert!(result.is_tame);
    }

    #[test]
    fn test_summon_chaotic_demon() {
        let mut rng = test_rng();
        let result = summon_minion(Alignment::Chaotic, 10, &mut rng);
        assert!(matches!(
            result.minion_type,
            MinionType::Demon | MinionType::Succubus | MinionType::Incubus
        ));
        // Chaotic minions may or may not be tame (RNG-dependent).
    }

    #[test]
    fn test_summon_angry_minion_is_not_tame() {
        let mut rng = test_rng();
        let result = summon_angry_minion(Alignment::Lawful, 10, &mut rng);
        assert!(!result.is_tame);
    }

    #[test]
    fn test_guardian_angel_is_tame_with_pets() {
        let mut rng = test_rng();
        let angel = gain_guardian_angel(Alignment::Lawful, true, &mut rng);
        assert!(angel.is_tame);
        assert_eq!(angel.alignment, Alignment::Lawful);
        assert!(angel.has_reflection);
        assert!((15..=22).contains(&angel.level));
    }

    #[test]
    fn test_guardian_angel_not_tame_petless() {
        let mut rng = test_rng();
        let angel = gain_guardian_angel(Alignment::Neutral, false, &mut rng);
        assert!(!angel.is_tame);
    }

    #[test]
    fn test_worthy_of_guardian() {
        assert!(worthy_of_guardian(9, false));
        assert!(worthy_of_guardian(20, false));
        assert!(!worthy_of_guardian(8, false)); // not fervent
        assert!(!worthy_of_guardian(20, true)); // has Conflict
    }

    #[test]
    fn test_lose_guardian_angel_spawns_hostiles() {
        let mut rng = test_rng();
        let (events, count) = lose_guardian_angel(&mut rng);
        assert!(!events.is_empty());
        assert!((2..=4).contains(&count));
    }

    #[test]
    fn test_demon_demand_zero_gold() {
        let mut rng = test_rng();
        assert_eq!(demon_demand(0, false, false, &mut rng), 0);
    }

    #[test]
    fn test_demon_demand_positive() {
        let mut rng = test_rng();
        let demand = demon_demand(1000, false, false, &mut rng);
        assert!(demand > 0);
        assert!(demand <= 1000); // at most 80% without at_home
    }

    #[test]
    fn test_demon_talk_angry_with_excalibur() {
        let mut rng = test_rng();
        let result = demon_talk(
            DemonTalkContext {
                player_gold: 1000,
                offer: 0,
                demand: 500,
                charisma: 10,
                wielding_excalibur_or_demonbane: true,
                player_is_demon: false,
                demon_has_amulet: false,
            },
            &mut rng,
        );
        assert_eq!(result, DemonTalkResult::Angry);
    }

    #[test]
    fn test_demon_talk_fellow_demon() {
        let mut rng = test_rng();
        let result = demon_talk(
            DemonTalkContext {
                player_gold: 1000,
                offer: 0,
                demand: 500,
                charisma: 10,
                wielding_excalibur_or_demonbane: false,
                player_is_demon: true,
                demon_has_amulet: false,
            },
            &mut rng,
        );
        assert_eq!(result, DemonTalkResult::FellowDemon);
    }

    #[test]
    fn test_demon_talk_bribed() {
        let mut rng = test_rng();
        let result = demon_talk(
            DemonTalkContext {
                player_gold: 1000,
                offer: 500,
                demand: 400,
                charisma: 10,
                wielding_excalibur_or_demonbane: false,
                player_is_demon: false,
                demon_has_amulet: false,
            },
            &mut rng,
        );
        assert_eq!(result, DemonTalkResult::Bribed { amount: 500 });
    }

    #[test]
    fn test_demon_talk_unbribable_with_amulet() {
        let mut rng = test_rng();
        let result = demon_talk(
            DemonTalkContext {
                player_gold: 1000,
                offer: 500,
                demand: 400,
                charisma: 10,
                wielding_excalibur_or_demonbane: false,
                player_is_demon: false,
                demon_has_amulet: true,
            },
            &mut rng,
        );
        assert_eq!(result, DemonTalkResult::Unbribable);
    }
}
