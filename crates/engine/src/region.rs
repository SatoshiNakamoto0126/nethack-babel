//! Region system: area-of-effect zones on the dungeon map.
//!
//! Implements poison gas clouds, stinking clouds, fog, acid clouds, fire
//! clouds, force fields, and other temporary area effects.  Regions have a
//! position, radius, duration, and an effect that is applied each turn to
//! entities within range.
//!
//! Also provides level flag checking functions for special level properties
//! (no-dig, no-teleport, etc.) aligned with NetHack's `region.c`.
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::action::Position;
use crate::event::{EngineEvent, HpSource, StatusEffect};
use crate::status::Intrinsics;
use crate::world::{Attributes, GameWorld, HitPoints, Positioned};

// ---------------------------------------------------------------------------
// Dice helpers (local, matching NetHack conventions)
// ---------------------------------------------------------------------------

/// Roll one die with `sides` faces: uniform in [1, sides].
#[inline]
fn rnd<R: Rng>(rng: &mut R, sides: u32) -> u32 {
    if sides == 0 {
        return 0;
    }
    rng.random_range(1..=sides)
}

/// Roll `n` dice of `s` sides.
#[inline]
fn d<R: Rng>(rng: &mut R, n: u32, s: u32) -> u32 {
    (0..n).map(|_| rnd(rng, s)).sum()
}

// ---------------------------------------------------------------------------
// Region types and data
// ---------------------------------------------------------------------------

/// What kind of effect a region applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegionType {
    /// Stinking cloud: causes confusion + poison damage.
    StinkingCloud,
    /// Poison gas: raw poison damage with CON save.
    PoisonGas,
    /// Fog: reduces visibility but no direct damage.
    Fog,
    /// Acid cloud: acid damage (from alchemy explosions).
    AcidCloud,
    /// Fire cloud: fire damage (from fumaroles / lava vents).
    FireCloud,
}

/// A temporary area-of-effect zone on the dungeon map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    /// Center of the region.
    pub position: Position,
    /// Radius in Chebyshev (king-move) distance.
    pub radius: u32,
    /// Turns remaining before the region expires.
    pub duration: u32,
    /// What effect this region applies.
    pub effect: RegionType,
}

/// Map-level collection of active regions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegionMap {
    pub regions: Vec<Region>,
    pub force_fields: Vec<ForceField>,
}

// ---------------------------------------------------------------------------
// Gas Cloud system (aligned with NetHack's create_gas_cloud / inside_gas_cloud)
// ---------------------------------------------------------------------------

/// Damage type for gas clouds (mirrors the `damage` arg in NetHack's
/// `make_gas_cloud`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GasCloudType {
    /// Poison gas from scroll of stinking cloud or similar.
    Poison,
    /// Acid vapour from alchemy explosion.
    Acid,
    /// Fire / heat from fumaroles or lava.
    Fire,
}

/// A gas cloud region, modelled after NetHack's gas cloud system.
///
/// Unlike the simpler `Region`, a `GasCloud` tracks per-turn damage and
/// supports the dissipation mechanic where thick clouds halve damage and
/// extend duration when they would otherwise expire (matching
/// `expire_gas_cloud` in `region.c`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasCloud {
    /// Center of the cloud.
    pub position: Position,
    /// Radius in Chebyshev distance.
    pub radius: u32,
    /// Turns remaining before the cloud expires.
    pub turns_remaining: u32,
    /// Type of gas damage.
    pub damage_type: GasCloudType,
    /// Base damage per turn (like `reg->arg.a_int` in NetHack).
    pub damage_per_turn: u32,
}

/// Create a gas cloud and return it.
///
/// This mirrors NetHack's `create_gas_cloud()` + `make_gas_cloud()`.  The
/// caller is responsible for adding the returned cloud to whatever collection
/// they maintain.
pub fn create_gas_cloud(
    center: Position,
    radius: u32,
    duration: u32,
    damage_type: GasCloudType,
    damage: u32,
) -> GasCloud {
    GasCloud {
        position: center,
        radius,
        turns_remaining: duration,
        damage_type,
        damage_per_turn: damage,
    }
}

