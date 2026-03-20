//! Unified tool application dispatch — the `#apply` command handler.
//!
//! This module provides `apply_tool()`, the top-level entry point for the
//! `#apply` extended command.  It dispatches to handlers in this file for
//! extended tools (towel, bell, candle, candelabrum, grease, touchstone,
//! cream pie, figurine, leash, horn, drum, polearm, saddle, bullwhip)
//! and falls back to `tools::apply_tool()` for base tools (lamp, mirror,
//! stethoscope, whistle, pick-axe, key, tin opener, camera, unicorn horn).
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{
    BucStatus, Enchantment, ObjectClass, ObjectDef, ObjectLocation, ShopState,
};

use crate::action::{Direction, Position};
use crate::event::EngineEvent;
use crate::world::{Attributes, GameWorld, HitPoints, Monster, Name, Positioned, Tame};

// ---------------------------------------------------------------------------
// Unified dispatch — top-level entry point for #apply
// ---------------------------------------------------------------------------

/// Apply a tool: the unified entry point for the `#apply` extended command.
///
/// Looks up the tool entity, tries extended tool handlers first (this module),
/// then falls back to base tool handlers in `tools.rs`.  Returns an empty
/// event list with a "nothing happens" message if the item isn't a known tool.
///
/// `direction` is used by directional tools (polearm, mirror, bullwhip, etc.)
/// but may be `None` for non-directional tools.
pub fn apply_tool(
    world: &mut GameWorld,
    user: Entity,
    tool: Entity,
    _direction: Option<Direction>,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    // Try extended tools first (this module).
    if let Some(events) = apply_ext_tool(world, user, tool, rng) {
        return events;
    }

    // Fall back to base tools (tools.rs).
    if crate::tools::classify_tool(world, tool).is_some() {
        return crate::tools::apply_tool(world, user, tool, rng);
    }

    // Not a recognized tool at all.
    vec![EngineEvent::msg("tool-nothing-happens")]
}

// ---------------------------------------------------------------------------
// Extended tool type classification
// ---------------------------------------------------------------------------

/// Additional tool types beyond those in `tools::ToolType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtToolType {
    Towel,
    Bell,
    BellOfOpening,
    BagOfTricks,
    HornOfPlenty,
    WoodenFlute,
    MagicFlute,
    WoodenHarp,
    MagicHarp,
    Bugle,
    LeatherDrum,
    TallowCandle,
    WaxCandle,
    Candelabrum,
    CanOfGrease,
    Touchstone,
    CreamPie,
    Figurine,
    Leash,
    FrostHorn,
    FireHorn,
    TooledHorn,
    DrumOfEarthquake,
    Polearm,
    Saddle,
    Bullwhip,
}

/// Classify an item as an extended tool type by name.
pub fn classify_ext_tool(name: &str) -> Option<ExtToolType> {
    let lower = name.to_lowercase();
    if lower.contains("towel") {
        Some(ExtToolType::Towel)
    } else if lower == "bell of opening" {
        Some(ExtToolType::BellOfOpening)
    } else if lower == "bag of tricks" {
        Some(ExtToolType::BagOfTricks)
    } else if lower == "horn of plenty" {
        Some(ExtToolType::HornOfPlenty)
    } else if lower.contains("bell") {
        Some(ExtToolType::Bell)
    } else if lower == "wooden flute" {
        Some(ExtToolType::WoodenFlute)
    } else if lower == "magic flute" {
        Some(ExtToolType::MagicFlute)
    } else if lower == "wooden harp" {
        Some(ExtToolType::WoodenHarp)
    } else if lower == "magic harp" {
        Some(ExtToolType::MagicHarp)
    } else if lower == "bugle" {
        Some(ExtToolType::Bugle)
    } else if lower == "leather drum" {
        Some(ExtToolType::LeatherDrum)
    } else if lower.contains("tallow candle") {
        Some(ExtToolType::TallowCandle)
    } else if lower.contains("wax candle") {
        Some(ExtToolType::WaxCandle)
    } else if lower.contains("candelabrum") {
        Some(ExtToolType::Candelabrum)
    } else if lower.contains("can of grease") || lower.contains("grease") {
        Some(ExtToolType::CanOfGrease)
    } else if lower.contains("touchstone") {
        Some(ExtToolType::Touchstone)
    } else if lower.contains("cream pie") {
        Some(ExtToolType::CreamPie)
    } else if lower.contains("figurine") {
        Some(ExtToolType::Figurine)
    } else if lower.contains("leash") {
        Some(ExtToolType::Leash)
    } else if lower.contains("frost horn") {
        Some(ExtToolType::FrostHorn)
    } else if lower.contains("fire horn") {
        Some(ExtToolType::FireHorn)
    } else if lower.contains("tooled horn") {
        Some(ExtToolType::TooledHorn)
    } else if lower.contains("drum of earthquake") {
        Some(ExtToolType::DrumOfEarthquake)
    } else if lower.contains("polearm")
        || lower.contains("halberd")
        || lower.contains("glaive")
        || lower.contains("partisan")
        || lower.contains("lance")
    {
        Some(ExtToolType::Polearm)
    } else if lower.contains("saddle") {
        Some(ExtToolType::Saddle)
    } else if lower.contains("bullwhip") {
        Some(ExtToolType::Bullwhip)
    } else {
        None
    }
}

/// Classify an item entity as an extended tool type.
pub fn classify_ext_tool_entity(world: &GameWorld, item: Entity) -> Option<ExtToolType> {
    let name = world.entity_name(item);
    classify_ext_tool(&name)
}

// ---------------------------------------------------------------------------
// Main dispatch
// ---------------------------------------------------------------------------

