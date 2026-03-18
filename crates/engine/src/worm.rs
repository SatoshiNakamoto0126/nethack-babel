//! Long worm tail mechanics for NetHack Babel.
//!
//! Long worms have segmented tails that follow the head in snake-like
//! fashion.  Each experience level adds one segment (up to ~20).
//! Hitting a tail segment can sever it, dropping a corpse and potentially
//! splitting the worm.
//!
//! All functions are pure: they operate on `GameWorld`, mutate world
//! state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use serde::{Deserialize, Serialize};

use crate::action::Position;
use crate::event::EngineEvent;
use crate::world::{GameWorld, HitPoints, Positioned};

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Component: the ordered list of tail segment positions for a long worm.
///
/// `segments[0]` is the segment closest to the head; the last element
/// is the tip of the tail.  The head position itself is stored in the
/// entity's `Positioned` component and is NOT duplicated here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WormSegments {
    pub segments: Vec<Position>,
}

impl WormSegments {
    /// Create a new empty worm tail (head-only worm).
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }
}

impl Default for WormSegments {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

/// Return the total length of the worm (head + tail segments).
pub fn worm_length(world: &GameWorld, worm: Entity) -> usize {
    let tail_len = world
        .get_component::<WormSegments>(worm)
        .map(|ws| ws.segments.len())
        .unwrap_or(0);
    // +1 for the head itself.
    1 + tail_len
}

// ---------------------------------------------------------------------------
// Movement
// ---------------------------------------------------------------------------

/// Move the worm head to `new_head_pos`.  The tail follows: every
/// segment shifts one position toward the head, and the segment
/// closest to the head takes the old head position.
///
/// Returns events describing the move.
pub fn move_worm(world: &mut GameWorld, worm: Entity, new_head_pos: Position) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let old_head_pos = match world.get_component::<Positioned>(worm) {
        Some(p) => p.0,
        None => return events,
    };

    // Update head position.
    if let Some(mut pos) = world.get_component_mut::<Positioned>(worm) {
        pos.0 = new_head_pos;
    }

    events.push(EngineEvent::EntityMoved {
        entity: worm,
        from: old_head_pos,
        to: new_head_pos,
    });

    // Shift tail segments: old head position becomes segments[0],
    // each subsequent segment takes the position of its predecessor.
    if let Some(mut ws) = world.get_component_mut::<WormSegments>(worm)
        && !ws.segments.is_empty()
    {
        // Walk from the tail tip toward the head, shifting each
        // segment to the position of the one in front of it.
        let len = ws.segments.len();
        for i in (1..len).rev() {
            ws.segments[i] = ws.segments[i - 1];
        }
        ws.segments[0] = old_head_pos;
    }

    events
}

// ---------------------------------------------------------------------------
// Growth / shrinkage
// ---------------------------------------------------------------------------

/// Grow the worm by adding one segment at the tail end.
///
/// The new segment appears at the same position as the current tail
/// tip (or at the head position if there are no segments yet).
pub fn grow_worm(world: &mut GameWorld, worm: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let tail_pos = {
        let ws = world.get_component::<WormSegments>(worm);
        let head_pos = world
            .get_component::<Positioned>(worm)
            .map(|p| p.0)
            .unwrap_or(Position::new(0, 0));
        ws.map(|w| w.segments.last().copied().unwrap_or(head_pos))
            .unwrap_or(head_pos)
    };

    if let Some(mut ws) = world.get_component_mut::<WormSegments>(worm) {
        ws.segments.push(tail_pos);
    }

    events.push(EngineEvent::msg("worm-grows"));
    events
}

/// Shrink the worm by removing the last tail segment.
///
/// Returns events describing the shrinkage.  If there are no segments
/// to remove, this is a no-op.
pub fn shrink_worm(world: &mut GameWorld, worm: Entity) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let removed = if let Some(mut ws) = world.get_component_mut::<WormSegments>(worm) {
        ws.segments.pop().is_some()
    } else {
        false
    };

    if removed {
        events.push(EngineEvent::msg("worm-shrinks"));
    }
    events
}

// ---------------------------------------------------------------------------
// Tail damage
// ---------------------------------------------------------------------------

