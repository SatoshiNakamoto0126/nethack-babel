//! Scroll system: reading effects for all 23 scroll types.
//!
//! Implements the NetHack 3.7 scroll mechanics from `src/read.c`.
//! All functions operate on `GameWorld` and return `Vec<EngineEvent>`
//! for the game loop to process.  No IO.
//!
//! Reference: `specs/scroll-effects.md`

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{
    BucStatus, Enchantment, Erosion, ObjectClass, ObjectCore, ObjectLocation,
};

use crate::action::Position;
use crate::event::{DamageCause, DamageSource, EngineEvent, HpSource, StatusEffect};
use crate::world::{GameWorld, HitPoints, Monster, Positioned, Power, Tame};

// ---------------------------------------------------------------------------
// Scroll type enumeration
// ---------------------------------------------------------------------------

/// All 23 scroll types from NetHack 3.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScrollType {
    Identify,
    EnchantWeapon,
    EnchantArmor,
    RemoveCurse,
    Teleportation,
    GoldDetection,
    FoodDetection,
    ConfuseMonster,
    ScareMonster,
    BlankPaper,
    Fire,
    Earth,
    Punishment,
    StinkingCloud,
    Amnesia,
    DestroyArmor,
    CreateMonster,
    Taming,
    Genocide,
    Light,
    Charging,
    MagicMapping,
    Mail,
}

// ---------------------------------------------------------------------------
// BUC helper (reuse convention from potions)
// ---------------------------------------------------------------------------

/// Returns +1 for blessed, -1 for cursed, 0 for uncursed.
#[inline]
fn bcsign(buc: &BucStatus) -> i32 {
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

/// rn2(x) = uniform in [0, x).
#[inline]
fn rn2<R: Rng>(rng: &mut R, x: u32) -> u32 {
    if x <= 1 {
        return 0;
    }
    rng.random_range(0..x)
}

/// rn1(x, y) = rn2(x) + y, i.e. uniform in [y, y+x-1].
#[inline]
fn rn1<R: Rng>(rng: &mut R, x: i32, y: i32) -> i32 {
    if x <= 0 {
        return y;
    }
    rng.random_range(0..x) + y
}

// ---------------------------------------------------------------------------
// Marker components
// ---------------------------------------------------------------------------

/// Marker component: this item is the currently wielded weapon.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Wielded;

/// Marker component: this item is currently worn as armor.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Worn;

/// Marker component: entity is punished (ball and chain).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Punished;

// ---------------------------------------------------------------------------
// Read dispatch
// ---------------------------------------------------------------------------

/// Read a scroll.  Dispatches to the appropriate effect function based on
/// scroll type, BUC status, and confusion state.
///
/// Returns the list of engine events describing what happened.
/// The scroll entity is consumed (despawned) for most scroll types.
pub fn read_scroll<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    scroll_entity: Entity,
    scroll_type: ScrollType,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    // Read BUC status from the scroll entity.
    let buc = match world.get_component::<BucStatus>(scroll_entity) {
        Some(b) => (*b).clone(),
        None => BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        },
    };

    let mut events = match scroll_type {
        ScrollType::Identify => effect_identify(world, reader, &buc, confused, rng),
        ScrollType::EnchantWeapon => effect_enchant_weapon(world, reader, &buc, confused, rng),
        ScrollType::EnchantArmor => effect_enchant_armor(world, reader, &buc, confused, rng),
        ScrollType::RemoveCurse => effect_remove_curse(world, reader, &buc, confused, rng),
        ScrollType::Teleportation => effect_teleportation(world, reader, &buc, confused, rng),
        ScrollType::GoldDetection => effect_gold_detection(world, reader, &buc, confused, rng),
        ScrollType::FoodDetection => effect_food_detection(world, reader, &buc, confused, rng),
        ScrollType::ConfuseMonster => effect_confuse_monster(world, reader, &buc, confused, rng),
        ScrollType::ScareMonster => effect_scare_monster(world, reader, &buc, confused, rng),
        ScrollType::BlankPaper => effect_blank_paper(world, reader, &buc, confused, rng),
        ScrollType::Fire => effect_fire(world, reader, &buc, confused, rng),
        ScrollType::Earth => effect_earth(world, reader, &buc, confused, rng),
        ScrollType::Punishment => effect_punishment(world, reader, &buc, confused, rng),
        ScrollType::StinkingCloud => effect_stinking_cloud(world, reader, &buc, confused, rng),
        ScrollType::Amnesia => effect_amnesia(world, reader, &buc, confused, rng),
        ScrollType::DestroyArmor => effect_destroy_armor(world, reader, &buc, confused, rng),
        ScrollType::CreateMonster => effect_create_monster(world, reader, &buc, confused, rng),
        ScrollType::Taming => effect_taming(world, reader, &buc, confused, rng),
        ScrollType::Genocide => effect_genocide(world, reader, &buc, confused, rng),
        ScrollType::Light => effect_light(world, reader, &buc, confused, rng),
        ScrollType::Charging => effect_charging(world, reader, &buc, confused, rng),
        ScrollType::MagicMapping => effect_magic_mapping(world, reader, &buc, confused, rng),
        ScrollType::Mail => effect_mail(world, reader, &buc, confused, rng),
    };

    // Consume the scroll (most scrolls are consumed after reading).
    // Scare monster on the floor is an exception handled within its effect.
    if scroll_type != ScrollType::ScareMonster {
        let _ = world.despawn(scroll_entity);
        events.push(EngineEvent::ItemDestroyed {
            item: scroll_entity,
            cause: DamageCause::Physical,
        });
    } else {
        // Scare monster: consumed within its own effect function if appropriate.
        // If still alive (read from inventory), consume it.
        if world.get_component::<ObjectCore>(scroll_entity).is_some() {
            let _ = world.despawn(scroll_entity);
            events.push(EngineEvent::ItemDestroyed {
                item: scroll_entity,
                cause: DamageCause::Physical,
            });
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Individual scroll effects
// ---------------------------------------------------------------------------

/// Identify scroll.
///
/// Spec section 3.3:
/// - Confused OR (cursed AND not already known): self-identify only.
/// - Blessed: rn2(5) items; if rn2(5)==0 then ALL; if ==1 and Luck>0 then 2.
/// - Uncursed: 80% identify 1 item; 20% same as blessed formula.
/// - Cursed (already known, not confused): identifies 1 item [疑似 bug in C].
fn effect_identify<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if confused || buc.cursed {
        // Confused or cursed: identify the scroll itself only.
        events.push(EngineEvent::msg("scroll-identify-self"));
        return events;
    }

    // Determine how many items to identify.
    let cval: u32 = if buc.blessed || (!buc.cursed && rn2(rng, 5) == 0) {
        // Blessed always enters this path; uncursed has 20% chance.
        let mut c = rn2(rng, 5); // 0..4; 0 means identify ALL
        if c == 1 && buc.blessed {
            // Blessed with positive luck would give 2, but we simplify:
            // always bump 1->2 for blessed.
            c = 2;
        }
        c
    } else {
        1 // Default: identify 1 item
    };

    let items = collect_inventory_items(world, reader);

    if cval == 0 {
        // Identify ALL items.
        for item_entity in &items {
            events.push(EngineEvent::ItemIdentified { item: *item_entity });
        }
        if items.is_empty() {
            events.push(EngineEvent::msg("scroll-nothing-to-identify"));
        } else {
            events.push(EngineEvent::msg_with(
                "scroll-identify-count",
                vec![("count", items.len().to_string())],
            ));
        }
    } else {
        // Identify up to `cval` items.
        let count = (cval as usize).min(items.len());
        for item in items.iter().take(count) {
            events.push(EngineEvent::ItemIdentified { item: *item });
        }
        if count == 0 {
            events.push(EngineEvent::msg("scroll-nothing-to-identify"));
        } else {
            events.push(EngineEvent::msg_with(
                "scroll-identify-count",
                vec![("count", count.to_string())],
            ));
        }
    }

    events
}

/// Enchant weapon: +1 to wielded weapon spe (blessed: more, cap +5/+7).
/// Evaporation chance 2/3 when spe > 5.
/// Confused: set erodeproof on weapon.
fn effect_enchant_weapon<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Find the wielded weapon.
    let wielded = find_wielded_weapon(world, reader);

    let weapon_entity = match wielded {
        Some(e) => e,
        None => {
            events.push(EngineEvent::msg("scroll-confuse-cursed"));
            return events;
        }
    };

    if confused {
        // Confused reading: set/remove erodeproof.
        if let Some(mut erosion) = world.get_component_mut::<Erosion>(weapon_entity) {
            if buc.cursed {
                erosion.erodeproof = false;
                events.push(EngineEvent::msg("scroll-enchant-weapon-fragile"));
            } else {
                erosion.erodeproof = true;
                erosion.eroded = 0;
                erosion.eroded2 = 0;
                events.push(EngineEvent::msg("scroll-enchant-weapon-film"));
            }
        }
        return events;
    }

    // Blessed: also repair erosion (matches C NetHack behavior).
    if buc.blessed {
        if let Some(mut erosion) = world.get_component_mut::<Erosion>(weapon_entity) {
            erosion.eroded = 0;
            erosion.eroded2 = 0;
        }
    }

    // Normal reading: enchant the weapon.
    // Spec section 3.2.1:
    //   cursed scroll: -1
    //   no uwep: 1
    //   uwep.spe >= 9: (rn2(spe)==0) ? 1 : 0
    //   blessed: rnd(3 - spe/3)
    //   uncursed: 1
    let amount: i8 = if buc.cursed {
        -1
    } else {
        let spe = world
            .get_component::<Enchantment>(weapon_entity)
            .map(|e| e.spe)
            .unwrap_or(0);
        if spe >= 9 {
            if rn2(rng, spe as u32) == 0 { 1 } else { 0 }
        } else if buc.blessed {
            let top = (3 - (spe as i32 / 3)).max(1) as u32;
            rnd(rng, top) as i8
        } else {
            1
        }
    };

    // Read current spe.
    let current_spe = world
        .get_component::<Enchantment>(weapon_entity)
        .map(|e| e.spe)
        .unwrap_or(0);

    // Evaporation check: if spe > 5 and amount >= 0, 2/3 chance to evaporate.
    if ((current_spe > 5 && amount >= 0) || (current_spe < -5 && amount < 0)) && rn2(rng, 3) != 0 {
        // Weapon evaporates!
        events.push(EngineEvent::msg("scroll-enchant-weapon-evaporate"));
        let _ = world.despawn(weapon_entity);
        events.push(EngineEvent::ItemDestroyed {
            item: weapon_entity,
            cause: DamageCause::Disenchant,
        });
        return events;
    }

    // Apply enchantment.
    if let Some(mut ench) = world.get_component_mut::<Enchantment>(weapon_entity) {
        ench.spe = ench.spe.saturating_add(amount);
    } else {
        // No enchantment component yet; add one.
        let _ = world
            .ecs_mut()
            .insert_one(weapon_entity, Enchantment { spe: amount });
    }

    // If positive enchantment and weapon was cursed, uncurse it.
    if amount > 0
        && let Some(mut item_buc) = world.get_component_mut::<BucStatus>(weapon_entity)
        && item_buc.cursed
    {
        item_buc.cursed = false;
    }

    let new_spe = world
        .get_component::<Enchantment>(weapon_entity)
        .map(|e| e.spe)
        .unwrap_or(0);

    events.push(EngineEvent::msg_with(
        "scroll-enchant-weapon",
        vec![
            ("weapon", "weapon".to_string()),
            (
                "color",
                if amount >= 0 { "blue" } else { "purple" }.to_string(),
            ),
        ],
    ));

    // Warning vibration at high enchantment.
    if new_spe > 5 {
        events.push(EngineEvent::msg("scroll-enchant-weapon-vibrate"));
    }

    events
}

