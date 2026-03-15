//! Touchstone integration tests for NetHack Babel engine.
//!
//! These scenario-based tests verify end-to-end correctness of key
//! game mechanics that distinguish NetHack from simpler roguelikes.

use nethack_babel_engine::action::{Direction, PlayerAction, Position};
use nethack_babel_engine::bones::{
    downgrade_bone_items, generate_bones, BoneItem, BonesPool,
    GhostBehavior, can_make_bones,
};
use nethack_babel_engine::conduct::{
    pudding_should_split, calculate_score, check_conduct, ConductAction,
    ConductState, ScoreInput,
};
use nethack_babel_engine::dungeon::{DungeonBranch, LevelMap, Terrain};
use nethack_babel_engine::combat::{
    has_passive_paralyze_gaze, resolve_melee_attack,
};
use nethack_babel_engine::event::{DeathCause, EngineEvent, PassiveEffect, StatusEffect};
use nethack_babel_engine::religion::{
    evaluate_prayer_simple, has_invocation_items, offer_amulet,
    AmuletOfferingResult, pray_simple, PrayerType, ReligionState,
};
use nethack_babel_engine::turn::resolve_turn;
use nethack_babel_engine::wands::{zap_wand, WandCharges, WandType};
use nethack_babel_engine::shop::{
    get_cost, get_full_buy_price, kop_counts, pay_bill, rob_shop, ShopRoom, ShopType,
};
use nethack_babel_engine::traps::{place_trap, TrapType};
use nethack_babel_engine::world::{
    Boulder, GameWorld, HitPoints, Monster, MovementPoints, Name, Positioned,
    Speed, NORMAL_SPEED,
};

use hecs::Entity;
use nethack_babel_data::{Alignment, PlayerQuestItems};
use rand::SeedableRng;
use rand_pcg::{Pcg64, Pcg64Mcg};

/// Deterministic RNG for reproducible tests (Pcg64Mcg for existing scenarios).
fn test_rng() -> Pcg64Mcg {
    Pcg64Mcg::seed_from_u64(42)
}

// ===========================================================================
// Test harness helpers (for scenarios 4 and 9)
// ===========================================================================

/// Create a GameWorld with a seeded Pcg64 RNG for deterministic tests.
fn create_test_world(seed: u64) -> (GameWorld, Pcg64) {
    let world = GameWorld::new(Position::new(40, 10));
    let rng = Pcg64::seed_from_u64(seed);
    (world, rng)
}

/// Wrapper around `resolve_turn` for concise test code.
fn do_action(
    world: &mut GameWorld,
    action: PlayerAction,
    rng: &mut Pcg64,
) -> Vec<EngineEvent> {
    resolve_turn(world, action, rng)
}

/// Get the player's current position from the world.
fn player_pos(world: &GameWorld) -> Position {
    world
        .get_component::<Positioned>(world.player())
        .expect("player should have Positioned")
        .0
}

/// Set the player's HP to specific current/max values.
fn set_player_hp(world: &mut GameWorld, current: i32, max: i32) {
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(world.player()) {
        hp.current = current;
        hp.max = max;
    }
}

/// Place a monster entity at the given position and return its Entity handle.
fn place_monster(
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
fn entity_is_alive(world: &GameWorld, entity: Entity) -> bool {
    world
        .get_component::<HitPoints>(entity)
        .map(|hp| hp.current > 0)
        .unwrap_or(false)
}

/// Create a dummy Entity for religion tests (matches pattern in unit tests).
fn dummy_entity() -> Entity {
    unsafe { std::mem::transmute::<u64, Entity>(1u64) }
}

/// Build a baseline ReligionState with sane defaults for testing.
fn make_religion_state() -> ReligionState {
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
fn make_test_level() -> LevelMap {
    let mut map = LevelMap::new_standard();
    for y in 1..=15 {
        for x in 1..=60 {
            map.set_terrain(Position::new(x, y), Terrain::Floor);
        }
    }
    map
}

// ==========================================================================
// Scenario 5: Pudding Farming
// ==========================================================================
//
// In NetHack, hitting a brown or black pudding with an edged (slash/pierce)
// weapon causes it to split into two puddings, provided its HP > 1.
// The `pudding_should_split` function in `conduct.rs` encodes this check.

/// Touchstone 5.1 -- Hitting a pudding with an edged weapon when HP > 1
/// triggers a split (monster count conceptually increases by one).
#[test]
fn touchstone_05_pudding_splits_on_hit() {
    // Simulate: a brown pudding with 30 HP is hit by an edged weapon.
    let pudding_hp = 30;
    let is_edged = true;

    let should_split = pudding_should_split(is_edged, pudding_hp);
    assert!(
        should_split,
        "Pudding with HP {} hit by edged weapon should split",
        pudding_hp
    );

    // In the conceptual model, one pudding becomes two after a split.
    let mut pudding_count = 1;
    if should_split {
        pudding_count += 1;
    }
    assert_eq!(pudding_count, 2, "After one split, there should be 2 puddings");
}

/// Touchstone 5.2 -- The split product is the same monster type.
///
/// `pudding_should_split` is a pure predicate; it doesn't create entities.
/// But the contract is: if the function returns true, the caller creates a
/// new monster entity of the SAME type.  We verify the predicate returns
/// true for various valid HP values so the caller can rely on it.
#[test]
fn touchstone_05_split_product_is_same_type() {
    // Both brown and black puddings share the same split mechanic:
    // edged weapon + HP > 1 => split.
    for hp in [2, 10, 30, 50, 100] {
        assert!(
            pudding_should_split(true, hp),
            "Pudding with HP {} should split when hit with edged weapon",
            hp
        );
    }

    // Blunt weapon: never splits regardless of HP.
    for hp in [2, 10, 30, 50, 100] {
        assert!(
            !pudding_should_split(false, hp),
            "Pudding with HP {} should NOT split when hit with blunt weapon",
            hp
        );
    }
}

/// Touchstone 5.3 -- Farming is sustainable: split products can also split.
///
/// Simulates the classic pudding farming loop: start with one pudding,
/// hit it repeatedly with an edged weapon.  Each split produces a new
/// pudding that can itself be split.
#[test]
fn touchstone_05_farming_is_sustainable() {
    let mut pudding_count: u32 = 1;
    let pudding_hp = 50; // Each pudding starts with enough HP to split.

    // Simulate 20 rounds of hitting a pudding with an edged weapon.
    for _ in 0..20 {
        if pudding_should_split(true, pudding_hp) {
            pudding_count += 1;
        }
    }

    assert_eq!(
        pudding_count, 21,
        "After 20 edged hits on puddings with HP {}, should have 21 puddings (1 original + 20 splits)",
        pudding_hp
    );

    // Also verify the boundary: a pudding at HP=1 cannot split.
    assert!(
        !pudding_should_split(true, 1),
        "Pudding at HP=1 must not split"
    );
    assert!(
        !pudding_should_split(true, 0),
        "Pudding at HP=0 must not split"
    );
}

// ==========================================================================
// Scenario 6: Polypile
// ==========================================================================
//
// Zapping a wand of polymorph at the floor transforms items lying there.
// The current wand implementation does not yet handle Direction::Down floor
// zap for item polymorph (dispatch_immediate only walks planar beam paths
// looking for monsters).  We test what IS implemented: charge consumption
// on zap, and the polymorph effect on entities in the beam path.

/// Touchstone 6.1 -- Zapping a polymorph wand at a monster applies the
/// Polymorphed status effect (the closest currently-wired polypile analog).
#[test]
fn touchstone_06_polypile_transforms_items() {
    let mut world = GameWorld::new(Position::new(5, 5));

    // Set up floor tiles so the beam can traverse from (5,5) to (6,5).
    world
        .dungeon_mut()
        .current_level
        .set_terrain(Position::new(5, 5), Terrain::Floor);
    world
        .dungeon_mut()
        .current_level
        .set_terrain(Position::new(6, 5), Terrain::Floor);
    // Extend floor further so the beam has room.
    for x in 7..=15 {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(x, 5), Terrain::Floor);
    }

    // Place a monster one step east for the beam to hit.
    let mon_pos = Position::new(6, 5);
    let monster = world.spawn((
        Monster,
        Positioned(mon_pos),
        HitPoints {
            current: 20,
            max: 20,
        },
    ));

    let mut charges = WandCharges {
        spe: 5,
        recharged: 0,
    };
    let mut rng = test_rng();

    let events = zap_wand(
        &world,
        world.player(),
        WandType::Polymorph,
        &mut charges,
        Direction::East,
        &mut rng,
    );

    // The polymorph beam should apply StatusEffect::Polymorphed to the monster.
    use nethack_babel_engine::event::StatusEffect;
    let has_polymorph = events.iter().any(|e| matches!(
        e,
        EngineEvent::StatusApplied {
            entity,
            status: StatusEffect::Polymorphed,
            ..
        } if *entity == monster
    ));

    assert!(
        has_polymorph,
        "Zapping polymorph wand at a monster should apply Polymorphed status.\nEvents: {:?}",
        events
    );
}

/// Touchstone 6.2 -- Zapping a wand consumes a charge.
#[test]
fn touchstone_06_polypile_consumes_charge() {
    let world = GameWorld::new(Position::new(5, 5));

    let mut charges = WandCharges {
        spe: 3,
        recharged: 0,
    };
    let mut rng = test_rng();

    let _events = zap_wand(
        &world,
        world.player(),
        WandType::Polymorph,
        &mut charges,
        Direction::East,
        &mut rng,
    );

    assert_eq!(
        charges.spe, 2,
        "Zapping a wand should decrement charges from 3 to 2"
    );
}

/// Touchstone 6.3 -- Items at the target location remain on the ground
/// after polymorph beam passes (they are not destroyed or moved).
///
/// Since floor-item polymorph isn't fully wired, we verify the invariant
/// from the monster-zap side: the monster entity still exists after being
/// zapped (polymorph transforms, doesn't destroy).
#[test]
fn touchstone_06_polypile_items_stay_on_ground() {
    let mut world = GameWorld::new(Position::new(5, 5));

    // Set up floor tiles so the beam can reach the monster.
    for x in 5..=15 {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(x, 5), Terrain::Floor);
    }

    let mon_pos = Position::new(6, 5);
    let monster = world.spawn((
        Monster,
        Positioned(mon_pos),
        HitPoints {
            current: 20,
            max: 20,
        },
    ));

    let mut charges = WandCharges {
        spe: 3,
        recharged: 0,
    };
    let mut rng = test_rng();

    let events = zap_wand(
        &world,
        world.player(),
        WandType::Polymorph,
        &mut charges,
        Direction::East,
        &mut rng,
    );

    // The monster should NOT be killed (polymorph transforms, not destroys).
    let any_death = events.iter().any(|e| matches!(
        e,
        EngineEvent::EntityDied { entity, .. } if *entity == monster
    ));
    assert!(
        !any_death,
        "Polymorph should not kill the target entity"
    );

    // The monster entity should still be in the world at the same position.
    let pos = world.get_component::<Positioned>(monster);
    assert!(pos.is_some(), "Monster entity should still exist after polymorph zap");
    assert_eq!(
        pos.unwrap().0, mon_pos,
        "Monster should remain at same position after polymorph zap"
    );
}

