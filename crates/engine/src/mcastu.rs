//! Monster spellcasting (from C `mcastu.c`).
//!
//! Provides `castmu()` — the primary entry point for a monster casting a
//! spell at the player.  Spell selection is level-gated: higher-level
//! monsters access more powerful spells (mage or cleric line).
//!
//! The spell enums and selection functions live in `monster_ai` (where
//! they were originally implemented).  This module re-exports them and
//! provides the `castmu()` wrapper expected by the task spec.
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;

use crate::event::{EngineEvent, HpSource, StatusEffect};
use crate::monster_ai::Spellcaster;
use crate::world::{GameWorld, HitPoints};

// Re-export the spell types so consumers can `use mcastu::MageSpell` etc.
pub use crate::monster_ai::{ClericSpell, MageSpell, choose_cleric_spell, choose_mage_spell};

// ---------------------------------------------------------------------------
// castmu — primary entry point
// ---------------------------------------------------------------------------

/// A monster casts a spell at the player.
///
/// Mirrors C `castmu(mtmp, mattk, thinks_it_requests_something)`.
/// The monster chooses a spell from the mage or cleric table based on its
/// level, then applies the effect.  Returns events describing what happened.
///
/// Requires the monster to have a `Spellcaster` component.
pub fn castmu(
    world: &mut GameWorld,
    monster: Entity,
    player: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let caster = match world.get_component::<Spellcaster>(monster) {
        Some(c) => *c,
        None => return events,
    };

    let mon_name = world.entity_name(monster);

    if caster.is_cleric {
        let spell = choose_cleric_spell(caster.monster_level, rng);
        events.push(EngineEvent::msg_with(
            "monster-casts-cleric",
            vec![
                ("monster", mon_name.clone()),
                ("spell", format!("{spell:?}")),
            ],
        ));
        apply_cleric_spell(
            world,
            monster,
            player,
            spell,
            caster.monster_level,
            rng,
            &mut events,
        );
    } else {
        let spell = choose_mage_spell(caster.monster_level, rng);
        events.push(EngineEvent::msg_with(
            "monster-casts-mage",
            vec![
                ("monster", mon_name.clone()),
                ("spell", format!("{spell:?}")),
            ],
        ));
        apply_mage_spell(
            world,
            monster,
            player,
            spell,
            caster.monster_level,
            rng,
            &mut events,
        );
    }

    events
}

// ---------------------------------------------------------------------------
// Cleric spell effects
// ---------------------------------------------------------------------------

