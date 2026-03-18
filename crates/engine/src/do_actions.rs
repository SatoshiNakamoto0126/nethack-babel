//! Miscellaneous player actions: sit, ride, wipe, tip, rub, invoke.
//!
//! Ported from C NetHack's `do.c` (selected actions), `sit.c`, and parts
//! of `zap.c` (invoke).  This module collects smaller extended commands
//! that don't warrant their own module.
//!
//! All functions operate on the ECS `GameWorld` plus RNG and return
//! `Vec<EngineEvent>` — no IO, no global state.

use rand::Rng;

use crate::event::EngineEvent;

// ---------------------------------------------------------------------------
// Wipe face (#wipe)
// ---------------------------------------------------------------------------

/// Result of the #wipe command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WipeResult {
    /// Successfully wiped cream off face; `creamed` counter reset to 0.
    WipedCream,
    /// Face was not creamed, nothing to wipe.
    NothingToWipe,
    /// Wearing a cursed towel over face — cannot wipe (blind from towel).
    CursedTowel,
}

/// Execute the #wipe command: wipe cream off the hero's face.
///
/// `creamed` is the hero's current creamed counter (0 = not creamed).
/// Returns the result and any events.
pub fn do_wipe(creamed: u32, is_blind_from_towel: bool) -> (WipeResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    if is_blind_from_towel {
        events.push(EngineEvent::msg("wipe-cursed-towel"));
        return (WipeResult::CursedTowel, events);
    }

    if creamed == 0 {
        events.push(EngineEvent::msg("wipe-nothing"));
        return (WipeResult::NothingToWipe, events);
    }

    events.push(EngineEvent::msg("wipe-cream-off"));
    (WipeResult::WipedCream, events)
}

// ---------------------------------------------------------------------------
// Tip container (#tip)
// ---------------------------------------------------------------------------

/// Result of tipping a container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TipResult {
    /// Items dumped onto the floor.
    Dumped { item_count: u32 },
    /// Container was empty.
    Empty,
    /// Container is locked.
    Locked,
    /// Cannot tip (levitating, etc.).
    CannotTip { reason: String },
    /// Bag of tricks: monster(s) generated.
    BagOfTricks { monsters_created: u32 },
    /// Horn of plenty: food/drink generated.
    HornOfPlenty { items_created: u32 },
}

/// Execute the #tip command for a container.
///
/// Returns events and the result. The caller is responsible for actually
/// moving items and modifying game state.
pub fn do_tip(
    is_locked: bool,
    is_empty: bool,
    item_count: u32,
    can_reach_floor: bool,
) -> (TipResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    if !can_reach_floor {
        events.push(EngineEvent::msg("tip-cannot-reach"));
        return (
            TipResult::CannotTip {
                reason: "cannot reach floor".to_string(),
            },
            events,
        );
    }

    if is_locked {
        events.push(EngineEvent::msg("tip-locked"));
        return (TipResult::Locked, events);
    }

    if is_empty {
        events.push(EngineEvent::msg("tip-empty"));
        return (TipResult::Empty, events);
    }

    events.push(EngineEvent::msg_with(
        "tip-dump",
        vec![("count", item_count.to_string())],
    ));
    (TipResult::Dumped { item_count }, events)
}

// ---------------------------------------------------------------------------
// Rub object (#rub)
// ---------------------------------------------------------------------------

/// Result of rubbing an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RubResult {
    /// Rubbed a lamp — a djinni might appear.
    Lamp { djinni_appears: bool },
    /// Rubbed a touchstone — used for identification.
    Touchstone,
    /// Rubbed something with no special effect.
    NoEffect,
    /// Item is a gray stone on a touchstone — streak test.
    StreakTest,
}

/// Determine the result of rubbing an item.
///
/// `is_lamp`: whether the item is an oil lamp or magic lamp.
/// `is_magic_lamp`: specifically a magic lamp (guaranteed djinni).
/// `is_touchstone`: whether the item is a touchstone.
pub fn do_rub<R: Rng>(
    rng: &mut R,
    is_lamp: bool,
    is_magic_lamp: bool,
    is_touchstone: bool,
) -> (RubResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    if is_touchstone {
        events.push(EngineEvent::msg("rub-touchstone"));
        return (RubResult::Touchstone, events);
    }

    if is_lamp {
        // Magic lamp: always produces djinni on first rub.
        // Oil lamp: 1/3 chance of djinni.
        let djinni = if is_magic_lamp {
            true
        } else {
            rng.random_range(0..3) == 0
        };

        if djinni {
            events.push(EngineEvent::msg("rub-lamp-djinni"));
        } else {
            events.push(EngineEvent::msg("rub-lamp-nothing"));
        }
        return (
            RubResult::Lamp {
                djinni_appears: djinni,
            },
            events,
        );
    }

    events.push(EngineEvent::msg("rub-no-effect"));
    (RubResult::NoEffect, events)
}