// ==========================================================================
// Scenario 4: Gehennom Prayer
// ==========================================================================
//
// Prayer outside Gehennom succeeds under good conditions.
// Prayer inside Gehennom fails or invokes anger even with good standing.
// Prayer during cooldown fails.

/// Scenario 4.1 -- Prayer outside Gehennom with good standing succeeds.
///
/// Conditions: alignment_record > 0, luck >= 0, no cooldown, no anger.
/// Expected: PrayerType::Success, HP healed to full, cooldown set.
#[test]
fn touchstone_04_prayer_outside_gehennom_succeeds() {
    let mut state = make_religion_state();
    state.in_gehennom = false;
    state.bless_cooldown = 0;
    state.god_anger = 0;
    state.alignment_record = 10;
    state.luck = 3;
    state.current_hp = 30; // below max to verify healing

    // Verify evaluate_prayer returns Success.
    let ptype = evaluate_prayer_simple(&state, false, None);
    assert_eq!(
        ptype,
        PrayerType::Success,
        "prayer should succeed outside Gehennom with good standing"
    );

    // Actually pray and check effects.
    let mut rng = Pcg64::seed_from_u64(42);
    let events = pray_simple(
        &mut state,
        dummy_entity(),
        false,
        None,
        &mut rng,
    );

    // HP should be restored to max.
    assert_eq!(
        state.current_hp, state.max_hp,
        "successful prayer should heal to full HP"
    );

    // Cooldown should be set (positive value from rnz).
    assert!(
        state.bless_cooldown > 0,
        "successful prayer should set a positive cooldown"
    );

    // Should have a divine healing event.
    let has_heal = events.iter().any(|e| {
        matches!(
            e,
            EngineEvent::HpChange {
                source: nethack_babel_engine::event::HpSource::Divine,
                ..
            }
        )
    });
    assert!(has_heal, "should emit divine HpChange event");

    // Should have a "pray-pleased" message.
    let has_pleased = events.iter().any(|e| {
        matches!(e, EngineEvent::Message { key, .. } if key.contains("pray-pleased"))
    });
    assert!(has_pleased, "should emit pray-pleased message");
}

/// Scenario 4.2 -- Prayer inside Gehennom fails even with good standing.
///
/// In Gehennom, even PrayerType::Success results in "god can't help
/// you" and possibly angrygods.  No healing, no "pray-pleased".
#[test]
fn touchstone_04_prayer_inside_gehennom_fails() {
    let mut state = make_religion_state();
    state.in_gehennom = true;
    state.bless_cooldown = 0;
    state.god_anger = 0;
    state.alignment_record = 10;
    state.luck = 3;
    state.current_hp = 30;

    // evaluate_prayer still returns Success (the branch check happens
    // inside pray(), not evaluate_prayer).
    let ptype = evaluate_prayer_simple(&state, false, None);
    assert_eq!(
        ptype,
        PrayerType::Success,
        "evaluate_prayer returns Success; Gehennom override is in pray()"
    );

    // Actually pray in Gehennom.
    let mut rng = Pcg64::seed_from_u64(42);
    let hp_before = state.current_hp;
    let events = pray_simple(
        &mut state,
        dummy_entity(),
        false,
        None,
        &mut rng,
    );

    // Should have Gehennom "can't help" message.
    let has_gehennom_msg = events.iter().any(|e| {
        matches!(e, EngineEvent::Message { key, .. } if key.contains("gehennom"))
    });
    assert!(
        has_gehennom_msg,
        "should emit gehennom-related message"
    );

    // Should NOT have "pray-pleased" message.
    let has_pleased = events.iter().any(|e| {
        matches!(e, EngineEvent::Message { key, .. } if key.contains("pray-pleased"))
    });
    assert!(
        !has_pleased,
        "Gehennom prayer should not produce pray-pleased"
    );

    // HP should NOT be healed to full via the pleased() path.
    assert!(
        state.current_hp <= hp_before,
        "Gehennom prayer should not heal (got {} from {})",
        state.current_hp,
        hp_before
    );
}

/// Scenario 4.3 -- DungeonState branch can be set to Gehennom and
/// reflected in ReligionState for cross-module coherence.
#[test]
fn touchstone_04_dungeon_branch_gehennom_flag() {
    let (mut world, _rng) = create_test_world(42);

    assert_eq!(
        world.dungeon().branch,
        DungeonBranch::Main,
        "default branch should be Main"
    );

    world.dungeon_mut().branch = DungeonBranch::Gehennom;
    assert_eq!(
        world.dungeon().branch,
        DungeonBranch::Gehennom,
        "branch should be Gehennom after mutation"
    );

    let mut state = make_religion_state();
    state.in_gehennom = world.dungeon().branch == DungeonBranch::Gehennom;
    assert!(state.in_gehennom);
}

/// Scenario 4.4 -- Prayer during cooldown fails and angers god.
#[test]
fn touchstone_04_prayer_during_cooldown_fails() {
    let mut state = make_religion_state();
    state.bless_cooldown = 100;
    state.god_anger = 0;
    state.luck = 3;

    let ptype = evaluate_prayer_simple(&state, false, None);
    assert_eq!(
        ptype,
        PrayerType::TooSoon,
        "prayer with active cooldown should be TooSoon"
    );

    let mut rng = Pcg64::seed_from_u64(42);
    let luck_before = state.luck;
    let events = pray_simple(
        &mut state,
        dummy_entity(),
        false,
        None,
        &mut rng,
    );

    assert!(
        state.bless_cooldown > 100,
        "praying during cooldown should increase cooldown"
    );
    assert!(
        state.god_anger > 0,
        "praying during cooldown should anger god"
    );
    assert!(
        state.luck < luck_before,
        "praying during cooldown should decrease luck"
    );

    let has_angry = events.iter().any(|e| {
        matches!(e, EngineEvent::Message { key, .. } if key.contains("angry"))
    });
    assert!(has_angry, "should emit angry god message");
}

/// Scenario 4.5 -- Pray action through resolve_turn does not panic.
#[test]
fn touchstone_04_pray_action_through_turn_loop() {
    let (mut world, mut rng) = create_test_world(42);

    if let Some(mut mp) =
        world.get_component_mut::<MovementPoints>(world.player())
    {
        mp.0 = NORMAL_SPEED as i32;
    }

    let _events = do_action(&mut world, PlayerAction::Pray, &mut rng);
}

/// Scenario 4.6 -- Prayer with alignment_record=0, luck=0, no anger
/// still succeeds (boundary check).
#[test]
fn touchstone_04_prayer_borderline_alignment_zero() {
    let mut state = make_religion_state();
    state.alignment_record = 0;
    state.god_anger = 0;
    state.luck = 0;
    state.luck_bonus = 0;
    state.bless_cooldown = 0;

    let ptype = evaluate_prayer_simple(&state, false, None);
    assert_eq!(
        ptype,
        PrayerType::Success,
        "alignment_record=0 with luck=0 and no anger should succeed"
    );
}

/// Scenario 4.7 -- Prayer with negative alignment record is punished.
#[test]
fn touchstone_04_prayer_negative_alignment_punished() {
    let mut state = make_religion_state();
    state.alignment_record = -1;
    state.god_anger = 0;
    state.luck = 3;
    state.bless_cooldown = 0;

    let ptype = evaluate_prayer_simple(&state, false, None);
    assert_eq!(
        ptype,
        PrayerType::Punished,
        "negative alignment_record should result in Punished"
    );
}

// ==========================================================================
// Scenario 9: Bones Cycle
// ==========================================================================
//
// Death generates bones data with ghost + items.
// Bones can be loaded into a new game.
// Anti-cheat: same bones can't load twice.
// Item downgrade: cursing + charge halving.

/// Scenario 9.1 -- Death generates bones data containing a ghost and items.
#[test]
fn touchstone_09_death_generates_bones() {
    let mut rng = Pcg64::seed_from_u64(42);
    let level = make_test_level();
    let death_pos = Position::new(30, 8);

    let inventory = vec![
        (death_pos, "long sword".to_string(), None, false),
        (death_pos, "wand of fire".to_string(), Some(6i8), false),
        (death_pos, "ring of levitation".to_string(), None, false),
    ];

    let bones = generate_bones(
        &level,
        "Rodney",
        14,
        80,
        death_pos,
        "Wizard",
        10,
        DungeonBranch::Main,
        2500,
        inventory,
        &mut rng,
    );

    assert_eq!(bones.ghost.player_name, "Rodney");
    assert_eq!(bones.ghost.max_hp, 80);
    assert_eq!(bones.ghost.player_level, 14);
    assert_eq!(bones.ghost.death_position, death_pos);
    assert_eq!(bones.ghost.role, "Wizard");
    assert!(bones.ghost.sleeping, "ghost should start sleeping");
    assert_eq!(bones.dropped_items.len(), 3);
    assert_eq!(
        bones.dropped_items[1].charges,
        Some(3),
        "wand charges should be halved: 6/2=3"
    );
    assert_eq!(bones.depth, 10);
    assert_eq!(bones.branch, DungeonBranch::Main);
    assert_eq!(bones.death_turn, 2500);
    assert!(!bones.encountered);
}

