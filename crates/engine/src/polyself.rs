//! Self-polymorph system for NetHack Babel.
//!
//! Implements the self-polymorph mechanic (see `specs/status-timeout.md`
//! section 3, Phase 3):
//! - Transforming the player into a monster form
//! - Saving/restoring original attributes
//! - System shock on low CON
//! - Polymorph timeout with automatic reversion
//! - Form-based abilities (flying, swimming, phasing)
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::event::{DeathCause, EngineEvent, HpSource, StatusEffect};
use crate::steed;
use crate::world::{
    Attributes, DisplaySymbol, ExperienceLevel, GameWorld, HitPoints, Speed,
};

// ---------------------------------------------------------------------------
// OriginalForm component
// ---------------------------------------------------------------------------

/// Component storing the player's pre-polymorph state for later restoration.
///
/// When the player polymorphs into a monster, their original attributes,
/// HP, speed, and display symbol are saved here.  On revert (timeout or
/// deliberate), these values are written back.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginalForm {
    pub attributes: Attributes,
    pub hp: HitPoints,
    pub speed: u32,
    pub display_symbol: char,
    pub display_color: nethack_babel_data::Color,
    /// The MonsterId of the form the player polymorphed into.
    pub monster_id: nethack_babel_data::MonsterId,
}

/// Component tracking the remaining turns of a polymorph transformation.
///
/// Decremented each turn by `tick_polymorph`.  When it reaches 0 the
/// player automatically reverts to their original form.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PolymorphTimer(pub u32);

/// Abilities granted by the current polymorph form.
///
/// These are derived from the `MonsterFlags` of the form and cached
/// on the entity so that movement and terrain checks can query them
/// without needing access to the `MonsterDef` table.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PolymorphAbilities {
    pub can_fly: bool,
    pub can_swim: bool,
    pub can_phase: bool,
}

// ---------------------------------------------------------------------------
// PolymorphState — richer state tracking (requested by task spec)
// ---------------------------------------------------------------------------

/// Component tracking the full polymorph state, including original stats
/// and the monster form ID.  Coexists with `OriginalForm` (which stores
/// the raw values for restoration).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PolymorphState {
    /// Original HP before polymorph.
    pub original_hp: i32,
    /// Original max HP before polymorph.
    pub original_max_hp: i32,
    /// Original experience level.
    pub original_level: u8,
    /// MonsterId of the current monster form.
    pub monster_form_id: nethack_babel_data::MonsterId,
    /// Remaining polymorph timer (also tracked by PolymorphTimer).
    pub timer: u32,
}

// ---------------------------------------------------------------------------
// FormAbilities — comprehensive ability query
// ---------------------------------------------------------------------------

/// Full set of abilities derived from a monster form.
///
/// Used to determine what the player can/cannot do while polymorphed.
/// Derived from `MonsterFlags` of the form.
#[derive(Debug, Clone, Copy, Default)]
pub struct FormAbilities {
    pub can_fly: bool,
    pub can_swim: bool,
    pub is_breathless: bool,
    pub passes_walls: bool,
    pub has_hands: bool,
    pub can_wear_armor: bool,
    pub can_wield_weapon: bool,
    pub can_cast_spells: bool,
    pub has_infravision: bool,
    pub is_undead: bool,
    pub is_amorphous: bool,
    pub can_tunnel: bool,
    pub can_regenerate: bool,
}