// ---------------------------------------------------------------------------
// Invoke artifact (#invoke)
// ---------------------------------------------------------------------------

/// Result of invoking an artifact or special item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvokeResult {
    /// Successfully invoked the artifact power.
    Invoked { power_name: String },
    /// Item has no invoke power.
    NoPower,
    /// Invoke is on cooldown.
    OnCooldown { turns_remaining: u32 },
    /// Not wielding the item.
    NotWielded,
}

/// The invoke cooldown period in turns.
///
/// In C NetHack, most artifact invocations have a `rnz(100)` cooldown
/// (roughly 100 turns with some randomness).
pub const INVOKE_COOLDOWN: u32 = 100;

/// Check whether an artifact invoke is available (not on cooldown).
///
/// `last_invoke_turn` is the turn when the artifact was last invoked.
/// `current_turn` is the current game turn.
pub fn invoke_available(last_invoke_turn: u32, current_turn: u32) -> bool {
    current_turn.saturating_sub(last_invoke_turn) >= INVOKE_COOLDOWN
}

/// Execute artifact invocation.
///
/// Returns events describing the invocation. The caller handles the
/// actual effect (e.g., energy boost, levitation, conflict, etc.).
pub fn do_invoke(
    has_invoke_power: bool,
    is_wielded: bool,
    is_on_cooldown: bool,
    cooldown_remaining: u32,
    artifact_name: &str,
) -> (InvokeResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    if !is_wielded {
        events.push(EngineEvent::msg("invoke-not-wielded"));
        return (InvokeResult::NotWielded, events);
    }

    if !has_invoke_power {
        events.push(EngineEvent::msg("invoke-no-power"));
        return (InvokeResult::NoPower, events);
    }

    if is_on_cooldown {
        events.push(EngineEvent::msg_with(
            "invoke-cooldown",
            vec![("turns", cooldown_remaining.to_string())],
        ));
        return (
            InvokeResult::OnCooldown {
                turns_remaining: cooldown_remaining,
            },
            events,
        );
    }

    events.push(EngineEvent::msg_with(
        "invoke-artifact",
        vec![("name", artifact_name.to_string())],
    ));
    (
        InvokeResult::Invoked {
            power_name: artifact_name.to_string(),
        },
        events,
    )
}

// ---------------------------------------------------------------------------
// Jump (#jump)
// ---------------------------------------------------------------------------

/// Result of the #jump command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpResult {
    /// Successfully jumped to the target position.
    Jumped,
    /// Cannot jump: too burdened.
    TooBurdened,
    /// Cannot jump: no jumping ability (no boots of jumping, etc.).
    NoAbility,
    /// Target is out of range.
    OutOfRange,
    /// Cannot jump: stuck in a trap or similar impediment.
    CannotJump { reason: &'static str },
}

/// Execute the #jump command.
///
/// `has_jumping`: player has boots of jumping or intrinsic jumping.
/// `encumbrance_level`: 0=unenc, 1=burdened, ..., 5=overloaded.
/// `distance`: Manhattan distance to target.
/// `max_range`: maximum jump distance (2 without boots, 3 with).
pub fn do_jump(
    has_jumping: bool,
    encumbrance_level: u32,
    distance: u32,
    max_range: u32,
) -> (JumpResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    if !has_jumping {
        events.push(EngineEvent::msg("jump-no-ability"));
        return (JumpResult::NoAbility, events);
    }

    // Burdened or worse blocks jumping.
    if encumbrance_level >= 1 {
        events.push(EngineEvent::msg("jump-too-burdened"));
        return (JumpResult::TooBurdened, events);
    }

    if distance > max_range || distance == 0 {
        events.push(EngineEvent::msg("jump-out-of-range"));
        return (JumpResult::OutOfRange, events);
    }

    events.push(EngineEvent::msg("jump-success"));
    (JumpResult::Jumped, events)
}

// ---------------------------------------------------------------------------
// Untrap (#untrap)
// ---------------------------------------------------------------------------

/// Result of the #untrap command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UntrapResult {
    /// Successfully disarmed the trap.
    Disarmed,
    /// Failed to disarm — trap is still active.
    Failed,
    /// No trap in the given direction.
    NoTrap,
    /// Triggered the trap while trying to disarm.
    Triggered,
}