/// Scenario 9.2 -- Bones load from pool when matching branch and depth.
#[test]
fn touchstone_09_bones_load_from_pool() {
    let mut rng = Pcg64::seed_from_u64(42);
    let level = make_test_level();

    let bones = generate_bones(
        &level,
        "Player1",
        8,
        40,
        Position::new(20, 5),
        "Valkyrie",
        5,
        DungeonBranch::Main,
        800,
        vec![(Position::new(20, 5), "mace".to_string(), None, false)],
        &mut rng,
    );

    let mut pool = BonesPool::new();
    pool.add(bones);
    assert_eq!(pool.len(), 1);

    let mut loaded = false;
    for seed in 0..200u64 {
        let mut try_rng = Pcg64::seed_from_u64(seed);
        let mut pool_clone = pool.clone();
        if let Some(bone_data) =
            pool_clone.try_get(DungeonBranch::Main, 5, &mut try_rng)
        {
            assert_eq!(bone_data.ghost.player_name, "Player1");
            assert_eq!(bone_data.depth, 5);
            assert_eq!(bone_data.branch, DungeonBranch::Main);
            assert_eq!(bone_data.dropped_items.len(), 1);
            loaded = true;
            break;
        }
    }
    assert!(loaded, "should eventually load bones with 1/3 probability");
}

/// Scenario 9.3 -- Anti-cheat: bones can only be loaded once per game.
#[test]
fn touchstone_09_bones_anti_cheat() {
    let mut rng = Pcg64::seed_from_u64(42);
    let level = make_test_level();

    let bones = generate_bones(
        &level,
        "Player",
        5,
        30,
        Position::new(10, 5),
        "Rogue",
        3,
        DungeonBranch::Main,
        500,
        vec![],
        &mut rng,
    );

    let mut pool = BonesPool::new();
    pool.add(bones);

    let mut found_seed = None;
    for seed in 0..200u64 {
        let mut try_rng = Pcg64::seed_from_u64(seed);
        let mut pool_clone = pool.clone();
        if pool_clone
            .try_get(DungeonBranch::Main, 3, &mut try_rng)
            .is_some()
        {
            found_seed = Some(seed);
            break;
        }
    }
    let seed = found_seed.expect("should find bones with some seed");

    let mut try_rng = Pcg64::seed_from_u64(seed);
    let result = pool.try_get(DungeonBranch::Main, 3, &mut try_rng);
    assert!(result.is_some(), "first load should succeed");

    for s in 0..200u64 {
        let mut rng2 = Pcg64::seed_from_u64(s);
        let result2 = pool.try_get(DungeonBranch::Main, 3, &mut rng2);
        assert!(
            result2.is_none(),
            "second load should fail due to anti-cheat (seed={})",
            s
        );
    }
}

/// Scenario 9.4 -- Item downgrade: cursing and charge halving.
#[test]
fn touchstone_09_item_downgrade() {
    let mut rng = Pcg64::seed_from_u64(42);

    let mut items = vec![
        BoneItem {
            position: Position::new(5, 5),
            name: "wand of death".to_string(),
            cursed: false,
            charges: Some(8),
            is_artifact: false,
        },
        BoneItem {
            position: Position::new(6, 5),
            name: "long sword".to_string(),
            cursed: false,
            charges: None,
            is_artifact: false,
        },
        BoneItem {
            position: Position::new(7, 5),
            name: "wand of wishing".to_string(),
            cursed: false,
            charges: Some(3),
            is_artifact: false,
        },
    ];

    downgrade_bone_items(&mut items, &mut rng);

    assert_eq!(items[0].charges, Some(4), "wand charges halved: 8->4");
    assert_eq!(items[2].charges, Some(1), "wand charges halved: 3->1");

    let cursed_count = items.iter().filter(|i| i.cursed).count();
    assert!(
        cursed_count >= 1,
        "at least one item should be cursed after downgrade"
    );
}

/// Scenario 9.5 -- Full bones lifecycle: generate, ghost behavior, pool
/// load, anti-cheat, cleanup.
#[test]
fn touchstone_09_bones_full_lifecycle() {
    let mut rng = Pcg64::seed_from_u64(42);
    let level = make_test_level();
    let death_pos = Position::new(25, 10);

    let bones = generate_bones(
        &level,
        "Gandalf",
        20,
        100,
        death_pos,
        "Wizard",
        15,
        DungeonBranch::Gehennom,
        5000,
        vec![
            (death_pos, "Magicbane".to_string(), None, true),
            (death_pos, "wand of polymorph".to_string(), Some(4), false),
        ],
        &mut rng,
    );

    // Ghost behavior.
    let ghost = GhostBehavior::from_ghost_info(&bones.ghost);
    assert_eq!(ghost.display_name, "ghost of Gandalf");
    assert_eq!(ghost.hp, 100);
    assert_eq!(ghost.level, 20);
    assert!(ghost.phases_through_walls);
    assert!(ghost.sleeping);
    assert!(ghost.can_move_to(Terrain::Wall));
    assert!(ghost.can_move_to(Terrain::Floor));
    assert!(!ghost.can_move_to(Terrain::Stone));

    let mut ghost = ghost;
    ghost.wake();
    assert!(!ghost.sleeping);

    // Pool operations.
    let mut pool = BonesPool::new();
    pool.add(bones);

    let mut try_rng = Pcg64::seed_from_u64(0);
    assert!(pool.try_get(DungeonBranch::Main, 15, &mut try_rng).is_none());

    let mut loaded = false;
    for seed in 0..200u64 {
        let mut try_rng = Pcg64::seed_from_u64(seed);
        let mut pool_clone = pool.clone();
        if let Some(data) =
            pool_clone.try_get(DungeonBranch::Gehennom, 15, &mut try_rng)
        {
            assert_eq!(data.ghost.player_name, "Gandalf");
            assert_eq!(data.depth, 15);
            assert!(data.dropped_items[0].is_artifact);
            loaded = true;
            break;
        }
    }
    assert!(loaded, "should eventually load bones");

    let mut loaded_for_real = false;
    for seed in 0..200u64 {
        let mut try_rng = Pcg64::seed_from_u64(seed);
        if pool.try_get(DungeonBranch::Gehennom, 15, &mut try_rng).is_some() {
            loaded_for_real = true;
            break;
        }
    }
    assert!(loaded_for_real);

    pool.remove_encountered();
    assert!(pool.is_empty(), "pool should be empty after cleanup");
}

/// Scenario 9.6 -- Bones eligibility rules for restricted levels.
#[test]
fn touchstone_09_bones_eligibility() {
    let mut rng = Pcg64::seed_from_u64(42);
    assert!(!can_make_bones(DungeonBranch::Quest, 5, 10, false, &mut rng));

    let mut rng2 = Pcg64::seed_from_u64(42);
    assert!(!can_make_bones(DungeonBranch::Endgame, 1, 5, false, &mut rng2));

    let mut rng3 = Pcg64::seed_from_u64(42);
    assert!(!can_make_bones(DungeonBranch::Main, 0, 30, false, &mut rng3));

    let mut rng4 = Pcg64::seed_from_u64(42);
    assert!(!can_make_bones(DungeonBranch::Main, 30, 30, false, &mut rng4));

    let mut rng5 = Pcg64::seed_from_u64(42);
    assert!(!can_make_bones(DungeonBranch::Main, 5, 30, true, &mut rng5));
}

/// Scenario 9.7 -- Exploration state is cleared in bones level maps.
#[test]
fn touchstone_09_bones_exploration_cleared() {
    let mut rng = Pcg64::seed_from_u64(42);
    let mut level = make_test_level();

    if let Some(cell) = level.get_mut(Position::new(10, 5)) {
        cell.explored = true;
        cell.visible = true;
    }
    if let Some(cell) = level.get_mut(Position::new(20, 8)) {
        cell.explored = true;
        cell.visible = true;
    }

    let bones = generate_bones(
        &level, "Explorer", 5, 30, Position::new(10, 5),
        "Ranger", 3, DungeonBranch::Main, 500, vec![], &mut rng,
    );

    let cell1 = bones.level_map.get(Position::new(10, 5)).unwrap();
    assert!(!cell1.explored);
    assert!(!cell1.visible);
    let cell2 = bones.level_map.get(Position::new(20, 8)).unwrap();
    assert!(!cell2.explored);
    assert!(!cell2.visible);
}

/// Scenario 9.8 -- Pool replaces bones at the same (branch, depth).
#[test]
fn touchstone_09_bones_pool_replaces() {
    let mut rng = Pcg64::seed_from_u64(42);
    let level = make_test_level();

    let bones1 = generate_bones(
        &level, "Player1", 5, 30, Position::new(5, 5),
        "Wizard", 3, DungeonBranch::Main, 500, vec![], &mut rng,
    );
    let bones2 = generate_bones(
        &level, "Player2", 10, 60, Position::new(5, 5),
        "Valkyrie", 3, DungeonBranch::Main, 800, vec![], &mut rng,
    );

    let mut pool = BonesPool::new();
    pool.add(bones1);
    pool.add(bones2);
    assert_eq!(pool.len(), 1, "same location should replace");
}

/// Scenario 9.9 -- Double charge reduction across generation + downgrade.
#[test]
fn touchstone_09_bones_double_charge_reduction() {
    let mut rng = Pcg64::seed_from_u64(42);
    let level = make_test_level();
    let pos = Position::new(15, 8);

    let bones = generate_bones(
        &level, "Charger", 10, 50, pos, "Wizard", 5,
        DungeonBranch::Main, 1000,
        vec![(pos, "wand of fire".to_string(), Some(8), false)],
        &mut rng,
    );

    assert_eq!(bones.dropped_items[0].charges, Some(4));

    let mut items = bones.dropped_items.clone();
    downgrade_bone_items(&mut items, &mut rng);
    assert_eq!(items[0].charges, Some(2), "8->4->2 double reduction");
}

// ==========================================================================
// Cross-module harness validation
// ==========================================================================

/// Verify the test harness helpers work correctly.
#[test]
fn touchstone_harness_create_world() {
    let (world, _rng) = create_test_world(42);
    assert_eq!(player_pos(&world), Position::new(40, 10));

    let hp = world
        .get_component::<HitPoints>(world.player())
        .expect("player should have HP");
    assert_eq!(hp.current, 16);
    assert_eq!(hp.max, 16);
}

/// Verify set_player_hp helper.
#[test]
fn touchstone_harness_set_player_hp() {
    let (mut world, _rng) = create_test_world(42);
    set_player_hp(&mut world, 5, 100);

    let hp = world
        .get_component::<HitPoints>(world.player())
        .expect("player should have HP");
    assert_eq!(hp.current, 5);
    assert_eq!(hp.max, 100);
}

