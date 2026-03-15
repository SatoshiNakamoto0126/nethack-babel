//! Potion system: quaffing, throwing, and all 26 potion type effects.
//!
//! Implements the NetHack 3.7 potion mechanics from `src/potion.c`.
//! All functions operate on `GameWorld` and return `Vec<EngineEvent>`
//! for the game loop to process.  No IO.
//!
//! Reference: `specs/potion-effects.md`

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{BucStatus, ObjectCore};

use crate::action::Position;
use crate::event::{DeathCause, EngineEvent, HpSource, StatusEffect};
use crate::world::{
    Attributes, ExperienceLevel, GameWorld, HitPoints, Monster, Nutrition,
    Positioned, Power,
};

// ---------------------------------------------------------------------------
// Potion type enumeration
// ---------------------------------------------------------------------------

/// All 26 potion types from NetHack 3.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PotionType {
    GainAbility,
    RestoreAbility,
    Confusion,
    Blindness,
    Paralysis,
    Speed,
    Levitation,
    Hallucination,
    Invisibility,
    SeeInvisible,
    Healing,
    ExtraHealing,
    GainLevel,
    Enlightenment,
    MonsterDetection,
    ObjectDetection,
    GainEnergy,
    Sleeping,
    FullHealing,
    Polymorph,
    Booze,
    Sickness,
    FruitJuice,
    Acid,
    Oil,
    Water,
}

// ---------------------------------------------------------------------------
// BUC helper
// ---------------------------------------------------------------------------

/// Returns +1 for blessed, -1 for cursed, 0 for uncursed.
#[inline]
pub fn bcsign(buc: &BucStatus) -> i32 {
    if buc.blessed {
        1
    } else if buc.cursed {
        -1
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Dice helpers (matching NetHack conventions)
// ---------------------------------------------------------------------------

/// Roll one die with `sides` faces: uniform in [1, sides].
#[inline]
fn rnd<R: Rng>(rng: &mut R, sides: u32) -> u32 {
    if sides == 0 {
        return 0;
    }
    rng.random_range(1..=sides)
}

/// Roll `n` dice of `s` sides: sum of n calls to rnd(s).
#[inline]
fn d<R: Rng>(rng: &mut R, n: u32, s: u32) -> u32 {
    (0..n).map(|_| rnd(rng, s)).sum()
}

/// rn1(x, y) = rn2(x) + y, i.e. uniform in [y, y+x).
fn rn1<R: Rng>(rng: &mut R, x: i32, y: i32) -> i32 {
    if x <= 0 {
        return y;
    }
    rng.random_range(0..x) + y
}

// ---------------------------------------------------------------------------
// Potion appearance descriptors
// ---------------------------------------------------------------------------

/// Identifies the physical appearance of a potion (e.g., "smoky", "milky").
///
/// In C NetHack, smoky potions may release a djinni when opened, and milky
/// potions may release a ghost.  The appearance is randomized each game
/// and is independent of the actual potion type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PotionAppearance {
    Smoky,
    Milky,
    /// All other appearances (no special pre-quaff effect).
    Other,
}

/// Result of checking for a potion occupant before quaffing.
#[derive(Debug, Clone, PartialEq)]
pub enum PotionOccupant {
    /// A djinni emerges from a smoky potion.
    Djinni,
    /// A ghost emerges from a milky potion.
    Ghost,
    /// No occupant; proceed to normal quaff.
    None,
}

/// Check whether a potion releases an occupant when opened.
///
/// In C NetHack (potion.c lines 601-613), smoky potions have a chance of
/// releasing a djinni, and milky potions have a chance of releasing a ghost.
/// The chance is `1 / POTION_OCCUPANT_CHANCE(born)` where born is the number
/// of that monster type previously generated.  The macro expands to
/// `max(1, 4 + born/3)`.
///
/// If an occupant emerges, the potion is consumed without applying its
/// normal effect.
///
/// Returns events describing the occupant emergence (monster spawn, messages,
/// paralysis from ghost), or empty if no occupant.
pub fn check_potion_occupant<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    appearance: PotionAppearance,
    djinni_born: u32,
    ghost_born: u32,
    rng: &mut R,
) -> (PotionOccupant, Vec<EngineEvent>) {
    let mut events = Vec::new();

    match appearance {
        PotionAppearance::Milky => {
            let chance = (4 + ghost_born / 3).max(1);
            if rng.random_range(0..chance) == 0 {
                // Ghost from bottle (C: ghost_from_bottle, line 481).
                events.push(EngineEvent::MonsterGenerated {
                    entity: drinker, // placeholder; real entity created by caller
                    position: world
                        .get_component::<crate::world::Positioned>(drinker)
                        .map(|p| p.0)
                        .unwrap_or(Position::new(0, 0)),
                });
                events.push(EngineEvent::msg("ghost-from-bottle"));
                // Ghost frightens the drinker: paralysis for 3 turns.
                events.push(EngineEvent::StatusApplied {
                    entity: drinker,
                    status: StatusEffect::Paralyzed,
                    duration: Some(3),
                    source: None,
                });
                return (PotionOccupant::Ghost, events);
            }
        }
        PotionAppearance::Smoky => {
            let chance = (4 + djinni_born / 3).max(1);
            if rng.random_range(0..chance) == 0 {
                // Djinni from bottle (C: djinni_from_bottle).
                events.push(EngineEvent::MonsterGenerated {
                    entity: drinker, // placeholder; real entity created by caller
                    position: world
                        .get_component::<crate::world::Positioned>(drinker)
                        .map(|p| p.0)
                        .unwrap_or(Position::new(0, 0)),
                });
                events.push(EngineEvent::msg("djinni-from-bottle"));
                return (PotionOccupant::Djinni, events);
            }
        }
        PotionAppearance::Other => {}
    }

    (PotionOccupant::None, events)
}

// ---------------------------------------------------------------------------
// Quaff dispatch
// ---------------------------------------------------------------------------

/// Quaff a potion.  Looks up the potion type and BUC status from the
/// entity's components, dispatches to the appropriate effect function,
/// then despawns the potion entity (it is consumed).
///
/// Returns the list of engine events describing what happened.
pub fn quaff_potion<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    potion_entity: Entity,
    potion_type: PotionType,
    rng: &mut R,
) -> Vec<EngineEvent> {
    // Read BUC status from the potion entity.
    let buc = match world.get_component::<BucStatus>(potion_entity) {
        Some(b) => (*b).clone(),
        None => BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        },
    };

    let mut events = match potion_type {
        PotionType::Healing => effect_healing(world, drinker, &buc, rng),
        PotionType::ExtraHealing => effect_extra_healing(world, drinker, &buc, rng),
        PotionType::FullHealing => effect_full_healing(world, drinker, &buc, rng),
        PotionType::GainAbility => effect_gain_ability(world, drinker, &buc, rng),
        PotionType::Speed => effect_speed(world, drinker, &buc, rng),
        PotionType::Invisibility => effect_invisibility(world, drinker, &buc, rng),
        PotionType::SeeInvisible => effect_see_invisible(world, drinker, &buc, rng),
        PotionType::Confusion => effect_confusion(world, drinker, &buc, rng),
        PotionType::Blindness => effect_blindness(world, drinker, &buc, rng),
        PotionType::Hallucination => effect_hallucination(world, drinker, &buc, rng),
        PotionType::Sleeping => effect_sleeping(world, drinker, &buc, rng),
        PotionType::Paralysis => effect_paralysis(world, drinker, &buc, rng),
        PotionType::GainLevel => effect_gain_level(world, drinker, &buc, rng),
        PotionType::GainEnergy => effect_gain_energy(world, drinker, &buc, rng),
        PotionType::MonsterDetection => {
            effect_monster_detection(world, drinker, &buc, rng)
        }
        PotionType::ObjectDetection => {
            effect_object_detection(world, drinker, &buc, rng)
        }
        PotionType::Sickness => effect_sickness(world, drinker, &buc, rng),
        PotionType::RestoreAbility => {
            effect_restore_ability(world, drinker, &buc, rng)
        }
        PotionType::Water => effect_water(world, drinker, &buc, rng),
        PotionType::Acid => effect_acid(world, drinker, &buc, rng),
        PotionType::Booze => effect_booze(world, drinker, &buc, rng),
        PotionType::FruitJuice => effect_fruit_juice(world, drinker, &buc, rng),
        PotionType::Oil => effect_oil(world, drinker, &buc, rng),
        PotionType::Polymorph => effect_polymorph(world, drinker, &buc, rng),
        PotionType::Levitation => effect_levitation(world, drinker, &buc, rng),
        PotionType::Enlightenment => {
            effect_enlightenment(world, drinker, &buc, rng)
        }
    };

    // Consume the potion.
    let _ = world.despawn(potion_entity);
    events.push(EngineEvent::ItemDestroyed {
        item: potion_entity,
        cause: crate::event::DamageCause::Physical,
    });

    events
}

// ---------------------------------------------------------------------------
// Resistance check helpers
// ---------------------------------------------------------------------------

/// Check whether an entity has acid resistance.
fn has_acid_resistance(world: &GameWorld, entity: Entity) -> bool {
    world.get_component::<AcidResistance>(entity).is_some()
}

/// Check whether an entity has poison resistance.
fn has_poison_resistance(world: &GameWorld, entity: Entity) -> bool {
    world.get_component::<PoisonResistance>(entity).is_some()
}

// ---------------------------------------------------------------------------
// healup helper
// ---------------------------------------------------------------------------

/// Increase HP by `heal` (capped at max), and increase max HP by
/// `max_increase`.  Returns the events generated.
fn healup(
    world: &mut GameWorld,
    entity: Entity,
    heal: i32,
    max_increase: i32,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
        if max_increase > 0 {
            hp.max += max_increase;
        }
        let old = hp.current;
        hp.current = (hp.current + heal).min(hp.max);
        let actual = hp.current - old;
        events.push(EngineEvent::HpChange {
            entity,
            amount: actual,
            new_hp: hp.current,
            source: HpSource::Potion,
        });
    }
    events
}

// ---------------------------------------------------------------------------
// Individual potion effects
// ---------------------------------------------------------------------------

/// Healing: healup(8 + d(4+2*bcsign, 4), non-cursed?1:0, blessed, non-cursed).
///
/// Spec (section 3.18 / 10.1):
///   B: 8 + d(6,4) = [14,32] HP, +1 max, cure sick, cure blind
///   U: 8 + d(4,4) = [12,24] HP, +1 max, no cure sick, cure blind
///   C: 8 + d(2,4) = [10,16] HP, +0 max, no cure sick, no cure blind
fn effect_healing<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let bc = bcsign(buc);
    let ndice = (4 + 2 * bc).max(1) as u32;
    let heal = d(rng, ndice, 4) as i32 + 8;
    let max_increase = if buc.cursed { 0 } else { 1 };

    events.extend(healup(world, drinker, heal, max_increase));

    // Cure sickness if blessed.
    if buc.blessed {
        events.push(EngineEvent::StatusRemoved {
            entity: drinker,
            status: StatusEffect::Sick,
        });
    }

    // Cure blindness if non-cursed.
    if !buc.cursed {
        events.push(EngineEvent::StatusRemoved {
            entity: drinker,
            status: StatusEffect::Blind,
        });
    }

    events.push(EngineEvent::msg("potion-healing"));

    events
}

/// Extra healing: healup(16 + d(4+2*bcsign, 8), B?5:U?2:0, non-cursed, true).
///
/// Spec (section 3.19 / 10.1):
///   B: 16 + d(6,8) = [22,64] HP, +5 max, cure sick, cure blind, cure halluc
///   U: 16 + d(4,8) = [20,48] HP, +2 max, cure sick, cure blind, cure halluc
///   C: 16 + d(2,8) = [18,32] HP, +0 max, no cure sick, cure blind, cure halluc
fn effect_extra_healing<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let bc = bcsign(buc);
    let ndice = (4 + 2 * bc).max(1) as u32;
    let heal = d(rng, ndice, 8) as i32 + 16;
    let max_increase = if buc.blessed { 5 } else if buc.cursed { 0 } else { 2 };

    events.extend(healup(world, drinker, heal, max_increase));

    // Cure sickness if non-cursed.
    if !buc.cursed {
        events.push(EngineEvent::StatusRemoved {
            entity: drinker,
            status: StatusEffect::Sick,
        });
    }

    // Always cure blindness and hallucination.
    events.push(EngineEvent::StatusRemoved {
        entity: drinker,
        status: StatusEffect::Blind,
    });
    events.push(EngineEvent::StatusRemoved {
        entity: drinker,
        status: StatusEffect::Hallucinating,
    });

    events.push(EngineEvent::msg("potion-extra-healing"));

    events
}