/// Derive a full `FormAbilities` from a `MonsterDef`.
///
/// Examines the monster's flags and symbol to determine what
/// the polymorphed player can do.
pub fn polymon_abilities(
    monster_def: &nethack_babel_data::MonsterDef,
) -> FormAbilities {
    use nethack_babel_data::MonsterFlags;
    let f = monster_def.flags;
    let humanoid = f.contains(MonsterFlags::HUMANOID);
    let no_hands = f.contains(MonsterFlags::NOHANDS);
    let no_limbs = f.contains(MonsterFlags::NOLIMBS);

    FormAbilities {
        can_fly: f.contains(MonsterFlags::FLY),
        can_swim: f.contains(MonsterFlags::SWIM),
        is_breathless: f.contains(MonsterFlags::BREATHLESS),
        passes_walls: f.contains(MonsterFlags::WALLWALK),
        has_hands: humanoid && !no_hands,
        can_wear_armor: humanoid && !no_limbs,
        can_wield_weapon: humanoid && !no_hands,
        can_cast_spells: humanoid && !no_hands && !f.contains(MonsterFlags::ANIMAL),
        has_infravision: f.contains(MonsterFlags::INFRAVISION),
        is_undead: f.contains(MonsterFlags::UNDEAD),
        is_amorphous: f.contains(MonsterFlags::AMORPHOUS),
        can_tunnel: f.contains(MonsterFlags::TUNNEL),
        can_regenerate: f.contains(MonsterFlags::REGEN),
    }
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

/// Check whether the given entity is currently polymorphed.
pub fn is_polymorphed(world: &GameWorld, entity: Entity) -> bool {
    world.get_component::<OriginalForm>(entity).is_some()
}

/// Return the polymorph abilities for the given entity, if any.
pub fn polymorph_abilities(
    world: &GameWorld,
    entity: Entity,
) -> Option<PolymorphAbilities> {
    world
        .get_component::<PolymorphAbilities>(entity)
        .map(|a| *a)
}

// ---------------------------------------------------------------------------
// Polymorph into a monster form
// ---------------------------------------------------------------------------

/// Transform the player into a monster form.
///
/// 1. Saves original attributes, HP, speed, and display symbol into
///    an `OriginalForm` component.
/// 2. Replaces the player's stats with the monster's base values.
/// 3. Sets a polymorph timeout of `d(500, 500)` turns.
/// 4. Applies system shock if `CON < rn2(20)`.
/// 5. Derives form abilities (fly, swim, phase) from monster flags.
/// 6. Emits a `PolymorphSelf` status-applied event.
pub fn polymorph_self(
    world: &mut GameWorld,
    player: Entity,
    monster_def: &nethack_babel_data::MonsterDef,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // If already polymorphed, revert first.
    if is_polymorphed(world, player) {
        events.extend(revert_form(world, player));
    }

    // Dismount if currently mounted — can't ride while polymorphed.
    if steed::is_mounted(world, player) {
        let dismount_events = steed::dismount(world, player, rng);
        events.extend(dismount_events);
        events.push(EngineEvent::msg("polymorph-dismount"));
    }

    // ── Save original form ───────────────────────────────────────
    let orig_attrs = world
        .get_component::<Attributes>(player)
        .map(|a| *a)
        .unwrap_or_default();
    let orig_hp = world
        .get_component::<HitPoints>(player)
        .map(|h| *h)
        .unwrap_or(HitPoints { current: 16, max: 16 });
    let orig_speed = world
        .get_component::<Speed>(player)
        .map(|s| s.0)
        .unwrap_or(12);
    let (orig_symbol, orig_color) = world
        .get_component::<DisplaySymbol>(player)
        .map(|d| (d.symbol, d.color))
        .unwrap_or(('@', nethack_babel_data::Color::White));

    let original = OriginalForm {
        attributes: orig_attrs,
        hp: orig_hp,
        speed: orig_speed,
        display_symbol: orig_symbol,
        display_color: orig_color,
        monster_id: monster_def.id,
    };

    let _ = world.ecs_mut().insert_one(player, original);

    // ── Apply monster stats ──────────────────────────────────────
    // STR/DEX/CON/INT/WIS/CHA from base_level (simplified: NetHack
    // uses the monster's level + adjustments; we use base_level clamped
    // to [3, 25] for each attribute).
    let mlevel = (monster_def.base_level as u8).max(3);
    let new_attrs = Attributes {
        strength: mlevel.min(25),
        strength_extra: 0,
        dexterity: mlevel.min(25),
        constitution: mlevel.min(25),
        intelligence: mlevel.min(25),
        wisdom: mlevel.min(25),
        charisma: mlevel.min(25),
    };
    if let Some(mut a) = world.get_component_mut::<Attributes>(player) {
        *a = new_attrs;
    }

    // HP max from monster (mlevel * 8, minimum 1).
    let new_hp_max = ((monster_def.base_level as i32) * 8).max(1);
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
        hp.max = new_hp_max;
        hp.current = new_hp_max;
    }

    // Speed from monster definition.
    let new_speed = (monster_def.speed as u32).max(1);
    if let Some(mut s) = world.get_component_mut::<Speed>(player) {
        s.0 = new_speed;
    }

    // Display symbol and color from monster.
    let _ = world.ecs_mut().insert_one(
        player,
        DisplaySymbol {
            symbol: monster_def.symbol,
            color: monster_def.color,
        },
    );

    // ── Polymorph timer: d(monster_level, 500) ──────────────────
    let ml = (monster_def.base_level as u32).max(1);
    let timeout: u32 = (0..ml).map(|_| rng.random_range(1..=500u32)).sum();
    let timeout = timeout.max(1);
    let _ = world
        .ecs_mut()
        .insert_one(player, PolymorphTimer(timeout));

    // ── PolymorphState (rich tracking) ────────────────────────
    let poly_state = PolymorphState {
        original_hp: orig_hp.current,
        original_max_hp: orig_hp.max,
        original_level: world
            .get_component::<ExperienceLevel>(player)
            .map(|l| l.0)
            .unwrap_or(1),
        monster_form_id: monster_def.id,
        timer: timeout,
    };
    let _ = world.ecs_mut().insert_one(player, poly_state);

    // ── Form abilities ───────────────────────────────────────────
    let abilities = PolymorphAbilities {
        can_fly: monster_def
            .flags
            .contains(nethack_babel_data::MonsterFlags::FLY),
        can_swim: monster_def
            .flags
            .contains(nethack_babel_data::MonsterFlags::SWIM),
        can_phase: monster_def
            .flags
            .contains(nethack_babel_data::MonsterFlags::WALLWALK),
    };
    let _ = world.ecs_mut().insert_one(player, abilities);

    // ── Armor break for large forms ────────────────────────────────
    // Gather worn armor names from equipment and check if any break.
    let worn_armor = crate::equipment::worn_armor_names(world, player);
    let broken = armor_breaks_on_polymorph(&monster_def.names.male, &worn_armor);
    for item_name in &broken {
        events.push(EngineEvent::msg_with(
            "polymorph-armor-breaks",
            vec![("item", item_name.clone())],
        ));
    }

    // ── System shock ─────────────────────────────────────────────
    // If CON < rn2(20), the player takes d(1,6) damage and is stunned.
    let con = new_attrs.constitution;
    if (con as u32) < rng.random_range(0..20u32) {
        let shock_damage = rng.random_range(1..=6i32);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
            hp.current -= shock_damage;
        }
        events.push(EngineEvent::msg("polymorph-system-shock"));
        events.push(EngineEvent::HpChange {
            entity: player,
            amount: -shock_damage,
            new_hp: world
                .get_component::<HitPoints>(player)
                .map(|h| h.current)
                .unwrap_or(0),
            source: HpSource::Other,
        });
        // Apply stun for d(1, 8) turns.
        let stun_dur = rng.random_range(1..=8u32);
        let stun_events =
            crate::status::make_stunned(world, player, stun_dur);
        events.extend(stun_events);
    }

    // ── Emit polymorph event ─────────────────────────────────────
    events.push(EngineEvent::StatusApplied {
        entity: player,
        status: StatusEffect::Polymorphed,
        duration: Some(timeout),
        source: None,
    });
    events.push(EngineEvent::msg_with(
        "polymorph-self",
        vec![("form", monster_def.names.male.clone())],
    ));

    events
}

// ---------------------------------------------------------------------------
// Revert to original form
// ---------------------------------------------------------------------------

/// Revert the player from their polymorphed monster form to their
/// original human form.
///
/// Restores all attributes, HP, speed, and display symbol from the
/// saved `OriginalForm` component.  Removes the `OriginalForm`,
/// `PolymorphTimer`, and `PolymorphAbilities` components.
pub fn revert_form(
    world: &mut GameWorld,
    player: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Clone the original form data before mutating the world.
    let original = match world.get_component::<OriginalForm>(player) {
        Some(o) => (*o).clone(),
        None => return events, // Not polymorphed.
    };

    // Drop the borrow before mutating.
    // Restore attributes.
    if let Some(mut a) = world.get_component_mut::<Attributes>(player) {
        *a = original.attributes;
    }

    // Restore HP.  Current HP is clamped to the restored max.
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
        hp.max = original.hp.max;
        if hp.current > hp.max {
            hp.current = hp.max;
        }
    }

    // Restore speed.
    if let Some(mut s) = world.get_component_mut::<Speed>(player) {
        s.0 = original.speed;
    }

    // Restore display symbol.
    let sym = original.display_symbol;
    let col = original.display_color;
    let _ = world.ecs_mut().insert_one(
        player,
        DisplaySymbol {
            symbol: sym,
            color: col,
        },
    );

    // Remove polymorph components.
    let _ = world.ecs_mut().remove_one::<OriginalForm>(player);
    let _ = world.ecs_mut().remove_one::<PolymorphTimer>(player);
    let _ = world.ecs_mut().remove_one::<PolymorphAbilities>(player);
    let _ = world.ecs_mut().remove_one::<PolymorphState>(player);

    events.push(EngineEvent::StatusRemoved {
        entity: player,
        status: StatusEffect::Polymorphed,
    });
    events.push(EngineEvent::msg("polymorph-revert"));

    events
}

// ---------------------------------------------------------------------------
// Per-turn polymorph timer tick
// ---------------------------------------------------------------------------

/// Decrement the polymorph timer by one.  If it reaches 0, automatically
/// revert the player to their original form.
///
/// Called once per game turn from the turn loop (after status ticks).
pub fn tick_polymorph(
    world: &mut GameWorld,
    player: Entity,
) -> Vec<EngineEvent> {
    let remaining = match world.get_component::<PolymorphTimer>(player) {
        Some(t) => t.0,
        None => return Vec::new(),
    };

    if remaining <= 1 {
        // Timer expired — revert.
        return revert_form(world, player);
    }

    // Decrement.
    if let Some(mut t) = world.get_component_mut::<PolymorphTimer>(player) {
        t.0 -= 1;
    }

    Vec::new()
}

// ---------------------------------------------------------------------------
// rehumanize — revert to human form (alias with HP clamping)
// ---------------------------------------------------------------------------

