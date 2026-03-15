//! Priest and temple mechanics for NetHack Babel.
//!
//! This module provides deeper priest/temple logic that complements the
//! NPC-level functions in `npc.rs`.  It covers temple donation tiers,
//! divine wrath (ghod_hitsu), protection purchase, priest anger/calm,
//! and sanctum behavior.
//!
//! Reference: C source `src/priest.c` (908 lines).

use rand::Rng;
use serde::{Deserialize, Serialize};

use nethack_babel_data::Alignment;

use crate::event::EngineEvent;

// ---------------------------------------------------------------------------
// Constants (from priest.c)
// ---------------------------------------------------------------------------

/// Alignment threshold: worse than strayed.
pub const ALGN_SINNED: i32 = -4;
/// Alignment threshold: better than fervent.
pub const ALGN_DEVOUT: i32 = 14;

// ---------------------------------------------------------------------------
// Temple info
// ---------------------------------------------------------------------------

/// Information about a temple on the current level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempleInfo {
    /// The alignment of this temple's altar.
    pub alignment: Alignment,
    /// Whether this temple currently has a priest.
    pub has_priest: bool,
    /// Whether the priest is angry at the player.
    pub priest_angry: bool,
    /// Total gold donated by the player to this temple.
    pub donations_made: i32,
    /// Whether this is a high temple / sanctum.
    pub is_sanctum: bool,
    /// Whether the altar has the AM_SHRINE flag.
    pub has_shrine: bool,
}

