//! Ball and chain (punishment) system.
//!
//! Implements NetHack's punishment mechanics from `ball.c`.  When punished,
//! the player is attached to a heavy iron ball via a chain.  The ball
//! restricts movement (chain length ~5 tiles), adds significant weight
//! to encumbrance, and can optionally be wielded as a weapon.
//!
//! All functions are pure: they operate on `GameWorld`, mutate world
//! state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use serde::{Deserialize, Serialize};

use crate::action::Position;
use crate::event::EngineEvent;
use crate::world::{GameWorld, Positioned};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Weight of the iron ball in weight units (cn).
pub const IRON_BALL_WEIGHT: u32 = 480;

/// Weight of the iron chain in weight units (cn).
pub const IRON_CHAIN_WEIGHT: u32 = 120;

/// Maximum chain length (Chebyshev distance) for movement restriction.
pub const CHAIN_LENGTH: i32 = 5;

/// Damage dealt by swinging the iron ball: d(25,1) = always 1..25.
/// (In NetHack, the heavy iron ball does d(25,1) base damage.)
pub const BALL_DAMAGE_SIDES: u32 = 25;
pub const BALL_DAMAGE_DICE: u32 = 1;

// ---------------------------------------------------------------------------
// ECS component: Punished
// ---------------------------------------------------------------------------

/// Component attached to the player when punished.
/// Holds entity handles for the iron ball and chain.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Punished {
    pub ball: Entity,
    pub chain: Entity,
}

/// Marker component for the iron ball entity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IronBall;

/// Marker component for the iron chain entity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IronChain;

// ---------------------------------------------------------------------------
// Query helpers
// ---------------------------------------------------------------------------

/// Check whether the player is currently punished (has ball and chain).
pub fn is_punished(world: &GameWorld, player: Entity) -> bool {
    world.get_component::<Punished>(player).is_some()
}

/// Get the Punished component if the player is punished.
pub fn get_punishment(world: &GameWorld, player: Entity) -> Option<Punished> {
    world.get_component::<Punished>(player).map(|p| *p)
}

// ---------------------------------------------------------------------------
// Apply punishment
// ---------------------------------------------------------------------------

/// Attach an iron ball and chain to the player.
///
/// Creates two new entities (ball and chain) at the player's position
/// and attaches the `Punished` component to the player.
pub fn apply_punishment(world: &mut GameWorld, player: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if is_punished(world, player) {
        events.push(EngineEvent::msg("already-punished"));
        return events;
    }

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    // Spawn the iron ball.
    let ball = world.spawn((IronBall, Positioned(player_pos)));

    // Spawn the iron chain.
    let chain = world.spawn((IronChain, Positioned(player_pos)));

    // Attach to player.
    let _ = world.ecs_mut().insert_one(player, Punished { ball, chain });

    events.push(EngineEvent::msg("punishment-applied"));
    events
}

// ---------------------------------------------------------------------------
// Remove punishment
// ---------------------------------------------------------------------------

/// Remove the iron ball and chain from the player (e.g., via scroll of
/// remove curse or prayer).
pub fn remove_punishment(world: &mut GameWorld, player: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let punishment = match get_punishment(world, player) {
        Some(p) => p,
        None => {
            events.push(EngineEvent::msg("not-punished"));
            return events;
        }
    };

    // Despawn ball and chain.
    let _ = world.despawn(punishment.ball);
    let _ = world.despawn(punishment.chain);

    // Remove the component.
    let _ = world.ecs_mut().remove_one::<Punished>(player);

    events.push(EngineEvent::msg("punishment-removed"));
    events
}

// ---------------------------------------------------------------------------
// Movement restriction check
// ---------------------------------------------------------------------------