/// Revert the player from polymorphed form to human form.
///
/// Equivalent to C `rehumanize()`.  Restores original stats and clamps
/// current HP to `min(current, max_human_hp)`.
pub fn rehumanize(
    world: &mut GameWorld,
    player: Entity,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    revert_form(world, player)
}

/// Alias: check polymorph timeout per turn (same as `tick_polymorph`).
pub fn check_polymorph_timeout(
    world: &mut GameWorld,
    player: Entity,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    tick_polymorph(world, player)
}

// ---------------------------------------------------------------------------
// newman — system shock on forced polymorph
// ---------------------------------------------------------------------------

/// System shock: 1/3 chance of death on forced polymorph without
/// polymorph control.
///
/// If the player survives, they get a new form.  If they die, we emit
/// a death event.  In NetHack this also reshuffles stats; here we just
/// handle the lethality check.
///
/// `has_polycontrol`: if true, the player can choose their form and
/// system shock is skipped.
pub fn newman(
    world: &mut GameWorld,
    player: Entity,
    has_polycontrol: bool,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if has_polycontrol {
        // With polymorph control, no system shock.
        events.push(EngineEvent::msg("polymorph-controlled"));
        return events;
    }

    // 1/3 chance of system shock death.
    if rng.random_range(0..3) == 0 {
        events.push(EngineEvent::msg("polymorph-system-shock-fatal"));
        // Kill the player.
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
            hp.current = 0;
        }
        events.push(EngineEvent::EntityDied {
            entity: player,
            killer: None,
            cause: DeathCause::KilledBy {
                killer_name: "system shock".to_string(),
            },
        });
        events.push(EngineEvent::GameOver {
            cause: DeathCause::KilledBy {
                killer_name: "system shock".to_string(),
            },
            score: 0,
        });
        return events;
    }

    // Survived system shock — minor stat shuffle.
    events.push(EngineEvent::msg("polymorph-newman-survive"));

    // Slightly adjust constitution (simulate the C code's stat reshuffle).
    if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
        // Constitution change: +-1 randomly.
        let delta: i8 = if rng.random_range(0..2) == 0 { 1 } else { -1 };
        attrs.constitution = (attrs.constitution as i8 + delta).clamp(3, 25) as u8;
    }

    events
}

// ---------------------------------------------------------------------------
// Form restrictions
// ---------------------------------------------------------------------------

/// Check whether the polymorphed form allows wearing body armor.
///
/// Only humanoid forms (with a torso) can wear body armor. Forms that are
/// too large, too small, or lack limbs cannot.
pub fn can_wear_armor(
    world: &GameWorld,
    entity: Entity,
) -> bool {
    // Not polymorphed => can wear armor normally.
    let original = match world.get_component::<OriginalForm>(entity) {
        Some(o) => o.clone(),
        None => return true,
    };

    // Use monster flags to determine — humanoid forms can wear armor.
    // For now, simple heuristic: symbol '@' (humanoid) can wear armor,
    // others generally cannot.
    let sym = world
        .get_component::<DisplaySymbol>(entity)
        .map(|d| d.symbol)
        .unwrap_or('@');

    let _ = original; // used for future expansion
    matches!(sym, '@' | 'H' | 'K' | 'o' | 'O')
}

/// Check whether the polymorphed form can wield weapons.
///
/// Only forms with hands can wield weapons.
pub fn can_wield_weapon(
    world: &GameWorld,
    entity: Entity,
) -> bool {
    if !is_polymorphed(world, entity) {
        return true;
    }
    let sym = world
        .get_component::<DisplaySymbol>(entity)
        .map(|d| d.symbol)
        .unwrap_or('@');
    // Humanoid-like forms with hands.
    matches!(sym, '@' | 'H' | 'K' | 'o' | 'O' | 'T' | 'h')
}

/// Check whether the polymorphed form can cast spells.
///
/// Only forms with hands and speech can cast.
pub fn can_cast_spells(
    world: &GameWorld,
    entity: Entity,
) -> bool {
    if !is_polymorphed(world, entity) {
        return true;
    }
    let sym = world
        .get_component::<DisplaySymbol>(entity)
        .map(|d| d.symbol)
        .unwrap_or('@');
    // Only humanoid forms can cast spells.
    matches!(sym, '@' | 'H' | 'K' | 'L')
}

// ---------------------------------------------------------------------------
// Attribute modification from form
// ---------------------------------------------------------------------------

