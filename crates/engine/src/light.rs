//! Light source management for NetHack Babel.
//!
//! Tracks fuel-based and permanent light sources.  Lit items consume
//! fuel each turn; when fuel reaches zero, the item is extinguished.
//! Magic lamps have infinite fuel.  Sunsword emits light when wielded
//! (no fuel needed).
//!
//! Light radius from carried/wielded sources adds to the player's FOV
//! radius.
//!
//! All functions are pure: they operate on `GameWorld`, mutate world
//! state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use serde::{Deserialize, Serialize};

use crate::event::EngineEvent;
use crate::world::GameWorld;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Oil lamp: 1500 turns of fuel.
pub const OIL_LAMP_FUEL: u32 = 1500;

/// Brass lantern: 1500 turns of fuel.
pub const BRASS_LANTERN_FUEL: u32 = 1500;

/// Tallow candle: 100-200 turns of fuel.
pub const TALLOW_CANDLE_FUEL_MIN: u32 = 100;
pub const TALLOW_CANDLE_FUEL_MAX: u32 = 200;

/// Wax candle: 400-500 turns of fuel.
pub const WAX_CANDLE_FUEL_MIN: u32 = 400;
pub const WAX_CANDLE_FUEL_MAX: u32 = 500;

/// Magic lamp: effectively infinite fuel.
pub const MAGIC_LAMP_FUEL: u32 = u32::MAX;

// ---------------------------------------------------------------------------
// Light source type
// ---------------------------------------------------------------------------

/// Classification of a light-emitting item for fuel and radius rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LightKind {
    /// Tallow candle (radius 1, 100-200 turns).
    TallowCandle,
    /// Wax candle (radius 1, 400-500 turns).
    WaxCandle,
    /// Oil lamp (radius 2, 1500 turns).
    OilLamp,
    /// Brass lantern (radius 2, 1500 turns).
    BrassLantern,
    /// Magic lamp (radius 2, infinite fuel).
    MagicLamp,
    /// Sunsword (radius 1, no fuel — emits light when wielded).
    Sunsword,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// Component: light-source state attached to an item entity.
///
/// This is distinct from the data-crate `LightSource` component (which
/// stores `lit` and `recharged` fields from C NetHack).  This engine-
/// level component adds fuel tracking for the game loop.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LightFuel {
    /// Whether the light is currently on.
    pub lit: bool,
    /// Remaining fuel in turns (0 means exhausted).
    pub fuel: u32,
    /// Maximum fuel capacity.
    pub max_fuel: u32,
    /// What kind of light source this is.
    pub kind: LightKind,
}

impl LightFuel {
    /// Create a new light source with the given kind and full fuel.
    pub fn new(kind: LightKind) -> Self {
        let max_fuel = match kind {
            LightKind::TallowCandle => TALLOW_CANDLE_FUEL_MIN,
            LightKind::WaxCandle => WAX_CANDLE_FUEL_MIN,
            LightKind::OilLamp => OIL_LAMP_FUEL,
            LightKind::BrassLantern => BRASS_LANTERN_FUEL,
            LightKind::MagicLamp => MAGIC_LAMP_FUEL,
            LightKind::Sunsword => MAGIC_LAMP_FUEL, // effectively infinite
        };
        Self {
            lit: false,
            fuel: max_fuel,
            max_fuel,
            kind,
        }
    }