/// Check if a position is inside a gas cloud.
#[inline]
pub fn in_gas_cloud(cloud: &GasCloud, pos: Position) -> bool {
    chebyshev(cloud.position, pos) <= cloud.radius
}

/// Tick all gas clouds: apply damage to entities inside, decrement duration,
/// handle the dissipation mechanic, and remove expired clouds.
///
/// The dissipation mechanic matches `expire_gas_cloud` in NetHack: when a
/// cloud with `damage_per_turn >= 5` would expire, the damage is halved and
/// the cloud gets 2 more turns of life.
///
/// Returns events for all effects applied this turn.
pub fn tick_gas_clouds(
    clouds: &mut Vec<GasCloud>,
    world: &mut GameWorld,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Collect entity positions and resistances.
    let entities: Vec<(Entity, Position, bool, bool)> = world
        .ecs()
        .query::<(&Positioned, &HitPoints)>()
        .iter()
        .map(|(e, (pos, _hp))| {
            let intr = world.get_component::<Intrinsics>(e);
            let poison_res = intr.as_ref().is_some_and(|i| i.poison_resistance);
            let fire_res = intr.as_ref().is_some_and(|i| i.fire_resistance);
            (e, pos.0, poison_res, fire_res)
        })
        .collect();

    for cloud in clouds.iter_mut() {
        if cloud.turns_remaining == 0 {
            continue;
        }

        // Apply effects to entities in the cloud.
        for &(entity, pos, poison_resistant, fire_resistant) in &entities {
            if !in_gas_cloud(cloud, pos) {
                continue;
            }

            match cloud.damage_type {
                GasCloudType::Poison => {
                    // Mirrors inside_gas_cloud(): eyes sting (blindness),
                    // lungs burn.  Poison-resistant entities just cough.
                    if !poison_resistant && cloud.damage_per_turn > 0 {
                        let dam = rnd(rng, cloud.damage_per_turn) + 5;
                        if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
                            hp.current -= dam as i32;
                        }
                        events.push(EngineEvent::HpChange {
                            entity,
                            amount: -(dam as i32),
                            new_hp: world
                                .get_component::<HitPoints>(entity)
                                .map(|hp| hp.current)
                                .unwrap_or(0),
                            source: HpSource::Poison,
                        });
                    }
                }
                GasCloudType::Acid => {
                    // Acid cloud: fixed damage, no resistance check yet
                    // (acid resistance would go here when intrinsics expand).
                    if cloud.damage_per_turn > 0 {
                        let dam = d(rng, 1, cloud.damage_per_turn);
                        if dam > 0 {
                            if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
                                hp.current -= dam as i32;
                            }
                            events.push(EngineEvent::HpChange {
                                entity,
                                amount: -(dam as i32),
                                new_hp: world
                                    .get_component::<HitPoints>(entity)
                                    .map(|hp| hp.current)
                                    .unwrap_or(0),
                                source: HpSource::Environment,
                            });
                        }
                    }
                }
                GasCloudType::Fire => {
                    // Fire cloud: fire damage, resisted by fire resistance.
                    if !fire_resistant && cloud.damage_per_turn > 0 {
                        let dam = d(rng, 1, cloud.damage_per_turn);
                        if dam > 0 {
                            if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
                                hp.current -= dam as i32;
                            }
                            events.push(EngineEvent::HpChange {
                                entity,
                                amount: -(dam as i32),
                                new_hp: world
                                    .get_component::<HitPoints>(entity)
                                    .map(|hp| hp.current)
                                    .unwrap_or(0),
                                source: HpSource::Environment,
                            });
                        }
                    }
                }
            }
        }

        // Decrement duration.
        cloud.turns_remaining = cloud.turns_remaining.saturating_sub(1);

        // Dissipation mechanic: thick clouds halve damage and persist longer.
        if cloud.turns_remaining == 0 && cloud.damage_per_turn >= 5 {
            cloud.damage_per_turn /= 2;
            cloud.turns_remaining = 2;
        }
    }

    // Remove fully expired clouds.
    clouds.retain(|c| c.turns_remaining > 0);

    events
}