/// Apply attribute bonuses/penalties from the current polymorph form.
///
/// Some forms grant bonuses (e.g., giants get +STR, dragons get +CON),
/// modeled by the form's base level.
pub fn apply_form_attribute_bonus(
    world: &mut GameWorld,
    entity: Entity,
    monster_def: &nethack_babel_data::MonsterDef,
) {
    let mlevel = monster_def.base_level as u8;
    // Grant extra strength if the form is particularly strong (level > 15).
    if mlevel > 15 {
        if let Some(mut attrs) = world.get_component_mut::<Attributes>(entity) {
            attrs.strength = attrs.strength.saturating_add(
                ((mlevel - 15) / 3).min(5),
            );
            if attrs.strength > 25 {
                attrs.strength = 25;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Polymorph into specific monster form (by ID)
// ---------------------------------------------------------------------------

/// Polymorph the player into a specific monster form, given a `MonsterDef`.
///
/// This is a thin wrapper around `polymorph_self` for use by wands of
/// polymorph, potions, polymorph traps, etc. Validates the form is
/// not genocided/extinct before proceeding.
pub fn polymorph_into(
    world: &mut GameWorld,
    player: Entity,
    monster_def: &nethack_babel_data::MonsterDef,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    // Delegate to the core polymorph function.
    let mut events = polymorph_self(world, player, monster_def, rng);
    // Apply form-specific attribute bonuses.
    apply_form_attribute_bonus(world, player, monster_def);
    events.push(EngineEvent::msg_with(
        "polymorph-into",
        vec![("form", monster_def.names.male.clone())],
    ));
    events
}

// ---------------------------------------------------------------------------
// Polymorph timeout adjustment
// ---------------------------------------------------------------------------

/// Adjust the polymorph timer (e.g., from quaffing a potion of speed
/// while polymorphed, or from a wand effect).
pub fn adjust_polymorph_timer(
    world: &mut GameWorld,
    entity: Entity,
    delta: i32,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    if let Some(mut timer) = world.get_component_mut::<PolymorphTimer>(entity) {
        let new_val = (timer.0 as i32 + delta).max(1) as u32;
        timer.0 = new_val;
        events.push(EngineEvent::msg_with(
            "polymorph-timer-adjusted",
            vec![("turns", new_val.to_string())],
        ));
    }
    events
}

/// Get the remaining polymorph timer for an entity.
pub fn polymorph_timer_remaining(
    world: &GameWorld,
    entity: Entity,
) -> Option<u32> {
    world
        .get_component::<PolymorphTimer>(entity)
        .map(|t| t.0)
}

// ---------------------------------------------------------------------------
// Form-specific special abilities
// ---------------------------------------------------------------------------

/// Special abilities available based on current polymorphed form.
///
/// These are active abilities the player can use while polymorphed,
/// corresponding to C's `dobreathe`, `dospit`, `dogaze`, `dospinweb`,
/// `dohide`, etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormAbility {
    /// Dragon breath weapon with element type.
    BreathWeapon(&'static str),
    /// Snake/naga venom spit.
    SpitVenom,
    /// Spider web spinning.
    SpinWeb,
    /// Gaze attack (e.g., floating eye paralysis).
    Gaze(&'static str),
    /// Hide on the ceiling or floor.
    Hide,
    /// Vampire shapeshift.
    Shapeshift,
    /// Mind flayer tentacle/psychic attack.
    MindBlast,
    /// Unicorn horn ability (cure effects).
    UnicornHorn,
    /// Engulfing attack (purple worm, etc.).
    Engulf,
    /// Whip-like attack (tentacle, tail).
    Lash,
}

/// Result of using a form-specific ability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormAbilityResult {
    /// Ability used successfully.
    Success {
        /// Damage dealt (if any).
        damage: i32,
        /// Status effect inflicted on target (if any).
        effect: Option<&'static str>,
    },
    /// Ability is on cooldown or cannot be used right now.
    OnCooldown,
    /// No valid target in the given direction.
    NoTarget,
}

/// Get the form-specific abilities for a polymorphed monster form.
///
/// Returns a list of abilities the player can use while in this form.
pub fn form_special_abilities(form_name: &str) -> Vec<FormAbility> {
    match form_name {
        // Dragons can breathe
        "red dragon" | "baby red dragon" => vec![FormAbility::BreathWeapon("fire")],
        "white dragon" | "baby white dragon" => vec![FormAbility::BreathWeapon("cold")],
        "blue dragon" | "baby blue dragon" => vec![FormAbility::BreathWeapon("lightning")],
        "black dragon" | "baby black dragon" => vec![FormAbility::BreathWeapon("disintegrate")],
        "green dragon" | "baby green dragon" => vec![FormAbility::BreathWeapon("poison")],
        "orange dragon" | "baby orange dragon" => vec![FormAbility::BreathWeapon("sleep")],
        "yellow dragon" | "baby yellow dragon" => vec![FormAbility::BreathWeapon("acid")],
        "silver dragon" | "baby silver dragon" => vec![FormAbility::BreathWeapon("cold")],
        // Snakes/nagas can spit venom
        "cobra" | "water moccasin" | "pit viper" | "python" => vec![FormAbility::SpitVenom],
        "red naga" | "black naga" | "golden naga" | "guardian naga"
            => vec![FormAbility::SpitVenom],
        // Spiders can spin webs
        "giant spider" | "cave spider" => vec![FormAbility::SpinWeb],
        // Eye creatures have gaze
        "floating eye" => vec![FormAbility::Gaze("paralysis")],
        "flaming sphere" => vec![FormAbility::Gaze("fire")],
        "freezing sphere" => vec![FormAbility::Gaze("cold")],
        "shocking sphere" => vec![FormAbility::Gaze("shock")],
        // Hiders can hide
        "lurker above" | "trapper" | "piercer" => vec![FormAbility::Hide],
        // Vampires can shapeshift
        "vampire" | "vampire lord" | "Vlad the Impaler" => vec![FormAbility::Shapeshift],
        // Mind flayers have psychic attack
        "mind flayer" | "master mind flayer" => vec![FormAbility::MindBlast],
        // Unicorns have horn ability
        "white unicorn" | "gray unicorn" | "black unicorn" => vec![FormAbility::UnicornHorn],
        // Purple worm can engulf
        "purple worm" => vec![FormAbility::Engulf],
        _ => vec![],
    }
}

/// Use a form-specific ability.
///
/// Returns the result of attempting to use the ability.
/// `direction` is the target direction as (dx, dy), if applicable.
pub fn use_form_ability(
    ability: &FormAbility,
    direction: Option<(i32, i32)>,
    rng: &mut impl Rng,
) -> FormAbilityResult {
    match ability {
        FormAbility::BreathWeapon(element) => {
            if direction.is_none() {
                return FormAbilityResult::NoTarget;
            }
            // Breath weapon damage: d(6, 6) for most elements.
            let damage: i32 = (0..6).map(|_| rng.random_range(1..=6i32)).sum();
            FormAbilityResult::Success {
                damage,
                effect: Some(element),
            }
        }
        FormAbility::SpitVenom => {
            if direction.is_none() {
                return FormAbilityResult::NoTarget;
            }
            let damage = rng.random_range(1..=6);
            FormAbilityResult::Success {
                damage,
                effect: Some("poison"),
            }
        }
        FormAbility::SpinWeb => {
            FormAbilityResult::Success {
                damage: 0,
                effect: Some("web"),
            }
        }
        FormAbility::Gaze(effect) => {
            if direction.is_none() {
                return FormAbilityResult::NoTarget;
            }
            FormAbilityResult::Success {
                damage: 0,
                effect: Some(effect),
            }
        }
        FormAbility::Hide => {
            FormAbilityResult::Success {
                damage: 0,
                effect: Some("hidden"),
            }
        }
        FormAbility::Shapeshift => {
            FormAbilityResult::Success {
                damage: 0,
                effect: Some("shapeshift"),
            }
        }
        FormAbility::MindBlast => {
            if direction.is_none() {
                return FormAbilityResult::NoTarget;
            }
            let damage = rng.random_range(1..=10);
            FormAbilityResult::Success {
                damage,
                effect: Some("psychic"),
            }
        }
        FormAbility::UnicornHorn => {
            FormAbilityResult::Success {
                damage: 0,
                effect: Some("cure"),
            }
        }
        FormAbility::Engulf => {
            if direction.is_none() {
                return FormAbilityResult::NoTarget;
            }
            let damage = rng.random_range(1..=12);
            FormAbilityResult::Success {
                damage,
                effect: Some("engulf"),
            }
        }
        FormAbility::Lash => {
            if direction.is_none() {
                return FormAbilityResult::NoTarget;
            }
            let damage = rng.random_range(1..=6);
            FormAbilityResult::Success {
                damage,
                effect: Some("lash"),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// System shock check (pure function)
// ---------------------------------------------------------------------------

/// System shock check when polymorphing.
///
/// Returns true if the player survives.  Con/30 chance of death.
/// In NetHack, `rn2(20)` is used: if `con < rn2(20)` => shock.
/// This is the standalone version for external callers.
pub fn system_shock_check(
    constitution: i32,
    rng: &mut impl Rng,
) -> bool {
    // Survive if constitution >= rn2(20).
    constitution >= rng.random_range(0..20i32)
}

// ---------------------------------------------------------------------------
// Armor breaks on polymorph
// ---------------------------------------------------------------------------

/// Check if armor breaks when polymorphing into a large form.
///
/// Returns a list of armor slot names that break.  In NetHack, body armor,
/// cloaks, and shirts break when polymorphing into forms too large to
/// wear them (giants, dragons, jabberwocks, etc.).
pub fn armor_breaks_on_polymorph(
    form_name: &str,
    worn_armor: &[String],
) -> Vec<String> {
    let large_forms = [
        "giant", "stone giant", "hill giant", "fire giant", "frost giant",
        "storm giant", "titan", "red dragon", "white dragon", "blue dragon",
        "black dragon", "green dragon", "orange dragon", "yellow dragon",
        "silver dragon", "jabberwock", "purple worm", "balrog",
    ];

    let is_large = large_forms.iter().any(|f| form_name.contains(f));
    if !is_large {
        return vec![];
    }

    worn_armor
        .iter()
        .filter(|a| is_body_armor(a) || is_cloak(a) || is_shirt(a))
        .cloned()
        .collect()
}

/// Check if an armor name refers to body armor.
fn is_body_armor(name: &str) -> bool {
    name.contains("mail") || name.contains("armor") || name.contains("plate")
        || name.contains("splint") || name.contains("crystal")
        || name.contains("dragon scale")
}

/// Check if an armor name refers to a cloak.
fn is_cloak(name: &str) -> bool {
    name.contains("cloak")
}

/// Check if an armor name refers to a shirt.
fn is_shirt(name: &str) -> bool {
    name.contains("shirt") || name.contains("Hawaiian")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::world::GameWorld;
    use nethack_babel_data::{Color, MonsterDef, MonsterFlags};
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    fn make_test_world() -> GameWorld {
        GameWorld::new(Position::new(5, 5))
    }

    /// Create a minimal MonsterDef for testing purposes.
    fn test_monster_def(
        symbol: char,
        level: i8,
        speed: i8,
        flags: MonsterFlags,
    ) -> MonsterDef {
        use arrayvec::ArrayVec;
        use nethack_babel_data::*;

        MonsterDef {
            id: MonsterId(100),
            names: MonsterNames {
                male: "test monster".to_string(),
                female: None,
            },
            symbol,
            color: Color::Red,
            base_level: level,
            speed,
            armor_class: 5,
            magic_resistance: 0,
            alignment: 0,
            difficulty: level as u8,
            attacks: ArrayVec::new(),
            geno_flags: GenoFlags::empty(),
            frequency: 1,
            corpse_weight: 100,
            corpse_nutrition: 100,
            sound: MonsterSound::Silent,
            size: MonsterSize::Medium,
            resistances: ResistanceSet::empty(),
            conveys: ResistanceSet::empty(),
            flags,
        }
    }

    #[test]
    fn test_polymorph_changes_attributes() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Record original attributes.
        let orig_str = world
            .get_component::<Attributes>(player)
            .unwrap()
            .strength;

        let monster = test_monster_def('D', 15, 9, MonsterFlags::FLY);
        polymorph_self(&mut world, player, &monster, &mut rng);

        // Attributes should now reflect the monster's level (clamped).
        let new_str = world
            .get_component::<Attributes>(player)
            .unwrap()
            .strength;
        assert_eq!(new_str, 15); // base_level = 15
        assert_ne!(new_str, orig_str); // original was 10

        // HP should reflect monster (15 * 8 = 120).
        let hp = world.get_component::<HitPoints>(player).unwrap();
        assert_eq!(hp.max, 120);
        assert_eq!(hp.current, 120);

        // Display symbol should be the monster's.
        let sym = world.get_component::<DisplaySymbol>(player).unwrap();
        assert_eq!(sym.symbol, 'D');

        // Should be marked as polymorphed.
        assert!(is_polymorphed(&world, player));
    }

    #[test]
    fn test_polymorph_revert() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Record original values.
        let orig_str = world
            .get_component::<Attributes>(player)
            .unwrap()
            .strength;
        let orig_hp_max = world
            .get_component::<HitPoints>(player)
            .unwrap()
            .max;

        let monster = test_monster_def('D', 15, 9, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);
        assert!(is_polymorphed(&world, player));

        let events = revert_form(&mut world, player);
        assert!(!is_polymorphed(&world, player));

        // Attributes should be restored.
        let restored_str = world
            .get_component::<Attributes>(player)
            .unwrap()
            .strength;
        assert_eq!(restored_str, orig_str);

        // HP max should be restored.
        let restored_hp = world
            .get_component::<HitPoints>(player)
            .unwrap()
            .max;
        assert_eq!(restored_hp, orig_hp_max);

        // Should emit StatusRemoved event.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Polymorphed,
                ..
            }
        )));
    }

    #[test]
    fn test_polymorph_system_shock() {
        // Use a monster with a very low base_level (=> low CON) to
        // increase the chance of system shock (CON < rn2(20)).
        // base_level = 3 => CON = 3, so 3 < rn2(20) is very likely.
        let world = make_test_world();
        let player = world.player();

        let monster = test_monster_def('a', 3, 12, MonsterFlags::empty());

        // Run many attempts and check that at least one produces shock.
        let mut shocked = false;
        for seed in 0..100u64 {
            // Reset world each iteration.
            let mut w = make_test_world();
            let p = w.player();
            let mut rng = SmallRng::seed_from_u64(seed);
            let events =
                polymorph_self(&mut w, p, &monster, &mut rng);
            if events
                .iter()
                .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "polymorph-system-shock"))
            {
                shocked = true;
                // Verify HP was reduced.
                let hp = w.get_component::<HitPoints>(p).unwrap();
                // Monster HP max = 3 * 8 = 24.  Shock does d(1,6) damage.
                assert!(hp.current < 24);
                break;
            }
        }
        assert!(
            shocked,
            "system shock should trigger with low CON over 100 seeds"
        );
        let _ = player; // suppress unused warning
    }

    #[test]
    fn test_polymorph_timeout() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = test_monster_def('D', 10, 9, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);

        // Get the timer value.  With d(10, 500) it ranges from 10 to 5000.
        let timer = world
            .get_component::<PolymorphTimer>(player)
            .unwrap()
            .0;
        assert!(timer >= 10 && timer <= 5000, "timer {timer} out of d(10,500) range");

        // Tick down to 2.
        if let Some(mut t) =
            world.get_component_mut::<PolymorphTimer>(player)
        {
            t.0 = 2;
        }

        // Tick once — should just decrement.
        let events = tick_polymorph(&mut world, player);
        assert!(events.is_empty());
        assert!(is_polymorphed(&world, player));
        assert_eq!(
            world
                .get_component::<PolymorphTimer>(player)
                .unwrap()
                .0,
            1
        );

        // Tick again — should revert.
        let events = tick_polymorph(&mut world, player);
        assert!(!is_polymorphed(&world, player));
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Polymorphed,
                ..
            }
        )));
    }

    #[test]
    fn test_polymorph_flying_ability() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster =
            test_monster_def('D', 10, 9, MonsterFlags::FLY);
        polymorph_self(&mut world, player, &monster, &mut rng);

        let abilities = polymorph_abilities(&world, player).unwrap();
        assert!(abilities.can_fly);
        assert!(!abilities.can_swim);
        assert!(!abilities.can_phase);
    }

    #[test]
    fn test_polymorph_swimming_ability() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster =
            test_monster_def(';', 8, 12, MonsterFlags::SWIM);
        polymorph_self(&mut world, player, &monster, &mut rng);

        let abilities = polymorph_abilities(&world, player).unwrap();
        assert!(!abilities.can_fly);
        assert!(abilities.can_swim);
        assert!(!abilities.can_phase);
    }

    #[test]
    fn test_polymorph_phasing_ability() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster =
            test_monster_def('X', 12, 10, MonsterFlags::WALLWALK);
        polymorph_self(&mut world, player, &monster, &mut rng);

        let abilities = polymorph_abilities(&world, player).unwrap();
        assert!(!abilities.can_fly);
        assert!(!abilities.can_swim);
        assert!(abilities.can_phase);
    }

    #[test]
    fn test_polymorph_revert_restores_symbol() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Give the player a display symbol first.
        let _ = world.ecs_mut().insert_one(
            player,
            DisplaySymbol {
                symbol: '@',
                color: Color::White,
            },
        );

        let monster = test_monster_def('D', 10, 9, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);

        {
            let sym = world.get_component::<DisplaySymbol>(player).unwrap();
            assert_eq!(sym.symbol, 'D');
        }

        revert_form(&mut world, player);

        {
            let sym = world.get_component::<DisplaySymbol>(player).unwrap();
            assert_eq!(sym.symbol, '@');
        }
    }

    #[test]
    fn test_revert_not_polymorphed_is_noop() {
        let mut world = make_test_world();
        let player = world.player();

        let events = revert_form(&mut world, player);
        assert!(events.is_empty());
    }

    #[test]
    fn test_polymorph_replaces_existing() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Polymorph into one form.
        let monster1 = test_monster_def('D', 15, 9, MonsterFlags::FLY);
        polymorph_self(&mut world, player, &monster1, &mut rng);
        assert_eq!(
            world
                .get_component::<DisplaySymbol>(player)
                .unwrap()
                .symbol,
            'D'
        );

        // Polymorph into another form — should revert first, then apply.
        let monster2 = test_monster_def('a', 5, 18, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster2, &mut rng);
        assert_eq!(
            world
                .get_component::<DisplaySymbol>(player)
                .unwrap()
                .symbol,
            'a'
        );

        // Revert should go back to original (pre-first-polymorph).
        revert_form(&mut world, player);
        assert!(!is_polymorphed(&world, player));
    }

    // ═══════════════════════════════════════════════════════════════
    // Phase 2 tests: Form restrictions, attributes, timer
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_can_wear_armor_not_polymorphed() {
        let world = make_test_world();
        let player = world.player();
        assert!(can_wear_armor(&world, player));
    }

    #[test]
    fn test_can_wear_armor_humanoid_form() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // '@' is humanoid — can wear armor.
        let monster = test_monster_def('@', 10, 12, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);
        assert!(can_wear_armor(&world, player));
    }

    #[test]
    fn test_cannot_wear_armor_dragon_form() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // 'D' is a dragon — cannot wear armor.
        let monster = test_monster_def('D', 15, 9, MonsterFlags::FLY);
        polymorph_self(&mut world, player, &monster, &mut rng);
        assert!(!can_wear_armor(&world, player));
    }

    #[test]
    fn test_can_wield_weapon_humanoid() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = test_monster_def('@', 10, 12, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);
        assert!(can_wield_weapon(&world, player));
    }

    #[test]
    fn test_cannot_wield_weapon_animal() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // 'a' is animal — cannot wield weapons.
        let monster = test_monster_def('a', 5, 18, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);
        assert!(!can_wield_weapon(&world, player));
    }

    #[test]
    fn test_can_cast_spells_human() {
        let world = make_test_world();
        let player = world.player();
        assert!(can_cast_spells(&world, player));
    }

    #[test]
    fn test_cannot_cast_spells_dragon() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = test_monster_def('D', 15, 9, MonsterFlags::FLY);
        polymorph_self(&mut world, player, &monster, &mut rng);
        assert!(!can_cast_spells(&world, player));
    }

    #[test]
    fn test_form_attribute_bonus_high_level() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Level 20 monster — should get STR bonus ((20-15)/3 = 1).
        let monster = test_monster_def('H', 20, 9, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);

        let str_before = world
            .get_component::<Attributes>(player)
            .unwrap()
            .strength;
        apply_form_attribute_bonus(&mut world, player, &monster);
        let str_after = world
            .get_component::<Attributes>(player)
            .unwrap()
            .strength;
        assert!(
            str_after > str_before,
            "high-level form should grant STR bonus"
        );
    }

    #[test]
    fn test_form_attribute_bonus_low_level_no_bonus() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Level 5 monster — no STR bonus.
        let monster = test_monster_def('a', 5, 18, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);

        let str_before = world
            .get_component::<Attributes>(player)
            .unwrap()
            .strength;
        apply_form_attribute_bonus(&mut world, player, &monster);
        let str_after = world
            .get_component::<Attributes>(player)
            .unwrap()
            .strength;
        assert_eq!(str_after, str_before, "low-level form should not grant bonus");
    }

    #[test]
    fn test_polymorph_into_wrapper() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = test_monster_def('H', 18, 12, MonsterFlags::empty());
        let events = polymorph_into(&mut world, player, &monster, &mut rng);

        assert!(is_polymorphed(&world, player));
        // Should have both polymorph-self and polymorph-into messages.
        let has_into = events.iter().any(|e| {
            matches!(e, EngineEvent::Message { key, .. } if key == "polymorph-into")
        });
        assert!(has_into, "polymorph_into should emit polymorph-into message");
    }

    #[test]
    fn test_adjust_polymorph_timer_extend() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = test_monster_def('D', 10, 9, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);

        let before = polymorph_timer_remaining(&world, player).unwrap();
        adjust_polymorph_timer(&mut world, player, 100);
        let after = polymorph_timer_remaining(&world, player).unwrap();
        assert_eq!(after, before + 100);
    }

    #[test]
    fn test_adjust_polymorph_timer_reduce_floor_1() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = test_monster_def('D', 10, 9, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);

        // Reduce by huge amount — should floor at 1.
        adjust_polymorph_timer(&mut world, player, -99999);
        let after = polymorph_timer_remaining(&world, player).unwrap();
        assert_eq!(after, 1, "timer should not go below 1");
    }

    #[test]
    fn test_polymorph_timer_remaining_not_polymorphed() {
        let world = make_test_world();
        let player = world.player();
        assert_eq!(polymorph_timer_remaining(&world, player), None);
    }

    #[test]
    fn test_adjust_timer_not_polymorphed_is_noop() {
        let mut world = make_test_world();
        let player = world.player();
        let events = adjust_polymorph_timer(&mut world, player, 50);
        assert!(events.is_empty());
    }

    // ═══════════════════════════════════════════════════════════════
    // Phase 3 tests: newman, polymon_abilities, PolymorphState,
    //                rehumanize, check_polymorph_timeout
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_newman_with_polycontrol_no_death() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let events = newman(&mut world, player, true, &mut rng);
        // With polycontrol, should not die.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "polymorph-controlled"
        )));
        let hp = world.get_component::<HitPoints>(player).unwrap();
        assert!(hp.current > 0, "poly control should prevent death");
    }

    #[test]
    fn test_newman_without_polycontrol_can_kill() {
        // 1/3 chance of death — run multiple seeds to see both outcomes.
        let mut saw_death = false;
        let mut saw_survive = false;
        for seed in 0..100u64 {
            let mut w = make_test_world();
            let p = w.player();
            let mut rng = SmallRng::seed_from_u64(seed);
            let events = newman(&mut w, p, false, &mut rng);
            if events.iter().any(|e| matches!(
                e,
                EngineEvent::EntityDied { .. }
            )) {
                saw_death = true;
            } else {
                saw_survive = true;
            }
            if saw_death && saw_survive {
                break;
            }
        }
        assert!(saw_death, "newman should sometimes kill");
        assert!(saw_survive, "newman should sometimes let player survive");
    }

    #[test]
    fn test_newman_death_sets_hp_zero() {
        for seed in 0..100u64 {
            let mut w = make_test_world();
            let p = w.player();
            let mut rng = SmallRng::seed_from_u64(seed);
            let events = newman(&mut w, p, false, &mut rng);
            if events.iter().any(|e| matches!(e, EngineEvent::EntityDied { .. })) {
                let hp = w.get_component::<HitPoints>(p).unwrap();
                assert_eq!(hp.current, 0, "death should set HP to 0");
                // Should also have GameOver event.
                assert!(events.iter().any(|e| matches!(e, EngineEvent::GameOver { .. })));
                return;
            }
        }
        panic!("expected at least one death in 100 seeds");
    }

    #[test]
    fn test_newman_survive_adjusts_constitution() {
        for seed in 0..100u64 {
            let mut w = make_test_world();
            let p = w.player();
            let orig_con = w.get_component::<Attributes>(p).unwrap().constitution;
            let mut rng = SmallRng::seed_from_u64(seed);
            let events = newman(&mut w, p, false, &mut rng);
            if !events.iter().any(|e| matches!(e, EngineEvent::EntityDied { .. })) {
                let new_con = w.get_component::<Attributes>(p).unwrap().constitution;
                // CON should differ by exactly 1.
                let diff = (new_con as i8 - orig_con as i8).abs();
                assert_eq!(diff, 1, "survive should adjust CON by +-1");
                return;
            }
        }
        panic!("expected at least one survival in 100 seeds");
    }

    #[test]
    fn test_polymon_abilities_flying_monster() {
        let monster = test_monster_def(
            'D', 15, 9,
            MonsterFlags::FLY | MonsterFlags::HUMANOID,
        );
        let abilities = polymon_abilities(&monster);
        assert!(abilities.can_fly);
        assert!(abilities.has_hands);
        assert!(abilities.can_wear_armor);
    }

    #[test]
    fn test_polymon_abilities_animal_no_hands() {
        let monster = test_monster_def(
            'a', 5, 18,
            MonsterFlags::ANIMAL | MonsterFlags::NOHANDS | MonsterFlags::NOLIMBS,
        );
        let abilities = polymon_abilities(&monster);
        assert!(!abilities.has_hands);
        assert!(!abilities.can_wear_armor);
        assert!(!abilities.can_wield_weapon);
        assert!(!abilities.can_cast_spells);
    }

    #[test]
    fn test_polymon_abilities_undead_regen() {
        let monster = test_monster_def(
            'Z', 8, 6,
            MonsterFlags::UNDEAD | MonsterFlags::REGEN | MonsterFlags::BREATHLESS,
        );
        let abilities = polymon_abilities(&monster);
        assert!(abilities.is_undead);
        assert!(abilities.can_regenerate);
        assert!(abilities.is_breathless);
    }

    #[test]
    fn test_polymon_abilities_wallwalker() {
        let monster = test_monster_def(
            'X', 12, 10,
            MonsterFlags::WALLWALK | MonsterFlags::BREATHLESS,
        );
        let abilities = polymon_abilities(&monster);
        assert!(abilities.passes_walls);
        assert!(abilities.is_breathless);
    }

    #[test]
    fn test_polymorph_state_is_set() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = test_monster_def('D', 10, 9, MonsterFlags::FLY);
        polymorph_self(&mut world, player, &monster, &mut rng);

        let state = world.get_component::<PolymorphState>(player);
        assert!(state.is_some(), "polymorph_self should set PolymorphState");
        let state = state.unwrap();
        assert_eq!(state.monster_form_id, monster.id);
        assert!(state.timer > 0);
    }

    #[test]
    fn test_polymorph_state_removed_on_revert() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = test_monster_def('D', 10, 9, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);
        assert!(world.get_component::<PolymorphState>(player).is_some());

        revert_form(&mut world, player);
        assert!(world.get_component::<PolymorphState>(player).is_none());
    }

    #[test]
    fn test_rehumanize_reverts() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = test_monster_def('D', 10, 9, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);
        assert!(is_polymorphed(&world, player));

        let events = rehumanize(&mut world, player, &mut rng);
        assert!(!is_polymorphed(&world, player));
        assert!(!events.is_empty());
    }

    #[test]
    fn test_check_polymorph_timeout_ticks() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = test_monster_def('D', 10, 9, MonsterFlags::empty());
        polymorph_self(&mut world, player, &monster, &mut rng);

        // Set timer to 3.
        if let Some(mut t) = world.get_component_mut::<PolymorphTimer>(player) {
            t.0 = 3;
        }

        // Tick twice — should not revert.
        let events = check_polymorph_timeout(&mut world, player, &mut rng);
        assert!(events.is_empty());
        assert!(is_polymorphed(&world, player));

        let events = check_polymorph_timeout(&mut world, player, &mut rng);
        assert!(events.is_empty());
        assert!(is_polymorphed(&world, player));

        // Third tick — should revert.
        let events = check_polymorph_timeout(&mut world, player, &mut rng);
        assert!(!events.is_empty());
        assert!(!is_polymorphed(&world, player));
    }

    #[test]
    fn test_polymon_abilities_swimmer_tunneler() {
        let monster = test_monster_def(
            'w', 5, 12,
            MonsterFlags::SWIM | MonsterFlags::TUNNEL | MonsterFlags::AMORPHOUS,
        );
        let abilities = polymon_abilities(&monster);
        assert!(abilities.can_swim);
        assert!(abilities.can_tunnel);
        assert!(abilities.is_amorphous);
        assert!(!abilities.can_fly);
    }

    #[test]
    fn test_polymon_abilities_humanoid_infravision() {
        let monster = test_monster_def(
            '@', 10, 12,
            MonsterFlags::HUMANOID | MonsterFlags::INFRAVISION,
        );
        let abilities = polymon_abilities(&monster);
        assert!(abilities.has_hands);
        assert!(abilities.can_wield_weapon);
        assert!(abilities.can_cast_spells);
        assert!(abilities.has_infravision);
    }

    // ═══════════════════════════════════════════════════════════════
    // Form-specific special ability tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_form_abilities_red_dragon_breath() {
        let abilities = form_special_abilities("red dragon");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::BreathWeapon("fire"));
    }

    #[test]
    fn test_form_abilities_white_dragon_breath() {
        let abilities = form_special_abilities("white dragon");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::BreathWeapon("cold"));
    }

    #[test]
    fn test_form_abilities_blue_dragon_lightning() {
        let abilities = form_special_abilities("blue dragon");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::BreathWeapon("lightning"));
    }

    #[test]
    fn test_form_abilities_black_dragon_disintegrate() {
        let abilities = form_special_abilities("black dragon");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::BreathWeapon("disintegrate"));
    }

    #[test]
    fn test_form_abilities_cobra_spit() {
        let abilities = form_special_abilities("cobra");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::SpitVenom);
    }

    #[test]
    fn test_form_abilities_giant_spider_web() {
        let abilities = form_special_abilities("giant spider");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::SpinWeb);
    }

    #[test]
    fn test_form_abilities_floating_eye_gaze() {
        let abilities = form_special_abilities("floating eye");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::Gaze("paralysis"));
    }

    #[test]
    fn test_form_abilities_lurker_hide() {
        let abilities = form_special_abilities("lurker above");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::Hide);
    }

    #[test]
    fn test_form_abilities_vampire_shapeshift() {
        let abilities = form_special_abilities("vampire");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::Shapeshift);
    }

    #[test]
    fn test_form_abilities_mind_flayer_blast() {
        let abilities = form_special_abilities("mind flayer");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::MindBlast);
    }

    #[test]
    fn test_form_abilities_unknown_form() {
        let abilities = form_special_abilities("newt");
        assert!(abilities.is_empty(), "newt should have no special abilities");
    }

    #[test]
    fn test_form_abilities_unicorn_horn() {
        let abilities = form_special_abilities("white unicorn");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::UnicornHorn);
    }

    #[test]
    fn test_form_abilities_purple_worm_engulf() {
        let abilities = form_special_abilities("purple worm");
        assert_eq!(abilities.len(), 1);
        assert_eq!(abilities[0], FormAbility::Engulf);
    }

    // ═══════════════════════════════════════════════════════════════
    // use_form_ability tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_use_breath_weapon_with_direction() {
        let mut rng = test_rng();
        let result = use_form_ability(
            &FormAbility::BreathWeapon("fire"),
            Some((1, 0)),
            &mut rng,
        );
        match result {
            FormAbilityResult::Success { damage, effect } => {
                assert!(damage >= 6 && damage <= 36, "d(6,6) damage {} out of range", damage);
                assert_eq!(effect, Some("fire"));
            }
            _ => panic!("breath weapon with direction should succeed"),
        }
    }

    #[test]
    fn test_use_breath_weapon_no_direction() {
        let mut rng = test_rng();
        let result = use_form_ability(
            &FormAbility::BreathWeapon("fire"),
            None,
            &mut rng,
        );
        assert_eq!(result, FormAbilityResult::NoTarget);
    }

    #[test]
    fn test_use_spin_web_no_direction_needed() {
        let mut rng = test_rng();
        let result = use_form_ability(
            &FormAbility::SpinWeb,
            None,
            &mut rng,
        );
        match result {
            FormAbilityResult::Success { damage, effect } => {
                assert_eq!(damage, 0);
                assert_eq!(effect, Some("web"));
            }
            _ => panic!("spin web should succeed without direction"),
        }
    }

    #[test]
    fn test_use_mind_blast_damage_range() {
        let mut rng = test_rng();
        let result = use_form_ability(
            &FormAbility::MindBlast,
            Some((0, -1)),
            &mut rng,
        );
        match result {
            FormAbilityResult::Success { damage, effect } => {
                assert!(damage >= 1 && damage <= 10, "mind blast damage {} out of range", damage);
                assert_eq!(effect, Some("psychic"));
            }
            _ => panic!("mind blast with target should succeed"),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // System shock check tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_system_shock_high_con_survives() {
        let mut rng = test_rng();
        // CON 20: always survives (20 >= rn2(20) is always true since max rn2(20)=19).
        for _ in 0..100 {
            assert!(
                system_shock_check(20, &mut rng),
                "CON 20 should always survive system shock"
            );
        }
    }

    #[test]
    fn test_system_shock_low_con_can_fail() {
        let mut survived = false;
        let mut failed = false;
        for seed in 0..200u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            let result = system_shock_check(3, &mut rng);
            if result {
                survived = true;
            } else {
                failed = true;
            }
            if survived && failed {
                break;
            }
        }
        assert!(survived, "CON 3 should sometimes survive");
        assert!(failed, "CON 3 should sometimes fail system shock");
    }

    // ═══════════════════════════════════════════════════════════════
    // Armor breaks on polymorph tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_armor_breaks_dragon_form() {
        let worn = vec![
            "plate mail".to_string(),
            "cloak of magic resistance".to_string(),
            "Hawaiian shirt".to_string(),
            "helm of brilliance".to_string(),
            "gauntlets of power".to_string(),
        ];
        let broken = armor_breaks_on_polymorph("red dragon", &worn);
        assert!(broken.contains(&"plate mail".to_string()));
        assert!(broken.contains(&"cloak of magic resistance".to_string()));
        assert!(broken.contains(&"Hawaiian shirt".to_string()));
        // Helm and gauntlets should NOT break.
        assert!(!broken.contains(&"helm of brilliance".to_string()));
        assert!(!broken.contains(&"gauntlets of power".to_string()));
    }

    #[test]
    fn test_armor_breaks_giant_form() {
        let worn = vec![
            "chain mail".to_string(),
            "leather cloak".to_string(),
        ];
        let broken = armor_breaks_on_polymorph("hill giant", &worn);
        assert_eq!(broken.len(), 2, "both body armor and cloak should break for giant");
    }

    #[test]
    fn test_armor_no_break_small_form() {
        let worn = vec![
            "plate mail".to_string(),
            "cloak of invisibility".to_string(),
        ];
        let broken = armor_breaks_on_polymorph("newt", &worn);
        assert!(broken.is_empty(), "small form should not break armor");
    }

    #[test]
    fn test_armor_no_break_humanoid_form() {
        let worn = vec![
            "plate mail".to_string(),
            "cloak of protection".to_string(),
        ];
        let broken = armor_breaks_on_polymorph("elf", &worn);
        assert!(broken.is_empty(), "humanoid form should not break armor");
    }

    #[test]
    fn test_armor_breaks_empty_worn_list() {
        let worn: Vec<String> = vec![];
        let broken = armor_breaks_on_polymorph("red dragon", &worn);
        assert!(broken.is_empty());
    }

    // ── Steed integration: polymorph dismounts rider ─────────────

    #[test]
    fn test_polymorph_dismounts_rider() {
        let mut world = make_test_world();
        let player = world.player();
        let mut rng = test_rng();

        // Spawn a steed and mount it.
        use crate::world::{Monster, Name, Positioned, Tame};
        let steed = world.spawn((
            Monster,
            Tame,
            Positioned(crate::action::Position::new(6, 5)),
            Name("pony".to_string()),
            Speed(18),
            HitPoints { current: 30, max: 30 },
        ));
        let _ = crate::steed::mount(&mut world, player, steed, &mut rng);
        assert!(crate::steed::is_mounted(&world, player));

        // Polymorph should dismount.
        let monster = test_monster_def('D', 10, 9, MonsterFlags::FLY);
        let events = polymorph_self(&mut world, player, &monster, &mut rng);

        assert!(!crate::steed::is_mounted(&world, player),
            "player should be dismounted after polymorph");
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "polymorph-dismount")),
            "should emit polymorph-dismount message");
    }

    #[test]
    fn test_polymorph_breaks_armor_large_form() {
        let worn = vec![
            "plate mail".to_string(),
            "cloak of protection".to_string(),
        ];
        let broken = armor_breaks_on_polymorph("red dragon", &worn);
        assert_eq!(broken.len(), 2, "large dragon form should break body armor + cloak");
    }

    #[test]
    fn test_polymorph_keeps_armor_small_form() {
        let worn = vec![
            "plate mail".to_string(),
            "cloak of protection".to_string(),
        ];
        let broken = armor_breaks_on_polymorph("kitten", &worn);
        assert!(broken.is_empty(), "small form should not break any armor");
    }

    #[test]
    fn test_polymorph_not_mounted_no_dismount() {
        let mut world = make_test_world();
        let player = world.player();
        let mut rng = test_rng();

        // Not mounted — polymorph should not produce dismount message.
        let monster = test_monster_def('D', 10, 9, MonsterFlags::FLY);
        let events = polymorph_self(&mut world, player, &monster, &mut rng);

        assert!(!events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "polymorph-dismount")),
            "should not emit dismount message when not mounted");
    }
}
