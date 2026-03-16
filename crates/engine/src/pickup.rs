//! Extended pickup and drop system: encumbrance interaction, container logic,
//! floor effects, gold weight, scare-monster scroll, and kick-objects.
//!
//! Ported from C NetHack's `pickup.c` and portions of `do.c` and `dokick.c`.
//! This module sits above the lower-level `items::pickup_item` / `items::drop_item`
//! and adds the higher-level rules that gate those operations.
//!
//! All functions operate on the ECS `GameWorld` and return `Vec<EngineEvent>` —
//! no IO, no global state.

use rand::Rng;

use nethack_babel_data::ObjectClass;

use crate::world::Encumbrance;

// ---------------------------------------------------------------------------
// Gold weight (C GOLD_WT macro)
// ---------------------------------------------------------------------------

/// Calculate the weight of `n` gold pieces.
///
/// Mirrors C `GOLD_WT(n)`: every 100 gold pieces weigh 1 unit, with
/// rounding that counts 50+ remainder as another unit.
pub fn gold_weight(n: u32) -> u32 {
    (n + 50) / 100
}

/// Calculate the net weight change when merging `count` gold into an
/// existing pile of `existing` gold.
///
/// Due to integer rounding, the merged weight can be less than the sum
/// of the two individual weights.
pub fn gold_weight_delta(existing: u32, count: u32) -> i32 {
    let before = gold_weight(existing) + gold_weight(count);
    let after = gold_weight(existing + count);
    after as i32 - before as i32
}

// ---------------------------------------------------------------------------
// Encumbrance queries
// ---------------------------------------------------------------------------

/// Encumbrance difficulty message corresponding to a level.
pub fn encumbrance_message(enc: Encumbrance) -> &'static str {
    match enc {
        Encumbrance::Burdened => "You have a little trouble",
        Encumbrance::Stressed => "You have trouble",
        Encumbrance::Strained => "You have much trouble",
        Encumbrance::Overtaxed | Encumbrance::Overloaded => "You have extreme difficulty",
        Encumbrance::Unencumbered => "",
    }
}

/// Whether the given encumbrance level requires a confirmation prompt
/// before allowing the pickup.
pub fn needs_confirmation(prev: Encumbrance, next: Encumbrance) -> bool {
    next > prev
}

// ---------------------------------------------------------------------------
// Scare Monster scroll state machine
// ---------------------------------------------------------------------------

/// Outcome of attempting to pick up a Scroll of Scare Monster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScareScrollPickup {
    /// Scroll survives pickup (first time, or blessed → unbless).
    Survives,
    /// Scroll crumbles to dust.
    Dust,
}

/// Determine what happens when picking up a scare-monster scroll.
///
/// `spe` is the scroll's special field tracking pickup history.
/// `blessed` and `cursed` are the scroll's BUC state.
///
/// Returns the outcome and the new `spe` value.
pub fn scare_scroll_pickup(spe: i8, blessed: bool, cursed: bool) -> (ScareScrollPickup, i8) {
    if blessed {
        // Blessed: unbless but survive; spe unchanged.
        (ScareScrollPickup::Survives, spe)
    } else if cursed {
        // Cursed: always turns to dust.
        (ScareScrollPickup::Dust, spe)
    } else if spe == 0 {
        // First pickup: mark as picked up, survives.
        (ScareScrollPickup::Survives, 1)
    } else {
        // Already picked up once: turns to dust.
        (ScareScrollPickup::Dust, spe)
    }
}

// ---------------------------------------------------------------------------
// Container interaction types
// ---------------------------------------------------------------------------

/// What the player chose to do with a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerAction {
    /// `:` — look at contents
    Look,
    /// `o` — take items out
    TakeOut,
    /// `i` — put items in
    PutIn,
    /// `b` — both: take out then put in
    Both,
    /// `r` — reversed: put in then take out
    Reversed,
    /// `s` — stash a single item
    Stash,
    /// `n` — skip to next container
    Next,
    /// `q` — quit
    Quit,
}

// ---------------------------------------------------------------------------
// Bag of Holding explosion check
// ---------------------------------------------------------------------------