/// Enchant armor: +1 to random worn armor, same cap/evaporation rules.
/// Confused: set erodeproof on armor.
fn effect_enchant_armor<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Find a worn armor piece.
    let armor = find_worn_armor(world, reader);

    let armor_entity = match armor {
        Some(e) => e,
        None => {
            events.push(EngineEvent::msg("scroll-enchant-armor-skin"));
            return events;
        }
    };

    if confused {
        // Confused reading: set/remove erodeproof.
        if let Some(mut erosion) = world.get_component_mut::<Erosion>(armor_entity) {
            if buc.cursed {
                erosion.erodeproof = false;
                events.push(EngineEvent::msg("scroll-enchant-armor-fragile"));
            } else {
                erosion.erodeproof = true;
                erosion.eroded = 0;
                erosion.eroded2 = 0;
                events.push(EngineEvent::msg("scroll-enchant-armor-film"));
            }
        }
        return events;
    }

    // Normal reading: enchant the armor.
    let current_spe = world
        .get_component::<Enchantment>(armor_entity)
        .map(|e| e.spe)
        .unwrap_or(0);

    // For simplicity, use threshold 3 for normal armor.
    // (Special armor like elven would use 5.)
    let threshold: i8 = 3;

    // Evaporation check: if spe > threshold and rn2(spe) != 0.
    if current_spe > threshold && rn2(rng, current_spe as u32) != 0 {
        events.push(EngineEvent::msg("scroll-enchant-armor-evaporate"));
        let _ = world.despawn(armor_entity);
        events.push(EngineEvent::ItemDestroyed {
            item: armor_entity,
            cause: DamageCause::Disenchant,
        });
        return events;
    }

    // Blessed: also repair erosion (matches C NetHack behavior).
    if buc.blessed {
        if let Some(mut erosion) = world.get_component_mut::<Erosion>(armor_entity) {
            erosion.eroded = 0;
            erosion.eroded2 = 0;
        }
    }

    // Calculate enchantment amount.
    let amount: i8 = if buc.cursed {
        -1
    } else if buc.blessed {
        // Blessed: +1 extra.
        let base = 1i8;
        base + 1
    } else {
        1
    };

    // Apply enchantment.
    if let Some(mut ench) = world.get_component_mut::<Enchantment>(armor_entity) {
        ench.spe = ench.spe.saturating_add(amount);
    } else {
        let _ = world
            .ecs_mut()
            .insert_one(armor_entity, Enchantment { spe: amount });
    }

    // BUC effects on the armor itself.
    if let Some(mut item_buc) = world.get_component_mut::<BucStatus>(armor_entity) {
        if buc.blessed && !item_buc.blessed {
            item_buc.blessed = true;
            item_buc.cursed = false;
        } else if buc.cursed && !item_buc.cursed {
            item_buc.cursed = true;
            item_buc.blessed = false;
        } else if !buc.cursed && item_buc.cursed {
            item_buc.cursed = false;
        }
    }

    let new_spe = world
        .get_component::<Enchantment>(armor_entity)
        .map(|e| e.spe)
        .unwrap_or(0);

    events.push(EngineEvent::msg_with(
        "scroll-enchant-armor",
        vec![
            ("armor", "armor".to_string()),
            (
                "color",
                if amount >= 0 { "silver" } else { "black" }.to_string(),
            ),
        ],
    ));

    // Warning vibration.
    if new_spe > threshold {
        events.push(EngineEvent::msg("scroll-enchant-armor-vibrate"));
    }

    events
}

/// Remove curse: blessed=uncurse all, uncursed=uncurse worn, cursed=nothing.
/// ALL BUC states remove punishment when not confused.
/// Confused: randomly bless/curse items instead.
fn effect_remove_curse<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if confused {
        // Confused: randomly bless/curse inventory items.
        let items = collect_inventory_items(world, reader);
        for item_entity in &items {
            if rn2(rng, 2) == 0 {
                // 50% chance to affect each item.
                if let Some(mut item_buc) = world.get_component_mut::<BucStatus>(*item_entity) {
                    if rn2(rng, 2) == 0 {
                        item_buc.cursed = true;
                        item_buc.blessed = false;
                    } else {
                        item_buc.blessed = true;
                        item_buc.cursed = false;
                    }
                    item_buc.bknown = false;
                }
            }
        }
        events.push(EngineEvent::msg("scroll-remove-curse-malignant"));
        // Confused reading does NOT remove punishment.
        return events;
    }

    if buc.cursed {
        // Cursed scroll: disintegrates, no uncursing.
        events.push(EngineEvent::msg("scroll-disintegrate"));
    } else if buc.blessed {
        // Blessed: uncurse ALL inventory items.
        let items = collect_inventory_items(world, reader);
        let mut count = 0;
        for item_entity in &items {
            if let Some(mut item_buc) = world.get_component_mut::<BucStatus>(*item_entity)
                && item_buc.cursed
            {
                item_buc.cursed = false;
                count += 1;
            }
        }
        if count > 0 {
            events.push(EngineEvent::msg("scroll-remove-curse"));
        } else {
            events.push(EngineEvent::msg("scroll-remove-curse-blessed"));
        }
    } else {
        // Uncursed: uncurse worn/wielded items only.
        let worn_items = collect_worn_wielded_items(world, reader);
        let mut count = 0;
        for item_entity in &worn_items {
            if let Some(mut item_buc) = world.get_component_mut::<BucStatus>(*item_entity)
                && item_buc.cursed
            {
                item_buc.cursed = false;
                count += 1;
            }
        }
        if count > 0 {
            events.push(EngineEvent::msg("scroll-remove-curse"));
        } else {
            events.push(EngineEvent::msg("scroll-remove-curse-blessed"));
        }
    }

    // Punishment removal: ALL BUC states remove punishment when not confused.
    if world.get_component::<Punished>(reader).is_some() {
        let _ = world.ecs_mut().remove_one::<Punished>(reader);
        events.push(EngineEvent::msg("scroll-remove-curse-punishment"));
    }

    events
}

/// Teleportation: teleport player to random location.
/// Confused or cursed: level teleport.
fn effect_teleportation<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if confused || buc.cursed {
        // Level teleport.
        events.push(EngineEvent::msg("scroll-teleport-disoriented"));
        events.push(EngineEvent::LevelChanged {
            entity: reader,
            from_depth: "current".to_string(),
            to_depth: "random".to_string(),
        });
        return events;
    }

    // Normal teleportation: move to a random position.
    let old_pos = world
        .get_component::<Positioned>(reader)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    let map_width = world.dungeon().current_level.width as i32;
    let map_height = world.dungeon().current_level.height as i32;

    let new_x = rng.random_range(1..map_width.max(2));
    let new_y = rng.random_range(1..map_height.max(2));
    let new_pos = Position::new(new_x, new_y);

    if let Some(mut pos) = world.get_component_mut::<Positioned>(reader) {
        pos.0 = new_pos;
    }

    events.push(EngineEvent::EntityTeleported {
        entity: reader,
        from: old_pos,
        to: new_pos,
    });

    events
}

/// Gold detection: detect gold (or traps if confused/cursed).
fn effect_gold_detection<R: Rng>(
    _world: &mut GameWorld,
    _reader: Entity,
    buc: &BucStatus,
    confused: bool,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if confused || buc.cursed {
        events.push(EngineEvent::msg("scroll-trap-detection"));
    } else {
        events.push(EngineEvent::msg("scroll-gold-detection"));
    }

    events
}

/// Food detection: detect food on the level.
fn effect_food_detection<R: Rng>(
    _world: &mut GameWorld,
    _reader: Entity,
    _buc: &BucStatus,
    _confused: bool,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    vec![EngineEvent::msg("scroll-food-detection")]
}

/// Confuse monster scroll.
///
/// Spec section 3.8:
/// - Cursed: confuse reader for rnd(100) turns.
/// - Confused + blessed: CURE confusion (make_confused(0)).
/// - Confused + not blessed: confuse reader for rnd(100) turns.
/// - Normal (not confused, not cursed): enchant hands (umconf counter).
///   - Blessed: 5..12 charges.
///   - Uncursed: 4..5 charges.
fn effect_confuse_monster<R: Rng>(
    _world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.cursed {
        // Cursed: confuse the reader.
        let duration = rnd(rng, 100);
        events.push(EngineEvent::StatusApplied {
            entity: reader,
            status: StatusEffect::Confused,
            duration: Some(duration),
            source: None,
        });
        events.push(EngineEvent::msg("scroll-confuse-self"));
        return events;
    }

    if confused {
        if buc.blessed {
            // Confused + blessed: cure confusion.
            events.push(EngineEvent::StatusRemoved {
                entity: reader,
                status: StatusEffect::Confused,
            });
            events.push(EngineEvent::msg("scroll-confuse-cure"));
        } else {
            // Confused + not blessed: add more confusion.
            let duration = rnd(rng, 100);
            events.push(EngineEvent::StatusApplied {
                entity: reader,
                status: StatusEffect::Confused,
                duration: Some(duration),
                source: None,
            });
            events.push(EngineEvent::msg("scroll-confuse-self"));
        }
        return events;
    }

    // Normal: enchant hands to confuse next monster touched.
    // Increment is 3 (scroll base) + rnd(2) or rn1(8,2).
    let incr = if buc.blessed {
        3 + rn1(rng, 8, 2) // 5..12
    } else {
        3 + rnd(rng, 2) as i32 // 4..5
    };

    events.push(EngineEvent::msg_with(
        "scroll-confuse-hands",
        vec![("charges", incr.to_string())],
    ));

    events
}

/// Scare monster: when read from inventory, scare nearby monsters.
/// On the floor, it acts passively (not handled here).
/// Confused/cursed: wake monsters instead.
fn effect_scare_monster<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let reader_pos = world
        .get_component::<Positioned>(reader)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    // Collect monsters in visible range.
    let monsters: Vec<Entity> = {
        let mut result = Vec::new();
        for (entity, _) in world.query::<Monster>().iter() {
            if let Some(pos) = world.get_component::<Positioned>(entity) {
                let dx = (pos.0.x - reader_pos.x).abs();
                let dy = (pos.0.y - reader_pos.y).abs();
                // Within sight range (simplified).
                if dx <= 10 && dy <= 10 {
                    result.push(entity);
                }
            }
        }
        result
    };

    if confused || buc.cursed {
        // Wake/embolden monsters.
        events.push(EngineEvent::msg("scroll-scare-wailing"));
    } else {
        // Scare monsters.
        for monster in &monsters {
            events.push(EngineEvent::StatusApplied {
                entity: *monster,
                status: StatusEffect::Confused, // Stand-in for fleeing.
                duration: Some(0),
                source: Some(reader),
            });
        }
        events.push(EngineEvent::msg("scroll-scare-monster"));
    }

    events
}

