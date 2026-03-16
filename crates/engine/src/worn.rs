//! Worn equipment intrinsics: granting/revoking properties from worn items.
//!
//! When the player equips or unequips an item, intrinsics must be
//! recalculated.  This module implements that logic by inspecting all
//! currently worn items and their `conferred_property` from `ObjectDef`.
//!
//! The `WornIntrinsics` component tracks which intrinsic properties are
//! currently granted by worn equipment (as opposed to permanent intrinsics
//! from corpses/race/class or timed status effects).

use hecs::Entity;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use nethack_babel_data::{ObjectCore, ObjectDef, Property};

use crate::equipment::EquipmentSlots;
use crate::event::{EngineEvent, StatusEffect};
use crate::world::{Attributes, GameWorld, HeroSpeed, HeroSpeedBonus};

// ---------------------------------------------------------------------------
// WornIntrinsics component
// ---------------------------------------------------------------------------

/// Component tracking which intrinsic properties are granted by worn
/// equipment.
///
/// Separating these from permanent `Intrinsics` allows the system to
/// correctly add/remove only equipment-granted properties when
/// equipping/unequipping without disturbing intrinsics from corpses,
/// race, or class.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WornIntrinsics {
    /// Set of properties currently conferred by worn equipment.
    pub properties: HashSet<Property>,
    /// If gauntlets of power are worn, the original STR is stored here
    /// so it can be restored on unequip.
    pub saved_strength: Option<u8>,
    /// If helm of brilliance is worn, the bonus applied to INT/WIS.
    pub brilliance_bonus: u8,
}

// ---------------------------------------------------------------------------
// Property-to-StatusEffect mapping
// ---------------------------------------------------------------------------

