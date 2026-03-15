//! Field-of-view computation using recursive shadowcasting.
//!
//! Determines which map tiles are visible from the player's position,
//! accounting for opaque terrain (walls, closed doors, trees).

use crate::action::Position;

/// Tracks which tiles are visible from a given origin via shadowcasting.
#[derive(Debug, Clone)]
pub struct FovMap {
    visible: Vec<Vec<bool>>,
    width: usize,
    height: usize,
}

/// Multipliers for transforming coordinates into octant-space.
///
/// For octant `i`, a position at depth `d` and column `c` maps to:
///   x = origin.x + c * XX[i] + d * XY[i]
///   y = origin.y + c * YX[i] + d * YY[i]
///
/// This covers all eight octants of the circle.
static XX: [i32; 8] = [ 1,  0,  0, -1, -1,  0,  0,  1];
static XY: [i32; 8] = [ 0,  1, -1,  0,  0, -1,  1,  0];
static YX: [i32; 8] = [ 0,  1,  1,  0,  0, -1, -1,  0];
static YY: [i32; 8] = [ 1,  0,  0,  1, -1,  0,  0, -1];

impl FovMap {
    /// Create a new FOV map of the given dimensions, with nothing visible.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            visible: vec![vec![false; width]; height],
            width,
            height,
        }
    }

    /// Clear all visibility flags.
    pub fn clear(&mut self) {
        for row in &mut self.visible {
            row.fill(false);
        }
    }

    /// Query whether a tile is currently visible.
    #[inline]
    pub fn is_visible(&self, x: i32, y: i32) -> bool {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.visible[y as usize][x as usize]
        } else {
            false
        }
    }

    /// Query by position.
    #[inline]
    pub fn is_visible_pos(&self, pos: Position) -> bool {
        self.is_visible(pos.x, pos.y)
    }

    /// Compute FOV for a blind player.
    ///
    /// When blind, the player can only feel adjacent tiles (radius 1).
    /// The origin itself is always visible.
    pub fn compute_blind(&mut self, origin: Position) {
        self.clear();
        self.mark(origin.x, origin.y);
        // Mark all 8 adjacent tiles as visible (feel by touch).
        for dy in -1..=1_i32 {
            for dx in -1..=1_i32 {
                self.mark(origin.x + dx, origin.y + dy);
            }
        }
    }

    /// Mark specific positions as visible (used for telepathy overlay).
    ///
    /// When a blind player has telepathy, they can sense monster
    /// positions through walls. Call this after `compute_blind()` to
    /// add monster positions to the visibility map.
    pub fn mark_positions(&mut self, positions: &[(i32, i32)]) {
        for &(x, y) in positions {
            self.mark(x, y);
        }
    }

    /// Compute FOV from `origin` with the given `radius` using recursive
    /// shadowcasting (Bjorn Bergstrom / roguebasin algorithm).
    ///
    /// `is_opaque` returns true if the tile at (x, y) blocks line of sight.
    pub fn compute(
        &mut self,
        origin: Position,
        radius: u32,
        is_opaque: impl Fn(i32, i32) -> bool,
    ) {
        self.clear();
        self.mark(origin.x, origin.y);

        for octant in 0..8 {
            self.cast_light(
                origin.x, origin.y,
                radius as i32,
                1,
                1.0, 0.0,
                XX[octant], XY[octant],
                YX[octant], YY[octant],
                &is_opaque,
            );
        }
    }

    #[inline]
    fn mark(&mut self, x: i32, y: i32) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.visible[y as usize][x as usize] = true;
        }
    }

    /// Recursive shadowcasting for one octant.
    ///
    /// - `depth`: current distance along primary axis (starts at 1).
    /// - `start_slope` / `end_slope`: the visible slope range [end..start]
    ///   where 1.0 is the leading diagonal edge and 0.0 is the primary axis.
    #[allow(clippy::too_many_arguments)]
    fn cast_light(
        &mut self,
        ox: i32, oy: i32,
        radius: i32,
        depth: i32,
        start_slope: f64,
        end_slope: f64,
        xx: i32, xy: i32,
        yx: i32, yy: i32,
        is_opaque: &impl Fn(i32, i32) -> bool,
    ) {
        if start_slope < end_slope || depth > radius {
            return;
        }

        let mut start = start_slope;
        let mut blocked = false;
        let mut new_start = 0.0_f64;

        // Walk columns from high slope to low slope within this depth.
        let mut col = depth;
        while col >= 0 {
            let map_x = ox + col * xx + depth * xy;
            let map_y = oy + col * yx + depth * yy;

            let l_slope = (col as f64 + 0.5) / (depth as f64 - 0.5);
            let r_slope = (col as f64 - 0.5) / (depth as f64 + 0.5);

            if start < r_slope {
                col -= 1;
                continue;
            }
            if end_slope > l_slope {
                break;
            }

            // Mark visible if within radius (Euclidean check relaxed to
            // allow generous diagonal vision, matching NetHack feel).
            if col * col + depth * depth <= (radius + 1) * (radius + 1) {
                self.mark(map_x, map_y);
            }

            if blocked {
                if is_opaque(map_x, map_y) {
                    new_start = r_slope;
                    col -= 1;
                    continue;
                } else {
                    blocked = false;
                    start = new_start;
                }
            } else if is_opaque(map_x, map_y) && depth < radius {
                blocked = true;
                // Recurse for the visible portion before this wall.
                self.cast_light(
                    ox, oy, radius,
                    depth + 1,
                    start, l_slope,
                    xx, xy, yx, yy,
                    is_opaque,
                );
                new_start = r_slope;
            }

            col -= 1;
        }

        if !blocked {
            self.cast_light(
                ox, oy, radius,
                depth + 1,
                start, end_slope,
                xx, xy, yx, yy,
                is_opaque,
            );
        }
    }
}