fn apply_cleric_spell(
    world: &mut GameWorld,
    monster: Entity,
    player: Entity,
    spell: ClericSpell,
    monster_level: u8,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    match spell {
        ClericSpell::OpenWounds => {
            // d(3, monster_level) damage.
            let sides = (monster_level as u32).max(1);
            let damage: u32 = (0..3u32).map(|_| rng.random_range(1..=sides)).sum();
            deal_damage(world, player, damage, HpSource::Spell, events);
        }
        ClericSpell::CureSelf => {
            // Heal d(3,6) + 6 HP on self.
            let heal: i32 = (0..3).map(|_| rng.random_range(1i32..=6)).sum::<i32>() + 6;
            heal_monster(world, monster, heal, events);
        }
        ClericSpell::ConfuseYou => {
            let dur = rng.random_range(5u32..=25);
            events.push(EngineEvent::StatusApplied {
                entity: player,
                status: StatusEffect::Confused,
                duration: Some(dur),
                source: Some(monster),
            });
        }
        ClericSpell::Paralyze => {
            let dur = rng.random_range(1u32..=6);
            events.push(EngineEvent::StatusApplied {
                entity: player,
                status: StatusEffect::Paralyzed,
                duration: Some(dur),
                source: Some(monster),
            });
        }
        ClericSpell::BlindYou => {
            let dur = rng.random_range(10u32..=50);
            events.push(EngineEvent::StatusApplied {
                entity: player,
                status: StatusEffect::Blind,
                duration: Some(dur),
                source: Some(monster),
            });
        }
        ClericSpell::Insects => {
            // Summon insects — placeholder event.
            events.push(EngineEvent::msg("spell-summon-insects"));
        }
        ClericSpell::CurseItems => {
            // Curse random player items — placeholder event.
            events.push(EngineEvent::msg("spell-curse-items"));
        }
        ClericSpell::Lightning => {
            // 4d6 lightning damage.
            let damage: u32 = (0..4u32).map(|_| rng.random_range(1u32..=6)).sum();
            deal_damage(world, player, damage, HpSource::Spell, events);
        }
        ClericSpell::FirePillar => {
            // 4d6 fire damage.
            let damage: u32 = (0..4u32).map(|_| rng.random_range(1u32..=6)).sum();
            deal_damage(world, player, damage, HpSource::Spell, events);
        }
        ClericSpell::Geyser => {
            // 6d6 water damage + stun.
            let damage: u32 = (0..6u32).map(|_| rng.random_range(1u32..=6)).sum();
            deal_damage(world, player, damage, HpSource::Spell, events);
            let stun_dur = rng.random_range(1u32..=5);
            events.push(EngineEvent::StatusApplied {
                entity: player,
                status: StatusEffect::Stunned,
                duration: Some(stun_dur),
                source: Some(monster),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Mage spell effects
// ---------------------------------------------------------------------------

fn apply_mage_spell(
    world: &mut GameWorld,
    monster: Entity,
    player: Entity,
    spell: MageSpell,
    monster_level: u8,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    match spell {
        MageSpell::PsiBolt => {
            // d(3, monster_level) psychic damage.
            let sides = (monster_level as u32).max(1);
            let damage: u32 = (0..3u32).map(|_| rng.random_range(1..=sides)).sum();
            deal_damage(world, player, damage, HpSource::Spell, events);
        }
        MageSpell::CureSelf => {
            let heal: i32 = (0..3).map(|_| rng.random_range(1i32..=6)).sum::<i32>() + 6;
            heal_monster(world, monster, heal, events);
        }
        MageSpell::HasteSelf => {
            let dur = rng.random_range(1u32..=50) + rng.random_range(1u32..=50);
            events.push(EngineEvent::StatusApplied {
                entity: monster,
                status: StatusEffect::FastSpeed,
                duration: Some(dur),
                source: None,
            });
        }
        MageSpell::StunYou => {
            let dur = rng.random_range(1u32..=8);
            events.push(EngineEvent::StatusApplied {
                entity: player,
                status: StatusEffect::Stunned,
                duration: Some(dur),
                source: Some(monster),
            });
        }
        MageSpell::Disappear => {
            events.push(EngineEvent::StatusApplied {
                entity: monster,
                status: StatusEffect::Invisible,
                duration: Some(200),
                source: None,
            });
        }
        MageSpell::WeakenYou => {
            // Reduce player STR by 1 — placeholder event.
            events.push(EngineEvent::msg("spell-weaken"));
        }
        MageSpell::DestroyArmor => {
            events.push(EngineEvent::msg("spell-destroy-armor"));
        }
        MageSpell::CurseItems => {
            events.push(EngineEvent::msg("spell-curse-items"));
        }
        MageSpell::Aggravation => {
            events.push(EngineEvent::msg("spell-aggravation"));
        }
        MageSpell::SummonMonster => {
            events.push(EngineEvent::msg("spell-summon-monster"));
        }
        MageSpell::DeathTouch => {
            // 8d6 massive damage.
            let damage: u32 = (0..8u32).map(|_| rng.random_range(1u32..=6)).sum();
            deal_damage(world, player, damage, HpSource::Spell, events);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Deal `damage` HP to an entity and emit the corresponding `HpChange` event.
fn deal_damage(
    world: &mut GameWorld,
    target: Entity,
    damage: u32,
    source: HpSource,
    events: &mut Vec<EngineEvent>,
) {
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(target) {
        hp.current -= damage as i32;
        events.push(EngineEvent::HpChange {
            entity: target,
            amount: -(damage as i32),
            new_hp: hp.current,
            source,
        });
    }
}

/// Heal a monster and emit `HpChange` if any healing actually occurred.
fn heal_monster(world: &mut GameWorld, monster: Entity, heal: i32, events: &mut Vec<EngineEvent>) {
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(monster) {
        let old = hp.current;
        hp.current = (hp.current + heal).min(hp.max);
        let actual = hp.current - old;
        if actual > 0 {
            events.push(EngineEvent::HpChange {
                entity: monster,
                amount: actual,
                new_hp: hp.current,
                source: HpSource::Spell,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::monster_ai::Spellcaster;
    use crate::world::{
        ArmorClass, Attributes, ExperienceLevel, HitPoints, Monster, MovementPoints, NORMAL_SPEED,
        Name, Positioned, Speed,
    };
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    fn make_test_world() -> GameWorld {
        let mut world = GameWorld::new(Position::new(8, 8));
        for y in 1..=15 {
            for x in 1..=15 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), crate::dungeon::Terrain::Floor);
            }
        }
        world
    }

    fn spawn_caster(world: &mut GameWorld, pos: Position, level: u8, is_cleric: bool) -> Entity {
        world.spawn((
            Monster,
            Positioned(pos),
            HitPoints {
                current: 40,
                max: 40,
            },
            ArmorClass(5),
            Attributes::default(),
            ExperienceLevel(level),
            Speed(12),
            MovementPoints(NORMAL_SPEED as i32),
            Name("spellcaster".to_string()),
            Spellcaster {
                monster_level: level,
                is_cleric,
            },
        ))
    }

    // ── castmu basic ─────────────────────────────────────────────

    #[test]
    fn castmu_requires_spellcaster_component() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        // Monster without Spellcaster component.
        let monster = world.spawn((
            Monster,
            Positioned(Position::new(12, 8)),
            HitPoints {
                current: 20,
                max: 20,
            },
            Name("dummy".to_string()),
        ));
        let events = castmu(&mut world, monster, player, &mut rng);
        assert!(events.is_empty());
    }

    #[test]
    fn castmu_mage_produces_events() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let caster = spawn_caster(&mut world, Position::new(12, 8), 10, false);

        let events = castmu(&mut world, caster, player, &mut rng);
        assert!(!events.is_empty(), "mage spell should produce events");
        // First event should be the cast message.
        assert!(
            matches!(&events[0], EngineEvent::Message { key, .. } if key == "monster-casts-mage")
        );
    }

    #[test]
    fn castmu_cleric_produces_events() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let caster = spawn_caster(&mut world, Position::new(12, 8), 10, true);

        let events = castmu(&mut world, caster, player, &mut rng);
        assert!(!events.is_empty(), "cleric spell should produce events");
        assert!(
            matches!(&events[0], EngineEvent::Message { key, .. } if key == "monster-casts-cleric")
        );
    }

    // ── Cleric spells ────────────────────────────────────────────

    #[test]
    fn cleric_open_wounds_damages_player() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 5, true);

        let mut events = Vec::new();
        apply_cleric_spell(
            &mut world,
            monster,
            player,
            ClericSpell::OpenWounds,
            5,
            &mut rng,
            &mut events,
        );

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::HpChange { amount, .. } if *amount < 0))
        );
    }

    #[test]
    fn cleric_cure_self_heals_monster() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, true);

        // Damage the monster first.
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(monster) {
            hp.current = 10;
        }

        let mut events = Vec::new();
        apply_cleric_spell(
            &mut world,
            monster,
            player,
            ClericSpell::CureSelf,
            10,
            &mut rng,
            &mut events,
        );

        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert!(hp.current > 10, "cure self should heal monster");
    }

    #[test]
    fn cleric_confuse_applies_status() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, true);

        let mut events = Vec::new();
        apply_cleric_spell(
            &mut world,
            monster,
            player,
            ClericSpell::ConfuseYou,
            10,
            &mut rng,
            &mut events,
        );

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Confused,
                ..
            }
        )));
    }

    #[test]
    fn cleric_paralyze_applies_status() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, true);

        let mut events = Vec::new();
        apply_cleric_spell(
            &mut world,
            monster,
            player,
            ClericSpell::Paralyze,
            10,
            &mut rng,
            &mut events,
        );

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Paralyzed,
                ..
            }
        )));
    }

    #[test]
    fn cleric_blind_applies_status() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, true);

        let mut events = Vec::new();
        apply_cleric_spell(
            &mut world,
            monster,
            player,
            ClericSpell::BlindYou,
            10,
            &mut rng,
            &mut events,
        );

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Blind,
                ..
            }
        )));
    }

    #[test]
    fn cleric_geyser_damages_and_stuns() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, true);

        let orig_hp = world.get_component::<HitPoints>(player).unwrap().current;

        let mut events = Vec::new();
        apply_cleric_spell(
            &mut world,
            monster,
            player,
            ClericSpell::Geyser,
            10,
            &mut rng,
            &mut events,
        );

        let new_hp = world.get_component::<HitPoints>(player).unwrap().current;
        assert!(new_hp < orig_hp, "geyser should deal damage");
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::Stunned,
                    ..
                }
            )),
            "geyser should stun"
        );
    }

    #[test]
    fn cleric_fire_pillar_damages() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, true);

        let orig_hp = world.get_component::<HitPoints>(player).unwrap().current;

        let mut events = Vec::new();
        apply_cleric_spell(
            &mut world,
            monster,
            player,
            ClericSpell::FirePillar,
            10,
            &mut rng,
            &mut events,
        );

        let new_hp = world.get_component::<HitPoints>(player).unwrap().current;
        assert!(new_hp < orig_hp, "fire pillar should deal damage");
    }

    #[test]
    fn cleric_lightning_damages() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, true);

        let orig_hp = world.get_component::<HitPoints>(player).unwrap().current;

        let mut events = Vec::new();
        apply_cleric_spell(
            &mut world,
            monster,
            player,
            ClericSpell::Lightning,
            10,
            &mut rng,
            &mut events,
        );

        let new_hp = world.get_component::<HitPoints>(player).unwrap().current;
        assert!(new_hp < orig_hp, "lightning should deal damage");
    }

    // ── Mage spells ──────────────────────────────────────────────

    #[test]
    fn mage_psi_bolt_damages() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, false);

        let orig_hp = world.get_component::<HitPoints>(player).unwrap().current;

        let mut events = Vec::new();
        apply_mage_spell(
            &mut world,
            monster,
            player,
            MageSpell::PsiBolt,
            10,
            &mut rng,
            &mut events,
        );

        let new_hp = world.get_component::<HitPoints>(player).unwrap().current;
        assert!(new_hp < orig_hp, "psi bolt should deal damage");
    }

    #[test]
    fn mage_death_touch_deals_heavy_damage() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 22, false);

        // Give the player lots of HP so we can measure damage.
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
            hp.current = 200;
            hp.max = 200;
        }

        let mut events = Vec::new();
        apply_mage_spell(
            &mut world,
            monster,
            player,
            MageSpell::DeathTouch,
            22,
            &mut rng,
            &mut events,
        );

        let new_hp = world.get_component::<HitPoints>(player).unwrap().current;
        // 8d6 minimum 8, maximum 48.
        assert!(new_hp < 200, "death touch should deal damage");
        assert!(
            new_hp <= 200 - 8,
            "death touch should deal at least 8 damage"
        );
    }

    #[test]
    fn mage_haste_self_speeds_monster() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, false);

        let mut events = Vec::new();
        apply_mage_spell(
            &mut world,
            monster,
            player,
            MageSpell::HasteSelf,
            10,
            &mut rng,
            &mut events,
        );

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                entity,
                status: StatusEffect::FastSpeed,
                ..
            } if *entity == monster
        )));
    }

    #[test]
    fn mage_stun_applies_status() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, false);

        let mut events = Vec::new();
        apply_mage_spell(
            &mut world,
            monster,
            player,
            MageSpell::StunYou,
            10,
            &mut rng,
            &mut events,
        );

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Stunned,
                ..
            }
        )));
    }

    #[test]
    fn mage_disappear_makes_invisible() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, false);

        let mut events = Vec::new();
        apply_mage_spell(
            &mut world,
            monster,
            player,
            MageSpell::Disappear,
            10,
            &mut rng,
            &mut events,
        );

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                entity,
                status: StatusEffect::Invisible,
                ..
            } if *entity == monster
        )));
    }

    #[test]
    fn mage_cure_self_heals() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let monster = spawn_caster(&mut world, Position::new(12, 8), 10, false);

        if let Some(mut hp) = world.get_component_mut::<HitPoints>(monster) {
            hp.current = 10;
        }

        let mut events = Vec::new();
        apply_mage_spell(
            &mut world,
            monster,
            player,
            MageSpell::CureSelf,
            10,
            &mut rng,
            &mut events,
        );

        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert!(hp.current > 10, "mage cure self should heal");
    }

    // ── Spell selection ──────────────────────────────────────────

    #[test]
    fn choose_mage_spell_low_level_gets_psi_bolt() {
        let mut rng = SmallRng::seed_from_u64(0);
        // Level 1: max spellval is 1, so always PsiBolt.
        let spell = choose_mage_spell(1, &mut rng);
        assert_eq!(spell, MageSpell::PsiBolt);
    }

    #[test]
    fn choose_cleric_spell_low_level_gets_open_wounds() {
        let mut rng = SmallRng::seed_from_u64(0);
        let spell = choose_cleric_spell(1, &mut rng);
        assert_eq!(spell, ClericSpell::OpenWounds);
    }

    #[test]
    fn choose_mage_spell_high_level_can_get_death_touch() {
        // With level 24, death touch is possible.
        let mut saw_death = false;
        for seed in 0..200u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            if choose_mage_spell(24, &mut rng) == MageSpell::DeathTouch {
                saw_death = true;
                break;
            }
        }
        assert!(
            saw_death,
            "level 24 mage should occasionally pick death touch"
        );
    }

    #[test]
    fn choose_cleric_spell_high_level_can_get_geyser() {
        let mut saw_geyser = false;
        for seed in 0..200u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            if choose_cleric_spell(24, &mut rng) == ClericSpell::Geyser {
                saw_geyser = true;
                break;
            }
        }
        assert!(
            saw_geyser,
            "level 24 cleric should occasionally pick geyser"
        );
    }
}