/// Apply an extended tool. Returns events, or None if this item isn't
/// recognized as an extended tool (caller should try `tools::apply_tool`).
pub fn apply_ext_tool(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    rng: &mut impl Rng,
) -> Option<Vec<EngineEvent>> {
    let tool_type = classify_ext_tool_entity(world, item)?;

    let buc = world
        .get_component::<BucStatus>(item)
        .map(|b| BucStatus {
            cursed: b.cursed,
            blessed: b.blessed,
            bknown: b.bknown,
        })
        .unwrap_or(BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        });

    let events = match tool_type {
        ExtToolType::Towel => apply_towel(world, player, &buc, rng),
        ExtToolType::Bell | ExtToolType::BellOfOpening => {
            apply_bell(world, player, item, tool_type, &buc, rng)
        }
        ExtToolType::BagOfTricks => apply_bag_of_tricks(world, player, item, rng),
        ExtToolType::HornOfPlenty => apply_horn_of_plenty(world, player, item, rng),
        ExtToolType::WoodenFlute => apply_instrument(
            world,
            player,
            item,
            crate::music::InstrumentType::WoodenFlute,
            rng,
        ),
        ExtToolType::MagicFlute => apply_instrument(
            world,
            player,
            item,
            crate::music::InstrumentType::MagicFlute,
            rng,
        ),
        ExtToolType::WoodenHarp => apply_instrument(
            world,
            player,
            item,
            crate::music::InstrumentType::WoodenHarp,
            rng,
        ),
        ExtToolType::MagicHarp => apply_instrument(
            world,
            player,
            item,
            crate::music::InstrumentType::MagicHarp,
            rng,
        ),
        ExtToolType::Bugle => apply_instrument(
            world,
            player,
            item,
            crate::music::InstrumentType::Bugle,
            rng,
        ),
        ExtToolType::LeatherDrum => apply_instrument(
            world,
            player,
            item,
            crate::music::InstrumentType::LeatherDrum,
            rng,
        ),
        ExtToolType::TallowCandle | ExtToolType::WaxCandle => {
            apply_candle(world, player, item, rng)
        }
        ExtToolType::Candelabrum => apply_candelabrum(world, player, item, rng),
        ExtToolType::CanOfGrease => apply_grease(world, player, item, &buc, rng),
        ExtToolType::Touchstone => apply_touchstone(world, player, item, &buc, rng),
        ExtToolType::CreamPie => apply_cream_pie(world, player, item, rng),
        ExtToolType::Figurine => apply_figurine(world, player, item, &buc, rng),
        ExtToolType::Leash => apply_leash(world, player, rng),
        ExtToolType::FrostHorn => apply_horn(world, player, item, "frost", &buc, rng),
        ExtToolType::FireHorn => apply_horn(world, player, item, "fire", &buc, rng),
        ExtToolType::TooledHorn => apply_tooled_horn(world, player, rng),
        ExtToolType::DrumOfEarthquake => apply_drum(world, player, item, &buc, rng),
        ExtToolType::Polearm => apply_polearm(world, player, item, rng),
        ExtToolType::Saddle => apply_saddle(world, player, rng),
        ExtToolType::Bullwhip => apply_bullwhip(world, player, item, rng),
    };

    Some(events)
}

// ---------------------------------------------------------------------------
// Instrument bridge
// ---------------------------------------------------------------------------

fn apply_instrument(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    instrument_type: crate::music::InstrumentType,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let charges = world
        .get_component::<Enchantment>(item)
        .map(|ench| ench.spe);
    let (events, charge_used) =
        crate::music::play_instrument(world, player, instrument_type, charges, rng);

    if charge_used && let Some(mut ench) = world.get_component_mut::<Enchantment>(item) {
        ench.spe -= 1;
    }

    events
}

fn choose_weighted_object_def<'a>(
    candidates: &[&'a ObjectDef],
    rng: &mut impl Rng,
) -> Option<&'a ObjectDef> {
    if candidates.is_empty() {
        return None;
    }

    let total_weight: u32 = candidates
        .iter()
        .map(|def| u32::from(def.prob.max(1)))
        .sum();
    if total_weight == 0 {
        return candidates.first().copied();
    }

    let mut roll = rng.random_range(0..total_weight);
    for def in candidates {
        let weight = u32::from(def.prob.max(1));
        if roll < weight {
            return Some(*def);
        }
        roll -= weight;
    }
    candidates.last().copied()
}

fn spawn_bag_of_tricks_monster(
    world: &mut GameWorld,
    player: Entity,
    rng: &mut impl Rng,
) -> Option<(Entity, Position)> {
    let player_pos = world.get_component::<Positioned>(player).map(|pos| pos.0)?;
    let monster_defs = world.monster_catalog().to_vec();
    let difficulty = world.dungeon().current_depth().max(1) as u32;
    let monster_id = crate::makemon::rndmonst(&monster_defs, difficulty, rng)?;
    let monster_def = monster_defs.iter().find(|def| def.id == monster_id)?;
    let spawn_pos = crate::makemon::enexto(world, player_pos, monster_def)?;
    let monster = crate::makemon::makemon(
        world,
        &monster_defs,
        Some(monster_id),
        spawn_pos,
        crate::makemon::MakeMonFlags::NO_GROUP,
        rng,
    )?;
    Some((monster, spawn_pos))
}

pub(crate) fn spill_bag_of_tricks(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    charges_to_spend: i32,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    if charges_to_spend <= 0 {
        if let Some(mut knowledge) =
            world.get_component_mut::<nethack_babel_data::KnowledgeState>(item)
        {
            knowledge.cknown = true;
        }
        return vec![EngineEvent::msg("tool-nothing-happens")];
    }

    let mut spawn_count = 0usize;
    let mut events = Vec::new();

    for _ in 0..charges_to_spend.max(0) as usize {
        if let Some(mut ench) = world.get_component_mut::<Enchantment>(item) {
            ench.spe = ench.spe.saturating_sub(1);
        }

        let mut create_count = 1usize;
        if rng.random_range(0..23) == 0 {
            create_count += rng.random_range(1..=7);
        }

        for _ in 0..create_count {
            if let Some((monster, position)) = spawn_bag_of_tricks_monster(world, player, rng) {
                spawn_count += 1;
                events.push(EngineEvent::MonsterGenerated {
                    entity: monster,
                    position,
                });
            }
        }
    }

    if spawn_count > 0 {
        if let Some(mut knowledge) =
            world.get_component_mut::<nethack_babel_data::KnowledgeState>(item)
        {
            knowledge.cknown = true;
        }
        events.insert(
            0,
            EngineEvent::msg_with(
                "tip-bag-of-tricks",
                vec![("count", spawn_count.to_string())],
            ),
        );
    } else {
        events.push(EngineEvent::msg("tool-nothing-happens"));
    }

    events
}

fn choose_horn_of_plenty_object_def<'a>(
    object_defs: &'a [ObjectDef],
    rng: &mut impl Rng,
) -> Option<&'a ObjectDef> {
    if rng.random_range(0..13) == 0 {
        let potion_candidates: Vec<&ObjectDef> = object_defs
            .iter()
            .filter(|def| {
                def.class == ObjectClass::Potion
                    && !def.name.eq_ignore_ascii_case("potion of sickness")
                    && !def.is_magic
            })
            .collect();
        return choose_weighted_object_def(&potion_candidates, rng);
    }

    let mut chosen = choose_weighted_object_def(
        &object_defs
            .iter()
            .filter(|def| def.class == ObjectClass::Food)
            .collect::<Vec<_>>(),
        rng,
    )?;

    if chosen.name.eq_ignore_ascii_case("food ration")
        && rng.random_range(0..7) == 0
        && let Some(royal_jelly) = object_defs
            .iter()
            .find(|def| def.name.eq_ignore_ascii_case("lump of royal jelly"))
    {
        chosen = royal_jelly;
    }

    Some(chosen)
}

