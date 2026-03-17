//! Monster AI: pursue, flee, wander, attack, item use, covetous behavior.
//!
//! Each monster gets a turn via [`resolve_monster_turn`], which runs a
//! multi-phase decision tree:
//!
//! 1. Covetous behavior (teleport to stairs, heal, steal).
//! 2. Defensive item use (heal, teleport away).
//! 3. Enhanced flee with `mfleetim` timer.
//! 4. Offensive item use (zap wands, throw potions).
//! 5. Monster pickup (useful items on the ground).
//! 6. Ranged attack decisions (prefer ranged at distance).
//! 7. Melee attack (if adjacent).
//! 8. Pursue / wander.
//! 9. Door handling (open / break).
//!
//! All functions are pure: they take a `GameWorld` plus an RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  Zero IO.

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{MonsterFlags, ObjectClass, ObjectCore, ObjectLocation};

use crate::action::{Direction, Position};
use crate::combat::{
    MonsterAttacks, monster_ranged_attack_dispatch, resolve_melee_attack, resolve_monster_attacks,
};
use crate::dungeon::Terrain;
use crate::event::{EngineEvent, HpSource, StatusEffect};
use crate::potions::PotionType;
use crate::wands::{WandCharges, WandType};
use crate::world::{GameWorld, HitPoints, Positioned};

// ---------------------------------------------------------------------------
// Monster intelligence classification
// ---------------------------------------------------------------------------

/// Intelligence tier for a monster, derived from its flags.
/// Controls which behaviors are available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterIntelligence {
    /// No item use, basic pursue/flee.  M1_ANIMAL or M1_MINDLESS.
    Animal,
    /// Can use items, open doors.  M1_HUMANOID or has hands.
    Humanoid,
    /// Prefer ranged attacks at distance.  Spellcaster monsters.
    Spellcaster,
}

/// ECS component: monster intelligence tier (attached at spawn).
#[derive(Debug, Clone, Copy)]
pub struct Intelligence(pub MonsterIntelligence);

/// ECS component: monster flags from the species definition.
#[derive(Debug, Clone, Copy)]
pub struct MonsterSpeciesFlags(pub MonsterFlags);

/// ECS component: flee timer (turns remaining).
/// When > 0, the monster is fleeing; decremented each turn.
#[derive(Debug, Clone, Copy)]
pub struct FleeTimer(pub u8);

/// ECS component: wand type tag on wand item entities.
#[derive(Debug, Clone, Copy)]
pub struct WandTypeTag(pub WandType);

/// ECS component: potion type tag on potion item entities.
#[derive(Debug, Clone, Copy)]
pub struct PotionTypeTag(pub PotionType);

/// ECS component: covetous flag (wants the Amulet, quest artifact, etc.).
#[derive(Debug, Clone, Copy)]
pub struct Covetous;

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

