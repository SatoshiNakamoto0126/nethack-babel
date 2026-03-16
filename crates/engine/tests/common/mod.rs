//! Shared test helpers for NetHack Babel engine integration tests.
//!
//! Extracted from touchstone.rs to avoid duplicating test harness code
//! across multiple integration test files.

use nethack_babel_engine::action::{PlayerAction, Position};
use nethack_babel_engine::dungeon::{LevelMap, Terrain};
use nethack_babel_engine::event::EngineEvent;
use nethack_babel_engine::religion::ReligionState;
use nethack_babel_engine::turn::resolve_turn;
use nethack_babel_engine::world::{
    GameWorld, HitPoints, Monster, MovementPoints, Name, Positioned, Speed, NORMAL_SPEED,
};

use hecs::Entity;
use nethack_babel_data::Alignment;
use rand::SeedableRng;
use rand_pcg::{Pcg64, Pcg64Mcg};

/// Deterministic RNG for reproducible tests (Pcg64Mcg for simple scenarios).
#[allow(dead_code)]
pub fn test_rng() -> Pcg64Mcg {
    Pcg64Mcg::seed_from_u64(42)
}

/// Create a GameWorld with a seeded Pcg64 RNG for deterministic tests.
#[allow(dead_code)]
pub fn create_test_world(seed: u64) -> (GameWorld, Pcg64) {
    let world = GameWorld::new(Position::new(40, 10));
    let rng = Pcg64::seed_from_u64(seed);
    (world, rng)
}

/// Wrapper around `resolve_turn` for concise test code.
#[allow(dead_code)]
pub fn do_action(
    world: &mut GameWorld,
    action: PlayerAction,
    rng: &mut Pcg64,
) -> Vec<EngineEvent> {
    resolve_turn(world, action, rng)
}

/// Get the player's current position from the world.
#[allow(dead_code)]
pub fn player_pos(world: &GameWorld) -> Position {
    world
        .get_component::<Positioned>(world.player())
        .expect("player should have Positioned")
        .0
}

/// Set the player's HP to specific current/max values.
#[allow(dead_code)]
pub fn set_player_hp(world: &mut GameWorld, current: i32, max: i32) {
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(world.player()) {
        hp.current = current;
        hp.max = max;
    }
}

/// Place a monster entity at the given position and return its Entity handle.
#[allow(dead_code)]
pub fn place_monster(
    world: &mut GameWorld,
    pos: Position,
    name: &str,
    hp: i32,
) -> Entity {
    let order = world.next_creation_order();
    world.spawn((
        Monster,
        Positioned(pos),
        HitPoints {
            current: hp,
            max: hp,
        },
        Speed(12),
        MovementPoints(NORMAL_SPEED as i32),
        Name(name.to_string()),
        order,
    ))
}

/// Check if an entity is alive (exists and has positive HP).
#[allow(dead_code)]
pub fn entity_is_alive(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<HitPoints>(entity)
        .map(|hp| hp.current > 0)
        .unwrap_or(false)
}

/// Create a dummy Entity for religion tests.
#[allow(dead_code)]
pub fn dummy_entity() -> Entity {
    unsafe { std::mem::transmute::<u64, Entity>(1u64) }
}

/// Build a baseline ReligionState with sane defaults for testing.
#[allow(dead_code)]
pub fn make_religion_state() -> ReligionState {
    ReligionState {
        alignment: Alignment::Neutral,
        alignment_record: 10,
        god_anger: 0,
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
        luck: 3,
        luck_bonus: 0,
        has_luckstone: false,
        luckstone_blessed: false,
        luckstone_cursed: false,
        in_gehennom: false,
        is_undead: false,
        is_demon: false,
        original_alignment: Alignment::Neutral,
        has_converted: false,
        alignment_abuse: 0,
    }
}

/// Create a standard test level map with a floor interior.
#[allow(dead_code)]
pub fn make_test_level() -> LevelMap {
    let mut map = LevelMap::new_standard();
    for y in 1..=15 {
        for x in 1..=60 {
            map.set_terrain(Position::new(x, y), Terrain::Floor);
        }
    }
    map
}