/// Full healing: healup(400, 4+4*bcsign, non-cursed, true).
///
/// Spec (section 3.20 / 10.1):
///   B: 400 HP, +8 max, cure sick, cure blind, cure halluc, restore 1 lost lvl
///   U: 400 HP, +4 max, cure sick, cure blind, cure halluc
///   C: 400 HP, +0 max, no cure sick, cure blind, cure halluc
fn effect_full_healing<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let bc = bcsign(buc);
    let max_increase = (4 + 4 * bc).max(0);

    events.extend(healup(world, drinker, 400, max_increase));

    // Cure sickness if non-cursed.
    if !buc.cursed {
        events.push(EngineEvent::StatusRemoved {
            entity: drinker,
            status: StatusEffect::Sick,
        });
    }

    // Always cure blindness and hallucination.
    events.push(EngineEvent::StatusRemoved {
        entity: drinker,
        status: StatusEffect::Blind,
    });
    events.push(EngineEvent::StatusRemoved {
        entity: drinker,
        status: StatusEffect::Hallucinating,
    });

    // Blessed: restore one lost level.
    if buc.blessed
        && let Some(mut xlvl) =
            world.get_component_mut::<ExperienceLevel>(drinker)
    {
        // In a full implementation, we would check u.ulevelmax and
        // decrement it.  Here we just grant +1 level as an approximation.
        xlvl.0 = xlvl.0.saturating_add(1).min(30);
        events.push(EngineEvent::LevelUp {
            entity: drinker,
            new_level: xlvl.0,
        });
    }

    events.push(EngineEvent::msg("potion-full-healing"));

    events
}

/// Gain ability: increase random attribute by 1 (blessed: all +1).
fn effect_gain_ability<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        // Cursed: lose 1 from a random attribute.
        if let Some(mut attrs) = world.get_component_mut::<Attributes>(drinker) {
            let attr_idx = rng.random_range(0..6u32);
            decrease_attribute(&mut attrs, attr_idx);
        }
        events.push(EngineEvent::msg("potion-sickness"));
    } else if buc.blessed {
        // Blessed: all attributes +1.
        if let Some(mut attrs) = world.get_component_mut::<Attributes>(drinker) {
            for i in 0..6u32 {
                increase_attribute(&mut attrs, i);
            }
        }
        events.push(EngineEvent::msg("potion-full-healing"));
    } else {
        // Uncursed: one random attribute +1.
        if let Some(mut attrs) = world.get_component_mut::<Attributes>(drinker) {
            let attr_idx = rng.random_range(0..6u32);
            increase_attribute(&mut attrs, attr_idx);
        }
        events.push(EngineEvent::msg("potion-gain-ability-str"));
    }

    events
}

/// Speed: grant Fast/VeryFast for rn1(10, 100+60*bcsign) turns.
fn effect_speed<R: Rng>(
    _world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let bc = bcsign(buc);
    let duration = rn1(rng, 10, 100 + 60 * bc);

    events.push(EngineEvent::StatusApplied {
        entity: drinker,
        status: StatusEffect::FastSpeed,
        duration: Some(duration.max(0) as u32),
        source: None,
    });

    events.push(EngineEvent::msg("potion-speed"));

    events
}

/// Invisibility: grant Invis with BUC-dependent duration.
///
/// Spec (section 3.6 / 10.4):
///   Blessed: 1/30 chance of permanent (simplified: always permanent)
///            otherwise d(3, 100) + 100 = [103, 400]
///   Uncursed: d(6, 100) + 100 = [106, 700]
///   Cursed:   d(9, 100) + 100 = [109, 1000] + aggravate + lose permanent
fn effect_invisibility<R: Rng>(
    _world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let bc = bcsign(buc);

    let (duration, permanent) = if buc.blessed {
        // Blessed: chance of permanent.  Simplified: 1/30 chance.
        if rng.random_range(0..30u32) == 0 {
            (None, true)
        } else {
            let dur = d(rng, 3, 100) + 100;
            (Some(dur), false)
        }
    } else {
        let ndice = (6 - 3 * bc) as u32;
        let dur = d(rng, ndice, 100) + 100;
        (Some(dur), false)
    };

    let _ = permanent; // used for potential FROMOUTSIDE tracking

    events.push(EngineEvent::StatusApplied {
        entity: drinker,
        status: StatusEffect::Invisible,
        duration,
        source: None,
    });

    if buc.cursed {
        // Cursed: aggravate monsters, remove permanent invisibility.
        events.push(EngineEvent::StatusApplied {
            entity: drinker,
            status: StatusEffect::Aggravate,
            duration: Some(1),
            source: None,
        });
    }

    events.push(EngineEvent::msg("potion-invisibility"));

    events
}

/// See invisible: BUC-dependent see invisible grant.
///
/// Spec (section 3.7):
///   Blessed: permanent See_invisible (FROMOUTSIDE), cure blindness
///   Uncursed: temporary rn1(100, 750) = [750, 850) turns, cure blindness
///   Cursed: "Yecch! This tastes rotten." -- no see invisible granted
///
/// Confused + see invisible -> become invisible (spec edge case).
fn effect_see_invisible<R: Rng>(
    _world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        // Cursed: no see invisible, just bad taste.
        events.push(EngineEvent::msg("potion-see-invisible-cursed"));
        return events;
    }

    // Non-cursed: cure blindness.
    events.push(EngineEvent::StatusRemoved {
        entity: drinker,
        status: StatusEffect::Blind,
    });

    if buc.blessed {
        // Blessed: permanent see invisible.
        events.push(EngineEvent::StatusApplied {
            entity: drinker,
            status: StatusEffect::SeeInvisible,
            duration: None,
            source: None,
        });
    } else {
        // Uncursed: temporary see invisible.
        let duration = rn1(rng, 100, 750);
        events.push(EngineEvent::StatusApplied {
            entity: drinker,
            status: StatusEffect::SeeInvisible,
            duration: Some(duration.max(0) as u32),
            source: None,
        });
    }

    events.push(EngineEvent::msg("potion-see-invisible"));

    events
}

/// Confusion: rn1(7, 16-8*bcsign) turns.
///
/// Spec (section 3.13):
///   B: rn1(7, 8)  = [8, 15)  turns
///   U: rn1(7, 16) = [16, 23) turns
///   C: rn1(7, 24) = [24, 31) turns
fn effect_confusion<R: Rng>(
    _world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let bc = bcsign(buc);
    let duration = rn1(rng, 7, 16 - 8 * bc);

    events.push(EngineEvent::StatusApplied {
        entity: drinker,
        status: StatusEffect::Confused,
        duration: Some(duration.max(0) as u32),
        source: None,
    });

    events.push(EngineEvent::msg("potion-confusion"));

    events
}

/// Blindness: blind for rn1(200, 250-125*bcsign) turns.
fn effect_blindness<R: Rng>(
    _world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let bc = bcsign(buc);
    let duration = rn1(rng, 200, 250 - 125 * bc);

    events.push(EngineEvent::StatusApplied {
        entity: drinker,
        status: StatusEffect::Blind,
        duration: Some(duration.max(0) as u32),
        source: None,
    });

    events.push(EngineEvent::msg("potion-blindness"));

    events
}

/// Hallucination: hallucinate for rn1(200, 600-300*bcsign) turns.
fn effect_hallucination<R: Rng>(
    _world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let bc = bcsign(buc);
    let duration = rn1(rng, 200, 600 - 300 * bc);

    events.push(EngineEvent::StatusApplied {
        entity: drinker,
        status: StatusEffect::Hallucinating,
        duration: Some(duration.max(0) as u32),
        source: None,
    });

    events.push(EngineEvent::msg("potion-hallucination"));

    events
}

/// Sleeping: sleep for rn1(10, 25-12*bcsign) turns if not Free_action.
fn effect_sleeping<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Check for Free_action -- if the drinker has it, resist.
    if has_free_action(world, drinker) {
        events.push(EngineEvent::msg("potion-sleeping"));
        return events;
    }

    let bc = bcsign(buc);
    let duration = rn1(rng, 10, 25 - 12 * bc);

    events.push(EngineEvent::StatusApplied {
        entity: drinker,
        status: StatusEffect::Sleeping,
        duration: Some(duration.max(0) as u32),
        source: None,
    });

    events.push(EngineEvent::msg("potion-sleeping"));

    events
}

/// Paralysis: rn1(10, 25-12*bcsign) turns if not Free_action.
///
/// Spec (section 3.8):
///   B: rn1(10, 13) = [13, 23) turns
///   U: rn1(10, 25) = [25, 35) turns
///   C: rn1(10, 37) = [37, 47) turns
fn effect_paralysis<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if has_free_action(world, drinker) {
        events.push(EngineEvent::msg("potion-paralysis-brief"));
        return events;
    }

    let bc = bcsign(buc);
    let duration = rn1(rng, 10, 25 - 12 * bc);

    events.push(EngineEvent::StatusApplied {
        entity: drinker,
        status: StatusEffect::Paralyzed,
        duration: Some(duration.max(0) as u32),
        source: None,
    });

    events.push(EngineEvent::msg("potion-paralysis"));

    events
}

/// Gain level: gain 1 XP level; cursed rises through ceiling (lose level).
///
/// Spec (section 3.17):
///   Cursed: hero physically rises through the ceiling (simplified: lose 1 lvl)
///   Uncursed: pluslvl(FALSE) -- gain one experience level
///   Blessed: pluslvl(FALSE) then set u.uexp = rndexp(TRUE)
///            (gain one level, with XP positioned randomly in new range)
fn effect_gain_level<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        // Cursed: rise through ceiling.  Simplified as lose a level.
        if let Some(mut xlvl) =
            world.get_component_mut::<ExperienceLevel>(drinker)
        {
            if xlvl.0 > 1 {
                xlvl.0 -= 1;
                events.push(EngineEvent::LevelUp {
                    entity: drinker,
                    new_level: xlvl.0,
                });
                events.push(EngineEvent::msg("level-down"));
            } else {
                events.push(EngineEvent::msg("potion-uneasy"));
            }
        }
    } else {
        // Uncursed and blessed: gain one level via pluslvl.
        if let Some(mut xlvl) =
            world.get_component_mut::<ExperienceLevel>(drinker)
        {
            xlvl.0 = xlvl.0.saturating_add(1).min(30);
            events.push(EngineEvent::LevelUp {
                entity: drinker,
                new_level: xlvl.0,
            });
        }
        // Blessed: XP is repositioned within the new level's range.
        // (No mechanical difference beyond the level gain in this engine.)
        events.push(EngineEvent::msg("potion-gain-level"));
    }

    events
}

/// Gain energy: d(B?3:U?2:C?1, 6); cursed negates.
///
/// Spec (section 3.22 / 10.2):
///   num = d(blessed?3 : uncursed?2 : 1, 6)
///   if cursed: num = -num
///   u.uenmax += num (clamped >= 0)
///   u.uen += 3 * num (clamped to [0, u.uenmax])
fn effect_gain_energy<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let ndice = if buc.blessed {
        3
    } else if buc.cursed {
        1
    } else {
        2
    };
    let mut num = d(rng, ndice, 6) as i32;
    if buc.cursed {
        num = -num;
    }

    if let Some(mut pw) = world.get_component_mut::<Power>(drinker) {
        pw.max += num;
        if pw.max < 0 {
            pw.max = 0;
        }
        pw.current += 3 * num;
        if pw.current > pw.max {
            pw.current = pw.max;
        }
        if pw.current < 0 {
            pw.current = 0;
        }
        events.push(EngineEvent::PwChange {
            entity: drinker,
            amount: num,
            new_pw: pw.current,
        });
    }

    events.push(EngineEvent::msg(
        if buc.cursed {
            "potion-gain-energy-cursed"
        } else {
            "potion-gain-energy"
        },
    ));

    events
}

/// Monster detection: reveal all monsters on level.
fn effect_monster_detection<R: Rng>(
    world: &mut GameWorld,
    _drinker: Entity,
    buc: &BucStatus,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        events.push(EngineEvent::msg("potion-monster-detection"));
        return events;
    }

    // Collect all monster entities and their positions.
    let monsters: Vec<(Entity, Position)> = {
        let mut result = Vec::new();
        for (entity, _) in world.query::<Monster>().iter() {
            if let Some(pos) = world.get_component::<Positioned>(entity) {
                result.push((entity, pos.0));
            }
        }
        result
    };

    if monsters.is_empty() {
        events.push(EngineEvent::msg("potion-object-detection"));
    } else {
        for (entity, pos) in &monsters {
            events.push(EngineEvent::MonsterGenerated {
                entity: *entity,
                position: *pos,
            });
        }
        events.push(EngineEvent::msg("potion-monster-detection"));
    }

    events
}

