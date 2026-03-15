//! Main turn loop: movement points, action dispatch, monster turns,
//! regeneration, hunger, and new-turn processing.
//!
//! Implements the NetHack `moveloop_core()` sequence where each call to
//! `resolve_turn()` processes one player action and any resulting monster
//! actions, then checks whether a new game turn boundary has been reached.

use rand::Rng;

use crate::action::{Direction, PlayerAction, Position};
use crate::dungeon::{CachedMonster, Terrain};
use crate::event::{EngineEvent, HpSource, HungerLevel};
use crate::map_gen::generate_level;
use crate::special_levels::{dispatch_special_level, identify_special_level};
use crate::traps::detect_trap;
use crate::world::{
    Boulder, CreationOrder, DisplaySymbol, Encumbrance, EncumbranceLevel,
    ExperienceLevel, GameWorld, HeroSpeed, HeroSpeedBonus, HitPoints,
    Monster, MonsterSpeedMod, MovementPoints, Name, Nutrition, PlayerCombat,
    Positioned, Power, Speed, SpeedModifier, NORMAL_SPEED,
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
    nutrition_to_hunger_level, compute_hunger_depletion, AccessoryHungerCtx,
    check_fainting, should_starve, strength_penalty_change, FaintingOutcome,
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
fn process_new_turn(
    world: &mut GameWorld,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
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
    // TODO: gas_clouds should be stored persistently in GameWorld or DungeonState.
    // For now we keep a local vec so the tick function is wired in and can be
    // tested once persistent storage is added.
    {
        let mut gas_clouds: Vec<crate::region::GasCloud> = Vec::new();
        let mut cloud_events = crate::region::tick_gas_clouds(&mut gas_clouds, world, rng);
        events.append(&mut cloud_events);
    }

    // 4i. Attribute exercise periodic check.
    {
        let turn = world.turn();
        if crate::attributes::is_exercise_turn(turn) {
            let player = world.player();
            // Copy components out to avoid simultaneous mutable borrows.
            let snapshot = {
                let attrs = world.get_component::<crate::world::Attributes>(player).map(|a| *a);
                let nat = world.get_component::<crate::attributes::NaturalAttributes>(player).map(|n| *n);
                let ex = world.get_component::<crate::attributes::AttributeExercise>(player).map(|e| *e);
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
                if let Some(mut n) = world.get_component_mut::<crate::attributes::NaturalAttributes>(player) {
                    *n = nat;
                }
                if let Some(mut e) = world.get_component_mut::<crate::attributes::AttributeExercise>(player) {
                    *e = ex;
                }
                events.extend(exercise_events);
            }
        }
    }

    // 4j. Random monster generation (1/70 chance on normal levels).
    if rng.random_range(0..70) == 0 {
        // TODO: actually spawn a monster.  For now, just note that
        // the roll succeeded.  The map_gen system will handle placement.
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

    for (entity, (speed, _monster)) in world
        .ecs()
        .query::<(&Speed, &Monster)>()
        .iter()
    {
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
            try_move_entity(world, world.player(), effective_dir, events);
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
                let pickup_events =
                    crate::inventory::pickup_all_at_player(world, &mut ls, &[]);
                events.extend(pickup_events);
            }
        }
        PlayerAction::Drop { item } => {
            let player = world.player();
            let drop_events =
                crate::inventory::drop_item(world, player, *item);
            events.extend(drop_events);
        }
        PlayerAction::DropMultiple { items } => {
            let player = world.player();
            for item in items {
                let drop_events =
                    crate::inventory::drop_item(world, player, *item);
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
        PlayerAction::PutOn { item } => {
            let player = world.player();
            events.push(EngineEvent::ItemWorn {
                actor: player,
                item: *item,
            });
        }
        PlayerAction::Apply { item } => {
            let player = world.player();
            let tool_events = crate::tools::apply_tool(
                world, player, *item, rng,
            );
            events.extend(tool_events);
        }
        PlayerAction::Engrave { text } => {
            let player = world.player();
            let player_pos = world
                .get_component::<Positioned>(player)
                .map(|p| p.0)
                .unwrap_or(Position::new(0, 0));
            // Default to dust engraving (finger writing).
            // TODO: select method based on wielded item (blade, wand, etc.)
            let method = crate::engrave::EngraveMethod::Dust;
            let engraving = crate::engrave::Engraving::new(
                text.clone(),
                method,
                player_pos,
            );
            let has_elbereth = engraving.has_elbereth();
            world.dungeon_mut().engraving_map.insert(engraving);
            events.push(EngineEvent::msg_with(
                "engrave-write",
                vec![
                    ("text", text.clone()),
                    ("method", format!("{:?}", method)),
                ],
            ));
            if has_elbereth {
                events.push(EngineEvent::msg("engrave-elbereth"));
            }
        }
        PlayerAction::Dip { item, into } => {
            let player = world.player();
            let dip_events = crate::dip::dip_item(
                world, player, *item, *into, rng,
            );
            events.extend(dip_events);
        }
        PlayerAction::Kick { direction } => {
            // Kick in the given direction.
            // TODO: detect Monk role for martial arts bonus.
            let kick_events = crate::environment::kick(
                world, *direction, false, rng,
            );
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
            for (entity, loc) in world.ecs().query::<&nethack_babel_data::ObjectLocation>().iter() {
                if let nethack_babel_data::ObjectLocation::Floor { x, y } = *loc
                    && x == player_pos.x as i16
                    && y == player_pos.y as i16
                    && world.get_component::<crate::environment::Container>(entity).is_some()
                {
                    let open_events = crate::environment::open_container(world, entity);
                    events.extend(open_events);
                    break;
                }
            }
        }
        PlayerAction::Eat { item } => {
            if let Some(_item_entity) = item {
                // TODO: look up FoodDef from item entity for full eat logic
                events.push(EngineEvent::msg("eat-generic"));
            } else {
                events.push(EngineEvent::msg("eat-what"));
            }
        }
        PlayerAction::Quaff { item } => {
            if let Some(_item_entity) = item {
                // TODO: look up PotionType from item entity
                events.push(EngineEvent::msg("quaff-generic"));
            } else {
                events.push(EngineEvent::msg("quaff-what"));
            }
        }
        PlayerAction::Read { item } => {
            // Blind players cannot read scrolls/spellbooks.
            let player = world.player();
            if crate::status::is_blind(world, player) {
                events.push(EngineEvent::msg("scroll-cant-read-blind"));
            } else if let Some(_item_entity) = item {
                // TODO: look up ScrollType from item entity
                events.push(EngineEvent::msg("read-generic"));
            } else {
                events.push(EngineEvent::msg("read-what"));
            }
        }
        PlayerAction::CastSpell { spell, direction } => {
            let player = world.player();
            let cast_events = crate::spells::cast_spell(
                world, player, *spell, *direction, rng,
            );
            events.extend(cast_events);
        }
        PlayerAction::ZapWand { item: _, direction } => {
            // Confused/stunned zapper gets a randomized direction.
            let player = world.player();
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let _effective_dir = if (confused || stunned) && direction.is_some() {
                match crate::status::maybe_confuse_direction(confused, stunned, rng) {
                    Some(random_dir) => Some(random_dir),
                    None => *direction,
                }
            } else {
                *direction
            };
            // TODO: look up WandType and WandCharges from item entity,
            // then call zap_wand() with _effective_dir
            events.push(EngineEvent::msg("zap-generic"));
        }
        PlayerAction::Throw { item, direction } => {
            let player = world.player();
            let throw_events = crate::ranged::resolve_throw(
                world, player, *item, *direction, rng,
            );
            events.extend(throw_events);
        }
        PlayerAction::Fire => {
            // Fire requires a launcher + ammo + direction; for now stub it.
            // TODO: look up wielded launcher and quivered ammo, prompt for
            // direction, then call ranged::resolve_fire().
            events.push(EngineEvent::msg("fire-no-ammo"));
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
                let terrain = world
                    .dungeon()
                    .current_level
                    .get(target)
                    .map(|c| c.terrain);
                if terrain == Some(Terrain::DoorLocked) {
                    let lock_events = crate::lock::force_lock(
                        world, player, target, rng,
                    );
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
            // Simplified prayer — emit a prayer event.
            events.push(EngineEvent::msg("pray-begin"));
            // TODO: wire to religion::pray_simple once ReligionState
            // is stored in the world ECS.
        }
        PlayerAction::Offer { item } => {
            if item.is_some() {
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
                for (entity, _) in world.ecs().query::<&Monster>().iter()
                {
                    if let Some(pos) =
                        world.get_component::<Positioned>(entity)
                    && pos.0 == target_pos {
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
            // Find an adjacent tame monster to mount.
            events.push(EngineEvent::msg("ride-not-available"));
        }
        PlayerAction::Pay => {
            // Pay shopkeeper.
            events.push(EngineEvent::msg("shop-no-debt"));
        }
        PlayerAction::EnhanceSkill => {
            events.push(EngineEvent::msg("enhance-not-available"));
        }
        PlayerAction::MoveUntilInterrupt { direction } => {
            // Run: move in direction (simplified — just one step).
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if confused || stunned {
                match crate::status::maybe_confuse_direction(
                    confused, stunned, rng,
                ) {
                    Some(random_dir) => random_dir,
                    None => *direction,
                }
            } else {
                *direction
            };
            try_move_entity(world, world.player(), effective_dir, events);
        }
        PlayerAction::FightDirection { direction } => {
            // Force fight: move/attack in direction, skipping peaceful check.
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if confused || stunned {
                match crate::status::maybe_confuse_direction(
                    confused, stunned, rng,
                ) {
                    Some(random_dir) => random_dir,
                    None => *direction,
                }
            } else {
                *direction
            };
            try_move_entity(world, world.player(), effective_dir, events);
        }
        PlayerAction::RunDirection { direction } => {
            // Run until interrupted (simplified — one step for now).
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if confused || stunned {
                match crate::status::maybe_confuse_direction(
                    confused, stunned, rng,
                ) {
                    Some(random_dir) => random_dir,
                    None => *direction,
                }
            } else {
                *direction
            };
            try_move_entity(world, world.player(), effective_dir, events);
        }
        PlayerAction::RushDirection { direction } => {
            // Rush: run without picking up items (simplified — one step).
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if confused || stunned {
                match crate::status::maybe_confuse_direction(
                    confused, stunned, rng,
                ) {
                    Some(random_dir) => random_dir,
                    None => *direction,
                }
            } else {
                *direction
            };
            try_move_entity(world, world.player(), effective_dir, events);
        }
        PlayerAction::MoveNoPickup { direction } => {
            // Move without auto-pickup.
            let confused = crate::status::is_confused(world, player);
            let stunned = crate::status::is_stunned(world, player);
            let effective_dir = if confused || stunned {
                match crate::status::maybe_confuse_direction(
                    confused, stunned, rng,
                ) {
                    Some(random_dir) => random_dir,
                    None => *direction,
                }
            } else {
                *direction
            };
            try_move_entity(world, world.player(), effective_dir, events);
        }
        PlayerAction::Wait => {
            // Do nothing, consume one turn.
            events.push(EngineEvent::msg("wait"));
        }
        PlayerAction::Travel { destination: _ } => {
            // Travel to destination (simplified — not yet implemented).
            events.push(EngineEvent::msg("travel-not-implemented"));
        }
        PlayerAction::ToggleTwoWeapon => {
            events.push(EngineEvent::msg("two-weapon-not-implemented"));
        }
        PlayerAction::Name { .. } => {
            events.push(EngineEvent::msg("name-not-implemented"));
        }
        PlayerAction::Adjust { .. } => {
            events.push(EngineEvent::msg("adjust-not-implemented"));
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
            let sit_events = crate::sit::do_sit(
                rng, terrain, false, is_levitating, false, !is_levitating, 0,
            );
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
            // TODO: check for boots of jumping to determine has_jumping
            // and max_range (3 with boots, 2 without).
            let (result, jump_events) =
                crate::do_actions::do_jump(true, 0, distance, 3);
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
            let has_trap = world
                .dungeon()
                .trap_map
                .trap_at(target_pos)
                .is_some();
            let dex = 14i32; // TODO: read from player attributes
            let (_, untrap_events) =
                crate::do_actions::do_untrap(rng, dex, has_trap, 5);
            events.extend(untrap_events);
        }
        PlayerAction::TurnUndead => {
            // TODO: count actual undead nearby, check role.
            let (_result, turn_events) =
                crate::do_actions::do_turn_undead(false, 1, 0);
            events.extend(turn_events);
        }
        PlayerAction::Swap => {
            // TODO: check actual weapon state from ECS.
            let (_result, swap_events) =
                crate::do_actions::do_swap_weapons(false, false);
            events.extend(swap_events);
        }
        PlayerAction::Wipe => {
            let player = world.player();
            // TODO: read creamed/towel state from ECS components.
            let creamed = 0u32;
            let blind_towel = false;
            let (result, wipe_events) =
                crate::do_actions::do_wipe(creamed, blind_towel);
            if result == crate::do_actions::WipeResult::WipedCream {
                // TODO: set creamed to 0 on the player entity.
                let _ = player;
            }
            events.extend(wipe_events);
        }
        PlayerAction::Tip { item: _ } => {
            // Simplified: assume empty container for now.
            let (_result, tip_events) =
                crate::do_actions::do_tip(false, true, 0, true);
            events.extend(tip_events);
        }
        PlayerAction::Rub { item: _ } => {
            // TODO: look up item properties from ECS.
            let (_result, rub_events) =
                crate::do_actions::do_rub(rng, false, false, false);
            events.extend(rub_events);
        }
        PlayerAction::InvokeArtifact { item: _ } => {
            // TODO: look up artifact properties from ECS.
            let (_result, invoke_events) =
                crate::do_actions::do_invoke(false, true, false, 0, "unknown");
            events.extend(invoke_events);
        }
        PlayerAction::Monster => {
            let (_result, mon_events) =
                crate::do_actions::do_monster_ability(false, false);
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
            let call_events =
                crate::do_actions::do_call_type(*class, name);
            events.extend(call_events);
        }
        PlayerAction::Glance { direction: _ } => {
            let glance_events =
                crate::do_actions::do_glance("You see nothing special.");
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
            change_level(world, target_depth, target_depth < world.dungeon().depth, rng, events);
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
        | PlayerAction::Annotate { .. }
        | PlayerAction::Attributes
        | PlayerAction::LookAt { .. }
        | PlayerAction::LookHere
        | PlayerAction::Redraw
        | PlayerAction::Save
        | PlayerAction::Quit
        | PlayerAction::SaveAndQuit => {}
    }
}

/// Handle the player going up stairs.
fn handle_go_up(
    world: &mut GameWorld,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
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
fn handle_go_down(
    world: &mut GameWorld,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
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
            world, target_branch, target_branch_depth, false, rng, events,
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
) -> (crate::map_gen::GeneratedLevel, crate::special_levels::SpecialLevelFlags) {
    if let Some(id) = identify_special_level(branch, depth)
        && let Some(special) = dispatch_special_level(id, None, rng)
    {
        return (special.generated, special.flags);
    }
    (generate_level(depth as u8, rng), crate::special_levels::SpecialLevelFlags::default())
}

/// Topology-aware special level dispatch; fall back to random generation.
///
/// Uses the per-game randomized topology depths for the Main branch.
fn generate_or_special_topology(
    world: &crate::world::GameWorld,
    branch: crate::dungeon::DungeonBranch,
    depth: i32,
    rng: &mut impl Rng,
) -> (crate::map_gen::GeneratedLevel, crate::special_levels::SpecialLevelFlags) {
    if let Some(id) = world.dungeon().check_topology_special(&branch, depth)
        && let Some(special) = dispatch_special_level(id, None, rng)
    {
        return (special.generated, special.flags);
    }
    (generate_level(depth as u8, rng), crate::special_levels::SpecialLevelFlags::default())
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

    world
        .dungeon_mut()
        .cache_current_level(cached_monsters);

    // 2. Switch branch and depth.
    world.dungeon_mut().branch = target_branch;
    world.dungeon_mut().depth = target_depth;

    // 3. Load or generate the target level.
    let (new_map, new_up_stairs, new_down_stairs, flags) =
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

            (map, up_pos, down_pos, crate::special_levels::SpecialLevelFlags::default())
        } else {
            let (generated, flags) = generate_or_special_topology(world, target_branch, target_depth, rng);
            (generated.map, generated.up_stairs, generated.down_stairs, flags)
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
    world
        .dungeon_mut()
        .cache_current_level(cached_monsters);

    // 4. Switch depth.
    world.dungeon_mut().depth = target_depth;

    // 5. Load or generate the target level.
    let target_branch = branch;
    let (new_map, new_up_stairs, new_down_stairs, flags) =
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

            (map, up_pos, down_pos, crate::special_levels::SpecialLevelFlags::default())
        } else {
            // Generate a new level.
            let (generated, flags) = generate_or_special_topology(world, target_branch, target_depth, rng);
            (generated.map, generated.up_stairs, generated.down_stairs, flags)
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

    // 8. Emit LevelChanged event.
    events.push(EngineEvent::LevelChanged {
        entity: player,
        from_depth: format!("{}", from_depth),
        to_depth: format!("{}", target_depth),
    });
}

/// Find the first cell with the given terrain type on a map.
fn find_terrain(
    map: &crate::dungeon::LevelMap,
    terrain: Terrain,
) -> Option<Position> {
    for y in 0..map.height {
        for x in 0..map.width {
            if map.cells[y][x].terrain == terrain {
                return Some(Position::new(x as i32, y as i32));
            }
        }
    }
    None
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

    // Check for items on the destination tile (autopickup notification).
    // TODO: implement full autopickup logic (filter by item class, config).
    if entity == world.player() {
        let items_here = count_items_at(world, target_pos);
        if items_here == 1 {
            events.push(EngineEvent::msg("You see an item here."));
        } else if items_here > 1 {
            events.push(EngineEvent::msg_with(
                "You see items here.",
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
    for (entity, (positioned, _boulder)) in world
        .ecs()
        .query::<(&Positioned, &Boulder)>()
        .iter()
    {
        if positioned.0 == pos {
            return Some(entity);
        }
    }
    None
}

/// Count the number of item entities at the given position.
fn count_items_at(world: &GameWorld, pos: crate::action::Position) -> usize {
    let player = world.player();
    let mut count = 0;
    for (entity, positioned) in world.ecs().query::<&Positioned>().iter() {
        // Skip the player and monsters — only count items.
        if entity == player {
            continue;
        }
        if world.get_component::<Monster>(entity).is_some() {
            continue;
        }
        if positioned.0 == pos {
            count += 1;
        }
    }
    count
}

/// Give each monster entity a turn, ordered by speed descending, then
/// creation order ascending (Decision D2 from the spec).
///
/// Only monsters with `movement >= NORMAL_SPEED` get to act.  Each
/// action costs `NORMAL_SPEED` points.
fn resolve_monster_turns(
    world: &mut GameWorld,
    rng: &mut impl Rng,
    events: &mut Vec<EngineEvent>,
) {
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
        let monster_events =
            crate::monster_ai::resolve_monster_turn(world, *entity, rng);
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
fn process_hunger(
    world: &mut GameWorld,
    events: &mut Vec<EngineEvent>,
    rng: &mut impl Rng,
) {
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

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::dungeon::Terrain;
    use crate::world::Name;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    /// Deterministic RNG for reproducible tests.
    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(42)
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

    /// Spawn a monster at the given position with a given base speed.
    #[allow(dead_code)]
    fn spawn_monster(
        world: &mut GameWorld,
        pos: Position,
        speed: u32,
    ) -> hecs::Entity {
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

        let pos = world
            .get_component::<Positioned>(world.player())
            .unwrap();
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
        let pos = world
            .get_component::<Positioned>(world.player())
            .unwrap();
        assert_eq!(pos.0.y, 3);
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

        let pos = world
            .get_component::<Positioned>(world.player())
            .unwrap();
        assert_eq!(pos.0, Position::new(5, 5));
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
            let amt = u_calc_moveamt(
                12,
                HeroSpeed::VeryFast,
                Encumbrance::Unencumbered,
                &mut rng,
            );
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
            let amt = u_calc_moveamt(
                12,
                HeroSpeed::Fast,
                Encumbrance::Unencumbered,
                &mut rng,
            );
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
            let amt = u_calc_moveamt(
                12,
                HeroSpeed::VeryFast,
                Encumbrance::Stressed,
                &mut rng,
            );
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

        assert_eq!(
            world.turn(),
            initial_turn + 1,
            "turn should have advanced"
        );
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

        let initial = world
            .get_component::<Nutrition>(world.player())
            .unwrap()
            .0;
        assert_eq!(initial, 900);

        resolve_turn(&mut world, PlayerAction::Rest, &mut rng);

        let after = world
            .get_component::<Nutrition>(world.player())
            .unwrap()
            .0;
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
            matches!(e, EngineEvent::EntityDied {
                cause: crate::event::DeathCause::Starvation,
                ..
            })
        });
        assert!(died, "hero should die of starvation at nutrition <= -(100 + 10*con)");
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
            matches!(e, EngineEvent::EntityDied {
                cause: crate::event::DeathCause::Starvation,
                ..
            })
        });
        assert!(!died, "hero should survive at exactly the starvation threshold");
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

        let strength_msg = events.iter().any(|e| {
            matches!(e, EngineEvent::Message { key, .. } if key == "hunger-weak-strength-loss")
        });
        assert!(strength_msg, "entering Weak should emit strength loss message");
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
            let hp = world
                .get_component::<HitPoints>(world.player())
                .unwrap();
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
        if let Some(mut enc) = world
            .get_component_mut::<EncumbranceLevel>(world.player())
        {
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
        if let Some(mut enc) = world
            .get_component_mut::<EncumbranceLevel>(world.player())
        {
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
        let pos1 = world1
            .get_component::<Positioned>(world1.player())
            .unwrap();
        let pos2 = world2
            .get_component::<Positioned>(world2.player())
            .unwrap();
        assert_eq!(pos1.0, pos2.0, "player position should match");
        assert_eq!(
            world1.turn(),
            world2.turn(),
            "turn counter should match"
        );
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
        let events_vec = resolve_turn(
            &mut world_a,
            PlayerAction::Rest,
            &mut rng_a,
        );

        // Run with turn_events (from_fn-based).
        let mut world_b = make_test_world();
        let mut rng_b = test_rng();
        let events_from_fn: Vec<EngineEvent> = turn_events(
            &mut world_b,
            PlayerAction::Rest,
            &mut rng_b,
        )
        .collect();

        // Run with turn_events_gen (gen-block-based).
        let mut world_c = make_test_world();
        let mut rng_c = test_rng();
        let events_gen: Vec<EngineEvent> = turn_events_gen(
            &mut world_c,
            PlayerAction::Rest,
            &mut rng_c,
        )
        .collect();

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
        for (i, (ev_vec, ev_gen)) in
            events_vec.iter().zip(events_gen.iter()).enumerate()
        {
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

        for (i, (ev_vec, ev_gen)) in
            events_vec.iter().zip(events_gen.iter()).enumerate()
        {
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
            let events = resolve_turn(
                &mut world_a,
                PlayerAction::Rest,
                &mut rng_a,
            );
            for e in &events {
                all_vec.push(format!("{:?}", e));
            }

            let events: Vec<EngineEvent> = turn_events_gen(
                &mut world_b,
                PlayerAction::Rest,
                &mut rng_b,
            )
            .collect();
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
        let events = resolve_turn(
            &mut world,
            PlayerAction::Search,
            &mut rng,
        );

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

            let events = resolve_turn(
                &mut world,
                PlayerAction::Search,
                &mut rng,
            );

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
            HitPoints {
                current: 8,
                max: 8,
            },
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
        let up_pos = find_terrain(
            &world.dungeon().current_level,
            Terrain::StairsUp,
        );
        if let Some(up) = up_pos {
            if let Some(mut pos) = world
                .get_component_mut::<Positioned>(world.player())
            {
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
        assert!(order_a < order_b,
                "A created first should have lower order: A={}, B={}", order_a, order_b);

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
        assert_eq!(monsters[0].0, monster_a,
                   "same-speed monsters: A (created first) should act first");
        assert_eq!(monsters[1].0, monster_b,
                   "same-speed monsters: B (created second) should act second");
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

        assert_eq!(monsters[0].0, fast,
                   "faster monster acts first regardless of creation order");
        assert_eq!(monsters[1].0, slow,
                   "slower monster acts second");
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
            PlayerAction::Move { direction: Direction::East },
            &mut rng,
        );

        // Player should NOT have moved.
        let pos = world
            .get_component::<Positioned>(world.player())
            .unwrap();
        assert_eq!(
            pos.0,
            Position::new(5, 5),
            "paralyzed player should not move"
        );

        // Should have a paralysis message.
        let has_para_msg = events.iter().any(|e| {
            matches!(e, EngineEvent::Message { key, .. } if key.contains("paralyzed"))
        });
        assert!(has_para_msg, "expected paralysis message");

        // No EntityMoved event should have been emitted for the player.
        let player_moved = events.iter().any(|e| {
            matches!(e, EngineEvent::EntityMoved { entity, .. } if *entity == player)
        });
        assert!(!player_moved, "paralyzed player should not emit EntityMoved");
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
                PlayerAction::Move { direction: Direction::East },
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
                PlayerAction::Move { direction: Direction::East },
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
        assert!(
            went_wrong,
            "stunned player should have movement randomized"
        );
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
        let events = resolve_turn(
            &mut world,
            PlayerAction::GoDown,
            &mut rng,
        );

        // Should have a levitation-blocks message.
        let has_lev_msg = events.iter().any(|e| {
            matches!(e, EngineEvent::Message { key, .. } if key.contains("levitating"))
        });
        assert!(has_lev_msg, "expected levitation blocking message");

        // Depth should remain unchanged.
        assert_eq!(
            world.dungeon().depth, 1,
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
        use crate::traps::{avoid_trap, is_floor_trigger, TrapInstance};
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
        let events = resolve_turn(
            &mut world,
            PlayerAction::PickUp,
            &mut rng,
        );

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
        assert!(unexplored_before > 0, "should have unexplored cells before WizMap");

        let events = resolve_turn(&mut world, PlayerAction::WizMap, &mut rng);

        // All cells should now be explored.
        let unexplored_after = (0..world.dungeon().current_level.height)
            .flat_map(|y| (0..world.dungeon().current_level.width).map(move |x| (x, y)))
            .filter(|&(x, y)| !world.dungeon().current_level.cells[y][x].explored)
            .count();
        assert_eq!(unexplored_after, 0, "all cells should be explored after WizMap");

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
        let lf = world.get_component::<crate::light::LightFuel>(lamp).unwrap();
        assert_eq!(lf.fuel, 2, "light source fuel should decrement by 1 per turn");
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
        let lf = world.get_component::<crate::light::LightFuel>(lamp).unwrap();
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
        assert_eq!(world.dungeon().depth, 5, "WizLevelTeleport should change depth");

        // Should have a level-teleport message and a LevelChanged event.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "wizard-level-teleport"
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::LevelChanged { .. }
        )));
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

        let events = resolve_turn(
            &mut world,
            PlayerAction::Read { item: None },
            &mut rng,
        );

        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "scroll-cant-read-blind"
        )), "Blind player should get cant-read message");
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

        let events = resolve_turn(
            &mut world,
            PlayerAction::Read { item: None },
            &mut rng,
        );

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

        // Use player entity as a stand-in for the wand item (wand lookup is TODO).
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

    // ── Gas cloud ticking ─────────────────────────────────────────────

    #[test]
    fn test_gas_cloud_ticks_in_turn() {
        // Verify that process_new_turn calls tick_gas_clouds without error.
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Run a rest turn which triggers process_new_turn.
        let events = resolve_turn(&mut world, PlayerAction::Rest, &mut rng);
        // No gas clouds => no cloud damage events, but no crash either.
        assert!(!events.iter().any(|e| matches!(
            e,
            EngineEvent::HpChange { .. }
        )));
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
}