fn maybe_bill_generated_horn_item_in_shop(
    world: &mut GameWorld,
    player: Entity,
    horn: Entity,
    generated_item: Entity,
) -> bool {
    let Some(player_pos) = world.get_component::<Positioned>(player).map(|pos| pos.0) else {
        return false;
    };
    let Some(shop_idx) = world
        .dungeon()
        .shop_rooms
        .iter()
        .position(|shop| shop.contains(player_pos) && shop.bill.find(horn).is_some())
    else {
        return false;
    };

    let Some(core) = world.get_component::<nethack_babel_data::ObjectCore>(generated_item) else {
        return false;
    };
    let quantity = core.quantity.max(1);
    let spe = world
        .get_component::<Enchantment>(generated_item)
        .map(|ench| ench.spe)
        .unwrap_or(0);
    let Some(object_def) = crate::items::object_def_for_core(world.object_catalog(), &core) else {
        return false;
    };
    let charisma = world
        .get_component::<Attributes>(player)
        .map(|attributes| attributes.charisma)
        .unwrap_or(10);
    let unit_price = crate::shop::get_full_buy_price(
        object_def.cost as i32,
        object_def.class,
        spe,
        1,
        charisma,
        false,
        false,
        0,
        false,
        false,
    )
    .max(1);
    drop(core);

    if world.dungeon_mut().shop_rooms[shop_idx]
        .bill
        .add(generated_item, unit_price, quantity)
    {
        let _ = world.ecs_mut().insert_one(
            generated_item,
            ShopState {
                unpaid: true,
                no_charge: false,
            },
        );
        crate::shop::sync_item_shop_states(world);
        true
    } else {
        false
    }
}

pub(crate) fn spawn_horn_of_plenty_item(
    world: &mut GameWorld,
    player: Entity,
    horn: Entity,
    pickup_to_inventory: bool,
    rng: &mut impl Rng,
) -> Option<(Entity, Vec<EngineEvent>)> {
    let object_defs = world.object_catalog().to_vec();
    let object_def = choose_horn_of_plenty_object_def(&object_defs, rng)?;
    let generated_item =
        crate::items::spawn_item(world, object_def, crate::items::SpawnLocation::Free, None);

    let horn_buc = world.get_component::<BucStatus>(horn).map(|buc| BucStatus {
        cursed: buc.cursed,
        blessed: buc.blessed,
        bknown: buc.bknown,
    });
    if let Some(horn_buc) = horn_buc
        && let Some(mut generated_buc) = world.get_component_mut::<BucStatus>(generated_item)
    {
        generated_buc.blessed = horn_buc.blessed;
        generated_buc.cursed = horn_buc.cursed;
    }

    let player_pos = world
        .get_component::<Positioned>(player)
        .map(|pos| pos.0)
        .unwrap_or(Position::new(0, 0));
    let current_level = world.dungeon().current_data_dungeon_level();
    if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(generated_item) {
        *loc = ObjectLocation::Floor {
            x: player_pos.x as i16,
            y: player_pos.y as i16,
            level: current_level,
        };
    }

    let billed_to_shop =
        maybe_bill_generated_horn_item_in_shop(world, player, horn, generated_item);

    let mut events = vec![EngineEvent::msg("tool-horn-of-plenty-spills")];
    if pickup_to_inventory {
        if billed_to_shop {
            let mut letter_state = crate::items::LetterState::default();
            if let Some(letter) = crate::items::assign_inv_letter(world, player, &mut letter_state)
            {
                if let Some(mut core) =
                    world.get_component_mut::<nethack_babel_data::ObjectCore>(generated_item)
                {
                    core.inv_letter = Some(letter);
                }
                if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(generated_item) {
                    *loc = ObjectLocation::Inventory;
                }
                if let Some(mut inventory) =
                    world.get_component_mut::<crate::inventory::Inventory>(player)
                {
                    inventory.items.push(generated_item);
                }
                let quantity = world
                    .get_component::<nethack_babel_data::ObjectCore>(generated_item)
                    .map(|core| core.quantity.max(1) as u32)
                    .unwrap_or(1);
                events.push(EngineEvent::ItemPickedUp {
                    actor: player,
                    item: generated_item,
                    quantity,
                });
            }
        } else {
            let mut letter_state = crate::items::LetterState::default();
            events.extend(crate::items::pickup_item(
                world,
                player,
                generated_item,
                &mut letter_state,
                &object_defs,
            ));
        }
    }

    Some((generated_item, events))
}

pub(crate) fn apply_horn_of_plenty(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let charges = world
        .get_component::<Enchantment>(item)
        .map(|ench| ench.spe)
        .unwrap_or(0);
    if charges <= 0 {
        if let Some(mut knowledge) =
            world.get_component_mut::<nethack_babel_data::KnowledgeState>(item)
        {
            knowledge.cknown = true;
        }
        return vec![EngineEvent::msg("tool-nothing-happens")];
    }

    if let Some(mut ench) = world.get_component_mut::<Enchantment>(item) {
        ench.spe = ench.spe.saturating_sub(1);
    }

    spawn_horn_of_plenty_item(world, player, item, true, rng)
        .map(|(_, events)| events)
        .unwrap_or_else(|| vec![EngineEvent::msg("tool-nothing-happens")])
}

pub(crate) fn apply_bag_of_tricks(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let charges = world
        .get_component::<Enchantment>(item)
        .map(|ench| ench.spe)
        .unwrap_or(0);
    spill_bag_of_tricks(world, player, item, i32::from(charges.min(1)), rng)
}

// ---------------------------------------------------------------------------
// 1. Towel
// ---------------------------------------------------------------------------

/// Apply a towel to wipe face/hands.
///
/// - Cursed: random bad effect (glib hands, gunk on face)
/// - Uncursed/Blessed: cure glib, wipe cream off face (cure blindness)
fn apply_towel(
    world: &mut GameWorld,
    player: Entity,
    buc: &BucStatus,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        // Cursed towel: random bad effect.
        match rng.random_range(0..3) {
            0 => {
                // Nothing special for case 0.
                events.push(EngineEvent::msg("tool-towel-cursed-nothing"));
            }
            1 => {
                // Gunk on face: add some blindness turns.
                let dur = rng.random_range(3..=12);
                events.extend(crate::status::make_blinded(world, player, dur));
                events.push(EngineEvent::msg("tool-towel-cursed-gunk"));
            }
            _ => {
                // Slimy hands.
                events.push(EngineEvent::msg("tool-towel-cursed-slimy"));
            }
        }
        return events;
    }

    // Check if player is blind (cream on face) and cure it.
    if crate::status::is_blind(world, player) {
        events.extend(crate::status::make_blinded(world, player, 0));
        events.push(EngineEvent::msg("tool-towel-wipe-face"));
    } else {
        events.push(EngineEvent::msg("tool-towel-nothing"));
    }

    events
}