impl TempleInfo {
    pub fn new(alignment: Alignment) -> Self {
        Self {
            alignment,
            has_priest: true,
            priest_angry: false,
            donations_made: 0,
            is_sanctum: false,
            has_shrine: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Protection purchase
// ---------------------------------------------------------------------------

/// Result of attempting to buy protection from a temple priest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectionResult {
    /// Player cannot afford the protection.
    CantAfford { cost: i32 },
    /// Maximum protection already reached.
    MaxReached,
    /// Protection successfully purchased.
    Purchased {
        cost: i32,
        new_protection: i32,
    },
}

/// Cost of buying one level of protection from a temple priest.
///
/// Mirrors `priest_talk()` in C: the cost is proportional to both
/// the player's experience level and the current protection level.
///
/// Formula: `10 * player_level * (current_protection + 1)`.
///
/// Note: the formula in `npc.rs` uses `400 * (current_protection + 1)`
/// which is a simpler version.  This one is closer to C's actual formula
/// which factors in player level.
pub fn protection_cost(player_level: u8, current_protection: i32) -> i32 {
    10 * player_level as i32 * (current_protection + 1)
}

/// Buy divine protection from a temple priest.
///
/// Protection maxes out at 9 (normal) or 20 (rare, with decreasing
/// probability).  We use 9 as the standard maximum here.
pub fn buy_protection(
    player_gold: i32,
    player_level: u8,
    current_protection: i32,
) -> ProtectionResult {
    let max_protection = 9;

    if current_protection >= max_protection {
        return ProtectionResult::MaxReached;
    }

    let cost = protection_cost(player_level, current_protection);

    if player_gold < cost {
        ProtectionResult::CantAfford { cost }
    } else {
        ProtectionResult::Purchased {
            cost,
            new_protection: current_protection + 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Donation tiers (mirrors priest_talk() donation logic)
// ---------------------------------------------------------------------------

/// Result of donating gold to a temple priest.
///
/// The tier is determined by the donation amount relative to
/// `player_level * 200` increments, matching the C code in
/// `priest_talk()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DonationEffect {
    /// "Thou shalt regret thine action!" (refused to donate).
    RegretWarning,
    /// "Cheapskate." (small offer, player has more).
    Cheapskate,
    /// "I thank thee for thy contribution." (small offer, player is poor).
    SmallThanks,
    /// "Thou art indeed a pious individual." (medium offer).
    Pious,
    /// Blessing: clairvoyance granted.
    Clairvoyance { turns: i32 },
    /// "Thou hast been rewarded for thy devotion." (protection boost).
    ProtectionGranted { new_level: i32 },
    /// "Thy selfless generosity is deeply appreciated." (large offer).
    SelflessGenerosity,
    /// Alignment cleansed (large offer + strayed + enough time).
    Cleansed,
}

/// Determine the effect of a donation to a temple priest.
///
/// Mirrors the donation tiers in C's `priest_talk()`.
pub fn donation_effect(
    offer: i32,
    player_gold_after: i32,
    player_level: u8,
    alignment_record: i32,
    coaligned: bool,
    current_protection: i32,
    has_protection_intrinsic: bool,
    turns_since_cleansed: u32,
    rng: &mut impl Rng,
) -> DonationEffect {
    if offer <= 0 {
        return DonationEffect::RegretWarning;
    }

    let threshold = player_level as i32 * 200;

    if offer < threshold {
        // Small donation tier.
        if player_gold_after > offer {
            DonationEffect::Cheapskate
        } else {
            DonationEffect::SmallThanks
        }
    } else if offer < threshold * 2 {
        // Pious tier.
        // If poor + coaligned + sinned: clairvoyance blessing.
        if player_gold_after < offer
            && coaligned
            && alignment_record <= ALGN_SINNED
        {
            let turns = rng.random_range(500..1000);
            DonationEffect::Clairvoyance { turns }
        } else {
            DonationEffect::Pious
        }
    } else if offer < threshold * 3
        && (!has_protection_intrinsic
            || (current_protection < 20
                && (current_protection < 9
                    || rng.random_range(0..current_protection.max(1)) == 0)))
    {
        // Protection reward tier.
        DonationEffect::ProtectionGranted {
            new_level: current_protection + 1,
        }
    } else {
        // Selfless generosity tier.
        if player_gold_after < offer
            && coaligned
            && alignment_record < 0
            && turns_since_cleansed > 5000
        {
            DonationEffect::Cleansed
        } else {
            DonationEffect::SelflessGenerosity
        }
    }
}

// ---------------------------------------------------------------------------
// Priest anger / calm
// ---------------------------------------------------------------------------

/// Make a temple priest angry.
///
/// Mirrors `angry_priest()` from C.  In C this also handles the case
/// where the altar has been destroyed or converted (priest becomes a
/// roaming minion); we emit events and let the caller handle that.
pub fn anger_priest(temple: &mut TempleInfo) -> Vec<EngineEvent> {
    temple.priest_angry = true;
    vec![EngineEvent::msg("priest-angry")]
}

/// Calm a temple priest (e.g., after enough time or alignment repair).
pub fn calm_priest(temple: &mut TempleInfo) -> Vec<EngineEvent> {
    temple.priest_angry = false;
    vec![EngineEvent::msg("priest-calmed")]
}

// ---------------------------------------------------------------------------
// Divine wrath (ghod_hitsu)
// ---------------------------------------------------------------------------

/// Wrath message selection when attacking a priest in a temple.
///
/// Mirrors `ghod_hitsu()` from C.  Returns a message key for the
/// god's angry declaration.
pub fn wrath_message(rng: &mut impl Rng) -> &'static str {
    match rng.random_range(0..3) {
        0 => "god-roars-suffer",
        1 => "god-how-dare-harm-servant",
        _ => "god-profane-shrine",
    }
}

/// Generate events for divine wrath when the player attacks a priest
/// in their temple.
pub fn ghod_hitsu(temple: &TempleInfo, rng: &mut impl Rng) -> Vec<EngineEvent> {
    if !temple.has_priest || !temple.has_shrine {
        return vec![];
    }

    let msg_key = wrath_message(rng);
    vec![
        EngineEvent::msg(msg_key),
        EngineEvent::msg("god-lightning-bolt"),
    ]
}

// ---------------------------------------------------------------------------
// Sanctum behavior
// ---------------------------------------------------------------------------

/// Generate events when the player first enters the Sanctum of Moloch.
///
/// The high priest becomes hostile and delivers the "Infidel" speech.
pub fn sanctum_entry(first_time: bool) -> Vec<EngineEvent> {
    if first_time {
        vec![
            EngineEvent::msg("sanctum-infidel"),
            EngineEvent::msg("sanctum-be-gone"),
        ]
    } else {
        vec![EngineEvent::msg("sanctum-desecrate")]
    }
}

// ---------------------------------------------------------------------------
// Priest coalignment check
// ---------------------------------------------------------------------------

/// Check if the player's alignment matches a temple's alignment.
pub fn player_coaligned(
    player_alignment: Alignment,
    temple_alignment: Alignment,
) -> bool {
    player_alignment == temple_alignment
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
        Pcg64::seed_from_u64(54321)
    }

    // ── Protection tests ────────────────────────────────────────

    #[test]
    fn test_protection_cost() {
        // Level 10, protection 0: 10 * 10 * 1 = 100
        assert_eq!(protection_cost(10, 0), 100);
        // Level 10, protection 1: 10 * 10 * 2 = 200
        assert_eq!(protection_cost(10, 1), 200);
        // Level 14, protection 5: 10 * 14 * 6 = 840
        assert_eq!(protection_cost(14, 5), 840);
    }

    #[test]
    fn test_buy_protection_success() {
        let result = buy_protection(500, 10, 0);
        assert_eq!(
            result,
            ProtectionResult::Purchased {
                cost: 100,
                new_protection: 1,
            }
        );
    }

    #[test]
    fn test_buy_protection_cant_afford() {
        let result = buy_protection(50, 10, 0);
        assert_eq!(result, ProtectionResult::CantAfford { cost: 100 });
    }

    #[test]
    fn test_buy_protection_max_reached() {
        let result = buy_protection(100000, 10, 9);
        assert_eq!(result, ProtectionResult::MaxReached);
    }

    // ── Donation tests ──────────────────────────────────────────

    #[test]
    fn test_donation_regret() {
        let mut rng = test_rng();
        let effect = donation_effect(0, 1000, 14, 5, true, 0, false, 0, &mut rng);
        assert_eq!(effect, DonationEffect::RegretWarning);
    }

    #[test]
    fn test_donation_cheapskate() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 100, gold_after 9900 > 100.
        let effect = donation_effect(100, 9900, 14, 5, true, 0, false, 0, &mut rng);
        assert_eq!(effect, DonationEffect::Cheapskate);
    }

    #[test]
    fn test_donation_small_thanks() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 100, gold_after 50 < 100.
        let effect = donation_effect(100, 50, 14, 5, true, 0, false, 0, &mut rng);
        assert_eq!(effect, DonationEffect::SmallThanks);
    }