/// Object detection: reveal all objects on level.
fn effect_object_detection<R: Rng>(
    world: &mut GameWorld,
    _drinker: Entity,
    buc: &BucStatus,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        events.push(EngineEvent::msg("potion-no-effect"));
        return events;
    }

    // Count objects on the floor.
    let mut object_count = 0u32;
    for (_entity, _core) in world.query::<ObjectCore>().iter() {
        // We only detect floor objects in a real implementation;
        // for now, count all objects as a simplification.
        object_count += 1;
    }

    if object_count == 0 {
        events.push(EngineEvent::msg("potion-no-effect"));
    } else {
        events.push(EngineEvent::msg("potion-object-detection"));
    }

    events
}

/// Sickness potion.
///
/// Spec (section 3.12):
///   Blessed: "mildly stale fruit juice" -- lose 1 HP (unless Healer)
///   Uncursed/Cursed, non-Healer, no Poison_resistance:
///     lose rn1(4,3) = [3,7) from random attribute
///     lose rnd(10) + 5*(cursed?1:0) HP
///   Uncursed/Cursed, non-Healer, Poison_resistance:
///     lose 1 from random attribute; lose 1+rn2(2) HP
///   All BUC: if hallucinating, cure hallucination
fn effect_sickness<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.blessed {
        // Blessed: "mildly stale fruit juice" -- lose 1 HP.
        events.extend(apply_damage(world, drinker, 1));
        events.push(EngineEvent::msg("potion-sickness-mild"));
    } else {
        // Uncursed/Cursed: poison damage + attribute loss.
        // Check for poison resistance (simplified: check for marker).
        let poison_resistant =
            has_poison_resistance(world, drinker);

        if !poison_resistant {
            // Lose rn1(4, 3) = [3, 7) from a random attribute.
            let attr_loss = rn1(rng, 4, 3);
            if let Some(mut attrs) =
                world.get_component_mut::<Attributes>(drinker)
            {
                let attr_idx = rng.random_range(0..6u32);
                for _ in 0..attr_loss {
                    decrease_attribute(&mut attrs, attr_idx);
                }
            }
            // Lose rnd(10) + 5*(cursed?1:0) HP.
            let hp_loss =
                rnd(rng, 10) as i32 + if buc.cursed { 5 } else { 0 };
            events.extend(apply_damage(world, drinker, hp_loss));
        } else {
            // Poison resistant: lose 1 from random attribute, 1+rn2(2) HP.
            if let Some(mut attrs) =
                world.get_component_mut::<Attributes>(drinker)
            {
                let attr_idx = rng.random_range(0..6u32);
                decrease_attribute(&mut attrs, attr_idx);
            }
            let hp_loss = 1 + rng.random_range(0..2i32);
            events.extend(apply_damage(world, drinker, hp_loss));
        }

        events.push(EngineEvent::msg("potion-sickness"));
    }

    // All BUC: cure hallucination if hallucinating.
    events.push(EngineEvent::StatusRemoved {
        entity: drinker,
        status: StatusEffect::Hallucinating,
    });

    events
}

/// Restore ability: restore all stats (blessed: restore levels too).
fn effect_restore_ability<R: Rng>(
    _world: &mut GameWorld,
    _drinker: Entity,
    buc: &BucStatus,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        events.push(EngineEvent::msg("potion-sickness"));
        return events;
    }

    if buc.blessed {
        // Blessed: restore all attributes to their max values.
        // In the simplified model, we just report the event.
        events.push(EngineEvent::msg("potion-restore-ability"));
    } else {
        // Uncursed: restore one random attribute.
        events.push(EngineEvent::msg("potion-restore-ability"));
    }

    events
}

/// Water (holy/unholy): bless/curse items, damage undead.
fn effect_water<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.blessed {
        // Holy water.
        events.push(EngineEvent::msg("potion-enlightenment"));
        // Cure sickness.
        events.push(EngineEvent::StatusRemoved {
            entity: drinker,
            status: StatusEffect::Sick,
        });
    } else if buc.cursed {
        // Unholy water -- damages non-undead.
        let dmg = d(rng, 2, 6) as i32;
        events.extend(apply_damage(world, drinker, dmg));
        events.push(EngineEvent::msg("potion-acid"));
    } else {
        // Plain water.
        if let Some(mut nut) = world.get_component_mut::<Nutrition>(drinker) {
            let bonus = rnd(rng, 10) as i32;
            nut.0 += bonus;
        }
        events.push(EngineEvent::msg("potion-water"));
    }

    events
}

/// Acid potion.
///
/// Spec (section 3.24):
///   If Acid_resistance: no damage ("This tastes sour.")
///   Otherwise: dmg = d(cursed?2:1, blessed?4:8)
///     B: d(1,4)  = [1,4]
///     U: d(1,8)  = [1,8]
///     C: d(2,8)  = [2,16]
///   All BUC: if Stoned, cure petrification
fn effect_acid<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if has_acid_resistance(world, drinker) {
        // Acid resistant: no damage.
        events.push(EngineEvent::msg("potion-acid-resist"));
    } else {
        let ndice = if buc.cursed { 2u32 } else { 1 };
        let sides = if buc.blessed { 4u32 } else { 8 };
        let dmg = d(rng, ndice, sides) as i32;
        events.extend(apply_damage(world, drinker, dmg));
        events.push(EngineEvent::msg("potion-acid"));
    }

    // Cure stoning regardless.
    events.push(EngineEvent::StatusRemoved {
        entity: drinker,
        status: StatusEffect::Stoning,
    });

    events
}

/// Booze: confuse + nutrition.
fn effect_booze<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if !buc.blessed {
        // Confusion unless blessed.
        let duration = d(rng, 3, 8);
        events.push(EngineEvent::StatusApplied {
            entity: drinker,
            status: StatusEffect::Confused,
            duration: Some(duration),
            source: None,
        });
    }

    // Heal 1 HP.
    events.extend(apply_healing(world, drinker, 1));

    // Nutrition: 10 * (2 + bcsign).
    let bc = bcsign(buc);
    let nutrition_gain = 10 * (2 + bc);
    if let Some(mut nut) = world.get_component_mut::<Nutrition>(drinker) {
        nut.0 += nutrition_gain;
    }

    if buc.cursed {
        // Cursed: pass out.
        let duration = rnd(rng, 15);
        events.push(EngineEvent::StatusApplied {
            entity: drinker,
            status: StatusEffect::Sleeping,
            duration: Some(duration),
            source: None,
        });
        events.push(EngineEvent::msg("potion-booze-passout"));
    } else {
        events.push(EngineEvent::msg("potion-booze"));
    }

    events
}

/// Fruit juice: nutrition = 10 * (2 + bcsign).
///
/// Spec (section 3.7):
///   Nutrition: (odiluted ? 5 : 10) * (2 + bcsign)
///   B: 30, U: 20, C: 10 (not diluted)
///   (Dilution not tracked yet; using non-diluted values.)
fn effect_fruit_juice<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let bc = bcsign(buc);
    let nutrition_gain = 10 * (2 + bc);
    if let Some(mut nut) = world.get_component_mut::<Nutrition>(drinker) {
        nut.0 += nutrition_gain;
    }

    events.push(EngineEvent::msg(
        if buc.cursed {
            "potion-fruit-juice-rotten"
        } else {
            "potion-fruit-juice"
        },
    ));

    events
}

/// Oil potion (unlit).
///
/// Spec (section 3.23):
///   Lit oil: fire damage d(vulnerable?4:2, 4) -- not modeled here (no lit
///            potions in engine yet).
///   Unlit, cursed: "This tastes like castor oil." -- no mechanical effect
///   Unlit, non-cursed: "That was smooth!" -- no mechanical effect
fn effect_oil<R: Rng>(
    _world: &mut GameWorld,
    _drinker: Entity,
    buc: &BucStatus,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    events.push(EngineEvent::msg(
        if buc.cursed {
            "potion-oil-cursed"
        } else {
            "potion-oil"
        },
    ));

    events
}

/// Polymorph: polymorph self.
fn effect_polymorph<R: Rng>(
    _world: &mut GameWorld,
    drinker: Entity,
    _buc: &BucStatus,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    vec![
        EngineEvent::StatusApplied {
            entity: drinker,
            status: StatusEffect::Polymorphed,
            duration: None,
            source: None,
        },
        EngineEvent::msg("potion-polymorph"),
    ]
}

/// Levitation potion.
///
/// Spec (section 3.21 / 10.3):
///   Cursed:   timeout stays at 1 (can't voluntarily descend)
///   Uncursed: rn1(140, 10) = [10, 150) turns
///   Blessed:  rn1(50, 250) = [250, 300) turns (can descend via >)
fn effect_levitation<R: Rng>(
    _world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let duration = if buc.cursed {
        1u32
    } else if buc.blessed {
        rn1(rng, 50, 250).max(0) as u32
    } else {
        rn1(rng, 140, 10).max(0) as u32
    };

    events.push(EngineEvent::StatusApplied {
        entity: drinker,
        status: StatusEffect::Levitating,
        duration: Some(duration),
        source: None,
    });

    events.push(EngineEvent::msg("potion-levitation"));

    events
}

/// Enlightenment potion.
///
/// Spec (section 3.5):
///   Cursed: "You have an uneasy feeling..." -- exercise WIS (negative)
///   Uncursed: show enlightenment screen, exercise WIS
///   Blessed: +1 INT, +1 WIS, then show enlightenment screen
fn effect_enlightenment<R: Rng>(
    world: &mut GameWorld,
    drinker: Entity,
    buc: &BucStatus,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        events.push(EngineEvent::msg("potion-uneasy"));
        return events;
    }

    if buc.blessed {
        // Blessed: +1 INT, +1 WIS.
        if let Some(mut attrs) =
            world.get_component_mut::<Attributes>(drinker)
        {
            increase_attribute(&mut attrs, 3); // INT
            increase_attribute(&mut attrs, 4); // WIS
        }
    }

    events.push(EngineEvent::msg("potion-enlightenment"));

    events
}

// ---------------------------------------------------------------------------
// Throw potion
// ---------------------------------------------------------------------------

