//! Status effects and intrinsics system for NetHack Babel.
//!
//! Implements the NetHack property system (see `specs/status-timeout.md`):
//! - Timed status effects with per-turn countdown
//! - Permanent intrinsics (from corpses, race, class)
//! - Deadly countdown sequences (stoning, sliming, food poisoning)
//! - Status acquisition, curing, and stacking rules
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::event::{DeathCause, EngineEvent, StatusEffect};
use crate::world::{GameWorld, HeroSpeed, HeroSpeedBonus};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum value for a timed status (24-bit, matching NetHack's TIMEOUT mask).
pub const TIMEOUT_MAX: u32 = 0x00FF_FFFF;

/// Initial stoning countdown (5 turns).
pub const STONING_INITIAL: u32 = 5;

/// Initial sliming countdown (10 turns).
pub const SLIMING_INITIAL: u32 = 10;

/// Initial strangling countdown (5 turns).
pub const STRANGLED_INITIAL: u32 = 5;

// ---------------------------------------------------------------------------
// Sick sub-type flags (matches NetHack's usick_type)
// ---------------------------------------------------------------------------

/// Food poisoning (curable by vomiting).
pub const SICK_VOMITABLE: u8 = 0x01;
/// Disease from monster attack (not curable by vomiting).
pub const SICK_NONVOMITABLE: u8 = 0x02;
/// Both types combined.
#[allow(dead_code)]
pub const SICK_ALL: u8 = 0x03;

// ---------------------------------------------------------------------------
// StatusEffects component
// ---------------------------------------------------------------------------

/// Component: all timed status effects on an entity.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatusEffects {
    pub confusion: u32,
    pub stun: u32,
    pub blindness: u32,
    pub hallucination: u32,
    pub clairvoyance: u32,
    pub deaf: u32,
    pub glib: u32,
    pub stoning: u32,
    pub sliming: u32,
    pub sick: u32,
    pub sick_type: u8,
    pub strangled: u32,
    pub vomiting: u32,
    pub levitation: u32,
    pub invisibility: u32,
    pub see_invisible: u32,
    pub speed: u32,
    pub protection: u32,
    pub acid_resistance: u32,
    pub stone_resistance: u32,
    pub paralysis: u32,
    /// Fumbling timer (self-cycling: on expiry, hero trips and timer resets).
    pub fumbling: u32,
    /// Sleepy timer (self-cycling: on expiry, hero falls asleep and timer resets).
    pub sleepy: u32,
    /// Wounded legs recovery timer.
    pub wounded_legs: u32,
    /// Passes_walls (phasing) temporary timer from prayer.
    pub passes_walls: u32,
    /// Magical_breathing temporary timer from prayer.
    pub magical_breathing: u32,
}

/// Component: spell-based AC protection state.
/// Dissipates one layer at a time via `usptime` countdown.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpellProtection {
    /// Number of AC bonus layers remaining.
    pub layers: u8,
    /// Turns remaining until next layer dissipates.
    pub countdown: u32,
    /// Per-layer interval (Expert: 20, other: 10).
    pub interval: u32,
}

/// Component: miscellaneous hero counters decremented per turn.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HeroCounters {
    /// Cream-on-face countdown (ucreamed).
    pub creamed: u32,
    /// Gallop countdown (from fast steed).
    pub gallop: u32,
}

impl StatusEffects {
    #[inline]
    pub fn clamp_timeout(val: u32) -> u32 {
        val.min(TIMEOUT_MAX)
    }

    #[inline]
    pub fn set_timeout(field: &mut u32, val: u32) {
        *field = Self::clamp_timeout(val);
    }

    #[inline]
    pub fn incr_timeout(field: &mut u32, incr: u32) {
        Self::set_timeout(field, field.saturating_add(incr));
    }
}

// ---------------------------------------------------------------------------
// Intrinsics component
// ---------------------------------------------------------------------------

/// Component: permanent intrinsic properties.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Intrinsics {
    pub fire_resistance: bool,
    pub cold_resistance: bool,
    pub sleep_resistance: bool,
    pub shock_resistance: bool,
    pub poison_resistance: bool,
    pub disintegration_resistance: bool,
    pub drain_resistance: bool,
    pub telepathy: bool,
    pub see_invisible: bool,
    pub infravision: bool,
    pub warning: bool,
    pub searching: bool,
    pub teleportitis: bool,
    pub teleport_control: bool,
    pub stealth: bool,
    pub regeneration: bool,
    pub polymorph_control: bool,
    pub giant_strength_gained: u8,
}

// ---------------------------------------------------------------------------
// Status queries
// ---------------------------------------------------------------------------

pub fn is_confused(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.confusion > 0)
}

pub fn is_stunned(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.stun > 0)
}

pub fn is_blind(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.blindness > 0)
}

pub fn is_hallucinating(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.hallucination > 0)
}

pub fn is_clairvoyant(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.clairvoyance > 0)
}

pub fn is_levitating(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.levitation > 0)
}

pub fn is_stoning(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.stoning > 0)
}

pub fn is_sliming(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.sliming > 0)
}

pub fn is_sick(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.sick > 0)
}

pub fn is_food_poisoned(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.sick > 0 && (s.sick_type & SICK_VOMITABLE) != 0)
}

pub fn is_strangled(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.strangled > 0)
}

pub fn is_vomiting(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.vomiting > 0)
}

pub fn is_paralyzed(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.paralysis > 0)
}

pub fn is_fumbling(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.fumbling > 0)
}

pub fn is_sleepy(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.sleepy > 0)
}

pub fn has_wounded_legs(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.wounded_legs > 0)
}

pub fn is_phasing(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<StatusEffects>(entity)
        .is_some_and(|s| s.passes_walls > 0)
}

pub fn has_intrinsic_fire_res(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<Intrinsics>(entity)
        .is_some_and(|i| i.fire_resistance)
}
pub fn has_intrinsic_cold_res(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<Intrinsics>(entity)
        .is_some_and(|i| i.cold_resistance)
}
pub fn has_intrinsic_sleep_res(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<Intrinsics>(entity)
        .is_some_and(|i| i.sleep_resistance)
}
pub fn has_intrinsic_shock_res(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<Intrinsics>(entity)
        .is_some_and(|i| i.shock_resistance)
}
pub fn has_intrinsic_poison_res(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<Intrinsics>(entity)
        .is_some_and(|i| i.poison_resistance)
}
pub fn has_intrinsic_disint_res(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<Intrinsics>(entity)
        .is_some_and(|i| i.disintegration_resistance)
}
pub fn has_intrinsic_telepathy(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<Intrinsics>(entity)
        .is_some_and(|i| i.telepathy)
}
pub fn has_intrinsic_teleportitis(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<Intrinsics>(entity)
        .is_some_and(|i| i.teleportitis)
}
pub fn has_intrinsic_teleport_control(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<Intrinsics>(entity)
        .is_some_and(|i| i.teleport_control)
}

// ---------------------------------------------------------------------------
// Status application helpers
// ---------------------------------------------------------------------------

fn apply_field(
    world: &mut GameWorld,
    entity: Entity,
    get: fn(&StatusEffects) -> u32,
    set: fn(&mut StatusEffects, u32),
    duration: u32,
) -> Option<(bool, bool)> {
    let mut s = world.get_component_mut::<StatusEffects>(entity)?;
    let was = get(&s) > 0;
    set(&mut s, StatusEffects::clamp_timeout(duration));
    let is = get(&s) > 0;
    Some((was, is))
}

pub fn make_confused(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.confusion, |s, v| s.confusion = v, dur) {
        if !was && is {
            ev.push(EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Confused,
                duration: Some(dur),
                source: None,
            });
        } else if was && !is {
            ev.push(EngineEvent::msg("status-confusion-end"));
            ev.push(EngineEvent::StatusRemoved {
                entity: e,
                status: StatusEffect::Confused,
            });
        }
    }
    ev
}

pub fn make_stunned(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.stun, |s, v| s.stun = v, dur) {
        if !was && is {
            ev.push(EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Stunned,
                duration: Some(dur),
                source: None,
            });
        } else if was && !is {
            ev.push(EngineEvent::msg("status-stun-end"));
            ev.push(EngineEvent::StatusRemoved {
                entity: e,
                status: StatusEffect::Stunned,
            });
        }
    }
    ev
}

pub fn make_blinded(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.blindness, |s, v| s.blindness = v, dur) {
        if !was && is {
            ev.push(EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Blind,
                duration: Some(dur),
                source: None,
            });
        } else if was && !is {
            ev.push(EngineEvent::msg("status-blindness-end"));
            ev.push(EngineEvent::StatusRemoved {
                entity: e,
                status: StatusEffect::Blind,
            });
        }
    }
    ev
}

pub fn make_hallucinated(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.hallucination, |s, v| s.hallucination = v, dur)
    {
        if !was && is {
            ev.push(EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Hallucinating,
                duration: Some(dur),
                source: None,
            });
        } else if was && !is {
            ev.push(EngineEvent::msg("status-hallucination-end"));
            ev.push(EngineEvent::StatusRemoved {
                entity: e,
                status: StatusEffect::Hallucinating,
            });
        }
    }
    ev
}

pub fn make_clairvoyant(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.clairvoyance, |s, v| s.clairvoyance = v, dur) {
        if !was && is {
            ev.push(EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Clairvoyant,
                duration: Some(dur),
                source: None,
            });
        } else if was && !is {
            ev.push(EngineEvent::msg("status-clairvoyance-end"));
            ev.push(EngineEvent::StatusRemoved {
                entity: e,
                status: StatusEffect::Clairvoyant,
            });
        }
    }
    ev
}

pub fn make_levitating(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.levitation, |s, v| s.levitation = v, dur) {
        if !was && is {
            ev.push(EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Levitating,
                duration: Some(dur),
                source: None,
            });
        } else if was && !is {
            ev.push(EngineEvent::msg("status-levitation-end"));
            ev.push(EngineEvent::StatusRemoved {
                entity: e,
                status: StatusEffect::Levitating,
            });
        }
    }
    ev
}

pub fn make_stoned(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.stoning, |s, v| s.stoning = v, dur) {
        if !was && is {
            ev.push(EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Stoning,
                duration: Some(dur),
                source: None,
            });
        } else if was && !is {
            ev.push(EngineEvent::StatusRemoved {
                entity: e,
                status: StatusEffect::Stoning,
            });
        }
    }
    ev
}

pub fn make_slimed(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.sliming, |s, v| s.sliming = v, dur) {
        if !was && is {
            ev.push(EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Slimed,
                duration: Some(dur),
                source: None,
            });
        } else if was && !is {
            ev.push(EngineEvent::StatusRemoved {
                entity: e,
                status: StatusEffect::Slimed,
            });
        }
    }
    ev
}

pub fn make_sick(w: &mut GameWorld, e: Entity, dur: u32, sick_type: u8) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    let r = {
        let mut s = match w.get_component_mut::<StatusEffects>(e) {
            Some(s) => s,
            None => return ev,
        };
        let was = s.sick > 0;
        s.sick = StatusEffects::clamp_timeout(dur);
        s.sick_type |= sick_type;
        let is = s.sick > 0;
        if was && !is {
            s.sick_type = 0;
        }
        (was, is)
    };
    if !r.0 && r.1 {
        let st = if (sick_type & SICK_VOMITABLE) != 0 {
            StatusEffect::FoodPoisoned
        } else {
            StatusEffect::Sick
        };
        ev.push(EngineEvent::StatusApplied {
            entity: e,
            status: st,
            duration: Some(dur),
            source: None,
        });
    } else if r.0 && !r.1 {
        ev.push(EngineEvent::StatusRemoved {
            entity: e,
            status: StatusEffect::Sick,
        });
    }
    ev
}

pub fn cure_sick(w: &mut GameWorld, e: Entity, cure_type: u8) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    let cured = {
        let mut s = match w.get_component_mut::<StatusEffects>(e) {
            Some(s) => s,
            None => return ev,
        };
        if s.sick == 0 {
            return ev;
        }
        s.sick_type &= !cure_type;
        if s.sick_type != 0 {
            let v = s.sick.saturating_mul(2);
            s.sick = StatusEffects::clamp_timeout(v);
            false
        } else {
            s.sick = 0;
            true
        }
    };
    if cured {
        ev.push(EngineEvent::msg("status-sick-cured"));
        ev.push(EngineEvent::StatusRemoved {
            entity: e,
            status: StatusEffect::Sick,
        });
    }
    ev
}