/// Hit a specific tail segment of the worm.
///
/// If `damage` >= the worm's remaining HP at that segment (simplified:
/// we just always sever), the segment and everything behind it is
/// removed.  In NetHack, severing can create a new worm entity from
/// the severed portion; here we drop the severed segments and emit
/// events.
///
/// Returns events describing the tail hit.  `segment_idx` is 0-based
/// into `WormSegments.segments`.
pub fn hit_worm_tail(
    world: &mut GameWorld,
    worm: Entity,
    segment_idx: usize,
    damage: u32,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Apply damage to the worm's HP.
    let died = if let Some(mut hp) = world.get_component_mut::<HitPoints>(worm) {
        hp.current -= damage as i32;
        hp.current <= 0
    } else {
        false
    };

    events.push(EngineEvent::ExtraDamage {
        target: worm,
        amount: damage,
        source: crate::event::DamageSource::Melee,
    });

    // Sever the tail at segment_idx: remove segments[segment_idx..].
    let severed_count = if let Some(mut ws) = world.get_component_mut::<WormSegments>(worm) {
        if segment_idx < ws.segments.len() {
            let count = ws.segments.len() - segment_idx;
            ws.segments.truncate(segment_idx);
            count
        } else {
            0
        }
    } else {
        0
    };

    if severed_count > 0 {
        events.push(EngineEvent::msg_with(
            "worm-tail-severed",
            vec![("segments", severed_count.to_string())],
        ));
    }

    if died {
        events.push(EngineEvent::EntityDied {
            entity: worm,
            killer: None,
            cause: crate::event::DeathCause::KilledBy {
                killer_name: "tail hit".to_string(),
            },
        });
    }

    events
}

// ---------------------------------------------------------------------------
// Standalone worm body (non-ECS, for flexible use)
// ---------------------------------------------------------------------------

/// A standalone long worm body with segmented tail.
///
/// Unlike `WormSegments` (which is an ECS component attached to an entity),
/// `WormBody` is a self-contained struct useful for algorithms that need
/// to manipulate worm geometry without touching the ECS (e.g., AI pathfinding,
/// serialization, or creating split worms).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WormBody {
    /// Position of the worm's head.
    pub head_pos: Position,
    /// Ordered segments from head to tail (segments[0] is closest to head).
    pub segments: Vec<Position>,
    /// Maximum allowed segments (growth cap).
    pub max_length: usize,
}

impl WormBody {
    /// Create a new worm body at the given head position.
    pub fn new(head: Position, initial_length: usize) -> Self {
        Self {
            head_pos: head,
            segments: Vec::new(),
            max_length: initial_length,
        }
    }

    /// Move the worm head to a new position; tail follows.
    pub fn move_head(&mut self, new_head: Position) {
        self.segments.insert(0, self.head_pos);
        self.head_pos = new_head;
        while self.segments.len() > self.max_length {
            self.segments.pop();
        }
    }

    /// Grow the worm by increasing its max length by one.
    pub fn grow(&mut self) {
        self.max_length += 1;
    }

    /// Shrink the worm by removing the last tail segment.
    /// Returns the removed segment position, if any.
    pub fn shrink(&mut self) -> Option<Position> {
        if self.max_length > 0 {
            self.max_length -= 1;
        }
        self.segments.pop()
    }

    /// Cut the worm at a specific segment index.
    ///
    /// Segments at and beyond `segment_index` are removed from this worm
    /// and returned as a new `WormBody` (the tail portion). The first
    /// segment of the removed portion becomes the new worm's head.
    ///
    /// Returns `None` if the index is out of range or would produce an
    /// empty tail.
    pub fn cut_at(&mut self, segment_index: usize) -> Option<WormBody> {
        if segment_index >= self.segments.len() {
            return None;
        }
        let tail_segments = self.segments.split_off(segment_index);
        if tail_segments.is_empty() {
            return None;
        }
        let tail_head = tail_segments[0];
        let remaining = tail_segments[1..].to_vec();
        let max_len = remaining.len();
        Some(WormBody {
            head_pos: tail_head,
            segments: remaining,
            max_length: max_len,
        })
    }

    /// Check if a position is occupied by any part of the worm (head or segments).
    pub fn occupies(&self, pos: Position) -> bool {
        self.head_pos == pos || self.segments.contains(&pos)
    }