/// Attempt to disarm a trap.
///
/// `dex`: player's dexterity score.
/// `has_trap`: whether there's a trap in the target direction.
/// `difficulty`: trap difficulty rating (higher = harder to disarm).
pub fn do_untrap<R: Rng>(
    rng: &mut R,
    dex: i32,
    has_trap: bool,
    difficulty: u32,
) -> (UntrapResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    if !has_trap {
        events.push(EngineEvent::msg("untrap-no-trap"));
        return (UntrapResult::NoTrap, events);
    }

    // Disarm chance: (dex + luck - difficulty) / 20, minimum 1/20.
    let chance = ((dex as u32).saturating_sub(difficulty)).max(1);
    let roll = rng.random_range(0..20u32);

    if roll < chance {
        events.push(EngineEvent::msg("untrap-success"));
        (UntrapResult::Disarmed, events)
    } else if roll >= 18 {
        // Critical failure: trigger the trap.
        events.push(EngineEvent::msg("untrap-triggered"));
        (UntrapResult::Triggered, events)
    } else {
        events.push(EngineEvent::msg("untrap-failed"));
        (UntrapResult::Failed, events)
    }
}

// ---------------------------------------------------------------------------
// Turn undead (#turn)
// ---------------------------------------------------------------------------

/// Result of the #turn undead command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnUndeadResult {
    /// Successfully turned undead in the area.
    Turned { affected_count: u32 },
    /// Player is not a cleric or knight — cannot turn.
    NotClerical,
    /// No undead nearby to turn.
    NoUndead,
}

/// Execute the #turn undead command.
///
/// `is_clerical`: player is a Priest, Knight, or has clerical powers.
/// `player_level`: player's experience level.
/// `undead_nearby`: number of undead monsters in range.
pub fn do_turn_undead(
    is_clerical: bool,
    player_level: u32,
    undead_nearby: u32,
) -> (TurnUndeadResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    if !is_clerical {
        events.push(EngineEvent::msg("turn-not-clerical"));
        return (TurnUndeadResult::NotClerical, events);
    }

    if undead_nearby == 0 {
        events.push(EngineEvent::msg("turn-no-undead"));
        return (TurnUndeadResult::NoUndead, events);
    }

    // In C NetHack, turn undead affects monsters whose level < player level.
    // Here we just report count; the caller handles actual flee/destroy.
    let affected = undead_nearby.min(player_level);
    events.push(EngineEvent::msg_with(
        "turn-undead-success",
        vec![("count", affected.to_string())],
    ));
    (
        TurnUndeadResult::Turned {
            affected_count: affected,
        },
        events,
    )
}

// ---------------------------------------------------------------------------
// Swap weapons (#swap / x command)
// ---------------------------------------------------------------------------

/// Result of swapping primary and secondary weapons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapResult {
    /// Successfully swapped weapons.
    Swapped,
    /// Cannot swap: primary weapon is welded.
    Welded,
    /// No secondary weapon set.
    NoSecondary,
}

/// Execute the swap weapons command.
///
/// `has_secondary`: whether the player has a secondary weapon.
/// `primary_welded`: whether the primary weapon is cursed/welded.
pub fn do_swap_weapons(
    has_secondary: bool,
    primary_welded: bool,
) -> (SwapResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    if primary_welded {
        events.push(EngineEvent::msg("swap-welded"));
        return (SwapResult::Welded, events);
    }

    if !has_secondary {
        events.push(EngineEvent::msg("swap-no-secondary"));
        return (SwapResult::NoSecondary, events);
    }

    events.push(EngineEvent::msg("swap-success"));
    (SwapResult::Swapped, events)
}

// ---------------------------------------------------------------------------
// Monster ability (#monster)
// ---------------------------------------------------------------------------

/// Result of using a monster ability while polymorphed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterAbilityResult {
    /// Successfully used the ability.
    Used,
    /// Not polymorphed into a monster form.
    NotPolymorphed,
    /// Current form has no usable ability.
    NoAbility,
}

/// Execute the #monster command (use monster ability).
///
/// `is_polymorphed`: whether the player is currently polymorphed.
/// `has_ability`: whether the current form has a usable ability.
pub fn do_monster_ability(
    is_polymorphed: bool,
    has_ability: bool,
) -> (MonsterAbilityResult, Vec<EngineEvent>) {
    let mut events = Vec::new();

    if !is_polymorphed {
        events.push(EngineEvent::msg("monster-not-polymorphed"));
        return (MonsterAbilityResult::NotPolymorphed, events);
    }

    if !has_ability {
        events.push(EngineEvent::msg("monster-no-ability"));
        return (MonsterAbilityResult::NoAbility, events);
    }

    events.push(EngineEvent::msg("monster-ability-used"));
    (MonsterAbilityResult::Used, events)
}

// ---------------------------------------------------------------------------
// Known items (#known)
// ---------------------------------------------------------------------------

/// List all identified item types for display.
///
/// `identified_types` contains the display names of all identified items.
/// Returns a UI event listing them.
pub fn do_known_items(identified_types: &[String]) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    if identified_types.is_empty() {
        events.push(EngineEvent::msg("known-nothing"));
    } else {
        events.push(EngineEvent::msg_with(
            "known-items",
            vec![("count", identified_types.len().to_string())],
        ));
    }
    events
}

