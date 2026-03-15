//! Riding (steed) system: mounting, dismounting, and mounted movement.
//!
//! Implements NetHack's riding mechanics from `steed.c`.  A player can
//! mount a tame animal to ride it, gaining the steed's movement speed
//! and carrying its weight.  Mounted combat has to-hit modifiers based
//! on riding skill.
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::event::EngineEvent;
use crate::world::{GameWorld, HitPoints, Monster, Positioned, Speed, Tame};

// ---------------------------------------------------------------------------
// ECS component: MountedOn
// ---------------------------------------------------------------------------

/// Marker component attached to the player entity when riding a steed.
/// Holds the entity handle of the steed being ridden.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MountedOn(pub Entity);

// ---------------------------------------------------------------------------
// Riding skill (affects combat and control)
// ---------------------------------------------------------------------------

/// Riding proficiency level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum RidingSkill {
    Unskilled = 0,
    Basic = 1,
    Skilled = 2,
    Expert = 3,
}

/// Combat to-hit modifier based on riding skill.
///
/// Unskilled: -2, Basic: -1, Skilled: 0, Expert: +2.
pub fn mounted_combat_modifier(skill: RidingSkill) -> i32 {
    match skill {
        RidingSkill::Unskilled => -2,
        RidingSkill::Basic => -1,
        RidingSkill::Skilled => 0,
        RidingSkill::Expert => 2,
    }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum steed carry capacity to support the rider (weight units).
/// The steed must be able to carry at least this much above its own weight.
const MIN_STEED_CAPACITY: u32 = 200;

// ---------------------------------------------------------------------------
// Query helpers
// ---------------------------------------------------------------------------

/// Check whether the player is currently mounted.
pub fn is_mounted(world: &GameWorld, player: Entity) -> bool {
    world.get_component::<MountedOn>(player).is_some()
}

/// Get the steed entity if the player is mounted.
pub fn get_steed(world: &GameWorld, player: Entity) -> Option<Entity> {
    world
        .get_component::<MountedOn>(player)
        .map(|m| m.0)
}

// ---------------------------------------------------------------------------
// Mount
// ---------------------------------------------------------------------------

/// Attempt to mount a steed.  The steed must be tame, adjacent to the
/// player, and strong enough to carry the rider.
///
/// Returns the events produced.
pub fn mount(
    world: &mut GameWorld,
    player: Entity,
    steed: Entity,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Already mounted?
    if is_mounted(world, player) {
        events.push(EngineEvent::msg("already-mounted"));
        return events;
    }

    // Steed must be a tame monster.
    if world.get_component::<Monster>(steed).is_none() {
        events.push(EngineEvent::msg("mount-not-monster"));
        return events;
    }
    if world.get_component::<Tame>(steed).is_none() {
        events.push(EngineEvent::msg("mount-not-tame"));
        return events;
    }

    // Adjacency check.
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };
    let steed_pos = match world.get_component::<Positioned>(steed) {
        Some(p) => p.0,
        None => return events,
    };
    let dx = (player_pos.x - steed_pos.x).abs();
    let dy = (player_pos.y - steed_pos.y).abs();
    if dx > 1 || dy > 1 {
        events.push(EngineEvent::msg("mount-too-far"));
        return events;
    }

    // Check steed's carry capacity (simplified: HP-based heuristic).
    let steed_hp = world
        .get_component::<HitPoints>(steed)
        .map(|hp| hp.max)
        .unwrap_or(10);
    if (steed_hp as u32) < MIN_STEED_CAPACITY / 20 {
        events.push(EngineEvent::msg("mount-too-weak"));
        return events;
    }

    // Mount: move player to steed's position and attach component.
    let _ = world.ecs_mut().insert_one(player, MountedOn(steed));
    if player_pos != steed_pos {
        let _ = world.ecs_mut().insert_one(player, Positioned(steed_pos));
        events.push(EngineEvent::EntityMoved {
            entity: player,
            from: player_pos,
            to: steed_pos,
        });
    }

    events.push(EngineEvent::msg_with(
        "mount-steed",
        vec![("name", world.entity_name(steed))],
    ));

    events
}