/// Map a `Property` to a `StatusEffect` for event emission.
/// Returns `None` for properties that don't map to status effects.
fn property_to_status(prop: Property) -> Option<StatusEffect> {
    match prop {
        Property::Fast => Some(StatusEffect::FastSpeed),
        Property::Levitation => Some(StatusEffect::Levitating),
        Property::Invis => Some(StatusEffect::Invisible),
        Property::SeeInvis => Some(StatusEffect::SeeInvisible),
        Property::FireRes => Some(StatusEffect::FireResistance),
        Property::ColdRes => Some(StatusEffect::ColdResistance),
        Property::ShockRes => Some(StatusEffect::ShockResistance),
        Property::SleepRes => Some(StatusEffect::SleepResistance),
        Property::PoisonRes => Some(StatusEffect::PoisonResistance),
        Property::DisintRes => Some(StatusEffect::DisintegrationResistance),
        Property::Reflecting => Some(StatusEffect::Reflection),
        Property::Antimagic => Some(StatusEffect::MagicResistance),
        Property::Telepat => Some(StatusEffect::Telepathy),
        Property::Warning => Some(StatusEffect::Warning),
        Property::Stealth => Some(StatusEffect::Stealth),
        Property::Searching => None,  // no status effect equivalent
        Property::FreeAction => None, // handled by marker component
        Property::Lifesaved => None,  // handled specially
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Recalculation
// ---------------------------------------------------------------------------

/// Recalculate all worn intrinsics from currently equipped items.
///
/// Called after every equip/unequip operation.  Compares the new set of
/// conferred properties against the previous `WornIntrinsics` to emit
/// `StatusApplied` / `StatusRemoved` events only for changes.
///
/// Special items handled:
/// - **Speed boots** (`Property::Fast`) -> `HeroSpeed::VeryFast`
/// - **Gauntlets of power** (name contains "gauntlets of power") -> STR 25
/// - **Helm of brilliance** (name contains "helm of brilliance") -> INT/WIS +3
/// - **Cloak of magic resistance** (`Property::Antimagic`) -> MagicResistance
/// - **Amulet of reflection** (`Property::Reflecting`) -> Reflection
/// - **Amulet of life saving** (`Property::Lifesaved`) -> tracked but no status
/// - **Ring of free action** (`Property::FreeAction`) -> marker component
pub fn recalc_worn_intrinsics(
    world: &mut GameWorld,
    player: Entity,
    obj_defs: &[ObjectDef],
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Collect all conferred properties from currently worn items.
    let mut new_props: HashSet<Property> = HashSet::new();
    let mut has_power_gauntlets = false;
    let mut has_brilliance_helm = false;

    let slot_items: Vec<Option<Entity>> = {
        let equip = match world.get_component::<EquipmentSlots>(player) {
            Some(e) => e,
            None => return events,
        };
        vec![
            equip.weapon,
            equip.off_hand,
            equip.helmet,
            equip.cloak,
            equip.body_armor,
            equip.shield,
            equip.gloves,
            equip.boots,
            equip.shirt,
            equip.ring_left,
            equip.ring_right,
            equip.amulet,
        ]
    };

    for item_opt in &slot_items {
        let item = match item_opt {
            Some(e) => *e,
            None => continue,
        };
        let core = match world.get_component::<ObjectCore>(item) {
            Some(c) => c,
            None => continue,
        };
        let obj_def = match obj_defs.iter().find(|d| d.id == core.otyp) {
            Some(d) => d,
            None => continue,
        };

        if let Some(prop) = obj_def.conferred_property {
            new_props.insert(prop);
        }

        // Check for special named items.
        let name_lower = obj_def.name.to_lowercase();
        if name_lower.contains("gauntlets of power") {
            has_power_gauntlets = true;
        }
        if name_lower.contains("helm of brilliance") {
            has_brilliance_helm = true;
        }
    }

    // Get (or create) the WornIntrinsics component.
    let old_props: HashSet<Property>;
    let old_saved_str: Option<u8>;
    let old_brilliance: u8;
    {
        match world.get_component::<WornIntrinsics>(player) {
            Some(wi) => {
                old_props = wi.properties.clone();
                old_saved_str = wi.saved_strength;
                old_brilliance = wi.brilliance_bonus;
            }
            None => {
                old_props = HashSet::new();
                old_saved_str = None;
                old_brilliance = 0;
            }
        }
    }

    // Determine added and removed properties.
    let added: HashSet<&Property> = new_props.difference(&old_props).collect();
    let removed: HashSet<&Property> = old_props.difference(&new_props).collect();

    // Emit events for newly gained properties.
    for prop in &added {
        if let Some(status) = property_to_status(**prop) {
            events.push(EngineEvent::StatusApplied {
                entity: player,
                status,
                duration: None, // extrinsic: permanent while worn
                source: None,
            });
        }
    }

    // Emit events for removed properties.
    for prop in &removed {
        if let Some(status) = property_to_status(**prop) {
            events.push(EngineEvent::StatusRemoved {
                entity: player,
                status,
            });
        }
    }

    // Handle speed boots: Fast property -> HeroSpeed::VeryFast.
    if added.contains(&Property::Fast)
        && let Some(mut speed) = world.get_component_mut::<HeroSpeedBonus>(player)
    {
        speed.0 = HeroSpeed::VeryFast;
    }
    if removed.contains(&Property::Fast)
        && let Some(mut speed) = world.get_component_mut::<HeroSpeedBonus>(player)
    {
        // Revert to Normal (or Fast if they have intrinsic fast from
        // corpse — but that check is left for the full intrinsic
        // stacking pass).
        speed.0 = HeroSpeed::Normal;
    }

    // Handle gauntlets of power: STR -> 25 on equip, restore on unequip.
    let mut new_saved_str = old_saved_str;
    if has_power_gauntlets && old_saved_str.is_none() {
        // Just equipped — save current STR and set to 25.
        if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
            new_saved_str = Some(attrs.strength);
            attrs.strength = 25;
            attrs.strength_extra = 0;
        }
        events.push(EngineEvent::msg("worn-gauntlets-power-on"));
    } else if !has_power_gauntlets && old_saved_str.is_some() {
        // Just unequipped — restore saved STR.
        if let Some(saved) = old_saved_str
            && let Some(mut attrs) = world.get_component_mut::<Attributes>(player)
        {
            attrs.strength = saved;
        }
        new_saved_str = None;
        events.push(EngineEvent::msg("worn-gauntlets-power-off"));
    }

    // Handle helm of brilliance: INT/WIS +3 on equip, -3 on unequip.
    let brilliance_bonus: u8 = 3;
    let mut new_brilliance = old_brilliance;
    if has_brilliance_helm && old_brilliance == 0 {
        // Just equipped.
        if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
            attrs.intelligence = attrs.intelligence.saturating_add(brilliance_bonus);
            attrs.wisdom = attrs.wisdom.saturating_add(brilliance_bonus);
        }
        new_brilliance = brilliance_bonus;
        events.push(EngineEvent::msg("worn-helm-brilliance-on"));
    } else if !has_brilliance_helm && old_brilliance > 0 {
        // Just unequipped.
        if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
            attrs.intelligence = attrs.intelligence.saturating_sub(old_brilliance);
            attrs.wisdom = attrs.wisdom.saturating_sub(old_brilliance);
        }
        new_brilliance = 0;
        events.push(EngineEvent::msg("worn-helm-brilliance-off"));
    }

    // Handle free action: add/remove marker component.
    if added.contains(&Property::FreeAction) {
        let _ = world
            .ecs_mut()
            .insert_one(player, crate::potions::FreeAction);
    }
    if removed.contains(&Property::FreeAction) {
        let _ = world
            .ecs_mut()
            .remove_one::<crate::potions::FreeAction>(player);
    }

    // Update the WornIntrinsics component.
    let new_wi = WornIntrinsics {
        properties: new_props,
        saved_strength: new_saved_str,
        brilliance_bonus: new_brilliance,
    };

    let has_component = world.get_component::<WornIntrinsics>(player).is_some();
    if has_component {
        if let Some(mut wi) = world.get_component_mut::<WornIntrinsics>(player) {
            *wi = new_wi;
        }
    } else {
        let _ = world.ecs_mut().insert_one(player, new_wi);
    }

    events
}