// ---------------------------------------------------------------------------
// Vanquished monsters (#vanquished)
// ---------------------------------------------------------------------------

/// List all monsters the player has killed.
///
/// `kill_counts` is a slice of (monster_name, count) pairs.
pub fn do_vanquished(kill_counts: &[(String, u32)]) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    if kill_counts.is_empty() {
        events.push(EngineEvent::msg("vanquished-none"));
    } else {
        let total: u32 = kill_counts.iter().map(|(_, c)| c).sum();
        events.push(EngineEvent::msg_with(
            "vanquished-list",
            vec![
                ("species", kill_counts.len().to_string()),
                ("total", total.to_string()),
            ],
        ));
    }
    events
}

// ---------------------------------------------------------------------------
// Call type (#call — name an object class)
// ---------------------------------------------------------------------------

/// Register a player-given name for an unidentified object type/class.
///
/// `class` is the item class character. `name` is the player-given label.
/// Returns events confirming the naming.
pub fn do_call_type(class: char, name: &str) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    if name.is_empty() {
        events.push(EngineEvent::msg("call-empty-name"));
    } else {
        events.push(EngineEvent::msg_with(
            "call-type-named",
            vec![("class", class.to_string()), ("name", name.to_string())],
        ));
    }
    events
}

// ---------------------------------------------------------------------------
// Glance (#glance — quick look at adjacent tile)
// ---------------------------------------------------------------------------

/// Quick look at an adjacent tile.
///
/// `description` is the pre-computed text describing the tile contents.
pub fn do_glance(description: &str) -> Vec<EngineEvent> {
    vec![EngineEvent::msg_with(
        "glance",
        vec![("description", description.to_string())],
    )]
}

// ---------------------------------------------------------------------------
// Chronicle (#chronicle — event log)
// ---------------------------------------------------------------------------

/// Return the player's event log for display.
///
/// `log_entries` contains chronological event descriptions.
pub fn do_chronicle(log_entries: &[String]) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    if log_entries.is_empty() {
        events.push(EngineEvent::msg("chronicle-empty"));
    } else {
        events.push(EngineEvent::msg_with(
            "chronicle",
            vec![("count", log_entries.len().to_string())],
        ));
    }
    events
}

// ---------------------------------------------------------------------------
// Sink ring effects (dosinkring)
// ---------------------------------------------------------------------------

/// What happens when a ring is dropped down a sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkRingEffect {
    /// Ring went down the drain.
    Drain,
    /// Ring clogged the drain.
    Clog,
    /// Ring bounced off — nothing happened.
    Bounce,
    /// Sink turned hot (fire ring).
    Hot,
    /// Sink turned cold (cold ring).
    Cold,
    /// Shock from the ring.
    Shock { damage: u32 },
    /// Ring dissolved in acid.
    Dissolve,
    /// Pudding appeared (ring of polymorph).
    PuddingAppears,
    /// Gushing fountain from ring.
    Fountain,
}

// ---------------------------------------------------------------------------
// Drop into water checks
// ---------------------------------------------------------------------------

/// Result of attempting to drop an item (specific to terrain effects).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropTerrainCheck {
    /// Normal drop, no special terrain.
    Normal,
    /// Item falls into a sink (ring).
    SinkRing,
    /// Item falls from levitation height — hitfloor.
    Hitfloor,
    /// Item is on an altar — BUC glow.
    AltarGlow,
}

/// Check whether the hero's current terrain has special drop effects.
pub fn check_drop_terrain(
    terrain: crate::dungeon::Terrain,
    is_ring: bool,
    can_reach_floor: bool,
) -> DropTerrainCheck {
    if !can_reach_floor {
        return DropTerrainCheck::Hitfloor;
    }

    if is_ring && terrain == crate::dungeon::Terrain::Sink {
        return DropTerrainCheck::SinkRing;
    }

    if terrain == crate::dungeon::Terrain::Altar {
        return DropTerrainCheck::AltarGlow;
    }

    DropTerrainCheck::Normal
}

// ---------------------------------------------------------------------------
// Multi-drop (#D command)
// ---------------------------------------------------------------------------

/// Statistics from a multi-drop operation.
#[derive(Debug, Clone, Default)]
pub struct MultiDropResult {
    /// Number of items successfully dropped.
    pub dropped: u32,
    /// Number of items that couldn't be dropped (welded, worn, etc.).
    pub blocked: u32,
}

// ---------------------------------------------------------------------------
// Canletgo check (can the player let go of an item?)
// ---------------------------------------------------------------------------