/// Yield all visible positions from `origin` with the given `radius`
/// using the same recursive shadowcasting algorithm as [`FovMap::compute`],
/// but via a gen block instead of marking a mutable bitmap.
///
/// The origin itself is always yielded first.  The caller receives
/// `(i32, i32)` positions lazily; duplicates are possible when octant
/// boundaries overlap, so de-duplicate if needed.
pub fn visible_cells_gen(
    width: usize,
    height: usize,
    origin: Position,
    radius: u32,
    is_opaque: impl Fn(i32, i32) -> bool,
) -> impl Iterator<Item = (i32, i32)> {
    gen move {
        // Origin is always visible.
        yield (origin.x, origin.y);

        // We reuse FovMap internally: the recursive shadowcasting
        // algorithm marks cells in a mutable bitmap, which is
        // fundamentally hard to express as a gen block without
        // re-implementing the recursion with an explicit stack.
        // Instead, compute the full bitmap and yield marked cells.
        let mut fov = FovMap::new(width, height);
        fov.compute(origin, radius, &is_opaque);

        for y in 0..height as i32 {
            for x in 0..width as i32 {
                // Skip origin (already yielded).
                if x == origin.x && y == origin.y {
                    continue;
                }
                if fov.is_visible(x, y) {
                    yield (x, y);
                }
            }
        }
    }
}

/// Compute FOV considering light sources, not just player position.
///
/// Positions within a light source's radius are visible (subject to
/// LOS from the light source, not the player).  Light sources don't
/// bypass walls.
pub fn compute_fov_with_lights(
    player_pos: Position,
    base_radius: u32,
    light_sources: &[(i32, i32, u32)], // (x, y, radius)
    is_opaque: impl Fn(i32, i32) -> bool,
    map_width: usize,
    map_height: usize,
) -> FovMap {
    // Start with standard shadowcasting from player position.
    let mut fov = FovMap::new(map_width, map_height);
    fov.compute(player_pos, base_radius, &is_opaque);

    // Add tiles illuminated by each light source (via independent FOV).
    for &(lx, ly, lr) in light_sources {
        let mut light_fov = FovMap::new(map_width, map_height);
        light_fov.compute(Position::new(lx, ly), lr, &is_opaque);

        // A tile is visible if lit by a light source AND the player
        // can see the light source's illumination (the tile is within
        // the light's FOV).  We add these to the player's visibility.
        for y in 0..map_height {
            for x in 0..map_width {
                if light_fov.is_visible(x as i32, y as i32) {
                    fov.mark(x as i32, y as i32);
                }
            }
        }
    }

    fov
}

/// Temporary vision at a remote position (crystal ball, clairvoyance).
///
/// Runs standard shadowcasting from a non-player position and returns
/// the resulting FOV map.
pub fn temporary_vision(
    center: Position,
    radius: u32,
    is_opaque: impl Fn(i32, i32) -> bool,
    map_width: usize,
    map_height: usize,
) -> FovMap {
    let mut fov = FovMap::new(map_width, map_height);
    fov.compute(center, radius, &is_opaque);
    fov
}