    /// Create with a specific fuel amount.
    pub fn with_fuel(kind: LightKind, fuel: u32) -> Self {
        let max_fuel = match kind {
            LightKind::TallowCandle => TALLOW_CANDLE_FUEL_MAX,
            LightKind::WaxCandle => WAX_CANDLE_FUEL_MAX,
            LightKind::OilLamp => OIL_LAMP_FUEL,
            LightKind::BrassLantern => BRASS_LANTERN_FUEL,
            LightKind::MagicLamp => MAGIC_LAMP_FUEL,
            LightKind::Sunsword => MAGIC_LAMP_FUEL,
        };
        Self {
            lit: false,
            fuel,
            max_fuel,
            kind,
        }
    }
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

/// Check whether a light source entity is currently lit.
pub fn is_lit(world: &GameWorld, item: Entity) -> bool {
    world
        .get_component::<LightFuel>(item)
        .is_some_and(|lf| lf.lit)
}

/// Return the light radius for a given light kind.
///
/// - Candles: radius 1
/// - Oil lamp / Brass lantern / Magic lamp: radius 2
/// - Sunsword: radius 1 (when wielded)
pub fn light_radius_for_kind(kind: LightKind) -> u32 {
    match kind {
        LightKind::TallowCandle | LightKind::WaxCandle | LightKind::Sunsword => 1,
        LightKind::OilLamp | LightKind::BrassLantern | LightKind::MagicLamp => 2,
    }
}

/// Compute the total light bonus from all lit light sources carried by
/// the player.  This value is added to the base FOV radius.
///
/// Takes the maximum radius among all lit sources (light sources don't
/// stack additively in NetHack).
pub fn player_light_radius(world: &GameWorld) -> u32 {
    let mut max_radius: u32 = 0;

    for (_entity, lf) in world.ecs().query::<&LightFuel>().iter() {
        if lf.lit {
            let r = light_radius_for_kind(lf.kind);
            if r > max_radius {
                max_radius = r;
            }
        }
    }

    max_radius
}

// ---------------------------------------------------------------------------
// Toggle
// ---------------------------------------------------------------------------

/// Toggle a light source on or off.
///
/// Turning on a light source with 0 fuel emits a "no fuel" message
/// and does nothing.
pub fn toggle_light(world: &mut GameWorld, item: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let state = world.get_component::<LightFuel>(item).map(|lf| (lf.lit, lf.fuel, lf.kind));
    match state {
        Some((true, _fuel, _kind)) => {
            // Turn off.
            if let Some(mut lf) = world.get_component_mut::<LightFuel>(item) {
                lf.lit = false;
            }
            events.push(EngineEvent::msg("light-extinguished"));
        }
        Some((false, fuel, kind)) => {
            if fuel == 0 {
                events.push(EngineEvent::msg("light-no-fuel"));
                return events;
            }
            // Sunsword doesn't need to be explicitly lit; it emits
            // light when wielded.  But we allow toggling for
            // consistency.
            if kind == LightKind::MagicLamp || kind == LightKind::Sunsword {
                // These never run out.
            }
            if let Some(mut lf) = world.get_component_mut::<LightFuel>(item) {
                lf.lit = true;
            }
            events.push(EngineEvent::msg("light-lit"));
        }
        None => {
            events.push(EngineEvent::msg("light-not-a-source"));
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Per-turn fuel tick
// ---------------------------------------------------------------------------

/// Tick all lit light sources: decrement fuel by 1 for each lit
/// non-infinite source.  Extinguish any that reach 0 fuel.
///
/// Returns events for each light source that goes out.
pub fn tick_light_sources(world: &mut GameWorld) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Collect entities to update (avoid borrow conflict).
    let to_update: Vec<(Entity, LightKind)> = world
        .ecs()
        .query::<&LightFuel>()
        .iter()
        .filter(|(_e, lf)| lf.lit)
        .map(|(e, lf)| (e, lf.kind))
        .collect();

    for (entity, kind) in to_update {
        // Magic lamp and Sunsword have infinite fuel.
        if kind == LightKind::MagicLamp || kind == LightKind::Sunsword {
            continue;
        }

        let went_out = if let Some(mut lf) = world.get_component_mut::<LightFuel>(entity) {
            lf.fuel = lf.fuel.saturating_sub(1);
            if lf.fuel == 0 {
                lf.lit = false;
                true
            } else {
                false
            }
        } else {
            false
        };

        if went_out {
            events.push(EngineEvent::msg_with(
                "light-burned-out",
                vec![("kind", format!("{:?}", kind))],
            ));
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Light source manager — level-wide tracking of all active light sources
// ---------------------------------------------------------------------------

/// Event emitted when a light source changes state.
#[derive(Debug, Clone)]
pub enum LightEvent {
    /// A light source was extinguished due to fuel exhaustion.
    Extinguished { id: u32, kind: LightKind },
    /// A light source is running low on fuel (< 20%).
    FuelLow { id: u32, kind: LightKind, fuel_remaining: u32 },
}

/// A tracked light source in the game world.
///
/// Unlike `LightFuel` (an ECS component attached to item entities),
/// `LightSourceEntry` is a standalone record used by `LightSourceManager`
/// to track positional light sources on a level (matching C NetHack's
/// `light_source` linked list).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightSourceEntry {
    pub id: u32,
    pub kind: LightKind,
    pub position: (i32, i32),
    pub radius: i32,
    pub fuel_remaining: i32, // -1 for permanent
    pub is_lit: bool,
}

/// Light source manager — tracks all active light sources on the level.
///
/// This mirrors C NetHack's `light_base` linked list and
/// `do_light_sources()` function from `light.c`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LightSourceManager {
    sources: Vec<LightSourceEntry>,
    next_id: u32,
}

impl LightSourceManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new light source at the given position.  Returns its ID.
    pub fn add(&mut self, kind: LightKind, pos: (i32, i32)) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let radius = light_radius_for_kind(kind) as i32;
        let fuel = initial_fuel(kind);
        self.sources.push(LightSourceEntry {
            id,
            kind,
            position: pos,
            radius,
            fuel_remaining: fuel,
            is_lit: true,
        });
        id
    }

    /// Remove a light source by ID.
    pub fn remove(&mut self, id: u32) {
        self.sources.retain(|s| s.id != id);
    }

    /// Tick all sources: reduce fuel, extinguish expired ones.
    ///
    /// Returns events for state changes.
    pub fn tick(&mut self) -> Vec<LightEvent> {
        let mut events = Vec::new();
        for source in &mut self.sources {
            if !source.is_lit || source.fuel_remaining < 0 {
                continue; // not lit, or permanent
            }
            source.fuel_remaining -= 1;
            if source.fuel_remaining <= 0 {
                source.is_lit = false;
                source.fuel_remaining = 0;
                events.push(LightEvent::Extinguished {
                    id: source.id,
                    kind: source.kind,
                });
            } else {
                // Warn when fuel drops below 20% of initial.
                let initial = initial_fuel(source.kind);
                if initial > 0 && source.fuel_remaining == initial / 5 {
                    events.push(LightEvent::FuelLow {
                        id: source.id,
                        kind: source.kind,
                        fuel_remaining: source.fuel_remaining as u32,
                    });
                }
            }
        }
        events
    }

    /// Calculate the light level at a position from all active sources.
    ///
    /// Uses Chebyshev distance.  Returns 0 if no source illuminates
    /// the position.
    pub fn light_at(&self, pos: (i32, i32)) -> i32 {
        self.sources
            .iter()
            .filter(|s| s.is_lit)
            .map(|s| {
                let dist = (s.position.0 - pos.0)
                    .abs()
                    .max((s.position.1 - pos.1).abs());
                if dist <= s.radius {
                    s.radius - dist + 1
                } else {
                    0
                }
            })
            .max()
            .unwrap_or(0)
    }

    /// Move a light source to a new position.
    pub fn move_source(&mut self, id: u32, new_pos: (i32, i32)) {
        if let Some(source) = self.sources.iter_mut().find(|s| s.id == id) {
            source.position = new_pos;
        }
    }

    /// Get all positions illuminated by any active light source.
    pub fn all_lit_positions(&self) -> Vec<(i32, i32)> {
        let mut positions = Vec::new();
        for source in &self.sources {
            if !source.is_lit {
                continue;
            }
            for dy in -source.radius..=source.radius {
                for dx in -source.radius..=source.radius {
                    let x = source.position.0 + dx;
                    let y = source.position.1 + dy;
                    if x >= 0 && y >= 0 {
                        positions.push((x, y));
                    }
                }
            }
        }
        positions.sort();
        positions.dedup();
        positions
    }

    /// Get the number of active (lit) sources.
    pub fn active_count(&self) -> usize {
        self.sources.iter().filter(|s| s.is_lit).count()
    }

    /// Get all tracked sources (for serialization or inspection).
    pub fn sources(&self) -> &[LightSourceEntry] {
        &self.sources
    }
}

/// Return the initial fuel amount for a light source kind.
///
/// Returns -1 for permanent sources (magic lamp, Sunsword).
pub fn initial_fuel(kind: LightKind) -> i32 {
    match kind {
        LightKind::TallowCandle => TALLOW_CANDLE_FUEL_MIN as i32,
        LightKind::WaxCandle => WAX_CANDLE_FUEL_MIN as i32,
        LightKind::OilLamp => OIL_LAMP_FUEL as i32,
        LightKind::BrassLantern => BRASS_LANTERN_FUEL as i32,
        LightKind::MagicLamp | LightKind::Sunsword => -1, // permanent
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;

    fn setup_light(world: &mut GameWorld, kind: LightKind) -> Entity {
        world.spawn((LightFuel::new(kind),))
    }

    #[test]
    fn toggle_on_and_off() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let lamp = setup_light(&mut world, LightKind::OilLamp);

        assert!(!is_lit(&world, lamp));
        toggle_light(&mut world, lamp);
        assert!(is_lit(&world, lamp));
        toggle_light(&mut world, lamp);
        assert!(!is_lit(&world, lamp));
    }

    #[test]
    fn toggle_no_fuel() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let lamp = world.spawn((LightFuel::with_fuel(LightKind::OilLamp, 0),));

        let events = toggle_light(&mut world, lamp);
        assert!(!is_lit(&world, lamp));
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "light-no-fuel"
        )));
    }

    #[test]
    fn fuel_decrements_each_tick() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let lamp = world.spawn((LightFuel {
            lit: true,
            fuel: 3,
            max_fuel: OIL_LAMP_FUEL,
            kind: LightKind::OilLamp,
        },));

        tick_light_sources(&mut world);
        assert_eq!(world.get_component::<LightFuel>(lamp).unwrap().fuel, 2);
        tick_light_sources(&mut world);
        assert_eq!(world.get_component::<LightFuel>(lamp).unwrap().fuel, 1);
        tick_light_sources(&mut world);
        // Should have gone out.
        assert!(!is_lit(&world, lamp));
        assert_eq!(world.get_component::<LightFuel>(lamp).unwrap().fuel, 0);
    }

    #[test]
    fn magic_lamp_infinite_fuel() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let lamp = setup_light(&mut world, LightKind::MagicLamp);
        toggle_light(&mut world, lamp);
        assert!(is_lit(&world, lamp));

        // Tick many times; should still be lit.
        for _ in 0..100 {
            tick_light_sources(&mut world);
        }
        assert!(is_lit(&world, lamp));
        assert_eq!(
            world.get_component::<LightFuel>(lamp).unwrap().fuel,
            MAGIC_LAMP_FUEL
        );
    }

    #[test]
    fn light_radius_values() {
        assert_eq!(light_radius_for_kind(LightKind::TallowCandle), 1);
        assert_eq!(light_radius_for_kind(LightKind::WaxCandle), 1);
        assert_eq!(light_radius_for_kind(LightKind::OilLamp), 2);
        assert_eq!(light_radius_for_kind(LightKind::BrassLantern), 2);
        assert_eq!(light_radius_for_kind(LightKind::MagicLamp), 2);
        assert_eq!(light_radius_for_kind(LightKind::Sunsword), 1);
    }

    #[test]
    fn player_light_radius_picks_max() {
        let mut world = GameWorld::new(Position::new(40, 10));

        // No lit sources: radius 0.
        assert_eq!(player_light_radius(&world), 0);

        // Light a candle (radius 1).
        let candle = setup_light(&mut world, LightKind::TallowCandle);
        toggle_light(&mut world, candle);
        assert_eq!(player_light_radius(&world), 1);

        // Light a lamp (radius 2) — should take the max.
        let lamp = setup_light(&mut world, LightKind::OilLamp);
        toggle_light(&mut world, lamp);
        assert_eq!(player_light_radius(&world), 2);

        // Turn off the lamp — should drop back to 1.
        toggle_light(&mut world, lamp);
        assert_eq!(player_light_radius(&world), 1);
    }

    // ── LightSourceManager tests ──────────────────────────────────

    #[test]
    fn test_manager_add_light_source() {
        let mut mgr = LightSourceManager::new();
        let id = mgr.add(LightKind::OilLamp, (10, 5));
        assert_eq!(mgr.sources().len(), 1);
        assert_eq!(mgr.sources()[0].id, id);
        assert!(mgr.sources()[0].is_lit);
        assert_eq!(mgr.sources()[0].radius, 2); // oil lamp radius
    }

    #[test]
    fn test_manager_remove_source() {
        let mut mgr = LightSourceManager::new();
        let id1 = mgr.add(LightKind::OilLamp, (10, 5));
        let id2 = mgr.add(LightKind::TallowCandle, (20, 5));
        assert_eq!(mgr.sources().len(), 2);

        mgr.remove(id1);
        assert_eq!(mgr.sources().len(), 1);
        assert_eq!(mgr.sources()[0].id, id2);
    }

    #[test]
    fn test_manager_tick_reduces_fuel() {
        let mut mgr = LightSourceManager::new();
        mgr.add(LightKind::OilLamp, (10, 5));

        let initial = mgr.sources()[0].fuel_remaining;
        mgr.tick();
        assert_eq!(
            mgr.sources()[0].fuel_remaining,
            initial - 1,
            "fuel should decrease by 1 per tick"
        );
    }

    #[test]
    fn test_manager_permanent_light_no_fuel_loss() {
        let mut mgr = LightSourceManager::new();
        mgr.add(LightKind::MagicLamp, (10, 5));

        assert_eq!(mgr.sources()[0].fuel_remaining, -1);
        for _ in 0..100 {
            mgr.tick();
        }
        assert_eq!(
            mgr.sources()[0].fuel_remaining, -1,
            "permanent source should not lose fuel"
        );
        assert!(mgr.sources()[0].is_lit, "permanent source should stay lit");
    }

    #[test]
    fn test_manager_candle_extinguishes_at_zero() {
        let mut mgr = LightSourceManager::new();
        mgr.add(LightKind::TallowCandle, (10, 5));

        // Tick until fuel runs out.
        let initial = mgr.sources()[0].fuel_remaining;
        let mut extinguished = false;
        for _ in 0..=initial {
            let events = mgr.tick();
            if events.iter().any(|e| matches!(e, LightEvent::Extinguished { .. })) {
                extinguished = true;
                break;
            }
        }
        assert!(extinguished, "candle should extinguish when fuel hits 0");
        assert!(!mgr.sources()[0].is_lit, "candle should no longer be lit");
    }

    #[test]
    fn test_manager_light_at_position() {
        let mut mgr = LightSourceManager::new();
        // Oil lamp at (10, 10) with radius 2.
        mgr.add(LightKind::OilLamp, (10, 10));

        // At the source: radius - 0 + 1 = 3
        assert_eq!(mgr.light_at((10, 10)), 3);
        // Distance 1: radius - 1 + 1 = 2
        assert_eq!(mgr.light_at((11, 10)), 2);
        // Distance 2: radius - 2 + 1 = 1
        assert_eq!(mgr.light_at((12, 10)), 1);
        // Distance 3: out of range = 0
        assert_eq!(mgr.light_at((13, 10)), 0);
    }

    #[test]
    fn test_manager_move_source() {
        let mut mgr = LightSourceManager::new();
        let id = mgr.add(LightKind::OilLamp, (10, 10));

        assert_eq!(mgr.light_at((10, 10)), 3);
        assert_eq!(mgr.light_at((20, 10)), 0);

        mgr.move_source(id, (20, 10));
        assert_eq!(mgr.light_at((10, 10)), 0);
        assert_eq!(mgr.light_at((20, 10)), 3);
    }

    #[test]
    fn test_manager_all_lit_positions() {
        let mut mgr = LightSourceManager::new();
        // Candle at (5, 5) with radius 1.
        mgr.add(LightKind::TallowCandle, (5, 5));

        let positions = mgr.all_lit_positions();
        // Radius 1: 3x3 = 9 positions.
        assert_eq!(positions.len(), 9);
        assert!(positions.contains(&(5, 5)));
        assert!(positions.contains(&(4, 4)));
        assert!(positions.contains(&(6, 6)));
    }

    #[test]
    fn test_initial_fuel_values() {
        assert!(initial_fuel(LightKind::TallowCandle) > 0);
        assert!(initial_fuel(LightKind::OilLamp) > 0);
        assert_eq!(initial_fuel(LightKind::MagicLamp), -1);
        assert_eq!(initial_fuel(LightKind::Sunsword), -1);
    }
}