/// Check if the player has a specific property from worn equipment.
pub fn has_worn_property(world: &GameWorld, player: Entity, prop: Property) -> bool {
    world
        .get_component::<WornIntrinsics>(player)
        .is_some_and(|wi| wi.properties.contains(&prop))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::equipment::{EquipSlot, equip_item, unequip_slot};
    use crate::items::{SpawnLocation, spawn_item};
    use crate::world::GameWorld;
    use nethack_babel_data::{
        ArmorCategory, ArmorInfo, Color, Material, ObjectClass, ObjectDef, ObjectTypeId, Property,
    };

    /// Build an armor ObjectDef with a conferred property.
    fn armor_with_property(
        id: u16,
        name: &str,
        category: ArmorCategory,
        ac_bonus: i8,
        prop: Option<Property>,
    ) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: None,
            class: ObjectClass::Armor,
            color: Color::White,
            material: Material::Iron,
            weight: 50,
            cost: 50,
            nutrition: 0,
            prob: 10,
            is_magic: prop.is_some(),
            is_mergeable: false,
            is_charged: false,
            is_unique: false,
            is_nowish: false,
            is_bimanual: false,
            is_bulky: false,
            is_tough: false,
            weapon: None,
            armor: Some(ArmorInfo {
                category,
                ac_bonus,
                magic_cancel: 0,
            }),
            spellbook: None,
            conferred_property: prop,
            use_delay: 0,
        }
    }

    /// Build a ring ObjectDef with a conferred property.
    fn ring_with_property(id: u16, name: &str, prop: Option<Property>) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: None,
            class: ObjectClass::Ring,
            color: Color::White,
            material: Material::Iron,
            weight: 3,
            cost: 100,
            nutrition: 0,
            prob: 10,
            is_magic: true,
            is_mergeable: false,
            is_charged: false,
            is_unique: false,
            is_nowish: false,
            is_bimanual: false,
            is_bulky: false,
            is_tough: false,
            weapon: None,
            armor: None,
            spellbook: None,
            conferred_property: prop,
            use_delay: 0,
        }
    }

    /// Build an amulet ObjectDef with a conferred property.
    fn amulet_with_property(id: u16, name: &str, prop: Option<Property>) -> ObjectDef {
        ObjectDef {
            id: ObjectTypeId(id),
            name: name.to_string(),
            appearance: None,
            class: ObjectClass::Amulet,
            color: Color::White,
            material: Material::Iron,
            weight: 20,
            cost: 150,
            nutrition: 0,
            prob: 10,
            is_magic: true,
            is_mergeable: false,
            is_charged: false,
            is_unique: false,
            is_nowish: false,
            is_bimanual: false,
            is_bulky: false,
            is_tough: false,
            weapon: None,
            armor: None,
            spellbook: None,
            conferred_property: prop,
            use_delay: 0,
        }
    }

    fn test_world() -> GameWorld {
        GameWorld::new(Position::new(40, 10))
    }

    // ── Speed boots grant/remove Fast ──────────────────────────────

    #[test]
    fn test_speed_boots_grant_fast() {
        let mut world = test_world();
        let player = world.player();

        let boots = armor_with_property(
            1,
            "speed boots",
            ArmorCategory::Boots,
            -1,
            Some(Property::Fast),
        );
        let defs = vec![boots.clone()];

        let item = spawn_item(&mut world, &boots, SpawnLocation::Inventory, Some(0));
        equip_item(&mut world, player, item, EquipSlot::Boots, &defs).unwrap();

        let events = recalc_worn_intrinsics(&mut world, player, &defs);

        // Should have StatusApplied for FastSpeed.
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::FastSpeed,
                    ..
                }
            )),
            "equipping speed boots should grant FastSpeed"
        );

        // HeroSpeedBonus should be VeryFast.
        let speed = world.get_component::<HeroSpeedBonus>(player).unwrap();
        assert_eq!(speed.0, HeroSpeed::VeryFast);
    }

    #[test]
    fn test_speed_boots_remove_fast() {
        let mut world = test_world();
        let player = world.player();

        let boots = armor_with_property(
            1,
            "speed boots",
            ArmorCategory::Boots,
            -1,
            Some(Property::Fast),
        );
        let defs = vec![boots.clone()];

        let item = spawn_item(&mut world, &boots, SpawnLocation::Inventory, Some(0));
        equip_item(&mut world, player, item, EquipSlot::Boots, &defs).unwrap();
        recalc_worn_intrinsics(&mut world, player, &defs);

        // Now unequip.
        unequip_slot(&mut world, player, EquipSlot::Boots).unwrap();
        let events = recalc_worn_intrinsics(&mut world, player, &defs);

        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::StatusRemoved {
                    status: StatusEffect::FastSpeed,
                    ..
                }
            )),
            "unequipping speed boots should remove FastSpeed"
        );

        let speed = world.get_component::<HeroSpeedBonus>(player).unwrap();
        assert_eq!(speed.0, HeroSpeed::Normal);
    }

    // ── Ring of fire resistance ────────────────────────────────────

    #[test]
    fn test_ring_fire_resist() {
        let mut world = test_world();
        let player = world.player();

        let ring = ring_with_property(1, "ring of fire resistance", Some(Property::FireRes));
        let defs = vec![ring.clone()];

        let item = spawn_item(&mut world, &ring, SpawnLocation::Inventory, None);
        equip_item(&mut world, player, item, EquipSlot::RingLeft, &defs).unwrap();

        let events = recalc_worn_intrinsics(&mut world, player, &defs);

        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::FireResistance,
                    ..
                }
            )),
            "ring of fire resistance should grant FireResistance"
        );

        assert!(has_worn_property(&world, player, Property::FireRes));
    }

    // ── Amulet of reflection ──────────────────────────────────────

    #[test]
    fn test_amulet_reflection() {
        let mut world = test_world();
        let player = world.player();

        let amulet = amulet_with_property(1, "amulet of reflection", Some(Property::Reflecting));
        let defs = vec![amulet.clone()];

        let item = spawn_item(&mut world, &amulet, SpawnLocation::Inventory, None);
        equip_item(&mut world, player, item, EquipSlot::Amulet, &defs).unwrap();

        let events = recalc_worn_intrinsics(&mut world, player, &defs);

        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::Reflection,
                    ..
                }
            )),
            "amulet of reflection should grant Reflection"
        );

        assert!(has_worn_property(&world, player, Property::Reflecting));
    }

    // ── Gauntlets of power -> STR 25 ──────────────────────────────

    #[test]
    fn test_gauntlets_power_str25() {
        let mut world = test_world();
        let player = world.player();

        // Set initial STR to 14.
        {
            let mut attrs = world.get_component_mut::<Attributes>(player).unwrap();
            attrs.strength = 14;
        }

        let gauntlets =
            armor_with_property(1, "gauntlets of power", ArmorCategory::Gloves, -1, None);
        let defs = vec![gauntlets.clone()];

        let item = spawn_item(&mut world, &gauntlets, SpawnLocation::Inventory, Some(0));
        equip_item(&mut world, player, item, EquipSlot::Gloves, &defs).unwrap();

        recalc_worn_intrinsics(&mut world, player, &defs);

        {
            let attrs = world.get_component::<Attributes>(player).unwrap();
            assert_eq!(
                attrs.strength, 25,
                "gauntlets of power should set STR to 25"
            );
        }

        // Unequip should restore.
        unequip_slot(&mut world, player, EquipSlot::Gloves).unwrap();
        recalc_worn_intrinsics(&mut world, player, &defs);

        {
            let attrs = world.get_component::<Attributes>(player).unwrap();
            assert_eq!(
                attrs.strength, 14,
                "STR should be restored after unequipping"
            );
        }
    }

    // ── Worn intrinsics stacking ──────────────────────────────────

    #[test]
    fn test_worn_intrinsics_stacking() {
        let mut world = test_world();
        let player = world.player();

        // Equip two items that both grant fire resistance.
        let ring = ring_with_property(1, "ring of fire resistance", Some(Property::FireRes));
        let cloak = armor_with_property(
            2,
            "cloak of fire resistance",
            ArmorCategory::Cloak,
            -1,
            Some(Property::FireRes),
        );
        let defs = vec![ring.clone(), cloak.clone()];

        let ring_item = spawn_item(&mut world, &ring, SpawnLocation::Inventory, None);
        let cloak_item = spawn_item(&mut world, &cloak, SpawnLocation::Inventory, Some(0));

        equip_item(&mut world, player, ring_item, EquipSlot::RingLeft, &defs).unwrap();
        recalc_worn_intrinsics(&mut world, player, &defs);

        equip_item(&mut world, player, cloak_item, EquipSlot::Cloak, &defs).unwrap();
        let events = recalc_worn_intrinsics(&mut world, player, &defs);

        // Second source of FireRes should NOT emit a new StatusApplied
        // (property was already in the set).
        let fire_applied = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    EngineEvent::StatusApplied {
                        status: StatusEffect::FireResistance,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(
            fire_applied, 0,
            "second source of same property should not double-grant"
        );

        // Remove one source — property should persist.
        unequip_slot(&mut world, player, EquipSlot::RingLeft).unwrap();
        let events = recalc_worn_intrinsics(&mut world, player, &defs);

        // FireRes should NOT be removed because cloak still provides it.
        let fire_removed = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    EngineEvent::StatusRemoved {
                        status: StatusEffect::FireResistance,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(
            fire_removed, 0,
            "removing one source should not remove property if another source remains"
        );

        assert!(has_worn_property(&world, player, Property::FireRes));
    }

    // ── Helm of brilliance INT/WIS +3 ─────────────────────────────

    #[test]
    fn test_helm_brilliance() {
        let mut world = test_world();
        let player = world.player();

        {
            let mut attrs = world.get_component_mut::<Attributes>(player).unwrap();
            attrs.intelligence = 12;
            attrs.wisdom = 14;
        }

        let helm = armor_with_property(1, "helm of brilliance", ArmorCategory::Helm, -1, None);
        let defs = vec![helm.clone()];

        let item = spawn_item(&mut world, &helm, SpawnLocation::Inventory, Some(0));
        equip_item(&mut world, player, item, EquipSlot::Helmet, &defs).unwrap();

        recalc_worn_intrinsics(&mut world, player, &defs);

        {
            let attrs = world.get_component::<Attributes>(player).unwrap();
            assert_eq!(attrs.intelligence, 15, "INT should be +3");
            assert_eq!(attrs.wisdom, 17, "WIS should be +3");
        }

        // Unequip.
        unequip_slot(&mut world, player, EquipSlot::Helmet).unwrap();
        recalc_worn_intrinsics(&mut world, player, &defs);

        {
            let attrs = world.get_component::<Attributes>(player).unwrap();
            assert_eq!(attrs.intelligence, 12, "INT should be restored");
            assert_eq!(attrs.wisdom, 14, "WIS should be restored");
        }
    }
}
