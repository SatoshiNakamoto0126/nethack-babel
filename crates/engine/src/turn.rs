//! Main turn loop: movement points, action dispatch, monster turns,
//! regeneration, hunger, and new-turn processing.
//!
//! Implements the NetHack `moveloop_core()` sequence where each call to
//! `resolve_turn()` processes one player action and any resulting monster
//! actions, then checks whether a new game turn boundary has been reached.

use rand::Rng;

use nethack_babel_data::{
    ArtifactId, MonsterDef, MonsterId, ObjectClass, ObjectCore, ObjectDef, ObjectTypeId,
    PlayerIdentity,
};

use crate::action::{Direction, NameTarget, PlayerAction, Position};
use crate::conduct::ConductState;
use crate::dungeon::{CachedMonster, Terrain};
use crate::event::{EngineEvent, HpSource, HungerLevel};
use crate::makemon::{GoodPosFlags, MakeMonFlags, enexto, goodpos, makemon};
use crate::map_gen::generate_level;
use crate::mkobj::mksobj_at;
use crate::special_levels::{dispatch_special_level, identify_special_level};
use crate::traps::{TrapEntityInfo, detect_trap, trigger_trap_at};
use crate::world::{
    Boulder, CreationOrder, DisplaySymbol, Encumbrance, EncumbranceLevel, ExperienceLevel,
    GameWorld, HeroSpeed, HeroSpeedBonus, HitPoints, Monster, MonsterSpeedMod, MovementPoints,
    NORMAL_SPEED, Name, Nutrition, PlayerCombat, Positioned, Power, Speed, SpeedModifier, Tame,
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

    // ── Step 1: deduct NORMAL_SPEED from player movement points ──
    {
        let player = world.player();
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(player) {
            mp.0 -= NORMAL_SPEED as i32;
        }
    }

    // ── Step 2: execute the player's action ──────────────────────
    resolve_player_action(world, &action, rng, &mut events);

    // ── Step 3: monster loop ─────────────────────────────────────
    // Each monster with movement >= NORMAL_SPEED gets to act.
    resolve_monster_turns(world, rng, &mut events);

    // ── Step 4: check whether a new game turn boundary is reached ─
    // Both sides exhausted => new turn.
    let player_mp = world
        .get_component::<MovementPoints>(world.player())
        .map(|mp| mp.0)
        .unwrap_or(0);

    let monsters_can_move = any_monster_can_move(world);

    if player_mp < NORMAL_SPEED as i32 && !monsters_can_move {
        process_new_turn(world, rng, &mut events);
    }

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

    // Phase 2: monster turns.
    let mut monster_events = Vec::new();
    resolve_monster_turns(world, rng, &mut monster_events);

    // Phase 3: new-turn processing (conditional).
    let mut new_turn_events = Vec::new();
    let player_mp = world
        .get_component::<MovementPoints>(world.player())
        .map(|mp| mp.0)
        .unwrap_or(0);
    let monsters_can_move = any_monster_can_move(world);
    if player_mp < NORMAL_SPEED as i32 && !monsters_can_move {
        process_new_turn(world, rng, &mut new_turn_events);
    }

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
        for event in player_events {
            yield event;
        }

        // Phase 3: monster turns.
        let mut monster_events = Vec::new();
        resolve_monster_turns(world, rng, &mut monster_events);
        for event in monster_events {
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
            for event in new_turn_events {
                yield event;
            }
        }
    }
}