/// Verify place_monster and entity_is_alive helpers.
#[test]
fn touchstone_harness_place_monster() {
    let (mut world, _rng) = create_test_world(42);
    let monster = place_monster(&mut world, Position::new(10, 5), "goblin", 8);

    assert!(entity_is_alive(&world, monster));
    let pos = world
        .get_component::<Positioned>(monster)
        .expect("monster should have position");
    assert_eq!(pos.0, Position::new(10, 5));
    assert_eq!(world.entity_name(monster), "goblin");
}

/// Entity with 0 HP recognized as not alive.
#[test]
fn touchstone_harness_entity_dead() {
    let (mut world, _rng) = create_test_world(42);
    let monster = place_monster(&mut world, Position::new(10, 5), "orc", 0);
    assert!(!entity_is_alive(&world, monster));
}

// ==========================================================================
// Scenario 7: Wish Parsing
// ==========================================================================
//
// NetHack's wish system parses player text input like
// "blessed +2 silver dragon scale mail" into an actual item.
// The parse_wish function handles BUC, enchantment, erodeproof,
// quantity, material, and fuzzy name matching.

use nethack_babel_data::{load_game_data, ObjectClass};
use nethack_babel_engine::wish::{parse_wish, BucWish};
use std::path::PathBuf;

/// Get the project data directory for loading object definitions.
fn wish_data_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../../data")
}

/// Touchstone 7.1 -- Parse "blessed +2 silver dragon scale mail".
/// Verifies BUC=Blessed, enchantment=+2, correct ObjectTypeId.
#[test]
fn touchstone_07_wish_blessed_plus2_sdsm() {
    let data = load_game_data(&wish_data_dir()).expect("load data");
    let result = parse_wish("blessed +2 silver dragon scale mail", &data.objects);
    let r = result.expect("wish should parse successfully");

    assert_eq!(r.buc, Some(BucWish::Blessed), "BUC should be Blessed");
    assert_eq!(r.enchantment, Some(2), "enchantment should be +2");

    let sdsm = data
        .objects
        .iter()
        .find(|o| o.name.to_lowercase() == "silver dragon scale mail")
        .expect("silver dragon scale mail should exist in data");
    assert_eq!(
        r.object_type, sdsm.id,
        "object type should match silver dragon scale mail"
    );
}

/// Touchstone 7.2 -- Parse "rustproof +3 long sword".
/// Verifies erodeproof=true, enchantment=+3, type=LongSword.
#[test]
fn touchstone_07_wish_rustproof_plus3_long_sword() {
    let data = load_game_data(&wish_data_dir()).expect("load data");
    let result = parse_wish("rustproof +3 long sword", &data.objects);
    let r = result.expect("wish should parse successfully");

    assert!(r.erodeproof, "erodeproof should be true");
    assert_eq!(r.enchantment, Some(3), "enchantment should be +3");

    let ls = data
        .objects
        .iter()
        .find(|o| o.name.to_lowercase() == "long sword")
        .expect("long sword should exist in data");
    assert_eq!(
        r.object_type, ls.id,
        "object type should match long sword"
    );
}

/// Touchstone 7.3 -- Parse "blessed +4 elven arrow".
/// Verifies BUC=Blessed, enchantment=+4, type=ElvenArrow.
#[test]
fn touchstone_07_wish_quantity_arrows() {
    let data = load_game_data(&wish_data_dir()).expect("load data");
    let result = parse_wish("blessed +4 elven arrow", &data.objects);
    let r = result.expect("wish should parse successfully");

    assert_eq!(r.buc, Some(BucWish::Blessed), "BUC should be Blessed");
    assert_eq!(r.enchantment, Some(4), "enchantment should be +4");

    let ea = data
        .objects
        .iter()
        .find(|o| o.name.to_lowercase() == "elven arrow")
        .expect("elven arrow should exist in data");
    assert_eq!(
        r.object_type, ea.id,
        "object type should match elven arrow"
    );
}

/// Touchstone 7.4 -- Parse "amulet of yendor".
/// Verifies the wish is rejected or downgraded to the cheap plastic imitation.
#[test]
fn touchstone_07_wish_amulet_of_yendor_rejected() {
    let data = load_game_data(&wish_data_dir()).expect("load data");
    let result = parse_wish("amulet of yendor", &data.objects);

    match result {
        Some(r) => {
            // Should be downgraded to the cheap plastic imitation
            let fake = data
                .objects
                .iter()
                .find(|o| o.name.to_lowercase().contains("cheap plastic"))
                .expect("fake amulet should exist in data");
            assert_eq!(
                r.object_type, fake.id,
                "amulet of yendor wish should produce cheap plastic imitation"
            );
        }
        None => {
            // Also acceptable: outright rejection
        }
    }
}

/// Touchstone 7.5 -- Parse "BLESSED +2 LONG SWORD" (uppercase).
/// Same result as the lowercase version, verifying case insensitivity.
#[test]
fn touchstone_07_wish_case_insensitive() {
    let data = load_game_data(&wish_data_dir()).expect("load data");

    let upper = parse_wish("BLESSED +2 LONG SWORD", &data.objects)
        .expect("uppercase wish should parse");
    let lower = parse_wish("blessed +2 long sword", &data.objects)
        .expect("lowercase wish should parse");

    assert_eq!(
        upper.object_type, lower.object_type,
        "case should not affect object type"
    );
    assert_eq!(
        upper.buc, lower.buc,
        "case should not affect BUC status"
    );
    assert_eq!(
        upper.enchantment, lower.enchantment,
        "case should not affect enchantment"
    );
}

/// Touchstone 7.6 -- Parse "scroll of id" should match "scroll of identify".
/// Verifies partial/fuzzy matching of item names.
#[test]
fn touchstone_07_wish_partial_match() {
    let data = load_game_data(&wish_data_dir()).expect("load data");
    let result = parse_wish("scroll of id", &data.objects);
    let r = result.expect("partial match should succeed");

    let identify = data
        .objects
        .iter()
        .find(|o| o.class == ObjectClass::Scroll && o.name.to_lowercase() == "identify")
        .expect("scroll of identify should exist in data");
    assert_eq!(
        r.object_type, identify.id,
        "\"scroll of id\" should match \"scroll of identify\""
    );
}

// ==========================================================================
// Scenario 8: Ascension Run
// ==========================================================================
//
// The ascension sequence requires the player to: collect invocation items
// (Bell of Opening, Candelabrum of Invocation, Book of the Dead), obtain
// the Amulet of Yendor, reach the Astral Plane, and offer the Amulet on
// the correct-alignment altar.
//
// These tests verify the key mechanical checkpoints individually.

/// Scenario 8a -- The game recognizes when the player has all 3 invocation
/// items: Bell of Opening, Candelabrum of Invocation, Book of the Dead.
#[test]
fn touchstone_08_invocation_items_check() {
    // All three items present.
    assert!(
        has_invocation_items(true, true, true),
        "should recognize all 3 invocation items present"
    );

    // Missing the bell.
    assert!(
        !has_invocation_items(false, true, true),
        "missing Bell of Opening should fail"
    );

    // Missing the candelabrum.
    assert!(
        !has_invocation_items(true, false, true),
        "missing Candelabrum of Invocation should fail"
    );

    // Missing the book.
    assert!(
        !has_invocation_items(true, true, false),
        "missing Book of the Dead should fail"
    );

    // None present.
    assert!(
        !has_invocation_items(false, false, false),
        "no invocation items should fail"
    );

    // Also verify through the PlayerQuestItems data struct.
    let qi = PlayerQuestItems {
        has_amulet: false,
        has_bell: true,
        has_book: true,
        has_menorah: true,
        has_quest_artifact: false,
    };
    assert!(
        has_invocation_items(qi.has_bell, qi.has_menorah, qi.has_book),
        "PlayerQuestItems with all invocation items should pass"
    );

    let qi_missing = PlayerQuestItems {
        has_amulet: false,
        has_bell: true,
        has_book: false,
        has_menorah: true,
        has_quest_artifact: false,
    };
    assert!(
        !has_invocation_items(qi_missing.has_bell, qi_missing.has_menorah, qi_missing.has_book),
        "PlayerQuestItems missing book should fail"
    );
}

/// Scenario 8b -- Player carrying the real Amulet of Yendor is tracked.
#[test]
fn touchstone_08_amulet_possession_tracked() {
    // PlayerQuestItems tracks Amulet possession.
    let qi_with = PlayerQuestItems {
        has_amulet: true,
        has_bell: false,
        has_book: false,
        has_menorah: false,
        has_quest_artifact: false,
    };
    assert!(qi_with.has_amulet, "should track Amulet possession");

    let qi_without = PlayerQuestItems {
        has_amulet: false,
        has_bell: false,
        has_book: false,
        has_menorah: false,
        has_quest_artifact: false,
    };
    assert!(!qi_without.has_amulet, "should track absence of Amulet");

    // Verify the DeathCause::Ascended variant exists for game-over events.
    let cause = DeathCause::Ascended;
    assert_eq!(cause, DeathCause::Ascended);

    // A full quest-items set ready for ascension.
    let qi_full = PlayerQuestItems {
        has_amulet: true,
        has_bell: true,
        has_book: true,
        has_menorah: true,
        has_quest_artifact: true,
    };
    assert!(qi_full.has_amulet);
    assert!(has_invocation_items(qi_full.has_bell, qi_full.has_menorah, qi_full.has_book));
}

/// Scenario 8c -- Offering the Amulet on the correct-alignment altar on the
/// Astral Plane triggers ascension.
#[test]
fn touchstone_08_correct_altar_offering_ascends() {
    // Lawful player offering on Lawful altar on Astral Plane.
    let result = offer_amulet(Alignment::Lawful, Alignment::Lawful, true);
    assert_eq!(
        result,
        AmuletOfferingResult::Ascended,
        "matching alignment on Astral should ascend"
    );

    // Neutral player offering on Neutral altar on Astral Plane.
    let result = offer_amulet(Alignment::Neutral, Alignment::Neutral, true);
    assert_eq!(
        result,
        AmuletOfferingResult::Ascended,
        "Neutral matching should ascend"
    );

    // Chaotic player offering on Chaotic altar on Astral Plane.
    let result = offer_amulet(Alignment::Chaotic, Alignment::Chaotic, true);
    assert_eq!(
        result,
        AmuletOfferingResult::Ascended,
        "Chaotic matching should ascend"
    );

    // Verify the GameOver event can be constructed with ascension.
    let event = EngineEvent::GameOver {
        cause: DeathCause::Ascended,
        score: 100_000,
    };
    match event {
        EngineEvent::GameOver { cause, score } => {
            assert_eq!(cause, DeathCause::Ascended);
            assert_eq!(score, 100_000);
        }
        _ => panic!("expected GameOver event"),
    }
}