/// Check whether the player can move to `target_pos` given the ball's
/// current position and the chain length constraint.
///
/// Returns `true` if the move is allowed, `false` if the chain is too
/// short.  The ball drags behind the player: if the player moves away,
/// the ball is dragged one step closer (handled by `drag_ball`).
pub fn can_move_with_ball(world: &GameWorld, player: Entity, target_pos: Position) -> bool {
    let punishment = match get_punishment(world, player) {
        Some(p) => p,
        None => return true, // Not punished, always ok.
    };

    let ball_pos = match world.get_component::<Positioned>(punishment.ball) {
        Some(p) => p.0,
        None => return true,
    };

    // Chebyshev distance from target to ball must be <= chain length.
    let dx = (target_pos.x - ball_pos.x).abs();
    let dy = (target_pos.y - ball_pos.y).abs();
    let dist = dx.max(dy);

    dist <= CHAIN_LENGTH
}

/// Drag the iron ball one step toward the player if needed.
///
/// Called after the player moves.  If the ball is more than 1 tile away
/// from the player's new position, it is dragged one step closer.
pub fn drag_ball(world: &mut GameWorld, player: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let punishment = match get_punishment(world, player) {
        Some(p) => p,
        None => return events,
    };

    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return events,
    };

    let ball_pos = match world.get_component::<Positioned>(punishment.ball) {
        Some(p) => p.0,
        None => return events,
    };

    let dx = player_pos.x - ball_pos.x;
    let dy = player_pos.y - ball_pos.y;
    let dist = dx.abs().max(dy.abs());

    if dist > 1 {
        // Move ball one step toward the player.
        let step_x = dx.signum();
        let step_y = dy.signum();
        let new_ball_pos = Position::new(ball_pos.x + step_x, ball_pos.y + step_y);

        let _ = world
            .ecs_mut()
            .insert_one(punishment.ball, Positioned(new_ball_pos));

        // Also move the chain to the ball's new position.
        let _ = world
            .ecs_mut()
            .insert_one(punishment.chain, Positioned(new_ball_pos));

        events.push(EngineEvent::EntityMoved {
            entity: punishment.ball,
            from: ball_pos,
            to: new_ball_pos,
        });
    }

    events
}

// ---------------------------------------------------------------------------
// Pure movement helpers (no ECS)
// ---------------------------------------------------------------------------

/// Result of attempting to move the ball and chain when the player moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BallMoveResult {
    /// Ball stays put; chain stretches between player and ball.
    ChainStretches { new_chain_pos: Position },
    /// Ball must be dragged one step toward the player.
    BallDragged {
        new_ball_pos: Position,
        new_chain_pos: Position,
    },
}

/// Result of the ball falling into a pit or hole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BallFallResult {
    /// Player is close enough to be dragged down by the ball.
    PlayerDraggedDown,
    /// Ball is too far; chain snaps.
    ChainSnaps,
}

/// Result of kicking the iron ball.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KickBallResult {
    /// Ball slides to a new position.
    Slides { new_pos: Position, distance: i32 },
    /// Ball is stuck (terrain blocks).
    Stuck,
}