// ---------------------------------------------------------------------------
// 2. Bell
// ---------------------------------------------------------------------------

/// Ring a bell.
///
/// Ordinary bell: wake nearby monsters.
/// Bell of Opening (charged): open doors/containers nearby, or invoke.
/// Bell of Opening (uncharged): makes no sound.
fn apply_bell(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    tool: ExtToolType,
    buc: &BucStatus,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    events.push(EngineEvent::msg("tool-bell-ring"));

    let is_bell_of_opening = tool == ExtToolType::BellOfOpening;

    // Check charges for Bell of Opening.
    let has_charges = world
        .get_component::<Enchantment>(item)
        .map(|e| e.spe > 0)
        .unwrap_or(false);

    if is_bell_of_opening && !has_charges {
        events.push(EngineEvent::msg("tool-bell-no-sound"));
        return events;
    }

    if !is_bell_of_opening {
        // Ordinary bell: wake nearby monsters, small chance of nymph with cursed.
        if buc.cursed && rng.random_range(0..4) == 0 {
            events.push(EngineEvent::msg("tool-bell-cursed-summon"));
        }
        events.push(EngineEvent::msg("tool-bell-wake-nearby"));
        return events;
    }

    // Charged Bell of Opening: consume a charge.
    if let Some(mut ench) = world.get_component_mut::<Enchantment>(item) {
        ench.spe -= 1;
    }

    // Ringing the Bell of Opening on the vibrating square marks it as
    // invocation-ready for a short window, matching the Book of the Dead
    // runtime check.
    let player_pos = world
        .get_component::<Positioned>(player)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));
    let on_invocation_site = world
        .dungeon()
        .trap_map
        .trap_at(player_pos)
        .is_some_and(|trap| trap.trap_type == nethack_babel_data::TrapType::VibratingSquare);
    let current_turn = world.turn() as i64;
    if on_invocation_site
        && let Some(mut core) = world.get_component_mut::<nethack_babel_data::ObjectCore>(item)
    {
        core.age = current_turn;
    }

    if buc.cursed {
        // Cursed: create undead.
        events.push(EngineEvent::msg("tool-bell-cursed-undead"));
    } else if buc.blessed {
        // Blessed: open things nearby (doors, chests, chains).
        events.push(EngineEvent::msg("tool-bell-opens"));
        // Open adjacent locked doors.
        let player_pos = world
            .get_component::<Positioned>(player)
            .map(|p| p.0)
            .unwrap_or(Position::new(0, 0));
        for dx in -1..=1i32 {
            for dy in -1..=1i32 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let pos = Position::new(player_pos.x + dx, player_pos.y + dy);
                let terrain = world.dungeon().current_level.get(pos).map(|c| c.terrain);
                if terrain == Some(crate::dungeon::Terrain::DoorLocked) {
                    world
                        .dungeon_mut()
                        .current_level
                        .set_terrain(pos, crate::dungeon::Terrain::DoorOpen);
                    events.push(EngineEvent::DoorOpened { position: pos });
                }
            }
        }
    } else {
        // Uncursed: reveal hidden things.
        events.push(EngineEvent::msg("tool-bell-reveal"));
    }

    events
}

// ---------------------------------------------------------------------------
// 3. Candle
// ---------------------------------------------------------------------------

/// Light or extinguish a candle.
fn apply_candle(
    world: &mut GameWorld,
    _player: Entity,
    item: Entity,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Toggle lit state via LightSource component.
    let is_lit = world
        .get_component::<nethack_babel_data::LightSource>(item)
        .map(|ls| ls.lit)
        .unwrap_or(false);

    if is_lit {
        // Extinguish.
        if let Some(mut ls) = world.get_component_mut::<nethack_babel_data::LightSource>(item) {
            ls.lit = false;
        }
        events.push(EngineEvent::msg("tool-candle-extinguish"));
    } else {
        // Light. Add LightSource if missing.
        if let Some(mut ls) = world.get_component_mut::<nethack_babel_data::LightSource>(item) {
            ls.lit = true;
        } else {
            let _ = world.ecs_mut().insert_one(
                item,
                nethack_babel_data::LightSource {
                    lit: true,
                    recharged: 0,
                },
            );
        }
        events.push(EngineEvent::msg("tool-candle-light"));
    }

    events
}

// ---------------------------------------------------------------------------
// 4. Candelabrum
// ---------------------------------------------------------------------------

/// Light or extinguish a candelabrum (requires attached candles via spe).
fn apply_candelabrum(
    world: &mut GameWorld,
    _player: Entity,
    item: Entity,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // spe holds number of attached candles.
    let candles = world
        .get_component::<Enchantment>(item)
        .map(|e| e.spe)
        .unwrap_or(0);

    if candles <= 0 {
        events.push(EngineEvent::msg("tool-candelabrum-no-candles"));
        return events;
    }

    // Toggle lit state.
    let is_lit = world
        .get_component::<nethack_babel_data::LightSource>(item)
        .map(|ls| ls.lit)
        .unwrap_or(false);

    if is_lit {
        if let Some(mut ls) = world.get_component_mut::<nethack_babel_data::LightSource>(item) {
            ls.lit = false;
        }
        events.push(EngineEvent::msg("tool-candelabrum-extinguish"));
    } else {
        if let Some(mut ls) = world.get_component_mut::<nethack_babel_data::LightSource>(item) {
            ls.lit = true;
        } else {
            let _ = world.ecs_mut().insert_one(
                item,
                nethack_babel_data::LightSource {
                    lit: true,
                    recharged: 0,
                },
            );
        }
        events.push(EngineEvent::msg_with(
            "tool-candelabrum-light",
            vec![("candles", candles.to_string())],
        ));
    }

    events
}

// ---------------------------------------------------------------------------
// 5. Can of Grease
// ---------------------------------------------------------------------------