pub fn make_vomiting(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.vomiting, |s, v| s.vomiting = v, dur) {
        if !was && is {
            ev.push(EngineEvent::msg("status-vomiting-start"));
        } else if was && !is {
            ev.push(EngineEvent::msg("status-vomiting-end"));
        }
    }
    ev
}

pub fn make_paralyzed(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.paralysis, |s, v| s.paralysis = v, dur) {
        if !was && is {
            ev.push(EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Paralyzed,
                duration: Some(dur),
                source: None,
            });
        } else if was && !is {
            ev.push(EngineEvent::msg("status-paralysis-end"));
            ev.push(EngineEvent::StatusRemoved {
                entity: e,
                status: StatusEffect::Paralyzed,
            });
        }
    }
    ev
}

pub fn make_fumbling(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.fumbling, |s, v| s.fumbling = v, dur) {
        if !was && is {
            ev.push(EngineEvent::msg("status-fumbling-start"));
        } else if was && !is {
            ev.push(EngineEvent::msg("status-fumbling-end"));
        }
    }
    ev
}

pub fn make_sleepy(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.sleepy, |s, v| s.sleepy = v, dur) {
        if !was && is {
            ev.push(EngineEvent::msg("status-sleepy-start"));
        } else if was && !is {
            ev.push(EngineEvent::msg("status-sleepy-end"));
        }
    }
    ev
}

pub fn wound_legs(w: &mut GameWorld, e: Entity, dur: u32) -> Vec<EngineEvent> {
    let mut ev = Vec::new();
    if let Some((was, is)) = apply_field(w, e, |s| s.wounded_legs, |s, v| s.wounded_legs = v, dur) {
        if !was && is {
            ev.push(EngineEvent::msg("status-wounded-legs-start"));
        } else if was && !is {
            ev.push(EngineEvent::msg("status-wounded-legs-healed"));
        }
    }
    ev
}

pub fn heal_legs(w: &mut GameWorld, e: Entity) -> Vec<EngineEvent> {
    wound_legs(w, e, 0)
}

// ---------------------------------------------------------------------------
// Intrinsic grants
// ---------------------------------------------------------------------------

pub fn grant_intrinsic(
    world: &mut GameWorld,
    entity: Entity,
    intrinsic: &crate::hunger::CorpseIntrinsic,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    match intrinsic {
        crate::hunger::CorpseIntrinsic::AcidResistance { duration } => {
            let d = *duration;
            if let Some(mut s) = world.get_component_mut::<StatusEffects>(entity) {
                StatusEffects::incr_timeout(&mut s.acid_resistance, d);
            }
            events.push(EngineEvent::msg("intrinsic-acid-res-temp"));
            return events;
        }
        crate::hunger::CorpseIntrinsic::StoneResistance { duration } => {
            let d = *duration;
            if let Some(mut s) = world.get_component_mut::<StatusEffects>(entity) {
                StatusEffects::incr_timeout(&mut s.stone_resistance, d);
            }
            events.push(EngineEvent::msg("intrinsic-stone-res-temp"));
            return events;
        }
        _ => {}
    }
    let mut intr = match world.get_component_mut::<Intrinsics>(entity) {
        Some(i) => i,
        None => return events,
    };
    match intrinsic {
        crate::hunger::CorpseIntrinsic::FireResistance => {
            if !intr.fire_resistance {
                intr.fire_resistance = true;
                events.push(EngineEvent::StatusApplied {
                    entity,
                    status: StatusEffect::FireResistance,
                    duration: None,
                    source: None,
                });
                events.push(EngineEvent::msg("intrinsic-fire-res"));
            }
        }
        crate::hunger::CorpseIntrinsic::ColdResistance => {
            if !intr.cold_resistance {
                intr.cold_resistance = true;
                events.push(EngineEvent::StatusApplied {
                    entity,
                    status: StatusEffect::ColdResistance,
                    duration: None,
                    source: None,
                });
                events.push(EngineEvent::msg("intrinsic-cold-res"));
            }
        }
        crate::hunger::CorpseIntrinsic::SleepResistance => {
            if !intr.sleep_resistance {
                intr.sleep_resistance = true;
                events.push(EngineEvent::StatusApplied {
                    entity,
                    status: StatusEffect::SleepResistance,
                    duration: None,
                    source: None,
                });
                events.push(EngineEvent::msg("intrinsic-sleep-res"));
            }
        }
        crate::hunger::CorpseIntrinsic::ShockResistance => {
            if !intr.shock_resistance {
                intr.shock_resistance = true;
                events.push(EngineEvent::StatusApplied {
                    entity,
                    status: StatusEffect::ShockResistance,
                    duration: None,
                    source: None,
                });
                events.push(EngineEvent::msg("intrinsic-shock-res"));
            }
        }
        crate::hunger::CorpseIntrinsic::PoisonResistance => {
            if !intr.poison_resistance {
                intr.poison_resistance = true;
                events.push(EngineEvent::StatusApplied {
                    entity,
                    status: StatusEffect::PoisonResistance,
                    duration: None,
                    source: None,
                });
                events.push(EngineEvent::msg("intrinsic-poison-res"));
            }
        }
        crate::hunger::CorpseIntrinsic::DisintegrationResistance => {
            if !intr.disintegration_resistance {
                intr.disintegration_resistance = true;
                events.push(EngineEvent::StatusApplied {
                    entity,
                    status: StatusEffect::DisintegrationResistance,
                    duration: None,
                    source: None,
                });
                events.push(EngineEvent::msg("intrinsic-disint-res"));
            }
        }
        crate::hunger::CorpseIntrinsic::Telepathy => {
            if !intr.telepathy {
                intr.telepathy = true;
                events.push(EngineEvent::StatusApplied {
                    entity,
                    status: StatusEffect::Telepathy,
                    duration: None,
                    source: None,
                });
                events.push(EngineEvent::msg("intrinsic-telepathy"));
            }
        }
        crate::hunger::CorpseIntrinsic::Teleportitis => {
            if !intr.teleportitis {
                intr.teleportitis = true;
                events.push(EngineEvent::msg("intrinsic-teleportitis"));
            }
        }
        crate::hunger::CorpseIntrinsic::TeleportControl => {
            if !intr.teleport_control {
                intr.teleport_control = true;
                events.push(EngineEvent::msg("intrinsic-teleport-control"));
            }
        }
        crate::hunger::CorpseIntrinsic::Strength => {
            intr.giant_strength_gained = intr.giant_strength_gained.saturating_add(1);
            events.push(EngineEvent::msg("intrinsic-strength"));
        }
        crate::hunger::CorpseIntrinsic::AcidResistance { .. }
        | crate::hunger::CorpseIntrinsic::StoneResistance { .. } => unreachable!(),
        crate::hunger::CorpseIntrinsic::Invisibility => {
            // Drop intr before borrowing world again for StatusEffects
            drop(intr);
            if let Some(mut s) = world.get_component_mut::<StatusEffects>(entity) {
                StatusEffects::incr_timeout(&mut s.invisibility, 250);
            }
            events.push(EngineEvent::StatusApplied {
                entity,
                status: StatusEffect::Invisible,
                duration: Some(250),
                source: None,
            });
            events.push(EngineEvent::msg("intrinsic-invisibility"));
            return events;
        }
        crate::hunger::CorpseIntrinsic::SeeInvisible => {
            drop(intr);
            if let Some(mut s) = world.get_component_mut::<StatusEffects>(entity) {
                StatusEffects::incr_timeout(&mut s.see_invisible, 250);
            }
            events.push(EngineEvent::StatusApplied {
                entity,
                status: StatusEffect::SeeInvisible,
                duration: Some(250),
                source: None,
            });
            events.push(EngineEvent::msg("intrinsic-see-invisible"));
            return events;
        }
    }
    events
}

// ---------------------------------------------------------------------------
// Per-turn status timeout processing
// ---------------------------------------------------------------------------

/// Process all status timeouts. Uses clone-modify-writeback.
pub fn tick_status_effects(
    world: &mut GameWorld,
    entity: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mut st: StatusEffects = match world.get_component::<StatusEffects>(entity) {
        Some(s) => (*s).clone(),
        None => return events,
    };

    // =====================================================================
    // Phase 1: dialogue sequences (pre-decrement, matching C nh_timeout).
    // =====================================================================

    // Stoning dialogue + side effects
    if st.stoning > 0 {
        let (msgs, side) = stoned_dialogue(st.stoning);
        events.extend(msgs);
        match side {
            StoningSideEffect::ClearSpeed => {
                st.speed = 0;
            }
            StoningSideEffect::Paralyze3 => {
                st.paralysis = st.paralysis.max(3);
                if st.wounded_legs > 0 {
                    st.wounded_legs = 0;
                    events.push(EngineEvent::msg("status-wounded-legs-healed"));
                }
            }
            StoningSideEffect::ClearVomitingSliming => {
                if st.deaf > 0 && st.deaf < 5 {
                    st.deaf = 5;
                }
                if st.vomiting > 0 {
                    st.vomiting = 0;
                }
                if st.sliming > 0 {
                    st.sliming = 0;
                    events.push(EngineEvent::StatusRemoved {
                        entity,
                        status: StatusEffect::Slimed,
                    });
                }
            }
            StoningSideEffect::None => {}
        }
    }

    // Sliming dialogue + side effects
    if st.sliming > 0 {
        let (msgs, side) = slime_dialogue(st.sliming);
        events.extend(msgs);
        match side {
            SlimingSideEffect::ClearSpeed => {
                st.speed = 0;
            }
            SlimingSideEffect::ExtendDeaf => {
                if st.deaf > 0 && st.deaf < 5 {
                    st.deaf = 5;
                }
            }
            SlimingSideEffect::ClearStoning => {
                if st.stoning > 0 {
                    st.stoning = 0;
                    events.push(EngineEvent::StatusRemoved {
                        entity,
                        status: StatusEffect::Stoning,
                    });
                }
            }
            SlimingSideEffect::None => {}
        }
    }

    // Vomiting dialogue + side effects
    if st.vomiting > 0 {
        let (msgs, side) = vomiting_dialogue(st.vomiting, st.confusion, st.stun, rng);
        events.extend(msgs);
        match side {
            VomitingSideEffect::AddConfusion(new_conf) => {
                let clamped = StatusEffects::clamp_timeout(new_conf);
                st.confusion = clamped;
                if clamped > 0 {
                    events.push(EngineEvent::StatusApplied {
                        entity,
                        status: StatusEffect::Confused,
                        duration: Some(clamped),
                        source: None,
                    });
                }
            }
            VomitingSideEffect::AddStunAndConfusion(new_stun, new_conf) => {
                st.stun = StatusEffects::clamp_timeout(new_stun);
                st.confusion = StatusEffects::clamp_timeout(new_conf);
            }
            VomitingSideEffect::Vomit => {
                // Actual vomit event — turn handler can process hunger/paralysis
            }
            VomitingSideEffect::None => {}
        }
    }

    // Strangling dialogue
    if st.strangled > 0 {
        events.extend(choke_dialogue(st.strangled, rng));
    }

    // Sickness dialogue
    if st.sick > 0 {
        events.extend(sickness_dialogue(st.sick, st.sick_type));
    }

    // Levitation pre-expiry dialogue
    if st.levitation > 0 {
        events.extend(levitation_dialogue(st.levitation));
    }

    // Phasing dialogue
    if st.passes_walls > 0 {
        events.extend(phaze_dialogue(st.passes_walls));
    }

    // Sleep dialogue
    if st.sleepy > 0 {
        events.extend(sleep_dialogue(st.sleepy));
    }

    // =====================================================================
    // Phase 2: decrement all timed properties.
    // =====================================================================

    macro_rules! dec {
        ($field:ident, $msg:expr, $status:expr) => {
            if st.$field > 0 {
                st.$field -= 1;
                if st.$field == 0 {
                    events.push(EngineEvent::msg($msg));
                    events.push(EngineEvent::StatusRemoved {
                        entity,
                        status: $status,
                    });
                }
            }
        };
    }
    macro_rules! dec_silent {
        ($field:ident) => {
            if st.$field > 0 {
                st.$field -= 1;
            }
        };
    }

    dec!(confusion, "status-confusion-end", StatusEffect::Confused);
    dec!(stun, "status-stun-end", StatusEffect::Stunned);
    dec!(blindness, "status-blindness-end", StatusEffect::Blind);
    dec!(
        hallucination,
        "status-hallucination-end",
        StatusEffect::Hallucinating
    );
    dec!(
        clairvoyance,
        "status-clairvoyance-end",
        StatusEffect::Clairvoyant
    );
    dec_silent!(deaf);
    dec_silent!(glib);
    dec_silent!(acid_resistance);
    dec_silent!(stone_resistance);
    dec_silent!(protection);
    dec!(
        levitation,
        "status-levitation-end",
        StatusEffect::Levitating
    );
    if st.invisibility > 0 {
        st.invisibility -= 1;
        if st.invisibility == 0 {
            events.push(EngineEvent::msg("status-invisibility-end"));
            events.push(EngineEvent::StatusRemoved {
                entity,
                status: StatusEffect::Invisible,
            });
        }
    }
    if st.see_invisible > 0 {
        st.see_invisible -= 1;
        if st.see_invisible == 0 {
            events.push(EngineEvent::StatusRemoved {
                entity,
                status: StatusEffect::SeeInvisible,
            });
        }
    }

    let mut reset_speed = false;
    if st.speed > 0 {
        st.speed -= 1;
        if st.speed == 0 {
            events.push(EngineEvent::msg("status-speed-end"));
            events.push(EngineEvent::StatusRemoved {
                entity,
                status: StatusEffect::FastSpeed,
            });
            reset_speed = true;
        }
    }
    dec_silent!(vomiting);
    dec!(paralysis, "status-paralysis-end", StatusEffect::Paralyzed);
    dec_silent!(passes_walls);
    dec_silent!(magical_breathing);

    // Wounded legs: decrement, message on expiry
    if st.wounded_legs > 0 {
        st.wounded_legs -= 1;
        if st.wounded_legs == 0 {
            events.push(EngineEvent::msg("status-wounded-legs-healed"));
        }
    }

    // Fumbling: self-cycling timer — on expiry, hero trips and timer resets
    if st.fumbling > 0 {
        st.fumbling -= 1;
        if st.fumbling == 0 {
            events.push(EngineEvent::msg("status-fumble-trip"));
            st.fumbling = rng.random_range(1..=20u32);
        }
    }

    // Sleepy: self-cycling timer — on expiry, hero falls asleep and timer resets
    if st.sleepy > 0 {
        st.sleepy -= 1;
        if st.sleepy == 0 {
            events.push(EngineEvent::msg("status-fall-asleep"));
            let sleep_time = rng.random_range(1..=20u32);
            st.paralysis = st.paralysis.max(sleep_time);
            st.sleepy = sleep_time + rng.random_range(1..=100u32);
        }
    }

    // =====================================================================
    // Phase 3: deadly countdowns.
    // =====================================================================

    if st.stoning > 0 {
        st.stoning -= 1;
        if st.stoning == 0 {
            events.push(EngineEvent::EntityDied {
                entity,
                killer: None,
                cause: DeathCause::Petrification,
            });
        }
    }
    if st.sliming > 0 {
        st.sliming -= 1;
        if st.sliming == 0 {
            events.push(EngineEvent::EntityDied {
                entity,
                killer: None,
                cause: DeathCause::Sickness,
            });
        }
    }
    if st.sick > 0 {
        st.sick -= 1;
        if st.sick == 0 {
            let can = (st.sick_type & SICK_NONVOMITABLE) == 0;
            let ok = can && rng.random_range(0..100u32) < 18;
            if ok {
                events.push(EngineEvent::msg("status-sick-recovered"));
                st.sick_type = 0;
            } else {
                events.push(EngineEvent::EntityDied {
                    entity,
                    killer: None,
                    cause: DeathCause::Poisoning,
                });
            }
        }
    }
    if st.strangled > 0 {
        st.strangled -= 1;
        if st.strangled == 0 {
            events.push(EngineEvent::EntityDied {
                entity,
                killer: None,
                cause: DeathCause::Strangulation,
            });
        }
    }

    // =====================================================================
    // Write back + post-processing.
    // =====================================================================

    if let Some(mut s) = world.get_component_mut::<StatusEffects>(entity) {
        *s = st;
    }
    if reset_speed
        && let Some(mut hsb) = world.get_component_mut::<HeroSpeedBonus>(entity)
        && hsb.0 == HeroSpeed::VeryFast
    {
        hsb.0 = HeroSpeed::Normal;
    }
    events
}

