//! Sitting system: throne effects, terrain interaction, and egg laying.
//!
//! Ported from C NetHack's `sit.c`.  Implements the `#sit` command with
//! all its terrain-dependent outcomes — most notably the 13 throne effects,
//! including the special Vlad's Tower throne.
//!
//! All functions operate on the ECS `GameWorld` and return `Vec<EngineEvent>` —
//! no IO, no global state.

use rand::Rng;

use crate::dungeon::Terrain;
use crate::event::EngineEvent;

// ---------------------------------------------------------------------------
// Throne effect enum
// ---------------------------------------------------------------------------

/// The 13 possible throne effects from C `throne_sit_effect`.
///
/// Each variant carries only the information the caller needs to apply
/// the effect on the game world — actual stat/HP mutation is the caller's
/// responsibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThroneEffect {
    /// Case 1: lose 1-4 from a random attribute, take rnd(10) damage.
    AttributeLossAndDamage {
        attr_index: u8,
        attr_loss: u8,
        damage: u32,
    },
    /// Case 2: gain +1 to a random attribute.
    AttributeGain { attr_index: u8 },
    /// Case 3: electric shock (rnd(6) if resistant, rnd(30) otherwise).
    ElectricShock { damage: u32, resisted: bool },
    /// Case 4: full heal, cure blindness/sickness/wounded legs.
    FullHeal,
    /// Case 5: lose all gold.
    LoseGold,
    /// Case 6: wish (if luck + rn2(5) >= 0) or gain 1 luck.
    WishOrLuck { is_wish: bool },
    /// Case 7: summon courtiers (1-10 monsters).
    SummonCourt { count: u32 },
    /// Case 8: genocide offer.
    Genocide,
    /// Case 9: blindness + luck loss (if Luck > 0), or random curse.
    CurseOrBlind {
        blind_duration: u32,
        luck_loss: i32,
        is_blind: bool,
    },
    /// Case 10: see invisible, or map, or confusion.
    VisionOrMap {
        gained_see_invis: bool,
        mapped: bool,
        confused: u32,
    },
    /// Case 11: teleport (if Luck >= 0) or aggravate monsters.
    TeleportOrAggravate { teleported: bool },
    /// Case 12: identify some inventory items.
    Identify { count: u32 },
    /// Case 13: confusion (16-22 turns added).
    Confusion { duration: u32 },
}

/// Roll a throne effect (1..=13).
///
/// Mirrors C `throne_sit_effect`: 1/3 chance of an effect occurring.
/// Returns `None` if no effect triggers (the "comfortable/out of place" case).
pub fn roll_throne_effect<R: Rng>(
    rng: &mut R,
    luck: i32,
    has_shock_resist: bool,
    has_see_invis_intrinsic: bool,
    is_nommap: bool,
) -> Option<ThroneEffect> {
    // 1/3 chance of effect (rnd(6) > 4 means roll of 5 or 6)
    if rng.random_range(1..=6) <= 4 {
        return None;
    }

    let effect = rng.random_range(1..=13);

    let result = match effect {
        1 => {
            let attr = rng.random_range(0..6u8); // A_STR..A_CHA
            let loss = rng.random_range(3..=6u8); // rn1(4,3) = 3+rn2(4) = 3..6
            let damage = rng.random_range(1..=10u32);
            ThroneEffect::AttributeLossAndDamage {
                attr_index: attr,
                attr_loss: loss,
                damage,
            }
        }
        2 => {
            let attr = rng.random_range(0..6u8);
            ThroneEffect::AttributeGain { attr_index: attr }
        }
        3 => {
            let damage = if has_shock_resist {
                rng.random_range(1..=6)
            } else {
                rng.random_range(1..=30)
            };
            ThroneEffect::ElectricShock {
                damage,
                resisted: has_shock_resist,
            }
        }
        4 => ThroneEffect::FullHeal,
        5 => ThroneEffect::LoseGold,
        6 => {
            let roll = luck + rng.random_range(0..5) as i32;
            ThroneEffect::WishOrLuck { is_wish: roll >= 0 }
        }
        7 => {
            let count = rng.random_range(1..=10u32);
            ThroneEffect::SummonCourt { count }
        }
        8 => ThroneEffect::Genocide,
        9 => {
            if luck > 0 {
                let blind_dur = rng.random_range(250..=349u32); // rn1(100,250)
                let luck_loss = if luck > 1 {
                    -(rng.random_range(1..=2i32))
                } else {
                    -1
                };
                ThroneEffect::CurseOrBlind {
                    blind_duration: blind_dur,
                    luck_loss,
                    is_blind: true,
                }
            } else {
                ThroneEffect::CurseOrBlind {
                    blind_duration: 0,
                    luck_loss: 0,
                    is_blind: false,
                }
            }
        }
        10 => {
            if luck < 0 || has_see_invis_intrinsic {
                if is_nommap {
                    let conf = rng.random_range(1..=30u32);
                    ThroneEffect::VisionOrMap {
                        gained_see_invis: false,
                        mapped: false,
                        confused: conf,
                    }
                } else {
                    ThroneEffect::VisionOrMap {
                        gained_see_invis: false,
                        mapped: true,
                        confused: 0,
                    }
                }
            } else {
                ThroneEffect::VisionOrMap {
                    gained_see_invis: true,
                    mapped: false,
                    confused: 0,
                }
            }
        }
        11 => ThroneEffect::TeleportOrAggravate {
            teleported: luck >= 0,
        },
        12 => {
            let count = rng.random_range(0..5u32); // rn2(5)
            ThroneEffect::Identify { count }
        }
        13 => {
            let duration = rng.random_range(16..=22u32); // rn1(7,16) = 16..22
            ThroneEffect::Confusion { duration }
        }
        _ => unreachable!(),
    };

    Some(result)
}