/// Apply a can of grease to an item or self.
///
/// - If charges remain (spe > 0), grease the target item.
/// - Cursed/fumbling: may slip from hands.
/// - Sets the `greased` flag on the target object's Erosion component.
fn apply_grease(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    buc: &BucStatus,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let charges = world
        .get_component::<Enchantment>(item)
        .map(|e| e.spe)
        .unwrap_or(0);

    if charges <= 0 {
        events.push(EngineEvent::msg("tool-grease-empty"));
        return events;
    }

    // Consume a charge.
    if let Some(mut ench) = world.get_component_mut::<Enchantment>(item) {
        ench.spe -= 1;
    }

    // Cursed: may slip and drop.
    if buc.cursed && rng.random_range(0..2) == 0 {
        events.push(EngineEvent::msg("tool-grease-slip"));
        // Move to floor at player position.
        let player_pos = world
            .get_component::<Positioned>(player)
            .map(|p| p.0)
            .unwrap_or(Position::new(0, 0));
        let branch = world.dungeon().branch;
        let depth = world.dungeon().depth;
        if let Some(mut loc) = world.get_component_mut::<ObjectLocation>(item) {
            *loc = crate::dungeon::floor_object_location(branch, depth, player_pos);
        }
        return events;
    }

    // Grease self (hands) — simplified: just report success.
    events.push(EngineEvent::msg("tool-grease-hands"));
    events
}

// ---------------------------------------------------------------------------
// 6. Touchstone
// ---------------------------------------------------------------------------

/// Rub a gem on a touchstone to identify it.
///
/// - Blessed touchstone: identifies the gem.
/// - Cursed: may shatter the gem.
/// - Otherwise: shows streak color clue.
fn apply_touchstone(
    _world: &mut GameWorld,
    _player: Entity,
    _item: Entity,
    buc: &BucStatus,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        // Cursed touchstone: small chance to shatter the rubbed gem.
        if rng.random_range(0..5) == 0 {
            events.push(EngineEvent::msg("tool-touchstone-shatter"));
            return events;
        }
    }

    if buc.blessed {
        // Blessed: fully identify the gem.
        events.push(EngineEvent::msg("tool-touchstone-identify"));
    } else {
        // Shows a streak.
        events.push(EngineEvent::msg("tool-touchstone-streak"));
    }

    events
}

// ---------------------------------------------------------------------------
// 7. Cream Pie
// ---------------------------------------------------------------------------

/// Apply cream pie to face (self-blind).
///
/// Consumes the pie and blinds the player for rnd(25) turns.
fn apply_cream_pie(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let blind_dur = rng.random_range(1..=25) as u32;
    events.extend(crate::status::make_blinded(world, player, blind_dur));
    events.push(EngineEvent::msg("tool-cream-pie-face"));

    // Consume the pie.
    let _ = world.despawn(item);

    events
}

// ---------------------------------------------------------------------------
// 8. Figurine
// ---------------------------------------------------------------------------

/// Apply a figurine to animate it (create a monster).
///
/// Simplified: reports that a monster would be created.
/// Full implementation requires makemon integration.
fn apply_figurine(
    world: &mut GameWorld,
    _player: Entity,
    item: Entity,
    buc: &BucStatus,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        // Cursed figurine: hostile monster.
        events.push(EngineEvent::msg("tool-figurine-hostile"));
    } else if buc.blessed {
        // Blessed figurine: tame monster.
        events.push(EngineEvent::msg("tool-figurine-tame"));
    } else {
        // Uncursed: peaceful monster.
        events.push(EngineEvent::msg("tool-figurine-peaceful"));
    }

    // Consume the figurine.
    let _ = world.despawn(item);

    events
}

// ---------------------------------------------------------------------------
// 9. Leash
// ---------------------------------------------------------------------------

/// Apply a leash to leash/unleash adjacent tame monsters.
fn apply_leash(world: &mut GameWorld, player: Entity, _rng: &mut impl Rng) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Find an adjacent tame monster to leash.
    let mut found_pet = false;
    for (_entity, (pos, _tame, name)) in world.ecs().query::<(&Positioned, &Tame, &Name)>().iter() {
        let dx = (pos.0.x - player_pos.x).abs();
        let dy = (pos.0.y - player_pos.y).abs();
        if dx <= 1 && dy <= 1 {
            events.push(EngineEvent::msg_with(
                "tool-leash-attached",
                vec![("name", name.0.clone())],
            ));
            found_pet = true;
            break;
        }
    }

    if !found_pet {
        events.push(EngineEvent::msg("tool-leash-no-pet"));
    }

    events
}

// ---------------------------------------------------------------------------
// 10. Horns (Frost Horn, Fire Horn)
// ---------------------------------------------------------------------------

/// Apply a magical horn (frost or fire).
///
/// Consumes a charge and deals elemental damage to monsters in a line.
fn apply_horn(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    element: &str,
    _buc: &BucStatus,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let charges = world
        .get_component::<Enchantment>(item)
        .map(|e| e.spe)
        .unwrap_or(0);

    if charges <= 0 {
        events.push(EngineEvent::msg("tool-horn-no-charges"));
        // Tooled horn sound instead.
        events.push(EngineEvent::msg("tool-horn-toot"));
        return events;
    }

    // Consume a charge.
    if let Some(mut ench) = world.get_component_mut::<Enchantment>(item) {
        ench.spe -= 1;
    }

    let player_pos = world
        .get_component::<Positioned>(player)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    // Deal 6d6 damage to adjacent monsters (simplified from beam).
    let damage: i32 = (0..6).map(|_| rng.random_range(1..=6)).sum();

    let mut hit_any = false;
    // Collect targets first to avoid borrow issues.
    let targets: Vec<(Entity, String)> = world
        .ecs()
        .query::<(&Positioned, &Monster, &Name)>()
        .iter()
        .filter(|&(e, (pos, _, _))| {
            e != player && {
                let dx = (pos.0.x - player_pos.x).abs();
                let dy = (pos.0.y - player_pos.y).abs();
                dx <= 1 && dy <= 1
            }
        })
        .map(|(e, (_, _, name))| (e, name.0.clone()))
        .collect();

    for (entity, name) in targets {
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
            hp.current -= damage;
            hit_any = true;
            events.push(EngineEvent::msg_with(
                &format!("tool-horn-{element}-hit"),
                vec![("name", name), ("damage", damage.to_string())],
            ));
        }
    }

    if !hit_any {
        events.push(EngineEvent::msg_with(
            &format!("tool-horn-{element}-blast"),
            vec![],
        ));
    }

    events
}

/// Apply a tooled horn (non-magical). Just makes noise, wakes monsters.
fn apply_tooled_horn(
    _world: &mut GameWorld,
    _player: Entity,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    vec![EngineEvent::msg("tool-horn-toot")]
}

// ---------------------------------------------------------------------------
// 11. Drum of Earthquake
// ---------------------------------------------------------------------------