/// Check whether putting an item into a Bag of Holding would cause it to
/// explode.
///
/// Mirrors C `mbag_explodes(obj, depthin)`.
/// At `depthin=0` (directly in the BoH), a wand of cancellation or another
/// BoH with charges/contents always explodes.  Deeper nesting lowers the
/// probability.
pub fn boh_explodes<R: Rng>(rng: &mut R, depthin: u32) -> bool {
    let shift = depthin.min(7);
    let range = 1u32 << shift;
    let roll = rng.random_range(0..range);
    roll <= depthin
}

/// Calculate Bag of Holding explosion damage: d(6,6).
pub fn boh_explosion_damage<R: Rng>(rng: &mut R) -> u32 {
    let mut total = 0u32;
    for _ in 0..6 {
        total += rng.random_range(1..=6);
    }
    total
}

// ---------------------------------------------------------------------------
// Cursed BoH item loss
// ---------------------------------------------------------------------------

/// Check whether an item vanishes from a cursed Bag of Holding when opened.
///
/// Each item has a 1/13 chance of disappearing.
pub fn boh_item_gone<R: Rng>(rng: &mut R) -> bool {
    rng.random_range(0..13) == 0
}

// ---------------------------------------------------------------------------
// Autopickup configuration
// ---------------------------------------------------------------------------

/// How an item was lost from the player's inventory (for autopickup decisions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HowLost {
    None,
    Thrown,
    Dropped,
    Stolen,
    Exploding,
}

/// Autopickup configuration flags.
#[derive(Debug, Clone)]
pub struct AutopickupConfig {
    /// Master on/off switch.
    pub enabled: bool,
    /// Object classes to auto-pick (e.g., `[Coin, Potion, Scroll]`).
    pub pickup_types: Vec<ObjectClass>,
    /// Maximum encumbrance level tolerated during autopickup (0-4).
    pub pickup_burden: Encumbrance,
    /// Auto-pick items the player previously threw.
    pub pickup_thrown: bool,
    /// Auto-pick items that were stolen from the player.
    pub pickup_stolen: bool,
    /// Skip items the player explicitly dropped.
    pub nopick_dropped: bool,
    /// Autopickup exceptions (checked in order; last match wins).
    pub exceptions: Vec<AutopickupException>,
}

/// An autopickup exception rule.
///
/// Mirrors C `struct autopickup_exception`.  `pattern` is a glob-like
/// string matched against the singular item description (as produced by
/// `doname`).  If `grab` is true the item is always picked up; if false
/// it is always skipped.  Exceptions are checked in order and the last
/// matching rule wins (consistent with C NetHack behavior).
#[derive(Debug, Clone)]
pub struct AutopickupException {
    /// Glob pattern (supports `*` and `?`).
    pub pattern: String,
    /// `true` = always pick up, `false` = never pick up.
    pub grab: bool,
}

impl Default for AutopickupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pickup_types: vec![ObjectClass::Coin],
            pickup_burden: Encumbrance::Unencumbered,
            pickup_thrown: false,
            pickup_stolen: false,
            nopick_dropped: false,
            exceptions: Vec::new(),
        }
    }
}

/// Test whether an item should be auto-picked-up.
///
/// Mirrors C `autopick_testobj`.  `how_lost` comes from the item's
/// `how_lost` field; `object_class` is the item's class; `is_costly`
/// indicates the item is in a shop and not `no_charge`.
/// `item_name` is the singular description used for exception matching.
pub fn should_autopickup(
    config: &AutopickupConfig,
    object_class: ObjectClass,
    how_lost: HowLost,
    is_costly: bool,
) -> bool {
    should_autopickup_named(config, object_class, how_lost, is_costly, "")
}

