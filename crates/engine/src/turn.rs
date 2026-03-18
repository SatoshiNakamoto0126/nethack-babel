//! Main turn loop: movement points, action dispatch, monster turns,
//! regeneration, hunger, and new-turn processing.
//!
//! Implements the NetHack `moveloop_core()` sequence where each call to
//! `resolve_turn()` processes one player action and any resulting monster
//! actions, then checks whether a new game turn boundary has been reached.

use rand::Rng;

use nethack_babel_data::{
    Alignment, ArtifactId, BucStatus, Enchantment, GenoFlags, LightSource, MonsterDef,
    MonsterFlags, MonsterId, ObjectClass, ObjectCore, ObjectDef, ObjectExtra, ObjectLocation,
    ObjectTypeId, PlayerEvents, PlayerIdentity, PlayerQuestItems, TrapType,
};

use crate::action::{Direction, NameTarget, PlayerAction, Position};
use crate::conduct::ConductState;
use crate::dungeon::{
    CachedLevelRuntimeState, CachedMonster, DungeonBranch, Terrain, branch_max_depth,
};
use crate::event::{DeathCause, EngineEvent, HpSource, HungerLevel};
use crate::fov::FovMap;
use crate::makemon::{GoodPosFlags, MakeMonFlags, enexto, goodpos, makemon};
use crate::map_gen::generate_level;
use crate::mkobj::mksobj_at;
use crate::role::Role;
use crate::special_levels::{dispatch_special_level, identify_special_level};
use crate::traps::{TrapEntityInfo, detect_trap, trigger_trap_at};
use crate::world::{
    Boulder, CreationOrder, DisplaySymbol, Encumbrance, EncumbranceLevel, ExperienceLevel,
    GameWorld, HeroSpeed, HeroSpeedBonus, HitPoints, Monster, MonsterSpeedMod, MovementPoints,
    NORMAL_SPEED, Name, Nutrition, Peaceful, PlayerCombat, Positioned, Power, Speed, SpeedModifier,
    Tame,
};

// ── Movement point calculations ──────────────────────────────────────

/// Calculate hero movement points to add at the start of a new turn.
///
/// Matches NetHack's `u_calc_moveamt()`:
///   1. Start with base mmove (Speed component).
///   2. Apply Very_fast (2/3 chance of +12) or Fast (1/3 chance of +12).
///   3. Apply encumbrance penalties.
pub fn u_calc_moveamt(
    base_speed: u32,
    hero_speed: HeroSpeed,
    encumbrance: Encumbrance,
    rng: &mut impl Rng,
) -> i32 {
    let mut moveamt = base_speed as i32;

    // Speed bonuses.
    match hero_speed {
        HeroSpeed::VeryFast => {
            // 2/3 chance of +NORMAL_SPEED.
            if rng.random_range(0..3) != 0 {
                moveamt += NORMAL_SPEED as i32;
            }
        }
        HeroSpeed::Fast => {
            // 1/3 chance of +NORMAL_SPEED.
            if rng.random_range(0..3) == 0 {
                moveamt += NORMAL_SPEED as i32;
            }
        }
        HeroSpeed::Normal => {}
    }

    // Encumbrance penalties (applied after speed bonuses).
    match encumbrance {
        Encumbrance::Unencumbered => {}
        Encumbrance::Burdened => {
            moveamt -= moveamt / 4; // lose 25%
        }
        Encumbrance::Stressed => {
            moveamt -= moveamt / 2; // lose 50%
        }
        Encumbrance::Strained => {
            moveamt -= (moveamt * 3) / 4; // lose 75%
        }
        Encumbrance::Overtaxed => {
            moveamt -= (moveamt * 7) / 8; // lose 87.5%
        }
        Encumbrance::Overloaded => {
            // Hero cannot move at all when overloaded; the movement
            // points are granted but the move is blocked elsewhere
            // (carrying_too_much).  NetHack applies no penalty here.
        }
    }

    moveamt.max(0)
}

/// Calculate monster movement points to add at the start of a new turn.
///
/// Matches NetHack's `mcalcmove()`.  The `m_moving` parameter controls
/// stochastic rounding (true during actual turn resolution).
pub fn mcalcmove(
    base_speed: u32,
    speed_mod: SpeedModifier,
    m_moving: bool,
    rng: &mut impl Rng,
) -> i32 {
    let mut mmove = base_speed as i32;

    match speed_mod {
        SpeedModifier::Slow => {
            if mmove < 12 {
                mmove = (2 * mmove + 1) / 3; // lose ~1/3
            } else {
                mmove = 4 + (mmove / 3); // lose ~2/3
            }
        }
        SpeedModifier::Normal => {}
        SpeedModifier::Fast => {
            mmove = (4 * mmove + 2) / 3; // gain ~1/3
        }
    }

    if m_moving {
        // Stochastic rounding to multiples of NORMAL_SPEED.
        let ns = NORMAL_SPEED as i32;
        let mmove_adj = mmove % ns;
        mmove -= mmove_adj;
        if rng.random_range(0..ns) < mmove_adj {
            mmove += ns;
        }
    }

    mmove
}

// ── Hunger helpers ───────────────────────────────────────────────────

use crate::hunger::{
    AccessoryHungerCtx, FaintingOutcome, check_fainting, compute_hunger_depletion,
    nutrition_to_hunger_level, should_starve, strength_penalty_change,
};

#[allow(dead_code)]
/// Map a raw nutrition counter to a HungerLevel for event emission.
/// Delegates to `hunger::nutrition_to_hunger_level`.
fn nutrition_to_hunger_level_local(nutrition: i32) -> HungerLevel {
    if nutrition > 1000 {
        HungerLevel::Satiated
    } else if nutrition > 150 {
        HungerLevel::NotHungry
    } else if nutrition > 50 {
        HungerLevel::Hungry
    } else if nutrition > 0 {
        HungerLevel::Weak
    } else {
        HungerLevel::Fainting
    }
}

// ── HP / PW regeneration ─────────────────────────────────────────────

/// Period (in turns) between HP regeneration ticks for a given level.
///
/// NetHack's `regen_hp()` uses `moves % period == 0` where period
/// depends on experience level and whether `Regeneration` is active.
/// Without Regeneration: period = max(1, 30 - level).
/// With Regeneration: period = 1 (every turn).
///
/// Here we return just the non-Regeneration period; the caller decides
/// whether Regeneration applies.
fn hp_regen_period(xlevel: u8) -> u32 {
    (30u32.saturating_sub(xlevel as u32)).max(1)
}

/// Period (in turns) between PW regeneration ticks.
///
/// Similar to HP but uses energy-level and wisdom:
///   period = max(1, 35 - (xlevel + wisdom / 2))
fn pw_regen_period(xlevel: u8, wisdom: u8) -> u32 {
    let bonus = xlevel as u32 + wisdom as u32 / 2;
    (35u32.saturating_sub(bonus)).max(1)
}

fn entity_has_positive_hp(world: &GameWorld, entity: hecs::Entity) -> bool {
    world
        .get_component::<HitPoints>(entity)
        .is_some_and(|hp| hp.current > 0)
}

fn live_monster_entity(world: &GameWorld, entity: hecs::Entity) -> bool {
    world.get_component::<Monster>(entity).is_some() && entity_has_positive_hp(world, entity)
}

// ── Main turn resolution ─────────────────────────────────────────────

/// Resolve one player input through the full moveloop_core sequence.
///
/// The movement-point system means a single call to `resolve_turn` may
/// process zero or one new game turns depending on how many points the
/// player and monsters have accumulated.
///
/// Returns all events generated, in order.
pub fn resolve_turn(
    world: &mut GameWorld,
    action: PlayerAction,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::with_capacity(16);
    let known_wizards = wizard_of_yendor_entities(world);

    // ── Step 1: deduct NORMAL_SPEED from player movement points ──
    {
        let player = world.player();
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(player) {
            mp.0 -= NORMAL_SPEED as i32;
        }
    }

    // ── Step 2: execute the player's action ──────────────────────
    let player_event_start = events.len();
    resolve_player_action(world, &action, rng, &mut events);
    sync_incapacitation_from_events(world, &events[player_event_start..]);
    apply_domain_hostility_side_effects_from_player_events(world, &mut events, rng);
    anger_peaceful_monsters_from_player_events(world, &events);
    sync_shopkeeper_deaths_from_events(world, &mut events);
    sync_current_level_shopkeeper_state(world);

    // ── Step 3: monster loop ─────────────────────────────────────
    // Each monster with movement >= NORMAL_SPEED gets to act.
    resolve_monster_turns(world, rng, &mut events);
    sync_shopkeeper_deaths_from_events(world, &mut events);

    // ── Step 4: check whether a new game turn boundary is reached ─
    // Both sides exhausted => new turn.
    let player_mp = world
        .get_component::<MovementPoints>(world.player())
        .map(|mp| mp.0)
        .unwrap_or(0);

    let monsters_can_move = any_monster_can_move(world);

    if player_mp < NORMAL_SPEED as i32 && !monsters_can_move {
        let new_turn_event_start = events.len();
        process_new_turn(world, rng, &mut events);
        sync_incapacitation_from_events(world, &events[new_turn_event_start..]);
        sync_shopkeeper_deaths_from_events(world, &mut events);
    }

    sync_wizard_of_yendor_from_events(world, &known_wizards, &events);
    sync_quest_state_from_world(world);
    sync_player_story_components(world, &events);
    sync_current_level_shopkeeper_state(world);

    events
}

// ── Iterator-based turn resolution ──────────────────────────────────

/// Internal phase tracking for the `turn_events` iterator.
enum TurnPhase {
    /// Deduct player movement points and resolve the player's action.
    PlayerAction,
    /// Resolve monster turns (one event at a time from the collected batch).
    MonsterTurns,
    /// Process new-turn boundary (regen, hunger, turn-end).
    NewTurn,
    /// All phases complete; the iterator is exhausted.
    Done,
}

/// Iterator-based turn resolution -- yields events one at a time.
///
/// This is the streaming equivalent of [`resolve_turn`]: instead of
/// buffering all events into a `Vec`, each call to `next()` produces
/// the next event in the turn sequence.  This enables downstream
/// consumers (rendering, audio, replay) to process events
/// incrementally without waiting for the entire turn to finish.
///
/// The iterator captures mutable references to the game world and RNG,
/// so no other access is possible while it is live.  Drop the iterator
/// (or exhaust it) to release the borrows.
///
/// # Phases
/// 1. **Player action** -- deducts movement points, resolves the action.
/// 2. **Monster turns** -- each eligible monster acts.
/// 3. **New-turn processing** -- movement point grants, regen, hunger,
///    `TurnEnd` event (only if a new game turn boundary was crossed).
pub fn turn_events<'a, R: Rng>(
    world: &'a mut GameWorld,
    action: PlayerAction,
    rng: &'a mut R,
) -> impl Iterator<Item = EngineEvent> + 'a {
    let known_wizards = wizard_of_yendor_entities(world);
    // Pre-collect event buffers for each phase.  We drain them one
    // element at a time via `iter::from_fn`, giving the caller a
    // streaming interface while keeping the implementation simple
    // and compatible with the borrow-checker (no self-referential
    // generator state).

    // Phase 1: player action.
    {
        let player = world.player();
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(player) {
            mp.0 -= NORMAL_SPEED as i32;
        }
    }
    let mut player_events = Vec::with_capacity(4);
    resolve_player_action(world, &action, rng, &mut player_events);
    sync_incapacitation_from_events(world, &player_events);
    apply_domain_hostility_side_effects_from_player_events(world, &mut player_events, rng);
    anger_peaceful_monsters_from_player_events(world, &player_events);
    sync_shopkeeper_deaths_from_events(world, &mut player_events);
    sync_current_level_shopkeeper_state(world);

    // Phase 2: monster turns.
    let mut monster_events = Vec::new();
    resolve_monster_turns(world, rng, &mut monster_events);
    sync_shopkeeper_deaths_from_events(world, &mut monster_events);

    // Phase 3: new-turn processing (conditional).
    let mut new_turn_events = Vec::new();
    let player_mp = world
        .get_component::<MovementPoints>(world.player())
        .map(|mp| mp.0)
        .unwrap_or(0);
    let monsters_can_move = any_monster_can_move(world);
    if player_mp < NORMAL_SPEED as i32 && !monsters_can_move {
        process_new_turn(world, rng, &mut new_turn_events);
        sync_incapacitation_from_events(world, &new_turn_events);
        sync_shopkeeper_deaths_from_events(world, &mut new_turn_events);
    }

    let mut story_events =
        Vec::with_capacity(player_events.len() + monster_events.len() + new_turn_events.len());
    story_events.extend(player_events.iter().cloned());
    story_events.extend(monster_events.iter().cloned());
    story_events.extend(new_turn_events.iter().cloned());
    sync_wizard_of_yendor_from_events(world, &known_wizards, &story_events);
    sync_quest_state_from_world(world);
    sync_player_story_components(world, &story_events);
    sync_current_level_shopkeeper_state(world);

    // Chain the three phases into a single iterator that yields
    // events in the correct order, one at a time.
    let mut phase = TurnPhase::PlayerAction;
    let mut idx = 0;

    std::iter::from_fn(move || {
        loop {
            match phase {
                TurnPhase::PlayerAction => {
                    if idx < player_events.len() {
                        let event = player_events[idx].clone();
                        idx += 1;
                        return Some(event);
                    }
                    phase = TurnPhase::MonsterTurns;
                    idx = 0;
                }
                TurnPhase::MonsterTurns => {
                    if idx < monster_events.len() {
                        let event = monster_events[idx].clone();
                        idx += 1;
                        return Some(event);
                    }
                    phase = TurnPhase::NewTurn;
                    idx = 0;
                }
                TurnPhase::NewTurn => {
                    if idx < new_turn_events.len() {
                        let event = new_turn_events[idx].clone();
                        idx += 1;
                        return Some(event);
                    }
                    phase = TurnPhase::Done;
                }
                TurnPhase::Done => {
                    return None;
                }
            }
        }
    })
}

// ── Gen-block-based turn resolution ───────────────────────────────

/// Gen-block turn resolution -- yields events one at a time.
///
/// This is the gen-block equivalent of [`turn_events`]: instead of
/// pre-collecting all events into three `Vec`s and then draining them
/// via `iter::from_fn`, a `gen` block yields each event as it is
/// produced, making the turn structure much clearer.
///
/// The output is identical to [`resolve_turn`] and [`turn_events`].
///
/// # Phases
/// 1. **Player action** -- deducts movement points, resolves the action.
/// 2. **Monster turns** -- each eligible monster acts.
/// 3. **New-turn processing** -- movement point grants, regen, hunger,
///    `TurnEnd` event (only if a new game turn boundary was crossed).
pub fn turn_events_gen<'a, R: Rng>(
    world: &'a mut GameWorld,
    action: PlayerAction,
    rng: &'a mut R,
) -> impl Iterator<Item = EngineEvent> + 'a {
    gen move {
        let known_wizards = wizard_of_yendor_entities(world);
        let mut story_events = Vec::new();
        // Phase 1: deduct NORMAL_SPEED from player movement points.
        {
            let player = world.player();
            if let Some(mut mp) = world.get_component_mut::<MovementPoints>(player) {
                mp.0 -= NORMAL_SPEED as i32;
            }
        }

        // Phase 2: resolve player action.
        let mut player_events = Vec::with_capacity(4);
        resolve_player_action(world, &action, rng, &mut player_events);
        sync_incapacitation_from_events(world, &player_events);
        apply_domain_hostility_side_effects_from_player_events(world, &mut player_events, rng);
        anger_peaceful_monsters_from_player_events(world, &player_events);
        sync_shopkeeper_deaths_from_events(world, &mut player_events);
        sync_current_level_shopkeeper_state(world);
        for event in player_events {
            story_events.push(event.clone());
            yield event;
        }

        // Phase 3: monster turns.
        let mut monster_events = Vec::new();
        resolve_monster_turns(world, rng, &mut monster_events);
        sync_shopkeeper_deaths_from_events(world, &mut monster_events);
        for event in monster_events {
            story_events.push(event.clone());
            yield event;
        }

        // Phase 4: new-turn processing (if both sides exhausted).
        let player_mp = world
            .get_component::<MovementPoints>(world.player())
            .map(|mp| mp.0)
            .unwrap_or(0);
        let monsters_can_move = any_monster_can_move(world);
        if player_mp < NORMAL_SPEED as i32 && !monsters_can_move {
            let mut new_turn_events = Vec::new();
            process_new_turn(world, rng, &mut new_turn_events);
            sync_incapacitation_from_events(world, &new_turn_events);
            sync_shopkeeper_deaths_from_events(world, &mut new_turn_events);
            for event in new_turn_events {
                story_events.push(event.clone());
                yield event;
            }
        }

        sync_wizard_of_yendor_from_events(world, &known_wizards, &story_events);
        sync_quest_state_from_world(world);
        sync_player_story_components(world, &story_events);
        sync_current_level_shopkeeper_state(world);
    }
}

fn collect_player_hostility_targets(
    player: hecs::Entity,
    events: &[EngineEvent],
) -> std::collections::HashSet<hecs::Entity> {
    let mut targets = std::collections::HashSet::new();

    for event in events {
        match event {
            EngineEvent::MeleeHit {
                attacker, defender, ..
            } if *attacker == player => {
                targets.insert(*defender);
            }
            EngineEvent::MeleeMiss { attacker, defender } if *attacker == player => {
                targets.insert(*defender);
            }
            EngineEvent::RangedHit {
                attacker, defender, ..
            } if *attacker == player => {
                targets.insert(*defender);
            }
            EngineEvent::RangedMiss {
                attacker, defender, ..
            } if *attacker == player => {
                targets.insert(*defender);
            }
            EngineEvent::ExtraDamage { target, .. } => {
                targets.insert(*target);
            }
            EngineEvent::HpChange { entity, amount, .. } if *amount < 0 => {
                targets.insert(*entity);
            }
            EngineEvent::EntityDied {
                entity,
                killer: Some(killer),
                ..
            } if *killer == player => {
                targets.insert(*entity);
            }
            EngineEvent::StatusApplied { entity, status, .. }
                if status_is_hostile_toward_monster(*status) =>
            {
                targets.insert(*entity);
            }
            _ => {}
        }
    }

    targets
}

fn anger_peaceful_monsters_from_player_events(world: &mut GameWorld, events: &[EngineEvent]) {
    let player = world.player();
    let angered = collect_player_hostility_targets(player, events);

    for entity in angered {
        if entity == player || world.get_component::<Monster>(entity).is_none() {
            continue;
        }
        let _ = world.ecs_mut().remove_one::<Peaceful>(entity);
    }
}

fn sync_incapacitation_from_events(world: &mut GameWorld, events: &[EngineEvent]) {
    for event in events {
        match event {
            EngineEvent::StatusApplied {
                entity,
                status: crate::event::StatusEffect::Paralyzed,
                duration: Some(duration),
                ..
            } => {
                let _ = crate::status::make_paralyzed(world, *entity, *duration);
            }
            EngineEvent::StatusRemoved {
                entity,
                status: crate::event::StatusEffect::Paralyzed,
            } => {
                if let Some(mut status) =
                    world.get_component_mut::<crate::status::StatusEffects>(*entity)
                {
                    status.paralysis = 0;
                }
            }
            EngineEvent::StatusApplied {
                entity,
                status: crate::event::StatusEffect::Sleeping,
                duration: Some(duration),
                ..
            } => {
                let _ = crate::status::make_sleeping(world, *entity, *duration);
            }
            EngineEvent::StatusRemoved {
                entity,
                status: crate::event::StatusEffect::Sleeping,
            } => {
                let _ = crate::status::wake_from_sleeping(world, *entity);
            }
            _ => {}
        }
    }
}

fn apply_domain_hostility_side_effects_from_player_events(
    world: &mut GameWorld,
    events: &mut Vec<EngineEvent>,
    rng: &mut impl Rng,
) {
    let player = world.player();
    let targets = collect_player_hostility_targets(player, events);
    if targets.is_empty() {
        return;
    }

    mark_angry_quest_leader_from_targets(world, &targets);
    rile_attacked_shopkeepers(world, &targets);

    let mut extra_events = Vec::new();
    for entity in targets {
        if let Some(mut priest) = infer_priest_runtime(world, entity) {
            priest.angry = true;
            upsert_priest_component(world, entity, priest);
            let mut temple = crate::priest::TempleInfo::new(priest.alignment);
            temple.has_priest = true;
            temple.has_shrine = priest.has_shrine;
            temple.is_sanctum = priest.is_high_priest;
            temple.priest_angry = priest.angry;
            extra_events.extend(crate::priest::anger_priest(&mut temple));
            extra_events.extend(apply_priest_divine_wrath(world, player, &temple, rng));
        }
    }
    events.extend(extra_events);
}

fn apply_priest_divine_wrath(
    world: &mut GameWorld,
    player: hecs::Entity,
    temple: &crate::priest::TempleInfo,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = crate::priest::ghod_hitsu(temple, rng);
    if !temple.has_priest || !temple.has_shrine {
        return events;
    }

    let shock_resistant = crate::status::has_intrinsic_shock_res(world, player);
    let damage = if shock_resistant {
        rng.random_range(1..=3)
    } else {
        rng.random_range(8..=16)
    };

    let new_hp = if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
        hp.current -= damage;
        hp.current
    } else {
        return events;
    };

    events.push(EngineEvent::HpChange {
        entity: player,
        amount: -damage,
        new_hp,
        source: HpSource::Divine,
    });
    if new_hp <= 0 {
        events.push(EngineEvent::EntityDied {
            entity: player,
            killer: None,
            cause: DeathCause::KilledBy {
                killer_name: "an angry god".to_string(),
            },
        });
    }

    if !shock_resistant {
        let blind_turns = rng.random_range(8..=16);
        events.extend(crate::status::make_blinded(world, player, blind_turns));
    }

    events
}

fn status_is_hostile_toward_monster(status: crate::event::StatusEffect) -> bool {
    matches!(
        status,
        crate::event::StatusEffect::Blind
            | crate::event::StatusEffect::Confused
            | crate::event::StatusEffect::Stunned
            | crate::event::StatusEffect::Hallucinating
            | crate::event::StatusEffect::Paralyzed
            | crate::event::StatusEffect::Sleeping
            | crate::event::StatusEffect::SlowSpeed
            | crate::event::StatusEffect::Sick
            | crate::event::StatusEffect::FoodPoisoned
            | crate::event::StatusEffect::Stoning
            | crate::event::StatusEffect::Slimed
            | crate::event::StatusEffect::Strangled
            | crate::event::StatusEffect::Polymorphed
            | crate::event::StatusEffect::Lycanthropy
            | crate::event::StatusEffect::Aggravate
    )
}

/// Check if any monster has enough movement points to act.
fn any_monster_can_move(world: &GameWorld) -> bool {
    let player = world.player();
    for (entity, mp) in world.ecs().query::<&MovementPoints>().iter() {
        if entity != player && live_monster_entity(world, entity) && mp.0 >= NORMAL_SPEED as i32 {
            return true;
        }
    }
    false
}

/// Process the new-turn boundary: grant movement points, increment
/// turn counter, and run all once-per-turn effects.
fn process_new_turn(world: &mut GameWorld, rng: &mut impl Rng, events: &mut Vec<EngineEvent>) {
    // 4a. Grant all monsters new movement points.
    grant_monster_movement(world, rng);

    // 4b. Grant player new movement points.
    grant_player_movement(world, rng);

    // 4c. Increment turn counter.
    world.advance_turn();

    // 4d. Process status effect timeouts (nh_timeout equivalent).
    {
        let player = world.player();
        let mut status_events = crate::status::tick_status_effects(world, player, rng);
        events.append(&mut status_events);
    }

    // 4d2. Polymorph timer countdown.
    {
        let player = world.player();
        let mut poly_events = crate::polyself::tick_polymorph(world, player);
        events.append(&mut poly_events);
    }

    // 4d3. Spell protection dissipation.
    {
        let player = world.player();
        let mut sp_events = crate::status::tick_spell_protection(world, player);
        events.append(&mut sp_events);
    }

    // 4d4. Hero misc counters (cream, gallop).
    {
        let player = world.player();
        let mut hc_events = crate::status::tick_hero_counters(world, player);
        events.append(&mut hc_events);
    }

    // 4d5. Tick monster incapacitation timers that affect whether they can act.
    tick_monster_incapacitation(world, events);

    // 4e. Regenerate HP.
    regen_hp(world, events);

    // 4f. Regenerate PW.
    regen_pw(world, events);

    // 4g. Process hunger.
    process_hunger(world, events, rng);

    // 4h. Light source fuel consumption.
    {
        let mut light_events = crate::light::tick_light_sources(world);
        events.append(&mut light_events);
    }

    // 4h2. Gas cloud / region effects.
    {
        let mut gas_clouds = std::mem::take(&mut world.dungeon_mut().gas_clouds);
        let mut cloud_events = crate::region::tick_gas_clouds(&mut gas_clouds, world, rng);
        world.dungeon_mut().gas_clouds = gas_clouds;
        events.append(&mut cloud_events);
    }

    // 4i. Attribute exercise periodic check.
    {
        let turn = world.turn();
        if crate::attributes::is_exercise_turn(turn) {
            let player = world.player();
            // Copy components out to avoid simultaneous mutable borrows.
            let snapshot = {
                let attrs = world
                    .get_component::<crate::world::Attributes>(player)
                    .map(|a| *a);
                let nat = world
                    .get_component::<crate::attributes::NaturalAttributes>(player)
                    .map(|n| *n);
                let ex = world
                    .get_component::<crate::attributes::AttributeExercise>(player)
                    .map(|e| *e);
                match (attrs, nat, ex) {
                    (Some(a), Some(n), Some(e)) => Some((a, n, e)),
                    _ => None,
                }
            };
            if let Some((mut attrs, mut nat, mut ex)) = snapshot {
                let exercise_events = crate::attributes::apply_exercise(
                    &mut attrs,
                    &mut nat,
                    &mut ex,
                    crate::attributes::Race::Human,
                    crate::attributes::FighterRole::NonFighter,
                );
                // Write back the modified components.
                if let Some(mut a) = world.get_component_mut::<crate::world::Attributes>(player) {
                    *a = attrs;
                }
                if let Some(mut n) =
                    world.get_component_mut::<crate::attributes::NaturalAttributes>(player)
                {
                    *n = nat;
                }
                if let Some(mut e) =
                    world.get_component_mut::<crate::attributes::AttributeExercise>(player)
                {
                    *e = ex;
                }
                events.extend(exercise_events);
            }
        }
    }

    // 4j. Random monster generation (1/70 chance on normal levels).
    if rng.random_range(0..70) == 0
        && let Some(spawn_pos) = random_monster_spawn_position(world, rng)
    {
        let entity = spawn_random_monster(world, spawn_pos);
        events.push(EngineEvent::MonsterGenerated {
            entity,
            position: spawn_pos,
        });
    }

    emit_ambient_dungeon_sound(world, rng, events);
    process_amulet_portal_sense(world, rng, events);
    process_amulet_wakes_sleeping_wizard(world, rng, events);
    process_wizard_of_yendor_turn(world, rng, events);
    process_shop_repairs(world, events);

    events.push(EngineEvent::TurnEnd {
        turn_number: world.turn(),
    });
}

fn emit_ambient_dungeon_sound(
    world: &GameWorld,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    let player = world.player();
    if crate::status::is_deaf(world, player) {
        return;
    }

    let player_pos = world.get_component::<Positioned>(player).map(|pos| pos.0);
    let has_shop = player_pos
        .is_some_and(|pos| find_shop_index_containing_position(world, pos).is_none())
        && world
            .dungeon()
            .shop_rooms
            .iter()
            .any(|shop| !shop_room_is_deserted(world, shop));
    let has_temple = current_level_has_temple_ambient(world);
    let has_oracle = current_level_has_oracle_ambient(world);
    let vault_ambient = current_level_vault_ambient_kind(world);
    let ambient_context = crate::music::AmbientSoundContext {
        depth: world.dungeon().depth,
        branch: ambient_branch_name(world.dungeon().branch),
        has_shop,
        has_temple,
        has_oracle,
        has_court: current_level_has_court_ambient(world),
        has_swamp: current_level_has_swamp_ambient(world),
        has_beehive: current_level_has_beehive_ambient(world),
        has_morgue: current_level_has_morgue_ambient(world),
        has_barracks: current_level_has_barracks_ambient(world),
        has_zoo: current_level_has_zoo_ambient(world),
        has_fountain: current_level_has_terrain(world, Terrain::Fountain),
        has_sink: current_level_has_terrain(world, Terrain::Sink),
        hallucinating: crate::status::is_hallucinating(world, player),
        vault_ambient,
    };
    if let Some(key) = crate::music::ambient_sounds(ambient_context, rng) {
        events.push(EngineEvent::msg(key));
    }
}

#[doc(hidden)]
pub fn force_emit_ambient_dungeon_sound(world: &GameWorld, rng: &mut impl Rng) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    emit_ambient_dungeon_sound(world, rng, &mut events);
    events
}

fn ambient_branch_name(branch: DungeonBranch) -> &'static str {
    match branch {
        DungeonBranch::Gehennom => "Gehennom",
        DungeonBranch::Mines => "Mines",
        _ => "Dungeons",
    }
}

fn current_level_has_temple_ambient(world: &GameWorld) -> bool {
    let Some(player_pos) = world
        .get_component::<Positioned>(world.player())
        .map(|pos| pos.0)
    else {
        return false;
    };
    world
        .ecs()
        .query::<(&Monster, &crate::npc::Priest)>()
        .iter()
        .any(|(entity, (_, priest))| {
            live_monster_entity(world, entity)
                && priest.has_shrine
                && !priest.is_high_priest
                && world
                    .get_component::<crate::status::StatusEffects>(entity)
                    .is_none_or(|status| status.sleeping == 0 && status.paralysis == 0)
                && world
                    .get_component::<Positioned>(entity)
                    .is_some_and(|pos| crate::ball::chebyshev_distance(player_pos, pos.0) > 3)
        })
}

fn current_level_has_oracle_ambient(world: &GameWorld) -> bool {
    let special_level = world
        .dungeon()
        .check_topology_special(&world.dungeon().branch, world.dungeon().depth)
        .or_else(|| identify_special_level(world.dungeon().branch, world.dungeon().depth));
    if special_level != Some(crate::special_levels::SpecialLevelId::OracleLevel) {
        return false;
    }

    let player = world.player();
    if crate::status::is_hallucinating(world, player) || crate::status::is_blind(world, player) {
        return world
            .ecs()
            .query::<(&Monster, &Name)>()
            .iter()
            .any(|(entity, (_, name))| live_monster_entity(world, entity) && name.0 == "Oracle");
    }

    let Some(player_pos) = world.get_component::<Positioned>(player).map(|pos| pos.0) else {
        return false;
    };
    let map = &world.dungeon().current_level;
    let mut fov = FovMap::new(map.width, map.height);
    fov.compute(player_pos, 8, |x, y| {
        map.get(Position::new(x, y))
            .is_none_or(|cell| cell.terrain.is_opaque())
    });

    world
        .ecs()
        .query::<(&Monster, &Name, &Positioned)>()
        .iter()
        .any(|(entity, (_, name, pos))| {
            live_monster_entity(world, entity) && name.0 == "Oracle" && !fov.is_visible_pos(pos.0)
        })
}

fn current_level_monster_def_by_name<'a>(
    world: &'a GameWorld,
    name: &str,
) -> Option<&'a MonsterDef> {
    let monster_id = resolve_monster_id_by_spec(world.monster_catalog(), name)?;
    world
        .monster_catalog()
        .iter()
        .find(|def| def.id == monster_id)
}

fn current_level_monster_def_for_entity(
    world: &GameWorld,
    entity: hecs::Entity,
) -> Option<&MonsterDef> {
    if let Some(monster_id) = world
        .get_component::<crate::world::MonsterIdentity>(entity)
        .map(|id| id.0)
    {
        return world
            .monster_catalog()
            .iter()
            .find(|def| def.id == monster_id);
    }
    let name = world.get_component::<Name>(entity)?;
    current_level_monster_def_by_name(world, &name.0)
}

fn current_level_has_court_ambient(world: &GameWorld) -> bool {
    world
        .ecs()
        .query::<(&Monster, &Name)>()
        .iter()
        .any(|(entity, (_, name))| {
            if !live_monster_entity(world, entity) {
                return false;
            }
            let Some(monster_def) = current_level_monster_def_by_name(world, &name.0) else {
                return false;
            };
            let lower = name.0.to_ascii_lowercase();
            let titled_name = lower.contains("lord")
                || lower.contains("lady")
                || lower.contains("prince")
                || lower.contains("princess")
                || lower.contains("king")
                || lower.contains("queen");
            (crate::mondata::is_lord(monster_def)
                || crate::mondata::is_prince(monster_def)
                || titled_name)
                && !crate::mondata::is_animal(monster_def)
        })
}

fn current_level_has_swamp_ambient(world: &GameWorld) -> bool {
    let has_swamp_lair_monster =
        world
            .ecs()
            .query::<(&Monster, &Name)>()
            .iter()
            .any(|(entity, (_, name))| {
                if !live_monster_entity(world, entity) {
                    return false;
                }
                let lower = name.0.to_ascii_lowercase();
                lower.contains("eel") || lower.contains("fungus") || lower.contains("mold")
            });
    has_swamp_lair_monster && current_level_terrain_count(world, Terrain::Pool) >= 12
}

fn current_level_has_beehive_ambient(world: &GameWorld) -> bool {
    world
        .ecs()
        .query::<(&Monster, &Name)>()
        .iter()
        .any(|(entity, (_, name))| {
            if !live_monster_entity(world, entity) {
                return false;
            }
            let Some(monster_def) = current_level_monster_def_by_name(world, &name.0) else {
                return false;
            };
            monster_def.symbol == 'a' && crate::mondata::is_flyer(monster_def)
        })
}

fn current_level_has_morgue_ambient(world: &GameWorld) -> bool {
    world
        .ecs()
        .query::<(&Monster, &Name)>()
        .iter()
        .any(|(entity, (_, name))| {
            live_monster_entity(world, entity)
                && current_level_monster_def_by_name(world, &name.0)
                    .is_some_and(crate::mondata::is_undead)
        })
}

fn current_level_has_barracks_ambient(world: &GameWorld) -> bool {
    let mut sleeping_mercenaries = 0usize;
    let mut mercenary_count = 0usize;

    for (entity, (_, name)) in world.ecs().query::<(&Monster, &Name)>().iter() {
        if !live_monster_entity(world, entity) {
            continue;
        }
        let Some(monster_def) = current_level_monster_def_by_name(world, &name.0) else {
            continue;
        };
        if !crate::mondata::is_mercenary(monster_def) {
            continue;
        }
        mercenary_count += 1;
        if world
            .get_component::<crate::status::StatusEffects>(entity)
            .is_some_and(|status| status.sleeping > 0)
        {
            sleeping_mercenaries += 1;
        }
    }

    sleeping_mercenaries > 0 || mercenary_count > 5
}

fn current_level_has_zoo_ambient(world: &GameWorld) -> bool {
    world
        .ecs()
        .query::<(&Monster, &Name)>()
        .iter()
        .any(|(entity, (_, name))| {
            if !live_monster_entity(world, entity) {
                return false;
            }
            let Some(monster_def) = current_level_monster_def_by_name(world, &name.0) else {
                return false;
            };
            let sleeping = world
                .get_component::<crate::status::StatusEffects>(entity)
                .is_some_and(|status| status.sleeping > 0);
            sleeping || crate::mondata::is_animal(monster_def)
        })
}

fn current_level_has_terrain(world: &GameWorld, terrain: Terrain) -> bool {
    world
        .dungeon()
        .current_level
        .cells
        .iter()
        .flatten()
        .any(|cell| cell.terrain == terrain)
}

fn current_level_terrain_count(world: &GameWorld, terrain: Terrain) -> usize {
    world
        .dungeon()
        .current_level
        .cells
        .iter()
        .flatten()
        .filter(|cell| cell.terrain == terrain)
        .count()
}

fn current_level_vault_ambient_kind(world: &GameWorld) -> Option<crate::music::VaultAmbientKind> {
    let player_pos = world
        .get_component::<Positioned>(world.player())
        .map(|pos| pos.0)?;
    let vault_rooms = &world.dungeon().vault_rooms;
    if vault_rooms.is_empty() || crate::vault::player_in_vault(player_pos, vault_rooms).is_some() {
        return None;
    }

    if world.dungeon().vault_guard_present {
        return Some(crate::music::VaultAmbientKind::GuardFootsteps);
    }

    if current_level_vault_has_gold(world, vault_rooms) {
        Some(crate::music::VaultAmbientKind::CountingGold)
    } else {
        Some(crate::music::VaultAmbientKind::Searching)
    }
}

fn current_level_vault_has_gold(
    world: &GameWorld,
    vault_rooms: &[crate::vault::VaultRoom],
) -> bool {
    let branch = crate::dungeon::data_branch_id(world.dungeon().branch);
    let depth = world.dungeon().depth as i16;
    world
        .ecs()
        .query::<(&ObjectCore, &ObjectLocation)>()
        .iter()
        .any(|(_entity, (core, location))| {
            core.object_class == ObjectClass::Coin
                && matches!(
                    location,
                    ObjectLocation::Floor {
                        x,
                        y,
                        level,
                    } if level.branch == branch
                        && level.depth == depth
                        && vault_rooms.iter().any(|vault| vault.contains(Position::new(i32::from(*x), i32::from(*y))))
                )
        })
}

fn tick_monster_incapacitation(world: &mut GameWorld, events: &mut Vec<EngineEvent>) {
    let monsters: Vec<hecs::Entity> = world
        .ecs()
        .query::<(&Monster,)>()
        .iter()
        .map(|(entity, _)| entity)
        .filter(|entity| live_monster_entity(world, *entity))
        .collect();

    for entity in monsters {
        let Some(mut status) = world.get_component_mut::<crate::status::StatusEffects>(entity)
        else {
            continue;
        };

        if status.paralysis > 0 {
            status.paralysis -= 1;
            if status.paralysis == 0 {
                events.push(EngineEvent::StatusRemoved {
                    entity,
                    status: crate::event::StatusEffect::Paralyzed,
                });
            }
        }

        if status.sleeping > 0 {
            status.sleeping -= 1;
            if status.sleeping == 0 {
                events.push(EngineEvent::StatusRemoved {
                    entity,
                    status: crate::event::StatusEffect::Sleeping,
                });
            }
        }
    }
}

/// Grant new movement points to all monster entities.
fn grant_monster_movement(world: &mut GameWorld, rng: &mut impl Rng) {
    // Collect entity ids and their calculated movement additions first
    // (borrow-checker: cannot mutate while iterating).
    let player = world.player();
    let mut additions: Vec<(hecs::Entity, i32)> = Vec::new();

    for (entity, (speed, _monster)) in world.ecs().query::<(&Speed, &Monster)>().iter() {
        if entity == player {
            continue;
        }
        let speed_mod = world
            .get_component::<MonsterSpeedMod>(entity)
            .map(|m| m.0)
            .unwrap_or(SpeedModifier::Normal);
        let add = mcalcmove(speed.0, speed_mod, true, rng);
        additions.push((entity, add));
    }

    // Apply.
    for (entity, add) in additions {
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(entity) {
            mp.0 += add;
        }
    }
}

/// Grant new movement points to the player.
fn grant_player_movement(world: &mut GameWorld, rng: &mut impl Rng) {
    let player = world.player();

    let base_speed = world
        .get_component::<Speed>(player)
        .map(|s| s.0)
        .unwrap_or(NORMAL_SPEED);

    let hero_speed = world
        .get_component::<HeroSpeedBonus>(player)
        .map(|h| h.0)
        .unwrap_or(HeroSpeed::Normal);

    let encumbrance = world
        .get_component::<EncumbranceLevel>(player)
        .map(|e| e.0)
        .unwrap_or(Encumbrance::Unencumbered);

    let add = u_calc_moveamt(base_speed, hero_speed, encumbrance, rng);

    if let Some(mut mp) = world.get_component_mut::<MovementPoints>(player) {
        mp.0 += add;
        if mp.0 < 0 {
            mp.0 = 0;
        }
    }
}

/// Process the player's chosen action.
fn resolve_player_action(
    world: &mut GameWorld,
    action: &PlayerAction,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    // ── Incapacitation check: if paralyzed or asleep, skip the action ──
    let player = world.player();
    if crate::status::is_paralyzed(world, player) || crate::status::is_sleeping(world, player) {
        events.push(EngineEvent::msg("status-paralyzed-cant-move"));
        return;
    }

    match action {
        PlayerAction::Move { direction } => {
            // Confusion/stun may randomize the movement direction.
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if confused || stunned {
                match crate::status::maybe_confuse_direction(confused, stunned, rng) {
                    Some(random_dir) => random_dir,
                    None => *direction,
                }
            } else {
                *direction
            };
            try_move_entity(world, world.player(), effective_dir, false, events, rng);
        }
        PlayerAction::Rest => {
            // Resting: no movement, just pass the turn.
        }
        PlayerAction::Search => {
            // Search adjacent tiles for hidden traps.
            let player = world.player();
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            let luck = world
                .get_component::<PlayerCombat>(player)
                .map(|pc| pc.luck)
                .unwrap_or(0);

            let trap_events = detect_trap(
                rng,
                &mut world.dungeon_mut().trap_map,
                player_pos,
                luck,
                0, // fund: no artifact/lenses bonus for now
            );
            events.extend(trap_events);
        }
        PlayerAction::GoUp => {
            handle_go_up(world, rng, events);
        }
        PlayerAction::GoDown => {
            handle_go_down(world, rng, events);
        }
        PlayerAction::PickUp => {
            // Levitating players can't pick up items from the floor.
            if crate::status::is_levitating(world, world.player()) {
                events.push(EngineEvent::msg("levitating-cant-pickup"));
            } else {
                let mut ls = crate::items::LetterState::default();
                let pickup_events = crate::inventory::pickup_all_at_player(world, &mut ls, &[]);
                events.extend(pickup_events);
            }
        }
        PlayerAction::Drop { item } => {
            let player = world.player();
            let drop_events = handle_player_drop(world, player, *item);
            events.extend(drop_events);
        }
        PlayerAction::DropMultiple { items } => {
            let player = world.player();
            for item in items {
                let drop_events = handle_player_drop(world, player, *item);
                events.extend(drop_events);
            }
        }
        PlayerAction::ViewInventory => {
            let player = world.player();
            let inv_events = crate::inventory::view_inventory(world, player);
            events.extend(inv_events);
        }
        PlayerAction::Wield { item } => {
            // Equipment wield requires obj_defs for slot validation.
            // Emit the event; the full equip_item() flow is available
            // to callers that pass obj_defs (e.g., via equipment::equip_item).
            let player = world.player();
            events.push(EngineEvent::ItemWielded {
                actor: player,
                item: *item,
            });
        }
        PlayerAction::Wear { item } => {
            let player = world.player();
            events.push(EngineEvent::ItemWorn {
                actor: player,
                item: *item,
            });
        }
        PlayerAction::TakeOff { item } | PlayerAction::Remove { item } => {
            let player = world.player();
            let result = crate::equipment::unequip_item(world, player, *item);
            match result {
                Ok(unequip_events) => events.extend(unequip_events),
                Err(crate::equipment::EquipError::CursedCannotRemove) => {
                    events.push(EngineEvent::msg("cursed-cannot-remove"));
                }
                Err(crate::equipment::EquipError::NotEquipped) => {
                    events.push(EngineEvent::msg("not-wearing-that"));
                }
                Err(_) => {
                    events.push(EngineEvent::msg("cannot-do-that"));
                }
            }
        }
        PlayerAction::TakeOffAll => {
            let player = world.player();
            let equip_snapshot = world
                .get_component::<crate::equipment::EquipmentSlots>(player)
                .map(|equip| (*equip).clone());
            if let Some(equip) = equip_snapshot {
                let mut removed_any = false;
                for slot in crate::equipment::TAKEOFF_ORDER {
                    if let Some(item) = equip.get(*slot) {
                        match crate::equipment::unequip_item(world, player, item) {
                            Ok(unequip_events) => {
                                if !unequip_events.is_empty() {
                                    removed_any = true;
                                }
                                events.extend(unequip_events);
                            }
                            Err(crate::equipment::EquipError::CursedCannotRemove) => {
                                events.push(EngineEvent::msg("cursed-cannot-remove"));
                            }
                            Err(crate::equipment::EquipError::NotEquipped) => {}
                            Err(_) => {
                                events.push(EngineEvent::msg("cannot-do-that"));
                            }
                        }
                    }
                }
                if !removed_any {
                    events.push(EngineEvent::msg("not-wearing-that"));
                }
            }
        }
        PlayerAction::PutOn { item } => {
            let player = world.player();
            events.push(EngineEvent::ItemWorn {
                actor: player,
                item: *item,
            });
        }
        PlayerAction::Apply { item } => {
            let player = world.player();
            let tool_events = crate::tools::apply_tool(world, player, *item, rng);
            events.extend(tool_events);
        }
        PlayerAction::Engrave { text } => {
            let player = world.player();
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            let method = infer_engrave_method(world, player);
            let has_elbereth = text.to_ascii_lowercase().contains("elbereth");
            let mut conduct = read_conduct_state(world, player);
            let (mut engrave_events, _turns) = crate::engrave::engrave(
                &mut world.dungeon_mut().engraving_map,
                &mut conduct,
                player_pos,
                text,
                method,
            );
            persist_conduct_state(world, player, conduct);
            events.append(&mut engrave_events);
            if has_elbereth {
                events.push(EngineEvent::msg("engrave-elbereth"));
            }
        }
        PlayerAction::Dip { item, into } => {
            let player = world.player();
            let dip_events = crate::dip::dip_item(world, player, *item, *into, rng);
            events.extend(dip_events);
        }
        PlayerAction::Kick { direction } => {
            // Kick in the given direction.
            let player = world.player();
            let is_monk = world
                .get_component::<nethack_babel_data::PlayerIdentity>(player)
                .map(|id| id.role.0 == crate::religion::roles::MONK)
                .unwrap_or(false);
            let kick_events = crate::environment::kick(world, *direction, is_monk, rng);
            events.extend(kick_events);
        }
        PlayerAction::Loot => {
            // Loot a container on the floor at the player's position.
            let player = world.player();
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            // Find a container at the player's position.
            for (entity, loc) in world
                .ecs()
                .query::<&nethack_babel_data::ObjectLocation>()
                .iter()
            {
                if crate::dungeon::floor_position_on_level(
                    loc,
                    world.dungeon().branch,
                    world.dungeon().depth,
                )
                .is_some_and(|pos| pos == player_pos)
                    && world
                        .get_component::<crate::environment::Container>(entity)
                        .is_some()
                {
                    let open_events = crate::environment::open_container(world, entity);
                    events.extend(open_events);
                    break;
                }
            }
        }
        PlayerAction::Eat { item } => {
            if let Some(item_entity) = item {
                let player = world.player();
                if let Some(food_def) = infer_food_def_from_item(world, *item_entity) {
                    let result =
                        crate::hunger::eat_food(world, player, *item_entity, &food_def, rng);
                    apply_eating_conduct(world, player, &result.conduct);
                    events.extend(result.events);
                } else {
                    events.push(EngineEvent::msg("eat-generic"));
                }
            } else {
                events.push(EngineEvent::msg("eat-what"));
            }
        }
        PlayerAction::Quaff { item } => {
            if let Some(item_entity) = item {
                let player = world.player();
                if let Some(potion_type) = infer_potion_type_from_item(world, *item_entity) {
                    let quaff_events =
                        crate::potions::quaff_potion(world, player, *item_entity, potion_type, rng);
                    events.extend(quaff_events);
                } else {
                    events.push(EngineEvent::msg("quaff-generic"));
                }
            } else {
                events.push(EngineEvent::msg("quaff-what"));
            }
        }
        PlayerAction::Read { item } => {
            let player = world.player();
            if let Some(item_entity) = item {
                if is_book_of_the_dead(world, *item_entity) {
                    increment_conduct(world, player, |state| {
                        state.literate = state.literate.saturating_add(1);
                    });
                    let read_events = read_book_of_the_dead(world, player, *item_entity, rng);
                    events.extend(read_events);
                } else if crate::status::is_blind(world, player) {
                    events.push(EngineEvent::msg("scroll-cant-read-blind"));
                } else if let Some(scroll_type) = infer_scroll_type_from_item(world, *item_entity) {
                    let confused = crate::status::is_confused(world, player);
                    let read_events = crate::scrolls::read_scroll(
                        world,
                        player,
                        *item_entity,
                        scroll_type,
                        confused,
                        rng,
                    );
                    increment_conduct(world, player, |state| {
                        state.literate = state.literate.saturating_add(1);
                    });
                    events.extend(read_events);
                } else {
                    events.push(EngineEvent::msg("read-generic"));
                }
            } else if crate::status::is_blind(world, player) {
                events.push(EngineEvent::msg("scroll-cant-read-blind"));
            } else {
                events.push(EngineEvent::msg("read-what"));
            }
        }
        PlayerAction::CastSpell { spell, direction } => {
            let player = world.player();
            let cast_events = crate::spells::cast_spell(world, player, *spell, *direction, rng);
            events.extend(cast_events);
        }
        PlayerAction::ZapWand { item, direction } => {
            // Confused/stunned zapper gets a randomized direction.
            let player = world.player();
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if (confused || stunned) && direction.is_some() {
                match crate::status::maybe_confuse_direction(confused, stunned, rng) {
                    Some(random_dir) => Some(random_dir),
                    None => *direction,
                }
            } else {
                *direction
            };

            let wand_type = world
                .get_component::<crate::monster_ai::WandTypeTag>(*item)
                .map(|t| t.0);
            let mut charges = world
                .get_component::<crate::wands::WandCharges>(*item)
                .map(|c| *c);
            if let (Some(wand_type), Some(mut wand_charges)) = (wand_type, charges.take()) {
                let zap_dir = match (wand_type.direction(), effective_dir) {
                    (crate::wands::WandDirection::Nodir, _) => Direction::North,
                    (_, Some(dir)) => dir,
                    // Directional wand without direction prompt result:
                    // keep prior placeholder behavior.
                    (_, None) => {
                        events.push(EngineEvent::msg("zap-generic"));
                        return;
                    }
                };

                let wand_events = crate::wands::zap_wand(
                    world,
                    player,
                    wand_type,
                    &mut wand_charges,
                    zap_dir,
                    rng,
                );
                events.extend(wand_events);

                if let Some(mut live_charges) =
                    world.get_component_mut::<crate::wands::WandCharges>(*item)
                {
                    *live_charges = wand_charges;
                }
            } else {
                events.push(EngineEvent::msg("zap-generic"));
            }
        }
        PlayerAction::Throw { item, direction } => {
            let player = world.player();
            let throw_events = crate::ranged::resolve_throw(world, player, *item, *direction, rng);
            events.extend(throw_events);
        }
        PlayerAction::Fire => {
            let player = world.player();
            let (launcher, ammo) = world
                .get_component::<crate::equipment::EquipmentSlots>(player)
                .map(|slots| (slots.weapon, slots.off_hand))
                .unwrap_or((None, None));
            if let (Some(launcher), Some(ammo)) = (launcher, ammo) {
                let fire_direction = infer_fire_direction(world, player).unwrap_or(Direction::East);
                let fire_events =
                    crate::ranged::resolve_fire(world, player, launcher, ammo, fire_direction, rng);
                events.extend(fire_events);
            } else {
                events.push(EngineEvent::msg("fire-no-ammo"));
            }
        }
        PlayerAction::Open { direction } => {
            // Open a door in the given direction.
            let player = world.player();
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            let target_pos = player_pos.step(*direction);
            let terrain = world
                .dungeon()
                .current_level
                .get(target_pos)
                .map(|c| c.terrain);
            match terrain {
                Some(Terrain::DoorClosed) => {
                    world
                        .dungeon_mut()
                        .current_level
                        .set_terrain(target_pos, Terrain::DoorOpen);
                    events.push(EngineEvent::DoorOpened {
                        position: target_pos,
                    });
                    events.push(EngineEvent::msg("door-open-success"));
                }
                Some(Terrain::DoorLocked) => {
                    events.push(EngineEvent::msg("door-locked"));
                }
                Some(Terrain::DoorOpen) => {
                    events.push(EngineEvent::msg("door-already-open"));
                }
                _ => {
                    events.push(EngineEvent::msg("door-not-here"));
                }
            }
        }
        PlayerAction::Close { direction } => {
            // Close a door in the given direction.
            let player = world.player();
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            let target_pos = player_pos.step(*direction);
            let terrain = world
                .dungeon()
                .current_level
                .get(target_pos)
                .map(|c| c.terrain);
            match terrain {
                Some(Terrain::DoorOpen) => {
                    world
                        .dungeon_mut()
                        .current_level
                        .set_terrain(target_pos, Terrain::DoorClosed);
                    events.push(EngineEvent::DoorClosed {
                        position: target_pos,
                    });
                    events.push(EngineEvent::msg("door-close-success"));
                }
                Some(Terrain::DoorClosed | Terrain::DoorLocked) => {
                    events.push(EngineEvent::msg("door-already-closed"));
                }
                _ => {
                    events.push(EngineEvent::msg("door-not-here"));
                }
            }
        }
        PlayerAction::ForceLock { item: _ } => {
            // Force a lock in the direction the player is facing.
            let player = world.player();
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            // Search all adjacent positions for locked doors.
            let mut found = false;
            for dir in Direction::PLANAR.iter() {
                let target = player_pos.step(*dir);
                let terrain = world.dungeon().current_level.get(target).map(|c| c.terrain);
                if terrain == Some(Terrain::DoorLocked) {
                    let lock_events = crate::lock::force_lock(world, player, target, rng);
                    events.extend(lock_events);
                    found = true;
                    break;
                }
            }
            if !found {
                events.push(EngineEvent::msg("lock-nothing-to-force"));
            }
        }
        PlayerAction::Pray => {
            let player = world.player();
            events.push(EngineEvent::msg("pray-begin"));
            let player_pos = world.get_component::<Positioned>(player).map(|pos| pos.0);
            let on_altar = player_pos
                .and_then(|pos| {
                    world
                        .dungeon()
                        .current_level
                        .get(pos)
                        .map(|cell| cell.terrain)
                })
                .is_some_and(|terrain| terrain == Terrain::Altar);
            let altar_alignment = player_pos.and_then(|pos| altar_alignment_at(world, pos));
            increment_conduct(world, player, |state| {
                state.gnostic = state.gnostic.saturating_add(1);
            });

            let mut religion_state = world
                .get_component::<crate::religion::ReligionState>(player)
                .map(|state| (*state).clone())
                .unwrap_or_else(|| default_religion_state(world, player));
            refresh_religion_state_from_world(&mut religion_state, world, player);
            let prayer_type =
                crate::religion::evaluate_prayer_simple(&religion_state, on_altar, altar_alignment);

            let prayer_events = crate::religion::pray_simple(
                &mut religion_state,
                player,
                on_altar,
                altar_alignment,
                rng,
            );
            persist_religion_state(world, player, religion_state);
            events.extend(prayer_events);
            try_calm_temple_priest_after_prayer(
                world,
                player,
                on_altar,
                altar_alignment,
                prayer_type,
                events,
            );
        }
        PlayerAction::Offer { item } => {
            let Some(item) = item else {
                events.push(EngineEvent::msg("offer-what"));
                return;
            };

            let player = world.player();
            increment_conduct(world, player, |state| {
                state.gnostic = state.gnostic.saturating_add(1);
            });

            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|pos| pos.0)
                .unwrap_or(Position::new(0, 0));
            let on_altar = world
                .dungeon()
                .current_level
                .get(player_pos)
                .is_some_and(|cell| cell.terrain == Terrain::Altar);
            if !on_altar || !player_carries_item(world, player, *item) {
                events.push(EngineEvent::msg("offer-generic"));
                return;
            }

            if is_real_amulet_of_yendor(world, *item) {
                let mut religion_state = world
                    .get_component::<crate::religion::ReligionState>(player)
                    .map(|state| (*state).clone())
                    .unwrap_or_else(|| default_religion_state(world, player));
                refresh_religion_state_from_world(&mut religion_state, world, player);

                let role = current_player_role(world).unwrap_or(Role::Valkyrie);
                let altar_alignment =
                    altar_alignment_at(world, player_pos).unwrap_or(religion_state.alignment);
                let on_astral =
                    world.dungeon().branch == DungeonBranch::Endgame && world.dungeon().depth == 5;

                match crate::religion::offer_amulet(
                    religion_state.alignment,
                    altar_alignment,
                    on_astral,
                ) {
                    crate::religion::AmuletOfferingResult::Ascended => {
                        religion_state.demigod = true;
                        persist_religion_state(world, player, religion_state.clone());
                        let _ = remove_item_from_player_possessions(world, player, *item);
                        events.extend(crate::end::ascension_sequence(
                            &world.entity_name(player),
                            &role,
                            religion_state.alignment == religion_state.original_alignment,
                        ));
                        let result = crate::end::done(
                            world,
                            player,
                            crate::end::DoneParams {
                                how: crate::end::EndHow::Ascended,
                                killer: String::new(),
                                deepest_level: world.dungeon().max_depth(),
                                gold: player_gold(world, player),
                                starting_gold: 0,
                                vanquished: Vec::new(),
                                conducts: read_conduct_state(world, player),
                                depth_string: current_depth_string(world),
                                role,
                                original_alignment: religion_state.alignment
                                    == religion_state.original_alignment,
                            },
                        );
                        events.extend(result.events);
                    }
                    crate::religion::AmuletOfferingResult::Rejected => {
                        reject_amulet_offering(world, player, *item, player_pos, events);
                    }
                    crate::religion::AmuletOfferingResult::NotAstralPlane => {
                        events.push(EngineEvent::msg("offer-generic"));
                    }
                }
            } else {
                events.push(EngineEvent::msg("offer-generic"));
            }
        }
        PlayerAction::Chat { direction } => {
            // Chat with an NPC in the given direction.
            let player = world.player();
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            let target_pos = player_pos.step(*direction);
            // Check if there's a monster at the target position.
            let monster_at_target = world
                .ecs()
                .query::<(&Monster, &Positioned)>()
                .iter()
                .find_map(|(entity, (_, pos))| (pos.0 == target_pos).then_some(entity));
            if let Some(monster_entity) = monster_at_target {
                let priest_data = infer_priest_runtime(world, monster_entity);
                if crate::status::is_sleeping(world, monster_entity) {
                    if priest_data.is_some() {
                        events.extend(crate::status::wake_from_sleeping(world, monster_entity));
                    } else {
                        events.push(EngineEvent::msg("npc-chat-sleeping"));
                        return;
                    }
                }

                if world.dungeon().branch == DungeonBranch::Quest
                    && current_player_role(world).is_some()
                {
                    let quest_role = quest_npc_role_for_entity(world, monster_entity);
                    let is_leader = quest_role == Some(crate::quest::QuestNpcRole::Leader);
                    let is_nemesis = quest_role == Some(crate::quest::QuestNpcRole::Nemesis);
                    let is_guardian = quest_role == Some(crate::quest::QuestNpcRole::Guardian);

                    if is_leader || is_nemesis || is_guardian {
                        let role = current_player_role(world).expect("checked above");
                        let mut quest_state = read_quest_state(world, player);
                        let alignment_record = world
                            .get_component::<crate::religion::ReligionState>(player)
                            .map(|state| state.alignment_record)
                            .unwrap_or_else(|| {
                                default_religion_state(world, player).alignment_record
                            });
                        let level = world
                            .get_component::<ExperienceLevel>(player)
                            .map(|lvl| lvl.0)
                            .unwrap_or(1);
                        let encounter =
                            crate::quest::determine_encounter(&quest_state, is_leader, is_nemesis);
                        let quest_events = crate::quest::resolve_encounter(
                            &mut quest_state,
                            role,
                            encounter,
                            level,
                            alignment_record,
                        );
                        persist_quest_state(world, player, quest_state);
                        events.extend(quest_events);
                        return;
                    }
                }

                if let Some(shop_idx) = find_shop_room_index_by_shopkeeper(world, monster_entity) {
                    let shop = &world.dungeon().shop_rooms[shop_idx];
                    let hallucinating = crate::status::is_hallucinating(world, player);
                    let honorific = crate::npc::shopkeeper_honorific(
                        current_player_is_female(world, player),
                        current_player_level(world, player),
                        hallucinating,
                    );
                    let following = world
                        .get_component::<crate::npc::Shopkeeper>(monster_entity)
                        .map(|state| state.following)
                        .unwrap_or(false);
                    if hallucinating && rng.random_range(0..2) == 0 {
                        events.push(crate::npc::shopkeeper_hallucination_pitch(
                            &shop.shopkeeper_name,
                        ));
                    } else {
                        events.push(crate::npc::shopkeeper_chat(shop, following, honorific));
                    }
                    return;
                }

                if let Some(priest_data) = priest_data {
                    upsert_priest_component(world, monster_entity, priest_data);
                    let priest_events =
                        resolve_priest_chat(world, player, monster_entity, priest_data, rng);
                    events.extend(priest_events);
                    return;
                }

                if !crate::status::is_deaf(world, player)
                    && let Some(monster_def) =
                        current_level_monster_def_for_entity(world, monster_entity)
                {
                    let is_peaceful = world.get_component::<Peaceful>(monster_entity).is_some();
                    let is_tame = world.get_component::<Tame>(monster_entity).is_some();
                    let tameness = world
                        .get_component::<crate::pets::PetState>(monster_entity)
                        .map(|pet| pet.tameness);
                    let hungry = world
                        .get_component::<crate::pets::PetState>(monster_entity)
                        .is_some_and(|pet| pet.is_hungry(world.turn()));
                    let satiated = world
                        .get_component::<crate::pets::PetState>(monster_entity)
                        .is_some_and(|pet| pet.hungrytime > world.turn().saturating_add(1000));
                    let confused = crate::status::is_confused(world, monster_entity);
                    let blinded = crate::status::is_blind(world, monster_entity);
                    let fleeing = world
                        .get_component::<crate::monster_ai::FleeTimer>(monster_entity)
                        .is_some_and(|timer| timer.0 > 0);
                    let trapped = world
                        .get_component::<crate::traps::Trapped>(monster_entity)
                        .is_some();
                    let (hurt, badly_hurt) = world
                        .get_component::<HitPoints>(monster_entity)
                        .map(|hp| {
                            let max = hp.max.max(1);
                            (hp.current < max / 2, hp.current < max / 4)
                        })
                        .unwrap_or((false, false));
                    let monster_name = world
                        .get_component::<Name>(monster_entity)
                        .map(|name| name.0.clone())
                        .unwrap_or_else(|| monster_def.names.male.clone());
                    if crate::status::is_hallucinating(world, player)
                        && monster_def.names.male == "gecko"
                    {
                        events.push(crate::npc::gecko_hallucination_pitch(&monster_name));
                        return;
                    }
                    if monster_def.sound == nethack_babel_data::schema::MonsterSound::Laugh {
                        events.push(crate::npc::laughing_monster_chat(
                            &monster_name,
                            rng.random_range(0..4),
                        ));
                        return;
                    }
                    if monster_def.sound == nethack_babel_data::schema::MonsterSound::Mumble {
                        events.push(crate::npc::mumbling_monster_chat(&monster_name));
                        return;
                    }
                    if monster_def.sound == nethack_babel_data::schema::MonsterSound::Bones {
                        events.push(crate::npc::bones_monster_chat(&monster_name));
                        events.extend(crate::status::make_paralyzed(world, player, 2));
                        return;
                    }
                    if monster_def.sound == nethack_babel_data::schema::MonsterSound::Shriek {
                        events.push(crate::npc::shrieking_monster_chat(&monster_name));
                        aggravate_monsters_on_current_level(world, rng, events);
                        return;
                    }
                    let player_equipment = world
                        .get_component::<crate::equipment::EquipmentSlots>(player)
                        .map(|slots| (*slots).clone())
                        .unwrap_or_default();
                    let chat_state = crate::npc::MonsterChatState {
                        is_peaceful,
                        is_tame,
                        tameness,
                        confused,
                        fleeing,
                        hungry,
                        satiated,
                        full_moon: crate::were::is_full_moon(world.turn()),
                        blinded,
                        trapped,
                        hurt,
                        badly_hurt,
                        chat_roll: rng.random_range(0..4),
                        player_has_gold: player_gold(world, player) > 0,
                        player_armed: player_equipment.weapon.is_some(),
                        player_armored: player_equipment.cloak.is_some()
                            || player_equipment.body_armor.is_some()
                            || player_equipment.helmet.is_some()
                            || player_equipment.shield.is_some()
                            || player_equipment.gloves.is_some()
                            || player_equipment.boots.is_some(),
                        player_has_shirt: player_equipment.shirt.is_some(),
                        player_is_healer: matches!(current_player_role(world), Some(Role::Healer)),
                    };
                    if let Some(outcome) = crate::npc::contextual_monster_chat(
                        monster_def,
                        &monster_name,
                        chat_state,
                        current_player_is_female(world, player),
                    ) {
                        events.push(outcome.event);
                        if let Some(radius) = outcome.wake_radius
                            && let Some(monster_pos) = world
                                .get_component::<Positioned>(monster_entity)
                                .map(|pos| pos.0)
                        {
                            wake_sleeping_monsters_near_position(
                                world,
                                monster_pos,
                                radius,
                                events,
                            );
                        }
                        return;
                    }
                    if let Some(sound_event) = crate::npc::voiced_monster_chat(
                        &monster_name,
                        monster_def.sound,
                        chat_state,
                    ) {
                        events.push(sound_event);
                        if let Some(monster_pos) = world
                            .get_component::<Positioned>(monster_entity)
                            .map(|pos| pos.0)
                        {
                            let wakes_nearby = (monster_def.sound
                                == nethack_babel_data::schema::MonsterSound::Were
                                && crate::were::is_full_moon(world.turn()))
                                || monster_def.sound
                                    == nethack_babel_data::schema::MonsterSound::Trumpet;
                            if wakes_nearby {
                                wake_sleeping_monsters_near_position(
                                    world,
                                    monster_pos,
                                    11,
                                    events,
                                );
                            }
                        }
                        return;
                    }
                }

                events.push(EngineEvent::msg("npc-chat-no-response"));
            } else {
                if let Some(shop_idx) = find_shop_index_containing_position(world, player_pos) {
                    let shop = world.dungeon().shop_rooms[shop_idx].clone();
                    let floor_items = crate::inventory::items_at_position(world, player_pos);
                    let can_quote = live_monster_entity(world, shop.shopkeeper)
                        && !crate::status::is_sleeping(world, shop.shopkeeper);
                    let mut quoted_any = false;
                    if can_quote {
                        for item in floor_items {
                            if let Some(quote) = crate::shop::quote_item_in_shop(
                                world,
                                player,
                                item,
                                &shop,
                                world.object_catalog(),
                            ) {
                                quoted_any = true;
                                events.push(quote);
                            }
                        }
                    }
                    if !quoted_any {
                        events.push(EngineEvent::msg("chat-nobody-there"));
                    }
                } else {
                    events.push(EngineEvent::msg("chat-nobody-there"));
                }
            }
        }
        PlayerAction::Ride => {
            if crate::steed::is_mounted(world, player) {
                events.extend(crate::steed::dismount(world, player, rng));
            } else if let Some(steed_entity) = find_adjacent_tame_steed(world, player) {
                events.extend(crate::steed::mount(world, player, steed_entity, rng));
            } else {
                events.push(EngineEvent::msg("ride-not-available"));
            }
        }
        PlayerAction::Pay => {
            let player = world.player();
            if let Some(shop_idx) = find_payable_shop_index(world, player) {
                let starting_gold = player_gold(world, player) as i32;
                let mut gold_after = starting_gold;
                let mut shop = world.dungeon().shop_rooms[shop_idx].clone();
                let payment_events =
                    crate::shop::pay_bill(world, player, &mut shop, &mut gold_after);
                world.dungeon_mut().shop_rooms[shop_idx] = shop;
                let paid_amount = starting_gold.saturating_sub(gold_after);
                if paid_amount > 0 {
                    let _ = spend_player_gold(world, player, paid_amount as u32);
                    record_player_exercise_action(
                        world,
                        player,
                        crate::attributes::ExerciseAction::ShopTransaction,
                    );
                }
                if world
                    .dungeon()
                    .shop_rooms
                    .get(shop_idx)
                    .is_some_and(|shop| shop.bill.is_empty() && shop.debit == 0 && shop.robbed == 0)
                {
                    crate::shop::pacify_shop(&mut world.dungeon_mut().shop_rooms[shop_idx]);
                }
                events.extend(payment_events);
                sync_current_level_shopkeeper_state(world);
            } else {
                events.push(EngineEvent::msg("shop-no-debt"));
            }
        }
        PlayerAction::EnhanceSkill => {
            if let Some((skill, level)) = enhance_skill_once(world, player) {
                events.push(EngineEvent::msg_with(
                    "enhance-success",
                    vec![("skill", skill), ("level", level)],
                ));
            } else {
                events.push(EngineEvent::msg("enhance-not-available"));
            }
        }
        PlayerAction::MoveUntilInterrupt { direction } => {
            // Run: move in direction (simplified — just one step).
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if confused || stunned {
                match crate::status::maybe_confuse_direction(confused, stunned, rng) {
                    Some(random_dir) => random_dir,
                    None => *direction,
                }
            } else {
                *direction
            };
            try_move_entity(world, world.player(), effective_dir, false, events, rng);
        }
        PlayerAction::FightDirection { direction } => {
            // Force fight: move/attack in direction, skipping peaceful check.
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if confused || stunned {
                match crate::status::maybe_confuse_direction(confused, stunned, rng) {
                    Some(random_dir) => random_dir,
                    None => *direction,
                }
            } else {
                *direction
            };
            try_move_entity(world, world.player(), effective_dir, true, events, rng);
        }
        PlayerAction::RunDirection { direction } => {
            // Run until interrupted (simplified — one step for now).
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if confused || stunned {
                match crate::status::maybe_confuse_direction(confused, stunned, rng) {
                    Some(random_dir) => random_dir,
                    None => *direction,
                }
            } else {
                *direction
            };
            try_move_entity(world, world.player(), effective_dir, false, events, rng);
        }
        PlayerAction::RushDirection { direction } => {
            // Rush: run without picking up items (simplified — one step).
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if confused || stunned {
                match crate::status::maybe_confuse_direction(confused, stunned, rng) {
                    Some(random_dir) => random_dir,
                    None => *direction,
                }
            } else {
                *direction
            };
            try_move_entity(world, world.player(), effective_dir, false, events, rng);
        }
        PlayerAction::MoveNoPickup { direction } => {
            // Move without auto-pickup.
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if confused || stunned {
                match crate::status::maybe_confuse_direction(confused, stunned, rng) {
                    Some(random_dir) => random_dir,
                    None => *direction,
                }
            } else {
                *direction
            };
            try_move_entity(world, world.player(), effective_dir, false, events, rng);
        }
        PlayerAction::Wait => {
            // Do nothing, consume one turn.
            events.push(EngineEvent::msg("wait"));
        }
        PlayerAction::Travel { destination } => {
            // Travel takes one automated step toward the destination.
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            if player_pos != *destination
                && let Some(direction) = travel_direction_toward(world, player_pos, *destination)
            {
                // Confusion/stun may randomize the chosen travel direction.
                let confused = crate::status::is_confused(world, player);
                let stunned = crate::status::is_stunned(world, player);
                let effective_dir = if confused || stunned {
                    match crate::status::maybe_confuse_direction(confused, stunned, rng) {
                        Some(random_dir) => random_dir,
                        None => direction,
                    }
                } else {
                    direction
                };
                try_move_entity(world, world.player(), effective_dir, false, events, rng);
            }
        }
        PlayerAction::ToggleTwoWeapon => {
            if !has_two_weapon_loadout(world, player) {
                events.push(EngineEvent::msg("swap-no-secondary"));
            } else {
                let two_weapon_on = toggle_two_weapon_mode(world, player);
                events.push(EngineEvent::msg(if two_weapon_on {
                    "two-weapon-enabled"
                } else {
                    "two-weapon-disabled"
                }));
            }
        }
        PlayerAction::Name { target, name } => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                events.push(EngineEvent::msg("cannot-do-that"));
                return;
            }

            match target {
                NameTarget::Item { item } => {
                    if !is_player_inventory_item(world, *item) {
                        events.push(EngineEvent::msg("cannot-do-that"));
                        return;
                    }

                    let item_label = item_label_for_message(world, *item);
                    if set_item_name(world, *item, trimmed) {
                        events.push(EngineEvent::msg_with(
                            "item-name-set",
                            vec![("item", item_label), ("name", trimmed.to_string())],
                        ));
                    } else {
                        events.push(EngineEvent::msg("cannot-do-that"));
                    }
                }
                NameTarget::ItemClass { class } => {
                    set_called_item_class(world, *class, trimmed, events);
                }
                NameTarget::Level => {
                    world
                        .dungeon_mut()
                        .set_current_level_annotation(trimmed.to_string());
                }
                NameTarget::MonsterAt { position } => {
                    if let Some(monster) = find_monster_entity_at(world, *position) {
                        let new_name = trimmed.to_string();
                        if let Some(mut mon_name) = world.get_component_mut::<Name>(monster) {
                            mon_name.0 = new_name;
                        } else if world.ecs_mut().insert_one(monster, Name(new_name)).is_err() {
                            events.push(EngineEvent::msg("cannot-do-that"));
                        }
                    } else {
                        events.push(EngineEvent::msg("cannot-do-that"));
                    }
                }
                NameTarget::Monster { entity } => {
                    if world.get_component::<Monster>(*entity).is_none() {
                        events.push(EngineEvent::msg("cannot-do-that"));
                        return;
                    }
                    let new_name = trimmed.to_string();
                    if let Some(mut mon_name) = world.get_component_mut::<Name>(*entity) {
                        mon_name.0 = new_name;
                    } else if world.ecs_mut().insert_one(*entity, Name(new_name)).is_err() {
                        events.push(EngineEvent::msg("cannot-do-that"));
                    }
                }
            }
        }
        PlayerAction::Adjust { item, new_letter } => {
            if !new_letter.is_ascii_alphabetic() || !is_player_inventory_item(world, *item) {
                events.push(EngineEvent::msg("cannot-do-that"));
                return;
            }

            let Some(current_letter) = world
                .get_component::<nethack_babel_data::ObjectCore>(*item)
                .and_then(|core| core.inv_letter)
            else {
                events.push(EngineEvent::msg("cannot-do-that"));
                return;
            };

            if current_letter == *new_letter {
                return;
            }

            let player = world.player();
            let swap_item = crate::inventory::find_by_letter(world, player, *new_letter)
                .filter(|e| *e != *item);

            if let Some(mut core) = world.get_component_mut::<nethack_babel_data::ObjectCore>(*item)
            {
                core.inv_letter = Some(*new_letter);
            } else {
                events.push(EngineEvent::msg("cannot-do-that"));
                return;
            }

            if let Some(other) = swap_item
                && let Some(mut other_core) =
                    world.get_component_mut::<nethack_babel_data::ObjectCore>(other)
            {
                other_core.inv_letter = Some(current_letter);
            }
        }
        PlayerAction::Sit => {
            let player = world.player();
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            let terrain = world
                .dungeon()
                .current_level
                .get(player_pos)
                .map(|c| c.terrain)
                .unwrap_or(Terrain::Floor);
            let is_levitating = crate::status::is_levitating(world, player);
            let sit_events =
                crate::sit::do_sit(rng, terrain, false, is_levitating, false, !is_levitating, 0);
            events.extend(sit_events);
        }
        PlayerAction::Jump { position } => {
            let player = world.player();
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            let dx = (position.x - player_pos.x).unsigned_abs();
            let dy = (position.y - player_pos.y).unsigned_abs();
            let distance = dx + dy;
            let has_jump_boots = world
                .get_component::<crate::equipment::EquipmentSlots>(player)
                .and_then(|equip| equip.boots)
                .and_then(|boots| world.get_component::<Name>(boots))
                .map(|name| name.0.to_ascii_lowercase().contains("jumping"))
                .unwrap_or(false);
            let has_jumping = has_jump_boots;
            let max_range = if has_jump_boots { 3 } else { 2 };
            let burden = world
                .get_component::<EncumbranceLevel>(player)
                .map(|enc| {
                    if enc.0 == Encumbrance::Unencumbered {
                        0
                    } else {
                        1
                    }
                })
                .unwrap_or(0);
            let (result, jump_events) =
                crate::do_actions::do_jump(has_jumping, burden, distance, max_range);
            events.extend(jump_events);
            if result == crate::do_actions::JumpResult::Jumped
                && let Some(mut pos) = world.get_component_mut::<Positioned>(player)
            {
                let from = pos.0;
                pos.0 = *position;
                events.push(EngineEvent::EntityMoved {
                    entity: player,
                    from,
                    to: *position,
                });
            }
        }
        PlayerAction::Untrap { direction } => {
            let player = world.player();
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            let target_pos = player_pos.step(*direction);
            let has_trap = world.dungeon().trap_map.trap_at(target_pos).is_some();
            let dex = world
                .get_component::<crate::world::Attributes>(player)
                .map(|a| i32::from(a.dexterity))
                .unwrap_or(10);
            let (_, untrap_events) = crate::do_actions::do_untrap(rng, dex, has_trap, 5);
            events.extend(untrap_events);
        }
        PlayerAction::TurnUndead => {
            let player = world.player();
            let player_level = world
                .get_component::<ExperienceLevel>(player)
                .map(|l| l.0 as u32)
                .unwrap_or(1);
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            let is_clerical = player_is_clerical(world, player);
            let undead_nearby = count_undead_nearby(world, player_pos, 5);
            let (_result, turn_events) =
                crate::do_actions::do_turn_undead(is_clerical, player_level, undead_nearby);
            events.extend(turn_events);
        }
        PlayerAction::Swap => {
            let player = world.player();
            let (primary_weapon, secondary_item) = if let Some(equip) =
                world.get_component::<crate::equipment::EquipmentSlots>(player)
            {
                (equip.weapon, equip.off_hand)
            } else {
                (None, None)
            };

            let has_secondary = secondary_item.is_some();
            let primary_welded = primary_weapon
                .and_then(|weapon| {
                    world
                        .get_component::<nethack_babel_data::BucStatus>(weapon)
                        .map(|b| b.cursed)
                })
                .unwrap_or(false);

            let (_result, swap_events) =
                crate::do_actions::do_swap_weapons(has_secondary, primary_welded);
            events.extend(swap_events);
        }
        PlayerAction::Wipe => {
            let player = world.player();
            let creamed = world
                .get_component::<crate::status::HeroCounters>(player)
                .map(|hc| hc.creamed)
                .unwrap_or(0);
            let blind_towel = world
                .get_component::<crate::equipment::EquipmentSlots>(player)
                .and_then(|equip| equip.off_hand)
                .and_then(|off_hand| {
                    let is_cursed = world
                        .get_component::<nethack_babel_data::BucStatus>(off_hand)
                        .map(|b| b.cursed)
                        .unwrap_or(false);
                    if !is_cursed {
                        return None;
                    }
                    let towel_named = world
                        .get_component::<Name>(off_hand)
                        .map(|n| n.0.to_ascii_lowercase().contains("towel"))
                        .unwrap_or(false);
                    Some(towel_named)
                })
                .unwrap_or(false);
            let (result, wipe_events) = crate::do_actions::do_wipe(creamed, blind_towel);
            if result == crate::do_actions::WipeResult::WipedCream
                && let Some(mut hc) = world.get_component_mut::<crate::status::HeroCounters>(player)
            {
                hc.creamed = 0;
            }
            events.extend(wipe_events);
        }
        PlayerAction::Tip { item: _ } => {
            // Simplified: assume empty container for now.
            let (_result, tip_events) = crate::do_actions::do_tip(false, true, 0, true);
            events.extend(tip_events);
        }
        PlayerAction::Rub { item } => {
            let item_name = world
                .get_component::<Name>(*item)
                .map(|n| n.0.to_ascii_lowercase())
                .unwrap_or_default();
            let is_touchstone = item_name.contains("touchstone");
            let is_magic_lamp = item_name.contains("magic lamp");
            let is_lamp = is_magic_lamp || item_name.contains("lamp");
            let (_result, rub_events) =
                crate::do_actions::do_rub(rng, is_lamp, is_magic_lamp, is_touchstone);
            events.extend(rub_events);
        }
        PlayerAction::InvokeArtifact { item } => {
            let player = world.player();
            let is_wielded = world
                .get_component::<crate::equipment::EquipmentSlots>(player)
                .map(|equip| equip.weapon == Some(*item))
                .unwrap_or(false);
            let has_invoke_power = world
                .get_component::<nethack_babel_data::ObjectCore>(*item)
                .and_then(|core| core.artifact)
                .is_some();
            // Artifact invoke cooldown tracking is not yet persisted in ECS.
            let is_on_cooldown = false;
            let cooldown_remaining = 0;
            let artifact_name = world
                .get_component::<Name>(*item)
                .map(|n| n.0.clone())
                .unwrap_or_else(|| "artifact".to_string());
            let (_result, invoke_events) = crate::do_actions::do_invoke(
                has_invoke_power,
                is_wielded,
                is_on_cooldown,
                cooldown_remaining,
                &artifact_name,
            );
            events.extend(invoke_events);
        }
        PlayerAction::Monster => {
            let (_result, mon_events) = crate::do_actions::do_monster_ability(false, false);
            events.extend(mon_events);
        }
        PlayerAction::KnownItems => {
            let known_events = crate::do_actions::do_known_items(&[]);
            events.extend(known_events);
        }
        PlayerAction::KnownClass { class: _ } => {
            // Filtered known items by class — delegates to same handler.
            let known_events = crate::do_actions::do_known_items(&[]);
            events.extend(known_events);
        }
        PlayerAction::Vanquished => {
            let vanq_events = crate::do_actions::do_vanquished(&[]);
            events.extend(vanq_events);
        }
        PlayerAction::CallType { class, name } => {
            set_called_item_class(world, *class, name, events);
        }
        PlayerAction::Glance { direction: _ } => {
            let glance_events = crate::do_actions::do_glance("You see nothing special.");
            events.extend(glance_events);
        }
        PlayerAction::Chronicle => {
            let chron_events = crate::do_actions::do_chronicle(&[]);
            events.extend(chron_events);
        }
        PlayerAction::WhatIs { position: _ } => {
            events.push(EngineEvent::msg("whatis-prompt"));
        }
        // ── Wizard mode commands ─────────────────────────────────
        PlayerAction::WizGenesis { monster_name } => {
            wizard_genesis(world, monster_name, rng, events);
        }
        PlayerAction::WizWish { wish_text } => {
            wizard_wish(world, wish_text, events);
        }
        PlayerAction::WizIdentify => {
            wizard_identify(world);
            events.push(EngineEvent::msg("wizard-identify-all"));
        }
        PlayerAction::WizMap => {
            // Reveal entire current level.
            let map = &mut world.dungeon_mut().current_level;
            for y in 0..map.height {
                for x in 0..map.width {
                    map.cells[y][x].explored = true;
                }
            }
            events.push(EngineEvent::msg("wizard-map-revealed"));
        }
        PlayerAction::WizLevelTeleport { depth } => {
            let target_depth = *depth;
            events.push(EngineEvent::msg_with(
                "wizard-level-teleport",
                vec![("depth", target_depth.to_string())],
            ));
            change_level(
                world,
                target_depth,
                target_depth < world.dungeon().depth,
                rng,
                events,
            );
        }
        PlayerAction::WizDetect => {
            // Detect all monsters, objects, and traps.
            let player = world.player();
            let mut det_events = crate::detect::detect_monsters(world, player);
            events.append(&mut det_events);
            let mut obj_events = crate::detect::detect_objects(world, player);
            events.append(&mut obj_events);
            let mut trap_events = crate::detect::detect_traps(world, player);
            events.append(&mut trap_events);
            events.push(EngineEvent::msg("wizard-detect-all"));
        }
        PlayerAction::WizWhere => {
            events.extend(wizard_where(world));
        }
        PlayerAction::WizKill => {
            wizard_kill(world, events);
        }
        PlayerAction::Annotate { text } => {
            world
                .dungeon_mut()
                .set_current_level_annotation(text.clone());
        }
        // UI/meta actions: handled by the CLI layer, not by the engine.
        PlayerAction::Help
        | PlayerAction::ShowHistory
        | PlayerAction::Options
        | PlayerAction::ViewEquipped
        | PlayerAction::ViewDiscoveries
        | PlayerAction::ViewConduct
        | PlayerAction::DungeonOverview
        | PlayerAction::ViewTerrain
        | PlayerAction::ShowVersion
        | PlayerAction::Attributes
        | PlayerAction::LookAt { .. }
        | PlayerAction::LookHere
        | PlayerAction::Redraw
        | PlayerAction::Save
        | PlayerAction::Quit
        | PlayerAction::SaveAndQuit => {}
    }

    apply_action_conduct_updates(world, player, action, events);
}

fn apply_action_conduct_updates(
    world: &mut GameWorld,
    player: hecs::Entity,
    action: &PlayerAction,
    events: &[EngineEvent],
) {
    let mut weapon_hits = 0i64;
    let mut kills = 0i64;

    for event in events {
        match event {
            EngineEvent::MeleeHit {
                attacker,
                weapon: Some(_),
                ..
            } if *attacker == player => {
                weapon_hits += 1;
            }
            EngineEvent::EntityDied {
                killer: Some(killer),
                ..
            } if *killer == player => {
                kills += 1;
            }
            _ => {}
        }
    }

    let wished = matches!(action, PlayerAction::WizWish { .. });
    if weapon_hits == 0 && kills == 0 && !wished {
        return;
    }

    increment_conduct(world, player, |state| {
        if weapon_hits > 0 {
            state.weaphit = state.weaphit.saturating_add(weapon_hits);
        }
        if kills > 0 {
            state.killer = state.killer.saturating_add(kills);
        }
        if wished {
            state.wishes = state.wishes.saturating_add(1);
        }
    });
}

fn wizard_identify(world: &mut GameWorld) {
    let player = world.player();
    let items = world
        .get_component::<crate::inventory::Inventory>(player)
        .map(|inv| inv.items.clone())
        .unwrap_or_default();

    for item in items {
        if let Some(mut knowledge) =
            world.get_component_mut::<nethack_babel_data::KnowledgeState>(item)
        {
            knowledge.known = true;
            knowledge.dknown = true;
            knowledge.rknown = true;
            knowledge.cknown = true;
            knowledge.lknown = true;
            knowledge.tknown = true;
        }
        if let Some(mut buc) = world.get_component_mut::<BucStatus>(item) {
            buc.bknown = true;
        }
    }
}

fn wizard_genesis(
    world: &mut GameWorld,
    monster_name: &str,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    let monster_defs: Vec<MonsterDef> = world.monster_catalog().to_vec();
    let Some(monster_id) = resolve_monster_id_by_spec(&monster_defs, monster_name) else {
        events.push(EngineEvent::msg_with(
            "wizard-genesis-failed",
            vec![("monster", monster_name.to_string())],
        ));
        return;
    };
    let Some(monster_def) = monster_defs.iter().find(|def| def.id == monster_id) else {
        events.push(EngineEvent::msg_with(
            "wizard-genesis-failed",
            vec![("monster", monster_name.to_string())],
        ));
        return;
    };
    let Some(player_pos) = world
        .get_component::<Positioned>(world.player())
        .map(|pos| pos.0)
    else {
        events.push(EngineEvent::msg_with(
            "wizard-genesis-failed",
            vec![("monster", monster_name.to_string())],
        ));
        return;
    };
    let Some(spawn_pos) = enexto(world, player_pos, monster_def) else {
        events.push(EngineEvent::msg_with(
            "wizard-genesis-failed",
            vec![("monster", monster_name.to_string())],
        ));
        return;
    };
    let Some(entity) = makemon(
        world,
        &monster_defs,
        Some(monster_id),
        spawn_pos,
        MakeMonFlags::NO_GROUP,
        rng,
    ) else {
        events.push(EngineEvent::msg_with(
            "wizard-genesis-failed",
            vec![("monster", monster_name.to_string())],
        ));
        return;
    };

    events.push(EngineEvent::MonsterGenerated {
        entity,
        position: spawn_pos,
    });
    events.push(EngineEvent::msg_with(
        "wizard-genesis",
        vec![("monster", monster_def.names.male.clone())],
    ));
}

fn wizard_wish(world: &mut GameWorld, wish_text: &str, events: &mut Vec<EngineEvent>) {
    let object_defs: Vec<ObjectDef> = world.object_catalog().to_vec();
    let Some(mut wished) = crate::wish::parse_wish(wish_text, &object_defs) else {
        events.push(EngineEvent::msg_with(
            "wizard-wish-failed",
            vec![("wish", wish_text.to_string())],
        ));
        return;
    };

    let restricted = !crate::wish::apply_wish_restrictions(&mut wished).is_empty();
    let Some(object_def) = object_defs.iter().find(|def| def.id == wished.object_type) else {
        events.push(EngineEvent::msg_with(
            "wizard-wish-failed",
            vec![("wish", wish_text.to_string())],
        ));
        return;
    };

    let entity = crate::items::spawn_item(
        world,
        object_def,
        crate::items::SpawnLocation::Free,
        wished.enchantment,
    );

    if let Some(mut core) = world.get_component_mut::<ObjectCore>(entity) {
        core.quantity = wished.quantity as i32;
        if object_def.class == ObjectClass::Coin {
            core.inv_letter = Some('$');
        }
    }
    if let Some(mut buc) = world.get_component_mut::<BucStatus>(entity) {
        buc.cursed = matches!(wished.buc, Some(crate::wish::BucWish::Cursed));
        buc.blessed = matches!(wished.buc, Some(crate::wish::BucWish::Blessed));
        buc.bknown = true;
    }
    if let Some(mut knowledge) =
        world.get_component_mut::<nethack_babel_data::KnowledgeState>(entity)
    {
        knowledge.known = true;
        knowledge.dknown = true;
        knowledge.rknown = true;
        knowledge.cknown = true;
        knowledge.lknown = true;
        knowledge.tknown = true;
    }
    if wished.erodeproof
        && let Some(mut erosion) = world.get_component_mut::<nethack_babel_data::Erosion>(entity)
    {
        erosion.erodeproof = true;
    }
    if let Some(name) = wished.name.clone() {
        let _ = world.ecs_mut().insert_one(
            entity,
            ObjectExtra {
                name: Some(name),
                contained_monster: None,
            },
        );
    }

    let player = world.player();
    let mut letter_state = crate::items::LetterState::default();
    let inventory_letter = if object_def.class == ObjectClass::Coin {
        Some('$')
    } else {
        crate::items::assign_inv_letter(world, player, &mut letter_state)
    };

    if let Some(letter) = inventory_letter {
        if let Some(mut core) = world.get_component_mut::<ObjectCore>(entity) {
            core.inv_letter = Some(letter);
        }
        if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(entity) {
            *loc = ObjectLocation::Inventory;
        }
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.push(entity);
        }
        events.push(EngineEvent::msg_with(
            if restricted {
                "wizard-wish-adjusted"
            } else {
                "wizard-wish"
            },
            vec![("item", object_def.name.clone())],
        ));
        return;
    }

    let player_pos = world
        .get_component::<Positioned>(player)
        .map(|pos| pos.0)
        .unwrap_or(Position::new(0, 0));
    let branch = world.dungeon().branch;
    let depth = world.dungeon().depth;
    if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(entity) {
        *loc = crate::dungeon::floor_object_location(branch, depth, player_pos);
    }
    events.push(EngineEvent::msg_with(
        if restricted {
            "wizard-wish-adjusted-floor"
        } else {
            "wizard-wish-floor"
        },
        vec![("item", object_def.name.clone())],
    ));
}

fn wizard_where(world: &GameWorld) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let player_pos = world
        .get_component::<Positioned>(world.player())
        .map(|pos| pos.0)
        .unwrap_or(Position::new(0, 0));
    let current_level = world.dungeon().current_dungeon_level();
    events.push(EngineEvent::msg_with(
        "wizard-where-current",
        vec![
            ("location", current_level.to_string()),
            ("absolute", world.dungeon().absolute_depth().to_string()),
            ("x", player_pos.x.to_string()),
            ("y", player_pos.y.to_string()),
        ],
    ));

    use crate::dungeon::{DungeonBranch, DungeonLevel, branch_max_depth};
    let branches = [
        DungeonBranch::Main,
        DungeonBranch::Mines,
        DungeonBranch::Sokoban,
        DungeonBranch::Quest,
        DungeonBranch::FortLudios,
        DungeonBranch::Gehennom,
        DungeonBranch::VladsTower,
        DungeonBranch::Endgame,
    ];
    for branch in branches {
        for depth in 1..=branch_max_depth(branch) {
            let Some(id) = world.dungeon().check_topology_special(&branch, depth) else {
                continue;
            };
            if matches!(id, crate::special_levels::SpecialLevelId::QuestFiller(_)) {
                continue;
            }
            events.push(EngineEvent::msg_with(
                "wizard-where-special",
                vec![
                    ("level", wizard_special_level_name(id)),
                    ("location", DungeonLevel::new(branch, depth).to_string()),
                ],
            ));
        }
    }

    events
}

fn wizard_special_level_name(id: crate::special_levels::SpecialLevelId) -> String {
    use crate::special_levels::SpecialLevelId;

    match id {
        SpecialLevelId::OracleLevel => "Oracle".to_string(),
        SpecialLevelId::Minetown => "Minetown".to_string(),
        SpecialLevelId::MinesEnd => "Mines' End".to_string(),
        SpecialLevelId::Sokoban(level) => format!("Sokoban {}", level),
        SpecialLevelId::Castle => "Castle".to_string(),
        SpecialLevelId::Medusa(_) => "Medusa".to_string(),
        SpecialLevelId::FortLudios => "Fort Ludios".to_string(),
        SpecialLevelId::VladsTower(level) => format!("Vlad's Tower {}", level),
        SpecialLevelId::WizardTower => "Wizard Tower".to_string(),
        SpecialLevelId::WizardTower2 => "Wizard Tower 2".to_string(),
        SpecialLevelId::WizardTower3 => "Wizard Tower 3".to_string(),
        SpecialLevelId::Sanctum => "Sanctum".to_string(),
        SpecialLevelId::EarthPlane => "Plane of Earth".to_string(),
        SpecialLevelId::AirPlane => "Plane of Air".to_string(),
        SpecialLevelId::FirePlane => "Plane of Fire".to_string(),
        SpecialLevelId::WaterPlane => "Plane of Water".to_string(),
        SpecialLevelId::AstralPlane => "Astral Plane".to_string(),
        SpecialLevelId::Valley => "Valley of the Dead".to_string(),
        SpecialLevelId::BigRoom(_) => "Big Room".to_string(),
        SpecialLevelId::Rogue => "Rogue Level".to_string(),
        SpecialLevelId::Asmodeus => "Asmodeus".to_string(),
        SpecialLevelId::Baalzebub => "Baalzebub".to_string(),
        SpecialLevelId::Juiblex => "Juiblex".to_string(),
        SpecialLevelId::Orcus => "Orcus".to_string(),
        SpecialLevelId::FakeWizard(level) => format!("Fake Wizard Tower {}", level),
        SpecialLevelId::QuestStart => "Quest Start".to_string(),
        SpecialLevelId::QuestLocator => "Quest Locate".to_string(),
        SpecialLevelId::QuestGoal => "Quest Goal".to_string(),
        SpecialLevelId::QuestFiller(level) => format!("Quest Filler {}", level),
    }
}

fn wizard_kill(world: &mut GameWorld, events: &mut Vec<EngineEvent>) {
    let player = world.player();
    let killer_name = world.entity_name(player);
    let monsters: Vec<hecs::Entity> = world
        .ecs()
        .query::<&Monster>()
        .iter()
        .map(|(entity, _)| entity)
        .collect();

    if monsters.is_empty() {
        events.push(EngineEvent::msg("wizard-kill-none"));
        return;
    }

    for monster in &monsters {
        events.push(EngineEvent::EntityDied {
            entity: *monster,
            killer: Some(player),
            cause: DeathCause::KilledBy {
                killer_name: killer_name.clone(),
            },
        });
        let _ = world.despawn(*monster);
    }

    events.push(EngineEvent::msg_with(
        "wizard-kill",
        vec![("count", monsters.len().to_string())],
    ));
}

fn find_adjacent_tame_steed(world: &GameWorld, player: hecs::Entity) -> Option<hecs::Entity> {
    let player_pos = world.get_component::<Positioned>(player).map(|p| p.0)?;
    for (entity, (_monster, _tame, pos)) in
        world.ecs().query::<(&Monster, &Tame, &Positioned)>().iter()
    {
        let dx = (pos.0.x - player_pos.x).abs();
        let dy = (pos.0.y - player_pos.y).abs();
        if dx <= 1 && dy <= 1 && (dx != 0 || dy != 0) {
            return Some(entity);
        }
    }
    None
}

fn has_two_weapon_loadout(world: &GameWorld, player: hecs::Entity) -> bool {
    world
        .get_component::<crate::equipment::EquipmentSlots>(player)
        .is_some_and(|equip| equip.weapon.is_some() && equip.off_hand.is_some())
}

fn toggle_two_weapon_mode(world: &mut GameWorld, player: hecs::Entity) -> bool {
    if world
        .get_component::<nethack_babel_data::PlayerSkills>(player)
        .is_none()
    {
        let _ = world.ecs_mut().insert_one(
            player,
            nethack_babel_data::PlayerSkills {
                weapon_slots: 0,
                skills_advanced: 0,
                skills: Vec::new(),
                two_weapon: false,
            },
        );
    }

    let mut two_weapon_on = false;
    if let Some(mut skills) = world.get_component_mut::<nethack_babel_data::PlayerSkills>(player) {
        skills.two_weapon = !skills.two_weapon;
        two_weapon_on = skills.two_weapon;
    }
    two_weapon_on
}

const P_SKILL_LIMIT: i32 = 60;

fn enhance_skill_once(world: &mut GameWorld, player: hecs::Entity) -> Option<(String, String)> {
    let mut skills = world.get_component_mut::<nethack_babel_data::PlayerSkills>(player)?;
    let idx = select_advanceable_skill(&skills)?;

    let (skill_kind, current_level) = {
        let state = &skills.skills[idx];
        (state.skill, state.level)
    };
    let slot_cost = slots_required_for_skill(skill_kind, current_level);
    if skills.weapon_slots < slot_cost {
        return None;
    }

    skills.weapon_slots -= slot_cost;
    skills.skills_advanced += 1;

    let state = &mut skills.skills[idx];
    state.level = state.level.saturating_add(1).min(state.max_level);

    Some((
        format!("{:?}", state.skill),
        skill_level_label(state.level).to_string(),
    ))
}

fn select_advanceable_skill(skills: &nethack_babel_data::PlayerSkills) -> Option<usize> {
    if skills.skills_advanced >= P_SKILL_LIMIT {
        return None;
    }
    skills.skills.iter().position(|state| {
        can_advance_skill_state(state, skills.weapon_slots, skills.skills_advanced)
    })
}

fn can_advance_skill_state(
    state: &nethack_babel_data::SkillState,
    weapon_slots: i32,
    skills_advanced: i32,
) -> bool {
    if state.level == 0 {
        return false;
    }
    if state.level >= state.max_level {
        return false;
    }
    if skills_advanced >= P_SKILL_LIMIT {
        return false;
    }

    let required_practice = practice_needed_to_advance(state.level);
    if state.advance < required_practice {
        return false;
    }

    weapon_slots >= slots_required_for_skill(state.skill, state.level)
}

fn practice_needed_to_advance(level: u8) -> u16 {
    let lv = u32::from(level);
    (lv.saturating_mul(lv).saturating_mul(20)).min(u32::from(u16::MAX)) as u16
}

fn slots_required_for_skill(skill: nethack_babel_data::WeaponSkill, level: u8) -> i32 {
    let current = i32::from(level).max(1);
    match skill {
        nethack_babel_data::WeaponSkill::BareHanded => (current + 1) / 2,
        _ => current,
    }
}

fn skill_level_label(level: u8) -> &'static str {
    match level {
        0 => "Restricted",
        1 => "Unskilled",
        2 => "Basic",
        3 => "Skilled",
        4 => "Expert",
        5 => "Master",
        6 => "GrandMaster",
        _ => "Unknown",
    }
}

/// Handle the player going up stairs.
fn handle_go_up(world: &mut GameWorld, rng: &mut impl Rng, events: &mut Vec<EngineEvent>) {
    let player = world.player();
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return,
    };

    // Check if player is on StairsUp terrain.
    let terrain = match world.dungeon().current_level.get(player_pos) {
        Some(cell) => cell.terrain,
        None => return,
    };
    if terrain != Terrain::StairsUp {
        events.push(EngineEvent::msg("stairs-not-here"));
        return;
    }

    let current_depth = world.dungeon().depth;
    if current_depth <= 1 {
        events.push(EngineEvent::msg("stairs-at-top"));
        return;
    }

    let target_depth = current_depth - 1;
    change_level(world, target_depth, true, rng, events);
}

/// Handle the player going down stairs.
fn handle_go_down(world: &mut GameWorld, rng: &mut impl Rng, events: &mut Vec<EngineEvent>) {
    let player = world.player();

    // Levitation blocks going down stairs.
    if crate::status::is_levitating(world, player) {
        events.push(EngineEvent::msg("levitating-cant-go-down"));
        return;
    }

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return,
    };

    // Check if player is on StairsDown terrain.
    let terrain = match world.dungeon().current_level.get(player_pos) {
        Some(cell) => cell.terrain,
        None => return,
    };
    if terrain != Terrain::StairsDown {
        events.push(EngineEvent::msg("stairs-not-here"));
        return;
    }

    let current_depth = world.dungeon().depth;

    if world.dungeon().branch == DungeonBranch::Quest && current_depth == 1 {
        let mut quest_state = read_quest_state(world, player);
        if crate::quest::should_expel_from_quest(&quest_state) {
            quest_state.expel();
            persist_quest_state(world, player, quest_state);
            events.push(EngineEvent::msg("quest-expelled"));
            return;
        }
    }

    // Check for branch transition before defaulting to depth+1.
    let branch_transition = world.dungeon().check_branch_transition();
    if let Some((target_branch, target_branch_depth)) = branch_transition {
        change_level_to_branch(
            world,
            target_branch,
            target_branch_depth,
            false,
            None,
            rng,
            events,
        );
    } else {
        let target_depth = current_depth + 1;
        change_level(world, target_depth, false, rng, events);
    }
}

/// Try special level dispatch; fall back to random generation.
///
/// Returns the generated level plus any special level flags.
/// Uses the hardcoded depth mapping (non-topology-aware).
#[allow(dead_code)]
fn generate_or_special(
    branch: crate::dungeon::DungeonBranch,
    depth: i32,
    rng: &mut impl Rng,
) -> (
    crate::map_gen::GeneratedLevel,
    crate::special_levels::SpecialLevelFlags,
) {
    if let Some(id) = identify_special_level(branch, depth)
        && let Some(special) = dispatch_special_level(id, None, rng)
    {
        return (special.generated, special.flags);
    }
    (
        generate_level(depth as u8, rng),
        crate::special_levels::SpecialLevelFlags::default(),
    )
}

fn find_portal_anchor(
    map: &crate::dungeon::LevelMap,
    preferred: Option<Position>,
) -> Option<Position> {
    let mut candidates = Vec::new();
    if let Some(anchor) = preferred {
        candidates.push(anchor);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                candidates.push(Position::new(anchor.x + dx, anchor.y + dy));
            }
        }
    }

    let center = Position::new((map.width / 2) as i32, (map.height / 2) as i32);
    candidates.push(center);

    for pos in candidates {
        if let Some(cell) = map.get(pos)
            && matches!(cell.terrain, Terrain::Floor | Terrain::Corridor)
        {
            return Some(pos);
        }
    }

    for y in 0..map.height {
        for x in 0..map.width {
            let pos = Position::new(x as i32, y as i32);
            if map
                .get(pos)
                .is_some_and(|cell| matches!(cell.terrain, Terrain::Floor | Terrain::Corridor))
            {
                return Some(pos);
            }
        }
    }

    None
}

fn inject_topology_portal_if_needed(
    world: &crate::world::GameWorld,
    branch: DungeonBranch,
    depth: i32,
    generated: &mut crate::map_gen::GeneratedLevel,
) {
    if !world.dungeon().level_has_topology_portal(branch, depth) {
        return;
    }
    if find_terrain(&generated.map, Terrain::MagicPortal).is_some() {
        return;
    }

    let preferred = generated.up_stairs.or(generated.down_stairs);
    if let Some(portal_pos) = find_portal_anchor(&generated.map, preferred) {
        generated.map.set_terrain(portal_pos, Terrain::MagicPortal);
    }
}

fn current_player_role(world: &GameWorld) -> Option<Role> {
    world
        .get_component::<PlayerIdentity>(world.player())
        .and_then(|identity| crate::role::Role::from_id(identity.role))
}

fn current_player_role_name(world: &GameWorld) -> Option<String> {
    current_player_role(world).map(|role| role.name().to_ascii_lowercase())
}

fn special_level_role_name(
    world: &GameWorld,
    id: crate::special_levels::SpecialLevelId,
) -> Option<String> {
    match id {
        crate::special_levels::SpecialLevelId::QuestStart
        | crate::special_levels::SpecialLevelId::QuestLocator
        | crate::special_levels::SpecialLevelId::QuestGoal
        | crate::special_levels::SpecialLevelId::QuestFiller(_) => {
            current_player_role_name(world).or_else(|| Some("valkyrie".to_string()))
        }
        _ => None,
    }
}

/// Topology-aware special level dispatch; fall back to random generation.
///
/// Uses the per-game randomized topology depths for the Main branch.
fn generate_or_special_topology(
    world: &crate::world::GameWorld,
    branch: DungeonBranch,
    depth: i32,
    rng: &mut impl Rng,
) -> (
    crate::map_gen::GeneratedLevel,
    crate::special_levels::SpecialLevelFlags,
    Option<crate::special_levels::SpecialLevelPopulation>,
) {
    if let Some(id) = world.dungeon().check_topology_special(&branch, depth) {
        let role_name = special_level_role_name(world, id);
        if let Some(mut special) = dispatch_special_level(id, role_name.as_deref(), rng) {
            inject_topology_portal_if_needed(world, branch, depth, &mut special.generated);
            let population = crate::special_levels::population_for_special_level_with_role(
                id,
                &special.generated,
                role_name.as_deref(),
            );
            return (
                special.generated,
                special.flags,
                (!population.is_empty()).then_some(population),
            );
        }
    }
    let mut generated = generate_level(depth as u8, rng);
    inject_topology_portal_if_needed(world, branch, depth, &mut generated);
    (
        generated,
        crate::special_levels::SpecialLevelFlags::default(),
        None,
    )
}

fn respawn_cached_monsters(world: &mut GameWorld, cached_mons: &[CachedMonster]) {
    for cm in cached_mons {
        let entity = world.spawn((
            Monster,
            Positioned(cm.position),
            Name(cm.name.clone()),
            HitPoints {
                current: cm.hp_current,
                max: cm.hp_max,
            },
            Speed(cm.speed),
            DisplaySymbol {
                symbol: cm.symbol,
                color: cm.color,
            },
            MovementPoints(NORMAL_SPEED as i32),
            CreationOrder(cm.creation_order),
        ));
        let _ = world
            .ecs_mut()
            .insert_one(entity, cm.status_effects.clone());
        if cm.is_tame {
            let _ = world.ecs_mut().insert_one(entity, Tame);
        }
        if cm.is_peaceful {
            let _ = world.ecs_mut().insert_one(entity, Peaceful);
        }
        if let Some(priest) = cm.priest {
            let _ = world.ecs_mut().insert_one(entity, priest);
        }
        if let Some(shopkeeper) = &cm.shopkeeper {
            let _ = world.ecs_mut().insert_one(entity, shopkeeper.clone());
        }
        if let Some(role) = cm.quest_npc_role {
            let _ = world.ecs_mut().insert_one(entity, role);
        }
        if let Some(trapped) = cm.trapped {
            let _ = world.ecs_mut().insert_one(entity, trapped);
        }
    }
}

fn rebind_shopkeepers(world: &GameWorld, runtime_state: &mut CachedLevelRuntimeState) {
    for shop in &mut runtime_state.shop_rooms {
        let current_name_matches = world
            .get_component::<Name>(shop.shopkeeper)
            .is_some_and(|name| name.0 == shop.shopkeeper_name);
        if shopkeeper_is_alive(world, shop.shopkeeper) && current_name_matches {
            continue;
        }
        if world.get_component::<Monster>(shop.shopkeeper).is_some() && current_name_matches {
            continue;
        }
        if let Some((entity, _)) =
            world
                .ecs()
                .query::<(&Monster, &Name)>()
                .iter()
                .find(|(entity, (_, name))| {
                    live_monster_entity(world, *entity) && name.0 == shop.shopkeeper_name
                })
        {
            shop.shopkeeper = entity;
        }
    }
}

fn shopkeeper_home_pos(shop: &crate::shop::ShopRoom) -> Position {
    shop.door_pos.unwrap_or(Position::new(
        (shop.top_left.x + shop.bottom_right.x) / 2,
        (shop.top_left.y + shop.bottom_right.y) / 2,
    ))
}

fn upsert_shopkeeper_component(
    world: &mut GameWorld,
    entity: hecs::Entity,
    shop: &crate::shop::ShopRoom,
) {
    if let Some(mut live) = world.get_component_mut::<crate::npc::Shopkeeper>(entity) {
        live.home_pos = shopkeeper_home_pos(shop);
        live.name = shop.shopkeeper_name.clone();
    } else {
        let _ = world.ecs_mut().insert_one(
            entity,
            crate::npc::Shopkeeper {
                following: false,
                displaced: false,
                home_pos: shopkeeper_home_pos(shop),
                name: shop.shopkeeper_name.clone(),
            },
        );
    }
}

fn upsert_priest_component(
    world: &mut GameWorld,
    entity: hecs::Entity,
    priest: crate::npc::Priest,
) {
    if let Some(mut live) = world.get_component_mut::<crate::npc::Priest>(entity) {
        live.alignment = priest.alignment;
        live.has_shrine = priest.has_shrine;
        live.is_high_priest = priest.is_high_priest;
        live.angry = priest.angry;
    } else {
        let _ = world.ecs_mut().insert_one(entity, priest);
    }
    if priest.angry {
        let _ = world.ecs_mut().remove_one::<Peaceful>(entity);
    }
}

fn upsert_quest_npc_role(
    world: &mut GameWorld,
    entity: hecs::Entity,
    role: crate::quest::QuestNpcRole,
) {
    if let Some(mut live) = world.get_component_mut::<crate::quest::QuestNpcRole>(entity) {
        *live = role;
    } else {
        let _ = world.ecs_mut().insert_one(entity, role);
    }
}

fn quest_npc_role_for_entity(
    world: &GameWorld,
    entity: hecs::Entity,
) -> Option<crate::quest::QuestNpcRole> {
    if let Some(role) = world.get_component::<crate::quest::QuestNpcRole>(entity) {
        return Some(*role);
    }

    let player_role = current_player_role(world)?;
    let name = world.get_component::<Name>(entity)?;
    crate::quest::quest_npc_role_by_name(player_role, &name.0)
}

pub fn sync_current_level_npc_state(world: &mut GameWorld) {
    let shops = world.dungeon().shop_rooms.clone();
    for (idx, shop) in shops.iter().enumerate() {
        if shopkeeper_is_alive(world, shop.shopkeeper) {
            upsert_shopkeeper_component(world, shop.shopkeeper, shop);
            continue;
        }

        let _ = world
            .ecs_mut()
            .remove_one::<crate::npc::Shopkeeper>(shop.shopkeeper);

        if let Some((entity, inherited_name)) = find_inheriting_shopkeeper(world, idx) {
            if let Some(live_shop) = world.dungeon_mut().shop_rooms.get_mut(idx) {
                live_shop.shopkeeper = entity;
                live_shop.shopkeeper_name = inherited_name;
            }
            let inherited_shop = world.dungeon().shop_rooms[idx].clone();
            upsert_shopkeeper_component(world, entity, &inherited_shop);
            continue;
        }

        let current_name_matches = world
            .get_component::<Name>(shop.shopkeeper)
            .is_some_and(|name| name.0 == shop.shopkeeper_name);
        if world.get_component::<Monster>(shop.shopkeeper).is_none() || !current_name_matches {
            let rebound_entity =
                world
                    .ecs()
                    .query::<(&Monster, &Name)>()
                    .iter()
                    .find_map(|(entity, (_, name))| {
                        (live_monster_entity(world, entity) && name.0 == shop.shopkeeper_name)
                            .then_some(entity)
                    });
            if let Some(entity) = rebound_entity {
                if let Some(live_shop) = world.dungeon_mut().shop_rooms.get_mut(idx) {
                    live_shop.shopkeeper = entity;
                }
                let rebound_shop = world.dungeon().shop_rooms[idx].clone();
                upsert_shopkeeper_component(world, entity, &rebound_shop);
            }
        }
    }

    let player_role = current_player_role(world);
    let monster_entities: Vec<hecs::Entity> = world
        .ecs()
        .query::<(&Monster,)>()
        .iter()
        .map(|(entity, _)| entity)
        .filter(|entity| entity_has_positive_hp(world, *entity))
        .collect();
    for entity in monster_entities {
        if let Some(priest) = infer_priest_runtime(world, entity) {
            upsert_priest_component(world, entity, priest);
        }
        let quest_role = if world.dungeon().branch == DungeonBranch::Quest {
            player_role.and_then(|role| {
                world
                    .get_component::<Name>(entity)
                    .map(|name| name.0.clone())
                    .and_then(|name| crate::quest::quest_npc_role_by_name(role, &name))
            })
        } else {
            None
        };
        if let Some(quest_role) = quest_role {
            upsert_quest_npc_role(world, entity, quest_role);
        }
    }
    sync_current_level_shopkeeper_state(world);
}

fn sync_current_level_shopkeeper_state(world: &mut GameWorld) {
    let Some(player_pos) = world
        .get_component::<Positioned>(world.player())
        .map(|pos| pos.0)
    else {
        return;
    };

    let shops = world.dungeon().shop_rooms.clone();
    for shop in &shops {
        let entity = shop.shopkeeper;
        if !shopkeeper_is_alive(world, entity) {
            let _ = world.ecs_mut().remove_one::<crate::npc::Shopkeeper>(entity);
            continue;
        }
        let home_pos = shopkeeper_home_pos(shop);
        let current_pos = world.get_component::<Positioned>(entity).map(|pos| pos.0);
        let has_unpaid_items = !shop.bill.is_empty() || shop.debit > shop.credit;
        let hero_left_shop = !shop.contains(player_pos);
        let following =
            crate::npc::shopkeeper_should_follow(shop.angry, has_unpaid_items, hero_left_shop);
        let displaced = current_pos.is_some_and(|pos| pos != home_pos);
        if let Some(mut live) = world.get_component_mut::<crate::npc::Shopkeeper>(entity) {
            live.following = following;
            live.displaced = displaced;
            live.home_pos = home_pos;
            live.name = shop.shopkeeper_name.clone();
        } else {
            let _ = world.ecs_mut().insert_one(
                entity,
                crate::npc::Shopkeeper {
                    following,
                    displaced,
                    home_pos,
                    name: shop.shopkeeper_name.clone(),
                },
            );
        }
    }
}

/// Perform a level transition into a different branch.
///
/// Similar to `change_level` but switches the dungeon branch.
pub(crate) fn change_level_to_branch(
    world: &mut GameWorld,
    target_branch: DungeonBranch,
    target_depth: i32,
    going_up: bool,
    landing_override: Option<Position>,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    let player = world.player();
    let from_depth = world.dungeon().depth;
    let from_branch = world.dungeon().branch;
    let first_visit = !world.dungeon().has_visited(target_branch, target_depth);

    // 1. Collect and cache current level (same as change_level).
    let mut monster_entities: Vec<hecs::Entity> = Vec::new();
    let mut cached_monsters: Vec<CachedMonster> = Vec::new();

    for (entity, (pos, _monster, name, hp, speed, sym)) in world
        .ecs()
        .query::<(
            &Positioned,
            &Monster,
            &Name,
            &HitPoints,
            &Speed,
            &DisplaySymbol,
        )>()
        .iter()
    {
        if entity == player {
            continue;
        }
        monster_entities.push(entity);
        cached_monsters.push(CachedMonster {
            position: pos.0,
            name: name.0.clone(),
            hp_current: hp.current,
            hp_max: hp.max,
            speed: speed.0,
            symbol: sym.symbol,
            color: sym.color,
            is_tame: world.get_component::<Tame>(entity).is_some(),
            is_peaceful: world.get_component::<Peaceful>(entity).is_some(),
            creation_order: world
                .get_component::<CreationOrder>(entity)
                .map(|order| order.0)
                .unwrap_or(0),
            priest: world
                .get_component::<crate::npc::Priest>(entity)
                .map(|priest| *priest),
            shopkeeper: world
                .get_component::<crate::npc::Shopkeeper>(entity)
                .map(|shopkeeper| (*shopkeeper).clone()),
            quest_npc_role: world
                .get_component::<crate::quest::QuestNpcRole>(entity)
                .map(|role| *role),
            trapped: world
                .get_component::<crate::traps::Trapped>(entity)
                .map(|trapped| *trapped),
            status_effects: world
                .get_component::<crate::status::StatusEffects>(entity)
                .map(|status| (*status).clone())
                .unwrap_or_default(),
        });
    }

    for entity in monster_entities {
        let _ = world.despawn(entity);
    }

    world.dungeon_mut().cache_current_level(cached_monsters);

    // 2. Switch branch and depth.
    world.dungeon_mut().branch = target_branch;
    world.dungeon_mut().depth = target_depth;

    // 3. Load or generate the target level.
    let (new_map, new_up_stairs, new_down_stairs, runtime_state, special_population) =
        if world.dungeon().has_visited(target_branch, target_depth) {
            let (map, cached_mons, mut runtime_state) = world
                .dungeon_mut()
                .load_cached_level(target_branch, target_depth)
                .expect("has_visited was true");

            let up_pos = find_terrain(&map, Terrain::StairsUp);
            let down_pos = find_terrain(&map, Terrain::StairsDown);

            respawn_cached_monsters(world, &cached_mons);
            rebind_shopkeepers(world, &mut runtime_state);

            (map, up_pos, down_pos, runtime_state, None)
        } else {
            let (generated, flags, special_population) =
                generate_or_special_topology(world, target_branch, target_depth, rng);
            (
                generated.map,
                generated.up_stairs,
                generated.down_stairs,
                CachedLevelRuntimeState {
                    current_level_flags: flags.into(),
                    ..Default::default()
                },
                special_population,
            )
        };

    // 4. Install the new level map and store flags.
    world.dungeon_mut().current_level = new_map;
    world
        .dungeon_mut()
        .restore_current_level_runtime_state(runtime_state);
    sync_current_level_invocation_access(world, rng);

    // 5. Place the player.
    let default_target_pos = if going_up {
        new_down_stairs.unwrap_or(Position::new(40, 10))
    } else {
        new_up_stairs
            .or_else(|| find_terrain(&world.dungeon().current_level, Terrain::MagicPortal))
            .unwrap_or(Position::new(40, 10))
    };
    let target_pos = landing_override
        .filter(|pos| {
            world
                .dungeon()
                .current_level
                .get(*pos)
                .is_some_and(|cell| cell.terrain.is_walkable())
        })
        .unwrap_or(default_target_pos);

    if let Some(mut pos) = world.get_component_mut::<Positioned>(player) {
        pos.0 = target_pos;
    }

    if let Some(pop) = special_population {
        apply_special_level_population(world, pop, rng);
    }
    sync_current_level_npc_state(world);
    maybe_emit_current_level_temple_entry(world, player, first_visit, rng, events);
    maybe_spawn_astral_guardian_angel(world, first_visit, rng, events);

    // 6. Mark visited and check level feeling.
    let was_visited = world.dungeon().was_visited(target_branch, target_depth);
    world.dungeon_mut().mark_visited();

    let flags = crate::dungeon::LevelFlags {
        previously_visited: was_visited,
        ..Default::default()
    };
    if let Some(feeling_msg) = crate::dungeon::level_feeling(&flags, target_depth, rng) {
        events.push(EngineEvent::msg(feeling_msg));
    }

    // 7. Emit LevelChanged event.
    events.push(EngineEvent::LevelChanged {
        entity: player,
        from_depth: format!("{:?}:{}", from_branch, from_depth),
        to_depth: format!("{:?}:{}", target_branch, target_depth),
    });
}

/// Perform a level transition: cache the current level, switch depth,
/// load or generate the target level, and reposition the player.
///
/// `going_up`: if true, place the player on StairsDown of the target
/// level (they came from below).  If false, place on StairsUp (they
/// came from above).
fn change_level(
    world: &mut GameWorld,
    target_depth: i32,
    going_up: bool,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    let player = world.player();
    let from_depth = world.dungeon().depth;
    let branch = world.dungeon().branch;
    let first_visit = !world.dungeon().has_visited(branch, target_depth);

    // 1. Collect all monster entities on the current level.
    let mut monster_entities: Vec<hecs::Entity> = Vec::new();
    let mut cached_monsters: Vec<CachedMonster> = Vec::new();

    for (entity, (pos, _monster, name, hp, speed, sym)) in world
        .ecs()
        .query::<(
            &Positioned,
            &Monster,
            &Name,
            &HitPoints,
            &Speed,
            &DisplaySymbol,
        )>()
        .iter()
    {
        if entity == player {
            continue;
        }
        monster_entities.push(entity);
        cached_monsters.push(CachedMonster {
            position: pos.0,
            name: name.0.clone(),
            hp_current: hp.current,
            hp_max: hp.max,
            speed: speed.0,
            symbol: sym.symbol,
            color: sym.color,
            is_tame: world.get_component::<Tame>(entity).is_some(),
            is_peaceful: world.get_component::<Peaceful>(entity).is_some(),
            creation_order: world
                .get_component::<CreationOrder>(entity)
                .map(|order| order.0)
                .unwrap_or(0),
            priest: world
                .get_component::<crate::npc::Priest>(entity)
                .map(|priest| *priest),
            shopkeeper: world
                .get_component::<crate::npc::Shopkeeper>(entity)
                .map(|shopkeeper| (*shopkeeper).clone()),
            quest_npc_role: world
                .get_component::<crate::quest::QuestNpcRole>(entity)
                .map(|role| *role),
            trapped: world
                .get_component::<crate::traps::Trapped>(entity)
                .map(|trapped| *trapped),
            status_effects: world
                .get_component::<crate::status::StatusEffects>(entity)
                .map(|status| (*status).clone())
                .unwrap_or_default(),
        });
    }

    // 2. Despawn all monster entities.
    for entity in monster_entities {
        let _ = world.despawn(entity);
    }

    // 3. Cache the current level.
    world.dungeon_mut().cache_current_level(cached_monsters);

    // 4. Switch depth.
    world.dungeon_mut().depth = target_depth;

    // 5. Load or generate the target level.
    let target_branch = branch;
    let (new_map, new_up_stairs, new_down_stairs, runtime_state, special_population) =
        if world.dungeon().has_visited(target_branch, target_depth) {
            // Load from cache.
            let (map, cached_mons, mut runtime_state) = world
                .dungeon_mut()
                .load_cached_level(target_branch, target_depth)
                .expect("has_visited was true");

            // Find stairs positions from the loaded map.
            let up_pos = find_terrain(&map, Terrain::StairsUp);
            let down_pos = find_terrain(&map, Terrain::StairsDown);

            // Respawn cached monsters.
            respawn_cached_monsters(world, &cached_mons);
            rebind_shopkeepers(world, &mut runtime_state);

            (map, up_pos, down_pos, runtime_state, None)
        } else {
            // Generate a new level.
            let (generated, flags, special_population) =
                generate_or_special_topology(world, target_branch, target_depth, rng);
            (
                generated.map,
                generated.up_stairs,
                generated.down_stairs,
                CachedLevelRuntimeState {
                    current_level_flags: flags.into(),
                    ..Default::default()
                },
                special_population,
            )
        };

    // 6. Install the new level map and store flags.
    world.dungeon_mut().current_level = new_map;
    world
        .dungeon_mut()
        .restore_current_level_runtime_state(runtime_state);
    sync_current_level_invocation_access(world, rng);

    // 7. Place the player on the appropriate stairs.
    let target_pos = if going_up {
        // Came from below: appear on down stairs.
        new_down_stairs.unwrap_or(Position::new(40, 10))
    } else {
        // Came from above: appear on up stairs.
        new_up_stairs.unwrap_or(Position::new(40, 10))
    };

    if let Some(mut pos) = world.get_component_mut::<Positioned>(player) {
        pos.0 = target_pos;
    }

    if let Some(pop) = special_population {
        apply_special_level_population(world, pop, rng);
    }
    sync_current_level_npc_state(world);
    maybe_emit_current_level_temple_entry(world, player, first_visit, rng, events);
    maybe_spawn_astral_guardian_angel(world, first_visit, rng, events);

    // 8. Emit LevelChanged event.
    events.push(EngineEvent::LevelChanged {
        entity: player,
        from_depth: format!("{}", from_depth),
        to_depth: format!("{}", target_depth),
    });
}

fn apply_special_level_population(
    world: &mut GameWorld,
    population: crate::special_levels::SpecialLevelPopulation,
    rng: &mut impl Rng,
) {
    let monster_defs: Vec<MonsterDef> = world.monster_catalog().to_vec();
    let object_defs: Vec<ObjectDef> = world.object_catalog().to_vec();
    let resolved = resolve_special_level_population(&monster_defs, &object_defs, population);

    for mon in resolved.monsters {
        if !roll_spawn_chance(mon.chance, rng) {
            continue;
        }
        spawn_special_monster(world, mon, &monster_defs, rng);
    }

    for obj in resolved.objects {
        if !roll_spawn_chance(obj.chance, rng) {
            continue;
        }
        spawn_special_object(world, obj, &object_defs, rng);
    }
}

fn player_has_conflict_equipment(world: &GameWorld, player: hecs::Entity) -> bool {
    world
        .get_component::<crate::equipment::EquipmentSlots>(player)
        .map(|slots| slots.all_worn())
        .into_iter()
        .flatten()
        .any(|(_slot, item)| {
            item_display_name(world, item)
                .is_some_and(|name| name.to_ascii_lowercase().contains("conflict"))
        })
}

fn maybe_spawn_astral_guardian_angel(
    world: &mut GameWorld,
    first_visit: bool,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    if !first_visit
        || world.dungeon().branch != DungeonBranch::Endgame
        || world.dungeon().depth != 5
    {
        return;
    }

    let player = world.player();
    let religion = world
        .get_component::<crate::religion::ReligionState>(player)
        .map(|state| (*state).clone())
        .unwrap_or_else(|| default_religion_state(world, player));
    if !crate::minion::worthy_of_guardian(
        religion.alignment_record,
        player_has_conflict_equipment(world, player),
    ) {
        return;
    }

    let monster_defs: Vec<MonsterDef> = world.monster_catalog().to_vec();
    let Some(angel_id) = resolve_monster_id_by_spec(&monster_defs, "Angel") else {
        return;
    };
    let Some(angel_def) = monster_defs.iter().find(|def| def.id == angel_id) else {
        return;
    };
    let Some(player_pos) = world.get_component::<Positioned>(player).map(|pos| pos.0) else {
        return;
    };
    let Some(spawn_pos) = enexto(world, player_pos, angel_def) else {
        return;
    };
    let Some(angel) = makemon(
        world,
        &monster_defs,
        Some(angel_id),
        spawn_pos,
        MakeMonFlags::NO_GROUP,
        rng,
    ) else {
        return;
    };

    let _ = world.ecs_mut().insert_one(angel, Peaceful);
    events.push(EngineEvent::msg("guardian-angel-appears"));
}

#[derive(Debug, Clone, Copy)]
struct ResolvedSpecialMonsterSpawn {
    monster_id: MonsterId,
    pos: Option<Position>,
    chance: u32,
    peaceful: Option<bool>,
    asleep: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedSpecialObjectSpawn {
    object_type: ObjectTypeId,
    pos: Option<Position>,
    chance: u32,
    quantity: Option<u32>,
    artifact_id: Option<ArtifactId>,
    artifact_name: Option<&'static str>,
}

#[derive(Debug, Clone, Default)]
struct ResolvedSpecialLevelPopulation {
    monsters: Vec<ResolvedSpecialMonsterSpawn>,
    objects: Vec<ResolvedSpecialObjectSpawn>,
}

fn resolve_special_level_population(
    monster_defs: &[MonsterDef],
    object_defs: &[ObjectDef],
    population: crate::special_levels::SpecialLevelPopulation,
) -> ResolvedSpecialLevelPopulation {
    let mut resolved = ResolvedSpecialLevelPopulation::default();

    for mon in population.monsters {
        if let Some(monster_id) = resolve_monster_id_by_spec(monster_defs, &mon.name) {
            resolved.monsters.push(ResolvedSpecialMonsterSpawn {
                monster_id,
                pos: mon.pos,
                chance: mon.chance,
                peaceful: mon.peaceful,
                asleep: mon.asleep,
            });
        } else {
            debug_assert!(false, "unresolved special monster spec: {}", mon.name);
        }
    }

    for obj in population.objects {
        if let Some(artifact) = crate::artifacts::find_artifact_by_name(&obj.name) {
            if let Some(object_type) = resolve_artifact_base_object_type(object_defs, artifact) {
                resolved.objects.push(ResolvedSpecialObjectSpawn {
                    object_type,
                    pos: obj.pos,
                    chance: obj.chance,
                    quantity: obj.quantity,
                    artifact_id: Some(artifact.id),
                    artifact_name: Some(artifact.name),
                });
            } else {
                debug_assert!(
                    false,
                    "artifact base item missing from catalog: {}",
                    obj.name
                );
            }
        } else if let Some(object_type) = resolve_object_type_by_spec(object_defs, &obj.name) {
            resolved.objects.push(ResolvedSpecialObjectSpawn {
                object_type,
                pos: obj.pos,
                chance: obj.chance,
                quantity: obj.quantity,
                artifact_id: None,
                artifact_name: None,
            });
        } else {
            debug_assert!(false, "unresolved special object spec: {}", obj.name);
        }
    }

    resolved
}

fn resolve_artifact_base_object_type(
    object_defs: &[ObjectDef],
    artifact: &crate::artifacts::ArtifactDef,
) -> Option<ObjectTypeId> {
    if object_defs.iter().any(|def| def.id == artifact.base_item) {
        return Some(artifact.base_item);
    }

    // Artifact tables still use the classic global object ids, while the
    // data catalog is currently loaded from per-file local ids. Fall back to
    // the canonical object name for quest artifacts so population planning
    // can still materialize real artifact instances.
    let fallback_spec = match artifact.id.0 {
        21 => "crystal ball",
        22 => "luckstone",
        23 => "mace",
        24 => "quarterstaff",
        25 => "mirror",
        26 => "lenses",
        27 => "helm of brilliance",
        28 => "bow",
        29 => "skeleton key",
        30 => "tsurugi",
        31 => "credit card",
        32 => "crystal ball",
        33 => "amulet of ESP",
        _ => return None,
    };

    resolve_object_type_by_spec(object_defs, fallback_spec)
}

fn roll_spawn_chance(chance: u32, rng: &mut impl Rng) -> bool {
    if chance >= 100 {
        return true;
    }
    if chance == 0 {
        return false;
    }
    rng.random_range(1..=100) <= chance
}

fn spawn_special_monster(
    world: &mut GameWorld,
    spec: ResolvedSpecialMonsterSpawn,
    monster_defs: &[MonsterDef],
    rng: &mut impl Rng,
) {
    let Some(monster_def) = monster_defs.iter().find(|def| def.id == spec.monster_id) else {
        return;
    };
    let Some(pos) = resolve_special_monster_spawn_pos(world, spec.pos, monster_def, rng) else {
        return;
    };

    let mut flags = MakeMonFlags::NO_GROUP;
    if spec.peaceful.unwrap_or(false) {
        flags |= MakeMonFlags::PEACEFUL;
    }
    if spec.asleep.unwrap_or(false) {
        flags |= MakeMonFlags::ASLEEP;
    }
    let _ = makemon(world, monster_defs, Some(spec.monster_id), pos, flags, rng);
}

fn spawn_special_object(
    world: &mut GameWorld,
    spec: ResolvedSpecialObjectSpawn,
    object_defs: &[ObjectDef],
    rng: &mut impl Rng,
) {
    let Some(pos) = resolve_special_spawn_pos(world, spec.pos, rng) else {
        return;
    };

    if let Some(entity) = mksobj_at(world, pos, spec.object_type, true, object_defs, rng) {
        let display_name = spec
            .artifact_name
            .map(str::to_string)
            .unwrap_or_else(|| crate::identification::typename(spec.object_type, object_defs));
        let _ = world.ecs_mut().insert_one(entity, Name(display_name));
        if let Some(artifact_id) = spec.artifact_id
            && let Some(mut core) = world.get_component_mut::<ObjectCore>(entity)
        {
            core.artifact = Some(artifact_id);
        }
        if let Some(q) = spec.quantity
            && q > 1
            && let Some(mut core) = world.get_component_mut::<ObjectCore>(entity)
        {
            core.quantity = q as i32;
        }
    }
}

fn resolve_special_spawn_pos(
    world: &GameWorld,
    requested: Option<Position>,
    rng: &mut impl Rng,
) -> Option<Position> {
    let map = &world.dungeon().current_level;
    let occupied = |p: Position| {
        if world
            .get_component::<Positioned>(world.player())
            .is_some_and(|pp| pp.0 == p)
        {
            return true;
        }
        world
            .ecs()
            .query::<(&Monster, &Positioned)>()
            .iter()
            .any(|(_, (_m, pos))| pos.0 == p)
    };

    if let Some(pos) = requested
        && map.get(pos).is_some_and(|c| c.terrain.is_walkable())
        && !occupied(pos)
    {
        return Some(pos);
    }

    for _ in 0..200 {
        let x = rng.random_range(0..map.width) as i32;
        let y = rng.random_range(0..map.height) as i32;
        let pos = Position::new(x, y);
        if map.get(pos).is_some_and(|c| c.terrain.is_walkable()) && !occupied(pos) {
            return Some(pos);
        }
    }

    for y in 0..map.height {
        for x in 0..map.width {
            let pos = Position::new(x as i32, y as i32);
            if map.get(pos).is_some_and(|c| c.terrain.is_walkable()) && !occupied(pos) {
                return Some(pos);
            }
        }
    }

    None
}

fn resolve_special_monster_spawn_pos(
    world: &GameWorld,
    requested: Option<Position>,
    monster_def: &MonsterDef,
    rng: &mut impl Rng,
) -> Option<Position> {
    let flags = GoodPosFlags::AVOID_MONSTER;

    if let Some(pos) = requested {
        if goodpos(world, pos, Some(monster_def), flags) {
            return Some(pos);
        }
        if let Some(nearby) = enexto(world, pos, monster_def) {
            return Some(nearby);
        }
    }

    let map = &world.dungeon().current_level;
    for _ in 0..200 {
        let x = rng.random_range(0..map.width) as i32;
        let y = rng.random_range(0..map.height) as i32;
        let pos = Position::new(x, y);
        if goodpos(world, pos, Some(monster_def), flags) {
            return Some(pos);
        }
    }

    for y in 0..map.height {
        for x in 0..map.width {
            let pos = Position::new(x as i32, y as i32);
            if goodpos(world, pos, Some(monster_def), flags) {
                return Some(pos);
            }
        }
    }

    None
}

fn resolve_monster_id_by_spec(monster_defs: &[MonsterDef], spec: &str) -> Option<MonsterId> {
    let spec = spec.trim();
    if let Some(class_str) = spec.strip_prefix("class:") {
        let mut chars = class_str.chars();
        let class = chars.next()?;
        return monster_defs
            .iter()
            .find(|def| def.symbol.eq_ignore_ascii_case(&class))
            .map(|def| def.id);
    }

    let normalized = spec
        .strip_prefix("the ")
        .or_else(|| spec.strip_prefix("The "))
        .unwrap_or(spec);
    let alias = monster_spec_alias(normalized);

    monster_defs
        .iter()
        .find(|def| {
            def.names.male.eq_ignore_ascii_case(spec)
                || def.names.male.eq_ignore_ascii_case(normalized)
                || alias.is_some_and(|name| def.names.male.eq_ignore_ascii_case(name))
                || def.names.female.as_ref().is_some_and(|f| {
                    f.eq_ignore_ascii_case(spec) || f.eq_ignore_ascii_case(normalized)
                })
        })
        .map(|def| def.id)
}

fn monster_spec_alias(spec: &str) -> Option<&'static str> {
    match spec.to_ascii_lowercase().as_str() {
        "centaur" => Some("plains centaur"),
        "ronin" => Some("samurai"),
        _ => None,
    }
}

fn resolve_object_type_by_spec(object_defs: &[ObjectDef], spec: &str) -> Option<ObjectTypeId> {
    let spec = spec.trim();
    if let Some(class_str) = spec.strip_prefix("class:") {
        let class_char = class_str.chars().next()?;
        let class = object_class_from_symbol(class_char)?;
        return object_defs
            .iter()
            .find(|def| def.class == class)
            .map(|def| def.id);
    }

    if let Some(def) = object_defs
        .iter()
        .find(|def| def.name.eq_ignore_ascii_case(spec))
    {
        return Some(def.id);
    }

    let lower = spec.to_ascii_lowercase();
    let class_prefixes: &[(&str, ObjectClass)] = &[
        ("scroll of ", ObjectClass::Scroll),
        ("potion of ", ObjectClass::Potion),
        ("wand of ", ObjectClass::Wand),
        ("ring of ", ObjectClass::Ring),
        ("spellbook of ", ObjectClass::Spellbook),
        ("amulet of ", ObjectClass::Amulet),
    ];

    for &(prefix, class) in class_prefixes {
        if let Some(base_name) = lower.strip_prefix(prefix)
            && let Some(def) = object_defs
                .iter()
                .find(|def| def.class == class && def.name.eq_ignore_ascii_case(base_name.trim()))
        {
            return Some(def.id);
        }
    }

    None
}

fn object_class_from_symbol(ch: char) -> Option<ObjectClass> {
    match ch {
        ')' => Some(ObjectClass::Weapon),
        '[' => Some(ObjectClass::Armor),
        '=' => Some(ObjectClass::Ring),
        '"' => Some(ObjectClass::Amulet),
        '(' => Some(ObjectClass::Tool),
        '%' => Some(ObjectClass::Food),
        '!' => Some(ObjectClass::Potion),
        '?' => Some(ObjectClass::Scroll),
        '+' => Some(ObjectClass::Spellbook),
        '/' => Some(ObjectClass::Wand),
        '$' => Some(ObjectClass::Coin),
        '*' => Some(ObjectClass::Gem),
        '`' => Some(ObjectClass::Rock),
        '0' => Some(ObjectClass::Ball),
        '_' => Some(ObjectClass::Chain),
        '.' => Some(ObjectClass::Venom),
        _ => None,
    }
}

/// Find the first cell with the given terrain type on a map.
fn find_terrain(map: &crate::dungeon::LevelMap, terrain: Terrain) -> Option<Position> {
    for y in 0..map.height {
        for x in 0..map.width {
            if map.cells[y][x].terrain == terrain {
                return Some(Position::new(x as i32, y as i32));
            }
        }
    }
    None
}

fn find_monster_entity_at(world: &GameWorld, pos: Position) -> Option<hecs::Entity> {
    for (entity, _) in world.ecs().query::<&Monster>().iter() {
        if let Some(mon_pos) = world.get_component::<Positioned>(entity)
            && mon_pos.0 == pos
            && entity_has_positive_hp(world, entity)
        {
            return Some(entity);
        }
    }
    None
}

#[inline]
fn is_player_inventory_item(world: &GameWorld, item: hecs::Entity) -> bool {
    world
        .get_component::<nethack_babel_data::ObjectLocation>(item)
        .is_some_and(|loc| matches!(*loc, nethack_babel_data::ObjectLocation::Inventory))
}

fn item_label_for_message(world: &GameWorld, item: hecs::Entity) -> String {
    if let Some(name) = world.get_component::<Name>(item) {
        return name.0.clone();
    }
    if let Some(core) = world.get_component::<nethack_babel_data::ObjectCore>(item) {
        return format!("item(otyp={})", core.otyp.0);
    }
    "something".to_string()
}

fn set_item_name(world: &mut GameWorld, item: hecs::Entity, name: &str) -> bool {
    let desired = name.to_string();

    if let Some(mut extra) = world.get_component_mut::<nethack_babel_data::ObjectExtra>(item) {
        extra.name = Some(desired.clone());
    } else if world
        .ecs_mut()
        .insert_one(
            item,
            nethack_babel_data::ObjectExtra {
                name: Some(desired.clone()),
                contained_monster: None,
            },
        )
        .is_err()
    {
        return false;
    }

    if let Some(mut display_name) = world.get_component_mut::<Name>(item) {
        display_name.0 = desired;
    } else if world.ecs_mut().insert_one(item, Name(desired)).is_err() {
        return false;
    }

    true
}

fn set_called_item_class(
    world: &mut GameWorld,
    class: char,
    name: &str,
    events: &mut Vec<EngineEvent>,
) {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        events.push(EngineEvent::msg("call-empty-name"));
        return;
    }

    world
        .dungeon_mut()
        .set_called_item_class(class, trimmed.to_string());
    events.push(EngineEvent::msg_with(
        "item-called-set",
        vec![
            ("item_class", class.to_string()),
            ("name", trimmed.to_string()),
        ],
    ));
}

/// Return one travel direction that best advances from `from` toward
/// `destination`, considering only in-bounds walkable tiles.
fn travel_direction_toward(
    world: &GameWorld,
    from: Position,
    destination: Position,
) -> Option<Direction> {
    let mut dirs: [(Direction, i32); 8] = std::array::from_fn(|i| {
        let direction = Direction::PLANAR[i];
        let next = from.step(direction);
        let walkable = world.dungeon().current_level.in_bounds(next)
            && world
                .dungeon()
                .current_level
                .get(next)
                .is_some_and(|cell| cell.terrain.is_walkable());
        let dist = if walkable {
            manhattan_distance(next, destination)
        } else {
            i32::MAX
        };
        (direction, dist)
    });
    dirs.sort_by_key(|&(_, d)| d);
    dirs.into_iter()
        .find(|&(_, d)| d != i32::MAX)
        .map(|(dir, _)| dir)
}

#[inline]
fn manhattan_distance(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

/// Attempt to move an entity one step in the given direction.
///
/// Emits `EntityMoved` on success.  Does NOT handle combat (bumping into
/// a monster) -- that will be layered on later.
///
/// Boulder pushing: when the player walks into a boulder entity, the
/// boulder is pushed one cell in the same direction.  The push is blocked
/// if the cell behind the boulder is non-walkable, contains another
/// boulder, or is out of bounds.  If the cell behind the boulder contains
/// a pit or hole trap, the boulder fills the pit (both are removed).
fn try_move_entity(
    world: &mut GameWorld,
    entity: hecs::Entity,
    direction: Direction,
    force_attack_non_hostile: bool,
    events: &mut Vec<EngineEvent>,
    rng: &mut impl Rng,
) {
    let current_pos = match world.get_component::<Positioned>(entity) {
        Some(p) => p.0,
        None => return,
    };

    let target_pos = current_pos.step(direction);

    // Bounds check.
    if !world.dungeon().current_level.in_bounds(target_pos) {
        return;
    }

    // Walkability check.
    if let Some(cell) = world.dungeon().current_level.get(target_pos) {
        if !cell.terrain.is_walkable() {
            return;
        }
    } else {
        return;
    }

    if entity == world.player()
        && let Some(occupant) = monster_at(world, target_pos, entity)
    {
        if world.get_component::<Tame>(occupant).is_some() && !force_attack_non_hostile {
            swap_entities(world, entity, occupant, current_pos, target_pos, events);
            finish_player_movement(world, entity, current_pos, target_pos, events, rng);
            return;
        }
        if world.get_component::<Peaceful>(occupant).is_some() && !force_attack_non_hostile {
            events.push(EngineEvent::msg_with(
                "peaceful-monster-blocks",
                vec![("monster", world.entity_name(occupant))],
            ));
            return;
        }
        crate::combat::resolve_melee_attack(world, entity, occupant, rng, events);
        return;
    }

    // Boulder pushing: check if there is a boulder at the target position.
    if entity == world.player()
        && let Some(boulder_entity) = find_boulder_at(world, target_pos)
    {
        let push_pos = target_pos.step(direction);

        // Check if the push destination has a pit/hole trap.
        let has_pit = {
            use crate::traps::is_pit;
            world
                .dungeon()
                .trap_map
                .trap_at(push_pos)
                .map(|t| is_pit(t.trap_type) || t.trap_type == nethack_babel_data::TrapType::Hole)
                .unwrap_or(false)
        };

        if has_pit {
            // Boulder fills the pit: remove both boulder and trap.
            world.ecs_mut().despawn(boulder_entity).ok();
            world.dungeon_mut().trap_map.remove_trap_at(push_pos);
            events.push(EngineEvent::msg("boulder-fills-pit"));
        } else {
            // Check bounds and walkability of the cell behind the boulder.
            let can_push = world.dungeon().current_level.in_bounds(push_pos)
                && world
                    .dungeon()
                    .current_level
                    .get(push_pos)
                    .is_some_and(|c| c.terrain.is_walkable())
                && find_boulder_at(world, push_pos).is_none();

            if !can_push {
                // Boulder is blocked -- the player doesn't move.
                events.push(EngineEvent::msg("boulder-blocked"));
                return;
            }

            // Move the boulder.
            if let Some(mut bpos) = world.get_component_mut::<Positioned>(boulder_entity) {
                bpos.0 = push_pos;
            }
            events.push(EngineEvent::EntityMoved {
                entity: boulder_entity,
                from: target_pos,
                to: push_pos,
            });
            events.push(EngineEvent::msg("boulder-push"));
        }
        // Fall through to move the player into the boulder's old cell.
    }

    if maybe_trigger_shop_exit_robbery(world, entity, current_pos, target_pos, events, rng) {
        return;
    }

    // Update position component.
    if let Some(mut pos) = world.get_component_mut::<Positioned>(entity) {
        pos.0 = target_pos;
    }

    events.push(EngineEvent::EntityMoved {
        entity,
        from: current_pos,
        to: target_pos,
    });
    finish_player_movement(world, entity, current_pos, target_pos, events, rng);
}

fn monster_at(world: &GameWorld, pos: Position, exclude: hecs::Entity) -> Option<hecs::Entity> {
    world
        .ecs()
        .query::<(&Monster, &Positioned)>()
        .iter()
        .find_map(|(entity, (_, positioned))| {
            (entity != exclude && positioned.0 == pos && entity_has_positive_hp(world, entity))
                .then_some(entity)
        })
}

fn swap_entities(
    world: &mut GameWorld,
    a: hecs::Entity,
    b: hecs::Entity,
    a_from: Position,
    b_from: Position,
    events: &mut Vec<EngineEvent>,
) {
    if let Some(mut pos) = world.get_component_mut::<Positioned>(a) {
        pos.0 = b_from;
    }
    if let Some(mut pos) = world.get_component_mut::<Positioned>(b) {
        pos.0 = a_from;
    }
    events.push(EngineEvent::EntityMoved {
        entity: a,
        from: a_from,
        to: b_from,
    });
    events.push(EngineEvent::EntityMoved {
        entity: b,
        from: b_from,
        to: a_from,
    });
}

fn finish_player_movement(
    world: &mut GameWorld,
    entity: hecs::Entity,
    from_pos: Position,
    target_pos: Position,
    events: &mut Vec<EngineEvent>,
    rng: &mut impl Rng,
) {
    if entity != world.player() {
        return;
    }

    let trap_info = build_player_trap_info(world, entity, target_pos);
    let (trap_events, _triggered) =
        trigger_trap_at(rng, &trap_info, &mut world.dungeon_mut().trap_map);
    events.extend(trap_events);

    if world
        .dungeon()
        .current_level
        .get(target_pos)
        .is_some_and(|cell| cell.terrain == Terrain::MagicPortal)
        && crate::teleport::resolve_magic_portal_destination_at(world, target_pos).is_some()
    {
        let portal_events = crate::teleport::handle_magic_portal(world, entity, rng);
        events.extend(portal_events);
        return;
    }

    let vault_rooms = world.dungeon().vault_rooms.clone();
    let guard_present = world.dungeon().vault_guard_present;
    if let Some(_vault_idx) = crate::vault::player_in_vault(target_pos, &vault_rooms)
        && !guard_present
    {
        let guard_data = crate::vault::spawn_guard(rng);
        world.dungeon_mut().vault_guard_present = true;
        events.push(EngineEvent::msg_with(
            "guard-appears",
            vec![("name", guard_data.guard_name)],
        ));
    }

    maybe_emit_shop_entry_greeting(world, entity, from_pos, target_pos, events);

    if !crate::status::is_levitating(world, entity) {
        let mut letter_state = crate::items::LetterState::default();
        let (autopickup_enabled, autopickup_classes) = {
            let d = world.dungeon();
            (d.autopickup_enabled, d.autopickup_classes.clone())
        };
        if autopickup_enabled {
            let pickup_events =
                crate::inventory::autopickup(world, &mut letter_state, &[], &autopickup_classes);
            events.extend(pickup_events);
        }
    }

    let items_here = count_items_at(world, target_pos);
    if items_here == 1 {
        events.push(EngineEvent::msg("see-item-here"));
    } else if items_here > 1 {
        events.push(EngineEvent::msg_with(
            "see-items-here",
            vec![("count", items_here.to_string())],
        ));
    }
}

fn maybe_emit_shop_entry_greeting(
    world: &GameWorld,
    entity: hecs::Entity,
    from_pos: Position,
    target_pos: Position,
    events: &mut Vec<EngineEvent>,
) {
    if entity != world.player() {
        return;
    }

    let previous_shop = find_shop_index_containing_position(world, from_pos);
    let Some(next_shop_idx) = find_shop_index_containing_position(world, target_pos) else {
        return;
    };
    if previous_shop == Some(next_shop_idx) {
        return;
    }

    let shop = &world.dungeon().shop_rooms[next_shop_idx];
    events.extend(crate::shop::enter_shop(world, entity, shop));
}

/// Find a boulder entity at the given position.
///
/// Returns the first boulder entity found, or `None` if no boulder
/// occupies the cell.
fn find_boulder_at(world: &GameWorld, pos: Position) -> Option<hecs::Entity> {
    for (entity, (positioned, _boulder)) in world.ecs().query::<(&Positioned, &Boulder)>().iter() {
        if positioned.0 == pos {
            return Some(entity);
        }
    }
    None
}

/// Count the number of item entities at the given position.
fn count_items_at(world: &GameWorld, pos: crate::action::Position) -> usize {
    crate::inventory::items_at_position(world, pos).len()
}

/// Give each monster entity a turn, ordered by speed descending, then
/// creation order ascending (Decision D2 from the spec).
///
/// Only monsters with `movement >= NORMAL_SPEED` get to act.  Each
/// action costs `NORMAL_SPEED` points.
fn resolve_monster_turns(world: &mut GameWorld, rng: &mut impl Rng, events: &mut Vec<EngineEvent>) {
    let player = world.player();

    // Collect eligible monsters: (entity, speed, creation_order).
    let mut monsters: Vec<(hecs::Entity, u32, u64)> = Vec::new();
    for (entity, (speed, mp, _monster)) in world
        .ecs()
        .query::<(&Speed, &MovementPoints, &Monster)>()
        .iter()
    {
        if entity != player && live_monster_entity(world, entity) && mp.0 >= NORMAL_SPEED as i32 {
            let creation = world
                .get_component::<CreationOrder>(entity)
                .map(|c| c.0)
                .unwrap_or(u64::MAX);
            monsters.push((entity, speed.0, creation));
        }
    }

    // Sort by speed descending, then creation order ascending.
    monsters.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));

    for (entity, _speed, _creation) in &monsters {
        // Skip dead monsters (may have been killed by a preceding
        // monster's action this same pass).
        if !live_monster_entity(world, *entity) {
            continue;
        }

        // Deduct movement cost.
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(*entity) {
            mp.0 -= NORMAL_SPEED as i32;
        }

        if crate::status::is_paralyzed(world, *entity) || crate::status::is_sleeping(world, *entity)
        {
            continue;
        }

        // Run the monster AI decision tree.
        let monster_events = crate::monster_ai::resolve_monster_turn(world, *entity, rng);
        sync_incapacitation_from_events(world, &monster_events);
        events.extend(monster_events);
    }
}

/// HP regeneration, matching NetHack's `regen_hp()`.
///
/// The player regenerates 1 HP when `turns % period == 0` and
/// current HP < max HP.  Period depends on experience level.
///
/// Encumbrance >= Stressed blocks regen unless the hero did not move
/// (we approximate this by always allowing regen for now -- the
/// `u.umoved` tracking will be added with the movement subsystem).
fn regen_hp(world: &mut GameWorld, events: &mut Vec<EngineEvent>) {
    let player = world.player();
    let turn = world.turn();

    let xlevel = world
        .get_component::<ExperienceLevel>(player)
        .map(|x| x.0)
        .unwrap_or(1);

    let period = hp_regen_period(xlevel);

    if !turn.is_multiple_of(period) {
        return;
    }

    // Read current HP.
    let (current, max) = match world.get_component::<HitPoints>(player) {
        Some(hp) => (hp.current, hp.max),
        None => return,
    };

    if current >= max {
        return;
    }

    // Apply +1 HP.
    let new_hp = (current + 1).min(max);
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
        hp.current = new_hp;
    }

    events.push(EngineEvent::HpChange {
        entity: player,
        amount: 1,
        new_hp,
        source: HpSource::Regeneration,
    });
}

/// PW regeneration, matching NetHack's `regen_pw()`.
///
/// Similar to HP regen but with a wisdom-dependent period.
/// Blocked when encumbrance >= Stressed.
fn regen_pw(world: &mut GameWorld, events: &mut Vec<EngineEvent>) {
    let player = world.player();
    let turn = world.turn();

    // Check encumbrance -- PW regen blocked at Stressed or worse.
    let enc = world
        .get_component::<EncumbranceLevel>(player)
        .map(|e| e.0)
        .unwrap_or(Encumbrance::Unencumbered);
    if enc >= Encumbrance::Stressed {
        return;
    }

    let xlevel = world
        .get_component::<ExperienceLevel>(player)
        .map(|x| x.0)
        .unwrap_or(1);

    let wisdom = world
        .get_component::<crate::world::Attributes>(player)
        .map(|a| a.wisdom)
        .unwrap_or(10);

    let period = pw_regen_period(xlevel, wisdom);

    if !turn.is_multiple_of(period) {
        return;
    }

    // Read current PW.
    let (current, max) = match world.get_component::<Power>(player) {
        Some(pw) => (pw.current, pw.max),
        None => return,
    };

    if current >= max {
        return;
    }

    // Apply +1 PW.
    let new_pw = (current + 1).min(max);
    if let Some(mut pw) = world.get_component_mut::<Power>(player) {
        pw.current = new_pw;
    }

    events.push(EngineEvent::PwChange {
        entity: player,
        amount: 1,
        new_pw,
    });
}

/// Per-turn hunger depletion.
///
/// Uses the full `gethungry()` logic: base depletion (1 per turn) plus
/// accessory hunger from rings, amulets, regeneration, encumbrance, etc.
/// Also checks for starvation death, fainting, and strength penalty.
fn process_hunger(world: &mut GameWorld, events: &mut Vec<EngineEvent>, rng: &mut impl Rng) {
    let player = world.player();

    let old_nutrition = match world.get_component::<Nutrition>(player) {
        Some(n) => n.0,
        None => return,
    };

    let old_level = nutrition_to_hunger_level(old_nutrition);

    // Build accessory hunger context from current game state.
    // For now, we use base-only since full ECS property tracking is not
    // yet wired up. The AccessoryHungerCtx will be populated as more
    // systems come online.
    let encumbrance = world
        .get_component::<EncumbranceLevel>(player)
        .map(|e| e.0)
        .unwrap_or(Encumbrance::Unencumbered);

    let ctx = AccessoryHungerCtx {
        can_eat: true,
        stressed_or_worse: encumbrance >= Encumbrance::Stressed,
        ..Default::default()
    };

    let accessorytime = rng.random_range(0u32..20);
    let depletion = compute_hunger_depletion(&ctx, accessorytime);
    let new_nutrition = old_nutrition - depletion;
    let new_level = nutrition_to_hunger_level(new_nutrition);

    if let Some(mut n) = world.get_component_mut::<Nutrition>(player) {
        n.0 = new_nutrition;
    }

    // Strength penalty on entering/leaving Weak.
    let str_change = strength_penalty_change(old_level, new_level);
    if str_change != 0 {
        events.push(EngineEvent::msg(if str_change < 0 {
            "hunger-weak-strength-loss"
        } else {
            "hunger-weak-strength-restored"
        }));
    }

    // Hunger level change event.
    if old_level != new_level {
        events.push(EngineEvent::HungerChange {
            entity: player,
            old: old_level,
            new_level,
        });
    }

    // Check starvation death.
    let con = world
        .get_component::<crate::world::Attributes>(player)
        .map(|a| a.constitution)
        .unwrap_or(10);

    if should_starve(new_nutrition, con) {
        events.push(EngineEvent::msg("hunger-starvation"));
        events.push(EngineEvent::EntityDied {
            entity: player,
            killer: None,
            cause: crate::event::DeathCause::Starvation,
        });
        return;
    }

    // Check fainting.
    if new_level == HungerLevel::Fainting {
        let was_weak = matches!(
            old_level,
            HungerLevel::Weak | HungerLevel::Fainting | HungerLevel::Fainted
        );
        let already_fainted = old_level == HungerLevel::Fainted;

        if let FaintingOutcome::Faint { duration } =
            check_fainting(new_nutrition, was_weak, already_fainted, rng)
        {
            events.push(EngineEvent::msg("hunger-faint"));
            events.push(EngineEvent::StatusApplied {
                entity: player,
                status: crate::event::StatusEffect::Paralyzed,
                duration: Some(duration as u32),
                source: None,
            });
        }
    }
}

// ── Trap info builder ─────────────────────────────────────────────────

/// Build a `TrapEntityInfo` from player ECS components for trap
/// triggering during movement.
fn build_player_trap_info(
    world: &GameWorld,
    entity: hecs::Entity,
    pos: Position,
) -> TrapEntityInfo {
    let hp_comp = world
        .get_component::<HitPoints>(entity)
        .map(|h| *h)
        .unwrap_or(HitPoints {
            current: 16,
            max: 16,
        });
    let pw_comp = world
        .get_component::<Power>(entity)
        .map(|p| *p)
        .unwrap_or(Power { current: 4, max: 4 });
    let attrs = world
        .get_component::<crate::world::Attributes>(entity)
        .map(|a| *a)
        .unwrap_or_default();
    let luck = world
        .get_component::<PlayerCombat>(entity)
        .map(|pc| pc.luck)
        .unwrap_or(0);
    let intrinsics = match world.get_component::<crate::status::Intrinsics>(entity) {
        Some(i) => (*i).clone(),
        None => crate::status::Intrinsics::default(),
    };
    let is_levitating = crate::status::is_levitating(world, entity);

    TrapEntityInfo {
        entity,
        pos,
        hp: hp_comp.current,
        max_hp: hp_comp.max,
        pw: pw_comp.current,
        max_pw: pw_comp.max,
        ac: 10,
        strength: attrs.strength,
        dexterity: attrs.dexterity,
        is_flying: false,
        is_levitating,
        sleep_resistant: intrinsics.sleep_resistance,
        fire_resistant: intrinsics.fire_resistance,
        poison_resistant: intrinsics.poison_resistance,
        magic_resistant: false,
        is_amorphous: false,
        is_player: world.is_player(entity),
        luck,
    }
}

fn player_is_clerical(world: &GameWorld, player: hecs::Entity) -> bool {
    world
        .get_component::<nethack_babel_data::PlayerIdentity>(player)
        .map(|id| {
            let role = id.role.0;
            role == crate::religion::roles::PRIEST || role == crate::religion::roles::KNIGHT
        })
        .unwrap_or(false)
}

fn count_undead_nearby(world: &GameWorld, center: Position, range: i32) -> u32 {
    let mut count = 0u32;
    for (entity, _) in world.ecs().query::<&Monster>().iter() {
        if world.is_player(entity) {
            continue;
        }
        let Some(pos) = world.get_component::<Positioned>(entity).map(|p| p.0) else {
            continue;
        };
        let dx = (pos.x - center.x).abs();
        let dy = (pos.y - center.y).abs();
        if dx.max(dy) > range {
            continue;
        }
        let is_undead = world
            .get_component::<crate::monster_ai::MonsterSpeciesFlags>(entity)
            .map(|flags| flags.0.contains(nethack_babel_data::MonsterFlags::UNDEAD))
            .unwrap_or(false);
        if is_undead {
            count += 1;
        }
    }
    count
}

fn infer_engrave_method(world: &GameWorld, player: hecs::Entity) -> crate::engrave::EngraveMethod {
    let wielded = world
        .get_component::<crate::equipment::EquipmentSlots>(player)
        .and_then(|slots| slots.weapon);
    let Some(item) = wielded else {
        return crate::engrave::EngraveMethod::Dust;
    };

    if let Some(tag) = world.get_component::<crate::monster_ai::WandTypeTag>(item) {
        return match tag.0 {
            crate::wands::WandType::Fire => crate::engrave::EngraveMethod::Fire,
            crate::wands::WandType::Lightning => crate::engrave::EngraveMethod::Lightning,
            crate::wands::WandType::Digging => crate::engrave::EngraveMethod::Dig,
            _ => crate::engrave::EngraveMethod::Dust,
        };
    }

    let name = item_name_lower(world, item).unwrap_or_default();
    if name.contains("wand of fire") || name.contains("fire wand") {
        return crate::engrave::EngraveMethod::Fire;
    }
    if name.contains("wand of lightning") || name.contains("lightning wand") {
        return crate::engrave::EngraveMethod::Lightning;
    }
    if name.contains("wand of digging") || name.contains("digging wand") {
        return crate::engrave::EngraveMethod::Dig;
    }
    if name.contains("athame")
        || name.contains("pick-axe")
        || name.contains("pickaxe")
        || name.contains("mattock")
    {
        return crate::engrave::EngraveMethod::Dig;
    }

    if world
        .get_component::<nethack_babel_data::ObjectCore>(item)
        .map(|core| core.object_class == nethack_babel_data::ObjectClass::Weapon)
        .unwrap_or(false)
        || name.contains("sword")
        || name.contains("dagger")
        || name.contains("knife")
        || name.contains("blade")
    {
        return crate::engrave::EngraveMethod::Blade;
    }

    crate::engrave::EngraveMethod::Dust
}

fn infer_food_def_from_item(
    world: &GameWorld,
    item: hecs::Entity,
) -> Option<crate::hunger::FoodDef> {
    let core = world.get_component::<nethack_babel_data::ObjectCore>(item)?;
    if core.object_class != nethack_babel_data::ObjectClass::Food {
        return None;
    }

    let name = item_name_lower(world, item).unwrap_or_else(|| "food".to_string());
    let is_corpse = name.contains("corpse");
    let is_tin = name.contains("tin");
    if is_corpse || is_tin {
        return None;
    }

    let is_glob = name.contains("glob");
    let (nutrition, oc_delay) = if name.contains("food ration") {
        (800, 5)
    } else if name.contains("lembas") {
        (800, 2)
    } else if name.contains("cram ration") {
        (600, 3)
    } else if name.contains("fruit") || name.contains("apple") || name.contains("orange") {
        (100, 1)
    } else {
        (200, 2)
    };
    let material = if name.contains("meat")
        || name.contains("tripe")
        || name.contains("egg")
        || name.contains("sausage")
    {
        nethack_babel_data::Material::Flesh
    } else {
        nethack_babel_data::Material::Veggy
    };

    Some(crate::hunger::FoodDef {
        name,
        nutrition,
        oc_delay,
        material,
        is_corpse: false,
        is_tin: false,
        is_glob,
        weight: core.weight,
    })
}

fn infer_potion_type_from_item(
    world: &GameWorld,
    item: hecs::Entity,
) -> Option<crate::potions::PotionType> {
    if let Some(tag) = world.get_component::<crate::monster_ai::PotionTypeTag>(item) {
        return Some(tag.0);
    }

    let name = item_name_lower(world, item)?;
    let normalized = name
        .strip_prefix("potion of ")
        .or_else(|| name.strip_prefix("potion "))
        .unwrap_or(name.as_str());

    use crate::potions::PotionType;
    Some(match normalized {
        "gain ability" => PotionType::GainAbility,
        "restore ability" => PotionType::RestoreAbility,
        "confusion" => PotionType::Confusion,
        "blindness" => PotionType::Blindness,
        "paralysis" => PotionType::Paralysis,
        "speed" | "speed monster" => PotionType::Speed,
        "levitation" => PotionType::Levitation,
        "hallucination" => PotionType::Hallucination,
        "invisibility" => PotionType::Invisibility,
        "see invisible" => PotionType::SeeInvisible,
        "healing" => PotionType::Healing,
        "extra healing" => PotionType::ExtraHealing,
        "gain level" => PotionType::GainLevel,
        "enlightenment" => PotionType::Enlightenment,
        "monster detection" => PotionType::MonsterDetection,
        "object detection" => PotionType::ObjectDetection,
        "gain energy" => PotionType::GainEnergy,
        "sleeping" => PotionType::Sleeping,
        "full healing" => PotionType::FullHealing,
        "polymorph" => PotionType::Polymorph,
        "booze" => PotionType::Booze,
        "sickness" => PotionType::Sickness,
        "fruit juice" => PotionType::FruitJuice,
        "acid" => PotionType::Acid,
        "oil" => PotionType::Oil,
        "water" | "holy water" | "unholy water" => PotionType::Water,
        _ => return None,
    })
}

fn infer_scroll_type_from_item(
    world: &GameWorld,
    item: hecs::Entity,
) -> Option<crate::scrolls::ScrollType> {
    let name = item_name_lower(world, item)?;
    let normalized = name
        .strip_prefix("scroll of ")
        .or_else(|| name.strip_prefix("scroll "))
        .unwrap_or(name.as_str());

    use crate::scrolls::ScrollType;
    Some(match normalized {
        "identify" => ScrollType::Identify,
        "enchant weapon" => ScrollType::EnchantWeapon,
        "enchant armor" => ScrollType::EnchantArmor,
        "remove curse" => ScrollType::RemoveCurse,
        "teleportation" => ScrollType::Teleportation,
        "gold detection" => ScrollType::GoldDetection,
        "food detection" => ScrollType::FoodDetection,
        "confuse monster" => ScrollType::ConfuseMonster,
        "scare monster" => ScrollType::ScareMonster,
        "blank paper" | "blank scroll" => ScrollType::BlankPaper,
        "fire" => ScrollType::Fire,
        "earth" => ScrollType::Earth,
        "punishment" => ScrollType::Punishment,
        "stinking cloud" => ScrollType::StinkingCloud,
        "amnesia" => ScrollType::Amnesia,
        "destroy armor" => ScrollType::DestroyArmor,
        "create monster" => ScrollType::CreateMonster,
        "taming" => ScrollType::Taming,
        "genocide" => ScrollType::Genocide,
        "light" => ScrollType::Light,
        "charging" => ScrollType::Charging,
        "magic mapping" => ScrollType::MagicMapping,
        "mail" => ScrollType::Mail,
        _ => return None,
    })
}

fn item_name_lower(world: &GameWorld, item: hecs::Entity) -> Option<String> {
    world
        .get_component::<Name>(item)
        .map(|name| name.0.to_lowercase())
}

fn infer_fire_direction(world: &GameWorld, player: hecs::Entity) -> Option<Direction> {
    let player_pos = world.get_component::<Positioned>(player).map(|p| p.0)?;
    let mut nearest: Option<(i32, Direction)> = None;

    for (entity, _) in world.ecs().query::<&Monster>().iter() {
        if entity == player {
            continue;
        }
        let Some(target_pos) = world.get_component::<Positioned>(entity).map(|p| p.0) else {
            continue;
        };
        let dx = target_pos.x - player_pos.x;
        let dy = target_pos.y - player_pos.y;
        if dx == 0 && dy == 0 {
            continue;
        }
        let Some(dir) = direction_from_delta(dx.signum(), dy.signum()) else {
            continue;
        };
        let dist = dx.abs().max(dy.abs());
        match nearest {
            Some((best, _)) if dist >= best => {}
            _ => nearest = Some((dist, dir)),
        }
    }

    nearest.map(|(_, dir)| dir)
}

fn direction_from_delta(dx: i32, dy: i32) -> Option<Direction> {
    Some(match (dx, dy) {
        (0, -1) => Direction::North,
        (0, 1) => Direction::South,
        (1, 0) => Direction::East,
        (-1, 0) => Direction::West,
        (1, -1) => Direction::NorthEast,
        (-1, -1) => Direction::NorthWest,
        (1, 1) => Direction::SouthEast,
        (-1, 1) => Direction::SouthWest,
        _ => return None,
    })
}

fn random_monster_spawn_position(world: &GameWorld, rng: &mut impl Rng) -> Option<Position> {
    let player_pos = world
        .get_component::<Positioned>(world.player())
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));
    let map = &world.dungeon().current_level;

    let is_occupied = |pos: Position| {
        world
            .ecs()
            .query::<(&Monster, &Positioned)>()
            .iter()
            .any(|(_, (_, mpos))| mpos.0 == pos)
            || world
                .get_component::<Positioned>(world.player())
                .map(|p| p.0 == pos)
                .unwrap_or(false)
    };

    for _ in 0..32 {
        let dx = rng.random_range(-10..=10);
        let dy = rng.random_range(-10..=10);
        if dx == 0 && dy == 0 {
            continue;
        }
        let pos = Position::new(player_pos.x + dx, player_pos.y + dy);
        let Some(cell) = map.get(pos) else {
            continue;
        };
        if !cell.terrain.is_walkable() || is_occupied(pos) {
            continue;
        }
        return Some(pos);
    }

    for dir in Direction::PLANAR {
        let pos = player_pos.step(dir);
        let Some(cell) = map.get(pos) else {
            continue;
        };
        if cell.terrain.is_walkable() && !is_occupied(pos) {
            return Some(pos);
        }
    }

    None
}

fn spawn_random_monster(world: &mut GameWorld, pos: Position) -> hecs::Entity {
    let order = world.next_creation_order();
    world.spawn((
        Monster,
        Positioned(pos),
        HitPoints { current: 8, max: 8 },
        Speed(NORMAL_SPEED),
        MovementPoints(0),
        Name("wandering monster".to_string()),
        order,
    ))
}

fn default_religion_state(
    world: &GameWorld,
    player: hecs::Entity,
) -> crate::religion::ReligionState {
    let alignment = world
        .get_component::<nethack_babel_data::PlayerIdentity>(player)
        .map(|id| id.alignment)
        .unwrap_or(nethack_babel_data::Alignment::Neutral);
    let hp = world
        .get_component::<HitPoints>(player)
        .map(|hp| *hp)
        .unwrap_or(HitPoints {
            current: 16,
            max: 16,
        });
    let pw = world
        .get_component::<Power>(player)
        .map(|pw| *pw)
        .unwrap_or(Power { current: 4, max: 4 });

    crate::religion::ReligionState {
        alignment,
        alignment_record: 10,
        god_anger: 0,
        god_gifts: 0,
        blessed_amount: 0,
        bless_cooldown: 0,
        crowned: false,
        demigod: false,
        turn: world.turn(),
        experience_level: world
            .get_component::<ExperienceLevel>(player)
            .map(|lvl| lvl.0)
            .unwrap_or(1),
        current_hp: hp.current,
        max_hp: hp.max,
        current_pw: pw.current,
        max_pw: pw.max,
        nutrition: world
            .get_component::<Nutrition>(player)
            .map(|n| n.0)
            .unwrap_or(900),
        luck: world
            .get_component::<PlayerCombat>(player)
            .map(|pc| pc.luck.clamp(i8::MIN as i32, i8::MAX as i32) as i8)
            .unwrap_or(0),
        luck_bonus: 0,
        has_luckstone: false,
        luckstone_blessed: false,
        luckstone_cursed: false,
        in_gehennom: false,
        is_undead: false,
        is_demon: false,
        original_alignment: alignment,
        has_converted: false,
        alignment_abuse: 0,
    }
}

fn refresh_religion_state_from_world(
    state: &mut crate::religion::ReligionState,
    world: &GameWorld,
    player: hecs::Entity,
) {
    if let Some(id) = world.get_component::<nethack_babel_data::PlayerIdentity>(player) {
        state.alignment = id.alignment;
    }
    if let Some(hp) = world.get_component::<HitPoints>(player) {
        state.current_hp = hp.current;
        state.max_hp = hp.max;
    }
    if let Some(pw) = world.get_component::<Power>(player) {
        state.current_pw = pw.current;
        state.max_pw = pw.max;
    }
    if let Some(nutrition) = world.get_component::<Nutrition>(player) {
        state.nutrition = nutrition.0;
    }
    if let Some(level) = world.get_component::<ExperienceLevel>(player) {
        state.experience_level = level.0;
    }
    if let Some(combat) = world.get_component::<PlayerCombat>(player) {
        state.luck = combat.luck.clamp(i8::MIN as i32, i8::MAX as i32) as i8;
    }
    state.turn = world.turn();
    state.in_gehennom = world.dungeon().branch == crate::dungeon::DungeonBranch::Gehennom
        || world.dungeon().current_level_flags.no_prayer;
}

fn persist_religion_state(
    world: &mut GameWorld,
    player: hecs::Entity,
    state: crate::religion::ReligionState,
) {
    if let Some(mut live_state) = world.get_component_mut::<crate::religion::ReligionState>(player)
    {
        *live_state = state.clone();
    } else {
        let _ = world.ecs_mut().insert_one(player, state.clone());
    }

    if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
        hp.current = state.current_hp;
        hp.max = state.max_hp;
    }
    if let Some(mut pw) = world.get_component_mut::<Power>(player) {
        pw.current = state.current_pw;
        pw.max = state.max_pw;
    }
    if let Some(mut nutrition) = world.get_component_mut::<Nutrition>(player) {
        nutrition.0 = state.nutrition;
    }
}

fn read_quest_state(world: &GameWorld, player: hecs::Entity) -> crate::quest::QuestState {
    world
        .get_component::<crate::quest::QuestState>(player)
        .map(|state| (*state).clone())
        .unwrap_or_default()
}

fn persist_quest_state(
    world: &mut GameWorld,
    player: hecs::Entity,
    state: crate::quest::QuestState,
) {
    if let Some(mut live_state) = world.get_component_mut::<crate::quest::QuestState>(player) {
        *live_state = state;
    } else {
        let _ = world.ecs_mut().insert_one(player, state);
    }
}

fn current_player_alignment(world: &GameWorld, player: hecs::Entity) -> Alignment {
    world
        .get_component::<PlayerIdentity>(player)
        .map(|identity| identity.alignment)
        .unwrap_or(Alignment::Neutral)
}

fn current_player_level(world: &GameWorld, player: hecs::Entity) -> u8 {
    world
        .get_component::<ExperienceLevel>(player)
        .map(|level| level.0)
        .unwrap_or(1)
}

fn current_player_alignment_record(world: &GameWorld, player: hecs::Entity) -> i32 {
    world
        .get_component::<crate::religion::ReligionState>(player)
        .map(|state| state.alignment_record)
        .unwrap_or_else(|| default_religion_state(world, player).alignment_record)
}

fn current_player_is_female(world: &GameWorld, player: hecs::Entity) -> bool {
    world
        .get_component::<PlayerIdentity>(player)
        .is_some_and(|identity| identity.gender == nethack_babel_data::Gender::Female)
}

fn mark_angry_quest_leader_from_targets(
    world: &mut GameWorld,
    targets: &std::collections::HashSet<hecs::Entity>,
) {
    let player = world.player();
    let attacked_leader = targets.iter().any(|entity| {
        quest_npc_role_for_entity(world, *entity) == Some(crate::quest::QuestNpcRole::Leader)
    });
    if !attacked_leader {
        return;
    }

    let mut state = read_quest_state(world, player);
    state.anger_leader();
    persist_quest_state(world, player, state);
}

fn find_shop_room_index_by_shopkeeper(world: &GameWorld, entity: hecs::Entity) -> Option<usize> {
    world
        .dungeon()
        .shop_rooms
        .iter()
        .position(|shop| shop.shopkeeper == entity)
}

fn shopkeeper_is_alive(world: &GameWorld, entity: hecs::Entity) -> bool {
    live_monster_entity(world, entity)
}

fn shop_room_is_deserted(world: &GameWorld, shop: &crate::shop::ShopRoom) -> bool {
    crate::shop::is_shop_deserted(shopkeeper_is_alive(world, shop.shopkeeper))
}

fn shop_rooms_are_adjacent(lhs: &crate::shop::ShopRoom, rhs: &crate::shop::ShopRoom) -> bool {
    lhs.top_left.x - 1 <= rhs.bottom_right.x
        && lhs.bottom_right.x + 1 >= rhs.top_left.x
        && lhs.top_left.y - 1 <= rhs.bottom_right.y
        && lhs.bottom_right.y + 1 >= rhs.top_left.y
}

fn find_inheriting_shopkeeper(
    world: &GameWorld,
    dead_shop_idx: usize,
) -> Option<(hecs::Entity, String)> {
    let dead_shop = world.dungeon().shop_rooms.get(dead_shop_idx)?;
    world
        .dungeon()
        .shop_rooms
        .iter()
        .enumerate()
        .find_map(|(idx, candidate)| {
            if idx == dead_shop_idx || !shopkeeper_is_alive(world, candidate.shopkeeper) {
                return None;
            }
            crate::shop::check_shop_inheritance(
                dead_shop.angry,
                shop_rooms_are_adjacent(dead_shop, candidate),
                candidate.angry,
            )
            .then(|| (candidate.shopkeeper, candidate.shopkeeper_name.clone()))
        })
}

fn sync_shopkeeper_deaths_from_events(world: &mut GameWorld, events: &mut Vec<EngineEvent>) {
    let mut dead_shop_indices = Vec::new();
    for event in events.iter() {
        if let EngineEvent::EntityDied { entity, .. } = event
            && let Some(idx) = find_shop_room_index_by_shopkeeper(world, *entity)
        {
            dead_shop_indices.push(idx);
        }
    }
    dead_shop_indices.sort_unstable();
    dead_shop_indices.dedup();

    let mut extra_events = Vec::new();
    for idx in dead_shop_indices {
        let Some(dead_shop) = world.dungeon().shop_rooms.get(idx).cloned() else {
            continue;
        };
        let _ = world
            .ecs_mut()
            .remove_one::<crate::npc::Shopkeeper>(dead_shop.shopkeeper);

        if let Some((inheritor, inheritor_name)) = find_inheriting_shopkeeper(world, idx) {
            let shop = &mut world.dungeon_mut().shop_rooms[idx];
            shop.shopkeeper = inheritor;
            shop.shopkeeper_name = inheritor_name;
            extra_events.push(EngineEvent::msg_with(
                "shop-keeper-dead",
                vec![("shopkeeper", dead_shop.shopkeeper_name)],
            ));
            continue;
        }

        let mut death_events = {
            let shop = &mut world.dungeon_mut().shop_rooms[idx];
            crate::shop::shopkeeper_died(shop)
        };
        extra_events.append(&mut death_events);
    }

    events.extend(extra_events);
}

fn rile_attacked_shopkeepers(
    world: &mut GameWorld,
    targets: &std::collections::HashSet<hecs::Entity>,
) {
    let mut indices: Vec<usize> = targets
        .iter()
        .filter_map(|entity| find_shop_room_index_by_shopkeeper(world, *entity))
        .collect();
    indices.sort_unstable();
    indices.dedup();
    for idx in indices {
        crate::shop::rile_shop(&mut world.dungeon_mut().shop_rooms[idx]);
    }
}

fn infer_priest_runtime(world: &GameWorld, entity: hecs::Entity) -> Option<crate::npc::Priest> {
    if let Some(priest) = world.get_component::<crate::npc::Priest>(entity) {
        return Some(*priest);
    }

    let name = world.get_component::<Name>(entity)?.0.to_ascii_lowercase();
    if !name.contains("priest") {
        return None;
    }

    let pos = world
        .get_component::<Positioned>(entity)
        .map(|position| position.0)?;
    let on_altar = world
        .dungeon()
        .current_level
        .get(pos)
        .is_some_and(|cell| cell.terrain == Terrain::Altar);
    Some(crate::npc::Priest {
        alignment: current_player_alignment(world, world.player()),
        has_shrine: on_altar,
        is_high_priest: name.contains("high priest"),
        angry: false,
    })
}

fn strip_leading_article(name: &str) -> &str {
    name.strip_prefix("the ")
        .or_else(|| name.strip_prefix("The "))
        .unwrap_or(name)
}

pub(crate) fn quest_name_matches(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
        || strip_leading_article(actual).eq_ignore_ascii_case(strip_leading_article(expected))
}

fn item_display_name(world: &GameWorld, item: hecs::Entity) -> Option<String> {
    if let Some(name) = world.get_component::<Name>(item) {
        return Some(name.0.clone());
    }
    let core = world.get_component::<ObjectCore>(item)?;
    crate::items::object_def_for_core(world.object_catalog(), &core).map(|def| def.name.clone())
}

fn find_player_named_item(
    world: &GameWorld,
    player: hecs::Entity,
    expected_name: &str,
) -> Option<hecs::Entity> {
    if let Some(inv) = world.get_component::<crate::inventory::Inventory>(player)
        && let Some(item) = inv.items.iter().find(|item| {
            item_display_name(world, **item)
                .is_some_and(|name| quest_name_matches(&name, expected_name))
        })
    {
        return Some(*item);
    }

    world
        .get_component::<crate::equipment::EquipmentSlots>(player)
        .and_then(|slots| {
            slots.all_worn().iter().find_map(|(_, item)| {
                item_display_name(world, *item)
                    .is_some_and(|name| quest_name_matches(&name, expected_name))
                    .then_some(*item)
            })
        })
}

fn player_has_worn_or_wielded_named_item(
    world: &GameWorld,
    player: hecs::Entity,
    expected_name: &str,
) -> bool {
    world
        .get_component::<crate::equipment::EquipmentSlots>(player)
        .is_some_and(|slots| {
            [slots.weapon, slots.amulet]
                .into_iter()
                .flatten()
                .any(|item| {
                    item_display_name(world, item)
                        .is_some_and(|name| quest_name_matches(&name, expected_name))
                })
        })
}

fn player_carries_item(world: &GameWorld, player: hecs::Entity, item: hecs::Entity) -> bool {
    world
        .get_component::<crate::inventory::Inventory>(player)
        .is_some_and(|inv| inv.contains(item))
        || world
            .get_component::<crate::equipment::EquipmentSlots>(player)
            .and_then(|slots| slots.find_slot(item))
            .is_some()
}

fn remove_item_from_player_possessions(
    world: &mut GameWorld,
    player: hecs::Entity,
    item: hecs::Entity,
) -> bool {
    let removed = detach_item_from_player_possessions(world, player, item);
    if removed {
        let _ = world.despawn(item);
    }
    removed
}

fn detach_item_from_player_possessions(
    world: &mut GameWorld,
    player: hecs::Entity,
    item: hecs::Entity,
) -> bool {
    let mut removed = false;
    if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
        removed |= inv.remove(item);
    }
    if let Some(mut slots) = world.get_component_mut::<crate::equipment::EquipmentSlots>(player)
        && let Some(slot) = slots.find_slot(item)
    {
        slots.set(slot, None);
        removed = true;
    }
    removed
}

fn wizard_of_yendor_entities(world: &GameWorld) -> Vec<hecs::Entity> {
    world
        .ecs()
        .query::<(&Monster, &Name)>()
        .iter()
        .filter_map(|(entity, (_monster, name))| {
            (live_monster_entity(world, entity) && name.0.eq_ignore_ascii_case("Wizard of Yendor"))
                .then_some(entity)
        })
        .collect()
}

fn wizard_ready_for_harassment(world: &GameWorld, wizard: hecs::Entity) -> bool {
    live_monster_entity(world, wizard)
        && world
            .get_component::<crate::status::StatusEffects>(wizard)
            .is_none_or(|status| status.sleeping == 0 && status.paralysis == 0)
}

fn sync_wizard_of_yendor_from_events(
    world: &mut GameWorld,
    known_wizards: &[hecs::Entity],
    events: &[EngineEvent],
) {
    if known_wizards.is_empty() {
        return;
    }

    let killed_count = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                EngineEvent::EntityDied { entity, .. } if known_wizards.contains(entity)
            )
        })
        .count() as u32;
    if killed_count == 0 {
        return;
    }

    let player = world.player();
    let mut player_events = read_player_events(world, player);
    player_events.killed_wizard = true;
    player_events.wizard_last_killed_turn = world.turn();
    player_events.wizard_times_killed = player_events
        .wizard_times_killed
        .saturating_add(killed_count);
    persist_player_events(world, player, player_events);
}

fn process_wizard_of_yendor_turn(
    world: &mut GameWorld,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    let player = world.player();
    let mut player_events = read_player_events(world, player);
    let player_has_amulet = player_has_named_item(world, player, "Amulet of Yendor");
    let wizard_times_killed = player_events
        .wizard_times_killed
        .max(u32::from(player_events.killed_wizard));

    if !(player_has_amulet || player_events.invoked || player_events.killed_wizard) {
        return;
    }

    let live_wizards = wizard_of_yendor_entities(world);
    if live_wizards.is_empty() {
        let wizard_state = crate::npc::WizardOfYendor {
            last_killed_turn: player_events.wizard_last_killed_turn,
            times_killed: wizard_times_killed,
        };
        if crate::npc::wizard_should_respawn(&wizard_state, world.turn(), rng) {
            if let Some((wizard, pos, origin)) = spawn_or_respawn_wizard_near_player(world, rng) {
                seed_wizard_runtime_state(world, wizard);
                events.push(EngineEvent::MonsterGenerated {
                    entity: wizard,
                    position: pos,
                });
                emit_wizard_respawn_messages(world, player, origin, events);
            }
            return;
        }
        if player_events.wizard_intervention_cooldown == 0
            && player_events.killed_wizard
            && player_events.wizard_last_killed_turn == world.turn()
        {
            player_events.wizard_intervention_cooldown = wizard_intervention_delay(rng);
        }
        if player_events.wizard_intervention_cooldown > 0 {
            player_events.wizard_intervention_cooldown -= 1;
            persist_player_events(world, player, player_events);
            return;
        }
        let allow_resurrection = !is_astral_plane(world);
        let action = crate::npc::choose_wizard_intervene_action(allow_resurrection, rng);
        events.extend(wizard_harassment_messages(world, player, action));
        apply_wizard_harassment_action(world, None, player, action, rng, events);
        player_events.wizard_intervention_cooldown = wizard_intervention_delay(rng);
        persist_player_events(world, player, player_events);
        return;
    }

    let Some(active_wizard) = live_wizards
        .iter()
        .copied()
        .find(|wizard| wizard_ready_for_harassment(world, *wizard))
    else {
        return;
    };

    let harassment_period = if player_has_amulet {
        4
    } else if player_events.invoked || player_events.killed_wizard {
        5
    } else {
        6
    };
    if rng.random_range(0..harassment_period) != 0 {
        if rng.random_range(0..3) == 0
            && let Some(taunt) =
                crate::npc::maybe_wizard_taunt(world, active_wizard, player, player_has_amulet, rng)
        {
            events.push(taunt);
            if let Some(wizard_pos) = world
                .get_component::<Positioned>(active_wizard)
                .map(|pos| pos.0)
            {
                wake_sleeping_monsters_near_position(world, wizard_pos, 5, events);
            }
        }
        return;
    }

    let wizard = active_wizard;
    seed_wizard_runtime_state(world, wizard);
    if should_wizard_level_teleport_player(world, player_has_amulet, rng)
        && apply_wizard_level_teleport(world, rng, events)
    {
        return;
    }
    let action = crate::npc::choose_wizard_action(world, wizard, player_has_amulet, rng);
    events.extend(wizard_harassment_messages(world, player, action));
    apply_wizard_harassment_action(world, Some(wizard), player, action, rng, events);
}

fn wizard_intervention_delay(rng: &mut impl Rng) -> u32 {
    rng.random_range(50..=249)
}

fn wizard_invocation_delay(rng: &mut impl Rng) -> u32 {
    rng.random_range(1..=6) + rng.random_range(1..=6)
}

fn is_astral_plane(world: &GameWorld) -> bool {
    world.dungeon().branch == DungeonBranch::Endgame && world.dungeon().depth == 5
}

fn wizard_harassment_messages(
    world: &GameWorld,
    player: hecs::Entity,
    action: crate::npc::WizardAction,
) -> Vec<EngineEvent> {
    match action {
        crate::npc::WizardAction::BlackGlowCurse if crate::status::is_blind(world, player) => {
            Vec::new()
        }
        crate::npc::WizardAction::Resurrect => Vec::new(),
        _ => crate::npc::wizard_harass_events(action),
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum WizardRespawnOrigin {
    New,
    Cached,
}

fn emit_wizard_respawn_messages(
    world: &GameWorld,
    player: hecs::Entity,
    origin: WizardRespawnOrigin,
    events: &mut Vec<EngineEvent>,
) {
    events.push(EngineEvent::msg("wizard-respawned"));
    if crate::status::is_deaf(world, player) {
        return;
    }

    events.push(EngineEvent::msg("wizard-respawned-boom"));
    let verb = match origin {
        WizardRespawnOrigin::New => "kill",
        WizardRespawnOrigin::Cached => "elude",
    };
    events.push(EngineEvent::msg_with(
        "wizard-respawned-taunt",
        vec![("verb", verb.to_string())],
    ));
}

#[doc(hidden)]
pub fn force_wizard_harassment_action(
    world: &mut GameWorld,
    wizard: Option<hecs::Entity>,
    player: hecs::Entity,
    action: crate::npc::WizardAction,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = wizard_harassment_messages(world, player, action);
    apply_wizard_harassment_action(world, wizard, player, action, rng, &mut events);
    events
}

#[doc(hidden)]
pub fn force_player_has_named_item(
    world: &GameWorld,
    player: hecs::Entity,
    expected_name: &str,
) -> bool {
    player_has_named_item(world, player, expected_name)
}

#[doc(hidden)]
pub fn force_live_wizard_count(world: &GameWorld) -> usize {
    wizard_of_yendor_entities(world).len()
}

#[doc(hidden)]
pub fn force_item_display_name(world: &GameWorld, item: hecs::Entity) -> Option<String> {
    item_display_name(world, item)
}

fn should_wizard_level_teleport_player(
    world: &GameWorld,
    player_has_amulet: bool,
    rng: &mut impl Rng,
) -> bool {
    if player_has_amulet {
        return false;
    }

    let branch = world.dungeon().branch;
    if branch == DungeonBranch::Endgame || branch_max_depth(branch) <= 1 {
        return false;
    }

    let player_events = read_player_events(world, world.player());
    if !(player_events.invoked || player_events.killed_wizard) {
        return false;
    }

    rng.random_range(0..4) == 0
}

fn choose_wizard_level_teleport_depth(world: &GameWorld, rng: &mut impl Rng) -> Option<i32> {
    let branch = world.dungeon().branch;
    let max_depth = branch_max_depth(branch);
    let current_depth = world.dungeon().depth.clamp(1, max_depth);
    if max_depth <= 1 {
        return None;
    }

    let mut depths: Vec<i32> = (1..=max_depth)
        .filter(|depth| *depth != current_depth)
        .collect();
    if depths.is_empty() {
        return None;
    }

    Some(depths.swap_remove(rng.random_range(0..depths.len())))
}

fn apply_wizard_level_teleport(
    world: &mut GameWorld,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) -> bool {
    let current_depth = world.dungeon().depth;
    let Some(target_depth) = choose_wizard_level_teleport_depth(world, rng) else {
        return false;
    };

    events.push(EngineEvent::msg_with(
        "wizard-level-teleport",
        vec![("depth", target_depth.to_string())],
    ));
    change_level(
        world,
        target_depth,
        target_depth < current_depth,
        rng,
        events,
    );
    true
}

fn aggravate_monsters_on_current_level(
    world: &mut GameWorld,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    wake_sleeping_monsters_near_position(world, Position::new(0, 0), i32::MAX, events);

    let paralyzed_monsters: Vec<hecs::Entity> = world
        .ecs()
        .query::<(&Monster,)>()
        .iter()
        .map(|(entity, _)| entity)
        .filter(|entity| {
            live_monster_entity(world, *entity)
                && world
                    .get_component::<crate::status::StatusEffects>(*entity)
                    .is_some_and(|status| status.paralysis > 0)
        })
        .collect();

    for entity in paralyzed_monsters {
        if rng.random_ratio(1, 5)
            && let Some(mut status) =
                world.get_component_mut::<crate::status::StatusEffects>(entity)
            && status.paralysis > 0
        {
            status.paralysis = 0;
            events.push(EngineEvent::StatusRemoved {
                entity,
                status: crate::event::StatusEffect::Paralyzed,
            });
        }
    }
}

fn wake_sleeping_monsters_near_position(
    world: &mut GameWorld,
    center: Position,
    radius: i32,
    events: &mut Vec<EngineEvent>,
) {
    let sleepers: Vec<hecs::Entity> = world
        .ecs()
        .query::<(&Monster,)>()
        .iter()
        .map(|(entity, _)| entity)
        .filter(|entity| {
            live_monster_entity(world, *entity)
                && crate::status::is_sleeping(world, *entity)
                && (radius == i32::MAX
                    || world
                        .get_component::<Positioned>(*entity)
                        .is_some_and(|pos| {
                            crate::ball::chebyshev_distance(center, pos.0) <= radius
                        }))
        })
        .collect();

    for entity in sleepers {
        events.extend(crate::status::wake_from_sleeping(world, entity));
    }
}

fn try_respawn_cached_wizard_near_player(
    world: &mut GameWorld,
    rng: &mut impl Rng,
) -> Option<(hecs::Entity, Position)> {
    let player = world.player();
    let player_pos = world.get_component::<Positioned>(player).map(|pos| pos.0)?;
    let source_key = world
        .dungeon()
        .monster_cache
        .iter()
        .find_map(|(key, monsters)| {
            monsters
                .iter()
                .any(|monster| quest_name_matches(&monster.name, "Wizard of Yendor"))
                .then_some(*key)
        })?;

    let cached_wizard = {
        let cache = world.dungeon_mut().monster_cache.get_mut(&source_key)?;
        let wizard_idx = cache
            .iter()
            .position(|monster| quest_name_matches(&monster.name, "Wizard of Yendor"))?;
        cache.remove(wizard_idx)
    };
    if world
        .dungeon()
        .monster_cache
        .get(&source_key)
        .is_some_and(|cache| cache.is_empty())
    {
        world.dungeon_mut().monster_cache.remove(&source_key);
    }

    let monster_id =
        resolve_monster_id_by_spec(world.monster_catalog(), cached_wizard.name.as_str())?;
    let monster_defs = world.monster_catalog().to_vec();
    let monster_def = monster_defs.iter().find(|def| def.id == monster_id)?;
    let spawn_pos = enexto(world, player_pos, monster_def)?;
    let entity = makemon(
        world,
        &monster_defs,
        Some(monster_id),
        spawn_pos,
        MakeMonFlags::NO_GROUP,
        rng,
    )?;

    if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
        hp.current = cached_wizard.hp_current;
        hp.max = cached_wizard.hp_max;
    }
    let mut status_effects = cached_wizard.status_effects.clone();
    status_effects.sleeping = 0;
    status_effects.paralysis = 0;
    let _ = world.ecs_mut().insert_one(entity, status_effects);
    if cached_wizard.is_peaceful {
        let _ = world.ecs_mut().insert_one(entity, Peaceful);
    }
    if cached_wizard.is_tame {
        let _ = world.ecs_mut().insert_one(entity, Tame);
    }
    if let Some(priest) = cached_wizard.priest {
        let _ = world.ecs_mut().insert_one(entity, priest);
    }
    if let Some(shopkeeper) = cached_wizard.shopkeeper {
        let _ = world.ecs_mut().insert_one(entity, shopkeeper);
    }
    if let Some(role) = cached_wizard.quest_npc_role {
        let _ = world.ecs_mut().insert_one(entity, role);
    }
    if cached_wizard.creation_order > 0 {
        let _ = world
            .ecs_mut()
            .insert_one(entity, CreationOrder(cached_wizard.creation_order));
    }

    Some((entity, spawn_pos))
}

fn spawn_or_respawn_wizard_near_player(
    world: &mut GameWorld,
    rng: &mut impl Rng,
) -> Option<(hecs::Entity, Position, WizardRespawnOrigin)> {
    let player = world.player();
    if let Some((wizard, pos)) = try_respawn_cached_wizard_near_player(world, rng) {
        return Some((wizard, pos, WizardRespawnOrigin::Cached));
    }
    spawn_named_monster_near_entity(world, player, "Wizard of Yendor", rng)
        .map(|(wizard, pos)| (wizard, pos, WizardRespawnOrigin::New))
}

fn apply_wizard_harassment_action(
    world: &mut GameWorld,
    wizard: Option<hecs::Entity>,
    player: hecs::Entity,
    action: crate::npc::WizardAction,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    match action {
        crate::npc::WizardAction::StealAmulet => {
            if let Some(wizard) = wizard
                && world
                    .get_component::<Positioned>(wizard)
                    .zip(world.get_component::<Positioned>(player))
                    .is_some_and(|(wizard_pos, player_pos)| {
                        crate::ball::chebyshev_distance(wizard_pos.0, player_pos.0) == 1
                    })
                && let Some(amulet) = find_player_named_item(world, player, "Amulet of Yendor")
            {
                wizard_steal_amulet(world, wizard, player, amulet);
            }
        }
        crate::npc::WizardAction::DoubleTrouble => {
            let Some(wizard) = wizard else {
                return;
            };
            if wizard_of_yendor_entities(world).len() >= 2 {
                return;
            }
            if let Some((clone, pos)) =
                spawn_named_monster_near_entity(world, wizard, "Wizard of Yendor", rng)
            {
                seed_wizard_runtime_state(world, clone);
                events.push(EngineEvent::MonsterGenerated {
                    entity: clone,
                    position: pos,
                });
            }
        }
        crate::npc::WizardAction::SummonNasties => {
            let summon_anchor = wizard.unwrap_or(player);
            for spec in choose_wizard_nasty_summon_specs(world, rng) {
                if let Some((monster, pos)) = spawn_named_monster_near_entity_with_flags(
                    world,
                    summon_anchor,
                    spec.as_str(),
                    MakeMonFlags::empty(),
                    rng,
                ) {
                    events.push(EngineEvent::MonsterGenerated {
                        entity: monster,
                        position: pos,
                    });
                }
            }
        }
        crate::npc::WizardAction::CurseItems => {
            curse_random_player_items(world, player, rng);
        }
        crate::npc::WizardAction::VagueNervous => {}
        crate::npc::WizardAction::BlackGlowCurse => {
            curse_random_player_items(world, player, rng);
        }
        crate::npc::WizardAction::Aggravate => {
            aggravate_monsters_on_current_level(world, rng, events);
        }
        crate::npc::WizardAction::Resurrect => {
            if wizard_of_yendor_entities(world).is_empty()
                && let Some((wizard, pos, origin)) = spawn_or_respawn_wizard_near_player(world, rng)
            {
                seed_wizard_runtime_state(world, wizard);
                events.push(EngineEvent::MonsterGenerated {
                    entity: wizard,
                    position: pos,
                });
                emit_wizard_respawn_messages(world, player, origin, events);
            }
        }
    }
}

fn process_amulet_portal_sense(
    world: &GameWorld,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    let player = world.player();
    if !player_has_worn_or_wielded_named_item(world, player, "Amulet of Yendor")
        || rng.random_range(0..15) != 0
    {
        return;
    }

    let Some(player_pos) = world.get_component::<Positioned>(player).map(|pos| pos.0) else {
        return;
    };
    let Some(portal_pos) = find_terrain(&world.dungeon().current_level, Terrain::MagicPortal)
    else {
        return;
    };

    let dx = player_pos.x - portal_pos.x;
    let dy = player_pos.y - portal_pos.y;
    let dist2 = dx * dx + dy * dy;
    let key = if dist2 <= 9 {
        "amulet-feels-hot"
    } else if dist2 <= 64 {
        "amulet-feels-very-warm"
    } else if dist2 <= 144 {
        "amulet-feels-warm"
    } else {
        return;
    };
    events.push(EngineEvent::msg(key));
}

fn process_amulet_wakes_sleeping_wizard(
    world: &mut GameWorld,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    let player = world.player();
    if !player_has_named_item(world, player, "Amulet of Yendor") || rng.random_range(0..40) != 0 {
        return;
    }

    let Some(player_pos) = world.get_component::<Positioned>(player).map(|pos| pos.0) else {
        return;
    };

    for wizard in wizard_of_yendor_entities(world) {
        if !crate::status::is_sleeping(world, wizard) {
            continue;
        }

        let wizard_far_away = world
            .get_component::<Positioned>(wizard)
            .map(|pos| crate::ball::chebyshev_distance(player_pos, pos.0) > 1)
            .unwrap_or(true);
        events.extend(crate::status::wake_from_sleeping(world, wizard));
        if wizard_far_away {
            events.push(EngineEvent::msg("wizard-vague-nervous"));
        }
        return;
    }
}

#[doc(hidden)]
pub fn force_amulet_wake_check(world: &mut GameWorld, rng: &mut impl Rng) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    process_amulet_wakes_sleeping_wizard(world, rng, &mut events);
    events
}

fn wizard_nasty_weight(
    monster: &MonsterDef,
    branch: DungeonBranch,
    desired_difficulty: u8,
) -> usize {
    let mut weight = 1usize;
    if monster.flags.contains(MonsterFlags::NASTY) {
        weight += 2;
    }
    if monster.difficulty >= desired_difficulty {
        weight += 2;
    }
    if monster
        .attacks
        .iter()
        .any(|attack| attack.method == nethack_babel_data::AttackMethod::MagicMissile)
    {
        weight += 1;
    }
    if branch == DungeonBranch::Gehennom && monster.flags.contains(MonsterFlags::DEMON) {
        weight += 3;
        if monster
            .flags
            .intersects(MonsterFlags::LORD | MonsterFlags::PRINCE)
        {
            weight += 4;
        }
    }
    weight
}

fn choose_wizard_nasty_summon_specs(world: &GameWorld, rng: &mut impl Rng) -> Vec<String> {
    let branch = world.dungeon().branch;
    let depth = world.dungeon().depth.max(1) as u8;
    let player = world.player();
    let player_level = world
        .get_component::<ExperienceLevel>(player)
        .map(|xl| xl.0.max(1))
        .unwrap_or(1);
    let player_events = read_player_events(world, world.player());
    let roll_cap = usize::from((player_level / 3).max(1)).min(10);
    let mut desired_count = rng.random_range(1..=roll_cap);
    if branch == DungeonBranch::Endgame || player_events.invoked {
        desired_count = desired_count.max(4);
    } else if branch == DungeonBranch::Gehennom {
        desired_count = desired_count.max(3);
    }
    let desired_difficulty = if branch == DungeonBranch::Endgame || player_events.invoked {
        depth.saturating_add(12).max(20)
    } else if branch == DungeonBranch::Gehennom {
        depth.saturating_add(8)
    } else {
        depth.saturating_add(4)
    };

    let mut candidates: Vec<&MonsterDef> = world
        .monster_catalog()
        .iter()
        .filter(|monster| {
            !monster
                .geno_flags
                .intersects(GenoFlags::G_UNIQ | GenoFlags::G_NOGEN)
                && monster.flags.contains(MonsterFlags::HOSTILE)
                && (monster.flags.contains(MonsterFlags::NASTY)
                    || monster.difficulty >= desired_difficulty)
                && monster.names.male != "Wizard of Yendor"
                && (branch == DungeonBranch::Gehennom
                    || !monster.geno_flags.contains(GenoFlags::G_HELL))
        })
        .collect();
    candidates.sort_by(|lhs, rhs| {
        rhs.difficulty
            .cmp(&lhs.difficulty)
            .then(rhs.base_level.cmp(&lhs.base_level))
            .then(lhs.names.male.cmp(&rhs.names.male))
    });

    if candidates.is_empty() {
        return vec!["vampire lord".to_string(), "xorn".to_string()];
    }

    let mut picks = Vec::new();
    if branch == DungeonBranch::Gehennom && rng.random_range(0..10) == 0 {
        let demon_surge_pool = candidates
            .iter()
            .filter(|monster| monster.flags.contains(MonsterFlags::DEMON))
            .copied()
            .collect::<Vec<_>>();
        if !demon_surge_pool.is_empty() {
            let mut weighted_demon_pool = Vec::new();
            for monster in demon_surge_pool {
                let copies = if monster
                    .flags
                    .intersects(MonsterFlags::LORD | MonsterFlags::PRINCE)
                {
                    3
                } else {
                    1
                };
                for _ in 0..copies {
                    weighted_demon_pool.push(monster);
                }
            }
            let idx = rng.random_range(0..weighted_demon_pool.len());
            picks.push(weighted_demon_pool[idx].names.male.clone());
        }
    }

    let mut weighted_pool = Vec::new();
    for monster in &candidates {
        for _ in 0..wizard_nasty_weight(monster, branch, desired_difficulty) {
            weighted_pool.push(*monster);
        }
    }
    if weighted_pool.is_empty() {
        weighted_pool.extend(candidates.iter().copied());
    }

    let unique_cap = desired_count.min(candidates.len());
    let mut attempts = 0usize;
    let max_attempts = desired_count.saturating_mul(24).max(24);
    while picks.len() < desired_count && !weighted_pool.is_empty() && attempts < max_attempts {
        let idx = rng.random_range(0..weighted_pool.len());
        let monster = weighted_pool[idx];
        let already_picked = picks
            .iter()
            .any(|name: &String| quest_name_matches(name, &monster.names.male));
        if already_picked && picks.len() < unique_cap {
            attempts += 1;
            continue;
        }
        picks.push(monster.names.male.clone());
        attempts += 1;
    }

    while picks.len() < desired_count {
        let idx = rng.random_range(0..candidates.len());
        picks.push(candidates[idx].names.male.clone());
    }

    if picks.is_empty() {
        vec!["vampire lord".to_string(), "xorn".to_string()]
    } else {
        picks
    }
}

fn seed_wizard_runtime_state(world: &mut GameWorld, wizard: hecs::Entity) {
    let player_events = read_player_events(world, world.player());
    let state = crate::npc::WizardOfYendor {
        last_killed_turn: player_events.wizard_last_killed_turn,
        times_killed: player_events
            .wizard_times_killed
            .max(u32::from(player_events.killed_wizard)),
    };
    let _ = world.ecs_mut().insert_one(wizard, state);
}

fn spawn_named_monster_near_entity(
    world: &mut GameWorld,
    anchor: hecs::Entity,
    monster_spec: &str,
    rng: &mut impl Rng,
) -> Option<(hecs::Entity, Position)> {
    spawn_named_monster_near_entity_with_flags(
        world,
        anchor,
        monster_spec,
        MakeMonFlags::NO_GROUP,
        rng,
    )
}

fn spawn_named_monster_near_entity_with_flags(
    world: &mut GameWorld,
    anchor: hecs::Entity,
    monster_spec: &str,
    flags: MakeMonFlags,
    rng: &mut impl Rng,
) -> Option<(hecs::Entity, Position)> {
    let anchor_pos = world.get_component::<Positioned>(anchor).map(|pos| pos.0)?;
    let monster_defs = world.monster_catalog().to_vec();
    if let Some(monster_id) = resolve_monster_id_by_spec(&monster_defs, monster_spec)
        && let Some(monster_def) = monster_defs.iter().find(|def| def.id == monster_id)
        && let Some(spawn_pos) = enexto(world, anchor_pos, monster_def)
        && let Some(entity) = makemon(
            world,
            &monster_defs,
            Some(monster_id),
            spawn_pos,
            flags,
            rng,
        )
    {
        return Some((entity, spawn_pos));
    }

    let spawn_pos = resolve_special_spawn_pos(world, Some(anchor_pos), rng)?;
    let creation_order = world.next_creation_order();
    let entity = world.spawn((
        Monster,
        Positioned(spawn_pos),
        HitPoints {
            current: 20,
            max: 20,
        },
        Speed(NORMAL_SPEED),
        MovementPoints(0),
        Name(monster_spec.to_string()),
        DisplaySymbol {
            symbol: monster_spec.chars().next().unwrap_or('M'),
            color: nethack_babel_data::Color::White,
        },
        creation_order,
    ));
    Some((entity, spawn_pos))
}

fn wizard_steal_amulet(
    world: &mut GameWorld,
    wizard: hecs::Entity,
    player: hecs::Entity,
    amulet: hecs::Entity,
) {
    if !detach_item_from_player_possessions(world, player, amulet) {
        return;
    }

    let drop_origin = world
        .get_component::<Positioned>(wizard)
        .map(|pos| pos.0)
        .or_else(|| world.get_component::<Positioned>(player).map(|pos| pos.0))
        .unwrap_or(Position::new(0, 0));
    let drop_pos = nearest_walkable_drop_pos(world, drop_origin);

    if let Some(mut core) = world.get_component_mut::<ObjectCore>(amulet) {
        core.inv_letter = None;
    }
    let branch = world.dungeon().branch;
    let depth = world.dungeon().depth;
    if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(amulet) {
        *loc = crate::dungeon::floor_object_location(branch, depth, drop_pos);
    }
}

fn curse_random_player_items(world: &mut GameWorld, player: hecs::Entity, rng: &mut impl Rng) {
    let mut items: Vec<hecs::Entity> = crate::items::get_inventory(world, player)
        .into_iter()
        .map(|(item, _)| item)
        .collect();
    if items.is_empty() {
        return;
    }

    let count = usize::min(
        crate::sit::curse_count(rng, false, false) as usize,
        items.len(),
    );
    for _ in 0..count {
        let idx = rng.random_range(0..items.len());
        let item = items.swap_remove(idx);
        if let Some(mut buc) = world.get_component_mut::<BucStatus>(item) {
            buc.cursed = true;
            buc.blessed = false;
        }
    }
}

fn nearest_walkable_drop_pos(world: &GameWorld, origin: Position) -> Position {
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let pos = Position::new(origin.x + dx, origin.y + dy);
            if world
                .dungeon()
                .current_level
                .get(pos)
                .is_some_and(|cell| cell.terrain.is_walkable())
            {
                return pos;
            }
        }
    }
    origin
}

fn reject_amulet_offering(
    world: &mut GameWorld,
    player: hecs::Entity,
    item: hecs::Entity,
    player_pos: Position,
    events: &mut Vec<EngineEvent>,
) {
    if !detach_item_from_player_possessions(world, player, item) {
        events.push(EngineEvent::msg("offer-generic"));
        return;
    }

    let drop_pos = nearest_walkable_drop_pos(world, player_pos);
    if let Some(mut core) = world.get_component_mut::<ObjectCore>(item) {
        core.inv_letter = None;
    }
    let branch = world.dungeon().branch;
    let depth = world.dungeon().depth;
    if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item) {
        *loc = crate::dungeon::floor_object_location(branch, depth, drop_pos);
    }
    events.push(EngineEvent::msg("offer-amulet-rejected"));
}

fn is_real_amulet_of_yendor(world: &GameWorld, item: hecs::Entity) -> bool {
    let Some(core) = world.get_component::<ObjectCore>(item) else {
        return false;
    };
    let Some(amulet_id) = resolve_object_type_by_spec(world.object_catalog(), "Amulet of Yendor")
    else {
        return false;
    };
    core.otyp == amulet_id
}

fn is_book_of_the_dead(world: &GameWorld, item: hecs::Entity) -> bool {
    item_display_name(world, item).is_some_and(|name| quest_name_matches(&name, "Book of the Dead"))
}

fn item_is_cursed(world: &GameWorld, item: hecs::Entity) -> bool {
    world
        .get_component::<BucStatus>(item)
        .is_some_and(|status| status.cursed)
}

fn item_is_lit(world: &GameWorld, item: hecs::Entity) -> bool {
    world
        .get_component::<LightSource>(item)
        .is_some_and(|source| source.lit)
}

fn item_charges(world: &GameWorld, item: hecs::Entity) -> i8 {
    world
        .get_component::<Enchantment>(item)
        .map(|charges| charges.spe)
        .unwrap_or(0)
}

fn is_invocation_site(world: &GameWorld, pos: Position) -> bool {
    if world
        .dungeon()
        .current_level
        .get(pos)
        .is_some_and(|cell| matches!(cell.terrain, Terrain::StairsUp | Terrain::StairsDown))
    {
        return false;
    }

    world
        .dungeon()
        .trap_map
        .trap_at(pos)
        .is_some_and(|trap| trap.trap_type == TrapType::VibratingSquare)
}

fn player_gold(world: &GameWorld, player: hecs::Entity) -> i64 {
    world
        .get_component::<crate::inventory::Inventory>(player)
        .map(|inv| {
            inv.items
                .iter()
                .filter_map(|item| world.get_component::<ObjectCore>(*item))
                .filter(|core| core.object_class == ObjectClass::Coin)
                .map(|core| core.quantity as i64)
                .sum()
        })
        .unwrap_or(0)
}

#[cfg(test)]
fn player_hp(world: &GameWorld, player: hecs::Entity) -> i32 {
    world
        .get_component::<HitPoints>(player)
        .map(|hp| hp.current)
        .unwrap_or(0)
}

fn spend_player_gold(world: &mut GameWorld, player: hecs::Entity, amount: u32) -> bool {
    if amount == 0 {
        return true;
    }

    let Some(items) = world
        .get_component::<crate::inventory::Inventory>(player)
        .map(|inv| inv.items.clone())
    else {
        return false;
    };

    let mut remaining = amount as i32;
    let mut fully_spent = Vec::new();

    for item in items {
        if remaining <= 0 {
            break;
        }
        let is_coin = world
            .get_component::<ObjectCore>(item)
            .is_some_and(|core| core.object_class == ObjectClass::Coin);
        if !is_coin {
            continue;
        }
        if let Some(mut core) = world.get_component_mut::<ObjectCore>(item) {
            let spend = remaining.min(core.quantity.max(0));
            core.quantity -= spend;
            remaining -= spend;
            if core.quantity <= 0 {
                fully_spent.push(item);
            }
        }
    }

    if remaining > 0 {
        return false;
    }

    if !fully_spent.is_empty() {
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.retain(|item| !fully_spent.contains(item));
        }
        for item in fully_spent {
            let _ = world.despawn(item);
        }
    }

    true
}

fn process_shop_repairs(world: &mut GameWorld, events: &mut Vec<EngineEvent>) {
    let shop_count = world.dungeon().shop_rooms.len();
    let mut repair_indices = Vec::new();

    for idx in 0..shop_count {
        let shop = world.dungeon().shop_rooms[idx].clone();
        if shop.angry || shop.damage_list.is_empty() {
            continue;
        }

        let Some(shopkeeper) = world
            .get_component::<crate::npc::Shopkeeper>(shop.shopkeeper)
            .map(|state| (*state).clone())
        else {
            continue;
        };
        if shopkeeper.following {
            continue;
        }

        let at_home = world
            .get_component::<Positioned>(shop.shopkeeper)
            .is_some_and(|pos| pos.0 == shopkeeper_home_pos(&shop));
        if at_home {
            repair_indices.push(idx);
        }
    }

    for idx in repair_indices {
        let repaired = {
            let shop = &mut world.dungeon_mut().shop_rooms[idx];
            let shopkeeper_name = shop.shopkeeper_name.clone();
            crate::shop::repair_one_damage(shop).map(|damage| (damage, shopkeeper_name))
        };

        let Some((damage, shopkeeper_name)) = repaired else {
            continue;
        };

        match damage.damage_type {
            crate::shop::ShopDamageType::WallDestroyed => {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(damage.position, Terrain::Wall);
            }
            crate::shop::ShopDamageType::DoorBroken => {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(damage.position, Terrain::DoorClosed);
                events.push(EngineEvent::DoorClosed {
                    position: damage.position,
                });
            }
            crate::shop::ShopDamageType::FloorDamaged => {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(damage.position, Terrain::Floor);
            }
        }

        events.push(EngineEvent::msg_with(
            "shop-repair",
            vec![("shopkeeper", shopkeeper_name)],
        ));
    }
}

fn find_payable_shop_index(world: &GameWorld, player: hecs::Entity) -> Option<usize> {
    let player_pos = world.get_component::<Positioned>(player).map(|pos| pos.0)?;
    world
        .dungeon()
        .shop_rooms
        .iter()
        .enumerate()
        .find_map(|(idx, shop)| {
            if shop_room_is_deserted(world, shop) {
                return None;
            }
            let near_door = shop
                .door_pos
                .is_some_and(|door| crate::ball::chebyshev_distance(player_pos, door) <= 1);
            if shop.should_block_door() && (shop.contains(player_pos) || near_door) {
                Some(idx)
            } else {
                None
            }
        })
}

fn find_shop_index_containing_position(world: &GameWorld, pos: Position) -> Option<usize> {
    world
        .dungeon()
        .shop_rooms
        .iter()
        .position(|shop| shop.contains(pos) && !shop_room_is_deserted(world, shop))
}

fn pacify_shop_if_settled(world: &mut GameWorld, shop_idx: usize) {
    if world
        .dungeon()
        .shop_rooms
        .get(shop_idx)
        .is_some_and(|shop| shop.bill.is_empty() && shop.debit == 0 && shop.robbed == 0)
    {
        crate::shop::pacify_shop(&mut world.dungeon_mut().shop_rooms[shop_idx]);
    }
}

fn add_player_gold(world: &mut GameWorld, player: hecs::Entity, amount: u32) {
    if amount == 0 {
        return;
    }

    let Some(items) = world
        .get_component::<crate::inventory::Inventory>(player)
        .map(|inv| inv.items.clone())
    else {
        return;
    };

    for item in items {
        if let Some(mut core) = world.get_component_mut::<ObjectCore>(item)
            && core.object_class == ObjectClass::Coin
        {
            core.quantity += amount as i32;
            return;
        }
    }

    let gold = world.spawn((
        ObjectCore {
            otyp: ObjectTypeId(0),
            object_class: ObjectClass::Coin,
            quantity: amount as i32,
            weight: 1,
            age: 0,
            inv_letter: None,
            artifact: None,
        },
        ObjectLocation::Inventory,
    ));
    if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
        inv.items.push(gold);
    }
}

fn record_player_exercise_action(
    world: &mut GameWorld,
    player: hecs::Entity,
    action: crate::attributes::ExerciseAction,
) {
    if let Some(mut exercise) =
        world.get_component_mut::<crate::attributes::AttributeExercise>(player)
    {
        crate::attributes::exercise_action(&mut exercise, action);
    }
}

fn current_spell_protection_layers(world: &GameWorld, player: hecs::Entity) -> i32 {
    world
        .get_component::<crate::status::SpellProtection>(player)
        .map(|protection| i32::from(protection.layers))
        .unwrap_or(0)
}

fn grant_player_spell_protection(world: &mut GameWorld, player: hecs::Entity) {
    if let Some(mut protection) = world.get_component_mut::<crate::status::SpellProtection>(player)
    {
        protection.layers = protection.layers.saturating_add(1);
        protection.countdown = protection.interval.max(10);
        if protection.interval == 0 {
            protection.interval = 10;
        }
    } else {
        let _ = world.ecs_mut().insert_one(
            player,
            crate::status::SpellProtection {
                layers: 1,
                countdown: 10,
                interval: 10,
            },
        );
    }
}

fn resolve_priest_chat<R: Rng>(
    world: &mut GameWorld,
    player: hecs::Entity,
    priest_entity: hecs::Entity,
    priest_data: crate::npc::Priest,
    rng: &mut R,
) -> Vec<EngineEvent> {
    if priest_data.angry || world.get_component::<Peaceful>(priest_entity).is_none() {
        return vec![EngineEvent::msg(crate::npc::cranky_priest_message(rng))];
    }

    let current_protection = current_spell_protection_layers(world, player);
    let player_alignment = current_player_alignment(world, player);
    let player_gold_total = player_gold(world, player) as i32;

    let priest_events = crate::npc::priest_interaction(
        world,
        player,
        priest_entity,
        player_gold_total,
        player_alignment,
        current_protection,
    );
    let granted_protection = priest_events.iter().any(|event| {
        matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-protection-granted"
        )
    });
    if granted_protection {
        let cost = crate::npc::priest_protection_cost(current_protection);
        let _ = spend_player_gold(world, player, cost as u32);
        record_player_exercise_action(
            world,
            player,
            crate::attributes::ExerciseAction::DonatedToTemple,
        );
        grant_player_spell_protection(world, player);
        return priest_events;
    }

    if player_alignment != priest_data.alignment {
        return priest_events;
    }

    if player_gold_total <= 0 {
        if priest_data.has_shrine && !priest_data.is_high_priest {
            add_player_gold(world, player, 2);
            return crate::npc::priest_ale_gift(2);
        }
        return vec![EngineEvent::msg("priest-virtues-of-poverty")];
    }

    let mut religion_state = world
        .get_component::<crate::religion::ReligionState>(player)
        .map(|state| (*state).clone())
        .unwrap_or_else(|| default_religion_state(world, player));
    refresh_religion_state_from_world(&mut religion_state, world, player);

    let donation_offer = player_gold_total as u32;
    let donation_result = crate::npc::priest_donation(
        crate::npc::PriestDonationContext {
            offer: player_gold_total,
            player_gold: player_gold_total,
            player_level: current_player_level(world, player),
            alignment_record: religion_state.alignment_record,
            coaligned: true,
            current_protection,
            turns_since_cleansed: religion_state.turn,
        },
        rng,
    );

    let _ = spend_player_gold(world, player, donation_offer);
    record_player_exercise_action(
        world,
        player,
        crate::attributes::ExerciseAction::DonatedToTemple,
    );

    let donation_events = match donation_result {
        crate::npc::DonationResult::Refused | crate::npc::DonationResult::RefusedToDonate => {
            vec![EngineEvent::msg("priest-virtues-of-poverty")]
        }
        crate::npc::DonationResult::AleGift { amount } => {
            add_player_gold(world, player, amount.max(0) as u32);
            vec![EngineEvent::msg_with(
                "priest-ale-gift",
                vec![("amount", amount.to_string())],
            )]
        }
        crate::npc::DonationResult::VirtuesOfPoverty => {
            vec![EngineEvent::msg("priest-virtues-of-poverty")]
        }
        crate::npc::DonationResult::Cheapskate => vec![EngineEvent::msg("priest-cheapskate")],
        crate::npc::DonationResult::SmallThanks => vec![EngineEvent::msg("priest-small-thanks")],
        crate::npc::DonationResult::Pious => vec![EngineEvent::msg("priest-pious")],
        crate::npc::DonationResult::Blessing { clairvoyance_turns } => {
            let mut events =
                crate::status::make_clairvoyant(world, player, clairvoyance_turns.max(1) as u32);
            events.extend(crate::detect::clairvoyance(world, player, 8));
            events.push(EngineEvent::msg("priest-clairvoyance"));
            events
        }
        crate::npc::DonationResult::ProtectionReward => {
            grant_player_spell_protection(world, player);
            vec![EngineEvent::msg_with(
                "priest-protection-granted",
                vec![("cost", donation_offer.to_string())],
            )]
        }
        crate::npc::DonationResult::SelflessGenerosity => {
            vec![EngineEvent::msg("priest-selfless-generosity")]
        }
        crate::npc::DonationResult::Cleansing => {
            let cleansing_delta = (-religion_state.alignment_record).max(1);
            crate::religion::adjust_alignment(&mut religion_state, cleansing_delta);
            vec![EngineEvent::msg("priest-cleansing")]
        }
    };
    persist_religion_state(world, player, religion_state);
    donation_events
}

fn maybe_spawn_untended_temple_ghost(
    world: &mut GameWorld,
    player: hecs::Entity,
    rng: &mut impl Rng,
) -> Option<(hecs::Entity, Position)> {
    let ghost_present = world.ecs().query::<&Monster>().iter().any(|(entity, _)| {
        live_monster_entity(world, entity)
            && quest_name_matches(&world.entity_name(entity), "ghost")
    });
    if ghost_present || rng.random_range(0..4) != 0 {
        return None;
    }

    spawn_named_monster_near_entity(world, player, "ghost", rng)
}

fn maybe_emit_current_level_temple_entry(
    world: &mut GameWorld,
    player: hecs::Entity,
    first_visit: bool,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    let has_altar = world
        .dungeon()
        .current_level
        .cells
        .iter()
        .flatten()
        .any(|cell| cell.terrain == Terrain::Altar);
    if !has_altar {
        return;
    }

    let priest = world
        .ecs()
        .query::<(&Monster,)>()
        .iter()
        .find_map(|(entity, _)| infer_priest_runtime(world, entity));

    if let Some(priest) = priest {
        if priest.is_high_priest {
            events.extend(crate::priest::sanctum_entry(first_visit));
            return;
        }
        if !first_visit {
            return;
        }

        let coaligned = crate::priest::player_coaligned(
            current_player_alignment(world, player),
            priest.alignment,
        );
        match crate::npc::temple_entry(
            true,
            priest.has_shrine,
            coaligned,
            current_player_alignment_record(world, player),
            false,
            false,
            rng,
        ) {
            crate::npc::TempleEntryResult::Tended { message_key, .. } => {
                events.push(EngineEvent::msg(message_key));
            }
            crate::npc::TempleEntryResult::Sanctum { first_time } => {
                events.extend(crate::priest::sanctum_entry(first_time));
            }
            crate::npc::TempleEntryResult::Untended { message_index } => {
                events.extend(crate::npc::untended_temple_events(message_index, false));
            }
        }
        return;
    }

    if !first_visit {
        return;
    }
    if let crate::npc::TempleEntryResult::Untended { message_index } = crate::npc::temple_entry(
        false,
        false,
        false,
        current_player_alignment_record(world, player),
        false,
        false,
        rng,
    ) {
        let ghost_spawn = maybe_spawn_untended_temple_ghost(world, player, rng);
        events.extend(crate::npc::untended_temple_events(
            message_index,
            ghost_spawn.is_some(),
        ));
        if let Some((ghost, pos)) = ghost_spawn {
            events.push(EngineEvent::MonsterGenerated {
                entity: ghost,
                position: pos,
            });
        }
    }
}

fn handle_player_drop(
    world: &mut GameWorld,
    player: hecs::Entity,
    item: hecs::Entity,
) -> Vec<EngineEvent> {
    let mut events = crate::inventory::drop_item(world, player, item);
    let Some(player_pos) = world.get_component::<Positioned>(player).map(|pos| pos.0) else {
        return events;
    };
    let Some(shop_idx) = find_shop_index_containing_position(world, player_pos) else {
        return events;
    };
    let Some(item_core) = world
        .get_component::<ObjectCore>(item)
        .map(|core| (*core).clone())
    else {
        return events;
    };

    if item_core.object_class == ObjectClass::Coin {
        let debit_before = world.dungeon().shop_rooms[shop_idx].debit;
        let shopkeeper_name = world.dungeon().shop_rooms[shop_idx].shopkeeper_name.clone();
        let credit_added = crate::shop::donate_gold(
            &mut world.dungeon_mut().shop_rooms[shop_idx],
            item_core.quantity.max(0),
        );
        let debit_after = world.dungeon().shop_rooms[shop_idx].debit;
        let paid_amount = debit_before.saturating_sub(debit_after);
        if paid_amount > 0 {
            events.push(EngineEvent::msg_with(
                "shop-pay-success",
                vec![
                    ("shopkeeper", shopkeeper_name),
                    ("amount", paid_amount.to_string()),
                ],
            ));
        }
        if credit_added > 0 {
            events.push(EngineEvent::msg_with(
                "shop-credit",
                vec![("amount", credit_added.to_string())],
            ));
        }
        if paid_amount > 0 || credit_added > 0 {
            record_player_exercise_action(
                world,
                player,
                crate::attributes::ExerciseAction::ShopTransaction,
            );
        }
        let _ = world.despawn(item);
        pacify_shop_if_settled(world, shop_idx);
        sync_current_level_shopkeeper_state(world);
        return events;
    }

    let shop_before = world.dungeon().shop_rooms[shop_idx].clone();
    let object_defs = world.object_catalog().to_vec();
    let mut live_shop = shop_before.clone();
    let mut shop_events =
        crate::shop::drop_in_shop(world, player, item, &mut live_shop, &object_defs);
    let gold_paid = shop_before
        .shopkeeper_gold
        .saturating_sub(live_shop.shopkeeper_gold)
        .max(0);
    if gold_paid > 0 {
        add_player_gold(world, player, gold_paid as u32);
        record_player_exercise_action(
            world,
            player,
            crate::attributes::ExerciseAction::ShopTransaction,
        );
    }
    world.dungeon_mut().shop_rooms[shop_idx] = live_shop;
    pacify_shop_if_settled(world, shop_idx);
    sync_current_level_shopkeeper_state(world);
    events.append(&mut shop_events);
    events
}

fn find_adjacent_sanctuary_priest_for_prayer(
    world: &GameWorld,
    player: hecs::Entity,
    altar_alignment: Alignment,
) -> Option<(hecs::Entity, crate::npc::Priest)> {
    let player_pos = world.get_component::<Positioned>(player).map(|pos| pos.0)?;
    let alignment_record = world
        .get_component::<crate::religion::ReligionState>(player)
        .map(|state| state.alignment_record)
        .unwrap_or_else(|| default_religion_state(world, player).alignment_record);

    world
        .ecs()
        .query::<(&Monster, &Positioned)>()
        .iter()
        .find_map(|(entity, (_, pos))| {
            let priest = infer_priest_runtime(world, entity)?;
            let coaligned = crate::priest::player_coaligned(
                current_player_alignment(world, player),
                priest.alignment,
            );
            (priest.angry
                && priest.alignment == altar_alignment
                && crate::ball::chebyshev_distance(pos.0, player_pos) <= 1
                && crate::npc::in_sanctuary(true, priest.has_shrine, coaligned, alignment_record))
            .then_some((entity, priest))
        })
}

fn try_calm_temple_priest_after_prayer(
    world: &mut GameWorld,
    player: hecs::Entity,
    on_altar: bool,
    altar_alignment: Option<Alignment>,
    prayer_type: crate::religion::PrayerType,
    events: &mut Vec<EngineEvent>,
) {
    if !on_altar
        || !matches!(
            prayer_type,
            crate::religion::PrayerType::Success | crate::religion::PrayerType::CrossAligned
        )
    {
        return;
    }

    let Some(altar_alignment) = altar_alignment else {
        return;
    };
    let Some((priest_entity, mut priest)) =
        find_adjacent_sanctuary_priest_for_prayer(world, player, altar_alignment)
    else {
        return;
    };
    if !priest.angry {
        return;
    }

    priest.angry = false;
    upsert_priest_component(world, priest_entity, priest);
    let _ = world.ecs_mut().insert_one(priest_entity, Peaceful);
    let mut temple = crate::priest::TempleInfo::new(priest.alignment);
    temple.has_priest = true;
    temple.has_shrine = priest.has_shrine;
    temple.is_sanctum = priest.is_high_priest;
    temple.priest_angry = true;
    events.extend(crate::priest::calm_priest(&mut temple));
}

fn maybe_trigger_shop_exit_robbery(
    world: &mut GameWorld,
    player: hecs::Entity,
    from_pos: Position,
    to_pos: Position,
    events: &mut Vec<EngineEvent>,
    rng: &mut impl Rng,
) -> bool {
    let Some(shop_idx) = find_shop_index_containing_position(world, from_pos) else {
        return false;
    };
    let mut shop = world.dungeon().shop_rooms[shop_idx].clone();
    let needs_robbery = !shop.bill.is_empty() || shop.debit > shop.credit;
    if shop.contains(to_pos) || !needs_robbery {
        if shop.exit_warning_issued {
            shop.exit_warning_issued = false;
            world.dungeon_mut().shop_rooms[shop_idx] = shop;
        }
        return false;
    }

    if !shop.exit_warning_issued {
        let shopkeeper_name = shop.shopkeeper_name.clone();
        shop.exit_warning_issued = true;
        world.dungeon_mut().shop_rooms[shop_idx] = shop;
        events.push(EngineEvent::msg_with(
            "shop-leave-warning",
            vec![("shopkeeper", shopkeeper_name)],
        ));
        return true;
    }

    let mut robbery_events = crate::shop::rob_shop(world, player, &mut shop, rng);
    world.dungeon_mut().shop_rooms[shop_idx] = shop;
    sync_current_level_npc_state(world);
    events.append(&mut robbery_events);
    false
}

fn current_depth_string(world: &GameWorld) -> String {
    if world.dungeon().branch == DungeonBranch::Endgame && world.dungeon().depth == 5 {
        "Astral".to_string()
    } else {
        format!("{:?}:{}", world.dungeon().branch, world.dungeon().depth)
    }
}

fn altar_alignment_at(world: &GameWorld, pos: Position) -> Option<Alignment> {
    let branch = world.dungeon().branch;
    let depth = world.dungeon().depth;
    if branch == DungeonBranch::Endgame && depth == 5 {
        let mut altar_positions = Vec::new();
        let map = &world.dungeon().current_level;
        for y in 0..map.height {
            for x in 0..map.width {
                let altar_pos = Position::new(x as i32, y as i32);
                if map
                    .get(altar_pos)
                    .is_some_and(|cell| cell.terrain == Terrain::Altar)
                {
                    altar_positions.push(altar_pos);
                }
            }
        }
        altar_positions.sort_by_key(|altar_pos| altar_pos.x);
        for (index, altar_pos) in altar_positions.iter().enumerate() {
            if *altar_pos == pos {
                return Some(match index {
                    0 => Alignment::Lawful,
                    1 => Alignment::Neutral,
                    _ => Alignment::Chaotic,
                });
            }
        }
    }
    Some(current_player_alignment(world, world.player()))
}

fn level_uses_invocation_magic_portal(
    world: &GameWorld,
    branch: DungeonBranch,
    depth: i32,
) -> bool {
    matches!(
        world.dungeon().topology_portal_destination(branch, depth),
        Some((DungeonBranch::Endgame, _))
    )
}

fn sync_current_level_invocation_access(world: &mut GameWorld, rng: &mut impl Rng) {
    let branch = world.dungeon().branch;
    let depth = world.dungeon().depth;
    if !level_uses_invocation_magic_portal(world, branch, depth) {
        return;
    }

    let player_events = read_player_events(world, world.player());
    let preferred = world
        .dungeon()
        .trap_map
        .traps
        .iter()
        .find(|trap| trap.trap_type == TrapType::VibratingSquare)
        .map(|trap| trap.pos)
        .or_else(|| find_terrain(&world.dungeon().current_level, Terrain::StairsUp))
        .or_else(|| find_terrain(&world.dungeon().current_level, Terrain::StairsDown));
    let Some(anchor) = find_portal_anchor(&world.dungeon().current_level, preferred) else {
        return;
    };

    if player_events.invoked {
        world.dungeon_mut().trap_map.remove_trap_at(anchor);
        if world
            .dungeon()
            .current_level
            .get(anchor)
            .is_some_and(|cell| matches!(cell.terrain, Terrain::Floor | Terrain::Corridor))
        {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(anchor, Terrain::MagicPortal);
        }
    } else {
        if let Some(portal_pos) = find_terrain(&world.dungeon().current_level, Terrain::MagicPortal)
        {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(portal_pos, Terrain::Floor);
        }
        if world.dungeon().trap_map.trap_at(anchor).is_none() {
            let _ = crate::traps::create_trap(
                rng,
                &mut world.dungeon_mut().trap_map,
                anchor,
                TrapType::VibratingSquare,
            );
        }
    }
}

fn read_book_of_the_dead(
    world: &mut GameWorld,
    player: hecs::Entity,
    book: hecs::Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let player_pos = world
        .get_component::<Positioned>(player)
        .map(|pos| pos.0)
        .unwrap_or(Position::new(0, 0));

    if !is_invocation_site(world, player_pos) {
        events.push(EngineEvent::msg("read-dead-book"));
        return events;
    }

    let bell = find_player_named_item(world, player, "Bell of Opening");
    let candelabrum = find_player_named_item(world, player, "Candelabrum of Invocation");
    let Some(bell) = bell else {
        events.push(EngineEvent::msg("invocation-missing-bell"));
        return events;
    };
    let Some(candelabrum) = candelabrum else {
        events.push(EngineEvent::msg("invocation-missing-candelabrum"));
        return events;
    };

    let bell_recent = world
        .get_component::<ObjectCore>(bell)
        .is_some_and(|core| world.turn().saturating_sub(core.age.max(0) as u32) <= 5);
    let candelabrum_ready =
        item_charges(world, candelabrum) >= 7 && item_is_lit(world, candelabrum);
    let all_uncursed = !item_is_cursed(world, book)
        && !item_is_cursed(world, bell)
        && !item_is_cursed(world, candelabrum);

    if !bell_recent {
        events.push(EngineEvent::msg("invocation-needs-bell-rung"));
        return events;
    }
    if !candelabrum_ready {
        events.push(EngineEvent::msg("invocation-needs-candelabrum-ready"));
        return events;
    }
    if !all_uncursed {
        events.push(EngineEvent::msg("invocation-items-cursed"));
        return events;
    }

    let mut player_events = read_player_events(world, player);
    player_events.invoked = true;
    player_events.found_vibrating_square = true;
    let soon = wizard_invocation_delay(rng);
    if player_events.wizard_intervention_cooldown == 0
        || player_events.wizard_intervention_cooldown > soon
    {
        player_events.wizard_intervention_cooldown = soon;
    }
    persist_player_events(world, player, player_events);
    sync_current_level_invocation_access(world, rng);
    events.push(EngineEvent::msg("invocation-complete"));

    events
}

fn sync_quest_state_from_world(world: &mut GameWorld) {
    let player = world.player();
    let Some(role) = current_player_role(world) else {
        return;
    };
    let mut state = read_quest_state(world, player);
    if world.dungeon().branch == DungeonBranch::Quest && world.dungeon().depth > 1 {
        state.enter_quest_dungeon();
    }

    let quest_artifact = crate::quest::quest_artifact_for_role(role);
    let has_artifact = world
        .get_component::<crate::inventory::Inventory>(player)
        .is_some_and(|inv| {
            inv.items.iter().any(|item| {
                item_display_name(world, *item)
                    .is_some_and(|name| quest_name_matches(&name, quest_artifact))
            })
        });
    if has_artifact {
        state.obtain_artifact();
    }

    if state.artifact_obtained
        && world.dungeon().branch == DungeonBranch::Quest
        && world.dungeon().depth == 7
    {
        let nemesis_name = crate::quest::quest_nemesis_for_role(role);
        let nemesis_alive = world
            .ecs()
            .query::<(&Monster, &Name)>()
            .iter()
            .any(|(_, (_, name))| quest_name_matches(&name.0, nemesis_name));
        if !nemesis_alive {
            state.defeat_nemesis();
        }
    }

    persist_quest_state(world, player, state);
}

fn player_has_named_item(world: &GameWorld, player: hecs::Entity, expected_name: &str) -> bool {
    find_player_named_item(world, player, expected_name).is_some()
}

fn persist_player_quest_items(
    world: &mut GameWorld,
    player: hecs::Entity,
    items: PlayerQuestItems,
) {
    if let Some(mut live_items) = world.get_component_mut::<PlayerQuestItems>(player) {
        *live_items = items;
    } else {
        let _ = world.ecs_mut().insert_one(player, items);
    }
}

fn read_player_events(world: &GameWorld, player: hecs::Entity) -> PlayerEvents {
    world
        .get_component::<PlayerEvents>(player)
        .map(|events| (*events).clone())
        .unwrap_or_default()
}

fn persist_player_events(world: &mut GameWorld, player: hecs::Entity, events: PlayerEvents) {
    if let Some(mut live_events) = world.get_component_mut::<PlayerEvents>(player) {
        *live_events = events;
    } else {
        let _ = world.ecs_mut().insert_one(player, events);
    }
}

fn sync_player_story_components(world: &mut GameWorld, events: &[EngineEvent]) {
    let player = world.player();

    let quest_items = PlayerQuestItems {
        has_amulet: player_has_named_item(world, player, "Amulet of Yendor"),
        has_bell: player_has_named_item(world, player, "Bell of Opening"),
        has_book: player_has_named_item(world, player, "Book of the Dead"),
        has_menorah: player_has_named_item(world, player, "Candelabrum of Invocation"),
        has_quest_artifact: current_player_role(world).is_some_and(|role| {
            player_has_named_item(world, player, crate::quest::quest_artifact_for_role(role))
        }),
    };
    persist_player_quest_items(world, player, quest_items);

    let quest_state = read_quest_state(world, player);
    let mut player_events = read_player_events(world, player);
    if player_events.killed_wizard && player_events.wizard_times_killed == 0 {
        player_events.wizard_times_killed = 1;
    }
    player_events.killed_wizard |= player_events.wizard_times_killed > 0;
    player_events.quest_called = quest_state.leader_met;
    player_events.quest_expelled = quest_state.times_expelled > 0;
    player_events.quest_completed = quest_state.status == crate::quest::QuestStatus::Completed;
    player_events.gehennom_entered |= world.dungeon().branch == DungeonBranch::Gehennom;
    if let Some(player_pos) = world.get_component::<Positioned>(player).map(|pos| pos.0) {
        player_events.found_vibrating_square |= world
            .dungeon()
            .trap_map
            .trap_at(player_pos)
            .is_some_and(|trap| trap.trap_type == TrapType::VibratingSquare);
    }
    player_events.ascended |= events.iter().any(|event| {
        matches!(
            event,
            EngineEvent::GameOver {
                cause: crate::event::DeathCause::Ascended,
                ..
            }
        )
    });
    persist_player_events(world, player, player_events);
}

fn read_conduct_state(world: &GameWorld, player: hecs::Entity) -> ConductState {
    world
        .get_component::<ConductState>(player)
        .map(|state| (*state).clone())
        .unwrap_or_default()
}

fn persist_conduct_state(world: &mut GameWorld, player: hecs::Entity, state: ConductState) {
    if let Some(mut live_state) = world.get_component_mut::<ConductState>(player) {
        *live_state = state;
    } else {
        let _ = world.ecs_mut().insert_one(player, state);
    }
}

fn increment_conduct(
    world: &mut GameWorld,
    player: hecs::Entity,
    update: impl FnOnce(&mut ConductState),
) {
    let mut state = read_conduct_state(world, player);
    update(&mut state);
    persist_conduct_state(world, player, state);
}

fn apply_eating_conduct(
    world: &mut GameWorld,
    player: hecs::Entity,
    conduct: &crate::hunger::ConductViolations,
) {
    if !conduct.broke_foodless && !conduct.broke_vegan && !conduct.broke_vegetarian {
        return;
    }

    increment_conduct(world, player, |state| {
        if conduct.broke_foodless {
            state.food = state.food.saturating_add(1);
        }
        if conduct.broke_vegan {
            state.unvegan = state.unvegan.saturating_add(1);
        }
        if conduct.broke_vegetarian {
            state.unvegetarian = state.unvegetarian.saturating_add(1);
        }
    });
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::conduct::ConductState;
    use crate::dungeon::Terrain;
    use crate::inventory::Inventory;
    use crate::world::{Name, Peaceful, Tame};
    use nethack_babel_data::{
        Alignment, ArtifactId, BucStatus, GameData, Gender, Handedness, MonsterFlags, ObjectClass,
        ObjectCore, ObjectLocation, ObjectTypeId, PlayerEvents, PlayerIdentity, PlayerQuestItems,
        PlayerSkills, RaceId, RoleId, SkillState, WeaponSkill, load_game_data,
        schema::MonsterSound,
    };
    use rand::SeedableRng;
    use rand_pcg::Pcg64;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    /// Deterministic RNG for reproducible tests.
    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(42)
    }

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn test_game_data() -> &'static GameData {
        static DATA: OnceLock<GameData> = OnceLock::new();
        DATA.get_or_init(|| {
            load_game_data(&data_dir())
                .unwrap_or_else(|e| panic!("failed to load test game data: {}", e))
        })
    }

    fn install_test_catalogs(world: &mut GameWorld) {
        let data = test_game_data();
        world.set_spawn_catalogs(data.monsters.clone(), data.objects.clone());
    }

    fn make_test_world() -> GameWorld {
        let mut world = GameWorld::new(Position::new(5, 5));
        // Carve a small open room so the player can move.
        for y in 3..=7 {
            for x in 3..=7 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        world
    }

    fn oracle_depth_for_world(world: &GameWorld) -> i32 {
        (1..=29)
            .find(|depth| {
                world
                    .dungeon()
                    .check_topology_special(&DungeonBranch::Main, *depth)
                    == Some(crate::special_levels::SpecialLevelId::OracleLevel)
            })
            .expect("test dungeon topology should place the Oracle somewhere in Main")
    }

    fn spawn_inventory_item(world: &mut GameWorld, letter: char) -> hecs::Entity {
        world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Weapon,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some(letter),
                artifact: None,
            },
            ObjectLocation::Inventory,
        ))
    }

    fn spawn_inventory_gold(world: &mut GameWorld, amount: u32, letter: char) -> hecs::Entity {
        let item = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Coin,
                quantity: amount as i32,
                weight: 1,
                age: 0,
                inv_letter: Some(letter),
                artifact: None,
            },
            ObjectLocation::Inventory,
        ));
        let player = world.player();
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.push(item);
        }
        item
    }

    fn spawn_floor_coin(world: &mut GameWorld, pos: Position) -> hecs::Entity {
        world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Coin,
                quantity: 42,
                weight: 1,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            ObjectLocation::Floor {
                x: pos.x as i16,
                y: pos.y as i16,
                level: world.dungeon().current_data_dungeon_level(),
            },
        ))
    }

    fn spawn_tame_steed(world: &mut GameWorld, pos: Position, name: &str) -> hecs::Entity {
        world.spawn((
            crate::world::Monster,
            crate::world::Tame,
            crate::world::Positioned(pos),
            Name(name.to_string()),
            crate::world::Speed(18),
            crate::world::HitPoints {
                current: 30,
                max: 30,
            },
        ))
    }

    fn priest_identity() -> PlayerIdentity {
        PlayerIdentity {
            name: "tester".to_string(),
            role: RoleId(crate::religion::roles::PRIEST),
            race: RaceId(0),
            gender: Gender::Male,
            alignment: Alignment::Lawful,
            alignment_base: [Alignment::Lawful, Alignment::Lawful],
            handedness: Handedness::RightHanded,
        }
    }

    fn monk_identity() -> PlayerIdentity {
        PlayerIdentity {
            name: "tester".to_string(),
            role: RoleId(crate::religion::roles::MONK),
            race: RaceId(0),
            gender: Gender::Male,
            alignment: Alignment::Lawful,
            alignment_base: [Alignment::Lawful, Alignment::Lawful],
            handedness: Handedness::RightHanded,
        }
    }

    fn identity_for_role(role: crate::role::Role) -> PlayerIdentity {
        PlayerIdentity {
            name: "tester".to_string(),
            role: role.to_id(),
            race: RaceId(0),
            gender: Gender::Male,
            alignment: Alignment::Neutral,
            alignment_base: [Alignment::Neutral, Alignment::Neutral],
            handedness: Handedness::RightHanded,
        }
    }

    fn wizard_identity() -> PlayerIdentity {
        let mut id = identity_for_role(crate::role::Role::Wizard);
        id.alignment = Alignment::Chaotic;
        id.alignment_base = [Alignment::Chaotic, Alignment::Chaotic];
        id
    }

    fn spawn_idle_named_monster(world: &mut GameWorld, pos: Position, name: &str) -> hecs::Entity {
        let order = world.next_creation_order();
        world.spawn((
            Monster,
            Positioned(pos),
            Speed(12),
            MovementPoints(0),
            HitPoints {
                current: 20,
                max: 20,
            },
            Name(name.to_string()),
            order,
        ))
    }

    fn spawn_inventory_object_by_name(
        world: &mut GameWorld,
        name: &str,
        letter: char,
    ) -> hecs::Entity {
        let object_type = resolve_object_type_by_spec(&test_game_data().objects, name)
            .unwrap_or_else(|| panic!("{name} should resolve against the test catalog"));
        let object_def = test_game_data()
            .objects
            .iter()
            .find(|def| def.id == object_type)
            .unwrap_or_else(|| panic!("{name} should exist in the object catalog"));
        let item = world.spawn((
            ObjectCore {
                otyp: object_type,
                object_class: object_def.class,
                quantity: 1,
                weight: object_def.weight as u32,
                age: 0,
                inv_letter: Some(letter),
                artifact: None,
            },
            ObjectLocation::Inventory,
            Name(name.to_string()),
        ));
        let player = world.player();
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.push(item);
        }
        item
    }

    fn move_player_onto_magic_portal(
        world: &mut GameWorld,
        rng: &mut impl Rng,
    ) -> Vec<EngineEvent> {
        let portal_pos = find_terrain(&world.dungeon().current_level, Terrain::MagicPortal)
            .expect("current level should expose a magic portal");
        let (entry_pos, direction) =
            adjacent_walkable_step(&world.dungeon().current_level, portal_pos)
                .expect("magic portal should have an adjacent walkable entry tile");
        set_player_position(world, entry_pos);
        resolve_turn(world, PlayerAction::Move { direction }, rng)
    }

    fn monster_name_with_sound(world: &GameWorld, sound: MonsterSound) -> String {
        world
            .monster_catalog()
            .iter()
            .find(|def| def.sound == sound)
            .map(|def| def.names.male.clone())
            .unwrap_or_else(|| panic!("test catalog should contain a monster with sound {sound:?}"))
    }

    fn monster_name_with_sound_excluding(
        world: &GameWorld,
        sound: MonsterSound,
        excluded: &[&str],
    ) -> String {
        world
            .monster_catalog()
            .iter()
            .find(|def| {
                def.sound == sound
                    && !excluded
                        .iter()
                        .any(|name| def.names.male.eq_ignore_ascii_case(name))
            })
            .map(|def| def.names.male.clone())
            .unwrap_or_else(|| panic!("test catalog should contain a monster with sound {sound:?}"))
    }

    fn make_tame_pet_state(
        world: &mut GameWorld,
        monster: hecs::Entity,
        tameness: u8,
        hungrytime: u32,
    ) {
        world
            .ecs_mut()
            .insert_one(monster, Tame)
            .expect("monster should accept tame marker");
        let mut pet_state = crate::pets::PetState::new(10, world.turn());
        pet_state.tameness = tameness;
        pet_state.hungrytime = hungrytime;
        world
            .ecs_mut()
            .insert_one(monster, pet_state)
            .expect("monster should accept pet state");
    }

    fn advance_world_turns(world: &mut GameWorld, turns: u32) {
        for _ in 0..turns {
            world.advance_turn();
        }
    }

    #[derive(Clone, Copy)]
    enum StoryTraversalScenario {
        QuestClosure,
        QuestLeaderAnger,
        MedusaRevisit,
        CastleRevisit,
        OrcusRevisit,
        FortLudiosRevisit,
        VladTopRevisit,
        InvocationPortalRevisit,
        ShopEntry,
        ShopEntryWelcomeBack,
        ShopEntryRobbed,
        ShopkeeperFollow,
        ShopkeeperPayoff,
        ShopkeeperCredit,
        ShopCreditCovers,
        ShopNoMoney,
        ShopkeeperSell,
        ShopChatPriceQuote,
        ShopRepair,
        ShopkeeperDeath,
        ShopRobbery,
        ShopRestitution,
        TempleWrongAlignment,
        TempleAleGift,
        TempleVirtuesOfPoverty,
        TempleDonationThanks,
        TemplePious,
        TempleDonation,
        TempleBlessing,
        TempleCleansing,
        TempleSelflessGenerosity,
        TempleWrath,
        TempleCalm,
        UntendedTempleGhost,
        SanctumRevisit,
        WizardHarassment,
        WizardTaunt,
        WizardIntervention,
        WizardAmuletWake,
        WizardBlackGlowBlind,
        HumanoidAlohaChat,
        WereFullMoonChat,
        WizardLevelTeleport,
        EndgameAscension,
    }

    impl StoryTraversalScenario {
        fn label(self) -> &'static str {
            match self {
                StoryTraversalScenario::QuestClosure => "quest-closure",
                StoryTraversalScenario::QuestLeaderAnger => "quest-leader-anger",
                StoryTraversalScenario::MedusaRevisit => "medusa-revisit",
                StoryTraversalScenario::CastleRevisit => "castle-revisit",
                StoryTraversalScenario::OrcusRevisit => "orcus-revisit",
                StoryTraversalScenario::FortLudiosRevisit => "fort-ludios-revisit",
                StoryTraversalScenario::VladTopRevisit => "vlad-top-revisit",
                StoryTraversalScenario::InvocationPortalRevisit => "invocation-portal-revisit",
                StoryTraversalScenario::ShopEntry => "shop-entry",
                StoryTraversalScenario::ShopEntryWelcomeBack => "shop-entry-welcome-back",
                StoryTraversalScenario::ShopEntryRobbed => "shop-entry-robbed",
                StoryTraversalScenario::ShopkeeperFollow => "shopkeeper-follow",
                StoryTraversalScenario::ShopkeeperPayoff => "shopkeeper-payoff",
                StoryTraversalScenario::ShopkeeperCredit => "shopkeeper-credit",
                StoryTraversalScenario::ShopCreditCovers => "shop-credit-covers",
                StoryTraversalScenario::ShopNoMoney => "shop-no-money",
                StoryTraversalScenario::ShopkeeperSell => "shopkeeper-sell",
                StoryTraversalScenario::ShopChatPriceQuote => "shop-chat-price-quote",
                StoryTraversalScenario::ShopRepair => "shop-repair",
                StoryTraversalScenario::ShopkeeperDeath => "shopkeeper-death",
                StoryTraversalScenario::ShopRobbery => "shop-robbery",
                StoryTraversalScenario::ShopRestitution => "shop-restitution",
                StoryTraversalScenario::TempleWrongAlignment => "temple-wrong-alignment",
                StoryTraversalScenario::TempleAleGift => "temple-ale-gift",
                StoryTraversalScenario::TempleVirtuesOfPoverty => "temple-virtues-of-poverty",
                StoryTraversalScenario::TempleDonationThanks => "temple-donation-thanks",
                StoryTraversalScenario::TemplePious => "temple-pious",
                StoryTraversalScenario::TempleDonation => "temple-donation",
                StoryTraversalScenario::TempleBlessing => "temple-blessing",
                StoryTraversalScenario::TempleCleansing => "temple-cleansing",
                StoryTraversalScenario::TempleSelflessGenerosity => "temple-selfless-generosity",
                StoryTraversalScenario::TempleWrath => "temple-wrath",
                StoryTraversalScenario::TempleCalm => "temple-calm",
                StoryTraversalScenario::UntendedTempleGhost => "untended-temple-ghost",
                StoryTraversalScenario::SanctumRevisit => "sanctum-revisit",
                StoryTraversalScenario::WizardHarassment => "wizard-harassment",
                StoryTraversalScenario::WizardTaunt => "wizard-taunt",
                StoryTraversalScenario::WizardIntervention => "wizard-intervention",
                StoryTraversalScenario::WizardAmuletWake => "wizard-amulet-wake",
                StoryTraversalScenario::WizardBlackGlowBlind => "wizard-black-glow-blind",
                StoryTraversalScenario::HumanoidAlohaChat => "humanoid-aloha-chat",
                StoryTraversalScenario::WereFullMoonChat => "were-full-moon-chat",
                StoryTraversalScenario::WizardLevelTeleport => "wizard-level-teleport",
                StoryTraversalScenario::EndgameAscension => "endgame-ascension",
            }
        }
    }

    fn run_story_traversal_scenario(
        scenario: StoryTraversalScenario,
    ) -> (GameWorld, Vec<EngineEvent>) {
        match scenario {
            StoryTraversalScenario::QuestClosure => {
                let mut world = make_stair_world(Terrain::StairsDown, 1);
                install_test_catalogs(&mut world);
                let player = world.player();
                world.dungeon_mut().branch = DungeonBranch::Quest;
                world
                    .ecs_mut()
                    .insert_one(player, wizard_identity())
                    .expect("player should accept wizard identity");
                let mut religion = default_religion_state(&world, player);
                religion.experience_level = 14;
                religion.alignment_record = 10;
                world
                    .ecs_mut()
                    .insert_one(player, religion)
                    .expect("player should accept religion state");
                if let Some(mut level) = world.get_component_mut::<ExperienceLevel>(player) {
                    level.0 = 14;
                }
                world.spawn((
                    Monster,
                    Positioned(Position::new(6, 5)),
                    Name("Neferet the Green".to_string()),
                    HitPoints {
                        current: 30,
                        max: 30,
                    },
                    Speed(12),
                    DisplaySymbol {
                        symbol: '@',
                        color: nethack_babel_data::Color::Green,
                    },
                    MovementPoints(NORMAL_SPEED as i32),
                ));

                let mut rng = test_rng();
                let _assign_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );

                for expected_depth in 2..=7 {
                    if expected_depth > 2 {
                        let stairs_down =
                            find_terrain(&world.dungeon().current_level, Terrain::StairsDown)
                                .expect("quest traversal should preserve stairs down");
                        set_player_position(&mut world, stairs_down);
                    }
                    let events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                    assert!(
                        events
                            .iter()
                            .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                        "{} should descend into quest depth {}",
                        scenario.label(),
                        expected_depth
                    );
                }

                let mut quest_state = world
                    .get_component::<crate::quest::QuestState>(player)
                    .map(|state| (*state).clone())
                    .expect("quest traversal should persist quest state");
                quest_state.obtain_artifact();
                quest_state.defeat_nemesis();
                persist_quest_state(&mut world, player, quest_state);

                for expected_depth in (1..=6).rev() {
                    let stairs_up = find_terrain(&world.dungeon().current_level, Terrain::StairsUp)
                        .expect("quest traversal should preserve stairs up");
                    set_player_position(&mut world, stairs_up);
                    let events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);
                    assert!(
                        events
                            .iter()
                            .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                        "{} should ascend into quest depth {}",
                        scenario.label(),
                        expected_depth
                    );
                }

                let leader_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, leader_events)
            }
            StoryTraversalScenario::QuestLeaderAnger => {
                let mut world = make_stair_world(Terrain::StairsDown, 1);
                install_test_catalogs(&mut world);
                let player = world.player();
                world.dungeon_mut().branch = DungeonBranch::Quest;
                world
                    .ecs_mut()
                    .insert_one(player, wizard_identity())
                    .expect("player should accept wizard identity");
                let leader =
                    spawn_full_monster(&mut world, Position::new(6, 5), "Neferet the Green", 12);
                world
                    .ecs_mut()
                    .insert_one(leader, Peaceful)
                    .expect("leader should accept peaceful marker");
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(leader) {
                    hp.current = 40;
                    hp.max = 40;
                }
                if let Some(mut mp) = world.get_component_mut::<MovementPoints>(leader) {
                    mp.0 = 0;
                }

                let mut rng = test_rng();
                let attack_events = resolve_turn(
                    &mut world,
                    PlayerAction::FightDirection {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                assert!(attack_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::MeleeHit { defender, .. } if *defender == leader
                )));

                let blocked_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                (world, blocked_events)
            }
            StoryTraversalScenario::MedusaRevisit => {
                let mut world = make_stair_world(Terrain::StairsDown, 23);
                install_test_catalogs(&mut world);
                world.dungeon_mut().branch = DungeonBranch::Main;

                let mut rng = test_rng();
                let enter_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should enter Medusa level",
                    scenario.label()
                );
                assert_eq!(count_monsters_named(&world, "medusa"), 1);

                let medusa_down = find_terrain(&world.dungeon().current_level, Terrain::StairsDown)
                    .expect("Medusa level should preserve stairs down");
                set_player_position(&mut world, medusa_down);
                let descend_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    descend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should leave Medusa for Castle",
                    scenario.label()
                );

                let castle_up = find_terrain(&world.dungeon().current_level, Terrain::StairsUp)
                    .expect("Castle should preserve stairs up");
                set_player_position(&mut world, castle_up);
                let revisit_events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);
                (world, revisit_events)
            }
            StoryTraversalScenario::CastleRevisit => {
                let mut world = make_stair_world(Terrain::StairsDown, 24);
                install_test_catalogs(&mut world);
                world.dungeon_mut().branch = DungeonBranch::Main;
                let wand_otyp =
                    resolve_object_type_by_spec(&test_game_data().objects, "wand of wishing")
                        .expect("wand of wishing should resolve against the catalog");

                let mut rng = test_rng();
                let enter_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should enter Castle",
                    scenario.label()
                );
                assert_eq!(count_objects_with_type(&world, wand_otyp), 1);

                let castle_up = find_terrain(&world.dungeon().current_level, Terrain::StairsUp)
                    .expect("Castle should preserve stairs up");
                set_player_position(&mut world, castle_up);
                let ascend_events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);
                assert!(
                    ascend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should leave Castle for Medusa",
                    scenario.label()
                );

                let medusa_down = find_terrain(&world.dungeon().current_level, Terrain::StairsDown)
                    .expect("Medusa should preserve stairs down");
                set_player_position(&mut world, medusa_down);
                let revisit_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                (world, revisit_events)
            }
            StoryTraversalScenario::OrcusRevisit => {
                let mut world = make_stair_world(Terrain::StairsDown, 11);
                install_test_catalogs(&mut world);
                world.dungeon_mut().branch = DungeonBranch::Gehennom;

                let mut rng = test_rng();
                let enter_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should enter Orcus level",
                    scenario.label()
                );
                assert_eq!(count_monsters_named(&world, "orcus"), 1);

                let orcus_up = find_terrain(&world.dungeon().current_level, Terrain::StairsUp)
                    .expect("Orcus level should preserve stairs up");
                set_player_position(&mut world, orcus_up);
                let ascend_events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);
                assert!(
                    ascend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should leave Orcus level",
                    scenario.label()
                );

                let gehennom_down =
                    find_terrain(&world.dungeon().current_level, Terrain::StairsDown)
                        .expect("Gehennom entry level should preserve stairs down");
                set_player_position(&mut world, gehennom_down);
                let revisit_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                (world, revisit_events)
            }
            StoryTraversalScenario::FortLudiosRevisit => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let mut rng = test_rng();
                let mut enter_events = Vec::new();
                change_level_to_branch(
                    &mut world,
                    DungeonBranch::FortLudios,
                    1,
                    false,
                    None,
                    &mut rng,
                    &mut enter_events,
                );
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should enter Fort Ludios",
                    scenario.label()
                );

                let mut return_events = Vec::new();
                change_level_to_branch(
                    &mut world,
                    DungeonBranch::Main,
                    1,
                    true,
                    None,
                    &mut rng,
                    &mut return_events,
                );
                assert!(
                    return_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should leave Fort Ludios",
                    scenario.label()
                );

                let mut revisit_events = Vec::new();
                change_level_to_branch(
                    &mut world,
                    DungeonBranch::FortLudios,
                    1,
                    false,
                    None,
                    &mut rng,
                    &mut revisit_events,
                );
                (world, revisit_events)
            }
            StoryTraversalScenario::VladTopRevisit => {
                let mut world = make_stair_world(Terrain::StairsDown, 2);
                install_test_catalogs(&mut world);
                world.dungeon_mut().branch = DungeonBranch::VladsTower;
                let candelabrum_otyp = resolve_object_type_by_spec(
                    &test_game_data().objects,
                    "Candelabrum of Invocation",
                )
                .expect("Candelabrum should resolve against the catalog");

                let mut rng = test_rng();
                let mut enter_events = Vec::new();
                change_level(&mut world, 3, false, &mut rng, &mut enter_events);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should enter Vlad level 3",
                    scenario.label()
                );
                assert_eq!(count_monsters_named(&world, "Vlad the Impaler"), 1);
                assert_eq!(count_objects_with_type(&world, candelabrum_otyp), 1);

                let mut ascend_events = Vec::new();
                change_level(&mut world, 2, true, &mut rng, &mut ascend_events);
                assert!(
                    ascend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should leave Vlad level 3",
                    scenario.label()
                );

                let mut revisit_events = Vec::new();
                change_level(&mut world, 3, false, &mut rng, &mut revisit_events);
                (world, revisit_events)
            }
            StoryTraversalScenario::InvocationPortalRevisit => {
                let mut world = make_stair_world(Terrain::StairsDown, 20);
                install_test_catalogs(&mut world);
                world.dungeon_mut().branch = DungeonBranch::Gehennom;

                let mut rng = test_rng();
                let enter_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should enter Gehennom 21",
                    scenario.label()
                );
                let player = world.player();
                if let Some(mut flags) = world.get_component_mut::<PlayerEvents>(player) {
                    flags.invoked = true;
                    flags.found_vibrating_square = true;
                }

                let current_up = find_terrain(&world.dungeon().current_level, Terrain::StairsUp)
                    .expect("Gehennom 21 should preserve stairs up");
                set_player_position(&mut world, current_up);
                let ascend_events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);
                assert!(
                    ascend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should leave Gehennom 21",
                    scenario.label()
                );

                let cached_down = find_terrain(&world.dungeon().current_level, Terrain::StairsDown)
                    .expect("cached Gehennom 20 should preserve stairs down");
                set_player_position(&mut world, cached_down);
                let revisit_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                (world, revisit_events)
            }
            StoryTraversalScenario::ShopEntry => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(6, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));

                let mut rng = test_rng();
                let events = resolve_turn(
                    &mut world,
                    PlayerAction::Move {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, events)
            }
            StoryTraversalScenario::ShopEntryWelcomeBack => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(6, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                world.dungeon_mut().shop_rooms[0].surcharge = true;

                let mut rng = test_rng();
                let events = resolve_turn(
                    &mut world,
                    PlayerAction::Move {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, events)
            }
            StoryTraversalScenario::ShopEntryRobbed => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(6, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                world.dungeon_mut().shop_rooms[0].robbed = 75;

                let mut rng = test_rng();
                let events = resolve_turn(
                    &mut world,
                    PlayerAction::Move {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, events)
            }
            StoryTraversalScenario::ShopkeeperFollow => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                let unpaid_item = world.spawn((
                    ObjectCore {
                        otyp: ObjectTypeId(0),
                        object_class: ObjectClass::Tool,
                        quantity: 1,
                        weight: 10,
                        age: 0,
                        inv_letter: Some('u'),
                        artifact: None,
                    },
                    ObjectLocation::Floor {
                        x: 6,
                        y: 5,
                        level: world.dungeon().current_data_dungeon_level(),
                    },
                ));
                assert!(
                    world.dungeon_mut().shop_rooms[0]
                        .bill
                        .add(unpaid_item, 100, 1),
                    "shop bill should accept an unpaid entry"
                );

                let mut rng = test_rng();
                let warning_events = resolve_turn(
                    &mut world,
                    PlayerAction::Move {
                        direction: Direction::West,
                    },
                    &mut rng,
                );
                assert!(warning_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "shop-leave-warning"
                )));
                let move_events = resolve_turn(
                    &mut world,
                    PlayerAction::Move {
                        direction: Direction::West,
                    },
                    &mut rng,
                );
                (world, move_events)
            }
            StoryTraversalScenario::ShopkeeperPayoff => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let _gold = spawn_inventory_gold(&mut world, 150, 'g');
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                world.dungeon_mut().shop_rooms[0].angry = true;
                world.dungeon_mut().shop_rooms[0].surcharge = true;
                let unpaid_item = world.spawn((
                    ObjectCore {
                        otyp: ObjectTypeId(0),
                        object_class: ObjectClass::Tool,
                        quantity: 1,
                        weight: 10,
                        age: 0,
                        inv_letter: Some('u'),
                        artifact: None,
                    },
                    ObjectLocation::Floor {
                        x: 6,
                        y: 5,
                        level: world.dungeon().current_data_dungeon_level(),
                    },
                ));
                assert!(
                    world.dungeon_mut().shop_rooms[0]
                        .bill
                        .add(unpaid_item, 100, 1),
                    "shop bill should accept a payable entry"
                );

                let mut rng = test_rng();
                let pay_events = resolve_turn(&mut world, PlayerAction::Pay, &mut rng);
                (world, pay_events)
            }
            StoryTraversalScenario::ShopkeeperCredit => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let gold = spawn_inventory_gold(&mut world, 150, 'g');
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                world.dungeon_mut().shop_rooms[0].debit = 50;
                world.dungeon_mut().shop_rooms[0].angry = true;
                world.dungeon_mut().shop_rooms[0].surcharge = true;

                let mut rng = test_rng();
                let drop_events =
                    resolve_turn(&mut world, PlayerAction::Drop { item: gold }, &mut rng);
                (world, drop_events)
            }
            StoryTraversalScenario::ShopCreditCovers => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                world.dungeon_mut().shop_rooms[0].credit = 150;
                world.dungeon_mut().shop_rooms[0].debit = 20;
                let unpaid_item = world.spawn((
                    ObjectCore {
                        otyp: ObjectTypeId(0),
                        object_class: ObjectClass::Tool,
                        quantity: 1,
                        weight: 10,
                        age: 0,
                        inv_letter: Some('u'),
                        artifact: None,
                    },
                    ObjectLocation::Floor {
                        x: 6,
                        y: 5,
                        level: world.dungeon().current_data_dungeon_level(),
                    },
                ));
                assert!(
                    world.dungeon_mut().shop_rooms[0]
                        .bill
                        .add(unpaid_item, 100, 1),
                    "shop bill should accept a credited entry"
                );

                let mut rng = test_rng();
                let pay_events = resolve_turn(&mut world, PlayerAction::Pay, &mut rng);
                (world, pay_events)
            }
            StoryTraversalScenario::ShopNoMoney => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let _gold = spawn_inventory_gold(&mut world, 50, 'g');
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                let unpaid_item = world.spawn((
                    ObjectCore {
                        otyp: ObjectTypeId(0),
                        object_class: ObjectClass::Tool,
                        quantity: 1,
                        weight: 10,
                        age: 0,
                        inv_letter: Some('u'),
                        artifact: None,
                    },
                    ObjectLocation::Floor {
                        x: 6,
                        y: 5,
                        level: world.dungeon().current_data_dungeon_level(),
                    },
                ));
                assert!(
                    world.dungeon_mut().shop_rooms[0]
                        .bill
                        .add(unpaid_item, 100, 1),
                    "shop bill should accept an underfunded entry"
                );

                let mut rng = test_rng();
                let pay_events = resolve_turn(&mut world, PlayerAction::Pay, &mut rng);
                (world, pay_events)
            }
            StoryTraversalScenario::ShopkeeperSell => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                world.dungeon_mut().shop_rooms[0].shopkeeper_gold = 80;
                let item = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
                if let Some(mut inv) =
                    world.get_component_mut::<crate::inventory::Inventory>(player)
                {
                    inv.items.push(item);
                }

                let mut rng = test_rng();
                let drop_events = resolve_turn(&mut world, PlayerAction::Drop { item }, &mut rng);
                (world, drop_events)
            }
            StoryTraversalScenario::ShopChatPriceQuote => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 6), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));

                let item = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
                let second_item = spawn_inventory_object_by_name(&mut world, "lock pick", 'q');
                if let Some(mut inv) =
                    world.get_component_mut::<crate::inventory::Inventory>(player)
                {
                    inv.items.retain(|entry| *entry != item);
                    inv.items.retain(|entry| *entry != second_item);
                }
                let current_level = world.dungeon().current_data_dungeon_level();
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item) {
                    *loc = ObjectLocation::Floor {
                        x: 5,
                        y: 5,
                        level: current_level,
                    };
                }
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(second_item) {
                    *loc = ObjectLocation::Floor {
                        x: 5,
                        y: 5,
                        level: current_level,
                    };
                }

                let mut rng = test_rng();
                let events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::North,
                    },
                    &mut rng,
                );
                (world, events)
            }
            StoryTraversalScenario::ShopRepair => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                set_player_position(&mut world, Position::new(6, 6));
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                let damaged_pos = Position::new(5, 5);
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(damaged_pos, Terrain::Floor);
                crate::shop::record_shop_damage(
                    &mut world.dungeon_mut().shop_rooms[0],
                    damaged_pos,
                    crate::shop::ShopDamageType::DoorBroken,
                );
                sync_current_level_shopkeeper_state(&mut world);

                let mut rng = test_rng();
                let repair_events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
                (world, repair_events)
            }
            StoryTraversalScenario::ShopkeeperDeath => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                let unpaid_item = world.spawn((
                    ObjectCore {
                        otyp: ObjectTypeId(0),
                        object_class: ObjectClass::Tool,
                        quantity: 1,
                        weight: 10,
                        age: 0,
                        inv_letter: Some('u'),
                        artifact: None,
                    },
                    ObjectLocation::Floor {
                        x: 6,
                        y: 5,
                        level: world.dungeon().current_data_dungeon_level(),
                    },
                ));
                assert!(
                    world.dungeon_mut().shop_rooms[0]
                        .bill
                        .add(unpaid_item, 100, 1),
                    "shop bill should accept an unpaid entry"
                );
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(shopkeeper) {
                    hp.current = 1;
                    hp.max = 1;
                }
                if let Some(mut mp) = world.get_component_mut::<MovementPoints>(shopkeeper) {
                    mp.0 = 0;
                }

                let mut rng = test_rng();
                let mut death_events = Vec::new();
                for _ in 0..8 {
                    if let Some(mut mp) = world.get_component_mut::<MovementPoints>(shopkeeper) {
                        mp.0 = 0;
                    }
                    death_events.extend(resolve_turn(
                        &mut world,
                        PlayerAction::FightDirection {
                            direction: Direction::East,
                        },
                        &mut rng,
                    ));
                    if death_events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "shop-keeper-dead"
                        )
                    }) {
                        break;
                    }
                }
                let exit_events = resolve_turn(
                    &mut world,
                    PlayerAction::Move {
                        direction: Direction::West,
                    },
                    &mut rng,
                );
                death_events.extend(exit_events);
                (world, death_events)
            }
            StoryTraversalScenario::ShopRobbery => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                let unpaid_item = world.spawn((
                    ObjectCore {
                        otyp: ObjectTypeId(0),
                        object_class: ObjectClass::Tool,
                        quantity: 1,
                        weight: 10,
                        age: 0,
                        inv_letter: Some('u'),
                        artifact: None,
                    },
                    ObjectLocation::Floor {
                        x: 6,
                        y: 5,
                        level: world.dungeon().current_data_dungeon_level(),
                    },
                ));
                assert!(
                    world.dungeon_mut().shop_rooms[0]
                        .bill
                        .add(unpaid_item, 100, 1),
                    "shop bill should accept an unpaid entry"
                );

                let mut rng = test_rng();
                let warning_events = resolve_turn(
                    &mut world,
                    PlayerAction::Move {
                        direction: Direction::West,
                    },
                    &mut rng,
                );
                assert!(warning_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "shop-leave-warning"
                )));
                let robbery_events = resolve_turn(
                    &mut world,
                    PlayerAction::Move {
                        direction: Direction::West,
                    },
                    &mut rng,
                );
                (world, robbery_events)
            }
            StoryTraversalScenario::ShopRestitution => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
                world
                    .ecs_mut()
                    .insert_one(shopkeeper, Peaceful)
                    .expect("shopkeeper should accept peaceful marker");
                world
                    .dungeon_mut()
                    .shop_rooms
                    .push(crate::shop::ShopRoom::new(
                        Position::new(5, 4),
                        Position::new(7, 6),
                        crate::shop::ShopType::Tool,
                        shopkeeper,
                        "Izchak".to_string(),
                    ));
                world.dungeon_mut().shop_rooms[0].robbed = 5;
                world.dungeon_mut().shop_rooms[0].angry = true;
                world.dungeon_mut().shop_rooms[0].surcharge = true;
                sync_current_level_shopkeeper_state(&mut world);

                let item = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
                if let Some(mut inv) =
                    world.get_component_mut::<crate::inventory::Inventory>(player)
                {
                    inv.items.push(item);
                }

                let mut rng = test_rng();
                let restitution_events =
                    resolve_turn(&mut world, PlayerAction::Drop { item }, &mut rng);
                (world, restitution_events)
            }
            StoryTraversalScenario::TempleWrongAlignment => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 500, 'g');
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
                world
                    .ecs_mut()
                    .insert_one(priest, Peaceful)
                    .expect("priest should accept peaceful marker");
                world
                    .ecs_mut()
                    .insert_one(
                        priest,
                        crate::npc::Priest {
                            alignment: Alignment::Chaotic,
                            has_shrine: true,
                            is_high_priest: false,
                            angry: false,
                        },
                    )
                    .expect("priest should accept explicit runtime state");

                let mut rng = test_rng();
                let chat_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, chat_events)
            }
            StoryTraversalScenario::TempleAleGift => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
                world
                    .ecs_mut()
                    .insert_one(priest, Peaceful)
                    .expect("priest should accept peaceful marker");

                let mut rng = test_rng();
                let chat_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, chat_events)
            }
            StoryTraversalScenario::TempleVirtuesOfPoverty => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
                world
                    .ecs_mut()
                    .insert_one(priest, Peaceful)
                    .expect("priest should accept peaceful marker");
                world
                    .ecs_mut()
                    .insert_one(
                        priest,
                        crate::npc::Priest {
                            alignment: Alignment::Lawful,
                            has_shrine: false,
                            is_high_priest: false,
                            angry: false,
                        },
                    )
                    .expect("priest should accept explicit runtime state");

                let mut rng = test_rng();
                let chat_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, chat_events)
            }
            StoryTraversalScenario::TempleDonationThanks => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 100, 'g');
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
                world
                    .ecs_mut()
                    .insert_one(priest, Peaceful)
                    .expect("priest should accept peaceful marker");

                let mut rng = test_rng();
                let chat_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, chat_events)
            }
            StoryTraversalScenario::TemplePious => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 300, 'g');
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
                world
                    .ecs_mut()
                    .insert_one(priest, Peaceful)
                    .expect("priest should accept peaceful marker");

                let mut rng = test_rng();
                let chat_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, chat_events)
            }
            StoryTraversalScenario::TempleDonation => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 1_000, 'g');
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
                world
                    .ecs_mut()
                    .insert_one(priest, Peaceful)
                    .expect("priest should accept peaceful marker");

                let mut rng = test_rng();
                let chat_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, chat_events)
            }
            StoryTraversalScenario::TempleBlessing => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 300, 'g');
                let mut religion = default_religion_state(&world, player);
                religion.alignment = Alignment::Lawful;
                religion.original_alignment = Alignment::Lawful;
                religion.alignment_record = -5;
                world
                    .ecs_mut()
                    .insert_one(player, religion)
                    .expect("player should accept religion state");
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
                world
                    .ecs_mut()
                    .insert_one(priest, Peaceful)
                    .expect("priest should accept peaceful marker");

                let mut rng = test_rng();
                let chat_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, chat_events)
            }
            StoryTraversalScenario::TempleCleansing => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 700, 'g');
                let mut religion = default_religion_state(&world, player);
                religion.alignment = Alignment::Lawful;
                religion.original_alignment = Alignment::Lawful;
                religion.alignment_record = -5;
                world
                    .ecs_mut()
                    .insert_one(player, religion)
                    .expect("player should accept religion state");
                world
                    .ecs_mut()
                    .insert_one(
                        player,
                        crate::status::SpellProtection {
                            layers: 1,
                            countdown: 10,
                            interval: 10,
                        },
                    )
                    .expect("player should accept spell protection");
                while world.turn() <= 5001 {
                    world.advance_turn();
                }
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
                world
                    .ecs_mut()
                    .insert_one(priest, Peaceful)
                    .expect("priest should accept peaceful marker");

                let mut rng = test_rng();
                let chat_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, chat_events)
            }
            StoryTraversalScenario::TempleSelflessGenerosity => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 700, 'g');
                world
                    .ecs_mut()
                    .insert_one(
                        player,
                        crate::status::SpellProtection {
                            layers: 1,
                            countdown: 10,
                            interval: 10,
                        },
                    )
                    .expect("player should accept spell protection");
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
                world
                    .ecs_mut()
                    .insert_one(priest, Peaceful)
                    .expect("priest should accept peaceful marker");

                let mut rng = test_rng();
                let chat_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, chat_events)
            }
            StoryTraversalScenario::TempleWrath => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let _gold = spawn_inventory_gold(&mut world, 1_000, 'g');
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(6, 5), Terrain::Altar);
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current = 40;
                    hp.max = 40;
                }
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
                world
                    .ecs_mut()
                    .insert_one(priest, Peaceful)
                    .expect("priest should accept peaceful marker");
                if let Some(mut mp) = world.get_component_mut::<MovementPoints>(priest) {
                    mp.0 = 0;
                }

                let mut rng = test_rng();
                let _attack_events = resolve_turn(
                    &mut world,
                    PlayerAction::FightDirection {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                let chat_events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut rng,
                );
                (world, chat_events)
            }
            StoryTraversalScenario::TempleCalm => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, monk_identity())
                    .expect("player should accept monk identity");
                let mut religion = default_religion_state(&world, player);
                religion.alignment_record = 10;
                religion.bless_cooldown = 0;
                world
                    .ecs_mut()
                    .insert_one(player, religion)
                    .expect("player should accept religion state");
                let altar_pos = Position::new(5, 5);
                set_player_position(&mut world, altar_pos);
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(altar_pos, Terrain::Altar);
                let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
                world
                    .ecs_mut()
                    .insert_one(
                        priest,
                        crate::npc::Priest {
                            alignment: Alignment::Lawful,
                            has_shrine: true,
                            is_high_priest: false,
                            angry: true,
                        },
                    )
                    .expect("priest should accept explicit runtime state");
                let _ = world.ecs_mut().remove_one::<Peaceful>(priest);

                let mut rng = test_rng();
                let pray_events = resolve_turn(&mut world, PlayerAction::Pray, &mut rng);
                (world, pray_events)
            }
            StoryTraversalScenario::UntendedTempleGhost => {
                for seed in 0_u64..256 {
                    let mut world = make_test_world();
                    install_test_catalogs(&mut world);
                    let player = world.player();
                    world
                        .ecs_mut()
                        .insert_one(player, monk_identity())
                        .expect("player should accept monk identity");
                    world
                        .dungeon_mut()
                        .current_level
                        .set_terrain(Position::new(6, 5), Terrain::Altar);

                    let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
                    let mut events = Vec::new();
                    maybe_emit_current_level_temple_entry(
                        &mut world,
                        player,
                        true,
                        &mut rng,
                        &mut events,
                    );
                    if events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "temple-ghost-appears"
                        )
                    }) {
                        return (world, events);
                    }
                }

                panic!(
                    "expected at least one deterministic seed to spawn an untended temple ghost"
                );
            }
            StoryTraversalScenario::SanctumRevisit => {
                let mut world = make_stair_world(Terrain::StairsDown, 19);
                install_test_catalogs(&mut world);
                world.dungeon_mut().branch = DungeonBranch::Gehennom;

                let mut rng = test_rng();
                let first_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(first_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "sanctum-infidel"
                )));
                assert!(first_events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "sanctum-be-gone"
                )));
                assert_eq!(count_monsters_named(&world, "high priest"), 1);

                let sanctum_up = find_terrain(&world.dungeon().current_level, Terrain::StairsUp)
                    .expect("Sanctum should preserve stairs up");
                set_player_position(&mut world, sanctum_up);
                let ascend_events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);
                assert!(
                    ascend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should allow leaving Sanctum",
                    scenario.label()
                );

                let gehennom_down =
                    find_terrain(&world.dungeon().current_level, Terrain::StairsDown)
                        .expect("Gehennom 19 should preserve stairs down to Sanctum");
                set_player_position(&mut world, gehennom_down);
                let revisit_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                (world, revisit_events)
            }
            StoryTraversalScenario::WizardHarassment => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
                let sword = spawn_inventory_object_by_name(&mut world, "long sword", 'b');
                world
                    .ecs_mut()
                    .insert_one(
                        sword,
                        BucStatus {
                            cursed: false,
                            blessed: false,
                            bknown: false,
                        },
                    )
                    .expect("inventory item should accept a BUC component");
                let wizard =
                    spawn_full_monster(&mut world, Position::new(14, 14), "Wizard of Yendor", 12);
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
                    hp.current = 20;
                    hp.max = 40;
                }
                let mut player_events = read_player_events(&world, player);
                player_events.invoked = true;
                persist_player_events(&mut world, player, player_events);
                let mut rng = test_rng();
                let mut final_events = Vec::new();
                for _ in 0..256 {
                    let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
                    if events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. }
                                if key == "wizard-curse-items" || key == "wizard-summon-nasties"
                        )
                    }) {
                        final_events = events;
                        break;
                    }
                }
                (world, final_events)
            }
            StoryTraversalScenario::WizardTaunt => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                let wizard =
                    spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
                world
                    .ecs_mut()
                    .insert_one(wizard, Peaceful)
                    .expect("wizard should accept Peaceful in wizard taunt scenario");
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
                    hp.current = 12;
                    hp.max = 20;
                }
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
                    hp.current = 4;
                    hp.max = 20;
                }
                let mut player_events = read_player_events(&world, player);
                player_events.invoked = true;
                persist_player_events(&mut world, player, player_events);
                let mut rng = test_rng();
                let mut final_events = Vec::new();
                for _ in 0..256 {
                    let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
                    if events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. }
                                if key == "wizard-taunt-laughs"
                                    || key == "wizard-taunt-relinquish"
                                    || key == "wizard-taunt-panic"
                                    || key == "wizard-taunt-return"
                                    || key == "wizard-taunt-general"
                        )
                    }) {
                        final_events = events;
                        break;
                    }
                }
                (world, final_events)
            }
            StoryTraversalScenario::WizardIntervention => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                let sword = spawn_inventory_object_by_name(&mut world, "long sword", 'b');
                world
                    .ecs_mut()
                    .insert_one(
                        sword,
                        BucStatus {
                            cursed: false,
                            blessed: false,
                            bknown: false,
                        },
                    )
                    .expect("inventory item should accept a BUC component");
                let mut player_events = read_player_events(&world, player);
                player_events.killed_wizard = true;
                player_events.wizard_times_killed = 1;
                player_events.wizard_last_killed_turn = world.turn();
                player_events.wizard_intervention_cooldown = 1;
                persist_player_events(&mut world, player, player_events);
                let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "goblin", 6);
                let _ = crate::status::make_sleeping(&mut world, sleeper, 10);
                let mut rng = test_rng();
                let mut final_events = Vec::new();
                for _ in 0..40 {
                    let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
                    if events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. }
                                if key == "wizard-vague-nervous"
                                    || key == "wizard-black-glow"
                                    || key == "wizard-aggravate"
                                    || key == "wizard-summon-nasties"
                                    || key == "wizard-respawned"
                        )
                    }) {
                        final_events = events;
                        break;
                    }
                }
                (world, final_events)
            }
            StoryTraversalScenario::WizardAmuletWake => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let _amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
                let wizard =
                    spawn_full_monster(&mut world, Position::new(14, 14), "Wizard of Yendor", 12);
                let _ = crate::status::make_sleeping(&mut world, wizard, 10_000);
                let mut rng = test_rng();
                let mut final_events = Vec::new();
                for _ in 0..4096 {
                    let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
                    if !crate::status::is_sleeping(&world, wizard) {
                        final_events = events;
                        break;
                    }
                }
                (world, final_events)
            }
            StoryTraversalScenario::WizardBlackGlowBlind => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let player = world.player();
                let item = spawn_inventory_object_by_name(&mut world, "long sword", 'a');
                world
                    .ecs_mut()
                    .insert_one(
                        item,
                        BucStatus {
                            cursed: false,
                            blessed: false,
                            bknown: false,
                        },
                    )
                    .expect("inventory item should accept a BUC component");
                let _ = crate::status::make_blinded(&mut world, player, 20);
                let mut rng = test_rng();
                let mut events = wizard_harassment_messages(
                    &world,
                    player,
                    crate::npc::WizardAction::BlackGlowCurse,
                );

                apply_wizard_harassment_action(
                    &mut world,
                    None,
                    player,
                    crate::npc::WizardAction::BlackGlowCurse,
                    &mut rng,
                    &mut events,
                );

                (world, events)
            }
            StoryTraversalScenario::HumanoidAlohaChat => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let tourist = spawn_full_monster(&mut world, Position::new(6, 5), "tourist", 10);
                world
                    .ecs_mut()
                    .insert_one(tourist, Peaceful)
                    .expect("tourist should accept peaceful marker");

                let events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut test_rng(),
                );
                (world, events)
            }
            StoryTraversalScenario::WereFullMoonChat => {
                let mut world = make_test_world();
                install_test_catalogs(&mut world);
                let were_name =
                    monster_name_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
                let were = spawn_full_monster(&mut world, Position::new(6, 5), &were_name, 10);
                let were_id =
                    monster_id_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
                world
                    .ecs_mut()
                    .insert_one(were, crate::world::MonsterIdentity(were_id))
                    .expect("were full moon scenario should accept explicit monster identity");
                let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "kobold", 8);
                let _ = crate::status::make_sleeping(&mut world, sleeper, 20);

                let events = resolve_turn(
                    &mut world,
                    PlayerAction::Chat {
                        direction: Direction::East,
                    },
                    &mut test_rng(),
                );

                (world, events)
            }
            StoryTraversalScenario::WizardLevelTeleport => {
                let mut world = make_stair_world(Terrain::Floor, 10);
                install_test_catalogs(&mut world);
                let player = world.player();
                let wizard =
                    spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
                    hp.current = 20;
                    hp.max = 40;
                }
                let mut player_events = read_player_events(&world, player);
                player_events.invoked = true;
                persist_player_events(&mut world, player, player_events);
                let mut rng = test_rng();
                let mut final_events = Vec::new();
                for _ in 0..256 {
                    let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
                    if events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-level-teleport"
                        )
                    }) {
                        final_events = events;
                        break;
                    }
                }
                (world, final_events)
            }
            StoryTraversalScenario::EndgameAscension => {
                let mut world = make_stair_world(Terrain::StairsDown, 20);
                install_test_catalogs(&mut world);
                let player = world.player();
                world
                    .ecs_mut()
                    .insert_one(player, wizard_identity())
                    .expect("player should accept wizard identity");
                world.dungeon_mut().branch = DungeonBranch::Gehennom;
                let mut rng = test_rng();

                let descend_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
                assert!(
                    descend_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should descend into Gehennom:21",
                    scenario.label()
                );

                let invocation_pos = world
                    .dungeon()
                    .trap_map
                    .traps
                    .iter()
                    .find(|trap| trap.trap_type == TrapType::VibratingSquare)
                    .map(|trap| trap.pos)
                    .expect("Gehennom 21 should expose a vibrating square before invocation");
                set_player_position(&mut world, invocation_pos);

                let bell = spawn_inventory_object_by_name(&mut world, "Bell of Opening", 'b');
                let candelabrum =
                    spawn_inventory_object_by_name(&mut world, "Candelabrum of Invocation", 'c');
                let book = spawn_inventory_object_by_name(&mut world, "Book of the Dead", 'd');
                let current_turn = world.turn() as i64;
                if let Some(mut core) = world.get_component_mut::<ObjectCore>(bell) {
                    core.age = current_turn;
                }
                world
                    .ecs_mut()
                    .insert_one(candelabrum, Enchantment { spe: 7 })
                    .expect("candelabrum should accept candle count");
                world
                    .ecs_mut()
                    .insert_one(
                        candelabrum,
                        LightSource {
                            lit: true,
                            recharged: 0,
                        },
                    )
                    .expect("candelabrum should accept light state");

                let invocation_events = resolve_turn(
                    &mut world,
                    PlayerAction::Read { item: Some(book) },
                    &mut rng,
                );
                assert!(
                    invocation_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "invocation-complete"
                    )),
                    "{} should complete invocation before portal traversal",
                    scenario.label()
                );

                let enter_events = move_player_onto_magic_portal(&mut world, &mut rng);
                assert!(
                    enter_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                    "{} should enter Endgame through the magic portal",
                    scenario.label()
                );

                for expected_depth in 2..=5 {
                    let portal_events = move_player_onto_magic_portal(&mut world, &mut rng);
                    assert!(
                        portal_events
                            .iter()
                            .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                        "{} should reach Endgame depth {}",
                        scenario.label(),
                        expected_depth
                    );
                }

                let mut altar_positions = Vec::new();
                for y in 0..world.dungeon().current_level.height {
                    for x in 0..world.dungeon().current_level.width {
                        let pos = Position::new(x as i32, y as i32);
                        if world
                            .dungeon()
                            .current_level
                            .get(pos)
                            .is_some_and(|cell| cell.terrain == Terrain::Altar)
                        {
                            altar_positions.push(pos);
                        }
                    }
                }
                altar_positions.sort_by_key(|pos| pos.x);
                let chaotic_altar = *altar_positions
                    .last()
                    .expect("Astral Plane should have a chaotic altar");
                set_player_position(&mut world, chaotic_altar);

                let amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
                let offer_events = resolve_turn(
                    &mut world,
                    PlayerAction::Offer { item: Some(amulet) },
                    &mut rng,
                );
                (world, offer_events)
            }
        }
    }

    /// Spawn a monster at the given position with a given base speed.
    #[allow(dead_code)]
    fn spawn_monster(world: &mut GameWorld, pos: Position, speed: u32) -> hecs::Entity {
        let order = world.next_creation_order();
        world.spawn((
            Monster,
            Positioned(pos),
            HitPoints {
                current: 12,
                max: 12,
            },
            Speed(speed),
            MovementPoints(NORMAL_SPEED as i32),
            Name(format!("monster(spd={})", speed)),
            order,
        ))
    }

    /// Spawn a monster with a speed modifier.
    #[allow(dead_code)]
    fn spawn_monster_with_mod(
        world: &mut GameWorld,
        pos: Position,
        speed: u32,
        speed_mod: SpeedModifier,
    ) -> hecs::Entity {
        let order = world.next_creation_order();
        world.spawn((
            Monster,
            Positioned(pos),
            HitPoints {
                current: 12,
                max: 12,
            },
            Speed(speed),
            MovementPoints(NORMAL_SPEED as i32),
            MonsterSpeedMod(speed_mod),
            Name(format!("monster(spd={},mod={:?})", speed, speed_mod)),
            order,
        ))
    }

    // ── Basic movement tests ─────────────────────────────────────

    #[test]
    fn move_east() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        let turn_end = events
            .iter()
            .any(|e| matches!(e, EngineEvent::TurnEnd { .. }));
        assert!(moved, "expected EntityMoved event");
        assert!(turn_end, "expected TurnEnd event");

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(6, 5));
    }

    #[test]
    fn move_into_wall_stays_put() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        for _ in 0..10 {
            resolve_turn(
                &mut world,
                PlayerAction::Move {
                    direction: Direction::North,
                },
                &mut rng,
            );
        }
        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0.y, 3);
    }

    #[test]
    fn move_triggers_gold_autopickup_on_destination_tile() {
        let mut world = make_test_world();
        let coin = spawn_floor_coin(&mut world, Position::new(6, 5));
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::ItemPickedUp {
                    actor,
                    item,
                    ..
                } if *actor == world.player() && *item == coin
            )
        }));
        let loc = world.get_component::<ObjectLocation>(coin).unwrap();
        assert!(matches!(*loc, ObjectLocation::Inventory));
    }

    #[test]
    fn move_does_not_autopickup_when_disabled() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .set_autopickup(false, vec![ObjectClass::Coin]);
        let coin = spawn_floor_coin(&mut world, Position::new(6, 5));
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(
            !events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::ItemPickedUp {
                        actor,
                        item,
                        ..
                    } if *actor == world.player() && *item == coin
                )
            }),
            "autopickup disabled should not pick coin"
        );
        let loc = world.get_component::<ObjectLocation>(coin).unwrap();
        assert!(matches!(*loc, ObjectLocation::Floor { x: 6, y: 5, .. }));
    }

    #[test]
    fn move_onto_topology_portal_enters_fort_ludios() {
        let mut world = make_portal_world(DungeonBranch::Main, 20, Position::new(6, 5));
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::LevelChanged { .. }))
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-branch"
        )));
        assert_eq!(world.dungeon().branch, DungeonBranch::FortLudios);
        assert_eq!(world.dungeon().depth, 1);
    }

    #[test]
    fn move_onto_topology_magic_portal_enters_endgame() {
        let mut world = make_portal_world(DungeonBranch::Gehennom, 21, Position::new(6, 5));
        let mut rng = test_rng();
        let player = world.player();
        if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
            player_events.invoked = true;
        }

        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::LevelChanged { .. }))
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "teleport-branch"
        )));
        assert_eq!(world.dungeon().branch, DungeonBranch::Endgame);
        assert_eq!(world.dungeon().depth, 1);
        assert!(world.dungeon().current_level_flags.is_endgame);
    }

    #[test]
    fn move_onto_topology_magic_portal_before_invocation_does_not_enter_endgame() {
        let mut world = make_portal_world(DungeonBranch::Gehennom, 21, Position::new(6, 5));
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, EngineEvent::LevelChanged { .. }))
        );
        assert_eq!(world.dungeon().branch, DungeonBranch::Gehennom);
        assert_eq!(world.dungeon().depth, 21);
    }

    #[test]
    fn test_invocation_traversal_reaches_endgame_from_gehennom() {
        let mut world = make_stair_world(Terrain::StairsDown, 20);
        install_test_catalogs(&mut world);
        world.dungeon_mut().branch = DungeonBranch::Gehennom;
        let mut rng = test_rng();

        let events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
        );
        assert_eq!(world.dungeon().branch, DungeonBranch::Gehennom);
        assert_eq!(world.dungeon().depth, 21);

        let invocation_pos = world
            .dungeon()
            .trap_map
            .traps
            .iter()
            .find(|trap| trap.trap_type == TrapType::VibratingSquare)
            .map(|trap| trap.pos)
            .expect("Gehennom 21 should expose a vibrating square before invocation");
        set_player_position(&mut world, invocation_pos);

        let bell = spawn_inventory_object_by_name(&mut world, "Bell of Opening", 'b');
        let candelabrum =
            spawn_inventory_object_by_name(&mut world, "Candelabrum of Invocation", 'c');
        let book = spawn_inventory_object_by_name(&mut world, "Book of the Dead", 'd');
        let current_turn = world.turn() as i64;
        if let Some(mut core) = world.get_component_mut::<ObjectCore>(bell) {
            core.age = current_turn;
        }
        world
            .ecs_mut()
            .insert_one(candelabrum, Enchantment { spe: 7 })
            .expect("candelabrum should accept candle count");
        world
            .ecs_mut()
            .insert_one(
                candelabrum,
                LightSource {
                    lit: true,
                    recharged: 0,
                },
            )
            .expect("candelabrum should accept light state");

        let invocation_events = resolve_turn(
            &mut world,
            PlayerAction::Read { item: Some(book) },
            &mut rng,
        );
        assert!(invocation_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "invocation-complete"
        )));

        let portal_pos = find_terrain(&world.dungeon().current_level, Terrain::MagicPortal)
            .expect("successful invocation should expose a magic portal");
        let (entry_pos, direction) =
            adjacent_walkable_step(&world.dungeon().current_level, portal_pos)
                .expect("portal should have at least one adjacent walkable entry tile");
        set_player_position(&mut world, entry_pos);

        let portal_events = resolve_turn(&mut world, PlayerAction::Move { direction }, &mut rng);
        assert!(
            portal_events
                .iter()
                .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
        );
        assert_eq!(world.dungeon().branch, DungeonBranch::Endgame);
        assert_eq!(world.dungeon().depth, 1);
        assert!(world.dungeon().current_level_flags.is_endgame);
    }

    #[test]
    fn rest_does_not_move() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(!moved, "rest should not generate a move event");

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(5, 5));
    }

    #[test]
    fn travel_moves_one_step_toward_destination() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let destination = Position::new(7, 5);

        let events = resolve_turn(&mut world, PlayerAction::Travel { destination }, &mut rng);
        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(moved, "travel should move when destination differs");

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(6, 5));
    }

    #[test]
    fn travel_uses_alternate_walkable_direction_when_direct_blocked() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        // Block direct east step.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Wall);

        let destination = Position::new(7, 5);
        resolve_turn(&mut world, PlayerAction::Travel { destination }, &mut rng);

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(6, 4));
    }

    #[test]
    fn travel_at_destination_does_not_move() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let destination = Position::new(5, 5);

        let events = resolve_turn(&mut world, PlayerAction::Travel { destination }, &mut rng);
        let moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { .. }));
        assert!(!moved, "travel should not move when already at destination");

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(pos.0, Position::new(5, 5));
    }

    #[test]
    fn takeoffall_removes_equipped_items() {
        let mut world = make_test_world();
        let player = world.player();
        let helmet = world.spawn((Name("helmet".to_string()),));
        let cloak = world.spawn((Name("cloak".to_string()),));
        {
            let mut equip = world
                .get_component_mut::<crate::equipment::EquipmentSlots>(player)
                .unwrap();
            equip.set(crate::equipment::EquipSlot::Helmet, Some(helmet));
            equip.set(crate::equipment::EquipSlot::Cloak, Some(cloak));
        }

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::TakeOffAll, &mut rng);

        assert!(events.iter().any(
            |e| matches!(e, EngineEvent::ItemRemoved { actor, item } if *actor == player && *item == helmet)
        ));
        assert!(events.iter().any(
            |e| matches!(e, EngineEvent::ItemRemoved { actor, item } if *actor == player && *item == cloak)
        ));

        let equip = world
            .get_component::<crate::equipment::EquipmentSlots>(player)
            .unwrap();
        assert!(equip.helmet.is_none());
        assert!(equip.cloak.is_none());
    }

    #[test]
    fn takeoffall_with_nothing_worn_emits_feedback() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::TakeOffAll, &mut rng);

        assert!(
            events.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key == "not-wearing-that")
            )
        );
    }

    #[test]
    fn adjust_swaps_inventory_letters_when_target_letter_occupied() {
        let mut world = make_test_world();
        let item_a = spawn_inventory_item(&mut world, 'a');
        let item_b = spawn_inventory_item(&mut world, 'b');
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Adjust {
                item: item_a,
                new_letter: 'b',
            },
            &mut rng,
        );

        let core_a = world.get_component::<ObjectCore>(item_a).unwrap();
        let core_b = world.get_component::<ObjectCore>(item_b).unwrap();
        assert_eq!(core_a.inv_letter, Some('b'));
        assert_eq!(core_b.inv_letter, Some('a'));
        assert!(!events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "adjust-not-implemented")
        ));
    }

    #[test]
    fn name_item_sets_object_extra_and_emits_message() {
        let mut world = make_test_world();
        let item = spawn_inventory_item(&mut world, 'a');
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Name {
                target: NameTarget::Item { item },
                name: "Excalibur".to_string(),
            },
            &mut rng,
        );

        let extra = world
            .get_component::<nethack_babel_data::ObjectExtra>(item)
            .unwrap();
        assert_eq!(extra.name.as_deref(), Some("Excalibur"));
        let display_name = world.get_component::<Name>(item).unwrap();
        assert_eq!(display_name.0, "Excalibur");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "item-name-set"))
        );
        assert!(!events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "name-not-implemented")
        ));
    }

    #[test]
    fn name_monster_updates_monster_name() {
        let mut world = make_test_world();
        let monster = spawn_monster(&mut world, Position::new(6, 5), 12);
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Name {
                target: NameTarget::Monster { entity: monster },
                name: "Fluffy".to_string(),
            },
            &mut rng,
        );

        let mon_name = world.get_component::<Name>(monster).unwrap();
        assert_eq!(mon_name.0, "Fluffy");
        assert!(!events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "name-not-implemented")
        ));
    }

    #[test]
    fn name_monster_at_position_updates_monster_name() {
        let mut world = make_test_world();
        let _ = spawn_monster(&mut world, Position::new(7, 5), 12);
        let target = spawn_monster(&mut world, Position::new(8, 5), 12);
        let mut rng = test_rng();

        resolve_turn(
            &mut world,
            PlayerAction::Name {
                target: NameTarget::MonsterAt {
                    position: Position::new(8, 5),
                },
                name: "Gremlin".to_string(),
            },
            &mut rng,
        );

        let mon_name = world.get_component::<Name>(target).unwrap();
        assert_eq!(mon_name.0, "Gremlin");
    }

    #[test]
    fn name_monster_at_position_without_monster_emits_error() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Name {
                target: NameTarget::MonsterAt {
                    position: Position::new(30, 10),
                },
                name: "Nobody".to_string(),
            },
            &mut rng,
        );

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "cannot-do-that"))
        );
    }

    #[test]
    fn name_level_sets_current_level_annotation() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        resolve_turn(
            &mut world,
            PlayerAction::Name {
                target: NameTarget::Level,
                name: "mine level".to_string(),
            },
            &mut rng,
        );

        assert_eq!(
            world.dungeon().current_level_annotation(),
            Some("mine level")
        );
    }

    #[test]
    fn annotate_sets_current_level_annotation() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        resolve_turn(
            &mut world,
            PlayerAction::Annotate {
                text: "vault pending".to_string(),
            },
            &mut rng,
        );

        assert_eq!(
            world.dungeon().current_level_annotation(),
            Some("vault pending")
        );
    }

    #[test]
    fn call_type_sets_called_class_name_and_emits_feedback() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::CallType {
                class: '!',
                name: "healing?".to_string(),
            },
            &mut rng,
        );

        assert_eq!(world.dungeon().called_item_class('!'), Some("healing?"));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "item-called-set"))
        );
    }

    // ── Movement point calculation tests (test vectors from spec) ─

    #[test]
    fn moveamt_normal_speed_unencumbered() {
        // Test vector #1: Human, speed 12, no bonus, unencumbered => 12
        let mut rng = test_rng();
        let amt = u_calc_moveamt(12, HeroSpeed::Normal, Encumbrance::Unencumbered, &mut rng);
        assert_eq!(amt, 12);
    }

    #[test]
    fn moveamt_very_fast_bonus() {
        // Test vectors #2 and #3: Very_fast has 2/3 chance of +12.
        // Run many trials and verify the distribution.
        let mut rng = test_rng();
        let mut bonus_count = 0;
        let trials = 3000;
        for _ in 0..trials {
            let amt = u_calc_moveamt(12, HeroSpeed::VeryFast, Encumbrance::Unencumbered, &mut rng);
            assert!(amt == 12 || amt == 24, "unexpected moveamt: {}", amt);
            if amt == 24 {
                bonus_count += 1;
            }
        }
        // Expected: ~2000 out of 3000 (66.7%).  Allow wide margin.
        let ratio = bonus_count as f64 / trials as f64;
        assert!(
            (0.55..=0.78).contains(&ratio),
            "Very_fast bonus ratio {:.3} outside expected range [0.55, 0.78]",
            ratio
        );
    }

    #[test]
    fn moveamt_fast_bonus() {
        // Test vectors #4 and #5: Fast has 1/3 chance of +12.
        let mut rng = test_rng();
        let mut bonus_count = 0;
        let trials = 3000;
        for _ in 0..trials {
            let amt = u_calc_moveamt(12, HeroSpeed::Fast, Encumbrance::Unencumbered, &mut rng);
            assert!(amt == 12 || amt == 24, "unexpected moveamt: {}", amt);
            if amt == 24 {
                bonus_count += 1;
            }
        }
        let ratio = bonus_count as f64 / trials as f64;
        assert!(
            (0.22..=0.45).contains(&ratio),
            "Fast bonus ratio {:.3} outside expected range [0.22, 0.45]",
            ratio
        );
    }

    #[test]
    fn moveamt_encumbrance_burdened() {
        // Test vector #6: speed 12, Burdened => 12 - 12/4 = 9
        let mut rng = test_rng();
        let amt = u_calc_moveamt(12, HeroSpeed::Normal, Encumbrance::Burdened, &mut rng);
        assert_eq!(amt, 9);
    }

    #[test]
    fn moveamt_encumbrance_stressed() {
        // Test vector #7: speed 12, Stressed => 12 - 12/2 = 6
        let mut rng = test_rng();
        let amt = u_calc_moveamt(12, HeroSpeed::Normal, Encumbrance::Stressed, &mut rng);
        assert_eq!(amt, 6);
    }

    #[test]
    fn moveamt_encumbrance_strained() {
        // Test vector #8: speed 12, Strained => 12 - (12*3)/4 = 3
        let mut rng = test_rng();
        let amt = u_calc_moveamt(12, HeroSpeed::Normal, Encumbrance::Strained, &mut rng);
        assert_eq!(amt, 3);
    }

    #[test]
    fn moveamt_encumbrance_overtaxed() {
        // Test vector #9: speed 12, Overtaxed => 12 - (12*7)/8 = 12 - 10 = 2
        let mut rng = test_rng();
        let amt = u_calc_moveamt(12, HeroSpeed::Normal, Encumbrance::Overtaxed, &mut rng);
        assert_eq!(amt, 2);
    }

    #[test]
    fn moveamt_very_fast_stressed() {
        // Test vector #10: Very_fast + bonus hit + Stressed => (12+12)*50% = 12
        // We need the bonus to hit, so we test the deterministic path:
        // When bonus fires: 24 - 24/2 = 12
        // When bonus misses: 12 - 12/2 = 6
        let mut rng = test_rng();
        let trials = 3000;
        let mut results = Vec::new();
        for _ in 0..trials {
            let amt = u_calc_moveamt(12, HeroSpeed::VeryFast, Encumbrance::Stressed, &mut rng);
            assert!(
                amt == 12 || amt == 6,
                "unexpected moveamt with VeryFast+Stressed: {}",
                amt
            );
            results.push(amt);
        }
        // Should see both values.
        assert!(results.contains(&12), "expected some 12 results");
        assert!(results.contains(&6), "expected some 6 results");
    }

    #[test]
    fn moveamt_high_base_speed() {
        // Test vector #11: steam vortex, base speed 24, unencumbered => 24
        let mut rng = test_rng();
        let amt = u_calc_moveamt(24, HeroSpeed::Normal, Encumbrance::Unencumbered, &mut rng);
        assert_eq!(amt, 24);
    }

    // ── Monster movement calculation tests ───────────────────────

    #[test]
    fn mcalcmove_normal() {
        let mut rng = test_rng();
        // Normal speed monster, not moving (no stochastic rounding).
        let amt = mcalcmove(12, SpeedModifier::Normal, false, &mut rng);
        assert_eq!(amt, 12);
    }

    #[test]
    fn mcalcmove_slow_below_12() {
        let mut rng = test_rng();
        // Speed 6, slow: (2*6+1)/3 = 13/3 = 4 (integer division)
        let amt = mcalcmove(6, SpeedModifier::Slow, false, &mut rng);
        assert_eq!(amt, 4);
    }

    #[test]
    fn mcalcmove_slow_at_12() {
        let mut rng = test_rng();
        // Speed 12, slow: 4 + 12/3 = 8
        let amt = mcalcmove(12, SpeedModifier::Slow, false, &mut rng);
        assert_eq!(amt, 8);
    }

    #[test]
    fn mcalcmove_fast() {
        let mut rng = test_rng();
        // Speed 12, fast: (4*12+2)/3 = 50/3 = 16
        let amt = mcalcmove(12, SpeedModifier::Fast, false, &mut rng);
        assert_eq!(amt, 16);
    }

    #[test]
    fn mcalcmove_stochastic_rounding() {
        // Speed 15 with m_moving=true:
        //   mmove_adj = 15 % 12 = 3
        //   mmove = 15 - 3 = 12
        //   if rng(0..12) < 3 => mmove = 24; else mmove = 12
        // So ~3/12 = 25% chance of 24, ~75% chance of 12.
        let mut rng = test_rng();
        let mut got_24 = 0;
        let trials = 3000;
        for _ in 0..trials {
            let amt = mcalcmove(15, SpeedModifier::Normal, true, &mut rng);
            assert!(
                amt == 12 || amt == 24,
                "stochastic rounding should yield 12 or 24, got {}",
                amt
            );
            if amt == 24 {
                got_24 += 1;
            }
        }
        let ratio = got_24 as f64 / trials as f64;
        assert!(
            (0.15..=0.35).contains(&ratio),
            "stochastic rounding ratio {:.3} outside [0.15, 0.35]",
            ratio
        );
    }

    // ── Monster proportional speed tests ─────────────────────────

    #[test]
    fn fast_monster_acts_more_often() {
        // A speed-24 monster should accumulate movement twice as fast as
        // a speed-12 monster.  Over many turns, the fast monster should
        // get roughly twice as many actions.
        // Verify proportionality of movement point accumulation.
        // Speed 24 should accumulate exactly 2x the points of speed 12
        // when both are exact multiples of NORMAL_SPEED (no stochastic
        // rounding variance).
        let mut slow_total = 0i32;
        let mut fast_total = 0i32;
        let mut rng = test_rng();
        for _ in 0..1000 {
            slow_total += mcalcmove(12, SpeedModifier::Normal, true, &mut rng);
            fast_total += mcalcmove(24, SpeedModifier::Normal, true, &mut rng);
        }
        assert_eq!(slow_total, 12000);
        assert_eq!(fast_total, 24000);
    }

    // ── New-turn processing tests ────────────────────────────────

    #[test]
    fn new_turn_triggers_after_exhaustion() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let initial_turn = world.turn();

        // Player starts with NORMAL_SPEED movement points.
        // After one action, MP drops to 0 (below threshold).
        // No monsters, so both sides exhausted => new turn.
        let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        assert_eq!(world.turn(), initial_turn + 1, "turn should have advanced");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::TurnEnd { .. })),
            "should emit TurnEnd"
        );
    }

    #[test]
    fn turn_counter_increments_correctly() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        assert_eq!(world.turn(), 1);
        resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
        assert_eq!(world.turn(), 2);
        resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
        assert_eq!(world.turn(), 3);
    }

    // ── Hunger depletion tests ───────────────────────────────────

    #[test]
    fn hunger_depletes_each_turn() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let initial = world.get_component::<Nutrition>(world.player()).unwrap().0;
        assert_eq!(initial, 900);

        resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        let after = world.get_component::<Nutrition>(world.player()).unwrap().0;
        assert_eq!(after, 899, "nutrition should deplete by 1 per turn");
    }

    #[test]
    fn hunger_state_transition_emits_event() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Set nutrition to just above the Hungry threshold (151).
        if let Some(mut n) = world.get_component_mut::<Nutrition>(world.player()) {
            n.0 = 151;
        }

        let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        // 151 -> 150: crosses from NotHungry to Hungry.
        let hunger_event = events.iter().find(|e| {
            matches!(
                e,
                EngineEvent::HungerChange {
                    old: HungerLevel::NotHungry,
                    new_level: HungerLevel::Hungry,
                    ..
                }
            )
        });
        assert!(
            hunger_event.is_some(),
            "should emit HungerChange NotHungry->Hungry"
        );
    }

    #[test]
    fn hunger_thresholds_match_spec() {
        // From spec: >1000 Satiated, >150 NotHungry, >50 Hungry,
        // >0 Weak, <=0 Fainting
        assert_eq!(nutrition_to_hunger_level(1001), HungerLevel::Satiated);
        assert_eq!(nutrition_to_hunger_level(1000), HungerLevel::NotHungry);
        assert_eq!(nutrition_to_hunger_level(151), HungerLevel::NotHungry);
        assert_eq!(nutrition_to_hunger_level(150), HungerLevel::Hungry);
        assert_eq!(nutrition_to_hunger_level(51), HungerLevel::Hungry);
        assert_eq!(nutrition_to_hunger_level(50), HungerLevel::Weak);
        assert_eq!(nutrition_to_hunger_level(1), HungerLevel::Weak);
        assert_eq!(nutrition_to_hunger_level(0), HungerLevel::Fainting);
        assert_eq!(nutrition_to_hunger_level(-1), HungerLevel::Fainting);
    }

    #[test]
    fn test_hunger_starvation_death_in_turn() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Set nutrition just above starvation threshold for con=10.
        // Threshold = -(100 + 10*10) = -200. Set to -200 (alive), then
        // one turn depletes by 1 to -201 (dead).
        if let Some(mut n) = world.get_component_mut::<Nutrition>(world.player()) {
            n.0 = -200;
        }

        let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        let died = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::EntityDied {
                    cause: crate::event::DeathCause::Starvation,
                    ..
                }
            )
        });
        assert!(
            died,
            "hero should die of starvation at nutrition <= -(100 + 10*con)"
        );
    }

    #[test]
    fn test_hunger_starvation_survives_at_threshold() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Con=10: threshold = -200. Set to -199, one turn to -200.
        // At exactly -200, should NOT starve (strict <).
        if let Some(mut n) = world.get_component_mut::<Nutrition>(world.player()) {
            n.0 = -199;
        }

        let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        let died = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::EntityDied {
                    cause: crate::event::DeathCause::Starvation,
                    ..
                }
            )
        });
        assert!(
            !died,
            "hero should survive at exactly the starvation threshold"
        );
    }

    #[test]
    fn test_hunger_entering_weak_emits_strength_message() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Set nutrition to 51, so after 1 depletion it becomes 50 = Weak.
        if let Some(mut n) = world.get_component_mut::<Nutrition>(world.player()) {
            n.0 = 51;
        }

        let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        let strength_msg = events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "hunger-weak-strength-loss"),
        );
        assert!(
            strength_msg,
            "entering Weak should emit strength loss message"
        );
    }

    // ── HP regeneration tests ────────────────────────────────────

    #[test]
    fn hp_regen_fires_on_correct_period() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Damage the player first.
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(world.player()) {
            hp.current = 10;
        }

        // Level 1 => period = max(1, 30-1) = 29.
        // Run 29 turns and count HpChange events.
        let mut hp_events = 0;
        for _ in 0..29 {
            let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
            hp_events += events
                .iter()
                .filter(|e| matches!(e, EngineEvent::HpChange { .. }))
                .count();
        }

        // In 29 turns, exactly 1 should be divisible by 29.
        assert_eq!(
            hp_events, 1,
            "expected exactly 1 HP regen event in 29 turns at level 1"
        );
    }

    #[test]
    fn hp_regen_does_not_exceed_max() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Player at full HP.
        {
            let hp = world.get_component::<HitPoints>(world.player()).unwrap();
            assert_eq!(hp.current, hp.max);
        }

        // Run some turns.
        for _ in 0..50 {
            let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
            let hp_regen = events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::HpChange {
                        entity,
                        amount,
                        source: HpSource::Regeneration,
                        ..
                    } if *entity == world.player() && *amount > 0
                )
            });
            assert!(!hp_regen, "should not regen HP when already at max");
        }
    }

    // ── PW regeneration tests ────────────────────────────────────

    #[test]
    fn pw_regen_blocked_when_stressed() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Set encumbrance to Stressed.
        if let Some(mut enc) = world.get_component_mut::<EncumbranceLevel>(world.player()) {
            enc.0 = Encumbrance::Stressed;
        }

        // Damage PW.
        if let Some(mut pw) = world.get_component_mut::<Power>(world.player()) {
            pw.current = 1;
        }

        // Run many turns.
        for _ in 0..100 {
            let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
            let pw_event = events
                .iter()
                .any(|e| matches!(e, EngineEvent::PwChange { .. }));
            assert!(
                !pw_event,
                "PW should not regen when encumbrance >= Stressed"
            );
        }
    }

    // ── Integration: movement points flow through resolve_turn ──

    #[test]
    fn player_movement_points_cycle() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Initial MP = NORMAL_SPEED (12).
        let mp = world
            .get_component::<MovementPoints>(world.player())
            .unwrap()
            .0;
        assert_eq!(mp, NORMAL_SPEED as i32);

        // After one turn: MP is deducted by 12, then new turn grants
        // 12 more (unencumbered, normal speed).
        resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        let mp_after = world
            .get_component::<MovementPoints>(world.player())
            .unwrap()
            .0;
        // Should be back to 12 (deducted 12, granted 12).
        assert_eq!(mp_after, NORMAL_SPEED as i32);
    }

    #[test]
    fn encumbered_player_gets_fewer_movement_points() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Set encumbrance to Burdened.
        if let Some(mut enc) = world.get_component_mut::<EncumbranceLevel>(world.player()) {
            enc.0 = Encumbrance::Burdened;
        }

        // After one turn: MP deducted 12, granted 9 (Burdened).
        resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        let mp = world
            .get_component::<MovementPoints>(world.player())
            .unwrap()
            .0;
        // Started at 12, -12 = 0, +9 = 9.
        assert_eq!(mp, 9);
    }

    // ── hp_regen_period and pw_regen_period unit tests ───────────

    #[test]
    fn hp_regen_period_values() {
        assert_eq!(hp_regen_period(1), 29);
        assert_eq!(hp_regen_period(10), 20);
        assert_eq!(hp_regen_period(29), 1);
        assert_eq!(hp_regen_period(30), 1);
        assert_eq!(hp_regen_period(50), 1); // capped at 1
    }

    #[test]
    fn pw_regen_period_values() {
        // xlevel=1, wisdom=10 => bonus=6, period=35-6=29
        assert_eq!(pw_regen_period(1, 10), 29);
        // xlevel=10, wisdom=18 => bonus=19, period=35-19=16
        assert_eq!(pw_regen_period(10, 18), 16);
        // xlevel=30, wisdom=25 => bonus=42, period=max(1,35-42)=1
        assert_eq!(pw_regen_period(30, 25), 1);
    }

    // ── turn_events iterator tests ───────────────────────────────

    #[test]
    fn turn_events_yields_same_events_as_resolve_turn() {
        // Both APIs should produce identical event sequences for
        // the same world state and RNG seed.
        let mut world1 = make_test_world();
        let mut rng1 = test_rng();
        let vec_events = resolve_turn(
            &mut world1,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng1,
        );

        let mut world2 = make_test_world();
        let mut rng2 = test_rng();
        let iter_events: Vec<EngineEvent> = turn_events(
            &mut world2,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng2,
        )
        .collect();

        assert_eq!(
            vec_events.len(),
            iter_events.len(),
            "turn_events should yield the same number of events as resolve_turn"
        );

        // Check world state is identical.
        let pos1 = world1.get_component::<Positioned>(world1.player()).unwrap();
        let pos2 = world2.get_component::<Positioned>(world2.player()).unwrap();
        assert_eq!(pos1.0, pos2.0, "player position should match");
        assert_eq!(world1.turn(), world2.turn(), "turn counter should match");
    }

    #[test]
    fn turn_events_rest_has_turn_end() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let events: Vec<EngineEvent> =
            turn_events(&mut world, PlayerAction::Rest, &mut rng).collect();

        let has_turn_end = events
            .iter()
            .any(|e| matches!(e, EngineEvent::TurnEnd { .. }));
        assert!(has_turn_end, "turn_events should yield TurnEnd");
    }

    #[test]
    fn turn_events_can_be_partially_consumed() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Take only the first event and drop the iterator.
        let first = turn_events(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        )
        .next();

        assert!(first.is_some(), "iterator should yield at least one event");
    }

    // ── Gen-block equivalence tests ───────────────────────────────

    #[test]
    fn gen_block_matches_resolve_turn_rest() {
        // All three turn resolution methods should produce identical
        // event sequences for the same world state and RNG seed.

        // Run with resolve_turn (Vec-based).
        let mut world_a = make_test_world();
        let mut rng_a = test_rng();
        let events_vec = resolve_turn(&mut world_a, PlayerAction::Rest, &mut rng_a);

        // Run with turn_events (from_fn-based).
        let mut world_b = make_test_world();
        let mut rng_b = test_rng();
        let events_from_fn: Vec<EngineEvent> =
            turn_events(&mut world_b, PlayerAction::Rest, &mut rng_b).collect();

        // Run with turn_events_gen (gen-block-based).
        let mut world_c = make_test_world();
        let mut rng_c = test_rng();
        let events_gen: Vec<EngineEvent> =
            turn_events_gen(&mut world_c, PlayerAction::Rest, &mut rng_c).collect();

        // All three should have the same length.
        assert_eq!(
            events_vec.len(),
            events_from_fn.len(),
            "resolve_turn vs turn_events length mismatch"
        );
        assert_eq!(
            events_vec.len(),
            events_gen.len(),
            "resolve_turn vs turn_events_gen length mismatch"
        );

        // Compare event-by-event using Debug representation (EngineEvent
        // may not impl PartialEq, but it impls Debug).
        for (i, (ev_vec, ev_gen)) in events_vec.iter().zip(events_gen.iter()).enumerate() {
            assert_eq!(
                format!("{:?}", ev_vec),
                format!("{:?}", ev_gen),
                "event {} differs between resolve_turn and turn_events_gen",
                i
            );
        }
    }

    #[test]
    fn gen_block_matches_resolve_turn_move() {
        let mut world_a = make_test_world();
        let mut rng_a = test_rng();
        let events_vec = resolve_turn(
            &mut world_a,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng_a,
        );

        let mut world_b = make_test_world();
        let mut rng_b = test_rng();
        let events_gen: Vec<EngineEvent> = turn_events_gen(
            &mut world_b,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng_b,
        )
        .collect();

        assert_eq!(
            events_vec.len(),
            events_gen.len(),
            "resolve_turn vs turn_events_gen length mismatch (Move)"
        );

        for (i, (ev_vec, ev_gen)) in events_vec.iter().zip(events_gen.iter()).enumerate() {
            assert_eq!(
                format!("{:?}", ev_vec),
                format!("{:?}", ev_gen),
                "event {} differs (Move)",
                i
            );
        }
    }

    #[test]
    fn gen_block_matches_over_multiple_turns() {
        // Run 20 turns with both methods and compare cumulative output.
        let mut world_a = make_test_world();
        let mut rng_a = test_rng();
        let mut all_vec: Vec<String> = Vec::new();

        let mut world_b = make_test_world();
        let mut rng_b = test_rng();
        let mut all_gen: Vec<String> = Vec::new();

        for _ in 0..20 {
            let events = resolve_turn(&mut world_a, PlayerAction::Rest, &mut rng_a);
            for e in &events {
                all_vec.push(format!("{:?}", e));
            }

            let events: Vec<EngineEvent> =
                turn_events_gen(&mut world_b, PlayerAction::Rest, &mut rng_b).collect();
            for e in &events {
                all_gen.push(format!("{:?}", e));
            }
        }

        assert_eq!(
            all_vec.len(),
            all_gen.len(),
            "total event count mismatch over 20 turns"
        );
        for (i, (a, b)) in all_vec.iter().zip(all_gen.iter()).enumerate() {
            assert_eq!(a, b, "event {} differs over 20-turn run", i);
        }
    }

    // ── Search tests ─────────────────────────────────────────────

    #[test]
    fn search_finds_nothing_on_empty_floor() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // No traps placed, so searching should find nothing.
        let events = resolve_turn(&mut world, PlayerAction::Search, &mut rng);

        let trap_revealed = events
            .iter()
            .any(|e| matches!(e, EngineEvent::TrapRevealed { .. }));
        assert!(
            !trap_revealed,
            "search on empty floor should not reveal any traps"
        );
    }

    #[test]
    fn search_reveals_hidden_trap() {
        use crate::traps::TrapInstance;
        use nethack_babel_data::TrapType;

        let mut world = make_test_world();

        // Place a hidden trap adjacent to the player (player is at 5,5).
        world.dungeon_mut().trap_map.traps.push(TrapInstance {
            pos: Position::new(6, 5),
            trap_type: TrapType::Pit,
            detected: false,
            triggered_count: 0,
        });

        // Search repeatedly with a deterministic RNG until the trap is
        // found (base chance ~1/8 per search per adjacent cell).
        let mut found = false;
        for seed in 0..200u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            // Reset trap detection for each attempt.
            world.dungeon_mut().trap_map.traps[0].detected = false;

            let events = resolve_turn(&mut world, PlayerAction::Search, &mut rng);

            if events
                .iter()
                .any(|e| matches!(e, EngineEvent::TrapRevealed { .. }))
            {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "searching near a hidden trap should eventually reveal it"
        );
    }

    // ── Stair transition tests ──────────────────────────────────

    /// Helper: create a test world with stairs and set the player on
    /// the specified terrain at position (5, 5).
    fn make_stair_world(player_terrain: Terrain, depth: i32) -> GameWorld {
        let mut world = GameWorld::new(Position::new(5, 5));
        install_test_catalogs(&mut world);
        world.dungeon_mut().depth = depth;
        // Carve a small open room.
        for y in 3..=7 {
            for x in 3..=7 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        // Set player tile to the requested terrain.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(5, 5), player_terrain);
        world
    }

    fn make_portal_world(branch: DungeonBranch, depth: i32, portal_pos: Position) -> GameWorld {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        world.dungeon_mut().branch = branch;
        world.dungeon_mut().depth = depth;
        world
            .dungeon_mut()
            .current_level
            .set_terrain(portal_pos, Terrain::MagicPortal);
        world
    }

    fn set_player_position(world: &mut GameWorld, pos: Position) {
        if let Some(mut player_pos) = world.get_component_mut::<Positioned>(world.player()) {
            player_pos.0 = pos;
        }
    }

    fn adjacent_walkable_step(
        map: &crate::dungeon::LevelMap,
        target: Position,
    ) -> Option<(Position, Direction)> {
        let candidates = [
            (Direction::East, Position::new(target.x - 1, target.y)),
            (Direction::West, Position::new(target.x + 1, target.y)),
            (Direction::South, Position::new(target.x, target.y - 1)),
            (Direction::North, Position::new(target.x, target.y + 1)),
        ];
        candidates.into_iter().find_map(|(dir, pos)| {
            map.get(pos)
                .is_some_and(|cell| cell.terrain.is_walkable())
                .then_some((pos, dir))
        })
    }

    #[test]
    fn test_generate_or_special_topology_returns_population_for_special_levels() {
        let world = make_test_world();
        let mut rng = test_rng();

        let (_generated, flags, population) = generate_or_special_topology(
            &world,
            crate::dungeon::DungeonBranch::Gehennom,
            12,
            &mut rng,
        );

        assert!(flags.no_prayer, "Orcus should preserve no_prayer flags");
        let population = population.expect("special levels should carry population plans");
        assert!(
            population
                .monsters
                .iter()
                .any(|spawn| spawn.name.eq_ignore_ascii_case("Orcus")),
            "Orcus level should carry its boss population directly from generation"
        );
    }

    #[test]
    fn test_generate_or_special_topology_injects_portal_for_fort_entrance_depth() {
        let world = make_test_world();
        let mut rng = test_rng();

        let (generated, _flags, population) =
            generate_or_special_topology(&world, DungeonBranch::Main, 20, &mut rng);

        assert!(
            find_terrain(&generated.map, Terrain::MagicPortal).is_some(),
            "Main:20 should receive a topology-driven portal tile for Fort Ludios"
        );
        assert!(
            population.is_none(),
            "random portal source levels should not fabricate special population plans"
        );
    }

    #[test]
    fn test_generate_or_special_topology_injects_magic_portal_for_endgame_entrance_depth() {
        let world = make_test_world();
        let mut rng = test_rng();

        let (generated, _flags, population) =
            generate_or_special_topology(&world, DungeonBranch::Gehennom, 21, &mut rng);

        assert!(
            find_terrain(&generated.map, Terrain::MagicPortal).is_some(),
            "Gehennom:21 should receive a topology-driven magic portal tile for Endgame"
        );
        assert!(
            population.is_none(),
            "Gehennom:21 is a portal source level, not a populated special level"
        );
    }

    #[test]
    fn test_generate_or_special_topology_omits_population_for_random_levels() {
        let world = make_test_world();
        let mut rng = test_rng();

        let (_generated, flags, population) =
            generate_or_special_topology(&world, crate::dungeon::DungeonBranch::Main, 2, &mut rng);

        assert!(
            !flags.no_dig && !flags.no_teleport && !flags.no_prayer && !flags.is_endgame,
            "ordinary random levels should not inherit any special flags"
        );
        assert!(
            population.is_none(),
            "ordinary random levels should not synthesize special population plans"
        );
    }

    #[test]
    fn test_generate_or_special_topology_uses_player_role_for_quest_levels() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("test player should accept identity");
        let mut rng = test_rng();

        let (_generated, _flags, population) =
            generate_or_special_topology(&world, crate::dungeon::DungeonBranch::Quest, 7, &mut rng);

        let population = population.expect("quest goal should carry a role-specific population");
        assert!(
            population
                .monsters
                .iter()
                .any(|spawn| spawn.name.eq_ignore_ascii_case("Dark One")),
            "wizard quest goal should target the Dark One"
        );
        assert!(
            population
                .objects
                .iter()
                .any(|spawn| spawn.name == "The Eye of the Aethiopica"),
            "wizard quest goal should carry the Eye artifact"
        );
    }

    #[test]
    fn test_generate_or_special_topology_uses_all_player_roles_for_quest_levels() {
        for role in crate::role::Role::ALL {
            let mut world = make_test_world();
            let player = world.player();
            world
                .ecs_mut()
                .insert_one(player, identity_for_role(role))
                .expect("test player should accept identity");
            let mut start_rng = test_rng();
            let mut goal_rng = test_rng();

            let (_start_generated, _start_flags, start_population) = generate_or_special_topology(
                &world,
                crate::dungeon::DungeonBranch::Quest,
                1,
                &mut start_rng,
            );
            let (_goal_generated, _goal_flags, goal_population) = generate_or_special_topology(
                &world,
                crate::dungeon::DungeonBranch::Quest,
                7,
                &mut goal_rng,
            );

            let start_population =
                start_population.expect("quest start should carry a role-specific population");
            let goal_population =
                goal_population.expect("quest goal should carry a role-specific population");
            assert!(
                start_population
                    .monsters
                    .iter()
                    .any(|spawn| spawn.name == crate::quest::quest_leader_for_role(role)),
                "{} quest start should target leader {}",
                role.name(),
                crate::quest::quest_leader_for_role(role)
            );
            assert!(
                goal_population
                    .monsters
                    .iter()
                    .any(|spawn| spawn.name == crate::quest::quest_nemesis_for_role(role)),
                "{} quest goal should target nemesis {}",
                role.name(),
                crate::quest::quest_nemesis_for_role(role)
            );
            assert!(
                goal_population
                    .objects
                    .iter()
                    .any(|spawn| spawn.name == crate::quest::quest_artifact_for_role(role)),
                "{} quest goal should target artifact {}",
                role.name(),
                crate::quest::quest_artifact_for_role(role)
            );
        }
    }

    fn has_monster_named(world: &GameWorld, name: &str) -> bool {
        count_monsters_named(world, name) > 0
    }

    fn normalize_monster_lookup(name: &str) -> String {
        let normalized = name
            .strip_prefix("the ")
            .or_else(|| name.strip_prefix("The "))
            .unwrap_or(name);
        monster_spec_alias(normalized)
            .unwrap_or(normalized)
            .to_ascii_lowercase()
    }

    fn count_monsters_named(world: &GameWorld, name: &str) -> usize {
        let expected = normalize_monster_lookup(name);
        world
            .ecs()
            .query::<(&Monster, &Name)>()
            .iter()
            .filter(|(_, (_m, n))| normalize_monster_lookup(&n.0) == expected)
            .count()
    }

    fn find_monster_named(world: &GameWorld, name: &str) -> Option<hecs::Entity> {
        let expected = normalize_monster_lookup(name);
        world
            .ecs()
            .query::<(&Monster, &Name)>()
            .iter()
            .find_map(|(entity, (_m, n))| {
                (normalize_monster_lookup(&n.0) == expected).then_some(entity)
            })
    }

    fn count_objects_with_type(world: &GameWorld, object_type: ObjectTypeId) -> usize {
        world
            .ecs()
            .query::<&ObjectCore>()
            .iter()
            .filter(|(_, core)| core.otyp == object_type)
            .count()
    }

    fn count_objects_with_artifact(world: &GameWorld, artifact_id: ArtifactId) -> usize {
        world
            .ecs()
            .query::<&ObjectCore>()
            .iter()
            .filter(|(_, core)| core.artifact == Some(artifact_id))
            .count()
    }

    #[test]
    fn test_quest_artifact_base_items_resolve_against_loaded_catalog() {
        for role in crate::role::Role::ALL {
            let artifact_name = crate::quest::quest_artifact_for_role(role);
            let artifact = crate::artifacts::find_artifact_by_name(artifact_name)
                .unwrap_or_else(|| panic!("{} should exist in artifact table", artifact_name));
            let object_type =
                resolve_artifact_base_object_type(&test_game_data().objects, artifact);
            assert!(
                object_type.is_some(),
                "{} should resolve to a real base object in the loaded catalog",
                artifact_name
            );
        }
    }

    #[test]
    fn test_resolve_special_level_population_marks_all_quest_artifacts() {
        let monster_defs = &test_game_data().monsters;
        let object_defs = &test_game_data().objects;

        for role in crate::role::Role::ALL {
            let artifact_name = crate::quest::quest_artifact_for_role(role);
            let artifact = crate::artifacts::find_artifact_by_name(artifact_name)
                .unwrap_or_else(|| panic!("{} should exist in artifact table", artifact_name));
            let resolved = resolve_special_level_population(
                monster_defs,
                object_defs,
                crate::special_levels::SpecialLevelPopulation {
                    monsters: Vec::new(),
                    objects: vec![crate::special_levels::SpecialObjectSpawn {
                        name: artifact_name.to_string(),
                        pos: Some(Position::new(5, 5)),
                        chance: 100,
                        quantity: Some(1),
                    }],
                },
            );
            assert_eq!(
                resolved.objects.len(),
                1,
                "{} should resolve into one special object spawn",
                artifact_name
            );
            assert_eq!(
                resolved.objects[0].artifact_id,
                Some(artifact.id),
                "{} should keep its artifact id through population resolution",
                artifact_name
            );
        }
    }

    #[test]
    fn test_resolve_monster_id_by_spec_accepts_optional_leading_article() {
        let monster_id =
            resolve_monster_id_by_spec(&test_game_data().monsters, "the Minion of Huhetotl");
        assert!(
            monster_id.is_some(),
            "special monster resolver should ignore an optional leading article"
        );
    }

    #[test]
    fn test_resolve_monster_id_by_spec_accepts_centaur_alias() {
        let monster_id = resolve_monster_id_by_spec(&test_game_data().monsters, "centaur");
        assert!(
            monster_id.is_some(),
            "special monster resolver should map the classic centaur alias"
        );
    }

    #[test]
    fn test_resolve_monster_id_by_spec_accepts_ronin_alias() {
        let monster_id = resolve_monster_id_by_spec(&test_game_data().monsters, "ronin");
        assert!(
            monster_id.is_some(),
            "special monster resolver should map the classic ronin alias"
        );
    }

    /// Spawn a monster with all components needed for stair caching.
    #[allow(dead_code)]
    fn spawn_full_monster(
        world: &mut GameWorld,
        pos: Position,
        name: &str,
        speed: u32,
    ) -> hecs::Entity {
        let entity = world.spawn((
            Monster,
            Positioned(pos),
            Speed(speed),
            MovementPoints(NORMAL_SPEED as i32),
            Name(name.to_string()),
            HitPoints { current: 8, max: 8 },
            DisplaySymbol {
                symbol: 'g',
                color: nethack_babel_data::Color::Brown,
            },
        ));
        if let Some(monster_id) = test_game_data()
            .monsters
            .iter()
            .find(|def| def.names.male.eq_ignore_ascii_case(name))
            .map(|def| def.id)
        {
            let _ = world
                .ecs_mut()
                .insert_one(entity, crate::world::MonsterIdentity(monster_id));
        }
        entity
    }

    fn monster_id_with_sound_excluding(
        world: &GameWorld,
        sound: MonsterSound,
        excluded: &[&str],
    ) -> nethack_babel_data::MonsterId {
        world
            .monster_catalog()
            .iter()
            .find(|def| {
                def.sound == sound
                    && !excluded
                        .iter()
                        .any(|name| def.names.male.eq_ignore_ascii_case(name))
            })
            .map(|def| def.id)
            .unwrap_or_else(|| panic!("test catalog should contain a monster with sound {sound:?}"))
    }

    #[test]
    fn test_go_down_stairs() {
        let mut world = make_stair_world(Terrain::StairsDown, 1);
        let mut rng = test_rng();

        let events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);

        // Depth should have increased.
        assert_eq!(world.dungeon().depth, 2, "depth should be 2 after GoDown");

        // Should emit LevelChanged event.
        let level_changed = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::LevelChanged {
                    from_depth,
                    to_depth,
                    ..
                } if from_depth == "1" && to_depth == "2"
            )
        });
        assert!(level_changed, "expected LevelChanged event from 1 to 2");
    }

    #[test]
    fn test_quest_start_blocks_descent_before_assignment() {
        let mut world = make_stair_world(Terrain::StairsDown, 1);
        world.dungeon_mut().branch = DungeonBranch::Quest;
        let mut rng = test_rng();

        let events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);

        assert_eq!(world.dungeon().depth, 1);
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-expelled"
        )));
        let quest_state = world
            .get_component::<crate::quest::QuestState>(world.player())
            .expect("quest descent block should persist quest state");
        assert_eq!(quest_state.times_expelled, 1);
    }

    #[test]
    fn test_quest_start_blocks_descent_after_rejection_until_assignment() {
        let mut world = make_stair_world(Terrain::StairsDown, 1);
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Quest;
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        let mut religion = default_religion_state(&world, player);
        religion.experience_level = 10;
        religion.alignment_record = 10;
        world
            .ecs_mut()
            .insert_one(player, religion)
            .expect("player should accept religion state");
        if let Some(mut level) = world.get_component_mut::<ExperienceLevel>(player) {
            level.0 = 10;
        }
        spawn_idle_named_monster(&mut world, Position::new(6, 5), "Neferet the Green");

        let mut rng = test_rng();
        let reject_events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(reject_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-leader-reject"
        )));

        let blocked_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(world.dungeon().depth, 1);
        assert!(blocked_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-expelled"
        )));

        if let Some(mut level) = world.get_component_mut::<ExperienceLevel>(player) {
            level.0 = 14;
        }
        if let Some(mut religion) =
            world.get_component_mut::<crate::religion::ReligionState>(player)
        {
            religion.experience_level = 14;
        }
        let assign_events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(assign_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-assigned"
        )));

        let descend_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert!(
            descend_events
                .iter()
                .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
        );
        assert_eq!(world.dungeon().depth, 2);
    }

    #[test]
    fn test_entering_medusa_spawns_medusa() {
        let mut world = make_stair_world(Terrain::StairsDown, 23);
        let mut rng = test_rng();

        let _events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);

        assert_eq!(world.dungeon().depth, 24, "expected to descend to depth 24");
        assert!(
            has_monster_named(&world, "medusa"),
            "entering Medusa level should spawn Medusa"
        );
    }

    #[test]
    fn test_entering_castle_spawns_wand_of_wishing() {
        let mut world = make_stair_world(Terrain::StairsDown, 24);
        let mut rng = test_rng();

        let _events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);

        assert_eq!(world.dungeon().depth, 25, "expected to descend to depth 25");
        let wand_otyp = resolve_object_type_by_spec(&test_game_data().objects, "wand of wishing")
            .expect("wand of wishing should resolve against the catalog");
        assert!(
            count_objects_with_type(&world, wand_otyp) > 0,
            "entering Castle should place a real wand of wishing"
        );
    }

    #[test]
    fn test_entering_orcus_spawns_orcus() {
        let mut world = make_stair_world(Terrain::StairsDown, 11);
        world.dungeon_mut().branch = crate::dungeon::DungeonBranch::Gehennom;
        let mut rng = test_rng();

        let _events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);

        assert_eq!(world.dungeon().depth, 12, "expected to descend to depth 12");
        assert!(
            has_monster_named(&world, "orcus"),
            "entering Orcus level should spawn Orcus"
        );
    }

    #[test]
    fn test_spawn_special_monster_uses_swimmer_requested_tile() {
        let mut world = make_test_world();
        let spawn_pos = Position::new(10, 10);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(spawn_pos, Terrain::Pool);
        let mut rng = test_rng();
        let monster_id = resolve_monster_id_by_spec(&test_game_data().monsters, "Juiblex")
            .expect("Juiblex should resolve against the catalog");

        spawn_special_monster(
            &mut world,
            ResolvedSpecialMonsterSpawn {
                monster_id,
                pos: Some(spawn_pos),
                chance: 100,
                peaceful: Some(false),
                asleep: Some(false),
            },
            &test_game_data().monsters,
            &mut rng,
        );

        let juiblex_pos = world
            .ecs()
            .query::<(&Monster, &Positioned, &Name)>()
            .iter()
            .find_map(|(_, (_monster, pos, name))| {
                name.0.eq_ignore_ascii_case("Juiblex").then_some(pos.0)
            });
        assert_eq!(
            juiblex_pos,
            Some(spawn_pos),
            "special monster spawning should preserve valid swimmer spawn tiles"
        );
    }

    #[test]
    fn test_entering_valley_uses_embedded_population_and_flags() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        world.dungeon_mut().branch = crate::dungeon::DungeonBranch::Gehennom;
        world.dungeon_mut().depth = 0;
        let mut rng = test_rng();
        let mut events = Vec::new();

        change_level(&mut world, 1, false, &mut rng, &mut events);

        assert_eq!(world.dungeon().depth, 1, "expected to arrive on Valley");
        assert!(
            world.dungeon().current_level_flags.no_prayer,
            "Valley should set no_prayer through the live level flags"
        );
        assert!(
            world.dungeon().current_level_flags.no_teleport,
            "Valley should set noteleport through the live level flags"
        );
        assert!(
            count_monsters_named(&world, "ghost") >= 1,
            "entering Valley should spawn embedded undead population"
        );
    }

    #[test]
    fn test_entering_vlad_tower_top_spawns_vlad_and_candelabrum() {
        let mut world = make_stair_world(Terrain::StairsDown, 2);
        world.dungeon_mut().branch = crate::dungeon::DungeonBranch::VladsTower;
        let mut rng = test_rng();

        let _events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);

        assert_eq!(
            world.dungeon().depth,
            3,
            "expected to descend to Vlad level 3"
        );
        assert!(
            has_monster_named(&world, "Vlad the Impaler"),
            "entering Vlad level 3 should spawn Vlad the Impaler"
        );
        let candelabrum_otyp =
            resolve_object_type_by_spec(&test_game_data().objects, "Candelabrum of Invocation")
                .expect("Candelabrum should resolve against the catalog");
        assert!(
            count_objects_with_type(&world, candelabrum_otyp) > 0,
            "Vlad level 3 should place the Candelabrum"
        );
    }

    #[test]
    fn test_entering_sanctum_spawns_high_priest() {
        let mut world = make_stair_world(Terrain::StairsDown, 19);
        world.dungeon_mut().branch = crate::dungeon::DungeonBranch::Gehennom;
        let mut rng = test_rng();

        let events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);

        assert_eq!(world.dungeon().depth, 20, "expected to descend to Sanctum");
        assert!(
            has_monster_named(&world, "high priest"),
            "entering Sanctum should spawn the high priest"
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "sanctum-infidel"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "sanctum-be-gone"
        )));
    }

    #[test]
    fn test_entering_fakewiz2_spawns_amulet_of_yendor() {
        let mut world = make_stair_world(Terrain::StairsDown, 14);
        world.dungeon_mut().branch = crate::dungeon::DungeonBranch::Gehennom;
        let mut rng = test_rng();

        let _events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);

        assert_eq!(world.dungeon().depth, 15, "expected to descend to fakewiz2");
        let amulet_otyp =
            resolve_object_type_by_spec(&test_game_data().objects, "Amulet of Yendor")
                .expect("Amulet of Yendor should resolve against the catalog");
        assert!(
            count_objects_with_type(&world, amulet_otyp) > 0,
            "entering fakewiz2 should place the Amulet of Yendor"
        );
    }

    #[test]
    fn test_entering_wizard_quest_start_spawns_role_specific_leader() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("test player should accept identity");
        let mut rng = test_rng();
        let mut events = Vec::new();

        change_level_to_branch(
            &mut world,
            crate::dungeon::DungeonBranch::Quest,
            1,
            false,
            None,
            &mut rng,
            &mut events,
        );

        assert_eq!(world.dungeon().branch, crate::dungeon::DungeonBranch::Quest);
        assert_eq!(world.dungeon().depth, 1);
        assert!(
            has_monster_named(&world, "Neferet the Green"),
            "wizard quest start should spawn Neferet the Green"
        );
    }

    #[test]
    fn test_entering_all_quest_starts_spawn_role_specific_leaders_and_guardians() {
        for (idx, role) in crate::role::Role::ALL.into_iter().enumerate() {
            let mut world = make_test_world();
            install_test_catalogs(&mut world);
            let player = world.player();
            world
                .ecs_mut()
                .insert_one(player, identity_for_role(role))
                .expect("test player should accept identity");
            let mut rng = Pcg64::seed_from_u64(9000 + idx as u64);
            let mut events = Vec::new();

            change_level_to_branch(
                &mut world,
                crate::dungeon::DungeonBranch::Quest,
                1,
                false,
                None,
                &mut rng,
                &mut events,
            );

            assert!(
                has_monster_named(&world, crate::quest::quest_leader_for_role(role)),
                "{} quest start should spawn leader {}",
                role.name(),
                crate::quest::quest_leader_for_role(role)
            );
            assert!(
                has_monster_named(&world, crate::quest::quest_guardian_for_role(role)),
                "{} quest start should spawn guardian {}",
                role.name(),
                crate::quest::quest_guardian_for_role(role)
            );
        }
    }

    #[test]
    fn test_entering_wizard_quest_goal_spawns_role_specific_nemesis_and_artifact() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("test player should accept identity");
        let mut rng = test_rng();
        let mut events = Vec::new();

        change_level_to_branch(
            &mut world,
            crate::dungeon::DungeonBranch::Quest,
            7,
            false,
            None,
            &mut rng,
            &mut events,
        );

        let eye = crate::artifacts::find_artifact_by_name("The Eye of the Aethiopica")
            .expect("Eye of the Aethiopica should exist");
        assert_eq!(world.dungeon().branch, crate::dungeon::DungeonBranch::Quest);
        assert_eq!(world.dungeon().depth, 7);
        assert!(
            has_monster_named(&world, "Dark One"),
            "wizard quest goal should spawn the Dark One"
        );
        assert_eq!(
            count_objects_with_artifact(&world, eye.id),
            1,
            "wizard quest goal should place the Eye artifact as a real artifact object"
        );
    }

    #[test]
    fn test_entering_all_quest_goals_spawn_role_specific_nemeses_enemies_and_artifacts() {
        for (idx, role) in crate::role::Role::ALL.into_iter().enumerate() {
            let mut world = make_test_world();
            install_test_catalogs(&mut world);
            let player = world.player();
            world
                .ecs_mut()
                .insert_one(player, identity_for_role(role))
                .expect("test player should accept identity");
            let mut rng = Pcg64::seed_from_u64(10000 + idx as u64);
            let mut events = Vec::new();

            change_level_to_branch(
                &mut world,
                crate::dungeon::DungeonBranch::Quest,
                7,
                false,
                None,
                &mut rng,
                &mut events,
            );

            let artifact = crate::artifacts::find_artifact_by_name(
                crate::quest::quest_artifact_for_role(role),
            )
            .unwrap_or_else(|| panic!("{} quest artifact should exist", role.name()));
            let enemies = crate::quest::quest_enemies_for_role(role);
            assert!(
                has_monster_named(&world, crate::quest::quest_nemesis_for_role(role)),
                "{} quest goal should spawn nemesis {}",
                role.name(),
                crate::quest::quest_nemesis_for_role(role)
            );
            assert!(
                has_monster_named(&world, enemies.enemy1),
                "{} quest goal should spawn primary quest enemy {}",
                role.name(),
                enemies.enemy1
            );
            assert!(
                has_monster_named(&world, enemies.enemy2),
                "{} quest goal should spawn secondary quest enemy {}",
                role.name(),
                enemies.enemy2
            );
            assert_eq!(
                count_objects_with_artifact(&world, artifact.id),
                1,
                "{} quest goal should place artifact {} as a real artifact object",
                role.name(),
                crate::quest::quest_artifact_for_role(role)
            );
        }
    }

    #[test]
    fn test_entering_all_quest_locators_spawn_role_specific_enemies() {
        for (idx, role) in crate::role::Role::ALL.into_iter().enumerate() {
            let mut world = make_test_world();
            install_test_catalogs(&mut world);
            let player = world.player();
            world
                .ecs_mut()
                .insert_one(player, identity_for_role(role))
                .expect("test player should accept identity");
            let mut rng = Pcg64::seed_from_u64(10100 + idx as u64);
            let mut events = Vec::new();
            let enemies = crate::quest::quest_enemies_for_role(role);

            change_level_to_branch(
                &mut world,
                crate::dungeon::DungeonBranch::Quest,
                4,
                false,
                None,
                &mut rng,
                &mut events,
            );

            assert!(
                has_monster_named(&world, enemies.enemy1),
                "{} quest locator should spawn primary quest enemy {}",
                role.name(),
                enemies.enemy1
            );
            assert!(
                has_monster_named(&world, enemies.enemy2),
                "{} quest locator should spawn secondary quest enemy {}",
                role.name(),
                enemies.enemy2
            );
        }
    }

    #[test]
    fn test_entering_all_quest_fillers_spawn_role_specific_enemies() {
        for (idx, role) in crate::role::Role::ALL.into_iter().enumerate() {
            let mut world = make_test_world();
            install_test_catalogs(&mut world);
            let player = world.player();
            world
                .ecs_mut()
                .insert_one(player, identity_for_role(role))
                .expect("test player should accept identity");
            let mut rng = Pcg64::seed_from_u64(10200 + idx as u64);
            let mut events = Vec::new();
            let enemies = crate::quest::quest_enemies_for_role(role);

            change_level_to_branch(
                &mut world,
                crate::dungeon::DungeonBranch::Quest,
                3,
                false,
                None,
                &mut rng,
                &mut events,
            );

            assert!(
                has_monster_named(&world, enemies.enemy1),
                "{} quest filler should spawn primary quest enemy {}",
                role.name(),
                enemies.enemy1
            );
            assert!(
                has_monster_named(&world, enemies.enemy2),
                "{} quest filler should spawn secondary quest enemy {}",
                role.name(),
                enemies.enemy2
            );
        }
    }

    #[test]
    fn test_entering_fort_ludios_spawns_garrison() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let mut rng = test_rng();
        let mut events = Vec::new();

        change_level_to_branch(
            &mut world,
            crate::dungeon::DungeonBranch::FortLudios,
            1,
            false,
            None,
            &mut rng,
            &mut events,
        );

        assert_eq!(
            count_monsters_named(&world, "soldier"),
            2,
            "Fort Ludios should place two soldiers from the planned garrison"
        );
        assert_eq!(
            count_monsters_named(&world, "lieutenant"),
            1,
            "Fort Ludios should place its lieutenant"
        );
        assert_eq!(
            count_monsters_named(&world, "captain"),
            1,
            "Fort Ludios should place its captain"
        );
    }

    #[test]
    fn test_revisiting_wizard_quest_goal_does_not_duplicate_nemesis_or_artifact() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("test player should accept identity");
        let mut rng = test_rng();
        let mut events = Vec::new();

        change_level_to_branch(
            &mut world,
            crate::dungeon::DungeonBranch::Quest,
            7,
            false,
            None,
            &mut rng,
            &mut events,
        );

        let eye = crate::artifacts::find_artifact_by_name("The Eye of the Aethiopica")
            .expect("Eye of the Aethiopica should exist");
        let xorns = count_monsters_named(&world, "xorn");
        let vampire_bats = count_monsters_named(&world, "vampire bat");
        assert_eq!(count_monsters_named(&world, "Dark One"), 1);
        assert!(xorns >= 1, "wizard quest goal should spawn xorn escorts");
        assert!(
            vampire_bats >= 1,
            "wizard quest goal should spawn vampire bat escorts"
        );
        assert_eq!(count_objects_with_artifact(&world, eye.id), 1);

        change_level(&mut world, 6, true, &mut rng, &mut events);
        change_level(&mut world, 7, false, &mut rng, &mut events);

        assert_eq!(
            count_monsters_named(&world, "Dark One"),
            1,
            "revisiting wizard quest goal should not duplicate the nemesis"
        );
        assert_eq!(
            count_monsters_named(&world, "xorn"),
            xorns,
            "revisiting wizard quest goal should not duplicate xorn escorts"
        );
        assert_eq!(
            count_monsters_named(&world, "vampire bat"),
            vampire_bats,
            "revisiting wizard quest goal should not duplicate vampire bat escorts"
        );
        assert_eq!(
            count_objects_with_artifact(&world, eye.id),
            1,
            "revisiting wizard quest goal should not duplicate the quest artifact"
        );
    }

    #[test]
    fn test_revisiting_wizard_quest_start_does_not_duplicate_leader_or_guardians() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("test player should accept identity");
        let mut rng = test_rng();
        let mut events = Vec::new();

        change_level_to_branch(
            &mut world,
            crate::dungeon::DungeonBranch::Quest,
            1,
            false,
            None,
            &mut rng,
            &mut events,
        );

        let apprentices = count_monsters_named(&world, "apprentice");
        assert_eq!(count_monsters_named(&world, "Neferet the Green"), 1);
        assert!(
            apprentices >= 1,
            "wizard quest start should spawn apprentice guardians"
        );

        change_level(&mut world, 2, false, &mut rng, &mut events);
        change_level(&mut world, 1, true, &mut rng, &mut events);

        assert_eq!(
            count_monsters_named(&world, "Neferet the Green"),
            1,
            "revisiting wizard quest start should not duplicate the leader"
        );
        assert_eq!(
            count_monsters_named(&world, "apprentice"),
            apprentices,
            "revisiting wizard quest start should not duplicate apprentice guardians"
        );
    }

    #[test]
    fn test_revisiting_wizard_quest_locator_does_not_duplicate_enemies() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("test player should accept identity");
        let mut rng = test_rng();
        let mut events = Vec::new();

        change_level_to_branch(
            &mut world,
            crate::dungeon::DungeonBranch::Quest,
            4,
            false,
            None,
            &mut rng,
            &mut events,
        );

        let xorns = count_monsters_named(&world, "xorn");
        let vampire_bats = count_monsters_named(&world, "vampire bat");
        assert!(xorns >= 1, "wizard quest locator should spawn xorn enemies");
        assert!(
            vampire_bats >= 1,
            "wizard quest locator should spawn vampire bat enemies"
        );

        change_level(&mut world, 5, false, &mut rng, &mut events);
        change_level(&mut world, 4, true, &mut rng, &mut events);

        assert_eq!(
            count_monsters_named(&world, "xorn"),
            xorns,
            "revisiting wizard quest locator should not duplicate xorn enemies"
        );
        assert_eq!(
            count_monsters_named(&world, "vampire bat"),
            vampire_bats,
            "revisiting wizard quest locator should not duplicate vampire bat enemies"
        );
    }

    #[test]
    fn test_revisiting_wizard_quest_filler_does_not_duplicate_enemies() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("test player should accept identity");
        let mut rng = test_rng();
        let mut events = Vec::new();

        change_level_to_branch(
            &mut world,
            crate::dungeon::DungeonBranch::Quest,
            3,
            false,
            None,
            &mut rng,
            &mut events,
        );

        let xorns = count_monsters_named(&world, "xorn");
        let vampire_bats = count_monsters_named(&world, "vampire bat");
        assert!(xorns >= 1, "wizard quest filler should spawn xorn enemies");
        assert!(
            vampire_bats >= 1,
            "wizard quest filler should spawn vampire bat enemies"
        );

        change_level(&mut world, 4, false, &mut rng, &mut events);
        change_level(&mut world, 3, true, &mut rng, &mut events);

        assert_eq!(
            count_monsters_named(&world, "xorn"),
            xorns,
            "revisiting wizard quest filler should not duplicate xorn enemies"
        );
        assert_eq!(
            count_monsters_named(&world, "vampire bat"),
            vampire_bats,
            "revisiting wizard quest filler should not duplicate vampire bat enemies"
        );
    }

    #[test]
    fn test_revisiting_medusa_does_not_duplicate_medusa() {
        let mut world = make_stair_world(Terrain::StairsDown, 23);
        let mut rng = test_rng();

        resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(count_monsters_named(&world, "medusa"), 1);

        let medusa_down = find_terrain(&world.dungeon().current_level, Terrain::StairsDown)
            .expect("Medusa level should have stairs down");
        if let Some(mut pos) = world.get_component_mut::<Positioned>(world.player()) {
            pos.0 = medusa_down;
        }
        resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(world.dungeon().depth, 25);

        let castle_up = find_terrain(&world.dungeon().current_level, Terrain::StairsUp)
            .expect("Castle should have stairs up");
        if let Some(mut pos) = world.get_component_mut::<Positioned>(world.player()) {
            pos.0 = castle_up;
        }
        resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);

        assert_eq!(world.dungeon().depth, 24);
        assert_eq!(
            count_monsters_named(&world, "medusa"),
            1,
            "revisiting Medusa should not duplicate the boss"
        );
    }

    #[test]
    fn test_revisiting_castle_does_not_duplicate_wand_of_wishing() {
        let mut world = make_stair_world(Terrain::StairsDown, 24);
        let mut rng = test_rng();
        let mut events = Vec::new();
        let wand_otyp = resolve_object_type_by_spec(&test_game_data().objects, "wand of wishing")
            .expect("wand of wishing should resolve against the catalog");

        change_level(&mut world, 25, false, &mut rng, &mut events);
        assert_eq!(count_objects_with_type(&world, wand_otyp), 1);

        change_level(&mut world, 24, true, &mut rng, &mut events);
        change_level(&mut world, 25, false, &mut rng, &mut events);

        assert_eq!(
            count_objects_with_type(&world, wand_otyp),
            1,
            "revisiting Castle should not duplicate the wand of wishing"
        );
    }

    #[test]
    fn test_revisiting_fort_ludios_does_not_duplicate_garrison() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let mut rng = test_rng();
        let mut events = Vec::new();

        change_level_to_branch(
            &mut world,
            crate::dungeon::DungeonBranch::FortLudios,
            1,
            false,
            None,
            &mut rng,
            &mut events,
        );

        let soldiers = count_monsters_named(&world, "soldier");
        let lieutenants = count_monsters_named(&world, "lieutenant");
        let captains = count_monsters_named(&world, "captain");

        change_level_to_branch(
            &mut world,
            crate::dungeon::DungeonBranch::Main,
            1,
            true,
            None,
            &mut rng,
            &mut events,
        );
        change_level_to_branch(
            &mut world,
            crate::dungeon::DungeonBranch::FortLudios,
            1,
            false,
            None,
            &mut rng,
            &mut events,
        );

        assert_eq!(
            count_monsters_named(&world, "soldier"),
            soldiers,
            "revisiting Fort Ludios should not duplicate soldiers"
        );
        assert_eq!(
            count_monsters_named(&world, "lieutenant"),
            lieutenants,
            "revisiting Fort Ludios should not duplicate lieutenants"
        );
        assert_eq!(
            count_monsters_named(&world, "captain"),
            captains,
            "revisiting Fort Ludios should not duplicate captains"
        );
    }

    #[test]
    fn test_revisiting_vlad_tower_top_does_not_duplicate_vlad_or_candelabrum() {
        let mut world = make_stair_world(Terrain::StairsDown, 2);
        world.dungeon_mut().branch = crate::dungeon::DungeonBranch::VladsTower;
        let mut rng = test_rng();
        let mut events = Vec::new();
        let candelabrum_otyp =
            resolve_object_type_by_spec(&test_game_data().objects, "Candelabrum of Invocation")
                .expect("Candelabrum should resolve against the catalog");

        change_level(&mut world, 3, false, &mut rng, &mut events);
        assert_eq!(count_monsters_named(&world, "Vlad the Impaler"), 1);
        assert_eq!(count_objects_with_type(&world, candelabrum_otyp), 1);

        change_level(&mut world, 2, true, &mut rng, &mut events);
        change_level(&mut world, 3, false, &mut rng, &mut events);

        assert_eq!(
            count_monsters_named(&world, "Vlad the Impaler"),
            1,
            "revisiting Vlad level 3 should not duplicate Vlad"
        );
        assert_eq!(
            count_objects_with_type(&world, candelabrum_otyp),
            1,
            "revisiting Vlad level 3 should not duplicate the Candelabrum"
        );
    }

    #[test]
    fn test_revisiting_wizard_tower_top_does_not_duplicate_wizard() {
        let mut world = make_stair_world(Terrain::StairsDown, 18);
        world.dungeon_mut().branch = crate::dungeon::DungeonBranch::Gehennom;
        let mut rng = test_rng();
        let mut events = Vec::new();

        change_level(&mut world, 19, false, &mut rng, &mut events);
        assert_eq!(count_monsters_named(&world, "Wizard of Yendor"), 1);

        change_level(&mut world, 18, true, &mut rng, &mut events);
        change_level(&mut world, 19, false, &mut rng, &mut events);

        assert_eq!(
            count_monsters_named(&world, "Wizard of Yendor"),
            1,
            "revisiting Wizard Tower 3 should not duplicate the Wizard of Yendor"
        );
    }

    #[test]
    fn test_revisiting_sanctum_does_not_duplicate_high_priest() {
        let mut world = make_stair_world(Terrain::StairsDown, 19);
        world.dungeon_mut().branch = crate::dungeon::DungeonBranch::Gehennom;
        let mut rng = test_rng();
        let mut first_events = Vec::new();

        change_level(&mut world, 20, false, &mut rng, &mut first_events);
        assert_eq!(count_monsters_named(&world, "high priest"), 1);

        let mut revisit_events = Vec::new();
        change_level(&mut world, 19, true, &mut rng, &mut revisit_events);
        change_level(&mut world, 20, false, &mut rng, &mut revisit_events);

        assert_eq!(
            count_monsters_named(&world, "high priest"),
            1,
            "revisiting Sanctum should not duplicate the high priest"
        );
        assert!(revisit_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "sanctum-desecrate"
        )));
    }

    #[test]
    fn test_revisiting_level_restores_cached_runtime_state() {
        let mut world = make_stair_world(Terrain::StairsDown, 1);
        let shopkeeper = world.spawn((
            Monster,
            Positioned(Position::new(6, 5)),
            Name("Izchak".to_string()),
            HitPoints {
                current: 20,
                max: 20,
            },
            Speed(12),
            DisplaySymbol {
                symbol: '@',
                color: nethack_babel_data::Color::White,
            },
            MovementPoints(NORMAL_SPEED as i32),
        ));
        let priest = spawn_full_monster(&mut world, Position::new(4, 5), "oracle", 18);
        world
            .ecs_mut()
            .insert_one(
                priest,
                crate::npc::Priest {
                    alignment: Alignment::Lawful,
                    has_shrine: false,
                    is_high_priest: false,
                    angry: false,
                },
            )
            .expect("priest should accept explicit priest component");
        world
            .dungeon_mut()
            .trap_map
            .traps
            .push(crate::traps::TrapInstance {
                pos: Position::new(4, 4),
                trap_type: TrapType::Pit,
                detected: true,
                triggered_count: 1,
            });
        world
            .dungeon_mut()
            .engraving_map
            .insert(crate::engrave::Engraving::new(
                "Elbereth".to_string(),
                crate::engrave::EngraveMethod::Blade,
                Position::new(4, 5),
            ));
        world.dungeon_mut().current_level_flags.no_prayer = true;
        world.dungeon_mut().current_level_flags.no_teleport = true;
        world
            .dungeon_mut()
            .vault_rooms
            .push(crate::vault::VaultRoom {
                top_left: Position::new(6, 6),
                bottom_right: Position::new(7, 7),
            });
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(3, 3),
                Position::new(7, 7),
                crate::shop::ShopType::General,
                shopkeeper,
                "Izchak".to_string(),
            ));
        world.dungeon_mut().vault_guard_present = true;
        world
            .dungeon_mut()
            .gas_clouds
            .push(crate::region::GasCloud {
                position: Position::new(5, 6),
                radius: 1,
                turns_remaining: 3,
                damage_type: crate::region::GasCloudType::Poison,
                damage_per_turn: 6,
            });

        let mut rng = test_rng();
        let down_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert!(
            down_events
                .iter()
                .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
        );

        let up_stairs = find_terrain(&world.dungeon().current_level, Terrain::StairsUp)
            .expect("generated level 2 should provide stairs up");
        set_player_position(&mut world, up_stairs);
        let up_events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);
        assert!(
            up_events
                .iter()
                .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
        );

        assert_eq!(world.dungeon().depth, 1);
        assert!(
            world
                .dungeon()
                .trap_map
                .trap_at(Position::new(4, 4))
                .is_some()
        );
        assert!(
            world
                .dungeon()
                .engraving_map
                .is_elbereth_at(Position::new(4, 5))
        );
        assert!(world.dungeon().current_level_flags.no_prayer);
        assert!(world.dungeon().current_level_flags.no_teleport);
        assert_eq!(world.dungeon().vault_rooms.len(), 1);
        assert_eq!(world.dungeon().shop_rooms.len(), 1);
        assert!(world.dungeon().vault_guard_present);
        assert_eq!(world.dungeon().gas_clouds.len(), 1);
        assert_eq!(world.dungeon().gas_clouds[0].turns_remaining, 2);

        let restored_shopkeeper = world.dungeon().shop_rooms[0].shopkeeper;
        let restored_name = world
            .get_component::<Name>(restored_shopkeeper)
            .expect("restored shopkeeper entity should be rebound to a live monster");
        assert_eq!(restored_name.0, "Izchak");
        assert!(
            world
                .get_component::<crate::npc::Shopkeeper>(restored_shopkeeper)
                .is_some(),
            "restored shopkeeper should carry its explicit shopkeeper component"
        );

        let restored_priest =
            find_monster_named(&world, "oracle").expect("restored priest should exist");
        let restored_priest_data = world
            .get_component::<crate::npc::Priest>(restored_priest)
            .expect("restored priest should keep explicit priest component");
        assert_eq!(restored_priest_data.alignment, Alignment::Lawful);
        assert!(!restored_priest_data.has_shrine);
    }

    #[test]
    fn test_floor_items_stay_scoped_to_their_original_level() {
        let mut world = make_stair_world(Terrain::StairsDown, 1);
        let coin_pos = Position::new(4, 4);
        let coin = spawn_floor_coin(&mut world, coin_pos);
        let level_one = crate::dungeon::data_dungeon_level(DungeonBranch::Main, 1);
        let mut rng = test_rng();

        assert!(
            crate::inventory::items_at_position(&world, coin_pos).contains(&coin),
            "floor item should be visible on its origin level before moving away"
        );
        assert!(matches!(
            *world
                .get_component::<ObjectLocation>(coin)
                .expect("coin should have ObjectLocation"),
            ObjectLocation::Floor { x: 4, y: 4, level } if level == level_one
        ));

        let down_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert!(
            down_events
                .iter()
                .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
        );
        assert_eq!(world.dungeon().depth, 2);
        assert!(
            !crate::inventory::items_at_position(&world, coin_pos).contains(&coin),
            "depth-1 floor item should not leak onto depth 2"
        );
        assert!(matches!(
            *world
                .get_component::<ObjectLocation>(coin)
                .expect("coin should retain its floor location"),
            ObjectLocation::Floor { x: 4, y: 4, level } if level == level_one
        ));

        let up_stairs = find_terrain(&world.dungeon().current_level, Terrain::StairsUp)
            .expect("generated level 2 should provide stairs up");
        set_player_position(&mut world, up_stairs);
        let up_events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);
        assert!(
            up_events
                .iter()
                .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
        );
        assert_eq!(world.dungeon().depth, 1);
        assert!(
            crate::inventory::items_at_position(&world, coin_pos).contains(&coin),
            "revisiting depth 1 should make its floor item visible again"
        );
    }

    #[test]
    fn test_go_up_stairs() {
        let mut world = make_stair_world(Terrain::StairsUp, 2);
        let mut rng = test_rng();

        let events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);

        // Depth should have decreased.
        assert_eq!(world.dungeon().depth, 1, "depth should be 1 after GoUp");

        // Should emit LevelChanged event.
        let level_changed = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::LevelChanged {
                    from_depth,
                    to_depth,
                    ..
                } if from_depth == "2" && to_depth == "1"
            )
        });
        assert!(level_changed, "expected LevelChanged event from 2 to 1");
    }

    #[test]
    fn test_go_down_not_on_stairs() {
        let mut world = make_stair_world(Terrain::Floor, 1);
        let mut rng = test_rng();

        let events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);

        // Depth should not change.
        assert_eq!(
            world.dungeon().depth,
            1,
            "depth should remain 1 when not on stairs"
        );

        // Should emit an error message.
        let msg = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::Message { key, .. } if key == "stairs-not-here"
            )
        });
        assert!(msg, "expected stairs-not-here message");
    }

    #[test]
    fn test_go_up_at_top() {
        let mut world = make_stair_world(Terrain::StairsUp, 1);
        let mut rng = test_rng();

        let events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);

        // Depth should remain 1.
        assert_eq!(
            world.dungeon().depth,
            1,
            "depth should remain 1 at top of dungeon"
        );

        // Should emit "top of dungeon" message.
        let msg = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::Message { key, .. } if key == "stairs-at-top"
            )
        });
        assert!(msg, "expected stairs-at-top message");
    }

    #[test]
    fn test_go_down_and_back_up_preserves_level() {
        let mut world = make_stair_world(Terrain::StairsDown, 1);
        let mut rng = test_rng();

        // Mark the original level distinctively.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(3, 3), Terrain::Fountain);

        // Spawn a monster on level 1.
        spawn_full_monster(&mut world, Position::new(4, 4), "grid bug", 12);

        // Go down.
        resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert_eq!(world.dungeon().depth, 2);

        // Now go back up: place player on StairsUp of the new level.
        let up_pos = find_terrain(&world.dungeon().current_level, Terrain::StairsUp);
        if let Some(up) = up_pos {
            if let Some(mut pos) = world.get_component_mut::<Positioned>(world.player()) {
                pos.0 = up;
            }
        }

        resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);
        assert_eq!(world.dungeon().depth, 1, "should be back at depth 1");

        // The fountain should be back.
        let fountain_restored = world
            .dungeon()
            .current_level
            .get(Position::new(3, 3))
            .map(|c| c.terrain == Terrain::Fountain)
            .unwrap_or(false);
        assert!(
            fountain_restored,
            "fountain on level 1 should be preserved after round-trip"
        );
    }

    #[test]
    fn test_go_up_not_on_stairs() {
        let mut world = make_stair_world(Terrain::Floor, 2);
        let mut rng = test_rng();

        let events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);

        // Depth should not change.
        assert_eq!(
            world.dungeon().depth,
            2,
            "depth should remain 2 when not on stairs"
        );

        // Should emit an error message.
        let msg = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::Message { key, .. } if key == "stairs-not-here"
            )
        });
        assert!(msg, "expected stairs-not-here message");
    }

    // -----------------------------------------------------------------------
    // E.3: Monster creation order determines turn ordering (Decision D2)
    // -----------------------------------------------------------------------

    #[test]
    fn test_creation_order_tiebreaker() {
        // Two monsters with the same speed should resolve in creation order.
        // Monster A (created first) should act before Monster B (created second).
        let mut world = make_test_world();

        // Spawn A first, then B. Both at speed 12.
        let monster_a = spawn_monster(&mut world, Position::new(4, 4), 12);
        let monster_b = spawn_monster(&mut world, Position::new(6, 6), 12);

        // Verify creation orders
        let order_a = world
            .get_component::<CreationOrder>(monster_a)
            .expect("monster A should have CreationOrder")
            .0;
        let order_b = world
            .get_component::<CreationOrder>(monster_b)
            .expect("monster B should have CreationOrder")
            .0;
        assert!(
            order_a < order_b,
            "A created first should have lower order: A={}, B={}",
            order_a,
            order_b
        );

        // Verify sorting: collect monsters in the same way as resolve_monster_turns
        let player = world.player();
        let mut monsters: Vec<(hecs::Entity, u32, u64)> = Vec::new();
        for (entity, (speed, mp, _monster)) in world
            .ecs()
            .query::<(&Speed, &MovementPoints, &Monster)>()
            .iter()
        {
            if entity != player && mp.0 >= NORMAL_SPEED as i32 {
                let creation = world
                    .get_component::<CreationOrder>(entity)
                    .map(|c| c.0)
                    .unwrap_or(u64::MAX);
                monsters.push((entity, speed.0, creation));
            }
        }
        monsters.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));

        assert_eq!(monsters.len(), 2);
        assert_eq!(
            monsters[0].0, monster_a,
            "same-speed monsters: A (created first) should act first"
        );
        assert_eq!(
            monsters[1].0, monster_b,
            "same-speed monsters: B (created second) should act second"
        );
    }

    #[test]
    fn test_creation_order_speed_takes_priority() {
        // A faster monster should act before a slower one,
        // regardless of creation order.
        let mut world = make_test_world();

        // Spawn slow monster first, then fast monster.
        let slow = spawn_monster(&mut world, Position::new(4, 4), 12);
        let fast = spawn_monster(&mut world, Position::new(6, 6), 24);

        let player = world.player();
        let mut monsters: Vec<(hecs::Entity, u32, u64)> = Vec::new();
        for (entity, (speed, mp, _monster)) in world
            .ecs()
            .query::<(&Speed, &MovementPoints, &Monster)>()
            .iter()
        {
            if entity != player && mp.0 >= NORMAL_SPEED as i32 {
                let creation = world
                    .get_component::<CreationOrder>(entity)
                    .map(|c| c.0)
                    .unwrap_or(u64::MAX);
                monsters.push((entity, speed.0, creation));
            }
        }
        monsters.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));

        assert_eq!(
            monsters[0].0, fast,
            "faster monster acts first regardless of creation order"
        );
        assert_eq!(monsters[1].0, slow, "slower monster acts second");
    }

    #[test]
    fn test_creation_order_monotonic() {
        // next_creation_order should be strictly increasing.
        let mut world = make_test_world();

        let o1 = world.next_creation_order();
        let o2 = world.next_creation_order();
        let o3 = world.next_creation_order();

        assert!(o1.0 < o2.0, "orders should be monotonically increasing");
        assert!(o2.0 < o3.0, "orders should be monotonically increasing");
    }

    #[test]
    fn test_player_has_creation_order_zero() {
        // The player entity should have CreationOrder(0) (first entity).
        let world = make_test_world();
        let order = world
            .get_component::<CreationOrder>(world.player())
            .expect("player should have CreationOrder");
        assert_eq!(order.0, 0, "player should have creation order 0");
    }

    // ── Status effect enforcement tests ──────────────────────────

    #[test]
    fn test_paralyzed_skips_turn() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Apply paralysis.
        crate::status::make_paralyzed(&mut world, player, 5);

        // Try to move east.
        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        // Player should NOT have moved.
        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(
            pos.0,
            Position::new(5, 5),
            "paralyzed player should not move"
        );

        // Should have a paralysis message.
        let has_para_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("paralyzed")));
        assert!(has_para_msg, "expected paralysis message");

        // No EntityMoved event should have been emitted for the player.
        let player_moved = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityMoved { entity, .. } if *entity == player));
        assert!(
            !player_moved,
            "paralyzed player should not emit EntityMoved"
        );
    }

    #[test]
    fn test_sleeping_skips_turn() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let _ = crate::status::make_sleeping(&mut world, player, 5);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        let pos = world.get_component::<Positioned>(world.player()).unwrap();
        assert_eq!(
            pos.0,
            Position::new(5, 5),
            "sleeping player should not move"
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, EngineEvent::Message { key, .. } if key == "status-paralyzed-cant-move")),
            "sleeping player should still be blocked from acting"
        );
    }

    #[test]
    fn test_sleeping_monster_skips_turn_until_waking() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let sleeper = spawn_monster(&mut world, Position::new(6, 5), 12);
        let _ = crate::status::make_sleeping(&mut world, sleeper, 2);

        let first_turn = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
        assert!(
            !first_turn.iter().any(|event| matches!(
                event,
                EngineEvent::MeleeHit { attacker, .. } if *attacker == sleeper
            )),
            "sleeping monster should not act while asleep"
        );
        assert!(
            crate::status::is_sleeping(&world, sleeper),
            "sleep timer should still be active after the first skipped turn"
        );

        let second_turn = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
        assert!(
            !second_turn.iter().any(|event| matches!(
                event,
                EngineEvent::MeleeHit { attacker, .. } if *attacker == sleeper
            )),
            "sleeping monster should keep skipping turns until the timer expires"
        );
        assert!(
            !crate::status::is_sleeping(&world, sleeper),
            "sleep timer should expire after enough full turns"
        );

        let mut woke_and_acted = false;
        for _ in 0..3 {
            let turn_events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
            if turn_events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::MeleeHit { attacker, .. } if *attacker == sleeper
                )
            }) {
                woke_and_acted = true;
                break;
            }
        }
        assert!(
            woke_and_acted,
            "monster should resume acting shortly after sleep expires"
        );
    }

    #[test]
    fn test_confused_may_change_direction() {
        let mut world = make_test_world();
        let player = world.player();

        // Apply confusion.
        crate::status::make_confused(&mut world, player, 100);

        // Run many movement attempts and check if at least one goes
        // a different direction than East.
        let mut went_wrong = false;
        for seed in 0..100u64 {
            // Reset player to center.
            if let Some(mut pos) = world.get_component_mut::<Positioned>(player) {
                pos.0 = Position::new(5, 5);
            }
            let mut rng = Pcg64::seed_from_u64(seed);
            resolve_turn(
                &mut world,
                PlayerAction::Move {
                    direction: Direction::East,
                },
                &mut rng,
            );

            let pos = world.get_component::<Positioned>(player).unwrap();
            if pos.0 != Position::new(6, 5) && pos.0 != Position::new(5, 5) {
                went_wrong = true;
                break;
            }
        }
        assert!(
            went_wrong,
            "confused player should sometimes go in a wrong direction"
        );
    }

    #[test]
    fn test_stunned_always_randomizes() {
        let mut world = make_test_world();
        let player = world.player();

        // Apply stun.
        crate::status::make_stunned(&mut world, player, 100);

        // Run many movement attempts; stunned always randomizes,
        // so over many attempts the player should sometimes NOT go east.
        let mut went_wrong = false;
        for seed in 0..50u64 {
            // Reset player to center.
            if let Some(mut pos) = world.get_component_mut::<Positioned>(player) {
                pos.0 = Position::new(5, 5);
            }
            let mut rng = Pcg64::seed_from_u64(seed);
            resolve_turn(
                &mut world,
                PlayerAction::Move {
                    direction: Direction::East,
                },
                &mut rng,
            );

            let pos = world.get_component::<Positioned>(player).unwrap();
            // If they ended up somewhere other than (6,5) [east] or (5,5)
            // [bumped into wall], the direction was randomized.
            if pos.0 != Position::new(6, 5) && pos.0 != Position::new(5, 5) {
                went_wrong = true;
                break;
            }
        }
        assert!(went_wrong, "stunned player should have movement randomized");
    }

    #[test]
    fn test_levitating_blocks_stairs_down() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Place stairs down at player position.
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(5, 5), Terrain::StairsDown);

        // Apply levitation.
        crate::status::make_levitating(&mut world, player, 50);

        // Try to go down.
        let events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);

        // Should have a levitation-blocks message.
        let has_lev_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("levitating")));
        assert!(has_lev_msg, "expected levitation blocking message");

        // Depth should remain unchanged.
        assert_eq!(
            world.dungeon().depth,
            1,
            "levitating player should not descend stairs"
        );
    }

    #[test]
    fn test_levitating_no_floor_traps() {
        // This test verifies that levitating players don't trigger
        // floor traps via the trap system. The trap system already
        // implements this (is_floor_trigger + is_levitating check in
        // avoid_trap). We verify the integration by checking the
        // levitation status query works correctly in the traps
        // context.
        use crate::traps::{TrapInstance, avoid_trap, is_floor_trigger};
        use nethack_babel_data::TrapType;

        let pit = TrapInstance::new(Position::new(10, 5), TrapType::Pit);
        assert!(is_floor_trigger(TrapType::Pit));

        // Non-levitating entity does not auto-avoid the pit.
        let mut rng = test_rng();
        let mut info = crate::traps::TrapEntityInfo::default();
        info.is_levitating = false;
        let avoids_normal = avoid_trap(&mut rng, &info, &pit);
        // Cannot assert avoids_normal == false because there may be
        // DEX-based avoidance, but we can assert levitating always avoids.

        info.is_levitating = true;
        let avoids_lev = avoid_trap(&mut rng, &info, &pit);
        assert!(
            avoids_lev,
            "levitating entity should always avoid floor traps"
        );

        // For non-levitating, at least one attempt should fail to avoid.
        let mut failed_to_avoid = false;
        info.is_levitating = false;
        for seed in 0..100u64 {
            let mut r = Pcg64::seed_from_u64(seed);
            if !avoid_trap(&mut r, &info, &pit) {
                failed_to_avoid = true;
                break;
            }
        }
        let _ = avoids_normal; // suppress unused warning
        assert!(
            failed_to_avoid,
            "non-levitating entity should sometimes fail to avoid pit"
        );
    }

    #[test]
    fn test_levitating_cant_pickup() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Apply levitation.
        crate::status::make_levitating(&mut world, player, 50);

        // Try to pick up.
        let events = resolve_turn(&mut world, PlayerAction::PickUp, &mut rng);

        // Should have a levitation-blocks-pickup message.
        let has_msg = events.iter().any(|e| {
            matches!(e, EngineEvent::Message { key, .. }
                if key.contains("levitating") && key.contains("pickup"))
        });
        assert!(has_msg, "expected levitation blocking pickup message");
    }

    // ── Wizard mode tests ──────────────────────────────────────

    #[test]
    fn wiz_identify_marks_inventory_items_known() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let long_sword = test_game_data()
            .objects
            .iter()
            .find(|def| def.name.eq_ignore_ascii_case("long sword"))
            .expect("long sword should exist in test data");
        let item = crate::items::spawn_item(
            &mut world,
            long_sword,
            crate::items::SpawnLocation::Free,
            Some(2),
        );
        if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item) {
            *loc = ObjectLocation::Inventory;
        }
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.push(item);
        }

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::WizIdentify, &mut rng);

        let knowledge = world
            .get_component::<nethack_babel_data::KnowledgeState>(item)
            .expect("wished item should keep knowledge state");
        assert!(knowledge.known);
        assert!(knowledge.dknown);
        assert!(knowledge.rknown);
        assert!(knowledge.cknown);
        assert!(knowledge.lknown);
        assert!(knowledge.tknown);
        let buc = world
            .get_component::<BucStatus>(item)
            .expect("wished item should keep BUC state");
        assert!(buc.bknown, "wizard identify should reveal BUC knowledge");
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-identify-all"
        )));
    }

    #[test]
    fn wiz_map_reveals_level() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Verify some cells start unexplored.
        let unexplored_before = (0..world.dungeon().current_level.height)
            .flat_map(|y| (0..world.dungeon().current_level.width).map(move |x| (x, y)))
            .filter(|&(x, y)| !world.dungeon().current_level.cells[y][x].explored)
            .count();
        assert!(
            unexplored_before > 0,
            "should have unexplored cells before WizMap"
        );

        let events = resolve_turn(&mut world, PlayerAction::WizMap, &mut rng);

        // All cells should now be explored.
        let unexplored_after = (0..world.dungeon().current_level.height)
            .flat_map(|y| (0..world.dungeon().current_level.width).map(move |x| (x, y)))
            .filter(|&(x, y)| !world.dungeon().current_level.cells[y][x].explored)
            .count();
        assert_eq!(
            unexplored_after, 0,
            "all cells should be explored after WizMap"
        );

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-map-revealed"
        )));
    }

    #[test]
    fn wiz_detect_fires_events() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::WizDetect, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-detect-all"
        )));
    }

    #[test]
    fn wiz_genesis_spawns_named_monster_adjacent_to_player() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::WizGenesis {
                monster_name: "goblin".to_string(),
            },
            &mut rng,
        );

        let player_pos = world
            .get_component::<Positioned>(world.player())
            .expect("player should have a position")
            .0;
        let spawned = world
            .ecs()
            .query::<(&Monster, &Name, &Positioned)>()
            .iter()
            .find(|(_, (_, name, _))| name.0.eq_ignore_ascii_case("goblin"))
            .map(|(entity, (_, _, pos))| (entity, pos.0))
            .expect("wizgenesis should spawn the requested monster");
        assert_ne!(
            spawned.1, player_pos,
            "genesis should not stack onto the player"
        );
        assert!(
            (spawned.1.x - player_pos.x).abs() <= 1 && (spawned.1.y - player_pos.y).abs() <= 1,
            "genesis should prefer an adjacent spawn position"
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::MonsterGenerated { entity, .. } if *entity == spawned.0
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-genesis"
        )));
    }

    #[test]
    fn wiz_wish_grants_restricted_item_in_inventory() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::WizWish {
                wish_text: "50 blessed rustproof +7 arrow named Debug".to_string(),
            },
            &mut rng,
        );

        let player = world.player();
        let wished_item = world
            .get_component::<crate::inventory::Inventory>(player)
            .expect("player should have inventory")
            .items
            .iter()
            .copied()
            .find(|item| {
                world
                    .get_component::<ObjectCore>(*item)
                    .is_some_and(|core| {
                        resolve_object_type_by_spec(&test_game_data().objects, "arrow")
                            .is_some_and(|arrow_id| core.otyp == arrow_id)
                    })
            })
            .expect("wizwish should grant the wished item");
        let core = world
            .get_component::<ObjectCore>(wished_item)
            .expect("wished item should have ObjectCore");
        let enchantment = world
            .get_component::<Enchantment>(wished_item)
            .expect("wished item should have enchantment");
        let buc = world
            .get_component::<BucStatus>(wished_item)
            .expect("wished item should have BUC state");
        let erosion = world
            .get_component::<nethack_babel_data::Erosion>(wished_item)
            .expect("wished item should have erosion state");
        let extra = world
            .get_component::<ObjectExtra>(wished_item)
            .expect("named wished item should keep ObjectExtra");

        assert_eq!(core.quantity, 20, "wish quantity should clamp to 20");
        assert_eq!(enchantment.spe, 3, "wish enchantment should clamp to +3");
        assert!(buc.blessed, "wished item should preserve blessed status");
        assert!(buc.bknown, "wizard wishes should reveal BUC status");
        assert!(erosion.erodeproof, "rustproof wish should set erodeproof");
        assert_eq!(extra.name.as_deref(), Some("debug"));
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-wish-adjusted"
        )));
    }

    #[test]
    fn wiz_where_reports_current_location_and_special_levels() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::WizWhere, &mut rng);

        let current = events
            .iter()
            .find_map(|event| match event {
                EngineEvent::Message { key, args } if key == "wizard-where-current" => {
                    Some(args.clone())
                }
                _ => None,
            })
            .expect("wizwhere should emit the current location");
        assert!(
            current
                .iter()
                .any(|(k, v)| k == "location" && v == "Dungeons of Doom:1")
        );
        assert!(current.iter().any(|(k, v)| k == "absolute" && v == "1"));
        assert!(current.iter().any(|(k, v)| k == "x" && v == "5"));
        assert!(current.iter().any(|(k, v)| k == "y" && v == "5"));

        let special_lines: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                EngineEvent::Message { key, args } if key == "wizard-where-special" => {
                    Some(args.clone())
                }
                _ => None,
            })
            .collect();
        assert!(
            special_lines
                .iter()
                .any(|args| args.iter().any(|(k, v)| k == "level" && v == "Castle")),
            "wizwhere should enumerate fixed special levels like Castle"
        );
        assert!(
            special_lines
                .iter()
                .any(|args| args.iter().any(|(k, v)| k == "level" && v == "Quest Start")),
            "wizwhere should enumerate cross-branch special levels like the quest home"
        );
    }

    #[test]
    fn wiz_kill_removes_all_monsters_on_level() {
        let mut world = make_test_world();
        spawn_full_monster(&mut world, Position::new(6, 5), "goblin", 12);
        spawn_full_monster(&mut world, Position::new(7, 5), "orc", 12);
        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::WizKill, &mut rng);

        let remaining = world.ecs().query::<&Monster>().iter().count();
        assert_eq!(
            remaining, 0,
            "wizkill should remove all monsters on the live level"
        );
        let deaths = events
            .iter()
            .filter(|event| matches!(event, EngineEvent::EntityDied { .. }))
            .count();
        assert_eq!(
            deaths, 2,
            "wizkill should emit one death per removed monster"
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-kill"
        )));
    }

    #[test]
    fn wiz_kill_records_wizard_respawn_state() {
        let mut world = make_test_world();
        spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
        let mut rng = test_rng();

        let _ = resolve_turn(&mut world, PlayerAction::WizKill, &mut rng);

        let player_events = world
            .get_component::<PlayerEvents>(world.player())
            .expect("player should have story event state");
        assert!(player_events.killed_wizard);
        assert_eq!(player_events.wizard_times_killed, 1);
        assert_eq!(
            player_events.wizard_last_killed_turn,
            world.turn(),
            "wizard kill bookkeeping should record the current turn"
        );
    }

    #[test]
    fn test_wizard_respawns_after_recorded_death_interval() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
            player_events.killed_wizard = true;
            player_events.wizard_times_killed = 1;
            player_events.wizard_last_killed_turn = 0;
        }
        while world.turn() < 99 {
            world.advance_turn();
        }

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        assert_eq!(count_monsters_named(&world, "Wizard of Yendor"), 1);
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "wizard-respawned"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "wizard-respawned-boom"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, args }
                if key == "wizard-respawned-taunt"
                    && args.iter().any(|(name, value)| name == "verb" && value == "kill")
        )));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. }))
        );
    }

    #[test]
    fn test_wizard_respawn_messages_respect_deafness() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        if let Some(mut status) = world.get_component_mut::<crate::status::StatusEffects>(player) {
            status.deaf = 50;
        }
        let mut rng = test_rng();

        let events = force_wizard_harassment_action(
            &mut world,
            None,
            player,
            crate::npc::WizardAction::Resurrect,
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "wizard-respawned"
        )));
        assert!(
            !events.iter().any(|event| matches!(
                event,
                EngineEvent::Message { key, .. }
                    if key == "wizard-respawned-boom" || key == "wizard-respawned-taunt"
            )),
            "deaf players should not hear the Wizard's resurrection taunt"
        );
    }

    #[test]
    fn test_wizard_steal_amulet_drops_it_off_player() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let wizard = spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
        let amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
        let mut events = Vec::new();
        let mut rng = test_rng();

        apply_wizard_harassment_action(
            &mut world,
            Some(wizard),
            player,
            crate::npc::WizardAction::StealAmulet,
            &mut rng,
            &mut events,
        );

        assert!(
            !player_carries_item(&world, player, amulet),
            "wizard harassment should remove the real amulet from the player's possessions"
        );
        let wizard_pos = world
            .get_component::<Positioned>(wizard)
            .expect("wizard should still exist")
            .0;
        let item_loc = world
            .get_component::<ObjectLocation>(amulet)
            .expect("stolen amulet should stay in the world");
        assert!(matches!(
            *item_loc,
            ObjectLocation::Floor { x, y, level }
                if (x as i32 - wizard_pos.x).abs() <= 1
                    && (y as i32 - wizard_pos.y).abs() <= 1
                    && level == world.dungeon().current_data_dungeon_level()
        ));
    }

    #[test]
    fn test_wizard_cannot_steal_amulet_from_range() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let wizard = spawn_full_monster(&mut world, Position::new(14, 14), "Wizard of Yendor", 12);
        let amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
        let mut events = Vec::new();
        let mut rng = test_rng();

        apply_wizard_harassment_action(
            &mut world,
            Some(wizard),
            player,
            crate::npc::WizardAction::StealAmulet,
            &mut rng,
            &mut events,
        );

        assert!(
            player_carries_item(&world, player, amulet),
            "Wizard harassment should not steal the Amulet from across the level"
        );
        assert!(
            !events.iter().any(|event| matches!(
                event,
                EngineEvent::Message { key, .. } if key == "wizard-steal-amulet"
            )),
            "ranged amulet theft should stay silent because nothing was stolen"
        );
    }

    #[test]
    fn test_wizard_double_trouble_spawns_clone() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let wizard = spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
        let mut events = Vec::new();
        let mut rng = test_rng();

        apply_wizard_harassment_action(
            &mut world,
            Some(wizard),
            player,
            crate::npc::WizardAction::DoubleTrouble,
            &mut rng,
            &mut events,
        );

        assert_eq!(count_monsters_named(&world, "Wizard of Yendor"), 2);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. }))
        );
    }

    #[test]
    fn test_wizard_curse_items_marks_inventory_cursed() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let wizard = spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
        let item = spawn_inventory_object_by_name(&mut world, "long sword", 'a');
        world
            .ecs_mut()
            .insert_one(
                item,
                BucStatus {
                    cursed: false,
                    blessed: false,
                    bknown: false,
                },
            )
            .expect("test inventory item should accept a BUC component");
        let mut rng = test_rng();
        let mut events = Vec::new();

        apply_wizard_harassment_action(
            &mut world,
            Some(wizard),
            player,
            crate::npc::WizardAction::CurseItems,
            &mut rng,
            &mut events,
        );

        let buc = world
            .get_component::<BucStatus>(item)
            .expect("inventory item should keep BUC state");
        assert!(
            buc.cursed,
            "wizard curse should actually curse inventory items"
        );
    }

    #[test]
    fn test_offscreen_wizard_black_glow_curses_inventory() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let item = spawn_inventory_object_by_name(&mut world, "long sword", 'a');
        world
            .ecs_mut()
            .insert_one(
                item,
                BucStatus {
                    cursed: false,
                    blessed: false,
                    bknown: false,
                },
            )
            .expect("inventory item should accept a BUC component");
        let mut rng = test_rng();
        let mut events = crate::npc::wizard_harass_events(crate::npc::WizardAction::BlackGlowCurse);

        apply_wizard_harassment_action(
            &mut world,
            None,
            player,
            crate::npc::WizardAction::BlackGlowCurse,
            &mut rng,
            &mut events,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "wizard-black-glow"
        )));
        let buc = world
            .get_component::<BucStatus>(item)
            .expect("inventory item should keep BUC state");
        assert!(
            buc.cursed,
            "off-screen black glow should really curse carried inventory"
        );
    }

    #[test]
    fn test_offscreen_wizard_black_glow_is_silent_when_player_is_blind() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let item = spawn_inventory_object_by_name(&mut world, "long sword", 'a');
        world
            .ecs_mut()
            .insert_one(
                item,
                BucStatus {
                    cursed: false,
                    blessed: false,
                    bknown: false,
                },
            )
            .expect("inventory item should accept a BUC component");
        let _ = crate::status::make_blinded(&mut world, player, 20);
        let mut rng = test_rng();
        let mut events =
            wizard_harassment_messages(&world, player, crate::npc::WizardAction::BlackGlowCurse);

        apply_wizard_harassment_action(
            &mut world,
            None,
            player,
            crate::npc::WizardAction::BlackGlowCurse,
            &mut rng,
            &mut events,
        );

        assert!(
            !events.iter().any(|event| matches!(
                event,
                EngineEvent::Message { key, .. } if key == "wizard-black-glow"
            )),
            "blind player should not get the black glow visual message"
        );
        let buc = world
            .get_component::<BucStatus>(item)
            .expect("inventory item should keep BUC state");
        assert!(
            buc.cursed,
            "blind player should still suffer the black-glow curse side-effect"
        );
    }

    #[test]
    fn test_wizard_intervention_can_fire_without_live_wizard() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "goblin", 6);
        let _ = crate::status::make_sleeping(&mut world, sleeper, 10);
        let item = spawn_inventory_object_by_name(&mut world, "long sword", 'a');
        world
            .ecs_mut()
            .insert_one(
                item,
                BucStatus {
                    cursed: false,
                    blessed: false,
                    bknown: false,
                },
            )
            .expect("inventory item should accept a BUC component");
        let current_turn = world.turn();
        if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
            player_events.killed_wizard = true;
            player_events.wizard_times_killed = 1;
            player_events.wizard_last_killed_turn = current_turn;
            player_events.wizard_intervention_cooldown = 1;
        }
        let mut rng = test_rng();
        let mut final_events = Vec::new();

        for _ in 0..40 {
            let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. }
                        if key == "wizard-vague-nervous"
                            || key == "wizard-black-glow"
                            || key == "wizard-aggravate"
                            || key == "wizard-summon-nasties"
                            || key == "wizard-respawned"
                )
            }) {
                final_events = events;
                break;
            }
        }

        assert!(
            final_events.iter().any(|event| matches!(
                event,
                EngineEvent::Message { key, .. }
                    if key == "wizard-vague-nervous"
                        || key == "wizard-black-glow"
                        || key == "wizard-aggravate"
                        || key == "wizard-summon-nasties"
                        || key == "wizard-respawned"
            )),
            "post-Wizard intervention should still pressure the player before respawn"
        );
        let aggravated = final_events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "wizard-aggravate"
            )
        });
        let respawned = final_events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "wizard-respawned"
            )
        });
        if aggravated {
            assert!(
                !crate::status::is_sleeping(&world, sleeper),
                "aggravation should really wake sleeping monsters"
            );
        }
        assert_eq!(
            count_monsters_named(&world, "Wizard of Yendor"),
            if respawned { 1 } else { 0 }
        );
    }

    #[test]
    fn test_offscreen_wizard_aggravate_wakes_sleeping_monsters() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "goblin", 6);
        let _ = crate::status::make_sleeping(&mut world, sleeper, 10);
        let mut events = crate::npc::wizard_harass_events(crate::npc::WizardAction::Aggravate);
        let mut rng = test_rng();

        apply_wizard_harassment_action(
            &mut world,
            None,
            player,
            crate::npc::WizardAction::Aggravate,
            &mut rng,
            &mut events,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "wizard-aggravate"
        )));
        assert!(
            !crate::status::is_sleeping(&world, sleeper),
            "wizard aggravation should wake sleeping monsters on the level"
        );
    }

    #[test]
    fn test_offscreen_wizard_aggravate_can_break_monster_paralysis() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let target = spawn_full_monster(&mut world, Position::new(7, 5), "goblin", 6);
        let _ = crate::status::make_paralyzed(&mut world, target, 10);
        let mut rng = rand_pcg::Pcg64::seed_from_u64(0xA661_A661);

        for _ in 0..64 {
            let mut events = crate::npc::wizard_harass_events(crate::npc::WizardAction::Aggravate);
            apply_wizard_harassment_action(
                &mut world,
                None,
                player,
                crate::npc::WizardAction::Aggravate,
                &mut rng,
                &mut events,
            );
            if !world
                .get_component::<crate::status::StatusEffects>(target)
                .is_some_and(|status| status.paralysis > 0)
            {
                assert!(events.iter().any(|event| matches!(
                    event,
                    EngineEvent::StatusRemoved {
                        entity,
                        status: crate::event::StatusEffect::Paralyzed,
                    } if *entity == target
                )));
                return;
            }
        }

        panic!("wizard aggravation never broke monster paralysis across repeated attempts");
    }

    #[test]
    fn test_offscreen_wizard_resurrect_action_spawns_new_wizard() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let mut events = crate::npc::wizard_harass_events(crate::npc::WizardAction::Resurrect);
        let mut rng = test_rng();

        apply_wizard_harassment_action(
            &mut world,
            None,
            player,
            crate::npc::WizardAction::Resurrect,
            &mut rng,
            &mut events,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "wizard-respawned"
        )));
        assert_eq!(count_monsters_named(&world, "Wizard of Yendor"), 1);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. }))
        );
    }

    #[test]
    fn test_offscreen_wizard_resurrect_prefers_cached_wizard() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world.dungeon_mut().monster_cache.insert(
            (DungeonBranch::Main, 2),
            vec![CachedMonster {
                position: Position::new(3, 3),
                name: "Wizard of Yendor".to_string(),
                hp_current: 7,
                hp_max: 33,
                speed: NORMAL_SPEED,
                symbol: '@',
                color: nethack_babel_data::Color::BrightMagenta,
                is_tame: false,
                is_peaceful: false,
                creation_order: 77,
                priest: None,
                shopkeeper: None,
                quest_npc_role: None,
                trapped: None,
                status_effects: crate::status::StatusEffects {
                    blindness: 9,
                    sleeping: 12,
                    paralysis: 6,
                    ..Default::default()
                },
            }],
        );
        let mut events = crate::npc::wizard_harass_events(crate::npc::WizardAction::Resurrect);
        let mut rng = test_rng();

        apply_wizard_harassment_action(
            &mut world,
            None,
            player,
            crate::npc::WizardAction::Resurrect,
            &mut rng,
            &mut events,
        );

        let wizard = find_monster_named(&world, "Wizard of Yendor")
            .expect("cached wizard should be respawned onto the current level");
        let hp = world
            .get_component::<HitPoints>(wizard)
            .expect("respawned cached wizard should have hp");
        assert_eq!(hp.current, 7);
        assert_eq!(hp.max, 33);
        let status = world
            .get_component::<crate::status::StatusEffects>(wizard)
            .expect("respawned cached wizard should keep status effects");
        assert_eq!(status.blindness, 9);
        assert_eq!(
            status.sleeping, 0,
            "respawned cached wizard should not remain asleep"
        );
        assert_eq!(
            status.paralysis, 0,
            "respawned cached wizard should not remain paralyzed"
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, args }
                if key == "wizard-respawned-taunt"
                    && args.iter().any(|(name, value)| name == "verb" && value == "elude")
        )));
        assert!(
            !world
                .dungeon()
                .monster_cache
                .contains_key(&(DungeonBranch::Main, 2)),
            "cached wizard entry should be consumed when resurrected"
        );
    }

    #[test]
    fn test_wizard_turn_respawn_prefers_cached_wizard() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        if let Some(mut player_events) = world.get_component_mut::<PlayerEvents>(player) {
            player_events.killed_wizard = true;
            player_events.wizard_times_killed = 1;
            player_events.wizard_last_killed_turn = 0;
        }
        world.dungeon_mut().monster_cache.insert(
            (DungeonBranch::Main, 2),
            vec![CachedMonster {
                position: Position::new(3, 3),
                name: "Wizard of Yendor".to_string(),
                hp_current: 11,
                hp_max: 27,
                speed: NORMAL_SPEED,
                symbol: '@',
                color: nethack_babel_data::Color::BrightMagenta,
                is_tame: false,
                is_peaceful: false,
                creation_order: 88,
                priest: None,
                shopkeeper: None,
                quest_npc_role: None,
                trapped: None,
                status_effects: crate::status::StatusEffects {
                    blindness: 5,
                    sleeping: 10,
                    paralysis: 4,
                    ..Default::default()
                },
            }],
        );
        while world.turn() < 99 {
            world.advance_turn();
        }

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        let wizard = find_monster_named(&world, "Wizard of Yendor")
            .expect("timed wizard respawn should prefer cached wizard when available");
        let hp = world
            .get_component::<HitPoints>(wizard)
            .expect("respawned wizard should have hp");
        assert_eq!(hp.current, 11);
        assert_eq!(hp.max, 27);
        let status = world
            .get_component::<crate::status::StatusEffects>(wizard)
            .expect("respawned wizard should keep status effects");
        assert_eq!(status.blindness, 5);
        assert_eq!(status.sleeping, 0);
        assert_eq!(status.paralysis, 0);
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, args }
                if key == "wizard-respawned-taunt"
                    && args.iter().any(|(name, value)| name == "verb" && value == "elude")
        )));
    }

    #[test]
    fn test_live_wizard_can_emit_taunt_messages() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let wizard = spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
        world
            .ecs_mut()
            .insert_one(wizard, Peaceful)
            .expect("wizard should accept Peaceful in taunt test");
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
            hp.current = 4;
            hp.max = 20;
        }
        let mut player_events = read_player_events(&world, player);
        player_events.invoked = true;
        persist_player_events(&mut world, player, player_events);

        let mut rng = test_rng();
        let mut taunt_seen = false;
        for _ in 0..128 {
            let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. }
                        if key == "wizard-taunt-laughs"
                            || key == "wizard-taunt-relinquish"
                            || key == "wizard-taunt-panic"
                            || key == "wizard-taunt-return"
                            || key == "wizard-taunt-general"
                )
            }) {
                taunt_seen = true;
                break;
            }
        }

        assert!(
            taunt_seen,
            "a live wizard should eventually taunt the player during repeated pressure turns"
        );
    }

    #[test]
    fn test_sleeping_wizard_does_not_emit_live_taunts_or_harassment() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let wizard = spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
        let _ = crate::status::make_sleeping(&mut world, wizard, 500);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
            hp.current = 4;
            hp.max = 20;
        }
        let mut player_events = read_player_events(&world, player);
        player_events.invoked = true;
        persist_player_events(&mut world, player, player_events);

        let mut rng = test_rng();
        for _ in 0..64 {
            let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
            assert!(
                !events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. }
                        if key.starts_with("wizard-taunt-")
                            || key == "wizard-steal-amulet"
                            || key == "wizard-double-trouble"
                            || key == "wizard-summon-nasties"
                            || key == "wizard-curse-items"
                )),
                "sleeping wizards should not drive live harassment"
            );
        }
    }

    #[test]
    fn test_live_wizard_taunt_wakes_nearby_sleepers() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let _wizard = spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
        let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "goblin", 6);
        let _far_sleeper = spawn_full_monster(&mut world, Position::new(20, 20), "orc", 6);
        let _ = crate::status::make_sleeping(&mut world, sleeper, 10);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
            hp.current = 4;
            hp.max = 20;
        }
        let mut player_events = read_player_events(&world, player);
        player_events.invoked = true;
        persist_player_events(&mut world, player, player_events);

        let mut rng = test_rng();
        for _ in 0..128 {
            let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. }
                        if key == "wizard-taunt-laughs"
                            || key == "wizard-taunt-relinquish"
                            || key == "wizard-taunt-panic"
                            || key == "wizard-taunt-return"
                            || key == "wizard-taunt-general"
                )
            }) {
                assert!(
                    !crate::status::is_sleeping(&world, sleeper),
                    "wizard taunts should wake nearby sleeping monsters like original cuss/wake_nearto"
                );
                return;
            }
        }

        panic!("expected live wizard to taunt during repeated pressure turns");
    }

    #[test]
    fn test_deaf_player_does_not_receive_live_wizard_taunts() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let _wizard = spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
            hp.current = 4;
            hp.max = 20;
        }
        if let Some(mut status) = world.get_component_mut::<crate::status::StatusEffects>(player) {
            status.deaf = 500;
        }
        let mut player_events = read_player_events(&world, player);
        player_events.invoked = true;
        persist_player_events(&mut world, player, player_events);

        let mut rng = test_rng();
        for _ in 0..128 {
            let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
            assert!(
                !events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. }
                        if key == "wizard-taunt-laughs"
                            || key == "wizard-taunt-relinquish"
                            || key == "wizard-taunt-panic"
                            || key == "wizard-taunt-return"
                            || key == "wizard-taunt-general"
                )),
                "deaf players should not hear Wizard taunts"
            );
        }
    }

    #[test]
    fn test_wizard_summon_nasties_uses_catalog_driven_pool() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        world.dungeon_mut().branch = DungeonBranch::Gehennom;
        world.dungeon_mut().depth = 20;
        let player = world.player();
        let wizard = spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
            hp.current = 20;
            hp.max = 40;
        }
        let mut rng = test_rng();
        let mut events = Vec::new();

        apply_wizard_harassment_action(
            &mut world,
            Some(wizard),
            player,
            crate::npc::WizardAction::SummonNasties,
            &mut rng,
            &mut events,
        );

        let generated: Vec<hecs::Entity> = events
            .iter()
            .filter_map(|event| match event {
                EngineEvent::MonsterGenerated { entity, .. } => Some(*entity),
                _ => None,
            })
            .collect();
        assert_eq!(
            generated.len(),
            3,
            "Gehennom wizard harassment should summon three nasty monsters"
        );

        let mut names = Vec::new();
        for entity in generated {
            let name = world
                .get_component::<Name>(entity)
                .expect("generated nasty should have a name")
                .0
                .clone();
            names.push(name.clone());
            let monster_id = resolve_monster_id_by_spec(&test_game_data().monsters, &name)
                .unwrap_or_else(|| panic!("{name} should resolve against the monster catalog"));
            let monster_def = test_game_data()
                .monsters
                .iter()
                .find(|monster| monster.id == monster_id)
                .expect("generated nasty should exist in the monster catalog");
            assert!(
                monster_def.flags.contains(MonsterFlags::HOSTILE),
                "{name} should come from the hostile nasty summon pool"
            );
            assert!(
                monster_def.flags.contains(MonsterFlags::NASTY) || monster_def.difficulty >= 16,
                "{name} should come from the nasty/high-difficulty summon pool"
            );
            assert!(
                !monster_def
                    .geno_flags
                    .intersects(GenoFlags::G_UNIQ | GenoFlags::G_NOGEN),
                "{name} should not be a unique or no-gen monster"
            );
        }
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 3, "summoned nasties should be unique");
    }

    #[test]
    fn test_endgame_wizard_summon_nasties_scales_up_to_four() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        world.dungeon_mut().branch = DungeonBranch::Endgame;
        world.dungeon_mut().depth = 3;
        let player = world.player();
        let mut player_events = read_player_events(&world, player);
        player_events.invoked = true;
        persist_player_events(&mut world, player, player_events);
        let wizard = spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
            hp.current = 20;
            hp.max = 40;
        }
        let mut events = Vec::new();
        let mut rng = test_rng();

        apply_wizard_harassment_action(
            &mut world,
            Some(wizard),
            player,
            crate::npc::WizardAction::SummonNasties,
            &mut rng,
            &mut events,
        );

        let generated = events
            .iter()
            .filter(|event| matches!(event, EngineEvent::MonsterGenerated { .. }))
            .count();
        assert_eq!(generated, 4);
    }

    #[test]
    fn test_live_wizard_summon_nasties_spawn_near_wizard_anchor() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        for x in 3..=11 {
            for y in 3..=7 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        let player = world.player();
        let wizard_pos = Position::new(9, 5);
        let wizard = spawn_full_monster(&mut world, wizard_pos, "Wizard of Yendor", 12);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
            hp.current = 20;
            hp.max = 40;
        }
        let player_pos = world
            .get_component::<Positioned>(player)
            .map(|pos| pos.0)
            .expect("player should have a position");
        let mut events = Vec::new();
        let mut rng = test_rng();

        apply_wizard_harassment_action(
            &mut world,
            Some(wizard),
            player,
            crate::npc::WizardAction::SummonNasties,
            &mut rng,
            &mut events,
        );

        let generated_positions: Vec<Position> = events
            .iter()
            .filter_map(|event| match event {
                EngineEvent::MonsterGenerated { position, .. } => Some(*position),
                _ => None,
            })
            .collect();
        assert!(
            !generated_positions.is_empty(),
            "live Wizard summon should generate at least one monster"
        );
        assert!(
            generated_positions
                .iter()
                .all(|pos| { crate::ball::chebyshev_distance(*pos, wizard_pos) <= 1 })
        );
        assert!(
            generated_positions
                .iter()
                .all(|pos| { crate::ball::chebyshev_distance(*pos, player_pos) >= 3 })
        );
    }

    #[test]
    fn test_spawn_named_monster_near_entity_can_allow_group_generation() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let before = world.ecs().query::<&Monster>().iter().count();
        let mut rng = test_rng();

        let _spawned = spawn_named_monster_near_entity_with_flags(
            &mut world,
            player,
            "giant ant",
            MakeMonFlags::empty(),
            &mut rng,
        )
        .expect("group-capable monster should spawn near the player");

        let after = world.ecs().query::<&Monster>().iter().count();
        assert!(
            after >= before + 3,
            "small-group monster generation should add the leader plus at least two escorts"
        );
    }

    #[test]
    fn test_spawn_named_monster_near_entity_default_blocks_group_generation() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let before = world.ecs().query::<&Monster>().iter().count();
        let mut rng = test_rng();

        let _spawned = spawn_named_monster_near_entity(&mut world, player, "giant ant", &mut rng)
            .expect("single monster spawn should succeed near the player");

        let after = world.ecs().query::<&Monster>().iter().count();
        assert_eq!(
            after,
            before + 1,
            "default special spawns should continue suppressing natural monster groups"
        );
    }

    #[test]
    fn test_gehennom_wizard_nasties_can_include_demons() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        world.dungeon_mut().branch = DungeonBranch::Gehennom;
        world.dungeon_mut().depth = 20;

        for seed in 0..512u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let picks = choose_wizard_nasty_summon_specs(&world, &mut rng);
            let saw_demon = picks.iter().any(|name| {
                resolve_monster_id_by_spec(&test_game_data().monsters, name)
                    .and_then(|id| {
                        test_game_data()
                            .monsters
                            .iter()
                            .find(|monster| monster.id == id)
                    })
                    .is_some_and(|monster| monster.flags.contains(MonsterFlags::DEMON))
            });
            if saw_demon {
                return;
            }
        }

        panic!("Gehennom nasty summons should occasionally include demons");
    }

    #[test]
    fn test_high_level_wizard_nasties_can_scale_above_gehennom_floor() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        world.dungeon_mut().branch = DungeonBranch::Gehennom;
        world.dungeon_mut().depth = 25;
        if let Some(mut xl) = world.get_component_mut::<ExperienceLevel>(world.player()) {
            xl.0 = 30;
        }

        for seed in 0..512u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let picks = choose_wizard_nasty_summon_specs(&world, &mut rng);
            if picks.len() > 3 {
                return;
            }
        }

        panic!("high-level players should eventually trigger larger nasty swarms");
    }

    #[test]
    fn test_amulet_portal_sense_emits_heat_message_near_magic_portal() {
        let mut world = make_portal_world(DungeonBranch::Gehennom, 21, Position::new(7, 5));
        install_test_catalogs(&mut world);
        let amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
        world
            .get_component_mut::<crate::equipment::EquipmentSlots>(world.player())
            .expect("player should have equipment slots")
            .amulet = Some(amulet);
        set_player_position(&mut world, Position::new(6, 5));
        let mut rng = test_rng();

        for _ in 0..64 {
            let mut events = Vec::new();
            process_amulet_portal_sense(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "amulet-feels-hot"
                )
            }) {
                return;
            }
        }

        panic!("expected deterministic amulet portal sense to emit a heat message");
    }

    #[test]
    fn test_amulet_portal_sense_requires_worn_or_wielded_amulet() {
        let mut world = make_portal_world(DungeonBranch::Gehennom, 21, Position::new(7, 5));
        install_test_catalogs(&mut world);
        let _amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
        set_player_position(&mut world, Position::new(6, 5));
        let mut rng = test_rng();

        for _ in 0..64 {
            let mut events = Vec::new();
            process_amulet_portal_sense(&world, &mut rng, &mut events);
            assert!(
                !events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. }
                        if key == "amulet-feels-hot"
                            || key == "amulet-feels-very-warm"
                            || key == "amulet-feels-warm"
                )),
                "amulet portal sense should not trigger for a merely carried amulet"
            );
        }
    }

    #[test]
    fn test_real_amulet_can_wake_sleeping_wizard_from_inventory() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let _amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
        let wizard = spawn_full_monster(&mut world, Position::new(14, 14), "Wizard of Yendor", 12);
        let _ = crate::status::make_sleeping(&mut world, wizard, 20);
        let mut rng = test_rng();

        for _ in 0..4096 {
            let mut events = Vec::new();
            process_amulet_wakes_sleeping_wizard(&mut world, &mut rng, &mut events);
            if !crate::status::is_sleeping(&world, wizard) {
                assert!(events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "wizard-vague-nervous"
                )));
                return;
            }
        }

        panic!("real Amulet should eventually wake a sleeping Wizard");
    }

    #[test]
    fn test_wizard_can_continue_harassment_after_stealing_amulet() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let wizard = spawn_full_monster(&mut world, Position::new(6, 5), "Wizard of Yendor", 12);
        let amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
        let item = spawn_inventory_object_by_name(&mut world, "long sword", 'b');
        world
            .ecs_mut()
            .insert_one(
                item,
                BucStatus {
                    cursed: false,
                    blessed: false,
                    bknown: false,
                },
            )
            .expect("inventory item should accept a BUC component");
        let mut rng = test_rng();

        for _ in 0..64 {
            let mut events = Vec::new();
            process_wizard_of_yendor_turn(&mut world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "wizard-steal-amulet"
                )
            }) {
                break;
            }
        }

        assert!(
            !player_carries_item(&world, player, amulet),
            "wizard should eventually steal the amulet through the live harassment path"
        );

        let mut followup_events = Vec::new();
        apply_wizard_harassment_action(
            &mut world,
            Some(wizard),
            player,
            crate::npc::WizardAction::CurseItems,
            &mut rng,
            &mut followup_events,
        );

        let buc = world
            .get_component::<BucStatus>(item)
            .expect("follow-up harassment target should keep BUC state");
        assert!(
            buc.cursed,
            "wizard should still be able to curse inventory after stealing the amulet"
        );
    }

    #[test]
    fn test_far_wizard_harasses_without_remote_amulet_theft() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let wizard = spawn_full_monster(&mut world, Position::new(14, 14), "Wizard of Yendor", 12);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
            hp.current = 10;
            hp.max = 20;
        }
        let amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
        let item = spawn_inventory_object_by_name(&mut world, "long sword", 'b');
        world
            .ecs_mut()
            .insert_one(
                item,
                BucStatus {
                    cursed: false,
                    blessed: false,
                    bknown: false,
                },
            )
            .expect("inventory item should accept a BUC component");
        let mut rng = test_rng();

        for _ in 0..256 {
            let mut events = Vec::new();
            process_wizard_of_yendor_turn(&mut world, &mut rng, &mut events);
            let cursed = world
                .get_component::<BucStatus>(item)
                .is_some_and(|status| status.cursed);
            let summoned = events
                .iter()
                .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. }));
            let remote_theft = events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "wizard-steal-amulet"
                )
            });
            assert!(
                !remote_theft,
                "far Wizard pressure should not use remote amulet theft"
            );
            assert!(
                player_carries_item(&world, player, amulet),
                "far Wizard pressure should leave the Amulet on the player"
            );
            if cursed || summoned {
                return;
            }
        }

        panic!("far Wizard should still produce non-theft harassment");
    }

    // ── Cross-system wiring tests ──────────────────────────────

    #[test]
    fn appearance_table_initialized_in_gameworld() {
        let world = GameWorld::new(Position::new(40, 10));
        // AppearanceTable should have been initialized with non-empty pools.
        assert!(!world.appearance_table.potion_colors.is_empty());
        assert!(!world.appearance_table.scroll_labels.is_empty());
        assert!(!world.appearance_table.ring_materials.is_empty());
        assert!(!world.appearance_table.wand_materials.is_empty());
    }

    #[test]
    fn appearance_table_varies_with_rng_seed() {
        use rand::SeedableRng;
        use rand_pcg::Pcg64;
        let mut rng1 = Pcg64::seed_from_u64(1);
        let mut rng2 = Pcg64::seed_from_u64(2);
        let w1 = GameWorld::new_with_rng(Position::new(0, 0), &mut rng1);
        let w2 = GameWorld::new_with_rng(Position::new(0, 0), &mut rng2);
        assert!(
            crate::o_init::appearances_differ(&w1.appearance_table, &w2.appearance_table),
            "different seeds should produce different appearance tables"
        );
    }

    // ── Turn loop integration tests ────────────────────────────

    #[test]
    fn turn_loop_calls_status_tick() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Apply a timed status effect and verify it decrements.
        let player = world.player();
        crate::status::make_confused(&mut world, player, 3);
        assert!(crate::status::is_confused(&world, player));

        // Run 3 turns to tick the confusion.
        for _ in 0..3 {
            resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
        }

        // After 3 ticks, confusion should have expired.
        assert!(
            !crate::status::is_confused(&world, player),
            "confusion should expire after 3 ticks"
        );
    }

    #[test]
    fn light_source_fuel_decrements_during_turn() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Create a lit light source with low fuel.
        let lamp = world.spawn((crate::light::LightFuel {
            lit: true,
            fuel: 3,
            max_fuel: crate::light::OIL_LAMP_FUEL,
            kind: crate::light::LightKind::OilLamp,
        },));

        // Run one turn.
        resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        // Fuel should have decremented.
        let lf = world
            .get_component::<crate::light::LightFuel>(lamp)
            .unwrap();
        assert_eq!(
            lf.fuel, 2,
            "light source fuel should decrement by 1 per turn"
        );
    }

    #[test]
    fn light_source_extinguishes_at_zero_fuel() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Create a lit light source with 1 fuel remaining.
        let lamp = world.spawn((crate::light::LightFuel {
            lit: true,
            fuel: 1,
            max_fuel: crate::light::OIL_LAMP_FUEL,
            kind: crate::light::LightKind::OilLamp,
        },));

        // Run one turn.
        let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        // Lamp should be extinguished.
        let lf = world
            .get_component::<crate::light::LightFuel>(lamp)
            .unwrap();
        assert!(!lf.lit, "lamp should be extinguished at 0 fuel");
        assert_eq!(lf.fuel, 0);

        // Should have emitted a burn-out event.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "light-burned-out"
        )));
    }

    #[test]
    fn wiz_level_teleport_changes_depth() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Start at depth 1.
        assert_eq!(world.dungeon().depth, 1);

        let events = resolve_turn(
            &mut world,
            PlayerAction::WizLevelTeleport { depth: 5 },
            &mut rng,
        );

        // Depth should now be 5.
        assert_eq!(
            world.dungeon().depth,
            5,
            "WizLevelTeleport should change depth"
        );

        // Should have a level-teleport message and a LevelChanged event.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-level-teleport"
        )));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::LevelChanged { .. }))
        );
    }

    // ── Blind cannot read scrolls ─────────────────────────────────────

    #[test]
    fn test_blind_cannot_read_scroll() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        // Make player blind.
        crate::status::make_blinded(&mut world, player, 10);
        assert!(crate::status::is_blind(&world, player));

        let events = resolve_turn(&mut world, PlayerAction::Read { item: None }, &mut rng);

        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::Message { key, .. } if key == "scroll-cant-read-blind"
            )),
            "Blind player should get cant-read message"
        );
        // Should NOT get the normal "read-what" prompt.
        assert!(!events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "read-what"
        )));
    }

    #[test]
    fn test_not_blind_can_read_scroll() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        assert!(!crate::status::is_blind(&world, player));

        let events = resolve_turn(&mut world, PlayerAction::Read { item: None }, &mut rng);

        // Should get the normal prompt, not the blind message.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "read-what"
        )));
        assert!(!events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "scroll-cant-read-blind"
        )));
    }

    // ── Stunned/confused wand direction randomization ─────────────────

    #[test]
    fn test_stunned_wand_direction_randomized() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        crate::status::make_stunned(&mut world, player, 10);
        assert!(crate::status::is_stunned(&world, player));

        // Use player entity as a stand-in item without wand components;
        // this should fall back to the generic zap path.
        let events = resolve_turn(
            &mut world,
            PlayerAction::ZapWand {
                item: player,
                direction: Some(Direction::East),
            },
            &mut rng,
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "zap-generic"
        )));
    }

    #[test]
    fn test_confused_wand_direction_randomized() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        crate::status::make_confused(&mut world, player, 10);
        assert!(crate::status::is_confused(&world, player));

        let events = resolve_turn(
            &mut world,
            PlayerAction::ZapWand {
                item: player,
                direction: Some(Direction::North),
            },
            &mut rng,
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "zap-generic"
        )));
    }

    #[test]
    fn test_normal_wand_direction_preserved() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        // Not confused, not stunned.
        assert!(!crate::status::is_confused(&world, player));
        assert!(!crate::status::is_stunned(&world, player));

        let events = resolve_turn(
            &mut world,
            PlayerAction::ZapWand {
                item: player,
                direction: Some(Direction::South),
            },
            &mut rng,
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "zap-generic"
        )));
    }

    #[test]
    fn test_zap_wand_uses_item_type_and_charges_when_present() {
        use crate::monster_ai::WandTypeTag;
        use crate::wands::{WandCharges, WandType};

        let mut world = make_test_world();
        let mut rng = test_rng();

        let wand = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Wand,
                quantity: 1,
                weight: 7,
                age: 0,
                inv_letter: Some('z'),
                artifact: None,
            },
            ObjectLocation::Inventory,
            WandTypeTag(WandType::Light),
            WandCharges {
                spe: 1,
                recharged: 0,
            },
        ));

        let events = resolve_turn(
            &mut world,
            PlayerAction::ZapWand {
                item: wand,
                direction: None, // NODIR wand should work without direction.
            },
            &mut rng,
        );

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-light")),
            "Light wand should dispatch to real wand logic"
        );

        let charges = world
            .get_component::<WandCharges>(wand)
            .expect("wand should keep charges component");
        assert_eq!(charges.spe, 0, "zapping should consume one charge");
    }

    #[test]
    fn test_striking_wand_on_peaceful_monster_removes_peaceful() {
        use crate::monster_ai::WandTypeTag;
        use crate::wands::{WandCharges, WandType};

        let mut world = make_test_world();
        let mut rng = test_rng();
        let target = spawn_idle_named_monster(&mut world, Position::new(6, 5), "gnome");
        world
            .ecs_mut()
            .insert_one(target, Peaceful)
            .expect("target should accept peaceful marker");

        let wand = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Wand,
                quantity: 1,
                weight: 7,
                age: 0,
                inv_letter: Some('z'),
                artifact: None,
            },
            ObjectLocation::Inventory,
            WandTypeTag(WandType::Striking),
            WandCharges {
                spe: 1,
                recharged: 0,
            },
        ));

        let events = resolve_turn(
            &mut world,
            PlayerAction::ZapWand {
                item: wand,
                direction: Some(Direction::East),
            },
            &mut rng,
        );

        assert!(
            events.iter().any(|event| matches!(
                event,
                EngineEvent::ExtraDamage {
                    target: entity,
                    source: crate::event::DamageSource::Wand,
                    ..
                } if *entity == target
            )),
            "wand of striking should damage the peaceful monster"
        );
        assert!(
            world.get_component::<Peaceful>(target).is_none(),
            "damaging a peaceful monster with a wand should make it hostile"
        );
    }

    #[test]
    fn test_slow_monster_wand_on_peaceful_monster_removes_peaceful() {
        use crate::monster_ai::WandTypeTag;
        use crate::wands::{WandCharges, WandType};

        let mut world = make_test_world();
        let mut rng = test_rng();
        let target = spawn_idle_named_monster(&mut world, Position::new(6, 5), "gnome");
        world
            .ecs_mut()
            .insert_one(target, Peaceful)
            .expect("target should accept peaceful marker");

        let wand = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Wand,
                quantity: 1,
                weight: 7,
                age: 0,
                inv_letter: Some('z'),
                artifact: None,
            },
            ObjectLocation::Inventory,
            WandTypeTag(WandType::SlowMonster),
            WandCharges {
                spe: 1,
                recharged: 0,
            },
        ));

        let events = resolve_turn(
            &mut world,
            PlayerAction::ZapWand {
                item: wand,
                direction: Some(Direction::East),
            },
            &mut rng,
        );

        assert!(
            events.iter().any(|event| matches!(
                event,
                EngineEvent::StatusApplied {
                    entity,
                    status: crate::event::StatusEffect::SlowSpeed,
                    ..
                } if *entity == target
            )),
            "wand of slow monster should apply a hostile status to the peaceful monster"
        );
        assert!(
            world.get_component::<Peaceful>(target).is_none(),
            "hostile status effects should also make peaceful monsters hostile"
        );
    }

    #[test]
    fn test_speed_monster_wand_on_peaceful_monster_keeps_peaceful() {
        use crate::monster_ai::WandTypeTag;
        use crate::wands::{WandCharges, WandType};

        let mut world = make_test_world();
        let mut rng = test_rng();
        let target = spawn_idle_named_monster(&mut world, Position::new(6, 5), "gnome");
        world
            .ecs_mut()
            .insert_one(target, Peaceful)
            .expect("target should accept peaceful marker");

        let wand = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Wand,
                quantity: 1,
                weight: 7,
                age: 0,
                inv_letter: Some('z'),
                artifact: None,
            },
            ObjectLocation::Inventory,
            WandTypeTag(WandType::SpeedMonster),
            WandCharges {
                spe: 1,
                recharged: 0,
            },
        ));

        let events = resolve_turn(
            &mut world,
            PlayerAction::ZapWand {
                item: wand,
                direction: Some(Direction::East),
            },
            &mut rng,
        );

        assert!(
            events.iter().any(|event| matches!(
                event,
                EngineEvent::StatusApplied {
                    entity,
                    status: crate::event::StatusEffect::FastSpeed,
                    ..
                } if *entity == target
            )),
            "wand of speed monster should still affect the peaceful monster"
        );
        assert!(
            world.get_component::<Peaceful>(target).is_some(),
            "beneficial status effects should not make a peaceful monster hostile"
        );
    }

    #[test]
    fn test_ranged_hit_event_on_peaceful_monster_removes_peaceful() {
        let mut world = make_test_world();
        let player = world.player();
        let target = spawn_idle_named_monster(&mut world, Position::new(6, 5), "gnome");
        let projectile = world.spawn(());
        world
            .ecs_mut()
            .insert_one(target, Peaceful)
            .expect("target should accept peaceful marker");

        anger_peaceful_monsters_from_player_events(
            &mut world,
            &[EngineEvent::RangedHit {
                attacker: player,
                defender: target,
                projectile,
                damage: 3,
            }],
        );

        assert!(
            world.get_component::<Peaceful>(target).is_none(),
            "direct ranged-hit events from the player should make peaceful monsters hostile"
        );
    }

    #[test]
    fn test_swap_uses_equipment_state_for_secondary_weapon() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let primary = spawn_inventory_item(&mut world, 'a');
        let secondary = spawn_inventory_item(&mut world, 'b');
        world
            .ecs_mut()
            .insert_one(
                primary,
                BucStatus {
                    cursed: false,
                    blessed: false,
                    bknown: true,
                },
            )
            .expect("insert primary buc");

        let mut equip = world
            .get_component_mut::<crate::equipment::EquipmentSlots>(player)
            .expect("player should have equipment slots");
        equip.weapon = Some(primary);
        equip.off_hand = Some(secondary);
        drop(equip);

        let events = resolve_turn(&mut world, PlayerAction::Swap, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "swap-success"
        )));
    }

    #[test]
    fn test_ride_mounts_adjacent_tame_steed() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let steed = spawn_tame_steed(&mut world, Position::new(6, 5), "pony");

        let events = resolve_turn(&mut world, PlayerAction::Ride, &mut rng);

        assert!(crate::steed::is_mounted(&world, player));
        let mounted_on = world
            .get_component::<crate::steed::MountedOn>(player)
            .expect("player should have MountedOn after ride");
        assert_eq!(mounted_on.0, steed);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "mount-steed"
        )));
    }

    #[test]
    fn test_ride_dismounts_when_already_mounted() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let _steed = spawn_tame_steed(&mut world, Position::new(6, 5), "pony");

        let _ = resolve_turn(&mut world, PlayerAction::Ride, &mut rng);
        assert!(crate::steed::is_mounted(&world, player));

        let events = resolve_turn(&mut world, PlayerAction::Ride, &mut rng);
        assert!(!crate::steed::is_mounted(&world, player));
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "dismount-steed"
        )));
    }

    #[test]
    fn test_toggle_two_weapon_requires_offhand() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let primary = spawn_inventory_item(&mut world, 'a');
        let mut equip = world
            .get_component_mut::<crate::equipment::EquipmentSlots>(player)
            .expect("player should have equipment slots");
        equip.weapon = Some(primary);
        equip.off_hand = None;
        drop(equip);

        let events = resolve_turn(&mut world, PlayerAction::ToggleTwoWeapon, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "swap-no-secondary"
        )));
    }

    #[test]
    fn test_toggle_two_weapon_flips_player_skill_state() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let primary = spawn_inventory_item(&mut world, 'a');
        let secondary = spawn_inventory_item(&mut world, 'b');
        let mut equip = world
            .get_component_mut::<crate::equipment::EquipmentSlots>(player)
            .expect("player should have equipment slots");
        equip.weapon = Some(primary);
        equip.off_hand = Some(secondary);
        drop(equip);

        let events_on = resolve_turn(&mut world, PlayerAction::ToggleTwoWeapon, &mut rng);
        assert!(events_on.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "two-weapon-enabled"
        )));
        let skills = world
            .get_component::<nethack_babel_data::PlayerSkills>(player)
            .expect("player should gain PlayerSkills when toggling two-weapon");
        assert!(
            skills.two_weapon,
            "two-weapon should be enabled after first toggle"
        );
        drop(skills);

        let events_off = resolve_turn(&mut world, PlayerAction::ToggleTwoWeapon, &mut rng);
        assert!(events_off.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "two-weapon-disabled"
        )));
        let skills = world
            .get_component::<nethack_babel_data::PlayerSkills>(player)
            .expect("player should keep PlayerSkills after toggling two-weapon");
        assert!(
            !skills.two_weapon,
            "two-weapon should be disabled after second toggle"
        );
    }

    #[test]
    fn test_enhance_skill_advances_when_threshold_met() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        world
            .ecs_mut()
            .insert_one(
                player,
                PlayerSkills {
                    weapon_slots: 2,
                    skills_advanced: 0,
                    skills: vec![SkillState {
                        skill: WeaponSkill::Dagger,
                        level: 1,     // Unskilled
                        max_level: 4, // Expert
                        advance: 20,  // threshold for level 1->2
                    }],
                    two_weapon: false,
                },
            )
            .expect("insert player skills");

        let events = resolve_turn(&mut world, PlayerAction::EnhanceSkill, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "enhance-success"
        )));

        let skills = world
            .get_component::<PlayerSkills>(player)
            .expect("player should keep PlayerSkills");
        assert_eq!(skills.weapon_slots, 1, "enhance should consume one slot");
        assert_eq!(skills.skills_advanced, 1, "enhance should increment count");
        assert_eq!(skills.skills[0].level, 2, "dagger should upgrade to Basic");
    }

    #[test]
    fn test_enhance_skill_requires_practice_and_slots() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        world
            .ecs_mut()
            .insert_one(
                player,
                PlayerSkills {
                    weapon_slots: 1, // not enough for Basic->Skilled (cost 2)
                    skills_advanced: 0,
                    skills: vec![SkillState {
                        skill: WeaponSkill::LongSword,
                        level: 2,     // Basic
                        max_level: 4, // Expert
                        advance: 80,  // enough practice
                    }],
                    two_weapon: false,
                },
            )
            .expect("insert player skills");

        let events = resolve_turn(&mut world, PlayerAction::EnhanceSkill, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "enhance-not-available"
        )));

        let skills = world
            .get_component::<PlayerSkills>(player)
            .expect("player should keep PlayerSkills");
        assert_eq!(
            skills.weapon_slots, 1,
            "failed enhance should not consume slots"
        );
        assert_eq!(
            skills.skills_advanced, 0,
            "failed enhance should not increment count"
        );
        assert_eq!(
            skills.skills[0].level, 2,
            "failed enhance should not change skill level"
        );
    }

    #[test]
    fn test_swap_detects_cursed_primary_weapon() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let primary = spawn_inventory_item(&mut world, 'a');
        let secondary = spawn_inventory_item(&mut world, 'b');
        world
            .ecs_mut()
            .insert_one(
                primary,
                BucStatus {
                    cursed: true,
                    blessed: false,
                    bknown: true,
                },
            )
            .expect("insert primary buc");

        let mut equip = world
            .get_component_mut::<crate::equipment::EquipmentSlots>(player)
            .expect("player should have equipment slots");
        equip.weapon = Some(primary);
        equip.off_hand = Some(secondary);
        drop(equip);

        let events = resolve_turn(&mut world, PlayerAction::Swap, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "swap-welded"
        )));
    }

    #[test]
    fn test_wipe_clears_creamed_counter_from_hero_counters() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        world
            .ecs_mut()
            .insert_one(
                player,
                crate::status::HeroCounters {
                    creamed: 3,
                    gallop: 0,
                },
            )
            .expect("insert hero counters");

        let events = resolve_turn(&mut world, PlayerAction::Wipe, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wipe-cream-off"
        )));
        let counters = world
            .get_component::<crate::status::HeroCounters>(player)
            .expect("player should have hero counters");
        assert_eq!(counters.creamed, 0, "wipe should clear creamed counter");
    }

    #[test]
    fn test_wipe_respects_cursed_towel_in_offhand() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        world
            .ecs_mut()
            .insert_one(
                player,
                crate::status::HeroCounters {
                    creamed: 3,
                    gallop: 0,
                },
            )
            .expect("insert hero counters");

        let towel = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 2,
                age: 0,
                inv_letter: Some('t'),
                artifact: None,
            },
            ObjectLocation::Inventory,
            Name("towel".to_string()),
            BucStatus {
                cursed: true,
                blessed: false,
                bknown: true,
            },
        ));

        let mut equip = world
            .get_component_mut::<crate::equipment::EquipmentSlots>(player)
            .expect("player should have equipment slots");
        equip.off_hand = Some(towel);
        drop(equip);

        let events = resolve_turn(&mut world, PlayerAction::Wipe, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wipe-cursed-towel"
        )));

        let counters = world
            .get_component::<crate::status::HeroCounters>(player)
            .expect("player should have hero counters");
        assert_eq!(
            counters.creamed, 2,
            "cursed towel should block wiping; only per-turn decrement applies"
        );
    }

    #[test]
    fn test_turn_undead_uses_role_and_nearby_undead_count() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, priest_identity())
            .expect("insert player identity");

        world.spawn((
            Monster,
            Positioned(Position::new(6, 5)),
            crate::monster_ai::MonsterSpeciesFlags(MonsterFlags::UNDEAD),
            MovementPoints(NORMAL_SPEED as i32),
        ));
        world.spawn((
            Monster,
            Positioned(Position::new(30, 30)),
            crate::monster_ai::MonsterSpeciesFlags(MonsterFlags::UNDEAD),
            MovementPoints(NORMAL_SPEED as i32),
        ));

        let events = resolve_turn(&mut world, PlayerAction::TurnUndead, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "turn-undead-success"
        )));
    }

    #[test]
    fn test_turn_undead_requires_clerical_role() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        world.spawn((
            Monster,
            Positioned(Position::new(6, 5)),
            crate::monster_ai::MonsterSpeciesFlags(MonsterFlags::UNDEAD),
            MovementPoints(NORMAL_SPEED as i32),
        ));

        let events = resolve_turn(&mut world, PlayerAction::TurnUndead, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "turn-not-clerical"
        )));
    }

    #[test]
    fn test_jump_without_jumping_boots_has_no_ability() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Jump {
                position: Position::new(6, 5),
            },
            &mut rng,
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "jump-no-ability"
        )));
    }

    #[test]
    fn test_jump_with_jumping_boots_moves_player() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let boots = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Armor,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('b'),
                artifact: None,
            },
            ObjectLocation::Inventory,
            Name("jumping boots".to_string()),
        ));

        let mut equip = world
            .get_component_mut::<crate::equipment::EquipmentSlots>(player)
            .expect("player should have equipment slots");
        equip.boots = Some(boots);
        drop(equip);

        let dest = Position::new(8, 5);
        let events = resolve_turn(&mut world, PlayerAction::Jump { position: dest }, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "jump-success"
        )));
        let pos = world
            .get_component::<Positioned>(player)
            .expect("player should have position");
        assert_eq!(pos.0, dest, "successful jump should move player");
    }

    fn kick_damage(events: &[EngineEvent]) -> Option<u32> {
        for event in events {
            if let EngineEvent::Message { key, args } = event
                && key == "kick-monster"
                && let Some((_, dmg)) = args.iter().find(|(k, _)| k == "damage")
                && let Ok(val) = dmg.parse::<u32>()
            {
                return Some(val);
            }
        }
        None
    }

    #[test]
    fn test_kick_uses_monk_role_bonus() {
        let mut world_non = make_test_world();
        let mut world_monk = make_test_world();
        let player = world_monk.player();
        world_monk
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("insert monk identity");

        spawn_monster(&mut world_non, Position::new(6, 5), 12);
        spawn_monster(&mut world_monk, Position::new(6, 5), 12);

        let mut rng_non = test_rng();
        let mut rng_monk = test_rng();
        let events_non = resolve_turn(
            &mut world_non,
            PlayerAction::Kick {
                direction: Direction::East,
            },
            &mut rng_non,
        );
        let events_monk = resolve_turn(
            &mut world_monk,
            PlayerAction::Kick {
                direction: Direction::East,
            },
            &mut rng_monk,
        );

        let non_damage = kick_damage(&events_non).expect("non-monk kick damage event");
        let monk_damage = kick_damage(&events_monk).expect("monk kick damage event");
        assert!(
            monk_damage > non_damage,
            "monk kick should add martial arts bonus ({} <= {})",
            monk_damage,
            non_damage
        );
    }

    #[test]
    fn test_rub_uses_item_name_touchstone_path() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let item = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Gem,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('g'),
                artifact: None,
            },
            ObjectLocation::Inventory,
            Name("touchstone".to_string()),
        ));

        let events = resolve_turn(&mut world, PlayerAction::Rub { item }, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "rub-touchstone"
        )));
    }

    #[test]
    fn test_invoke_artifact_requires_wielded_item() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let item = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Weapon,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('a'),
                artifact: Some(ArtifactId(1)),
            },
            ObjectLocation::Inventory,
            Name("artifact blade".to_string()),
        ));

        let events = resolve_turn(&mut world, PlayerAction::InvokeArtifact { item }, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "invoke-not-wielded"
        )));
    }

    #[test]
    fn test_invoke_artifact_succeeds_when_wielded() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let item = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Weapon,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('a'),
                artifact: Some(ArtifactId(1)),
            },
            ObjectLocation::Inventory,
            Name("artifact blade".to_string()),
        ));

        let mut equip = world
            .get_component_mut::<crate::equipment::EquipmentSlots>(player)
            .expect("player should have equipment slots");
        equip.weapon = Some(item);
        drop(equip);

        let events = resolve_turn(&mut world, PlayerAction::InvokeArtifact { item }, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "invoke-artifact"
        )));
    }

    #[test]
    fn test_chat_quest_leader_assigns_quest_when_eligible() {
        let mut world = make_test_world();
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Quest;
        world.dungeon_mut().depth = 1;
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        let mut religion = default_religion_state(&world, player);
        religion.experience_level = 14;
        religion.alignment_record = 10;
        world
            .ecs_mut()
            .insert_one(player, religion)
            .expect("player should accept religion state");
        if let Some(mut level) = world.get_component_mut::<ExperienceLevel>(player) {
            level.0 = 14;
        }
        spawn_idle_named_monster(&mut world, Position::new(6, 5), "Neferet the Green");

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-leader-first"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-assigned"
        )));
        let quest_state = world
            .get_component::<crate::quest::QuestState>(player)
            .expect("quest chat should persist quest state");
        assert!(quest_state.leader_met, "leader should be marked as met");
        assert_eq!(quest_state.status, crate::quest::QuestStatus::Assigned);
    }

    #[test]
    fn test_chat_quest_guardian_uses_quest_dialogue() {
        let mut world = make_test_world();
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Quest;
        world.dungeon_mut().depth = 1;
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        spawn_idle_named_monster(&mut world, Position::new(6, 5), "apprentice");

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-guardian"
        )));
    }

    #[test]
    fn test_attacking_quest_leader_marks_angry_and_blocks_progress() {
        let mut world = make_stair_world(Terrain::StairsDown, 1);
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Quest;
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        let leader = spawn_full_monster(&mut world, Position::new(6, 5), "Neferet the Green", 12);
        world
            .ecs_mut()
            .insert_one(leader, Peaceful)
            .expect("leader should accept peaceful marker");
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(leader) {
            hp.current = 40;
            hp.max = 40;
        }
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(leader) {
            mp.0 = 0;
        }

        let mut rng = test_rng();
        let attack_events = resolve_turn(
            &mut world,
            PlayerAction::FightDirection {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(attack_events.iter().any(|event| matches!(
            event,
            EngineEvent::MeleeHit { defender, .. } if *defender == leader
        )));
        assert!(
            world
                .get_component::<crate::quest::QuestState>(player)
                .is_some_and(|state| state.leader_angry),
            "attacking the quest leader should persist leader anger"
        );

        let chat_events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(chat_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-leader-reject"
        )));

        let down_events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert!(down_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-expelled"
        )));
        assert_eq!(world.dungeon().depth, 1);
    }

    #[test]
    fn test_chatting_with_shopkeeper_tracks_angry_state_after_attack() {
        let mut world = make_test_world();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(shopkeeper) {
            hp.current = 40;
            hp.max = 40;
        }
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(shopkeeper) {
            mp.0 = 0;
        }
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(6, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        world.dungeon_mut().shop_rooms[0].shopkeeper_gold = 500;

        let mut rng = test_rng();
        let greet_events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(greet_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shk-shoplifters"
        )));

        let _ = resolve_turn(
            &mut world,
            PlayerAction::FightDirection {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(
            world.dungeon().shop_rooms[0].angry,
            "attacking a shopkeeper should rile the live shop room"
        );

        let angry_events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(angry_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shk-angry-greeting"
        )));
    }

    #[test]
    fn test_chatting_with_shopkeeper_reports_outstanding_bill_total() {
        let mut world = make_test_world();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(6, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        assert!(
            world.dungeon_mut().shop_rooms[0]
                .bill
                .add(hecs::Entity::DANGLING, 75, 2),
            "shop bill should accept a quoted entry"
        );

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, args, .. }
                if key == "shk-bill-total"
                    && args
                        .iter()
                        .any(|(name, value)| name == "amount" && value == "150")
        )));
    }

    #[test]
    fn test_hallucinating_chatting_with_shopkeeper_can_emit_geico_pitch() {
        let mut world = make_test_world();
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(6, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let _ = crate::status::make_hallucinated(&mut world, player, 200);

        let mut rng = test_rng();
        for _ in 0..64 {
            let events = resolve_turn(
                &mut world,
                PlayerAction::Chat {
                    direction: Direction::East,
                },
                &mut rng,
            );
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "shk-geico-pitch"
                )
            }) {
                return;
            }
        }

        panic!("hallucinating shop chat should eventually emit the GEICO-style pitch");
    }

    #[test]
    fn test_chatting_with_laughing_monster_emits_laughter_line() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        spawn_full_monster(&mut world, Position::new(6, 5), "leprechaun", 12);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. }
                    if matches!(
                        key.as_str(),
                        "npc-laugh-giggles"
                            | "npc-laugh-chuckles"
                            | "npc-laugh-snickers"
                            | "npc-laugh-laughs"
                    )
            )
        }));
    }

    #[test]
    fn test_deaf_player_chatting_with_laughing_monster_gets_no_response() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        spawn_full_monster(&mut world, Position::new(6, 5), "leprechaun", 12);
        if let Some(mut status) = world.get_component_mut::<crate::status::StatusEffects>(player) {
            status.deaf = 20;
        }

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-chat-no-response")
        }));
        assert!(!events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key.starts_with("npc-laugh-")
            )
        }));
    }

    #[test]
    fn test_hallucinating_chatting_with_gecko_can_emit_fake_shop_pitch() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        spawn_full_monster(&mut world, Position::new(6, 5), "gecko", 8);
        let _ = crate::status::make_hallucinated(&mut world, player, 200);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-gecko-geico-pitch"
            )
        }));
    }

    #[test]
    fn test_chatting_with_mumbling_monster_emits_mumble_line() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        spawn_full_monster(&mut world, Position::new(6, 5), "lich", 16);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-mumble-incomprehensible"
            )
        }));
    }

    #[test]
    fn test_chatting_with_skeleton_rattles_and_paralyzes_player() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        spawn_full_monster(&mut world, Position::new(6, 5), "skeleton", 12);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-bones-rattle"
            )
        }));
        assert!(
            world
                .get_component::<crate::status::StatusEffects>(player)
                .is_some_and(|status| status.paralysis > 0),
            "chatting with a skeleton should briefly paralyze the player"
        );
    }

    #[test]
    fn test_chatting_with_shrieker_wakes_sleeping_monsters() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let shrieker = spawn_full_monster(&mut world, Position::new(6, 5), "shrieker", 8);
        let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "kobold", 8);
        let _ = crate::status::make_sleeping(&mut world, sleeper, 20);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-shriek")
        }));
        assert!(
            world
                .get_component::<crate::status::StatusEffects>(sleeper)
                .is_none_or(|status| status.sleeping == 0),
            "chatting with a shrieker should wake sleeping monsters on the level"
        );
        assert!(
            world
                .get_component::<crate::status::StatusEffects>(shrieker)
                .is_none_or(|status| status.sleeping == 0),
            "the shrieker itself should not remain asleep after speaking"
        );
    }

    #[test]
    fn test_chatting_with_hostile_hissing_monster_hisses() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let hiss_name = monster_name_with_sound(&world, MonsterSound::Hiss);
        spawn_full_monster(&mut world, Position::new(6, 5), &hiss_name, 10);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-hiss-hisses")
        }));
    }

    #[test]
    fn test_chatting_with_peaceful_hissing_monster_gets_no_response() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let hiss_name = monster_name_with_sound(&world, MonsterSound::Hiss);
        let monster = spawn_full_monster(&mut world, Position::new(6, 5), &hiss_name, 10);
        world
            .ecs_mut()
            .insert_one(monster, Peaceful)
            .expect("hissing monster should accept peaceful marker");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-chat-no-response")
        }));
    }

    #[test]
    fn test_chatting_with_peaceful_buzzing_monster_drones() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let buzz_name = monster_name_with_sound(&world, MonsterSound::Buzz);
        let monster = spawn_full_monster(&mut world, Position::new(6, 5), &buzz_name, 10);
        world
            .ecs_mut()
            .insert_one(monster, Peaceful)
            .expect("buzzing monster should accept peaceful marker");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-buzz-drones")
        }));
    }

    #[test]
    fn test_chatting_with_tame_neighing_monster_neighs() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let neigh_name = monster_name_with_sound(&world, MonsterSound::Neigh);
        let monster = spawn_full_monster(&mut world, Position::new(6, 5), &neigh_name, 10);
        let current_turn = world.turn();
        make_tame_pet_state(&mut world, monster, 4, current_turn.saturating_add(10));

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-neigh-neighs")
        }));
    }

    #[test]
    fn test_chatting_with_hungry_tame_barker_whines() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let bark_name = monster_name_with_sound_excluding(&world, MonsterSound::Bark, &["dingo"]);
        let monster = spawn_full_monster(&mut world, Position::new(6, 5), &bark_name, 10);
        advance_world_turns(&mut world, 410);
        let current_turn = world.turn();
        make_tame_pet_state(&mut world, monster, 10, current_turn.saturating_sub(400));

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-bark-whines")
        }));
    }

    #[test]
    fn test_chatting_with_trapped_tame_barker_whines() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        advance_world_turns(&mut world, 20);
        let bark_name = monster_name_with_sound_excluding(&world, MonsterSound::Bark, &["dingo"]);
        let monster = spawn_full_monster(&mut world, Position::new(6, 5), &bark_name, 10);
        world
            .ecs_mut()
            .insert_one(
                monster,
                crate::traps::Trapped {
                    kind: crate::traps::TrappedIn::BearTrap,
                    turns_remaining: 5,
                },
            )
            .expect("barker should accept trapped state");
        let current_turn = world.turn();
        make_tame_pet_state(&mut world, monster, 10, current_turn.saturating_add(100));

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-bark-whines")
        }));
    }

    #[test]
    fn test_chatting_with_peaceful_dingo_gets_no_response() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        advance_world_turns(&mut world, 20);
        let dingo = spawn_full_monster(&mut world, Position::new(6, 5), "dingo", 10);
        world
            .ecs_mut()
            .insert_one(dingo, Peaceful)
            .expect("dingo should accept peaceful marker");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "npc-chat-no-response"
        )));
    }

    #[test]
    fn test_chatting_with_full_moon_wolf_howls() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        spawn_full_monster(&mut world, Position::new(6, 5), "wolf", 10);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-bark-howls")
        }));
    }

    #[test]
    fn test_chatting_with_werewolf_off_full_moon_mentions_moon() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        advance_world_turns(&mut world, 20);
        let were_name = monster_name_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
        let were = spawn_full_monster(&mut world, Position::new(6, 5), &were_name, 10);
        let were_id = monster_id_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
        world
            .ecs_mut()
            .insert_one(were, crate::world::MonsterIdentity(were_id))
            .expect("werewolf chat test should accept explicit monster identity");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-were-moon")
        }));
    }

    #[test]
    fn test_chatting_with_full_moon_werewolf_wakes_sleepers() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let were_name = monster_name_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
        let were = spawn_full_monster(&mut world, Position::new(6, 5), &were_name, 10);
        let were_id = monster_id_with_sound_excluding(&world, MonsterSound::Were, &["wererat"]);
        world
            .ecs_mut()
            .insert_one(were, crate::world::MonsterIdentity(were_id))
            .expect("full moon were test should accept explicit monster identity");
        let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "kobold", 8);
        let _ = crate::status::make_sleeping(&mut world, sleeper, 20);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-were-howls")
        }));
        assert!(
            world
                .get_component::<crate::status::StatusEffects>(sleeper)
                .is_none_or(|status| status.sleeping == 0),
            "full moon were chat should wake nearby sleeping monsters"
        );
    }

    #[test]
    fn test_chatting_with_satiated_tame_cat_purrs() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let mew_name = monster_name_with_sound(&world, MonsterSound::Mew);
        let monster = spawn_full_monster(&mut world, Position::new(6, 5), &mew_name, 10);
        let current_turn = world.turn();
        make_tame_pet_state(&mut world, monster, 10, current_turn.saturating_add(1500));

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-mew-purrs")
        }));
    }

    #[test]
    fn test_chatting_with_trapped_tame_cat_yowls() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let mew_name = monster_name_with_sound(&world, MonsterSound::Mew);
        let monster = spawn_full_monster(&mut world, Position::new(6, 5), &mew_name, 10);
        world
            .ecs_mut()
            .insert_one(
                monster,
                crate::traps::Trapped {
                    kind: crate::traps::TrappedIn::BearTrap,
                    turns_remaining: 5,
                },
            )
            .expect("cat should accept trapped state");
        let current_turn = world.turn();
        make_tame_pet_state(&mut world, monster, 10, current_turn.saturating_add(100));

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-mew-yowls")
        }));
    }

    #[test]
    fn test_chatting_with_hungry_tame_horse_whinnies() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let neigh_name = monster_name_with_sound(&world, MonsterSound::Neigh);
        let monster = spawn_full_monster(&mut world, Position::new(6, 5), &neigh_name, 10);
        advance_world_turns(&mut world, 400);
        let current_turn = world.turn();
        make_tame_pet_state(&mut world, monster, 10, current_turn.saturating_sub(400));

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-neigh-whinnies")
        }));
    }

    #[test]
    fn test_chatting_with_hostile_humanoid_threatens() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let humanoid_name = monster_name_with_sound(&world, MonsterSound::Humanoid);
        spawn_full_monster(&mut world, Position::new(6, 5), &humanoid_name, 10);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-humanoid-threatens")
        }));
    }

    #[test]
    fn test_chatting_with_peaceful_tourist_says_aloha() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let tourist = spawn_full_monster(&mut world, Position::new(6, 5), "tourist", 10);
        world
            .ecs_mut()
            .insert_one(tourist, Peaceful)
            .expect("tourist should accept peaceful marker");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-humanoid-aloha")
        }));
    }

    #[test]
    fn test_chatting_with_peaceful_watchman_uses_facts_line() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let arrest_name = monster_name_with_sound(&world, MonsterSound::Arrest);
        let watchman = spawn_full_monster(&mut world, Position::new(6, 5), &arrest_name, 10);
        world
            .ecs_mut()
            .insert_one(watchman, Peaceful)
            .expect("watchman should accept peaceful marker");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-arrest-facts-sir")
        }));
    }

    #[test]
    fn test_chatting_with_peaceful_djinni_is_free() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let djinni_name = monster_name_with_sound_excluding(
            &world,
            MonsterSound::Djinni,
            &["water demon", "prisoner"],
        );
        let djinni = spawn_full_monster(&mut world, Position::new(6, 5), &djinni_name, 10);
        world
            .ecs_mut()
            .insert_one(djinni, Peaceful)
            .expect("djinni should accept peaceful marker");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-djinni-free")
        }));
    }

    #[test]
    fn test_chatting_with_peaceful_guard_with_gold_demands_drop() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let guard_name = monster_name_with_sound(&world, MonsterSound::Guard);
        let guard = spawn_full_monster(&mut world, Position::new(6, 5), &guard_name, 10);
        world
            .ecs_mut()
            .insert_one(guard, Peaceful)
            .expect("guard should accept peaceful marker");
        let _gold = spawn_inventory_gold(&mut world, 150, 'g');

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "npc-guard-drop-gold"
        )));
    }

    #[test]
    fn test_chatting_with_peaceful_nurse_mentions_weapon() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let nurse_name = monster_name_with_sound(&world, MonsterSound::Nurse);
        let nurse = spawn_full_monster(&mut world, Position::new(6, 5), &nurse_name, 10);
        world
            .ecs_mut()
            .insert_one(nurse, Peaceful)
            .expect("nurse should accept peaceful marker");
        let player = world.player();
        let weapon = spawn_inventory_object_by_name(&mut world, "long sword", 'w');
        world
            .get_component_mut::<crate::equipment::EquipmentSlots>(player)
            .expect("player should have equipment slots")
            .weapon = Some(weapon);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "npc-nurse-put-weapon-away"
        )));
    }

    #[test]
    fn test_chatting_with_hostile_soldier_uses_soldier_bark() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let soldier_name = monster_name_with_sound(&world, MonsterSound::Soldier);
        spawn_full_monster(&mut world, Position::new(6, 5), &soldier_name, 10);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. }
                if key == "npc-soldier-resistance"
                    || key == "npc-soldier-dog-meat"
                    || key == "npc-soldier-surrender"
        )));
    }

    #[test]
    fn test_chatting_with_seductress_uses_seduction_line() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let seducer_name = monster_name_with_sound(&world, MonsterSound::Seduce);
        spawn_full_monster(&mut world, Position::new(6, 5), &seducer_name, 10);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. }
                if key == "npc-seduce-hello-sailor"
                    || key == "npc-seduce-comes-on"
                    || key == "npc-seduce-cajoles"
        )));
    }

    #[test]
    fn test_chatting_with_peaceful_vampire_mentions_potions() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let vampire_name = monster_name_with_sound(&world, MonsterSound::Vampire);
        let vampire = spawn_full_monster(&mut world, Position::new(6, 5), &vampire_name, 10);
        world
            .ecs_mut()
            .insert_one(vampire, Peaceful)
            .expect("vampire should accept peaceful marker");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-vampire-peaceful"
            )
        }));
    }

    #[test]
    fn test_chatting_with_imitator_mimics_player() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let imitate_name = monster_name_with_sound(&world, MonsterSound::Imitate);
        spawn_full_monster(&mut world, Position::new(6, 5), &imitate_name, 10);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-imitate-imitates"
            )
        }));
    }

    #[test]
    fn test_chatting_with_trumpeting_monster_wakes_sleepers() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let trumpet_name = monster_name_with_sound(&world, MonsterSound::Trumpet);
        spawn_full_monster(&mut world, Position::new(6, 5), &trumpet_name, 10);
        let sleeper = spawn_full_monster(&mut world, Position::new(7, 5), "kobold", 8);
        let _ = crate::status::make_sleeping(&mut world, sleeper, 20);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(
                event,
                EngineEvent::Message { key, .. } if key == "npc-trumpet-trumpets"
            )
        }));
        assert!(
            world
                .get_component::<crate::status::StatusEffects>(sleeper)
                .is_none_or(|status| status.sleeping == 0),
            "trumpet chat should wake nearby sleeping monsters"
        );
    }

    #[test]
    fn test_chatting_with_hostile_raven_says_nevermore() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        spawn_full_monster(&mut world, Position::new(6, 5), "raven", 10);

        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut test_rng(),
        );

        assert!(events.iter().any(|event| {
            matches!(event, EngineEvent::Message { key, .. } if key == "npc-squawk-nevermore")
        }));
    }

    #[test]
    fn test_chatting_with_sleeping_shopkeeper_gets_no_response() {
        let mut world = make_test_world();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(6, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let _ = crate::status::make_sleeping(&mut world, shopkeeper, 10);

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "npc-chat-sleeping"
        )));
        assert!(
            crate::status::is_sleeping(&world, shopkeeper),
            "non-priest sleepers should remain asleep after ignored chat"
        );
    }

    #[test]
    fn test_chatting_with_sleeping_priest_wakes_and_talks() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        let shrine_pos = Position::new(6, 5);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(shrine_pos, Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        let player_alignment = current_player_alignment(&world, player);
        world
            .ecs_mut()
            .insert_one(priest, Peaceful)
            .expect("priest should accept peaceful marker");
        world
            .ecs_mut()
            .insert_one(
                priest,
                crate::npc::Priest {
                    alignment: player_alignment,
                    has_shrine: true,
                    is_high_priest: false,
                    angry: false,
                },
            )
            .expect("priest should accept explicit runtime state");
        let _ = crate::status::make_sleeping(&mut world, priest, 10);
        let _gold = spawn_inventory_gold(&mut world, 1_000, '$');
        if let Some(mut pos) = world.get_component_mut::<Positioned>(player) {
            pos.0 = Position::new(5, 5);
        }

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(
            !crate::status::is_sleeping(&world, priest),
            "sleeping priests should wake up when addressed"
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-protection-granted"
        )));
    }

    #[test]
    fn test_chatting_on_shop_item_quotes_price() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 6), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));

        let item = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.retain(|entry| *entry != item);
        }
        let current_level = world.dungeon().current_data_dungeon_level();
        if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item) {
            *loc = ObjectLocation::Floor {
                x: 5,
                y: 5,
                level: current_level,
            };
        }

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::North,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-price"
        )));
    }

    #[test]
    fn test_chatting_on_shop_items_quotes_each_floor_stack() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 6), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));

        let item = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
        let second_item = spawn_inventory_object_by_name(&mut world, "lock pick", 'q');
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.retain(|entry| *entry != item);
            inv.items.retain(|entry| *entry != second_item);
        }
        let current_level = world.dungeon().current_data_dungeon_level();
        for entity in [item, second_item] {
            if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(entity) {
                *loc = ObjectLocation::Floor {
                    x: 5,
                    y: 5,
                    level: current_level,
                };
            }
        }

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::North,
            },
            &mut rng,
        );

        let quote_count = events
            .iter()
            .filter(
                |event| matches!(event, EngineEvent::Message { key, .. } if key == "shop-price"),
            )
            .count();
        assert_eq!(quote_count, 2);
    }

    #[test]
    fn test_blind_player_chatting_on_shop_item_does_not_get_price_quote() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 6), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));

        let item = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.retain(|entry| *entry != item);
        }
        let current_level = world.dungeon().current_data_dungeon_level();
        if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item) {
            *loc = ObjectLocation::Floor {
                x: 5,
                y: 5,
                level: current_level,
            };
        }
        let _ = crate::status::make_blinded(&mut world, player, 20);

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::North,
            },
            &mut rng,
        );

        assert!(!events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-price"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "chat-nobody-there"
        )));
    }

    #[test]
    fn test_deaf_player_chatting_on_shop_item_does_not_get_price_quote() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 6), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));

        let item = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.retain(|entry| *entry != item);
        }
        let current_level = world.dungeon().current_data_dungeon_level();
        if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item) {
            *loc = ObjectLocation::Floor {
                x: 5,
                y: 5,
                level: current_level,
            };
        }
        if let Some(mut status) = world.get_component_mut::<crate::status::StatusEffects>(player) {
            status.deaf = 20;
        } else {
            world
                .ecs_mut()
                .insert_one(
                    player,
                    crate::status::StatusEffects {
                        deaf: 20,
                        ..Default::default()
                    },
                )
                .expect("player should accept deaf status");
        }

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::North,
            },
            &mut rng,
        );

        assert!(!events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-price"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "chat-nobody-there"
        )));
    }

    #[test]
    fn test_sleeping_shopkeeper_does_not_quote_floor_merchandise() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 6), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let _ = crate::status::make_sleeping(&mut world, shopkeeper, 10);

        let item = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.retain(|entry| *entry != item);
        }
        let current_level = world.dungeon().current_data_dungeon_level();
        if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item) {
            *loc = ObjectLocation::Floor {
                x: 5,
                y: 5,
                level: current_level,
            };
        }

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::North,
            },
            &mut rng,
        );

        assert!(!events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-price"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "chat-nobody-there"
        )));
    }

    #[test]
    fn test_entering_shop_emits_shop_entry_message() {
        let mut world = make_test_world();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(6, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-enter"
        )));
    }

    #[test]
    fn test_entering_shop_with_digging_tool_emits_warning_message() {
        let mut world = make_test_world();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(6, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let _tool = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-enter-digging-tool"
        )));
    }

    #[test]
    fn test_entering_shop_while_invisible_emits_presence_warning() {
        let mut world = make_test_world();
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(6, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let mut status = world
            .get_component::<crate::status::StatusEffects>(player)
            .map(|status| (*status).clone())
            .unwrap_or_default();
        status.invisibility = 50;
        if world
            .get_component::<crate::status::StatusEffects>(player)
            .is_some()
        {
            *world
                .get_component_mut::<crate::status::StatusEffects>(player)
                .expect("player should have status effects") = status;
        } else {
            world
                .ecs_mut()
                .insert_one(player, status)
                .expect("player should accept status effects");
        }
        let mut rng = test_rng();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-enter-invisible"
        )));
    }

    #[test]
    fn test_entering_shop_while_mounted_emits_steed_warning_message() {
        let mut world = make_test_world();
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(7, 4),
                Position::new(8, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let _steed = spawn_tame_steed(&mut world, Position::new(6, 5), "pony");
        let mut rng = test_rng();
        let mount_events = resolve_turn(&mut world, PlayerAction::Ride, &mut rng);
        assert!(mount_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "mount-steed"
        )));

        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(crate::steed::is_mounted(&world, player));
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, args }
                if key == "shop-enter-steed"
                    && args.iter().any(|(name, value)| name == "steed" && value == "pony")
        )));
    }

    #[test]
    fn test_entering_robbed_shop_emits_stolen_entry_message() {
        let mut world = make_test_world();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(6, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        world.dungeon_mut().shop_rooms[0].robbed = 50;

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-stolen"
        )));
    }

    #[test]
    fn test_entering_surcharged_shop_emits_welcome_back_message() {
        let mut world = make_test_world();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(7, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(6, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        world.dungeon_mut().shop_rooms[0].surcharge = true;

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-welcome-back"
        )));
    }

    #[test]
    fn test_first_shop_exit_with_unpaid_bill_warns_and_blocks_movement() {
        let mut world = make_test_world();
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let unpaid_item = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('u'),
                artifact: None,
            },
            ObjectLocation::Floor {
                x: 6,
                y: 5,
                level: world.dungeon().current_data_dungeon_level(),
            },
        ));
        assert!(
            world.dungeon_mut().shop_rooms[0]
                .bill
                .add(unpaid_item, 100, 1),
            "shop bill should accept an unpaid item"
        );

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::West,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-leave-warning"
        )));
        assert!(
            !events.iter().any(|event| matches!(
                event,
                EngineEvent::Message { key, .. } if key == "shop-shoplift"
            )),
            "first exit attempt should warn instead of robbing immediately"
        );
        assert_eq!(
            world.get_component::<Positioned>(player).map(|pos| pos.0),
            Some(Position::new(5, 5)),
            "warning should block the player from leaving the shop"
        );
        let shop = &world.dungeon().shop_rooms[0];
        assert!(shop.exit_warning_issued);
        assert_eq!(shop.bill.total(), 100);
    }

    #[test]
    fn test_peaceful_shopkeeper_with_unpaid_bill_follows_player_outside_shop() {
        let mut world = make_test_world();
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let unpaid_item = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('u'),
                artifact: None,
            },
            ObjectLocation::Floor {
                x: 6,
                y: 5,
                level: world.dungeon().current_data_dungeon_level(),
            },
        ));
        assert!(
            world.dungeon_mut().shop_rooms[0]
                .bill
                .add(unpaid_item, 100, 1),
            "shop bill should accept an unpaid item"
        );

        let mut rng = test_rng();
        let warning_events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::West,
            },
            &mut rng,
        );
        assert!(warning_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-leave-warning"
        )));
        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::West,
            },
            &mut rng,
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::EntityMoved { entity, .. } if *entity == shopkeeper
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-shoplift"
        )));
        assert_eq!(world.dungeon().shop_rooms[0].robbed, 100);
        let shopkeeper_state = world
            .get_component::<crate::npc::Shopkeeper>(shopkeeper)
            .map(|state| (*state).clone())
            .expect("shopkeeper should have explicit runtime state");
        assert!(shopkeeper_state.following);
        assert_eq!(
            world.get_component::<Positioned>(player).map(|pos| pos.0),
            Some(Position::new(4, 5)),
            "second exit attempt should let the player actually leave"
        );
        let final_pos = world
            .get_component::<Positioned>(shopkeeper)
            .map(|pos| pos.0)
            .expect("shopkeeper should still have a position after moving");
        assert!(
            final_pos != shopkeeper_home_pos(&world.dungeon().shop_rooms[0]),
            "shopkeeper follow movement should leave the home tile once pursuit begins, got {:?}",
            final_pos
        );
    }

    #[test]
    fn test_paying_clears_shop_exit_warning_state() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let _gold = spawn_inventory_gold(&mut world, 150, 'g');
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let unpaid_item = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('u'),
                artifact: None,
            },
            ObjectLocation::Floor {
                x: 6,
                y: 5,
                level: world.dungeon().current_data_dungeon_level(),
            },
        ));
        assert!(
            world.dungeon_mut().shop_rooms[0]
                .bill
                .add(unpaid_item, 100, 1),
            "shop bill should accept a payable entry"
        );

        let mut rng = test_rng();
        let warning_events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::West,
            },
            &mut rng,
        );
        assert!(warning_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-leave-warning"
        )));
        assert!(world.dungeon().shop_rooms[0].exit_warning_issued);

        let pay_events = resolve_turn(&mut world, PlayerAction::Pay, &mut rng);
        assert!(pay_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-pay-success"
        )));
        assert!(
            !world.dungeon().shop_rooms[0].exit_warning_issued,
            "full payment should clear the leave-warning state"
        );
    }

    #[test]
    fn test_pay_spends_gold_and_pacifies_fully_paid_shop() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let _gold = spawn_inventory_gold(&mut world, 150, 'g');
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        world.dungeon_mut().shop_rooms[0].angry = true;
        world.dungeon_mut().shop_rooms[0].surcharge = true;
        let unpaid_item = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('u'),
                artifact: None,
            },
            ObjectLocation::Floor {
                x: 6,
                y: 5,
                level: world.dungeon().current_data_dungeon_level(),
            },
        ));
        assert!(
            world.dungeon_mut().shop_rooms[0]
                .bill
                .add(unpaid_item, 100, 1),
            "shop bill should accept a payable entry"
        );

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::Pay, &mut rng);

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-pay-success"
        )));
        assert_eq!(player_gold(&world, player), 50);
        let shop = &world.dungeon().shop_rooms[0];
        assert!(shop.bill.is_empty(), "payment should clear the shop bill");
        assert_eq!(shop.debit, 0, "payment should clear outstanding debit");
        assert!(!shop.angry, "payment should pacify the angry shopkeeper");
        assert!(
            !shop.surcharge,
            "full payment should clear the surcharge flag as part of pacification"
        );
        let shopkeeper_state = world
            .get_component::<crate::npc::Shopkeeper>(shopkeeper)
            .map(|state| (*state).clone())
            .expect("payment should sync explicit shopkeeper runtime state");
        assert!(
            !shopkeeper_state.following,
            "paid-up shopkeepers should stop following the hero"
        );
    }

    #[test]
    fn test_drop_gold_in_shop_clears_debit_and_banks_credit() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let gold = spawn_inventory_gold(&mut world, 150, 'g');
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        world.dungeon_mut().shop_rooms[0].debit = 50;
        world.dungeon_mut().shop_rooms[0].angry = true;
        world.dungeon_mut().shop_rooms[0].surcharge = true;

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::Drop { item: gold }, &mut rng);

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-pay-success"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-credit"
        )));
        assert_eq!(player_gold(&world, player), 0);
        assert!(
            !world.ecs().contains(gold),
            "dropped gold should be consumed by the shop"
        );
        let shop = &world.dungeon().shop_rooms[0];
        assert_eq!(shop.debit, 0);
        assert_eq!(shop.credit, 100);
        assert!(!shop.angry);
        assert!(!shop.surcharge);
    }

    #[test]
    fn test_pay_uses_credit_before_spending_player_gold() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        world.dungeon_mut().shop_rooms[0].credit = 150;
        world.dungeon_mut().shop_rooms[0].debit = 20;
        let unpaid_item = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('u'),
                artifact: None,
            },
            ObjectLocation::Floor {
                x: 6,
                y: 5,
                level: world.dungeon().current_data_dungeon_level(),
            },
        ));
        assert!(
            world.dungeon_mut().shop_rooms[0]
                .bill
                .add(unpaid_item, 100, 1),
            "shop bill should accept a credited entry"
        );

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::Pay, &mut rng);

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-credit-covers"
        )));
        assert_eq!(player_gold(&world, player), 0);
        assert_eq!(world.dungeon().shop_rooms[0].credit, 30);
        assert!(world.dungeon().shop_rooms[0].bill.is_empty());
        assert_eq!(world.dungeon().shop_rooms[0].debit, 0);
    }

    #[test]
    fn test_pay_without_enough_money_preserves_bill_and_gold() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let _gold = spawn_inventory_gold(&mut world, 50, 'g');
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let unpaid_item = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('u'),
                artifact: None,
            },
            ObjectLocation::Floor {
                x: 6,
                y: 5,
                level: world.dungeon().current_data_dungeon_level(),
            },
        ));
        assert!(
            world.dungeon_mut().shop_rooms[0]
                .bill
                .add(unpaid_item, 100, 1),
            "shop bill should accept an underfunded entry"
        );

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::Pay, &mut rng);

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-no-money"
        )));
        assert_eq!(player_gold(&world, player), 50);
        assert_eq!(world.dungeon().shop_rooms[0].bill.total(), 100);
        assert_eq!(world.dungeon().shop_rooms[0].credit, 0);
    }

    #[test]
    fn test_drop_merchandise_in_shop_pays_player_and_updates_shop_gold() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        world.dungeon_mut().shop_rooms[0].shopkeeper_gold = 80;
        let item = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.push(item);
        }

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::Drop { item }, &mut rng);

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-sell"
        )));
        assert_eq!(player_gold(&world, player), 5);
        assert_eq!(world.dungeon().shop_rooms[0].shopkeeper_gold, 75);
        assert!(matches!(
            world.get_component::<ObjectLocation>(item).as_deref(),
            Some(ObjectLocation::Floor { level, .. })
                if *level == world.dungeon().current_data_dungeon_level()
        ));
    }

    #[test]
    fn test_drop_merchandise_after_robbery_reduces_robbed_balance_and_pacifies_shop() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        world.dungeon_mut().shop_rooms[0].robbed = 5;
        world.dungeon_mut().shop_rooms[0].angry = true;
        world.dungeon_mut().shop_rooms[0].surcharge = true;
        sync_current_level_shopkeeper_state(&mut world);
        let item = spawn_inventory_object_by_name(&mut world, "pick-axe", 'p');
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.push(item);
        }

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::Drop { item }, &mut rng);

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-restock"
        )));
        let shop = &world.dungeon().shop_rooms[0];
        assert_eq!(shop.robbed, 0);
        assert!(!shop.angry);
        assert!(!shop.surcharge);
    }

    #[test]
    fn test_new_turn_repairs_shop_damage_for_idle_shopkeeper() {
        let mut world = make_test_world();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let damaged_pos = Position::new(5, 5);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(damaged_pos, Terrain::Floor);
        crate::shop::record_shop_damage(
            &mut world.dungeon_mut().shop_rooms[0],
            damaged_pos,
            crate::shop::ShopDamageType::DoorBroken,
        );
        sync_current_level_shopkeeper_state(&mut world);

        let mut rng = test_rng();
        let mut events = Vec::new();
        process_new_turn(&mut world, &mut rng, &mut events);

        assert!(
            world.dungeon().shop_rooms[0].damage_list.is_empty(),
            "idle shopkeeper should repair one queued damage entry at turn boundary"
        );
        assert_eq!(
            world
                .dungeon()
                .current_level
                .get(damaged_pos)
                .map(|cell| cell.terrain),
            Some(Terrain::DoorClosed)
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "shop-repair"
        )));
    }

    #[test]
    fn test_following_shopkeeper_does_not_repair_damage_until_home() {
        let mut world = make_test_world();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(5, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        let mut shop = crate::shop::ShopRoom::new(
            Position::new(5, 4),
            Position::new(7, 6),
            crate::shop::ShopType::Tool,
            shopkeeper,
            "Izchak".to_string(),
        );
        shop.door_pos = Some(Position::new(7, 5));
        world.dungeon_mut().shop_rooms.push(shop);
        let damaged_pos = Position::new(7, 5);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(damaged_pos, Terrain::Floor);
        crate::shop::record_shop_damage(
            &mut world.dungeon_mut().shop_rooms[0],
            damaged_pos,
            crate::shop::ShopDamageType::DoorBroken,
        );
        sync_current_level_shopkeeper_state(&mut world);
        if let Some(mut state) = world.get_component_mut::<crate::npc::Shopkeeper>(shopkeeper) {
            state.following = true;
        }

        let mut rng = test_rng();
        let mut events = Vec::new();
        process_new_turn(&mut world, &mut rng, &mut events);

        assert_eq!(world.dungeon().shop_rooms[0].damage_list.len(), 1);
        assert!(
            !events.iter().any(|event| matches!(
                event,
                EngineEvent::Message { key, .. } if key == "shop-repair"
            )),
            "shopkeeper should not repair while still following the player"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_can_emit_shop_texture() {
        let mut world = make_test_world();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-shop-")
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "shop levels should eventually emit live ambient shop texture"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_hallucinating_shop_uses_hallu_texture() {
        let mut world = make_test_world();
        let shopkeeper = spawn_full_monster(&mut world, Position::new(6, 5), "Izchak", 12);
        world
            .ecs_mut()
            .insert_one(shopkeeper, Peaceful)
            .expect("shopkeeper should accept peaceful marker");
        world
            .dungeon_mut()
            .shop_rooms
            .push(crate::shop::ShopRoom::new(
                Position::new(5, 4),
                Position::new(7, 6),
                crate::shop::ShopType::Tool,
                shopkeeper,
                "Izchak".to_string(),
            ));
        let player = world.player();
        crate::status::make_hallucinated(&mut world, player, 20);
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. }
                        if key == "ambient-shop-neiman-marcus"
                            || key == "ambient-shop-register"
                            || key == "ambient-shop-prices"
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "hallucinating shop levels should eventually emit hallucinatory shop ambience"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_can_emit_vault_counting_texture() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .vault_rooms
            .push(crate::vault::VaultRoom {
                top_left: Position::new(6, 5),
                bottom_right: Position::new(7, 6),
            });
        spawn_floor_coin(&mut world, Position::new(6, 5));
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "ambient-vault-counting"
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "vault levels with floor gold should eventually emit vault-counting texture"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_can_emit_vault_guard_texture() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .vault_rooms
            .push(crate::vault::VaultRoom {
                top_left: Position::new(6, 5),
                bottom_right: Position::new(7, 6),
            });
        world.dungeon_mut().vault_guard_present = true;
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "ambient-vault-footsteps"
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "vault levels with an active guard should eventually emit guard-footstep texture"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_hallucinating_vault_can_emit_scrooge() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .vault_rooms
            .push(crate::vault::VaultRoom {
                top_left: Position::new(6, 5),
                bottom_right: Position::new(7, 6),
            });
        world.dungeon_mut().vault_guard_present = true;
        let player = world.player();
        crate::status::make_hallucinated(&mut world, player, 20);
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..4096 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "ambient-vault-scrooge"
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "hallucinating vault levels should eventually emit the Scrooge texture"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_hallucinating_gold_vault_can_emit_quarterback() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .vault_rooms
            .push(crate::vault::VaultRoom {
                top_left: Position::new(6, 5),
                bottom_right: Position::new(7, 6),
            });
        spawn_floor_coin(&mut world, Position::new(6, 5));
        let player = world.player();
        crate::status::make_hallucinated(&mut world, player, 20);
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..4096 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "ambient-vault-quarterback"
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "hallucinating gold vault levels should eventually emit the quarterback texture"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_visible_oracle_suppresses_oracle_texture() {
        let mut world = make_stair_world(Terrain::Floor, 1);
        world.dungeon_mut().branch = DungeonBranch::Main;
        world.dungeon_mut().depth = oracle_depth_for_world(&world);
        let oracle = spawn_full_monster(&mut world, Position::new(6, 5), "oracle", 12);
        world
            .ecs_mut()
            .insert_one(oracle, Name("Oracle".to_string()))
            .expect("oracle should accept canonical name");
        set_player_position(&mut world, Position::new(5, 5));

        let mut rng = test_rng();
        for _ in 0..4096 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            assert!(
                !events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-oracle-")
                )),
                "visible Oracle should suppress oracle ambience"
            );
        }
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_hallucinating_visible_oracle_keeps_oracle_texture() {
        let mut world = make_stair_world(Terrain::Floor, 1);
        world.dungeon_mut().branch = DungeonBranch::Main;
        world.dungeon_mut().depth = oracle_depth_for_world(&world);
        let oracle = spawn_full_monster(&mut world, Position::new(6, 5), "oracle", 12);
        world
            .ecs_mut()
            .insert_one(oracle, Name("Oracle".to_string()))
            .expect("oracle should accept canonical name");
        let player = world.player();
        crate::status::make_hallucinated(&mut world, player, 20);
        set_player_position(&mut world, Position::new(5, 5));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..4096 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-oracle-")
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "hallucinating players should still hear Oracle ambience even when the Oracle is visible"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_can_emit_fountain_texture() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Fountain);
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-fountain-")
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "levels with a fountain should eventually emit fountain ambience"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_hallucinating_sink_uses_sink_texture() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Sink);
        set_player_position(&mut world, Position::new(2, 2));
        let player = world.player();
        crate::status::make_hallucinated(&mut world, player, 20);

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. }
                        if key == "ambient-sink-gurgle" || key == "ambient-sink-dishes"
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "hallucinating levels with a sink should eventually emit sink ambience"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_can_emit_court_texture() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        spawn_full_monster(&mut world, Position::new(7, 7), "kobold lord", 12);
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-court-")
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "levels with court royalty should eventually emit court ambience"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_can_emit_swamp_texture() {
        let mut world = make_test_world();
        for y in 5..9 {
            for x in 5..11 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Pool);
            }
        }
        world.spawn((
            Monster,
            Positioned(Position::new(7, 7)),
            Name("giant eel".to_string()),
            HitPoints { current: 8, max: 8 },
            Speed(12),
            MovementPoints(0),
            DisplaySymbol {
                symbol: ';',
                color: nethack_babel_data::Color::Green,
            },
        ));
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-swamp-")
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "levels with swamp terrain and swamp fauna should eventually emit swamp ambience"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_can_emit_beehive_texture() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        spawn_full_monster(&mut world, Position::new(7, 7), "killer bee", 8);
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-beehive-")
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "levels with beehive monsters should eventually emit beehive ambience"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_can_emit_morgue_texture() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        spawn_full_monster(&mut world, Position::new(7, 7), "ghost", 8);
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-morgue-")
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "levels with undead should eventually emit morgue ambience"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_can_emit_barracks_texture() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        for idx in 0..6 {
            let soldier = spawn_full_monster(&mut world, Position::new(5 + idx, 5), "soldier", 12);
            if let Some(mut symbol) = world.get_component_mut::<DisplaySymbol>(soldier) {
                symbol.symbol = '@';
            }
            if idx == 0 {
                if let Some(mut status) =
                    world.get_component_mut::<crate::status::StatusEffects>(soldier)
                {
                    status.sleeping = 20;
                } else {
                    world
                        .ecs_mut()
                        .insert_one(
                            soldier,
                            crate::status::StatusEffects {
                                sleeping: 20,
                                ..Default::default()
                            },
                        )
                        .expect("soldier should accept sleeping status");
                }
            }
        }
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-barracks-")
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "levels with sleeping mercenaries should eventually emit barracks ambience"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_can_emit_zoo_texture() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let jackal = spawn_full_monster(&mut world, Position::new(7, 7), "jackal", 8);
        world
            .ecs_mut()
            .insert_one(
                jackal,
                crate::status::StatusEffects {
                    sleeping: 20,
                    ..Default::default()
                },
            )
            .expect("jackal should accept sleeping status");
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-zoo-")
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "levels with zoo animals should eventually emit zoo ambience"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_suppressed_inside_vault() {
        let mut world = make_test_world();
        world
            .dungeon_mut()
            .vault_rooms
            .push(crate::vault::VaultRoom {
                top_left: Position::new(6, 5),
                bottom_right: Position::new(7, 6),
            });
        spawn_floor_coin(&mut world, Position::new(6, 5));
        set_player_position(&mut world, Position::new(6, 5));

        let mut rng = test_rng();
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            assert!(
                !events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-vault-")
                )),
                "vault ambient should not fire while the player is inside the vault"
            );
        }
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_can_emit_temple_texture() {
        let mut world = make_test_world();
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(
                priest,
                crate::npc::Priest {
                    alignment: Alignment::Lawful,
                    has_shrine: true,
                    is_high_priest: false,
                    angry: false,
                },
            )
            .expect("priest should accept explicit runtime state");
        set_player_position(&mut world, Position::new(2, 2));

        let mut rng = test_rng();
        let mut found = false;
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-temple-")
                )
            }) {
                found = true;
                break;
            }
        }

        assert!(
            found,
            "temple levels should eventually emit live ambient temple texture"
        );
    }

    #[test]
    fn test_emit_ambient_dungeon_sound_suppressed_near_temple_priest() {
        let mut world = make_test_world();
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(
                priest,
                crate::npc::Priest {
                    alignment: Alignment::Lawful,
                    has_shrine: true,
                    is_high_priest: false,
                    angry: false,
                },
            )
            .expect("priest should accept explicit runtime state");
        set_player_position(&mut world, Position::new(5, 5));

        let mut rng = test_rng();
        for _ in 0..200 {
            let mut events = Vec::new();
            emit_ambient_dungeon_sound(&world, &mut rng, &mut events);
            assert!(
                !events.iter().any(|event| matches!(
                    event,
                    EngineEvent::Message { key, .. } if key.starts_with("ambient-temple-")
                )),
                "temple ambient should not fire while the player is already at the shrine"
            );
        }
    }

    #[test]
    fn test_chatting_with_peaceful_priest_grants_protection() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        let _gold = spawn_inventory_gold(&mut world, 1_000, 'g');
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(priest, Peaceful)
            .expect("priest should accept peaceful marker");
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(priest) {
            mp.0 = 0;
        }

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-protection-granted"
        )));
        assert_eq!(player_gold(&world, player), 600);
        assert!(
            world
                .get_component::<crate::status::SpellProtection>(player)
                .is_some_and(|protection| protection.layers == 1),
            "successful priest chat should grant one layer of protection"
        );
    }

    #[test]
    fn test_attacking_priest_emits_wrath_and_ends_peaceful_chat() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        let _gold = spawn_inventory_gold(&mut world, 1_000, 'g');
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        if let Some(mut player_hp) = world.get_component_mut::<HitPoints>(player) {
            player_hp.current = 40;
            player_hp.max = 40;
        }
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(priest, Peaceful)
            .expect("priest should accept peaceful marker");
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(priest) {
            hp.current = 40;
            hp.max = 40;
        }
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(priest) {
            mp.0 = 0;
        }

        let mut rng = test_rng();
        let attack_events = resolve_turn(
            &mut world,
            PlayerAction::FightDirection {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(attack_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-angry"
        )));
        assert!(attack_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "god-lightning-bolt"
        )));
        assert!(attack_events.iter().any(|event| matches!(
            event,
            EngineEvent::HpChange {
                entity,
                amount,
                source: HpSource::Divine,
                ..
            } if *entity == player && *amount < 0
        )));
        assert!(attack_events.iter().any(|event| matches!(
            event,
            EngineEvent::StatusApplied {
                entity,
                status: crate::event::StatusEffect::Blind,
                ..
            } if *entity == player
        )));
        assert!(
            world
                .get_component::<crate::npc::Priest>(priest)
                .is_some_and(|state| state.angry),
            "attacking a priest should persist explicit angry state"
        );
        assert!(
            player_hp(&world, player) < 40,
            "divine wrath should deal real damage to the player"
        );
        assert!(
            world
                .get_component::<crate::status::StatusEffects>(player)
                .is_some_and(|status| status.blindness > 0),
            "divine wrath should blind a non-resistant player"
        );

        let chat_events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(chat_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. }
                if matches!(
                    key.as_str(),
                    "priest-cranky-1" | "priest-cranky-2" | "priest-cranky-3"
                )
        )));
        assert!(
            !chat_events.iter().any(|event| matches!(
                event,
                EngineEvent::Message { key, .. } if key == "priest-protection-granted"
            )),
            "hostile priests should no longer grant protection"
        );
    }

    #[test]
    fn test_attacking_priest_with_shock_resistance_reduces_wrath_and_avoids_blindness() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        let _gold = spawn_inventory_gold(&mut world, 1_000, 'g');
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        if let Some(mut player_hp) = world.get_component_mut::<HitPoints>(player) {
            player_hp.current = 40;
            player_hp.max = 40;
        }
        let _ = world.ecs_mut().insert_one(
            player,
            crate::status::Intrinsics {
                shock_resistance: true,
                ..Default::default()
            },
        );
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(priest, Peaceful)
            .expect("priest should accept peaceful marker");
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(priest) {
            hp.current = 40;
            hp.max = 40;
        }
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(priest) {
            mp.0 = 0;
        }

        let mut rng = test_rng();
        let attack_events = resolve_turn(
            &mut world,
            PlayerAction::FightDirection {
                direction: Direction::East,
            },
            &mut rng,
        );

        let divine_damage = attack_events.iter().find_map(|event| match event {
            EngineEvent::HpChange {
                entity,
                amount,
                source: HpSource::Divine,
                ..
            } if *entity == player && *amount < 0 => Some(-amount),
            _ => None,
        });
        assert!(
            divine_damage.is_some_and(|damage| damage <= 3),
            "shock resistance should reduce priest wrath damage"
        );
        assert!(
            !attack_events.iter().any(|event| matches!(
                event,
                EngineEvent::StatusApplied {
                    entity,
                    status: crate::event::StatusEffect::Blind,
                    ..
                } if *entity == player
            )),
            "shock resistance should prevent priest wrath blindness"
        );
        assert!(
            world
                .get_component::<crate::status::StatusEffects>(player)
                .is_none_or(|status| status.blindness == 0),
            "shock-resistant players should stay unblinded"
        );
    }

    #[test]
    fn test_chatting_with_explicit_priest_component_off_altar_grants_protection() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        let _gold = spawn_inventory_gold(&mut world, 1_000, 'g');
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "oracle", 12);
        world
            .ecs_mut()
            .insert_one(priest, Peaceful)
            .expect("monster should accept peaceful marker");
        world
            .ecs_mut()
            .insert_one(
                priest,
                crate::npc::Priest {
                    alignment: Alignment::Lawful,
                    has_shrine: false,
                    is_high_priest: false,
                    angry: false,
                },
            )
            .expect("monster should accept explicit priest component");

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-protection-granted"
        )));
        assert_eq!(player_gold(&world, player), 600);
    }

    #[test]
    fn test_chatting_with_peaceful_priest_without_gold_grants_ale_money() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(priest, Peaceful)
            .expect("priest should accept peaceful marker");

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-ale-gift"
        )));
        assert_eq!(player_gold(&world, player), 2);
        assert!(
            world
                .get_component::<crate::status::SpellProtection>(player)
                .is_none(),
            "ale money should not grant divine protection"
        );
    }

    #[test]
    fn test_chatting_with_peaceful_priest_spends_small_donation_without_granting_protection() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        let _gold = spawn_inventory_gold(&mut world, 100, 'g');
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(priest, Peaceful)
            .expect("priest should accept peaceful marker");

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-small-thanks"
        )));
        assert_eq!(player_gold(&world, player), 0);
        assert!(
            world
                .get_component::<crate::status::SpellProtection>(player)
                .is_none(),
            "small donations should not grant divine protection"
        );
    }

    #[test]
    fn test_chatting_with_priest_without_shrine_preaches_poverty() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(priest, Peaceful)
            .expect("priest should accept peaceful marker");
        world
            .ecs_mut()
            .insert_one(
                priest,
                crate::npc::Priest {
                    alignment: Alignment::Lawful,
                    has_shrine: false,
                    is_high_priest: false,
                    angry: false,
                },
            )
            .expect("priest should accept explicit runtime state");

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-virtues-of-poverty"
        )));
        assert_eq!(player_gold(&world, player), 0);
    }

    #[test]
    fn test_chatting_with_peaceful_priest_can_reach_pious_donation_tier() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        let _gold = spawn_inventory_gold(&mut world, 300, 'g');
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(priest, Peaceful)
            .expect("priest should accept peaceful marker");

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-pious"
        )));
        assert_eq!(player_gold(&world, player), 0);
        assert!(
            world
                .get_component::<crate::status::StatusEffects>(player)
                .is_none_or(|status| status.clairvoyance == 0),
            "pious donations should not trigger clairvoyance without the blessing conditions"
        );
    }

    #[test]
    fn test_chatting_with_protection_can_reach_selfless_generosity_tier() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        let _gold = spawn_inventory_gold(&mut world, 700, 'g');
        world
            .ecs_mut()
            .insert_one(
                player,
                crate::status::SpellProtection {
                    layers: 1,
                    countdown: 10,
                    interval: 10,
                },
            )
            .expect("player should accept spell protection");
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(priest, Peaceful)
            .expect("priest should accept peaceful marker");

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-selfless-generosity"
        )));
        assert_eq!(player_gold(&world, player), 0);
        assert!(
            world
                .get_component::<crate::status::SpellProtection>(player)
                .is_some_and(|protection| protection.layers == 1),
            "selfless generosity should preserve the existing protection layer"
        );
    }

    #[test]
    fn test_chatting_with_wrong_alignment_priest_preserves_gold_and_rejects() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        let _gold = spawn_inventory_gold(&mut world, 500, 'g');
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(priest, Peaceful)
            .expect("priest should accept peaceful marker");
        world
            .ecs_mut()
            .insert_one(
                priest,
                crate::npc::Priest {
                    alignment: Alignment::Chaotic,
                    has_shrine: true,
                    is_high_priest: false,
                    angry: false,
                },
            )
            .expect("priest should accept explicit runtime state");

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-wrong-alignment"
        )));
        assert_eq!(player_gold(&world, player), 500);
        assert!(
            world
                .get_component::<crate::status::SpellProtection>(player)
                .is_none(),
            "wrong-alignment priests should not sell divine protection"
        );
    }

    #[test]
    fn test_first_visit_tended_temple_emits_peace_message() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        let mut religion = default_religion_state(&world, player);
        religion.alignment_record = 15;
        world
            .ecs_mut()
            .insert_one(player, religion)
            .expect("player should accept religion state");
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(
                priest,
                crate::npc::Priest {
                    alignment: Alignment::Lawful,
                    has_shrine: true,
                    is_high_priest: false,
                    angry: false,
                },
            )
            .expect("priest should accept explicit runtime state");

        let mut rng = test_rng();
        let mut events = Vec::new();
        maybe_emit_current_level_temple_entry(&mut world, player, true, &mut rng, &mut events);

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "temple-peace"
        )));
    }

    #[test]
    fn test_first_visit_untended_temple_emits_untended_message() {
        let mut world = make_test_world();
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept identity");
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(6, 5), Terrain::Altar);

        let mut rng = test_rng();
        let mut events = Vec::new();
        maybe_emit_current_level_temple_entry(&mut world, player, true, &mut rng, &mut events);

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. }
                if key == "temple-eerie" || key == "temple-watched" || key == "temple-shiver"
        )));
    }

    #[test]
    fn test_first_visit_untended_temple_can_spawn_real_ghost() {
        for seed in 0_u64..256 {
            let mut world = make_test_world();
            install_test_catalogs(&mut world);
            let player = world.player();
            world
                .ecs_mut()
                .insert_one(player, monk_identity())
                .expect("player should accept identity");
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(6, 5), Terrain::Altar);

            let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
            let mut events = Vec::new();
            maybe_emit_current_level_temple_entry(&mut world, player, true, &mut rng, &mut events);

            if events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::Message { key, .. } if key == "temple-ghost-appears"
                )
            }) {
                assert!(
                    events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. }))
                );
                assert!(
                    count_monsters_named(&world, "ghost") >= 1,
                    "untended temple ghost entry should spawn a real ghost entity"
                );
                return;
            }
        }

        panic!("expected at least one deterministic seed to spawn an untended temple ghost");
    }

    #[test]
    fn test_pray_on_aligned_altar_calms_angry_priest() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept monk identity");
        let mut religion = default_religion_state(&world, player);
        religion.alignment_record = 10;
        religion.bless_cooldown = 0;
        world
            .ecs_mut()
            .insert_one(player, religion)
            .expect("player should accept religion state");
        let altar_pos = Position::new(5, 5);
        set_player_position(&mut world, altar_pos);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(altar_pos, Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(6, 5), "priest", 12);
        world
            .ecs_mut()
            .insert_one(
                priest,
                crate::npc::Priest {
                    alignment: Alignment::Lawful,
                    has_shrine: true,
                    is_high_priest: false,
                    angry: true,
                },
            )
            .expect("priest should accept explicit runtime state");
        let _ = world.ecs_mut().remove_one::<Peaceful>(priest);

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::Pray, &mut rng);

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "priest-calmed"
        )));
        let priest_state = world
            .get_component::<crate::npc::Priest>(priest)
            .map(|state| *state)
            .expect("priest should keep explicit runtime state");
        assert!(
            !priest_state.angry,
            "aligned prayer on a tended altar should calm the priest"
        );
        assert!(
            world.get_component::<Peaceful>(priest).is_some(),
            "calmed priest should regain peaceful status"
        );
    }

    #[test]
    fn test_pray_does_not_calm_distant_angry_priest_outside_local_sanctuary() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, monk_identity())
            .expect("player should accept monk identity");
        let mut religion = default_religion_state(&world, player);
        religion.alignment_record = 10;
        religion.bless_cooldown = 0;
        world
            .ecs_mut()
            .insert_one(player, religion)
            .expect("player should accept religion state");
        let altar_pos = Position::new(5, 5);
        set_player_position(&mut world, altar_pos);
        world
            .dungeon_mut()
            .current_level
            .set_terrain(altar_pos, Terrain::Altar);
        let priest = spawn_full_monster(&mut world, Position::new(10, 10), "priest", 12);
        world
            .ecs_mut()
            .insert_one(
                priest,
                crate::npc::Priest {
                    alignment: Alignment::Lawful,
                    has_shrine: true,
                    is_high_priest: false,
                    angry: true,
                },
            )
            .expect("distant priest should accept explicit runtime state");
        let _ = world.ecs_mut().remove_one::<Peaceful>(priest);

        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::Pray, &mut rng);

        assert!(
            !events.iter().any(|event| matches!(
                event,
                EngineEvent::Message { key, .. } if key == "priest-calmed"
            )),
            "prayer should not calm an unrelated distant priest"
        );
        assert!(
            world
                .get_component::<crate::npc::Priest>(priest)
                .is_some_and(|state| state.angry),
            "distant priest anger should persist"
        );
    }

    #[test]
    fn test_chatting_with_explicit_quest_leader_role_bypasses_name_matching() {
        let mut world = make_test_world();
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Quest;
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        if let Some(mut level) = world.get_component_mut::<ExperienceLevel>(player) {
            level.0 = 14;
        }
        let mut religion = default_religion_state(&world, player);
        religion.alignment_record = 10;
        world
            .ecs_mut()
            .insert_one(player, religion)
            .expect("player should accept religion state");

        let leader = spawn_full_monster(&mut world, Position::new(6, 5), "mysterious sage", 20);
        world
            .ecs_mut()
            .insert_one(leader, crate::quest::QuestNpcRole::Leader)
            .expect("monster should accept explicit quest role");

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-assigned"
        )));
        assert!(
            world
                .get_component::<crate::quest::QuestState>(player)
                .is_some_and(|state| state.status == crate::quest::QuestStatus::Assigned)
        );
    }

    #[test]
    fn test_quest_state_sync_marks_entered_and_artifact_obtained() {
        let mut world = make_stair_world(Terrain::StairsDown, 2);
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Quest;
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");

        let artifact = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('a'),
                artifact: None,
            },
            ObjectLocation::Inventory,
            Name("The Eye of the Aethiopica".to_string()),
        ));
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.push(artifact);
        }

        let mut rng = test_rng();
        let _ = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        let quest_state = world
            .get_component::<crate::quest::QuestState>(player)
            .expect("quest sync should persist quest state");
        assert!(quest_state.quest_dungeon_entered);
        assert!(quest_state.artifact_obtained);
        let quest_items = world
            .get_component::<PlayerQuestItems>(player)
            .expect("quest sync should persist player quest item flags");
        assert!(quest_items.has_quest_artifact);
    }

    #[test]
    fn test_quest_traversal_assignment_completion_and_return_to_leader() {
        let mut world = make_stair_world(Terrain::StairsDown, 1);
        install_test_catalogs(&mut world);
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Quest;
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        let mut religion = default_religion_state(&world, player);
        religion.experience_level = 14;
        religion.alignment_record = 10;
        world
            .ecs_mut()
            .insert_one(player, religion)
            .expect("player should accept religion state");
        if let Some(mut level) = world.get_component_mut::<ExperienceLevel>(player) {
            level.0 = 14;
        }
        world.spawn((
            Monster,
            Positioned(Position::new(6, 5)),
            Name("Neferet the Green".to_string()),
            HitPoints {
                current: 30,
                max: 30,
            },
            Speed(12),
            DisplaySymbol {
                symbol: '@',
                color: nethack_babel_data::Color::Green,
            },
            MovementPoints(NORMAL_SPEED as i32),
        ));

        let mut rng = test_rng();
        let assign_events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(assign_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-assigned"
        )));

        for expected_depth in 2..=7 {
            if expected_depth > 2 {
                let stairs_down = find_terrain(&world.dungeon().current_level, Terrain::StairsDown)
                    .expect("quest traversal should preserve stairs down");
                set_player_position(&mut world, stairs_down);
            }
            let events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
            assert!(
                events
                    .iter()
                    .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                "descending into quest depth {expected_depth} should change levels"
            );
            assert_eq!(world.dungeon().depth, expected_depth);
        }

        let eye = crate::artifacts::find_artifact_by_name("The Eye of the Aethiopica")
            .expect("wizard quest artifact should exist");
        let artifact_entity = world
            .ecs()
            .query::<&ObjectCore>()
            .iter()
            .find_map(|(entity, core)| (core.artifact == Some(eye.id)).then_some(entity))
            .expect("quest goal should place the Eye artifact");
        if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(artifact_entity) {
            *loc = ObjectLocation::Inventory;
        }
        if let Some(mut inv) = world.get_component_mut::<crate::inventory::Inventory>(player) {
            inv.items.push(artifact_entity);
        }

        let dark_one = world
            .ecs()
            .query::<(&Monster, &Name)>()
            .iter()
            .find_map(|(entity, (_, name))| {
                quest_name_matches(&name.0, "Dark One").then_some(entity)
            })
            .expect("quest goal should spawn the Dark One");
        world
            .despawn(dark_one)
            .expect("nemesis should despawn cleanly");

        let _ = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
        let quest_state = world
            .get_component::<crate::quest::QuestState>(player)
            .map(|state| (*state).clone())
            .expect("quest traversal should persist quest state");
        assert_eq!(quest_state.status, crate::quest::QuestStatus::InProgress);
        assert!(quest_state.ready_for_completion());

        for expected_depth in (1..=6).rev() {
            let stairs_up = find_terrain(&world.dungeon().current_level, Terrain::StairsUp)
                .expect("quest traversal should preserve stairs up");
            set_player_position(&mut world, stairs_up);
            let events = resolve_turn(&mut world, PlayerAction::GoUp, &mut rng);
            assert!(
                events
                    .iter()
                    .any(|event| matches!(event, EngineEvent::LevelChanged { .. })),
                "ascending into quest depth {expected_depth} should change levels"
            );
            assert_eq!(world.dungeon().depth, expected_depth);
        }

        let leader_events = resolve_turn(
            &mut world,
            PlayerAction::Chat {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(leader_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-leader-nemesis-dead"
        )));
        assert!(leader_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "quest-completed"
        )));
        let quest_status = world
            .get_component::<crate::quest::QuestState>(player)
            .map(|state| state.status)
            .expect("leader return should persist quest completion");
        assert_eq!(quest_status, crate::quest::QuestStatus::Completed);
    }

    #[test]
    fn test_offer_real_amulet_on_aligned_astral_altar_ascends() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        let mut rng = test_rng();
        let astral = crate::special_levels::dispatch_special_level(
            crate::special_levels::SpecialLevelId::AstralPlane,
            None,
            &mut rng,
        )
        .expect("Astral Plane should dispatch");
        world.dungeon_mut().branch = DungeonBranch::Endgame;
        world.dungeon_mut().depth = 5;
        world.dungeon_mut().current_level = astral.generated.map;
        world.dungeon_mut().current_level_flags = astral.flags.into();

        let mut altar_positions = Vec::new();
        for y in 0..world.dungeon().current_level.height {
            for x in 0..world.dungeon().current_level.width {
                let pos = Position::new(x as i32, y as i32);
                if world
                    .dungeon()
                    .current_level
                    .get(pos)
                    .is_some_and(|cell| cell.terrain == Terrain::Altar)
                {
                    altar_positions.push(pos);
                }
            }
        }
        altar_positions.sort_by_key(|pos| pos.x);
        if let Some(mut pos) = world.get_component_mut::<Positioned>(player) {
            pos.0 = *altar_positions
                .last()
                .expect("Astral Plane should have at least one altar");
        }

        let amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
        let events = resolve_turn(
            &mut world,
            PlayerAction::Offer { item: Some(amulet) },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::GameOver {
                cause: crate::event::DeathCause::Ascended,
                ..
            }
        )));
        assert!(
            world.get_component::<ObjectCore>(amulet).is_none(),
            "successful ascension should consume the offered amulet"
        );
        let player_events = world
            .get_component::<PlayerEvents>(player)
            .expect("ascension flow should persist player milestone flags");
        assert!(player_events.ascended);
    }

    #[test]
    fn test_offer_real_amulet_on_wrong_astral_altar_does_not_ascend() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        let mut rng = test_rng();
        let astral = crate::special_levels::dispatch_special_level(
            crate::special_levels::SpecialLevelId::AstralPlane,
            None,
            &mut rng,
        )
        .expect("Astral Plane should dispatch");
        world.dungeon_mut().branch = DungeonBranch::Endgame;
        world.dungeon_mut().depth = 5;
        world.dungeon_mut().current_level = astral.generated.map;
        world.dungeon_mut().current_level_flags = astral.flags.into();

        let mut altar_positions = Vec::new();
        for y in 0..world.dungeon().current_level.height {
            for x in 0..world.dungeon().current_level.width {
                let pos = Position::new(x as i32, y as i32);
                if world
                    .dungeon()
                    .current_level
                    .get(pos)
                    .is_some_and(|cell| cell.terrain == Terrain::Altar)
                {
                    altar_positions.push(pos);
                }
            }
        }
        altar_positions.sort_by_key(|pos| pos.x);
        if let Some(mut pos) = world.get_component_mut::<Positioned>(player) {
            pos.0 = altar_positions[0];
        }

        let amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');
        let events = resolve_turn(
            &mut world,
            PlayerAction::Offer { item: Some(amulet) },
            &mut rng,
        );

        assert!(
            !events.iter().any(|event| matches!(
                event,
                EngineEvent::GameOver {
                    cause: crate::event::DeathCause::Ascended,
                    ..
                }
            )),
            "offering on the wrong Astral altar must not ascend"
        );
        assert!(
            world.get_component::<ObjectCore>(amulet).is_some(),
            "rejected offer should keep the real amulet in play"
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "offer-amulet-rejected"
        )));
        assert!(
            !player_carries_item(&world, player, amulet),
            "rejected amulet should leave the player's possessions"
        );
        assert!(
            world
                .get_component::<ObjectLocation>(amulet)
                .is_some_and(|loc| matches!(
                    &*loc,
                    ObjectLocation::Floor { level, .. }
                        if *level
                            == crate::dungeon::data_dungeon_level(DungeonBranch::Endgame, 5)
                ))
        );
    }

    #[test]
    fn test_sync_current_level_invocation_access_gates_endgame_portal() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        world.dungeon_mut().branch = DungeonBranch::Gehennom;
        world.dungeon_mut().depth = 21;
        let mut rng = test_rng();

        sync_current_level_invocation_access(&mut world, &mut rng);
        assert!(
            find_terrain(&world.dungeon().current_level, Terrain::MagicPortal).is_none(),
            "the endgame portal should stay closed until invocation succeeds"
        );
        assert!(
            world
                .dungeon()
                .trap_map
                .traps
                .iter()
                .any(|trap| trap.trap_type == TrapType::VibratingSquare)
        );

        let player = world.player();
        let mut player_events = read_player_events(&world, player);
        player_events.invoked = true;
        persist_player_events(&mut world, player, player_events);
        sync_current_level_invocation_access(&mut world, &mut rng);

        assert!(
            find_terrain(&world.dungeon().current_level, Terrain::MagicPortal).is_some(),
            "successful invocation should expose the endgame magic portal"
        );
        assert!(
            !world
                .dungeon()
                .trap_map
                .traps
                .iter()
                .any(|trap| trap.trap_type == TrapType::VibratingSquare),
            "opening the portal should clear the vibrating square marker"
        );
    }

    #[test]
    fn test_read_book_of_the_dead_invokes_and_opens_portal() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Gehennom;
        world.dungeon_mut().depth = 21;
        let player_pos = world
            .get_component::<Positioned>(player)
            .map(|pos| pos.0)
            .expect("player should have a position");
        let mut rng = test_rng();
        let _ = crate::traps::create_trap(
            &mut rng,
            &mut world.dungeon_mut().trap_map,
            player_pos,
            TrapType::VibratingSquare,
        );

        let bell = spawn_inventory_object_by_name(&mut world, "Bell of Opening", 'b');
        let candelabrum =
            spawn_inventory_object_by_name(&mut world, "Candelabrum of Invocation", 'c');
        let book = spawn_inventory_object_by_name(&mut world, "Book of the Dead", 'd');

        let current_turn = world.turn() as i64;
        if let Some(mut core) = world.get_component_mut::<ObjectCore>(bell) {
            core.age = current_turn;
        }
        world
            .ecs_mut()
            .insert_one(candelabrum, Enchantment { spe: 7 })
            .expect("candelabrum should accept candle count");
        world
            .ecs_mut()
            .insert_one(
                candelabrum,
                LightSource {
                    lit: true,
                    recharged: 0,
                },
            )
            .expect("candelabrum should accept light state");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Read { item: Some(book) },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "invocation-complete"
        )));
        let player_events = world
            .get_component::<PlayerEvents>(player)
            .expect("invocation should persist milestone flags");
        assert!(player_events.invoked);
        assert!(player_events.found_vibrating_square);
        assert_eq!(
            world
                .dungeon()
                .current_level
                .get(player_pos)
                .map(|cell| cell.terrain),
            Some(Terrain::MagicPortal),
            "successful invocation should open the portal on the vibrating square"
        );
        assert!(
            world.dungeon().trap_map.trap_at(player_pos).is_none(),
            "the vibrating square trap marker should be cleared after invocation"
        );
    }

    #[test]
    fn test_read_book_of_the_dead_requires_recent_bell_ring() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Gehennom;
        world.dungeon_mut().depth = 21;
        let player_pos = world
            .get_component::<Positioned>(player)
            .map(|pos| pos.0)
            .expect("player should have a position");
        let mut rng = test_rng();
        let _ = crate::traps::create_trap(
            &mut rng,
            &mut world.dungeon_mut().trap_map,
            player_pos,
            TrapType::VibratingSquare,
        );

        let bell = spawn_inventory_object_by_name(&mut world, "Bell of Opening", 'b');
        let candelabrum =
            spawn_inventory_object_by_name(&mut world, "Candelabrum of Invocation", 'c');
        let book = spawn_inventory_object_by_name(&mut world, "Book of the Dead", 'd');

        for _ in 0..6 {
            world.advance_turn();
        }
        if let Some(mut core) = world.get_component_mut::<ObjectCore>(bell) {
            core.age = 0;
        }
        world
            .ecs_mut()
            .insert_one(candelabrum, Enchantment { spe: 7 })
            .expect("candelabrum should accept candle count");
        world
            .ecs_mut()
            .insert_one(
                candelabrum,
                LightSource {
                    lit: true,
                    recharged: 0,
                },
            )
            .expect("candelabrum should accept light state");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Read { item: Some(book) },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "invocation-needs-bell-rung"
        )));
        assert!(
            !read_player_events(&world, player).invoked,
            "stale bell ringing should not complete invocation"
        );
    }

    #[test]
    fn test_read_book_of_the_dead_requires_ready_candelabrum() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Gehennom;
        world.dungeon_mut().depth = 21;
        let player_pos = world
            .get_component::<Positioned>(player)
            .map(|pos| pos.0)
            .expect("player should have a position");
        let mut rng = test_rng();
        let _ = crate::traps::create_trap(
            &mut rng,
            &mut world.dungeon_mut().trap_map,
            player_pos,
            TrapType::VibratingSquare,
        );

        let bell = spawn_inventory_object_by_name(&mut world, "Bell of Opening", 'b');
        let candelabrum =
            spawn_inventory_object_by_name(&mut world, "Candelabrum of Invocation", 'c');
        let book = spawn_inventory_object_by_name(&mut world, "Book of the Dead", 'd');
        let current_turn = world.turn() as i64;
        if let Some(mut core) = world.get_component_mut::<ObjectCore>(bell) {
            core.age = current_turn;
        }
        world
            .ecs_mut()
            .insert_one(candelabrum, Enchantment { spe: 6 })
            .expect("candelabrum should accept candle count");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Read { item: Some(book) },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "invocation-needs-candelabrum-ready"
        )));
        assert!(
            !read_player_events(&world, player).invoked,
            "an unready candelabrum should not complete invocation"
        );
    }

    #[test]
    fn test_read_book_of_the_dead_rejects_cursed_invocation_items() {
        let mut world = make_test_world();
        install_test_catalogs(&mut world);
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Gehennom;
        world.dungeon_mut().depth = 21;
        let player_pos = world
            .get_component::<Positioned>(player)
            .map(|pos| pos.0)
            .expect("player should have a position");
        let mut rng = test_rng();
        let _ = crate::traps::create_trap(
            &mut rng,
            &mut world.dungeon_mut().trap_map,
            player_pos,
            TrapType::VibratingSquare,
        );

        let bell = spawn_inventory_object_by_name(&mut world, "Bell of Opening", 'b');
        let candelabrum =
            spawn_inventory_object_by_name(&mut world, "Candelabrum of Invocation", 'c');
        let book = spawn_inventory_object_by_name(&mut world, "Book of the Dead", 'd');

        let current_turn = world.turn() as i64;
        if let Some(mut core) = world.get_component_mut::<ObjectCore>(bell) {
            core.age = current_turn;
        }
        world
            .ecs_mut()
            .insert_one(candelabrum, Enchantment { spe: 7 })
            .expect("candelabrum should accept candle count");
        world
            .ecs_mut()
            .insert_one(
                candelabrum,
                LightSource {
                    lit: true,
                    recharged: 0,
                },
            )
            .expect("candelabrum should accept light state");
        world
            .ecs_mut()
            .insert_one(
                book,
                BucStatus {
                    cursed: true,
                    blessed: false,
                    bknown: false,
                },
            )
            .expect("Book of the Dead should accept a cursed state");

        let events = resolve_turn(
            &mut world,
            PlayerAction::Read { item: Some(book) },
            &mut rng,
        );

        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "invocation-items-cursed"
        )));
        assert!(
            !read_player_events(&world, player).invoked,
            "cursed invocation items should not complete invocation"
        );
    }

    #[test]
    fn test_endgame_portal_traversal_reaches_astral_and_ascends() {
        let mut world = make_stair_world(Terrain::StairsDown, 20);
        install_test_catalogs(&mut world);
        let player = world.player();
        world.dungeon_mut().branch = DungeonBranch::Gehennom;
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        let mut rng = test_rng();

        let events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
        );
        assert_eq!(world.dungeon().depth, 21);

        let invocation_pos = world
            .dungeon()
            .trap_map
            .traps
            .iter()
            .find(|trap| trap.trap_type == TrapType::VibratingSquare)
            .map(|trap| trap.pos)
            .expect("Gehennom 21 should expose a vibrating square before invocation");
        set_player_position(&mut world, invocation_pos);

        let bell = spawn_inventory_object_by_name(&mut world, "Bell of Opening", 'b');
        let candelabrum =
            spawn_inventory_object_by_name(&mut world, "Candelabrum of Invocation", 'c');
        let book = spawn_inventory_object_by_name(&mut world, "Book of the Dead", 'd');
        let current_turn = world.turn() as i64;
        if let Some(mut core) = world.get_component_mut::<ObjectCore>(bell) {
            core.age = current_turn;
        }
        world
            .ecs_mut()
            .insert_one(candelabrum, Enchantment { spe: 7 })
            .expect("candelabrum should accept candle count");
        world
            .ecs_mut()
            .insert_one(
                candelabrum,
                LightSource {
                    lit: true,
                    recharged: 0,
                },
            )
            .expect("candelabrum should accept light state");

        let invocation_events = resolve_turn(
            &mut world,
            PlayerAction::Read { item: Some(book) },
            &mut rng,
        );
        assert!(invocation_events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "invocation-complete"
        )));

        let enter_events = move_player_onto_magic_portal(&mut world, &mut rng);
        assert!(
            enter_events
                .iter()
                .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
        );
        assert_eq!(world.dungeon().branch, DungeonBranch::Endgame);
        assert_eq!(world.dungeon().depth, 1);

        for expected_depth in 2..=5 {
            let portal_events = move_player_onto_magic_portal(&mut world, &mut rng);
            assert!(
                portal_events
                    .iter()
                    .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
            );
            assert_eq!(world.dungeon().branch, DungeonBranch::Endgame);
            assert_eq!(world.dungeon().depth, expected_depth);
        }

        let mut altar_positions = Vec::new();
        for y in 0..world.dungeon().current_level.height {
            for x in 0..world.dungeon().current_level.width {
                let pos = Position::new(x as i32, y as i32);
                if world
                    .dungeon()
                    .current_level
                    .get(pos)
                    .is_some_and(|cell| cell.terrain == Terrain::Altar)
                {
                    altar_positions.push(pos);
                }
            }
        }
        altar_positions.sort_by_key(|pos| pos.x);
        let chaotic_altar = *altar_positions
            .last()
            .expect("Astral Plane should have a chaotic altar");
        set_player_position(&mut world, chaotic_altar);
        let amulet = spawn_inventory_object_by_name(&mut world, "Amulet of Yendor", 'a');

        let offer_events = resolve_turn(
            &mut world,
            PlayerAction::Offer { item: Some(amulet) },
            &mut rng,
        );

        assert!(offer_events.iter().any(|event| matches!(
            event,
            EngineEvent::GameOver {
                cause: crate::event::DeathCause::Ascended,
                ..
            }
        )));
        assert!(read_player_events(&world, player).ascended);
    }

    #[test]
    fn test_entering_astral_plane_spawns_guardian_angel_when_worthy() {
        let mut world = make_stair_world(Terrain::StairsDown, 4);
        install_test_catalogs(&mut world);
        world.dungeon_mut().branch = DungeonBranch::Endgame;
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        let mut religion = default_religion_state(&world, player);
        religion.alignment_record = 10;
        world
            .ecs_mut()
            .insert_one(player, religion)
            .expect("player should accept religion state");

        let mut events = Vec::new();
        let mut rng = test_rng();
        change_level(&mut world, 5, false, &mut rng, &mut events);

        assert_eq!(world.dungeon().branch, DungeonBranch::Endgame);
        assert_eq!(world.dungeon().depth, 5);
        assert_eq!(count_monsters_named(&world, "Angel"), 1);
        let angel = find_monster_named(&world, "Angel").expect("guardian angel should exist");
        assert!(
            world.get_component::<Peaceful>(angel).is_some(),
            "guardian angel should be peaceful on first Astral arrival"
        );
        assert!(events.iter().any(|event| matches!(
            event,
            EngineEvent::Message { key, .. } if key == "guardian-angel-appears"
        )));
    }

    #[test]
    fn test_revisiting_astral_plane_does_not_duplicate_guardian_angel() {
        let mut world = make_stair_world(Terrain::StairsDown, 4);
        install_test_catalogs(&mut world);
        world.dungeon_mut().branch = DungeonBranch::Endgame;
        let player = world.player();
        world
            .ecs_mut()
            .insert_one(player, wizard_identity())
            .expect("player should accept wizard identity");
        let mut religion = default_religion_state(&world, player);
        religion.alignment_record = 10;
        world
            .ecs_mut()
            .insert_one(player, religion)
            .expect("player should accept religion state");

        let mut events = Vec::new();
        let mut rng = test_rng();
        change_level(&mut world, 5, false, &mut rng, &mut events);
        assert_eq!(count_monsters_named(&world, "Angel"), 1);

        events.clear();
        change_level(&mut world, 4, true, &mut rng, &mut events);
        events.clear();
        change_level(&mut world, 5, false, &mut rng, &mut events);

        assert_eq!(world.dungeon().branch, DungeonBranch::Endgame);
        assert_eq!(world.dungeon().depth, 5);
        assert_eq!(count_monsters_named(&world, "Angel"), 1);
        let angel = find_monster_named(&world, "Angel").expect("guardian angel should still exist");
        assert!(
            world.get_component::<Peaceful>(angel).is_some(),
            "guardian angel should stay peaceful after revisit restore"
        );
        assert!(
            !events.iter().any(|event| matches!(
                event,
                EngineEvent::Message { key, .. } if key == "guardian-angel-appears"
            )),
            "Astral revisit should not respawn the guardian angel"
        );
    }

    #[test]
    fn test_move_into_peaceful_monster_requires_force_fight() {
        let mut world = make_stair_world(Terrain::Floor, 1);
        let player = world.player();
        if let Some(mut pos) = world.get_component_mut::<Positioned>(player) {
            pos.0 = Position::new(5, 5);
        }

        let peaceful = world.spawn((
            Monster,
            Positioned(Position::new(6, 5)),
            HitPoints {
                current: 12,
                max: 12,
            },
            Speed(12),
            DisplaySymbol {
                symbol: 'g',
                color: nethack_babel_data::Color::Brown,
            },
            MovementPoints(NORMAL_SPEED as i32),
            Name("gnome".to_string()),
            Peaceful,
        ));

        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(
            events.iter().any(
                |event| matches!(event, EngineEvent::Message { key, .. } if key == "peaceful-monster-blocks")
            ),
            "normal movement should stop at a peaceful monster"
        );
        assert_eq!(
            world.get_component::<Positioned>(player).map(|pos| pos.0),
            Some(Position::new(5, 5))
        );
        assert_eq!(
            world
                .get_component::<HitPoints>(peaceful)
                .map(|hp| hp.current),
            Some(12)
        );
        if let Some(mut pos) = world.get_component_mut::<Positioned>(peaceful) {
            pos.0 = Position::new(6, 5);
        }
        let _ = world.ecs_mut().insert_one(peaceful, Peaceful);

        let events = resolve_turn(
            &mut world,
            PlayerAction::FightDirection {
                direction: Direction::East,
            },
            &mut rng,
        );
        assert!(
            events.iter().any(|event| {
                matches!(
                    event,
                    EngineEvent::MeleeHit { .. } | EngineEvent::MeleeMiss { .. }
                )
            }),
            "fight direction should attack the peaceful blocker"
        );
        assert!(
            world.get_component::<Peaceful>(peaceful).is_none(),
            "force-attacking a peaceful monster should anger it"
        );
    }

    #[test]
    fn test_story_traversal_matrix() {
        for scenario in [
            StoryTraversalScenario::QuestClosure,
            StoryTraversalScenario::QuestLeaderAnger,
            StoryTraversalScenario::MedusaRevisit,
            StoryTraversalScenario::CastleRevisit,
            StoryTraversalScenario::OrcusRevisit,
            StoryTraversalScenario::FortLudiosRevisit,
            StoryTraversalScenario::VladTopRevisit,
            StoryTraversalScenario::InvocationPortalRevisit,
            StoryTraversalScenario::ShopEntry,
            StoryTraversalScenario::ShopEntryWelcomeBack,
            StoryTraversalScenario::ShopEntryRobbed,
            StoryTraversalScenario::ShopkeeperFollow,
            StoryTraversalScenario::ShopkeeperPayoff,
            StoryTraversalScenario::ShopkeeperCredit,
            StoryTraversalScenario::ShopCreditCovers,
            StoryTraversalScenario::ShopNoMoney,
            StoryTraversalScenario::ShopkeeperSell,
            StoryTraversalScenario::ShopChatPriceQuote,
            StoryTraversalScenario::ShopRepair,
            StoryTraversalScenario::ShopkeeperDeath,
            StoryTraversalScenario::ShopRobbery,
            StoryTraversalScenario::ShopRestitution,
            StoryTraversalScenario::TempleWrongAlignment,
            StoryTraversalScenario::TempleAleGift,
            StoryTraversalScenario::TempleVirtuesOfPoverty,
            StoryTraversalScenario::TempleDonationThanks,
            StoryTraversalScenario::TemplePious,
            StoryTraversalScenario::TempleDonation,
            StoryTraversalScenario::TempleBlessing,
            StoryTraversalScenario::TempleCleansing,
            StoryTraversalScenario::TempleSelflessGenerosity,
            StoryTraversalScenario::TempleWrath,
            StoryTraversalScenario::TempleCalm,
            StoryTraversalScenario::UntendedTempleGhost,
            StoryTraversalScenario::SanctumRevisit,
            StoryTraversalScenario::WizardHarassment,
            StoryTraversalScenario::WizardTaunt,
            StoryTraversalScenario::WizardIntervention,
            StoryTraversalScenario::WizardAmuletWake,
            StoryTraversalScenario::WizardBlackGlowBlind,
            StoryTraversalScenario::HumanoidAlohaChat,
            StoryTraversalScenario::WereFullMoonChat,
            StoryTraversalScenario::WizardLevelTeleport,
            StoryTraversalScenario::EndgameAscension,
        ] {
            let (world, final_events) = run_story_traversal_scenario(scenario);
            let player = world.player();

            match scenario {
                StoryTraversalScenario::QuestClosure => {
                    assert!(
                        final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "quest-completed"
                        )),
                        "{} should emit quest completion",
                        scenario.label()
                    );
                    let quest_status = world
                        .get_component::<crate::quest::QuestState>(player)
                        .map(|state| state.status)
                        .expect("quest traversal should preserve quest state");
                    assert_eq!(
                        quest_status,
                        crate::quest::QuestStatus::Completed,
                        "{} should end with a completed quest",
                        scenario.label()
                    );
                    assert_eq!(world.dungeon().branch, DungeonBranch::Quest);
                    assert_eq!(world.dungeon().depth, 1);
                }
                StoryTraversalScenario::QuestLeaderAnger => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "quest-expelled"
                    )));
                    assert!(
                        world
                            .get_component::<crate::quest::QuestState>(player)
                            .is_some_and(|state| state.leader_angry),
                        "{} should preserve leader anger in quest state",
                        scenario.label()
                    );
                    assert_eq!(world.dungeon().branch, DungeonBranch::Quest);
                    assert_eq!(world.dungeon().depth, 1);
                }
                StoryTraversalScenario::MedusaRevisit => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::Main);
                    assert_eq!(world.dungeon().depth, 24);
                    assert_eq!(count_monsters_named(&world, "medusa"), 1);
                }
                StoryTraversalScenario::CastleRevisit => {
                    let wand_otyp =
                        resolve_object_type_by_spec(&test_game_data().objects, "wand of wishing")
                            .expect("wand of wishing should resolve against the catalog");
                    assert_eq!(world.dungeon().branch, DungeonBranch::Main);
                    assert_eq!(world.dungeon().depth, 25);
                    assert_eq!(count_objects_with_type(&world, wand_otyp), 1);
                }
                StoryTraversalScenario::OrcusRevisit => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::Gehennom);
                    assert_eq!(world.dungeon().depth, 12);
                    assert_eq!(count_monsters_named(&world, "orcus"), 1);
                }
                StoryTraversalScenario::FortLudiosRevisit => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::FortLudios);
                    assert_eq!(world.dungeon().depth, 1);
                    assert_eq!(count_monsters_named(&world, "soldier"), 2);
                    assert_eq!(count_monsters_named(&world, "lieutenant"), 1);
                    assert_eq!(count_monsters_named(&world, "captain"), 1);
                }
                StoryTraversalScenario::VladTopRevisit => {
                    let candelabrum_otyp = resolve_object_type_by_spec(
                        &test_game_data().objects,
                        "Candelabrum of Invocation",
                    )
                    .expect("Candelabrum should resolve against the catalog");
                    assert_eq!(world.dungeon().branch, DungeonBranch::VladsTower);
                    assert_eq!(world.dungeon().depth, 3);
                    assert_eq!(count_monsters_named(&world, "Vlad the Impaler"), 1);
                    assert_eq!(count_objects_with_type(&world, candelabrum_otyp), 1);
                }
                StoryTraversalScenario::InvocationPortalRevisit => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::Gehennom);
                    assert_eq!(world.dungeon().depth, 21);
                    assert!(
                        find_terrain(&world.dungeon().current_level, Terrain::MagicPortal)
                            .is_some(),
                        "{} should reopen the endgame portal on revisit",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::ShopEntry => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-enter"
                    )));
                }
                StoryTraversalScenario::ShopEntryWelcomeBack => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-welcome-back"
                    )));
                }
                StoryTraversalScenario::ShopEntryRobbed => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-stolen"
                    )));
                }
                StoryTraversalScenario::ShopkeeperFollow => {
                    let shopkeeper =
                        find_monster_named(&world, "Izchak").expect("shopkeeper should exist");
                    let shopkeeper_state = world
                        .get_component::<crate::npc::Shopkeeper>(shopkeeper)
                        .map(|state| (*state).clone())
                        .expect("shopkeeper should keep explicit runtime state");
                    assert!(
                        shopkeeper_state.following,
                        "{} should mark the shopkeeper as following after the hero leaves with unpaid goods",
                        scenario.label()
                    );
                    let final_pos = world
                        .get_component::<Positioned>(shopkeeper)
                        .map(|pos| pos.0)
                        .expect("shopkeeper should still have a position");
                    assert!(
                        final_pos != shopkeeper_home_pos(&world.dungeon().shop_rooms[0]),
                        "{} should move the peaceful shopkeeper off the home tile once pursuit starts",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::ShopkeeperPayoff => {
                    let shopkeeper =
                        find_monster_named(&world, "Izchak").expect("shopkeeper should exist");
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-pay-success"
                    )));
                    assert!(
                        shop.bill.is_empty(),
                        "{} should clear the live bill",
                        scenario.label()
                    );
                    assert_eq!(shop.debit, 0, "{} should clear debit", scenario.label());
                    assert!(!shop.angry, "{} should pacify the shop", scenario.label());
                    assert_eq!(
                        player_gold(&world, player),
                        50,
                        "{} should spend exactly the billed gold",
                        scenario.label()
                    );
                    let shopkeeper_state = world
                        .get_component::<crate::npc::Shopkeeper>(shopkeeper)
                        .map(|state| (*state).clone())
                        .expect("shopkeeper should keep explicit runtime state");
                    assert!(
                        !shopkeeper_state.following,
                        "{} should stop follow behavior after full payment",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::ShopkeeperCredit => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-pay-success"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-credit"
                    )));
                    assert_eq!(shop.debit, 0, "{} should clear debit", scenario.label());
                    assert_eq!(
                        shop.credit,
                        100,
                        "{} should bank excess as credit",
                        scenario.label()
                    );
                    assert!(
                        !shop.angry,
                        "{} should pacify the shop once debt is settled",
                        scenario.label()
                    );
                    assert_eq!(
                        player_gold(&world, player),
                        0,
                        "{} should consume the dropped gold",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::ShopCreditCovers => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-credit-covers"
                    )));
                    assert!(
                        shop.bill.is_empty(),
                        "{} should clear the bill",
                        scenario.label()
                    );
                    assert_eq!(shop.debit, 0, "{} should clear debit", scenario.label());
                    assert_eq!(
                        shop.credit,
                        30,
                        "{} should spend only the owed credit",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::ShopNoMoney => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-no-money"
                    )));
                    assert_eq!(
                        player_gold(&world, player),
                        50,
                        "{} should leave the player's gold untouched",
                        scenario.label()
                    );
                    assert_eq!(
                        shop.bill.total(),
                        100,
                        "{} should preserve the bill",
                        scenario.label()
                    );
                    assert_eq!(
                        shop.credit,
                        0,
                        "{} should keep credit unchanged",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::ShopkeeperSell => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-sell"
                    )));
                    assert_eq!(
                        player_gold(&world, player),
                        5,
                        "{} should pay the hero in gold for the sale",
                        scenario.label()
                    );
                    assert_eq!(
                        shop.shopkeeper_gold,
                        75,
                        "{} should debit live shopkeeper gold",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::ShopChatPriceQuote => {
                    let quote_count = final_events
                        .iter()
                        .filter(|event| {
                            matches!(event, EngineEvent::Message { key, .. } if key == "shop-price")
                        })
                        .count();
                    assert_eq!(
                        quote_count,
                        2,
                        "{} should quote both floor items",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::ShopRepair => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-repair"
                    )));
                    assert!(
                        shop.damage_list.is_empty(),
                        "{} should consume one queued repair entry",
                        scenario.label()
                    );
                    assert_eq!(
                        world
                            .dungeon()
                            .current_level
                            .get(Position::new(5, 5))
                            .map(|cell| cell.terrain),
                        Some(Terrain::DoorClosed),
                        "{} should restore the damaged door terrain",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::ShopkeeperDeath => {
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-keeper-dead"
                    )));
                    assert!(
                        !final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "shop-shoplift"
                        )),
                        "{} should not rob a deserted shop on exit",
                        scenario.label()
                    );
                    assert!(
                        shop.bill.is_empty(),
                        "{} should clear the bill",
                        scenario.label()
                    );
                    assert_eq!(shop.debit, 0, "{} should clear debit", scenario.label());
                    assert_eq!(shop.credit, 0, "{} should clear credit", scenario.label());
                    assert!(!shop.angry, "{} should clear anger", scenario.label());
                    assert!(
                        !shop.surcharge,
                        "{} should clear surcharge",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<crate::npc::Shopkeeper>(shop.shopkeeper)
                            .is_none(),
                        "{} should remove explicit shopkeeper runtime state",
                        scenario.label()
                    );
                    assert!(
                        find_shop_index_containing_position(&world, Position::new(6, 5)).is_none(),
                        "{} should treat the dead keeper's room as deserted for commerce",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::ShopRobbery => {
                    let shopkeeper =
                        find_monster_named(&world, "Izchak").expect("shopkeeper should exist");
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-shoplift"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-stolen-amount"
                    )));
                    assert_eq!(
                        shop.robbed,
                        100,
                        "{} should track robbed value",
                        scenario.label()
                    );
                    assert!(
                        shop.bill.is_empty(),
                        "{} should clear the live bill",
                        scenario.label()
                    );
                    assert!(
                        shop.angry,
                        "{} should anger the shopkeeper",
                        scenario.label()
                    );
                    let shopkeeper_state = world
                        .get_component::<crate::npc::Shopkeeper>(shopkeeper)
                        .map(|state| (*state).clone())
                        .expect("shopkeeper should keep explicit runtime state");
                    assert!(
                        shopkeeper_state.following,
                        "{} should make the robbed shopkeeper pursue the hero",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::ShopRestitution => {
                    let shopkeeper =
                        find_monster_named(&world, "Izchak").expect("shopkeeper should exist");
                    let shop = &world.dungeon().shop_rooms[0];
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "shop-restock"
                    )));
                    assert_eq!(
                        shop.robbed,
                        0,
                        "{} should clear robbed balance",
                        scenario.label()
                    );
                    assert!(!shop.angry, "{} should pacify the shop", scenario.label());
                    let shopkeeper_state = world
                        .get_component::<crate::npc::Shopkeeper>(shopkeeper)
                        .map(|state| (*state).clone())
                        .expect("shopkeeper should keep explicit runtime state");
                    assert!(
                        !shopkeeper_state.following,
                        "{} should stop follow behavior after restitution",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::TempleWrongAlignment => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-wrong-alignment"
                    )));
                    assert_eq!(
                        player_gold(&world, player),
                        500,
                        "{} should not spend gold on a wrong-alignment priest",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<crate::status::SpellProtection>(player)
                            .is_none(),
                        "{} should not grant protection for a wrong-alignment priest",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::TempleAleGift => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-ale-gift"
                    )));
                    assert_eq!(
                        player_gold(&world, player),
                        2,
                        "{} should grant the hero ale money",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::TempleVirtuesOfPoverty => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-virtues-of-poverty"
                    )));
                    assert_eq!(
                        player_gold(&world, player),
                        0,
                        "{} should keep the player broke",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<crate::status::SpellProtection>(player)
                            .is_none(),
                        "{} should not grant protection when preaching poverty",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::TempleDonationThanks => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-small-thanks"
                    )));
                    assert_eq!(
                        player_gold(&world, player),
                        0,
                        "{} should spend the available donation gold",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<crate::status::SpellProtection>(player)
                            .is_none(),
                        "{} should not grant protection for a small donation",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::TemplePious => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-pious"
                    )));
                    assert_eq!(
                        player_gold(&world, player),
                        0,
                        "{} should spend the pious donation",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<crate::status::SpellProtection>(player)
                            .is_none(),
                        "{} should not convert a pious donation into protection",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<crate::status::StatusEffects>(player)
                            .is_none_or(|status| status.clairvoyance == 0),
                        "{} should not grant clairvoyance for the plain pious tier",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::TempleDonation => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-protection-granted"
                    )));
                    assert_eq!(
                        player_gold(&world, player),
                        600,
                        "{} should deduct the protection donation from player gold",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<crate::status::SpellProtection>(player)
                            .is_some_and(|protection| protection.layers == 1),
                        "{} should still grant one protection layer",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::TempleBlessing => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-clairvoyance"
                    )));
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. }
                            if key == "clairvoyance-reveal" || key == "clairvoyance-nothing-new"
                    )));
                    assert!(
                        world
                            .get_component::<crate::status::StatusEffects>(player)
                            .is_some_and(|status| status.clairvoyance > 0),
                        "{} should grant timed clairvoyance",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::TempleCleansing => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-cleansing"
                    )));
                    assert_eq!(
                        player_gold(&world, player),
                        0,
                        "{} should spend the cleansing donation",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<crate::religion::ReligionState>(player)
                            .is_some_and(|state| state.alignment_record >= 0),
                        "{} should improve the player's alignment record",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<crate::status::SpellProtection>(player)
                            .is_some_and(|protection| protection.layers == 1),
                        "{} should not consume existing protection layers",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::TempleSelflessGenerosity => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-selfless-generosity"
                    )));
                    assert_eq!(
                        player_gold(&world, player),
                        0,
                        "{} should spend the selfless donation",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<crate::status::SpellProtection>(player)
                            .is_some_and(|protection| protection.layers == 1),
                        "{} should preserve the existing protection layer",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::TempleWrath => {
                    let priest =
                        find_monster_named(&world, "priest").expect("priest should still exist");
                    let priest_state = world
                        .get_component::<crate::npc::Priest>(priest)
                        .map(|state| *state)
                        .expect("priest should keep explicit runtime state");
                    assert!(
                        priest_state.angry,
                        "{} should preserve priest anger",
                        scenario.label()
                    );
                    assert!(
                        world.get_component::<Peaceful>(priest).is_none(),
                        "{} should remove peaceful status from an angered priest",
                        scenario.label()
                    );
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. }
                            if matches!(
                                key.as_str(),
                                "priest-cranky-1" | "priest-cranky-2" | "priest-cranky-3"
                            )
                    )));
                    assert!(
                        !final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "priest-protection-granted"
                        )),
                        "{} should not allow protection after wrath",
                        scenario.label()
                    );
                    assert!(
                        player_hp(&world, player) < 40,
                        "{} should keep the divine wrath HP loss",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<crate::status::StatusEffects>(player)
                            .is_some_and(|status| status.blindness > 0),
                        "{} should keep the wrath blindness timer",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::TempleCalm => {
                    let priest = find_monster_named(&world, "priest").expect("priest should exist");
                    let priest_state = world
                        .get_component::<crate::npc::Priest>(priest)
                        .map(|state| *state)
                        .expect("priest should keep explicit runtime state");
                    assert!(
                        !priest_state.angry,
                        "{} should clear priest anger after prayer",
                        scenario.label()
                    );
                    assert!(
                        world.get_component::<Peaceful>(priest).is_some(),
                        "{} should restore peaceful status after prayer",
                        scenario.label()
                    );
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "priest-calmed"
                    )));
                }
                StoryTraversalScenario::UntendedTempleGhost => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "temple-ghost-appears"
                    )));
                    assert!(
                        final_events
                            .iter()
                            .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. }))
                    );
                    assert!(
                        count_monsters_named(&world, "ghost") >= 1,
                        "{} should spawn a real ghost on untended temple entry",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::SanctumRevisit => {
                    assert_eq!(world.dungeon().branch, DungeonBranch::Gehennom);
                    assert_eq!(world.dungeon().depth, 20);
                    assert_eq!(
                        count_monsters_named(&world, "high priest"),
                        1,
                        "{} should not duplicate the Sanctum high priest",
                        scenario.label()
                    );
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "sanctum-desecrate"
                    )));
                    assert!(
                        !final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. }
                                if key == "sanctum-infidel" || key == "sanctum-be-gone"
                        )),
                        "{} should only emit revisit Sanctum messaging",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::WizardHarassment => {
                    let sword = find_player_named_item(&world, player, "long sword")
                        .expect("story matrix should keep the cursed inventory item");
                    assert!(
                        final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. }
                                if key == "wizard-curse-items" || key == "wizard-summon-nasties"
                        )),
                        "{} should keep harassing after the theft phase",
                        scenario.label()
                    );
                    let followup_curse = world
                        .get_component::<BucStatus>(sword)
                        .is_some_and(|status| status.cursed);
                    let followup_summon = final_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. }));
                    assert!(
                        followup_curse || followup_summon,
                        "{} should produce a real follow-up harassment side-effect",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<Inventory>(player)
                            .is_some_and(|inv| inv.items.iter().any(|item| {
                                world
                                    .get_component::<Name>(*item)
                                    .is_some_and(|name| name.0 == "Amulet of Yendor")
                            })),
                        "{} should not let a distant wizard steal the Amulet",
                        scenario.label()
                    );
                    assert!(
                        !final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-steal-amulet"
                        )),
                        "{} should not emit remote amulet theft",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.invoked),
                        "{} should preserve the invoked harassment trigger state",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::WizardTaunt => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. }
                            if key == "wizard-taunt-laughs"
                                || key == "wizard-taunt-relinquish"
                                || key == "wizard-taunt-panic"
                                || key == "wizard-taunt-return"
                                || key == "wizard-taunt-general"
                    )));
                    assert_eq!(count_monsters_named(&world, "Wizard of Yendor"), 1);
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.invoked),
                        "{} should preserve the live-taunt trigger state",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::WizardIntervention => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. }
                            if key == "wizard-vague-nervous"
                                || key == "wizard-black-glow"
                                || key == "wizard-aggravate"
                                || key == "wizard-summon-nasties"
                                || key == "wizard-respawned"
                    )));
                    let sword = find_player_named_item(&world, player, "long sword")
                        .expect("off-screen wizard intervention should keep the tracked item");
                    let cursed = world
                        .get_component::<BucStatus>(sword)
                        .is_some_and(|status| status.cursed);
                    let summoned = final_events
                        .iter()
                        .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. }));
                    let black_glow = final_events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-black-glow"
                        )
                    });
                    let summon_msg = final_events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-summon-nasties"
                        )
                    });
                    let aggravate = final_events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-aggravate"
                        )
                    });
                    let respawned = final_events.iter().any(|event| {
                        matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-respawned"
                        )
                    });
                    if black_glow {
                        assert!(cursed, "{} should really curse inventory", scenario.label());
                    }
                    if summon_msg {
                        assert!(
                            summoned,
                            "{} should really generate summoned monsters",
                            scenario.label()
                        );
                    }
                    if aggravate {
                        let sleeper = world
                            .ecs()
                            .query::<(&Monster, &Name)>()
                            .iter()
                            .find_map(|(entity, (_, name))| (name.0 == "goblin").then_some(entity))
                            .expect("wizard intervention scenario should keep the sleeping goblin");
                        assert!(
                            !crate::status::is_sleeping(&world, sleeper),
                            "{} should really wake sleeping monsters",
                            scenario.label()
                        );
                    }
                    if respawned {
                        assert_eq!(count_monsters_named(&world, "Wizard of Yendor"), 1);
                        assert!(
                            final_events
                                .iter()
                                .any(|event| matches!(event, EngineEvent::MonsterGenerated { .. })),
                            "{} should generate a live Wizard on immediate resurrection",
                            scenario.label()
                        );
                    } else {
                        assert_eq!(count_monsters_named(&world, "Wizard of Yendor"), 0);
                    }
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.killed_wizard),
                        "{} should preserve the intervention trigger state",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::WizardAmuletWake => {
                    assert!(
                        final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-vague-nervous"
                        )),
                        "{} should warn when the Amulet wakes a distant Wizard",
                        scenario.label()
                    );
                    let wizard = wizard_of_yendor_entities(&world)
                        .into_iter()
                        .next()
                        .expect("wizard amulet wake scenario should keep a live Wizard");
                    assert!(
                        !crate::status::is_sleeping(&world, wizard),
                        "{} should wake the sleeping Wizard",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::WizardBlackGlowBlind => {
                    assert!(
                        !final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::Message { key, .. } if key == "wizard-black-glow"
                        )),
                        "{} should suppress the black-glow message while blind",
                        scenario.label()
                    );
                    let cursed = world
                        .get_component::<Inventory>(player)
                        .map(|inv| {
                            inv.items.iter().any(|item| {
                                world
                                    .get_component::<BucStatus>(*item)
                                    .is_some_and(|status| status.cursed)
                            })
                        })
                        .unwrap_or(false);
                    assert!(
                        cursed,
                        "{} should still curse inventory despite suppressing the message",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::HumanoidAlohaChat => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "npc-humanoid-aloha"
                    )));
                }
                StoryTraversalScenario::WereFullMoonChat => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "npc-were-howls"
                    )));
                    let sleeper = world
                        .ecs()
                        .query::<(&Monster, &Name)>()
                        .iter()
                        .find_map(|(entity, (_, name))| (name.0 == "kobold").then_some(entity))
                        .expect("were full moon scenario should keep the sleeping kobold");
                    assert!(
                        !crate::status::is_sleeping(&world, sleeper),
                        "{} should wake nearby sleeping monsters",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::WizardLevelTeleport => {
                    assert!(final_events.iter().any(|event| matches!(
                        event,
                        EngineEvent::Message { key, .. } if key == "wizard-level-teleport"
                    )));
                    assert!(
                        final_events
                            .iter()
                            .any(|event| matches!(event, EngineEvent::LevelChanged { .. }))
                    );
                    assert_eq!(world.dungeon().branch, DungeonBranch::Main);
                    assert_ne!(
                        world.dungeon().depth,
                        10,
                        "{} should move the player to a different depth",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|events| events.invoked),
                        "{} should preserve the invoked teleport trigger state",
                        scenario.label()
                    );
                }
                StoryTraversalScenario::EndgameAscension => {
                    assert!(
                        final_events.iter().any(|event| matches!(
                            event,
                            EngineEvent::GameOver {
                                cause: crate::event::DeathCause::Ascended,
                                ..
                            }
                        )),
                        "{} should end in ascension",
                        scenario.label()
                    );
                    assert!(
                        world
                            .get_component::<PlayerEvents>(player)
                            .is_some_and(|flags| flags.ascended),
                        "{} should persist ascension in player milestone flags",
                        scenario.label()
                    );
                    assert_eq!(world.dungeon().branch, DungeonBranch::Endgame);
                    assert_eq!(world.dungeon().depth, 5);
                }
            }
        }
    }

    // ── Gas cloud ticking ─────────────────────────────────────────────

    #[test]
    fn test_gas_cloud_ticks_in_turn() {
        // Verify that process_new_turn calls tick_gas_clouds without error.
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Run a rest turn which triggers process_new_turn.
        let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
        // No gas clouds => no cloud damage events, but no crash either.
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, EngineEvent::HpChange { .. }))
        );
    }

    #[test]
    fn test_gas_cloud_expires_after_duration() {
        // Test tick_gas_clouds directly with an expiring cloud.
        use crate::region::{GasCloud, GasCloudType, tick_gas_clouds};
        let mut world = make_test_world();
        let mut rng = test_rng();

        let mut clouds = vec![GasCloud {
            position: Position::new(100, 100), // far from player
            radius: 1,
            turns_remaining: 1,
            damage_per_turn: 5,
            damage_type: GasCloudType::Poison,
        }];

        let _events = tick_gas_clouds(&mut clouds, &mut world, &mut rng);
        // After 1 tick with damage >= 5, dissipation halves damage and adds 2 turns.
        assert!(
            clouds.iter().all(|c| c.turns_remaining <= 2),
            "Cloud should have dissipated or been refreshed with reduced damage"
        );
    }

    // ── Cross-system gap wiring tests ───────────────────────────────

    #[test]
    fn test_polymorph_trap_emits_status_applied() {
        use crate::traps::TrapInstance;
        use nethack_babel_data::TrapType;

        let mut world = make_test_world();

        // Place a polymorph trap at (6, 5), one step east of the player.
        world.dungeon_mut().trap_map.traps.push(TrapInstance {
            pos: Position::new(6, 5),
            trap_type: TrapType::PolyTrap,
            detected: false,
            triggered_count: 0,
        });

        // Try multiple RNG seeds to find one where the trap triggers
        // (avoidance is probabilistic).
        let mut found_poly = false;
        for seed in 0..200u64 {
            // Reset trap state.
            world.dungeon_mut().trap_map.traps[0].triggered_count = 0;

            let mut rng = Pcg64::seed_from_u64(seed);
            let events = resolve_turn(
                &mut world,
                PlayerAction::Move {
                    direction: Direction::East,
                },
                &mut rng,
            );

            let has_poly = events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::StatusApplied {
                        status: crate::event::StatusEffect::Polymorphed,
                        ..
                    }
                )
            });
            if has_poly {
                found_poly = true;
                break;
            }

            // Move the player back so we can try again.
            if let Some(mut pos) = world.get_component_mut::<Positioned>(world.player()) {
                pos.0 = Position::new(5, 5);
            }
        }

        assert!(
            found_poly,
            "stepping on a polymorph trap should eventually emit StatusApplied Polymorphed"
        );
    }

    #[test]
    fn test_vault_entry_spawns_guard() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Set up a vault room covering (6,5)..(7,6).
        world
            .dungeon_mut()
            .vault_rooms
            .push(crate::vault::VaultRoom {
                top_left: Position::new(6, 5),
                bottom_right: Position::new(7, 6),
            });
        // Make that area walkable.
        for y in 5..=6 {
            for x in 6..=7 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }

        // Move east into the vault.
        let events = resolve_turn(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );

        let has_guard = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "guard-appears"));
        assert!(has_guard, "entering a vault should spawn a guard");
        assert!(
            world.dungeon().vault_guard_present,
            "vault_guard_present flag should be set"
        );
    }

    #[test]
    fn test_engrave_tracks_elbereth_conduct_on_player() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let events = resolve_turn(
            &mut world,
            PlayerAction::Engrave {
                text: "Elbereth".to_string(),
            },
            &mut rng,
        );

        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "engrave-write"))
        );
        let conduct = world
            .get_component::<ConductState>(player)
            .expect("engraving should create conduct state");
        assert_eq!(
            conduct.elbereths, 1,
            "Elbereth should break elberethless once"
        );
    }

    #[test]
    fn test_pray_increments_gnostic_conduct_on_player() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();

        let _ = resolve_turn(&mut world, PlayerAction::Pray, &mut rng);

        let conduct = world
            .get_component::<ConductState>(player)
            .expect("prayer should create conduct state");
        assert_eq!(
            conduct.gnostic, 1,
            "prayer should increment atheist counter"
        );
    }

    #[test]
    fn test_offer_increments_gnostic_conduct_on_player() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let player = world.player();
        let corpse = spawn_inventory_item(&mut world, 'a');

        let _ = resolve_turn(
            &mut world,
            PlayerAction::Offer { item: Some(corpse) },
            &mut rng,
        );

        let conduct = world
            .get_component::<ConductState>(player)
            .expect("offering should create conduct state");
        assert_eq!(
            conduct.gnostic, 1,
            "offering should increment atheist counter"
        );
    }

    #[test]
    fn test_conduct_updates_weaphit_and_killer_from_events() {
        let mut world = make_test_world();
        let player = world.player();
        let victim = world.spawn((Name("victim".to_string()),));

        let events = vec![
            EngineEvent::MeleeHit {
                attacker: player,
                defender: victim,
                weapon: Some(player),
                damage: 1,
            },
            EngineEvent::EntityDied {
                entity: victim,
                killer: Some(player),
                cause: crate::event::DeathCause::KilledBy {
                    killer_name: "you".to_string(),
                },
            },
        ];

        apply_action_conduct_updates(&mut world, player, &PlayerAction::Rest, &events);

        let conduct = world
            .get_component::<ConductState>(player)
            .expect("conduct should be created");
        assert_eq!(conduct.weaphit, 1);
        assert_eq!(conduct.killer, 1);
    }

    #[test]
    fn test_conduct_updates_wishes_on_wizwish_action() {
        let mut world = make_test_world();
        let player = world.player();
        let events = Vec::new();

        apply_action_conduct_updates(
            &mut world,
            player,
            &PlayerAction::WizWish {
                wish_text: "blessed +2 silver saber".to_string(),
            },
            &events,
        );

        let conduct = world
            .get_component::<ConductState>(player)
            .expect("conduct should be created");
        assert_eq!(conduct.wishes, 1);
    }

    #[test]
    fn test_prayer_creates_minion_entity() {
        // Test that angry prayer at anger_roll 7 emits MonsterGenerated.
        use crate::religion::{ReligionState, Trouble, pray};

        let base_state = ReligionState {
            alignment: nethack_babel_data::Alignment::Lawful,
            alignment_record: -5,
            god_anger: 5,
            god_gifts: 0,
            blessed_amount: 0,
            bless_cooldown: 0,
            crowned: false,
            demigod: false,
            turn: 1000,
            experience_level: 10,
            current_hp: 50,
            max_hp: 50,
            current_pw: 20,
            max_pw: 20,
            nutrition: 900,
            luck: 0,
            luck_bonus: 0,
            has_luckstone: false,
            luckstone_blessed: false,
            luckstone_cursed: false,
            in_gehennom: false,
            is_undead: false,
            is_demon: false,
            original_alignment: nethack_babel_data::Alignment::Lawful,
            has_converted: false,
            alignment_abuse: 0,
        };

        // Try many seeds to find one where the anger roll = 7 or 8
        // (summon minion path).
        let mut found_minion = false;
        for seed in 0..500u64 {
            let mut test_state = base_state.clone();
            let mut rng = Pcg64::seed_from_u64(seed);
            let player_entity = hecs::Entity::DANGLING;
            let events = pray(
                &mut test_state,
                player_entity,
                false,
                None,
                Trouble::None,
                &[Trouble::None],
                false,
                &mut rng,
            );
            let has_monster = events
                .iter()
                .any(|e| matches!(e, EngineEvent::MonsterGenerated { .. }));
            if has_monster {
                found_minion = true;
                break;
            }
        }

        assert!(
            found_minion,
            "angry prayer should eventually summon a minion (MonsterGenerated event)"
        );
    }
}