// ---------------------------------------------------------------------------
// Force field system (aligned with NetHack's create_force_field, currently
// #if 0'd in region.c but spec'd for Babel)
// ---------------------------------------------------------------------------

/// An invisible barrier that blocks movement.
///
/// In NetHack this was planned but commented out (`#if 0` in region.c).
/// Babel implements it as a simple circle-based barrier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForceField {
    /// Center of the force field.
    pub center: Position,
    /// Radius in Chebyshev distance.
    pub radius: u32,
    /// Turns remaining; 0 means permanent until explicitly removed.
    pub duration: u32,
}

/// Create a force field barrier.
pub fn create_force_field(center: Position, radius: u32, duration: u32) -> ForceField {
    ForceField {
        center,
        radius,
        duration,
    }
}

/// Check whether a position is blocked by any active force field.
pub fn in_force_field(pos: Position, fields: &[ForceField]) -> bool {
    fields
        .iter()
        .any(|ff| chebyshev(ff.center, pos) <= ff.radius)
}

/// Tick force fields: decrement durations and remove expired ones.
///
/// A field with `duration == 0` at creation is treated as permanent and is
/// never removed by ticking.  Fields with `duration > 0` count down each
/// tick; when they reach 0 they are removed.
pub fn tick_force_fields(fields: &mut Vec<ForceField>) {
    // Track which indices had duration > 0 before decrementing.
    let was_nonzero: Vec<bool> = fields.iter().map(|ff| ff.duration > 0).collect();

    for ff in fields.iter_mut() {
        if ff.duration > 0 {
            ff.duration -= 1;
        }
    }

    // Remove fields that were non-permanent (had duration > 0) and have
    // now reached 0.
    let mut idx = 0;
    fields.retain(|ff| {
        let keep = !(was_nonzero[idx] && ff.duration == 0);
        idx += 1;
        keep
    });
}

// ---------------------------------------------------------------------------
// Level flags (checking functions for special level properties)
// ---------------------------------------------------------------------------

/// Extended level flags covering all special level properties from NetHack.
///
/// This supplements `CurrentLevelFlags` in `dungeon.rs` with additional
/// environmental flags that affect gameplay mechanics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtendedLevelFlags {
    /// Digging is forbidden (e.g. Sokoban, Fort Ludios vault).
    pub no_dig: bool,
    /// Teleporting is forbidden (Sokoban, some quest levels).
    pub no_teleport: bool,
    /// Prayer does not work (Gehennom, Sanctum).
    pub no_prayer: bool,
    /// Hard floor: no pit/hole digging (not the same as no_dig which
    /// prevents wall digging too).
    pub hardfloor: bool,
    /// Magic mapping does not work.
    pub nommap: bool,
    /// Monsters have reduced vision range (Gnomish Mines).
    pub shortsighted: bool,
    /// Trees instead of stone (Woodland levels, some quest levels).
    pub arboreal: bool,
    /// Lava vents emit poison gas clouds.
    pub fumaroles: bool,
    /// Lightning strikes from storm clouds.
    pub stormy: bool,
    /// Level is a graveyard (undead-heavy, special messages).
    pub graveyard: bool,
    /// Level uses maze layout.
    pub is_maze: bool,
    /// Level uses rogue-like display.
    pub is_rogue: bool,
}

/// Check if digging is forbidden on this level.
#[inline]
pub fn check_no_dig(flags: &ExtendedLevelFlags) -> bool {
    flags.no_dig
}

/// Check if teleportation is forbidden on this level.
#[inline]
pub fn check_no_teleport(flags: &ExtendedLevelFlags) -> bool {
    flags.no_teleport
}

/// Check if prayer is forbidden on this level.
#[inline]
pub fn check_no_prayer(flags: &ExtendedLevelFlags) -> bool {
    flags.no_prayer
}

/// Check if monsters have reduced vision range on this level.
#[inline]
pub fn check_shortsighted(flags: &ExtendedLevelFlags) -> bool {
    flags.shortsighted
}