/// Like [`should_autopickup`] but also checks the item's name against
/// autopickup exceptions.
pub fn should_autopickup_named(
    config: &AutopickupConfig,
    object_class: ObjectClass,
    how_lost: HowLost,
    is_costly: bool,
    item_name: &str,
) -> bool {
    // Shop items are never auto-picked.
    if is_costly {
        return false;
    }

    // how_lost overrides take priority.
    if config.pickup_thrown && how_lost == HowLost::Thrown {
        return true;
    }
    if config.pickup_stolen && how_lost == HowLost::Stolen {
        return true;
    }
    if config.nopick_dropped && how_lost == HowLost::Dropped {
        return false;
    }
    if how_lost == HowLost::Exploding {
        return false;
    }

    // Check pickup_types.
    let mut pickit = if config.pickup_types.is_empty() {
        true // empty means "pick up everything"
    } else {
        config.pickup_types.contains(&object_class)
    };

    // Check autopickup exceptions (last match wins, mirroring C).
    if !item_name.is_empty() {
        for exc in &config.exceptions {
            if glob_match(&exc.pattern, item_name) {
                pickit = exc.grab;
            }
        }
    }

    pickit
}

/// Simple glob matching supporting `*` (any sequence) and `?` (any char).
///
/// Case-insensitive, consistent with C NetHack's exception matching.
fn glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.to_lowercase().chars().collect();
    let txt: Vec<char> = text.to_lowercase().chars().collect();
    glob_match_inner(&pat, &txt)
}

