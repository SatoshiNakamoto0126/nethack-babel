//! Dipping system: dip items into potions, holy/unholy water, and fountains.
//!
//! Implements the NetHack 3.7 dipping mechanics from `src/potion.c` (dodip).
//! All functions operate on `GameWorld` and return `Vec<EngineEvent>`
//! for the game loop to process.  No IO.

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{ArtifactId, BucStatus, ObjectClass, ObjectCore, Erosion};

use crate::artifacts::{try_create_excalibur, ExcaliburResult};
use crate::dungeon::Terrain;
use crate::event::{DamageCause, EngineEvent};
use crate::potions::PotionType;
use crate::world::{ExperienceLevel, GameWorld, Positioned};

// ---------------------------------------------------------------------------
// Alchemy table
// ---------------------------------------------------------------------------

/// Look up the result of mixing two potion types.
///
/// Mirrors the full C NetHack `mixtype()` alchemy table from `potion.c`.
/// Order-independent: both orderings are checked.
///
/// Returns `None` if the combination has no alchemical result.
pub fn alchemy_result(a: PotionType, b: PotionType) -> Option<PotionType> {
    // Try (a, b) then (b, a) via the directed lookup.
    alchemy_directed(a, b).or_else(|| alchemy_directed(b, a))
}

/// One-directional alchemy lookup (o1=first, o2=second).
fn alchemy_directed(o1: PotionType, o2: PotionType) -> Option<PotionType> {
    match o1 {
        // Healing chain: healing + speed → extra healing
        PotionType::Healing => match o2 {
            PotionType::Speed => Some(PotionType::ExtraHealing),
            PotionType::GainLevel | PotionType::GainEnergy => Some(PotionType::ExtraHealing),
            PotionType::Sickness => Some(PotionType::FruitJuice),
            PotionType::Hallucination | PotionType::Blindness | PotionType::Confusion => {
                Some(PotionType::Water)
            }
            _ => None,
        },
        PotionType::ExtraHealing => match o2 {
            PotionType::GainLevel | PotionType::GainEnergy => Some(PotionType::FullHealing),
            PotionType::Sickness => Some(PotionType::FruitJuice),
            PotionType::Hallucination | PotionType::Blindness | PotionType::Confusion => {
                Some(PotionType::Water)
            }
            _ => None,
        },
        PotionType::FullHealing => match o2 {
            PotionType::GainLevel | PotionType::GainEnergy => Some(PotionType::GainAbility),
            PotionType::Sickness => Some(PotionType::FruitJuice),
            PotionType::Hallucination | PotionType::Blindness | PotionType::Confusion => {
                Some(PotionType::Water)
            }
            _ => None,
        },
        // Gain level / gain energy chain
        PotionType::GainLevel | PotionType::GainEnergy => match o2 {
            PotionType::Confusion => Some(PotionType::Booze), // 2/3 booze, 1/3 enlightenment
            PotionType::Healing => Some(PotionType::ExtraHealing),
            PotionType::ExtraHealing => Some(PotionType::FullHealing),
            PotionType::FullHealing => Some(PotionType::GainAbility),
            PotionType::FruitJuice => Some(PotionType::SeeInvisible),
            PotionType::Booze => Some(PotionType::Hallucination),
            _ => None,
        },
        // Fruit juice chain
        PotionType::FruitJuice => match o2 {
            PotionType::Sickness => Some(PotionType::Sickness),
            PotionType::Enlightenment | PotionType::Speed => Some(PotionType::Booze),
            PotionType::GainLevel | PotionType::GainEnergy => Some(PotionType::SeeInvisible),
            _ => None,
        },
        // Enlightenment chain
        PotionType::Enlightenment => match o2 {
            PotionType::Levitation => Some(PotionType::GainLevel), // 2/3 chance
            PotionType::FruitJuice => Some(PotionType::Booze),
            PotionType::Booze => Some(PotionType::Confusion),
            _ => None,
        },
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// PotionType ↔ ObjectTypeId mapping
// ---------------------------------------------------------------------------

/// Convert a `PotionType` to a synthetic `ObjectTypeId`.
///
/// These IDs are stable within the engine and match the ordering used
/// in the test harness for potion objects.  The exact offset (200) is
/// arbitrary but consistent — no real object shares this range.
fn potion_otyp(pt: PotionType) -> nethack_babel_data::ObjectTypeId {
    let idx: u16 = match pt {
        PotionType::GainAbility => 0,
        PotionType::RestoreAbility => 1,
        PotionType::Confusion => 2,
        PotionType::Blindness => 3,
        PotionType::Paralysis => 4,
        PotionType::Speed => 5,
        PotionType::Levitation => 6,
        PotionType::Hallucination => 7,
        PotionType::Invisibility => 8,
        PotionType::SeeInvisible => 9,
        PotionType::Healing => 10,
        PotionType::ExtraHealing => 11,
        PotionType::GainLevel => 12,
        PotionType::Enlightenment => 13,
        PotionType::MonsterDetection => 14,
        PotionType::ObjectDetection => 15,
        PotionType::GainEnergy => 16,
        PotionType::Sleeping => 17,
        PotionType::FullHealing => 18,
        PotionType::Polymorph => 19,
        PotionType::Booze => 20,
        PotionType::Sickness => 21,
        PotionType::FruitJuice => 22,
        PotionType::Acid => 23,
        PotionType::Oil => 24,
        PotionType::Water => 25,
    };
    nethack_babel_data::ObjectTypeId(200 + idx)
}

// ---------------------------------------------------------------------------
// Core dip function: item into another item (potion)
// ---------------------------------------------------------------------------

/// Dip `item` into `into` (a potion entity).
///
/// Handles:
/// - Holy water (blessed potion of water) → blesses item
/// - Unholy water (cursed potion of water) → curses item
/// - Potion of water (uncursed) → dilutes if item is a potion
/// - Potion of sickness + weapon → poisons weapon
/// - Alchemy: two potions produce a third
///
/// Both the dipped item and the target potion may be consumed or
/// transformed depending on the interaction.
pub fn dip_item<R: Rng>(
    world: &mut GameWorld,
    _player: Entity,
    item: Entity,
    into: Entity,
    _rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // ── Validate the "into" entity is a potion ──────────────────
    let into_class = match world.get_component::<ObjectCore>(into) {
        Some(core) => core.object_class,
        None => {
            events.push(EngineEvent::msg("dip-not-a-potion"));
            return events;
        }
    };
    if into_class != ObjectClass::Potion {
        events.push(EngineEvent::msg("dip-not-a-potion"));
        return events;
    }

    // Read BUC of the target potion.
    let into_buc = match world.get_component::<BucStatus>(into) {
        Some(b) => BucStatus {
            cursed: b.cursed,
            blessed: b.blessed,
            bknown: b.bknown,
        },
        None => BucStatus {
            cursed: false,
            blessed: false,
            bknown: false,
        },
    };

    // Determine the potion type of `into`.
    // We use a marker component `DipPotionType` if present, or fall back
    // to checking the entity for known potion-type markers.
    let into_potion_type = world
        .get_component::<DipPotionType>(into)
        .map(|d| d.0);

    // ── Holy water: blessed potion of water → bless the item ────
    if into_potion_type == Some(PotionType::Water) && into_buc.blessed {
        // Bless the item.
        if let Some(mut buc) = world.get_component_mut::<BucStatus>(item) {
            buc.blessed = true;
            buc.cursed = false;
        }
        events.push(EngineEvent::msg("dip-holy-water"));
        // Consume the holy water.
        let _ = world.despawn(into);
        events.push(EngineEvent::ItemDestroyed {
            item: into,
            cause: DamageCause::Physical,
        });
        return events;
    }

    // ── Unholy water: cursed potion of water → curse the item ───
    if into_potion_type == Some(PotionType::Water) && into_buc.cursed {
        // Curse the item.
        if let Some(mut buc) = world.get_component_mut::<BucStatus>(item) {
            buc.cursed = true;
            buc.blessed = false;
        }
        events.push(EngineEvent::msg("dip-unholy-water"));
        // Consume the unholy water.
        let _ = world.despawn(into);
        events.push(EngineEvent::ItemDestroyed {
            item: into,
            cause: DamageCause::Physical,
        });
        return events;
    }

    // ── Uncursed water: dilute a potion ─────────────────────────
    if into_potion_type == Some(PotionType::Water) && !into_buc.blessed && !into_buc.cursed {
        let item_class = world
            .get_component::<ObjectCore>(item)
            .map(|c| c.object_class);
        if item_class == Some(ObjectClass::Potion) {
            // Mark the dipped potion as diluted by changing it to water.
            if let Some(mut pt) = world.get_component_mut::<DipPotionType>(item) {
                pt.0 = PotionType::Water;
            }
            events.push(EngineEvent::msg("dip-diluted"));
            // Consume the water.
            let _ = world.despawn(into);
            events.push(EngineEvent::ItemDestroyed {
                item: into,
                cause: DamageCause::Physical,
            });
        } else {
            events.push(EngineEvent::msg("dip-nothing-happens"));
        }
        return events;
    }

    // ── Unicorn horn dip: cure bad potions ─────────────────────
    // C mixtype(): UNICORN_HORN dipped into sickness/hallucination/
    // blindness/confusion potions produces fruit_juice or water.
    if let Some(item_tag) = world.get_component::<DipItemTag>(item).map(|t| t.0) {
        if item_tag == DipItemKind::UnicornHorn {
            if let Some(into_pt) = into_potion_type {
                let result = match into_pt {
                    PotionType::Sickness => Some(PotionType::FruitJuice),
                    PotionType::Hallucination
                    | PotionType::Blindness
                    | PotionType::Confusion => Some(PotionType::Water),
                    _ => None,
                };
                if let Some(result_type) = result {
                    // Transform the target potion into the result.
                    if let Some(mut pt) = world.get_component_mut::<DipPotionType>(into) {
                        pt.0 = result_type;
                    }
                    events.push(EngineEvent::msg("dip-unicorn-horn-cure"));
                    return events;
                }
            }
        }
        // Amethyst dip: "a-methyst" == "not intoxicated"
        // C mixtype(): AMETHYST dipped into booze → fruit juice.
        if item_tag == DipItemKind::Amethyst {
            if into_potion_type == Some(PotionType::Booze) {
                if let Some(mut pt) = world.get_component_mut::<DipPotionType>(into) {
                    pt.0 = PotionType::FruitJuice;
                }
                events.push(EngineEvent::msg("dip-amethyst-cure"));
                return events;
            }
        }
    }

    // ── Acid potion: remove erosion from metallic items ─────────
    if into_potion_type == Some(PotionType::Acid) {
        let item_class = world
            .get_component::<ObjectCore>(item)
            .map(|c| c.object_class);
        if matches!(
            item_class,
            Some(ObjectClass::Weapon) | Some(ObjectClass::Armor) | Some(ObjectClass::Tool)
        ) {
            if let Some(mut ero) = world.get_component_mut::<Erosion>(item) {
                if ero.eroded > 0 || ero.eroded2 > 0 {
                    ero.eroded = 0;
                    ero.eroded2 = 0;
                    events.push(EngineEvent::msg("dip-acid-repair"));
                } else {
                    events.push(EngineEvent::msg("dip-acid-nothing"));
                }
            } else {
                events.push(EngineEvent::msg("dip-acid-nothing"));
            }
            // Consume the acid.
            let _ = world.despawn(into);
            events.push(EngineEvent::ItemDestroyed {
                item: into,
                cause: DamageCause::Physical,
            });
            return events;
        }
    }

    // ── Poison weapon: dip weapon into potion of sickness ───────
    if into_potion_type == Some(PotionType::Sickness) {
        let item_class = world
            .get_component::<ObjectCore>(item)
            .map(|c| c.object_class);
        if item_class == Some(ObjectClass::Weapon) {
            // Add a Poisoned marker to the weapon.
            let _ = world.ecs_mut().insert_one(item, Poisoned { uses: 3 });
            events.push(EngineEvent::msg("dip-poison-weapon"));
            // Consume the sickness potion.
            let _ = world.despawn(into);
            events.push(EngineEvent::ItemDestroyed {
                item: into,
                cause: DamageCause::Physical,
            });
            return events;
        }
    }

    // ── Alchemy: both items are potions → produce a result ──────
    let item_class = world
        .get_component::<ObjectCore>(item)
        .map(|c| c.object_class);
    let item_potion_type = world
        .get_component::<DipPotionType>(item)
        .map(|d| d.0);

    if item_class == Some(ObjectClass::Potion)
        && let (Some(a_type), Some(b_type)) = (item_potion_type, into_potion_type)
            && let Some(result_type) = alchemy_result(a_type, b_type) {
                // BUC of the result comes from the dipped potion (item).
                let item_buc = match world.get_component::<BucStatus>(item) {
                    Some(b) => BucStatus {
                        cursed: b.cursed,
                        blessed: b.blessed,
                        bknown: b.bknown,
                    },
                    None => BucStatus {
                        cursed: false,
                        blessed: false,
                        bknown: false,
                    },
                };

                // Despawn both potions.
                let _ = world.despawn(item);
                events.push(EngineEvent::ItemDestroyed {
                    item,
                    cause: DamageCause::Physical,
                });
                let _ = world.despawn(into);
                events.push(EngineEvent::ItemDestroyed {
                    item: into,
                    cause: DamageCause::Physical,
                });

                // Spawn the result potion.
                let result_entity = world.spawn((
                    ObjectCore {
                        otyp: potion_otyp(result_type),
                        object_class: ObjectClass::Potion,
                        quantity: 1,
                        weight: 20,
                        age: 0,
                        inv_letter: None,
                        artifact: None,
                    },
                    item_buc,
                    DipPotionType(result_type),
                    nethack_babel_data::ObjectLocation::Inventory,
                ));

                events.push(EngineEvent::msg_with(
                    "dip-alchemy",
                    vec![("result", format!("{:?}", result_type))],
                ));
                events.push(EngineEvent::ItemPickedUp {
                    actor: _player,
                    item: result_entity,
                    quantity: 1,
                });

                return events;
            }

    // ── No special interaction ──────────────────────────────────
    events.push(EngineEvent::msg("dip-nothing-happens"));
    events
}

// ---------------------------------------------------------------------------
// Fountain dipping
// ---------------------------------------------------------------------------

/// Dip an item into a fountain at the player's position.
///
/// Handles:
/// - Long sword + Lawful + level >= 5 → Excalibur (via `try_create_excalibur`)
/// - Otherwise: may rust the weapon, etc.
pub fn dip_fountain<R: Rng>(
    world: &mut GameWorld,
    player: Entity,
    item: Entity,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // ── Verify player is on a fountain ──────────────────────────
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => {
            events.push(EngineEvent::msg("dip-no-fountain"));
            return events;
        }
    };
    let terrain = world.dungeon().current_level.get(player_pos);
    if terrain.map(|c| c.terrain) != Some(Terrain::Fountain) {
        events.push(EngineEvent::msg("dip-no-fountain"));
        return events;
    }

    // ── Get item info ───────────────────────────────────────────
    let item_otyp = match world.get_component::<ObjectCore>(item) {
        Some(core) => core.otyp,
        None => {
            events.push(EngineEvent::msg("dip-nothing-happens"));
            return events;
        }
    };

    // ── Get player info for Excalibur check ─────────────────────
    let player_level = world
        .get_component::<ExperienceLevel>(player)
        .map(|el| el.0)
        .unwrap_or(1);

    // Read alignment and role from PlayerIdentity if present.
    let (alignment, is_knight) = world
        .get_component::<nethack_babel_data::PlayerIdentity>(player)
        .map(|id| {
            let is_knight = id.role == nethack_babel_data::RoleId(10); // Knight
            (id.alignment, is_knight)
        })
        .unwrap_or((nethack_babel_data::Alignment::Neutral, false));

    // Check whether Excalibur already exists in the world.
    let excalibur_exists = world
        .query::<ObjectCore>()
        .iter()
        .any(|(_, core)| core.artifact == Some(ArtifactId(1)));

    // ── Attempt Excalibur creation ──────────────────────────────
    let result = try_create_excalibur(
        item_otyp,
        player_level,
        alignment,
        is_knight,
        excalibur_exists,
        rng,
    );

    match result {
        ExcaliburResult::Success => {
            // Transform the weapon into Excalibur: blessed, erodeproof, artifact.
            if let Some(mut core) = world.get_component_mut::<ObjectCore>(item) {
                core.artifact = Some(ArtifactId(1));
            }
            if let Some(mut buc) = world.get_component_mut::<BucStatus>(item) {
                buc.blessed = true;
                buc.cursed = false;
                buc.bknown = true;
            }
            if let Some(mut ero) = world.get_component_mut::<Erosion>(item) {
                ero.erodeproof = true;
            }
            events.push(EngineEvent::msg("dip-excalibur"));
        }
        ExcaliburResult::Cursed => {
            // Non-lawful: the sword gets cursed.
            if let Some(mut buc) = world.get_component_mut::<BucStatus>(item) {
                buc.cursed = true;
                buc.blessed = false;
            }
            events.push(EngineEvent::msg("dip-fountain-cursed"));
        }
        ExcaliburResult::NoEffect | ExcaliburResult::Invalid => {
            // Generic fountain effects: may rust weapon, etc.
            let item_class = world
                .get_component::<ObjectCore>(item)
                .map(|c| c.object_class);
            if item_class == Some(ObjectClass::Weapon) {
                // 1/3 chance of rusting the weapon.
                if rng.random_range(0..3) == 0
                    && let Some(mut ero) = world.get_component_mut::<Erosion>(item)
                        && !ero.erodeproof && ero.eroded < 3 {
                            ero.eroded += 1;
                            events.push(EngineEvent::msg("dip-fountain-rust"));
                            events.push(EngineEvent::ItemDamaged {
                                item,
                                cause: DamageCause::Rust,
                            });
                        }
            }
            if events.is_empty() {
                events.push(EngineEvent::msg("dip-fountain-nothing"));
            }
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Marker components
// ---------------------------------------------------------------------------

/// Marker component that stores the potion type of an item entity.
///
/// Attached to potion entities so the dip system can determine what
/// kind of potion it is without relying on ObjectTypeId lookup tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DipPotionType(pub PotionType);

/// Marker component indicating a weapon has been poisoned by dipping.
#[derive(Debug, Clone, Copy)]
pub struct Poisoned {
    /// Number of poisoned uses remaining.
    pub uses: u8,
}

/// Item kinds relevant to dip interactions (non-potion reagents).
///
/// In C NetHack, certain non-potion items act as alchemy reagents when
/// dipped into potions (unicorn horn cures bad potions, amethyst cures
/// booze).  This component tags items that participate in these interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DipItemKind {
    /// Unicorn horn: cures sickness, hallucination, blindness, confusion.
    UnicornHorn,
    /// Amethyst gem: "a-methyst" == "not intoxicated" — cures booze.
    Amethyst,
}

/// Marker component tagging an item for special dip interactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DipItemTag(pub DipItemKind);

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use nethack_babel_data::{Alignment, BucStatus, ObjectClass, ObjectCore, ObjectTypeId, Erosion};
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

    /// Spawn a potion with the given type and BUC status in the player's
    /// inventory.
    fn spawn_potion(
        world: &mut GameWorld,
        pt: PotionType,
        buc: BucStatus,
    ) -> Entity {
        world.spawn((
            ObjectCore {
                otyp: potion_otyp(pt),
                object_class: ObjectClass::Potion,
                quantity: 1,
                weight: 20,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            buc,
            DipPotionType(pt),
            nethack_babel_data::ObjectLocation::Inventory,
        ))
    }

    /// Spawn a generic weapon in inventory.
    fn spawn_weapon(world: &mut GameWorld, buc: BucStatus) -> Entity {
        world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(28), // long sword
                object_class: ObjectClass::Weapon,
                quantity: 1,
                weight: 40,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            buc,
            Erosion {
                eroded: 0,
                eroded2: 0,
                erodeproof: false,
                greased: false,
            },
            nethack_babel_data::ObjectLocation::Inventory,
        ))
    }

    /// Spawn a generic non-weapon, non-potion item.
    fn spawn_generic_item(world: &mut GameWorld, buc: BucStatus) -> Entity {
        world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(50),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            buc,
            nethack_babel_data::ObjectLocation::Inventory,
        ))
    }

    // ── Test: dip into holy water blesses item ──────────────────

    #[test]
    fn test_dip_holy_water_blesses() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let item = spawn_generic_item(&mut world, uncursed());
        let holy_water = spawn_potion(&mut world, PotionType::Water, blessed());

        let events = dip_item(&mut world, player, item, holy_water, &mut rng);

        // Item should now be blessed.
        let buc = world.get_component::<BucStatus>(item).unwrap();
        assert!(buc.blessed, "item should be blessed after dipping in holy water");
        assert!(!buc.cursed, "item should not be cursed");

        // Holy water should be consumed (despawned).
        assert!(
            world.get_component::<ObjectCore>(holy_water).is_none(),
            "holy water should be consumed"
        );

        // Should have relevant message.
        let has_msg = events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-holy-water"));
        assert!(has_msg, "should emit dip-holy-water message");
    }

    // ── Test: dip into unholy water curses item ─────────────────

    #[test]
    fn test_dip_unholy_water_curses() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let item = spawn_generic_item(&mut world, uncursed());
        let unholy_water = spawn_potion(&mut world, PotionType::Water, cursed());

        let events = dip_item(&mut world, player, item, unholy_water, &mut rng);

        // Item should now be cursed.
        let buc = world.get_component::<BucStatus>(item).unwrap();
        assert!(buc.cursed, "item should be cursed after dipping in unholy water");
        assert!(!buc.blessed, "item should not be blessed");

        // Unholy water should be consumed.
        assert!(
            world.get_component::<ObjectCore>(unholy_water).is_none(),
            "unholy water should be consumed"
        );

        let has_msg = events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-unholy-water"));
        assert!(has_msg, "should emit dip-unholy-water message");
    }

    // ── Test: alchemy healing + speed = extra healing ───────────

    #[test]
    fn test_alchemy_healing_plus_speed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let healing = spawn_potion(&mut world, PotionType::Healing, uncursed());
        let speed = spawn_potion(&mut world, PotionType::Speed, uncursed());

        let events = dip_item(&mut world, player, healing, speed, &mut rng);

        // Both originals should be consumed.
        assert!(
            world.get_component::<ObjectCore>(healing).is_none(),
            "healing potion should be consumed"
        );
        assert!(
            world.get_component::<ObjectCore>(speed).is_none(),
            "speed potion should be consumed"
        );

        // A new potion of extra healing should exist.
        let mut found_extra_healing = false;
        for (_, pt) in world.query::<DipPotionType>().iter() {
            if pt.0 == PotionType::ExtraHealing {
                found_extra_healing = true;
            }
        }
        assert!(found_extra_healing, "alchemy should produce extra healing potion");

        // Should have alchemy message.
        let has_msg = events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-alchemy"));
        assert!(has_msg, "should emit dip-alchemy message");
    }

    // ── Test: alchemy consumes both potions ─────────────────────

    #[test]
    fn test_alchemy_consumes_both() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // C mixtype: gain_level + healing → extra_healing
        let gain_level = spawn_potion(&mut world, PotionType::GainLevel, uncursed());
        let healing = spawn_potion(&mut world, PotionType::Healing, uncursed());

        let _events = dip_item(&mut world, player, gain_level, healing, &mut rng);

        // Both originals must be gone.
        assert!(
            world.get_component::<ObjectCore>(gain_level).is_none(),
            "gain level potion should be consumed"
        );
        assert!(
            world.get_component::<ObjectCore>(healing).is_none(),
            "healing potion should be consumed"
        );

        // Result potion (extra healing) should exist.
        let mut found = false;
        for (_, pt) in world.query::<DipPotionType>().iter() {
            if pt.0 == PotionType::ExtraHealing {
                found = true;
            }
        }
        assert!(found, "alchemy should produce extra healing potion");
    }

    // ── Test: alchemy BUC inherited from dipped potion ──────────

    #[test]
    fn test_alchemy_buc_from_dipped() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // The dipped potion (healing) is blessed; target (speed) is cursed.
        let healing = spawn_potion(&mut world, PotionType::Healing, blessed());
        let speed = spawn_potion(&mut world, PotionType::Speed, cursed());

        let _events = dip_item(&mut world, player, healing, speed, &mut rng);

        // Result should inherit the BUC of the dipped potion (blessed).
        for (entity, pt) in world.query::<DipPotionType>().iter() {
            if pt.0 == PotionType::ExtraHealing {
                let buc = world.get_component::<BucStatus>(entity).unwrap();
                assert!(
                    buc.blessed,
                    "result potion should inherit blessed from dipped potion"
                );
                assert!(
                    !buc.cursed,
                    "result potion should not be cursed"
                );
            }
        }
    }

    // ── Test: dip weapon into potion of sickness → poisoned ─────

    #[test]
    fn test_dip_poison_weapon() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let weapon = spawn_weapon(&mut world, uncursed());
        let sickness = spawn_potion(&mut world, PotionType::Sickness, uncursed());

        let events = dip_item(&mut world, player, weapon, sickness, &mut rng);

        // Weapon should now have a Poisoned component.
        let poisoned = world.get_component::<Poisoned>(weapon);
        assert!(poisoned.is_some(), "weapon should be poisoned after dipping");
        assert_eq!(poisoned.unwrap().uses, 3, "poison should have 3 uses");

        // Sickness potion should be consumed.
        assert!(
            world.get_component::<ObjectCore>(sickness).is_none(),
            "sickness potion should be consumed"
        );

        let has_msg = events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-poison-weapon"));
        assert!(has_msg, "should emit dip-poison-weapon message");
    }

    // ── Test: dip into uncursed water dilutes potion ────────────

    #[test]
    fn test_dip_water_dilutes() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let potion = spawn_potion(&mut world, PotionType::Healing, uncursed());
        let water = spawn_potion(&mut world, PotionType::Water, uncursed());

        let events = dip_item(&mut world, player, potion, water, &mut rng);

        // The potion should now be water (diluted).
        let pt = world.get_component::<DipPotionType>(potion).unwrap();
        assert_eq!(
            pt.0,
            PotionType::Water,
            "potion should be diluted to water"
        );

        // Water potion should be consumed.
        assert!(
            world.get_component::<ObjectCore>(water).is_none(),
            "water potion should be consumed"
        );

        let has_msg = events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-diluted"));
        assert!(has_msg, "should emit dip-diluted message");
    }

    // ── Test: fountain dip → Excalibur (lawful, level 5) ────────

    #[test]
    fn test_dip_sword_fountain_excalibur() {
        let mut world = make_world();
        let player = world.player();

        // Set player to level 5.
        {
            let mut el = world.get_component_mut::<ExperienceLevel>(player).unwrap();
            el.0 = 5;
        }

        // Add PlayerIdentity with Lawful alignment and Knight role.
        let identity = nethack_babel_data::PlayerIdentity {
            name: "TestHero".to_string(),
            role: nethack_babel_data::RoleId(10), // Knight
            race: nethack_babel_data::RaceId(0),
            gender: nethack_babel_data::Gender::Male,
            alignment: Alignment::Lawful,
            alignment_base: [Alignment::Lawful, Alignment::Lawful],
            handedness: nethack_babel_data::Handedness::RightHanded,
        };
        let _ = world.ecs_mut().insert_one(player, identity);

        // Place a fountain under the player.
        let player_pos = world.get_component::<Positioned>(player).unwrap().0;
        if let Some(cell) = world.dungeon_mut().current_level.get_mut(player_pos) {
            cell.terrain = Terrain::Fountain;
        }

        // Spawn a long sword.
        let sword = spawn_weapon(&mut world, uncursed());

        // Try many seeds until Excalibur succeeds (1/6 for knight).
        let mut found_excalibur = false;
        for seed in 0..200u64 {
            // Reset sword state.
            if let Some(mut core) = world.get_component_mut::<ObjectCore>(sword) {
                core.artifact = None;
            }
            if let Some(mut buc) = world.get_component_mut::<BucStatus>(sword) {
                buc.blessed = false;
                buc.cursed = false;
            }

            let mut rng = Pcg64::seed_from_u64(seed);
            let events = dip_fountain(&mut world, player, sword, &mut rng);

            if events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-excalibur")) {
                // Verify the sword is now Excalibur.
                let core = world.get_component::<ObjectCore>(sword).unwrap();
                assert_eq!(core.artifact, Some(ArtifactId(1)), "should be Excalibur artifact");

                let buc = world.get_component::<BucStatus>(sword).unwrap();
                assert!(buc.blessed, "Excalibur should be blessed");

                let ero = world.get_component::<Erosion>(sword).unwrap();
                assert!(ero.erodeproof, "Excalibur should be erodeproof");

                found_excalibur = true;
                break;
            }
        }
        assert!(found_excalibur, "lawful knight at level 5 should eventually get Excalibur");
    }

    // ── Test: fountain dip non-lawful → cursed ──────────────────

    #[test]
    fn test_dip_fountain_non_lawful() {
        let mut world = make_world();
        let player = world.player();

        // Set player to level 5.
        {
            let mut el = world.get_component_mut::<ExperienceLevel>(player).unwrap();
            el.0 = 5;
        }

        // Neutral alignment.
        let identity = nethack_babel_data::PlayerIdentity {
            name: "TestHero".to_string(),
            role: nethack_babel_data::RoleId(0), // not Knight
            race: nethack_babel_data::RaceId(0),
            gender: nethack_babel_data::Gender::Male,
            alignment: Alignment::Neutral,
            alignment_base: [Alignment::Neutral, Alignment::Neutral],
            handedness: nethack_babel_data::Handedness::RightHanded,
        };
        let _ = world.ecs_mut().insert_one(player, identity);

        // Fountain.
        let player_pos = world.get_component::<Positioned>(player).unwrap().0;
        if let Some(cell) = world.dungeon_mut().current_level.get_mut(player_pos) {
            cell.terrain = Terrain::Fountain;
        }

        let sword = spawn_weapon(&mut world, uncursed());

        // Non-lawful, non-knight (1/30): try many seeds for the cursed outcome.
        let mut found_cursed = false;
        for seed in 0..500u64 {
            if let Some(mut buc) = world.get_component_mut::<BucStatus>(sword) {
                buc.cursed = false;
                buc.blessed = false;
            }

            let mut rng = Pcg64::seed_from_u64(seed);
            let events = dip_fountain(&mut world, player, sword, &mut rng);

            if events.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-fountain-cursed")) {
                let buc = world.get_component::<BucStatus>(sword).unwrap();
                assert!(buc.cursed, "sword should be cursed");
                found_cursed = true;
                break;
            }
        }
        assert!(found_cursed, "non-lawful character should eventually get cursed sword");
    }

    // ── Test: alchemy extra healing + gain level = full healing ──

    #[test]
    fn test_alchemy_extra_healing_gain_level() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let extra_healing = spawn_potion(&mut world, PotionType::ExtraHealing, uncursed());
        let gain_level = spawn_potion(&mut world, PotionType::GainLevel, uncursed());

        let _events = dip_item(&mut world, player, extra_healing, gain_level, &mut rng);

        let mut found = false;
        for (_, pt) in world.query::<DipPotionType>().iter() {
            if pt.0 == PotionType::FullHealing {
                found = true;
            }
        }
        assert!(found, "extra healing + gain level should produce full healing");
    }

    // ── Test: alchemy healing + sickness = fruit juice ──────────

    #[test]
    fn test_alchemy_healing_plus_sickness() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let healing = spawn_potion(&mut world, PotionType::Healing, uncursed());
        let sickness = spawn_potion(&mut world, PotionType::Sickness, uncursed());

        let _events = dip_item(&mut world, player, healing, sickness, &mut rng);

        let mut found = false;
        for (_, pt) in world.query::<DipPotionType>().iter() {
            if pt.0 == PotionType::FruitJuice {
                found = true;
            }
        }
        assert!(found, "healing + sickness should produce fruit juice");
    }

    // ── Test: alchemy full healing + gain level = gain ability ───

    #[test]
    fn test_alchemy_full_healing_gain_level() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let full = spawn_potion(&mut world, PotionType::FullHealing, uncursed());
        let gain_level = spawn_potion(&mut world, PotionType::GainLevel, uncursed());

        let _events = dip_item(&mut world, player, full, gain_level, &mut rng);

        let mut found = false;
        for (_, pt) in world.query::<DipPotionType>().iter() {
            if pt.0 == PotionType::GainAbility {
                found = true;
            }
        }
        assert!(found, "full healing + gain level should produce gain ability");
    }

    // ── Test: alchemy gain level + fruit juice = see invisible ──

    #[test]
    fn test_alchemy_gain_level_fruit_juice() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let gain_level = spawn_potion(&mut world, PotionType::GainLevel, uncursed());
        let juice = spawn_potion(&mut world, PotionType::FruitJuice, uncursed());

        let _events = dip_item(&mut world, player, gain_level, juice, &mut rng);

        let mut found = false;
        for (_, pt) in world.query::<DipPotionType>().iter() {
            if pt.0 == PotionType::SeeInvisible {
                found = true;
            }
        }
        assert!(found, "gain level + fruit juice should produce see invisible");
    }

    // ── Test: alchemy gain level + booze = hallucination ────────

    #[test]
    fn test_alchemy_gain_level_booze() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let gain_level = spawn_potion(&mut world, PotionType::GainLevel, uncursed());
        let booze = spawn_potion(&mut world, PotionType::Booze, uncursed());

        let _events = dip_item(&mut world, player, gain_level, booze, &mut rng);

        let mut found = false;
        for (_, pt) in world.query::<DipPotionType>().iter() {
            if pt.0 == PotionType::Hallucination {
                found = true;
            }
        }
        assert!(found, "gain level + booze should produce hallucination");
    }

    // ── Test: alchemy healing + blindness = water (cure) ────────

    #[test]
    fn test_alchemy_healing_blindness_cures() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let healing = spawn_potion(&mut world, PotionType::Healing, uncursed());
        let blind = spawn_potion(&mut world, PotionType::Blindness, uncursed());

        let _events = dip_item(&mut world, player, healing, blind, &mut rng);

        let mut found = false;
        for (_, pt) in world.query::<DipPotionType>().iter() {
            if pt.0 == PotionType::Water {
                found = true;
            }
        }
        assert!(found, "healing + blindness should produce water (cure)");
    }

    // ── Test: alchemy enlightenment + booze = confusion ─────────

    #[test]
    fn test_alchemy_enlightenment_booze() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let enlightenment = spawn_potion(&mut world, PotionType::Enlightenment, uncursed());
        let booze = spawn_potion(&mut world, PotionType::Booze, uncursed());

        let _events = dip_item(&mut world, player, enlightenment, booze, &mut rng);

        let mut found = false;
        for (_, pt) in world.query::<DipPotionType>().iter() {
            if pt.0 == PotionType::Confusion {
                found = true;
            }
        }
        assert!(found, "enlightenment + booze should produce confusion");
    }

    // ── Test: alchemy fruit juice + speed = booze ───────────────

    #[test]
    fn test_alchemy_fruit_juice_speed() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let juice = spawn_potion(&mut world, PotionType::FruitJuice, uncursed());
        let speed = spawn_potion(&mut world, PotionType::Speed, uncursed());

        let _events = dip_item(&mut world, player, juice, speed, &mut rng);

        let mut found = false;
        for (_, pt) in world.query::<DipPotionType>().iter() {
            if pt.0 == PotionType::Booze {
                found = true;
            }
        }
        assert!(found, "fruit juice + speed should produce booze");
    }

    // ── Test: acid dip removes rust from weapon ─────────────────

    #[test]
    fn test_dip_acid_removes_rust() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let weapon = spawn_weapon(&mut world, uncursed());
        // Rust the weapon.
        if let Some(mut ero) = world.get_component_mut::<Erosion>(weapon) {
            ero.eroded = 2;
        }

        let acid = spawn_potion(&mut world, PotionType::Acid, uncursed());
        let events = dip_item(&mut world, player, weapon, acid, &mut rng);

        // Weapon should no longer be eroded.
        let ero = world.get_component::<Erosion>(weapon).unwrap();
        assert_eq!(ero.eroded, 0, "acid dip should remove rust");

        // Acid should be consumed.
        assert!(
            world.get_component::<ObjectCore>(acid).is_none(),
            "acid potion should be consumed"
        );

        let has_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-acid-repair"));
        assert!(has_msg, "should emit dip-acid-repair message");
    }

    // ── Test: acid dip on non-rusted weapon does nothing ────────

    #[test]
    fn test_dip_acid_no_rust_nothing() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let weapon = spawn_weapon(&mut world, uncursed());
        let acid = spawn_potion(&mut world, PotionType::Acid, uncursed());
        let events = dip_item(&mut world, player, weapon, acid, &mut rng);

        let has_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-acid-nothing"));
        assert!(has_msg, "should emit dip-acid-nothing for clean weapon");
    }

    // ── Test: alchemy_result is symmetric ────────────────────────

    #[test]
    fn test_alchemy_result_symmetric() {
        // All recipes should work regardless of order.
        let pairs = [
            (PotionType::Healing, PotionType::Speed, PotionType::ExtraHealing),
            (PotionType::Healing, PotionType::Sickness, PotionType::FruitJuice),
            (PotionType::ExtraHealing, PotionType::GainLevel, PotionType::FullHealing),
            (PotionType::FullHealing, PotionType::GainLevel, PotionType::GainAbility),
            (PotionType::GainLevel, PotionType::FruitJuice, PotionType::SeeInvisible),
            (PotionType::GainLevel, PotionType::Booze, PotionType::Hallucination),
        ];
        for (a, b, expected) in &pairs {
            assert_eq!(
                alchemy_result(*a, *b),
                Some(*expected),
                "{:?} + {:?} should produce {:?}",
                a, b, expected
            );
            assert_eq!(
                alchemy_result(*b, *a),
                Some(*expected),
                "{:?} + {:?} (reversed) should produce {:?}",
                b, a, expected
            );
        }
    }

    // ── Test: no alchemy result for unrelated potions ────────────

    #[test]
    fn test_alchemy_no_result() {
        assert_eq!(
            alchemy_result(PotionType::Oil, PotionType::Water),
            None,
            "oil + water should have no alchemy result"
        );
        assert_eq!(
            alchemy_result(PotionType::Polymorph, PotionType::Invisibility),
            None,
            "polymorph + invisibility should have no alchemy result"
        );
    }

    // ── Test: dip non-potion into non-water → nothing happens ───

    #[test]
    fn test_dip_generic_item_into_potion_nothing() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let tool = spawn_generic_item(&mut world, uncursed());
        let speed = spawn_potion(&mut world, PotionType::Speed, uncursed());

        let events = dip_item(&mut world, player, tool, speed, &mut rng);

        let has_nothing = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-nothing-happens"));
        assert!(
            has_nothing,
            "dipping a tool into a non-special potion should do nothing"
        );
    }

    // ── Test: unicorn horn dip cures sickness → fruit juice ─────

    #[test]
    fn test_dip_unicorn_horn_cures_sickness() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        // Spawn a unicorn horn item.
        let horn = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(55),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 20,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            uncursed(),
            DipItemTag(DipItemKind::UnicornHorn),
            nethack_babel_data::ObjectLocation::Inventory,
        ));

        let sickness = spawn_potion(&mut world, PotionType::Sickness, uncursed());
        let events = dip_item(&mut world, player, horn, sickness, &mut rng);

        // Sickness potion should now be fruit juice.
        let pt = world.get_component::<DipPotionType>(sickness).unwrap();
        assert_eq!(
            pt.0,
            PotionType::FruitJuice,
            "unicorn horn should cure sickness → fruit juice"
        );

        let has_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-unicorn-horn-cure"));
        assert!(has_msg, "should emit dip-unicorn-horn-cure message");
    }

    // ── Test: unicorn horn dip cures hallucination → water ──────

    #[test]
    fn test_dip_unicorn_horn_cures_hallucination() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let horn = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(55),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 20,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            uncursed(),
            DipItemTag(DipItemKind::UnicornHorn),
            nethack_babel_data::ObjectLocation::Inventory,
        ));

        let hallu = spawn_potion(&mut world, PotionType::Hallucination, uncursed());
        let _events = dip_item(&mut world, player, horn, hallu, &mut rng);

        let pt = world.get_component::<DipPotionType>(hallu).unwrap();
        assert_eq!(
            pt.0,
            PotionType::Water,
            "unicorn horn should cure hallucination → water"
        );
    }

    // ── Test: unicorn horn dip cures blindness → water ──────────

    #[test]
    fn test_dip_unicorn_horn_cures_blindness() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let horn = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(55),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 20,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            uncursed(),
            DipItemTag(DipItemKind::UnicornHorn),
            nethack_babel_data::ObjectLocation::Inventory,
        ));

        let blind = spawn_potion(&mut world, PotionType::Blindness, uncursed());
        let _events = dip_item(&mut world, player, horn, blind, &mut rng);

        let pt = world.get_component::<DipPotionType>(blind).unwrap();
        assert_eq!(
            pt.0,
            PotionType::Water,
            "unicorn horn should cure blindness → water"
        );
    }

    // ── Test: unicorn horn dip cures confusion → water ──────────

    #[test]
    fn test_dip_unicorn_horn_cures_confusion() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let horn = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(55),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 20,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            uncursed(),
            DipItemTag(DipItemKind::UnicornHorn),
            nethack_babel_data::ObjectLocation::Inventory,
        ));

        let conf = spawn_potion(&mut world, PotionType::Confusion, uncursed());
        let _events = dip_item(&mut world, player, horn, conf, &mut rng);

        let pt = world.get_component::<DipPotionType>(conf).unwrap();
        assert_eq!(
            pt.0,
            PotionType::Water,
            "unicorn horn should cure confusion → water"
        );
    }

    // ── Test: unicorn horn into healing does nothing ─────────────

    #[test]
    fn test_dip_unicorn_horn_no_effect_healing() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let horn = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(55),
                object_class: ObjectClass::Tool,
                quantity: 1,
                weight: 20,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            uncursed(),
            DipItemTag(DipItemKind::UnicornHorn),
            nethack_babel_data::ObjectLocation::Inventory,
        ));

        let healing = spawn_potion(&mut world, PotionType::Healing, uncursed());
        let events = dip_item(&mut world, player, horn, healing, &mut rng);

        // Should still be healing (no cure effect).
        let pt = world.get_component::<DipPotionType>(healing).unwrap();
        assert_eq!(pt.0, PotionType::Healing, "unicorn horn should not affect healing");

        let has_nothing = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-nothing-happens"));
        assert!(has_nothing, "should emit nothing-happens for non-curable potion");
    }

    // ── Test: amethyst dip into booze → fruit juice ─────────────

    #[test]
    fn test_dip_amethyst_cures_booze() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let amethyst = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(60),
                object_class: ObjectClass::Gem,
                quantity: 1,
                weight: 1,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            uncursed(),
            DipItemTag(DipItemKind::Amethyst),
            nethack_babel_data::ObjectLocation::Inventory,
        ));

        let booze = spawn_potion(&mut world, PotionType::Booze, uncursed());
        let events = dip_item(&mut world, player, amethyst, booze, &mut rng);

        let pt = world.get_component::<DipPotionType>(booze).unwrap();
        assert_eq!(
            pt.0,
            PotionType::FruitJuice,
            "amethyst should cure booze → fruit juice"
        );

        let has_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-amethyst-cure"));
        assert!(has_msg, "should emit dip-amethyst-cure message");
    }

    // ── Test: amethyst into non-booze does nothing ──────────────

    #[test]
    fn test_dip_amethyst_no_effect_healing() {
        let mut world = make_world();
        let mut rng = make_rng();
        let player = world.player();

        let amethyst = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(60),
                object_class: ObjectClass::Gem,
                quantity: 1,
                weight: 1,
                age: 0,
                inv_letter: None,
                artifact: None,
            },
            uncursed(),
            DipItemTag(DipItemKind::Amethyst),
            nethack_babel_data::ObjectLocation::Inventory,
        ));

        let healing = spawn_potion(&mut world, PotionType::Healing, uncursed());
        let events = dip_item(&mut world, player, amethyst, healing, &mut rng);

        let has_nothing = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "dip-nothing-happens"));
        assert!(has_nothing, "amethyst into healing should do nothing");
    }
}