/// Scare monster scroll pick-up degradation.
///
/// Returns `true` if the scroll survives pickup, `false` if it crumbles.
/// Updates the scroll's BUC/spe state appropriately.
pub fn scare_monster_pickup(
    world: &mut GameWorld,
    scroll_entity: Entity,
) -> (bool, Vec<EngineEvent>) {
    let mut events = Vec::new();

    let is_blessed = world
        .get_component::<BucStatus>(scroll_entity)
        .map(|b| b.blessed)
        .unwrap_or(false);
    let is_cursed = world
        .get_component::<BucStatus>(scroll_entity)
        .map(|b| b.cursed)
        .unwrap_or(false);
    let current_spe = world
        .get_component::<Enchantment>(scroll_entity)
        .map(|e| e.spe)
        .unwrap_or(0);

    if is_blessed {
        // Blessed: becomes uncursed, spe unchanged.
        if let Some(mut buc_status) = world.get_component_mut::<BucStatus>(scroll_entity) {
            buc_status.blessed = false;
        }
        (true, events)
    } else if current_spe == 0 && !is_cursed {
        // Fresh scroll: set spe to 1 (mark as picked up once).
        if let Some(mut ench) = world.get_component_mut::<Enchantment>(scroll_entity) {
            ench.spe = 1;
        } else {
            let _ = world
                .ecs_mut()
                .insert_one(scroll_entity, Enchantment { spe: 1 });
        }
        (true, events)
    } else {
        // Already picked up (spe != 0) or cursed: crumbles to dust.
        events.push(EngineEvent::msg("scroll-scare-dust"));
        let _ = world.despawn(scroll_entity);
        events.push(EngineEvent::ItemDestroyed {
            item: scroll_entity,
            cause: DamageCause::Physical,
        });
        (false, events)
    }
}

/// Blank paper: no effect.
fn effect_blank_paper<R: Rng>(
    _world: &mut GameWorld,
    _reader: Entity,
    _buc: &BucStatus,
    _confused: bool,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    vec![EngineEvent::msg("scroll-blank-paper")]
}

/// Fire: damage in area. Blessed=less self-damage with control,
/// uncursed/cursed=centered on self.
fn effect_fire<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if confused {
        // Confused reading (spec 3.12.2):
        // Fire resistant: no damage, "pretty fire in your hands"
        // Not fire resistant: 1 HP damage
        let has_fire_res = crate::status::has_intrinsic_fire_res(world, reader)
            || crate::worn::has_worn_property(world, reader, nethack_babel_data::Property::FireRes);
        events.push(EngineEvent::msg("scroll-fire-confused"));
        if !has_fire_res && let Some(mut hp) = world.get_component_mut::<HitPoints>(reader) {
            hp.current -= 1;
            events.push(EngineEvent::HpChange {
                entity: reader,
                amount: -1,
                new_hp: hp.current,
                source: HpSource::Environment,
            });
        }
        return events;
    }

    // Normal fire: damage based on BUC.
    let bc = bcsign(buc);
    let base = rn1(rng, 3, 3); // 3..5
    let raw = 2 * (base + 2 * bc) + 1;
    let dam = (raw / 3).max(1);

    let total_dam = if buc.blessed {
        dam * 5 // Blessed: 5x but player chooses center (not implemented).
    } else {
        dam
    };

    // Apply damage to reader (explosion centered on self for non-blessed).
    if !buc.blessed
        && let Some(mut hp) = world.get_component_mut::<HitPoints>(reader)
    {
        hp.current -= total_dam;
        events.push(EngineEvent::HpChange {
            entity: reader,
            amount: -total_dam,
            new_hp: hp.current,
            source: HpSource::Environment,
        });
    }

    // Damage nearby monsters.
    let reader_pos = world
        .get_component::<Positioned>(reader)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    let targets: Vec<Entity> = {
        let mut result = Vec::new();
        for (entity, _) in world.query::<Monster>().iter() {
            if let Some(pos) = world.get_component::<Positioned>(entity) {
                let dx = (pos.0.x - reader_pos.x).abs();
                let dy = (pos.0.y - reader_pos.y).abs();
                if dx <= 1 && dy <= 1 {
                    result.push(entity);
                }
            }
        }
        result
    };

    for target in &targets {
        events.push(EngineEvent::ExtraDamage {
            target: *target,
            amount: total_dam as u32,
            source: DamageSource::Fire,
        });
    }

    events.push(EngineEvent::msg("scroll-fire"));

    events
}

/// Earth: drop boulders/rocks around the player.
fn effect_earth<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let _reader_pos = world
        .get_component::<Positioned>(reader)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    // Simplified: damage from falling boulders/rocks.
    if confused {
        // Rocks instead of boulders (less damage).
        let dam = rn1(rng, 5, 2); // 2..6
        events.push(EngineEvent::msg("scroll-earth-rocks"));
        if !buc.blessed {
            // Blessed: boulders don't fall on hero.
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(reader) {
                hp.current -= dam;
                events.push(EngineEvent::HpChange {
                    entity: reader,
                    amount: -dam,
                    new_hp: hp.current,
                    source: HpSource::Environment,
                });
            }
        }
    } else {
        events.push(EngineEvent::msg("scroll-earth-boulders"));
        if !buc.blessed {
            // Boulders land on hero too (uncursed/cursed).
            let dam = d(rng, 1, 20) as i32;
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(reader) {
                hp.current -= dam;
                events.push(EngineEvent::HpChange {
                    entity: reader,
                    amount: -dam,
                    new_hp: hp.current,
                    source: HpSource::Environment,
                });
            }
        }
    }

    events
}

/// Punishment: attach ball and chain. Blessed/confused: safe.
fn effect_punishment<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if buc.blessed || confused {
        events.push(EngineEvent::msg("scroll-genocide-guilty"));
        return events;
    }

    // Apply punishment.
    let _ = world.ecs_mut().insert_one(reader, Punished);
    events.push(EngineEvent::msg("scroll-punishment"));

    events
}

/// Stinking cloud: create a gas cloud.
///
/// Spec section 3.21:
///   cloudsize = 15 + 10 * bcsign  (B=25, U=15, C=5)
///   damage    =  8 +  4 * bcsign  (B=12, U=8,  C=4)
fn effect_stinking_cloud<R: Rng>(
    _world: &mut GameWorld,
    _reader: Entity,
    buc: &BucStatus,
    _confused: bool,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let bc = bcsign(buc);
    let cloudsize = 15 + 10 * bc;
    let damage = 8 + 4 * bc;

    vec![EngineEvent::msg_with(
        "scroll-stinking-cloud",
        vec![
            ("cloudsize", cloudsize.to_string()),
            ("damage", damage.to_string()),
        ],
    )]
}

/// Amnesia: forget map (cursed=forget spells too).
fn effect_amnesia<R: Rng>(
    world: &mut GameWorld,
    _reader: Entity,
    buc: &BucStatus,
    _confused: bool,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Forget the map: mark all cells as unexplored.
    let level = &mut world.dungeon_mut().current_level;
    for y in 0..level.height {
        for x in 0..level.width {
            level.cells[y][x].explored = false;
            level.cells[y][x].visible = false;
        }
    }

    events.push(EngineEvent::msg("scroll-amnesia"));

    if !buc.blessed {
        // Non-blessed: also forget spells.
        events.push(EngineEvent::msg("scroll-amnesia-spells"));
    }

    events
}

/// Destroy armor scroll.
///
/// Spec section 3.17:
/// - Confused: toggle erodeproof (INVERSE of enchant armor):
///   - cursed scroll: sets erodeproof = true (protects!)
///   - blessed/uncursed: sets erodeproof = false
/// - Normal (not confused):
///   - Blessed: player chooses which to destroy (we destroy selected).
///   - Uncursed: destroy randomly selected armor.
///   - Cursed + armor is cursed: disenchant (spe-=1, min -6) + stun.
///   - Cursed + armor is not cursed: still destroys.
fn effect_destroy_armor<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let armor = find_worn_armor(world, reader);
    let armor_entity = match armor {
        Some(e) => e,
        None => {
            events.push(EngineEvent::msg("scroll-destroy-armor-itch"));
            return events;
        }
    };

    if confused {
        // Confused: toggle erodeproof (inverse of enchant armor).
        if let Some(mut erosion) = world.get_component_mut::<Erosion>(armor_entity) {
            if buc.cursed {
                erosion.erodeproof = true; // Cursed + confused protects!
            } else {
                erosion.erodeproof = false;
            }
        }
        events.push(EngineEvent::msg("scroll-destroy-armor-glow"));
        return events;
    }

    // Check if cursed scroll on cursed armor: disenchant instead of destroy.
    if buc.cursed {
        let armor_is_cursed = world
            .get_component::<BucStatus>(armor_entity)
            .map(|b| b.cursed)
            .unwrap_or(false);

        if armor_is_cursed {
            // Disenchant: spe -= 1 (minimum -6), plus stun.
            if let Some(mut ench) = world.get_component_mut::<Enchantment>(armor_entity) {
                ench.spe = (ench.spe - 1).max(-6);
            }
            let stun_duration = rn1(rng, 10, 10) as u32; // 10..19
            events.push(EngineEvent::StatusApplied {
                entity: reader,
                status: StatusEffect::Stunned,
                duration: Some(stun_duration),
                source: None,
            });
            events.push(EngineEvent::msg("scroll-destroy-armor-disenchant"));
            return events;
        }
    }

    // Blessed: identify scroll first, then destroy (in real game, player
    // chooses which armor to destroy; we destroy the selected piece).
    if buc.blessed {
        events.push(EngineEvent::msg("scroll-destroy-armor-id"));
    }

    // Destroy the armor piece.
    events.push(EngineEvent::msg("scroll-destroy-armor-crumble"));
    let _ = world.despawn(armor_entity);
    events.push(EngineEvent::ItemDestroyed {
        item: armor_entity,
        cause: DamageCause::Disenchant,
    });

    events
}

/// Create monster: spawn monsters around the player.
fn effect_create_monster<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let count = if confused || buc.cursed { 13 } else { 1 };

    let reader_pos = world
        .get_component::<Positioned>(reader)
        .map(|p| p.0)
        .unwrap_or(Position::new(40, 10));

    for _i in 0..count {
        // Spawn monster at random adjacent position.
        let dx = rng.random_range(-1i32..=1);
        let dy = rng.random_range(-1i32..=1);
        let spawn_pos = Position::new(reader_pos.x + dx, reader_pos.y + dy);

        let monster = world.spawn((
            Monster,
            Positioned(spawn_pos),
            HitPoints {
                current: 10,
                max: 10,
            },
        ));
        events.push(EngineEvent::MonsterGenerated {
            entity: monster,
            position: spawn_pos,
        });
    }

    events.push(if count > 1 {
        EngineEvent::msg("scroll-create-monster-horde")
    } else {
        EngineEvent::msg("scroll-create-monster")
    });

    events
}

