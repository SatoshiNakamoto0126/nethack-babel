//! Monster-hits-player combat resolution.
//!
//! Implements `mhitu.c` logic: when a monster attacks the player.
//! Covers melee attacks, engulfment, gaze attacks, passive damage,
//! and all damage-type special effects (fire, cold, poison, stoning, etc.).
//!
//! All functions are pure: they operate on `GameWorld` plus an RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  Zero IO.

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{AttackDef, AttackMethod, DamageType, ResistanceSet};

use crate::combat::{Engulfed, MonsterAttacks, apply_damage_type, roll_dice};
use crate::event::{DamageSource, DeathCause, EngineEvent, HpSource, PassiveEffect};
use crate::status::{self, Intrinsics};
use crate::world::{ArmorClass, ExperienceLevel, GameWorld, HitPoints};

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// Parameters for resolving a monster's attack against the player.
pub struct MonsterAttackParams {
    pub attacker: Entity,
    pub defender: Entity,
    pub attack: AttackDef,
    pub monster_level: u8,
    pub monster_str: u8,
    /// Player resistances (from intrinsics + equipment).
    pub player_resistances: ResistanceSet,
    /// Player's current AC.
    pub player_ac: i32,
}

/// Result of a single monster attack resolution.
pub struct AttackResult {
    pub events: Vec<EngineEvent>,
    pub damage: i32,
    pub hit: bool,
    /// Whether the attack killed the defender.
    pub fatal: bool,
    /// Whether the attacker died (e.g., passive damage).
    pub attacker_died: bool,
}

impl AttackResult {
    fn miss(attacker: Entity, defender: Entity) -> Self {
        Self {
            events: vec![EngineEvent::MeleeMiss { attacker, defender }],
            damage: 0,
            hit: false,
            fatal: false,
            attacker_died: false,
        }
    }
}

/// Monster flags relevant to passive damage resolution.
pub struct MonsterFlags {
    pub acidic: bool,
    pub shock_touch: bool,
    pub poisonous: bool,
    pub stoning: bool,
}

// ---------------------------------------------------------------------------
// Hit chance
// ---------------------------------------------------------------------------

/// Calculate whether a monster hits the player.
///
/// Matches the existing `resolve_monster_attack_slot` formula in `combat.rs`:
/// `to_hit = monster_level + 10 + player_ac`, then hit if `to_hit > d20`.
///
/// AC convention: 10 = unarmored (easy to hit), negative = well armored
/// (hard to hit).  Natural 20 always hits; natural 1 always misses.
pub fn monster_hit_chance(monster_level: u8, player_ac: i32, rng: &mut impl Rng) -> bool {
    let roll = rng.random_range(1..=20);
    // Natural 20 always hits; natural 1 always misses.
    if roll >= 20 {
        return true;
    }
    if roll <= 1 {
        return false;
    }
    // to_hit = monster_level + 10 + player_ac
    // Higher AC (10) = easier to hit; lower AC (-5) = harder to hit.
    let to_hit = monster_level as i32 + 10 + player_ac;
    to_hit > roll
}

// ---------------------------------------------------------------------------
// Physical damage
// ---------------------------------------------------------------------------

/// Roll physical damage from a monster attack's dice expression.
fn resolve_physical_damage(attack: &AttackDef, rng: &mut impl Rng) -> i32 {
    roll_dice(attack.dice, rng).max(0)
}

// ---------------------------------------------------------------------------
// Special damage effects
// ---------------------------------------------------------------------------

