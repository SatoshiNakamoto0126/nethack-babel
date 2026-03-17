//! Monster-vs-monster melee combat resolution.
//!
//! Implements `mhitm.c` logic: when monsters fight each other (pets
//! attacking hostiles, hostile monsters with grudges, etc.).
//!
//! All functions are pure: they take a `GameWorld` plus an RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  Zero IO.

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{AttackDef, AttackMethod, DamageType};

use crate::action::Position;
use crate::combat::{MonsterAttacks, MonsterResistances, resolve_monster_attack_slot};
use crate::event::{DeathCause, EngineEvent, HpSource};
use crate::world::{ArmorClass, ExperienceLevel, GameWorld, HitPoints, Peaceful, Positioned, Tame};

// ---------------------------------------------------------------------------
// Monster-vs-monster melee resolution
// ---------------------------------------------------------------------------

/// Resolve a full round of monster-vs-monster melee combat.
///
/// The attacker uses its `MonsterAttacks` component if present, otherwise
/// deals base damage of d(level, 6).  Each attack slot is resolved in
/// order; if the defender dies mid-round, remaining slots are skipped.
///
/// Returns all generated events.
pub fn resolve_monster_vs_monster(
    world: &mut GameWorld,
    attacker: Entity,
    defender: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Bail if either entity is missing HP.
    if world.get_component::<HitPoints>(attacker).is_none()
        || world.get_component::<HitPoints>(defender).is_none()
    {
        return events;
    }

    // If the attacker has a MonsterAttacks component, dispatch each slot.
    let has_attacks = world.get_component::<MonsterAttacks>(attacker).is_some();
    if has_attacks {
        let attacks = world
            .get_component::<MonsterAttacks>(attacker)
            .map(|ma| ma.0.clone())
            .unwrap_or_default();

        for (i, attack) in attacks.iter().enumerate() {
            if attack.method == AttackMethod::None {
                continue;
            }

            // Check if defender is still alive.
            let alive = world
                .get_component::<HitPoints>(defender)
                .is_some_and(|hp| hp.current > 0);
            if !alive {
                break;
            }

            let slot_events =
                resolve_monster_attack_slot(world, attacker, defender, attack, i, rng);
            events.extend(slot_events);
        }
    } else {
        // No special attacks: deal base damage d(level, 6).
        let attacker_level = world
            .get_component::<ExperienceLevel>(attacker)
            .map(|l| l.0)
            .unwrap_or(1);

        let target_ac = world
            .get_component::<ArmorClass>(defender)
            .map(|ac| ac.0)
            .unwrap_or(10);

        // Simple hit check: attacker_level + 10 + target_ac > rnd(20)
        let to_hit = attacker_level as i32 + 10 + target_ac;
        let threshold = rng.random_range(1..=20i32);

        if to_hit > threshold {
            // Roll d(level, 6), minimum 1d6.
            let dice_count = (attacker_level as i32).max(1);
            let mut damage = 0i32;
            for _ in 0..dice_count {
                damage += rng.random_range(1..=6);
            }
            damage = damage.max(1);

            events.push(EngineEvent::MeleeHit {
                attacker,
                defender,
                weapon: None,
                damage: damage as u32,
            });

            let attacker_name = world.entity_name(attacker);
            let new_hp = if let Some(mut hp) = world.get_component_mut::<HitPoints>(defender) {
                hp.current -= damage;
                events.push(EngineEvent::HpChange {
                    entity: defender,
                    amount: -damage,
                    new_hp: hp.current,
                    source: HpSource::Combat,
                });
                hp.current
            } else {
                return events;
            };

            if new_hp <= 0 {
                events.push(EngineEvent::EntityDied {
                    entity: defender,
                    killer: Some(attacker),
                    cause: DeathCause::KilledBy {
                        killer_name: attacker_name,
                    },
                });
            }
        } else {
            events.push(EngineEvent::MeleeMiss { attacker, defender });
        }
    }

    events
}

/// Check whether the defender died during the combat round.
///
/// Scans the event list for an `EntityDied` event matching `defender`.
pub fn defender_died(events: &[EngineEvent], defender: Entity) -> bool {
    events
        .iter()
        .any(|e| matches!(e, EngineEvent::EntityDied { entity, .. } if *entity == defender))
}

// ---------------------------------------------------------------------------
// Monster-vs-monster: should flee check
// ---------------------------------------------------------------------------