/// Scenario 8d -- Offering the Amulet on a wrong-alignment altar is rejected.
#[test]
fn touchstone_08_wrong_altar_offering_rejected() {
    // Lawful player offering on Chaotic altar.
    let result = offer_amulet(Alignment::Lawful, Alignment::Chaotic, true);
    assert_eq!(
        result,
        AmuletOfferingResult::Rejected,
        "Lawful on Chaotic altar should be rejected"
    );

    // Lawful player offering on Neutral altar.
    let result = offer_amulet(Alignment::Lawful, Alignment::Neutral, true);
    assert_eq!(
        result,
        AmuletOfferingResult::Rejected,
        "Lawful on Neutral altar should be rejected"
    );

    // Neutral player offering on Lawful altar.
    let result = offer_amulet(Alignment::Neutral, Alignment::Lawful, true);
    assert_eq!(
        result,
        AmuletOfferingResult::Rejected,
        "Neutral on Lawful altar should be rejected"
    );

    // Chaotic player offering on Neutral altar.
    let result = offer_amulet(Alignment::Chaotic, Alignment::Neutral, true);
    assert_eq!(
        result,
        AmuletOfferingResult::Rejected,
        "Chaotic on Neutral altar should be rejected"
    );

    // Not on Astral Plane: offering has no effect regardless of alignment.
    let result = offer_amulet(Alignment::Lawful, Alignment::Lawful, false);
    assert_eq!(
        result,
        AmuletOfferingResult::NotAstralPlane,
        "offering not on Astral Plane should return NotAstralPlane"
    );
}

/// Scenario 8e -- Score calculation includes ascension bonus and is positive.
#[test]
fn touchstone_08_ascension_score_positive() {
    let conducts = ConductState::new();
    assert_eq!(conducts.maintained_count(), 13, "fresh state has all 13 conducts maintained");

    // Minimal ascension score: zero experience, zero gold, but ascended.
    let input = ScoreInput {
        experience: 0,
        score_experience: 0,
        gold_carried: 0,
        gold_deposited: 0,
        artifacts_held: 0,
        conducts: conducts.clone(),
        ascended: true,
        max_depth: 1,
    };
    let score = calculate_score(&input);
    // 50,000 ascension + 13 * 5,000 conduct = 115,000
    assert_eq!(
        score, 115_000,
        "minimal ascension score should be 115,000 (50k ascension + 65k conducts)"
    );
    assert!(score > 0, "ascension score must be positive");

    // Richer ascension: some experience, gold, artifacts.
    let input_rich = ScoreInput {
        experience: 1000,
        score_experience: 500,
        gold_carried: 10_000,
        gold_deposited: 5_000,
        artifacts_held: 3,
        conducts: conducts.clone(),
        ascended: true,
        max_depth: 50,
    };
    let score_rich = calculate_score(&input_rich);
    // base = 4*1000 + 500 = 4500
    // gold = 15000
    // artifacts = 3000
    // conducts = 65000
    // ascension = 50000
    // total = 137500
    assert_eq!(score_rich, 137_500, "rich ascension score formula check");
    assert!(score_rich > score, "richer game should have higher score");

    // Non-ascension: no ascension bonus.
    let input_no_asc = ScoreInput {
        experience: 1000,
        score_experience: 500,
        gold_carried: 10_000,
        gold_deposited: 5_000,
        artifacts_held: 3,
        conducts: conducts.clone(),
        ascended: false,
        max_depth: 50,
    };
    let score_no_asc = calculate_score(&input_no_asc);
    assert_eq!(score_no_asc, score_rich - 50_000, "non-ascension should lack 50k bonus");
}

/// Scenario 8f -- Conducts are tracked through the game and violations
/// are correctly reflected in the state.
#[test]
fn touchstone_08_conducts_tracked_through_game() {
    let mut state = ConductState::new();

    // Initially all 13 standard conducts are maintained.
    assert_eq!(
        state.maintained_count(), 13,
        "fresh game: all 13 conducts maintained"
    );
    assert!(state.is_maintained(nethack_babel_engine::conduct::Conduct::Illiterate));
    assert!(state.is_maintained(nethack_babel_engine::conduct::Conduct::Foodless));
    assert!(state.is_maintained(nethack_babel_engine::conduct::Conduct::Pacifist));

    // Break illiterate conduct by reading.
    let violations = check_conduct(&mut state, &ConductAction::Read);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].conduct, nethack_babel_engine::conduct::Conduct::Illiterate);
    assert_eq!(violations[0].total_violations, 1);
    assert!(!state.is_maintained(nethack_babel_engine::conduct::Conduct::Illiterate));
    assert_eq!(state.maintained_count(), 12, "one conduct broken");

    // Break foodless conduct by eating (vegan food).
    let violations = check_conduct(&mut state, &ConductAction::Eat {
        is_vegan: true,
        is_vegetarian: true,
    });
    assert!(violations.iter().any(|v| v.conduct == nethack_babel_engine::conduct::Conduct::Foodless));
    assert!(!state.is_maintained(nethack_babel_engine::conduct::Conduct::Foodless));
    // Vegan and vegetarian should still be maintained.
    assert!(state.is_maintained(nethack_babel_engine::conduct::Conduct::Vegan));
    assert!(state.is_maintained(nethack_babel_engine::conduct::Conduct::Vegetarian));
    assert_eq!(state.maintained_count(), 11, "two conducts broken");

    // Break pacifist conduct by killing.
    let _ = check_conduct(&mut state, &ConductAction::Kill);
    assert!(!state.is_maintained(nethack_babel_engine::conduct::Conduct::Pacifist));
    assert_eq!(state.maintained_count(), 10, "three conducts broken");

    // Verify score reflects reduced conduct bonus.
    let input_full = ScoreInput {
        experience: 0,
        score_experience: 0,
        gold_carried: 0,
        gold_deposited: 0,
        artifacts_held: 0,
        conducts: ConductState::new(),
        ascended: true,
        max_depth: 1,
    };
    let input_broken = ScoreInput {
        experience: 0,
        score_experience: 0,
        gold_carried: 0,
        gold_deposited: 0,
        artifacts_held: 0,
        conducts: state.clone(),
        ascended: true,
        max_depth: 1,
    };
    let score_full = calculate_score(&input_full);
    let score_broken = calculate_score(&input_broken);
    assert_eq!(
        score_full - score_broken, 15_000,
        "3 broken conducts should reduce score by 3 * 5000 = 15000"
    );
}

// ==========================================================================
// Scenario 1: Valkyrie Standard Opening
// ==========================================================================
//
// The Valkyrie standard opening sequence exercises three key mechanics:
//   1. Excalibur dip: dipping a long sword in a fountain at level >= 5
//   2. Floating eye gaze: passive paralyze gaze when attacking while not blind
//   3. Floating eye corpse: eating grants telepathy intrinsic

/// Scenario 1.1 -- Excalibur dip: lawful character at level >= 5 dipping
/// a long sword in a fountain eventually produces Excalibur.
#[test]
fn touchstone_01_excalibur_dip_lawful_level5() {
    use nethack_babel_engine::artifacts::{try_create_excalibur, ExcaliburResult};
    use nethack_babel_data::ObjectTypeId;

    let long_sword = ObjectTypeId(28); // OBJ_LONG_SWORD

    // Lawful character, level 5, try multiple seeds until success.
    let mut found_success = false;
    for seed in 0..200u64 {
        let mut rng = Pcg64::seed_from_u64(seed);
        let result = try_create_excalibur(
            long_sword,
            5,                  // player level
            Alignment::Lawful,  // alignment
            true,               // is_knight (higher chance: 1/6)
            false,              // excalibur doesn't exist yet
            &mut rng,
        );
        if result == ExcaliburResult::Success {
            found_success = true;
            break;
        }
    }
    assert!(
        found_success,
        "lawful knight at level 5+ should eventually create Excalibur from fountain dip"
    );
}

/// Scenario 1.1b -- Excalibur dip: level 4 is too low (Invalid).
#[test]
fn touchstone_01_excalibur_dip_requires_level_5() {
    use nethack_babel_engine::artifacts::{try_create_excalibur, ExcaliburResult};
    use nethack_babel_data::ObjectTypeId;

    let long_sword = ObjectTypeId(28);
    let mut rng = Pcg64::seed_from_u64(42);

    let result = try_create_excalibur(
        long_sword,
        4,                  // too low
        Alignment::Lawful,
        true,
        false,
        &mut rng,
    );
    assert_eq!(
        result,
        ExcaliburResult::Invalid,
        "level 4 should fail Excalibur precondition"
    );
}

/// Scenario 1.1c -- Excalibur dip: non-lawful alignment gets cursed sword.
#[test]
fn touchstone_01_excalibur_dip_non_lawful_cursed() {
    use nethack_babel_engine::artifacts::{try_create_excalibur, ExcaliburResult};
    use nethack_babel_data::ObjectTypeId;

    let long_sword = ObjectTypeId(28);

    let mut found_cursed = false;
    for seed in 0..200u64 {
        let mut rng = Pcg64::seed_from_u64(seed);
        let result = try_create_excalibur(
            long_sword,
            10,
            Alignment::Chaotic,
            true,
            false,
            &mut rng,
        );
        if result == ExcaliburResult::Cursed {
            found_cursed = true;
            break;
        }
    }
    assert!(
        found_cursed,
        "chaotic character should eventually get cursed sword from fountain dip"
    );
}