/// Reason the player cannot drop or put down an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CannotLetGo {
    /// Item is worn armor (must take off first).
    WornArmor,
    /// Item is worn accessory (ring, amulet).
    WornAccessory,
    /// Item is welded to hand.
    Welded,
    /// Item is a cursed loadstone.
    CursedLoadstone,
    /// Leash attached to a pet.
    LeashWithPet,
    /// Saddle currently in use (riding).
    SaddleInUse,
}

/// Check whether the player can let go of an item.
///
/// Returns `None` if the item can be dropped, or `Some(reason)` if not.
pub fn canletgo(
    is_worn_armor: bool,
    is_worn_accessory: bool,
    is_welded: bool,
    is_cursed_loadstone: bool,
    is_leash_with_pet: bool,
    is_saddle_in_use: bool,
) -> Option<CannotLetGo> {
    if is_worn_armor {
        return Some(CannotLetGo::WornArmor);
    }
    if is_worn_accessory {
        return Some(CannotLetGo::WornAccessory);
    }
    if is_welded {
        return Some(CannotLetGo::Welded);
    }
    if is_cursed_loadstone {
        return Some(CannotLetGo::CursedLoadstone);
    }
    if is_leash_with_pet {
        return Some(CannotLetGo::LeashWithPet);
    }
    if is_saddle_in_use {
        return Some(CannotLetGo::SaddleInUse);
    }
    None
}

// ---------------------------------------------------------------------------
// Cockatrice corpse safety check
// ---------------------------------------------------------------------------