/// Resolve special damage effects (fire, cold, poison, drain, etc.).
///
/// Delegates to `apply_damage_type` from the combat module, which handles
/// all AD_xxx variants.  Wraps the result into an `AttackResult`.
fn resolve_special_damage(
    world: &mut GameWorld,
    params: &MonsterAttackParams,
    base_damage: i32,
    rng: &mut impl Rng,
) -> AttackResult {
    let result = apply_damage_type(
        world,
        params.defender,
        params.attack.damage_type,
        base_damage,
        params.attacker,
        rng,
    );

    let hp_damage = result.hp_damage.max(0);
    let mut events = result.events;

    if hp_damage > 0 {
        events.push(EngineEvent::MeleeHit {
            attacker: params.attacker,
            defender: params.defender,
            weapon: None,
            damage: hp_damage as u32,
        });

        let (_new_hp, fatal) = apply_hp_damage_tracked(
            world,
            params.defender,
            hp_damage,
            params.attacker,
            &mut events,
        );

        AttackResult {
            events,
            damage: hp_damage,
            hit: true,
            fatal,
            attacker_died: false,
        }
    } else {
        // Status-only effects (sleep, blind, etc.) still count as a hit
        // if the damage type produced events.
        let hit = params.attack.damage_type != DamageType::Physical || base_damage > 0;
        AttackResult {
            events,
            damage: 0,
            hit,
            fatal: false,
            attacker_died: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Single attack resolution
// ---------------------------------------------------------------------------

/// Resolve a single attack from a monster against the player.
pub fn resolve_monster_attack(
    world: &mut GameWorld,
    params: &MonsterAttackParams,
    rng: &mut impl Rng,
) -> AttackResult {
    match params.attack.method {
        AttackMethod::Claw
        | AttackMethod::Bite
        | AttackMethod::Kick
        | AttackMethod::Butt
        | AttackMethod::Sting
        | AttackMethod::Touch
        | AttackMethod::Tentacle
        | AttackMethod::Weapon
        | AttackMethod::Hug => {
            // Standard melee: hit check + damage type dispatch.
            if !monster_hit_chance(params.monster_level, params.player_ac, rng) {
                return AttackResult::miss(params.attacker, params.defender);
            }
            let base_damage = resolve_physical_damage(&params.attack, rng);
            resolve_special_damage(world, params, base_damage, rng)
        }

        AttackMethod::Engulf => {
            gulpmu(world, params.attacker, params.defender, &params.attack, rng)
        }

        AttackMethod::Gaze => gazemu(world, params.attacker, params.defender, &params.attack, rng),

        AttackMethod::Breath | AttackMethod::Spit => {
            // Breath/spit: auto-hit, roll dice, apply damage type.
            let base_damage = resolve_physical_damage(&params.attack, rng);
            resolve_special_damage(world, params, base_damage, rng)
        }

        AttackMethod::MagicMissile => {
            // Monster spellcasting: stub — treat as physical for now.
            if !monster_hit_chance(params.monster_level, params.player_ac, rng) {
                return AttackResult::miss(params.attacker, params.defender);
            }
            let base_damage = resolve_physical_damage(&params.attack, rng);
            resolve_special_damage(world, params, base_damage, rng)
        }

        AttackMethod::Explode => {
            // Explode: auto-hit, attacker dies.
            let base_damage = resolve_physical_damage(&params.attack, rng);
            let mut result = resolve_special_damage(world, params, base_damage, rng);
            // The exploding monster dies.
            let attacker_name = world.entity_name(params.attacker);
            result.events.push(EngineEvent::EntityDied {
                entity: params.attacker,
                killer: None,
                cause: DeathCause::KilledBy {
                    killer_name: attacker_name,
                },
            });
            result.attacker_died = true;
            result
        }

        // Passive (AT_NONE) and boom (AT_BOOM) are not active attacks.
        AttackMethod::None | AttackMethod::Boom => AttackResult {
            events: vec![],
            damage: 0,
            hit: false,
            fatal: false,
            attacker_died: false,
        },
    }
}

// ---------------------------------------------------------------------------
// Full monster attack round (mattacku)
// ---------------------------------------------------------------------------

/// Resolve all attacks from a monster's attack array against the player.
///
/// Iterates through the monster's `MonsterAttacks` component, resolving
/// each slot in order.  Stops early if the defender dies or the attacker
/// dies (e.g., from passive damage or explosion).
pub fn mattacku(world: &mut GameWorld, attacker: Entity, rng: &mut impl Rng) -> Vec<EngineEvent> {
    let mut all_events = Vec::new();
    let defender = world.player();

    // Bail if either entity is missing HP.
    if world.get_component::<HitPoints>(attacker).is_none()
        || world.get_component::<HitPoints>(defender).is_none()
    {
        return all_events;
    }

    // Extract attack array.
    let attacks: arrayvec::ArrayVec<AttackDef, 6> = world
        .get_component::<MonsterAttacks>(attacker)
        .map(|ma| ma.0.clone())
        .unwrap_or_default();

    if attacks.is_empty() {
        return all_events;
    }

    // Extract defender stats once.
    let player_ac = world
        .get_component::<ArmorClass>(defender)
        .map(|ac| ac.0)
        .unwrap_or(10);

    let player_resistances = get_player_resistances(world, defender);

    let monster_level = world
        .get_component::<ExperienceLevel>(attacker)
        .map(|l| l.0)
        .unwrap_or(1);

    for attack in attacks.iter() {
        if attack.method == AttackMethod::None {
            continue;
        }

        // Check if defender is still alive.
        let defender_alive = world
            .get_component::<HitPoints>(defender)
            .is_some_and(|hp| hp.current > 0);
        if !defender_alive {
            break;
        }

        // Check if attacker is still alive.
        let attacker_alive = world
            .get_component::<HitPoints>(attacker)
            .is_some_and(|hp| hp.current > 0);
        if !attacker_alive {
            break;
        }

        let params = MonsterAttackParams {
            attacker,
            defender,
            attack: attack.clone(),
            monster_level,
            monster_str: 10,
            player_resistances,
            player_ac,
        };

        let result = resolve_monster_attack(world, &params, rng);
        all_events.extend(result.events);

        // If the attack hit, check for passive damage from the defender.
        if result.hit && !result.attacker_died {
            let passive_events = check_passive_damage(world, attacker, defender, rng);
            all_events.extend(passive_events);
        }

        if result.fatal || result.attacker_died {
            break;
        }
    }

    all_events
}

// ---------------------------------------------------------------------------
// Engulfment (gulpmu)
// ---------------------------------------------------------------------------

/// Resolve an engulfment attack.
///
/// On hit, the player is swallowed by the monster and takes initial
/// damage based on the attack's damage type.
pub fn gulpmu(
    world: &mut GameWorld,
    attacker: Entity,
    defender: Entity,
    attack: &AttackDef,
    rng: &mut impl Rng,
) -> AttackResult {
    // Already engulfed? No effect.
    if world.get_component::<Engulfed>(defender).is_some() {
        return AttackResult {
            events: vec![],
            damage: 0,
            hit: false,
            fatal: false,
            attacker_died: false,
        };
    }

    let monster_level = world
        .get_component::<ExperienceLevel>(attacker)
        .map(|l| l.0 as i32)
        .unwrap_or(1);

    // Hit check for engulf.
    let threshold = rng.random_range(1..=20i32);
    if monster_level + 10 <= threshold {
        return AttackResult::miss(attacker, defender);
    }

    // Apply engulf component.
    let duration = (rng.random_range(1..=(monster_level + 5).max(1)) as u32).max(2);
    let _ = world.ecs_mut().insert_one(
        defender,
        Engulfed {
            by: attacker,
            turns_remaining: duration,
        },
    );

    let mut events = Vec::new();
    let attacker_name = world.entity_name(attacker);
    events.push(EngineEvent::msg_with(
        "attack-engulf",
        vec![("monster", attacker_name)],
    ));

    // Initial digest/damage.
    let base_damage = roll_dice(attack.dice, rng);
    let type_result = apply_damage_type(
        world,
        defender,
        attack.damage_type,
        base_damage,
        attacker,
        rng,
    );
    events.extend(type_result.events);

    let hp_damage = type_result.hp_damage.max(0);
    let mut fatal = false;
    if hp_damage > 0 {
        events.push(EngineEvent::MeleeHit {
            attacker,
            defender,
            weapon: None,
            damage: hp_damage as u32,
        });
        let (_, died) = apply_hp_damage_tracked(world, defender, hp_damage, attacker, &mut events);
        fatal = died;
    }

    AttackResult {
        events,
        damage: hp_damage,
        hit: true,
        fatal,
        attacker_died: false,
    }
}

// ---------------------------------------------------------------------------
// Gaze attack (gazemu)
// ---------------------------------------------------------------------------

/// Resolve a gaze attack.
///
/// Gaze attacks auto-hit (no to-hit roll) and apply the damage type effect.
pub fn gazemu(
    world: &mut GameWorld,
    attacker: Entity,
    defender: Entity,
    attack: &AttackDef,
    rng: &mut impl Rng,
) -> AttackResult {
    let base_damage = roll_dice(attack.dice, rng);

    let type_result = apply_damage_type(
        world,
        defender,
        attack.damage_type,
        base_damage,
        attacker,
        rng,
    );

    let mut events = type_result.events;
    let hp_damage = type_result.hp_damage.max(0);
    let mut fatal = false;

    if hp_damage > 0 {
        events.push(EngineEvent::ExtraDamage {
            target: defender,
            amount: hp_damage as u32,
            source: DamageSource::Melee,
        });
        let (_, died) = apply_hp_damage_tracked(world, defender, hp_damage, attacker, &mut events);
        fatal = died;
    }

    AttackResult {
        events,
        damage: hp_damage,
        hit: true,
        fatal,
        attacker_died: false,
    }
}

// ---------------------------------------------------------------------------
// Passive damage (passiveum)
// ---------------------------------------------------------------------------

/// Resolve passive damage when a monster hits the player.
///
/// Certain monsters (e.g., acid blobs, electric eels) deal damage back
/// to attackers that hit them.  This is the reverse direction: the
/// player's intrinsic properties may passively damage the attacking monster.
///
/// In the mhitu context, this checks the *defender's* (player's) flags
/// and applies effects to the *attacker* (monster).
pub fn passiveum(
    world: &mut GameWorld,
    attacker: Entity,
    defender: Entity,
    defender_flags: &MonsterFlags,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Acid splash: acidic defenders splash acid on melee attackers.
    if defender_flags.acidic {
        let damage = rng.random_range(1..=4);
        events.push(EngineEvent::PassiveAttack {
            attacker: defender,
            defender: attacker,
            effect: PassiveEffect::AcidSplash {
                damage: damage as u32,
            },
        });
        let (_, _) = apply_hp_damage_tracked(world, attacker, damage, defender, &mut events);
    }

    // Electric shock.
    if defender_flags.shock_touch {
        let damage = rng.random_range(1..=6);
        events.push(EngineEvent::PassiveAttack {
            attacker: defender,
            defender: attacker,
            effect: PassiveEffect::ElectricShock {
                damage: damage as u32,
            },
        });
        let (_, _) = apply_hp_damage_tracked(world, attacker, damage, defender, &mut events);
    }

    // Petrification touch.
    if defender_flags.stoning {
        events.push(EngineEvent::PassiveAttack {
            attacker: defender,
            defender: attacker,
            effect: PassiveEffect::Petrify,
        });
        let stoning_events = status::make_stoned(world, attacker, status::STONING_INITIAL);
        events.extend(stoning_events);
    }

    events
}

// ---------------------------------------------------------------------------
// Passive damage check helper (for mattacku)
// ---------------------------------------------------------------------------

/// Check if the defender (player) has passive damage properties and apply them
/// to the attacker.  Used after a successful monster melee hit.
fn check_passive_damage(
    world: &mut GameWorld,
    attacker: Entity,
    _defender: Entity,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    // In the current implementation, passive damage from the player to
    // attacking monsters is minimal (player is not acidic/shocking).
    // This is a stub that will be expanded when polymorphed player forms
    // with passive attacks are implemented.
    let _ = world;
    let _ = attacker;
    Vec::new()
}

// ---------------------------------------------------------------------------
// Helper: player resistance set
// ---------------------------------------------------------------------------

/// Gather the player's resistance set from intrinsics.
fn get_player_resistances(world: &GameWorld, player: Entity) -> ResistanceSet {
    let mut res = ResistanceSet::empty();
    if let Some(intr) = world.get_component::<Intrinsics>(player) {
        if intr.fire_resistance {
            res |= ResistanceSet::FIRE;
        }
        if intr.cold_resistance {
            res |= ResistanceSet::COLD;
        }
        if intr.sleep_resistance {
            res |= ResistanceSet::SLEEP;
        }
        if intr.shock_resistance {
            res |= ResistanceSet::SHOCK;
        }
        if intr.poison_resistance {
            res |= ResistanceSet::POISON;
        }
        if intr.disintegration_resistance {
            res |= ResistanceSet::DISINTEGRATE;
        }
    }
    res
}

// ---------------------------------------------------------------------------
// Helper: apply HP damage and check death
// ---------------------------------------------------------------------------

/// Apply HP damage to a target, emitting HpChange and EntityDied events.
/// Returns (new_hp, died).
fn apply_hp_damage_tracked(
    world: &mut GameWorld,
    target: Entity,
    damage: i32,
    attacker: Entity,
    events: &mut Vec<EngineEvent>,
) -> (i32, bool) {
    let attacker_name = world.entity_name(attacker);
    let new_hp = if let Some(mut hp) = world.get_component_mut::<HitPoints>(target) {
        hp.current -= damage;
        events.push(EngineEvent::HpChange {
            entity: target,
            amount: -damage,
            new_hp: hp.current,
            source: HpSource::Combat,
        });
        hp.current
    } else {
        return (0, false);
    };

    let died = new_hp <= 0;
    if died {
        events.push(EngineEvent::EntityDied {
            entity: target,
            killer: Some(attacker),
            cause: DeathCause::KilledBy {
                killer_name: attacker_name,
            },
        });
    }
    (new_hp, died)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::combat::MonsterAttacks;
    use crate::event::StatusEffect;
    use crate::status::StatusEffects;
    use crate::world::{ArmorClass, ExperienceLevel, GameWorld, HitPoints, Monster, Name};
    use nethack_babel_data::{AttackDef, AttackMethod, DamageType, DiceExpr};
    use rand::SeedableRng;

    type TestRng = rand::rngs::SmallRng;

    fn make_test_world() -> GameWorld {
        GameWorld::new(Position::new(5, 5))
    }

    fn spawn_monster(
        world: &mut GameWorld,
        level: u8,
        hp: i32,
        ac: i32,
        attacks: Vec<AttackDef>,
    ) -> Entity {
        let entity = world.spawn((
            Monster,
            HitPoints {
                current: hp,
                max: hp,
            },
            ExperienceLevel(level),
            ArmorClass(ac),
            Name("test monster".to_string()),
            StatusEffects::default(),
            Intrinsics::default(),
        ));
        if !attacks.is_empty() {
            let mut av = arrayvec::ArrayVec::new();
            for a in attacks {
                av.push(a);
            }
            let _ = world.ecs_mut().insert_one(entity, MonsterAttacks(av));
        }
        entity
    }

    fn make_attack(method: AttackMethod, dtype: DamageType, count: u8, sides: u8) -> AttackDef {
        AttackDef {
            method,
            damage_type: dtype,
            dice: DiceExpr { count, sides },
        }
    }

    // --- Test 1: Physical attack damage ranges ---

    #[test]
    fn physical_damage_in_range() {
        let mut rng = TestRng::seed_from_u64(42);
        let attack = make_attack(AttackMethod::Claw, DamageType::Physical, 2, 6);
        // Roll many times and verify range [0, 12].
        for _ in 0..1000 {
            let dmg = resolve_physical_damage(&attack, &mut rng);
            assert!(dmg >= 2 && dmg <= 12, "damage {} out of range", dmg);
        }
    }

    // --- Test 2: Hit chance at various AC levels ---

    #[test]
    fn hit_chance_low_ac_harder() {
        let mut rng = TestRng::seed_from_u64(123);
        let trials = 10000;

        // Count hits against AC 10 (bad armor) and AC -5 (great armor).
        let mut hits_ac10 = 0;
        let mut hits_ac_neg5 = 0;
        for _ in 0..trials {
            if monster_hit_chance(5, 10, &mut rng) {
                hits_ac10 += 1;
            }
            if monster_hit_chance(5, -5, &mut rng) {
                hits_ac_neg5 += 1;
            }
        }

        // AC -5 should be hit less often than AC 10.
        assert!(
            hits_ac_neg5 < hits_ac10,
            "lower AC should reduce hit rate: AC10={}, AC-5={}",
            hits_ac10,
            hits_ac_neg5,
        );
    }

    // --- Test 3: Natural 20 always hits ---

    #[test]
    fn natural_20_always_hits() {
        // Use a seeded RNG and run many trials.  At least some should
        // hit even against very low (good) AC.
        let mut rng = TestRng::seed_from_u64(999);
        let mut any_hit = false;
        for _ in 0..10000 {
            if monster_hit_chance(1, -20, &mut rng) {
                any_hit = true;
                break;
            }
        }
        assert!(any_hit, "natural 20 should eventually hit even AC -20");
    }

    // --- Test 4: Fire damage with resistance ---

    #[test]
    fn fire_damage_with_resistance() {
        let mut world = make_test_world();
        let defender = world.player();

        // Give player fire resistance.
        if let Some(mut intr) = world.get_component_mut::<Intrinsics>(defender) {
            intr.fire_resistance = true;
        }

        let attacker = spawn_monster(&mut world, 5, 20, 10, vec![]);
        let mut rng = TestRng::seed_from_u64(55);

        let params = MonsterAttackParams {
            attacker,
            defender,
            attack: make_attack(AttackMethod::Claw, DamageType::Fire, 2, 6),
            monster_level: 5,
            monster_str: 10,
            player_resistances: ResistanceSet::FIRE,
            player_ac: 10,
        };

        // Force a hit by using resolve_special_damage directly.
        let result = resolve_special_damage(&mut world, &params, 8, &mut rng);
        // Fire-resistant player takes 0 HP damage.
        assert_eq!(result.damage, 0, "fire resistance should negate damage");
    }

    // --- Test 5: Fire damage without resistance ---

    #[test]
    fn fire_damage_without_resistance() {
        let mut world = make_test_world();
        let defender = world.player();
        let attacker = spawn_monster(&mut world, 5, 20, 10, vec![]);
        let mut rng = TestRng::seed_from_u64(55);

        let params = MonsterAttackParams {
            attacker,
            defender,
            attack: make_attack(AttackMethod::Claw, DamageType::Fire, 2, 6),
            monster_level: 5,
            monster_str: 10,
            player_resistances: ResistanceSet::empty(),
            player_ac: 10,
        };

        let result = resolve_special_damage(&mut world, &params, 8, &mut rng);
        assert!(
            result.damage > 0,
            "fire should deal damage without resistance"
        );
    }

    // --- Test 6: Poison damage with resistance ---

    #[test]
    fn poison_with_resistance() {
        let mut world = make_test_world();
        let defender = world.player();

        // Give player poison resistance.
        if let Some(mut intr) = world.get_component_mut::<Intrinsics>(defender) {
            intr.poison_resistance = true;
        }

        let attacker = spawn_monster(&mut world, 5, 20, 10, vec![]);
        let mut rng = TestRng::seed_from_u64(77);

        let params = MonsterAttackParams {
            attacker,
            defender,
            attack: make_attack(AttackMethod::Sting, DamageType::Poison, 1, 6),
            monster_level: 5,
            monster_str: 10,
            player_resistances: ResistanceSet::POISON,
            player_ac: 10,
        };

        // Poison still deals base damage but won't cause extra poison effect.
        let result = resolve_special_damage(&mut world, &params, 4, &mut rng);
        // No ExtraDamage with Poison source should appear.
        let has_poison_extra = result.events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::ExtraDamage {
                    source: DamageSource::Poison,
                    ..
                }
            )
        });
        assert!(
            !has_poison_extra,
            "poison resistance should block extra poison damage"
        );
    }

    // --- Test 7: Poison damage without resistance can cause extra damage ---

    #[test]
    fn poison_without_resistance() {
        let mut world = make_test_world();
        let defender = world.player();
        let attacker = spawn_monster(&mut world, 5, 20, 10, vec![]);

        // Run many trials; eventually the 1/8 poison should trigger.
        let mut found_poison_extra = false;
        for seed in 0..200 {
            let mut rng = TestRng::seed_from_u64(seed);
            let params = MonsterAttackParams {
                attacker,
                defender,
                attack: make_attack(AttackMethod::Sting, DamageType::Poison, 1, 6),
                monster_level: 5,
                monster_str: 10,
                player_resistances: ResistanceSet::empty(),
                player_ac: 10,
            };
            let result = resolve_special_damage(&mut world, &params, 4, &mut rng);
            if result.events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::ExtraDamage {
                        source: DamageSource::Poison,
                        ..
                    }
                )
            }) {
                found_poison_extra = true;
                break;
            }
            // Restore HP for next trial.
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(defender) {
                hp.current = hp.max;
            }
        }
        assert!(
            found_poison_extra,
            "poison without resistance should eventually trigger"
        );
    }

    // --- Test 8: Stoning initiates countdown ---

    #[test]
    fn stoning_initiates_countdown() {
        let mut world = make_test_world();
        let defender = world.player();
        let attacker = spawn_monster(&mut world, 10, 30, 5, vec![]);

        // Stone effect is probabilistic (1/3 * 1/10 = 1/30).
        // Run many trials to trigger it.
        let mut stoned = false;
        for seed in 0..1000 {
            let mut rng = TestRng::seed_from_u64(seed);
            let params = MonsterAttackParams {
                attacker,
                defender,
                attack: make_attack(AttackMethod::Touch, DamageType::Stone, 0, 0),
                monster_level: 10,
                monster_str: 10,
                player_resistances: ResistanceSet::empty(),
                player_ac: 10,
            };
            let result = resolve_special_damage(&mut world, &params, 0, &mut rng);
            if result.events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::StatusApplied {
                        status: StatusEffect::Stoning,
                        ..
                    }
                )
            }) {
                stoned = true;
                break;
            }
            // Reset stoning status for next trial.
            if let Some(mut st) = world.get_component_mut::<StatusEffects>(defender) {
                st.stoning = 0;
            }
        }
        assert!(stoned, "stoning should eventually trigger");
    }

    // --- Test 9: Sliming initiates countdown ---

    #[test]
    fn sliming_initiates_countdown() {
        let mut world = make_test_world();
        let defender = world.player();
        let _attacker = spawn_monster(&mut world, 5, 20, 10, vec![]);

        // Slime (AD_SLIM) is handled by the default branch in apply_damage_type
        // which treats it as physical damage.  For a proper test we check the
        // status module directly.
        let events = status::make_slimed(&mut world, defender, status::SLIMING_INITIAL);

        let has_slime_status = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::Slimed,
                    ..
                }
            )
        });
        assert!(has_slime_status, "sliming should apply Slimed status");

        let st = world.get_component::<StatusEffects>(defender).unwrap();
        assert_eq!(
            st.sliming,
            status::SLIMING_INITIAL,
            "sliming countdown should be set"
        );
    }

    // --- Test 10: DrainLife reduces level ---

    #[test]
    fn drain_life_reduces_level() {
        let mut world = make_test_world();
        let defender = world.player();

        // Set player to level 5.
        if let Some(mut xlvl) = world.get_component_mut::<ExperienceLevel>(defender) {
            xlvl.0 = 5;
        }

        let attacker = spawn_monster(&mut world, 8, 30, 5, vec![]);

        // DrainLife has 1/3 chance.  Run trials.
        let mut drained = false;
        for seed in 0..200 {
            let mut rng = TestRng::seed_from_u64(seed);
            let params = MonsterAttackParams {
                attacker,
                defender,
                attack: make_attack(AttackMethod::Bite, DamageType::DrainLife, 1, 4),
                monster_level: 8,
                monster_str: 10,
                player_resistances: ResistanceSet::empty(),
                player_ac: 10,
            };
            let _result = resolve_special_damage(&mut world, &params, 3, &mut rng);

            let current_level = world
                .get_component::<ExperienceLevel>(defender)
                .map(|l| l.0)
                .unwrap();
            if current_level < 5 {
                drained = true;
                break;
            }
            // Restore HP for next trial.
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(defender) {
                hp.current = hp.max;
            }
        }
        assert!(drained, "drain life should eventually reduce level");
    }

    // --- Test 11: GoldSteal via default path ---

    #[test]
    fn gold_steal_treated_as_physical() {
        let mut world = make_test_world();
        let defender = world.player();
        let attacker = spawn_monster(&mut world, 5, 20, 10, vec![]);
        let mut rng = TestRng::seed_from_u64(42);

        // GoldSteal falls through to the default branch in apply_damage_type
        // which treats it as base physical damage.
        let params = MonsterAttackParams {
            attacker,
            defender,
            attack: make_attack(AttackMethod::Touch, DamageType::GoldSteal, 1, 6),
            monster_level: 5,
            monster_str: 10,
            player_resistances: ResistanceSet::empty(),
            player_ac: 10,
        };

        let result = resolve_special_damage(&mut world, &params, 5, &mut rng);
        // The default path returns base_damage as hp_damage.
        assert!(
            result.damage > 0,
            "gold steal should deal physical damage (stub)"
        );
    }

    // --- Test 12: Paralysis applies status ---

    #[test]
    fn paralysis_applies_status() {
        let mut world = make_test_world();
        let defender = world.player();
        let attacker = spawn_monster(&mut world, 5, 20, 10, vec![]);

        // Paralyze has 1/3 chance.  Run trials.
        let mut paralyzed = false;
        for seed in 0..200 {
            let mut rng = TestRng::seed_from_u64(seed);
            let params = MonsterAttackParams {
                attacker,
                defender,
                attack: make_attack(AttackMethod::Touch, DamageType::Paralyze, 1, 4),
                monster_level: 5,
                monster_str: 10,
                player_resistances: ResistanceSet::empty(),
                player_ac: 10,
            };
            let result = resolve_special_damage(&mut world, &params, 3, &mut rng);
            if result.events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::StatusApplied {
                        status: StatusEffect::Paralyzed,
                        ..
                    }
                )
            }) {
                paralyzed = true;
                break;
            }
            // Reset paralysis for next trial.
            if let Some(mut st) = world.get_component_mut::<StatusEffects>(defender) {
                st.paralysis = 0;
            }
        }
        assert!(paralyzed, "paralyze attack should eventually apply status");
    }

    // --- Test 13: Passive acid splash damage ---

    #[test]
    fn passive_acid_splash() {
        let mut world = make_test_world();
        let attacker = spawn_monster(&mut world, 5, 20, 10, vec![]);
        let defender = world.player();
        let mut rng = TestRng::seed_from_u64(42);

        let flags = MonsterFlags {
            acidic: true,
            shock_touch: false,
            poisonous: false,
            stoning: false,
        };

        let events = passiveum(&mut world, attacker, defender, &flags, &mut rng);

        let has_acid_passive = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::PassiveAttack {
                    effect: PassiveEffect::AcidSplash { .. },
                    ..
                }
            )
        });
        assert!(
            has_acid_passive,
            "acidic defender should splash acid on attacker"
        );

        // Attacker should have taken damage.
        let attacker_hp = world.get_component::<HitPoints>(attacker).unwrap();
        assert!(
            attacker_hp.current < 20,
            "attacker should lose HP from acid"
        );
    }

    // --- Test 14: Engulfment digest damage ---

    #[test]
    fn engulfment_digest_damage() {
        let mut world = make_test_world();
        let defender = world.player();
        let attacker = spawn_monster(&mut world, 8, 30, 5, vec![]);
        let initial_hp = world.get_component::<HitPoints>(defender).unwrap().current;

        // Run trials until engulf succeeds.
        let mut engulfed = false;
        for seed in 0..100 {
            let mut rng = TestRng::seed_from_u64(seed);
            let attack = make_attack(AttackMethod::Engulf, DamageType::Digest, 2, 4);
            let result = gulpmu(&mut world, attacker, defender, &attack, &mut rng);

            if result.hit && result.damage > 0 {
                engulfed = true;
                // Check that defender took damage.
                let hp = world.get_component::<HitPoints>(defender).unwrap();
                assert!(hp.current < initial_hp, "engulfment should deal damage");
                break;
            }
            // Reset state.
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(defender) {
                hp.current = initial_hp;
            }
            // Remove engulfed component if present.
            let _ = world.ecs_mut().remove_one::<Engulfed>(defender);
        }
        assert!(
            engulfed,
            "engulfment should eventually succeed and deal damage"
        );
    }

    // --- Test 15: Multiple attacks in sequence via mattacku ---

    #[test]
    fn multiple_attacks_in_sequence() {
        let mut world = make_test_world();
        let defender = world.player();

        // Give player lots of HP so they survive.
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(defender) {
            hp.current = 100;
            hp.max = 100;
        }

        let attacks = vec![
            make_attack(AttackMethod::Claw, DamageType::Physical, 1, 4),
            make_attack(AttackMethod::Claw, DamageType::Physical, 1, 4),
            make_attack(AttackMethod::Bite, DamageType::Physical, 1, 6),
        ];
        let _attacker = spawn_monster(&mut world, 5, 20, 10, attacks);

        let mut rng = TestRng::seed_from_u64(42);
        let events = mattacku(&mut world, _attacker, &mut rng);

        // Should have some combination of hits and misses.
        let hits = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::MeleeHit { .. }))
            .count();
        let misses = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::MeleeMiss { .. }))
            .count();

        // With 3 attacks, we should have at least 1 total event.
        assert!(
            hits + misses >= 1,
            "three attack slots should produce at least one hit or miss",
        );
        assert!(hits + misses <= 3, "should not exceed 3 attacks",);
    }

    // --- Test 16: Monster death from passive damage ---

    #[test]
    fn monster_killed_by_passive_acid() {
        let mut world = make_test_world();
        let attacker = spawn_monster(&mut world, 1, 1, 10, vec![]);
        let defender = world.player();
        let mut rng = TestRng::seed_from_u64(42);

        let flags = MonsterFlags {
            acidic: true,
            shock_touch: false,
            poisonous: false,
            stoning: false,
        };

        let events = passiveum(&mut world, attacker, defender, &flags, &mut rng);

        // Monster with 1 HP should die from acid splash (1-4 damage).
        let died = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::EntityDied { entity, .. } if *entity == attacker
            )
        });
        assert!(died, "monster with 1 HP should die from passive acid");
    }

    // --- Test 17: Gaze attack auto-hits ---

    #[test]
    fn gaze_attack_auto_hits() {
        let mut world = make_test_world();
        let defender = world.player();

        // Give player very good AC.
        if let Some(mut ac) = world.get_component_mut::<ArmorClass>(defender) {
            ac.0 = -20;
        }
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(defender) {
            hp.current = 100;
            hp.max = 100;
        }

        let attacker = spawn_monster(&mut world, 10, 30, 5, vec![]);
        let mut rng = TestRng::seed_from_u64(42);

        let attack = make_attack(AttackMethod::Gaze, DamageType::Physical, 2, 6);
        let result = gazemu(&mut world, attacker, defender, &attack, &mut rng);

        // Gaze auto-hits regardless of AC.
        assert!(result.hit, "gaze attacks should auto-hit");
        assert!(result.damage > 0, "gaze should deal physical damage");
    }

    // --- Test 18: mattacku with no attacks returns empty ---

    #[test]
    fn mattacku_no_attacks_empty() {
        let mut world = make_test_world();
        let attacker = spawn_monster(&mut world, 3, 10, 10, vec![]);
        let mut rng = TestRng::seed_from_u64(42);

        let events = mattacku(&mut world, attacker, &mut rng);
        assert!(
            events.is_empty(),
            "monster with no attacks should produce no events"
        );
    }

    // --- Test 19: Sleep attack with resistance ---

    #[test]
    fn sleep_with_resistance_no_effect() {
        let mut world = make_test_world();
        let defender = world.player();

        // Give player sleep resistance.
        if let Some(mut intr) = world.get_component_mut::<Intrinsics>(defender) {
            intr.sleep_resistance = true;
        }

        let attacker = spawn_monster(&mut world, 5, 20, 10, vec![]);

        // Run many trials; none should apply Sleeping status.
        for seed in 0..100 {
            let mut rng = TestRng::seed_from_u64(seed);
            let params = MonsterAttackParams {
                attacker,
                defender,
                attack: make_attack(AttackMethod::Gaze, DamageType::Sleep, 1, 4),
                monster_level: 5,
                monster_str: 10,
                player_resistances: ResistanceSet::SLEEP,
                player_ac: 10,
            };
            let _result = resolve_special_damage(&mut world, &params, 3, &mut rng);
            let has_sleep = _result.events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::StatusApplied {
                        status: StatusEffect::Sleeping,
                        ..
                    }
                )
            });
            assert!(
                !has_sleep,
                "sleep resistance should block sleep (seed {})",
                seed
            );
        }
    }
}