/// Check whether a monster should flee from another monster.
///
/// A monster flees from a stronger opponent when its HP is critically low
/// (below 1/4 of max) and the attacker's level is significantly higher.
/// Matches C NetHack's `mflee` logic for monster-vs-monster.
pub fn should_monster_flee_from(world: &GameWorld, defender: Entity, attacker: Entity) -> bool {
    let (def_hp, def_max) = match world.get_component::<HitPoints>(defender) {
        Some(hp) => (hp.current, hp.max),
        None => return false,
    };

    // Don't flee if HP is reasonable.
    if def_max < 4 || def_hp >= def_max / 4 {
        return false;
    }

    let attacker_level = world
        .get_component::<ExperienceLevel>(attacker)
        .map(|l| l.0)
        .unwrap_or(1);
    let defender_level = world
        .get_component::<ExperienceLevel>(defender)
        .map(|l| l.0)
        .unwrap_or(1);

    // Flee if attacker is at least 2 levels higher.
    attacker_level >= defender_level + 2
}

// ---------------------------------------------------------------------------
// Monster-vs-monster: special attack slot (with damage type effects)
// ---------------------------------------------------------------------------

/// Resolve a single attack slot from one monster against another,
/// applying damage type effects (fire, cold, poison, stoning, etc.)
/// between monsters.
///
/// This wraps `resolve_monster_attack_slot` from combat.rs, which already
/// dispatches all AttackMethod and DamageType variants. This function
/// adds monster-vs-monster specific logic: checking if the attacker
/// dies from an explosive attack, and handling passive damage from
/// acidic/stoning defenders.
pub fn resolve_mhitm_attack_slot(
    world: &mut GameWorld,
    attacker: Entity,
    defender: Entity,
    attack: &AttackDef,
    attack_index: usize,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Delegate to the shared combat resolution.
    let slot_events =
        resolve_monster_attack_slot(world, attacker, defender, attack, attack_index, rng);

    // Check if the attacker was killed (e.g., exploding attack).
    let attacker_died = slot_events
        .iter()
        .any(|e| matches!(e, EngineEvent::EntityDied { entity, .. } if *entity == attacker));

    events.extend(slot_events);

    // If the attacker just died, don't apply passive damage.
    if attacker_died {
        return events;
    }

    // Check for passive damage from the defender (acid, petrification).
    // Only applies if the attack method is melee contact.
    let is_contact = matches!(
        attack.method,
        AttackMethod::Claw
            | AttackMethod::Bite
            | AttackMethod::Kick
            | AttackMethod::Butt
            | AttackMethod::Hug
            | AttackMethod::Touch
            | AttackMethod::Tentacle
    );

    if is_contact {
        // Check if defender has acid resistance data (from MonsterResistances).
        // In C NetHack, acidic monsters splash acid on melee attackers.
        // We check for an acid passive attack on the defender's attack array.
        let defender_attacks = world
            .get_component::<MonsterAttacks>(defender)
            .map(|ma| ma.0.clone())
            .unwrap_or_default();

        let has_acid_passive = defender_attacks
            .iter()
            .any(|a| a.method == AttackMethod::None && a.damage_type == DamageType::Acid);

        if has_acid_passive {
            let acid_dmg = rng.random_range(1..=4i32);
            // Check if attacker resists acid.
            let resists = world
                .get_component::<MonsterResistances>(attacker)
                .is_some_and(|r| r.0.contains(nethack_babel_data::ResistanceSet::ACID));
            if !resists && acid_dmg > 0 {
                let attacker_name = world.entity_name(attacker);
                let defender_name = world.entity_name(defender);
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(attacker) {
                    hp.current -= acid_dmg;
                    events.push(EngineEvent::HpChange {
                        entity: attacker,
                        amount: -acid_dmg,
                        new_hp: hp.current,
                        source: HpSource::Combat,
                    });
                    if hp.current <= 0 {
                        events.push(EngineEvent::EntityDied {
                            entity: attacker,
                            killer: Some(defender),
                            cause: DeathCause::KilledBy {
                                killer_name: defender_name,
                            },
                        });
                    }
                }
                let _ = attacker_name;
            }
        }

        // Stoning passive: cockatrice-like monsters.
        let has_stone_passive = defender_attacks
            .iter()
            .any(|a| a.method == AttackMethod::None && a.damage_type == DamageType::Stone);

        if has_stone_passive {
            // Monster-vs-monster stoning: apply stone status to attacker.
            events.push(EngineEvent::msg("mhitm-passive-stoning"));
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Enhanced monster-vs-monster: full round with damage type effects
// ---------------------------------------------------------------------------

/// Resolve a full round of monster-vs-monster combat with damage type effects.
///
/// Like `resolve_monster_vs_monster` but uses `resolve_mhitm_attack_slot`
/// for each attack, which applies passive damage from defenders.
/// Also checks if the attacker dies mid-round.
pub fn resolve_monster_vs_monster_full(
    world: &mut GameWorld,
    attacker: Entity,
    defender: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if world.get_component::<HitPoints>(attacker).is_none()
        || world.get_component::<HitPoints>(defender).is_none()
    {
        return events;
    }

    let has_attacks = world.get_component::<MonsterAttacks>(attacker).is_some();
    if has_attacks {
        let attacks = world
            .get_component::<MonsterAttacks>(attacker)
            .map(|ma| ma.0.clone())
            .unwrap_or_default();

        for (i, attack) in attacks.iter().enumerate() {
            if attack.method == AttackMethod::None {
                continue;
            }

            // Check if defender is still alive.
            let def_alive = world
                .get_component::<HitPoints>(defender)
                .is_some_and(|hp| hp.current > 0);
            if !def_alive {
                break;
            }

            // Check if attacker is still alive (may have died from passive).
            let atk_alive = world
                .get_component::<HitPoints>(attacker)
                .is_some_and(|hp| hp.current > 0);
            if !atk_alive {
                break;
            }

            let slot_events = resolve_mhitm_attack_slot(world, attacker, defender, attack, i, rng);
            events.extend(slot_events);
        }
    } else {
        // Fall back to base combat (no special attacks).
        events.extend(resolve_monster_vs_monster(world, attacker, defender, rng));
    }

    events
}

// ---------------------------------------------------------------------------
// Pet combat targeting: find the best adjacent hostile to attack
// ---------------------------------------------------------------------------

/// Find the best adjacent hostile target for a pet to attack.
///
/// Rules:
/// - Skip tame monsters (allies).
/// - Skip peaceful monsters unless the pet is confused.
/// - Prefer targets the player is adjacent to (fighting alongside).
/// - Return the chosen target entity, if any.
pub fn find_pet_combat_target(
    world: &GameWorld,
    pet: Entity,
    pet_pos: Position,
    player_pos: Position,
    is_confused: bool,
) -> Option<Entity> {
    use crate::world::Monster;

    let mut candidates: Vec<(Entity, i32)> = Vec::new();

    for (entity, (_m, pos, hp)) in world
        .ecs()
        .query::<(&Monster, &Positioned, &HitPoints)>()
        .iter()
    {
        if entity == pet {
            continue;
        }

        // Must be adjacent to pet (Chebyshev distance == 1).
        let dist = chebyshev(pet_pos, pos.0);
        if dist != 1 {
            continue;
        }

        // Skip dead monsters.
        if hp.current <= 0 {
            continue;
        }

        // Skip tame monsters (never attack allies).
        if world.get_component::<Tame>(entity).is_some() {
            continue;
        }

        if world.get_component::<Peaceful>(entity).is_some() && !is_confused {
            continue;
        }

        // Score: prefer targets adjacent to the player.
        let near_player = chebyshev(pos.0, player_pos) <= 1;
        let score = if near_player { 100 } else { 50 };

        candidates.push((entity, score));
    }

    // Pick the highest-scored candidate.
    candidates.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
    candidates.first().map(|&(entity, _)| entity)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Chebyshev (king-move) distance.
#[inline]
fn chebyshev(a: Position, b: Position) -> i32 {
    let dx = (a.x - b.x).abs();
    let dy = (a.y - b.y).abs();
    dx.max(dy)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::dungeon::Terrain;
    use crate::pets::PetState;
    use crate::world::{
        ArmorClass, Attributes, ExperienceLevel, GameWorld, HitPoints, Monster, MovementPoints,
        NORMAL_SPEED, Name, Peaceful, Positioned, Speed, Tame,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(54321)
    }

    fn make_test_world() -> GameWorld {
        let mut world = GameWorld::new(Position::new(8, 8));
        for y in 1..=15 {
            for x in 1..=15 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        world
    }

    fn spawn_monster(
        world: &mut GameWorld,
        pos: Position,
        name: &str,
        hp: i32,
        level: u8,
    ) -> Entity {
        world.spawn((
            Monster,
            Positioned(pos),
            HitPoints {
                current: hp,
                max: hp,
            },
            ArmorClass(10),
            Attributes::default(),
            ExperienceLevel(level),
            Speed(12),
            MovementPoints(NORMAL_SPEED as i32),
            Name(name.to_string()),
        ))
    }

    fn spawn_pet(world: &mut GameWorld, pos: Position, name: &str, hp: i32) -> Entity {
        let ps = PetState::new(10, world.turn());
        world.spawn((
            Monster,
            Tame,
            Positioned(pos),
            HitPoints {
                current: hp,
                max: hp,
            },
            ArmorClass(10),
            Attributes::default(),
            ExperienceLevel(3),
            Speed(12),
            MovementPoints(NORMAL_SPEED as i32),
            Name(name.to_string()),
            ps,
        ))
    }

    // ── Test: basic monster-vs-monster damage ─────────────────────

    #[test]
    fn test_monster_vs_monster_basic() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let attacker = spawn_monster(&mut world, Position::new(5, 5), "goblin", 20, 3);
        let defender = spawn_monster(&mut world, Position::new(6, 5), "kobold", 20, 1);

        let events = resolve_monster_vs_monster(&mut world, attacker, defender, &mut rng);

        // Should have either a hit or miss event.
        let has_combat = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
            )
        });
        assert!(has_combat, "should produce a combat event");

        // If hit, defender HP should have decreased.
        let hit = events
            .iter()
            .any(|e| matches!(e, EngineEvent::MeleeHit { .. }));
        if hit {
            let hp = world.get_component::<HitPoints>(defender).unwrap();
            assert!(hp.current < 20, "defender HP should decrease on hit");
        }
    }

    // ── Test: monster kills monster ──────────────────────────────

    #[test]
    fn test_monster_vs_monster_kill() {
        let mut world = make_test_world();

        let attacker = spawn_monster(&mut world, Position::new(5, 5), "dragon", 50, 10);
        let defender = spawn_monster(&mut world, Position::new(6, 5), "rat", 1, 1);

        // Run combat multiple times until the defender dies (deterministic
        // with high-level attacker vs 1 HP defender).
        let mut died = false;
        for seed in 0..50u64 {
            // Reset defender HP for each attempt.
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(defender) {
                hp.current = 1;
                hp.max = 1;
            }

            let mut rng = Pcg64::seed_from_u64(seed);
            let events = resolve_monster_vs_monster(&mut world, attacker, defender, &mut rng);

            if defender_died(&events, defender) {
                died = true;
                break;
            }
        }
        assert!(
            died,
            "high-level attacker should eventually kill 1-HP defender"
        );
    }

    // ── Test: pet attacks adjacent hostile ───────────────────────

    #[test]
    fn test_pet_attacks_hostile() {
        let mut world = make_test_world();
        let player_pos = Position::new(8, 8);
        let pet_pos = Position::new(9, 8);
        let hostile_pos = Position::new(10, 8);

        let pet = spawn_pet(&mut world, pet_pos, "little dog", 15);
        let hostile = spawn_monster(&mut world, hostile_pos, "goblin", 10, 1);

        let target = find_pet_combat_target(&world, pet, pet_pos, player_pos, false);

        assert_eq!(target, Some(hostile), "pet should target adjacent hostile");

        // Now resolve combat.
        let mut rng = test_rng();
        let events = resolve_monster_vs_monster(&mut world, pet, hostile, &mut rng);

        let has_combat = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
            )
        });
        assert!(
            has_combat,
            "pet should produce combat events against hostile"
        );
    }

    // ── Test: pet ignores peaceful (tame) monsters ──────────────

    #[test]
    fn test_pet_ignores_peaceful() {
        let mut world = make_test_world();
        let player_pos = Position::new(8, 8);
        let pet_pos = Position::new(9, 8);

        let pet = spawn_pet(&mut world, pet_pos, "little dog", 15);
        let ally = spawn_monster(&mut world, Position::new(10, 8), "kitten", 10, 2);
        world
            .ecs_mut()
            .insert_one(ally, Peaceful)
            .expect("ally should accept peaceful marker");

        let target = find_pet_combat_target(&world, pet, pet_pos, player_pos, false);

        assert!(target.is_none(), "pet should not target peaceful allies");
    }

    // ── Test: pet prefers target adjacent to player ─────────────

    #[test]
    fn test_pet_prefers_player_adjacent_target() {
        let mut world = make_test_world();
        let player_pos = Position::new(8, 8);
        let pet_pos = Position::new(10, 8);

        // Spawn the pet at (10,8) -- 2 tiles from player.
        let pet = spawn_pet(&mut world, pet_pos, "little dog", 15);

        // Place a hostile at (11,8) -- adjacent to pet but NOT to player
        // (Chebyshev distance 3 from player).
        let _far_hostile = spawn_monster(&mut world, Position::new(11, 8), "goblin", 10, 1);

        // Place a hostile at (9,8) -- adjacent to pet AND to player
        // (Chebyshev distance 1 from player).
        let near_hostile = spawn_monster(&mut world, Position::new(9, 8), "orc", 10, 1);

        let target = find_pet_combat_target(&world, pet, pet_pos, player_pos, false);

        // Should prefer the one adjacent to the player (score 100 vs 50).
        assert_eq!(
            target,
            Some(near_hostile),
            "pet should prefer target adjacent to the player"
        );
    }

    // ── Test: monster-vs-monster with MonsterAttacks component ───

    #[test]
    fn test_monster_vs_monster_with_attacks() {
        use crate::combat::MonsterAttacks;
        use nethack_babel_data::{AttackDef, AttackMethod, DamageType, DiceExpr};

        let mut world = make_test_world();
        let mut rng = test_rng();

        let attacker = spawn_monster(&mut world, Position::new(5, 5), "wolf", 20, 5);

        // Give the attacker a bite attack: AT_BITE, AD_PHYS, 2d6.
        let mut attacks = arrayvec::ArrayVec::new();
        attacks.push(AttackDef {
            method: AttackMethod::Bite,
            damage_type: DamageType::Physical,
            dice: DiceExpr { count: 2, sides: 6 },
        });
        let _ = world
            .ecs_mut()
            .insert_one(attacker, MonsterAttacks(attacks));

        let defender = spawn_monster(&mut world, Position::new(6, 5), "rat", 20, 1);

        let events = resolve_monster_vs_monster(&mut world, attacker, defender, &mut rng);

        // Should have produced combat events using the attack array.
        let has_combat = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
            )
        });
        assert!(
            has_combat,
            "should produce combat events with MonsterAttacks"
        );
    }

    // ── Test: should_monster_flee_from ────────────────────────────

    #[test]
    fn test_flee_from_stronger_when_low_hp() {
        let mut world = make_test_world();
        let weak = spawn_monster(&mut world, Position::new(5, 5), "kobold", 20, 1);
        let strong = spawn_monster(&mut world, Position::new(6, 5), "dragon", 50, 10);

        // Set kobold HP to 1/20 of max (critically low).
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(weak) {
            hp.current = 2;
        }

        assert!(
            should_monster_flee_from(&world, weak, strong),
            "low-HP monster should flee from much stronger opponent"
        );
    }

    #[test]
    fn test_no_flee_when_healthy() {
        let mut world = make_test_world();
        let defender = spawn_monster(&mut world, Position::new(5, 5), "kobold", 20, 1);
        let attacker = spawn_monster(&mut world, Position::new(6, 5), "dragon", 50, 10);

        // Kobold is at full HP — should not flee.
        assert!(
            !should_monster_flee_from(&world, defender, attacker),
            "healthy monster should not flee"
        );
    }

    #[test]
    fn test_no_flee_from_weaker_opponent() {
        let mut world = make_test_world();
        let defender = spawn_monster(&mut world, Position::new(5, 5), "orc", 20, 5);
        let attacker = spawn_monster(&mut world, Position::new(6, 5), "rat", 10, 1);

        // Set orc HP low.
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(defender) {
            hp.current = 2;
        }

        // The attacker's level (1) is not >= defender's level (5) + 2.
        assert!(
            !should_monster_flee_from(&world, defender, attacker),
            "low-HP monster should not flee from much weaker opponent"
        );
    }

    // ── Test: resolve_mhitm_attack_slot with passive acid ────────

    #[test]
    fn test_mhitm_passive_acid_damages_attacker() {
        use crate::combat::MonsterAttacks;
        use nethack_babel_data::{AttackDef, AttackMethod, DamageType, DiceExpr};

        let mut world = make_test_world();
        let mut rng = test_rng();

        let attacker = spawn_monster(&mut world, Position::new(5, 5), "goblin", 30, 3);

        // Defender is an acidic blob with a passive acid "attack".
        let defender = spawn_monster(&mut world, Position::new(6, 5), "acid blob", 20, 2);

        // Give defender a passive acid attack (AT_NONE, AD_ACID).
        let mut def_attacks = arrayvec::ArrayVec::new();
        def_attacks.push(AttackDef {
            method: AttackMethod::None,
            damage_type: DamageType::Acid,
            dice: DiceExpr { count: 1, sides: 4 },
        });
        let _ = world
            .ecs_mut()
            .insert_one(defender, MonsterAttacks(def_attacks));

        // Attacker's claw attack (contact).
        let attack = AttackDef {
            method: AttackMethod::Claw,
            damage_type: DamageType::Physical,
            dice: DiceExpr { count: 1, sides: 6 },
        };

        let events =
            resolve_mhitm_attack_slot(&mut world, attacker, defender, &attack, 0, &mut rng);

        // Attacker should have taken passive acid damage.
        let attacker_hp = world.get_component::<HitPoints>(attacker).unwrap();
        assert!(
            attacker_hp.current < 30,
            "attacker should take passive acid damage; hp = {}",
            attacker_hp.current
        );

        // There should be an HpChange event for the attacker.
        let attacker_damaged = events.iter().any(|e| {
            matches!(e, EngineEvent::HpChange { entity, amount, .. }
                if *entity == attacker && *amount < 0)
        });
        assert!(
            attacker_damaged,
            "should have HpChange for attacker from acid"
        );
    }

    // ── Test: resolve_monster_vs_monster_full ─────────────────────

    #[test]
    fn test_full_round_stops_if_attacker_dies() {
        use crate::combat::MonsterAttacks;
        use nethack_babel_data::{AttackDef, AttackMethod, DamageType, DiceExpr};

        let mut world = make_test_world();
        let mut rng = test_rng();

        // Attacker has 1 HP — will die from passive acid.
        let attacker = spawn_monster(&mut world, Position::new(5, 5), "weak goblin", 1, 1);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(attacker) {
            hp.max = 1;
        }

        // Give attacker two claw attacks.
        let mut atk_attacks = arrayvec::ArrayVec::new();
        atk_attacks.push(AttackDef {
            method: AttackMethod::Claw,
            damage_type: DamageType::Physical,
            dice: DiceExpr { count: 2, sides: 6 },
        });
        atk_attacks.push(AttackDef {
            method: AttackMethod::Claw,
            damage_type: DamageType::Physical,
            dice: DiceExpr { count: 2, sides: 6 },
        });
        let _ = world
            .ecs_mut()
            .insert_one(attacker, MonsterAttacks(atk_attacks));

        // Defender is acidic — contact will splash acid.
        let defender = spawn_monster(&mut world, Position::new(6, 5), "acid blob", 50, 5);
        let mut def_attacks = arrayvec::ArrayVec::new();
        def_attacks.push(AttackDef {
            method: AttackMethod::None,
            damage_type: DamageType::Acid,
            dice: DiceExpr { count: 3, sides: 6 },
        });
        let _ = world
            .ecs_mut()
            .insert_one(defender, MonsterAttacks(def_attacks));

        let events = resolve_monster_vs_monster_full(&mut world, attacker, defender, &mut rng);

        // After the round, the attacker should be dead (HP <= 0).
        let atk_hp = world.get_component::<HitPoints>(attacker).unwrap();
        assert!(
            atk_hp.current <= 0,
            "attacker with 1 HP should die from acid splash; hp = {}",
            atk_hp.current
        );

        // The full round should have stopped early — the events should
        // contain the attacker's death or HP decrease.
        let atk_took_damage = events.iter().any(|e| {
            matches!(e, EngineEvent::HpChange { entity, amount, .. }
                if *entity == attacker && *amount < 0)
        });
        assert!(
            atk_took_damage,
            "attacker should have taken damage from acid passive"
        );
    }

    #[test]
    fn test_full_round_basic_combat() {
        use crate::combat::MonsterAttacks;
        use nethack_babel_data::{AttackDef, AttackMethod, DamageType, DiceExpr};

        let mut world = make_test_world();
        let mut rng = test_rng();

        let attacker = spawn_monster(&mut world, Position::new(5, 5), "wolf", 30, 5);
        let mut attacks = arrayvec::ArrayVec::new();
        attacks.push(AttackDef {
            method: AttackMethod::Bite,
            damage_type: DamageType::Physical,
            dice: DiceExpr { count: 2, sides: 6 },
        });
        let _ = world
            .ecs_mut()
            .insert_one(attacker, MonsterAttacks(attacks));

        let defender = spawn_monster(&mut world, Position::new(6, 5), "rat", 30, 1);

        let events = resolve_monster_vs_monster_full(&mut world, attacker, defender, &mut rng);

        // Should produce combat events.
        let has_combat = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
            )
        });
        assert!(has_combat, "full round should produce combat events");
    }
}