/// Check whether touching a corpse is fatal (cockatrice without gloves).
///
/// Returns `true` if touching the corpse would cause instant petrification.
pub fn is_fatal_corpse_touch(
    is_cockatrice_corpse: bool,
    has_gloves: bool,
    has_stone_resistance: bool,
    is_remote: bool,
) -> bool {
    if !is_cockatrice_corpse {
        return false;
    }
    if has_gloves || has_stone_resistance || is_remote {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Wizard mode commands
// ---------------------------------------------------------------------------

/// Wizard mode: create a monster by name.
pub fn do_wiz_genesis(monster_name: &str) -> Vec<EngineEvent> {
    vec![EngineEvent::msg_with(
        "wizard-genesis",
        vec![("monster", monster_name.to_string())],
    )]
}

/// Wizard mode: grant a wish.
pub fn do_wiz_wish(wish_text: &str) -> Vec<EngineEvent> {
    vec![EngineEvent::msg_with(
        "wizard-wish",
        vec![("wish", wish_text.to_string())],
    )]
}

/// Wizard mode: identify all items in inventory.
pub fn do_wiz_identify() -> Vec<EngineEvent> {
    vec![EngineEvent::msg("wizard-identify-all")]
}

/// Wizard mode: reveal entire map.
pub fn do_wiz_map() -> Vec<EngineEvent> {
    vec![EngineEvent::msg("wizard-map-revealed")]
}

/// Wizard mode: teleport to a specific dungeon level.
pub fn do_wiz_level_teleport(target_depth: i32) -> Vec<EngineEvent> {
    vec![EngineEvent::msg_with(
        "wizard-level-teleport",
        vec![("depth", target_depth.to_string())],
    )]
}

/// Wizard mode: detect all monsters, objects, and traps.
pub fn do_wiz_detect() -> Vec<EngineEvent> {
    vec![
        EngineEvent::msg("wizard-detect-monsters"),
        EngineEvent::msg("wizard-detect-objects"),
        EngineEvent::msg("wizard-detect-traps"),
    ]
}

/// Wizard mode: show where special levels are.
pub fn do_wiz_where(topology: &[(String, i32)]) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    for (name, depth) in topology {
        events.push(EngineEvent::msg_with(
            "wizard-where",
            vec![("level", name.clone()), ("depth", depth.to_string())],
        ));
    }
    events
}

/// Check if wizard mode is enabled for the given username.
pub fn is_wizard_mode(config_wizards: &str, username: &str) -> bool {
    config_wizards == "*" || config_wizards.split_whitespace().any(|w| w == username)
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

    // ── Wipe ──────────────────────────────────────────────────

    #[test]
    fn wipe_cream() {
        let (result, events) = do_wipe(5, false);
        assert_eq!(result, WipeResult::WipedCream);
        assert!(!events.is_empty());
    }

    #[test]
    fn wipe_nothing() {
        let (result, events) = do_wipe(0, false);
        assert_eq!(result, WipeResult::NothingToWipe);
        assert!(!events.is_empty());
    }

    #[test]
    fn wipe_cursed_towel() {
        let (result, events) = do_wipe(5, true);
        assert_eq!(result, WipeResult::CursedTowel);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "wipe-cursed-towel")));
    }

    // ── Tip ───────────────────────────────────────────────────

    #[test]
    fn tip_items() {
        let (result, events) = do_tip(false, false, 5, true);
        assert_eq!(result, TipResult::Dumped { item_count: 5 });
        assert!(!events.is_empty());
    }

    #[test]
    fn tip_empty() {
        let (result, _) = do_tip(false, true, 0, true);
        assert_eq!(result, TipResult::Empty);
    }

    #[test]
    fn tip_locked() {
        let (result, _) = do_tip(true, false, 3, true);
        assert_eq!(result, TipResult::Locked);
    }

    #[test]
    fn tip_cannot_reach() {
        let (result, _) = do_tip(false, false, 3, false);
        assert!(matches!(result, TipResult::CannotTip { .. }));
    }

    // ── Rub ───────────────────────────────────────────────────

    #[test]
    fn rub_magic_lamp_always_djinni() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let (result, _) = do_rub(&mut rng, true, true, false);
            assert!(matches!(
                result,
                RubResult::Lamp {
                    djinni_appears: true
                }
            ));
        }
    }

    #[test]
    fn rub_oil_lamp_sometimes_djinni() {
        let mut rng = test_rng();
        let mut djinni_count = 0;
        for _ in 0..3000 {
            let (result, _) = do_rub(&mut rng, true, false, false);
            if let RubResult::Lamp {
                djinni_appears: true,
            } = result
            {
                djinni_count += 1;
            }
        }
        // 1/3 chance → ~1000
        assert!(
            djinni_count > 700 && djinni_count < 1300,
            "expected ~1000 djinnis, got {}",
            djinni_count
        );
    }

    #[test]
    fn rub_touchstone() {
        let mut rng = test_rng();
        let (result, _) = do_rub(&mut rng, false, false, true);
        assert_eq!(result, RubResult::Touchstone);
    }

    #[test]
    fn rub_no_effect() {
        let mut rng = test_rng();
        let (result, _) = do_rub(&mut rng, false, false, false);
        assert_eq!(result, RubResult::NoEffect);
    }

    // ── Invoke ────────────────────────────────────────────────

    #[test]
    fn invoke_available_after_cooldown() {
        assert!(invoke_available(0, 100));
        assert!(invoke_available(0, 200));
    }

    #[test]
    fn invoke_not_available_during_cooldown() {
        assert!(!invoke_available(50, 100));
        assert!(!invoke_available(0, 99));
    }

    #[test]
    fn invoke_success() {
        let (result, events) = do_invoke(true, true, false, 0, "Excalibur");
        assert!(matches!(result, InvokeResult::Invoked { .. }));
        assert!(!events.is_empty());
    }

    #[test]
    fn invoke_not_wielded() {
        let (result, _) = do_invoke(true, false, false, 0, "Excalibur");
        assert_eq!(result, InvokeResult::NotWielded);
    }

    #[test]
    fn invoke_no_power() {
        let (result, _) = do_invoke(false, true, false, 0, "dagger");
        assert_eq!(result, InvokeResult::NoPower);
    }

    #[test]
    fn invoke_cooldown() {
        let (result, _) = do_invoke(true, true, true, 50, "Excalibur");
        assert!(matches!(
            result,
            InvokeResult::OnCooldown {
                turns_remaining: 50
            }
        ));
    }

    // ── Drop terrain ──────────────────────────────────────────

    #[test]
    fn drop_ring_on_sink() {
        let check = check_drop_terrain(crate::dungeon::Terrain::Sink, true, true);
        assert_eq!(check, DropTerrainCheck::SinkRing);
    }

    #[test]
    fn drop_on_altar() {
        let check = check_drop_terrain(crate::dungeon::Terrain::Altar, false, true);
        assert_eq!(check, DropTerrainCheck::AltarGlow);
    }

    #[test]
    fn drop_cannot_reach_floor() {
        let check = check_drop_terrain(crate::dungeon::Terrain::Floor, false, false);
        assert_eq!(check, DropTerrainCheck::Hitfloor);
    }

    #[test]
    fn drop_normal() {
        let check = check_drop_terrain(crate::dungeon::Terrain::Floor, false, true);
        assert_eq!(check, DropTerrainCheck::Normal);
    }

    // ── Canletgo ──────────────────────────────────────────────

    #[test]
    fn canletgo_free_item() {
        assert!(canletgo(false, false, false, false, false, false).is_none());
    }

    #[test]
    fn canletgo_worn_armor() {
        assert_eq!(
            canletgo(true, false, false, false, false, false),
            Some(CannotLetGo::WornArmor)
        );
    }

    #[test]
    fn canletgo_welded() {
        assert_eq!(
            canletgo(false, false, true, false, false, false),
            Some(CannotLetGo::Welded)
        );
    }

    #[test]
    fn canletgo_cursed_loadstone() {
        assert_eq!(
            canletgo(false, false, false, true, false, false),
            Some(CannotLetGo::CursedLoadstone)
        );
    }

    #[test]
    fn canletgo_leash() {
        assert_eq!(
            canletgo(false, false, false, false, true, false),
            Some(CannotLetGo::LeashWithPet)
        );
    }

    #[test]
    fn canletgo_saddle() {
        assert_eq!(
            canletgo(false, false, false, false, false, true),
            Some(CannotLetGo::SaddleInUse)
        );
    }

    // ── Cockatrice corpse ─────────────────────────────────────

    #[test]
    fn cockatrice_corpse_fatal_without_gloves() {
        assert!(is_fatal_corpse_touch(true, false, false, false));
    }

    #[test]
    fn cockatrice_corpse_safe_with_gloves() {
        assert!(!is_fatal_corpse_touch(true, true, false, false));
    }

    #[test]
    fn cockatrice_corpse_safe_with_resistance() {
        assert!(!is_fatal_corpse_touch(true, false, true, false));
    }

    #[test]
    fn cockatrice_corpse_safe_remote() {
        assert!(!is_fatal_corpse_touch(true, false, false, true));
    }

    #[test]
    fn non_cockatrice_corpse_safe() {
        assert!(!is_fatal_corpse_touch(false, false, false, false));
    }

    // ── Jump ─────────────────────────────────────────────────

    #[test]
    fn jump_with_boots_success() {
        let (result, events) = do_jump(true, 0, 2, 3);
        assert_eq!(result, JumpResult::Jumped);
        assert!(!events.is_empty());
    }

    #[test]
    fn jump_no_ability() {
        let (result, _) = do_jump(false, 0, 2, 3);
        assert_eq!(result, JumpResult::NoAbility);
    }

    #[test]
    fn jump_too_burdened() {
        let (result, _) = do_jump(true, 1, 2, 3);
        assert_eq!(result, JumpResult::TooBurdened);
    }

    #[test]
    fn jump_out_of_range() {
        let (result, _) = do_jump(true, 0, 5, 3);
        assert_eq!(result, JumpResult::OutOfRange);
    }

    // ── Untrap ───────────────────────────────────────────────

    #[test]
    fn untrap_no_trap() {
        let mut rng = test_rng();
        let (result, _) = do_untrap(&mut rng, 16, false, 5);
        assert_eq!(result, UntrapResult::NoTrap);
    }

    #[test]
    fn untrap_outcomes_vary() {
        // Over many attempts, we should see at least disarmed and failed.
        let mut disarmed = 0;
        let mut failed = 0;
        for seed in 0..500u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            let (result, _) = do_untrap(&mut rng, 14, true, 5);
            match result {
                UntrapResult::Disarmed => disarmed += 1,
                UntrapResult::Failed => failed += 1,
                _ => {}
            }
        }
        assert!(disarmed > 0, "expected some disarms");
        assert!(failed > 0, "expected some failures");
    }

    // ── Turn undead ──────────────────────────────────────────

    #[test]
    fn turn_undead_cleric() {
        let (result, events) = do_turn_undead(true, 10, 3);
        assert_eq!(result, TurnUndeadResult::Turned { affected_count: 3 });
        assert!(!events.is_empty());
    }

    #[test]
    fn turn_undead_not_clerical() {
        let (result, _) = do_turn_undead(false, 10, 3);
        assert_eq!(result, TurnUndeadResult::NotClerical);
    }

    #[test]
    fn turn_undead_no_undead() {
        let (result, _) = do_turn_undead(true, 10, 0);
        assert_eq!(result, TurnUndeadResult::NoUndead);
    }

    // ── Swap weapons ─────────────────────────────────────────

    #[test]
    fn swap_weapons_success() {
        let (result, events) = do_swap_weapons(true, false);
        assert_eq!(result, SwapResult::Swapped);
        assert!(!events.is_empty());
    }

    #[test]
    fn swap_weapons_welded() {
        let (result, _) = do_swap_weapons(true, true);
        assert_eq!(result, SwapResult::Welded);
    }

    #[test]
    fn swap_weapons_no_secondary() {
        let (result, _) = do_swap_weapons(false, false);
        assert_eq!(result, SwapResult::NoSecondary);
    }

    // ── Monster ability ──────────────────────────────────────

    #[test]
    fn monster_ability_success() {
        let (result, events) = do_monster_ability(true, true);
        assert_eq!(result, MonsterAbilityResult::Used);
        assert!(!events.is_empty());
    }

    #[test]
    fn monster_ability_not_polymorphed() {
        let (result, _) = do_monster_ability(false, true);
        assert_eq!(result, MonsterAbilityResult::NotPolymorphed);
    }

    #[test]
    fn monster_ability_no_ability() {
        let (result, _) = do_monster_ability(true, false);
        assert_eq!(result, MonsterAbilityResult::NoAbility);
    }

    // ── Known items ─────────────────────────────────────────

    #[test]
    fn known_items_empty() {
        let events = do_known_items(&[]);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "known-nothing")));
    }

    #[test]
    fn known_items_with_entries() {
        let items = vec![
            "potion of healing".to_string(),
            "scroll of identify".to_string(),
        ];
        let events = do_known_items(&items);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "known-items")));
    }

    // ── Vanquished ──────────────────────────────────────────

    #[test]
    fn vanquished_empty() {
        let events = do_vanquished(&[]);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "vanquished-none")));
    }

    #[test]
    fn vanquished_with_kills() {
        let kills = vec![("grid bug".to_string(), 3u32), ("newt".to_string(), 1u32)];
        let events = do_vanquished(&kills);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "vanquished-list")));
    }

    // ── Call type ────────────────────────────────────────────

    #[test]
    fn call_type_names_class() {
        let events = do_call_type('!', "healing");
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "call-type-named")));
    }

    #[test]
    fn call_type_empty_name() {
        let events = do_call_type('!', "");
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "call-empty-name")));
    }

    // ── Glance ──────────────────────────────────────────────

    #[test]
    fn glance_returns_description() {
        let events = do_glance("a closed door");
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "glance")));
    }

    // ── Chronicle ───────────────────────────────────────────

    #[test]
    fn chronicle_empty() {
        let events = do_chronicle(&[]);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "chronicle-empty")));
    }

    #[test]
    fn chronicle_with_entries() {
        let entries = vec!["entered dungeon".to_string()];
        let events = do_chronicle(&entries);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "chronicle")));
    }

    // ── Wizard mode ─────────────────────────────────────────

    #[test]
    fn wiz_genesis_event() {
        let events = do_wiz_genesis("dragon");
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0],
            EngineEvent::Message { key, args } if key == "wizard-genesis"
                && args.iter().any(|(k, v)| k == "monster" && v == "dragon")));
    }

    #[test]
    fn wiz_wish_event() {
        let events = do_wiz_wish("blessed +3 silver dragon scale mail");
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0],
            EngineEvent::Message { key, args } if key == "wizard-wish"
                && args.iter().any(|(k, v)| k == "wish" && v == "blessed +3 silver dragon scale mail")));
    }

    #[test]
    fn wiz_identify_event() {
        let events = do_wiz_identify();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0],
            EngineEvent::Message { key, .. } if key == "wizard-identify-all"));
    }

    #[test]
    fn wiz_map_event() {
        let events = do_wiz_map();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0],
            EngineEvent::Message { key, .. } if key == "wizard-map-revealed"));
    }

    #[test]
    fn wiz_level_teleport_event() {
        let events = do_wiz_level_teleport(10);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0],
            EngineEvent::Message { key, args } if key == "wizard-level-teleport"
                && args.iter().any(|(k, v)| k == "depth" && v == "10")));
    }

    #[test]
    fn wiz_detect_event() {
        let events = do_wiz_detect();
        assert_eq!(events.len(), 3);
        assert!(matches!(&events[0],
            EngineEvent::Message { key, .. } if key == "wizard-detect-monsters"));
        assert!(matches!(&events[1],
            EngineEvent::Message { key, .. } if key == "wizard-detect-objects"));
        assert!(matches!(&events[2],
            EngineEvent::Message { key, .. } if key == "wizard-detect-traps"));
    }

    #[test]
    fn wiz_where_event() {
        let topology = vec![("Oracle".to_string(), 5), ("Rogue".to_string(), 15)];
        let events = do_wiz_where(&topology);
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0],
            EngineEvent::Message { key, args } if key == "wizard-where"
                && args.iter().any(|(k, v)| k == "level" && v == "Oracle")));
        assert!(matches!(&events[1],
            EngineEvent::Message { key, args } if key == "wizard-where"
                && args.iter().any(|(k, v)| k == "level" && v == "Rogue")));
    }

    #[test]
    fn wiz_where_empty() {
        let events = do_wiz_where(&[]);
        assert!(events.is_empty());
    }

    #[test]
    fn wizard_mode_wildcard() {
        assert!(is_wizard_mode("*", "anyone"));
        assert!(is_wizard_mode("*", "root"));
    }

    #[test]
    fn wizard_mode_specific_user() {
        assert!(is_wizard_mode("wizard debug", "wizard"));
        assert!(is_wizard_mode("wizard debug", "debug"));
        assert!(!is_wizard_mode("wizard debug", "player"));
    }

    #[test]
    fn wizard_mode_no_match() {
        assert!(!is_wizard_mode("root games", "player"));
        assert!(!is_wizard_mode("", "player"));
    }
}