/// Check if this level uses trees instead of stone walls.
#[inline]
pub fn check_arboreal(flags: &ExtendedLevelFlags) -> bool {
    flags.arboreal
}

/// Check if this level has lava vents that emit poison gas.
#[inline]
pub fn check_fumaroles(flags: &ExtendedLevelFlags) -> bool {
    flags.fumaroles
}

/// Check if this level has storm clouds that produce lightning.
#[inline]
pub fn check_stormy(flags: &ExtendedLevelFlags) -> bool {
    flags.stormy
}

/// Check if this level is a graveyard.
#[inline]
pub fn check_graveyard(flags: &ExtendedLevelFlags) -> bool {
    flags.graveyard
}

/// Check if magic mapping works on this level.
#[inline]
pub fn check_nommap(flags: &ExtendedLevelFlags) -> bool {
    flags.nommap
}

/// Check if the floor is hard (no pit/hole digging).
#[inline]
pub fn check_hardfloor(flags: &ExtendedLevelFlags) -> bool {
    flags.hardfloor
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// Chebyshev (king-move) distance between two positions.
#[inline]
fn chebyshev(a: Position, b: Position) -> u32 {
    let dx = (a.x - b.x).unsigned_abs();
    let dy = (a.y - b.y).unsigned_abs();
    dx.max(dy)
}

/// Check whether a position is within a region's area of effect.
#[inline]
pub fn in_region(region: &Region, pos: Position) -> bool {
    chebyshev(region.position, pos) <= region.radius
}

// ---------------------------------------------------------------------------
// Region lifecycle
// ---------------------------------------------------------------------------

/// Create a new region and add it to the region map.
///
/// Returns an event announcing the region's creation.
pub fn create_region(
    region_map: &mut RegionMap,
    pos: Position,
    radius: u32,
    effect: RegionType,
    duration: u32,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    region_map.regions.push(Region {
        position: pos,
        radius,
        duration,
        effect,
    });

    let effect_name = match effect {
        RegionType::StinkingCloud => "stinking cloud",
        RegionType::PoisonGas => "poison gas",
        RegionType::Fog => "fog",
        RegionType::AcidCloud => "acid cloud",
        RegionType::FireCloud => "fire cloud",
    };

    events.push(EngineEvent::msg_with(
        "region-created",
        vec![("effect", effect_name.to_string())],
    ));

    events
}

/// Remove all expired regions (duration == 0) from the map.
pub fn remove_expired_regions(region_map: &mut RegionMap) {
    region_map.regions.retain(|r| r.duration > 0);
}

// ---------------------------------------------------------------------------
// Per-turn tick
// ---------------------------------------------------------------------------

/// Tick all active regions: decrement durations, apply effects to
/// entities inside each region.
///
/// Returns events for all effects applied this turn.
pub fn tick_regions(
    world: &mut GameWorld,
    region_map: &mut RegionMap,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Collect entities with positions and HP for region effect checks.
    let entities: Vec<(Entity, Position, bool, bool)> = world
        .ecs()
        .query::<(&Positioned, &HitPoints)>()
        .iter()
        .map(|(e, (pos, _hp))| {
            let intr = world.get_component::<Intrinsics>(e);
            let poison_res = intr.as_ref().is_some_and(|i| i.poison_resistance);
            let fire_res = intr.as_ref().is_some_and(|i| i.fire_resistance);
            (e, pos.0, poison_res, fire_res)
        })
        .collect();

    for region in region_map.regions.iter_mut() {
        if region.duration == 0 {
            continue;
        }
        region.duration = region.duration.saturating_sub(1);

        for &(entity, pos, poison_resistant, fire_resistant) in &entities {
            if !in_region(region, pos) {
                continue;
            }

            match region.effect {
                RegionType::StinkingCloud => {
                    // Confusion effect.
                    events.push(EngineEvent::StatusApplied {
                        entity,
                        status: StatusEffect::Confused,
                        duration: Some(d(rng, 1, 4) + 1),
                        source: None,
                    });

                    // Poison damage (resisted if poison-resistant).
                    if !poison_resistant {
                        let damage = d(rng, 2, 4);
                        if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
                            hp.current -= damage as i32;
                        }
                        events.push(EngineEvent::HpChange {
                            entity,
                            amount: -(damage as i32),
                            new_hp: world
                                .get_component::<HitPoints>(entity)
                                .map(|hp| hp.current)
                                .unwrap_or(0),
                            source: HpSource::Poison,
                        });
                    }
                }
                RegionType::PoisonGas => {
                    if !poison_resistant {
                        let mut damage = d(rng, 1, 10);
                        // CON save: if CON check passes, halve damage.
                        let con = world
                            .get_component::<Attributes>(entity)
                            .map(|a| a.constitution)
                            .unwrap_or(10);
                        if rng.random_range(1..=20) <= con as u32 {
                            damage /= 2;
                        }
                        if damage > 0 {
                            if let Some(mut hp) =
                                world.get_component_mut::<HitPoints>(entity)
                            {
                                hp.current -= damage as i32;
                            }
                            events.push(EngineEvent::HpChange {
                                entity,
                                amount: -(damage as i32),
                                new_hp: world
                                    .get_component::<HitPoints>(entity)
                                    .map(|hp| hp.current)
                                    .unwrap_or(0),
                                source: HpSource::Poison,
                            });
                        }
                    }
                }
                RegionType::AcidCloud => {
                    let damage = d(rng, 2, 6);
                    if damage > 0 {
                        if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
                            hp.current -= damage as i32;
                        }
                        events.push(EngineEvent::HpChange {
                            entity,
                            amount: -(damage as i32),
                            new_hp: world
                                .get_component::<HitPoints>(entity)
                                .map(|hp| hp.current)
                                .unwrap_or(0),
                            source: HpSource::Environment,
                        });
                    }
                }
                RegionType::FireCloud => {
                    if !fire_resistant {
                        let damage = d(rng, 2, 6);
                        if damage > 0 {
                            if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
                                hp.current -= damage as i32;
                            }
                            events.push(EngineEvent::HpChange {
                                entity,
                                amount: -(damage as i32),
                                new_hp: world
                                    .get_component::<HitPoints>(entity)
                                    .map(|hp| hp.current)
                                    .unwrap_or(0),
                                source: HpSource::Environment,
                            });
                        }
                    }
                }
                RegionType::Fog => {
                    // Fog has no direct damage, just visibility reduction.
                    // In a full implementation this would modify the FOV.
                    // Emit an event so consumers can handle it.
                    events.push(EngineEvent::msg("region-fog-obscures"));
                }
            }
        }
    }

    // Clean up expired regions.
    remove_expired_regions(region_map);

    events
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn make_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    // ── Existing region tests ─────────────────────────────────────────

    #[test]
    fn create_and_expire_region() {
        let mut rm = RegionMap::default();
        let events = create_region(
            &mut rm,
            Position::new(10, 5),
            2,
            RegionType::StinkingCloud,
            3,
        );
        assert_eq!(rm.regions.len(), 1);
        assert!(!events.is_empty());

        // Manually expire.
        rm.regions[0].duration = 0;
        remove_expired_regions(&mut rm);
        assert!(rm.regions.is_empty());
    }

    #[test]
    fn in_region_check() {
        let r = Region {
            position: Position::new(10, 10),
            radius: 2,
            duration: 5,
            effect: RegionType::Fog,
        };
        assert!(in_region(&r, Position::new(10, 10)));
        assert!(in_region(&r, Position::new(11, 11)));
        assert!(in_region(&r, Position::new(12, 10)));
        assert!(!in_region(&r, Position::new(13, 10)));
        assert!(!in_region(&r, Position::new(10, 13)));
    }

    #[test]
    fn stinking_cloud_damages_entities() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();

        // Place a monster in the cloud area.
        let mon = world.spawn((
            Positioned(Position::new(10, 5)),
            HitPoints {
                current: 30,
                max: 30,
            },
            crate::world::Attributes::default(),
        ));

        let mut rm = RegionMap::default();
        create_region(
            &mut rm,
            Position::new(10, 5),
            1,
            RegionType::StinkingCloud,
            3,
        );

        let events = tick_regions(&mut world, &mut rm, &mut rng);
        // Should have applied confusion and poison damage.
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied {
                status: StatusEffect::Confused,
                ..
            }
        )));
        // HP should have decreased.
        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert!(hp.current < 30);
    }

    #[test]
    fn poison_gas_respects_resistance() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();

        // Entity with poison resistance.
        let mon = world.spawn((
            Positioned(Position::new(10, 5)),
            HitPoints {
                current: 30,
                max: 30,
            },
            Intrinsics {
                poison_resistance: true,
                ..Intrinsics::default()
            },
        ));

        let mut rm = RegionMap::default();
        create_region(
            &mut rm,
            Position::new(10, 5),
            1,
            RegionType::PoisonGas,
            3,
        );

        let _events = tick_regions(&mut world, &mut rm, &mut rng);
        // Poison-resistant: no HP change.
        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert_eq!(hp.current, 30);
    }

    #[test]
    fn region_duration_decrements() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();
        let mut rm = RegionMap::default();
        create_region(
            &mut rm,
            Position::new(10, 5),
            1,
            RegionType::Fog,
            2,
        );
        assert_eq!(rm.regions[0].duration, 2);

        tick_regions(&mut world, &mut rm, &mut rng);
        assert_eq!(rm.regions[0].duration, 1);

        tick_regions(&mut world, &mut rm, &mut rng);
        // Duration hit 0 and region was removed.
        assert!(rm.regions.is_empty());
    }

    #[test]
    fn fog_no_damage() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();

        let mon = world.spawn((
            Positioned(Position::new(10, 5)),
            HitPoints {
                current: 20,
                max: 20,
            },
        ));

        let mut rm = RegionMap::default();
        create_region(
            &mut rm,
            Position::new(10, 5),
            1,
            RegionType::Fog,
            3,
        );

        tick_regions(&mut world, &mut rm, &mut rng);
        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert_eq!(hp.current, 20);
    }

    // ── Gas cloud tests ───────────────────────────────────────────────

    #[test]
    fn gas_cloud_creation() {
        let cloud = create_gas_cloud(
            Position::new(5, 5),
            3,
            10,
            GasCloudType::Poison,
            8,
        );
        assert_eq!(cloud.position, Position::new(5, 5));
        assert_eq!(cloud.radius, 3);
        assert_eq!(cloud.turns_remaining, 10);
        assert_eq!(cloud.damage_type, GasCloudType::Poison);
        assert_eq!(cloud.damage_per_turn, 8);
    }

    #[test]
    fn gas_cloud_in_range() {
        let cloud = create_gas_cloud(
            Position::new(10, 10),
            2,
            5,
            GasCloudType::Poison,
            4,
        );
        assert!(in_gas_cloud(&cloud, Position::new(10, 10)));
        assert!(in_gas_cloud(&cloud, Position::new(11, 11)));
        assert!(in_gas_cloud(&cloud, Position::new(12, 10)));
        assert!(!in_gas_cloud(&cloud, Position::new(13, 10)));
    }

    #[test]
    fn gas_cloud_poison_damages() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();

        let mon = world.spawn((
            Positioned(Position::new(5, 5)),
            HitPoints {
                current: 50,
                max: 50,
            },
        ));

        let mut clouds = vec![create_gas_cloud(
            Position::new(5, 5),
            1,
            3,
            GasCloudType::Poison,
            10,
        )];

        let events = tick_gas_clouds(&mut clouds, &mut world, &mut rng);
        assert!(!events.is_empty());
        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert!(hp.current < 50, "poison gas should deal damage");
    }

    #[test]
    fn gas_cloud_acid_damages() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();

        let mon = world.spawn((
            Positioned(Position::new(5, 5)),
            HitPoints {
                current: 50,
                max: 50,
            },
        ));

        let mut clouds = vec![create_gas_cloud(
            Position::new(5, 5),
            1,
            3,
            GasCloudType::Acid,
            8,
        )];

        let events = tick_gas_clouds(&mut clouds, &mut world, &mut rng);
        assert!(!events.is_empty());
        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert!(hp.current < 50, "acid cloud should deal damage");
    }

    #[test]
    fn gas_cloud_fire_damages() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();

        let mon = world.spawn((
            Positioned(Position::new(5, 5)),
            HitPoints {
                current: 50,
                max: 50,
            },
        ));

        let mut clouds = vec![create_gas_cloud(
            Position::new(5, 5),
            1,
            3,
            GasCloudType::Fire,
            8,
        )];

        let events = tick_gas_clouds(&mut clouds, &mut world, &mut rng);
        assert!(!events.is_empty());
        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert!(hp.current < 50, "fire cloud should deal damage");
    }

    #[test]
    fn gas_cloud_fire_resisted() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();

        let mon = world.spawn((
            Positioned(Position::new(5, 5)),
            HitPoints {
                current: 50,
                max: 50,
            },
            Intrinsics {
                fire_resistance: true,
                ..Intrinsics::default()
            },
        ));

        let mut clouds = vec![create_gas_cloud(
            Position::new(5, 5),
            1,
            3,
            GasCloudType::Fire,
            8,
        )];

        let _events = tick_gas_clouds(&mut clouds, &mut world, &mut rng);
        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert_eq!(hp.current, 50, "fire-resistant entity takes no damage");
    }

    #[test]
    fn gas_cloud_expiry() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();

        let mut clouds = vec![create_gas_cloud(
            Position::new(5, 5),
            1,
            1,
            GasCloudType::Poison,
            2, // damage < 5, so no dissipation extension
        )];

        tick_gas_clouds(&mut clouds, &mut world, &mut rng);
        assert!(clouds.is_empty(), "cloud with 1 turn should be removed");
    }

    #[test]
    fn gas_cloud_dissipation_extends_life() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();

        let mut clouds = vec![create_gas_cloud(
            Position::new(5, 5),
            1,
            1,
            GasCloudType::Poison,
            10, // damage >= 5, triggers dissipation
        )];

        tick_gas_clouds(&mut clouds, &mut world, &mut rng);
        // Should still be alive with halved damage and 2 turns.
        assert_eq!(clouds.len(), 1);
        assert_eq!(clouds[0].damage_per_turn, 5);
        assert_eq!(clouds[0].turns_remaining, 2);
    }

    // ── Force field tests ─────────────────────────────────────────────

    #[test]
    fn force_field_creation() {
        let ff = create_force_field(Position::new(10, 10), 3, 5);
        assert_eq!(ff.center, Position::new(10, 10));
        assert_eq!(ff.radius, 3);
        assert_eq!(ff.duration, 5);
    }

    #[test]
    fn force_field_blocks_position() {
        let fields = vec![
            create_force_field(Position::new(10, 10), 2, 5),
        ];

        assert!(in_force_field(Position::new(10, 10), &fields));
        assert!(in_force_field(Position::new(11, 11), &fields));
        assert!(in_force_field(Position::new(12, 10), &fields));
        assert!(!in_force_field(Position::new(13, 10), &fields));
        assert!(!in_force_field(Position::new(10, 13), &fields));
    }

    #[test]
    fn force_field_no_fields_never_blocks() {
        let fields: Vec<ForceField> = vec![];
        assert!(!in_force_field(Position::new(5, 5), &fields));
    }

    #[test]
    fn force_field_multiple_overlap() {
        let fields = vec![
            create_force_field(Position::new(5, 5), 1, 10),
            create_force_field(Position::new(20, 20), 1, 10),
        ];
        assert!(in_force_field(Position::new(5, 5), &fields));
        assert!(in_force_field(Position::new(20, 20), &fields));
        assert!(!in_force_field(Position::new(12, 12), &fields));
    }

    #[test]
    fn force_field_tick_expiry() {
        let mut fields = vec![
            create_force_field(Position::new(5, 5), 2, 2),
        ];
        assert_eq!(fields.len(), 1);

        tick_force_fields(&mut fields);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].duration, 1);

        tick_force_fields(&mut fields);
        assert!(fields.is_empty(), "field should be removed when duration hits 0");
    }

    // ── Level flag tests ──────────────────────────────────────────────

    #[test]
    fn level_flags_default_all_false() {
        let flags = ExtendedLevelFlags::default();
        assert!(!check_no_dig(&flags));
        assert!(!check_no_teleport(&flags));
        assert!(!check_no_prayer(&flags));
        assert!(!check_shortsighted(&flags));
        assert!(!check_arboreal(&flags));
        assert!(!check_fumaroles(&flags));
        assert!(!check_stormy(&flags));
        assert!(!check_graveyard(&flags));
        assert!(!check_nommap(&flags));
        assert!(!check_hardfloor(&flags));
    }

    #[test]
    fn level_flags_no_dig() {
        let flags = ExtendedLevelFlags {
            no_dig: true,
            ..Default::default()
        };
        assert!(check_no_dig(&flags));
        assert!(!check_no_teleport(&flags));
    }

    #[test]
    fn level_flags_no_teleport() {
        let flags = ExtendedLevelFlags {
            no_teleport: true,
            ..Default::default()
        };
        assert!(check_no_teleport(&flags));
        assert!(!check_no_dig(&flags));
    }

    #[test]
    fn level_flags_no_prayer() {
        let flags = ExtendedLevelFlags {
            no_prayer: true,
            ..Default::default()
        };
        assert!(check_no_prayer(&flags));
    }

    #[test]
    fn level_flags_shortsighted() {
        let flags = ExtendedLevelFlags {
            shortsighted: true,
            ..Default::default()
        };
        assert!(check_shortsighted(&flags));
    }

    #[test]
    fn level_flags_arboreal() {
        let flags = ExtendedLevelFlags {
            arboreal: true,
            ..Default::default()
        };
        assert!(check_arboreal(&flags));
    }

    #[test]
    fn level_flags_fumaroles() {
        let flags = ExtendedLevelFlags {
            fumaroles: true,
            ..Default::default()
        };
        assert!(check_fumaroles(&flags));
    }

    #[test]
    fn level_flags_stormy() {
        let flags = ExtendedLevelFlags {
            stormy: true,
            ..Default::default()
        };
        assert!(check_stormy(&flags));
    }

    #[test]
    fn level_flags_combined() {
        let flags = ExtendedLevelFlags {
            no_dig: true,
            no_teleport: true,
            graveyard: true,
            is_maze: true,
            ..Default::default()
        };
        assert!(check_no_dig(&flags));
        assert!(check_no_teleport(&flags));
        assert!(check_graveyard(&flags));
        assert!(flags.is_maze);
        assert!(!check_no_prayer(&flags));
        assert!(!check_fumaroles(&flags));
    }

    // ── Acid/Fire region tests ────────────────────────────────────────

    #[test]
    fn acid_cloud_region_damages() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();

        let mon = world.spawn((
            Positioned(Position::new(10, 5)),
            HitPoints {
                current: 30,
                max: 30,
            },
        ));

        let mut rm = RegionMap::default();
        create_region(
            &mut rm,
            Position::new(10, 5),
            1,
            RegionType::AcidCloud,
            3,
        );

        let events = tick_regions(&mut world, &mut rm, &mut rng);
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::HpChange {
                source: HpSource::Environment,
                ..
            }
        )));
        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert!(hp.current < 30);
    }

    #[test]
    fn fire_cloud_region_resisted() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let mut rng = make_rng();

        let mon = world.spawn((
            Positioned(Position::new(10, 5)),
            HitPoints {
                current: 30,
                max: 30,
            },
            Intrinsics {
                fire_resistance: true,
                ..Intrinsics::default()
            },
        ));

        let mut rm = RegionMap::default();
        create_region(
            &mut rm,
            Position::new(10, 5),
            1,
            RegionType::FireCloud,
            3,
        );

        let _events = tick_regions(&mut world, &mut rm, &mut rng);
        let hp = world.get_component::<HitPoints>(mon).unwrap();
        assert_eq!(hp.current, 30, "fire-resistant entity takes no fire cloud damage");
    }
}