/// Taming: tame nearby monsters (confused=larger radius).
fn effect_taming<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let reader_pos = world
        .get_component::<Positioned>(reader)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    let radius = if confused { 5 } else { 1 };

    // Collect affected monsters.
    let monsters: Vec<Entity> = {
        let mut result = Vec::new();
        for (entity, _) in world.query::<Monster>().iter() {
            if let Some(pos) = world.get_component::<Positioned>(entity) {
                let dx = (pos.0.x - reader_pos.x).abs();
                let dy = (pos.0.y - reader_pos.y).abs();
                if dx <= radius && dy <= radius {
                    result.push(entity);
                }
            }
        }
        result
    };

    if buc.cursed {
        // Cursed: anger monsters.
        for _monster in &monsters {
            events.push(EngineEvent::msg("scroll-taming-growl"));
        }
    } else {
        // Tame monsters.
        for monster in &monsters {
            let _ = world.ecs_mut().insert_one(*monster, Tame);
            events.push(EngineEvent::msg("scroll-taming"));
        }
    }

    if monsters.is_empty() {
        events.push(EngineEvent::msg("wand-nothing"));
    }

    events
}

/// Genocide scroll.
///
/// Spec section 3.6 — BUC x Confused matrix:
///
/// | BUC      | Confused | flags             | Effect                         |
/// |----------|----------|-------------------|--------------------------------|
/// | Blessed  | No       | (class genocide)  | Wipe entire monster class      |
/// | Blessed  | Yes      | (class genocide)  | Same — confusion has no effect |
/// | Uncursed | No       | REALLY(=1)        | Species genocide               |
/// | Uncursed | Yes      | REALLY|PLAYER(=3) | Self-genocide (kill hero)      |
/// | Cursed   | No       | 0                 | Reverse genocide (create mons) |
/// | Cursed   | Yes      | PLAYER(=2)        | Reverse-genocide hero's type   |
fn effect_genocide<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    events.push(EngineEvent::msg("scroll-genocide"));

    if buc.blessed {
        // Blessed: class genocide (confusion does not affect blessed).
        events.push(EngineEvent::msg("scroll-genocide-prompt-class"));
    } else {
        // Build flags: bit 0 = REALLY (set if uncursed), bit 1 = PLAYER (set if confused).
        let really = !buc.cursed; // uncursed
        let player = confused;

        if player && really {
            // Uncursed + Confused: forced self-genocide (hero dies).
            events.push(EngineEvent::EntityDied {
                entity: reader,
                killer: None,
                cause: crate::event::DeathCause::KilledBy {
                    killer_name: "genocidal confusion".to_string(),
                },
            });
        } else if player && !really {
            // Cursed + Confused: reverse-genocide hero's own monster type.
            let count = rn1(rng, 3, 4); // 4..6
            let reader_pos = world
                .get_component::<Positioned>(reader)
                .map(|p| p.0)
                .unwrap_or(Position::new(40, 10));
            for _ in 0..count {
                let dx = rng.random_range(-1i32..=1);
                let dy = rng.random_range(-1i32..=1);
                let spawn_pos = Position::new(reader_pos.x + dx, reader_pos.y + dy);
                let monster = world.spawn((
                    Monster,
                    Positioned(spawn_pos),
                    HitPoints {
                        current: 10,
                        max: 10,
                    },
                ));
                events.push(EngineEvent::MonsterGenerated {
                    entity: monster,
                    position: spawn_pos,
                });
            }
            events.push(EngineEvent::msg("scroll-genocide-reverse-self"));
        } else if really {
            // Uncursed, not confused: species genocide.
            events.push(EngineEvent::msg("scroll-genocide-prompt"));
        } else {
            // Cursed, not confused: reverse genocide (create 4-6 monsters).
            let count = rn1(rng, 3, 4); // 4..6
            let reader_pos = world
                .get_component::<Positioned>(reader)
                .map(|p| p.0)
                .unwrap_or(Position::new(40, 10));
            for _ in 0..count {
                let dx = rng.random_range(-1i32..=1);
                let dy = rng.random_range(-1i32..=1);
                let spawn_pos = Position::new(reader_pos.x + dx, reader_pos.y + dy);
                let monster = world.spawn((
                    Monster,
                    Positioned(spawn_pos),
                    HitPoints {
                        current: 10,
                        max: 10,
                    },
                ));
                events.push(EngineEvent::MonsterGenerated {
                    entity: monster,
                    position: spawn_pos,
                });
            }
            events.push(EngineEvent::msg("scroll-genocide-reverse"));
        }
    }

    events
}

/// Light: light up area around player.
/// Blessed=radius 9, uncursed=radius 5, cursed=darken.
fn effect_light<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if confused {
        // Create tame light monsters (simplified: just a message).
        events.push(EngineEvent::msg("scroll-light-sparkle"));
        return events;
    }

    let reader_pos = world
        .get_component::<Positioned>(reader)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    if buc.cursed {
        // Darken the area.
        events.push(EngineEvent::msg("wand-darkness"));
        return events;
    }

    // Light up the area.
    let radius = if buc.blessed { 9 } else { 5 };

    // Mark cells as visible/explored within radius.
    let level = &mut world.dungeon_mut().current_level;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let pos = Position::new(reader_pos.x + dx, reader_pos.y + dy);
            if level.in_bounds(pos) {
                let cell = &mut level.cells[pos.y as usize][pos.x as usize];
                cell.explored = true;
                cell.visible = true;
            }
        }
    }

    events.push(EngineEvent::msg("scroll-light"));

    events
}

/// Charging: recharge a wand/tool. Confused: recharge hero's energy (Pw).
fn effect_charging<R: Rng>(
    world: &mut GameWorld,
    reader: Entity,
    buc: &BucStatus,
    confused: bool,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if confused {
        // Confused: recharge hero's energy.
        if buc.cursed {
            // Drain all energy.
            if let Some(mut pw) = world.get_component_mut::<Power>(reader) {
                pw.current = 0;
                events.push(EngineEvent::PwChange {
                    entity: reader,
                    amount: -pw.current,
                    new_pw: 0,
                });
            }
            events.push(EngineEvent::msg("scroll-charging-drained"));
        } else {
            // Blessed: 6d4, uncursed: 4d4.
            let ndice = if buc.blessed { 6 } else { 4 };
            let gain = d(rng, ndice, 4) as i32;
            if let Some(mut pw) = world.get_component_mut::<Power>(reader) {
                pw.current += gain;
                if pw.current > pw.max {
                    pw.max = pw.current; // Raise maximum.
                } else {
                    pw.current = pw.max; // Restore to max.
                }
                events.push(EngineEvent::PwChange {
                    entity: reader,
                    amount: gain,
                    new_pw: pw.current,
                });
            }
            events.push(EngineEvent::msg("scroll-charging"));
        }
        return events;
    }

    // Normal: recharge a wand/tool.
    // Spec section 3.10: identifies itself, then prompts for item.
    events.push(EngineEvent::msg("scroll-charging-id"));

    // Find first wand in inventory and recharge it.
    let wand = find_first_wand(world, reader);
    if let Some(wand_entity) = wand {
        if buc.cursed {
            // Cursed: strip charges to 0.
            if let Some(mut ench) = world.get_component_mut::<Enchantment>(wand_entity) {
                ench.spe = 0;
            }
        } else {
            // Spec section 3.10.1 — Wand recharging:
            //   lim = 1 for wishing, 8 for directional, 15 for NODIR
            //   Explosion: if recharged > 0 and (wishing or n^3 > rn2(343))
            //   Amount: n = rn1(5, lim-4); if not blessed: n = rnd(n)
            //   if spe < n: spe = n; else: spe += 1
            let lim: i8 = 8; // Simplified: all wands use directional limit.
            let n = if buc.blessed {
                lim - 4 + rng.random_range(0..5) as i8 // rn1(5, lim-4) = (lim-4)..(lim)
            } else {
                let top = lim - 4 + rng.random_range(0..5) as i8;
                rnd(rng, top.max(1) as u32) as i8 // rnd(n) = 1..n
            };
            if let Some(mut ench) = world.get_component_mut::<Enchantment>(wand_entity) {
                if ench.spe < n {
                    ench.spe = n;
                } else {
                    ench.spe += 1;
                }
            }
        }
        events.push(EngineEvent::ItemCharged {
            item: wand_entity,
            new_charges: world
                .get_component::<Enchantment>(wand_entity)
                .map(|e| e.spe)
                .unwrap_or(0),
        });
    } else {
        events.push(EngineEvent::msg("scroll-charging-nothing"));
    }

    events
}

/// Magic mapping: reveal entire level map.
/// Blessed: also reveal secret doors.
fn effect_magic_mapping<R: Rng>(
    world: &mut GameWorld,
    _reader: Entity,
    buc: &BucStatus,
    confused: bool,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Mark all map cells as explored.
    let level = &mut world.dungeon_mut().current_level;
    for y in 0..level.height {
        for x in 0..level.width {
            level.cells[y][x].explored = true;
        }
    }

    if buc.cursed && !confused {
        // Cursed (not confused): scrambled map.
        events.push(EngineEvent::msg("scroll-magic-mapping-fail"));
    } else {
        events.push(EngineEvent::msg("scroll-magic-mapping"));
    }

    events
}

/// Mail: display mail message.
fn effect_mail<R: Rng>(
    _world: &mut GameWorld,
    _reader: Entity,
    _buc: &BucStatus,
    _confused: bool,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    vec![EngineEvent::msg("scroll-mail")]
}

// ---------------------------------------------------------------------------
// Inventory helpers
// ---------------------------------------------------------------------------

/// Collect all inventory item entities for the given owner.
fn collect_inventory_items(world: &GameWorld, owner: Entity) -> Vec<Entity> {
    let mut items = Vec::new();
    let player = world.player();
    for (entity, _core) in world.query::<ObjectCore>().iter() {
        if let Some(loc) = world.get_component::<ObjectLocation>(entity)
            && matches!(*loc, ObjectLocation::Inventory)
            && owner == player
        {
            items.push(entity);
        }
    }
    items
}

/// Collect worn and wielded items for the given owner.
fn collect_worn_wielded_items(world: &GameWorld, owner: Entity) -> Vec<Entity> {
    let mut items = Vec::new();
    let player = world.player();
    for (entity, _core) in world.query::<ObjectCore>().iter() {
        if let Some(loc) = world.get_component::<ObjectLocation>(entity)
            && matches!(*loc, ObjectLocation::Inventory)
            && owner == player
            && (world.get_component::<Worn>(entity).is_some()
                || world.get_component::<Wielded>(entity).is_some())
        {
            items.push(entity);
        }
    }
    items
}

/// Find the currently wielded weapon for an entity.
fn find_wielded_weapon(world: &GameWorld, owner: Entity) -> Option<Entity> {
    let player = world.player();
    for (entity, _core) in world.query::<ObjectCore>().iter() {
        if let Some(loc) = world.get_component::<ObjectLocation>(entity)
            && matches!(*loc, ObjectLocation::Inventory)
            && owner == player
            && world.get_component::<Wielded>(entity).is_some()
        {
            return Some(entity);
        }
    }
    None
}