// ---------------------------------------------------------------------------
// Luck decay (per-turn, from nh_timeout Phase 1)
// ---------------------------------------------------------------------------

/// Decay luck toward base_luck on the appropriate interval.
/// `has_luckstone`: whether carrying any luckstone.
/// `luckstone_cursed`: true if the carried luckstone is cursed.
/// `luckstone_blessed`: true if the carried luckstone is blessed.
/// `has_amulet_or_angry_god`: shortens interval from 600 to 300.
#[derive(Debug, Clone, Copy)]
pub struct LuckTickContext {
    pub turn: u64,
    pub base_luck: i32,
    pub has_luckstone: bool,
    pub luckstone_cursed: bool,
    pub luckstone_blessed: bool,
    pub has_amulet_or_angry_god: bool,
}

pub fn tick_luck(
    world: &mut GameWorld,
    entity: Entity,
    context: LuckTickContext,
) -> Vec<EngineEvent> {
    let interval: u64 = if context.has_amulet_or_angry_god {
        300
    } else {
        600
    };
    if !context.turn.is_multiple_of(interval) {
        return vec![];
    }

    let mut pc = match world.get_component_mut::<crate::world::PlayerCombat>(entity) {
        Some(pc) => pc,
        None => return vec![],
    };

    if pc.luck == context.base_luck {
        return vec![];
    }

    // Determine if luck should decay.
    // No luckstone: always decay.
    // Uncursed luckstone: no decay.
    // Blessed luckstone: negative luck recovers, positive doesn't decay.
    // Cursed luckstone: positive luck decays, negative doesn't recover.
    let no_stone = !context.has_luckstone;
    let time_luck_effect = if context.luckstone_blessed {
        1i32
    } else if context.luckstone_cursed {
        -1i32
    } else {
        0i32
    };

    if pc.luck > context.base_luck && (no_stone || time_luck_effect < 0) {
        pc.luck -= 1;
    } else if pc.luck < context.base_luck && (no_stone || time_luck_effect > 0) {
        pc.luck += 1;
    }

    vec![]
}

// ---------------------------------------------------------------------------
// Spell protection dissipation
// ---------------------------------------------------------------------------

/// Tick spell protection: countdown decrements, layers peel off.
pub fn tick_spell_protection(world: &mut GameWorld, entity: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mut sp = match world.get_component_mut::<SpellProtection>(entity) {
        Some(sp) => sp,
        None => return events,
    };
    if sp.countdown == 0 || sp.layers == 0 {
        return events;
    }
    sp.countdown -= 1;
    if sp.countdown == 0 && sp.layers > 0 {
        sp.layers -= 1;
        if sp.layers > 0 {
            sp.countdown = sp.interval;
            events.push(EngineEvent::msg("spell-protection-less-dense"));
        } else {
            events.push(EngineEvent::msg("spell-protection-disappears"));
        }
    }
    events
}

// ---------------------------------------------------------------------------
// Hero misc counter ticks
// ---------------------------------------------------------------------------

/// Tick misc hero counters (cream, gallop).
pub fn tick_hero_counters(world: &mut GameWorld, entity: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mut hc = match world.get_component_mut::<HeroCounters>(entity) {
        Some(hc) => hc,
        None => return events,
    };
    if hc.creamed > 0 {
        hc.creamed -= 1;
    }
    if hc.gallop > 0 {
        hc.gallop -= 1;
        if hc.gallop == 0 {
            events.push(EngineEvent::msg("steed-stops-galloping"));
        }
    }
    events
}

// ---------------------------------------------------------------------------
// burn_away_slime helper
// ---------------------------------------------------------------------------