    #[test]
    fn test_donation_pious() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 3000 (>= 2800 < 5600). Gold 10000.
        let effect = donation_effect(3000, 7000, 14, 5, true, 0, false, 0, &mut rng);
        assert_eq!(effect, DonationEffect::Pious);
    }

    #[test]
    fn test_donation_protection() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 7000 (>= 5600 < 8400). Protection 0.
        let effect = donation_effect(7000, 13000, 14, 5, true, 0, false, 0, &mut rng);
        assert_eq!(
            effect,
            DonationEffect::ProtectionGranted { new_level: 1 }
        );
    }

    #[test]
    fn test_donation_selfless() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 10000 (>= 8400). Protection 20 (too high).
        let effect = donation_effect(10000, 40000, 14, 5, true, 20, true, 0, &mut rng);
        assert_eq!(effect, DonationEffect::SelflessGenerosity);
    }

    #[test]
    fn test_donation_cleansing() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 10000. Gold_after 5000 < 10000.
        // Coaligned, alignment_record < 0, turns since cleansed > 5000.
        let effect = donation_effect(10000, 5000, 14, -2, true, 20, true, 6000, &mut rng);
        assert_eq!(effect, DonationEffect::Cleansed);
    }

    // ── Priest anger / calm tests ───────────────────────────────

    #[test]
    fn test_anger_priest() {
        let mut temple = TempleInfo::new(Alignment::Lawful);
        assert!(!temple.priest_angry);
        let events = anger_priest(&mut temple);
        assert!(temple.priest_angry);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "priest-angry"
        )));
    }

    #[test]
    fn test_calm_priest() {
        let mut temple = TempleInfo::new(Alignment::Neutral);
        temple.priest_angry = true;
        let events = calm_priest(&mut temple);
        assert!(!temple.priest_angry);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "priest-calmed"
        )));
    }

    // ── Divine wrath tests ──────────────────────────────────────

    #[test]
    fn test_ghod_hitsu_with_shrine() {
        let mut rng = test_rng();
        let temple = TempleInfo::new(Alignment::Lawful);
        let events = ghod_hitsu(&temple, &mut rng);
        assert!(events.len() >= 2);
        // Should have a wrath message + lightning bolt.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "god-lightning-bolt"
        )));
    }

    #[test]
    fn test_ghod_hitsu_no_priest() {
        let mut rng = test_rng();
        let mut temple = TempleInfo::new(Alignment::Lawful);
        temple.has_priest = false;
        let events = ghod_hitsu(&temple, &mut rng);
        assert!(events.is_empty());
    }

    // ── Sanctum tests ───────────────────────────────────────────

    #[test]
    fn test_sanctum_entry_first_time() {
        let events = sanctum_entry(true);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "sanctum-infidel"
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "sanctum-be-gone"
        )));
    }

    #[test]
    fn test_sanctum_entry_repeat() {
        let events = sanctum_entry(false);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "sanctum-desecrate"
        )));
    }

    // ── Coalignment test ────────────────────────────────────────

    #[test]
    fn test_player_coaligned() {
        assert!(player_coaligned(Alignment::Lawful, Alignment::Lawful));
        assert!(!player_coaligned(Alignment::Lawful, Alignment::Chaotic));
        assert!(!player_coaligned(Alignment::Neutral, Alignment::Lawful));
    }
}