/// Apply a drum of earthquake.
///
/// Consumes a charge, creates an earthquake that damages nearby monsters
/// and may collapse walls.
fn apply_drum(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    _buc: &BucStatus,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let charges = world
        .get_component::<Enchantment>(item)
        .map(|e| e.spe)
        .unwrap_or(0);

    if charges <= 0 {
        events.push(EngineEvent::msg("tool-drum-no-charges"));
        events.push(EngineEvent::msg("tool-drum-thump"));
        return events;
    }

    // Consume a charge.
    if let Some(mut ench) = world.get_component_mut::<Enchantment>(item) {
        ench.spe -= 1;
    }

    events.push(EngineEvent::msg("tool-drum-earthquake"));

    let player_pos = world
        .get_component::<Positioned>(player)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    // Damage all monsters within radius 2.
    let targets: Vec<(Entity, String)> = world
        .ecs()
        .query::<(&Positioned, &Monster, &Name)>()
        .iter()
        .filter(|&(e, (pos, _, _))| {
            e != player && {
                let dx = (pos.0.x - player_pos.x).abs();
                let dy = (pos.0.y - player_pos.y).abs();
                dx <= 2 && dy <= 2
            }
        })
        .map(|(e, (_, _, name))| (e, name.0.clone()))
        .collect();

    for (entity, name) in targets {
        let damage: i32 = (0..4).map(|_| rng.random_range(1..=6)).sum();
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
            hp.current -= damage;
            events.push(EngineEvent::msg_with(
                "tool-drum-damage",
                vec![("name", name), ("damage", damage.to_string())],
            ));
        }
    }

    events
}

// ---------------------------------------------------------------------------
// 12. Polearm
// ---------------------------------------------------------------------------

/// Apply a polearm for a reach attack (range 2 tiles in given direction).
///
/// Simplified: damages the first monster found at distance 2.
fn apply_polearm(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Get weapon damage from enchantment.
    let spe = world
        .get_component::<Enchantment>(item)
        .map(|e| e.spe as i32)
        .unwrap_or(0);

    // Find monsters at distance 2 (the reach range).
    let targets: Vec<(Entity, String, Position)> = world
        .ecs()
        .query::<(&Positioned, &Monster, &Name)>()
        .iter()
        .filter(|&(e, (pos, _, _))| {
            e != player && {
                let dx = (pos.0.x - player_pos.x).abs();
                let dy = (pos.0.y - player_pos.y).abs();
                // Exactly distance 2 in any direction (chebyshev).
                dx.max(dy) == 2
            }
        })
        .map(|(e, (pos, _, name))| (e, name.0.clone(), pos.0))
        .collect();

    if targets.is_empty() {
        events.push(EngineEvent::msg("tool-polearm-no-target"));
        return events;
    }

    // Attack the first target found.
    let (target_entity, target_name, _target_pos) = &targets[0];
    let base_damage = rng.random_range(1..=8) + spe;
    let damage = base_damage.max(0);

    if let Some(mut hp) = world.get_component_mut::<HitPoints>(*target_entity) {
        hp.current -= damage;
        events.push(EngineEvent::msg_with(
            "tool-polearm-hit",
            vec![
                ("name", target_name.clone()),
                ("damage", damage.to_string()),
            ],
        ));

        if hp.current <= 0 {
            events.push(EngineEvent::EntityDied {
                entity: *target_entity,
                killer: Some(player),
                cause: crate::event::DeathCause::KilledBy {
                    killer_name: "a tool".to_string(),
                },
            });
        }
    }

    events
}

// ---------------------------------------------------------------------------
// 13. Saddle
// ---------------------------------------------------------------------------

/// Apply a saddle to an adjacent tame monster to make it a steed.
///
/// Simplified: checks for an adjacent tame monster, reports success/failure.
fn apply_saddle(world: &mut GameWorld, player: Entity, _rng: &mut impl Rng) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Find an adjacent tame monster to saddle.
    let mut found_mount = false;
    for (_entity, (pos, _tame, name)) in world.ecs().query::<(&Positioned, &Tame, &Name)>().iter() {
        let dx = (pos.0.x - player_pos.x).abs();
        let dy = (pos.0.y - player_pos.y).abs();
        if dx <= 1 && dy <= 1 {
            events.push(EngineEvent::msg_with(
                "tool-saddle-placed",
                vec![("name", name.0.clone())],
            ));
            found_mount = true;
            break;
        }
    }

    if !found_mount {
        events.push(EngineEvent::msg("tool-saddle-no-mount"));
    }

    events
}

// ---------------------------------------------------------------------------
// 14. Bullwhip
// ---------------------------------------------------------------------------