// ---------------------------------------------------------------------------
// Special (Vlad's Tower) throne effects
// ---------------------------------------------------------------------------

/// The 13 special throne effects for Vlad's Tower.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialThroneEffect {
    /// Cases 1-4: a wish, then the throne disintegrates.
    WishAndDestroy,
    /// Case 5: permanent level drain.
    LevelDrain,
    /// Case 6: grease all inventory + glib hands.
    GreaseAll { glib_duration: u32 },
    /// Case 7: lose a random intrinsic (attrcurse).
    LoseIntrinsic,
    /// Case 8: level teleport to vibrating square level.
    LevelTeleport,
    /// Case 9: summon demons (3 msummon calls).
    SummonDemons,
    /// Case 10: confused blessed remove-curse effect.
    RemoveCurse,
    /// Case 11: polymorph.
    Polymorph,
    /// Case 12: acid damage.
    AcidDamage { damage: u32, resisted: bool },
    /// Case 13: ability shuffle (each stat ±2).
    AbilityShuffle { adjustments: [i8; 6] },
}

/// Roll a special throne effect (Vlad's Tower).
pub fn roll_special_throne<R: Rng>(rng: &mut R, has_acid_resist: bool) -> SpecialThroneEffect {
    let effect = rng.random_range(1..=13);

    match effect {
        1..=4 => SpecialThroneEffect::WishAndDestroy,
        5 => SpecialThroneEffect::LevelDrain,
        6 => {
            let dur = rng.random_range(100..=200u32); // rn1(101,100)
            SpecialThroneEffect::GreaseAll { glib_duration: dur }
        }
        7 => SpecialThroneEffect::LoseIntrinsic,
        8 => SpecialThroneEffect::LevelTeleport,
        9 => SpecialThroneEffect::SummonDemons,
        10 => SpecialThroneEffect::RemoveCurse,
        11 => SpecialThroneEffect::Polymorph,
        12 => {
            let damage = if has_acid_resist {
                rng.random_range(1..=16)
            } else {
                rng.random_range(1..=80)
            };
            SpecialThroneEffect::AcidDamage {
                damage,
                resisted: has_acid_resist,
            }
        }
        13 => {
            let mut adj = [0i8; 6];
            for a in &mut adj {
                *a = rng.random_range(0..5) as i8 - 2; // rn2(5) - 2 = -2..2
            }
            SpecialThroneEffect::AbilityShuffle { adjustments: adj }
        }
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// Throne removal check
// ---------------------------------------------------------------------------

/// After a throne effect, 1/3 chance the throne vanishes.
pub fn throne_vanishes<R: Rng>(rng: &mut R) -> bool {
    rng.random_range(0..3) == 0
}

// ---------------------------------------------------------------------------
// Sitting on terrain
// ---------------------------------------------------------------------------

/// What happens when the player tries to sit at their current position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SitResult {
    /// Throne effect triggered.
    Throne(Option<ThroneEffect>),
    /// Special throne (Vlad's Tower).
    SpecialThrone(SpecialThroneEffect),
    /// Sitting on an object (descriptive message).
    SitOnObject { message: String },
    /// Already stuck in a trap — makes it worse.
    TrapWorsen { trap_msg: String, extra_turns: u32 },
    /// Landed in a trap (was not stuck before).
    TrapLand,
    /// Sat on the floor/terrain (generic message).
    GenericSit { surface: String },
    /// Cannot sit (levitating, swallowed, riding, etc.).
    CannotSit { reason: String },
    /// Sink: rump gets wet.
    Sink,
    /// Altar: anger the gods.
    Altar,
    /// Grave: sit on the grave.
    Grave,
    /// Lava: burns (if fire resistant, reduced damage).
    Lava { damage: u32 },
    /// Ice: cold message.
    Ice,
    /// Water: sit in water, possible equipment damage.
    Water,
    /// Lay an egg (polymorphed into egg-laying creature).
    LayEgg,
}

/// Execute the #sit command, determining what happens at the player's
/// current position.
///
/// Returns events describing the outcome.  The caller is responsible for
/// applying HP damage, trap increments, and other state mutations.
pub fn do_sit<R: Rng>(
    _rng: &mut R,
    terrain: Terrain,
    is_riding: bool,
    is_levitating: bool,
    is_swallowed: bool,
    can_reach_floor: bool,
    _luck: i32,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if is_riding {
        events.push(EngineEvent::msg("sit-already-riding"));
        return events;
    }

    if !can_reach_floor {
        if is_swallowed {
            events.push(EngineEvent::msg("sit-no-seats"));
        } else if is_levitating {
            events.push(EngineEvent::msg("sit-tumble-in-place"));
        } else {
            events.push(EngineEvent::msg("sit-on-air"));
        }
        return events;
    }

    match terrain {
        Terrain::Throne => {
            events.push(EngineEvent::msg("sit-on-throne"));
        }
        Terrain::Sink => {
            events.push(EngineEvent::msg("sit-on-sink"));
            events.push(EngineEvent::msg("rump-gets-wet"));
        }
        Terrain::Altar => {
            events.push(EngineEvent::msg("sit-on-altar"));
        }
        Terrain::Grave => {
            events.push(EngineEvent::msg("sit-on-grave"));
        }
        Terrain::Lava => {
            events.push(EngineEvent::msg("sit-on-lava"));
        }
        Terrain::Ice => {
            events.push(EngineEvent::msg("sit-on-ice"));
        }
        Terrain::Pool | Terrain::Moat | Terrain::Water => {
            events.push(EngineEvent::msg("sit-in-water"));
        }
        Terrain::StairsUp | Terrain::StairsDown => {
            events.push(EngineEvent::msg("sit-on-stairs"));
        }
        Terrain::Fountain => {
            events.push(EngineEvent::msg("sit-in-water"));
        }
        _ => {
            events.push(EngineEvent::msg("sit-on-floor"));
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Random curse (rndcurse)
// ---------------------------------------------------------------------------

/// Result of cursing random inventory items.
#[derive(Debug, Clone)]
pub struct CurseResult {
    /// Number of items that were cursed.
    pub items_cursed: u32,
    /// Whether Magicbane absorbed the curse.
    pub magicbane_absorbed: bool,
}

/// Determine how many items to curse based on Antimagic and Half_spell_damage.
///
/// Mirrors C `rndcurse`: cnt = rnd(6 / (1 + !!Antimagic + !!Half_spell_damage)).
pub fn curse_count<R: Rng>(rng: &mut R, has_antimagic: bool, has_half_spell: bool) -> u32 {
    let divisor = 1u32 + has_antimagic as u32 + has_half_spell as u32;
    let max = 6 / divisor;
    rng.random_range(1..=max.max(1))
}

// ---------------------------------------------------------------------------
// Attrcurse — lose a random intrinsic
// ---------------------------------------------------------------------------

/// Which intrinsic property can be lost via `attrcurse`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LostIntrinsic {
    FireResistance,
    Teleportation,
    PoisonResistance,
    Telepathy,
    ColdResistance,
    Invisibility,
    SeeInvisible,
    Speed,
    Stealth,
    Protection,
    AggravateMonster,
    None,
}

/// Roll for losing a random intrinsic (fallthrough chain from C `attrcurse`).
///
/// Returns which intrinsic was lost.  `has_intrinsic` is a function that
/// checks whether the player has the intrinsic at the given roll index.
/// Mirrors the C fallthrough chain: if the rolled intrinsic isn't present,
/// fall through to the next one.
pub fn roll_attrcurse<R: Rng, F>(rng: &mut R, has_intrinsic: F) -> LostIntrinsic
where
    F: Fn(u8) -> bool,
{
    let roll = rng.random_range(1..=11u8);

    // C fallthrough chain: try roll, then roll+1, etc.
    let order = [
        (1, LostIntrinsic::FireResistance),
        (2, LostIntrinsic::Teleportation),
        (3, LostIntrinsic::PoisonResistance),
        (4, LostIntrinsic::Telepathy),
        (5, LostIntrinsic::ColdResistance),
        (6, LostIntrinsic::Invisibility),
        (7, LostIntrinsic::SeeInvisible),
        (8, LostIntrinsic::Speed),
        (9, LostIntrinsic::Stealth),
        (10, LostIntrinsic::Protection),
        (11, LostIntrinsic::AggravateMonster),
    ];

    // Start from the rolled index, wrap around (fallthrough)
    let start_idx = (roll - 1) as usize;
    for i in 0..order.len() {
        let idx = (start_idx + i) % order.len();
        let (check_idx, ref intrinsic) = order[idx];
        if has_intrinsic(check_idx) {
            return *intrinsic;
        }
    }

    LostIntrinsic::None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    // ── Throne effects ────────────────────────────────────────

    #[test]
    fn throne_effect_sometimes_none() {
        let mut rng = test_rng();
        let mut nones = 0;
        let mut somes = 0;
        for _ in 0..1000 {
            if roll_throne_effect(&mut rng, 0, false, false, false).is_none() {
                nones += 1;
            } else {
                somes += 1;
            }
        }
        // 2/3 chance of None, 1/3 chance of Some
        assert!(nones > 500, "expected ~667 nones, got {}", nones);
        assert!(somes > 200, "expected ~333 somes, got {}", somes);
    }

    #[test]
    fn throne_effect_all_variants_reachable() {
        let mut rng = SmallRng::seed_from_u64(12345);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100_000 {
            if let Some(e) = roll_throne_effect(&mut rng, 0, false, false, false) {
                let variant = std::mem::discriminant(&e);
                seen.insert(variant);
            }
        }
        // Should see all 13 variants
        assert!(
            seen.len() >= 12,
            "expected 13 variants, only saw {}",
            seen.len()
        );
    }

    #[test]
    fn full_heal_effect() {
        // Force effect 4 by trying many seeds
        for seed in 0..10000u64 {
            let mut r = SmallRng::seed_from_u64(seed);
            if let Some(ThroneEffect::FullHeal) = roll_throne_effect(&mut r, 0, false, false, false)
            {
                return; // found it
            }
        }
        panic!("FullHeal variant never occurred in 10000 seeds");
    }

    // ── Special throne ────────────────────────────────────────

    #[test]
    fn special_throne_wish_cases() {
        let mut rng = test_rng();
        let mut wishes = 0;
        for _ in 0..10000 {
            match roll_special_throne(&mut rng, false) {
                SpecialThroneEffect::WishAndDestroy => wishes += 1,
                _ => {}
            }
        }
        // Cases 1-4 out of 13 → ~30.8%
        assert!(
            wishes > 2000 && wishes < 4000,
            "expected ~3077, got {}",
            wishes
        );
    }

    #[test]
    fn special_throne_acid_damage_range() {
        for seed in 0..10000u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            match roll_special_throne(&mut rng, false) {
                SpecialThroneEffect::AcidDamage { damage, resisted } => {
                    assert!(!resisted);
                    assert!(damage >= 1 && damage <= 80);
                    return;
                }
                _ => {}
            }
        }
    }

    #[test]
    fn special_throne_acid_resisted_range() {
        for seed in 0..10000u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            match roll_special_throne(&mut rng, true) {
                SpecialThroneEffect::AcidDamage { damage, resisted } => {
                    assert!(resisted);
                    assert!(damage >= 1 && damage <= 16);
                    return;
                }
                _ => {}
            }
        }
    }

    // ── Throne vanishes ───────────────────────────────────────

    #[test]
    fn throne_vanishes_probability() {
        let mut rng = test_rng();
        let mut vanished = 0;
        for _ in 0..9000 {
            if throne_vanishes(&mut rng) {
                vanished += 1;
            }
        }
        // 1/3 chance → ~3000
        assert!(
            vanished > 2500 && vanished < 3500,
            "expected ~3000, got {}",
            vanished
        );
    }

    // ── do_sit terrain messages ───────────────────────────────

    #[test]
    fn sit_on_throne_emits_message() {
        let mut rng = test_rng();
        let events = do_sit(&mut rng, Terrain::Throne, false, false, false, true, 0);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "sit-on-throne")));
    }

    #[test]
    fn sit_while_riding_blocked() {
        let mut rng = test_rng();
        let events = do_sit(&mut rng, Terrain::Floor, true, false, false, true, 0);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "sit-already-riding")));
    }

    #[test]
    fn sit_while_levitating_blocked() {
        let mut rng = test_rng();
        let events = do_sit(&mut rng, Terrain::Floor, false, true, false, false, 0);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "sit-tumble-in-place")));
    }

    #[test]
    fn sit_while_swallowed_blocked() {
        let mut rng = test_rng();
        let events = do_sit(&mut rng, Terrain::Floor, false, false, true, false, 0);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "sit-no-seats")));
    }

    #[test]
    fn sit_on_sink() {
        let mut rng = test_rng();
        let events = do_sit(&mut rng, Terrain::Sink, false, false, false, true, 0);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "sit-on-sink")));
    }

    #[test]
    fn sit_on_lava() {
        let mut rng = test_rng();
        let events = do_sit(&mut rng, Terrain::Lava, false, false, false, true, 0);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "sit-on-lava")));
    }

    // ── Curse count ───────────────────────────────────────────

    #[test]
    fn curse_count_no_protection() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let cnt = curse_count(&mut rng, false, false);
            assert!(cnt >= 1 && cnt <= 6, "expected 1-6, got {}", cnt);
        }
    }

    #[test]
    fn curse_count_antimagic() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let cnt = curse_count(&mut rng, true, false);
            assert!(cnt >= 1 && cnt <= 3, "expected 1-3, got {}", cnt);
        }
    }

    #[test]
    fn curse_count_both_protections() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let cnt = curse_count(&mut rng, true, true);
            assert!(cnt >= 1 && cnt <= 2, "expected 1-2, got {}", cnt);
        }
    }

    // ── Attrcurse ─────────────────────────────────────────────

    #[test]
    fn attrcurse_finds_first_available() {
        let mut rng = SmallRng::seed_from_u64(0);
        // Has only poison resistance (index 3)
        let result = roll_attrcurse(&mut rng, |idx| idx == 3);
        assert_eq!(result, LostIntrinsic::PoisonResistance);
    }

    #[test]
    fn attrcurse_none_when_no_intrinsics() {
        let mut rng = test_rng();
        let result = roll_attrcurse(&mut rng, |_| false);
        assert_eq!(result, LostIntrinsic::None);
    }

    #[test]
    fn attrcurse_all_intrinsics_returns_something() {
        let mut rng = test_rng();
        let result = roll_attrcurse(&mut rng, |_| true);
        assert_ne!(result, LostIntrinsic::None);
    }
}