/// Resolve a single monster's turn.
///
/// Called from `turn.rs` for each monster that has enough movement points
/// to act.  Returns the events produced during the monster's action.
pub fn resolve_monster_turn(
    world: &mut GameWorld,
    monster: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Bail out if the monster has no position (shouldn't happen).
    let monster_pos = match world.get_component::<Positioned>(monster) {
        Some(p) => p.0,
        None => return events,
    };

    // Find the player position.
    let player = world.player();
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Read monster HP for flee threshold.
    let (current_hp, max_hp) = match world.get_component::<HitPoints>(monster) {
        Some(hp) => (hp.current, hp.max),
        None => (1, 1),
    };

    // Read intelligence tier (default: Humanoid for backward compat).
    let intelligence = world
        .get_component::<Intelligence>(monster)
        .map(|i| i.0)
        .unwrap_or(MonsterIntelligence::Humanoid);

    // Read species flags.
    let species_flags = world
        .get_component::<MonsterSpeciesFlags>(monster)
        .map(|f| f.0)
        .unwrap_or_else(MonsterFlags::empty);

    let is_covetous = world.get_component::<Covetous>(monster).is_some()
        || species_flags.intersects(MonsterFlags::COVETOUS);

    // Check adjacency (Chebyshev distance <= 1).
    let adjacent = is_adjacent(monster_pos, player_pos);

    // Check line-of-sight visibility.
    let can_see = can_see_target(world, monster_pos, player_pos);

    let distance = chebyshev_distance(monster_pos, player_pos);

    // ── Phase 0: Teleporting monsters (Tengu, etc.) ────────────────
    // Monsters with M1_TPORT can randomly teleport each turn.
    // Per spec: 1/5 chance per turn (not cancelled).
    if species_flags.contains(MonsterFlags::TPORT) && rng.random_range(0..5) == 0 {
        if current_hp < 7 || rng.random_range(0..2) == 0 {
            // Teleport to a random position.
            if let Some(dest) = find_random_floor_tile(world, rng) {
                if let Some(mut pos) = world.get_component_mut::<Positioned>(monster) {
                    let from = pos.0;
                    pos.0 = dest;
                    events.push(EngineEvent::EntityTeleported {
                        entity: monster,
                        from,
                        to: dest,
                    });
                }
                return events;
            }
        } else {
            // Teleport to near the player (mnexto equivalent).
            let near_player = find_adjacent_floor(world, player_pos, rng);
            if let Some(dest) = near_player {
                if let Some(mut pos) = world.get_component_mut::<Positioned>(monster) {
                    let from = pos.0;
                    pos.0 = dest;
                    events.push(EngineEvent::EntityTeleported {
                        entity: monster,
                        from,
                        to: dest,
                    });
                }
                return events;
            }
        }
    }

    // ── Phase 1: Covetous behavior ─────────────────────────────────
    if is_covetous {
        let covetous_events = covetous_behavior(
            world,
            monster,
            monster_pos,
            player_pos,
            current_hp,
            max_hp,
            rng,
        );
        if !covetous_events.is_empty() {
            return covetous_events;
        }
    }

    // ── Phase 2: Determine flee state (enhanced) ───────────────────
    // Check flee state BEFORE ticking the timer (so timer=1 still flees).
    let should_flee = is_fleeing(world, monster, current_hp, max_hp, rng);

    // Tick down flee timer after checking it.
    tick_flee_timer(world, monster);

    if should_flee {
        // Defensive item use before fleeing (non-animal only).
        if intelligence != MonsterIntelligence::Animal {
            let def_events = monster_use_defensive(world, monster, rng);
            if !def_events.is_empty() {
                events.extend(def_events);
                return events;
            }
        }

        // Flee: move away from player.
        if let Some(move_events) = move_away(world, monster, monster_pos, player_pos, rng) {
            events.extend(move_events);
        }
        return events;
    }

    // ── Phase 3: Offensive item use at range (non-animal only) ─────
    if intelligence != MonsterIntelligence::Animal && can_see && !adjacent {
        let off_events = monster_use_offensive(world, monster, player, rng);
        if !off_events.is_empty() {
            events.extend(off_events);
            return events;
        }
    }

    // ── Phase 4: Ranged attack decisions ───────────────────────────
    // Monsters with breath/gaze attacks or spellcasters prefer ranged.
    if can_see && distance > 1 {
        // First check for innate ranged attacks (breath, gaze).
        if world.get_component::<MonsterAttacks>(monster).is_some() {
            let ranged_events = monster_ranged_attack_dispatch(world, monster, player_pos, rng);
            if !ranged_events.is_empty() {
                events.extend(ranged_events);
                return events;
            }
        }
        // Then check for wand/potion use (non-animal only).
        if intelligence == MonsterIntelligence::Spellcaster {
            let ranged_events = monster_ranged_attack(world, monster, player, rng);
            if !ranged_events.is_empty() {
                events.extend(ranged_events);
                return events;
            }
        }
    }

    // ── Phase 5: Melee attack ──────────────────────────────────────
    if adjacent {
        // Elbereth check: if the player is standing on an Elbereth
        // engraving and this monster is not immune, the monster refuses
        // to attack and may flee instead.
        let elbereth_active =
            crate::engrave::is_elbereth_at(&world.dungeon().engraving_map, player_pos);
        if elbereth_active {
            let is_blind = crate::status::is_blind(world, monster);
            let immune =
                crate::engrave::is_elbereth_immune(species_flags, is_blind, is_covetous, false);
            if !immune {
                // Monster is scared by Elbereth — flee instead of attacking.
                events.push(EngineEvent::msg("monster-scared-elbereth"));
                if let Some(flee_events) = move_away(world, monster, monster_pos, player_pos, rng) {
                    events.extend(flee_events);
                }
                return events;
            }
        }

        // Use full attack array if the monster has one, else fall back
        // to basic melee.
        if world.get_component::<MonsterAttacks>(monster).is_some() {
            let attack_events = resolve_monster_attacks(world, monster, player, rng);
            events.extend(attack_events);
        } else {
            resolve_melee_attack(world, monster, player, rng, &mut events);
        }
        return events;
    }

    // ── Phase 6: Door handling (non-animal) ─────────────────────────
    // Try door handling before movement.  Doors are opaque and block
    // LOS, so we check for doors toward the player even when can_see
    // is false (because the door itself blocks sight).
    if intelligence != MonsterIntelligence::Animal && distance <= 8 {
        let door_events =
            try_open_doors_toward(world, monster, monster_pos, player_pos, species_flags);
        if !door_events.is_empty() {
            events.extend(door_events);
            return events;
        }
    }

    // ── Phase 7: Pursue / wander ───────────────────────────────────
    // Re-check can_see since a door may have been opened above.
    let can_see_now = can_see_target(world, monster_pos, player_pos);
    if can_see_now {
        if let Some(move_events) = move_toward(world, monster, monster_pos, player_pos, rng) {
            events.extend(move_events);
        }
    } else {
        if let Some(move_events) = wander(world, monster, monster_pos, rng) {
            events.extend(move_events);
        }
    }

    // ── Phase 8: Monster pickup (at new position) ─────────────────
    if intelligence != MonsterIntelligence::Animal {
        let pickup_events = monster_pickup(world, monster, rng);
        events.extend(pickup_events);
    }

    events
}

// ---------------------------------------------------------------------------
// Monster regeneration
// ---------------------------------------------------------------------------

/// Apply monster regeneration per spec: 1 HP every 20 turns for normal
/// monsters, 1 HP every turn for monsters with M1_REGEN.
///
/// Called once per game turn for each monster (not per action).
pub fn monster_regen(
    world: &mut GameWorld,
    monster: Entity,
    game_turn: u32,
    events: &mut Vec<EngineEvent>,
) {
    let species_flags = world
        .get_component::<MonsterSpeciesFlags>(monster)
        .map(|f| f.0)
        .unwrap_or_else(MonsterFlags::empty);

    let should_regen = species_flags.contains(MonsterFlags::REGEN) || game_turn.is_multiple_of(20);

    if !should_regen {
        return;
    }

    let (current, max) = match world.get_component::<HitPoints>(monster) {
        Some(hp) => (hp.current, hp.max),
        None => return,
    };

    if current >= max {
        return;
    }

    let new_hp = (current + 1).min(max);
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(monster) {
        hp.current = new_hp;
    }
    events.push(EngineEvent::HpChange {
        entity: monster,
        amount: 1,
        new_hp,
        source: HpSource::Regeneration,
    });
}

// ---------------------------------------------------------------------------
// Monster item use: defensive
// ---------------------------------------------------------------------------

/// Attempt defensive item use when HP is low.
/// Checks monster inventory for healing potions and teleport wands.
pub fn monster_use_defensive(
    world: &mut GameWorld,
    monster: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let (current_hp, max_hp) = match world.get_component::<HitPoints>(monster) {
        Some(hp) => (hp.current, hp.max),
        None => return events,
    };

    // Only use defensive items when HP is below threshold.
    let threshold = max_hp / 3;
    if current_hp >= threshold || max_hp < 3 {
        return events;
    }

    // Scan inventory for defensive items.
    let monster_items = get_monster_inventory(world, monster);

    // Priority 1: Quaff healing potion (full > extra > normal).
    let potion_priorities = [
        find_potion_in_list(&monster_items, world, PotionType::FullHealing),
        find_potion_in_list(&monster_items, world, PotionType::ExtraHealing),
        find_potion_in_list(&monster_items, world, PotionType::Healing),
    ];
    if let Some(&(item, potion_type)) = potion_priorities.iter().flatten().next() {
        let heal_amount = match potion_type {
            PotionType::FullHealing => max_hp - current_hp,
            PotionType::ExtraHealing => {
                let ndice = 6u32;
                let total: u32 = (0..ndice).map(|_| rng.random_range(1u32..=8)).sum();
                total as i32 + 8
            }
            PotionType::Healing => {
                let ndice = 6u32;
                let total: u32 = (0..ndice).map(|_| rng.random_range(1u32..=4)).sum();
                total as i32 + 8
            }
            _ => 0,
        };

        // Apply healing.
        if heal_amount > 0
            && let Some(mut hp) = world.get_component_mut::<HitPoints>(monster)
        {
            let old = hp.current;
            hp.current = (hp.current + heal_amount).min(hp.max);
            let actual = hp.current - old;
            if actual > 0 {
                events.push(EngineEvent::HpChange {
                    entity: monster,
                    amount: actual,
                    new_hp: hp.current,
                    source: HpSource::Potion,
                });
            }
        }

        let mon_name = world.entity_name(monster);
        events.push(EngineEvent::msg_with(
            "monster-quaffs",
            vec![("monster", mon_name.clone())],
        ));

        // Consume the potion.
        let _ = world.despawn(item);
        return events;
    }

    // Priority 2: Zap wand of teleportation on self.
    if let Some(wand_item) = find_wand_in_list(&monster_items, world, WandType::Teleportation) {
        let has_charges = world
            .get_component::<WandCharges>(wand_item)
            .map(|c| c.spe > 0)
            .unwrap_or(false);
        if has_charges {
            // Decrement charges.
            if let Some(mut charges) = world.get_component_mut::<WandCharges>(wand_item) {
                charges.spe -= 1;
            }
            // Teleport to a random walkable position.
            let from = world.get_component::<Positioned>(monster).unwrap().0;
            if let Some(dest) = find_random_floor_tile(world, rng) {
                if let Some(mut pos) = world.get_component_mut::<Positioned>(monster) {
                    pos.0 = dest;
                }
                let mon_name = world.entity_name(monster);
                events.push(EngineEvent::EntityTeleported {
                    entity: monster,
                    from,
                    to: dest,
                });
                events.push(EngineEvent::msg_with(
                    "monster-teleport-away",
                    vec![("monster", mon_name.clone())],
                ));
            }
            return events;
        }
    }

    // Priority 3: Zap wand of digging (escape downward).
    if let Some(wand_item) = find_wand_in_list(&monster_items, world, WandType::Digging) {
        let has_charges = world
            .get_component::<WandCharges>(wand_item)
            .map(|c| c.spe > 0)
            .unwrap_or(false);
        if has_charges {
            if let Some(mut charges) = world.get_component_mut::<WandCharges>(wand_item) {
                charges.spe -= 1;
            }
            let mon_name = world.entity_name(monster);
            events.push(EngineEvent::msg_with(
                "monster-uses-wand",
                vec![
                    ("monster", mon_name.clone()),
                    ("wand_type", "digging".to_string()),
                ],
            ));
            // For now, the monster just "escapes" -- despawn it.
            let _ = world.despawn(monster);
            return events;
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Monster item use: offensive
// ---------------------------------------------------------------------------

/// Attempt offensive item use: zap wands at the player.
/// Priority: death > sleep > fire > cold > lightning > magic missile.
pub fn monster_use_offensive(
    world: &mut GameWorld,
    monster: Entity,
    player: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let monster_pos = match world.get_component::<Positioned>(monster) {
        Some(p) => p.0,
        None => return events,
    };

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Must have line of sight.
    if !can_see_target(world, monster_pos, player_pos) {
        return events;
    }

    let monster_items = get_monster_inventory(world, monster);

    // Wand priority order (offensive).
    let wand_priority = [
        WandType::Death,
        WandType::Sleep,
        WandType::Fire,
        WandType::Cold,
        WandType::Lightning,
        WandType::MagicMissile,
    ];

    for &wtype in &wand_priority {
        if let Some(wand_item) = find_wand_in_list(&monster_items, world, wtype) {
            let has_charges = world
                .get_component::<WandCharges>(wand_item)
                .map(|c| c.spe > 0)
                .unwrap_or(false);
            if !has_charges {
                continue;
            }
            // Decrement charges.
            if let Some(mut charges) = world.get_component_mut::<WandCharges>(wand_item) {
                charges.spe -= 1;
            }

            let mon_name = world.entity_name(monster);
            events.push(EngineEvent::msg_with(
                "monster-uses-wand",
                vec![
                    ("monster", mon_name.clone()),
                    ("wand_type", wtype.ray_name().to_string()),
                ],
            ));

            // Apply damage to the player based on wand type.
            let damage = match wtype {
                WandType::Death => {
                    // Instant death represented as very high damage.
                    999
                }
                WandType::Sleep => {
                    let duration = rng.random_range(1u32..=25);
                    events.push(EngineEvent::StatusApplied {
                        entity: player,
                        status: StatusEffect::Sleeping,
                        duration: Some(duration),
                        source: Some(monster),
                    });
                    0
                }
                WandType::Fire | WandType::Cold | WandType::Lightning => {
                    let nd = wtype.ray_nd();
                    let total: u32 = (0..nd).map(|_| rng.random_range(1u32..=6)).sum();
                    total
                }
                WandType::MagicMissile => {
                    let nd = wtype.ray_nd();
                    let total: u32 = (0..nd).map(|_| rng.random_range(1u32..=6)).sum();
                    total
                }
                _ => 0,
            };

            if damage > 0
                && let Some(mut hp) = world.get_component_mut::<HitPoints>(player)
            {
                hp.current -= damage as i32;
                events.push(EngineEvent::HpChange {
                    entity: player,
                    amount: -(damage as i32),
                    new_hp: hp.current,
                    source: HpSource::Combat,
                });
            }

            return events;
        }
    }

    // Throw offensive potions.
    let throw_potions = [
        PotionType::Paralysis,
        PotionType::Blindness,
        PotionType::Confusion,
        PotionType::Sleeping,
        PotionType::Acid,
    ];

    for &ptype in &throw_potions {
        if let Some((item, _)) = find_potion_in_list(&monster_items, world, ptype) {
            let mon_name = world.entity_name(monster);
            events.push(EngineEvent::msg_with(
                "monster-throws",
                vec![
                    ("monster", mon_name.clone()),
                    ("item", "potion".to_string()),
                ],
            ));

            match ptype {
                PotionType::Paralysis => {
                    let duration = rng.random_range(1u32..=10);
                    events.push(EngineEvent::StatusApplied {
                        entity: player,
                        status: StatusEffect::Paralyzed,
                        duration: Some(duration),
                        source: Some(monster),
                    });
                }
                PotionType::Blindness => {
                    let duration = rng.random_range(10u32..=50);
                    events.push(EngineEvent::StatusApplied {
                        entity: player,
                        status: StatusEffect::Blind,
                        duration: Some(duration),
                        source: Some(monster),
                    });
                }
                PotionType::Confusion => {
                    let duration = rng.random_range(5u32..=25);
                    events.push(EngineEvent::StatusApplied {
                        entity: player,
                        status: StatusEffect::Confused,
                        duration: Some(duration),
                        source: Some(monster),
                    });
                }
                PotionType::Sleeping => {
                    let duration = rng.random_range(1u32..=25);
                    events.push(EngineEvent::StatusApplied {
                        entity: player,
                        status: StatusEffect::Sleeping,
                        duration: Some(duration),
                        source: Some(monster),
                    });
                }
                PotionType::Acid => {
                    let damage: u32 = (0..2).map(|_| rng.random_range(1u32..=6)).sum();
                    if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                        hp.current -= damage as i32;
                        events.push(EngineEvent::HpChange {
                            entity: player,
                            amount: -(damage as i32),
                            new_hp: hp.current,
                            source: HpSource::Combat,
                        });
                    }
                }
                _ => {}
            }

            // Consume the potion.
            let _ = world.despawn(item);
            return events;
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Monster item use: miscellaneous
// ---------------------------------------------------------------------------

/// Attempt miscellaneous item use: quaff speed, invisibility.
pub fn monster_use_misc(
    world: &mut GameWorld,
    monster: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let _ = rng; // may be used for probability later

    let monster_items = get_monster_inventory(world, monster);

    // Quaff potion of speed.
    if let Some((item, _)) = find_potion_in_list(&monster_items, world, PotionType::Speed) {
        let mon_name = world.entity_name(monster);
        events.push(EngineEvent::StatusApplied {
            entity: monster,
            status: StatusEffect::FastSpeed,
            duration: Some(100),
            source: None,
        });
        events.push(EngineEvent::msg_with(
            "monster-quaffs",
            vec![("monster", mon_name.clone())],
        ));
        let _ = world.despawn(item);
        return events;
    }

    // Quaff potion of invisibility.
    if let Some((item, _)) = find_potion_in_list(&monster_items, world, PotionType::Invisibility) {
        let mon_name = world.entity_name(monster);
        events.push(EngineEvent::StatusApplied {
            entity: monster,
            status: StatusEffect::Invisible,
            duration: Some(200),
            source: None,
        });
        events.push(EngineEvent::msg_with(
            "monster-quaffs",
            vec![("monster", mon_name.clone())],
        ));
        let _ = world.despawn(item);
        return events;
    }

    events
}

// ---------------------------------------------------------------------------
// Monster item use: combined dispatch
// ---------------------------------------------------------------------------

/// Try defensive, then offensive, then miscellaneous item use.
pub fn monster_use_items(
    world: &mut GameWorld,
    monster: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let def = monster_use_defensive(world, monster, rng);
    if !def.is_empty() {
        return def;
    }
    let player = world.player();
    let off = monster_use_offensive(world, monster, player, rng);
    if !off.is_empty() {
        return off;
    }
    monster_use_misc(world, monster, rng)
}

// ---------------------------------------------------------------------------
// Covetous behavior
// ---------------------------------------------------------------------------

/// Covetous monster behavior: teleport to stairs when hurt, heal, return.
fn covetous_behavior(
    world: &mut GameWorld,
    monster: Entity,
    monster_pos: Position,
    player_pos: Position,
    current_hp: i32,
    max_hp: i32,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // HP ratio determines strategy (per spec section 4.5):
    //   ratio 0: HP < 33%  -> STRAT_HEAL
    //   ratio 1: HP 33-66% -> STRAT_HEAL (non-Wizard), STRAT_HEAL as default
    //   ratio 2: HP 66-99% -> STRAT_HEAL as default
    //   ratio 3: HP = 100% -> STRAT_NONE (harass)
    let ratio = if max_hp > 0 {
        (current_hp * 3) / max_hp
    } else {
        3
    };

    if ratio < 2 {
        // STRAT_HEAL: teleport to stairs and heal.
        if let Some(stairs_pos) = find_stairs(world) {
            if monster_pos != stairs_pos {
                // Teleport to stairs.
                if let Some(mut pos) = world.get_component_mut::<Positioned>(monster) {
                    pos.0 = stairs_pos;
                }
                let mon_name = world.entity_name(monster);
                events.push(EngineEvent::EntityTeleported {
                    entity: monster,
                    from: monster_pos,
                    to: stairs_pos,
                });
                events.push(EngineEvent::msg_with(
                    "monster-teleport-away",
                    vec![("monster", mon_name.clone())],
                ));
                return events;
            } else {
                // Already at stairs: heal if far from player.
                let dist2 = dist2_positions(monster_pos, player_pos);
                if dist2 > 64 && current_hp <= max_hp - 8 {
                    let heal = rng.random_range(1i32..=8);
                    if let Some(mut hp) = world.get_component_mut::<HitPoints>(monster) {
                        let old = hp.current;
                        hp.current = (hp.current + heal).min(hp.max);
                        let actual = hp.current - old;
                        if actual > 0 {
                            events.push(EngineEvent::HpChange {
                                entity: monster,
                                amount: actual,
                                new_hp: hp.current,
                                source: HpSource::Regeneration,
                            });
                        }
                    }
                    return events;
                }
            }
        }
    } else if ratio == 3 {
        // STRAT_NONE: harass -- 1/5 chance teleport to player.
        if rng.random_range(0..5) == 0
            && let Some(dest) = find_adjacent_floor(world, player_pos, rng)
        {
            let teleported = if let Some(mut pos) = world.get_component_mut::<Positioned>(monster) {
                let from = pos.0;
                pos.0 = dest;
                events.push(EngineEvent::EntityTeleported {
                    entity: monster,
                    from,
                    to: dest,
                });
                true
            } else {
                false
            };
            if teleported {
                let mon_name = world.entity_name(monster);
                events.push(EngineEvent::msg_with(
                    "monster-teleport-near",
                    vec![("monster", mon_name.clone())],
                ));
                return events;
            }
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Monster door handling
// ---------------------------------------------------------------------------

/// Attempt to open a door at `door_pos`.
/// Intelligent monsters can open doors; giants can break them.
pub fn try_open_door(
    world: &mut GameWorld,
    monster: Entity,
    door_pos: Position,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let species_flags = world
        .get_component::<MonsterSpeciesFlags>(monster)
        .map(|f| f.0)
        .unwrap_or_else(MonsterFlags::empty);

    let intelligence = world
        .get_component::<Intelligence>(monster)
        .map(|i| i.0)
        .unwrap_or(MonsterIntelligence::Animal);

    let terrain = match world.dungeon().current_level.get(door_pos) {
        Some(cell) => cell.terrain,
        None => return events,
    };

    match terrain {
        Terrain::DoorClosed => {
            // Intelligent monsters can open closed doors.
            if intelligence != MonsterIntelligence::Animal
                && !species_flags.contains(MonsterFlags::NOHANDS)
            {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(door_pos, Terrain::DoorOpen);
                let mon_name = world.entity_name(monster);
                events.push(EngineEvent::DoorOpened { position: door_pos });
                events.push(EngineEvent::msg_with("monster-open-door", vec![("monster", mon_name.clone())]));
            } else if species_flags.contains(MonsterFlags::GIANT) {
                // Giants can break doors.
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(door_pos, Terrain::DoorOpen);
                let mon_name = world.entity_name(monster);
                events.push(EngineEvent::DoorBroken { position: door_pos });
                events.push(EngineEvent::msg_with("monster-break-door", vec![("monster", mon_name.clone())]));
            }
        }
        Terrain::DoorLocked
            // Giants can break locked doors.
            if species_flags.contains(MonsterFlags::GIANT) =>
        {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(door_pos, Terrain::DoorOpen);
            let mon_name = world.entity_name(monster);
            events.push(EngineEvent::DoorBroken { position: door_pos });
            events.push(EngineEvent::msg_with("monster-break-door", vec![("monster", mon_name.clone())]));
        }
        _ => {}
    }

    events
}

/// Try to open doors in the direction toward the player.
fn try_open_doors_toward(
    world: &mut GameWorld,
    monster: Entity,
    monster_pos: Position,
    target_pos: Position,
    _species_flags: MonsterFlags,
) -> Vec<EngineEvent> {
    let candidates = directions_toward(monster_pos, target_pos);
    for &dir in &candidates {
        let door_pos = monster_pos.step(dir);
        if let Some(cell) = world.dungeon().current_level.get(door_pos)
            && matches!(cell.terrain, Terrain::DoorClosed | Terrain::DoorLocked)
        {
            let events = try_open_door(world, monster, door_pos);
            if !events.is_empty() {
                return events;
            }
        }
    }
    Vec::new()
}

// ---------------------------------------------------------------------------
// Monster pickup
// ---------------------------------------------------------------------------

/// Monster picks up useful items at its current position.
/// Weapons, armor, wands, and potions are considered useful.
pub fn monster_pickup(
    world: &mut GameWorld,
    monster: Entity,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let monster_pos = match world.get_component::<Positioned>(monster) {
        Some(p) => p.0,
        None => return events,
    };

    let intelligence = world
        .get_component::<Intelligence>(monster)
        .map(|i| i.0)
        .unwrap_or(MonsterIntelligence::Animal);

    // Animals do not pick up items.
    if intelligence == MonsterIntelligence::Animal {
        return events;
    }

    // Find items on the floor at the monster's position.
    let floor_items: Vec<Entity> = world
        .query::<ObjectCore>()
        .iter()
        .filter(|&(entity, core)| {
            is_useful_pickup(core)
                && world
                    .get_component::<ObjectLocation>(entity)
                    .is_some_and(|loc| {
                        crate::dungeon::floor_position_on_level(
                            &loc,
                            world.dungeon().branch,
                            world.dungeon().depth,
                        )
                        .is_some_and(|pos| pos == monster_pos)
                    })
        })
        .map(|(entity, _)| entity)
        .collect();

    // Pick up the first useful item (one per turn).
    if let Some(&item) = floor_items.first() {
        let carrier_id = monster.to_bits().get() as u32;
        {
            // Scope the mutable borrow so it drops before entity_name().
            if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item) {
                *loc = ObjectLocation::MonsterInventory { carrier_id };
            }
        }
        let mon_name = world.entity_name(monster);
        events.push(EngineEvent::ItemPickedUp {
            actor: monster,
            item,
            quantity: 1,
        });
        events.push(EngineEvent::msg_with(
            "monster-picks-up",
            vec![("monster", mon_name.clone()), ("item", "item".to_string())],
        ));
    }

    events
}

/// Whether an item class is considered useful for monster pickup.
fn is_useful_pickup(core: &ObjectCore) -> bool {
    matches!(
        core.object_class,
        ObjectClass::Weapon | ObjectClass::Armor | ObjectClass::Tool | ObjectClass::Potion
    )
}

// ---------------------------------------------------------------------------
// Ranged attack decisions
// ---------------------------------------------------------------------------

/// Attempt a ranged wand attack from a distance.
/// Used by spellcaster-intelligence monsters that have offensive wands.
fn monster_ranged_attack(
    world: &mut GameWorld,
    monster: Entity,
    player: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    // Delegate to offensive item use -- it already handles wand zapping.
    monster_use_offensive(world, monster, player, rng)
}

// ---------------------------------------------------------------------------
// Enhanced flee with mfleetim timer
// ---------------------------------------------------------------------------

/// Determine if a monster is currently fleeing, using the enhanced flee
/// system with `FleeTimer` and HP-based threshold.
fn is_fleeing(
    world: &mut GameWorld,
    monster: Entity,
    current_hp: i32,
    max_hp: i32,
    rng: &mut impl Rng,
) -> bool {
    // Check flee timer first.
    if let Some(timer) = world.get_component::<FleeTimer>(monster)
        && timer.0 > 0
    {
        return true;
    }

    // HP-based flee threshold: HP < max_hp / 3 (and max_hp >= 3).
    if current_hp < max_hp / 3 && max_hp >= 3 {
        // Set the flee timer: rnd(rn2(7) ? 10 : 100)
        let upper = if rng.random_range(0..7) != 0 {
            10u8
        } else {
            100u8
        };
        let flee_time = rng.random_range(1..=upper);
        let _ = world.ecs_mut().insert_one(monster, FleeTimer(flee_time));
        return true;
    }

    false
}

/// Tick down the flee timer by 1 each turn.
fn tick_flee_timer(world: &mut GameWorld, monster: Entity) {
    if let Some(mut timer) = world.get_component_mut::<FleeTimer>(monster)
        && timer.0 > 0
    {
        timer.0 -= 1;
    }
}

// ---------------------------------------------------------------------------
// Movement helpers
// ---------------------------------------------------------------------------

/// Move `monster` one step toward `target_pos` using simple distance
/// reduction.  Tries the best direction first, then falls back to
/// adjacent options.
fn move_toward(
    world: &mut GameWorld,
    monster: Entity,
    monster_pos: Position,
    target_pos: Position,
    rng: &mut impl Rng,
) -> Option<Vec<EngineEvent>> {
    let candidates = directions_toward(monster_pos, target_pos);
    try_move_monster(world, monster, monster_pos, &candidates, rng)
}

/// Move `monster` one step away from `threat_pos`.
fn move_away(
    world: &mut GameWorld,
    monster: Entity,
    monster_pos: Position,
    threat_pos: Position,
    rng: &mut impl Rng,
) -> Option<Vec<EngineEvent>> {
    let candidates = directions_away(monster_pos, threat_pos);
    try_move_monster(world, monster, monster_pos, &candidates, rng)
}

/// Move `monster` in a random valid direction.
fn wander(
    world: &mut GameWorld,
    monster: Entity,
    monster_pos: Position,
    rng: &mut impl Rng,
) -> Option<Vec<EngineEvent>> {
    let mut dirs = Direction::PLANAR;
    // Fisher-Yates shuffle on stack array.
    for i in (1..dirs.len()).rev() {
        let j = rng.random_range(0..=i);
        dirs.swap(i, j);
    }
    try_move_monster(world, monster, monster_pos, &dirs, rng)
}

/// Try each direction in `candidates` in order; execute the first valid
/// move.  Returns `None` if no direction is passable.
fn try_move_monster(
    world: &mut GameWorld,
    monster: Entity,
    from: Position,
    candidates: &[Direction],
    _rng: &mut impl Rng,
) -> Option<Vec<EngineEvent>> {
    for &dir in candidates {
        let to = from.step(dir);
        if is_valid_monster_move(world, to, monster) {
            // Execute the move.
            if let Some(mut pos) = world.get_component_mut::<Positioned>(monster) {
                pos.0 = to;
            }
            let events = vec![EngineEvent::EntityMoved {
                entity: monster,
                from,
                to,
            }];
            return Some(events);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Whether a position is a valid destination for a monster move.
///
/// Checks: in-bounds, terrain is passable for this monster (accounting
/// for flying, swimming, phasing), no other monster already occupying
/// the tile.  The player's tile is NOT valid for movement (combat is
/// handled separately).
pub fn is_valid_monster_move(world: &GameWorld, pos: Position, self_entity: Entity) -> bool {
    let map = &world.dungeon().current_level;

    // Bounds check.
    if !map.in_bounds(pos) {
        return false;
    }

    // Read species flags for special movement abilities.
    let species_flags = world
        .get_component::<MonsterSpeciesFlags>(self_entity)
        .map(|f| f.0)
        .unwrap_or_else(MonsterFlags::empty);

    // Terrain check -- flag-aware.
    match map.get(pos) {
        Some(cell) => {
            if !terrain_passable_for(cell.terrain, species_flags) {
                return false;
            }
        }
        None => return false,
    }

    // No other entity (monster or player) at the target.
    if entity_at_pos(world, pos, self_entity).is_some() {
        return false;
    }

    true
}

/// Whether a terrain type is passable for a monster with the given flags.
///
/// Flying monsters can cross water, lava, and pools.
/// Swimming monsters can cross water and pools.
/// Phasing (wall-walking) monsters can move through walls and stone.
/// Amorphous monsters can squeeze through iron bars.
fn terrain_passable_for(terrain: Terrain, flags: MonsterFlags) -> bool {
    // Standard walkable terrain is always passable.
    if terrain.is_walkable() {
        return true;
    }

    match terrain {
        // Water/pool/moat: flyers, swimmers, and amorphous can cross.
        Terrain::Pool | Terrain::Moat | Terrain::Water => {
            flags.intersects(MonsterFlags::FLY | MonsterFlags::SWIM | MonsterFlags::AMORPHOUS)
        }
        // Lava: only flyers can cross safely.
        Terrain::Lava => flags.contains(MonsterFlags::FLY),
        // Walls and stone: only phasing monsters.
        Terrain::Wall | Terrain::Stone => flags.contains(MonsterFlags::WALLWALK),
        // Closed/locked doors: amorphous monsters can flow under.
        Terrain::DoorClosed | Terrain::DoorLocked => flags.contains(MonsterFlags::AMORPHOUS),
        // Iron bars: amorphous or very small can pass.
        Terrain::IronBars => flags.contains(MonsterFlags::AMORPHOUS),
        // Tree: flyers can pass over.
        Terrain::Tree => flags.contains(MonsterFlags::FLY),
        _ => false,
    }
}

/// Whether two positions are adjacent (Chebyshev distance <= 1, not the
/// same tile).
#[inline]
fn is_adjacent(a: Position, b: Position) -> bool {
    let dx = (a.x - b.x).abs();
    let dy = (a.y - b.y).abs();
    dx <= 1 && dy <= 1 && (dx + dy) > 0
}

/// Simplified line-of-sight check: Chebyshev distance <= 8 and no
/// opaque terrain on the Bresenham line between the two positions.
fn can_see_target(world: &GameWorld, from: Position, to: Position) -> bool {
    let dx = (to.x - from.x).abs();
    let dy = (to.y - from.y).abs();
    let chebyshev = dx.max(dy);

    if chebyshev > 8 {
        return false;
    }

    // Walk a Bresenham-style line; if any intermediate cell is opaque,
    // LOS is blocked.
    let map = &world.dungeon().current_level;
    let steps = dx.max(dy);
    if steps == 0 {
        return true;
    }

    for i in 1..steps {
        let ix = from.x + (to.x - from.x) * i / steps;
        let iy = from.y + (to.y - from.y) * i / steps;
        let check_pos = Position::new(ix, iy);
        let Some(cell) = map.get(check_pos) else {
            return false;
        };
        if cell.terrain.is_opaque() {
            return false;
        }
    }

    true
}

/// Find an entity (monster or player) at `pos`, excluding `exclude`.
fn entity_at_pos(world: &GameWorld, pos: Position, exclude: Entity) -> Option<Entity> {
    for (entity, positioned) in world.query::<Positioned>().iter() {
        if entity != exclude && positioned.0 == pos {
            return Some(entity);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Direction ranking
// ---------------------------------------------------------------------------

/// Return directions sorted by how much they reduce the distance to the
/// target.  Best direction first.
///
/// Uses a fixed-size stack array (always 8 planar directions) to avoid
/// heap allocation on this per-monster-per-turn hot path.
fn directions_toward(from: Position, target: Position) -> [Direction; 8] {
    let mut dirs: [(Direction, i32); 8] = std::array::from_fn(|i| {
        let d = Direction::PLANAR[i];
        let next = from.step(d);
        let dist = chebyshev_distance(next, target);
        (d, dist)
    });
    dirs.sort_by_key(|&(_, d)| d);
    dirs.map(|(d, _)| d)
}

/// Return directions sorted by how much they increase the distance from
/// `threat`.  Best escape direction first.
///
/// Uses a fixed-size stack array to avoid heap allocation.
fn directions_away(from: Position, threat: Position) -> [Direction; 8] {
    let mut dirs: [(Direction, i32); 8] = std::array::from_fn(|i| {
        let d = Direction::PLANAR[i];
        let next = from.step(d);
        let dist = chebyshev_distance(next, threat);
        (d, dist)
    });
    // Sort descending by distance (farthest first).
    dirs.sort_by_key(|&(_, d)| std::cmp::Reverse(d));
    dirs.map(|(d, _)| d)
}

/// Chebyshev (king-move) distance between two positions.
#[inline]
fn chebyshev_distance(a: Position, b: Position) -> i32 {
    let dx = (a.x - b.x).abs();
    let dy = (a.y - b.y).abs();
    dx.max(dy)
}

// ---------------------------------------------------------------------------
// Inventory helpers
// ---------------------------------------------------------------------------

/// Get all item entities carried by a monster.
fn get_monster_inventory(world: &GameWorld, monster: Entity) -> Vec<Entity> {
    let carrier_id = monster.to_bits().get() as u32;
    world
        .query::<ObjectCore>()
        .iter()
        .filter(|&(entity, _)| {
            world
                .get_component::<ObjectLocation>(entity)
                .is_some_and(|loc| {
                    matches!(*loc, ObjectLocation::MonsterInventory { carrier_id: cid, .. }
                        if cid == carrier_id)
                })
        })
        .map(|(entity, _)| entity)
        .collect()
}

/// Find a potion of the given type in the item list.
fn find_potion_in_list(
    items: &[Entity],
    world: &GameWorld,
    potion_type: PotionType,
) -> Option<(Entity, PotionType)> {
    items
        .iter()
        .copied()
        .find(|&item| {
            world
                .get_component::<ObjectCore>(item)
                .is_some_and(|core| core.object_class == ObjectClass::Potion)
                && world
                    .get_component::<PotionTypeTag>(item)
                    .is_some_and(|tag| tag.0 == potion_type)
        })
        .map(|item| (item, potion_type))
}

/// Find a wand of the given type in the item list.
fn find_wand_in_list(items: &[Entity], world: &GameWorld, wand_type: WandType) -> Option<Entity> {
    items.iter().copied().find(|&item| {
        world
            .get_component::<ObjectCore>(item)
            .is_some_and(|core| core.object_class == ObjectClass::Tool)
            && world
                .get_component::<WandTypeTag>(item)
                .is_some_and(|tag| tag.0 == wand_type)
    })
}

/// Find stairs on the current level (first found).
fn find_stairs(world: &GameWorld) -> Option<Position> {
    let map = &world.dungeon().current_level;
    for y in 0..map.height {
        for x in 0..map.width {
            let pos = Position::new(x as i32, y as i32);
            if let Some(cell) = map.get(pos)
                && matches!(cell.terrain, Terrain::StairsUp | Terrain::StairsDown)
            {
                return Some(pos);
            }
        }
    }
    None
}

/// Squared Euclidean distance between two positions.
/// Matches NetHack's `dist2()` / `mdistu()` used for range checks.
#[inline]
fn dist2_positions(a: Position, b: Position) -> i32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

/// Find a random walkable, unoccupied floor tile adjacent to the given
/// position.  Used for "teleport near player" (mnexto equivalent).
fn find_adjacent_floor(
    world: &GameWorld,
    center: Position,
    rng: &mut impl Rng,
) -> Option<Position> {
    let mut candidates: Vec<Position> = Vec::new();
    for &dir in &Direction::PLANAR {
        let pos = center.step(dir);
        if let Some(cell) = world.dungeon().current_level.get(pos)
            && cell.terrain.is_walkable()
        {
            // Check no entity at this position.
            let occupied = world.query::<Positioned>().iter().any(|(_, p)| p.0 == pos);
            if !occupied {
                candidates.push(pos);
            }
        }
    }
    if candidates.is_empty() {
        None
    } else {
        let idx = rng.random_range(0..candidates.len());
        Some(candidates[idx])
    }
}

/// Find a random walkable floor tile on the current level.
fn find_random_floor_tile(world: &GameWorld, rng: &mut impl Rng) -> Option<Position> {
    let map = &world.dungeon().current_level;
    let mut candidates = Vec::new();
    for y in 0..map.height {
        for x in 0..map.width {
            let pos = Position::new(x as i32, y as i32);
            if let Some(cell) = map.get(pos)
                && cell.terrain.is_walkable()
            {
                candidates.push(pos);
            }
        }
    }
    if candidates.is_empty() {
        None
    } else {
        let idx = rng.random_range(0..candidates.len());
        Some(candidates[idx])
    }
}

// ---------------------------------------------------------------------------
// Monster spellcasting (mcastu)
// ---------------------------------------------------------------------------

/// Mage spells a monster can cast (from C enum `mcast_mage_spells`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MageSpell {
    /// MGC_PSI_BOLT — psychic damage (d(3, monster_level))
    PsiBolt,
    /// MGC_CURE_SELF — heal d(3, 6) + 6 HP
    CureSelf,
    /// MGC_HASTE_SELF — haste for d(2, 50) turns
    HasteSelf,
    /// MGC_STUN_YOU — stun the player
    StunYou,
    /// MGC_DISAPPEAR — make self invisible
    Disappear,
    /// MGC_WEAKEN_YOU — reduce player Str by 1
    WeakenYou,
    /// MGC_DESTRY_ARMR — damage player's armor
    DestroyArmor,
    /// MGC_CURSE_ITEMS — curse random player items
    CurseItems,
    /// MGC_AGGRAVATION — wake all monsters
    Aggravation,
    /// MGC_SUMMON_MONS — summon allied monsters
    SummonMonster,
    /// MGC_DEATH_TOUCH — massive damage (d(8, 6))
    DeathTouch,
}

/// Cleric spells a monster can cast (from C enum `mcast_cleric_spells`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClericSpell {
    /// CLC_OPEN_WOUNDS — damage d(3, monster_level)
    OpenWounds,
    /// CLC_CURE_SELF — heal self
    CureSelf,
    /// CLC_CONFUSE_YOU — confuse the player
    ConfuseYou,
    /// CLC_PARALYZE — paralyze the player
    Paralyze,
    /// CLC_BLIND_YOU — blind the player
    BlindYou,
    /// CLC_INSECTS — summon insects
    Insects,
    /// CLC_CURSE_ITEMS — curse random player items
    CurseItems,
    /// CLC_LIGHTNING — lightning bolt damage
    Lightning,
    /// CLC_FIRE_PILLAR — fire pillar damage
    FirePillar,
    /// CLC_GEYSER — geyser (water + stun)
    Geyser,
}

/// Choose a mage spell based on monster level.
///
/// Higher-level monsters get access to more powerful spells.
/// Mirrors C `choose_magic_spell()`.
pub fn choose_mage_spell(monster_level: u8, rng: &mut impl Rng) -> MageSpell {
    let mut spellval = rng.random_range(0..=(monster_level as u32).min(24));
    // Higher values sometimes re-roll lower (C: `while spellval > 24 && rn2(25)`)
    while spellval > 24 {
        spellval = rng.random_range(0..spellval);
    }
    match spellval {
        22..=u32::MAX => MageSpell::DeathTouch,
        20..=21 => MageSpell::SummonMonster,
        17..=19 => MageSpell::Aggravation,
        14..=16 => MageSpell::CurseItems,
        12..=13 => MageSpell::DestroyArmor,
        10..=11 => MageSpell::WeakenYou,
        8..=9 => MageSpell::Disappear,
        6..=7 => MageSpell::StunYou,
        4..=5 => MageSpell::HasteSelf,
        2..=3 => MageSpell::CureSelf,
        _ => MageSpell::PsiBolt,
    }
}

/// Choose a cleric spell based on monster level.
///
/// Higher-level monsters get access to more powerful spells.
/// Mirrors C `choose_clerical_spell()`.
pub fn choose_cleric_spell(monster_level: u8, rng: &mut impl Rng) -> ClericSpell {
    let spellval = rng.random_range(0..=(monster_level as u32).min(24));
    match spellval {
        22..=u32::MAX => ClericSpell::Geyser,
        19..=21 => ClericSpell::FirePillar,
        16..=18 => ClericSpell::Lightning,
        14..=15 => ClericSpell::CurseItems,
        11..=13 => ClericSpell::Insects,
        8..=10 => ClericSpell::BlindYou,
        6..=7 => ClericSpell::Paralyze,
        4..=5 => ClericSpell::ConfuseYou,
        2..=3 => ClericSpell::CureSelf,
        _ => ClericSpell::OpenWounds,
    }
}

/// ECS component for monsters that can cast spells.
///
/// `is_cleric` determines whether it picks from the cleric or mage table.
#[derive(Debug, Clone, Copy)]
pub struct Spellcaster {
    pub monster_level: u8,
    pub is_cleric: bool,
}

/// Resolve a monster spellcasting action.
///
/// The monster chooses a spell based on its level, then applies the effect.
/// Returns events describing what happened.
pub fn monster_cast_spell(
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
            vec![("monster", mon_name.clone())],
        ));
        match spell {
            ClericSpell::OpenWounds => {
                let dice = 3u32;
                let sides = (caster.monster_level as u32).max(1);
                let damage: u32 = (0..dice).map(|_| rng.random_range(1..=sides)).sum();
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current -= damage as i32;
                    events.push(EngineEvent::HpChange {
                        entity: player,
                        amount: -(damage as i32),
                        new_hp: hp.current,
                        source: HpSource::Spell,
                    });
                }
            }
            ClericSpell::CureSelf => {
                let heal: i32 = (0..3).map(|_| rng.random_range(1i32..=6)).sum::<i32>() + 6;
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
                events.push(EngineEvent::msg("spell-summon-insects"));
            }
            ClericSpell::CurseItems => {
                events.push(EngineEvent::msg("spell-curse-items"));
            }
            ClericSpell::Lightning => {
                let damage: u32 = (0..4).map(|_| rng.random_range(1u32..=6)).sum();
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current -= damage as i32;
                    events.push(EngineEvent::HpChange {
                        entity: player,
                        amount: -(damage as i32),
                        new_hp: hp.current,
                        source: HpSource::Spell,
                    });
                }
            }
            ClericSpell::FirePillar => {
                let damage: u32 = (0..4).map(|_| rng.random_range(1u32..=6)).sum();
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current -= damage as i32;
                    events.push(EngineEvent::HpChange {
                        entity: player,
                        amount: -(damage as i32),
                        new_hp: hp.current,
                        source: HpSource::Spell,
                    });
                }
            }
            ClericSpell::Geyser => {
                let damage: u32 = (0..6).map(|_| rng.random_range(1u32..=6)).sum();
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current -= damage as i32;
                    events.push(EngineEvent::HpChange {
                        entity: player,
                        amount: -(damage as i32),
                        new_hp: hp.current,
                        source: HpSource::Spell,
                    });
                }
                // Geyser also stuns.
                let stun_dur = rng.random_range(1u32..=5);
                events.push(EngineEvent::StatusApplied {
                    entity: player,
                    status: StatusEffect::Stunned,
                    duration: Some(stun_dur),
                    source: Some(monster),
                });
            }
        }
    } else {
        let spell = choose_mage_spell(caster.monster_level, rng);
        events.push(EngineEvent::msg_with(
            "monster-casts-mage",
            vec![("monster", mon_name.clone())],
        ));
        match spell {
            MageSpell::PsiBolt => {
                let dice = 3u32;
                let sides = (caster.monster_level as u32).max(1);
                let damage: u32 = (0..dice).map(|_| rng.random_range(1..=sides)).sum();
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current -= damage as i32;
                    events.push(EngineEvent::HpChange {
                        entity: player,
                        amount: -(damage as i32),
                        new_hp: hp.current,
                        source: HpSource::Spell,
                    });
                }
            }
            MageSpell::CureSelf => {
                let heal: i32 = (0..3).map(|_| rng.random_range(1i32..=6)).sum::<i32>() + 6;
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
                let damage: u32 = (0..8).map(|_| rng.random_range(1u32..=6)).sum();
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current -= damage as i32;
                    events.push(EngineEvent::HpChange {
                        entity: player,
                        amount: -(damage as i32),
                        new_hp: hp.current,
                        source: HpSource::Spell,
                    });
                }
            }
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Monster throwing (mthrowu)
// ---------------------------------------------------------------------------

/// ECS component for throwable items (daggers, spears, rocks, etc.).
#[derive(Debug, Clone, Copy)]
pub struct Throwable {
    /// Base damage dice count.
    pub dice_count: u8,
    /// Base damage dice sides.
    pub dice_sides: u8,
}

/// Determine whether a monster should throw an item at the player.
///
/// Returns true if the monster has a ranged weapon and has line of sight
/// to the player at a distance > 1.
pub fn should_monster_throw(world: &GameWorld, monster: Entity, player_pos: Position) -> bool {
    let monster_pos = match world.get_component::<Positioned>(monster) {
        Some(p) => p.0,
        None => return false,
    };

    let distance = chebyshev_distance(monster_pos, player_pos);
    if distance <= 1 || distance > 7 {
        return false;
    }

    if !can_see_target(world, monster_pos, player_pos) {
        return false;
    }

    // Check if the monster has a throwable item.
    let items = get_monster_inventory(world, monster);
    items
        .iter()
        .any(|&item| world.get_component::<Throwable>(item).is_some())
}

/// Monster throws an item at the player.
///
/// Finds the best throwable item in the monster's inventory, calculates
/// hit/miss based on distance and AC, and applies damage if hit.
pub fn monster_throw_item(
    world: &mut GameWorld,
    monster: Entity,
    player: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let monster_pos = match world.get_component::<Positioned>(monster) {
        Some(p) => p.0,
        None => return events,
    };
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    let distance = chebyshev_distance(monster_pos, player_pos);

    // Find a throwable item.
    let items = get_monster_inventory(world, monster);
    let throwable_item = items
        .iter()
        .copied()
        .find(|&item| world.get_component::<Throwable>(item).is_some());

    let item = match throwable_item {
        Some(i) => i,
        None => return events,
    };

    let throwable = *world.get_component::<Throwable>(item).unwrap();
    let mon_name = world.entity_name(monster);

    events.push(EngineEvent::msg_with(
        "monster-throws",
        vec![
            ("monster", mon_name.clone()),
            ("item", "projectile".to_string()),
        ],
    ));

    // Hit check: to_hit = monster_level + 1 + d(1,20) vs player AC + distance penalty.
    let monster_level = world
        .get_component::<crate::world::ExperienceLevel>(monster)
        .map(|l| l.0 as i32)
        .unwrap_or(1);
    let roll = rng.random_range(1i32..=20);
    let to_hit = monster_level + 1 + roll;

    let player_ac = world
        .get_component::<crate::world::ArmorClass>(player)
        .map(|ac| ac.0)
        .unwrap_or(10);
    let effective_ac = player_ac + distance; // harder to hit at range

    if to_hit > effective_ac {
        // Hit — roll damage.
        let damage: u32 = (0..throwable.dice_count as u32)
            .map(|_| rng.random_range(1..=(throwable.dice_sides as u32).max(1)))
            .sum();
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
            hp.current -= damage as i32;
            events.push(EngineEvent::RangedHit {
                attacker: monster,
                defender: player,
                projectile: item,
                damage,
            });
            events.push(EngineEvent::HpChange {
                entity: player,
                amount: -(damage as i32),
                new_hp: hp.current,
                source: HpSource::Combat,
            });
        }
    } else {
        events.push(EngineEvent::RangedMiss {
            attacker: monster,
            defender: player,
            projectile: item,
        });
    }

    // Consume one from stack (or despawn if quantity is 1).
    let should_despawn = world
        .get_component::<ObjectCore>(item)
        .map(|c| c.quantity <= 1)
        .unwrap_or(true);
    if should_despawn {
        let _ = world.despawn(item);
    } else if let Some(mut core) = world.get_component_mut::<ObjectCore>(item) {
        core.quantity -= 1;
    }

    events
}

// ---------------------------------------------------------------------------
// Monster generation: difficulty-based filtering
// ---------------------------------------------------------------------------

/// Calculate the difficulty window for monster generation.
///
/// Returns `(min_difficulty, max_difficulty)` based on dungeon depth
/// and player level.  Per spec:
///   `monmin_difficulty(levdif) = levdif / 6`
///   `monmax_difficulty(levdif) = (levdif + ulevel) / 2`
pub fn difficulty_window(dungeon_depth: u32, player_level: u32) -> (u32, u32) {
    let min_diff = dungeon_depth / 6;
    let max_diff = (dungeon_depth + player_level) / 2;
    (min_diff, max_diff)
}

/// Calculate the spontaneous monster spawn rate (turns between spawns).
///
/// Per spec:
///   - After Wizard killed: 1/25 per turn
///   - Deeper than Castle (depth > ~25): 1/50 per turn
///   - Otherwise: 1/70 per turn
pub fn spawn_rate(wizard_killed: bool, dungeon_depth: u32) -> u32 {
    if wizard_killed {
        25
    } else if dungeon_depth > 25 {
        50
    } else {
        70
    }
}

/// Check whether a spontaneous monster should be generated this turn.
///
/// Returns true with probability `1/spawn_rate`.
pub fn should_spawn_monster(wizard_killed: bool, dungeon_depth: u32, rng: &mut impl Rng) -> bool {
    let rate = spawn_rate(wizard_killed, dungeon_depth);
    rng.random_range(0..rate) == 0
}

/// Calculate group size for G_SGROUP or G_LGROUP generation.
///
/// Per spec:
///   Small group: rnd(3), divided by 4 if ulevel<3, by 2 if ulevel<5.
///   Large group: rnd(10), same divisor, 1/3 chance of small instead.
pub fn group_size(is_large: bool, player_level: u32, rng: &mut impl Rng) -> u32 {
    let n = if is_large { 10u32 } else { 3u32 };
    let mut cnt = rng.random_range(1..=n);

    if player_level < 3 {
        cnt /= 4;
    } else if player_level < 5 {
        cnt /= 2;
    }

    cnt.max(1)
}

/// Adjust a monster's level based on dungeon depth and player level.
///
/// Per spec (`adj_lev`):
///   - base = mlevel
///   - diff = level_difficulty - mlevel
///   - if diff < 0: base -= 1
///   - else: base += diff / 5
///   - player_diff = ulevel - mlevel; if > 0: base += player_diff / 4
///   - cap at min(3*mlevel/2, 49), floor at 0
pub fn adj_lev(base_mlevel: u32, dungeon_depth: u32, player_level: u32) -> u32 {
    if base_mlevel > 49 {
        return 50; // special demon lords
    }

    let mut tmp = base_mlevel as i32;
    let diff = dungeon_depth as i32 - base_mlevel as i32;

    if diff < 0 {
        tmp -= 1;
    } else {
        tmp += diff / 5;
    }

    let player_diff = player_level as i32 - base_mlevel as i32;
    if player_diff > 0 {
        tmp += player_diff / 4;
    }

    let upper = ((3 * base_mlevel) / 2).min(49) as i32;
    tmp.clamp(0, upper) as u32
}

// ---------------------------------------------------------------------------
// Demon lord special behaviors
// ---------------------------------------------------------------------------

/// Demon lord identity, used to dispatch special behaviors.
///
/// Each demon lord has unique attack patterns from C NetHack's `demonpet.c`
/// and `mhitu.c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemonLord {
    /// Uses wand of death, summons undead.
    Orcus,
    /// Cold attacks + spellcasting.
    Asmodeus,
    /// Poison sting + fly attacks.
    Baalzebub,
    /// Acid engulf + spit.
    Juiblex,
    /// Disease gaze + confuse attack.
    Demogorgon,
    /// Confusion attacks + drain life.
    Yeenoghu,
}

/// Identify whether a monster is a known demon lord, based on its name
/// and species flags.
pub fn identify_demon_lord(world: &GameWorld, monster: Entity) -> Option<DemonLord> {
    let flags = world
        .get_component::<MonsterSpeciesFlags>(monster)
        .map(|f| f.0)
        .unwrap_or_else(MonsterFlags::empty);

    // Must be a demon (lord or prince).
    if !flags.contains(MonsterFlags::DEMON) {
        return None;
    }
    if !flags.intersects(MonsterFlags::LORD | MonsterFlags::PRINCE) {
        return None;
    }

    let name = world.entity_name(monster);
    match name.as_str() {
        "Orcus" => Some(DemonLord::Orcus),
        "Asmodeus" => Some(DemonLord::Asmodeus),
        "Baalzebub" => Some(DemonLord::Baalzebub),
        "Juiblex" => Some(DemonLord::Juiblex),
        "Demogorgon" => Some(DemonLord::Demogorgon),
        "Yeenoghu" => Some(DemonLord::Yeenoghu),
        _ => None,
    }
}

/// Resolve demon lord special behavior.
///
/// Called during the monster's turn when the monster is identified as
/// a demon lord.  Each lord has unique abilities:
///
/// - **Orcus**: Prioritizes wand of death; if no wand, casts death touch
/// - **Asmodeus**: Cold-based attacks + spellcasting
/// - **Baalzebub**: Poison sting, flies to pursue
/// - **Juiblex**: Acid spit at range, engulf at melee
/// - **Demogorgon**: Disease gaze at range, confusion in melee
/// - **Yeenoghu**: Confusion + drain life attacks
///
/// Returns events if the lord performed a special action, empty vec
/// if it should fall through to normal AI.
pub fn demon_lord_special(
    world: &mut GameWorld,
    monster: Entity,
    lord: DemonLord,
    player: Entity,
    distance: i32,
    can_see: bool,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mon_name = world.entity_name(monster);

    match lord {
        DemonLord::Orcus => {
            // Orcus prioritizes wand of death if he has one.
            let monster_items = get_monster_inventory(world, monster);
            if let Some(wand) = find_wand_in_list(&monster_items, world, WandType::Death) {
                let has_charges = world
                    .get_component::<WandCharges>(wand)
                    .map(|c| c.spe > 0)
                    .unwrap_or(false);
                if has_charges && can_see {
                    if let Some(mut charges) = world.get_component_mut::<WandCharges>(wand) {
                        charges.spe -= 1;
                    }
                    events.push(EngineEvent::msg_with(
                        "orcus-zaps-death",
                        vec![("monster", mon_name)],
                    ));
                    // Death ray: 999 damage (player must have MR to survive).
                    if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                        hp.current -= 999;
                        events.push(EngineEvent::HpChange {
                            entity: player,
                            amount: -999,
                            new_hp: hp.current,
                            source: HpSource::Combat,
                        });
                    }
                    return events;
                }
            }
            // Fallback: cast death touch spell if high enough level.
            if can_see && distance <= 8 {
                let caster_level = world
                    .get_component::<crate::world::ExperienceLevel>(monster)
                    .map(|l| l.0)
                    .unwrap_or(1);
                if caster_level >= 22 {
                    let damage = rng.random_range(1..=20i32);
                    events.push(EngineEvent::msg_with(
                        "orcus-death-touch",
                        vec![("monster", mon_name)],
                    ));
                    if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                        hp.current -= damage;
                        events.push(EngineEvent::HpChange {
                            entity: player,
                            amount: -damage,
                            new_hp: hp.current,
                            source: HpSource::Spell,
                        });
                    }
                    return events;
                }
            }
        }
        DemonLord::Asmodeus => {
            // Asmodeus uses cold attacks at range.
            if can_see && distance > 1 && distance <= 6 {
                let damage: i32 = (0..6).map(|_| rng.random_range(1..=6i32)).sum();
                events.push(EngineEvent::msg_with(
                    "asmodeus-cold-blast",
                    vec![("monster", mon_name)],
                ));
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current -= damage;
                    events.push(EngineEvent::HpChange {
                        entity: player,
                        amount: -damage,
                        new_hp: hp.current,
                        source: HpSource::Spell,
                    });
                }
                return events;
            }
        }
        DemonLord::Baalzebub => {
            // Baalzebub uses poison sting at melee range.
            if distance <= 1 {
                let damage = rng.random_range(2..=12i32);
                events.push(EngineEvent::msg_with(
                    "baalzebub-poison-sting",
                    vec![("monster", mon_name)],
                ));
                events.push(EngineEvent::StatusApplied {
                    entity: player,
                    status: StatusEffect::Sick,
                    duration: Some(rng.random_range(10..=40u32)),
                    source: Some(monster),
                });
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current -= damage;
                    events.push(EngineEvent::HpChange {
                        entity: player,
                        amount: -damage,
                        new_hp: hp.current,
                        source: HpSource::Combat,
                    });
                }
                return events;
            }
        }
        DemonLord::Juiblex => {
            // Juiblex spits acid at range.
            if can_see && distance > 1 && distance <= 4 {
                let damage: i32 = (0..4).map(|_| rng.random_range(1..=6i32)).sum();
                events.push(EngineEvent::msg_with(
                    "juiblex-acid-spit",
                    vec![("monster", mon_name)],
                ));
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current -= damage;
                    events.push(EngineEvent::HpChange {
                        entity: player,
                        amount: -damage,
                        new_hp: hp.current,
                        source: HpSource::Combat,
                    });
                }
                return events;
            }
        }
        DemonLord::Demogorgon => {
            // Demogorgon has a disease gaze at range.
            if can_see && distance <= 6 {
                events.push(EngineEvent::msg_with(
                    "demogorgon-disease-gaze",
                    vec![("monster", mon_name)],
                ));
                events.push(EngineEvent::StatusApplied {
                    entity: player,
                    status: StatusEffect::Sick,
                    duration: Some(rng.random_range(20..=60u32)),
                    source: Some(monster),
                });
                return events;
            }
        }
        DemonLord::Yeenoghu => {
            // Yeenoghu causes confusion in melee.
            if distance <= 1 {
                let damage = rng.random_range(3..=18i32);
                events.push(EngineEvent::msg_with(
                    "yeenoghu-confuse-attack",
                    vec![("monster", mon_name)],
                ));
                events.push(EngineEvent::StatusApplied {
                    entity: player,
                    status: StatusEffect::Confused,
                    duration: Some(rng.random_range(5..=20u32)),
                    source: Some(monster),
                });
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current -= damage;
                    events.push(EngineEvent::HpChange {
                        entity: player,
                        amount: -damage,
                        new_hp: hp.current,
                        source: HpSource::Combat,
                    });
                }
                return events;
            }
        }
    }

    // No special action taken — fall through to normal AI.
    events
}