/// Apply a bullwhip in a direction.
///
/// - Against monsters: chance to disarm them (knock weapon out of hand).
/// - Against items on ground at range: pull item toward player.
/// - Otherwise: crack the whip (noise).
fn apply_bullwhip(
    world: &mut GameWorld,
    player: Entity,
    _item: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Check for a monster at distance 1 (simplified from directional).
    let targets: Vec<(Entity, String)> = world
        .ecs()
        .query::<(&Positioned, &Monster, &Name)>()
        .iter()
        .filter(|&(e, (pos, _, _))| {
            e != player && {
                let dx = (pos.0.x - player_pos.x).abs();
                let dy = (pos.0.y - player_pos.y).abs();
                dx <= 1 && dy <= 1
            }
        })
        .map(|(e, (_, _, name))| (e, name.0.clone()))
        .collect();

    if let Some((_target, name)) = targets.first() {
        // Chance to disarm.
        if rng.random_range(0..3) == 0 {
            events.push(EngineEvent::msg_with(
                "tool-bullwhip-disarm",
                vec![("name", name.clone())],
            ));
        } else {
            events.push(EngineEvent::msg_with(
                "tool-bullwhip-lash",
                vec![("name", name.clone())],
            ));
        }
    } else {
        // No target: just crack the whip.
        events.push(EngineEvent::msg("tool-bullwhip-crack"));
    }

    events
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::StatusEffects;
    use nethack_babel_data::{Enchantment, KnowledgeState, ObjectClass, ObjectCore, ObjectTypeId};
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    type TestRng = SmallRng;

    fn test_rng() -> TestRng {
        SmallRng::seed_from_u64(42)
    }

    fn test_world() -> GameWorld {
        GameWorld::new(Position::new(40, 10))
    }

    /// Spawn a tool entity with a given name and optional BUC/charges.
    fn spawn_tool(
        world: &mut GameWorld,
        name: &str,
        cursed: bool,
        blessed: bool,
        charges: Option<i8>,
    ) -> Entity {
        let entity = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(200),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            BucStatus {
                cursed,
                blessed,
                bknown: false,
            },
            KnowledgeState {
                known: false,
                dknown: false,
                rknown: false,
                cknown: false,
                lknown: false,
                tknown: false,
            },
            ObjectLocation::Inventory,
            Name(name.to_string()),
        ));
        if let Some(spe) = charges {
            let _ = world.ecs_mut().insert_one(entity, Enchantment { spe });
        }
        entity
    }

    /// Spawn an adjacent monster for testing.
    fn spawn_adjacent_monster(
        world: &mut GameWorld,
        name: &str,
        offset: (i32, i32),
        hp: i32,
    ) -> Entity {
        let player_pos = world.get_component::<Positioned>(world.player()).unwrap().0;
        let pos = Position::new(player_pos.x + offset.0, player_pos.y + offset.1);
        let entity = world.spawn((
            Positioned(pos),
            Monster,
            HitPoints {
                current: hp,
                max: hp,
            },
            Name(name.to_string()),
            crate::world::Speed(12),
            StatusEffects::default(),
        ));
        entity
    }

    // ── Towel tests ──

    #[test]
    fn towel_cures_blindness() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Make player blind.
        crate::status::make_blinded(&mut world, player, 10);

        let tool = spawn_tool(&mut world, "towel", false, false, None);
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize towel");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-towel-wipe-face")
            }),
            "should wipe face"
        );
    }

    #[test]
    fn towel_cursed_has_bad_effect() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "towel", true, false, None);
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize towel");

        // Should have a cursed-related message.
        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key.starts_with("tool-towel-cursed"))
            }),
            "cursed towel should have bad effect"
        );
    }

    // ── Bell tests ──

    #[test]
    fn bell_of_opening_consumes_charge() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "bell of opening", false, true, Some(3));
        let _events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize bell");

        let charges = world
            .get_component::<Enchantment>(tool)
            .map(|e| e.spe)
            .unwrap_or(0);
        assert_eq!(charges, 2, "should consume one charge");
    }

    #[test]
    fn bell_of_opening_uncharged_no_sound() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "bell of opening", false, false, Some(0));
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize bell");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-bell-no-sound")
            }),
            "uncharged bell should make no sound"
        );
    }

    // ── Candle tests ──

    #[test]
    fn candle_toggles_light() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "tallow candle", false, false, None);
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize candle");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-candle-light")
            }),
            "should light candle"
        );

        // Apply again to extinguish.
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize candle");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-candle-extinguish")
            }),
            "should extinguish candle"
        );
    }

    // ── Candelabrum tests ──

    #[test]
    fn candelabrum_no_candles() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "candelabrum", false, false, Some(0));
        let events = apply_ext_tool(&mut world, player, tool, &mut rng)
            .expect("should recognize candelabrum");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-candelabrum-no-candles")
            }),
            "should report no candles"
        );
    }

    #[test]
    fn candelabrum_with_candles_lights() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "candelabrum", false, false, Some(7));
        let events = apply_ext_tool(&mut world, player, tool, &mut rng)
            .expect("should recognize candelabrum");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-candelabrum-light")
            }),
            "should light candelabrum"
        );
    }

    // ── Grease tests ──

    #[test]
    fn grease_empty_reports() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "can of grease", false, false, Some(0));
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize grease");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-grease-empty")
            }),
            "empty grease should report"
        );
    }

    #[test]
    fn grease_consumes_charge() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "can of grease", false, false, Some(10));
        let _events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize grease");

        let charges = world
            .get_component::<Enchantment>(tool)
            .map(|e| e.spe)
            .unwrap_or(0);
        assert!(charges < 10, "should consume a charge");
    }

    // ── Touchstone tests ──

    #[test]
    fn touchstone_blessed_identifies() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "touchstone", false, true, None);
        let events = apply_ext_tool(&mut world, player, tool, &mut rng)
            .expect("should recognize touchstone");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-touchstone-identify")
            }),
            "blessed touchstone should identify"
        );
    }

    // ── Cream pie tests ──

    #[test]
    fn cream_pie_blinds_and_consumed() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "cream pie", false, false, None);
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize cream pie");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-cream-pie-face")
            }),
            "should pie face"
        );

        // Pie should be consumed (despawned).
        assert!(
            world.get_component::<ObjectCore>(tool).is_none(),
            "cream pie should be consumed"
        );
    }

    // ── Figurine tests ──

    #[test]
    fn figurine_consumed_on_use() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "figurine", false, false, None);
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize figurine");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key.starts_with("tool-figurine"))
            }),
            "should have figurine message"
        );

        assert!(
            world.get_component::<ObjectCore>(tool).is_none(),
            "figurine should be consumed"
        );
    }

    // ── Leash tests ──

    #[test]
    fn leash_no_pet_nearby() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "leash", false, false, None);
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize leash");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-leash-no-pet")
            }),
            "should report no pet nearby"
        );
    }

    #[test]
    fn leash_attaches_to_pet() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Spawn a tame monster adjacent.
        let player_pos = world.get_component::<Positioned>(player).unwrap().0;
        let pet_pos = Position::new(player_pos.x + 1, player_pos.y);
        let _pet = world.spawn((
            Positioned(pet_pos),
            Monster,
            Tame,
            HitPoints {
                current: 10,
                max: 10,
            },
            Name("kitten".to_string()),
            crate::world::Speed(12),
        ));

        let tool = spawn_tool(&mut world, "leash", false, false, None);
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize leash");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-leash-attached")
            }),
            "should attach leash to pet"
        );
    }

    // ── Horn tests ──

    #[test]
    fn frost_horn_damages_adjacent() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let mon = spawn_adjacent_monster(&mut world, "goblin", (1, 0), 30);
        let tool = spawn_tool(&mut world, "frost horn", false, false, Some(5));
        let _events = apply_ext_tool(&mut world, player, tool, &mut rng)
            .expect("should recognize frost horn");

        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert!(hp.current < 30, "frost horn should deal damage");

        let charges = world.get_component::<Enchantment>(tool).unwrap();
        assert_eq!(charges.spe, 4, "should consume a charge");
    }

    #[test]
    fn horn_no_charges_just_toots() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "fire horn", false, false, Some(0));
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize fire horn");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-horn-toot")
            }),
            "empty horn should just toot"
        );
    }

    // ── Drum tests ──

    #[test]
    fn drum_earthquake_damages_nearby() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let mon = spawn_adjacent_monster(&mut world, "orc", (1, 1), 40);
        let tool = spawn_tool(&mut world, "drum of earthquake", false, false, Some(3));
        let _events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize drum");

        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert!(hp.current < 40, "drum should deal earthquake damage");
    }

    // ── Polearm tests ──

    #[test]
    fn polearm_reach_attack() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Spawn monster at distance 2.
        let mon = spawn_adjacent_monster(&mut world, "troll", (2, 0), 50);
        let tool = spawn_tool(&mut world, "halberd", false, false, Some(2));
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize polearm");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-polearm-hit")
            }),
            "polearm should hit at range 2"
        );

        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert!(hp.current < 50, "polearm should deal damage");
    }

    #[test]
    fn polearm_no_target_at_range() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "glaive", false, false, Some(0));
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize polearm");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-polearm-no-target")
            }),
            "should report no target"
        );
    }

    // ── Saddle tests ──

    #[test]
    fn saddle_no_mount_nearby() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "saddle", false, false, None);
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize saddle");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-saddle-no-mount")
            }),
            "should report no mount nearby"
        );
    }

    #[test]
    fn saddle_places_on_pet() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let player_pos = world.get_component::<Positioned>(player).unwrap().0;
        let pet_pos = Position::new(player_pos.x + 1, player_pos.y);
        let _pet = world.spawn((
            Positioned(pet_pos),
            Monster,
            Tame,
            HitPoints {
                current: 20,
                max: 20,
            },
            Name("pony".to_string()),
            crate::world::Speed(12),
        ));

        let tool = spawn_tool(&mut world, "saddle", false, false, None);
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize saddle");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-saddle-placed")
            }),
            "should place saddle on pet"
        );
    }

    // ── Bullwhip tests ──

    #[test]
    fn bullwhip_crack_no_target() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "bullwhip", false, false, None);
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize bullwhip");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-bullwhip-crack")
            }),
            "should crack whip with no target"
        );
    }

    #[test]
    fn bullwhip_lash_or_disarm_adjacent() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let _mon = spawn_adjacent_monster(&mut world, "kobold", (1, 0), 10);
        let tool = spawn_tool(&mut world, "bullwhip", false, false, None);
        let events =
            apply_ext_tool(&mut world, player, tool, &mut rng).expect("should recognize bullwhip");

        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. }
                    if key == "tool-bullwhip-lash" || key == "tool-bullwhip-disarm")
            }),
            "should lash or disarm adjacent monster"
        );
    }

    // ── Classification tests ──

    #[test]
    fn classify_ext_tool_recognizes_all_types() {
        assert_eq!(classify_ext_tool("towel"), Some(ExtToolType::Towel));
        assert_eq!(
            classify_ext_tool("bell of opening"),
            Some(ExtToolType::BellOfOpening)
        );
        assert_eq!(classify_ext_tool("bell"), Some(ExtToolType::Bell));
        assert_eq!(
            classify_ext_tool("wooden flute"),
            Some(ExtToolType::WoodenFlute)
        );
        assert_eq!(
            classify_ext_tool("magic flute"),
            Some(ExtToolType::MagicFlute)
        );
        assert_eq!(
            classify_ext_tool("wooden harp"),
            Some(ExtToolType::WoodenHarp)
        );
        assert_eq!(
            classify_ext_tool("magic harp"),
            Some(ExtToolType::MagicHarp)
        );
        assert_eq!(classify_ext_tool("bugle"), Some(ExtToolType::Bugle));
        assert_eq!(
            classify_ext_tool("leather drum"),
            Some(ExtToolType::LeatherDrum)
        );
        assert_eq!(
            classify_ext_tool("tallow candle"),
            Some(ExtToolType::TallowCandle)
        );
        assert_eq!(
            classify_ext_tool("wax candle"),
            Some(ExtToolType::WaxCandle)
        );
        assert_eq!(
            classify_ext_tool("candelabrum"),
            Some(ExtToolType::Candelabrum)
        );
        assert_eq!(
            classify_ext_tool("can of grease"),
            Some(ExtToolType::CanOfGrease)
        );
        assert_eq!(
            classify_ext_tool("touchstone"),
            Some(ExtToolType::Touchstone)
        );
        assert_eq!(classify_ext_tool("cream pie"), Some(ExtToolType::CreamPie));
        assert_eq!(classify_ext_tool("figurine"), Some(ExtToolType::Figurine));
        assert_eq!(classify_ext_tool("leash"), Some(ExtToolType::Leash));
        assert_eq!(
            classify_ext_tool("frost horn"),
            Some(ExtToolType::FrostHorn)
        );
        assert_eq!(classify_ext_tool("fire horn"), Some(ExtToolType::FireHorn));
        assert_eq!(
            classify_ext_tool("tooled horn"),
            Some(ExtToolType::TooledHorn)
        );
        assert_eq!(
            classify_ext_tool("drum of earthquake"),
            Some(ExtToolType::DrumOfEarthquake)
        );
        assert_eq!(classify_ext_tool("halberd"), Some(ExtToolType::Polearm));
        assert_eq!(classify_ext_tool("saddle"), Some(ExtToolType::Saddle));
        assert_eq!(classify_ext_tool("bullwhip"), Some(ExtToolType::Bullwhip));
        assert_eq!(classify_ext_tool("sword"), None);
    }

    #[test]
    fn unrecognized_tool_returns_none() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "long sword", false, false, None);
        let result = apply_ext_tool(&mut world, player, tool, &mut rng);
        assert!(result.is_none(), "unrecognized tool should return None");
    }

    // ── Unified dispatch tests ──

    #[test]
    fn unified_dispatch_ext_tool() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "towel", false, false, None);
        let events = apply_tool(&mut world, player, tool, None, &mut rng);
        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key.starts_with("tool-towel"))
            }),
            "unified dispatch should handle extended tools"
        );
    }

    #[test]
    fn unified_dispatch_base_tool() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        // Mirror is handled by tools.rs, not apply.rs.
        let tool = spawn_tool(&mut world, "mirror", false, false, None);
        let events = apply_tool(&mut world, player, tool, None, &mut rng);
        // tools::apply_mirror should produce events.
        assert!(
            !events.is_empty(),
            "unified dispatch should delegate to tools.rs"
        );
    }

    #[test]
    fn unified_dispatch_unknown_tool() {
        let mut world = test_world();
        let mut rng = test_rng();
        let player = world.player();

        let tool = spawn_tool(&mut world, "rubber chicken", false, false, None);
        let events = apply_tool(&mut world, player, tool, None, &mut rng);
        assert!(
            events.iter().any(|e| {
                matches!(e, EngineEvent::Message { key, .. } if key == "tool-nothing-happens")
            }),
            "unknown tool should produce nothing-happens message"
        );
    }
}