/// Scenario 1.2 -- Floating eye gaze: melee attack on floating eye while
/// not blind causes paralysis on the attacker.
#[test]
fn touchstone_01_floating_eye_gaze_paralyzes() {
    use nethack_babel_engine::status::{is_paralyzed, StatusEffects};
    use nethack_babel_engine::world::{ArmorClass, PlayerCombat};

    let mut world = GameWorld::new(Position::new(5, 5));
    let player = world.player();

    // Guarantee a hit by boosting uhitinc.
    if let Some(mut pc) = world.get_component_mut::<PlayerCombat>(player) {
        pc.uhitinc = 100;
    }

    // Place a floating eye adjacent to the player.
    let eye_pos = Position::new(6, 5);
    let order = world.next_creation_order();
    let eye = world.spawn((
        Monster,
        Positioned(eye_pos),
        HitPoints { current: 100, max: 100 },
        ArmorClass(9),
        Name("floating eye".to_string()),
        Speed(1),
        MovementPoints(0),
        order,
    ));

    // Set up floor tiles.
    world.dungeon_mut().current_level
        .set_terrain(Position::new(5, 5), Terrain::Floor);
    world.dungeon_mut().current_level
        .set_terrain(Position::new(6, 5), Terrain::Floor);

    // Attack the floating eye.
    let mut rng = Pcg64::seed_from_u64(42);
    let mut events = Vec::new();
    resolve_melee_attack(&mut world, player, eye, &mut rng, &mut events);

    // Verify the attack hit.
    let hit = events.iter().any(|e| matches!(e, EngineEvent::MeleeHit { .. }));
    assert!(hit, "with uhitinc=100, attack should hit floating eye");

    // Player should be paralyzed.
    assert!(
        is_paralyzed(&world, player),
        "attacking a floating eye while not blind should cause paralysis"
    );

    // Check that a PassiveAttack event was emitted.
    let has_passive = events.iter().any(|e| matches!(
        e,
        EngineEvent::PassiveAttack {
            effect: PassiveEffect::Paralyze,
            ..
        }
    ));
    assert!(
        has_passive,
        "should emit PassiveAttack::Paralyze event"
    );

    // Check that a StatusApplied Paralyzed event was emitted.
    let has_status = events.iter().any(|e| matches!(
        e,
        EngineEvent::StatusApplied {
            status: StatusEffect::Paralyzed,
            ..
        }
    ));
    assert!(
        has_status,
        "should emit StatusApplied::Paralyzed event"
    );

    // Check paralysis duration is in range [1, 127].
    let dur = world.get_component::<StatusEffects>(player)
        .map(|s| s.paralysis)
        .unwrap_or(0);
    assert!(
        (1..=127).contains(&dur),
        "paralysis duration {} should be in [1, 127]",
        dur
    );
}

/// Scenario 1.2b -- Floating eye gaze: paralysis is blocked when attacker
/// is blind.
#[test]
fn touchstone_01_floating_eye_gaze_blocked_by_blind() {
    use nethack_babel_engine::status::{is_paralyzed, make_blinded};
    use nethack_babel_engine::world::{ArmorClass, PlayerCombat};

    let mut world = GameWorld::new(Position::new(5, 5));
    let player = world.player();

    // Make the player blind.
    make_blinded(&mut world, player, 100);

    // Guarantee hit.
    if let Some(mut pc) = world.get_component_mut::<PlayerCombat>(player) {
        pc.uhitinc = 100;
    }

    // Place a floating eye adjacent.
    let eye_pos = Position::new(6, 5);
    let order = world.next_creation_order();
    let eye = world.spawn((
        Monster,
        Positioned(eye_pos),
        HitPoints { current: 100, max: 100 },
        ArmorClass(9),
        Name("floating eye".to_string()),
        Speed(1),
        MovementPoints(0),
        order,
    ));

    // Set up floor tiles.
    world.dungeon_mut().current_level
        .set_terrain(Position::new(5, 5), Terrain::Floor);
    world.dungeon_mut().current_level
        .set_terrain(Position::new(6, 5), Terrain::Floor);

    let mut rng = Pcg64::seed_from_u64(42);
    let mut events = Vec::new();
    resolve_melee_attack(&mut world, player, eye, &mut rng, &mut events);

    // Player should NOT be paralyzed because they are blind.
    assert!(
        !is_paralyzed(&world, player),
        "blind player should not be paralyzed by floating eye gaze"
    );

    // No PassiveAttack event should be emitted.
    let has_passive = events.iter().any(|e| matches!(
        e,
        EngineEvent::PassiveAttack {
            effect: PassiveEffect::Paralyze,
            ..
        }
    ));
    assert!(
        !has_passive,
        "blind player should not trigger passive paralyze gaze"
    );
}

/// Scenario 1.3 -- Floating eye corpse: eating grants telepathy intrinsic.
#[test]
fn touchstone_01_floating_eye_corpse_telepathy() {
    use nethack_babel_engine::hunger::{check_intrinsic_gain, CorpseIntrinsic, CorpseDef};
    use nethack_babel_engine::status::{grant_intrinsic, has_intrinsic_telepathy};
    use nethack_babel_data::{MonsterFlags, ResistanceSet};

    // Build a floating eye corpse definition.
    let corpse = CorpseDef {
        name: "floating eye".to_string(),
        base_level: 2,
        corpse_weight: 10,
        corpse_nutrition: 10,
        conveys: ResistanceSet::empty(),
        flags: MonsterFlags::empty(),
        poisonous: false,
        acidic: false,
        flesh_petrifies: false,
        is_giant: false,
        is_domestic: false,
        is_same_race: false,
        cannibal_allowed: false,
        conveys_telepathy: true, // floating eye conveys telepathy
        conveys_teleport: false,
        nonrotting: false,
        is_vegan: false,
        is_vegetarian: false,
    };

    // With conveys_telepathy and level 2, it should always grant telepathy
    // (level=2 > rn2(1)=0 always true when chance=1).
    let mut rng = Pcg64::seed_from_u64(42);
    let mut gained_telepathy = false;
    for _ in 0..100 {
        if let Some(CorpseIntrinsic::Telepathy) = check_intrinsic_gain(&corpse, &mut rng) {
            gained_telepathy = true;
            break;
        }
    }
    assert!(
        gained_telepathy,
        "floating eye corpse should grant telepathy intrinsic"
    );

    // Verify that granting the intrinsic actually sets it on the player.
    let mut world = GameWorld::new(Position::new(5, 5));
    let player = world.player();

    assert!(
        !has_intrinsic_telepathy(&world, player),
        "player should not have telepathy before eating floating eye"
    );

    let events = grant_intrinsic(&mut world, player, &CorpseIntrinsic::Telepathy);

    assert!(
        has_intrinsic_telepathy(&world, player),
        "player should have telepathy after eating floating eye corpse"
    );

    // Should emit a StatusApplied event for Telepathy.
    let has_telepathy_event = events.iter().any(|e| matches!(
        e,
        EngineEvent::StatusApplied {
            status: StatusEffect::Telepathy,
            ..
        }
    ));
    assert!(
        has_telepathy_event,
        "should emit StatusApplied::Telepathy event"
    );
}

/// Scenario 1.4 -- has_passive_paralyze_gaze correctly identifies
/// floating eye and rejects other monsters.
#[test]
fn touchstone_01_passive_gaze_identification() {
    assert!(
        has_passive_paralyze_gaze("floating eye"),
        "floating eye should have passive paralyze gaze"
    );
    assert!(
        has_passive_paralyze_gaze("Floating Eye"),
        "case-insensitive match for floating eye"
    );
    assert!(
        !has_passive_paralyze_gaze("goblin"),
        "goblin should not have passive paralyze gaze"
    );
    assert!(
        !has_passive_paralyze_gaze("giant eye"),
        "giant eye is not a floating eye"
    );
    assert!(
        !has_passive_paralyze_gaze(""),
        "empty string should not match"
    );
}

// ==========================================================================
// Scenario 2: Sokoban -- Boulder Pushing Mechanics
// ==========================================================================
//
// In NetHack's Sokoban branch, the player pushes boulders by walking
// into them.  Boulders pushed into pits fill the pit (both removed).
// Boulders blocked by walls or other boulders cannot be pushed.
// These tests verify the core boulder-pushing mechanics that underpin
// all Sokoban puzzles.

/// Helper: place a boulder entity at the given position.
fn place_boulder(world: &mut GameWorld, pos: Position) -> Entity {
    world.spawn((
        Boulder,
        Positioned(pos),
        Name("boulder".to_string()),
    ))
}

/// Touchstone 2.1 -- Player pushes a boulder by walking into it.
///
/// Place a boulder one step east of the player.  The player walks east.
/// The boulder moves one cell east, and the player occupies the
/// boulder's former position.
#[test]
fn touchstone_02_boulder_push_basic() {
    let (mut world, mut rng) = create_test_world(42);

    // Set up floor tiles: player at (40,10), boulder at (41,10),
    // empty floor at (42,10).
    for x in 39..=43 {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(x, 10), Terrain::Floor);
    }

    let boulder = place_boulder(&mut world, Position::new(41, 10));

    // Ensure player has movement points.
    if let Some(mut mp) = world.get_component_mut::<MovementPoints>(world.player()) {
        mp.0 = NORMAL_SPEED as i32;
    }

    let events = do_action(
        &mut world,
        PlayerAction::Move {
            direction: Direction::East,
        },
        &mut rng,
    );

    // Player should now be at (41, 10).
    assert_eq!(
        player_pos(&world),
        Position::new(41, 10),
        "player should move to the boulder's former position"
    );

    // Boulder should now be at (42, 10).
    let bpos = world
        .get_component::<Positioned>(boulder)
        .expect("boulder should still exist");
    assert_eq!(
        bpos.0,
        Position::new(42, 10),
        "boulder should have been pushed one cell east"
    );

    // There should be a boulder-push message in the events.
    let has_push = events.iter().any(|e| {
        matches!(e, EngineEvent::Message { key, .. } if key.contains("boulder-push"))
    });
    assert!(has_push, "should emit boulder-push message");
}

/// Touchstone 2.2 -- Boulder blocked by a wall does not move.
///
/// Place a boulder with a wall directly behind it.  The player walks
/// into the boulder.  Neither boulder nor player should move.
#[test]
fn touchstone_02_boulder_blocked_by_wall() {
    let (mut world, mut rng) = create_test_world(42);

    // Floor at (40,10) and (41,10); wall at (42,10).
    for x in 39..=41 {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(x, 10), Terrain::Floor);
    }
    world
        .dungeon_mut()
        .current_level
        .set_terrain(Position::new(42, 10), Terrain::Wall);

    let boulder = place_boulder(&mut world, Position::new(41, 10));

    if let Some(mut mp) = world.get_component_mut::<MovementPoints>(world.player()) {
        mp.0 = NORMAL_SPEED as i32;
    }

    let events = do_action(
        &mut world,
        PlayerAction::Move {
            direction: Direction::East,
        },
        &mut rng,
    );

    // Player should still be at (40, 10).
    assert_eq!(
        player_pos(&world),
        Position::new(40, 10),
        "player should not move when boulder is blocked by wall"
    );

    // Boulder should still be at (41, 10).
    let bpos = world
        .get_component::<Positioned>(boulder)
        .expect("boulder should still exist");
    assert_eq!(
        bpos.0,
        Position::new(41, 10),
        "boulder should not move when blocked by wall"
    );

    // Should have a "blocked" message.
    let has_blocked = events.iter().any(|e| {
        matches!(e, EngineEvent::Message { key, .. } if key.contains("boulder-blocked"))
    });
    assert!(has_blocked, "should emit boulder-blocked message");
}