fn glob_match_inner(pat: &[char], txt: &[char]) -> bool {
    let (mut pi, mut ti) = (0, 0);
    let (mut star_pi, mut star_ti) = (usize::MAX, 0);

    while ti < txt.len() {
        if pi < pat.len() && (pat[pi] == '?' || pat[pi] == txt[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pat.len() && pat[pi] == '*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < pat.len() && pat[pi] == '*' {
        pi += 1;
    }
    pi == pat.len()
}

// ---------------------------------------------------------------------------
// Floor effects
// ---------------------------------------------------------------------------

/// Possible special floor effect when an item is placed on the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorEffect {
    /// Item is placed normally.
    Normal,
    /// Item fell into water and was damaged.
    WaterDamage,
    /// Item fell into lava and was destroyed.
    LavaDestroyed,
    /// Boulder filled a pool/pit.
    BoulderFill,
    /// Boulder sank.
    BoulderSink,
}

// ---------------------------------------------------------------------------
// Kick object distance calculation
// ---------------------------------------------------------------------------

/// Calculate how far a kicked object travels.
///
/// Mirrors C `really_kick_object` distance formula.
/// `strength`: player's current strength (ACURRSTR).
/// `item_weight`: weight of a single item from the stack.
/// `is_martial`: whether the player has martial arts skill.
pub fn kick_distance<R: Rng>(
    rng: &mut R,
    strength: u32,
    item_weight: u32,
    is_martial: bool,
) -> u32 {
    let mut range = (strength / 2).saturating_sub(item_weight / 40);
    if is_martial {
        range += rng.random_range(1..=3);
    }
    range.max(1)
}

// ---------------------------------------------------------------------------
// Menu-based pickup
// ---------------------------------------------------------------------------

/// A single entry in a pickup menu presented to the player.
#[derive(Debug, Clone)]
pub struct PickupMenuItem {
    /// Display label (e.g. "a +1 long sword").
    pub label: String,
    /// Object class symbol.
    pub class: char,
    /// Stack quantity.
    pub quantity: u32,
    /// Index into the original ground-items list.
    pub index: usize,
}

/// An action resulting from a pickup menu selection.
#[derive(Debug, Clone)]
pub struct PickupAction {
    /// Index into the original ground-items list.
    pub index: usize,
    /// Name of the item.
    pub name: String,
    /// Object class.
    pub class: char,
    /// How many to pick up.
    pub quantity: u32,
}

/// Generate a pickup menu from items on the ground.
pub fn pickup_menu_items(ground_items: &[(String, char, u32)]) -> Vec<PickupMenuItem> {
    ground_items
        .iter()
        .enumerate()
        .map(|(i, (name, class, qty))| PickupMenuItem {
            label: if *qty > 1 {
                format!("{} {}", qty, name)
            } else {
                name.clone()
            },
            class: *class,
            quantity: *qty,
            index: i,
        })
        .collect()
}

/// Process pickup selections (indices into the ground-items list) and
/// return the corresponding pickup actions.
pub fn process_pickup_selections(
    selections: &[usize],
    ground_items: &[(String, char, u32)],
) -> Vec<PickupAction> {
    selections
        .iter()
        .filter_map(|&idx| {
            ground_items
                .get(idx)
                .map(|(name, class, qty)| PickupAction {
                    index: idx,
                    name: name.clone(),
                    class: *class,
                    quantity: *qty,
                })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Container interaction (Loot)
// ---------------------------------------------------------------------------

/// Actions available during loot-style container interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LootAction {
    /// Take a specific item out of the container.
    TakeOut { item_index: usize },
    /// Put a specific inventory item into the container.
    PutIn { item_index: usize },
    /// Take everything out.
    TakeAll,
    /// Put everything in.
    PutAll,
    /// Do nothing.
    Nothing,
}

/// Generate a loot menu showing container contents and player inventory.
///
/// Returns `(label, action)` pairs suitable for display in a UI menu.
pub fn container_menu(
    container_contents: &[String],
    player_inventory: &[String],
) -> Vec<(String, LootAction)> {
    let mut items = Vec::new();

    // Container contents — take-out options
    for (i, name) in container_contents.iter().enumerate() {
        items.push((
            format!("Take out: {}", name),
            LootAction::TakeOut { item_index: i },
        ));
    }
    if !container_contents.is_empty() {
        items.push(("Take out everything".to_string(), LootAction::TakeAll));
    }

    // Player inventory — put-in options
    for (i, name) in player_inventory.iter().enumerate() {
        items.push((
            format!("Put in: {}", name),
            LootAction::PutIn { item_index: i },
        ));
    }
    if !player_inventory.is_empty() {
        items.push(("Put in everything".to_string(), LootAction::PutAll));
    }

    items
}

/// Process taking items from a container by selected indices.
///
/// Removes the selected items from `container_items` and returns them.
/// Indices are processed in reverse order to avoid invalidation.
pub fn take_from_container(container_items: &mut Vec<String>, selections: &[usize]) -> Vec<String> {
    let mut sorted: Vec<usize> = selections
        .iter()
        .copied()
        .filter(|&i| i < container_items.len())
        .collect();
    sorted.sort_unstable();
    sorted.dedup();

    let taken: Vec<String> = sorted.iter().map(|&i| container_items[i].clone()).collect();

    // Remove in reverse order so indices stay valid.
    for &i in sorted.iter().rev() {
        container_items.remove(i);
    }

    taken
}

// ---------------------------------------------------------------------------
// Pile limit
// ---------------------------------------------------------------------------

/// Check if a pile of items should trigger the pickup menu based on the
/// `pile_limit` option.
///
/// Mirrors C NetHack's `iflags.pile_limit` behavior: if `pile_limit` is 0
/// the menu is never auto-shown; otherwise it triggers when item count
/// exceeds the limit.
pub fn should_show_pile_menu(item_count: usize, pile_limit: usize) -> bool {
    pile_limit > 0 && item_count > pile_limit
}

// ---------------------------------------------------------------------------
// Weight / encumbrance checking
// ---------------------------------------------------------------------------

/// Calculate encumbrance level from weight and capacity.
///
/// Mirrors C `calc_capacity`: `wt` is the amount over normal capacity
/// (i.e. `inv_weight() + xtra_wt`), and `weight_cap` is the player's
/// carrying capacity (`weight_cap()`).
///
/// The formula is: `cap = (wt * 2 / weight_cap) + 1`, clamped to 0..=5.
pub fn calc_encumbrance(wt: i32, weight_cap: i32) -> Encumbrance {
    if wt <= 0 {
        return Encumbrance::Unencumbered;
    }
    if weight_cap <= 1 {
        return Encumbrance::Overloaded;
    }
    let cap = (wt * 2 / weight_cap) + 1;
    match cap.min(5) {
        0 => Encumbrance::Unencumbered,
        1 => Encumbrance::Burdened,
        2 => Encumbrance::Stressed,
        3 => Encumbrance::Strained,
        4 => Encumbrance::Overtaxed,
        _ => Encumbrance::Overloaded,
    }
}

/// Check if picking up an item would change encumbrance level.
///
/// `current_carried` is weight currently carried (not the excess — total),
/// `item_weight` is the weight of the item to pick up,
/// `max_carry` is the player's carrying capacity.
///
/// Returns `(current_level, new_level)`.
pub fn check_encumbrance(
    current_carried: i32,
    item_weight: i32,
    max_carry: i32,
) -> (Encumbrance, Encumbrance) {
    let cur_excess = current_carried - max_carry;
    let new_excess = cur_excess + item_weight;
    (
        calc_encumbrance(cur_excess, max_carry),
        calc_encumbrance(new_excess, max_carry),
    )
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

    // ── Gold weight ───────────────────────────────────────────

    #[test]
    fn gold_weight_zero() {
        assert_eq!(gold_weight(0), 0);
    }

    #[test]
    fn gold_weight_49() {
        // (49 + 50) / 100 = 0
        assert_eq!(gold_weight(49), 0);
    }

    #[test]
    fn gold_weight_50() {
        // (50 + 50) / 100 = 1
        assert_eq!(gold_weight(50), 1);
    }

    #[test]
    fn gold_weight_100() {
        // (100 + 50) / 100 = 1
        assert_eq!(gold_weight(100), 1);
    }

    #[test]
    fn gold_weight_150() {
        // (150 + 50) / 100 = 2
        assert_eq!(gold_weight(150), 2);
    }

    #[test]
    fn gold_weight_delta_merge_saves_weight() {
        // Having 50 gold (wt=1) + picking up 50 (wt=1) => merged 100 (wt=1)
        // delta = 1 - (1+1) = -1
        let delta = gold_weight_delta(50, 50);
        assert_eq!(delta, -1);
    }

    #[test]
    fn gold_weight_delta_zero_to_49() {
        // 0 gold (wt=0) + 49 gold (wt=0) => 49 gold (wt=0)
        let delta = gold_weight_delta(0, 49);
        assert_eq!(delta, 0);
    }

    // ── Encumbrance messages ──────────────────────────────────

    #[test]
    fn encumbrance_messages() {
        assert_eq!(encumbrance_message(Encumbrance::Unencumbered), "");
        assert!(encumbrance_message(Encumbrance::Burdened).contains("little"));
        assert!(encumbrance_message(Encumbrance::Overloaded).contains("extreme"));
    }

    #[test]
    fn needs_confirmation_only_when_worse() {
        assert!(needs_confirmation(
            Encumbrance::Unencumbered,
            Encumbrance::Burdened
        ));
        assert!(!needs_confirmation(
            Encumbrance::Stressed,
            Encumbrance::Stressed
        ));
        assert!(!needs_confirmation(
            Encumbrance::Strained,
            Encumbrance::Burdened
        ));
    }

    // ── Scare monster scroll ──────────────────────────────────

    #[test]
    fn scare_scroll_first_pickup() {
        let (result, new_spe) = scare_scroll_pickup(0, false, false);
        assert_eq!(result, ScareScrollPickup::Survives);
        assert_eq!(new_spe, 1);
    }

    #[test]
    fn scare_scroll_second_pickup_dust() {
        let (result, _) = scare_scroll_pickup(1, false, false);
        assert_eq!(result, ScareScrollPickup::Dust);
    }

    #[test]
    fn scare_scroll_blessed_survives() {
        let (result, new_spe) = scare_scroll_pickup(0, true, false);
        assert_eq!(result, ScareScrollPickup::Survives);
        assert_eq!(new_spe, 0); // spe unchanged
    }

    #[test]
    fn scare_scroll_cursed_dust() {
        let (result, _) = scare_scroll_pickup(0, false, true);
        assert_eq!(result, ScareScrollPickup::Dust);
    }

    // ── Bag of Holding explosion ──────────────────────────────

    #[test]
    fn boh_depth_0_always_explodes() {
        let mut rng = test_rng();
        // At depth 0: rn2(1) = 0, 0 <= 0 → always true
        for _ in 0..100 {
            assert!(boh_explodes(&mut rng, 0));
        }
    }

    #[test]
    fn boh_depth_1_always_explodes() {
        let mut rng = test_rng();
        // At depth 1: rn2(2) → {0,1}, both <= 1 → always true
        for _ in 0..100 {
            assert!(boh_explodes(&mut rng, 1));
        }
    }

    #[test]
    fn boh_depth_3_probabilistic() {
        let mut rng = test_rng();
        // At depth 3: rn2(8), pass if <= 3 → 50% chance
        let mut explosions = 0;
        for _ in 0..10000 {
            if boh_explodes(&mut rng, 3) {
                explosions += 1;
            }
        }
        // Expect ~5000 ± margin
        assert!(
            explosions > 4000 && explosions < 6000,
            "expected ~5000, got {}",
            explosions
        );
    }

    #[test]
    fn boh_explosion_damage_range() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let dmg = boh_explosion_damage(&mut rng);
            assert!(dmg >= 6 && dmg <= 36, "d(6,6) out of range: {}", dmg);
        }
    }

    // ── Cursed BoH item loss ──────────────────────────────────

    #[test]
    fn boh_item_gone_probability() {
        let mut rng = test_rng();
        let mut gone = 0;
        for _ in 0..13000 {
            if boh_item_gone(&mut rng) {
                gone += 1;
            }
        }
        // Expect ~1000 (13000/13), allow wide range
        assert!(gone > 700 && gone < 1500, "expected ~1000, got {}", gone);
    }

    // ── Autopickup logic ──────────────────────────────────────

    #[test]
    fn autopickup_costly_items_rejected() {
        let config = AutopickupConfig::default();
        assert!(!should_autopickup(
            &config,
            ObjectClass::Weapon,
            HowLost::None,
            true
        ));
    }

    #[test]
    fn autopickup_thrown_overrides() {
        let config = AutopickupConfig {
            pickup_thrown: true,
            ..Default::default()
        };
        assert!(should_autopickup(
            &config,
            ObjectClass::Weapon,
            HowLost::Thrown,
            false
        ));
    }

    #[test]
    fn autopickup_stolen_overrides() {
        let config = AutopickupConfig {
            pickup_stolen: true,
            ..Default::default()
        };
        assert!(should_autopickup(
            &config,
            ObjectClass::Weapon,
            HowLost::Stolen,
            false
        ));
    }

    #[test]
    fn autopickup_dropped_blocked() {
        let config = AutopickupConfig {
            nopick_dropped: true,
            ..Default::default()
        };
        assert!(!should_autopickup(
            &config,
            ObjectClass::Coin,
            HowLost::Dropped,
            false
        ));
    }

    #[test]
    fn autopickup_exploding_blocked() {
        let config = AutopickupConfig::default();
        assert!(!should_autopickup(
            &config,
            ObjectClass::Coin,
            HowLost::Exploding,
            false
        ));
    }

    #[test]
    fn autopickup_matching_class() {
        let config = AutopickupConfig {
            pickup_types: vec![ObjectClass::Potion, ObjectClass::Scroll],
            ..Default::default()
        };
        assert!(should_autopickup(
            &config,
            ObjectClass::Potion,
            HowLost::None,
            false
        ));
        assert!(!should_autopickup(
            &config,
            ObjectClass::Weapon,
            HowLost::None,
            false
        ));
    }

    #[test]
    fn autopickup_empty_types_picks_all() {
        let config = AutopickupConfig {
            pickup_types: vec![],
            ..Default::default()
        };
        assert!(should_autopickup(
            &config,
            ObjectClass::Weapon,
            HowLost::None,
            false
        ));
    }

    // ── Kick distance ─────────────────────────────────────────

    #[test]
    fn kick_distance_basic() {
        let mut rng = test_rng();
        // STR=18, item_weight=40 → range = 9 - 1 = 8
        let dist = kick_distance(&mut rng, 18, 40, false);
        assert_eq!(dist, 8);
    }

    #[test]
    fn kick_distance_martial_adds() {
        let mut rng = test_rng();
        // STR=18, item_weight=40, martial → 8 + rnd(3)
        let dist = kick_distance(&mut rng, 18, 40, true);
        assert!(dist >= 9 && dist <= 11, "expected 9-11, got {}", dist);
    }

    #[test]
    fn kick_distance_heavy_item_minimum_one() {
        let mut rng = test_rng();
        // STR=10, item_weight=400 → 5 - 10 = underflow → clamped to 1
        let dist = kick_distance(&mut rng, 10, 400, false);
        assert_eq!(dist, 1);
    }

    // ── Autopickup exceptions ─────────────────────────────────

    #[test]
    fn autopickup_exception_grab() {
        let config = AutopickupConfig {
            pickup_types: vec![], // empty = pick all
            exceptions: vec![AutopickupException {
                pattern: "*long sword*".to_string(),
                grab: true,
            }],
            ..Default::default()
        };
        assert!(should_autopickup_named(
            &config,
            ObjectClass::Weapon,
            HowLost::None,
            false,
            "a +1 long sword",
        ));
    }

    #[test]
    fn autopickup_exception_nograb() {
        let config = AutopickupConfig {
            pickup_types: vec![ObjectClass::Weapon],
            exceptions: vec![AutopickupException {
                pattern: "*corpse*".to_string(),
                grab: false,
            }],
            ..Default::default()
        };
        // Weapon class matches, but exception says no.
        assert!(!should_autopickup_named(
            &config,
            ObjectClass::Weapon,
            HowLost::None,
            false,
            "a jackal corpse",
        ));
    }

    #[test]
    fn autopickup_exception_last_match_wins() {
        let config = AutopickupConfig {
            pickup_types: vec![],
            exceptions: vec![
                AutopickupException {
                    pattern: "*sword*".to_string(),
                    grab: false,
                },
                AutopickupException {
                    pattern: "*long sword*".to_string(),
                    grab: true,
                },
            ],
            ..Default::default()
        };
        // Both match, but last one (grab=true) wins.
        assert!(should_autopickup_named(
            &config,
            ObjectClass::Weapon,
            HowLost::None,
            false,
            "a long sword",
        ));
    }

    #[test]
    fn autopickup_exception_case_insensitive() {
        let config = AutopickupConfig {
            pickup_types: vec![],
            exceptions: vec![AutopickupException {
                pattern: "*Excalibur*".to_string(),
                grab: true,
            }],
            ..Default::default()
        };
        assert!(should_autopickup_named(
            &config,
            ObjectClass::Weapon,
            HowLost::None,
            false,
            "the blessed rustproof excalibur",
        ));
    }

    // ── Glob matching ─────────────────────────────────────────

    #[test]
    fn glob_match_star() {
        assert!(glob_match("*sword*", "a long sword"));
        assert!(!glob_match("*sword*", "a long spear"));
    }

    #[test]
    fn glob_match_question() {
        assert!(glob_match("f?o", "foo"));
        assert!(!glob_match("f?o", "fooo"));
    }

    #[test]
    fn glob_match_exact() {
        assert!(glob_match("hello", "hello"));
        assert!(!glob_match("hello", "world"));
    }

    // ── Menu-based pickup ─────────────────────────────────────

    #[test]
    fn pickup_menu_items_labels() {
        let ground = vec![
            ("long sword".to_string(), ')', 1),
            ("gold piece".to_string(), '$', 100),
        ];
        let menu = pickup_menu_items(&ground);
        assert_eq!(menu.len(), 2);
        assert_eq!(menu[0].label, "long sword");
        assert_eq!(menu[1].label, "100 gold piece");
        assert_eq!(menu[1].index, 1);
    }

    #[test]
    fn process_pickup_selections_filters() {
        let ground = vec![
            ("dagger".to_string(), ')', 1),
            ("potion".to_string(), '!', 3),
            ("scroll".to_string(), '?', 1),
        ];
        let actions = process_pickup_selections(&[0, 2], &ground);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].name, "dagger");
        assert_eq!(actions[1].name, "scroll");
    }

    #[test]
    fn process_pickup_selections_out_of_bounds() {
        let ground = vec![("dagger".to_string(), ')', 1)];
        let actions = process_pickup_selections(&[0, 5], &ground);
        assert_eq!(actions.len(), 1);
    }

    // ── Container interaction ─────────────────────────────────

    #[test]
    fn container_menu_entries() {
        let contents = vec!["gem".to_string()];
        let inv = vec!["potion".to_string()];
        let menu = container_menu(&contents, &inv);
        // 1 take-out + TakeAll + 1 put-in + PutAll = 4
        assert_eq!(menu.len(), 4);
        assert_eq!(menu[0].1, LootAction::TakeOut { item_index: 0 });
        assert_eq!(menu[1].1, LootAction::TakeAll);
        assert_eq!(menu[2].1, LootAction::PutIn { item_index: 0 });
        assert_eq!(menu[3].1, LootAction::PutAll);
    }

    #[test]
    fn take_from_container_removes_correct_items() {
        let mut contents = vec![
            "gem".to_string(),
            "scroll".to_string(),
            "potion".to_string(),
        ];
        let taken = take_from_container(&mut contents, &[0, 2]);
        assert_eq!(taken, vec!["gem", "potion"]);
        assert_eq!(contents, vec!["scroll"]);
    }

    #[test]
    fn take_from_container_deduplicates() {
        let mut contents = vec!["gem".to_string(), "scroll".to_string()];
        let taken = take_from_container(&mut contents, &[0, 0, 0]);
        assert_eq!(taken.len(), 1);
        assert_eq!(contents.len(), 1);
    }

    // ── Pile limit ────────────────────────────────────────────

    #[test]
    fn pile_limit_zero_never_shows() {
        assert!(!should_show_pile_menu(10, 0));
    }

    #[test]
    fn pile_limit_at_threshold() {
        assert!(!should_show_pile_menu(5, 5));
        assert!(should_show_pile_menu(6, 5));
    }

    // ── Encumbrance calculation ───────────────────────────────

    #[test]
    fn calc_encumbrance_unencumbered() {
        assert_eq!(calc_encumbrance(0, 1000), Encumbrance::Unencumbered);
        assert_eq!(calc_encumbrance(-50, 1000), Encumbrance::Unencumbered);
    }

    #[test]
    fn calc_encumbrance_levels() {
        // wt=1, cap=1000: cap = (1*2/1000)+1 = 1 → Burdened
        assert_eq!(calc_encumbrance(1, 1000), Encumbrance::Burdened);
        // wt=500, cap=1000: cap = (500*2/1000)+1 = 2 → Stressed
        assert_eq!(calc_encumbrance(500, 1000), Encumbrance::Stressed);
        // wt=1000, cap=1000: cap = (1000*2/1000)+1 = 3 → Strained
        assert_eq!(calc_encumbrance(1000, 1000), Encumbrance::Strained);
        // wt=1500, cap=1000: cap = (1500*2/1000)+1 = 4 → Overtaxed
        assert_eq!(calc_encumbrance(1500, 1000), Encumbrance::Overtaxed);
        // wt=2500, cap=1000: cap = (2500*2/1000)+1 = 6 → clamped to Overloaded
        assert_eq!(calc_encumbrance(2500, 1000), Encumbrance::Overloaded);
    }

    #[test]
    fn calc_encumbrance_tiny_cap() {
        // weight_cap <= 1 → always Overloaded when carrying anything
        assert_eq!(calc_encumbrance(1, 1), Encumbrance::Overloaded);
        assert_eq!(calc_encumbrance(1, 0), Encumbrance::Overloaded);
    }

    #[test]
    fn check_encumbrance_transition() {
        // Carrying 400 of 1000 capacity => excess = -600 => Unencumbered
        // Adding 700 => excess = 100 => (100*2/1000)+1 = 1 => Burdened
        let (cur, next) = check_encumbrance(400, 700, 1000);
        assert_eq!(cur, Encumbrance::Unencumbered);
        assert_eq!(next, Encumbrance::Burdened);
    }

    #[test]
    fn check_encumbrance_no_change() {
        let (cur, next) = check_encumbrance(400, 10, 1000);
        assert_eq!(cur, Encumbrance::Unencumbered);
        assert_eq!(next, Encumbrance::Unencumbered);
    }
}