// ---------------------------------------------------------------------------
// Leprechaun avoidance AI (C: mon.c / monmove.c)
// ---------------------------------------------------------------------------

/// What a leprechaun should do after stealing gold.
///
/// C NetHack: leprechauns teleport away immediately after stealing gold.
/// If they can't teleport, they flee from the player. When they have no
/// gold, they behave normally (approach to steal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeprechaunAction {
    /// Has gold + can teleport + close to player: teleport away.
    TeleportAway,
    /// Has gold but can't teleport: run away from player.
    FleeFromPlayer,
    /// No stolen gold: approach normally to steal.
    Normal,
}

/// Determine leprechaun-specific behavior based on gold possession
/// and proximity to the player.
pub fn leprechaun_avoidance(
    has_gold: bool,
    can_teleport: bool,
    distance_to_player: i32,
) -> LeprechaunAction {
    if has_gold && can_teleport && distance_to_player <= 2 {
        LeprechaunAction::TeleportAway
    } else if has_gold {
        LeprechaunAction::FleeFromPlayer
    } else {
        LeprechaunAction::Normal
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::dungeon::Terrain;
    use crate::world::{
        ArmorClass, Attributes, CreationOrder, ExperienceLevel, HitPoints, Monster, MovementPoints,
        NORMAL_SPEED, Name, Positioned, Speed,
    };
    use nethack_babel_data::{ObjectClass, ObjectCore, ObjectLocation, ObjectTypeId};
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    /// Deterministic RNG for tests.
    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(54321)
    }

    /// Build a small test world with floor from (1,1) to (15,15), player
    /// at (8,8), surrounded by stone walls at the edges.
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

    /// Spawn a monster at the given position with given HP.
    fn spawn_monster_at(
        world: &mut GameWorld,
        pos: Position,
        current_hp: i32,
        max_hp: i32,
    ) -> Entity {
        world.spawn((
            Monster,
            Positioned(pos),
            HitPoints {
                current: current_hp,
                max: max_hp,
            },
            ArmorClass(10),
            Attributes::default(),
            ExperienceLevel(1),
            Speed(12),
            MovementPoints(NORMAL_SPEED as i32),
            Name("goblin".to_string()),
        ))
    }

    /// Spawn a monster with a specific intelligence tier.
    fn spawn_intelligent_monster(
        world: &mut GameWorld,
        pos: Position,
        current_hp: i32,
        max_hp: i32,
        intel: MonsterIntelligence,
    ) -> Entity {
        world.spawn((
            Monster,
            Positioned(pos),
            HitPoints {
                current: current_hp,
                max: max_hp,
            },
            ArmorClass(10),
            Attributes::default(),
            ExperienceLevel(1),
            Speed(12),
            MovementPoints(NORMAL_SPEED as i32),
            Name("creature".to_string()),
            Intelligence(intel),
        ))
    }

    /// Create a potion entity in a monster's inventory.
    fn give_monster_potion(
        world: &mut GameWorld,
        monster: Entity,
        potion_type: PotionType,
    ) -> Entity {
        let carrier_id = monster.to_bits().get() as u32;
        let core = ObjectCore {
            otyp: ObjectTypeId(100),
            object_class: ObjectClass::Potion,
            quantity: 1,
            weight: 20,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let loc = ObjectLocation::MonsterInventory { carrier_id };
        world.spawn((core, loc, PotionTypeTag(potion_type)))
    }

    /// Create a wand entity in a monster's inventory.
    fn give_monster_wand(
        world: &mut GameWorld,
        monster: Entity,
        wand_type: WandType,
        charges: i8,
    ) -> Entity {
        let carrier_id = monster.to_bits().get() as u32;
        let core = ObjectCore {
            otyp: ObjectTypeId(200),
            object_class: ObjectClass::Tool,
            quantity: 1,
            weight: 7,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let loc = ObjectLocation::MonsterInventory { carrier_id };
        let wand_charges = WandCharges {
            spe: charges,
            recharged: 0,
        };
        world.spawn((core, loc, WandTypeTag(wand_type), wand_charges))
    }

    /// Place a weapon on the floor for pickup tests.
    fn place_weapon_on_floor(world: &mut GameWorld, x: i32, y: i32) -> Entity {
        let core = ObjectCore {
            otyp: ObjectTypeId(300),
            object_class: ObjectClass::Weapon,
            quantity: 1,
            weight: 30,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let loc = ObjectLocation::Floor {
            x: x as i16,
            y: y as i16,
            level: world.dungeon().current_data_dungeon_level(),
        };
        world.spawn((core, loc))
    }

    // ── Existing tests (Phase 1) ──────────────────────────────────

    #[test]
    fn monster_at_low_hp_flees() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Monster at (9,8), player at (8,8).
        // HP = 2 out of 12 => 2 < 12/3=4, should flee.
        let monster = spawn_monster_at(&mut world, Position::new(9, 8), 2, 12);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Monster should have moved.
        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(moved, "low-HP monster should move (flee)");

        // Should NOT have attacked.
        let attacked = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
            )
        });
        assert!(!attacked, "fleeing monster should not attack");
    }

    #[test]
    fn flee_moves_away_from_player() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Monster at (9,8), player at (8,8).
        let monster = spawn_monster_at(&mut world, Position::new(9, 8), 2, 12);
        let player_pos = Position::new(8, 8);

        let initial_dist = chebyshev_distance(Position::new(9, 8), player_pos);

        resolve_monster_turn(&mut world, monster, &mut rng);

        let new_pos = world.get_component::<Positioned>(monster).unwrap().0;
        let new_dist = chebyshev_distance(new_pos, player_pos);

        assert!(
            new_dist >= initial_dist,
            "flee should increase distance: was {}, now {}",
            initial_dist,
            new_dist
        );
    }

    #[test]
    fn monster_adjacent_to_player_attacks() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Monster at (9,8), adjacent to player at (8,8).  Full HP.
        let monster = spawn_monster_at(&mut world, Position::new(9, 8), 10, 10);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Should have generated a combat event.
        let has_combat = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
            )
        });
        assert!(
            has_combat,
            "adjacent monster with full HP should attack player"
        );

        // Monster should NOT have moved.
        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(!moved, "attacking monster should not also move");
    }

    #[test]
    fn monster_pursues_visible_player() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Monster at (12,8), player at (8,8).  Distance = 4, within LOS.
        let monster = spawn_monster_at(&mut world, Position::new(12, 8), 10, 10);
        let player_pos = Position::new(8, 8);

        let initial_dist = chebyshev_distance(Position::new(12, 8), player_pos);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Monster should have moved.
        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(moved, "monster should pursue visible player");

        // Distance should have decreased.
        let new_pos = world.get_component::<Positioned>(monster).unwrap().0;
        let new_dist = chebyshev_distance(new_pos, player_pos);
        assert!(
            new_dist < initial_dist,
            "pursuit should decrease distance: was {}, now {}",
            initial_dist,
            new_dist
        );
    }

    #[test]
    fn monster_wanders_when_player_not_visible() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Place a wall between monster and player to block LOS.
        for y in 1..=15 {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(11, y), Terrain::Wall);
        }

        let monster = spawn_monster_at(&mut world, Position::new(14, 8), 10, 10);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        let attacked = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
            )
        });
        assert!(!attacked, "monster with blocked LOS should not attack");
    }

    #[test]
    fn monster_respects_walls() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let monster_pos = Position::new(5, 5);
        let monster = spawn_monster_at(&mut world, monster_pos, 10, 10);

        // Block all directions except south.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(5, 4), Terrain::Wall);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Wall);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(4, 5), Terrain::Wall);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 4), Terrain::Wall);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(4, 4), Terrain::Wall);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 6), Terrain::Wall);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(4, 6), Terrain::Wall);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        for event in &events {
            if let EngineEvent::EntityMoved { to, .. } = event {
                assert_eq!(
                    *to,
                    Position::new(5, 6),
                    "monster should only move south (the only open direction)"
                );
            }
        }

        let final_pos = world.get_component::<Positioned>(monster).unwrap().0;
        let terrain = world
            .dungeon()
            .current_level
            .get(final_pos)
            .unwrap()
            .terrain;
        assert!(
            terrain.is_walkable(),
            "monster should never end up on non-walkable terrain"
        );
    }

    #[test]
    fn monster_with_max_hp_1_never_flees() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let monster = spawn_monster_at(&mut world, Position::new(9, 8), 1, 1);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        let has_combat = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
            )
        });
        assert!(
            has_combat,
            "monster with max_hp=1 should not flee (would always flee otherwise)"
        );
    }

    #[test]
    fn monster_does_not_walk_onto_other_monster() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let _monster_b = spawn_monster_at(&mut world, Position::new(9, 8), 10, 10);
        let monster_a = spawn_monster_at(&mut world, Position::new(10, 8), 10, 10);

        resolve_monster_turn(&mut world, monster_a, &mut rng);

        let pos_a = world.get_component::<Positioned>(monster_a).unwrap().0;
        assert_ne!(
            pos_a,
            Position::new(9, 8),
            "monster should not walk onto another monster"
        );
    }

    #[test]
    fn test_is_adjacent() {
        let center = Position::new(5, 5);
        assert!(is_adjacent(center, Position::new(4, 4)));
        assert!(is_adjacent(center, Position::new(5, 4)));
        assert!(is_adjacent(center, Position::new(6, 4)));
        assert!(is_adjacent(center, Position::new(4, 5)));
        assert!(is_adjacent(center, Position::new(6, 5)));
        assert!(is_adjacent(center, Position::new(4, 6)));
        assert!(is_adjacent(center, Position::new(5, 6)));
        assert!(is_adjacent(center, Position::new(6, 6)));
        assert!(!is_adjacent(center, center));
        assert!(!is_adjacent(center, Position::new(7, 5)));
    }

    #[test]
    fn test_chebyshev_distance() {
        assert_eq!(
            chebyshev_distance(Position::new(0, 0), Position::new(3, 4)),
            4
        );
        assert_eq!(
            chebyshev_distance(Position::new(5, 5), Position::new(5, 5)),
            0
        );
        assert_eq!(
            chebyshev_distance(Position::new(0, 0), Position::new(8, 0)),
            8
        );
    }

    #[test]
    fn test_can_see_through_open_floor() {
        let world = make_test_world();
        assert!(can_see_target(
            &world,
            Position::new(8, 8),
            Position::new(12, 8)
        ));
    }

    #[test]
    fn test_cannot_see_through_wall() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(10, 8), Terrain::Wall);

        assert!(!can_see_target(
            &world,
            Position::new(8, 8),
            Position::new(12, 8)
        ));
    }

    #[test]
    fn test_cannot_see_beyond_range() {
        let world = make_test_world();
        assert!(!can_see_target(
            &world,
            Position::new(1, 1),
            Position::new(11, 1)
        ));
    }

    // ═══════════════════════════════════════════════════════════════
    // Phase 2 tests: Enhanced monster AI
    // ═══════════════════════════════════════════════════════════════

    // ── Test 1: Monster quaffs healing when low HP ────────────────

    #[test]
    fn monster_quaffs_healing_when_low_hp() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Monster at (12,8), low HP.
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            3,  // current HP
            30, // max HP (3 < 30/3=10, below threshold)
            MonsterIntelligence::Humanoid,
        );

        // Give monster a healing potion.
        let potion = give_monster_potion(&mut world, monster, PotionType::Healing);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Should have healed.
        let healed = events
            .iter()
            .any(|e| matches!(e, EngineEvent::HpChange { amount, .. } if *amount > 0));
        assert!(healed, "monster should quaff healing potion when low HP");

        // Potion should be consumed (despawned).
        assert!(
            world.get_component::<ObjectCore>(potion).is_none(),
            "potion should be consumed after quaffing"
        );

        // HP should have increased.
        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert!(hp.current > 3, "HP should have increased from 3");
    }

    // ── Test 2: Monster zaps offensive wand at player ─────────────

    #[test]
    fn monster_zaps_offensive_wand_at_player() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Monster at (12,8) with full HP, player at (8,8).
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            20,
            20,
            MonsterIntelligence::Humanoid,
        );

        // Give monster a wand of fire with 3 charges.
        let wand = give_monster_wand(&mut world, monster, WandType::Fire, 3);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Should have zapped the wand.
        let zapped = events.iter().any(|e| {
            if let EngineEvent::Message { key, .. } = e {
                key.contains("monster-uses-wand")
            } else {
                false
            }
        });
        assert!(zapped, "monster should zap offensive wand at player");

        // Player should have taken damage.
        let player_damaged = events
            .iter()
            .any(|e| matches!(e, EngineEvent::HpChange { amount, .. } if *amount < 0));
        assert!(
            player_damaged,
            "player should take damage from wand of fire"
        );

        // Wand charges should have decremented.
        let charges = world.get_component::<WandCharges>(wand).unwrap();
        assert_eq!(
            charges.spe, 2,
            "wand charges should have decremented from 3 to 2"
        );
    }

    // ── Test 3: Covetous monster teleports to stairs when hurt ────

    #[test]
    fn covetous_monster_teleports_to_stairs_when_hurt() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Place stairs.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(3, 3), Terrain::StairsDown);

        // Covetous monster at (12,8), very low HP (< 33%).
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            2,  // current HP
            30, // max HP (2*3/30 = 0, triggers STRAT_HEAL)
            MonsterIntelligence::Humanoid,
        );
        // Add Covetous marker.
        let _ = world.ecs_mut().insert_one(monster, Covetous);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Should have teleported to stairs.
        let teleported = events.iter().any(
            |e| matches!(e, EngineEvent::EntityTeleported { to, .. } if *to == Position::new(3, 3)),
        );
        assert!(
            teleported,
            "covetous monster should teleport to stairs when critically hurt"
        );

        // Monster should now be at stairs position.
        let pos = world.get_component::<Positioned>(monster).unwrap().0;
        assert_eq!(pos, Position::new(3, 3));
    }

    // ── Test 4: Intelligent monster opens doors ───────────────────

    #[test]
    fn intelligent_monster_opens_doors() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Put a closed door between monster and player.
        // Monster at (10,8), player at (8,8), door at (9,8).
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(9, 8), Terrain::DoorClosed);

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(10, 8),
            10,
            10,
            MonsterIntelligence::Humanoid,
        );

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Should have opened the door.
        let door_opened = events
            .iter()
            .any(|e| matches!(e, EngineEvent::DoorOpened { .. }));
        assert!(door_opened, "intelligent monster should open closed doors");

        // Verify the door is now open.
        let terrain = world
            .dungeon()
            .current_level
            .get(Position::new(9, 8))
            .unwrap()
            .terrain;
        assert_eq!(terrain, Terrain::DoorOpen, "door should now be open");
    }

    // ── Test 5: Animal monster does not use items ─────────────────

    #[test]
    fn animal_monster_does_not_use_items() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Animal monster at (12,8), low HP.
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            3,
            30,
            MonsterIntelligence::Animal,
        );

        // Give it a healing potion -- it should NOT use it.
        let potion = give_monster_potion(&mut world, monster, PotionType::Healing);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Should NOT have healed (no potion quaff).
        let healed = events
            .iter()
            .any(|e| matches!(e, EngineEvent::HpChange { amount, .. } if *amount > 0));
        assert!(!healed, "animal monster should not quaff healing potion");

        // Potion should still exist.
        assert!(
            world.get_component::<ObjectCore>(potion).is_some(),
            "potion should not be consumed by animal"
        );
    }

    // ── Test 6: Monster picks up weapon ───────────────────────────

    #[test]
    fn monster_picks_up_weapon() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Humanoid monster at (5,5), standing on a weapon.
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(5, 5),
            10,
            10,
            MonsterIntelligence::Humanoid,
        );

        // Place a weapon on the floor at the monster's position.
        let weapon = place_weapon_on_floor(&mut world, 5, 5);

        // Directly test the pickup function (not resolve_monster_turn,
        // because the monster may move off the item before pickup phase).
        let events = monster_pickup(&mut world, monster, &mut rng);

        // Check that the weapon was picked up.
        let picked_up = events
            .iter()
            .any(|e| matches!(e, EngineEvent::ItemPickedUp { item, .. } if *item == weapon));
        assert!(picked_up, "humanoid monster should pick up weapon");

        // Item location should now be monster inventory.
        let loc = world.get_component::<ObjectLocation>(weapon).unwrap();
        assert!(
            matches!(*loc, ObjectLocation::MonsterInventory { .. }),
            "weapon should be in monster inventory after pickup"
        );
    }

    // ── Test 7: Monster prefers ranged at distance ────────────────

    #[test]
    fn monster_prefers_ranged_at_distance() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Spellcaster monster at (12,8), full HP, player at (8,8).
        // Distance = 4, should prefer ranged over moving.
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            20,
            20,
            MonsterIntelligence::Spellcaster,
        );

        // Give monster a wand of magic missile.
        give_monster_wand(&mut world, monster, WandType::MagicMissile, 5);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Should have used ranged attack (zapped wand), not moved.
        let zapped = events.iter().any(|e| {
            if let EngineEvent::Message { key, .. } = e {
                key.contains("monster-uses-wand")
            } else {
                false
            }
        });
        assert!(
            zapped,
            "spellcaster should prefer ranged wand zap at distance"
        );

        // Monster should NOT have moved.
        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(
            !moved,
            "spellcaster using ranged attack should not also move"
        );
    }

    // ── Test 8: Enhanced flee timer behavior ──────────────────────

    #[test]
    fn enhanced_flee_timer_behavior() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Monster at (9,8) with full HP -- set a flee timer manually.
        let monster = spawn_monster_at(&mut world, Position::new(9, 8), 10, 10);
        let _ = world.ecs_mut().insert_one(monster, FleeTimer(5));

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Monster should flee (move away) due to active flee timer,
        // even though HP is full.
        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(moved, "monster with active flee timer should flee");

        // Should NOT have attacked (even though adjacent to player).
        let attacked = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
            )
        });
        assert!(
            !attacked,
            "fleeing monster (timer active) should not attack"
        );

        // Flee timer should have decremented by 1 (from 5 to 4).
        let timer = world.get_component::<FleeTimer>(monster).unwrap();
        assert_eq!(timer.0, 4, "flee timer should have decremented from 5 to 4");
    }

    // ── Test 9: Flee timer expires and monster resumes attacking ───

    #[test]
    fn flee_timer_expires_and_monster_attacks() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Monster at (9,8) adjacent to player, full HP, flee timer = 1.
        let monster = spawn_monster_at(&mut world, Position::new(9, 8), 10, 10);
        let _ = world.ecs_mut().insert_one(monster, FleeTimer(1));

        // First turn: timer = 1, should flee.
        let events1 = resolve_monster_turn(&mut world, monster, &mut rng);
        let fled = events1
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(fled, "monster with timer=1 should flee on first turn");

        // Timer should now be 0.
        {
            let timer = world.get_component::<FleeTimer>(monster).unwrap();
            assert_eq!(timer.0, 0, "timer should be 0 after decrement");
        }

        // Move monster back adjacent to player for next turn.
        if let Some(mut pos) = world.get_component_mut::<Positioned>(monster) {
            pos.0 = Position::new(9, 8);
        }

        // Second turn: timer = 0, full HP, should attack (not flee).
        let events2 = resolve_monster_turn(&mut world, monster, &mut rng);
        let attacked = events2.iter().any(|e| {
            matches!(
                e,
                EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
            )
        });
        assert!(
            attacked,
            "monster with expired flee timer and full HP should attack"
        );
    }

    // ── Test 10: Giant monster breaks doors ────────────────────────

    #[test]
    fn giant_monster_breaks_doors() {
        let mut world = make_test_world();

        // Place a closed door at (9,8).
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(9, 8), Terrain::DoorClosed);

        // Giant monster with animal intelligence but GIANT flag.
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(10, 8),
            20,
            20,
            MonsterIntelligence::Animal,
        );
        let _ = world
            .ecs_mut()
            .insert_one(monster, MonsterSpeciesFlags(MonsterFlags::GIANT));

        let events = try_open_door(&mut world, monster, Position::new(9, 8));

        // Should have broken the door.
        let door_broken = events
            .iter()
            .any(|e| matches!(e, EngineEvent::DoorBroken { .. }));
        assert!(door_broken, "giant monster should break closed doors");

        // Verify the door is now open.
        let terrain = world
            .dungeon()
            .current_level
            .get(Position::new(9, 8))
            .unwrap()
            .terrain;
        assert_eq!(
            terrain,
            Terrain::DoorOpen,
            "door should be open after giant breaks it"
        );
    }

    // ── Test 11: Animal does not pick up items ────────────────────

    #[test]
    fn animal_does_not_pick_up_items() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Animal monster standing on a weapon.
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(5, 5),
            10,
            10,
            MonsterIntelligence::Animal,
        );

        let weapon = place_weapon_on_floor(&mut world, 5, 5);

        let events = monster_pickup(&mut world, monster, &mut rng);

        // Should NOT pick up.
        let picked_up = events
            .iter()
            .any(|e| matches!(e, EngineEvent::ItemPickedUp { .. }));
        assert!(!picked_up, "animal monster should not pick up items");

        // Weapon should remain on floor.
        let loc = world.get_component::<ObjectLocation>(weapon).unwrap();
        assert!(
            matches!(*loc, ObjectLocation::Floor { .. }),
            "weapon should remain on the floor"
        );
    }

    // ── Test 12: Covetous monster heals at stairs ─────────────────

    #[test]
    fn covetous_monster_heals_at_stairs() {
        // Extend the map to 25x25 so we can place things far apart.
        let mut world = GameWorld::new(Position::new(14, 14));
        for y in 1..=23 {
            for x in 1..=23 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        let mut rng = test_rng();

        // Place stairs far from player: stairs at (2,2), player at (14,14).
        // dist2 = 12^2 + 12^2 = 288 > 64 (BOLT_LIM^2).
        let stairs_pos = Position::new(2, 2);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(stairs_pos, Terrain::StairsDown);

        // Covetous monster already at stairs, very low HP.
        let monster =
            spawn_intelligent_monster(&mut world, stairs_pos, 2, 30, MonsterIntelligence::Humanoid);
        let _ = world.ecs_mut().insert_one(monster, Covetous);

        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Should have healed (HpChange with positive amount).
        let healed = events
            .iter()
            .any(|e| matches!(e, EngineEvent::HpChange { amount, .. } if *amount > 0));
        assert!(healed, "covetous monster at stairs should heal");
    }

    // ═══════════════════════════════════════════════════════════════
    // Track G tests: Monster AI & Behavior System Alignment
    // ═══════════════════════════════════════════════════════════════

    // ── G.1: Special movement ────────────────────────────────────

    #[test]
    fn test_monster_fly_over_water() {
        let mut world = make_test_world();
        // Place water filling a row -- the only path to the player
        // is through water.  Block alternatives with walls.
        for y in 1..=15 {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(9, y), Terrain::Water);
        }

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(10, 8),
            10,
            10,
            MonsterIntelligence::Animal,
        );
        let _ = world
            .ecs_mut()
            .insert_one(monster, MonsterSpeciesFlags(MonsterFlags::FLY));

        // Verify is_valid_monster_move works for water tile.
        assert!(
            is_valid_monster_move(&world, Position::new(9, 8), monster),
            "flying monster should be able to move to water tile"
        );

        let mut rng = test_rng();
        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        // Flying monster should move through the water column.
        let moved = events.iter().any(|e| {
            matches!(e, EngineEvent::EntityMoved { to, .. }
                if to.x == 9) // moved into the water column
        });
        assert!(moved, "flying monster should be able to cross water");
    }

    #[test]
    fn test_monster_swim_through_water() {
        let mut world = make_test_world();
        // Fill column 9 with pool to force the path through water.
        for y in 1..=15 {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(9, y), Terrain::Pool);
        }

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(10, 8),
            10,
            10,
            MonsterIntelligence::Animal,
        );
        let _ = world
            .ecs_mut()
            .insert_one(monster, MonsterSpeciesFlags(MonsterFlags::SWIM));

        let mut rng = test_rng();
        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        let moved_through_water = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { to, .. } if to.x == 9));
        assert!(
            moved_through_water,
            "swimming monster should be able to cross pool"
        );
    }

    #[test]
    fn test_monster_non_swimmer_blocked_by_water() {
        let mut world = make_test_world();
        // Surround the monster with water on all sides toward player.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(9, 8), Terrain::Water);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(9, 7), Terrain::Water);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(9, 9), Terrain::Water);

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(10, 8),
            10,
            10,
            MonsterIntelligence::Animal,
        );
        // No FLY or SWIM flag -- should be blocked.

        let mut rng = test_rng();
        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        let moved_to_water = events.iter().any(|e| {
            matches!(e, EngineEvent::EntityMoved { to, .. }
                if *to == Position::new(9, 8)
                    || *to == Position::new(9, 7)
                    || *to == Position::new(9, 9))
        });
        assert!(
            !moved_to_water,
            "non-swimming monster should not cross water"
        );
    }

    #[test]
    fn test_monster_phase_through_walls() {
        let mut world = make_test_world();
        // Place a full wall column to force phasing.
        for y in 1..=15 {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(9, y), Terrain::Wall);
        }

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(10, 8),
            10,
            10,
            MonsterIntelligence::Animal,
        );
        let _ = world
            .ecs_mut()
            .insert_one(monster, MonsterSpeciesFlags(MonsterFlags::WALLWALK));

        // Wall blocks LOS, so monster won't see player. But WALLWALK
        // should still allow the move into wall tiles when wandering.
        // Instead, test is_valid_monster_move directly.
        assert!(
            is_valid_monster_move(&world, Position::new(9, 8), monster),
            "wall-walking monster should be able to move into wall tiles"
        );

        // Also verify terrain_passable_for directly.
        assert!(terrain_passable_for(Terrain::Wall, MonsterFlags::WALLWALK));
        assert!(terrain_passable_for(Terrain::Stone, MonsterFlags::WALLWALK));
    }

    #[test]
    fn test_monster_fly_over_lava() {
        let mut world = make_test_world();
        // Fill column 9 with lava to force the path through it.
        for y in 1..=15 {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(9, y), Terrain::Lava);
        }

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(10, 8),
            10,
            10,
            MonsterIntelligence::Animal,
        );
        let _ = world
            .ecs_mut()
            .insert_one(monster, MonsterSpeciesFlags(MonsterFlags::FLY));

        // Test passability directly (monster can't see through lava-walls to player).
        assert!(
            is_valid_monster_move(&world, Position::new(9, 8), monster),
            "flying monster should be able to move to lava tile"
        );
        assert!(terrain_passable_for(Terrain::Lava, MonsterFlags::FLY));
    }

    #[test]
    fn test_monster_non_flyer_blocked_by_lava() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(9, 8), Terrain::Lava);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(9, 7), Terrain::Lava);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(9, 9), Terrain::Lava);

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(10, 8),
            10,
            10,
            MonsterIntelligence::Animal,
        );
        let _ = world
            .ecs_mut()
            .insert_one(monster, MonsterSpeciesFlags(MonsterFlags::SWIM)); // swim doesn't help with lava

        let mut rng = test_rng();
        let events = resolve_monster_turn(&mut world, monster, &mut rng);

        let moved_to_lava = events.iter().any(|e| {
            matches!(e, EngineEvent::EntityMoved { to, .. }
                if *to == Position::new(9, 8)
                    || *to == Position::new(9, 7)
                    || *to == Position::new(9, 9))
        });
        assert!(
            !moved_to_lava,
            "swimming (non-flying) monster should not cross lava"
        );
    }

    #[test]
    fn test_monster_amorphous_flows_under_door() {
        let mut world = make_test_world();
        // Block all paths except through a closed door.
        for y in 1..=15 {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(9, y), Terrain::Wall);
        }
        // Replace the wall at (9,8) with a closed door.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(9, 8), Terrain::DoorClosed);

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(10, 8),
            10,
            10,
            MonsterIntelligence::Animal,
        );
        let _ = world
            .ecs_mut()
            .insert_one(monster, MonsterSpeciesFlags(MonsterFlags::AMORPHOUS));

        // Verify the amorphous monster can move through the closed door.
        assert!(
            is_valid_monster_move(&world, Position::new(9, 8), monster),
            "amorphous monster should be able to move through closed doors"
        );
        assert!(terrain_passable_for(
            Terrain::DoorClosed,
            MonsterFlags::AMORPHOUS
        ));
    }

    // ── G.1: Teleporting monsters ────────────────────────────────

    #[test]
    fn test_monster_teleport_with_tport_flag() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            10,
            10,
            MonsterIntelligence::Humanoid,
        );
        let _ = world
            .ecs_mut()
            .insert_one(monster, MonsterSpeciesFlags(MonsterFlags::TPORT));

        // Run many turns; at least one should teleport (1/5 chance per turn).
        let mut teleported = false;
        for _ in 0..50 {
            // Reset position each time.
            if let Some(mut pos) = world.get_component_mut::<Positioned>(monster) {
                pos.0 = Position::new(12, 8);
            }
            let events = resolve_monster_turn(&mut world, monster, &mut rng);
            if events
                .iter()
                .any(|e| matches!(e, EngineEvent::EntityTeleported { .. }))
            {
                teleported = true;
                break;
            }
        }
        assert!(
            teleported,
            "monster with TPORT flag should teleport randomly within 50 turns"
        );
    }

    // ── G.1: Covetous harass teleport ────────────────────────────

    #[test]
    fn test_monster_covetous_harass_teleport() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Full-HP covetous monster far from player.
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(14, 14),
            30,
            30,
            MonsterIntelligence::Humanoid,
        );
        let _ = world.ecs_mut().insert_one(monster, Covetous);

        // Place stairs so covetous_behavior doesn't fail on no-stairs.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(2, 2), Terrain::StairsDown);

        // Run many turns; at least one should harass-teleport (1/5 chance).
        let mut teleported_near = false;
        for _ in 0..100 {
            if let Some(mut pos) = world.get_component_mut::<Positioned>(monster) {
                pos.0 = Position::new(14, 14);
            }
            let events = resolve_monster_turn(&mut world, monster, &mut rng);
            if events
                .iter()
                .any(|e| matches!(e, EngineEvent::EntityTeleported { .. }))
            {
                // Check the teleport was to near the player.
                let new_pos = world.get_component::<Positioned>(monster).unwrap().0;
                let player_pos = Position::new(8, 8);
                if chebyshev_distance(new_pos, player_pos) <= 2 {
                    teleported_near = true;
                    break;
                }
            }
        }
        assert!(
            teleported_near,
            "full-HP covetous monster should sometimes harass-teleport near player"
        );
    }

    // ── G.1: Door interaction (no-hands) ─────────────────────────

    #[test]
    fn test_monster_nohands_cannot_open_door() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(9, 8), Terrain::DoorClosed);

        // Non-animal but NOHANDS monster (e.g., a blob).
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(10, 8),
            10,
            10,
            MonsterIntelligence::Humanoid,
        );
        let _ = world
            .ecs_mut()
            .insert_one(monster, MonsterSpeciesFlags(MonsterFlags::NOHANDS));

        let events = try_open_door(&mut world, monster, Position::new(9, 8));
        let opened = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::DoorOpened { .. } | EngineEvent::DoorBroken { .. }
            )
        });
        assert!(!opened, "monster with NOHANDS should not open doors");

        // Verify door is still closed.
        let terrain = world
            .dungeon()
            .current_level
            .get(Position::new(9, 8))
            .unwrap()
            .terrain;
        assert_eq!(terrain, Terrain::DoorClosed, "door should remain closed");
    }

    // ── G.1: Monster regeneration ────────────────────────────────

    #[test]
    fn test_monster_regen_every_20_turns() {
        let mut world = make_test_world();
        let mut events = Vec::new();

        let monster = spawn_monster_at(&mut world, Position::new(5, 5), 5, 10);

        // No REGEN flag -- should heal on turn 20 but not turn 19.
        monster_regen(&mut world, monster, 19, &mut events);
        assert!(
            events.is_empty(),
            "non-REGEN monster should not heal on turn 19"
        );

        monster_regen(&mut world, monster, 20, &mut events);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::HpChange { amount: 1, .. })),
            "non-REGEN monster should heal 1 HP on turn 20"
        );

        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert_eq!(hp.current, 6, "HP should be 6 after regen");
    }

    #[test]
    fn test_monster_regen_with_regen_flag() {
        let mut world = make_test_world();
        let mut events = Vec::new();

        let monster = spawn_monster_at(&mut world, Position::new(5, 5), 5, 10);
        let _ = world
            .ecs_mut()
            .insert_one(monster, MonsterSpeciesFlags(MonsterFlags::REGEN));

        // REGEN flag -- should heal every turn.
        monster_regen(&mut world, monster, 1, &mut events);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::HpChange { amount: 1, .. })),
            "REGEN monster should heal on turn 1"
        );
        {
            let hp = world.get_component::<HitPoints>(monster).unwrap();
            assert_eq!(hp.current, 6);
        }

        events.clear();
        monster_regen(&mut world, monster, 2, &mut events);
        {
            let hp = world.get_component::<HitPoints>(monster).unwrap();
            assert_eq!(hp.current, 7, "REGEN monster should heal on turn 2 too");
        }
    }

    #[test]
    fn test_monster_regen_does_not_exceed_max() {
        let mut world = make_test_world();
        let mut events = Vec::new();

        let monster = spawn_monster_at(&mut world, Position::new(5, 5), 10, 10);
        let _ = world
            .ecs_mut()
            .insert_one(monster, MonsterSpeciesFlags(MonsterFlags::REGEN));

        monster_regen(&mut world, monster, 1, &mut events);
        assert!(events.is_empty(), "should not regen when HP is full");

        let hp = world.get_component::<HitPoints>(monster).unwrap();
        assert_eq!(hp.current, 10, "HP should stay at max");
    }

    // ── G.2: Difficulty window ───────────────────────────────────

    #[test]
    fn test_monster_difficulty_window_basic() {
        // Spec T4: zlevel=12, ulevel=8
        let (min, max) = difficulty_window(12, 8);
        assert_eq!(min, 2, "monmin_difficulty(12) = 12/6 = 2");
        assert_eq!(max, 10, "monmax_difficulty(12, 8) = (12+8)/2 = 10");
    }

    #[test]
    fn test_monster_difficulty_window_depth1() {
        // Spec T5: zlevel=1, ulevel=1
        let (min, max) = difficulty_window(1, 1);
        assert_eq!(min, 0, "monmin_difficulty(1) = 1/6 = 0");
        assert_eq!(max, 1, "monmax_difficulty(1, 1) = (1+1)/2 = 1");
    }

    #[test]
    fn test_monster_difficulty_window_depth0() {
        // Spec T6: zlevel=0, ulevel=1
        let (min, max) = difficulty_window(0, 1);
        assert_eq!(min, 0, "monmin_difficulty(0) = 0/6 = 0");
        assert_eq!(max, 0, "monmax_difficulty(0, 1) = (0+1)/2 = 0");
    }

    // ── G.2: adj_lev ─────────────────────────────────────────────

    #[test]
    fn test_monster_adj_lev_basic() {
        // Spec T7: mlevel=5, depth=15, plvl=10
        let result = adj_lev(5, 15, 10);
        // tmp = 5 + (15-5)/5 = 5+2 = 7
        // player_diff = 10-5 = 5, tmp += 5/4 = 1 -> 8
        // upper = min(3*5/2, 49) = 7
        // clamp(8, 0, 7) = 7
        assert_eq!(result, 7, "adj_lev(5, 15, 10) should be 7");
    }

    #[test]
    fn test_monster_adj_lev_low_depth() {
        // mlevel=1, depth=1, plvl=1 (spec boundary #4)
        let result = adj_lev(1, 1, 1);
        // diff = 1-1=0, tmp = 1 + 0/5 = 1
        // player_diff = 1-1 = 0 (not > 0, no boost)
        // upper = min(3*1/2, 49) = 1
        assert_eq!(result, 1, "adj_lev(1, 1, 1) should be 1");
    }

    #[test]
    fn test_monster_adj_lev_capped() {
        // mlevel=1, depth=50, plvl=30 (spec boundary #5)
        let result = adj_lev(1, 50, 30);
        // diff = 50-1 = 49, tmp = 1 + 49/5 = 1+9 = 10
        // player_diff = 30-1 = 29, tmp += 29/4 = 7 -> 17
        // upper = min(3*1/2, 49) = 1
        // clamp(17, 0, 1) = 1
        assert_eq!(result, 1, "adj_lev(1, 50, 30) should be capped at 1");
    }

    #[test]
    fn test_monster_adj_lev_special_demon() {
        // mlevel > 49 returns 50.
        assert_eq!(adj_lev(106, 25, 15), 50, "mlevel > 49 should return 50");
    }

    #[test]
    fn test_monster_adj_lev_harder_than_depth() {
        // When monster is harder than depth, tmp decreases by 1.
        let result = adj_lev(10, 5, 5);
        // diff = 5 - 10 = -5, tmp = 10 - 1 = 9
        // player_diff = 5 - 10 = -5 (not > 0)
        // upper = min(15, 49) = 15
        // clamp(9, 0, 15) = 9
        assert_eq!(result, 9, "adj_lev(10, 5, 5) with harder monster");
    }

    // ── G.2: Spawn rate ──────────────────────────────────────────

    #[test]
    fn test_monster_spawn_rate_normal() {
        assert_eq!(spawn_rate(false, 10), 70, "normal dungeon spawn rate is 70");
    }

    #[test]
    fn test_monster_spawn_rate_deep() {
        assert_eq!(spawn_rate(false, 30), 50, "deep dungeon spawn rate is 50");
    }

    #[test]
    fn test_monster_spawn_rate_wizard_killed() {
        assert_eq!(spawn_rate(true, 10), 25, "post-wizard spawn rate is 25");
    }

    // ── G.2: Group generation ────────────────────────────────────

    #[test]
    fn test_monster_group_size_small_low_level() {
        let mut rng = test_rng();
        // At low player level (< 3), group is always 1.
        for _ in 0..20 {
            let size = group_size(false, 1, &mut rng);
            assert_eq!(size, 1, "small group at plvl 1 should always be 1");
        }
    }

    #[test]
    fn test_monster_group_size_large_high_level() {
        let mut rng = test_rng();
        // At high player level, large groups can go up to 10.
        let mut sizes: Vec<u32> = Vec::new();
        for _ in 0..100 {
            sizes.push(group_size(true, 10, &mut rng));
        }
        let max_size = *sizes.iter().max().unwrap();
        let min_size = *sizes.iter().min().unwrap();
        assert!(min_size >= 1, "group size should be at least 1");
        assert!(
            max_size >= 3,
            "large group at plvl 10 should sometimes be >= 3"
        );
        assert!(max_size <= 10, "large group should not exceed 10");
    }

    // ── G.3: Monster action ordering (verified by turn.rs) ──────

    #[test]
    fn test_monster_ordering_speed_desc_creation_asc() {
        let mut world = make_test_world();
        // Place player out of the way.
        if let Some(mut pos) = world.get_component_mut::<Positioned>(world.player()) {
            pos.0 = Position::new(1, 1);
        }

        // Spawn fast monster first (speed 18), then slow monster (speed 6).
        let fast_order = world.next_creation_order();
        let fast = world.spawn((
            Monster,
            Positioned(Position::new(14, 14)),
            HitPoints {
                current: 10,
                max: 10,
            },
            ArmorClass(10),
            Attributes::default(),
            ExperienceLevel(1),
            Speed(18),
            MovementPoints(NORMAL_SPEED as i32),
            Name("fast".to_string()),
            fast_order,
        ));
        let slow_order = world.next_creation_order();
        let slow = world.spawn((
            Monster,
            Positioned(Position::new(14, 12)),
            HitPoints {
                current: 10,
                max: 10,
            },
            ArmorClass(10),
            Attributes::default(),
            ExperienceLevel(1),
            Speed(6),
            MovementPoints(NORMAL_SPEED as i32),
            Name("slow".to_string()),
            slow_order,
        ));

        // Collect sorted order.
        let mut monsters: Vec<(Entity, u32, u64)> = Vec::new();
        for (entity, (speed, mp, _)) in world
            .ecs()
            .query::<(&Speed, &MovementPoints, &Monster)>()
            .iter()
        {
            if mp.0 >= NORMAL_SPEED as i32 {
                let creation = world
                    .get_component::<CreationOrder>(entity)
                    .map(|c| c.0)
                    .unwrap_or(u64::MAX);
                monsters.push((entity, speed.0, creation));
            }
        }
        monsters.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));

        assert_eq!(monsters.len(), 2);
        assert_eq!(monsters[0].0, fast, "fast monster should act first");
        assert_eq!(monsters[1].0, slow, "slow monster should act second");
    }

    // ── G.1: HP-low flee threshold ───────────────────────────────

    #[test]
    fn test_monster_flee_hp_threshold() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // HP = 3, max = 12.  3 < 12/3 = 4 -> should flee.
        let monster1 = spawn_monster_at(&mut world, Position::new(9, 8), 3, 12);
        assert!(
            is_fleeing(&mut world, monster1, 3, 12, &mut rng),
            "HP 3/12 should trigger flee (3 < 4)"
        );

        // HP = 4, max = 12.  4 < 12/3=4 is FALSE -> should NOT flee.
        let monster2 = spawn_monster_at(&mut world, Position::new(12, 12), 4, 12);
        let mut rng2 = test_rng();
        assert!(
            !is_fleeing(&mut world, monster2, 4, 12, &mut rng2),
            "HP 4/12 should NOT trigger flee (4 >= 4)"
        );
    }

    // ── G: terrain_passable_for unit tests ───────────────────────

    #[test]
    fn test_terrain_passable_floor_always() {
        assert!(terrain_passable_for(Terrain::Floor, MonsterFlags::empty()));
        assert!(terrain_passable_for(Terrain::Floor, MonsterFlags::FLY));
        assert!(terrain_passable_for(
            Terrain::Corridor,
            MonsterFlags::empty()
        ));
    }

    #[test]
    fn test_terrain_passable_water_requires_flag() {
        assert!(!terrain_passable_for(Terrain::Water, MonsterFlags::empty()));
        assert!(terrain_passable_for(Terrain::Water, MonsterFlags::FLY));
        assert!(terrain_passable_for(Terrain::Water, MonsterFlags::SWIM));
    }

    #[test]
    fn test_terrain_passable_lava_only_fly() {
        assert!(!terrain_passable_for(Terrain::Lava, MonsterFlags::empty()));
        assert!(!terrain_passable_for(Terrain::Lava, MonsterFlags::SWIM));
        assert!(terrain_passable_for(Terrain::Lava, MonsterFlags::FLY));
    }

    #[test]
    fn test_terrain_passable_wall_only_wallwalk() {
        assert!(!terrain_passable_for(Terrain::Wall, MonsterFlags::empty()));
        assert!(!terrain_passable_for(Terrain::Wall, MonsterFlags::FLY));
        assert!(terrain_passable_for(Terrain::Wall, MonsterFlags::WALLWALK));
        assert!(terrain_passable_for(Terrain::Stone, MonsterFlags::WALLWALK));
    }

    #[test]
    fn test_terrain_passable_door_amorphous() {
        assert!(!terrain_passable_for(
            Terrain::DoorClosed,
            MonsterFlags::empty()
        ));
        assert!(terrain_passable_for(
            Terrain::DoorClosed,
            MonsterFlags::AMORPHOUS
        ));
        assert!(terrain_passable_for(
            Terrain::DoorLocked,
            MonsterFlags::AMORPHOUS
        ));
    }

    // ── G: dist2_positions ───────────────────────────────────────

    #[test]
    fn test_dist2_positions() {
        assert_eq!(
            dist2_positions(Position::new(0, 0), Position::new(3, 4)),
            25
        );
        assert_eq!(dist2_positions(Position::new(5, 5), Position::new(5, 5)), 0);
        assert_eq!(
            dist2_positions(Position::new(0, 0), Position::new(8, 0)),
            64
        );
    }

    // ═══════════════════════════════════════════════════════════════
    // Wave 5E tests: Monster spellcasting & throwing
    // ═══════════════════════════════════════════════════════════════

    // ── Spellcasting ─────────────────────────────────────────────

    #[test]
    fn test_choose_mage_spell_low_level() {
        let mut rng = test_rng();
        // Low-level (1) monster — should only get PsiBolt or CureSelf.
        let mut saw_psi = false;
        for _ in 0..100 {
            let spell = choose_mage_spell(1, &mut rng);
            if spell == MageSpell::PsiBolt {
                saw_psi = true;
            }
        }
        assert!(saw_psi, "level 1 should mostly cast PsiBolt");
    }

    #[test]
    fn test_choose_mage_spell_high_level_gets_death_touch() {
        let mut rng = test_rng();
        let mut saw_death = false;
        for _ in 0..500 {
            if choose_mage_spell(24, &mut rng) == MageSpell::DeathTouch {
                saw_death = true;
                break;
            }
        }
        assert!(
            saw_death,
            "level 24 mage should sometimes choose DeathTouch"
        );
    }

    #[test]
    fn test_choose_cleric_spell_low_level() {
        let mut rng = test_rng();
        let mut saw_wounds = false;
        for _ in 0..100 {
            if choose_cleric_spell(1, &mut rng) == ClericSpell::OpenWounds {
                saw_wounds = true;
                break;
            }
        }
        assert!(
            saw_wounds,
            "level 1 cleric should sometimes cast OpenWounds"
        );
    }

    #[test]
    fn test_choose_cleric_spell_high_level_gets_geyser() {
        let mut rng = test_rng();
        let mut saw_geyser = false;
        for _ in 0..500 {
            if choose_cleric_spell(24, &mut rng) == ClericSpell::Geyser {
                saw_geyser = true;
                break;
            }
        }
        assert!(saw_geyser, "level 24 cleric should sometimes choose Geyser");
    }

    #[test]
    fn test_monster_cast_mage_spell_psi_bolt_damages_player() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            20,
            20,
            MonsterIntelligence::Spellcaster,
        );
        let _ = world.ecs_mut().insert_one(
            monster,
            Spellcaster {
                monster_level: 1, // low level => only PsiBolt or CureSelf
                is_cleric: false,
            },
        );

        // Cast several times and check at least one damages the player.
        let mut damaged = false;
        for _ in 0..20 {
            let orig_hp = world.get_component::<HitPoints>(player).unwrap().current;
            let events = monster_cast_spell(&mut world, monster, player, &mut rng);

            let player_hp = world.get_component::<HitPoints>(player).unwrap().current;

            if player_hp < orig_hp {
                damaged = true;
                // Should have a HpChange event.
                assert!(events.iter().any(|e| matches!(
                    e,
                    EngineEvent::HpChange { amount, .. } if *amount < 0
                )));
                break;
            }

            // Reset HP for next attempt.
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                hp.current = hp.max;
            }
        }
        assert!(
            damaged,
            "PsiBolt should damage the player at least once in 20 casts"
        );
    }

    #[test]
    fn test_monster_cast_cleric_heal_self() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            5, // low HP
            30,
            MonsterIntelligence::Spellcaster,
        );
        let _ = world.ecs_mut().insert_one(
            monster,
            Spellcaster {
                monster_level: 2, // low level => OpenWounds or CureSelf
                is_cleric: true,
            },
        );

        // Cast several times; check if CureSelf heals the monster.
        let mut healed = false;
        for _ in 0..50 {
            // Reset monster HP to be hurt.
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(monster) {
                hp.current = 5;
            }
            let events = monster_cast_spell(&mut world, monster, player, &mut rng);
            if events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::HpChange { entity, amount, source: HpSource::Spell, .. }
                        if *entity == monster && *amount > 0
                )
            }) {
                healed = true;
                break;
            }
        }
        assert!(healed, "cleric CureSelf should heal the monster");
    }

    #[test]
    fn test_monster_cast_spell_no_spellcaster_component() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = spawn_monster_at(&mut world, Position::new(12, 8), 10, 10);

        let events = monster_cast_spell(&mut world, monster, player, &mut rng);
        assert!(
            events.is_empty(),
            "monster without Spellcaster component should do nothing"
        );
    }

    #[test]
    fn test_monster_cast_cleric_geyser_stuns_player() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            20,
            20,
            MonsterIntelligence::Spellcaster,
        );
        let _ = world.ecs_mut().insert_one(
            monster,
            Spellcaster {
                monster_level: 24,
                is_cleric: true,
            },
        );

        let mut stunned = false;
        for _ in 0..200 {
            // Reset player HP each time.
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                hp.current = hp.max;
            }
            let events = monster_cast_spell(&mut world, monster, player, &mut rng);
            if events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::StatusApplied {
                        status: StatusEffect::Stunned,
                        ..
                    }
                )
            }) {
                stunned = true;
                break;
            }
        }
        assert!(
            stunned,
            "high-level cleric should sometimes cast Geyser which stuns"
        );
    }

    #[test]
    fn test_monster_cast_mage_haste_self() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            20,
            20,
            MonsterIntelligence::Spellcaster,
        );
        let _ = world.ecs_mut().insert_one(
            monster,
            Spellcaster {
                monster_level: 5, // can cast HasteSelf (spellval 4-5)
                is_cleric: false,
            },
        );

        let mut hasted = false;
        for _ in 0..100 {
            let events = monster_cast_spell(&mut world, monster, player, &mut rng);
            if events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::StatusApplied {
                        entity,
                        status: StatusEffect::FastSpeed,
                        ..
                    } if *entity == monster
                )
            }) {
                hasted = true;
                break;
            }
        }
        assert!(hasted, "mage should sometimes cast HasteSelf");
    }

    // ── Monster throwing ─────────────────────────────────────────

    /// Create a throwable item in a monster's inventory.
    fn give_monster_throwable(
        world: &mut GameWorld,
        monster: Entity,
        dice_count: u8,
        dice_sides: u8,
        quantity: u32,
    ) -> Entity {
        let carrier_id = monster.to_bits().get() as u32;
        let core = ObjectCore {
            otyp: ObjectTypeId(400),
            object_class: ObjectClass::Weapon,
            quantity: quantity as i32,
            weight: 10,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let loc = ObjectLocation::MonsterInventory { carrier_id };
        world.spawn((
            core,
            loc,
            Throwable {
                dice_count,
                dice_sides,
            },
        ))
    }

    #[test]
    fn test_should_monster_throw_with_throwable_in_range() {
        let mut world = make_test_world();
        let player_pos = Position::new(8, 8);

        // Monster at distance 4 with throwable.
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            10,
            10,
            MonsterIntelligence::Humanoid,
        );
        give_monster_throwable(&mut world, monster, 1, 6, 5);

        assert!(
            should_monster_throw(&world, monster, player_pos),
            "monster with throwable at range 4 should want to throw"
        );
    }

    #[test]
    fn test_should_monster_throw_too_close() {
        let mut world = make_test_world();
        let player_pos = Position::new(8, 8);

        // Monster adjacent (distance 1) — should NOT throw.
        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(9, 8),
            10,
            10,
            MonsterIntelligence::Humanoid,
        );
        give_monster_throwable(&mut world, monster, 1, 6, 5);

        assert!(
            !should_monster_throw(&world, monster, player_pos),
            "monster adjacent should prefer melee over throw"
        );
    }

    #[test]
    fn test_should_monster_throw_no_throwable() {
        let mut world = make_test_world();
        let player_pos = Position::new(8, 8);

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            10,
            10,
            MonsterIntelligence::Humanoid,
        );
        // No throwable item given.

        assert!(
            !should_monster_throw(&world, monster, player_pos),
            "monster without throwable should not throw"
        );
    }

    #[test]
    fn test_monster_throw_item_hits_or_misses() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            20,
            20,
            MonsterIntelligence::Humanoid,
        );
        let _ = world
            .ecs_mut()
            .insert_one(monster, crate::world::ExperienceLevel(5));

        // Give multiple throwables.
        give_monster_throwable(&mut world, monster, 2, 6, 10);

        let mut hit_count = 0;
        let mut miss_count = 0;
        for _ in 0..30 {
            // Reset player HP.
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                hp.current = hp.max;
            }
            // Give fresh throwables if consumed.
            let items = get_monster_inventory(&world, monster);
            let has_throwable = items
                .iter()
                .any(|&i| world.get_component::<Throwable>(i).is_some());
            if !has_throwable {
                give_monster_throwable(&mut world, monster, 2, 6, 10);
            }

            let events = monster_throw_item(&mut world, monster, player, &mut rng);
            if events
                .iter()
                .any(|e| matches!(e, EngineEvent::RangedHit { .. }))
            {
                hit_count += 1;
            }
            if events
                .iter()
                .any(|e| matches!(e, EngineEvent::RangedMiss { .. }))
            {
                miss_count += 1;
            }
        }
        assert!(
            hit_count > 0,
            "monster should hit at least once in 30 throws"
        );
        assert!(
            miss_count > 0 || hit_count == 30,
            "results should be a mix of hits and misses (or all hits)"
        );
    }

    #[test]
    fn test_monster_throw_consumes_item() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            20,
            20,
            MonsterIntelligence::Humanoid,
        );

        // Give exactly 1 throwable.
        let item = give_monster_throwable(&mut world, monster, 1, 6, 1);

        monster_throw_item(&mut world, monster, player, &mut rng);

        // Item should be despawned (quantity was 1).
        assert!(
            world.get_component::<ObjectCore>(item).is_none(),
            "throwable with quantity=1 should be despawned after throw"
        );
    }

    #[test]
    fn test_monster_throw_decrements_stack() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = spawn_intelligent_monster(
            &mut world,
            Position::new(12, 8),
            20,
            20,
            MonsterIntelligence::Humanoid,
        );

        // Give 5 throwables.
        let item = give_monster_throwable(&mut world, monster, 1, 6, 5);

        monster_throw_item(&mut world, monster, player, &mut rng);

        // Item should still exist with quantity 4.
        let core = world.get_component::<ObjectCore>(item).unwrap();
        assert_eq!(
            core.quantity, 4,
            "throwable stack should decrement from 5 to 4"
        );
    }

    #[test]
    fn test_monster_throw_no_throwable_is_noop() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let monster = spawn_monster_at(&mut world, Position::new(12, 8), 10, 10);

        let events = monster_throw_item(&mut world, monster, player, &mut rng);
        assert!(
            events.is_empty(),
            "throw with no throwable items should be noop"
        );
    }

    // ── Demon lord identification ────────────────────────────────

    fn spawn_demon_lord(world: &mut GameWorld, pos: Position, name: &str) -> Entity {
        world.spawn((
            Monster,
            Positioned(pos),
            HitPoints {
                current: 100,
                max: 100,
            },
            ArmorClass(0),
            Attributes::default(),
            ExperienceLevel(25),
            Speed(18),
            MovementPoints(NORMAL_SPEED as i32),
            Name(name.to_string()),
            MonsterSpeciesFlags(MonsterFlags::DEMON | MonsterFlags::PRINCE),
        ))
    }

    #[test]
    fn test_identify_orcus() {
        let mut world = make_test_world();
        let orcus = spawn_demon_lord(&mut world, Position::new(5, 5), "Orcus");

        assert_eq!(identify_demon_lord(&world, orcus), Some(DemonLord::Orcus),);
    }

    #[test]
    fn test_identify_demogorgon() {
        let mut world = make_test_world();
        let demo = spawn_demon_lord(&mut world, Position::new(5, 5), "Demogorgon");

        assert_eq!(
            identify_demon_lord(&world, demo),
            Some(DemonLord::Demogorgon),
        );
    }

    #[test]
    fn test_identify_non_demon_returns_none() {
        let mut world = make_test_world();
        let goblin = spawn_monster_at(&mut world, Position::new(5, 5), 10, 10);

        assert_eq!(identify_demon_lord(&world, goblin), None);
    }

    // ── Demon lord special actions ───────────────────────────────

    #[test]
    fn test_orcus_zaps_wand_of_death() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let orcus = spawn_demon_lord(&mut world, Position::new(12, 8), "Orcus");

        // Give Orcus a wand of death.
        let carrier_id = orcus.to_bits().get() as u32;
        let core = ObjectCore {
            otyp: ObjectTypeId(200),
            object_class: ObjectClass::Tool,
            quantity: 1,
            weight: 7,
            age: 0,
            inv_letter: None,
            artifact: None,
        };
        let loc = ObjectLocation::MonsterInventory { carrier_id };
        let wand_charges = WandCharges {
            spe: 3,
            recharged: 0,
        };
        let _wand = world.spawn((core, loc, WandTypeTag(WandType::Death), wand_charges));

        let orig_hp = world.get_component::<HitPoints>(player).unwrap().current;

        let events = demon_lord_special(
            &mut world,
            orcus,
            DemonLord::Orcus,
            player,
            4,
            true,
            &mut rng,
        );

        assert!(!events.is_empty(), "Orcus should zap wand of death");
        let new_hp = world.get_component::<HitPoints>(player).unwrap().current;
        assert!(new_hp < orig_hp, "player should take damage from death ray");
    }

    #[test]
    fn test_demogorgon_disease_gaze() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let demo = spawn_demon_lord(&mut world, Position::new(12, 8), "Demogorgon");

        let events = demon_lord_special(
            &mut world,
            demo,
            DemonLord::Demogorgon,
            player,
            4,
            true,
            &mut rng,
        );

        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::Sick,
                    ..
                }
            )),
            "Demogorgon should apply disease via gaze"
        );
    }

    #[test]
    fn test_yeenoghu_confusion_attack() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let yeen = spawn_demon_lord(&mut world, Position::new(9, 8), "Yeenoghu");

        // Distance 1 — melee range.
        let events = demon_lord_special(
            &mut world,
            yeen,
            DemonLord::Yeenoghu,
            player,
            1,
            true,
            &mut rng,
        );

        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::Confused,
                    ..
                }
            )),
            "Yeenoghu should confuse at melee range"
        );

        let player_hp = world.get_component::<HitPoints>(player).unwrap().current;
        let orig_hp = world.get_component::<HitPoints>(player).unwrap().max;
        assert!(player_hp < orig_hp, "Yeenoghu should also deal damage");
    }

    #[test]
    fn test_asmodeus_cold_blast_at_range() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let asmo = spawn_demon_lord(&mut world, Position::new(12, 8), "Asmodeus");

        let orig_hp = world.get_component::<HitPoints>(player).unwrap().current;

        let events = demon_lord_special(
            &mut world,
            asmo,
            DemonLord::Asmodeus,
            player,
            4,
            true,
            &mut rng,
        );

        assert!(
            !events.is_empty(),
            "Asmodeus should use cold blast at range"
        );
        let new_hp = world.get_component::<HitPoints>(player).unwrap().current;
        assert!(new_hp < orig_hp, "player should take cold damage");
    }

    #[test]
    fn test_juiblex_acid_spit_at_range() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let jui = spawn_demon_lord(&mut world, Position::new(12, 8), "Juiblex");

        let orig_hp = world.get_component::<HitPoints>(player).unwrap().current;

        let events = demon_lord_special(
            &mut world,
            jui,
            DemonLord::Juiblex,
            player,
            3,
            true,
            &mut rng,
        );

        assert!(!events.is_empty(), "Juiblex should spit acid at range");
        let new_hp = world.get_component::<HitPoints>(player).unwrap().current;
        assert!(new_hp < orig_hp, "player should take acid damage");
    }

    #[test]
    fn test_baalzebub_poison_sting_melee() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let baal = spawn_demon_lord(&mut world, Position::new(9, 8), "Baalzebub");

        let events = demon_lord_special(
            &mut world,
            baal,
            DemonLord::Baalzebub,
            player,
            1,
            true,
            &mut rng,
        );

        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::Sick,
                    ..
                }
            )),
            "Baalzebub should apply sickness from poison sting"
        );
    }

    #[test]
    fn test_demon_lord_no_action_when_out_of_range() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Baalzebub at distance > 1 — his special is melee-only.
        let baal = spawn_demon_lord(&mut world, Position::new(12, 8), "Baalzebub");

        let events = demon_lord_special(
            &mut world,
            baal,
            DemonLord::Baalzebub,
            player,
            4,
            true,
            &mut rng,
        );

        assert!(
            events.is_empty(),
            "Baalzebub at range should not do special action"
        );
    }

    // ── Leprechaun avoidance ─────────────────────────────────────

    #[test]
    fn leprechaun_teleports_when_has_gold_and_close() {
        assert_eq!(
            leprechaun_avoidance(true, true, 1),
            LeprechaunAction::TeleportAway,
        );
        assert_eq!(
            leprechaun_avoidance(true, true, 2),
            LeprechaunAction::TeleportAway,
        );
    }

    #[test]
    fn leprechaun_flees_when_has_gold_but_cannot_teleport() {
        assert_eq!(
            leprechaun_avoidance(true, false, 1),
            LeprechaunAction::FleeFromPlayer,
        );
    }

    #[test]
    fn leprechaun_normal_when_no_gold() {
        assert_eq!(
            leprechaun_avoidance(false, true, 1),
            LeprechaunAction::Normal,
        );
        assert_eq!(
            leprechaun_avoidance(false, false, 5),
            LeprechaunAction::Normal,
        );
    }
}