/// Chebyshev (king's move) distance between two positions.
#[inline]
pub fn chebyshev_distance(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

/// Compute the midpoint between two positions (integer division).
/// Used to place the chain between the player and ball.
#[inline]
fn midpoint(a: Position, b: Position) -> Position {
    Position::new((a.x + b.x) / 2, (a.y + b.y) / 2)
}

/// Determine how the ball and chain should move when the player moves.
///
/// The ball follows the player, dragging along the chain.
/// If the new player position is still within chain length of the ball,
/// only the chain position updates.  Otherwise the ball is dragged one
/// step toward the player.
pub fn move_bc(
    _player_pos: Position,
    new_player_pos: Position,
    ball_pos: Position,
    chain_length: i32,
) -> BallMoveResult {
    let dist = chebyshev_distance(new_player_pos, ball_pos);
    if dist <= chain_length {
        BallMoveResult::ChainStretches {
            new_chain_pos: midpoint(new_player_pos, ball_pos),
        }
    } else {
        let new_ball = drag_ball_pure(ball_pos, new_player_pos, |_, _| true);
        BallMoveResult::BallDragged {
            new_ball_pos: new_ball,
            new_chain_pos: midpoint(new_player_pos, new_ball),
        }
    }
}

/// Drag the ball one step toward `target_pos`, checking terrain passability.
///
/// Returns the new ball position (unchanged if terrain blocks).
pub fn drag_ball_pure(
    ball_pos: Position,
    target_pos: Position,
    is_passable: impl Fn(i32, i32) -> bool,
) -> Position {
    let dx = (target_pos.x - ball_pos.x).signum();
    let dy = (target_pos.y - ball_pos.y).signum();
    let new_pos = Position::new(ball_pos.x + dx, ball_pos.y + dy);
    if is_passable(new_pos.x, new_pos.y) {
        new_pos
    } else {
        ball_pos // stuck
    }
}

/// Check if player can move given ball constraints.
///
/// The move is allowed if the target is within chain length of the ball,
/// or if the ball can be dragged (`can_drag` is true).
pub fn ball_allows_move(
    target_pos: Position,
    ball_pos: Position,
    chain_length: i32,
    can_drag: bool,
) -> bool {
    let dist = chebyshev_distance(target_pos, ball_pos);
    dist <= chain_length || can_drag
}

/// Check if the ball falling into a pit/hole drags the player down.
///
/// If the ball is within chain length, the player is pulled down.
/// Otherwise the chain effectively snaps (player is freed).
pub fn ball_falls(ball_pos: Position, player_pos: Position, chain_length: i32) -> BallFallResult {
    if chebyshev_distance(ball_pos, player_pos) <= chain_length {
        BallFallResult::PlayerDraggedDown
    } else {
        BallFallResult::ChainSnaps
    }
}

/// Kick the ball in a direction.
///
/// The ball slides 1 to min(strength, 3) tiles.  Each intermediate
/// position must be passable or the ball stops short.
pub fn kick_ball(
    ball_pos: Position,
    direction: (i32, i32),
    strength: i32,
    is_passable: impl Fn(i32, i32) -> bool,
    rng: &mut impl rand::Rng,
) -> KickBallResult {
    let max_dist = strength.clamp(1, 3);
    let distance = rng.random_range(1..=max_dist as u32) as i32;
    let mut pos = ball_pos;
    for _ in 0..distance {
        let next = Position::new(pos.x + direction.0, pos.y + direction.1);
        if !is_passable(next.x, next.y) {
            return if pos == ball_pos {
                KickBallResult::Stuck
            } else {
                KickBallResult::Slides {
                    new_pos: pos,
                    distance: chebyshev_distance(ball_pos, pos),
                }
            };
        }
        pos = next;
    }
    KickBallResult::Slides {
        new_pos: pos,
        distance: chebyshev_distance(ball_pos, pos),
    }
}

// ---------------------------------------------------------------------------
// Ball as weapon
// ---------------------------------------------------------------------------

/// Compute damage from swinging the iron ball as a weapon.
/// Returns d(1,25) using the provided RNG.
pub fn ball_damage(rng: &mut impl rand::Rng) -> u32 {
    rng.random_range(1..=BALL_DAMAGE_SIDES)
}

/// Total encumbrance added by punishment (ball + chain weights).
pub fn punishment_weight() -> u32 {
    IRON_BALL_WEIGHT + IRON_CHAIN_WEIGHT
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

    use crate::world::{GameWorld, Positioned};

    fn test_rng() -> Pcg64Mcg {
        Pcg64Mcg::seed_from_u64(42)
    }

    fn make_world() -> GameWorld {
        GameWorld::new(Position::new(10, 10))
    }

    // -----------------------------------------------------------------------
    // Test 1: Apply punishment
    // -----------------------------------------------------------------------
    #[test]
    fn apply_creates_ball_and_chain() {
        let mut world = make_world();
        let player = world.player();

        let events = apply_punishment(&mut world, player);
        assert!(is_punished(&world, player));

        let punishment = get_punishment(&world, player).unwrap();
        assert!(world.get_component::<IronBall>(punishment.ball).is_some());
        assert!(world.get_component::<IronChain>(punishment.chain).is_some());

        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "punishment-applied")));
    }

    // -----------------------------------------------------------------------
    // Test 2: Remove punishment
    // -----------------------------------------------------------------------
    #[test]
    fn remove_despawns_ball_and_chain() {
        let mut world = make_world();
        let player = world.player();

        let _ = apply_punishment(&mut world, player);
        let punishment = get_punishment(&world, player).unwrap();
        let ball = punishment.ball;
        let chain = punishment.chain;

        let events = remove_punishment(&mut world, player);
        assert!(!is_punished(&world, player));
        assert!(world.get_component::<IronBall>(ball).is_none());
        assert!(world.get_component::<IronChain>(chain).is_none());

        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "punishment-removed")));
    }

    // -----------------------------------------------------------------------
    // Test 3: Double punishment prevented
    // -----------------------------------------------------------------------
    #[test]
    fn cannot_double_punish() {
        let mut world = make_world();
        let player = world.player();

        let _ = apply_punishment(&mut world, player);
        let events = apply_punishment(&mut world, player);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "already-punished")));
    }

    // -----------------------------------------------------------------------
    // Test 4: Movement within chain length allowed
    // -----------------------------------------------------------------------
    #[test]
    fn move_within_chain_ok() {
        let mut world = make_world();
        let player = world.player();

        let _ = apply_punishment(&mut world, player);

        // Player at (10,10), ball at (10,10): moving to (15,10) is dist=5, ok.
        assert!(can_move_with_ball(&world, player, Position::new(15, 10)));
        // Moving to (16,10) is dist=6, blocked.
        assert!(!can_move_with_ball(&world, player, Position::new(16, 10)));
    }

    // -----------------------------------------------------------------------
    // Test 5: Ball dragging
    // -----------------------------------------------------------------------
    #[test]
    fn ball_drags_toward_player() {
        let mut world = make_world();
        let player = world.player();

        let _ = apply_punishment(&mut world, player);

        // Move the player to (13,10) — 3 tiles away from ball at (10,10).
        let _ = world
            .ecs_mut()
            .insert_one(player, Positioned(Position::new(13, 10)));

        let events = drag_ball(&mut world, player);

        let punishment = get_punishment(&world, player).unwrap();
        let ball_pos = world
            .get_component::<Positioned>(punishment.ball)
            .unwrap()
            .0;
        // Ball should move one step toward player: (11, 10).
        assert_eq!(ball_pos, Position::new(11, 10));
        assert!(!events.is_empty(), "should emit EntityMoved for ball");
    }

    // -----------------------------------------------------------------------
    // Test 6: Ball damage is in valid range
    // -----------------------------------------------------------------------
    #[test]
    fn ball_damage_range() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let dmg = ball_damage(&mut rng);
            assert!((1..=25).contains(&dmg), "ball damage {} out of range", dmg);
        }
    }

    // -----------------------------------------------------------------------
    // Test 7: Chebyshev distance calculation
    // -----------------------------------------------------------------------
    #[test]
    fn chebyshev_distance_cases() {
        let a = Position::new(0, 0);
        assert_eq!(chebyshev_distance(a, Position::new(3, 4)), 4);
        assert_eq!(chebyshev_distance(a, Position::new(5, 5)), 5);
        assert_eq!(chebyshev_distance(a, Position::new(0, 0)), 0);
        assert_eq!(chebyshev_distance(a, Position::new(1, 0)), 1);
        assert_eq!(chebyshev_distance(a, Position::new(-3, 2)), 3);
    }

    // -----------------------------------------------------------------------
    // Test 8: move_bc chain stretches when within range
    // -----------------------------------------------------------------------
    #[test]
    fn move_bc_chain_stretches() {
        let player = Position::new(5, 5);
        let new_player = Position::new(6, 5);
        let ball = Position::new(5, 5);
        let result = move_bc(player, new_player, ball, CHAIN_LENGTH);
        match result {
            BallMoveResult::ChainStretches { new_chain_pos } => {
                // Chain is midpoint between new player (6,5) and ball (5,5)
                assert_eq!(new_chain_pos, Position::new(5, 5));
            }
            _ => panic!("expected ChainStretches"),
        }
    }

    // -----------------------------------------------------------------------
    // Test 9: move_bc ball dragged when too far
    // -----------------------------------------------------------------------
    #[test]
    fn move_bc_ball_dragged() {
        let player = Position::new(5, 5);
        let ball = Position::new(0, 5);
        // Move player further away (dist = 6 > CHAIN_LENGTH=5)
        let new_player = Position::new(6, 5);
        let result = move_bc(player, new_player, ball, CHAIN_LENGTH);
        match result {
            BallMoveResult::BallDragged {
                new_ball_pos,
                new_chain_pos,
            } => {
                // Ball should move one step toward new_player: (1, 5)
                assert_eq!(new_ball_pos, Position::new(1, 5));
                // Chain midpoint between (6,5) and (1,5) = (3,5)
                assert_eq!(new_chain_pos, Position::new(3, 5));
            }
            _ => panic!("expected BallDragged"),
        }
    }

    // -----------------------------------------------------------------------
    // Test 10: ball_allows_move within chain length
    // -----------------------------------------------------------------------
    #[test]
    fn ball_allows_move_within_range() {
        let target = Position::new(3, 0);
        let ball = Position::new(0, 0);
        assert!(ball_allows_move(target, ball, 5, false));
        assert!(ball_allows_move(target, ball, 5, true));
    }

    // -----------------------------------------------------------------------
    // Test 11: ball_allows_move blocked without drag
    // -----------------------------------------------------------------------
    #[test]
    fn ball_allows_move_blocked() {
        let target = Position::new(7, 0);
        let ball = Position::new(0, 0);
        assert!(!ball_allows_move(target, ball, 5, false));
        // But allowed if can_drag is true
        assert!(ball_allows_move(target, ball, 5, true));
    }

    // -----------------------------------------------------------------------
    // Test 12: ball_falls within chain pulls player
    // -----------------------------------------------------------------------
    #[test]
    fn ball_falls_drags_player() {
        let ball = Position::new(5, 5);
        let player = Position::new(7, 5);
        assert_eq!(
            ball_falls(ball, player, 5),
            BallFallResult::PlayerDraggedDown
        );
    }

    // -----------------------------------------------------------------------
    // Test 13: ball_falls too far snaps chain
    // -----------------------------------------------------------------------
    #[test]
    fn ball_falls_chain_snaps() {
        let ball = Position::new(0, 0);
        let player = Position::new(10, 10);
        assert_eq!(ball_falls(ball, player, 5), BallFallResult::ChainSnaps);
    }

    // -----------------------------------------------------------------------
    // Test 14: kick_ball slides
    // -----------------------------------------------------------------------
    #[test]
    fn kick_ball_slides() {
        let mut rng = test_rng();
        let ball = Position::new(5, 5);
        let result = kick_ball(ball, (1, 0), 3, |_, _| true, &mut rng);
        match result {
            KickBallResult::Slides { new_pos, distance } => {
                assert!(distance >= 1 && distance <= 3);
                assert_eq!(new_pos.y, 5);
                assert!(new_pos.x > 5 && new_pos.x <= 8);
            }
            _ => panic!("expected Slides"),
        }
    }

    // -----------------------------------------------------------------------
    // Test 15: kick_ball stuck on impassable terrain
    // -----------------------------------------------------------------------
    #[test]
    fn kick_ball_stuck() {
        let mut rng = test_rng();
        let ball = Position::new(5, 5);
        let result = kick_ball(ball, (1, 0), 3, |_, _| false, &mut rng);
        assert_eq!(result, KickBallResult::Stuck);
    }

    // -----------------------------------------------------------------------
    // Test 16: drag_ball_pure toward target
    // -----------------------------------------------------------------------
    #[test]
    fn drag_ball_pure_moves_toward_target() {
        let ball = Position::new(3, 3);
        let target = Position::new(6, 3);
        let new = drag_ball_pure(ball, target, |_, _| true);
        assert_eq!(new, Position::new(4, 3));
    }

    // -----------------------------------------------------------------------
    // Test 17: drag_ball_pure stuck on impassable
    // -----------------------------------------------------------------------
    #[test]
    fn drag_ball_pure_stuck() {
        let ball = Position::new(3, 3);
        let target = Position::new(6, 3);
        let new = drag_ball_pure(ball, target, |_, _| false);
        assert_eq!(new, Position::new(3, 3));
    }
}