    /// Get the tail tip position (last segment, or head if no segments).
    pub fn tail_pos(&self) -> Position {
        self.segments.last().copied().unwrap_or(self.head_pos)
    }

    /// Total length including head.
    pub fn length(&self) -> usize {
        1 + self.segments.len()
    }
}

/// Result of hitting a worm segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WormHitResult {
    /// Segment was hit but worm remains intact (damage insufficient).
    Hit { segments_remaining: usize },
    /// Worm was severed; tail portion split off.
    Severed {
        segments_remaining: usize,
        severed_count: usize,
    },
    /// Worm died from the damage.
    Killed,
}

/// Hit a specific segment of a standalone worm body.
///
/// If damage is sufficient (>= 5), the worm is cut at that segment.
/// If the worm's effective HP (approximated as `length * 4`) is exceeded
/// by total damage, the worm is killed.
pub fn hit_worm_segment(worm: &mut WormBody, segment_pos: Position, damage: i32) -> WormHitResult {
    // Find the segment index.
    let idx = worm.segments.iter().position(|&p| p == segment_pos);

    // If damage is lethal (worm total HP approximated as length * 4).
    let effective_hp = (worm.length() * 4) as i32;
    if damage >= effective_hp {
        worm.segments.clear();
        return WormHitResult::Killed;
    }

    // Cut if damage >= 5 and we found the segment.
    if damage >= 5
        && let Some(idx) = idx
    {
        let severed = worm.segments.len() - idx;
        worm.segments.truncate(idx);
        if worm.max_length > worm.segments.len() {
            worm.max_length = worm.segments.len();
        }
        return WormHitResult::Severed {
            segments_remaining: worm.segments.len(),
            severed_count: severed,
        };
    }

    WormHitResult::Hit {
        segments_remaining: worm.segments.len(),
    }
}

// ---------------------------------------------------------------------------
// ECS query helpers
// ---------------------------------------------------------------------------

/// Check if any part of a worm entity occupies the given position.
pub fn worm_occupies(world: &GameWorld, worm: Entity, pos: Position) -> bool {
    // Check head.
    if let Some(p) = world.get_component::<Positioned>(worm)
        && p.0 == pos
    {
        return true;
    }
    // Check segments.
    if let Some(ws) = world.get_component::<WormSegments>(worm) {
        return ws.segments.contains(&pos);
    }
    false
}

/// Get the tail tip position of a worm entity.
pub fn worm_tail_pos(world: &GameWorld, worm: Entity) -> Option<Position> {
    let head = world.get_component::<Positioned>(worm).map(|p| p.0)?;
    let tail = world
        .get_component::<WormSegments>(worm)
        .and_then(|ws| ws.segments.last().copied())
        .unwrap_or(head);
    Some(tail)
}