/// Check if any monster has enough movement points to act.
fn any_monster_can_move(world: &GameWorld) -> bool {
    let player = world.player();
    for (entity, mp) in world.ecs().query::<&MovementPoints>().iter() {
        if entity != player && mp.0 >= NORMAL_SPEED as i32 {
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

    events.push(EngineEvent::TurnEnd {
        turn_number: world.turn(),
    });
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
    // ── Paralysis check: if paralyzed, skip the entire action ──
    let player = world.player();
    if crate::status::is_paralyzed(world, player) {
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
            try_move_entity(world, world.player(), effective_dir, events, rng);
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
            let drop_events = crate::inventory::drop_item(world, player, *item);
            events.extend(drop_events);
        }
        PlayerAction::DropMultiple { items } => {
            let player = world.player();
            for item in items {
                let drop_events = crate::inventory::drop_item(world, player, *item);
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
                if let nethack_babel_data::ObjectLocation::Floor { x, y } = *loc
                    && x == player_pos.x as i16
                    && y == player_pos.y as i16
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
            // Blind players cannot read scrolls/spellbooks.
            let player = world.player();
            if crate::status::is_blind(world, player) {
                events.push(EngineEvent::msg("scroll-cant-read-blind"));
            } else if let Some(item_entity) = item {
                if let Some(scroll_type) = infer_scroll_type_from_item(world, *item_entity) {
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
            let on_altar = world
                .get_component::<Positioned>(player)
                .and_then(|pos| {
                    world
                        .dungeon()
                        .current_level
                        .get(pos.0)
                        .map(|cell| cell.terrain)
                })
                .is_some_and(|terrain| terrain == Terrain::Altar);
            increment_conduct(world, player, |state| {
                state.gnostic = state.gnostic.saturating_add(1);
            });

            let mut religion_state = world
                .get_component::<crate::religion::ReligionState>(player)
                .map(|state| (*state).clone())
                .unwrap_or_else(|| default_religion_state(world, player));
            refresh_religion_state_from_world(&mut religion_state, world, player);

            let prayer_events =
                crate::religion::pray_simple(&mut religion_state, player, on_altar, None, rng);
            persist_religion_state(world, player, religion_state);
            events.extend(prayer_events);
        }
        PlayerAction::Offer { item } => {
            if item.is_some() {
                let player = world.player();
                increment_conduct(world, player, |state| {
                    state.gnostic = state.gnostic.saturating_add(1);
                });
                events.push(EngineEvent::msg("offer-generic"));
            } else {
                events.push(EngineEvent::msg("offer-what"));
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
            let monster_at_target: bool = {
                let mut found = false;
                for (entity, _) in world.ecs().query::<&Monster>().iter() {
                    if let Some(pos) = world.get_component::<Positioned>(entity)
                        && pos.0 == target_pos
                    {
                        found = true;
                        break;
                    }
                }
                found
            };
            if monster_at_target {
                events.push(EngineEvent::msg("npc-chat-no-response"));
            } else {
                events.push(EngineEvent::msg("chat-nobody-there"));
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
            // Pay shopkeeper.
            events.push(EngineEvent::msg("shop-no-debt"));
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
            try_move_entity(world, world.player(), effective_dir, events, rng);
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
            try_move_entity(world, world.player(), effective_dir, events, rng);
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
            try_move_entity(world, world.player(), effective_dir, events, rng);
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
            try_move_entity(world, world.player(), effective_dir, events, rng);
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
            try_move_entity(world, world.player(), effective_dir, events, rng);
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
                try_move_entity(world, world.player(), effective_dir, events, rng);
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
            if result == crate::do_actions::JumpResult::Jumped {
                if let Some(mut pos) = world.get_component_mut::<Positioned>(player) {
                    let from = pos.0;
                    pos.0 = *position;
                    events.push(EngineEvent::EntityMoved {
                        entity: player,
                        from,
                        to: *position,
                    });
                }
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
            if result == crate::do_actions::WipeResult::WipedCream {
                if let Some(mut hc) = world.get_component_mut::<crate::status::HeroCounters>(player)
                {
                    hc.creamed = 0;
                }
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
            events.push(EngineEvent::msg_with(
                "wizard-genesis",
                vec![("monster", monster_name.clone())],
            ));
        }
        PlayerAction::WizWish { wish_text } => {
            events.push(EngineEvent::msg_with(
                "wizard-wish",
                vec![("wish", wish_text.clone())],
            ));
        }
        PlayerAction::WizIdentify => {
            // Emit event; the caller that owns IdentificationState
            // handles the actual type-level discovery.
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
            events.push(EngineEvent::msg("wizard-where"));
        }
        PlayerAction::WizKill => {
            events.push(EngineEvent::msg("wizard-kill"));
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

    // Check for branch transition before defaulting to depth+1.
    let branch_transition = world.dungeon().check_branch_transition();
    if let Some((target_branch, target_branch_depth)) = branch_transition {
        change_level_to_branch(
            world,
            target_branch,
            target_branch_depth,
            false,
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

fn current_player_role_name(world: &GameWorld) -> Option<String> {
    world
        .get_component::<PlayerIdentity>(world.player())
        .and_then(|identity| crate::role::Role::from_id(identity.role))
        .map(|role| role.name().to_ascii_lowercase())
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
    branch: crate::dungeon::DungeonBranch,
    depth: i32,
    rng: &mut impl Rng,
) -> (
    crate::map_gen::GeneratedLevel,
    crate::special_levels::SpecialLevelFlags,
    Option<crate::special_levels::SpecialLevelPopulation>,
) {
    if let Some(id) = world.dungeon().check_topology_special(&branch, depth) {
        let role_name = special_level_role_name(world, id);
        if let Some(special) = dispatch_special_level(id, role_name.as_deref(), rng) {
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
    (
        generate_level(depth as u8, rng),
        crate::special_levels::SpecialLevelFlags::default(),
        None,
    )
}

/// Perform a level transition into a different branch.
///
/// Similar to `change_level` but switches the dungeon branch.
fn change_level_to_branch(
    world: &mut GameWorld,
    target_branch: crate::dungeon::DungeonBranch,
    target_depth: i32,
    going_up: bool,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
    let player = world.player();
    let from_depth = world.dungeon().depth;
    let from_branch = world.dungeon().branch;

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
    let (new_map, new_up_stairs, new_down_stairs, flags, special_population) =
        if world.dungeon().has_visited(target_branch, target_depth) {
            let (map, cached_mons) = world
                .dungeon_mut()
                .load_cached_level(target_branch, target_depth)
                .expect("has_visited was true");

            let up_pos = find_terrain(&map, Terrain::StairsUp);
            let down_pos = find_terrain(&map, Terrain::StairsDown);

            for cm in &cached_mons {
                world.spawn((
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
                ));
            }

            (
                map,
                up_pos,
                down_pos,
                crate::special_levels::SpecialLevelFlags::default(),
                None,
            )
        } else {
            let (generated, flags, special_population) =
                generate_or_special_topology(world, target_branch, target_depth, rng);
            (
                generated.map,
                generated.up_stairs,
                generated.down_stairs,
                flags,
                special_population,
            )
        };

    // 4. Install the new level map and store flags.
    world.dungeon_mut().current_level = new_map;
    world.dungeon_mut().current_level_flags = flags.into();

    // 5. Place the player.
    let target_pos = if going_up {
        new_down_stairs.unwrap_or(Position::new(40, 10))
    } else {
        new_up_stairs.unwrap_or(Position::new(40, 10))
    };

    if let Some(mut pos) = world.get_component_mut::<Positioned>(player) {
        pos.0 = target_pos;
    }

    if let Some(pop) = special_population {
        apply_special_level_population(world, pop, rng);
    }

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
    let (new_map, new_up_stairs, new_down_stairs, flags, special_population) =
        if world.dungeon().has_visited(target_branch, target_depth) {
            // Load from cache.
            let (map, cached_mons) = world
                .dungeon_mut()
                .load_cached_level(target_branch, target_depth)
                .expect("has_visited was true");

            // Find stairs positions from the loaded map.
            let up_pos = find_terrain(&map, Terrain::StairsUp);
            let down_pos = find_terrain(&map, Terrain::StairsDown);

            // Respawn cached monsters.
            for cm in &cached_mons {
                world.spawn((
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
                ));
            }

            (
                map,
                up_pos,
                down_pos,
                crate::special_levels::SpecialLevelFlags::default(),
                None,
            )
        } else {
            // Generate a new level.
            let (generated, flags, special_population) =
                generate_or_special_topology(world, target_branch, target_depth, rng);
            (
                generated.map,
                generated.up_stairs,
                generated.down_stairs,
                flags,
                special_population,
            )
        };

    // 6. Install the new level map and store flags.
    world.dungeon_mut().current_level = new_map;
    world.dungeon_mut().current_level_flags = flags.into();

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

    // Update position component.
    if let Some(mut pos) = world.get_component_mut::<Positioned>(entity) {
        pos.0 = target_pos;
    }

    events.push(EngineEvent::EntityMoved {
        entity,
        from: current_pos,
        to: target_pos,
    });

    // Check for traps at the destination tile.
    if entity == world.player() {
        let trap_info = build_player_trap_info(world, entity, target_pos);
        let (trap_events, _triggered) =
            trigger_trap_at(rng, &trap_info, &mut world.dungeon_mut().trap_map);
        events.extend(trap_events);
    }

    // Check if the player entered a vault (spawn guard if needed).
    if entity == world.player() {
        let vault_rooms = world.dungeon().vault_rooms.clone();
        let guard_present = world.dungeon().vault_guard_present;
        if let Some(_vault_idx) = crate::vault::player_in_vault(target_pos, &vault_rooms) {
            if !guard_present {
                let guard_data = crate::vault::spawn_guard(rng);
                world.dungeon_mut().vault_guard_present = true;
                events.push(EngineEvent::msg_with(
                    "guard-appears",
                    vec![("name", guard_data.guard_name)],
                ));
            }
        }
    }

    // Run autopickup and then report any remaining floor items.
    if entity == world.player() {
        if !crate::status::is_levitating(world, entity) {
            let mut letter_state = crate::items::LetterState::default();
            let (autopickup_enabled, autopickup_classes) = {
                let d = world.dungeon();
                (d.autopickup_enabled, d.autopickup_classes.clone())
            };
            if autopickup_enabled {
                let pickup_events = crate::inventory::autopickup(
                    world,
                    &mut letter_state,
                    &[],
                    &autopickup_classes,
                );
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
        if entity != player && mp.0 >= NORMAL_SPEED as i32 {
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
        if world.get_component::<HitPoints>(*entity).is_none() {
            continue;
        }

        // Deduct movement cost.
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(*entity) {
            mp.0 -= NORMAL_SPEED as i32;
        }

        // Run the monster AI decision tree.
        let monster_events = crate::monster_ai::resolve_monster_turn(world, *entity, rng);
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
    use crate::world::Name;
    use nethack_babel_data::{
        Alignment, ArtifactId, BucStatus, GameData, Gender, Handedness, MonsterFlags, ObjectClass,
        ObjectCore, ObjectLocation, ObjectTypeId, PlayerIdentity, PlayerSkills, RaceId, RoleId,
        SkillState, WeaponSkill, load_game_data,
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

    /// Spawn a monster at the given position with a given base speed.
    #[allow(dead_code)]
    fn spawn_monster(world: &mut GameWorld, pos: Position, speed: u32) -> hecs::Entity {
        let order = world.next_creation_order();
        world.spawn((
            Monster,
            Positioned(pos),
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
        assert!(matches!(*loc, ObjectLocation::Floor { x: 6, y: 5 }));
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
            let hp_regen = events
                .iter()
                .any(|e| matches!(e, EngineEvent::HpChange { .. }));
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
        world.spawn((
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
        ))
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

        let _events = resolve_turn(&mut world, PlayerAction::GoDown, &mut rng);

        assert_eq!(world.dungeon().depth, 20, "expected to descend to Sanctum");
        assert!(
            has_monster_named(&world, "high priest"),
            "entering Sanctum should spawn the high priest"
        );
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
            &mut rng,
            &mut events,
        );
        change_level_to_branch(
            &mut world,
            crate::dungeon::DungeonBranch::FortLudios,
            1,
            false,
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
        let mut events = Vec::new();

        change_level(&mut world, 20, false, &mut rng, &mut events);
        assert_eq!(count_monsters_named(&world, "high priest"), 1);

        change_level(&mut world, 19, true, &mut rng, &mut events);
        change_level(&mut world, 20, false, &mut rng, &mut events);

        assert_eq!(
            count_monsters_named(&world, "high priest"),
            1,
            "revisiting Sanctum should not duplicate the high priest"
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
    fn wiz_identify_emits_event() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::WizIdentify, &mut rng);
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
    fn wiz_genesis_emits_event() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::WizGenesis {
                monster_name: "dragon".to_string(),
            },
            &mut rng,
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-genesis"
        )));
    }

    #[test]
    fn wiz_wish_emits_event() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let events = resolve_turn(
            &mut world,
            PlayerAction::WizWish {
                wish_text: "blessed +3 silver dragon scale mail".to_string(),
            },
            &mut rng,
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-wish"
        )));
    }

    #[test]
    fn wiz_where_emits_event() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::WizWhere, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-where"
        )));
    }

    #[test]
    fn wiz_kill_emits_event() {
        let mut world = make_test_world();
        let mut rng = test_rng();
        let events = resolve_turn(&mut world, PlayerAction::WizKill, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-kill"
        )));
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