// ---------------------------------------------------------------------------
// Dismount
// ---------------------------------------------------------------------------

/// Dismount the current steed.  The player stays at their current
/// position; the steed remains adjacent.
pub fn dismount(
    world: &mut GameWorld,
    player: Entity,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let steed = match get_steed(world, player) {
        Some(s) => s,
        None => {
            events.push(EngineEvent::msg("not-mounted"));
            return events;
        }
    };

    let steed_name = world.entity_name(steed);

    // Remove the MountedOn component.
    let _ = world.ecs_mut().remove_one::<MountedOn>(player);

    events.push(EngineEvent::msg_with(
        "dismount-steed",
        vec![("name", steed_name)],
    ));

    events
}

// ---------------------------------------------------------------------------
// Steed panic (thrown from steed)
// ---------------------------------------------------------------------------

/// Chance of being thrown from the steed during combat, based on riding
/// skill.  Returns `true` if the rider is thrown.
pub fn check_thrown(rng: &mut impl Rng, skill: RidingSkill) -> bool {
    let threshold = match skill {
        RidingSkill::Unskilled => 25, // 25% chance
        RidingSkill::Basic => 10,
        RidingSkill::Skilled => 3,
        RidingSkill::Expert => 0,
    };
    rng.random_range(0..100u32) < threshold
}

/// Handle being thrown from the steed (during combat or steed panic).
pub fn throw_rider(
    world: &mut GameWorld,
    player: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if let Some(steed) = get_steed(world, player) {
        let steed_name = world.entity_name(steed);
        let _ = world.ecs_mut().remove_one::<MountedOn>(player);
        events.push(EngineEvent::msg_with(
            "thrown-from-steed",
            vec![("name", steed_name)],
        ));
    }

    events
}

// ---------------------------------------------------------------------------
// Saddle check
// ---------------------------------------------------------------------------

/// Whether a monster can be saddled.
///
/// In NetHack, a monster can be saddled if it is an animal-type, not
/// amorphous/nolimbs/verysmall/tiny, and strong enough to carry a rider.
pub fn can_saddle(
    is_animal: bool,
    is_amorphous: bool,
    is_nolimbs: bool,
    size: CreatureSize,
) -> bool {
    if !is_animal {
        return false;
    }
    if is_amorphous || is_nolimbs {
        return false;
    }
    !matches!(size, CreatureSize::Tiny)
}

/// Creature size categories for steed eligibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CreatureSize {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
    Gigantic,
}

// ---------------------------------------------------------------------------
// Can-ride check (can_ride from steed.c)
// ---------------------------------------------------------------------------

/// Whether the player can ride a monster.
///
/// Requirements: monster must be tame, player must be humanoid and not
/// very small or big.
pub fn can_ride(
    is_tame: bool,
    player_is_humanoid: bool,
    player_size: CreatureSize,
) -> bool {
    is_tame
        && player_is_humanoid
        && !matches!(
            player_size,
            CreatureSize::Tiny | CreatureSize::Huge | CreatureSize::Gigantic
        )
}

// ---------------------------------------------------------------------------
// Kick steed (kick_steed from steed.c)
// ---------------------------------------------------------------------------

/// Result of kicking the steed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KickSteedResult {
    /// Steed was sleeping/paralyzed, kicking may rouse it.
    Roused { still_helpless: bool },
    /// Steed didn't respond to the kick.
    NoResponse,
    /// Steed went untame and threw the rider.
    ThrownOff,
    /// Steed gallops (speed boost).
    Gallops { gallop_turns: u32 },
}