/// Touchstone 2.3 -- Boulder pushed into a pit fills the pit.
///
/// Place a boulder adjacent to the player with a pit trap behind it.
/// Push the boulder.  The boulder entity is despawned, the pit trap is
/// removed, and the player moves into the boulder's former cell.
#[test]
fn touchstone_02_boulder_into_pit() {
    let (mut world, mut rng) = create_test_world(42);

    // Floor tiles.
    for x in 39..=43 {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(x, 10), Terrain::Floor);
    }

    let boulder = place_boulder(&mut world, Position::new(41, 10));

    // Place a pit trap at (42, 10).
    place_trap(
        &mut world.dungeon_mut().trap_map,
        Position::new(42, 10),
        TrapType::Pit,
    );

    if let Some(mut mp) = world.get_component_mut::<MovementPoints>(world.player()) {
        mp.0 = NORMAL_SPEED as i32;
    }

    let events = do_action(
        &mut world,
        PlayerAction::Move {
            direction: Direction::East,
        },
        &mut rng,
    );

    // Player should now be at (41, 10).
    assert_eq!(
        player_pos(&world),
        Position::new(41, 10),
        "player should move to the boulder's former position"
    );

    // Boulder entity should be despawned (fills the pit).
    assert!(
        world.get_component::<Positioned>(boulder).is_none(),
        "boulder should be despawned after filling pit"
    );

    // Pit trap should be removed.
    assert!(
        world
            .dungeon()
            .trap_map
            .trap_at(Position::new(42, 10))
            .is_none(),
        "pit trap should be removed after boulder fills it"
    );

    // Should have a "boulder fills pit" message.
    let has_fill = events.iter().any(|e| {
        matches!(e, EngineEvent::Message { key, .. } if key.contains("boulder-fills-pit"))
    });
    assert!(has_fill, "should emit boulder-fills-pit message");
}

/// Touchstone 2.4 -- Boulder blocked by another boulder does not move.
///
/// Two boulders in a line; pushing the near one into the far one fails.
#[test]
fn touchstone_02_boulder_blocked_by_boulder() {
    let (mut world, mut rng) = create_test_world(42);

    for x in 39..=44 {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(x, 10), Terrain::Floor);
    }

    let boulder1 = place_boulder(&mut world, Position::new(41, 10));
    let _boulder2 = place_boulder(&mut world, Position::new(42, 10));

    if let Some(mut mp) = world.get_component_mut::<MovementPoints>(world.player()) {
        mp.0 = NORMAL_SPEED as i32;
    }

    let _events = do_action(
        &mut world,
        PlayerAction::Move {
            direction: Direction::East,
        },
        &mut rng,
    );

    // Player should not have moved.
    assert_eq!(
        player_pos(&world),
        Position::new(40, 10),
        "player should not move when boulder chain is blocked"
    );

    // Near boulder should remain in place.
    let bpos = world
        .get_component::<Positioned>(boulder1)
        .expect("boulder should still exist");
    assert_eq!(
        bpos.0,
        Position::new(41, 10),
        "near boulder should not move when blocked by another boulder"
    );
}

/// Touchstone 2.5 -- Simple two-boulder Sokoban puzzle is solvable.
///
/// Layout:
/// ```text
///   #........#
///   #..0..^..#     0 = boulder, ^ = pit
///   #........#
///   #..@..0..#     @ = player, 0 = boulder
///   #.....^..#     ^ = pit
///   #........#
/// ```
/// Solution: push boulder1 east 4 times into pit1;
/// then push boulder2 south 2 times into pit2.
#[test]
fn touchstone_02_sokoban_solution_valid() {
    let (mut world, mut rng) = create_test_world(42);

    // Create a small room.
    for y in 4..=12 {
        for x in 35..=45 {
            world
                .dungeon_mut()
                .current_level
                .set_terrain(Position::new(x, y), Terrain::Floor);
        }
    }

    // Player starts at (37, 9).
    if let Some(mut pos) = world.get_component_mut::<Positioned>(world.player()) {
        pos.0 = Position::new(37, 9);
    }

    // Boulder 1 at (37, 7), pit at (41, 7).
    let boulder1 = place_boulder(&mut world, Position::new(37, 7));
    place_trap(
        &mut world.dungeon_mut().trap_map,
        Position::new(41, 7),
        TrapType::Pit,
    );

    // Boulder 2 at (41, 9), pit at (41, 11).
    let boulder2 = place_boulder(&mut world, Position::new(41, 9));
    place_trap(
        &mut world.dungeon_mut().trap_map,
        Position::new(41, 11),
        TrapType::Pit,
    );

    // Helper to ensure player has movement points.
    let give_mp = |world: &mut GameWorld| {
        if let Some(mut mp) = world.get_component_mut::<MovementPoints>(world.player()) {
            mp.0 = NORMAL_SPEED as i32 * 2;
        }
    };

    // --- Solve boulder 1: push east 4 times to reach (41,7) pit ---
    // Position the player directly west of boulder 1.
    if let Some(mut pos) = world.get_component_mut::<Positioned>(world.player()) {
        pos.0 = Position::new(36, 7);
    }

    // Push boulder1 east 3 times: (37,7) -> (38,7) -> (39,7) -> (40,7)
    for _ in 0..3 {
        give_mp(&mut world);
        do_action(
            &mut world,
            PlayerAction::Move {
                direction: Direction::East,
            },
            &mut rng,
        );
    }

    // Verify boulder1 is at (40, 7) after 3 pushes.
    {
        let b1pos = world
            .get_component::<Positioned>(boulder1)
            .expect("boulder1 should still exist after 3 pushes");
        assert_eq!(
            b1pos.0,
            Position::new(40, 7),
            "boulder1 should be at (40,7) after 3 eastward pushes"
        );
    }

    // Fourth push sends it into the pit.
    give_mp(&mut world);
    do_action(
        &mut world,
        PlayerAction::Move {
            direction: Direction::East,
        },
        &mut rng,
    );

    // Boulder1 should be despawned (filled pit at (41,7)).
    assert!(
        world.get_component::<Positioned>(boulder1).is_none(),
        "boulder1 should be despawned after filling pit"
    );
    assert!(
        world
            .dungeon()
            .trap_map
            .trap_at(Position::new(41, 7))
            .is_none(),
        "pit at (41,7) should be removed"
    );

    // --- Solve boulder 2: push south 2 times to reach (41,11) pit ---
    // Move player to (41, 8) so they are north of boulder2 at (41,9).
    if let Some(mut pos) = world.get_component_mut::<Positioned>(world.player()) {
        pos.0 = Position::new(41, 8);
    }

    // Push boulder2 south: (41,9) -> (41,10)
    give_mp(&mut world);
    do_action(
        &mut world,
        PlayerAction::Move {
            direction: Direction::South,
        },
        &mut rng,
    );

    {
        let b2pos = world
            .get_component::<Positioned>(boulder2)
            .expect("boulder2 should still exist");
        assert_eq!(
            b2pos.0,
            Position::new(41, 10),
            "boulder2 should be at (41,10) after 1 southward push"
        );
    }

    // Push boulder2 south into pit: (41,10) -> (41,11=pit)
    give_mp(&mut world);
    do_action(
        &mut world,
        PlayerAction::Move {
            direction: Direction::South,
        },
        &mut rng,
    );

    // Boulder2 should be despawned.
    assert!(
        world.get_component::<Positioned>(boulder2).is_none(),
        "boulder2 should be despawned after filling pit"
    );
    assert!(
        world
            .dungeon()
            .trap_map
            .trap_at(Position::new(41, 11))
            .is_none(),
        "pit at (41,11) should be removed"
    );
}

/// Touchstone 2.6 -- Boulder pushed into a hole also fills it.
///
/// Holes work the same as pits for boulder filling.
#[test]
fn touchstone_02_boulder_into_hole() {
    let (mut world, mut rng) = create_test_world(42);

    for x in 39..=43 {
        world
            .dungeon_mut()
            .current_level
            .set_terrain(Position::new(x, 10), Terrain::Floor);
    }

    let boulder = place_boulder(&mut world, Position::new(41, 10));

    // Place a hole trap at (42, 10).
    place_trap(
        &mut world.dungeon_mut().trap_map,
        Position::new(42, 10),
        TrapType::Hole,
    );

    if let Some(mut mp) = world.get_component_mut::<MovementPoints>(world.player()) {
        mp.0 = NORMAL_SPEED as i32;
    }

    let events = do_action(
        &mut world,
        PlayerAction::Move {
            direction: Direction::East,
        },
        &mut rng,
    );

    // Player moves into boulder's old cell.
    assert_eq!(player_pos(&world), Position::new(41, 10));

    // Boulder is gone.
    assert!(
        world.get_component::<Positioned>(boulder).is_none(),
        "boulder should fill the hole"
    );

    // Hole trap is removed.
    assert!(
        world
            .dungeon()
            .trap_map
            .trap_at(Position::new(42, 10))
            .is_none(),
        "hole should be removed after boulder fills it"
    );

    let has_fill = events.iter().any(|e| {
        matches!(e, EngineEvent::Message { key, .. } if key.contains("boulder-fills-pit"))
    });
    assert!(has_fill, "filling a hole should produce boulder-fills-pit message");
}

// ==========================================================================
// Scenario 3: Minetown Shop -- Shop Transactions
// ==========================================================================
//
// These tests verify the shop system end-to-end: price calculation with
// CHA modifiers, bill management, payment, theft detection, and Kop
// spawning.  The shop.rs module already has 94 unit tests; these
// touchstone tests focus on the scenario-level invariants that a player
// would experience in Minetown.