/// Cut a worm entity at a specific segment index, removing segments
/// at and beyond that index. Returns the positions of removed segments.
pub fn cut_worm_at(world: &mut GameWorld, worm: Entity, segment_idx: usize) -> Vec<Position> {
    if let Some(mut ws) = world.get_component_mut::<WormSegments>(worm)
        && segment_idx < ws.segments.len()
    {
        let removed = ws.segments.split_off(segment_idx);
        return removed;
    }
    Vec::new()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_worm(world: &mut GameWorld, pos: Position) -> Entity {
        let worm = world.spawn((
            Positioned(pos),
            HitPoints {
                current: 20,
                max: 20,
            },
            WormSegments::new(),
        ));
        worm
    }

    #[test]
    fn new_worm_has_length_one() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let worm = setup_worm(&mut world, Position::new(10, 5));
        assert_eq!(worm_length(&world, worm), 1);
    }

    #[test]
    fn grow_increases_length() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let worm = setup_worm(&mut world, Position::new(10, 5));
        grow_worm(&mut world, worm);
        assert_eq!(worm_length(&world, worm), 2);
        grow_worm(&mut world, worm);
        assert_eq!(worm_length(&world, worm), 3);
    }

    #[test]
    fn shrink_decreases_length() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let worm = setup_worm(&mut world, Position::new(10, 5));
        grow_worm(&mut world, worm);
        grow_worm(&mut world, worm);
        assert_eq!(worm_length(&world, worm), 3);
        shrink_worm(&mut world, worm);
        assert_eq!(worm_length(&world, worm), 2);
        shrink_worm(&mut world, worm);
        assert_eq!(worm_length(&world, worm), 1);
        // Shrinking below 1 (head-only) is a no-op.
        let events = shrink_worm(&mut world, worm);
        assert_eq!(worm_length(&world, worm), 1);
        assert!(events.is_empty());
    }

    #[test]
    fn move_worm_shifts_tail() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let worm = setup_worm(&mut world, Position::new(10, 5));
        // Grow to 3 segments (head + 2 tail).
        grow_worm(&mut world, worm);
        grow_worm(&mut world, worm);

        // Move head from (10,5) to (11,5).
        move_worm(&mut world, worm, Position::new(11, 5));
        let pos = world.get_component::<Positioned>(worm).unwrap();
        assert_eq!(pos.0, Position::new(11, 5));

        let ws = world.get_component::<WormSegments>(worm).unwrap();
        // segments[0] should now be the old head position.
        assert_eq!(ws.segments[0], Position::new(10, 5));
    }

    #[test]
    fn hit_tail_severs_segments() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let worm = setup_worm(&mut world, Position::new(10, 5));
        // Build up 5 segments.
        for _ in 0..5 {
            grow_worm(&mut world, worm);
        }
        assert_eq!(worm_length(&world, worm), 6); // head + 5

        // Sever at segment 2 (removes segments 2,3,4 = 3 segments).
        let events = hit_worm_tail(&mut world, worm, 2, 5);
        assert_eq!(worm_length(&world, worm), 3); // head + 2
        // Should have ExtraDamage and tail-severed messages.
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::ExtraDamage { .. }))
        );
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "worm-tail-severed"
        )));
    }

    #[test]
    fn hit_tail_kills_worm() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let worm = setup_worm(&mut world, Position::new(10, 5));
        grow_worm(&mut world, worm);

        // Deal lethal damage.
        let events = hit_worm_tail(&mut world, worm, 0, 999);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::EntityDied { .. }))
        );
    }

    // ── WormBody standalone tests ──

    #[test]
    fn worm_body_move_head() {
        let mut wb = WormBody::new(Position::new(5, 5), 3);
        assert_eq!(wb.length(), 1);

        // Move a few times to build up segments.
        wb.move_head(Position::new(6, 5));
        assert_eq!(wb.head_pos, Position::new(6, 5));
        assert_eq!(wb.segments.len(), 1);
        assert_eq!(wb.segments[0], Position::new(5, 5));

        wb.move_head(Position::new(7, 5));
        assert_eq!(wb.segments.len(), 2);
        assert_eq!(wb.segments[0], Position::new(6, 5));
        assert_eq!(wb.segments[1], Position::new(5, 5));

        wb.move_head(Position::new(8, 5));
        assert_eq!(wb.segments.len(), 3);

        // Next move should drop the oldest segment (max_length = 3).
        wb.move_head(Position::new(9, 5));
        assert_eq!(wb.segments.len(), 3);
        assert_eq!(wb.segments[0], Position::new(8, 5));
        assert_eq!(*wb.segments.last().unwrap(), Position::new(6, 5));
    }

    #[test]
    fn worm_body_grow_and_shrink() {
        let mut wb = WormBody::new(Position::new(5, 5), 2);
        wb.move_head(Position::new(6, 5));
        wb.move_head(Position::new(7, 5));
        assert_eq!(wb.segments.len(), 2);

        // Grow: next move won't drop tail.
        wb.grow();
        assert_eq!(wb.max_length, 3);
        wb.move_head(Position::new(8, 5));
        assert_eq!(wb.segments.len(), 3);

        // Shrink removes tail.
        let removed = wb.shrink();
        assert!(removed.is_some());
        assert_eq!(wb.segments.len(), 2);
    }

    #[test]
    fn worm_body_cut_at() {
        let mut wb = WormBody::new(Position::new(10, 5), 5);
        // Build segments by moving.
        for i in 1..=5 {
            wb.move_head(Position::new(10 + i, 5));
        }
        assert_eq!(wb.length(), 6); // head + 5 segments

        // Cut at segment 2: keeps segments[0..2], removes segments[2..5].
        let tail = wb.cut_at(2);
        assert!(tail.is_some());
        let tail = tail.unwrap();
        assert_eq!(wb.segments.len(), 2);
        assert_eq!(tail.length(), 3); // new head + 2 remaining segments
    }

    #[test]
    fn worm_body_cut_at_out_of_range() {
        let mut wb = WormBody::new(Position::new(5, 5), 3);
        wb.move_head(Position::new(6, 5));
        assert_eq!(wb.segments.len(), 1);

        // Cut at index beyond range.
        let tail = wb.cut_at(5);
        assert!(tail.is_none());
    }

    #[test]
    fn worm_body_occupies() {
        let mut wb = WormBody::new(Position::new(5, 5), 3);
        wb.move_head(Position::new(6, 5));
        wb.move_head(Position::new(7, 5));

        assert!(wb.occupies(Position::new(7, 5))); // head
        assert!(wb.occupies(Position::new(6, 5))); // segment
        assert!(wb.occupies(Position::new(5, 5))); // segment
        assert!(!wb.occupies(Position::new(8, 5))); // not occupied
    }

    #[test]
    fn worm_body_tail_pos() {
        let mut wb = WormBody::new(Position::new(5, 5), 3);
        // No segments: tail is head.
        assert_eq!(wb.tail_pos(), Position::new(5, 5));

        wb.move_head(Position::new(6, 5));
        assert_eq!(wb.tail_pos(), Position::new(5, 5));

        wb.move_head(Position::new(7, 5));
        assert_eq!(wb.tail_pos(), Position::new(5, 5));
    }

    #[test]
    fn hit_worm_segment_severs() {
        let mut wb = WormBody::new(Position::new(10, 5), 5);
        for i in 1..=4 {
            wb.move_head(Position::new(10 + i, 5));
        }
        assert_eq!(wb.length(), 5); // head + 4 segments

        // Hit segment at position (12, 5) with enough damage to cut.
        let result = hit_worm_segment(&mut wb, Position::new(12, 5), 5);
        match result {
            WormHitResult::Severed {
                segments_remaining,
                severed_count,
            } => {
                assert!(severed_count > 0);
                assert!(segments_remaining < 4);
            }
            other => panic!("expected Severed, got {other:?}"),
        }
    }

    #[test]
    fn hit_worm_segment_kills() {
        let mut wb = WormBody::new(Position::new(5, 5), 2);
        wb.move_head(Position::new(6, 5));
        // length = 2, effective HP = 8.
        let result = hit_worm_segment(&mut wb, Position::new(5, 5), 100);
        assert_eq!(result, WormHitResult::Killed);
    }

    // ── ECS worm query tests ──

    #[test]
    fn worm_occupies_checks_head_and_segments() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let worm = setup_worm(&mut world, Position::new(10, 5));
        grow_worm(&mut world, worm);
        move_worm(&mut world, worm, Position::new(11, 5));

        assert!(worm_occupies(&world, worm, Position::new(11, 5))); // head
        assert!(worm_occupies(&world, worm, Position::new(10, 5))); // segment
        assert!(!worm_occupies(&world, worm, Position::new(12, 5))); // empty
    }

    #[test]
    fn worm_tail_pos_returns_tip() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let worm = setup_worm(&mut world, Position::new(10, 5));

        // Head-only worm: tail is head.
        assert_eq!(worm_tail_pos(&world, worm), Some(Position::new(10, 5)));

        grow_worm(&mut world, worm);
        move_worm(&mut world, worm, Position::new(11, 5));

        // After growing and moving, tail should be the old position.
        let tail = worm_tail_pos(&world, worm).unwrap();
        assert_eq!(tail, Position::new(10, 5));
    }

    #[test]
    fn cut_worm_at_removes_segments() {
        let mut world = GameWorld::new(Position::new(40, 10));
        let worm = setup_worm(&mut world, Position::new(10, 5));
        for _ in 0..5 {
            grow_worm(&mut world, worm);
        }
        assert_eq!(worm_length(&world, worm), 6);

        let removed = cut_worm_at(&mut world, worm, 2);
        assert_eq!(removed.len(), 3); // segments 2,3,4
        assert_eq!(worm_length(&world, worm), 3); // head + 2
    }
}