/// Throw a potion at a target position.
///
/// The potion shatters on impact.  Adjacent creatures within 1 square
/// are affected by the vapor.  Healing potions heal the target, acid
/// damages, etc.
pub fn throw_potion<R: Rng>(
    world: &mut GameWorld,
    thrower: Entity,
    potion_entity: Entity,
    potion_type: PotionType,
    target_pos: Position,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Read BUC status.
    let buc = match world.get_component::<BucStatus>(potion_entity) {
        Some(b) => (*b).clone(),
        None => BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        },
    };

    // Find entities at the target position and adjacent squares.
    let affected: Vec<Entity> = {
        let mut result = Vec::new();
        for (entity, pos) in world.query::<Positioned>().iter() {
            let dx = (pos.0.x - target_pos.x).abs();
            let dy = (pos.0.y - target_pos.y).abs();
            if dx <= 1 && dy <= 1 && entity != thrower {
                result.push(entity);
            }
        }
        result
    };

    // Potion shatters.
    events.push(EngineEvent::msg("potion-shatter"));

    // Apply vapor effects to affected entities.
    for target in &affected {
        match potion_type {
            PotionType::Healing | PotionType::ExtraHealing
            | PotionType::FullHealing => {
                let heal = match potion_type {
                    PotionType::Healing => d(rng, 6, 4) as i32 + 8,
                    PotionType::ExtraHealing => d(rng, 6, 8) as i32 + 8,
                    PotionType::FullHealing => {
                        // Heal to max for thrown full healing.
                        if let Some(hp) =
                            world.get_component::<HitPoints>(*target)
                        {
                            (hp.max - hp.current).max(0)
                        } else {
                            0
                        }
                    }
                    _ => unreachable!(),
                };
                events.extend(apply_healing(world, *target, heal));
            }
            PotionType::Acid => {
                let dmg = d(rng, 2, 6) as i32;
                events.extend(apply_damage(world, *target, dmg));
            }
            PotionType::Confusion => {
                let duration = d(rng, 3, 8);
                events.push(EngineEvent::StatusApplied {
                    entity: *target,
                    status: StatusEffect::Confused,
                    duration: Some(duration),
                    source: Some(thrower),
                });
            }
            PotionType::Blindness => {
                let duration = rn1(rng, 200, 250) as u32;
                events.push(EngineEvent::StatusApplied {
                    entity: *target,
                    status: StatusEffect::Blind,
                    duration: Some(duration),
                    source: Some(thrower),
                });
            }
            PotionType::Sleeping
                if !has_free_action(world, *target) =>
            {
                let duration = rn1(rng, 10, 25).max(0) as u32;
                events.push(EngineEvent::StatusApplied {
                    entity: *target,
                    status: StatusEffect::Sleeping,
                    duration: Some(duration),
                    source: Some(thrower),
                });
            }
            PotionType::Paralysis
                if !has_free_action(world, *target) =>
            {
                let duration = d(rng, 4, 6);
                events.push(EngineEvent::StatusApplied {
                    entity: *target,
                    status: StatusEffect::Paralyzed,
                    duration: Some(duration),
                    source: Some(thrower),
                });
            }
            PotionType::Speed => {
                events.push(EngineEvent::StatusApplied {
                    entity: *target,
                    status: StatusEffect::FastSpeed,
                    duration: Some(rn1(rng, 10, 100).max(0) as u32),
                    source: Some(thrower),
                });
            }
            PotionType::Invisibility => {
                events.push(EngineEvent::StatusApplied {
                    entity: *target,
                    status: StatusEffect::Invisible,
                    duration: Some(rn1(rng, 15, 31).max(0) as u32),
                    source: Some(thrower),
                });
            }
            PotionType::Polymorph => {
                events.push(EngineEvent::StatusApplied {
                    entity: *target,
                    status: StatusEffect::Polymorphed,
                    duration: None,
                    source: Some(thrower),
                });
            }
            PotionType::Sickness => {
                // Sickness halves HP (like C do_illness).
                if !has_poison_resistance(world, *target) {
                    let hp_current = world.get_component::<HitPoints>(*target)
                        .map(|hp| hp.current);
                    if let Some(cur) = hp_current {
                        if cur > 2 {
                            let dmg = cur - cur / 2;
                            events.extend(apply_damage(world, *target, dmg));
                        }
                    }
                }
            }
            PotionType::Booze => {
                // Booze causes confusion when thrown at monster (like C).
                let duration = d(rng, 3, 8);
                events.push(EngineEvent::StatusApplied {
                    entity: *target,
                    status: StatusEffect::Confused,
                    duration: Some(duration),
                    source: Some(thrower),
                });
            }
            PotionType::RestoreAbility | PotionType::GainAbility => {
                // These heal monsters when thrown (like C do_healing).
                let heal_amount = world.get_component::<HitPoints>(*target)
                    .map(|hp| (hp.max - hp.current).max(0));
                if let Some(heal) = heal_amount {
                    if heal > 0 {
                        events.extend(apply_healing(world, *target, heal));
                    }
                }
            }
            PotionType::Water
                if buc.blessed
                // Holy water damages undead.
                && world.get_component::<Monster>(*target).is_some() => {
                    let dmg = d(rng, 2, 6) as i32;
                    events.extend(apply_damage(world, *target, dmg));
                }
            _ => {
                // Other potion types have no splash effect.
            }
        }
    }

    // Consume the potion.
    let _ = world.despawn(potion_entity);
    events.push(EngineEvent::ItemDestroyed {
        item: potion_entity,
        cause: crate::event::DamageCause::Physical,
    });

    events
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Apply healing to an entity.  Clamps at max HP.
fn apply_healing(world: &mut GameWorld, entity: Entity, amount: i32) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
        let old = hp.current;
        hp.current = (hp.current + amount).min(hp.max);
        let actual = hp.current - old;
        if actual != 0 {
            events.push(EngineEvent::HpChange {
                entity,
                amount: actual,
                new_hp: hp.current,
                source: HpSource::Potion,
            });
        }
    }
    events
}

/// Apply damage to an entity.  May kill them.
fn apply_damage(
    world: &mut GameWorld,
    entity: Entity,
    amount: i32,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
        hp.current -= amount;
        events.push(EngineEvent::HpChange {
            entity,
            amount: -amount,
            new_hp: hp.current,
            source: HpSource::Potion,
        });
        if hp.current <= 0 {
            events.push(EngineEvent::EntityDied {
                entity,
                killer: None,
                cause: DeathCause::Poisoning,
            });
        }
    }
    events
}

/// Check whether an entity has Free_action.
///
/// Checks the `FreeAction` marker component, which is granted
/// automatically by `worn::recalc_worn_intrinsics()` when a ring of
/// free action is equipped.
fn has_free_action(world: &GameWorld, entity: Entity) -> bool {
    world.get_component::<FreeAction>(entity).is_some()
}

/// Increase one of the six attributes by 1 (capped at 25).
fn increase_attribute(attrs: &mut Attributes, index: u32) {
    match index {
        0 => attrs.strength = (attrs.strength + 1).min(25),
        1 => attrs.dexterity = (attrs.dexterity + 1).min(25),
        2 => attrs.constitution = (attrs.constitution + 1).min(25),
        3 => attrs.intelligence = (attrs.intelligence + 1).min(25),
        4 => attrs.wisdom = (attrs.wisdom + 1).min(25),
        5 => attrs.charisma = (attrs.charisma + 1).min(25),
        _ => {}
    }
}