/// Cure sliming via fire damage (e.g., fire trap, fire spell, etc.).
pub fn burn_away_slime(w: &mut GameWorld, e: Entity) -> Vec<EngineEvent> {
    if is_sliming(w, e) {
        let mut ev = make_slimed(w, e, 0);
        ev.insert(0, EngineEvent::msg("slime-burned-away"));
        ev
    } else {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// Dialogue sequences
// ---------------------------------------------------------------------------

/// Stoning 5-stage dialogue with side effects.
/// Called *before* decrement, so r is the pre-decrement value.
/// Returns (messages, side_effect_flags).
fn stoned_dialogue(r: u32) -> (Vec<EngineEvent>, StoningSideEffect) {
    let msg = match r {
        5 => vec![EngineEvent::msg("stoning-slowing-down")],
        4 => vec![EngineEvent::msg("stoning-limbs-stiffening")],
        3 => vec![EngineEvent::msg("stoning-limbs-stone")],
        2 => vec![EngineEvent::msg("stoning-turned-to-stone")],
        1 => vec![EngineEvent::msg("stoning-you-are-statue")],
        _ => vec![],
    };
    let effect = match r {
        5 => StoningSideEffect::ClearSpeed,
        3 => StoningSideEffect::Paralyze3,
        2 => StoningSideEffect::ClearVomitingSliming,
        _ => StoningSideEffect::None,
    };
    (msg, effect)
}

#[derive(Debug, PartialEq)]
enum StoningSideEffect {
    None,
    /// Stage 5: clear intrinsic speed.
    ClearSpeed,
    /// Stage 3: paralysis for 3 turns, heal wounded legs.
    Paralyze3,
    /// Stage 2: clear vomiting and sliming, extend deafness.
    ClearVomitingSliming,
}

/// Sliming 10-stage dialogue with side effects.
/// Messages only on odd t values. Side effects on i = t/2.
fn slime_dialogue(r: u32) -> (Vec<EngineEvent>, SlimingSideEffect) {
    let i = r / 2;
    let msg = if !r.is_multiple_of(2) {
        match i {
            4 => vec![EngineEvent::msg("sliming-turning-green")],
            3 => vec![EngineEvent::msg("sliming-limbs-oozy")],
            2 => vec![EngineEvent::msg("sliming-skin-peeling")],
            1 => vec![EngineEvent::msg("sliming-turning-into")],
            0 => vec![EngineEvent::msg("sliming-become-slime")],
            _ => vec![],
        }
    } else {
        vec![]
    };
    let effect = match i {
        3 => SlimingSideEffect::ClearSpeed,
        2 => SlimingSideEffect::ExtendDeaf,
        1 => SlimingSideEffect::ClearStoning,
        _ => SlimingSideEffect::None,
    };
    (msg, effect)
}

#[derive(Debug, PartialEq)]
enum SlimingSideEffect {
    None,
    /// i=3: clear intrinsic speed.
    ClearSpeed,
    /// i=2: extend deafness to at least 5.
    ExtendDeaf,
    /// i=1: clear stoning (sliming overrides stoning).
    ClearStoning,
}

/// Vomiting dialogue and side effects.
/// Side effects from NetHack timeout.c: at v-1==6 stun+confuse, v-1==9 confuse,
/// v-1==0 actual vomit.
fn vomiting_dialogue(
    r: u32,
    confusion: u32,
    stun: u32,
    rng: &mut impl Rng,
) -> (Vec<EngineEvent>, VomitingSideEffect) {
    let v = r.saturating_sub(1);
    let mut msgs = Vec::new();
    let mut effect = VomitingSideEffect::None;
    match v {
        14 => {
            msgs.push(EngineEvent::msg("vomiting-mildly-nauseated"));
        }
        11 => {
            msgs.push(EngineEvent::msg("vomiting-slightly-confused"));
        }
        9 => {
            // Add confusion: existing + 2d4
            let add = roll_2d4(rng);
            effect = VomitingSideEffect::AddConfusion(confusion.saturating_add(add));
        }
        8 => {
            msgs.push(EngineEvent::msg("vomiting-cant-think"));
        }
        6 => {
            // Add stun AND confusion (FALLTHROUGH from case 6 to case 9 in C)
            let stun_add = roll_2d4(rng);
            let conf_add = roll_2d4(rng);
            effect = VomitingSideEffect::AddStunAndConfusion(
                stun.saturating_add(stun_add),
                confusion.saturating_add(conf_add),
            );
        }
        5 => {
            msgs.push(EngineEvent::msg("vomiting-incredibly-sick"));
        }
        2 => {
            msgs.push(EngineEvent::msg("vomiting-about-to"));
        }
        0 => {
            msgs.push(EngineEvent::msg("vomiting-vomit"));
            effect = VomitingSideEffect::Vomit;
        }
        _ => {}
    }
    (msgs, effect)
}

fn roll_2d4(rng: &mut impl Rng) -> u32 {
    rng.random_range(1..=4u32) + rng.random_range(1..=4u32)
}

#[derive(Debug, PartialEq)]
enum VomitingSideEffect {
    None,
    /// v-1==9: set confusion to this value.
    AddConfusion(u32),
    /// v-1==6: set stun to first, confusion to second (FALLTHROUGH).
    AddStunAndConfusion(u32, u32),
    /// v-1==0: actual vomit (hunger increase, nomul).
    Vomit,
}

/// Strangling (choke) dialogue — 5 stages.
/// Two text sets: standard breathing texts vs constriction texts (2% chance or breathless).
fn choke_dialogue(r: u32, rng: &mut impl Rng) -> Vec<EngineEvent> {
    if r == 0 || r > 5 {
        return vec![];
    }
    // 2% chance of alternate text set
    let alt = rng.random_range(0..50u32) == 0;
    if alt {
        match r {
            5 => vec![EngineEvent::msg("choke-neck-constricted")],
            4 => vec![EngineEvent::msg("choke-blood-trouble")],
            3 => vec![EngineEvent::msg("choke-neck-pressure")],
            2 => vec![EngineEvent::msg("choke-consciousness-fading")],
            1 => vec![EngineEvent::msg("choke-suffocate")],
            _ => vec![],
        }
    } else {
        match r {
            5 => vec![EngineEvent::msg("choke-hard-to-breathe")],
            4 => vec![EngineEvent::msg("choke-gasping-for-air")],
            3 => vec![EngineEvent::msg("choke-no-longer-breathe")],
            2 => vec![EngineEvent::msg("choke-turning-blue")],
            1 => vec![EngineEvent::msg("choke-suffocate")],
            _ => vec![],
        }
    }
}

/// Levitation pre-expiry dialogue.
/// Messages on odd timeout values, using i = (timeout - 1) / 2.
fn levitation_dialogue(r: u32) -> Vec<EngineEvent> {
    if r == 0 {
        return vec![];
    }
    let i = (r.saturating_sub(1)) / 2;
    if r.is_multiple_of(2) {
        return vec![];
    }
    match i {
        2 => vec![EngineEvent::msg("levitation-float-lower")],
        1 => vec![EngineEvent::msg("levitation-wobble")],
        _ => vec![],
    }
}

/// Sleep dialogue — yawn at 4 turns remaining.
fn sleep_dialogue(r: u32) -> Vec<EngineEvent> {
    if r == 4 {
        vec![EngineEvent::msg("sleepy-yawn")]
    } else {
        vec![]
    }
}

/// Phasing (Passes_walls) pre-expiry dialogue.
fn phaze_dialogue(r: u32) -> Vec<EngineEvent> {
    if r == 0 {
        return vec![];
    }
    let i = r / 2;
    if r.is_multiple_of(2) {
        return vec![];
    }
    match i {
        2 => vec![EngineEvent::msg("phaze-feeling-bloated")],
        1 => vec![EngineEvent::msg("phaze-feeling-flabby")],
        _ => vec![],
    }
}

fn sickness_dialogue(r: u32, sick_type: u8) -> Vec<EngineEvent> {
    if r.is_multiple_of(2) {
        return vec![];
    }
    match r / 2 {
        3 => vec![EngineEvent::msg(if (sick_type & SICK_VOMITABLE) != 0 {
            "sick-sickness-worse"
        } else {
            "sick-illness-worse"
        })],
        2 => vec![EngineEvent::msg("sick-illness-severe")],
        1 => vec![EngineEvent::msg("sick-deaths-door")],
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Timer expiration system
// ---------------------------------------------------------------------------

/// Describes what happens when a timed effect expires or reaches a warning
/// threshold. This is a higher-level abstraction over the dialogue/side-effect
/// system used by `tick_status_effects`. It can be used to query expiration
/// semantics without running the full tick loop.
#[derive(Debug, Clone, PartialEq)]
pub enum ExpirationEffect {
    /// Simple message to display.
    Message(String),
    /// Remove an intrinsic property.
    RemoveIntrinsic(String),
    /// Restore a stat to its natural value.
    RestoreStat { stat: String, natural: i32 },
    /// Transform back from polymorph.
    RevertPolymorph,
    /// Stoning completes — player dies.
    StoningComplete,
    /// Sliming completes — player dies.
    SlimingComplete,
    /// Sickness kills.
    SicknessKills,
    /// Strangulation kills.
    StrangulationKills,
    /// Levitation ends — player falls.
    FallFromLevitation { height: i32 },
    /// Invisibility ends — becomes visible.
    BecomeVisible,
    /// Speed returns to normal.
    SpeedNormal,
    /// Blindness ends — can see again.
    SightRestored,
    /// Confusion clears.
    ConfusionClears,
    /// Stun clears.
    StunClears,
    /// Hallucination ends.
    HallucinationEnds,
    /// Egg hatches into a monster.
    EggHatches { monster_type: String },
    /// Nothing special happens.
    NoEffect,
}

/// Get the expiration effect for a timed property at a given duration.
///
/// When `duration_left == 0`, returns the terminal effect (death, transformation,
/// or expiry message). When `duration_left > 0`, returns progressive warnings
/// for deadly countdowns (stoning, sliming, sickness, strangulation).
pub fn expiration_effect(property: &str, duration_left: i32) -> ExpirationEffect {
    match property {
        "fast" | "very_fast" | "speed" => {
            if duration_left <= 0 {
                ExpirationEffect::SpeedNormal
            } else {
                ExpirationEffect::NoEffect
            }
        }
        "invisibility" => {
            if duration_left <= 0 {
                ExpirationEffect::BecomeVisible
            } else {
                ExpirationEffect::NoEffect
            }
        }
        "see_invisible" => {
            if duration_left <= 0 {
                ExpirationEffect::Message("You thought you saw something!".into())
            } else {
                ExpirationEffect::NoEffect
            }
        }
        "levitation" => {
            if duration_left <= 0 {
                ExpirationEffect::FallFromLevitation { height: 0 }
            } else {
                ExpirationEffect::NoEffect
            }
        }
        "stoning" => {
            if duration_left <= 0 {
                ExpirationEffect::StoningComplete
            } else {
                // Progressive warnings (matching C stoned_texts[])
                match duration_left {
                    5 => ExpirationEffect::Message("You are slowing down.".into()),
                    4 => ExpirationEffect::Message("Your limbs are stiffening.".into()),
                    3 => ExpirationEffect::Message("Your limbs have turned to stone.".into()),
                    2 => ExpirationEffect::Message("You have turned to stone.".into()),
                    1 => ExpirationEffect::Message("You are a statue.".into()),
                    _ => ExpirationEffect::NoEffect,
                }
            }
        }
        "sliming" => {
            if duration_left <= 0 {
                ExpirationEffect::SlimingComplete
            } else {
                // Messages on odd values only, using i = t/2 (matching C slime_texts[])
                if duration_left % 2 != 0 {
                    match duration_left / 2 {
                        4 => ExpirationEffect::Message("You are turning a little green.".into()),
                        3 => ExpirationEffect::Message("Your limbs are getting oozy.".into()),
                        2 => ExpirationEffect::Message("Your skin begins to peel away.".into()),
                        1 => {
                            ExpirationEffect::Message("You are turning into a green slime.".into())
                        }
                        0 => ExpirationEffect::Message("You have become a green slime.".into()),
                        _ => ExpirationEffect::NoEffect,
                    }
                } else {
                    ExpirationEffect::NoEffect
                }
            }
        }
        "sick" => {
            if duration_left <= 0 {
                ExpirationEffect::SicknessKills
            } else {
                // Messages on odd values only, using i = t/2 (matching C sickness_texts[])
                if duration_left % 2 != 0 {
                    match duration_left / 2 {
                        3 => ExpirationEffect::Message("Your illness feels worse.".into()),
                        2 => ExpirationEffect::Message("Your illness is severe.".into()),
                        1 => ExpirationEffect::Message("You are at Death's door.".into()),
                        _ => ExpirationEffect::NoEffect,
                    }
                } else {
                    ExpirationEffect::NoEffect
                }
            }
        }
        "strangled" => {
            if duration_left <= 0 {
                ExpirationEffect::StrangulationKills
            } else {
                // Choking dialogue at each stage 1-5 (matching C choke_texts[])
                match duration_left {
                    5 => ExpirationEffect::Message("You find it hard to breathe.".into()),
                    4 => ExpirationEffect::Message("You're gasping for air.".into()),
                    3 => ExpirationEffect::Message("You can no longer breathe.".into()),
                    2 => ExpirationEffect::Message("You're turning blue.".into()),
                    1 => ExpirationEffect::Message("You suffocate.".into()),
                    _ => ExpirationEffect::NoEffect,
                }
            }
        }
        "blind" | "blindness" => {
            if duration_left <= 0 {
                ExpirationEffect::SightRestored
            } else {
                ExpirationEffect::NoEffect
            }
        }
        "confused" | "confusion" => {
            if duration_left <= 0 {
                ExpirationEffect::ConfusionClears
            } else {
                ExpirationEffect::NoEffect
            }
        }
        "stunned" | "stun" => {
            if duration_left <= 0 {
                ExpirationEffect::StunClears
            } else {
                ExpirationEffect::NoEffect
            }
        }
        "hallucinating" | "hallucination" => {
            if duration_left <= 0 {
                ExpirationEffect::HallucinationEnds
            } else {
                ExpirationEffect::NoEffect
            }
        }
        "polymorph" => {
            if duration_left <= 0 {
                ExpirationEffect::RevertPolymorph
            } else {
                ExpirationEffect::NoEffect
            }
        }
        "protection" => {
            if duration_left <= 0 {
                ExpirationEffect::Message("Your protected feeling fades.".into())
            } else {
                ExpirationEffect::NoEffect
            }
        }
        "wounded_legs" => {
            if duration_left <= 0 {
                ExpirationEffect::Message("Your legs feel somewhat better.".into())
            } else {
                ExpirationEffect::NoEffect
            }
        }
        _ => ExpirationEffect::NoEffect,
    }
}

/// Tick a list of named timed properties, decrementing each by 1 and collecting
/// any expiration effects (warnings or terminal events).
///
/// Expired properties (reaching 0) are removed from the list.
/// Returns a list of `(property_name, effect)` pairs for any non-`NoEffect` results.
pub fn tick_timed_properties(
    properties: &mut Vec<(String, i32)>,
) -> Vec<(String, ExpirationEffect)> {
    let mut effects = Vec::new();

    for (name, turns) in properties.iter_mut() {
        *turns -= 1;

        let effect = expiration_effect(name, *turns);
        if effect != ExpirationEffect::NoEffect {
            effects.push((name.clone(), effect));
        }
    }

    // Remove expired properties
    properties.retain(|(_, turns)| *turns > 0);

    effects
}

/// Check if a timed property is one of the deadly countdowns that
/// kills or transforms the player when it reaches 0.
pub fn is_fatal_timeout(property: &str) -> bool {
    matches!(property, "stoning" | "sliming" | "sick" | "strangled")
}

/// Check if an egg should hatch based on its age vs hatch time.
///
/// `egg_age`: how many turns the egg has existed.
/// `hatch_time`: the species-specific incubation period (typically 50-200 turns).
/// `monster_type`: what hatches from the egg.
pub fn check_egg_hatch(
    egg_age: i32,
    hatch_time: i32,
    monster_type: &str,
) -> Option<ExpirationEffect> {
    if egg_age >= hatch_time {
        Some(ExpirationEffect::EggHatches {
            monster_type: monster_type.to_string(),
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Hallucination name scrambling
// ---------------------------------------------------------------------------

/// Bogus monster names used when the player is hallucinating.
/// Drawn from NetHack's bogusmons list.
const BOGUS_MONSTERS: &[&str] = &[
    "hallucinatory monster",
    "acid blob",
    "rubber ducky",
    "one-eyed one-horned flying purple people eater",
    "Langstrumpf",
    "Invisible Pink Unicorn",
    "bonsai kitten",
    "Heffalump",
    "Woozle",
    "jabberwock",
    "snark",
    "boojum",
    "amphisbaena",
    "basilisk",
    "catoblepas",
    "chimera",
    "dragon",
    "gargoyle",
    "harpy",
    "hippocampus",
];

/// Bogus item names used when the player is hallucinating.
const BOGUS_ITEMS: &[&str] = &[
    "glorkum",
    "frobozz",
    "xyzzy",
    "plugh",
    "zorkmid",
    "gizmo",
    "widget",
    "thingamajig",
    "doodad",
    "whatchamacallit",
    "doohickey",
    "contraption",
    "squishyblob",
    "flickerfrost",
    "moonbeam shard",
    "dream bottle",
    "quantum pickle",
    "inverted wombat",
    "recursive teapot",
    "chaos muffin",
];

/// Return a random bogus monster name for hallucination display.
///
/// When the player is hallucinating, monster names should be scrambled
/// to random bogus names. This function picks a random name from the
/// `BOGUS_MONSTERS` list. The `_real_name` parameter is reserved for
/// future use (e.g., to ensure the bogus name differs from the real one).
pub fn hallucinate_monster_name(_real_name: &str, rng: &mut impl Rng) -> String {
    BOGUS_MONSTERS[rng.random_range(0..BOGUS_MONSTERS.len())].to_string()
}

/// Return a random bogus item name for hallucination display.
///
/// When the player is hallucinating, item names should be scrambled
/// to random bogus names. This function picks a random name from the
/// `BOGUS_ITEMS` list.
pub fn hallucinate_item_name(_real_name: &str, rng: &mut impl Rng) -> String {
    BOGUS_ITEMS[rng.random_range(0..BOGUS_ITEMS.len())].to_string()
}

// ---------------------------------------------------------------------------
// Drowning check
// ---------------------------------------------------------------------------

/// Check if player drowns when entering water.
/// Returns true if player should die from drowning.
///
/// Any of the listed protections prevents drowning.
pub fn check_drowning(
    has_water_walking: bool,
    has_magical_breathing: bool,
    is_levitating: bool,
    is_flying: bool,
    can_swim: bool,
) -> bool {
    if has_water_walking || has_magical_breathing || is_levitating || is_flying || can_swim {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Levitation fall
// ---------------------------------------------------------------------------

/// Result of levitation ending (player falls).
#[derive(Debug, Clone, PartialEq)]
pub enum LevitationEndResult {
    /// Fatal fall into the void (air/clouds in endgame).
    Fatal { cause: String },
    /// Fall into lava — fatal without fire resistance.
    FallIntoLava,
    /// Fall into water — drowning check needed.
    FallIntoWater,
    /// Normal landing — takes d6 damage.
    Landing { damage: i32 },
    /// Safe landing (feather falling, flying, etc.).
    SafeLanding,
}

/// Check what happens when levitation ends over different terrain.
///
/// `is_flying` or `has_feather_fall` provide safe landing.
/// Otherwise terrain determines the outcome.
pub fn check_levitation_fall(
    is_over_void: bool,
    is_over_water: bool,
    is_over_lava: bool,
    is_flying: bool,
    has_feather_fall: bool,
    rng: &mut impl Rng,
) -> LevitationEndResult {
    if is_flying || has_feather_fall {
        return LevitationEndResult::SafeLanding;
    }
    if is_over_void {
        LevitationEndResult::Fatal {
            cause: "fell to your death".to_string(),
        }
    } else if is_over_lava {
        LevitationEndResult::FallIntoLava
    } else if is_over_water {
        LevitationEndResult::FallIntoWater
    } else {
        let damage = rng.random_range(1..=6i32);
        LevitationEndResult::Landing { damage }
    }
}

// ---------------------------------------------------------------------------
// Direction randomization
// ---------------------------------------------------------------------------

pub fn maybe_confuse_direction(
    confused: bool,
    stunned: bool,
    rng: &mut impl Rng,
) -> Option<crate::action::Direction> {
    if stunned || (confused && rng.random_range(0..5u32) == 0) {
        let dirs = [
            crate::action::Direction::North,
            crate::action::Direction::NorthEast,
            crate::action::Direction::East,
            crate::action::Direction::SouthEast,
            crate::action::Direction::South,
            crate::action::Direction::SouthWest,
            crate::action::Direction::West,
            crate::action::Direction::NorthWest,
        ];
        Some(dirs[rng.random_range(0..8usize)])
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::world::GameWorld;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }
    fn make_test_world() -> GameWorld {
        GameWorld::new(Position::new(5, 5))
    }

    // I.1: Status effects

    #[test]
    fn test_status_confusion_apply() {
        let mut w = make_test_world();
        let p = w.player();
        assert!(!is_confused(&w, p));
        let ev = make_confused(&mut w, p, 10);
        assert!(is_confused(&w, p));
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Confused,
                ..
            }
        )));
    }

    #[test]
    fn test_status_confusion_decay() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_confused(&mut w, p, 3);
        for _ in 0..2 {
            tick_status_effects(&mut w, p, &mut r);
        }
        assert!(is_confused(&w, p));
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(!is_confused(&w, p));
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Confused,
                ..
            }
        )));
    }

    #[test]
    fn test_status_confusion_replace() {
        let mut w = make_test_world();
        let p = w.player();
        make_confused(&mut w, p, 10);
        assert_eq!(w.get_component::<StatusEffects>(p).unwrap().confusion, 10);
        make_confused(&mut w, p, 5);
        assert_eq!(w.get_component::<StatusEffects>(p).unwrap().confusion, 5);
        make_confused(&mut w, p, 20);
        assert_eq!(w.get_component::<StatusEffects>(p).unwrap().confusion, 20);
    }

    #[test]
    fn test_status_confusion_cure() {
        let mut w = make_test_world();
        let p = w.player();
        make_confused(&mut w, p, 10);
        let ev = make_confused(&mut w, p, 0);
        assert!(!is_confused(&w, p));
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Confused,
                ..
            }
        )));
    }

    #[test]
    fn test_status_blindness_apply_decay() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_blinded(&mut w, p, 2);
        assert!(is_blind(&w, p));
        tick_status_effects(&mut w, p, &mut r);
        assert!(is_blind(&w, p));
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(!is_blind(&w, p));
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Blind,
                ..
            }
        )));
    }

    #[test]
    fn test_status_blindness_cure() {
        let mut w = make_test_world();
        let p = w.player();
        make_blinded(&mut w, p, 100);
        make_blinded(&mut w, p, 0);
        assert!(!is_blind(&w, p));
    }

    #[test]
    fn test_status_stun_apply_decay() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_stunned(&mut w, p, 2);
        tick_status_effects(&mut w, p, &mut r);
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(!is_stunned(&w, p));
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Stunned,
                ..
            }
        )));
    }

    #[test]
    fn test_status_hallucination_apply_decay() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_hallucinated(&mut w, p, 3);
        tick_status_effects(&mut w, p, &mut r);
        tick_status_effects(&mut w, p, &mut r);
        assert!(is_hallucinating(&w, p));
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(!is_hallucinating(&w, p));
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Hallucinating,
                ..
            }
        )));
    }

    #[test]
    fn test_status_levitation_apply_decay() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_levitating(&mut w, p, 2);
        tick_status_effects(&mut w, p, &mut r);
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(!is_levitating(&w, p));
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Levitating,
                ..
            }
        )));
    }

    #[test]
    fn test_status_levitation_queryable() {
        let mut w = make_test_world();
        let p = w.player();
        assert!(!is_levitating(&w, p));
        make_levitating(&mut w, p, 50);
        assert!(is_levitating(&w, p));
    }

    #[test]
    fn test_status_stoning_death() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_stoned(&mut w, p, STONING_INITIAL);
        for _ in 0..4 {
            let ev = tick_status_effects(&mut w, p, &mut r);
            assert!(
                !ev.iter()
                    .any(|e| matches!(e, EngineEvent::EntityDied { .. }))
            );
        }
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::EntityDied {
                cause: DeathCause::Petrification,
                ..
            }
        )));
    }

    #[test]
    fn test_status_stoning_cure() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_stoned(&mut w, p, STONING_INITIAL);
        tick_status_effects(&mut w, p, &mut r);
        tick_status_effects(&mut w, p, &mut r);
        make_stoned(&mut w, p, 0);
        assert!(!is_stoning(&w, p));
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(
            !ev.iter()
                .any(|e| matches!(e, EngineEvent::EntityDied { .. }))
        );
    }

    #[test]
    fn test_status_stoning_dialogue() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_stoned(&mut w, p, STONING_INITIAL);
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(ev.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "stoning-slowing-down")
        ));
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(ev.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "stoning-limbs-stiffening")
        ));
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(ev.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "stoning-limbs-stone")
        ));
    }

    #[test]
    fn test_status_sliming_death() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_slimed(&mut w, p, SLIMING_INITIAL);
        for _ in 0..9 {
            let ev = tick_status_effects(&mut w, p, &mut r);
            assert!(
                !ev.iter()
                    .any(|e| matches!(e, EngineEvent::EntityDied { .. }))
            );
        }
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(
            ev.iter()
                .any(|e| matches!(e, EngineEvent::EntityDied { .. }))
        );
    }

    #[test]
    fn test_status_sliming_cure() {
        let mut w = make_test_world();
        let p = w.player();
        make_slimed(&mut w, p, SLIMING_INITIAL);
        make_slimed(&mut w, p, 0);
        assert!(!is_sliming(&w, p));
    }

    #[test]
    fn test_status_sliming_dialogue_odd() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_slimed(&mut w, p, SLIMING_INITIAL);
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(
            !ev.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key.starts_with("sliming-"))
            )
        );
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(ev.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "sliming-turning-green")
        ));
    }

    #[test]
    fn test_status_food_poisoning() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = SmallRng::seed_from_u64(999);
        make_sick(&mut w, p, 3, SICK_VOMITABLE);
        assert!(is_food_poisoned(&w, p));
        let mut ok = false;
        for _ in 0..5 {
            let ev = tick_status_effects(&mut w, p, &mut r);
            if ev.iter().any(|e| matches!(e, EngineEvent::EntityDied { .. }))
                || ev.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "status-sick-recovered"))
            { ok = true; break; }
        }
        assert!(ok, "food poisoning should resolve");
    }

    #[test]
    fn test_status_disease_fatal() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_sick(&mut w, p, 2, SICK_NONVOMITABLE);
        tick_status_effects(&mut w, p, &mut r);
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::EntityDied {
                cause: DeathCause::Poisoning,
                ..
            }
        )));
    }

    #[test]
    fn test_status_sick_dual_type() {
        let mut w = make_test_world();
        let p = w.player();
        make_sick(&mut w, p, 10, SICK_VOMITABLE);
        make_sick(&mut w, p, 10, SICK_NONVOMITABLE);
        assert_eq!(
            w.get_component::<StatusEffects>(p).unwrap().sick_type,
            SICK_ALL
        );
        cure_sick(&mut w, p, SICK_VOMITABLE);
        let s = w.get_component::<StatusEffects>(p).unwrap();
        assert_eq!(s.sick_type, SICK_NONVOMITABLE);
        assert!(s.sick > 0);
        assert_eq!(s.sick, 20);
    }

    #[test]
    fn test_status_sick_full_cure() {
        let mut w = make_test_world();
        let p = w.player();
        make_sick(&mut w, p, 10, SICK_VOMITABLE);
        let ev = cure_sick(&mut w, p, SICK_VOMITABLE);
        assert!(!is_sick(&w, p));
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Sick,
                ..
            }
        )));
    }

    #[test]
    fn test_status_vomiting_decay() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_vomiting(&mut w, p, 3);
        tick_status_effects(&mut w, p, &mut r);
        tick_status_effects(&mut w, p, &mut r);
        assert_eq!(w.get_component::<StatusEffects>(p).unwrap().vomiting, 1);
        tick_status_effects(&mut w, p, &mut r);
        assert!(!is_vomiting(&w, p));
    }

    #[test]
    fn test_status_timeout_clamp() {
        let mut w = make_test_world();
        let p = w.player();
        make_confused(&mut w, p, TIMEOUT_MAX + 1000);
        assert_eq!(
            w.get_component::<StatusEffects>(p).unwrap().confusion,
            TIMEOUT_MAX
        );
    }

    #[test]
    fn test_status_incr_clamp() {
        let mut v: u32 = TIMEOUT_MAX - 5;
        StatusEffects::incr_timeout(&mut v, 100);
        assert_eq!(v, TIMEOUT_MAX);
    }

    #[test]
    fn test_status_multiple_simultaneous() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_confused(&mut w, p, 5);
        make_stunned(&mut w, p, 3);
        make_blinded(&mut w, p, 7);
        for _ in 0..3 {
            tick_status_effects(&mut w, p, &mut r);
        }
        assert!(is_confused(&w, p));
        assert!(!is_stunned(&w, p));
        assert!(is_blind(&w, p));
        for _ in 0..2 {
            tick_status_effects(&mut w, p, &mut r);
        }
        assert!(!is_confused(&w, p));
        assert!(is_blind(&w, p));
        for _ in 0..2 {
            tick_status_effects(&mut w, p, &mut r);
        }
        assert!(!is_blind(&w, p));
    }

    #[test]
    fn test_status_confdir_randomization() {
        let mut r = test_rng();
        for _ in 0..100 {
            assert!(maybe_confuse_direction(false, false, &mut r).is_none());
        }
        for _ in 0..10 {
            assert!(maybe_confuse_direction(false, true, &mut r).is_some());
        }
        let mut n = 0;
        let t = 5000;
        for _ in 0..t {
            if maybe_confuse_direction(true, false, &mut r).is_some() {
                n += 1;
            }
        }
        let ratio = n as f64 / t as f64;
        assert!((0.12..=0.28).contains(&ratio), "ratio {:.3}", ratio);
    }

    // I.2: Intrinsics

    #[test]
    fn test_intrinsic_fire_res() {
        let mut w = make_test_world();
        let p = w.player();
        let ev = grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::FireResistance);
        assert!(has_intrinsic_fire_res(&w, p));
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::FireResistance,
                ..
            }
        )));
    }

    #[test]
    fn test_intrinsic_fire_res_no_dup() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::FireResistance);
        let ev = grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::FireResistance);
        assert!(ev.is_empty());
    }

    #[test]
    fn test_intrinsic_cold_res() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::ColdResistance);
        assert!(has_intrinsic_cold_res(&w, p));
    }

    #[test]
    fn test_intrinsic_sleep_res() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::SleepResistance);
        assert!(has_intrinsic_sleep_res(&w, p));
    }

    #[test]
    fn test_intrinsic_shock_res() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::ShockResistance);
        assert!(has_intrinsic_shock_res(&w, p));
    }

    #[test]
    fn test_intrinsic_poison_res() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::PoisonResistance);
        assert!(has_intrinsic_poison_res(&w, p));
    }

    #[test]
    fn test_intrinsic_disint_res() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(
            &mut w,
            p,
            &crate::hunger::CorpseIntrinsic::DisintegrationResistance,
        );
        assert!(has_intrinsic_disint_res(&w, p));
    }

    #[test]
    fn test_intrinsic_telepathy() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::Telepathy);
        assert!(has_intrinsic_telepathy(&w, p));
    }

    #[test]
    fn test_intrinsic_teleportitis() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::Teleportitis);
        assert!(has_intrinsic_teleportitis(&w, p));
    }

    #[test]
    fn test_intrinsic_teleport_control() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::TeleportControl);
        assert!(has_intrinsic_teleport_control(&w, p));
    }

    #[test]
    fn test_intrinsic_strength_giant() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::Strength);
        assert_eq!(
            w.get_component::<Intrinsics>(p)
                .unwrap()
                .giant_strength_gained,
            1
        );
        grant_intrinsic(&mut w, p, &crate::hunger::CorpseIntrinsic::Strength);
        assert_eq!(
            w.get_component::<Intrinsics>(p)
                .unwrap()
                .giant_strength_gained,
            2
        );
    }

    #[test]
    fn test_intrinsic_temp_acid_res() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        grant_intrinsic(
            &mut w,
            p,
            &crate::hunger::CorpseIntrinsic::AcidResistance { duration: 10 },
        );
        assert_eq!(
            w.get_component::<StatusEffects>(p).unwrap().acid_resistance,
            10
        );
        for _ in 0..10 {
            tick_status_effects(&mut w, p, &mut r);
        }
        assert_eq!(
            w.get_component::<StatusEffects>(p).unwrap().acid_resistance,
            0
        );
    }

    #[test]
    fn test_intrinsic_temp_stone_res() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(
            &mut w,
            p,
            &crate::hunger::CorpseIntrinsic::StoneResistance { duration: 8 },
        );
        assert_eq!(
            w.get_component::<StatusEffects>(p)
                .unwrap()
                .stone_resistance,
            8
        );
    }

    #[test]
    fn test_intrinsic_temp_res_stacks() {
        let mut w = make_test_world();
        let p = w.player();
        grant_intrinsic(
            &mut w,
            p,
            &crate::hunger::CorpseIntrinsic::AcidResistance { duration: 5 },
        );
        grant_intrinsic(
            &mut w,
            p,
            &crate::hunger::CorpseIntrinsic::AcidResistance { duration: 7 },
        );
        assert_eq!(
            w.get_component::<StatusEffects>(p).unwrap().acid_resistance,
            12
        );
    }

    #[test]
    fn test_status_sick_dialogue_food() {
        let ev = sickness_dialogue(7, SICK_VOMITABLE);
        assert!(ev.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "sick-sickness-worse")
        ));
    }

    #[test]
    fn test_status_sick_dialogue_disease() {
        let ev = sickness_dialogue(7, SICK_NONVOMITABLE);
        assert!(
            ev.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key == "sick-illness-worse")
            )
        );
    }

    #[test]
    fn test_status_no_components_safe() {
        let mut w = make_test_world();
        let bare = w.spawn((crate::world::Positioned(Position::new(1, 1)),));
        assert!(!is_confused(&w, bare));
        assert!(!is_stunned(&w, bare));
    }

    #[test]
    fn test_status_zero_duration_noop() {
        let mut w = make_test_world();
        let p = w.player();
        let ev = make_confused(&mut w, p, 0);
        assert!(ev.is_empty());
        assert!(!is_confused(&w, p));
    }

    #[test]
    fn test_status_strangled_death() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        {
            let mut s = w.get_component_mut::<StatusEffects>(p).unwrap();
            s.strangled = STRANGLED_INITIAL;
        }
        for _ in 0..4 {
            let ev = tick_status_effects(&mut w, p, &mut r);
            assert!(
                !ev.iter()
                    .any(|e| matches!(e, EngineEvent::EntityDied { .. }))
            );
        }
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::EntityDied {
                cause: DeathCause::Strangulation,
                ..
            }
        )));
    }

    #[test]
    fn test_status_speed_expiry() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        {
            let mut s = w.get_component_mut::<StatusEffects>(p).unwrap();
            s.speed = 2;
        }
        {
            let mut h = w.get_component_mut::<HeroSpeedBonus>(p).unwrap();
            h.0 = HeroSpeed::VeryFast;
        }
        tick_status_effects(&mut w, p, &mut r);
        assert_eq!(
            w.get_component::<HeroSpeedBonus>(p).unwrap().0,
            HeroSpeed::VeryFast
        );
        let ev = tick_status_effects(&mut w, p, &mut r);
        assert!(ev.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::FastSpeed,
                ..
            }
        )));
        assert_eq!(
            w.get_component::<HeroSpeedBonus>(p).unwrap().0,
            HeroSpeed::Normal
        );
    }

    // I.3: Hallucination name scrambling

    #[test]
    fn test_hallucinate_names_different() {
        let mut r = test_rng();
        let real_monster = "goblin";
        let real_item = "long sword";

        // Run multiple times; at least one should differ from the real name.
        let mut monster_differed = false;
        let mut item_differed = false;
        for _ in 0..20 {
            let hm = hallucinate_monster_name(real_monster, &mut r);
            if hm != real_monster {
                monster_differed = true;
            }
            let hi = hallucinate_item_name(real_item, &mut r);
            if hi != real_item {
                item_differed = true;
            }
        }
        assert!(
            monster_differed,
            "hallucinated monster name should differ from real name"
        );
        assert!(
            item_differed,
            "hallucinated item name should differ from real name"
        );
    }

    #[test]
    fn test_hallucinate_monster_name_returns_bogus() {
        let mut r = test_rng();
        let name = hallucinate_monster_name("grid bug", &mut r);
        assert!(!name.is_empty(), "hallucinated name should not be empty");
        assert!(
            BOGUS_MONSTERS.contains(&name.as_str()),
            "hallucinated name should come from the bogus list"
        );
    }

    #[test]
    fn test_hallucinate_item_name_returns_bogus() {
        let mut r = test_rng();
        let name = hallucinate_item_name("mace", &mut r);
        assert!(!name.is_empty(), "hallucinated name should not be empty");
        assert!(
            BOGUS_ITEMS.contains(&name.as_str()),
            "hallucinated name should come from the bogus list"
        );
    }

    // ---------------------------------------------------------------
    // New timeout system tests
    // ---------------------------------------------------------------

    // Strangling dialogue
    #[test]
    fn test_choke_dialogue_emits_messages() {
        let mut r = SmallRng::seed_from_u64(0);
        for stage in 1..=5 {
            let ev = choke_dialogue(stage, &mut r);
            assert!(
                !ev.is_empty(),
                "choke dialogue should emit message at stage {stage}"
            );
        }
        assert!(choke_dialogue(0, &mut r).is_empty());
        assert!(choke_dialogue(6, &mut r).is_empty());
    }

    // Stoning side effects: stage 5 clears speed
    #[test]
    fn test_stoning_stage5_clears_speed() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        {
            let mut s = w.get_component_mut::<StatusEffects>(p).unwrap();
            s.speed = 50;
        }
        make_stoned(&mut w, p, STONING_INITIAL);
        tick_status_effects(&mut w, p, &mut r);
        // After stoning stage 5 dialogue, speed should be cleared
        assert_eq!(w.get_component::<StatusEffects>(p).unwrap().speed, 0);
    }

    // Stoning side effects: stage 3 paralyzes
    // Dialogue(3) fires when stoning==3. After tick 1 (5→4), tick 2 (4→3),
    // tick 3 sees dialogue(3) → Paralyze3, then phase 2 decrements paralysis.
    // So effective paralysis after tick 3 = max(0,3) - 1 = 2.
    #[test]
    fn test_stoning_stage3_paralyzes() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_stoned(&mut w, p, STONING_INITIAL);
        tick_status_effects(&mut w, p, &mut r); // dialogue(5), stoning 5→4
        tick_status_effects(&mut w, p, &mut r); // dialogue(4), stoning 4→3
        tick_status_effects(&mut w, p, &mut r); // dialogue(3)→Paralyze3 sets paralysis=3, phase2 decrements to 2, stoning 3→2
        // Paralysis was set to 3 in phase 1, then decremented by 1 in phase 2 = 2
        assert!(w.get_component::<StatusEffects>(p).unwrap().paralysis >= 2);
    }

    // Stoning side effects: stage 2 clears vomiting and sliming
    // Dialogue(2) fires when stoning==2. Need 4 ticks: 5→4→3→2, then tick 4 sees dialogue(2).
    #[test]
    fn test_stoning_stage2_clears_vomiting_sliming() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_stoned(&mut w, p, STONING_INITIAL);
        make_vomiting(&mut w, p, 20);
        make_slimed(&mut w, p, SLIMING_INITIAL);
        // Ticks: dialogue(5) stoning 5→4, dialogue(4) 4→3, dialogue(3) 3→2
        for _ in 0..3 {
            tick_status_effects(&mut w, p, &mut r);
        }
        // Now stoning==2. Next tick: dialogue(2) → ClearVomitingSliming
        tick_status_effects(&mut w, p, &mut r);
        let st = w.get_component::<StatusEffects>(p).unwrap();
        assert_eq!(st.vomiting, 0, "stoning stage 2 should clear vomiting");
        assert_eq!(st.sliming, 0, "stoning stage 2 should clear sliming");
    }

    // Sliming side effects: i=3 clears speed
    // sliming=10. i=t/2. ClearSpeed fires when i=3.
    // After phase 3 decrement each tick: 10→9→8→7. At tick 4, dialogue(7) fires i=3.
    #[test]
    fn test_sliming_stage_clears_speed() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        {
            let mut s = w.get_component_mut::<StatusEffects>(p).unwrap();
            s.speed = 50;
        }
        make_slimed(&mut w, p, SLIMING_INITIAL); // t=10
        // Speed should still be >0 after 3 ticks (i values: 5, 4, 4)
        for _ in 0..3 {
            tick_status_effects(&mut w, p, &mut r);
        }
        // sliming is now 7, speed has been decremented by phase 2 each turn: 50→47
        assert!(w.get_component::<StatusEffects>(p).unwrap().speed > 0);
        // Tick 4: dialogue(7) → i=3 → ClearSpeed sets speed=0
        tick_status_effects(&mut w, p, &mut r);
        assert_eq!(w.get_component::<StatusEffects>(p).unwrap().speed, 0);
    }

    // Sliming side effects: i=1 clears stoning
    #[test]
    fn test_sliming_clears_stoning() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_slimed(&mut w, p, SLIMING_INITIAL);
        make_stoned(&mut w, p, STONING_INITIAL);
        // Fast-forward to t=3 (i=1): need 7 ticks from t=10
        for _ in 0..7 {
            tick_status_effects(&mut w, p, &mut r);
        }
        // At t=3, i=1, ClearStoning fires
        // Note: stoning also counts down, so it may already be 0.
        // But the side effect explicitly clears it.
        let st = w.get_component::<StatusEffects>(p).unwrap();
        assert_eq!(st.stoning, 0, "sliming at i=1 should clear stoning");
    }

    // Vomiting side effects: v-1==9 adds confusion
    #[test]
    fn test_vomiting_adds_confusion() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_vomiting(&mut w, p, 10); // v=10, v-1=9 on first tick
        tick_status_effects(&mut w, p, &mut r);
        assert!(
            w.get_component::<StatusEffects>(p).unwrap().confusion > 0,
            "vomiting at v-1=9 should add confusion"
        );
    }

    // Vomiting side effects: v-1==6 adds stun AND confusion
    #[test]
    fn test_vomiting_adds_stun_and_confusion() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        make_vomiting(&mut w, p, 7); // v=7, v-1=6 on first tick
        tick_status_effects(&mut w, p, &mut r);
        let st = w.get_component::<StatusEffects>(p).unwrap();
        assert!(st.stun > 0, "vomiting at v-1=6 should add stun");
        assert!(
            st.confusion > 0,
            "vomiting at v-1=6 should add confusion (FALLTHROUGH)"
        );
    }

    // Fumbling self-cycling timer
    #[test]
    fn test_fumbling_self_cycling() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        {
            let mut s = w.get_component_mut::<StatusEffects>(p).unwrap();
            s.fumbling = 2;
        }
        tick_status_effects(&mut w, p, &mut r); // 2→1
        assert!(w.get_component::<StatusEffects>(p).unwrap().fumbling == 1);
        let ev = tick_status_effects(&mut w, p, &mut r); // 1→0→reset
        let st = w.get_component::<StatusEffects>(p).unwrap();
        assert!(st.fumbling > 0, "fumbling should reset on expiry");
        assert!(
            ev.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key == "status-fumble-trip")
            )
        );
    }

    // Sleepy self-cycling timer
    #[test]
    fn test_sleepy_self_cycling() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        {
            let mut s = w.get_component_mut::<StatusEffects>(p).unwrap();
            s.sleepy = 1;
        }
        let ev = tick_status_effects(&mut w, p, &mut r);
        let st = w.get_component::<StatusEffects>(p).unwrap();
        assert!(st.sleepy > 0, "sleepy should reset on expiry");
        assert!(st.paralysis > 0, "sleepy expiry should cause paralysis");
        assert!(
            ev.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key == "status-fall-asleep")
            )
        );
    }

    // Wounded legs recovery
    #[test]
    fn test_wounded_legs_recovery() {
        let mut w = make_test_world();
        let p = w.player();
        let mut r = test_rng();
        wound_legs(&mut w, p, 2);
        assert!(has_wounded_legs(&w, p));
        tick_status_effects(&mut w, p, &mut r); // 2→1
        assert!(has_wounded_legs(&w, p));
        let ev = tick_status_effects(&mut w, p, &mut r); // 1→0
        assert!(!has_wounded_legs(&w, p));
        assert!(ev.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "status-wounded-legs-healed")
        ));
    }

    // Levitation dialogue
    #[test]
    fn test_levitation_dialogue_at_3() {
        // t=3, i=(3-1)/2=1, odd → "wobble"
        let ev = levitation_dialogue(3);
        assert!(
            ev.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key == "levitation-wobble")
            )
        );
    }

    #[test]
    fn test_levitation_dialogue_even_silent() {
        // t=4, even → no message
        let ev = levitation_dialogue(4);
        assert!(ev.is_empty());
    }

    // Sleep dialogue: yawn at 4
    #[test]
    fn test_sleep_dialogue_yawn_at_4() {
        let ev = sleep_dialogue(4);
        assert!(
            ev.iter()
                .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "sleepy-yawn"))
        );
    }

    #[test]
    fn test_sleep_dialogue_silent_at_3() {
        assert!(sleep_dialogue(3).is_empty());
    }

    // Phaze dialogue
    #[test]
    fn test_phaze_dialogue_at_3() {
        // t=3, odd, i=3/2=1 → "feeling flabby"
        let ev = phaze_dialogue(3);
        assert!(ev.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "phaze-feeling-flabby")
        ));
    }

    // Spell protection dissipation
    #[test]
    fn test_spell_protection_dissipation() {
        let mut w = make_test_world();
        let p = w.player();
        let _ = w.ecs_mut().insert_one(
            p,
            SpellProtection {
                layers: 2,
                countdown: 1,
                interval: 10,
            },
        );
        let ev = tick_spell_protection(&mut w, p);
        assert!(ev.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "spell-protection-less-dense")));
        let sp = w.get_component::<SpellProtection>(p).unwrap();
        assert_eq!(sp.layers, 1);
        assert_eq!(sp.countdown, 10);
    }

    #[test]
    fn test_spell_protection_final_layer() {
        let mut w = make_test_world();
        let p = w.player();
        let _ = w.ecs_mut().insert_one(
            p,
            SpellProtection {
                layers: 1,
                countdown: 1,
                interval: 10,
            },
        );
        let ev = tick_spell_protection(&mut w, p);
        assert!(ev.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "spell-protection-disappears")));
        let sp = w.get_component::<SpellProtection>(p).unwrap();
        assert_eq!(sp.layers, 0);
    }

    // Hero counters
    #[test]
    fn test_hero_counters_gallop() {
        let mut w = make_test_world();
        let p = w.player();
        let _ = w.ecs_mut().insert_one(
            p,
            HeroCounters {
                creamed: 3,
                gallop: 1,
            },
        );
        let ev = tick_hero_counters(&mut w, p);
        assert!(ev.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "steed-stops-galloping")
        ));
        let hc = w.get_component::<HeroCounters>(p).unwrap();
        assert_eq!(hc.creamed, 2);
        assert_eq!(hc.gallop, 0);
    }

    // Burn away slime
    #[test]
    fn test_burn_away_slime() {
        let mut w = make_test_world();
        let p = w.player();
        make_slimed(&mut w, p, SLIMING_INITIAL);
        assert!(is_sliming(&w, p));
        let ev = burn_away_slime(&mut w, p);
        assert!(!is_sliming(&w, p));
        assert!(
            ev.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key == "slime-burned-away")
            )
        );
    }

    #[test]
    fn test_burn_away_slime_noop_if_not_slimed() {
        let mut w = make_test_world();
        let p = w.player();
        let ev = burn_away_slime(&mut w, p);
        assert!(ev.is_empty());
    }

    // Luck decay
    #[test]
    fn test_luck_decay_toward_base() {
        let mut w = make_test_world();
        let p = w.player();
        {
            let mut pc = w
                .get_component_mut::<crate::world::PlayerCombat>(p)
                .unwrap();
            pc.luck = 5;
        }
        // At turn 600, no luckstone
        tick_luck(
            &mut w,
            p,
            LuckTickContext {
                turn: 600,
                base_luck: 0,
                has_luckstone: false,
                luckstone_cursed: false,
                luckstone_blessed: false,
                has_amulet_or_angry_god: false,
            },
        );
        assert_eq!(
            w.get_component::<crate::world::PlayerCombat>(p)
                .unwrap()
                .luck,
            4
        );
    }

    #[test]
    fn test_luck_no_decay_with_uncursed_luckstone() {
        let mut w = make_test_world();
        let p = w.player();
        {
            let mut pc = w
                .get_component_mut::<crate::world::PlayerCombat>(p)
                .unwrap();
            pc.luck = 5;
        }
        tick_luck(
            &mut w,
            p,
            LuckTickContext {
                turn: 600,
                base_luck: 0,
                has_luckstone: true,
                luckstone_cursed: false,
                luckstone_blessed: false,
                has_amulet_or_angry_god: false,
            },
        );
        assert_eq!(
            w.get_component::<crate::world::PlayerCombat>(p)
                .unwrap()
                .luck,
            5
        );
    }

    #[test]
    fn test_luck_decay_with_cursed_luckstone() {
        let mut w = make_test_world();
        let p = w.player();
        {
            let mut pc = w
                .get_component_mut::<crate::world::PlayerCombat>(p)
                .unwrap();
            pc.luck = 5;
        }
        // Cursed luckstone: positive luck decays, negative doesn't recover
        tick_luck(
            &mut w,
            p,
            LuckTickContext {
                turn: 600,
                base_luck: 0,
                has_luckstone: true,
                luckstone_cursed: true,
                luckstone_blessed: false,
                has_amulet_or_angry_god: false,
            },
        );
        assert_eq!(
            w.get_component::<crate::world::PlayerCombat>(p)
                .unwrap()
                .luck,
            4
        );
    }

    #[test]
    fn test_luck_recovery_with_blessed_luckstone() {
        let mut w = make_test_world();
        let p = w.player();
        {
            let mut pc = w
                .get_component_mut::<crate::world::PlayerCombat>(p)
                .unwrap();
            pc.luck = -3;
        }
        // Blessed luckstone: negative luck recovers
        tick_luck(
            &mut w,
            p,
            LuckTickContext {
                turn: 600,
                base_luck: 0,
                has_luckstone: true,
                luckstone_cursed: false,
                luckstone_blessed: true,
                has_amulet_or_angry_god: false,
            },
        );
        assert_eq!(
            w.get_component::<crate::world::PlayerCombat>(p)
                .unwrap()
                .luck,
            -2
        );
    }

    #[test]
    fn test_luck_no_decay_off_interval() {
        let mut w = make_test_world();
        let p = w.player();
        {
            let mut pc = w
                .get_component_mut::<crate::world::PlayerCombat>(p)
                .unwrap();
            pc.luck = 5;
        }
        tick_luck(
            &mut w,
            p,
            LuckTickContext {
                turn: 601,
                base_luck: 0,
                has_luckstone: false,
                luckstone_cursed: false,
                luckstone_blessed: false,
                has_amulet_or_angry_god: false,
            },
        );
        assert_eq!(
            w.get_component::<crate::world::PlayerCombat>(p)
                .unwrap()
                .luck,
            5
        );
    }

    #[test]
    fn test_luck_faster_decay_with_amulet() {
        let mut w = make_test_world();
        let p = w.player();
        {
            let mut pc = w
                .get_component_mut::<crate::world::PlayerCombat>(p)
                .unwrap();
            pc.luck = 5;
        }
        // With amulet/angry god, interval is 300 instead of 600
        tick_luck(
            &mut w,
            p,
            LuckTickContext {
                turn: 300,
                base_luck: 0,
                has_luckstone: false,
                luckstone_cursed: false,
                luckstone_blessed: false,
                has_amulet_or_angry_god: true,
            },
        );
        assert_eq!(
            w.get_component::<crate::world::PlayerCombat>(p)
                .unwrap()
                .luck,
            4
        );
    }

    // ---------------------------------------------------------------
    // Expiration effect system tests
    // ---------------------------------------------------------------

    #[test]
    fn test_expiration_stoning_complete_at_zero() {
        assert_eq!(
            expiration_effect("stoning", 0),
            ExpirationEffect::StoningComplete
        );
    }

    #[test]
    fn test_expiration_stoning_progressive_warnings() {
        assert!(
            matches!(expiration_effect("stoning", 5), ExpirationEffect::Message(m) if m.contains("slowing down"))
        );
        assert!(
            matches!(expiration_effect("stoning", 4), ExpirationEffect::Message(m) if m.contains("stiffening"))
        );
        assert!(
            matches!(expiration_effect("stoning", 3), ExpirationEffect::Message(m) if m.contains("turned to stone"))
        );
        assert!(
            matches!(expiration_effect("stoning", 2), ExpirationEffect::Message(m) if m.contains("turned to stone"))
        );
        assert!(
            matches!(expiration_effect("stoning", 1), ExpirationEffect::Message(m) if m.contains("statue"))
        );
    }

    #[test]
    fn test_expiration_sliming_complete_at_zero() {
        assert_eq!(
            expiration_effect("sliming", 0),
            ExpirationEffect::SlimingComplete
        );
    }

    #[test]
    fn test_expiration_sliming_progressive_warnings() {
        // Messages only on odd values; even values are NoEffect
        assert!(
            matches!(expiration_effect("sliming", 9), ExpirationEffect::Message(m) if m.contains("green"))
        );
        assert_eq!(expiration_effect("sliming", 8), ExpirationEffect::NoEffect);
        assert!(
            matches!(expiration_effect("sliming", 7), ExpirationEffect::Message(m) if m.contains("oozy"))
        );
        assert!(
            matches!(expiration_effect("sliming", 5), ExpirationEffect::Message(m) if m.contains("peel"))
        );
        assert!(
            matches!(expiration_effect("sliming", 3), ExpirationEffect::Message(m) if m.contains("turning into"))
        );
        assert!(
            matches!(expiration_effect("sliming", 1), ExpirationEffect::Message(m) if m.contains("become"))
        );
    }

    #[test]
    fn test_expiration_sickness_kills_at_zero() {
        assert_eq!(
            expiration_effect("sick", 0),
            ExpirationEffect::SicknessKills
        );
    }

    #[test]
    fn test_expiration_sickness_progression() {
        // Messages at odd values with i = t/2
        assert!(
            matches!(expiration_effect("sick", 7), ExpirationEffect::Message(m) if m.contains("worse"))
        );
        assert_eq!(expiration_effect("sick", 6), ExpirationEffect::NoEffect);
        assert!(
            matches!(expiration_effect("sick", 5), ExpirationEffect::Message(m) if m.contains("severe"))
        );
        assert!(
            matches!(expiration_effect("sick", 3), ExpirationEffect::Message(m) if m.contains("Death's door"))
        );
    }

    #[test]
    fn test_expiration_strangulation_kills_at_zero() {
        assert_eq!(
            expiration_effect("strangled", 0),
            ExpirationEffect::StrangulationKills
        );
    }

    #[test]
    fn test_expiration_strangulation_warnings() {
        assert!(
            matches!(expiration_effect("strangled", 5), ExpirationEffect::Message(m) if m.contains("hard to breathe"))
        );
        assert!(
            matches!(expiration_effect("strangled", 4), ExpirationEffect::Message(m) if m.contains("gasping"))
        );
        assert!(
            matches!(expiration_effect("strangled", 3), ExpirationEffect::Message(m) if m.contains("no longer breathe"))
        );
        assert!(
            matches!(expiration_effect("strangled", 2), ExpirationEffect::Message(m) if m.contains("turning blue"))
        );
        assert!(
            matches!(expiration_effect("strangled", 1), ExpirationEffect::Message(m) if m.contains("suffocate"))
        );
    }

    #[test]
    fn test_expiration_speed_normal() {
        assert_eq!(expiration_effect("speed", 0), ExpirationEffect::SpeedNormal);
        assert_eq!(expiration_effect("fast", 0), ExpirationEffect::SpeedNormal);
        assert_eq!(
            expiration_effect("very_fast", 0),
            ExpirationEffect::SpeedNormal
        );
        assert_eq!(expiration_effect("speed", 5), ExpirationEffect::NoEffect);
    }

    #[test]
    fn test_expiration_blindness_restored() {
        assert_eq!(
            expiration_effect("blind", 0),
            ExpirationEffect::SightRestored
        );
        assert_eq!(
            expiration_effect("blindness", 0),
            ExpirationEffect::SightRestored
        );
    }

    #[test]
    fn test_expiration_confusion_clears() {
        assert_eq!(
            expiration_effect("confused", 0),
            ExpirationEffect::ConfusionClears
        );
        assert_eq!(
            expiration_effect("confusion", 0),
            ExpirationEffect::ConfusionClears
        );
    }

    #[test]
    fn test_expiration_stun_clears() {
        assert_eq!(
            expiration_effect("stunned", 0),
            ExpirationEffect::StunClears
        );
        assert_eq!(expiration_effect("stun", 0), ExpirationEffect::StunClears);
    }

    #[test]
    fn test_expiration_hallucination_ends() {
        assert_eq!(
            expiration_effect("hallucinating", 0),
            ExpirationEffect::HallucinationEnds
        );
        assert_eq!(
            expiration_effect("hallucination", 0),
            ExpirationEffect::HallucinationEnds
        );
    }

    #[test]
    fn test_expiration_levitation_fall() {
        assert_eq!(
            expiration_effect("levitation", 0),
            ExpirationEffect::FallFromLevitation { height: 0 }
        );
        assert_eq!(
            expiration_effect("levitation", 5),
            ExpirationEffect::NoEffect
        );
    }

    #[test]
    fn test_expiration_invisibility_ends() {
        assert_eq!(
            expiration_effect("invisibility", 0),
            ExpirationEffect::BecomeVisible
        );
    }

    #[test]
    fn test_expiration_polymorph_reverts() {
        assert_eq!(
            expiration_effect("polymorph", 0),
            ExpirationEffect::RevertPolymorph
        );
    }

    #[test]
    fn test_expiration_protection_fades() {
        assert!(
            matches!(expiration_effect("protection", 0), ExpirationEffect::Message(m) if m.contains("protected"))
        );
    }

    #[test]
    fn test_expiration_wounded_legs_heals() {
        assert!(
            matches!(expiration_effect("wounded_legs", 0), ExpirationEffect::Message(m) if m.contains("legs"))
        );
    }

    #[test]
    fn test_expiration_unknown_property() {
        assert_eq!(
            expiration_effect("nonexistent", 0),
            ExpirationEffect::NoEffect
        );
        assert_eq!(
            expiration_effect("nonexistent", 5),
            ExpirationEffect::NoEffect
        );
    }

    #[test]
    fn test_is_fatal_timeout_true() {
        assert!(is_fatal_timeout("stoning"));
        assert!(is_fatal_timeout("sliming"));
        assert!(is_fatal_timeout("sick"));
        assert!(is_fatal_timeout("strangled"));
    }

    #[test]
    fn test_is_fatal_timeout_false() {
        assert!(!is_fatal_timeout("confused"));
        assert!(!is_fatal_timeout("blind"));
        assert!(!is_fatal_timeout("hallucinating"));
        assert!(!is_fatal_timeout("levitation"));
        assert!(!is_fatal_timeout("speed"));
        assert!(!is_fatal_timeout("protection"));
    }

    #[test]
    fn test_tick_timed_properties_decrements_all() {
        let mut props = vec![
            ("confused".to_string(), 5),
            ("blind".to_string(), 3),
            ("stunned".to_string(), 1),
        ];
        tick_timed_properties(&mut props);
        // confused: 4, blind: 2, stunned: 0 → removed
        assert_eq!(props.len(), 2);
        assert_eq!(props[0], ("confused".to_string(), 4));
        assert_eq!(props[1], ("blind".to_string(), 2));
    }

    #[test]
    fn test_tick_timed_properties_removes_expired() {
        let mut props = vec![("confused".to_string(), 1), ("blind".to_string(), 1)];
        let effects = tick_timed_properties(&mut props);
        assert!(props.is_empty(), "all expired properties should be removed");
        // Both should produce expiration effects
        assert!(effects.iter().any(|(name, _)| name == "confused"));
        assert!(effects.iter().any(|(name, _)| name == "blind"));
    }

    #[test]
    fn test_tick_timed_properties_stoning_warnings() {
        let mut props = vec![("stoning".to_string(), 6)];
        let effects = tick_timed_properties(&mut props);
        // 6→5, stoning at 5 should produce warning
        assert_eq!(effects.len(), 1);
        assert!(matches!(&effects[0], (name, ExpirationEffect::Message(m))
            if name == "stoning" && m.contains("slowing")));
        assert_eq!(props[0].1, 5);
    }

    #[test]
    fn test_tick_timed_properties_fatal_at_zero() {
        let mut props = vec![("stoning".to_string(), 1)];
        let effects = tick_timed_properties(&mut props);
        // 1→0, fatal
        assert!(props.is_empty());
        assert!(
            effects
                .iter()
                .any(|(_, e)| *e == ExpirationEffect::StoningComplete)
        );
    }

    #[test]
    fn test_tick_timed_properties_non_fatal_expiry() {
        let mut props = vec![("speed".to_string(), 1)];
        let effects = tick_timed_properties(&mut props);
        assert!(props.is_empty());
        assert!(
            effects
                .iter()
                .any(|(_, e)| *e == ExpirationEffect::SpeedNormal)
        );
    }

    #[test]
    fn test_egg_hatch_at_threshold() {
        let result = check_egg_hatch(100, 100, "cockatrice");
        assert!(
            matches!(result, Some(ExpirationEffect::EggHatches { monster_type }) if monster_type == "cockatrice")
        );
    }

    #[test]
    fn test_egg_hatch_past_threshold() {
        let result = check_egg_hatch(150, 100, "snake");
        assert!(
            matches!(result, Some(ExpirationEffect::EggHatches { monster_type }) if monster_type == "snake")
        );
    }

    #[test]
    fn test_egg_no_hatch_before_threshold() {
        assert!(check_egg_hatch(50, 100, "cockatrice").is_none());
        assert!(check_egg_hatch(0, 100, "cockatrice").is_none());
    }

    // ── Death condition tests ──────────────────────────────────────────

    #[test]
    fn test_sickness_kills_at_zero() {
        let mut w = make_test_world();
        let p = w.player();
        let mut rng = test_rng();
        // Set sickness to 1 with non-vomitable type (guaranteed death).
        if let Some(mut s) = w.get_component_mut::<StatusEffects>(p) {
            s.sick = 1;
            s.sick_type = SICK_NONVOMITABLE;
        }
        let ev = tick_status_effects(&mut w, p, &mut rng);
        assert!(
            ev.iter().any(|e| matches!(
                e,
                EngineEvent::EntityDied {
                    cause: DeathCause::Poisoning,
                    ..
                }
            )),
            "sickness at 1 with non-vomitable type should kill"
        );
    }

    #[test]
    fn test_sickness_warns_before_death() {
        // Sickness dialogue at r=3 (r/2=1) emits "sick-deaths-door".
        let ev = sickness_dialogue(3, SICK_NONVOMITABLE);
        assert!(
            ev.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key == "sick-deaths-door")
            ),
            "sickness at 3 should warn about death's door"
        );
    }

    #[test]
    fn test_levitation_fall_damage() {
        let mut rng = test_rng();
        let result = check_levitation_fall(false, false, false, false, false, &mut rng);
        match result {
            LevitationEndResult::Landing { damage } => {
                assert!(
                    (1..=6).contains(&damage),
                    "fall damage should be 1-6, got {}",
                    damage
                );
            }
            other => panic!("expected Landing, got {:?}", other),
        }
    }

    #[test]
    fn test_levitation_fall_over_lava_fatal() {
        let mut rng = test_rng();
        let result = check_levitation_fall(false, false, true, false, false, &mut rng);
        assert_eq!(
            result,
            LevitationEndResult::FallIntoLava,
            "falling over lava should produce FallIntoLava"
        );
    }

    #[test]
    fn test_levitation_fall_over_void_fatal() {
        let mut rng = test_rng();
        let result = check_levitation_fall(true, false, false, false, false, &mut rng);
        assert!(
            matches!(result, LevitationEndResult::Fatal { .. }),
            "falling over void should be Fatal"
        );
    }

    #[test]
    fn test_levitation_fall_safe_with_flying() {
        let mut rng = test_rng();
        let result = check_levitation_fall(true, false, true, true, false, &mut rng);
        assert_eq!(
            result,
            LevitationEndResult::SafeLanding,
            "flying should prevent fatal fall"
        );
    }

    #[test]
    fn test_drowning_without_protection() {
        assert!(
            check_drowning(false, false, false, false, false),
            "unprotected player should drown"
        );
    }

    #[test]
    fn test_no_drowning_with_water_walking() {
        assert!(
            !check_drowning(true, false, false, false, false),
            "water walking should prevent drowning"
        );
    }

    #[test]
    fn test_no_drowning_with_magical_breathing() {
        assert!(
            !check_drowning(false, true, false, false, false),
            "magical breathing should prevent drowning"
        );
    }

    #[test]
    fn test_no_drowning_with_swimming() {
        assert!(
            !check_drowning(false, false, false, false, true),
            "swimming form should prevent drowning"
        );
    }

    #[test]
    fn test_strangled_kills_at_zero() {
        let mut w = make_test_world();
        let p = w.player();
        let mut rng = test_rng();
        if let Some(mut s) = w.get_component_mut::<StatusEffects>(p) {
            s.strangled = 1;
        }
        let ev = tick_status_effects(&mut w, p, &mut rng);
        assert!(
            ev.iter().any(|e| matches!(
                e,
                EngineEvent::EntityDied {
                    cause: DeathCause::Strangulation,
                    ..
                }
            )),
            "strangulation at 1 should kill"
        );
    }
}