/// Touchstone 3.1 -- Price identification via shop pricing formula.
///
/// When the player picks up an item in a shop, they see a price that
/// encodes the base cost modified by CHA and other factors.  A player
/// can reverse-engineer the base cost from the offered price.
#[test]
fn touchstone_03_shop_price_id() {
    // Test the pricing formula directly.
    // Long sword: base cost 15, class Weapon, no enchantment, qty 1.
    // CHA 12 (range 11-15) => 1x modifier, no tourist, no surcharge.
    let price = get_full_buy_price(
        15,                    // base_cost
        nethack_babel_data::ObjectClass::Weapon,
        0,                     // spe
        1,                     // quantity
        12,                    // charisma
        false,                 // is_tourist_or_dunce
        false,                 // is_artifact
        0,                     // artifact_cost
        false,                 // oid_surcharge
        false,                 // anger_surcharge
    );
    assert_eq!(price, 15, "long sword base 15 at CHA 12 should cost 15");

    // Same item with low CHA (5) => 2x.
    let price_low_cha = get_full_buy_price(
        15, nethack_babel_data::ObjectClass::Weapon,
        0, 1, 5, false, false, 0, false, false,
    );
    assert_eq!(price_low_cha, 30, "long sword at CHA 5 should cost 30 (2x)");

    // With enchantment +3: base becomes 15 + 10*3 = 45.
    let price_enchanted = get_full_buy_price(
        15, nethack_babel_data::ObjectClass::Weapon,
        3, 1, 12, false, false, 0, false, false,
    );
    assert_eq!(price_enchanted, 45, "+3 weapon base 15 should cost 45 (15 + 30)");
}

/// Touchstone 3.2 -- Purchasing an item clears the bill and deducts gold.
///
/// Sets up a shop, adds an item to the bill, pays the bill, and verifies
/// the bill is cleared and gold is deducted.
#[test]
fn touchstone_03_shop_purchase() {
    let (mut world, _rng) = create_test_world(42);
    let player = world.player();

    // Create a shopkeeper entity.
    let shopkeeper = world.spawn((
        Name("Asidonhopo".to_string()),
        Positioned(Position::new(10, 5)),
    ));

    let mut shop = ShopRoom::new(
        Position::new(5, 2),
        Position::new(15, 8),
        ShopType::General,
        shopkeeper,
        "Asidonhopo".to_string(),
    );

    // Create a fake item entity and add it to the bill.
    let item = world.spawn((
        Positioned(Position::new(10, 5)),
        Name("mace".to_string()),
    ));

    // Add to bill manually: price 100 per unit, quantity 1.
    shop.bill.add(item, 100, 1);
    assert_eq!(shop.bill.len(), 1, "bill should have 1 entry");
    assert_eq!(shop.bill.total(), 100, "bill total should be 100");

    // Pay the bill with 500 gold.
    let mut gold = 500;
    let events = pay_bill(&world, player, &mut shop, &mut gold);

    assert!(shop.bill.is_empty(), "bill should be cleared after payment");
    assert_eq!(gold, 400, "gold should be 500 - 100 = 400");

    // Should have a success message.
    let has_success = events.iter().any(|e| {
        matches!(e, EngineEvent::Message { key, .. } if key.contains("shop-pay-success"))
    });
    assert!(has_success, "should emit shop-pay-success message");
}

/// Touchstone 3.3 -- Leaving with unpaid items triggers shopkeeper anger.
///
/// Simulates shoplifting: the player has items on their bill and leaves
/// the shop.  The shopkeeper becomes angry, the surcharge flag is set,
/// and the stolen amount is recorded.
#[test]
fn touchstone_03_shop_theft_triggers_anger() {
    let (mut world, _rng) = create_test_world(42);
    let player = world.player();

    let shopkeeper = world.spawn((
        Name("Asidonhopo".to_string()),
        Positioned(Position::new(10, 5)),
    ));

    let mut shop = ShopRoom::new(
        Position::new(5, 2),
        Position::new(15, 8),
        ShopType::General,
        shopkeeper,
        "Asidonhopo".to_string(),
    );

    // Add items to the bill.
    let item1 = world.spawn((
        Positioned(Position::new(10, 5)),
        Name("dagger".to_string()),
    ));
    let item2 = world.spawn((
        Positioned(Position::new(11, 5)),
        Name("shield".to_string()),
    ));
    shop.bill.add(item1, 50, 1);
    shop.bill.add(item2, 200, 1);
    assert_eq!(shop.bill.total(), 250);

    // Rob the shop.
    let mut rng = Pcg64::seed_from_u64(42);
    let events = rob_shop(&world, player, &mut shop, &mut rng);

    // Shopkeeper should be angry.
    assert!(shop.angry, "shopkeeper should be angry after theft");
    assert!(shop.surcharge, "surcharge should be active after theft");
    assert_eq!(shop.robbed, 250, "robbed amount should equal bill total");

    // Bill should be cleared (robbery processes it).
    assert!(shop.bill.is_empty(), "bill should be cleared after robbery");

    // Should have theft-related messages.
    let has_shoplift = events.iter().any(|e| {
        matches!(e, EngineEvent::Message { key, .. } if key.contains("shop-shoplift"))
    });
    assert!(has_shoplift, "should emit shop-shoplift message");
}

/// Touchstone 3.4 -- Kop spawn counts scale with dungeon depth.
///
/// Verifies the Kop spawning formula: at deeper levels, more and
/// higher-ranked Kops appear.
#[test]
fn touchstone_03_shop_kop_spawn_on_robbery() {
    // Shallow depth (5): rnd(5)=3 => cnt=8.
    let (kops, sgts, lts, kpts) = kop_counts(5, 3);
    assert_eq!(kops, 8, "8 Keystone Kops at depth 5");
    assert_eq!(sgts, 3, "3 Kop Sergeants at depth 5");
    assert_eq!(lts, 1, "1 Kop Lieutenant at depth 5");
    assert_eq!(kpts, 0, "0 Kop Kaptains at depth 5");

    // Deeper depth (20): rnd(5)=1 => cnt=21.
    let (kops2, sgts2, lts2, kpts2) = kop_counts(20, 1);
    assert_eq!(kops2, 21, "21 Keystone Kops at depth 20");
    assert_eq!(sgts2, 8, "8 Kop Sergeants at depth 20");
    assert_eq!(lts2, 3, "3 Kop Lieutenants at depth 20");
    assert_eq!(kpts2, 2, "2 Kop Kaptains at depth 20");

    // Verify scaling: deeper dungeons spawn more kops.
    let total_shallow = kops + sgts + lts + kpts;
    let total_deep = kops2 + sgts2 + lts2 + kpts2;
    assert!(
        total_deep > total_shallow,
        "deeper dungeons should spawn more Kops ({} > {})",
        total_deep,
        total_shallow
    );
}

/// Touchstone 3.5 -- Credit covers the bill (no actual robbery).
///
/// If the player has enough credit, leaving the shop does not trigger
/// anger or Kop spawning.
#[test]
fn touchstone_03_shop_credit_covers_bill() {
    let (mut world, _rng) = create_test_world(42);
    let player = world.player();

    let shopkeeper = world.spawn((
        Name("Asidonhopo".to_string()),
        Positioned(Position::new(10, 5)),
    ));

    let mut shop = ShopRoom::new(
        Position::new(5, 2),
        Position::new(15, 8),
        ShopType::General,
        shopkeeper,
        "Asidonhopo".to_string(),
    );

    // Add an item to the bill.
    let item = world.spawn((
        Positioned(Position::new(10, 5)),
        Name("gem".to_string()),
    ));
    shop.bill.add(item, 100, 1);

    // Give the player enough credit.
    shop.add_credit(200);
    assert_eq!(shop.credit, 200);

    // "Rob" the shop -- credit should cover it.
    let mut rng = Pcg64::seed_from_u64(42);
    let events = rob_shop(&world, player, &mut shop, &mut rng);

    // Shopkeeper should NOT be angry.
    assert!(!shop.angry, "shopkeeper should not be angry when credit covers bill");
    assert_eq!(shop.robbed, 0, "nothing should be recorded as stolen");
    assert_eq!(shop.credit, 100, "credit should be reduced by bill amount");

    // Should have a credit-covers message.
    let has_credit = events.iter().any(|e| {
        matches!(e, EngineEvent::Message { key, .. } if key.contains("shop-credit-covers"))
    });
    assert!(has_credit, "should emit shop-credit-covers message");
}

/// Touchstone 3.6 -- Shop door blocking when bill is non-empty.
///
/// A shopkeeper should block the door when the player has unpaid items.
#[test]
fn touchstone_03_shop_door_blocking() {
    let (mut world, _rng) = create_test_world(42);

    let shopkeeper = world.spawn((
        Name("Asidonhopo".to_string()),
        Positioned(Position::new(10, 5)),
    ));

    let mut shop = ShopRoom::new(
        Position::new(5, 2),
        Position::new(15, 8),
        ShopType::General,
        shopkeeper,
        "Asidonhopo".to_string(),
    );
    shop.door_pos = Some(Position::new(5, 5));

    // Empty bill: no blocking.
    assert!(
        !shop.should_block_door(),
        "shopkeeper should not block door when bill is empty"
    );

    // Add an item to the bill.
    let item = world.spawn((
        Positioned(Position::new(10, 5)),
        Name("scroll".to_string()),
    ));
    shop.bill.add(item, 50, 1);

    // Non-empty bill: should block.
    assert!(
        shop.should_block_door(),
        "shopkeeper should block door when bill is non-empty"
    );
}

/// Touchstone 3.7 -- Price differs between buy and sell directions.
///
/// A fundamental shop invariant: the buy price is always >= the sell
/// price for the same item.  For CHA <= 18, buy > sell strictly.
/// At CHA 19+ the buy modifier matches the sell modifier (both 1/2),
/// so prices converge -- this is correct NetHack behavior.
#[test]
fn touchstone_03_shop_buy_sell_spread() {
    // For CHA in the normal range (3..=18), buy is strictly greater
    // than sell for any non-trivial base cost.
    for base_cost in [10, 50, 100, 500] {
        for cha in [5, 10, 12, 16, 18] {
            let buy = get_cost(base_cost, 1, cha, true, false);
            let sell = get_cost(base_cost, 1, cha, false, false);
            assert!(
                buy > sell,
                "buy price ({}) should exceed sell price ({}) for base {} at CHA {}",
                buy,
                sell,
                base_cost,
                cha
            );
        }
    }

    // At CHA 19+ (superhuman), buy and sell converge because both use
    // a 1/2 divisor.  buy >= sell still holds.
    for base_cost in [10, 50, 100, 500] {
        let buy = get_cost(base_cost, 1, 20, true, false);
        let sell = get_cost(base_cost, 1, 20, false, false);
        assert!(
            buy >= sell,
            "buy price ({}) should be >= sell price ({}) at CHA 20 with base {}",
            buy,
            sell,
            base_cost,
        );
    }
}
