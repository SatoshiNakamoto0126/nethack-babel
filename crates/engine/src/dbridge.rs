//! Drawbridge mechanics: raising, lowering, and toggling drawbridges.
//!
//! Implements NetHack's drawbridge system from `dbridge.c`.  A drawbridge
//! can be open (walkable `Terrain::Drawbridge`) or raised (becomes
//! `Terrain::Wall`).  Entities standing on a drawbridge when it raises
//! are crushed.
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;

use crate::action::Position;
use crate::dungeon::Terrain;
use crate::event::{DeathCause, EngineEvent, HpSource};
use crate::world::{GameWorld, HitPoints, Monster, Player, Positioned};

// ---------------------------------------------------------------------------
// Dice helpers
// ---------------------------------------------------------------------------

/// Roll `n` dice of `s` sides using a simple deterministic formula
/// for testing.  The real game uses an RNG, but drawbridge functions
/// don't take an RNG parameter — damage is fixed.
#[inline]
fn d_fixed(n: u32, s: u32) -> u32 {
    // Average roll: n * (s+1) / 2.  We use the midpoint for tests.
    n * (s + 1) / 2
}

// ---------------------------------------------------------------------------
// Helper: read terrain at a position
// ---------------------------------------------------------------------------

/// Read the terrain at `pos` from the current level.
#[inline]
fn level_terrain(world: &GameWorld, pos: Position) -> Option<Terrain> {
    world
        .dungeon()
        .current_level
        .get(pos)
        .map(|c| c.terrain)
}

// ---------------------------------------------------------------------------
// Drawbridge state query
// ---------------------------------------------------------------------------

/// Drawbridge state: open (lowered / walkable) or raised (wall).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawbridgeState {
    Open,
    Raised,
}