/// Decrease one of the six attributes by 1 (floored at 3).
fn decrease_attribute(attrs: &mut Attributes, index: u32) {
    match index {
        0 => attrs.strength = attrs.strength.saturating_sub(1).max(3),
        1 => attrs.dexterity = attrs.dexterity.saturating_sub(1).max(3),
        2 => attrs.constitution = attrs.constitution.saturating_sub(1).max(3),
        3 => attrs.intelligence = attrs.intelligence.saturating_sub(1).max(3),
        4 => attrs.wisdom = attrs.wisdom.saturating_sub(1).max(3),
        5 => attrs.charisma = attrs.charisma.saturating_sub(1).max(3),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Potion splash effect descriptor
// ---------------------------------------------------------------------------

/// Describes the effect of a potion shattering on or near a creature.
///
/// This is a lightweight descriptor used by AI and UI code to decide
/// whether to throw a given potion type.  The actual application is
/// handled by `throw_potion()`.
#[derive(Debug, Clone, PartialEq)]
pub enum PotionSplashEffect {
    /// Acid damage to the target.
    AcidDamage,
    /// Holy water damages undead monsters.
    HolyDamage,
    /// Puts the target to sleep.
    Sleep,
    /// Paralyzes the target.
    Paralyze,
    /// Blinds the target.
    Blind,
    /// Confuses the target.
    Confuse,
    /// Causes hallucination.
    Hallucinate,
    /// Polymorphs the target.
    Polymorph,
    /// Heals the target.
    Heal,
    /// Speeds up the target.
    Speed,
    /// Makes the target invisible.
    MakeInvisible,
    /// Sickens the target (halves HP).
    Sicken,
    /// No splash effect.
    NoEffect,
}

/// Determine the splash effect category for a thrown potion.
///
/// This is a pure classification function -- it does not apply any effect.
/// Use `throw_potion()` for actual application with damage/duration rolls.
pub fn potion_splash_category(
    potion_type: PotionType,
    is_blessed_water: bool,
) -> PotionSplashEffect {
    match potion_type {
        PotionType::Acid => PotionSplashEffect::AcidDamage,
        PotionType::Water if is_blessed_water => PotionSplashEffect::HolyDamage,
        PotionType::Sleeping => PotionSplashEffect::Sleep,
        PotionType::Paralysis => PotionSplashEffect::Paralyze,
        PotionType::Blindness => PotionSplashEffect::Blind,
        PotionType::Confusion | PotionType::Booze => PotionSplashEffect::Confuse,
        PotionType::Hallucination => PotionSplashEffect::Hallucinate,
        PotionType::Polymorph => PotionSplashEffect::Polymorph,
        PotionType::Healing | PotionType::ExtraHealing
        | PotionType::FullHealing => PotionSplashEffect::Heal,
        PotionType::Speed => PotionSplashEffect::Speed,
        PotionType::Invisibility => PotionSplashEffect::MakeInvisible,
        PotionType::Sickness => PotionSplashEffect::Sicken,
        PotionType::RestoreAbility | PotionType::GainAbility => {
            PotionSplashEffect::Heal
        }
        _ => PotionSplashEffect::NoEffect,
    }
}

// ---------------------------------------------------------------------------
// Marker components for resistances / intrinsics
// ---------------------------------------------------------------------------

/// Marker component indicating an entity has the Free_action property.
/// In a full implementation this would be part of the property system.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct FreeAction;

/// Marker component indicating an entity has acid resistance.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct AcidResistance;

/// Marker component indicating an entity has poison resistance.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct PoisonResistance;

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use nethack_babel_data::ObjectClass;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    fn make_rng() -> Pcg64 {
        Pcg64::seed_from_u64(42)
    }

    fn make_world() -> GameWorld {
        GameWorld::new(Position::new(40, 10))
    }

    fn uncursed() -> BucStatus {
        BucStatus {
            cursed: false,
            blessed: false,
            bknown: true,
        }
    }

    fn blessed() -> BucStatus {
        BucStatus {
            cursed: false,
            blessed: true,
            bknown: true,
        }
    }

    fn cursed() -> BucStatus {
        BucStatus {
            cursed: true,
            blessed: false,
            bknown: true,
        }
    }

    fn spawn_potion(world: &mut GameWorld, buc: BucStatus) -> Entity {
        world.spawn((
            ObjectCore {
                otyp: nethack_babel_data::ObjectTypeId(100),
                object_class: ObjectClass::Potion,
                quantity: 1,
                weight: 20,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            buc,
        ))
    }

    // ── Test: bcsign ─────────────────────────────────────────────

    #[test]
    fn bcsign_values() {
        assert_eq!(bcsign(&blessed()), 1);
        assert_eq!(bcsign(&uncursed()), 0);
        assert_eq!(bcsign(&cursed()), -1);
    }

    // ── Test: Healing potion restores HP ─────────────────────────

    #[test]
    fn healing_potion_restores_hp_by_buc() {
        let mut rng = make_rng();

        // Damage the player and test healing for each BUC.
        for (buc_status, label) in
            [(&blessed(), "blessed"), (&uncursed(), "uncursed"), (&cursed(), "cursed")]
        {
            let mut world = make_world();
            let player = world.player();

            // Damage the player.
            {
                let mut hp = world.get_component_mut::<HitPoints>(player).unwrap();
                hp.current = 1;
            }

            let potion = spawn_potion(&mut world, buc_status.clone());
            let events = quaff_potion(
                &mut world,
                player,
                potion,
                PotionType::Healing,
                &mut rng,
            );

            // Player should have more HP now.
            let hp = world.get_component::<HitPoints>(player).unwrap();
            assert!(
                hp.current > 1,
                "{} healing potion should restore HP (got {})",
                label,
                hp.current
            );

            // Should have HpChange event.
            let has_hp_change = events
                .iter()
                .any(|e| matches!(e, EngineEvent::HpChange { .. }));
            assert!(has_hp_change, "{} healing should emit HpChange", label);
        }
    }

    // ── Test: Full healing raises max HP when blessed ─────────────

    #[test]
    fn full_healing_raises_max_hp_when_blessed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let original_max = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.max
        };

        let potion = spawn_potion(&mut world, blessed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::FullHealing,
            &mut rng,
        );

        let hp = world.get_component::<HitPoints>(player).unwrap();
        assert!(
            hp.max > original_max,
            "blessed full healing should increase max HP: {} vs {}",
            hp.max,
            original_max
        );
        assert_eq!(hp.current, hp.max, "full healing should set current to max");
    }

    // ── Test: Speed grants timed Fast property ───────────────────

    #[test]
    fn speed_grants_timed_fast_property() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Speed,
            &mut rng,
        );

        let has_fast = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::FastSpeed,
                duration: Some(d),
                ..
            } if *d > 0
        ));
        assert!(has_fast, "speed potion should apply timed FastSpeed");
    }

    // ── Test: Gain level increases player level ──────────────────

    #[test]
    fn gain_level_increases_player_level() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let before = {
            let xlvl = world.get_component::<ExperienceLevel>(player).unwrap();
            xlvl.0
        };

        let potion = spawn_potion(&mut world, uncursed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::GainLevel,
            &mut rng,
        );

        let after = {
            let xlvl = world.get_component::<ExperienceLevel>(player).unwrap();
            xlvl.0
        };

        assert_eq!(after, before + 1, "gain level should increase XL by 1");
    }

    // ── Test: Cursed gain level loses a level ────────────────────

    #[test]
    fn cursed_gain_level_decreases_level() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Start at level 5.
        {
            let mut xlvl = world.get_component_mut::<ExperienceLevel>(player).unwrap();
            xlvl.0 = 5;
        }

        let potion = spawn_potion(&mut world, cursed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::GainLevel,
            &mut rng,
        );

        let after = {
            let xlvl = world.get_component::<ExperienceLevel>(player).unwrap();
            xlvl.0
        };

        assert_eq!(after, 4, "cursed gain level should decrease XL to 4");
    }

    // ── Test: Cursed sickness deals HP damage ─────────────────────

    #[test]
    fn sickness_cursed_deals_hp_damage() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let hp_before = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.current
        };

        let potion = spawn_potion(&mut world, cursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Sickness,
            &mut rng,
        );

        // Cursed sickness should deal HP damage (rnd(10)+5).
        let has_damage = events.iter().any(|e| matches!(
            e,
            EngineEvent::HpChange { amount, .. } if *amount < 0
        ));
        assert!(has_damage, "cursed sickness should deal HP damage");

        let hp_after = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.current
        };
        assert!(
            hp_after < hp_before,
            "HP should decrease from {} to {}", hp_before, hp_after
        );
    }

    // ── Test: Holy water message ─────────────────────────────────

    #[test]
    fn holy_water_cures_sickness() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, blessed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Water,
            &mut rng,
        );

        let has_cure = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Sick,
                ..
            }
        ));
        assert!(has_cure, "holy water should cure sickness");

        let has_awe = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("enlightenment")
        ));
        assert!(has_awe, "holy water should produce awe message");
    }

    // ── Test: Confusion duration varies by BUC ───────────────────

    #[test]
    fn confusion_duration_varies_by_buc() {
        let mut rng_b = make_rng();
        let mut rng_c = make_rng();

        // Blessed confusion.
        let mut world_b = make_world();
        let player_b = world_b.player();
        let potion_b = spawn_potion(&mut world_b, blessed());
        let events_b = quaff_potion(
            &mut world_b,
            player_b,
            potion_b,
            PotionType::Confusion,
            &mut rng_b,
        );

        let dur_blessed = events_b.iter().find_map(|e| match e {
            EngineEvent::StatusApplied {
                status: StatusEffect::Confused,
                duration: Some(d),
                ..
            } => Some(*d),
            _ => None,
        });

        // Cursed confusion.
        let mut world_c = make_world();
        let player_c = world_c.player();
        let potion_c = spawn_potion(&mut world_c, cursed());
        let events_c = quaff_potion(
            &mut world_c,
            player_c,
            potion_c,
            PotionType::Confusion,
            &mut rng_c,
        );

        let dur_cursed = events_c.iter().find_map(|e| match e {
            EngineEvent::StatusApplied {
                status: StatusEffect::Confused,
                duration: Some(d),
                ..
            } => Some(*d),
            _ => None,
        });

        assert!(
            dur_blessed.is_some() && dur_cursed.is_some(),
            "both should produce confusion"
        );
        // Blessed uses d(4,1) [4..4], cursed uses d(4,8) [4..32].
        // With the same seed, blessed max is 4 while cursed can go much higher.
        assert!(
            dur_blessed.unwrap() <= dur_cursed.unwrap(),
            "blessed confusion ({}) should be <= cursed ({})",
            dur_blessed.unwrap(),
            dur_cursed.unwrap()
        );
    }

    // ── Test: Paralysis blocked by Free_action ───────────────────

    #[test]
    fn paralysis_blocked_by_free_action() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Grant Free_action to the player.
        let _ = world.ecs_mut().insert_one(player, FreeAction);

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Paralysis,
            &mut rng,
        );

        let has_paralysis = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Paralyzed,
                ..
            }
        ));
        assert!(
            !has_paralysis,
            "paralysis should be blocked by Free_action"
        );

        let has_resist_msg = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("paralysis")
        ));
        assert!(has_resist_msg, "should get resistance message");
    }

    // ── Test: Blessed gain ability boosts all stats ───────────────

    #[test]
    fn blessed_gain_ability_boosts_all_stats() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let before = {
            let attrs = world.get_component::<Attributes>(player).unwrap();
            (
                attrs.strength,
                attrs.dexterity,
                attrs.constitution,
                attrs.intelligence,
                attrs.wisdom,
                attrs.charisma,
            )
        };

        let potion = spawn_potion(&mut world, blessed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::GainAbility,
            &mut rng,
        );

        let after = {
            let attrs = world.get_component::<Attributes>(player).unwrap();
            (
                attrs.strength,
                attrs.dexterity,
                attrs.constitution,
                attrs.intelligence,
                attrs.wisdom,
                attrs.charisma,
            )
        };

        assert_eq!(after.0, before.0 + 1, "STR should increase by 1");
        assert_eq!(after.1, before.1 + 1, "DEX should increase by 1");
        assert_eq!(after.2, before.2 + 1, "CON should increase by 1");
        assert_eq!(after.3, before.3 + 1, "INT should increase by 1");
        assert_eq!(after.4, before.4 + 1, "WIS should increase by 1");
        assert_eq!(after.5, before.5 + 1, "CHA should increase by 1");
    }

    // ── Test: Monster detection reveals all monsters ─────────────

    #[test]
    fn monster_detection_reveals_all_monsters() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Spawn some monsters.
        let _m1 = world.spawn((
            Monster,
            Positioned(Position::new(10, 10)),
        ));
        let _m2 = world.spawn((
            Monster,
            Positioned(Position::new(20, 20)),
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::MonsterDetection,
            &mut rng,
        );

        let detection_count = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::MonsterGenerated { .. }))
            .count();

        assert_eq!(
            detection_count, 2,
            "monster detection should reveal all 2 monsters"
        );
    }

    // ── Test: Sleeping blocked by Free_action ────────────────────

    #[test]
    fn sleeping_blocked_by_free_action() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let _ = world.ecs_mut().insert_one(player, FreeAction);

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Sleeping,
            &mut rng,
        );

        let has_sleep = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Sleeping,
                ..
            }
        ));
        assert!(!has_sleep, "sleeping should be blocked by Free_action");
    }

    // ── Test: Gain energy restores and increases max Pw ───────────

    #[test]
    fn gain_energy_restores_pw() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Drain player power.
        {
            let mut pw = world.get_component_mut::<Power>(player).unwrap();
            pw.current = 0;
        }

        let potion = spawn_potion(&mut world, uncursed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::GainEnergy,
            &mut rng,
        );

        let pw = world.get_component::<Power>(player).unwrap();
        assert!(pw.current > 0, "gain energy should restore Pw");
        assert!(pw.max > 4, "gain energy should increase max Pw");
    }

    // ── Test: Acid cures stoning ─────────────────────────────────

    #[test]
    fn acid_cures_stoning() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Acid,
            &mut rng,
        );

        let has_cure = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Stoning,
                ..
            }
        ));
        assert!(has_cure, "acid potion should emit Stoning removal");
    }

    // ── Test: Throw potion shatters and affects target ────────────

    #[test]
    fn throw_potion_shatters_and_heals_target() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Spawn a monster at a nearby position.
        let monster = world.spawn((
            Monster,
            Positioned(Position::new(41, 10)),
            HitPoints { current: 5, max: 20 },
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::Healing,
            Position::new(41, 10),
            &mut rng,
        );

        // Monster should be healed.
        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert!(hp.current > 5, "thrown healing potion should heal target");

        // Potion should be destroyed.
        let has_destroyed = events.iter().any(|e| matches!(
            e,
            EngineEvent::ItemDestroyed { .. }
        ));
        assert!(has_destroyed, "thrown potion should be destroyed");
    }

    // ── Test: Levitation duration varies by BUC ──────────────────

    #[test]
    fn levitation_applies_with_duration() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Levitation,
            &mut rng,
        );

        let has_lev = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Levitating,
                duration: Some(d),
                ..
            } if *d > 0
        ));
        assert!(has_lev, "levitation should apply timed Levitating status");
    }

    // ===================================================================
    // Track F.1 alignment tests -- spec edge cases and test vectors
    // ===================================================================

    // ── (a) Cursed gain level at level 1 emits uneasy message ────────

    #[test]
    fn test_potion_gain_level_cursed_at_level_1() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Player starts at level 1.
        let before = {
            let xlvl = world.get_component::<ExperienceLevel>(player).unwrap();
            xlvl.0
        };
        assert_eq!(before, 1);

        let potion = spawn_potion(&mut world, cursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::GainLevel,
            &mut rng,
        );

        // Level should not change (can't go below 1).
        let after = {
            let xlvl = world.get_component::<ExperienceLevel>(player).unwrap();
            xlvl.0
        };
        assert_eq!(after, 1, "level should stay at 1");

        // Should emit uneasy message.
        let has_uneasy = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("uneasy")
        ));
        assert!(has_uneasy, "should emit uneasy message at level 1");
    }

    // ── (a) Cursed gain level loses a level from higher level ────────

    #[test]
    fn test_potion_gain_level_cursed_loses_level() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Set player to level 10.
        {
            let mut xlvl =
                world.get_component_mut::<ExperienceLevel>(player).unwrap();
            xlvl.0 = 10;
        }

        let potion = spawn_potion(&mut world, cursed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::GainLevel,
            &mut rng,
        );

        let after = {
            let xlvl = world.get_component::<ExperienceLevel>(player).unwrap();
            xlvl.0
        };
        assert_eq!(after, 9, "cursed gain level should decrease XL from 10 to 9");
    }

    // ── (b) See invisible: cursed gives no effect ─────────────────────

    #[test]
    fn test_potion_see_invisible_cursed_no_effect() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, cursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::SeeInvisible,
            &mut rng,
        );

        let has_see_invis = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::SeeInvisible,
                ..
            }
        ));
        assert!(
            !has_see_invis,
            "cursed see invisible should NOT grant see invisible"
        );
    }

    // ── (b) See invisible: blessed grants permanent ───────────────────

    #[test]
    fn test_potion_see_invisible_blessed_permanent() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, blessed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::SeeInvisible,
            &mut rng,
        );

        let see_invis = events.iter().find_map(|e| match e {
            EngineEvent::StatusApplied {
                status: StatusEffect::SeeInvisible,
                duration,
                ..
            } => Some(*duration),
            _ => None,
        });
        assert_eq!(
            see_invis,
            Some(None),
            "blessed see invisible should grant permanent (None duration)"
        );

        // Should also cure blindness.
        let has_blind_cure = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Blind,
                ..
            }
        ));
        assert!(
            has_blind_cure,
            "blessed see invisible should cure blindness"
        );
    }

    // ── (b) See invisible: uncursed grants temporary ──────────────────

    #[test]
    fn test_potion_see_invisible_uncursed_temporary() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::SeeInvisible,
            &mut rng,
        );

        let see_invis = events.iter().find_map(|e| match e {
            EngineEvent::StatusApplied {
                status: StatusEffect::SeeInvisible,
                duration: Some(d),
                ..
            } => Some(*d),
            _ => None,
        });
        // Spec: rn1(100, 750) = [750, 850)
        assert!(
            see_invis.is_some(),
            "uncursed see invisible should grant timed see invisible"
        );
        let dur = see_invis.unwrap();
        assert!(
            dur >= 750 && dur < 850,
            "duration {} should be in [750, 850)",
            dur
        );
    }

    // ── (c) Acid potion with acid resistance: no damage ───────────────

    #[test]
    fn test_potion_acid_resistant_no_damage() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Grant acid resistance.
        let _ = world.ecs_mut().insert_one(player, AcidResistance);

        let hp_before = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.current
        };

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Acid,
            &mut rng,
        );

        let hp_after = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.current
        };

        assert_eq!(
            hp_after, hp_before,
            "acid resistant hero should take no damage"
        );

        // Should still cure stoning.
        let has_cure = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Stoning,
                ..
            }
        ));
        assert!(has_cure, "acid should still cure stoning even if resistant");

        // Should emit resist message.
        let has_resist = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("resist")
        ));
        assert!(has_resist, "should emit acid resist message");
    }

    // ── (c) Acid potion BUC damage formulas ───────────────────────────

    #[test]
    fn test_potion_acid_blessed_less_damage() {
        // Blessed acid: d(1, 4) = [1, 4]
        // Cursed acid: d(2, 8) = [2, 16]
        // Run multiple times to verify the range difference.
        for seed in 42..52 {
            let mut world_b = make_world();
            let mut rng_b = Pcg64::seed_from_u64(seed);
            let player_b = world_b.player();
            {
                let mut hp =
                    world_b.get_component_mut::<HitPoints>(player_b).unwrap();
                hp.current = 100;
                hp.max = 100;
            }
            let potion_b = spawn_potion(&mut world_b, blessed());
            let _events_b = quaff_potion(
                &mut world_b,
                player_b,
                potion_b,
                PotionType::Acid,
                &mut rng_b,
            );
            let hp_b = world_b
                .get_component::<HitPoints>(player_b)
                .unwrap()
                .current;
            let dmg_b = 100 - hp_b;
            // Blessed: d(1,4) -> damage in [1, 4]
            assert!(
                dmg_b >= 1 && dmg_b <= 4,
                "blessed acid damage {} should be in [1,4]",
                dmg_b
            );
        }
    }

    // ── Healing: max HP increase for non-cursed ───────────────────────

    #[test]
    fn test_potion_healing_blessed_full_hp() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Set player HP to full (16/16).
        let original_max = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.max
        };

        let potion = spawn_potion(&mut world, blessed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Healing,
            &mut rng,
        );

        let hp = world.get_component::<HitPoints>(player).unwrap();
        // Blessed: max HP should increase by 1 when healed amount exceeds
        // max (which it does since player is at max already + large heal).
        assert!(
            hp.max > original_max,
            "blessed healing should increase max HP: got {} vs original {}",
            hp.max,
            original_max
        );
    }

    // ── Healing: cursed does not increase max HP ──────────────────────

    #[test]
    fn test_potion_healing_cursed_no_max_increase() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let original_max = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.max
        };

        let potion = spawn_potion(&mut world, cursed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Healing,
            &mut rng,
        );

        let hp = world.get_component::<HitPoints>(player).unwrap();
        assert_eq!(
            hp.max, original_max,
            "cursed healing should NOT increase max HP"
        );
    }

    // ── Healing: uncursed cures blindness, cursed does not ────────────

    #[test]
    fn test_potion_healing_uncursed_cures_blindness() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Healing,
            &mut rng,
        );

        let cures_blind = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Blind,
                ..
            }
        ));
        assert!(cures_blind, "uncursed healing should cure blindness");
    }

    #[test]
    fn test_potion_healing_cursed_no_blindness_cure() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, cursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Healing,
            &mut rng,
        );

        let cures_blind = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Blind,
                ..
            }
        ));
        assert!(
            !cures_blind,
            "cursed healing should NOT cure blindness"
        );
    }

    // ── Extra healing: cures hallucination ────────────────────────────

    #[test]
    fn test_potion_extra_healing_cures_hallucination() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::ExtraHealing,
            &mut rng,
        );

        let cures_halluc = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusRemoved {
                status: StatusEffect::Hallucinating,
                ..
            }
        ));
        assert!(
            cures_halluc,
            "extra healing should always cure hallucination"
        );
    }

    // ── Extra healing: max HP increase varies by BUC ──────────────────

    #[test]
    fn test_potion_extra_healing_max_hp_increase() {
        // Blessed: +5 max HP, Uncursed: +2, Cursed: +0
        for (buc_status, expected_increase, label) in [
            (blessed(), 5, "blessed"),
            (uncursed(), 2, "uncursed"),
            (cursed(), 0, "cursed"),
        ] {
            let mut world = make_world();
            let mut rng = make_rng();
            let player = world.player();

            let original_max = {
                let hp = world.get_component::<HitPoints>(player).unwrap();
                hp.max
            };

            let potion = spawn_potion(&mut world, buc_status);
            let _events = quaff_potion(
                &mut world,
                player,
                potion,
                PotionType::ExtraHealing,
                &mut rng,
            );

            let hp = world.get_component::<HitPoints>(player).unwrap();
            assert_eq!(
                hp.max,
                original_max + expected_increase,
                "{} extra healing max HP increase should be {}",
                label,
                expected_increase
            );
        }
    }

    // ── Full healing: max HP increase varies by BUC ───────────────────

    #[test]
    fn test_potion_full_healing_max_hp_by_buc() {
        for (buc_status, expected_increase, label) in [
            (blessed(), 8, "blessed"),
            (uncursed(), 4, "uncursed"),
            (cursed(), 0, "cursed"),
        ] {
            let mut world = make_world();
            let mut rng = make_rng();
            let player = world.player();

            let original_max = {
                let hp = world.get_component::<HitPoints>(player).unwrap();
                hp.max
            };

            let potion = spawn_potion(&mut world, buc_status);
            let _events = quaff_potion(
                &mut world,
                player,
                potion,
                PotionType::FullHealing,
                &mut rng,
            );

            let hp = world.get_component::<HitPoints>(player).unwrap();
            assert_eq!(
                hp.max,
                original_max + expected_increase,
                "{} full healing max HP increase should be {}",
                label,
                expected_increase
            );
        }
    }

    // ── Full healing blessed: restores a lost level ───────────────────

    #[test]
    fn test_potion_full_healing_blessed_restores_level() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Set player to level 5.
        {
            let mut xlvl =
                world.get_component_mut::<ExperienceLevel>(player).unwrap();
            xlvl.0 = 5;
        }

        let potion = spawn_potion(&mut world, blessed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::FullHealing,
            &mut rng,
        );

        let after = {
            let xlvl = world.get_component::<ExperienceLevel>(player).unwrap();
            xlvl.0
        };
        assert_eq!(after, 6, "blessed full healing should restore a level");

        let has_levelup = events.iter().any(|e| matches!(
            e,
            EngineEvent::LevelUp { new_level: 6, .. }
        ));
        assert!(has_levelup, "should emit LevelUp event");
    }

    // ── Gain energy: cursed depletes ──────────────────────────────────

    #[test]
    fn test_potion_gain_energy_cursed_depletes() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Set initial power.
        {
            let mut pw = world.get_component_mut::<Power>(player).unwrap();
            pw.current = 3;
            pw.max = 5;
        }

        let potion = spawn_potion(&mut world, cursed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::GainEnergy,
            &mut rng,
        );

        let pw = world.get_component::<Power>(player).unwrap();
        // Cursed: num = d(1,6) then negated -> max decreases, current decreases
        assert!(pw.max < 5, "cursed gain energy should decrease max Pw");
        assert!(
            pw.current >= 0,
            "current Pw should be clamped to >= 0"
        );
    }

    // ── Confusion: duration matches spec ranges ───────────────────────

    #[test]
    fn test_potion_confusion_blessed_duration_range() {
        // Spec: blessed rn1(7, 8) = [8, 15)
        for seed in 42..52 {
            let mut world = make_world();
            let mut rng = Pcg64::seed_from_u64(seed);
            let player = world.player();

            let potion = spawn_potion(&mut world, blessed());
            let events = quaff_potion(
                &mut world,
                player,
                potion,
                PotionType::Confusion,
                &mut rng,
            );

            let dur = events.iter().find_map(|e| match e {
                EngineEvent::StatusApplied {
                    status: StatusEffect::Confused,
                    duration: Some(d),
                    ..
                } => Some(*d),
                _ => None,
            });
            assert!(dur.is_some(), "should apply confusion");
            let d = dur.unwrap();
            assert!(
                d >= 8 && d < 15,
                "blessed confusion duration {} should be in [8, 15)",
                d
            );
        }
    }

    #[test]
    fn test_potion_confusion_cursed_duration_range() {
        // Spec: cursed rn1(7, 24) = [24, 31)
        for seed in 42..52 {
            let mut world = make_world();
            let mut rng = Pcg64::seed_from_u64(seed);
            let player = world.player();

            let potion = spawn_potion(&mut world, cursed());
            let events = quaff_potion(
                &mut world,
                player,
                potion,
                PotionType::Confusion,
                &mut rng,
            );

            let dur = events.iter().find_map(|e| match e {
                EngineEvent::StatusApplied {
                    status: StatusEffect::Confused,
                    duration: Some(d),
                    ..
                } => Some(*d),
                _ => None,
            });
            assert!(dur.is_some(), "should apply confusion");
            let d = dur.unwrap();
            assert!(
                d >= 24 && d < 31,
                "cursed confusion duration {} should be in [24, 31)",
                d
            );
        }
    }

    // ── Paralysis: duration matches spec ranges ───────────────────────

    #[test]
    fn test_potion_paralysis_cursed_duration_range() {
        // Spec: cursed rn1(10, 37) = [37, 47)
        for seed in 42..52 {
            let mut world = make_world();
            let mut rng = Pcg64::seed_from_u64(seed);
            let player = world.player();

            let potion = spawn_potion(&mut world, cursed());
            let events = quaff_potion(
                &mut world,
                player,
                potion,
                PotionType::Paralysis,
                &mut rng,
            );

            let dur = events.iter().find_map(|e| match e {
                EngineEvent::StatusApplied {
                    status: StatusEffect::Paralyzed,
                    duration: Some(d),
                    ..
                } => Some(*d),
                _ => None,
            });
            assert!(dur.is_some(), "should apply paralysis");
            let d = dur.unwrap();
            assert!(
                d >= 37 && d < 47,
                "cursed paralysis duration {} should be in [37, 47)",
                d
            );
        }
    }

    // ── Levitation: cursed duration is 1, blessed is high ─────────────

    #[test]
    fn test_potion_levitation_cursed_duration_1() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, cursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Levitation,
            &mut rng,
        );

        let dur = events.iter().find_map(|e| match e {
            EngineEvent::StatusApplied {
                status: StatusEffect::Levitating,
                duration: Some(d),
                ..
            } => Some(*d),
            _ => None,
        });
        assert_eq!(dur, Some(1), "cursed levitation should have duration 1");
    }

    #[test]
    fn test_potion_levitation_blessed_duration_range() {
        // Spec: blessed rn1(50, 250) = [250, 300)
        for seed in 42..52 {
            let mut world = make_world();
            let mut rng = Pcg64::seed_from_u64(seed);
            let player = world.player();

            let potion = spawn_potion(&mut world, blessed());
            let events = quaff_potion(
                &mut world,
                player,
                potion,
                PotionType::Levitation,
                &mut rng,
            );

            let dur = events.iter().find_map(|e| match e {
                EngineEvent::StatusApplied {
                    status: StatusEffect::Levitating,
                    duration: Some(d),
                    ..
                } => Some(*d),
                _ => None,
            });
            assert!(dur.is_some(), "should apply levitation");
            let d = dur.unwrap();
            assert!(
                d >= 250 && d < 300,
                "blessed levitation duration {} should be in [250, 300)",
                d
            );
        }
    }

    // ── Enlightenment: cursed gives uneasy, blessed raises INT/WIS ───

    #[test]
    fn test_potion_enlightenment_cursed_no_info() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, cursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Enlightenment,
            &mut rng,
        );

        let has_uneasy = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("uneasy")
        ));
        assert!(has_uneasy, "cursed enlightenment should emit uneasy message");

        let has_enlighten = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("enlightenment")
        ));
        assert!(
            !has_enlighten,
            "cursed enlightenment should NOT emit enlightenment message"
        );
    }

    #[test]
    fn test_potion_enlightenment_blessed_raises_int_wis() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let (int_before, wis_before) = {
            let attrs = world.get_component::<Attributes>(player).unwrap();
            (attrs.intelligence, attrs.wisdom)
        };

        let potion = spawn_potion(&mut world, blessed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Enlightenment,
            &mut rng,
        );

        let (int_after, wis_after) = {
            let attrs = world.get_component::<Attributes>(player).unwrap();
            (attrs.intelligence, attrs.wisdom)
        };

        assert_eq!(
            int_after,
            int_before + 1,
            "blessed enlightenment should raise INT by 1"
        );
        assert_eq!(
            wis_after,
            wis_before + 1,
            "blessed enlightenment should raise WIS by 1"
        );
    }

    // ── Sickness: blessed only loses 1 HP ─────────────────────────────

    #[test]
    fn test_potion_sickness_blessed_loses_1_hp() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let hp_before = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.current
        };

        let potion = spawn_potion(&mut world, blessed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Sickness,
            &mut rng,
        );

        let hp_after = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.current
        };

        assert_eq!(
            hp_after,
            hp_before - 1,
            "blessed sickness should lose exactly 1 HP"
        );
    }

    // ── Sickness: all BUC cures hallucination ─────────────────────────

    #[test]
    fn test_potion_sickness_cures_hallucination() {
        for (buc_status, label) in [
            (blessed(), "blessed"),
            (uncursed(), "uncursed"),
            (cursed(), "cursed"),
        ] {
            let mut world = make_world();
            let mut rng = make_rng();
            let player = world.player();

            let potion = spawn_potion(&mut world, buc_status);
            let events = quaff_potion(
                &mut world,
                player,
                potion,
                PotionType::Sickness,
                &mut rng,
            );

            let cures_halluc = events.iter().any(|e| matches!(
                e,
                EngineEvent::StatusRemoved {
                    status: StatusEffect::Hallucinating,
                    ..
                }
            ));
            assert!(
                cures_halluc,
                "{} sickness should cure hallucination",
                label
            );
        }
    }

    // ── Sickness: poison resistant reduces damage ─────────────────────

    #[test]
    fn test_potion_sickness_poison_resistant() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Grant poison resistance.
        let _ = world.ecs_mut().insert_one(player, PoisonResistance);

        let hp_before = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.current
        };

        let potion = spawn_potion(&mut world, uncursed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Sickness,
            &mut rng,
        );

        let hp_after = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.current
        };

        // Poison resistant: lose 1+rn2(2) = 1 or 2 HP.
        let damage = hp_before - hp_after;
        assert!(
            damage >= 1 && damage <= 2,
            "poison resistant sickness damage {} should be in [1, 2]",
            damage
        );
    }

    // ── Invisibility: cursed aggravates ───────────────────────────────

    #[test]
    fn test_potion_invisibility_cursed_aggravates() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, cursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Invisibility,
            &mut rng,
        );

        let has_aggravate = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Aggravate,
                ..
            }
        ));
        assert!(
            has_aggravate,
            "cursed invisibility should aggravate monsters"
        );
    }

    // ── Oil potion: no mechanical effect (unlit) ──────────────────────

    #[test]
    fn test_potion_oil_no_damage() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let hp_before = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.current
        };

        let potion = spawn_potion(&mut world, uncursed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Oil,
            &mut rng,
        );

        let hp_after = {
            let hp = world.get_component::<HitPoints>(player).unwrap();
            hp.current
        };
        assert_eq!(
            hp_after, hp_before,
            "unlit oil should cause no HP change"
        );
    }

    // ── Fruit juice: nutrition varies by BUC ──────────────────────────

    #[test]
    fn test_potion_fruit_juice_nutrition_by_buc() {
        for (buc_status, expected_gain, label) in [
            (blessed(), 30, "blessed"),
            (uncursed(), 20, "uncursed"),
            (cursed(), 10, "cursed"),
        ] {
            let mut world = make_world();
            let mut rng = make_rng();
            let player = world.player();

            let nut_before = {
                let n = world.get_component::<Nutrition>(player).unwrap();
                n.0
            };

            let potion = spawn_potion(&mut world, buc_status);
            let _events = quaff_potion(
                &mut world,
                player,
                potion,
                PotionType::FruitJuice,
                &mut rng,
            );

            let nut_after = {
                let n = world.get_component::<Nutrition>(player).unwrap();
                n.0
            };
            assert_eq!(
                nut_after - nut_before,
                expected_gain,
                "{} fruit juice should give {} nutrition",
                label,
                expected_gain
            );
        }
    }

    // ── Blessed gain level only gains ONE level (not two) ─────────────

    #[test]
    fn test_potion_gain_level_blessed_one_level() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        {
            let mut xlvl =
                world.get_component_mut::<ExperienceLevel>(player).unwrap();
            xlvl.0 = 5;
        }

        let potion = spawn_potion(&mut world, blessed());
        let _events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::GainLevel,
            &mut rng,
        );

        let after = {
            let xlvl = world.get_component::<ExperienceLevel>(player).unwrap();
            xlvl.0
        };
        assert_eq!(
            after, 6,
            "blessed gain level should gain exactly 1 level (from 5 to 6)"
        );
    }

    // ── Blindness: duration matches spec ranges ───────────────────────

    #[test]
    fn test_potion_blindness_duration_range() {
        // Spec: blessed rn1(200, 125) = [125, 325)
        //       cursed  rn1(200, 375) = [375, 575)
        for seed in 42..52 {
            let mut world_b = make_world();
            let mut rng_b = Pcg64::seed_from_u64(seed);
            let player_b = world_b.player();
            let potion_b = spawn_potion(&mut world_b, blessed());
            let events_b = quaff_potion(
                &mut world_b,
                player_b,
                potion_b,
                PotionType::Blindness,
                &mut rng_b,
            );

            let dur_b = events_b.iter().find_map(|e| match e {
                EngineEvent::StatusApplied {
                    status: StatusEffect::Blind,
                    duration: Some(d),
                    ..
                } => Some(*d),
                _ => None,
            });
            assert!(dur_b.is_some());
            let db = dur_b.unwrap();
            assert!(
                db >= 125 && db < 325,
                "blessed blindness {} should be in [125, 325)",
                db
            );

            let mut world_c = make_world();
            let mut rng_c = Pcg64::seed_from_u64(seed);
            let player_c = world_c.player();
            let potion_c = spawn_potion(&mut world_c, cursed());
            let events_c = quaff_potion(
                &mut world_c,
                player_c,
                potion_c,
                PotionType::Blindness,
                &mut rng_c,
            );

            let dur_c = events_c.iter().find_map(|e| match e {
                EngineEvent::StatusApplied {
                    status: StatusEffect::Blind,
                    duration: Some(d),
                    ..
                } => Some(*d),
                _ => None,
            });
            assert!(dur_c.is_some());
            let dc = dur_c.unwrap();
            assert!(
                dc >= 375 && dc < 575,
                "cursed blindness {} should be in [375, 575)",
                dc
            );
        }
    }

    // ── Hallucination: duration matches spec ranges ───────────────────

    #[test]
    fn test_potion_hallucination_duration_range() {
        // Spec: blessed rn1(200, 300) = [300, 500)
        //       cursed  rn1(200, 900) = [900, 1100)
        for seed in 42..52 {
            let mut world_b = make_world();
            let mut rng_b = Pcg64::seed_from_u64(seed);
            let player_b = world_b.player();
            let potion_b = spawn_potion(&mut world_b, blessed());
            let events_b = quaff_potion(
                &mut world_b,
                player_b,
                potion_b,
                PotionType::Hallucination,
                &mut rng_b,
            );

            let dur_b = events_b.iter().find_map(|e| match e {
                EngineEvent::StatusApplied {
                    status: StatusEffect::Hallucinating,
                    duration: Some(d),
                    ..
                } => Some(*d),
                _ => None,
            });
            assert!(dur_b.is_some());
            let db = dur_b.unwrap();
            assert!(
                db >= 300 && db < 500,
                "blessed hallucination {} should be in [300, 500)",
                db
            );

            let mut world_c = make_world();
            let mut rng_c = Pcg64::seed_from_u64(seed);
            let player_c = world_c.player();
            let potion_c = spawn_potion(&mut world_c, cursed());
            let events_c = quaff_potion(
                &mut world_c,
                player_c,
                potion_c,
                PotionType::Hallucination,
                &mut rng_c,
            );

            let dur_c = events_c.iter().find_map(|e| match e {
                EngineEvent::StatusApplied {
                    status: StatusEffect::Hallucinating,
                    duration: Some(d),
                    ..
                } => Some(*d),
                _ => None,
            });
            assert!(dur_c.is_some());
            let dc = dur_c.unwrap();
            assert!(
                dc >= 900 && dc < 1100,
                "cursed hallucination {} should be in [900, 1100)",
                dc
            );
        }
    }

    // ── Speed: duration matches spec ranges ───────────────────────────

    #[test]
    fn test_potion_speed_duration_range() {
        // Spec: blessed rn1(10, 160) = [160, 170)
        //       cursed  rn1(10, 40)  = [40, 50)
        for seed in 42..52 {
            let mut world = make_world();
            let mut rng = Pcg64::seed_from_u64(seed);
            let player = world.player();
            let potion = spawn_potion(&mut world, blessed());
            let events = quaff_potion(
                &mut world,
                player,
                potion,
                PotionType::Speed,
                &mut rng,
            );

            let dur = events.iter().find_map(|e| match e {
                EngineEvent::StatusApplied {
                    status: StatusEffect::FastSpeed,
                    duration: Some(d),
                    ..
                } => Some(*d),
                _ => None,
            });
            assert!(dur.is_some());
            let d = dur.unwrap();
            assert!(
                d >= 160 && d < 170,
                "blessed speed duration {} should be in [160, 170)",
                d
            );
        }
    }

    // ── Sleeping: duration matches spec ───────────────────────────────

    #[test]
    fn test_potion_sleeping_duration_range() {
        // Spec: blessed rn1(10, 13) = [13, 23)
        for seed in 42..52 {
            let mut world = make_world();
            let mut rng = Pcg64::seed_from_u64(seed);
            let player = world.player();
            let potion = spawn_potion(&mut world, blessed());
            let events = quaff_potion(
                &mut world,
                player,
                potion,
                PotionType::Sleeping,
                &mut rng,
            );

            let dur = events.iter().find_map(|e| match e {
                EngineEvent::StatusApplied {
                    status: StatusEffect::Sleeping,
                    duration: Some(d),
                    ..
                } => Some(*d),
                _ => None,
            });
            assert!(dur.is_some(), "should apply sleeping");
            let d = dur.unwrap();
            assert!(
                d >= 13 && d < 23,
                "blessed sleeping duration {} should be in [13, 23)",
                d
            );
        }
    }

    // ── Thrown potion: invisibility applies Invisible status ─────

    #[test]
    fn thrown_invisibility_applies_status() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Spawn a monster near the target position.
        let target_pos = Position::new(41, 10);
        let monster = world.spawn((
            Positioned(target_pos),
            HitPoints { current: 20, max: 20 },
            Monster,
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::Invisibility,
            target_pos,
            &mut rng,
        );

        let has_invis = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Invisible,
                ..
            } if *e == monster
        ));
        assert!(has_invis, "thrown invisibility should apply Invisible status");
    }

    // ── Thrown potion: polymorph applies Polymorphed status ──────

    #[test]
    fn thrown_polymorph_applies_status() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let target_pos = Position::new(41, 10);
        let monster = world.spawn((
            Positioned(target_pos),
            HitPoints { current: 20, max: 20 },
            Monster,
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::Polymorph,
            target_pos,
            &mut rng,
        );

        let has_poly = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Polymorphed,
                ..
            } if *e == monster
        ));
        assert!(has_poly, "thrown polymorph should apply Polymorphed status");
    }

    // ── Thrown potion: sickness halves HP (no resistance) ────────

    #[test]
    fn thrown_sickness_halves_hp() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let target_pos = Position::new(41, 10);
        let monster = world.spawn((
            Positioned(target_pos),
            HitPoints { current: 20, max: 20 },
            Monster,
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let _events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::Sickness,
            target_pos,
            &mut rng,
        );

        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert_eq!(hp.current, 10, "thrown sickness should halve HP: got {}", hp.current);
    }

    // ── Thrown potion: sickness does nothing with poison resistance ──

    #[test]
    fn thrown_sickness_resisted_by_poison() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let target_pos = Position::new(41, 10);
        let monster = world.spawn((
            Positioned(target_pos),
            HitPoints { current: 20, max: 20 },
            Monster,
            PoisonResistance,
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let _events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::Sickness,
            target_pos,
            &mut rng,
        );

        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert_eq!(hp.current, 20, "poison resistance should block thrown sickness");
    }

    // ── Thrown potion: booze causes confusion ────────────────────

    #[test]
    fn thrown_booze_causes_confusion() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let target_pos = Position::new(41, 10);
        let monster = world.spawn((
            Positioned(target_pos),
            HitPoints { current: 20, max: 20 },
            Monster,
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::Booze,
            target_pos,
            &mut rng,
        );

        let has_conf = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Confused,
                ..
            } if *e == monster
        ));
        assert!(has_conf, "thrown booze should cause confusion");
    }

    // ── Thrown potion: restore ability heals monster to max ──────

    #[test]
    fn thrown_restore_ability_heals_monster() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let target_pos = Position::new(41, 10);
        let monster = world.spawn((
            Positioned(target_pos),
            HitPoints { current: 5, max: 20 },
            Monster,
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let _events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::RestoreAbility,
            target_pos,
            &mut rng,
        );

        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert_eq!(hp.current, 20, "thrown restore ability should heal monster to max HP");
    }

    // ── Ghost from bottle: milky potion releases ghost ──────────

    #[test]
    fn ghost_from_bottle_triggers() {
        let mut world = make_world();
        let player = world.player();

        // With seed 42 and 0 ghosts born, chance = max(1, 4+0) = 4,
        // so 1/4 chance. Try multiple seeds until we get a ghost.
        let mut found_ghost = false;
        for seed in 0..50u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let (occupant, events) = check_potion_occupant(
                &mut world,
                player,
                PotionAppearance::Milky,
                0, // djinni_born
                0, // ghost_born
                &mut rng,
            );

            if occupant == PotionOccupant::Ghost {
                // Should have monster generated event.
                let has_monster = events.iter().any(|e| matches!(
                    e,
                    EngineEvent::MonsterGenerated { .. }
                ));
                assert!(has_monster, "ghost should generate a monster");

                let has_msg = events.iter().any(|e| matches!(
                    e,
                    EngineEvent::Message { key, .. } if key == "ghost-from-bottle"
                ));
                assert!(has_msg, "should emit ghost-from-bottle message");

                // Ghost should paralyze the drinker.
                let has_para = events.iter().any(|e| matches!(
                    e,
                    EngineEvent::StatusApplied {
                        entity: e,
                        status: StatusEffect::Paralyzed,
                        duration: Some(3),
                        ..
                    } if *e == player
                ));
                assert!(has_para, "ghost should paralyze drinker for 3 turns");

                found_ghost = true;
                break;
            }
        }
        assert!(found_ghost, "milky potion should eventually release a ghost");
    }

    // ── Djinni from bottle: smoky potion releases djinni ────────

    #[test]
    fn djinni_from_bottle_triggers() {
        let mut world = make_world();
        let player = world.player();

        let mut found_djinni = false;
        for seed in 0..50u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let (occupant, events) = check_potion_occupant(
                &mut world,
                player,
                PotionAppearance::Smoky,
                0, // djinni_born
                0, // ghost_born
                &mut rng,
            );

            if occupant == PotionOccupant::Djinni {
                let has_monster = events.iter().any(|e| matches!(
                    e,
                    EngineEvent::MonsterGenerated { .. }
                ));
                assert!(has_monster, "djinni should generate a monster");

                let has_msg = events.iter().any(|e| matches!(
                    e,
                    EngineEvent::Message { key, .. } if key == "djinni-from-bottle"
                ));
                assert!(has_msg, "should emit djinni-from-bottle message");

                found_djinni = true;
                break;
            }
        }
        assert!(found_djinni, "smoky potion should eventually release a djinni");
    }

    // ── No occupant for non-special appearance ──────────────────

    #[test]
    fn no_occupant_for_other_appearance() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = make_rng();

        let (occupant, events) = check_potion_occupant(
            &mut world,
            player,
            PotionAppearance::Other,
            0,
            0,
            &mut rng,
        );
        assert_eq!(occupant, PotionOccupant::None, "Other appearance should never have occupant");
        assert!(events.is_empty(), "no events for non-special appearance");
    }

    // ── Higher born count reduces occupant chance ───────────────

    #[test]
    fn high_born_count_reduces_chance() {
        let mut world = make_world();
        let player = world.player();

        // With 100 ghosts born, chance = max(1, 4 + 100/3) = 37.
        // So only 1/37 per attempt. In 50 tries, it's unlikely to trigger.
        let mut triggered_count = 0;
        for seed in 0..50u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let (occupant, _) = check_potion_occupant(
                &mut world,
                player,
                PotionAppearance::Milky,
                0,
                100, // high ghost_born
                &mut rng,
            );
            if occupant == PotionOccupant::Ghost {
                triggered_count += 1;
            }
        }
        // With 1/37 chance and 50 trials, expected ~1.35 triggers.
        // It's possible to get 0-4; just verify it's much less than
        // the ~12.5 expected with born=0.
        assert!(
            triggered_count <= 10,
            "high born count should reduce trigger frequency: got {}",
            triggered_count
        );
    }

    // ── Thrown potion: gain ability heals monster to max ─────────

    #[test]
    fn thrown_gain_ability_heals_monster() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let target_pos = Position::new(41, 10);
        let monster = world.spawn((
            Positioned(target_pos),
            HitPoints { current: 3, max: 20 },
            Monster,
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let _events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::GainAbility,
            target_pos,
            &mut rng,
        );

        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert_eq!(hp.current, 20, "thrown gain ability should heal monster to max HP");
    }

    // ── Quaff: Levitation applies status ─────────────────────────

    #[test]
    fn quaff_levitation_applies_status() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Levitation,
            &mut rng,
        );

        let has_lev = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Levitating,
                ..
            }
        ));
        assert!(has_lev, "quaffing levitation should apply Levitating status");
    }

    // ── Quaff: Oil applies status or message ─────────────────────

    #[test]
    fn quaff_oil_produces_event() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Oil,
            &mut rng,
        );

        // Oil potion should produce some event (slippery message or status).
        // At minimum the potion is consumed (ItemDestroyed).
        let has_destroyed = events.iter().any(|e| matches!(
            e, EngineEvent::ItemDestroyed { .. }
        ));
        assert!(has_destroyed, "quaffing oil should consume the potion");
    }

    // ── Quaff: Polymorph applies polymorphed status ─────────────

    #[test]
    fn quaff_polymorph_applies_status() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Polymorph,
            &mut rng,
        );

        let has_poly = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Polymorphed,
                ..
            }
        ));
        assert!(has_poly, "quaffing polymorph should apply Polymorphed status");
    }

    // ── Quaff: Water produces message ────────────────────────────

    #[test]
    fn quaff_water_uncursed_message() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Water,
            &mut rng,
        );

        // Potion should be consumed.
        let has_destroyed = events.iter().any(|e| matches!(
            e, EngineEvent::ItemDestroyed { .. }
        ));
        assert!(has_destroyed, "quaffing water should consume the potion");
    }

    // ── Quaff: Booze has effects ─────────────────────────────────

    #[test]
    fn quaff_booze_produces_effects() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::Booze,
            &mut rng,
        );

        // Booze should produce confusion or nutrition events.
        assert!(!events.is_empty(), "quaffing booze should produce events");
    }

    // ── Quaff: Fruit juice restores nutrition ────────────────────

    #[test]
    fn quaff_fruit_juice_restores_nutrition() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Lower the player's nutrition.
        if let Some(mut nut) = world.get_component_mut::<Nutrition>(player) {
            nut.0 = 100;
        }

        let potion = spawn_potion(&mut world, uncursed());
        let events = quaff_potion(
            &mut world,
            player,
            potion,
            PotionType::FruitJuice,
            &mut rng,
        );

        // At minimum the potion is consumed.
        let has_destroyed = events.iter().any(|e| matches!(
            e, EngineEvent::ItemDestroyed { .. }
        ));
        assert!(has_destroyed, "quaffing fruit juice should consume the potion");
    }

    // ── Splash category: acid ────────────────────────────────────

    #[test]
    fn splash_category_acid() {
        assert_eq!(
            potion_splash_category(PotionType::Acid, false),
            PotionSplashEffect::AcidDamage,
        );
    }

    // ── Splash category: holy water ──────────────────────────────

    #[test]
    fn splash_category_holy_water() {
        assert_eq!(
            potion_splash_category(PotionType::Water, true),
            PotionSplashEffect::HolyDamage,
        );
    }

    // ── Splash category: plain water is no effect ────────────────

    #[test]
    fn splash_category_plain_water() {
        assert_eq!(
            potion_splash_category(PotionType::Water, false),
            PotionSplashEffect::NoEffect,
        );
    }

    // ── Splash category: sleeping ────────────────────────────────

    #[test]
    fn splash_category_sleeping() {
        assert_eq!(
            potion_splash_category(PotionType::Sleeping, false),
            PotionSplashEffect::Sleep,
        );
    }

    // ── Splash category: paralysis ───────────────────────────────

    #[test]
    fn splash_category_paralysis() {
        assert_eq!(
            potion_splash_category(PotionType::Paralysis, false),
            PotionSplashEffect::Paralyze,
        );
    }

    // ── Splash category: blindness ───────────────────────────────

    #[test]
    fn splash_category_blindness() {
        assert_eq!(
            potion_splash_category(PotionType::Blindness, false),
            PotionSplashEffect::Blind,
        );
    }

    // ── Splash category: confusion ───────────────────────────────

    #[test]
    fn splash_category_confusion() {
        assert_eq!(
            potion_splash_category(PotionType::Confusion, false),
            PotionSplashEffect::Confuse,
        );
    }

    // ── Splash category: booze causes confusion ──────────────────

    #[test]
    fn splash_category_booze_confuses() {
        assert_eq!(
            potion_splash_category(PotionType::Booze, false),
            PotionSplashEffect::Confuse,
        );
    }

    // ── Splash category: polymorph ───────────────────────────────

    #[test]
    fn splash_category_polymorph() {
        assert_eq!(
            potion_splash_category(PotionType::Polymorph, false),
            PotionSplashEffect::Polymorph,
        );
    }

    // ── Splash category: healing types ───────────────────────────

    #[test]
    fn splash_category_healing() {
        assert_eq!(
            potion_splash_category(PotionType::Healing, false),
            PotionSplashEffect::Heal,
        );
        assert_eq!(
            potion_splash_category(PotionType::ExtraHealing, false),
            PotionSplashEffect::Heal,
        );
        assert_eq!(
            potion_splash_category(PotionType::FullHealing, false),
            PotionSplashEffect::Heal,
        );
    }

    // ── Splash category: speed ───────────────────────────────────

    #[test]
    fn splash_category_speed() {
        assert_eq!(
            potion_splash_category(PotionType::Speed, false),
            PotionSplashEffect::Speed,
        );
    }

    // ── Splash category: invisibility ────────────────────────────

    #[test]
    fn splash_category_invisibility() {
        assert_eq!(
            potion_splash_category(PotionType::Invisibility, false),
            PotionSplashEffect::MakeInvisible,
        );
    }

    // ── Splash category: sickness ────────────────────────────────

    #[test]
    fn splash_category_sickness() {
        assert_eq!(
            potion_splash_category(PotionType::Sickness, false),
            PotionSplashEffect::Sicken,
        );
    }

    // ── Splash category: restore/gain ability heals ──────────────

    #[test]
    fn splash_category_restore_heals() {
        assert_eq!(
            potion_splash_category(PotionType::RestoreAbility, false),
            PotionSplashEffect::Heal,
        );
        assert_eq!(
            potion_splash_category(PotionType::GainAbility, false),
            PotionSplashEffect::Heal,
        );
    }

    // ── Splash category: no-effect types ─────────────────────────

    #[test]
    fn splash_category_no_effect() {
        assert_eq!(
            potion_splash_category(PotionType::Oil, false),
            PotionSplashEffect::NoEffect,
        );
        assert_eq!(
            potion_splash_category(PotionType::FruitJuice, false),
            PotionSplashEffect::NoEffect,
        );
        assert_eq!(
            potion_splash_category(PotionType::Enlightenment, false),
            PotionSplashEffect::NoEffect,
        );
    }

    // ── Thrown potion: acid deals damage ──────────────────────────

    #[test]
    fn thrown_acid_deals_damage() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let target_pos = Position::new(41, 10);
        let monster = world.spawn((
            Positioned(target_pos),
            HitPoints { current: 20, max: 20 },
            Monster,
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let _events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::Acid,
            target_pos,
            &mut rng,
        );

        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert!(
            hp.current < 20,
            "thrown acid should deal damage: HP={}",
            hp.current
        );
    }

    // ── Thrown potion: sleeping applies sleep status ──────────────

    #[test]
    fn thrown_sleeping_applies_status() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let target_pos = Position::new(41, 10);
        let monster = world.spawn((
            Positioned(target_pos),
            HitPoints { current: 20, max: 20 },
            Monster,
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::Sleeping,
            target_pos,
            &mut rng,
        );

        let has_sleep = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Sleeping,
                ..
            } if *e == monster
        ));
        assert!(has_sleep, "thrown sleeping should apply Sleeping status");
    }

    // ── Thrown potion: paralysis applies status ───────────────────

    #[test]
    fn thrown_paralysis_applies_status() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let target_pos = Position::new(41, 10);
        let monster = world.spawn((
            Positioned(target_pos),
            HitPoints { current: 20, max: 20 },
            Monster,
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::Paralysis,
            target_pos,
            &mut rng,
        );

        let has_para = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Paralyzed,
                ..
            } if *e == monster
        ));
        assert!(has_para, "thrown paralysis should apply Paralyzed status");
    }

    // ── Thrown potion: blindness applies status ───────────────────

    #[test]
    fn thrown_blindness_applies_status() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let target_pos = Position::new(41, 10);
        let monster = world.spawn((
            Positioned(target_pos),
            HitPoints { current: 20, max: 20 },
            Monster,
        ));

        let potion = spawn_potion(&mut world, uncursed());
        let events = throw_potion(
            &mut world,
            player,
            potion,
            PotionType::Blindness,
            target_pos,
            &mut rng,
        );

        let has_blind = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                entity: e,
                status: StatusEffect::Blind,
                ..
            } if *e == monster
        ));
        assert!(has_blind, "thrown blindness should apply Blind status");
    }

    // ── Hallucination splash ─────────────────────────────────────

    #[test]
    fn splash_category_hallucination() {
        assert_eq!(
            potion_splash_category(PotionType::Hallucination, false),
            PotionSplashEffect::Hallucinate,
        );
    }
}