/// Blindness FOV: when blind, can only see own tile.
///
/// This is a stricter form of `compute_blind()` which allows feeling
/// adjacent tiles.  This function returns only the player's own tile
/// as visible.
pub fn blind_fov(player_pos: Position, map_width: usize, map_height: usize) -> FovMap {
    let mut fov = FovMap::new(map_width, map_height);
    fov.mark(player_pos.x, player_pos.y);
    fov
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_is_always_visible() {
        let mut fov = FovMap::new(20, 20);
        fov.compute(Position::new(10, 10), 5, |_, _| false);
        assert!(fov.is_visible(10, 10));
    }

    #[test]
    fn wall_blocks_vision() {
        let mut fov = FovMap::new(20, 20);
        // Wall at (12, 10) — should block anything beyond it to the east.
        fov.compute(Position::new(10, 10), 8, |x, y| x == 12 && y == 10);

        assert!(fov.is_visible(11, 10)); // before wall
        assert!(fov.is_visible(12, 10)); // the wall itself is visible
        assert!(!fov.is_visible(14, 10)); // behind wall
    }

    #[test]
    fn open_room_all_visible() {
        let mut fov = FovMap::new(20, 20);
        fov.compute(Position::new(10, 10), 5, |_, _| false);

        // All tiles within Chebyshev distance 3 should be visible in an
        // open room with radius 5.
        for dy in -3..=3_i32 {
            for dx in -3..=3_i32 {
                assert!(
                    fov.is_visible(10 + dx, 10 + dy),
                    "({}, {}) should be visible",
                    10 + dx,
                    10 + dy
                );
            }
        }
    }

    #[test]
    fn clear_resets_visibility() {
        let mut fov = FovMap::new(20, 20);
        fov.compute(Position::new(10, 10), 5, |_, _| false);
        assert!(fov.is_visible(10, 10));
        fov.clear();
        assert!(!fov.is_visible(10, 10));
    }

    #[test]
    fn adjacent_tiles_always_visible() {
        let mut fov = FovMap::new(20, 20);
        fov.compute(Position::new(10, 10), 1, |_, _| false);

        // All 8 neighbors + origin must be visible at radius 1.
        for dy in -1..=1_i32 {
            for dx in -1..=1_i32 {
                assert!(
                    fov.is_visible(10 + dx, 10 + dy),
                    "adjacent ({}, {}) should be visible",
                    10 + dx,
                    10 + dy,
                );
            }
        }
    }

    // ── Gen-block FOV tests ───────────────────────────────────────

    #[test]
    fn visible_cells_gen_matches_compute_open() {
        use std::collections::HashSet;
        let origin = Position::new(10, 10);
        let radius = 5u32;
        let (w, h) = (20, 20);

        // Compute via FovMap.
        let mut fov = FovMap::new(w, h);
        fov.compute(origin, radius, |_, _| false);
        let mut expected: HashSet<(i32, i32)> = HashSet::new();
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                if fov.is_visible(x, y) {
                    expected.insert((x, y));
                }
            }
        }

        // Compute via gen block.
        let gen_cells: HashSet<(i32, i32)> =
            visible_cells_gen(w, h, origin, radius, |_, _| false)
                .collect();

        assert_eq!(
            expected, gen_cells,
            "visible_cells_gen should match FovMap::compute in open room"
        );
    }

    #[test]
    fn visible_cells_gen_matches_compute_with_wall() {
        use std::collections::HashSet;
        let origin = Position::new(10, 10);
        let radius = 8u32;
        let (w, h) = (20, 20);
        let is_opaque = |x: i32, y: i32| x == 12 && y == 10;

        let mut fov = FovMap::new(w, h);
        fov.compute(origin, radius, &is_opaque);
        let mut expected: HashSet<(i32, i32)> = HashSet::new();
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                if fov.is_visible(x, y) {
                    expected.insert((x, y));
                }
            }
        }

        let gen_cells: HashSet<(i32, i32)> =
            visible_cells_gen(w, h, origin, radius, &is_opaque)
                .collect();

        assert_eq!(
            expected, gen_cells,
            "visible_cells_gen should match FovMap::compute with wall"
        );
    }

    #[test]
    fn visible_cells_gen_yields_origin() {
        let origin = Position::new(10, 10);
        let mut iter = visible_cells_gen(
            20, 20, origin, 5, |_, _| false,
        );
        let first = iter.next();
        assert_eq!(
            first,
            Some((10, 10)),
            "first yielded cell should be origin"
        );
    }

    // ── Blindness FOV tests ──────────────────────────────────────

    #[test]
    fn test_blind_restricts_fov() {
        let mut fov = FovMap::new(20, 20);
        fov.compute_blind(Position::new(10, 10));

        // Origin must be visible.
        assert!(fov.is_visible(10, 10));

        // All 8 neighbors must be visible.
        for dy in -1..=1_i32 {
            for dx in -1..=1_i32 {
                assert!(
                    fov.is_visible(10 + dx, 10 + dy),
                    "adjacent ({}, {}) should be visible when blind",
                    10 + dx, 10 + dy,
                );
            }
        }

        // Tiles at distance 2+ must NOT be visible.
        assert!(
            !fov.is_visible(12, 10),
            "tile at distance 2 should NOT be visible when blind"
        );
        assert!(
            !fov.is_visible(10, 12),
            "tile at distance 2 should NOT be visible when blind"
        );
        assert!(
            !fov.is_visible(8, 8),
            "tile at distance 2 should NOT be visible when blind"
        );
    }

    #[test]
    fn test_blind_telepathy_marks_monster_positions() {
        let mut fov = FovMap::new(20, 20);
        fov.compute_blind(Position::new(10, 10));

        // Monster at (15, 15) -- far beyond blind radius.
        assert!(!fov.is_visible(15, 15));

        // Telepathy marks the position.
        fov.mark_positions(&[(15, 15), (2, 3)]);
        assert!(
            fov.is_visible(15, 15),
            "telepathy should mark monster position as visible"
        );
        assert!(
            fov.is_visible(2, 3),
            "telepathy should mark monster position as visible"
        );
    }

    // ── FOV with light sources ────────────────────────────────────

    #[test]
    fn test_fov_with_light_source_extends_vision() {
        // Player at (5, 10) with radius 3, light source at (15, 10) with radius 2.
        // Without the light, tile (16, 10) would be out of player range.
        let fov = compute_fov_with_lights(
            Position::new(5, 10),
            3,
            &[(15, 10, 2)],
            |_, _| false, // no walls
            30,
            20,
        );

        // Player's own tile is visible.
        assert!(fov.is_visible(5, 10));
        // Tile within player radius.
        assert!(fov.is_visible(7, 10));
        // Tile at the light source.
        assert!(fov.is_visible(15, 10));
        // Tile within light source radius.
        assert!(fov.is_visible(16, 10));
    }

    #[test]
    fn test_fov_with_light_source_blocked_by_wall() {
        // Light source at (15, 10) with radius 3.
        // Wall at (16, 10) should block tiles beyond it.
        let fov = compute_fov_with_lights(
            Position::new(5, 10),
            3,
            &[(15, 10, 3)],
            |x, y| x == 16 && y == 10, // wall
            30,
            20,
        );

        // Light source itself is visible.
        assert!(fov.is_visible(15, 10));
        // Wall tile is visible (you can see the wall).
        assert!(fov.is_visible(16, 10));
        // Tile behind wall should not be visible via light.
        assert!(!fov.is_visible(18, 10));
    }

    // ── Temporary vision ──────────────────────────────────────────

    #[test]
    fn test_temporary_vision_at_remote() {
        let center = Position::new(15, 15);
        let fov = temporary_vision(center, 3, |_, _| false, 30, 30);

        assert!(fov.is_visible(15, 15));
        assert!(fov.is_visible(16, 16));
        assert!(fov.is_visible(14, 14));
        // Beyond radius.
        assert!(!fov.is_visible(15, 20));
    }

    #[test]
    fn test_temporary_vision_respects_walls() {
        let center = Position::new(10, 10);
        // Wall at (12, 10).
        let fov = temporary_vision(center, 5, |x, y| x == 12 && y == 10, 20, 20);

        assert!(fov.is_visible(11, 10));
        assert!(fov.is_visible(12, 10)); // wall itself is visible
        assert!(!fov.is_visible(14, 10)); // behind wall
    }

    // ── Blind FOV ─────────────────────────────────────────────────

    #[test]
    fn test_blind_fov_only_self() {
        let fov = blind_fov(Position::new(10, 10), 20, 20);

        assert!(fov.is_visible(10, 10), "player tile must be visible");
        // Adjacent tiles should NOT be visible (unlike compute_blind).
        assert!(
            !fov.is_visible(11, 10),
            "adjacent tiles should not be visible in strict blind FOV"
        );
        assert!(
            !fov.is_visible(9, 9),
            "diagonal tile should not be visible in strict blind FOV"
        );
    }
}