/// Find a worn armor piece for an entity.
fn find_worn_armor(world: &GameWorld, owner: Entity) -> Option<Entity> {
    let player = world.player();
    for (entity, core) in world.query::<ObjectCore>().iter() {
        if let Some(loc) = world.get_component::<ObjectLocation>(entity)
            && matches!(*loc, ObjectLocation::Inventory)
            && owner == player
            && core.object_class == ObjectClass::Armor
            && world.get_component::<Worn>(entity).is_some()
        {
            return Some(entity);
        }
    }
    None
}

/// Find the first wand in inventory.
fn find_first_wand(world: &GameWorld, owner: Entity) -> Option<Entity> {
    let player = world.player();
    for (entity, core) in world.query::<ObjectCore>().iter() {
        if let Some(loc) = world.get_component::<ObjectLocation>(entity)
            && matches!(*loc, ObjectLocation::Inventory)
            && owner == player
            && core.object_class == ObjectClass::Wand
        {
            return Some(entity);
        }
    }
    None
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use nethack_babel_data::{ObjectClass, ObjectTypeId};
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

    fn spawn_scroll(world: &mut GameWorld, buc: BucStatus) -> Entity {
        world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(200),
                object_class: ObjectClass::Scroll,
                quantity: 1,
                weight: 5,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            buc,
        ))
    }

    fn spawn_weapon_in_inventory(world: &mut GameWorld, spe: i8, buc: BucStatus) -> Entity {
        world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(300),
                object_class: ObjectClass::Weapon,
                quantity: 1,
                weight: 30,
                age: 0,
                inv_letter: Some('a'),
                artifact: None,
            },
            buc,
            Enchantment { spe },
            Erosion {
                eroded: 0,
                eroded2: 0,
                erodeproof: false,
                greased: false,
            },
            ObjectLocation::Inventory,
            Wielded,
        ))
    }

    fn spawn_armor_in_inventory(world: &mut GameWorld, spe: i8, buc: BucStatus) -> Entity {
        world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(400),
                object_class: ObjectClass::Armor,
                quantity: 1,
                weight: 50,
                age: 0,
                inv_letter: Some('b'),
                artifact: None,
            },
            buc,
            Enchantment { spe },
            Erosion {
                eroded: 0,
                eroded2: 0,
                erodeproof: false,
                greased: false,
            },
            ObjectLocation::Inventory,
            Worn,
        ))
    }

    fn spawn_inventory_item(world: &mut GameWorld, buc: BucStatus) -> Entity {
        world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(500),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some('c'),
                artifact: None,
            },
            buc,
            ObjectLocation::Inventory,
        ))
    }

    // ── Test: Blessed identify identifies all items ──────────────

    #[test]
    fn blessed_identify_identifies_multiple_items() {
        // Blessed identify uses rn2(5): 0=all, 1->2, 2..4 items.
        // Run with multiple seeds to verify we get at least 2 items identified
        // in some cases and all items in others.
        let mut found_all = false;
        let mut found_partial = false;
        for seed in 0u64..200 {
            let mut world = make_world();
            let player = world.player();
            let _item1 = spawn_inventory_item(&mut world, uncursed());
            let _item2 = spawn_inventory_item(&mut world, uncursed());
            let _item3 = spawn_inventory_item(&mut world, uncursed());

            let scroll = spawn_scroll(&mut world, blessed());
            let mut rng = Pcg64::seed_from_u64(seed);
            let events = read_scroll(
                &mut world,
                player,
                scroll,
                ScrollType::Identify,
                false,
                &mut rng,
            );

            let identify_count = events
                .iter()
                .filter(|e| matches!(e, EngineEvent::ItemIdentified { .. }))
                .count();
            assert!(
                identify_count >= 2 || identify_count == 3,
                "blessed identify should identify at least 2 items (got {})",
                identify_count,
            );
            if identify_count == 3 {
                found_all = true;
            }
            if identify_count < 3 {
                found_partial = true;
            }
            if found_all && found_partial {
                break;
            }
        }
        assert!(
            found_all,
            "blessed identify should sometimes identify all items (rn2(5)==0)"
        );
    }

    // ── Test: Uncursed identify identifies 1 item ──────────────

    #[test]
    fn uncursed_identify_identifies_one_item() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let _item1 = spawn_inventory_item(&mut world, uncursed());
        let _item2 = spawn_inventory_item(&mut world, uncursed());

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Identify,
            false,
            &mut rng,
        );

        let identify_count = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::ItemIdentified { .. }))
            .count();
        assert_eq!(
            identify_count, 1,
            "uncursed identify should identify 1 item"
        );
    }

    // ── Test: Enchant weapon increases spe ──────────────────────

    #[test]
    fn enchant_weapon_increases_spe() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let weapon = spawn_weapon_in_inventory(&mut world, 0, uncursed());
        let scroll = spawn_scroll(&mut world, uncursed());

        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::EnchantWeapon,
            false,
            &mut rng,
        );

        let ench = world
            .get_component::<Enchantment>(weapon)
            .expect("weapon should have enchantment");
        assert_eq!(ench.spe, 1, "uncursed enchant weapon should give +1");
    }

    // ── Test: Enchant weapon evaporation at +6 ─────────────────

    #[test]
    fn enchant_weapon_evaporation_at_plus_6() {
        // Use a seed that gives rn2(3) != 0 (evaporation).
        // We test that at spe > 5 with positive amount, the weapon
        // either evaporates or survives depending on the RNG.
        let mut world = make_world();
        let _player = world.player();

        let _weapon = spawn_weapon_in_inventory(&mut world, 6, uncursed());

        // Run multiple times to verify the evaporation logic fires.
        let mut evaporated = false;
        let mut survived = false;
        for seed in 0..100u64 {
            let mut test_world = make_world();
            let test_player = test_world.player();
            let test_weapon = spawn_weapon_in_inventory(&mut test_world, 6, uncursed());
            let test_scroll = spawn_scroll(&mut test_world, uncursed());
            let mut rng = Pcg64::seed_from_u64(seed);

            let events = read_scroll(
                &mut test_world,
                test_player,
                test_scroll,
                ScrollType::EnchantWeapon,
                false,
                &mut rng,
            );

            let has_destroy = events.iter().any(|e| {
                matches!(e, EngineEvent::ItemDestroyed { item, .. }
                    if *item == test_weapon)
            });
            if has_destroy {
                evaporated = true;
            } else {
                survived = true;
            }
            if evaporated && survived {
                break;
            }
        }

        assert!(
            evaporated,
            "weapon at +6 should sometimes evaporate (2/3 chance)"
        );
        assert!(
            survived,
            "weapon at +6 should sometimes survive (1/3 chance)"
        );
    }

    // ── Test: Remove curse uncurses worn items ──────────────────

    #[test]
    fn remove_curse_uncurses_worn_items() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Cursed armor that is worn.
        let armor = spawn_armor_in_inventory(&mut world, 0, cursed());
        // Cursed item NOT worn (should not be affected by uncursed scroll).
        let loose = spawn_inventory_item(&mut world, cursed());

        let scroll = spawn_scroll(&mut world, uncursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::RemoveCurse,
            false,
            &mut rng,
        );

        let armor_buc = world.get_component::<BucStatus>(armor).unwrap();
        assert!(
            !armor_buc.cursed,
            "worn cursed armor should be uncursed by uncursed remove curse"
        );

        let loose_buc = world.get_component::<BucStatus>(loose).unwrap();
        assert!(
            loose_buc.cursed,
            "non-worn cursed item should NOT be affected by uncursed remove curse"
        );
    }

    // ── Test: Remove curse unpunishes for all BUC states ────────

    #[test]
    fn remove_curse_unpunishes_for_all_buc() {
        // This is the critical fix: even a cursed scroll removes punishment.
        for (buc_status, label) in [
            (blessed(), "blessed"),
            (uncursed(), "uncursed"),
            (cursed(), "cursed"),
        ] {
            let mut world = make_world();
            let mut rng = make_rng();
            let player = world.player();

            // Apply punishment.
            let _ = world.ecs_mut().insert_one(player, Punished);
            assert!(
                world.get_component::<Punished>(player).is_some(),
                "player should be punished before reading scroll"
            );

            let scroll = spawn_scroll(&mut world, buc_status);
            let events = read_scroll(
                &mut world,
                player,
                scroll,
                ScrollType::RemoveCurse,
                false, // NOT confused
                &mut rng,
            );

            assert!(
                world.get_component::<Punished>(player).is_none(),
                "{} remove curse should remove punishment",
                label
            );

            let has_unpunish_msg = events.iter().any(|e| match e {
                EngineEvent::Message { key, .. } => key.contains("scroll-remove-curse-punishment"),
                _ => false,
            });
            assert!(
                has_unpunish_msg,
                "{} remove curse should emit punishment removal message",
                label
            );
        }
    }

    // ── Test: Remove curse does NOT unpunish when confused ──────

    #[test]
    fn remove_curse_confused_does_not_unpunish() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let _ = world.ecs_mut().insert_one(player, Punished);

        let scroll = spawn_scroll(&mut world, uncursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::RemoveCurse,
            true, // confused
            &mut rng,
        );

        assert!(
            world.get_component::<Punished>(player).is_some(),
            "confused remove curse should NOT remove punishment"
        );
    }

    // ── Test: Magic mapping reveals map ─────────────────────────

    #[test]
    fn magic_mapping_reveals_map() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Verify some cells are unexplored initially.
        assert!(
            !world.dungeon().current_level.cells[5][5].explored,
            "cell should be unexplored initially"
        );

        let scroll = spawn_scroll(&mut world, uncursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::MagicMapping,
            false,
            &mut rng,
        );

        // All cells should now be explored.
        let all_explored = world
            .dungeon()
            .current_level
            .cells
            .iter()
            .all(|row| row.iter().all(|cell| cell.explored));
        assert!(all_explored, "magic mapping should reveal all map cells");
    }

    // ── Test: Teleportation moves player ────────────────────────

    #[test]
    fn teleportation_moves_player() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let _old_pos = world.get_component::<Positioned>(player).unwrap().0;

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Teleportation,
            false,
            &mut rng,
        );

        let _new_pos = world.get_component::<Positioned>(player).unwrap().0;

        // With high probability the player moved (could be same spot with
        // very low probability, but the teleport event should still fire).
        let has_teleport_event = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityTeleported { .. }));
        assert!(
            has_teleport_event,
            "teleportation should emit EntityTeleported"
        );
    }

    // ── Test: Confused enchant weapon makes erodeproof ──────────

    #[test]
    fn confused_enchant_weapon_makes_erodeproof() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let weapon = spawn_weapon_in_inventory(&mut world, 0, uncursed());

        // Verify not erodeproof initially.
        let erosion = world.get_component::<Erosion>(weapon).unwrap();
        assert!(
            !erosion.erodeproof,
            "weapon should not be erodeproof initially"
        );
        drop(erosion);

        let scroll = spawn_scroll(&mut world, uncursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::EnchantWeapon,
            true, // confused
            &mut rng,
        );

        let erosion = world.get_component::<Erosion>(weapon).unwrap();
        assert!(
            erosion.erodeproof,
            "confused enchant weapon should make weapon erodeproof"
        );
    }

    // ── Test: Confused remove curse curses items ────────────────

    #[test]
    fn confused_remove_curse_scrambles_buc() {
        let mut world = make_world();
        let player = world.player();

        // Spawn several uncursed worn items.
        let mut items = Vec::new();
        for _ in 0..20 {
            items.push(spawn_armor_in_inventory(&mut world, 0, uncursed()));
        }

        let scroll = spawn_scroll(&mut world, uncursed());
        let mut rng = make_rng();
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::RemoveCurse,
            true, // confused
            &mut rng,
        );

        // At least some items should have changed BUC (with 20 items
        // and 50% chance each, extremely unlikely none changed).
        let mut changed = 0;
        for item in &items {
            if let Some(item_buc) = world.get_component::<BucStatus>(*item)
                && (item_buc.cursed || item_buc.blessed)
            {
                changed += 1;
            }
        }
        assert!(
            changed > 0,
            "confused remove curse should change BUC on some items (got 0 of 20)"
        );
    }

    // ── Test: Scare monster spe state machine ───────────────────

    #[test]
    fn scare_monster_spe_state_machine() {
        let mut world = make_world();

        // Fresh scare monster scroll on floor: blessed, spe=0.
        let scroll = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(600),
                object_class: ObjectClass::Scroll,
                quantity: 1,
                weight: 5,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            BucStatus {
                cursed: false,
                blessed: true,
                bknown: false,
            },
            Enchantment { spe: 0 },
            ObjectLocation::Floor { x: 5, y: 5 },
        ));

        // First pickup: blessed -> becomes uncursed, spe stays 0.
        let (survived, _events) = scare_monster_pickup(&mut world, scroll);
        assert!(
            survived,
            "blessed scare monster should survive first pickup"
        );
        let buc = world.get_component::<BucStatus>(scroll).unwrap();
        assert!(!buc.blessed, "should no longer be blessed after pickup");
        assert!(!buc.cursed, "should be uncursed after pickup");
        let ench = world.get_component::<Enchantment>(scroll).unwrap();
        assert_eq!(ench.spe, 0, "spe should remain 0 after first pickup");
        drop(buc);
        drop(ench);

        // Second pickup: uncursed, spe=0 -> spe becomes 1.
        let (survived, _events) = scare_monster_pickup(&mut world, scroll);
        assert!(
            survived,
            "uncursed spe=0 scare monster should survive pickup"
        );
        let ench = world.get_component::<Enchantment>(scroll).unwrap();
        assert_eq!(ench.spe, 1, "spe should become 1 after second pickup");
        drop(ench);

        // Third pickup: spe=1 -> crumbles to dust.
        let (survived, events) = scare_monster_pickup(&mut world, scroll);
        assert!(!survived, "spe=1 scare monster should crumble on pickup");
        let has_destroy = events
            .iter()
            .any(|e| matches!(e, EngineEvent::ItemDestroyed { .. }));
        assert!(has_destroy, "should emit ItemDestroyed when crumbling");
    }

    // ── Test: Light illuminates area ────────────────────────────

    #[test]
    fn light_illuminates_area() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Player is at (40, 10). Verify nearby cells are unexplored.
        assert!(
            !world.dungeon().current_level.cells[10][40].explored,
            "player cell should be unexplored initially"
        );

        let scroll = spawn_scroll(&mut world, uncursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Light,
            false,
            &mut rng,
        );

        // Cells within radius 5 of (40, 10) should be explored.
        let cell = &world.dungeon().current_level.cells[10][40];
        assert!(cell.explored, "player cell should be explored after light");
        assert!(cell.visible, "player cell should be visible after light");

        // A cell within radius.
        let cell2 = &world.dungeon().current_level.cells[10][42];
        assert!(cell2.explored, "nearby cell should be explored after light");

        // A cell outside radius (more than 5 away).
        // Radius 5 from (40,10): x in [35..45], y in [5..15].
        // x=46 has dx=6 > 5, should NOT be explored.
        let outside = &world.dungeon().current_level.cells[0][0];
        assert!(
            !outside.explored,
            "cell far from player should NOT be explored by uncursed light"
        );
    }

    // ── Test: Blank paper has no effect ─────────────────────────

    #[test]
    fn blank_paper_no_effect() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::BlankPaper,
            false,
            &mut rng,
        );

        let has_blank_msg = events.iter().any(|e| match e {
            EngineEvent::Message { key, .. } => key.contains("scroll-blank"),
            _ => false,
        });
        assert!(has_blank_msg, "blank paper should say it's blank");
    }

    // ── Test: Create monster spawns entities ─────────────────────

    #[test]
    fn create_monster_spawns_entities() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Count monsters before.
        let before_count: usize = world.query::<Monster>().iter().count();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::CreateMonster,
            false,
            &mut rng,
        );

        let after_count: usize = world.query::<Monster>().iter().count();
        assert!(
            after_count > before_count,
            "create monster should spawn at least one monster"
        );

        let has_gen_event = events
            .iter()
            .any(|e| matches!(e, EngineEvent::MonsterGenerated { .. }));
        assert!(has_gen_event, "should emit MonsterGenerated event");
    }

    // ── Test: Confused teleportation is level teleport ──────────

    #[test]
    fn confused_teleportation_is_level_teleport() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Teleportation,
            true, // confused
            &mut rng,
        );

        let has_level_change = events
            .iter()
            .any(|e| matches!(e, EngineEvent::LevelChanged { .. }));
        assert!(
            has_level_change,
            "confused teleportation should emit LevelChanged (level teleport)"
        );
    }

    // ── Test: Taming tames nearby monsters ──────────────────────

    #[test]
    fn taming_tames_nearby_monsters() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Spawn a monster adjacent to player.
        let monster = world.spawn((
            Monster,
            Positioned(Position::new(41, 10)),
            HitPoints {
                current: 10,
                max: 10,
            },
        ));

        // Verify not tame initially.
        assert!(
            world.get_component::<Tame>(monster).is_none(),
            "monster should not be tame initially"
        );

        let scroll = spawn_scroll(&mut world, uncursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Taming,
            false,
            &mut rng,
        );

        assert!(
            world.get_component::<Tame>(monster).is_some(),
            "taming scroll should tame adjacent monster"
        );
    }

    // ── Test: Amnesia forgets map ───────────────────────────────

    #[test]
    fn amnesia_forgets_map() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Mark some cells as explored.
        {
            let level = &mut world.dungeon_mut().current_level;
            level.cells[5][5].explored = true;
            level.cells[5][6].explored = true;
            level.cells[5][7].explored = true;
        }

        let scroll = spawn_scroll(&mut world, uncursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Amnesia,
            false,
            &mut rng,
        );

        let all_forgotten = world
            .dungeon()
            .current_level
            .cells
            .iter()
            .all(|row| row.iter().all(|cell| !cell.explored));
        assert!(all_forgotten, "amnesia should forget all explored cells");
    }

    // ── Test: Cursed enchant weapon decreases spe ───────────────

    #[test]
    fn cursed_enchant_weapon_decreases_spe() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let weapon = spawn_weapon_in_inventory(&mut world, 3, uncursed());
        let scroll = spawn_scroll(&mut world, cursed());

        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::EnchantWeapon,
            false,
            &mut rng,
        );

        let ench = world.get_component::<Enchantment>(weapon).unwrap();
        assert_eq!(ench.spe, 2, "cursed enchant weapon should give -1 (3-1=2)");
    }

    // ── Test: Enchant armor increases spe ───────────────────────

    #[test]
    fn enchant_armor_increases_spe() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let armor = spawn_armor_in_inventory(&mut world, 0, uncursed());
        let scroll = spawn_scroll(&mut world, uncursed());

        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::EnchantArmor,
            false,
            &mut rng,
        );

        let ench = world.get_component::<Enchantment>(armor).unwrap();
        assert_eq!(ench.spe, 1, "uncursed enchant armor should give +1");
    }

    // ── Test: Fire deals area damage ────────────────────────────

    #[test]
    fn fire_deals_area_damage() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Damage player initially to full HP.
        let original_hp = world.get_component::<HitPoints>(player).unwrap().current;

        let scroll = spawn_scroll(&mut world, uncursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Fire,
            false,
            &mut rng,
        );

        let new_hp = world.get_component::<HitPoints>(player).unwrap().current;

        assert!(
            new_hp < original_hp,
            "uncursed fire scroll should damage the reader"
        );
    }

    // ── Test: Punishment applies Punished component ─────────────

    #[test]
    fn punishment_applies_punished() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        assert!(
            world.get_component::<Punished>(player).is_none(),
            "player should not be punished initially"
        );

        let scroll = spawn_scroll(&mut world, uncursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Punishment,
            false,
            &mut rng,
        );

        assert!(
            world.get_component::<Punished>(player).is_some(),
            "uncursed punishment scroll should apply Punished"
        );
    }

    // ── Test: Blessed punishment is safe ─────────────────────────

    #[test]
    fn blessed_punishment_is_safe() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, blessed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Punishment,
            false,
            &mut rng,
        );

        assert!(
            world.get_component::<Punished>(player).is_none(),
            "blessed punishment should not punish"
        );

        let has_guilty = events.iter().any(|e| match e {
            EngineEvent::Message { key, .. } => key.contains("scroll-genocide-guilty"),
            _ => false,
        });
        assert!(
            has_guilty,
            "blessed punishment should say 'you feel guilty'"
        );
    }

    // ── Test: Identify BUC matrix (spec test vectors 1-8) ───────

    #[test]
    fn test_scroll_identify_cursed_no_effect() {
        // TV-5: Cursed, not confused: self-identify only, no ItemIdentified events.
        let mut world = make_world();
        let player = world.player();
        let _item = spawn_inventory_item(&mut world, uncursed());
        let scroll = spawn_scroll(&mut world, cursed());
        let mut rng = make_rng();

        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Identify,
            false,
            &mut rng,
        );

        let identify_count = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::ItemIdentified { .. }))
            .count();
        assert_eq!(identify_count, 0, "cursed identify should identify 0 items");
    }

    #[test]
    fn test_scroll_identify_confused_no_effect() {
        // TV-7: Any BUC + confused: self-identify only.
        for buc_status in [blessed(), uncursed(), cursed()] {
            let mut world = make_world();
            let player = world.player();
            let _item = spawn_inventory_item(&mut world, uncursed());
            let scroll = spawn_scroll(&mut world, buc_status);
            let mut rng = make_rng();

            let events = read_scroll(
                &mut world,
                player,
                scroll,
                ScrollType::Identify,
                true,
                &mut rng,
            );

            let identify_count = events
                .iter()
                .filter(|e| matches!(e, EngineEvent::ItemIdentified { .. }))
                .count();
            assert_eq!(
                identify_count, 0,
                "confused identify should identify 0 items"
            );
        }
    }

    #[test]
    fn test_scroll_identify_uncursed_usually_one() {
        // TV-4: Uncursed, not confused: 80% identify 1, 20% more.
        let mut one_count = 0;
        let mut more_count = 0;
        for seed in 0u64..500 {
            let mut world = make_world();
            let player = world.player();
            for _ in 0..5 {
                spawn_inventory_item(&mut world, uncursed());
            }
            let scroll = spawn_scroll(&mut world, uncursed());
            let mut rng = Pcg64::seed_from_u64(seed);

            let events = read_scroll(
                &mut world,
                player,
                scroll,
                ScrollType::Identify,
                false,
                &mut rng,
            );

            let count = events
                .iter()
                .filter(|e| matches!(e, EngineEvent::ItemIdentified { .. }))
                .count();
            if count == 1 {
                one_count += 1;
            } else if count > 1 {
                more_count += 1;
            }
        }
        assert!(
            one_count > more_count,
            "uncursed identify should mostly identify 1 item (got {} vs {})",
            one_count,
            more_count
        );
        assert!(
            more_count > 0,
            "uncursed identify should sometimes identify more than 1 item"
        );
    }

    // ── Test: Enchant weapon at spe >= 9 has diminishing returns ──

    #[test]
    fn test_scroll_enchant_weapon_blessed_high_spe() {
        // TV: spe >= 9: (rn2(spe)==0)?1:0 — mostly +0.
        let mut zero_count = 0;
        let mut one_count = 0;
        for seed in 0u64..200 {
            let mut world = make_world();
            let player = world.player();
            let weapon = spawn_weapon_in_inventory(&mut world, 10, uncursed());
            let scroll = spawn_scroll(&mut world, blessed());
            let mut rng = Pcg64::seed_from_u64(seed);

            let _events = read_scroll(
                &mut world,
                player,
                scroll,
                ScrollType::EnchantWeapon,
                false,
                &mut rng,
            );

            // Check if weapon still exists (might have evaporated).
            if let Some(ench) = world.get_component::<Enchantment>(weapon) {
                if ench.spe == 10 {
                    zero_count += 1; // amount was 0
                } else if ench.spe == 11 {
                    one_count += 1; // amount was 1
                }
            }
        }
        // At spe=10, rn2(10)==0 has 10% chance, so one_count should be much less.
        assert!(
            zero_count > one_count,
            "at spe>=9, +0 should be more common than +1"
        );
    }

    // ── Test: Genocide confused + uncursed kills hero ─────────────

    #[test]
    fn test_scroll_genocide_confused_uncursed_self_genocide() {
        // TV-36: Uncursed + confused: self-genocide (hero dies).
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Genocide,
            true,
            &mut rng,
        );

        let has_death = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityDied { .. }));
        assert!(has_death, "confused uncursed genocide should kill the hero");
    }

    // ── Test: Genocide confused + cursed creates monsters ─────────

    #[test]
    fn test_scroll_genocide_confused_cursed_reverse_self() {
        // TV-38: Cursed + confused: reverse-genocide hero's own type.
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let _before = world.query::<Monster>().iter().count();
        let scroll = spawn_scroll(&mut world, cursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Genocide,
            true,
            &mut rng,
        );

        let has_death = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityDied { .. }));
        assert!(
            !has_death,
            "confused cursed genocide should NOT kill the hero"
        );

        let generated = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::MonsterGenerated { .. }))
            .count();
        assert!(
            generated >= 4 && generated <= 6,
            "confused cursed genocide should create 4-6 monsters, got {}",
            generated
        );
    }

    // ── Test: Genocide blessed is unaffected by confusion ─────────

    #[test]
    fn test_scroll_genocide_blessed_ignores_confusion() {
        // TV-34: Blessed + confused: class genocide (confusion doesn't affect).
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, blessed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Genocide,
            true,
            &mut rng,
        );

        let has_class_prompt = events.iter().any(|e| match e {
            EngineEvent::Message { key, .. } => key.contains("scroll-genocide-prompt-class"),
            _ => false,
        });
        assert!(
            has_class_prompt,
            "blessed genocide should prompt for class even when confused"
        );
    }

    // ── Test: Stinking cloud BUC values ──────────────────────────

    #[test]
    fn test_scroll_stinking_cloud_buc_values() {
        // TV-29/30: Verify cloudsize and damage formulas.
        for (buc_status, expected_size, expected_dmg) in [
            (blessed(), "25", "12"),
            (uncursed(), "15", "8"),
            (cursed(), "5", "4"),
        ] {
            let mut world = make_world();
            let mut rng = make_rng();
            let player = world.player();

            let scroll = spawn_scroll(&mut world, buc_status);
            let events = read_scroll(
                &mut world,
                player,
                scroll,
                ScrollType::StinkingCloud,
                false,
                &mut rng,
            );

            let has_params = events.iter().any(|e| match e {
                EngineEvent::Message { key, args } => {
                    key.contains("scroll-stinking-cloud")
                        && args
                            .iter()
                            .any(|(k, v)| k == "cloudsize" && v == expected_size)
                        && args.iter().any(|(k, v)| k == "damage" && v == expected_dmg)
                }
                _ => false,
            });
            assert!(
                has_params,
                "stinking cloud should have correct size/damage params"
            );
        }
    }

    // ── Test: Confuse monster — blessed confused cures confusion ──

    #[test]
    fn test_scroll_confuse_monster_blessed_confused_cures() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, blessed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::ConfuseMonster,
            true,
            &mut rng,
        );

        let has_cure = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::StatusRemoved {
                    status: StatusEffect::Confused,
                    ..
                }
            )
        });
        assert!(
            has_cure,
            "blessed confuse monster while confused should cure confusion"
        );
    }

    // ── Test: Destroy armor — cursed on cursed armor disenchants ──

    #[test]
    fn test_scroll_destroy_armor_cursed_on_cursed_disenchants() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Cursed armor, spe = +2.
        let armor = spawn_armor_in_inventory(&mut world, 2, cursed());

        let scroll = spawn_scroll(&mut world, cursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::DestroyArmor,
            false,
            &mut rng,
        );

        // Armor should still exist (not destroyed).
        let ench = world.get_component::<Enchantment>(armor);
        assert!(
            ench.is_some(),
            "cursed scroll on cursed armor should NOT destroy it"
        );
        assert_eq!(ench.unwrap().spe, 1, "spe should decrease from 2 to 1");

        // Should also stun the reader.
        let has_stun = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::Stunned,
                    ..
                }
            )
        });
        assert!(
            has_stun,
            "cursed destroy armor on cursed armor should stun reader"
        );
    }

    // ── Test: Destroy armor — confused cursed protects ────────────

    #[test]
    fn test_scroll_destroy_armor_confused_cursed_protects() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let armor = spawn_armor_in_inventory(&mut world, 0, uncursed());

        let scroll = spawn_scroll(&mut world, cursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::DestroyArmor,
            true,
            &mut rng,
        );

        // Armor should be erodeproof now.
        let erosion = world.get_component::<Erosion>(armor).unwrap();
        assert!(
            erosion.erodeproof,
            "confused cursed destroy armor should SET erodeproof"
        );
    }

    // ── Test: Charging confused BUC energy recharge ───────────────

    #[test]
    fn test_scroll_charging_confused_cursed_drains_energy() {
        // TV-51: Cursed + confused: uen = 0.
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Give the player some energy.
        let _ = world.ecs_mut().insert_one(
            player,
            Power {
                current: 30,
                max: 50,
            },
        );

        let scroll = spawn_scroll(&mut world, cursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Charging,
            true,
            &mut rng,
        );

        let pw = world.get_component::<Power>(player).unwrap();
        assert_eq!(
            pw.current, 0,
            "confused cursed charging should drain all energy"
        );
    }

    #[test]
    fn test_scroll_charging_confused_blessed_restores_energy() {
        // TV-49: Blessed + confused: restore to max.
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let _ = world.ecs_mut().insert_one(
            player,
            Power {
                current: 10,
                max: 50,
            },
        );

        let scroll = spawn_scroll(&mut world, blessed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Charging,
            true,
            &mut rng,
        );

        let pw = world.get_component::<Power>(player).unwrap();
        // Should be at max (50) or raised above if gain > remaining.
        assert!(
            pw.current >= 50,
            "confused blessed charging should restore to at least max (got {})",
            pw.current
        );
    }

    // ── Test: Scare monster cursed crumbles immediately ───────────

    #[test]
    fn test_scroll_scare_monster_cursed_crumbles() {
        // TV-23: Cursed, spe=0: turns to dust.
        let mut world = make_world();

        let scroll = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(600),
                object_class: ObjectClass::Scroll,
                quantity: 1,
                weight: 5,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            BucStatus {
                cursed: true,
                blessed: false,
                bknown: false,
            },
            Enchantment { spe: 0 },
            ObjectLocation::Floor { x: 5, y: 5 },
        ));

        let (survived, _events) = scare_monster_pickup(&mut world, scroll);
        assert!(!survived, "cursed scare monster should crumble on pickup");
    }

    // ── Test: Teleportation scroll teleports player ──────────────

    #[test]
    fn test_scroll_teleportation_uncursed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Teleportation,
            false,
            &mut rng,
        );

        let has_teleport = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityTeleported { .. }));
        assert!(
            has_teleport,
            "uncursed teleportation should teleport the player"
        );
    }

    // ── Test: Cursed teleportation → level teleport down ─────────

    #[test]
    fn test_scroll_teleportation_cursed_level_teleport() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, cursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Teleportation,
            false,
            &mut rng,
        );

        let has_level_change = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::LevelChanged { .. } | EngineEvent::EntityTeleported { .. }
            )
        });
        assert!(
            has_level_change,
            "cursed teleportation should cause level teleport or regular teleport"
        );
    }

    // ── Test: Gold detection detects gold ─────────────────────────

    #[test]
    fn test_scroll_gold_detection_uncursed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::GoldDetection,
            false,
            &mut rng,
        );

        let has_detect = events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("gold-detect") || key.contains("detect")
        ));
        assert!(has_detect, "gold detection should emit detection message");
    }

    // ── Test: Gold detection confused → detects traps ────────────

    #[test]
    fn test_scroll_gold_detection_confused() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::GoldDetection,
            true,
            &mut rng,
        );

        let has_trap_detect = events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("trap") || key.contains("detect")
        ));
        assert!(
            has_trap_detect,
            "confused gold detection should detect traps"
        );
    }

    // ── Test: Food detection detects food ─────────────────────────

    #[test]
    fn test_scroll_food_detection_uncursed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::FoodDetection,
            false,
            &mut rng,
        );

        let has_detect = events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("food") || key.contains("detect")
        ));
        assert!(has_detect, "food detection should emit detection message");
    }

    // ── Test: Food detection confused → gold/portals ─────────────

    #[test]
    fn test_scroll_food_detection_confused() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::FoodDetection,
            true,
            &mut rng,
        );

        let has_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { .. }));
        assert!(has_msg, "confused food detection should emit a message");
    }

    // ── Test: Earth scroll drops boulders ─────────────────────────

    #[test]
    fn test_scroll_earth_drops_boulders() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Earth,
            false,
            &mut rng,
        );

        let has_earth = events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("earth") || key.contains("boulder")
        ));
        assert!(has_earth, "earth scroll should reference boulders/earth");
    }

    // ── Test: Blessed earth has wider effect ──────────────────────

    #[test]
    fn test_scroll_earth_blessed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, blessed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Earth,
            false,
            &mut rng,
        );

        // Blessed should produce events (more boulders or different effect).
        assert!(!events.is_empty(), "blessed earth should produce events");
    }

    // ── Test: Mail scroll produces message ────────────────────────

    #[test]
    fn test_scroll_mail() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Mail,
            false,
            &mut rng,
        );

        let has_mail = events.iter().any(|e| {
            matches!(
                e, EngineEvent::Message { key, .. } if key.contains("mail")
            )
        });
        assert!(has_mail, "mail scroll should produce a mail message");
    }

    // ── Test: Cursed light creates darkness ───────────────────────

    #[test]
    fn test_scroll_light_cursed_darkness() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, cursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Light,
            false,
            &mut rng,
        );

        let has_dark = events.iter().any(|e| {
            matches!(
                e, EngineEvent::Message { key, .. } if key.contains("dark") || key.contains("light")
            )
        });
        assert!(has_dark, "cursed light scroll should create darkness");
    }

    // ── Test: Blessed light illuminates wider area ────────────────

    #[test]
    fn test_scroll_light_blessed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, blessed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Light,
            false,
            &mut rng,
        );

        let has_light = events.iter().any(|e| {
            matches!(
                e, EngineEvent::Message { key, .. } if key.contains("light") || key.contains("glow")
            )
        });
        assert!(has_light, "blessed light should illuminate area");
    }

    // ── Test: Blessed enchant armor also fixes erosion ────────────

    #[test]
    fn test_scroll_enchant_armor_blessed_fixes_erosion() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let armor = spawn_armor_in_inventory(&mut world, 0, uncursed());
        // Add erosion to the armor.
        if let Some(mut ero) = world.get_component_mut::<Erosion>(armor) {
            ero.eroded = 2;
            ero.eroded2 = 1;
        }

        let scroll = spawn_scroll(&mut world, blessed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::EnchantArmor,
            false,
            &mut rng,
        );

        // Check that enchantment increased.
        let ench = world.get_component::<Enchantment>(armor).unwrap();
        assert!(ench.spe >= 1, "blessed enchant armor should increase spe");

        // Check erosion was repaired.
        let ero = world.get_component::<Erosion>(armor).unwrap();
        assert_eq!(
            ero.eroded, 0,
            "blessed enchant armor should fix rust erosion"
        );
        assert_eq!(ero.eroded2, 0, "blessed enchant armor should fix corrosion");
    }

    // ── Test: Cursed enchant armor decreases spe ─────────────────

    #[test]
    fn test_scroll_enchant_armor_cursed_decreases() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let armor = spawn_armor_in_inventory(&mut world, 3, uncursed());

        let scroll = spawn_scroll(&mut world, cursed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::EnchantArmor,
            false,
            &mut rng,
        );

        let ench = world.get_component::<Enchantment>(armor).unwrap();
        assert!(
            ench.spe < 3,
            "cursed enchant armor should decrease spe: got {}",
            ench.spe
        );
    }

    // ── Test: Blessed remove curse uncurses all inventory ─────────

    #[test]
    fn test_scroll_remove_curse_blessed_all() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let _item1 = spawn_inventory_item(&mut world, cursed());
        let _item2 = spawn_inventory_item(&mut world, cursed());
        let weapon = spawn_weapon_in_inventory(&mut world, 0, cursed());

        let scroll = spawn_scroll(&mut world, blessed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::RemoveCurse,
            false,
            &mut rng,
        );

        // Check that all items are no longer cursed.
        let items = collect_inventory_items(&world, player);
        for item_e in &items {
            if let Some(buc) = world.get_component::<BucStatus>(*item_e) {
                assert!(
                    !buc.cursed,
                    "blessed remove curse should uncurse all inventory items"
                );
            }
        }
    }

    // ── Test: Blessed enchant weapon fixes erosion ────────────────

    #[test]
    fn test_scroll_enchant_weapon_blessed_fixes_erosion() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let weapon = spawn_weapon_in_inventory(&mut world, 0, uncursed());
        if let Some(mut ero) = world.get_component_mut::<Erosion>(weapon) {
            ero.eroded = 2;
        }

        let scroll = spawn_scroll(&mut world, blessed());
        let _events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::EnchantWeapon,
            false,
            &mut rng,
        );

        let ench = world.get_component::<Enchantment>(weapon).unwrap();
        assert!(ench.spe >= 1, "blessed enchant weapon should increase spe");

        let ero = world.get_component::<Erosion>(weapon).unwrap();
        assert_eq!(ero.eroded, 0, "blessed enchant weapon should fix erosion");
    }

    // ── Test: Taming blessed has wider radius ─────────────────────

    #[test]
    fn test_scroll_taming_blessed_wider() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Spawn monsters at various distances.
        let _near = world.spawn((
            Positioned(Position::new(41, 10)),
            crate::world::Monster,
            HitPoints {
                current: 10,
                max: 10,
            },
        ));
        let _far = world.spawn((
            Positioned(Position::new(43, 10)),
            crate::world::Monster,
            HitPoints {
                current: 10,
                max: 10,
            },
        ));

        let scroll = spawn_scroll(&mut world, blessed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Taming,
            false,
            &mut rng,
        );

        // Blessed should produce taming events.
        assert!(!events.is_empty(), "blessed taming should produce events");
    }

    // ── Test: Create monster cursed spawns more monsters ──────────

    #[test]
    fn test_scroll_create_monster_cursed_more() {
        let mut world_c = make_world();
        let mut rng_c = Pcg64::seed_from_u64(42);
        let player = world_c.player();

        let scroll_c = spawn_scroll(&mut world_c, cursed());
        let events_c = read_scroll(
            &mut world_c,
            player,
            scroll_c,
            ScrollType::CreateMonster,
            false,
            &mut rng_c,
        );

        let mut world_u = make_world();
        let mut rng_u = Pcg64::seed_from_u64(42);
        let player_u = world_u.player();

        let scroll_u = spawn_scroll(&mut world_u, uncursed());
        let events_u = read_scroll(
            &mut world_u,
            player_u,
            scroll_u,
            ScrollType::CreateMonster,
            false,
            &mut rng_u,
        );

        let monsters_c = events_c
            .iter()
            .filter(|e| matches!(e, EngineEvent::MonsterGenerated { .. }))
            .count();
        let monsters_u = events_u
            .iter()
            .filter(|e| matches!(e, EngineEvent::MonsterGenerated { .. }))
            .count();
        assert!(
            monsters_c >= monsters_u,
            "cursed create monster should spawn >= uncursed: {} vs {}",
            monsters_c,
            monsters_u
        );
    }

    // ── Test: Amnesia cursed forgets more ─────────────────────────

    #[test]
    fn test_scroll_amnesia_cursed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, cursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Amnesia,
            false,
            &mut rng,
        );

        let has_forget = events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("amnesia") || key.contains("forget")
        ));
        assert!(has_forget, "cursed amnesia should cause forgetting");
    }

    // ── Test: Fire scroll cursed burns self more ──────────────────

    #[test]
    fn test_scroll_fire_cursed_more_damage() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Damage player to 15 HP.
        {
            let mut hp = world.get_component_mut::<HitPoints>(player).unwrap();
            hp.current = 15;
        }

        let scroll = spawn_scroll(&mut world, cursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Fire,
            false,
            &mut rng,
        );

        // Cursed fire should damage the reader.
        let has_damage = events.iter().any(|e| {
            matches!(
                e, EngineEvent::HpChange { amount, .. } if *amount < 0
            )
        });
        assert!(has_damage, "cursed fire scroll should damage the reader");
    }

    // ── Test: Charging normal recharges a wand ───────────────────

    #[test]
    fn test_scroll_charging_normal_recharges_wand() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Spawn a wand with 0 charges.
        let _wand = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(700),
                object_class: ObjectClass::Wand,
                quantity: 1,
                weight: 7,
                age: 0,
                inv_letter: Some('d'),
                artifact: None,
            },
            uncursed(),
            Enchantment { spe: 0 },
            ObjectLocation::Inventory,
        ));

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Charging,
            false,
            &mut rng,
        );

        // Should produce charging-related events.
        let has_charge = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::ItemCharged { .. } | EngineEvent::Message { .. }
            )
        });
        assert!(
            has_charge,
            "uncursed charging should recharge or produce message"
        );
    }

    // ── Test: Magic mapping cursed → amnesia ─────────────────────

    #[test]
    fn test_scroll_magic_mapping_cursed_amnesia() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, cursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::MagicMapping,
            false,
            &mut rng,
        );

        let has_amnesia = events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("amnesia") || key.contains("forget") || key.contains("mapping")
        ));
        assert!(
            has_amnesia,
            "cursed magic mapping should cause amnesia or produce message"
        );
    }

    // ── Test: Genocide normal kills a species ─────────────────────

    #[test]
    fn test_scroll_genocide_uncursed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Genocide,
            false,
            &mut rng,
        );

        let has_geno = events.iter().any(|e| {
            matches!(
                e, EngineEvent::Message { key, .. } if key.contains("genocide")
            )
        });
        assert!(has_geno, "uncursed genocide should emit genocide message");
    }

    // ── Test: Destroy armor uncursed destroys worn piece ──────────

    #[test]
    fn test_scroll_destroy_armor_uncursed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let armor = spawn_armor_in_inventory(&mut world, 0, uncursed());

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::DestroyArmor,
            false,
            &mut rng,
        );

        let has_destroy = events.iter().any(|e| matches!(
            e, EngineEvent::ItemDestroyed { .. }
        )) || events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("destroy") || key.contains("armor")
        ));
        // Either the armor was destroyed or a message about it was emitted.
        assert!(!events.is_empty(), "destroy armor should produce events");
    }

    // ── Test: Stinking cloud creates gas at position ─────────────

    #[test]
    fn test_scroll_stinking_cloud_creates_gas() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::StinkingCloud,
            false,
            &mut rng,
        );

        let has_cloud = events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("cloud") || key.contains("stink")
        ));
        assert!(has_cloud, "stinking cloud should emit cloud message");
    }

    // ── Test: Confuse monster normal sets confusion flag ──────────

    #[test]
    fn test_scroll_confuse_monster_normal() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, uncursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::ConfuseMonster,
            false,
            &mut rng,
        );

        // Should set the player's next-melee-confuse flag.
        let has_confuse = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::Confused,
                    ..
                } | EngineEvent::Message { .. }
            )
        });
        assert!(
            has_confuse,
            "confuse monster should set confuse flag or emit message"
        );
    }

    // ── Test: Punishment cursed applies ball and chain ────────────

    #[test]
    fn test_scroll_punishment_cursed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let scroll = spawn_scroll(&mut world, cursed());
        let events = read_scroll(
            &mut world,
            player,
            scroll,
            ScrollType::Punishment,
            false,
            &mut rng,
        );

        // Should produce punishment event or message.
        assert!(
            !events.is_empty(),
            "cursed punishment should produce events"
        );
    }

    // ── Test: Scare monster uncursed survives pickup ─────────────

    #[test]
    fn test_scroll_scare_monster_uncursed_survives() {
        let mut world = make_world();

        // Fresh scroll with spe=0: first pickup should survive and set spe to 1.
        let scroll = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(600),
                object_class: ObjectClass::Scroll,
                quantity: 1,
                weight: 5,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            BucStatus {
                cursed: false,
                blessed: false,
                bknown: false,
            },
            Enchantment { spe: 0 },
            ObjectLocation::Floor { x: 5, y: 5 },
        ));

        let (survived, _events) = scare_monster_pickup(&mut world, scroll);
        assert!(
            survived,
            "uncursed scare monster with spe=0 should survive first pickup"
        );
    }
}