/// Simulate kicking the steed.
///
/// Kicking reduces tameness by 1. If tameness reaches 0, or a level
/// check fails, the steed throws the rider.  Otherwise, the steed gallops.
pub fn kick_steed(
    tameness: u32,
    player_level: u32,
    steed_helpless: bool,
    rng: &mut impl Rng,
) -> (KickSteedResult, u32) {
    // Kicking a helpless steed.
    if steed_helpless {
        if rng.random_range(0..2u32) == 0 {
            return (
                KickSteedResult::Roused {
                    still_helpless: false,
                },
                tameness,
            );
        } else {
            return (
                KickSteedResult::Roused {
                    still_helpless: true,
                },
                tameness,
            );
        }
    }

    let new_tameness = tameness.saturating_sub(1);

    // Check if steed resists (untame or level check fails).
    if new_tameness == 0
        || player_level + new_tameness < rng.random_range(1..=15)
    {
        return (KickSteedResult::ThrownOff, 0);
    }

    // Gallop!
    let gallop_turns = rng.random_range(30..=49);
    (KickSteedResult::Gallops { gallop_turns }, new_tameness)
}

// ---------------------------------------------------------------------------
// Exercise riding skill
// ---------------------------------------------------------------------------

/// Track riding turns and determine if riding skill should be exercised.
///
/// In NetHack, every 100 turns of riding exercises the riding skill once.
pub fn exercise_steed(ride_turns: &mut u32) -> bool {
    *ride_turns += 1;
    if *ride_turns >= 100 {
        *ride_turns = 0;
        true
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Dismount reason
// ---------------------------------------------------------------------------

/// Reason for dismounting (dismount_steed from steed.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DismountReason {
    /// Player chose to dismount (#ride).
    ByChoice,
    /// Thrown off by steed (kick_steed or bucking).
    Thrown,
    /// Knocked off by an attack.
    Knocked,
    /// Fell off (slippery saddle, trap, etc.).
    Fell,
    /// Player polymorphed into a form that can't ride.
    Poly,
    /// Player was engulfed by a monster.
    Engulfed,
    /// Player died (bones file handling).
    Bones,
    /// Generic forced dismount.
    Generic,
}

/// Calculate fall damage when thrown/knocked from steed.
///
/// In NetHack: `rn1(10, 10)` = 10-19 damage, then `Maybe_Half_Phys`.
/// Also sets `Wounded_legs` for `rn1(5,5)` = 5-9 turns.
pub fn dismount_fall_damage(
    reason: DismountReason,
    is_levitating: bool,
    is_flying: bool,
    rng: &mut impl Rng,
) -> (i32, u32) {
    if is_levitating || is_flying {
        return (0, 0);
    }
    match reason {
        DismountReason::Thrown | DismountReason::Knocked | DismountReason::Fell => {
            let damage = rng.random_range(10..=19);
            let leg_turns = rng.random_range(5..=9);
            (damage, leg_turns)
        }
        _ => (0, 0),
    }
}

// ---------------------------------------------------------------------------
// Landing spot (finding where to stand after dismount)
// ---------------------------------------------------------------------------

/// Find the best adjacent position to land after dismounting.
///
/// Simplified version of landing_spot() from steed.c.
/// Prefers orthogonal positions, avoids walls and traps.
pub fn find_landing_spot(
    steed_pos: crate::action::Position,
    terrain_checker: impl Fn(crate::action::Position) -> bool,
) -> Option<crate::action::Position> {
    use crate::action::Position;

    // Check orthogonal first, then diagonal.
    let offsets = [
        (0, -1), (0, 1), (-1, 0), (1, 0),
        (-1, -1), (1, -1), (-1, 1), (1, 1),
    ];

    for &(dx, dy) in &offsets {
        let candidate = Position::new(steed_pos.x + dx, steed_pos.y + dy);
        if terrain_checker(candidate) {
            return Some(candidate);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Steed polymorph (poly_steed from steed.c)
// ---------------------------------------------------------------------------

/// What happens when the steed polymorphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolySteedResult {
    /// The new form can still be ridden.
    StillRideable,
    /// The new form can't be ridden; rider must dismount.
    MustDismount,
    /// The new form is too small; rider is dumped.
    TooSmall,
}

/// Determine what happens when a steed polymorphs.
pub fn poly_steed_check(
    new_form_is_animal: bool,
    new_form_size: CreatureSize,
) -> PolySteedResult {
    if !new_form_is_animal {
        return PolySteedResult::MustDismount;
    }
    if matches!(new_form_size, CreatureSize::Tiny | CreatureSize::Small) {
        return PolySteedResult::TooSmall;
    }
    PolySteedResult::StillRideable
}

// ---------------------------------------------------------------------------
// Stucksteed: check if steed is stuck and rider can't move
// ---------------------------------------------------------------------------

/// Whether the steed is stuck (e.g., in a bear trap, web, or being
/// held by a monster) and the rider therefore can't move.
pub fn steed_is_stuck(
    steed_trapped: bool,
    steed_held: bool,
) -> bool {
    steed_trapped || steed_held
}

// ---------------------------------------------------------------------------
// Mounted movement speed
// ---------------------------------------------------------------------------

/// Get the effective movement speed when mounted.  Uses the steed's
/// base speed.  Returns `None` if not mounted or steed has no Speed.
pub fn mounted_speed(world: &GameWorld, player: Entity) -> Option<u32> {
    let steed = get_steed(world, player)?;
    world.get_component::<Speed>(steed).map(|s| s.0)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

    use crate::action::Position;
    use crate::world::{GameWorld, HitPoints, Monster, Name, Positioned, Speed, Tame};

    fn test_rng() -> Pcg64Mcg {
        Pcg64Mcg::seed_from_u64(42)
    }

    fn make_world() -> GameWorld {
        GameWorld::new(Position::new(10, 10))
    }

    fn spawn_tame_steed(
        world: &mut GameWorld,
        name: &str,
        pos: Position,
    ) -> Entity {
        world.spawn((
            Monster,
            Tame,
            Positioned(pos),
            Name(name.to_string()),
            Speed(18),
            HitPoints { current: 30, max: 30 },
        ))
    }

    // -----------------------------------------------------------------------
    // Test 1: Successful mount
    // -----------------------------------------------------------------------
    #[test]
    fn mount_tame_adjacent_steed() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        let steed = spawn_tame_steed(&mut world, "warhorse", Position::new(11, 10));

        let events = mount(&mut world, player, steed, &mut rng);
        assert!(is_mounted(&world, player));
        assert_eq!(get_steed(&world, player), Some(steed));
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "mount-steed")));
    }

    // -----------------------------------------------------------------------
    // Test 2: Cannot mount non-tame monster
    // -----------------------------------------------------------------------
    #[test]
    fn cannot_mount_wild() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        let wild = world.spawn((
            Monster,
            Positioned(Position::new(11, 10)),
            Name("warhorse".to_string()),
            Speed(18),
            HitPoints { current: 30, max: 30 },
        ));

        let events = mount(&mut world, player, wild, &mut rng);
        assert!(!is_mounted(&world, player));
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "mount-not-tame")));
    }

    // -----------------------------------------------------------------------
    // Test 3: Dismount
    // -----------------------------------------------------------------------
    #[test]
    fn dismount_succeeds() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        let steed = spawn_tame_steed(&mut world, "pony", Position::new(11, 10));
        let _ = mount(&mut world, player, steed, &mut rng);
        assert!(is_mounted(&world, player));

        let events = dismount(&mut world, player, &mut rng);
        assert!(!is_mounted(&world, player));
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "dismount-steed")));
    }

    // -----------------------------------------------------------------------
    // Test 4: Dismount when not mounted
    // -----------------------------------------------------------------------
    #[test]
    fn dismount_when_not_mounted() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        let events = dismount(&mut world, player, &mut rng);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "not-mounted")));
    }

    // -----------------------------------------------------------------------
    // Test 5: Mounted speed uses steed's speed
    // -----------------------------------------------------------------------
    #[test]
    fn mounted_uses_steed_speed() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        let steed = spawn_tame_steed(&mut world, "warhorse", Position::new(11, 10));
        let _ = mount(&mut world, player, steed, &mut rng);

        assert_eq!(mounted_speed(&world, player), Some(18));
    }

    // -----------------------------------------------------------------------
    // Test 6: Mounted combat modifier
    // -----------------------------------------------------------------------
    #[test]
    fn combat_modifier_by_skill() {
        assert_eq!(mounted_combat_modifier(RidingSkill::Unskilled), -2);
        assert_eq!(mounted_combat_modifier(RidingSkill::Basic), -1);
        assert_eq!(mounted_combat_modifier(RidingSkill::Skilled), 0);
        assert_eq!(mounted_combat_modifier(RidingSkill::Expert), 2);
    }

    // -----------------------------------------------------------------------
    // Test 7: Cannot mount when already mounted
    // -----------------------------------------------------------------------
    #[test]
    fn cannot_double_mount() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        let s1 = spawn_tame_steed(&mut world, "pony", Position::new(11, 10));
        let _s2 = spawn_tame_steed(&mut world, "horse", Position::new(9, 10));

        let _ = mount(&mut world, player, s1, &mut rng);
        let events = mount(&mut world, player, _s2, &mut rng);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "already-mounted")));
    }

    // -----------------------------------------------------------------------
    // Test 8: Throw rider check
    // -----------------------------------------------------------------------
    #[test]
    fn expert_never_thrown() {
        let mut rng = test_rng();
        // Expert threshold is 0%, so check_thrown should always be false.
        for _ in 0..100 {
            assert!(!check_thrown(&mut rng, RidingSkill::Expert));
        }
    }

    // -----------------------------------------------------------------------
    // Test 9: Can saddle
    // -----------------------------------------------------------------------
    #[test]
    fn can_saddle_normal_animal() {
        assert!(can_saddle(true, false, false, CreatureSize::Large));
        assert!(can_saddle(true, false, false, CreatureSize::Medium));
    }

    #[test]
    fn cannot_saddle_non_animal() {
        assert!(!can_saddle(false, false, false, CreatureSize::Large));
    }

    #[test]
    fn cannot_saddle_amorphous() {
        assert!(!can_saddle(true, true, false, CreatureSize::Large));
    }

    #[test]
    fn cannot_saddle_tiny() {
        assert!(!can_saddle(true, false, false, CreatureSize::Tiny));
    }

    // -----------------------------------------------------------------------
    // Test 10: Can ride
    // -----------------------------------------------------------------------
    #[test]
    fn can_ride_tame_humanoid_medium() {
        assert!(can_ride(true, true, CreatureSize::Medium));
    }

    #[test]
    fn cannot_ride_wild() {
        assert!(!can_ride(false, true, CreatureSize::Medium));
    }

    #[test]
    fn cannot_ride_non_humanoid() {
        assert!(!can_ride(true, false, CreatureSize::Medium));
    }

    #[test]
    fn cannot_ride_huge_player() {
        assert!(!can_ride(true, true, CreatureSize::Huge));
    }

    #[test]
    fn cannot_ride_tiny_player() {
        assert!(!can_ride(true, true, CreatureSize::Tiny));
    }

    // -----------------------------------------------------------------------
    // Test 11: Kick steed
    // -----------------------------------------------------------------------
    #[test]
    fn kick_helpless_steed() {
        let mut rng = test_rng();
        let (result, tame) = kick_steed(5, 10, true, &mut rng);
        assert_eq!(tame, 5); // tameness unchanged for helpless
        assert!(matches!(result, KickSteedResult::Roused { .. }));
    }

    #[test]
    fn kick_steed_gallops() {
        // High tameness + high level = gallop.
        let mut galloped = false;
        for seed in 0..50u64 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let (result, _) = kick_steed(20, 20, false, &mut rng);
            if matches!(result, KickSteedResult::Gallops { .. }) {
                galloped = true;
            }
        }
        assert!(galloped, "should gallop with high tameness and level");
    }

    #[test]
    fn kick_steed_thrown_low_tame() {
        // tameness=1 -> new_tameness=0 -> always thrown.
        let mut rng = test_rng();
        let (result, tame) = kick_steed(1, 1, false, &mut rng);
        assert_eq!(tame, 0);
        assert_eq!(result, KickSteedResult::ThrownOff);
    }

    // -----------------------------------------------------------------------
    // Test 12: Exercise steed
    // -----------------------------------------------------------------------
    #[test]
    fn exercise_steed_at_100_turns() {
        let mut turns = 0u32;
        for i in 1..=100 {
            let exercised = exercise_steed(&mut turns);
            if i < 100 {
                assert!(!exercised);
            } else {
                assert!(exercised);
                assert_eq!(turns, 0);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test 13: Dismount fall damage
    // -----------------------------------------------------------------------
    #[test]
    fn dismount_fall_damage_thrown() {
        let mut rng = test_rng();
        let (dmg, legs) = dismount_fall_damage(
            DismountReason::Thrown, false, false, &mut rng,
        );
        assert!(dmg >= 10 && dmg <= 19);
        assert!(legs >= 5 && legs <= 9);
    }

    #[test]
    fn dismount_no_damage_levitating() {
        let mut rng = test_rng();
        let (dmg, legs) = dismount_fall_damage(
            DismountReason::Thrown, true, false, &mut rng,
        );
        assert_eq!(dmg, 0);
        assert_eq!(legs, 0);
    }

    #[test]
    fn dismount_no_damage_by_choice() {
        let mut rng = test_rng();
        let (dmg, legs) = dismount_fall_damage(
            DismountReason::ByChoice, false, false, &mut rng,
        );
        assert_eq!(dmg, 0);
        assert_eq!(legs, 0);
    }

    // -----------------------------------------------------------------------
    // Test 14: Find landing spot
    // -----------------------------------------------------------------------
    #[test]
    fn find_landing_spot_prefers_orthogonal() {
        let pos = Position::new(10, 10);
        let spot = find_landing_spot(pos, |p| {
            // Only orthogonal positions are "valid".
            let dx = (p.x - pos.x).abs();
            let dy = (p.y - pos.y).abs();
            dx + dy == 1 // orthogonal
        });
        assert!(spot.is_some());
        let s = spot.unwrap();
        let dx = (s.x - pos.x).abs();
        let dy = (s.y - pos.y).abs();
        assert_eq!(dx + dy, 1, "should pick orthogonal spot");
    }

    #[test]
    fn find_landing_spot_falls_back_to_diagonal() {
        let pos = Position::new(10, 10);
        let spot = find_landing_spot(pos, |p| {
            // Only diagonal positions are "valid".
            let dx = (p.x - pos.x).abs();
            let dy = (p.y - pos.y).abs();
            dx == 1 && dy == 1
        });
        assert!(spot.is_some());
    }

    #[test]
    fn find_landing_spot_none_if_all_blocked() {
        let pos = Position::new(10, 10);
        let spot = find_landing_spot(pos, |_| false);
        assert!(spot.is_none());
    }

    // -----------------------------------------------------------------------
    // Test 15: Poly steed
    // -----------------------------------------------------------------------
    #[test]
    fn poly_steed_still_rideable() {
        assert_eq!(
            poly_steed_check(true, CreatureSize::Large),
            PolySteedResult::StillRideable,
        );
    }

    #[test]
    fn poly_steed_must_dismount_non_animal() {
        assert_eq!(
            poly_steed_check(false, CreatureSize::Large),
            PolySteedResult::MustDismount,
        );
    }

    #[test]
    fn poly_steed_too_small() {
        assert_eq!(
            poly_steed_check(true, CreatureSize::Tiny),
            PolySteedResult::TooSmall,
        );
        assert_eq!(
            poly_steed_check(true, CreatureSize::Small),
            PolySteedResult::TooSmall,
        );
    }

    // -----------------------------------------------------------------------
    // Test 16: Steed stuck
    // -----------------------------------------------------------------------
    #[test]
    fn steed_stuck_trapped() {
        assert!(steed_is_stuck(true, false));
    }

    #[test]
    fn steed_stuck_held() {
        assert!(steed_is_stuck(false, true));
    }

    #[test]
    fn steed_not_stuck() {
        assert!(!steed_is_stuck(false, false));
    }
}