/// Query the state of a drawbridge at `pos`.
/// Returns `None` if the tile is neither a drawbridge nor a wall that
/// was once a drawbridge (we treat any `Wall` at the position as
/// potentially raised).
pub fn drawbridge_state(world: &GameWorld, pos: Position) -> Option<DrawbridgeState> {
    let terrain = level_terrain(world, pos)?;
    match terrain {
        Terrain::Drawbridge => Some(DrawbridgeState::Open),
        Terrain::Wall => Some(DrawbridgeState::Raised),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Raise drawbridge
// ---------------------------------------------------------------------------

/// Raise the drawbridge at `pos`: it becomes a wall.
///
/// Entities on the drawbridge are crushed:
/// - Monsters: instakill
/// - Player: takes d(6,10) damage
///
/// Returns the events produced.  Does nothing if the tile is not an
/// open drawbridge.
pub fn raise_drawbridge(
    world: &mut GameWorld,
    pos: Position,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Verify it's an open drawbridge.
    let terrain = match level_terrain(world, pos) {
        Some(t) => t,
        None => return events,
    };
    if terrain != Terrain::Drawbridge {
        events.push(EngineEvent::msg("not-a-drawbridge"));
        return events;
    }

    // Find entities on the drawbridge.
    let entities_on: Vec<Entity> = world
        .query::<Positioned>()
        .iter()
        .filter(|(_, p)| p.0 == pos)
        .map(|(e, _)| e)
        .collect();

    // Change terrain to Wall.
    world
        .dungeon_mut()
        .current_level
        .set_terrain(pos, Terrain::Wall);

    events.push(EngineEvent::msg("drawbridge-raises"));

    // Crush entities.
    for entity in entities_on {
        if world.get_component::<Player>(entity).is_some() {
            // Player: take d(6,10) damage.
            let damage = d_fixed(6, 10) as i32;
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
                hp.current -= damage;
                events.push(EngineEvent::HpChange {
                    entity,
                    amount: -damage,
                    new_hp: hp.current,
                    source: HpSource::Environment,
                });
                if hp.current <= 0 {
                    events.push(EngineEvent::EntityDied {
                        entity,
                        killer: None,
                        cause: DeathCause::CrushedByBoulder, // closest match
                    });
                }
            }
        } else if world.get_component::<Monster>(entity).is_some() {
            // Monster: instakill.
            let name = world.entity_name(entity);
            events.push(EngineEvent::EntityDied {
                entity,
                killer: None,
                cause: DeathCause::KilledBy {
                    killer_name: "drawbridge".to_string(),
                },
            });
            events.push(EngineEvent::msg_with(
                "drawbridge-crushes",
                vec![("name", name)],
            ));
            let _ = world.despawn(entity);
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Lower drawbridge
// ---------------------------------------------------------------------------

/// Lower the drawbridge at `pos`: the wall becomes walkable drawbridge
/// terrain.
///
/// Returns the events produced.  Does nothing if the tile is not a wall
/// (or raised drawbridge).
pub fn lower_drawbridge(
    world: &mut GameWorld,
    pos: Position,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let terrain = match level_terrain(world, pos) {
        Some(t) => t,
        None => return events,
    };
    if terrain != Terrain::Wall {
        events.push(EngineEvent::msg("not-a-raised-drawbridge"));
        return events;
    }

    world
        .dungeon_mut()
        .current_level
        .set_terrain(pos, Terrain::Drawbridge);

    events.push(EngineEvent::msg("drawbridge-lowers"));
    events
}

// ---------------------------------------------------------------------------
// Toggle drawbridge
// ---------------------------------------------------------------------------

/// Toggle the drawbridge at `pos`: if open, raise it; if raised, lower it.
pub fn toggle_drawbridge(
    world: &mut GameWorld,
    pos: Position,
) -> Vec<EngineEvent> {
    let terrain = level_terrain(world, pos);
    match terrain {
        Some(Terrain::Drawbridge) => raise_drawbridge(world, pos),
        Some(Terrain::Wall) => lower_drawbridge(world, pos),
        _ => vec![EngineEvent::msg("not-a-drawbridge")],
    }
}

// ---------------------------------------------------------------------------
// Wand of striking vs raised drawbridge
// ---------------------------------------------------------------------------

/// Resolve a wand of striking hitting a raised drawbridge.
/// 1/4 chance to destroy it (make it floor).
pub fn strike_drawbridge(
    world: &mut GameWorld,
    pos: Position,
    rng: &mut impl rand::Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let terrain = match level_terrain(world, pos) {
        Some(t) => t,
        None => return events,
    };
    if terrain != Terrain::Wall {
        return events;
    }

    if rng.random_range(0..4u32) == 0 {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(pos, Terrain::Floor);
        events.push(EngineEvent::msg("drawbridge-destroyed"));
    } else {
        events.push(EngineEvent::msg("drawbridge-resists"));
    }

    events
}

// ---------------------------------------------------------------------------
// Entity interaction helpers (pure functions, no ECS)
// ---------------------------------------------------------------------------

/// Result of a drawbridge closing on an entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeCrushResult {
    /// Entity dodged (e.g., flying or noncorporeal).
    Dodged { entity: String },
    /// Entity was crushed.
    Crushed { entity: String },
}

/// What happens when the drawbridge closes on entities at a position.
///
/// Flying entities dodge; others are crushed.  Mirrors the NetHack C
/// `e_missed()` / `automiss()` logic where flyers and noncorporeal
/// entities survive.
pub fn bridge_crush_check(
    entities_at_pos: &[(String, bool)], // (name, is_flying)
) -> Vec<BridgeCrushResult> {
    entities_at_pos
        .iter()
        .map(|(name, flying)| {
            if *flying {
                BridgeCrushResult::Dodged {
                    entity: name.clone(),
                }
            } else {
                BridgeCrushResult::Crushed {
                    entity: name.clone(),
                }
            }
        })
        .collect()
}

/// Entity tries to jump clear of a closing drawbridge / portcullis.
///
/// Based on NetHack's `e_jumps()`: base 4/10 chance, modified by
/// dexterity.  Each point above 10 adds +1, each below subtracts 1.
pub fn entity_jumps_clear(
    dexterity: i32,
    rng: &mut impl rand::Rng,
) -> bool {
    let mut chance = 4 + (dexterity - 10); // base 4 out of 10
    chance = chance.clamp(0, 9);
    (rng.random_range(0..10i32)) < chance
}

/// Terrain type underneath a drawbridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeUnderTerrain {
    Water,
    Lava,
    Moat,
    Floor,
}

/// What terrain is underneath the drawbridge, based on its descriptor.
///
/// Mirrors NetHack's `DB_UNDER` mask (DB_MOAT, DB_LAVA, DB_ICE, etc.).
pub fn bridge_terrain_below(bridge_type: &str) -> BridgeUnderTerrain {
    match bridge_type {
        "over_water" | "water" => BridgeUnderTerrain::Water,
        "over_lava" | "lava" => BridgeUnderTerrain::Lava,
        "over_moat" | "moat" => BridgeUnderTerrain::Moat,
        _ => BridgeUnderTerrain::Floor,
    }
}

/// Trigger a bridge state change via a lever, button, or explicit command.
///
/// Returns the new state.  `"toggle"` flips; `"lower"` / `"raise"` are
/// idempotent if the bridge is already in the target state.
pub fn trigger_bridge(
    current_state: DrawbridgeState,
    trigger_type: &str,
) -> DrawbridgeState {
    match (current_state, trigger_type) {
        (DrawbridgeState::Raised, "lower") => DrawbridgeState::Open,
        (DrawbridgeState::Open, "raise") => DrawbridgeState::Raised,
        (_, "toggle") => {
            if current_state == DrawbridgeState::Raised {
                DrawbridgeState::Open
            } else {
                DrawbridgeState::Raised
            }
        }
        _ => current_state,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

    use crate::dungeon::{LevelMap, Terrain};
    use crate::world::{GameWorld, HitPoints, Monster, Name, Positioned, Speed};

    fn _test_rng() -> Pcg64Mcg {
        Pcg64Mcg::seed_from_u64(42)
    }

    fn make_world_with_drawbridge() -> (GameWorld, Position) {
        let bridge_pos = Position::new(10, 5);
        let mut world = GameWorld::new(Position::new(8, 5));

        let mut map = LevelMap::new(20, 10);
        for y in 0..10 {
            for x in 0..20 {
                let t = if x == 0 || y == 0 || x == 19 || y == 9 {
                    Terrain::Wall
                } else {
                    Terrain::Floor
                };
                map.set_terrain(Position::new(x as i32, y as i32), t);
            }
        }
        // Place a drawbridge at bridge_pos.
        map.set_terrain(bridge_pos, Terrain::Drawbridge);
        world.dungeon_mut().current_level = map;

        (world, bridge_pos)
    }

    /// Helper to read terrain in tests.
    fn terrain_at(world: &GameWorld, pos: Position) -> Terrain {
        world
            .dungeon()
            .current_level
            .get(pos)
            .unwrap()
            .terrain
    }

    // -----------------------------------------------------------------------
    // Test 1: Raise an open drawbridge
    // -----------------------------------------------------------------------
    #[test]
    fn raise_open_drawbridge() {
        let (mut world, pos) = make_world_with_drawbridge();

        let events = raise_drawbridge(&mut world, pos);
        assert_eq!(terrain_at(&world, pos), Terrain::Wall);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "drawbridge-raises")));
    }

    // -----------------------------------------------------------------------
    // Test 2: Lower a raised drawbridge
    // -----------------------------------------------------------------------
    #[test]
    fn lower_raised_drawbridge() {
        let (mut world, pos) = make_world_with_drawbridge();

        // First raise it.
        let _ = raise_drawbridge(&mut world, pos);
        assert_eq!(terrain_at(&world, pos), Terrain::Wall);

        // Now lower it.
        let events = lower_drawbridge(&mut world, pos);
        assert_eq!(terrain_at(&world, pos), Terrain::Drawbridge);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "drawbridge-lowers")));
    }

    // -----------------------------------------------------------------------
    // Test 3: Toggle drawbridge
    // -----------------------------------------------------------------------
    #[test]
    fn toggle_cycles_state() {
        let (mut world, pos) = make_world_with_drawbridge();

        // Open -> Raised
        let _ = toggle_drawbridge(&mut world, pos);
        assert_eq!(terrain_at(&world, pos), Terrain::Wall);

        // Raised -> Open
        let _ = toggle_drawbridge(&mut world, pos);
        assert_eq!(terrain_at(&world, pos), Terrain::Drawbridge);
    }

    // -----------------------------------------------------------------------
    // Test 4: Raising crushes monsters (instakill)
    // -----------------------------------------------------------------------
    #[test]
    fn raising_crushes_monster() {
        let (mut world, pos) = make_world_with_drawbridge();

        let monster = world.spawn((
            Monster,
            Positioned(pos),
            Name("orc".to_string()),
            HitPoints { current: 20, max: 20 },
            Speed(12),
        ));

        let events = raise_drawbridge(&mut world, pos);

        // Monster should be despawned.
        assert!(
            world.get_component::<Monster>(monster).is_none(),
            "monster should be destroyed"
        );
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::EntityDied { .. })));
    }

    // -----------------------------------------------------------------------
    // Test 5: Raising damages player
    // -----------------------------------------------------------------------
    #[test]
    fn raising_damages_player() {
        let bridge_pos = Position::new(10, 5);
        // Place player ON the drawbridge.
        let mut world = GameWorld::new(bridge_pos);

        let mut map = LevelMap::new(20, 10);
        for y in 0..10 {
            for x in 0..20 {
                map.set_terrain(
                    Position::new(x as i32, y as i32),
                    Terrain::Floor,
                );
            }
        }
        map.set_terrain(bridge_pos, Terrain::Drawbridge);
        world.dungeon_mut().current_level = map;

        let events = raise_drawbridge(&mut world, bridge_pos);

        // Player should take damage.
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::HpChange { amount, .. } if *amount < 0)));
    }

    // -----------------------------------------------------------------------
    // Test 6: Strike drawbridge has 1/4 destroy chance
    // -----------------------------------------------------------------------
    #[test]
    fn strike_drawbridge_destroy_chance() {
        // Run many trials to verify roughly 1/4 destruction rate.
        let mut destroyed = 0u32;
        let trials = 400;

        for seed in 0..trials {
            let (mut world, pos) = make_world_with_drawbridge();
            // Raise it first so it's a wall.
            let _ = raise_drawbridge(&mut world, pos);

            let mut rng = Pcg64Mcg::seed_from_u64(seed as u64);
            let events = strike_drawbridge(&mut world, pos, &mut rng);

            if events.iter().any(|e| matches!(e,
                EngineEvent::Message { key, .. } if key == "drawbridge-destroyed"))
            {
                destroyed += 1;
            }
        }

        // Allow wide tolerance: 10%-40% (expected 25%).
        let rate = destroyed as f64 / trials as f64;
        assert!(
            rate > 0.10 && rate < 0.40,
            "destruction rate {:.1}% outside expected range",
            rate * 100.0
        );
    }

    // -----------------------------------------------------------------------
    // Test 7: Bridge crush - flying entity dodges
    // -----------------------------------------------------------------------
    #[test]
    fn bridge_crush_flying_dodges() {
        let entities = vec![
            ("bat".to_string(), true),
            ("orc".to_string(), false),
        ];
        let results = bridge_crush_check(&entities);
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0],
            BridgeCrushResult::Dodged {
                entity: "bat".to_string()
            }
        );
        assert_eq!(
            results[1],
            BridgeCrushResult::Crushed {
                entity: "orc".to_string()
            }
        );
    }

    // -----------------------------------------------------------------------
    // Test 8: Bridge crush - all non-flying crushed
    // -----------------------------------------------------------------------
    #[test]
    fn bridge_crush_all_grounded() {
        let entities = vec![
            ("orc".to_string(), false),
            ("troll".to_string(), false),
        ];
        let results = bridge_crush_check(&entities);
        assert!(results.iter().all(|r| matches!(r, BridgeCrushResult::Crushed { .. })));
    }

    // -----------------------------------------------------------------------
    // Test 9: Bridge trigger toggle
    // -----------------------------------------------------------------------
    #[test]
    fn trigger_bridge_toggle() {
        assert_eq!(
            trigger_bridge(DrawbridgeState::Open, "toggle"),
            DrawbridgeState::Raised
        );
        assert_eq!(
            trigger_bridge(DrawbridgeState::Raised, "toggle"),
            DrawbridgeState::Open
        );
    }

    // -----------------------------------------------------------------------
    // Test 10: Bridge trigger lower/raise
    // -----------------------------------------------------------------------
    #[test]
    fn trigger_bridge_explicit() {
        assert_eq!(
            trigger_bridge(DrawbridgeState::Raised, "lower"),
            DrawbridgeState::Open
        );
        assert_eq!(
            trigger_bridge(DrawbridgeState::Open, "raise"),
            DrawbridgeState::Raised
        );
        // Idempotent: lowering an open bridge stays open.
        assert_eq!(
            trigger_bridge(DrawbridgeState::Open, "lower"),
            DrawbridgeState::Open
        );
    }

    // -----------------------------------------------------------------------
    // Test 11: Entity jump check - high dex succeeds more often
    // -----------------------------------------------------------------------
    #[test]
    fn entity_jump_high_dex() {
        let mut successes = 0;
        let trials = 200;
        for seed in 0..trials {
            let mut rng = Pcg64Mcg::seed_from_u64(seed as u64);
            if entity_jumps_clear(18, &mut rng) {
                successes += 1;
            }
        }
        // dex 18 => chance = 4 + 8 = 12, clamped to 9, so ~90%
        let rate = successes as f64 / trials as f64;
        assert!(
            rate > 0.75,
            "high-dex jump rate {:.1}% too low",
            rate * 100.0
        );
    }

    // -----------------------------------------------------------------------
    // Test 12: Entity jump check - low dex fails more often
    // -----------------------------------------------------------------------
    #[test]
    fn entity_jump_low_dex() {
        let mut successes = 0;
        let trials = 200;
        for seed in 0..trials {
            let mut rng = Pcg64Mcg::seed_from_u64(seed as u64);
            if entity_jumps_clear(3, &mut rng) {
                successes += 1;
            }
        }
        // dex 3 => chance = 4 + (3-10) = -3, clamped to 0, so ~0%
        assert_eq!(successes, 0, "low-dex should never jump clear");
    }

    // -----------------------------------------------------------------------
    // Test 13: Bridge terrain below
    // -----------------------------------------------------------------------
    #[test]
    fn bridge_terrain_below_variants() {
        assert_eq!(bridge_terrain_below("over_water"), BridgeUnderTerrain::Water);
        assert_eq!(bridge_terrain_below("over_lava"), BridgeUnderTerrain::Lava);
        assert_eq!(bridge_terrain_below("over_moat"), BridgeUnderTerrain::Moat);
        assert_eq!(bridge_terrain_below("moat"), BridgeUnderTerrain::Moat);
        assert_eq!(bridge_terrain_below("other"), BridgeUnderTerrain::Floor);
    }
}
