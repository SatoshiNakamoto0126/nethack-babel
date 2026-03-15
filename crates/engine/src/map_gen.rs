//! Dungeon level map generation.
//!
//! Produces playable dungeon levels with rooms, corridors, doors, and stairs.
//! The generator follows the classic NetHack algorithm described in the spec
//! (rooms + corridors style) but simplified for the initial implementation.

use rand::Rng;

use crate::action::Position;
use crate::dungeon::{LevelMap, Terrain};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A rectangular room on the map.
///
/// Coordinates refer to the *interior* floor area (walls sit one cell outside).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    /// Left column of the interior.
    pub x: usize,
    /// Top row of the interior.
    pub y: usize,
    /// Interior width (number of floor columns).
    pub width: usize,
    /// Interior height (number of floor rows).
    pub height: usize,
    /// Whether the room is lit.
    pub lit: bool,
}

impl Room {
    /// Right column of the interior (inclusive).
    pub fn right(&self) -> usize {
        self.x + self.width - 1
    }

    /// Bottom row of the interior (inclusive).
    pub fn bottom(&self) -> usize {
        self.y + self.height - 1
    }

    /// Centre of the room (rounded down).
    pub fn center(&self) -> (usize, usize) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    /// Whether `(px, py)` is inside the interior.
    pub fn contains(&self, px: usize, py: usize) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }

    /// Whether two rooms overlap *including* a 1-cell wall margin around each.
    pub fn overlaps_with_margin(&self, other: &Room, margin: usize) -> bool {
        let ax1 = self.x.saturating_sub(margin);
        let ay1 = self.y.saturating_sub(margin);
        let ax2 = self.right() + margin;
        let ay2 = self.bottom() + margin;

        let bx1 = other.x.saturating_sub(margin);
        let by1 = other.y.saturating_sub(margin);
        let bx2 = other.right() + margin;
        let by2 = other.bottom() + margin;

        ax1 <= bx2 && ax2 >= bx1 && ay1 <= by2 && ay2 >= by1
    }
}

// ---------------------------------------------------------------------------
// Generation result
// ---------------------------------------------------------------------------

/// The output of [`generate_level`]: a filled-in level map plus metadata.
#[derive(Debug)]
pub struct GeneratedLevel {
    pub map: LevelMap,
    pub rooms: Vec<Room>,
    pub up_stairs: Option<Position>,
    pub down_stairs: Option<Position>,
}

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

/// Generate a random dungeon level at the given `depth` (1-based).
///
/// The caller must supply a seeded RNG; this function never touches global
/// state.
pub fn generate_level(depth: u8, rng: &mut impl Rng) -> GeneratedLevel {
    let mut map = LevelMap::new_standard();
    let w = map.width;
    let h = map.height;

    // 1. Place rooms ----------------------------------------------------------
    let room_count = rng.random_range(3..=8u32).min(8) as usize;
    let mut rooms: Vec<Room> = Vec::with_capacity(room_count);

    for _ in 0..room_count * 30 {
        // 30x attempts per desired room
        if rooms.len() >= room_count {
            break;
        }
        if let Some(room) = try_place_room(w, h, depth, &rooms, rng) {
            rooms.push(room);
        }
    }

    // Sort rooms left-to-right (NetHack does this for corridor connectivity).
    rooms.sort_by_key(|r| r.x);

    // 2. Carve rooms into the map ---------------------------------------------
    for room in &rooms {
        carve_room(&mut map, room);
    }

    // 3. Connect rooms with corridors -----------------------------------------
    connect_rooms(&mut map, &rooms, rng);

    // 4. Place doors where corridors meet room walls --------------------------
    place_doors(&mut map, &rooms, depth, rng);

    // 5. Place stairs ---------------------------------------------------------
    let (up_stairs, down_stairs) = place_stairs(&mut map, &rooms, depth, rng);

    // 6. Special rooms and themed rooms ---------------------------------------
    if rooms.len() >= 2 {
        if let Some(room_type) =
            select_special_room(depth as u32, rooms.len(), false, 24, rng)
        {
            // Pick a room (not first or last, to avoid stair conflicts).
            let room_idx = if rooms.len() > 2 {
                rng.random_range(1..rooms.len() - 1)
            } else {
                rng.random_range(0..rooms.len())
            };
            populate_special_room(&mut map, &rooms[room_idx], room_type, rng);

            // Try themed room on another room.
            if rooms.len() >= 3 {
                let other_idx = (room_idx + 1) % rooms.len();
                maybe_apply_theme(&mut map, &rooms[other_idx], depth, rng);
            }
        } else if rooms.len() >= 3 {
            // No special room — still try a themed room.
            let idx = rng.random_range(1..rooms.len());
            maybe_apply_theme(&mut map, &rooms[idx], depth, rng);
        }
    }

    GeneratedLevel {
        map,
        rooms,
        up_stairs,
        down_stairs,
    }
}

// ---------------------------------------------------------------------------
// Room placement
// ---------------------------------------------------------------------------

/// Try to create a room that fits on the map without overlapping existing ones.
fn try_place_room(
    map_w: usize,
    map_h: usize,
    depth: u8,
    existing: &[Room],
    rng: &mut impl Rng,
) -> Option<Room> {
    // Random interior dimensions following NetHack's algorithm:
    //   width  = 2 + rn2(rect_width > 28 ? 12 : 8)  => [2, 13] or [2, 9]
    //   height = 2 + rn2(4)                           => [2, 5]
    //   if width * height > 50: height = 50 / width
    // We approximate rect_width > 28 as 50% probability since we don't
    // track free rectangles.
    let wide_rect = rng.random_bool(0.5);
    let width = 2 + rng.random_range(0..if wide_rect { 12u32 } else { 8 }) as usize;
    let mut height = 2 + rng.random_range(0..4u32) as usize;
    if width * height > 50 {
        height = 50 / width;
    }
    // Minimum dimensions: width >= 2, height >= 2 (guaranteed by formula).
    let width = width.max(2);
    let height = height.max(2);

    // We need room for walls around the interior (+1 on each side) plus a
    // 1-cell stone margin so rooms never touch the map border directly.
    // Minimum x: 2 (1 border + 1 wall). Maximum right wall: map_w - 2.
    let min_x = 2usize;
    let min_y = 2usize;
    let max_x = map_w.saturating_sub(width + 2);
    let max_y = map_h.saturating_sub(height + 2);
    if max_x < min_x || max_y < min_y {
        return None;
    }

    let x = rng.random_range(min_x..=max_x);
    let y = rng.random_range(min_y..=max_y);

    let room = Room {
        x,
        y,
        width,
        height,
        lit: room_is_lit(depth, rng),
    };

    // Overlap check — margin of 2 keeps walls from merging.
    for existing_room in existing {
        if room.overlaps_with_margin(existing_room, 2) {
            return None;
        }
    }

    Some(room)
}

/// Lighting probability: shallower levels are more likely to be lit.
///
/// NetHack formula: `rnd(1 + abs(depth)) < 11 && rn2(77)`
/// - `rnd(n)` returns uniform in `[1, n]`
/// - At depth 1: rnd(2) in [1,2], always < 11, then 76/77 chance lit
/// - At depth 10: rnd(11) in [1,10], all < 11, then 76/77 chance lit
/// - At depth 20: rnd(21) in [1,20], ~50% chance rnd(21) < 11
pub fn room_is_lit(depth: u8, rng: &mut impl Rng) -> bool {
    let d = depth.max(1) as u32;
    let roll = rng.random_range(1..=1 + d); // rnd(1 + abs(depth))
    roll < 11 && rng.random_range(0..77u32) != 0 // rn2(77) != 0
}

// ---------------------------------------------------------------------------
// Carving rooms
// ---------------------------------------------------------------------------

/// Write room walls and floor into the level map.
fn carve_room(map: &mut LevelMap, room: &Room) {
    let lx = room.x as i32 - 1;
    let ly = room.y as i32 - 1;
    let hx = room.right() as i32 + 1;
    let hy = room.bottom() as i32 + 1;

    // Corners
    map.set_terrain(Position::new(lx, ly), Terrain::Wall);
    map.set_terrain(Position::new(hx, ly), Terrain::Wall);
    map.set_terrain(Position::new(lx, hy), Terrain::Wall);
    map.set_terrain(Position::new(hx, hy), Terrain::Wall);

    // Horizontal walls (top and bottom)
    for x in room.x..=room.right() {
        map.set_terrain(Position::new(x as i32, ly), Terrain::Wall);
        map.set_terrain(Position::new(x as i32, hy), Terrain::Wall);
    }

    // Vertical walls (left and right)
    for y in room.y..=room.bottom() {
        map.set_terrain(Position::new(lx, y as i32), Terrain::Wall);
        map.set_terrain(Position::new(hx, y as i32), Terrain::Wall);
    }

    // Interior floor
    for y in room.y..=room.bottom() {
        for x in room.x..=room.right() {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }
}

// ---------------------------------------------------------------------------
// Corridor connection
// ---------------------------------------------------------------------------

/// Connect all rooms with corridors ensuring full connectivity.
fn connect_rooms(map: &mut LevelMap, rooms: &[Room], rng: &mut impl Rng) {
    if rooms.len() < 2 {
        return;
    }

    // Equivalence classes for connectivity (union-find with path compression).
    let n = rooms.len();
    let mut parent: Vec<usize> = (0..n).collect();

    let find = |parent: &mut Vec<usize>, mut i: usize| -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    };

    let union = |parent: &mut Vec<usize>, a: usize, b: usize| {
        let ra = {
            let mut i = a;
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        };
        let rb = {
            let mut i = b;
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        };
        if ra != rb {
            parent[ra.max(rb)] = ra.min(rb);
        }
    };

    // Pass 1: Connect sequential rooms (sorted left-to-right).
    for i in 0..n - 1 {
        dig_corridor_between(map, &rooms[i], &rooms[i + 1], rng);
        union(&mut parent, i, i + 1);
        // NetHack has a 1/50 chance to stop early; skip for reliability.
    }

    // Pass 2: Connect rooms two apart if not yet connected.
    for i in 0..n.saturating_sub(2) {
        let ra = find(&mut parent, i);
        let rb = find(&mut parent, i + 2);
        if ra != rb {
            dig_corridor_between(map, &rooms[i], &rooms[i + 2], rng);
            union(&mut parent, i, i + 2);
        }
    }

    // Pass 3: Guarantee full connectivity.
    loop {
        let mut merged = false;
        for i in 0..n {
            for j in i + 1..n {
                let ra = find(&mut parent, i);
                let rb = find(&mut parent, j);
                if ra != rb {
                    dig_corridor_between(map, &rooms[i], &rooms[j], rng);
                    union(&mut parent, i, j);
                    merged = true;
                }
            }
        }
        if !merged {
            break;
        }
    }

    // Pass 4: A few extra random corridors for variety.
    if n > 2 {
        let extras = rng.random_range(0..n as u32) + 2;
        for _ in 0..extras {
            let a = rng.random_range(0..n);
            let mut b = rng.random_range(0..n.saturating_sub(1));
            if b >= a {
                b += 1;
            }
            dig_corridor_between(map, &rooms[a], &rooms[b], rng);
        }
    }
}

/// Dig an L-shaped corridor between two rooms.
///
/// Picks a random interior point in each room as the source/destination,
/// then carves horizontally first, then vertically (or vice versa, chosen
/// randomly).
fn dig_corridor_between(map: &mut LevelMap, a: &Room, b: &Room, rng: &mut impl Rng) {
    let (ax, ay) = random_point_in_room(a, rng);
    let (bx, by) = random_point_in_room(b, rng);

    if rng.random_bool(0.5) {
        dig_h(map, ax, bx, ay);
        dig_v(map, ay, by, bx);
    } else {
        dig_v(map, ay, by, ax);
        dig_h(map, ax, bx, by);
    }
}

/// Return a random floor coordinate inside the room.
fn random_point_in_room(room: &Room, rng: &mut impl Rng) -> (i32, i32) {
    let x = rng.random_range(room.x..=room.right()) as i32;
    let y = rng.random_range(room.y..=room.bottom()) as i32;
    (x, y)
}

/// Carve a horizontal corridor segment, converting stone and wall cells to
/// corridor.
fn dig_h(map: &mut LevelMap, x1: i32, x2: i32, y: i32) {
    let (lo, hi) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
    for x in lo..=hi {
        let pos = Position::new(x, y);
        if let Some(cell) = map.get(pos) {
            match cell.terrain {
                Terrain::Stone | Terrain::Wall => {
                    map.set_terrain(pos, Terrain::Corridor);
                }
                _ => {} // don't overwrite floor, doors, etc.
            }
        }
    }
}

/// Carve a vertical corridor segment.
fn dig_v(map: &mut LevelMap, y1: i32, y2: i32, x: i32) {
    let (lo, hi) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
    for y in lo..=hi {
        let pos = Position::new(x, y);
        if let Some(cell) = map.get(pos) {
            match cell.terrain {
                Terrain::Stone | Terrain::Wall => {
                    map.set_terrain(pos, Terrain::Corridor);
                }
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Door placement
// ---------------------------------------------------------------------------

/// Place doors where corridors pass through room walls.
fn place_doors(map: &mut LevelMap, rooms: &[Room], depth: u8, rng: &mut impl Rng) {
    for room in rooms {
        let lx = room.x as i32 - 1;
        let ly = room.y as i32 - 1;
        let hx = room.right() as i32 + 1;
        let hy = room.bottom() as i32 + 1;

        // Scan top and bottom walls.
        for x in room.x..=room.right() {
            maybe_place_door(map, Position::new(x as i32, ly), depth, rng);
            maybe_place_door(map, Position::new(x as i32, hy), depth, rng);
        }
        // Scan left and right walls.
        for y in room.y..=room.bottom() {
            maybe_place_door(map, Position::new(lx, y as i32), depth, rng);
            maybe_place_door(map, Position::new(hx, y as i32), depth, rng);
        }
    }
}

/// If the cell at `pos` is a corridor (dug through a wall position), convert
/// it to a door.  The door state follows simplified NetHack probabilities.
fn maybe_place_door(map: &mut LevelMap, pos: Position, depth: u8, rng: &mut impl Rng) {
    let cell = match map.get(pos) {
        Some(c) => c,
        None => return,
    };

    if cell.terrain != Terrain::Corridor {
        return;
    }

    // Check that at least one cardinal neighbour is floor (confirms this
    // corridor cell sits on the room boundary).
    let has_floor_neighbour = [(0, -1), (0, 1), (-1, 0), (1, 0)]
        .iter()
        .any(|&(dx, dy)| {
            map.get(Position::new(pos.x + dx, pos.y + dy))
                .is_some_and(|c| c.terrain == Terrain::Floor)
        });

    if !has_floor_neighbour {
        return;
    }

    let door_terrain = choose_door_terrain(depth, rng);
    map.set_terrain(pos, door_terrain);
}

/// Pick a door terrain variant using simplified NetHack probabilities.
///
/// At depth > 2 there is a small chance of a secret door (rendered as wall
/// in the engine's simplified model).
fn choose_door_terrain(depth: u8, rng: &mut impl Rng) -> Terrain {
    // Secret door: 1/8 chance at depth > 2.
    if depth > 2 && rng.random_range(0..8u32) == 0 {
        // Secret doors look like walls until discovered.
        return Terrain::DoorClosed;
    }

    // 2/3 empty doorway (we model as open door).
    // 1/3 has a door panel.
    if rng.random_range(0..3u32) != 0 {
        return Terrain::DoorOpen; // doorway
    }

    // Door panel: 1/5 open, 1/6 of remainder locked, rest closed.
    if rng.random_range(0..5u32) == 0 {
        Terrain::DoorOpen
    } else if rng.random_range(0..6u32) == 0 {
        Terrain::DoorLocked
    } else {
        Terrain::DoorClosed
    }
}

// ---------------------------------------------------------------------------
// Stairs
// ---------------------------------------------------------------------------

/// Place up-stairs and down-stairs in different rooms.
///
/// Returns `(up_stairs, down_stairs)` positions.
fn place_stairs(
    map: &mut LevelMap,
    rooms: &[Room],
    depth: u8,
    rng: &mut impl Rng,
) -> (Option<Position>, Option<Position>) {
    if rooms.is_empty() {
        return (None, None);
    }

    let mut up_pos = None;
    let down_pos;

    // Up stairs (not on depth 1 — that's the surface).
    if depth > 1 {
        let idx = rng.random_range(0..rooms.len());
        let pos = random_floor_in_room(&rooms[idx], rng);
        map.set_terrain(pos, Terrain::StairsUp);
        up_pos = Some(pos);
    }

    // Down stairs: pick a different room if possible.
    {
        let mut idx = rng.random_range(0..rooms.len());
        // Try to avoid same room as up stairs.
        if rooms.len() > 1
            && let Some(up) = up_pos {
                for _ in 0..20 {
                    if !rooms[idx].contains(up.x as usize, up.y as usize) {
                        break;
                    }
                    idx = rng.random_range(0..rooms.len());
                }
            }
        let pos = random_floor_in_room(&rooms[idx], rng);
        map.set_terrain(pos, Terrain::StairsDown);
        down_pos = Some(pos);
    }

    (up_pos, down_pos)
}

/// Pick a random floor cell inside a room (returns a `Position`).
fn random_floor_in_room(room: &Room, rng: &mut impl Rng) -> Position {
    let x = rng.random_range(room.x..=room.right()) as i32;
    let y = rng.random_range(room.y..=room.bottom()) as i32;
    Position::new(x, y)
}

// ---------------------------------------------------------------------------
// Special room types
// ---------------------------------------------------------------------------

/// Types of special rooms that can be generated on a level.
///
/// Corresponds to NetHack's `mkroom.h` room type enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialRoomType {
    /// Ordinary room (default).
    ORoom,
    /// Throne room (court) with courtiers and a king.
    Court,
    /// Swamp with water and fungi.
    Swamp,
    /// Vault filled with gold (2x2 room).
    Vault,
    /// Beehive with killer bees and a queen.
    Beehive,
    /// Morgue with undead.
    Morgue,
    /// Barracks with soldiers.
    Barracks,
    /// Zoo with random monsters and items.
    Zoo,
    /// Temple with an aligned altar and priest.
    Temple,
    /// Leprechaun hall.
    LeprechaunHall,
    /// Cockatrice nest.
    CockatriceNest,
    /// Anthole with ants.
    Anthole,
    /// General or specific shop.
    Shop,
}

/// Select a special room type for the given depth, following NetHack's
/// priority-ordered evaluation from `mkroom.c`.
///
/// `u_depth` is the absolute dungeon depth.
/// `nroom` is the number of rooms on the level.
/// `has_branch` is true if a branch entrance exists on this level.
/// `medusa_depth` is the depth of the Medusa level (shop cutoff).
///
/// Returns `None` if no special room should be created.
///
/// The checks are evaluated in order; exactly zero or one special room
/// is created per level.
pub fn select_special_room(
    u_depth: u32,
    nroom: usize,
    has_branch: bool,
    medusa_depth: u32,
    rng: &mut impl Rng,
) -> Option<SpecialRoomType> {
    let threshold: usize = if has_branch { 4 } else { 3 };

    // Shop: depth > 1 and < medusa, enough rooms, and rn2(depth) < 3
    if u_depth > 1
        && u_depth < medusa_depth
        && nroom >= threshold
        && rng.random_range(0..u_depth) < 3
    {
        return Some(SpecialRoomType::Shop);
    }
    // Court: depth > 4, 1/6 chance
    if u_depth > 4 && rng.random_range(0..6u32) == 0 {
        return Some(SpecialRoomType::Court);
    }
    // Leprechaun hall: depth > 5, 1/8 chance
    if u_depth > 5 && rng.random_range(0..8u32) == 0 {
        return Some(SpecialRoomType::LeprechaunHall);
    }
    // Zoo: depth > 6, 1/7 chance
    if u_depth > 6 && rng.random_range(0..7u32) == 0 {
        return Some(SpecialRoomType::Zoo);
    }
    // Temple: depth > 8, 1/5 chance
    if u_depth > 8 && rng.random_range(0..5u32) == 0 {
        return Some(SpecialRoomType::Temple);
    }
    // Beehive: depth > 9, 1/5 chance
    if u_depth > 9 && rng.random_range(0..5u32) == 0 {
        return Some(SpecialRoomType::Beehive);
    }
    // Morgue: depth > 11, 1/6 chance
    if u_depth > 11 && rng.random_range(0..6u32) == 0 {
        return Some(SpecialRoomType::Morgue);
    }
    // Anthole: depth > 12, 1/8 chance
    if u_depth > 12 && rng.random_range(0..8u32) == 0 {
        return Some(SpecialRoomType::Anthole);
    }
    // Barracks: depth > 14, 1/4 chance
    if u_depth > 14 && rng.random_range(0..4u32) == 0 {
        return Some(SpecialRoomType::Barracks);
    }
    // Swamp: depth > 15, 1/6 chance
    if u_depth > 15 && rng.random_range(0..6u32) == 0 {
        return Some(SpecialRoomType::Swamp);
    }
    // Cockatrice nest: depth > 16, 1/8 chance
    if u_depth > 16 && rng.random_range(0..8u32) == 0 {
        return Some(SpecialRoomType::CockatriceNest);
    }

    None
}

// ---------------------------------------------------------------------------
// Door state (detailed model for testing)
// ---------------------------------------------------------------------------

/// Detailed door state for spec-conformance testing.
///
/// Maps to NetHack's `dosdoor()` output: `D_NODOOR`, `D_ISOPEN`,
/// `D_CLOSED`, `D_LOCKED`, and `D_TRAPPED` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DoorState {
    NoDoor,
    Open,
    Closed,
    Locked,
    ClosedTrapped,
    LockedTrapped,
    SecretClosed,
    SecretLocked,
}

/// Choose a door state following NetHack's `dodoor()` + `dosdoor()`.
///
/// `is_secret`: whether this is a secret door (from `maybe_sdoor`).
/// `difficulty`: the level difficulty (affects trapped probability).
/// `is_shop_door`: if true, the door is on a shop boundary.
///
/// Returns the resulting `DoorState`.
pub fn choose_door_state(
    is_secret: bool,
    difficulty: u32,
    is_shop_door: bool,
    rng: &mut impl Rng,
) -> DoorState {
    if is_secret {
        // Secret door: shop or 1/5 => locked, else closed.
        // Trap: if not shop and difficulty >= 4, 1/20 chance.
        let locked = is_shop_door || rng.random_range(0..5u32) == 0;
        let trapped = !is_shop_door
            && difficulty >= 4
            && rng.random_range(0..20u32) == 0;
        if locked {
            if trapped {
                DoorState::LockedTrapped
            } else {
                DoorState::SecretLocked
            }
        } else if trapped {
            DoorState::ClosedTrapped
        } else {
            DoorState::SecretClosed
        }
    } else {
        // Regular door.
        if rng.random_range(0..3u32) != 0 {
            // 2/3: no door panel (empty doorway).
            if is_shop_door {
                return DoorState::Open;
            }
            return DoorState::NoDoor;
        }
        // 1/3: has a door panel.
        if rng.random_range(0..5u32) == 0 {
            // 1/5 of 1/3 = 1/15 overall: open door.
            return DoorState::Open;
        }
        if rng.random_range(0..6u32) == 0 {
            // 1/6 of 4/15: locked.
            let trapped = difficulty >= 5
                && rng.random_range(0..25u32) == 0;
            return if trapped {
                DoorState::LockedTrapped
            } else {
                DoorState::Locked
            };
        }
        // Remainder: closed.
        let trapped = difficulty >= 5
            && rng.random_range(0..25u32) == 0;
        if trapped {
            DoorState::ClosedTrapped
        } else {
            DoorState::Closed
        }
    }
}

// ---------------------------------------------------------------------------
// Random trap selection by depth (traptype_rnd)
// ---------------------------------------------------------------------------

use crate::traps::TrapType;

/// Select a random trap type appropriate for the given difficulty.
///
/// Follows NetHack's `traptype_rnd()`: roll rnd(25) = [1..25], then
/// filter by minimum difficulty and special rules.
///
/// `difficulty` is the level difficulty (depth + bonuses).
/// `in_hell` is true if generating in Gehennom.
/// `no_teleport` is true if the level has no-teleport flag.
/// `single_branch` is true if the branch has only one level.
///
/// Returns `None` if the random selection produced an invalid trap
/// (the caller should skip placement).
pub fn traptype_rnd(
    difficulty: u32,
    in_hell: bool,
    no_teleport: bool,
    single_branch: bool,
    rng: &mut impl Rng,
) -> Option<TrapType> {
    let kind_val = rng.random_range(1..=25u32);

    let trap = match kind_val {
        1 => TrapType::ArrowTrap,
        2 => TrapType::DartTrap,
        3 => TrapType::RockTrap,
        4 => TrapType::SqueakyBoard,
        5 => TrapType::BearTrap,
        6 => TrapType::Landmine,
        7 => TrapType::RollingBoulderTrap,
        8 => TrapType::SleepingGasTrap,
        9 => TrapType::RustTrap,
        10 => TrapType::FireTrap,
        11 => TrapType::Pit,
        12 => TrapType::SpikedPit,
        13 => TrapType::Hole,
        14 => TrapType::TrapDoor,
        15 => TrapType::TeleportTrap,
        16 => TrapType::LevelTeleport,
        17 => TrapType::MagicPortal,
        18 => TrapType::Web,
        19 => TrapType::StatueTrap,
        20 => TrapType::MagicTrap,
        21 => TrapType::AntiMagic,
        22 => TrapType::PolyTrap,
        23 => TrapType::VibratingSquare,
        24 => TrapType::TrappedDoor,
        25 => TrapType::TrappedChest,
        _ => return None,
    };

    // Filter by minimum difficulty and special rules.
    match trap {
        // Always available (min difficulty 1)
        TrapType::ArrowTrap
        | TrapType::DartTrap
        | TrapType::RockTrap
        | TrapType::SqueakyBoard
        | TrapType::BearTrap
        | TrapType::Pit
        | TrapType::RustTrap => Some(trap),

        // Teleport trap: min difficulty 1, not on no-teleport levels
        TrapType::TeleportTrap => {
            if no_teleport { None } else { Some(trap) }
        }

        // Min difficulty 2
        TrapType::SleepingGasTrap | TrapType::RollingBoulderTrap => {
            if difficulty >= 2 { Some(trap) } else { None }
        }

        // Min difficulty 5
        TrapType::SpikedPit => {
            if difficulty >= 5 { Some(trap) } else { None }
        }
        TrapType::LevelTeleport => {
            if difficulty >= 5 && !no_teleport && !single_branch {
                Some(trap)
            } else {
                None
            }
        }

        // Min difficulty 6
        TrapType::Landmine => {
            if difficulty >= 6 { Some(trap) } else { None }
        }

        // Min difficulty 7
        TrapType::Web => {
            if difficulty >= 7 { Some(trap) } else { None }
        }

        // Min difficulty 8
        TrapType::StatueTrap | TrapType::PolyTrap => {
            if difficulty >= 8 { Some(trap) } else { None }
        }

        // Fire trap: Gehennom only
        TrapType::FireTrap => {
            if in_hell { Some(trap) } else { None }
        }

        // Hole: min difficulty 1, but only 1/7 chance to keep
        TrapType::Hole => {
            if rng.random_range(0..7u32) == 0 {
                Some(trap)
            } else {
                None
            }
        }

        // Trapdoor: min difficulty 1
        TrapType::TrapDoor => Some(trap),

        // Never randomly generated
        TrapType::MagicPortal
        | TrapType::VibratingSquare
        | TrapType::TrappedDoor
        | TrapType::TrappedChest
        | TrapType::NoTrap
        | TrapType::MagicTrap
        | TrapType::AntiMagic => None,
    }
}

// ---------------------------------------------------------------------------
// Branch configuration
// ---------------------------------------------------------------------------

/// Configuration for a dungeon branch.
///
/// Describes the depth range, level count, and connection rules for
/// each branch in the dungeon topology.
#[derive(Debug, Clone)]
pub struct BranchConfig {
    /// Human-readable name.
    pub name: &'static str,
    /// Which `DungeonBranch` variant this corresponds to.
    pub branch: crate::dungeon::DungeonBranch,
    /// Base number of levels in this branch.
    pub base_levels: u32,
    /// Additional random levels (total = base + rn2(range)).
    pub range: u32,
    /// Whether levels are mazelike by default.
    pub mazelike: bool,
    /// Whether this branch is hellish (fire traps, no bones, etc.).
    pub hellish: bool,
    /// Entry depth in the parent branch (base value).
    pub entry_base: u32,
    /// Random range added to entry_base.
    pub entry_range: u32,
}

use crate::dungeon::DungeonBranch;

/// All branch configurations matching `dat/dungeon.lua`.
pub fn branch_configs() -> Vec<BranchConfig> {
    vec![
        BranchConfig {
            name: "The Dungeons of Doom",
            branch: DungeonBranch::Main,
            base_levels: 25,
            range: 5,
            mazelike: false,
            hellish: false,
            entry_base: 0,
            entry_range: 0,
        },
        BranchConfig {
            name: "Gehennom",
            branch: DungeonBranch::Gehennom,
            base_levels: 20,
            range: 5,
            mazelike: true,
            hellish: true,
            entry_base: 0, // chained to Castle
            entry_range: 0,
        },
        BranchConfig {
            name: "The Gnomish Mines",
            branch: DungeonBranch::Mines,
            base_levels: 8,
            range: 2,
            mazelike: true,
            hellish: false,
            entry_base: 2,
            entry_range: 3,
        },
        BranchConfig {
            name: "The Quest",
            branch: DungeonBranch::Quest,
            base_levels: 5,
            range: 2,
            mazelike: false,
            hellish: false,
            entry_base: 6, // chained to Oracle, base=6, range=2
            entry_range: 2,
        },
        BranchConfig {
            name: "Sokoban",
            branch: DungeonBranch::Sokoban,
            base_levels: 4,
            range: 0,
            mazelike: true,
            hellish: false,
            entry_base: 1, // 1 above Oracle
            entry_range: 0,
        },
        BranchConfig {
            name: "Fort Ludios",
            branch: DungeonBranch::FortLudios,
            base_levels: 1,
            range: 0,
            mazelike: true,
            hellish: false,
            entry_base: 18,
            entry_range: 4,
        },
        BranchConfig {
            name: "Vlad's Tower",
            branch: DungeonBranch::VladsTower,
            base_levels: 3,
            range: 0,
            mazelike: true,
            hellish: false,
            entry_base: 9,
            entry_range: 5,
        },
        BranchConfig {
            name: "The Elemental Planes",
            branch: DungeonBranch::Endgame,
            base_levels: 6,
            range: 0,
            mazelike: true,
            hellish: false,
            entry_base: 1,
            entry_range: 0,
        },
    ]
}

/// Get the branch config for the Mines.
pub fn mines_entry_depth(rng: &mut impl Rng) -> u32 {
    let cfg = branch_configs()
        .into_iter()
        .find(|c| c.branch == DungeonBranch::Mines)
        .unwrap();
    cfg.entry_base + rng.random_range(0..=cfg.entry_range)
}

/// Compute the total number of levels in a branch.
pub fn branch_level_count(branch: DungeonBranch, rng: &mut impl Rng) -> u32 {
    let cfg = branch_configs()
        .into_iter()
        .find(|c| c.branch == branch)
        .unwrap();
    cfg.base_levels + if cfg.range > 0 {
        rng.random_range(0..cfg.range)
    } else {
        0
    }
}

/// Level trap placement: determine how many traps to place on a
/// regular (rooms+corridors) level, following the spec.
///
/// For regular levels, traps are placed per-room via
/// `fill_ordinary_room()`: while `!rn2(x)` where
/// `x = max(2, 8 - difficulty/6)`.
///
/// For maze levels: `rn1(6, 7) = [7, 12]` traps.
///
/// This function computes the per-room trap count for a regular level.
pub fn room_trap_count(difficulty: u32, rng: &mut impl Rng) -> u32 {
    let x = (8u32.saturating_sub(difficulty / 6)).max(2);
    let mut count = 0u32;
    // "while !rn2(x)" loop: geometric distribution
    while rng.random_range(0..x) == 0 && count < 1000 {
        count += 1;
    }
    count
}

/// Maze level trap count: rn1(6, 7) = [7, 12].
pub fn maze_trap_count(rng: &mut impl Rng) -> u32 {
    7 + rng.random_range(0..6u32)
}

// ---------------------------------------------------------------------------
// Vault generation
// ---------------------------------------------------------------------------

/// A generated vault — a 2x2 room filled with gold, isolated from the
/// rest of the level (reachable only via teleportation or phasing).
///
/// In NetHack, vaults are special 2x2 rooms that contain gold. They have
/// no door connections; the guard arrives via a magic corridor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vault {
    /// Interior left-x coordinate.
    pub x: usize,
    /// Interior top-y coordinate.
    pub y: usize,
}

/// Try to place a vault on the level, following NetHack's vault placement.
///
/// A vault is a 2x2 floor room surrounded by walls on all sides, with no
/// door connections.  It must not overlap any existing room (with margin).
///
/// Returns `Some(Vault)` if placement succeeded.
pub fn try_place_vault(
    map: &mut LevelMap,
    existing_rooms: &[Room],
    rng: &mut impl Rng,
) -> Option<Vault> {
    let w = map.width;
    let h = map.height;

    // Try up to 200 times to find a valid position.
    for _ in 0..200 {
        // Vault interior is 2x2.  Walls add 1 on each side, plus 1 margin.
        let min_x = 3usize;
        let min_y = 3usize;
        let max_x = w.saturating_sub(5);
        let max_y = h.saturating_sub(5);
        if max_x < min_x || max_y < min_y {
            return None;
        }

        let vx = rng.random_range(min_x..=max_x);
        let vy = rng.random_range(min_y..=max_y);

        let vault_room = Room {
            x: vx,
            y: vy,
            width: 2,
            height: 2,
            lit: true,
        };

        // Check no overlap with existing rooms (margin=3 to keep vault isolated).
        let overlaps = existing_rooms
            .iter()
            .any(|r| vault_room.overlaps_with_margin(r, 3));
        if overlaps {
            continue;
        }

        // Check that the vault area is currently all stone (uncarved).
        let all_stone = (vy.saturating_sub(1)..=vy + 2)
            .all(|y| {
                (vx.saturating_sub(1)..=vx + 2).all(|x| {
                    map.get(Position::new(x as i32, y as i32))
                        .is_some_and(|c| c.terrain == Terrain::Stone)
                })
            });
        if !all_stone {
            continue;
        }

        // Carve the vault: walls on perimeter, floor inside.
        for y in vy.saturating_sub(1)..=vy + 2 {
            for x in vx.saturating_sub(1)..=vx + 2 {
                let pos = Position::new(x as i32, y as i32);
                let on_edge = x == vx.saturating_sub(1)
                    || x == vx + 2
                    || y == vy.saturating_sub(1)
                    || y == vy + 2;
                if on_edge {
                    map.set_terrain(pos, Terrain::Wall);
                } else {
                    map.set_terrain(pos, Terrain::Floor);
                }
            }
        }

        return Some(Vault { x: vx, y: vy });
    }

    None
}

// ---------------------------------------------------------------------------
// Shop room interior population
// ---------------------------------------------------------------------------

/// Type of shop to generate (corresponds to NetHack's shclass entries).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShopType {
    General,
    Armor,
    ScrollsAndSpellbooks,
    Potions,
    Weapons,
    Food,
    Rings,
    Wands,
    Tools,
    Instruments,
    Candles,
}

/// Choose a random shop type appropriate for the given depth.
///
/// Follows the probability distribution from NetHack's `shclass[]`:
/// deeper shops tend toward more specialized types.
pub fn choose_shop_type(depth: u32, rng: &mut impl Rng) -> ShopType {
    // At shallow depths, general stores dominate.
    // At deeper depths, specialty shops become more common.
    if depth <= 3 || rng.random_range(0..5u32) == 0 {
        return ShopType::General;
    }

    match rng.random_range(0..10u32) {
        0 => ShopType::Armor,
        1 => ShopType::ScrollsAndSpellbooks,
        2 => ShopType::Potions,
        3 => ShopType::Weapons,
        4 => ShopType::Food,
        5 => ShopType::Rings,
        6 => ShopType::Wands,
        7 => ShopType::Tools,
        8 => ShopType::Instruments,
        9 => ShopType::Candles,
        _ => ShopType::General,
    }
}

/// Configure a room as a shop: place a door on a wall and mark it as
/// a shop room.
///
/// This sets up the room terrain for a shop.  The caller is responsible
/// for spawning the shopkeeper entity and shop items.
///
/// The shop door is always a closed door (shopkeepers open it for customers).
pub fn configure_shop_room(map: &mut LevelMap, room: &Room, rng: &mut impl Rng) {
    // Ensure the room has exactly one door.
    place_shop_door(map, room, rng);
}

/// Place a single closed door on a random wall of the room.
fn place_shop_door(map: &mut LevelMap, room: &Room, rng: &mut impl Rng) {
    let side = rng.random_range(0..4u32);
    let pos = match side {
        0 => {
            let dx = rng.random_range(room.x..=room.right()) as i32;
            Position::new(dx, room.y as i32 - 1)
        }
        1 => {
            let dx = rng.random_range(room.x..=room.right()) as i32;
            Position::new(dx, room.bottom() as i32 + 1)
        }
        2 => {
            let dy = rng.random_range(room.y..=room.bottom()) as i32;
            Position::new(room.x as i32 - 1, dy)
        }
        _ => {
            let dy = rng.random_range(room.y..=room.bottom()) as i32;
            Position::new(room.right() as i32 + 1, dy)
        }
    };
    if map.in_bounds(pos) {
        map.set_terrain(pos, Terrain::DoorClosed);
    }
}

// ---------------------------------------------------------------------------
// Temple room setup
// ---------------------------------------------------------------------------

/// Alignment for a temple altar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AltarAlignment {
    Lawful,
    Neutral,
    Chaotic,
    /// Unaligned altar (Moloch, Astral).
    None,
}

/// Configure a room as a temple: place an altar at the center.
///
/// Returns the altar position and alignment.  The caller is responsible
/// for spawning the priest entity.
pub fn configure_temple_room(
    map: &mut LevelMap,
    room: &Room,
    rng: &mut impl Rng,
) -> (Position, AltarAlignment) {
    let (cx, cy) = room.center();
    let altar_pos = Position::new(cx as i32, cy as i32);
    map.set_terrain(altar_pos, Terrain::Altar);

    // Random alignment.
    let alignment = match rng.random_range(0..3u32) {
        0 => AltarAlignment::Lawful,
        1 => AltarAlignment::Neutral,
        _ => AltarAlignment::Chaotic,
    };

    (altar_pos, alignment)
}

// ---------------------------------------------------------------------------
// Room population plan (entity spawning descriptors)
// ---------------------------------------------------------------------------

/// Describes what entities should be spawned in a special room.
/// The caller (game init code) is responsible for creating actual ECS entities.
#[derive(Debug, Clone)]
pub struct RoomPopulation {
    pub room_type: SpecialRoomType,
    pub room: Room,
    /// (monster_class_char, x, y, asleep, peaceful)
    pub monsters: Vec<(char, usize, usize, bool, bool)>,
    /// (object_class_char, x, y)
    pub objects: Vec<(char, usize, usize)>,
}

/// Plan entity population for a special room.
///
/// Returns a `RoomPopulation` describing which monsters and objects to spawn.
/// The caller is responsible for creating actual entities from this plan.
/// This function also applies terrain modifications (thrones, altars, graves, etc.).
pub fn plan_room_population(
    map: &mut LevelMap,
    room: &Room,
    room_type: SpecialRoomType,
    rng: &mut impl Rng,
) -> RoomPopulation {
    let mut pop = RoomPopulation {
        room_type,
        room: room.clone(),
        monsters: Vec::new(),
        objects: Vec::new(),
    };

    match room_type {
        SpecialRoomType::Temple => {
            let (cx, cy) = room.center();
            map.set_terrain(Position::new(cx as i32, cy as i32), Terrain::Altar);
            // Priest at altar position
            pop.monsters.push(('p', cx, cy, false, true));
        }
        SpecialRoomType::Court => {
            let (cx, cy) = room.center();
            map.set_terrain(Position::new(cx as i32, cy as i32), Terrain::Throne);
            // Throne monster at center
            pop.monsters.push(('K', cx, cy, true, false));
            // Fill with courtiers (orc/gnome/kobold/etc.) on remaining tiles
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    if x == cx && y == cy {
                        continue;
                    }
                    if map.get(Position::new(x as i32, y as i32))
                        .is_some_and(|c| c.terrain == Terrain::Floor)
                    {
                        // 'o' for orc-class courtier
                        pop.monsters.push(('o', x, y, true, false));
                        // Gold near throne
                        pop.objects.push(('$', x, y));
                    }
                }
            }
        }
        SpecialRoomType::Morgue => {
            // Scatter graves on ~20% of floor tiles (matching C: !rn2(5))
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    let pos = Position::new(x as i32, y as i32);
                    if map.get(pos).is_some_and(|c| c.terrain == Terrain::Floor) {
                        // Monster (zombie/wraith/ghost class 'Z')
                        pop.monsters.push(('Z', x, y, true, false));
                        // Corpse on ~20% of tiles
                        if rng.random_range(0..5u32) == 0 {
                            pop.objects.push(('%', x, y));
                        }
                        // Chest/box on ~10%
                        if rng.random_range(0..10u32) == 0 {
                            pop.objects.push(('(', x, y));
                        }
                        // Grave on ~20%
                        if rng.random_range(0..5u32) == 0 {
                            map.set_terrain(pos, Terrain::Grave);
                        }
                    }
                }
            }
        }
        SpecialRoomType::Swamp => {
            // Replace alternating floor tiles with pools (matching C: (sx+sy)%2).
            // Preserve edge tiles to keep corridor door connections walkable.
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    let pos = Position::new(x as i32, y as i32);
                    if map.get(pos).is_some_and(|c| c.terrain == Terrain::Floor) {
                        let on_edge = x == room.x || x == room.right()
                            || y == room.y || y == room.bottom();
                        if !on_edge && (x + y) % 2 == 1 {
                            map.set_terrain(pos, Terrain::Pool);
                            // Eel in some pools
                            if rng.random_range(0..4u32) == 0 {
                                pop.monsters.push((';', x, y, false, false));
                            }
                        } else if rng.random_range(0..4u32) == 0 {
                            // Fungus on dry tiles
                            pop.monsters.push(('F', x, y, false, false));
                        }
                    }
                }
            }
        }
        SpecialRoomType::Beehive => {
            // Queen bee at center
            let (cx, cy) = room.center();
            pop.monsters.push(('a', cx, cy, true, false)); // queen bee
            // Fill with killer bees and royal jelly
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    if x == cx && y == cy {
                        continue;
                    }
                    if map.get(Position::new(x as i32, y as i32))
                        .is_some_and(|c| c.terrain == Terrain::Floor)
                    {
                        // Killer bee on every tile (C NetHack fills all tiles)
                        pop.monsters.push(('a', x, y, true, false));
                        // Royal jelly on ~33% of tiles (C: !rn2(3))
                        if rng.random_range(0..3u32) == 0 {
                            pop.objects.push(('%', x, y));
                        }
                    }
                }
            }
        }
        SpecialRoomType::Zoo => {
            // Fill with random monsters (asleep) and gold
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    if map.get(Position::new(x as i32, y as i32))
                        .is_some_and(|c| c.terrain == Terrain::Floor)
                    {
                        // Random monster on every tile, all asleep
                        pop.monsters.push(('?', x, y, true, false));
                        // Gold on every tile (amount varies by distance to door)
                        pop.objects.push(('$', x, y));
                    }
                }
            }
        }
        SpecialRoomType::Barracks => {
            // Fill with soldiers
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    if map.get(Position::new(x as i32, y as i32))
                        .is_some_and(|c| c.terrain == Terrain::Floor)
                    {
                        // Soldier class '@'
                        pop.monsters.push(('@', x, y, true, false));
                        // Chest on ~5% of tiles (C: !rn2(20))
                        if rng.random_range(0..20u32) == 0 {
                            pop.objects.push(('(', x, y));
                        }
                    }
                }
            }
        }
        SpecialRoomType::LeprechaunHall => {
            // Fill with leprechauns and gold
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    if map.get(Position::new(x as i32, y as i32))
                        .is_some_and(|c| c.terrain == Terrain::Floor)
                    {
                        // Leprechaun 'l'
                        pop.monsters.push(('l', x, y, true, false));
                        // Gold
                        pop.objects.push(('$', x, y));
                    }
                }
            }
        }
        SpecialRoomType::CockatriceNest => {
            // Fill with cockatrices, statues, and eggs
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    if map.get(Position::new(x as i32, y as i32))
                        .is_some_and(|c| c.terrain == Terrain::Floor)
                    {
                        // Cockatrice 'c'
                        pop.monsters.push(('c', x, y, true, false));
                        // Statue on ~33% of tiles (C: !rn2(3))
                        if rng.random_range(0..3u32) == 0 {
                            pop.objects.push(('`', x, y)); // statue
                        }
                    }
                }
            }
        }
        SpecialRoomType::Anthole => {
            // Fill with ants and food
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    if map.get(Position::new(x as i32, y as i32))
                        .is_some_and(|c| c.terrain == Terrain::Floor)
                    {
                        // Ant 'a'
                        pop.monsters.push(('a', x, y, true, false));
                        // Food on ~33% of tiles (C: !rn2(3))
                        if rng.random_range(0..3u32) == 0 {
                            pop.objects.push(('%', x, y));
                        }
                    }
                }
            }
        }
        SpecialRoomType::Vault => {
            // Vault terrain is handled by try_place_vault; gold spawned by caller.
        }
        SpecialRoomType::Shop => {
            // Shop terrain is set by configure_shop_room; items by caller.
        }
        SpecialRoomType::ORoom => {
            // Ordinary room — no special population.
        }
    }

    pop
}

// ---------------------------------------------------------------------------
// Room feature population (terrain only, legacy API)
// ---------------------------------------------------------------------------

/// Add special features to a room based on its type.
///
/// This handles terrain modifications only (fountains, thrones, altars).
/// Entity spawning (monsters, items, NPCs) is handled by the caller.
/// For a full population plan including entities, use [`plan_room_population`].
pub fn populate_special_room(
    map: &mut LevelMap,
    room: &Room,
    room_type: SpecialRoomType,
    rng: &mut impl Rng,
) {
    // Delegate to plan_room_population which handles both terrain and
    // entity planning; we discard the entity plan here.
    let _pop = plan_room_population(map, room, room_type, rng);
}

/// Count how many floor cells exist in a room (useful for density checks).
pub fn room_floor_count(map: &LevelMap, room: &Room) -> usize {
    (room.y..=room.bottom())
        .flat_map(|y| (room.x..=room.right()).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            map.get(Position::new(x as i32, y as i32))
                .is_some_and(|c| c.terrain == Terrain::Floor)
        })
        .count()
}

// ---------------------------------------------------------------------------
// Themed rooms
// ---------------------------------------------------------------------------

/// Themed room decorations that can be applied to ordinary rooms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeRoom {
    /// Pillared hall: stone pillars in a grid pattern.
    PillaredHall,
    /// Garden: trees and fountains.
    Garden,
    /// Graveyard corner: a few graves.
    GraveyardCorner,
    /// Flooded room: scattered pools.
    FloodedRoom,
    /// Throne room lite: just a throne, no courtiers.
    MinorThrone,
    /// Library: bookshelves (no special terrain).
    Library,
    /// Lava room: lava pools on some tiles.
    LavaRoom,
}

/// Possibly apply a themed decoration to an ordinary room.
/// Returns the theme applied, if any.
///
/// Based on NetHack's `dat/themerms.lua` — simplified version with
/// ~15% chance of theming an ordinary room.
pub fn maybe_apply_theme(
    map: &mut LevelMap,
    room: &Room,
    depth: u8,
    rng: &mut impl Rng,
) -> Option<ThemeRoom> {
    // ~15% chance of theming an ordinary room.
    if rng.random_range(0..100u32) >= 15 {
        return None;
    }

    // Build candidate list based on depth.
    let mut candidates: Vec<ThemeRoom> = vec![
        ThemeRoom::PillaredHall,
        ThemeRoom::Garden,
        ThemeRoom::GraveyardCorner,
        ThemeRoom::FloodedRoom,
    ];
    if depth > 3 {
        candidates.push(ThemeRoom::MinorThrone);
    }
    if depth > 5 {
        candidates.push(ThemeRoom::Library);
    }
    if depth > 10 {
        candidates.push(ThemeRoom::LavaRoom);
    }

    let idx = rng.random_range(0..candidates.len());
    let theme = candidates[idx];

    apply_theme(map, room, theme, rng);
    Some(theme)
}

/// Apply a specific theme to a room's terrain.
fn apply_theme(map: &mut LevelMap, room: &Room, theme: ThemeRoom, rng: &mut impl Rng) {
    match theme {
        ThemeRoom::PillaredHall => {
            // Place stone pillars every 2 tiles.
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    if (x - room.x) % 2 == 0
                        && (y - room.y) % 2 == 0
                        && x != room.x
                        && y != room.y
                        && x != room.right()
                        && y != room.bottom()
                    {
                        map.set_terrain(
                            Position::new(x as i32, y as i32),
                            Terrain::Stone,
                        );
                    }
                }
            }
        }
        ThemeRoom::Garden => {
            // Trees on ~20% of interior tiles, fountain at center.
            // Preserve edge tiles to keep corridor connections.
            let (cx, cy) = room.center();
            map.set_terrain(Position::new(cx as i32, cy as i32), Terrain::Fountain);
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    if x == cx && y == cy {
                        continue;
                    }
                    if x == room.x || x == room.right()
                        || y == room.y || y == room.bottom()
                    {
                        continue;
                    }
                    if rng.random_range(0..5u32) == 0 {
                        map.set_terrain(
                            Position::new(x as i32, y as i32),
                            Terrain::Tree,
                        );
                    }
                }
            }
        }
        ThemeRoom::GraveyardCorner => {
            // 2-4 graves in one corner of the room.
            let grave_count = rng.random_range(2..=4u32);
            let mut placed = 0u32;
            for _ in 0..grave_count * 10 {
                if placed >= grave_count {
                    break;
                }
                let gx = rng.random_range(room.x..=room.right());
                let gy = rng.random_range(room.y..=room.bottom());
                let pos = Position::new(gx as i32, gy as i32);
                if map.get(pos).is_some_and(|c| c.terrain == Terrain::Floor) {
                    map.set_terrain(pos, Terrain::Grave);
                    placed += 1;
                }
            }
        }
        ThemeRoom::FloodedRoom => {
            // Pools on ~25% of interior tiles (skip edge rows/cols to keep paths).
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    // Preserve edge tiles adjacent to walls (where doors connect).
                    if x == room.x || x == room.right()
                        || y == room.y || y == room.bottom()
                    {
                        continue;
                    }
                    if rng.random_range(0..4u32) == 0 {
                        map.set_terrain(
                            Position::new(x as i32, y as i32),
                            Terrain::Pool,
                        );
                    }
                }
            }
        }
        ThemeRoom::MinorThrone => {
            // Just a throne at the center.
            let (cx, cy) = room.center();
            map.set_terrain(Position::new(cx as i32, cy as i32), Terrain::Throne);
        }
        ThemeRoom::Library => {
            // No special terrain changes (books would be objects).
            // Place a fountain for ambiance.
            if room.width >= 4 && room.height >= 3 {
                let (cx, cy) = room.center();
                map.set_terrain(Position::new(cx as i32, cy as i32), Terrain::Fountain);
            }
        }
        ThemeRoom::LavaRoom => {
            // Lava on ~20% of interior tiles (skip edge rows/cols to keep paths).
            for y in room.y..=room.bottom() {
                for x in room.x..=room.right() {
                    if x == room.x || x == room.right()
                        || y == room.y || y == room.bottom()
                    {
                        continue;
                    }
                    if rng.random_range(0..5u32) == 0 {
                        map.set_terrain(
                            Position::new(x as i32, y as i32),
                            Terrain::Lava,
                        );
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Maze generation
// ---------------------------------------------------------------------------

/// Generate a maze using a recursive-backtracker algorithm.
///
/// The maze is carved out of a solid wall grid.  Each "cell" in the maze
/// occupies a 2x2 block (one floor tile + one wall/floor tile between
/// cells).  The resulting passages are 1-tile wide corridors separated by
/// 1-tile walls.
///
/// `width` and `height` are the output map dimensions.
/// Up and down stairs are placed at the two ends of the maze farthest apart.
pub fn generate_maze(width: usize, height: usize, rng: &mut impl Rng) -> LevelMap {
    let mut map = LevelMap::new(width, height);

    // Fill everything with walls first.
    for y in 0..height {
        for x in 0..width {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
        }
    }

    // Maze cells: use odd coordinates for passages so that even coordinates
    // remain as walls between cells.
    let cells_w = (width - 1) / 2;
    let cells_h = (height - 1) / 2;

    if cells_w == 0 || cells_h == 0 {
        return map;
    }

    let mut visited = vec![vec![false; cells_w]; cells_h];
    let mut stack: Vec<(usize, usize)> = Vec::new();

    // Start from cell (0, 0) — which corresponds to map position (1, 1).
    let start_cx = 0usize;
    let start_cy = 0usize;
    visited[start_cy][start_cx] = true;
    stack.push((start_cx, start_cy));

    // Carve the starting cell.
    let sx = start_cx * 2 + 1;
    let sy = start_cy * 2 + 1;
    map.set_terrain(Position::new(sx as i32, sy as i32), Terrain::Corridor);

    while let Some(&(cx, cy)) = stack.last() {
        // Collect unvisited neighbours.
        let mut neighbours = Vec::new();
        if cx > 0 && !visited[cy][cx - 1] {
            neighbours.push((cx - 1, cy));
        }
        if cx + 1 < cells_w && !visited[cy][cx + 1] {
            neighbours.push((cx + 1, cy));
        }
        if cy > 0 && !visited[cy - 1][cx] {
            neighbours.push((cx, cy - 1));
        }
        if cy + 1 < cells_h && !visited[cy + 1][cx] {
            neighbours.push((cx, cy + 1));
        }

        if neighbours.is_empty() {
            stack.pop();
            continue;
        }

        // Pick a random neighbour.
        let idx = rng.random_range(0..neighbours.len());
        let (nx, ny) = neighbours[idx];

        // Carve the wall between current cell and neighbour.
        // Wall position is the midpoint between the two cell map positions.
        let cx_map = cx * 2 + 1;
        let cy_map = cy * 2 + 1;
        let nx_map = nx * 2 + 1;
        let ny_map = ny * 2 + 1;
        let wall_x = (cx_map + nx_map) / 2;
        let wall_y = (cy_map + ny_map) / 2;
        map.set_terrain(
            Position::new(wall_x as i32, wall_y as i32),
            Terrain::Corridor,
        );

        // Carve the neighbour cell.
        map.set_terrain(
            Position::new(nx_map as i32, ny_map as i32),
            Terrain::Corridor,
        );

        visited[ny][nx] = true;
        stack.push((nx, ny));
    }

    map
}

/// Generate a Gehennom (Hell) level: maze-based with lava pools and
/// fire traps.
///
/// `depth` is the absolute depth within Gehennom (1-based).
pub fn generate_gehennom_level(depth: u8, rng: &mut impl Rng) -> GeneratedLevel {
    let w = LevelMap::DEFAULT_WIDTH;
    let h = LevelMap::DEFAULT_HEIGHT;

    // Start with a maze.
    let mut map = generate_maze(w, h, rng);

    // Scatter lava pools on wall tiles adjacent to corridors.
    let lava_count = rng.random_range(15..=30u32) + depth as u32;
    let mut placed_lava = 0u32;
    for _ in 0..lava_count * 10 {
        if placed_lava >= lava_count {
            break;
        }
        let x = rng.random_range(1..w - 1) as i32;
        let y = rng.random_range(1..h - 1) as i32;
        let pos = Position::new(x, y);
        if let Some(cell) = map.get(pos)
            && cell.terrain == Terrain::Wall
        {
            // Check if at least one neighbour is a corridor/floor.
            let has_passage = [(0i32, -1i32), (0, 1), (-1, 0), (1, 0)]
                .iter()
                .any(|&(dx, dy)| {
                    map.get(Position::new(x + dx, y + dy))
                        .is_some_and(|c| {
                            matches!(
                                c.terrain,
                                Terrain::Corridor | Terrain::Floor
                            )
                        })
                });
            if has_passage {
                map.set_terrain(pos, Terrain::Lava);
                placed_lava += 1;
            }
        }
    }

    // Also scatter some lava pools in open areas.
    for _ in 0..10 {
        let x = rng.random_range(1..w - 1) as i32;
        let y = rng.random_range(1..h - 1) as i32;
        let pos = Position::new(x, y);
        if let Some(cell) = map.get(pos)
            && cell.terrain == Terrain::Wall
        {
            map.set_terrain(pos, Terrain::Lava);
        }
    }

    // Place fire traps on some corridor tiles (represented as Floor markers;
    // actual trap placement is handled by the trap system, but we convert
    // some corridor tiles to Floor for variety).
    let fire_trap_count = rng.random_range(3..=8u32);
    let mut placed_traps = 0u32;
    for _ in 0..fire_trap_count * 20 {
        if placed_traps >= fire_trap_count {
            break;
        }
        let x = rng.random_range(1..w - 1) as i32;
        let y = rng.random_range(1..h - 1) as i32;
        let pos = Position::new(x, y);
        if let Some(cell) = map.get(pos)
            && cell.terrain == Terrain::Corridor
        {
            // Mark as Floor (caller will place FireTrap objects here).
            map.set_terrain(pos, Terrain::Floor);
            placed_traps += 1;
        }
    }

    // Place stairs: up-stairs near the top, down-stairs near the bottom.
    let mut up_stairs = None;
    let mut down_stairs = None;

    // Find corridor cells for stair placement.
    for y in 1..h - 1 {
        if up_stairs.is_some() {
            break;
        }
        for x in 1..w - 1 {
            let pos = Position::new(x as i32, y as i32);
            if let Some(cell) = map.get(pos)
                && matches!(cell.terrain, Terrain::Corridor | Terrain::Floor)
            {
                map.set_terrain(pos, Terrain::StairsUp);
                up_stairs = Some(pos);
                break;
            }
        }
    }

    for y in (1..h - 1).rev() {
        if down_stairs.is_some() {
            break;
        }
        for x in (1..w - 1).rev() {
            let pos = Position::new(x as i32, y as i32);
            if let Some(cell) = map.get(pos)
                && matches!(cell.terrain, Terrain::Corridor | Terrain::Floor)
            {
                map.set_terrain(pos, Terrain::StairsDown);
                down_stairs = Some(pos);
                break;
            }
        }
    }

    // Create a pseudo-room list (empty — maze levels have no rooms).
    GeneratedLevel {
        map,
        rooms: Vec::new(),
        up_stairs,
        down_stairs,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    /// Helper: create a deterministic RNG.
    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(42)
    }

    #[test]
    fn generated_map_has_correct_dimensions() {
        let mut rng = test_rng();
        let level = generate_level(5, &mut rng);
        assert_eq!(level.map.width, LevelMap::DEFAULT_WIDTH);
        assert_eq!(level.map.height, LevelMap::DEFAULT_HEIGHT);
        assert_eq!(level.map.cells.len(), LevelMap::DEFAULT_HEIGHT);
        assert_eq!(level.map.cells[0].len(), LevelMap::DEFAULT_WIDTH);
    }

    #[test]
    fn generated_map_has_rooms() {
        let mut rng = test_rng();
        let level = generate_level(5, &mut rng);
        assert!(
            level.rooms.len() >= 3,
            "expected at least 3 rooms, got {}",
            level.rooms.len()
        );
    }

    #[test]
    fn up_and_down_stairs_exist() {
        let mut rng = test_rng();
        let level = generate_level(5, &mut rng);
        assert!(level.up_stairs.is_some(), "up stairs should exist at depth 5");
        assert!(
            level.down_stairs.is_some(),
            "down stairs should exist at depth 5"
        );

        // Verify the terrain on the map matches.
        let up = level.up_stairs.unwrap();
        assert_eq!(
            level.map.get(up).unwrap().terrain,
            Terrain::StairsUp,
            "up stairs position should have StairsUp terrain"
        );
        let down = level.down_stairs.unwrap();
        assert_eq!(
            level.map.get(down).unwrap().terrain,
            Terrain::StairsDown,
            "down stairs position should have StairsDown terrain"
        );
    }

    #[test]
    fn depth_1_has_no_up_stairs() {
        let mut rng = test_rng();
        let level = generate_level(1, &mut rng);
        assert!(
            level.up_stairs.is_none(),
            "depth 1 should not have up stairs"
        );
        assert!(
            level.down_stairs.is_some(),
            "depth 1 should have down stairs"
        );
    }

    #[test]
    fn all_rooms_reachable_via_flood_fill() {
        let mut rng = test_rng();
        let level = generate_level(5, &mut rng);
        let map = &level.map;

        // Find the first walkable cell.
        let start = find_first_walkable(map).expect("map should have at least one walkable cell");

        // Flood-fill from the start.
        let reachable = flood_fill(map, start);

        // Every room should have at least one reachable cell.
        for (i, room) in level.rooms.iter().enumerate() {
            let room_reachable = (room.y..=room.bottom()).any(|y| {
                (room.x..=room.right()).any(|x| reachable[y][x])
            });
            assert!(
                room_reachable,
                "room {} at ({},{}) {}x{} is not reachable from the flood fill",
                i, room.x, room.y, room.width, room.height
            );
        }
    }

    #[test]
    fn reproducible_with_same_seed() {
        let level_a = generate_level(5, &mut Pcg64::seed_from_u64(123));
        let level_b = generate_level(5, &mut Pcg64::seed_from_u64(123));

        assert_eq!(level_a.rooms, level_b.rooms);
        assert_eq!(level_a.up_stairs, level_b.up_stairs);
        assert_eq!(level_a.down_stairs, level_b.down_stairs);

        // Compare terrain cell by cell.
        for y in 0..level_a.map.height {
            for x in 0..level_a.map.width {
                assert_eq!(
                    level_a.map.cells[y][x].terrain,
                    level_b.map.cells[y][x].terrain,
                    "terrain mismatch at ({}, {})",
                    x,
                    y
                );
            }
        }
    }

    #[test]
    fn rooms_do_not_overlap() {
        let mut rng = test_rng();
        let level = generate_level(8, &mut rng);
        for i in 0..level.rooms.len() {
            for j in i + 1..level.rooms.len() {
                // Rooms should not share wall space (margin >= 1).
                assert!(
                    !level.rooms[i].overlaps_with_margin(&level.rooms[j], 1),
                    "rooms {} and {} overlap (with margin=1)",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn multiple_depths_produce_valid_maps() {
        for depth in [1, 3, 5, 10, 15, 20, 30] {
            let mut rng = Pcg64::seed_from_u64(depth as u64 * 7 + 99);
            let level = generate_level(depth, &mut rng);
            assert!(!level.rooms.is_empty(), "depth {} produced no rooms", depth);
            assert!(
                level.down_stairs.is_some(),
                "depth {} has no down stairs",
                depth
            );

            // Connectivity check.
            if let Some(start) = find_first_walkable(&level.map) {
                let reachable = flood_fill(&level.map, start);
                for (i, room) in level.rooms.iter().enumerate() {
                    let ok = (room.y..=room.bottom())
                        .any(|y| (room.x..=room.right()).any(|x| reachable[y][x]));
                    assert!(ok, "depth {}: room {} unreachable", depth, i);
                }
            }
        }
    }

    // ── Test helpers ────────────────────────────────────────────────────

    fn find_first_walkable(map: &LevelMap) -> Option<(usize, usize)> {
        for y in 0..map.height {
            for x in 0..map.width {
                if is_passable(map.cells[y][x].terrain) {
                    return Some((x, y));
                }
            }
        }
        None
    }

    /// Whether a cell is passable for connectivity testing.
    /// Includes closed/locked doors which are reachable but not "walkable"
    /// in the gameplay sense.
    fn is_passable(terrain: Terrain) -> bool {
        terrain.is_walkable()
            || matches!(terrain, Terrain::DoorClosed | Terrain::DoorLocked)
    }

    fn flood_fill(map: &LevelMap, start: (usize, usize)) -> Vec<Vec<bool>> {
        let mut visited = vec![vec![false; map.width]; map.height];
        let mut stack = vec![start];
        while let Some((x, y)) = stack.pop() {
            if visited[y][x] {
                continue;
            }
            let terrain = map.cells[y][x].terrain;
            if !is_passable(terrain) {
                continue;
            }
            visited[y][x] = true;
            // 4-directional flood fill.
            if x > 0 {
                stack.push((x - 1, y));
            }
            if x + 1 < map.width {
                stack.push((x + 1, y));
            }
            if y > 0 {
                stack.push((x, y - 1));
            }
            if y + 1 < map.height {
                stack.push((x, y + 1));
            }
        }
        visited
    }

    // =================================================================
    // H.1: Generic Floor Generation
    // =================================================================

    // ── Room size ranges ────────────────────────────────────────

    #[test]
    fn test_dungeon_room_size_width_range() {
        // Spec: width = 2 + rn2(8 or 12) => [2, 9] or [2, 13].
        // The combined range across many seeds should be [2, 13].
        let mut min_w = usize::MAX;
        let mut max_w = 0usize;
        for seed in 0..2000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let level = generate_level(5, &mut rng);
            for room in &level.rooms {
                min_w = min_w.min(room.width);
                max_w = max_w.max(room.width);
            }
        }
        assert!(
            min_w >= 2,
            "Room width min should be >= 2, got {}",
            min_w
        );
        assert!(
            max_w <= 13,
            "Room width max should be <= 13, got {}",
            max_w
        );
        // Should hit at least width 2 and width 9+ with 2000 seeds.
        assert_eq!(min_w, 2, "Should observe min width of 2");
        assert!(
            max_w >= 9,
            "Should observe width >= 9 over 2000 seeds, got {}",
            max_w
        );
    }

    #[test]
    fn test_dungeon_room_size_height_range() {
        // Spec: height = 2 + rn2(4) => [2, 5].
        let mut min_h = usize::MAX;
        let mut max_h = 0usize;
        for seed in 0..2000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let level = generate_level(5, &mut rng);
            for room in &level.rooms {
                min_h = min_h.min(room.height);
                max_h = max_h.max(room.height);
            }
        }
        assert!(
            min_h >= 2,
            "Room height min should be >= 2, got {}",
            min_h
        );
        assert!(
            max_h <= 5,
            "Room height max should be <= 5, got {}",
            max_h
        );
        assert_eq!(min_h, 2, "Should hit min height 2");
        assert_eq!(max_h, 5, "Should hit max height 5");
    }

    #[test]
    fn test_dungeon_room_size_area_cap() {
        // Spec: if width * height > 50, height = 50 / width.
        for seed in 0..2000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let level = generate_level(5, &mut rng);
            for room in &level.rooms {
                assert!(
                    room.width * room.height <= 50,
                    "Room area {} x {} = {} exceeds cap of 50",
                    room.width,
                    room.height,
                    room.width * room.height,
                );
            }
        }
    }

    // ── Stair reachability ──────────────────────────────────────

    #[test]
    fn test_dungeon_stairs_reachable() {
        // Both up and down stairs must be reachable via flood fill
        // from any walkable cell.
        for seed in 0..100 {
            let depth = (seed % 25 + 1) as u8;
            let mut rng = Pcg64::seed_from_u64(seed);
            let level = generate_level(depth, &mut rng);

            let start = find_first_walkable(&level.map);
            if start.is_none() {
                continue; // extremely unlikely
            }
            let reachable = flood_fill(&level.map, start.unwrap());

            if let Some(up) = level.up_stairs {
                assert!(
                    reachable[up.y as usize][up.x as usize],
                    "seed {}: up stairs at ({},{}) not reachable",
                    seed,
                    up.x,
                    up.y,
                );
            }
            if let Some(down) = level.down_stairs {
                assert!(
                    reachable[down.y as usize][down.x as usize],
                    "seed {}: down stairs at ({},{}) not reachable",
                    seed,
                    down.x,
                    down.y,
                );
            }
        }
    }

    // ── Corridor connectivity ───────────────────────────────────

    #[test]
    fn test_dungeon_corridor_full_connectivity() {
        // All rooms must be in the same connected component.
        for seed in 0..50 {
            let mut rng = Pcg64::seed_from_u64(seed + 1000);
            let level = generate_level(5, &mut rng);
            let start = find_first_walkable(&level.map)
                .expect("should have walkable cells");
            let reachable = flood_fill(&level.map, start);

            for (i, room) in level.rooms.iter().enumerate() {
                let ok = (room.y..=room.bottom())
                    .any(|y| (room.x..=room.right()).any(|x| reachable[y][x]));
                assert!(
                    ok,
                    "seed {}: room {} at ({},{}) is disconnected",
                    seed, i, room.x, room.y,
                );
            }
        }
    }

    // ── Door state distribution ─────────────────────────────────

    #[test]
    fn test_dungeon_door_state_distribution() {
        // Generate many doors and verify the distribution roughly
        // matches the spec.
        let mut nodoor = 0u64;
        let mut open = 0u64;
        let mut closed = 0u64;
        let mut locked = 0u64;
        let total = 100_000u64;

        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed);
            let state = choose_door_state(false, 10, false, &mut rng);
            match state {
                DoorState::NoDoor => nodoor += 1,
                DoorState::Open => open += 1,
                DoorState::Closed | DoorState::ClosedTrapped => closed += 1,
                DoorState::Locked | DoorState::LockedTrapped => locked += 1,
                _ => {}
            }
        }

        let nodoor_pct = nodoor as f64 / total as f64;
        let open_pct = open as f64 / total as f64;
        let closed_pct = closed as f64 / total as f64;
        let locked_pct = locked as f64 / total as f64;

        // Spec: D_NODOOR ~2/3 = 0.667
        assert!(
            (nodoor_pct - 0.667).abs() < 0.03,
            "NoDoor should be ~66.7%, got {:.1}%",
            nodoor_pct * 100.0
        );
        // Spec: D_ISOPEN ~1/15 = 0.067
        assert!(
            (open_pct - 0.067).abs() < 0.02,
            "Open should be ~6.7%, got {:.1}%",
            open_pct * 100.0
        );
        // Locked should be much rarer than closed.
        assert!(
            locked_pct < closed_pct,
            "Locked ({:.1}%) should be rarer than Closed ({:.1}%)",
            locked_pct * 100.0,
            closed_pct * 100.0
        );
    }

    #[test]
    fn test_dungeon_door_trapped_requires_difficulty() {
        // At difficulty < 5, regular doors should never be trapped.
        for seed in 0..10_000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let state = choose_door_state(false, 3, false, &mut rng);
            assert!(
                !matches!(
                    state,
                    DoorState::ClosedTrapped | DoorState::LockedTrapped
                ),
                "Doors at difficulty 3 should never be trapped"
            );
        }
    }

    // ── Special room selection ──────────────────────────────────

    #[test]
    fn test_dungeon_special_room_never_at_depth_1() {
        // At depth 1, no special room should be generated.
        for seed in 0..1000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let result = select_special_room(1, 5, false, 25, &mut rng);
            // At depth 1, shop requires depth > 1, and all other types
            // require higher depth. The rn2(depth) < 3 for shop: rn2(1) = 0 < 3
            // is always true, BUT depth must be > 1. So this must be None.
            assert!(
                result.is_none(),
                "Depth 1 should not produce special rooms, got {:?}",
                result
            );
        }
    }

    #[test]
    fn test_dungeon_special_room_shop_at_depth_2() {
        // At depth 2, a shop can appear: rn2(2) < 3 is always true
        // (rn2(2) returns 0 or 1, both < 3).
        let mut shop_count = 0u32;
        for seed in 0..10_000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let result = select_special_room(2, 5, false, 25, &mut rng);
            if result == Some(SpecialRoomType::Shop) {
                shop_count += 1;
            }
        }
        // rn2(2) < 3 is always true at depth 2, so shop is the first
        // check and should pass ~100% of the time.
        assert!(
            shop_count > 9000,
            "Shop should appear in >90% at depth 2, got {}/10000",
            shop_count
        );
    }

    #[test]
    fn test_dungeon_special_room_depth_priority() {
        // At high depth, multiple room types are possible.
        // Verify that each type can be produced over many seeds.
        let mut court_seen = false;
        let mut zoo_seen = false;
        let mut temple_seen = false;
        let mut beehive_seen = false;
        let mut morgue_seen = false;
        let mut barracks_seen = false;

        for seed in 0..50_000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            // depth 20, above medusa (25), few rooms
            let result = select_special_room(20, 2, false, 25, &mut rng);
            match result {
                Some(SpecialRoomType::Court) => court_seen = true,
                Some(SpecialRoomType::Zoo) => zoo_seen = true,
                Some(SpecialRoomType::Temple) => temple_seen = true,
                Some(SpecialRoomType::Beehive) => beehive_seen = true,
                Some(SpecialRoomType::Morgue) => morgue_seen = true,
                Some(SpecialRoomType::Barracks) => barracks_seen = true,
                _ => {}
            }
        }

        assert!(court_seen, "Court should appear at depth 20");
        assert!(zoo_seen, "Zoo should appear at depth 20");
        assert!(temple_seen, "Temple should appear at depth 20");
        assert!(beehive_seen, "Beehive should appear at depth 20");
        assert!(morgue_seen, "Morgue should appear at depth 20");
        assert!(barracks_seen, "Barracks should appear at depth 20");
    }

    #[test]
    fn test_dungeon_special_room_depth_gating() {
        // Court requires depth > 4. At depth 4, it should never appear.
        for seed in 0..10_000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let result = select_special_room(4, 2, false, 25, &mut rng);
            assert!(
                result != Some(SpecialRoomType::Court),
                "Court should not appear at depth 4"
            );
        }
        // Zoo requires depth > 6. At depth 6, no zoo.
        for seed in 0..10_000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let result = select_special_room(6, 2, false, 25, &mut rng);
            assert!(
                result != Some(SpecialRoomType::Zoo),
                "Zoo should not appear at depth 6"
            );
        }
    }

    // ── Room lighting ───────────────────────────────────────────

    #[test]
    fn test_dungeon_lighting_shallow_mostly_lit() {
        // At depth 1, rooms should be lit ~76/77 of the time.
        let mut lit_count = 0u32;
        let total = 10_000u32;
        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            if room_is_lit(1, &mut rng) {
                lit_count += 1;
            }
        }
        let pct = lit_count as f64 / total as f64;
        // rnd(2) in [1,2], both < 11 always. Then rn2(77) != 0: 76/77 ~ 0.987
        assert!(
            pct > 0.96,
            "Depth 1 rooms should be lit >96%, got {:.1}%",
            pct * 100.0,
        );
    }

    #[test]
    fn test_dungeon_lighting_deep_less_lit() {
        // At depth 20, rooms should be lit roughly 50% of the time.
        let mut lit_count = 0u32;
        let total = 10_000u32;
        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            if room_is_lit(20, &mut rng) {
                lit_count += 1;
            }
        }
        let pct = lit_count as f64 / total as f64;
        // rnd(21) in [1,20], P(< 11) = 10/21 ~ 0.476, times 76/77 ~ 0.465
        assert!(
            pct > 0.35 && pct < 0.65,
            "Depth 20 rooms should be lit ~47%, got {:.1}%",
            pct * 100.0,
        );
    }

    // =================================================================
    // H.2: Trap System Alignment
    // =================================================================

    // ── Trap type pool by depth ─────────────────────────────────

    #[test]
    fn test_trap_type_pool_shallow() {
        // At difficulty 1, only basic traps should be generated.
        let mut types_seen = std::collections::HashSet::new();
        for seed in 0..10_000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            if let Some(tt) = traptype_rnd(1, false, false, false, &mut rng) {
                types_seen.insert(tt);
            }
        }
        // Should NOT see spiked pit (min 5), landmine (min 6), web (min 7),
        // statue/poly (min 8), fire (gehennom only).
        assert!(
            !types_seen.contains(&TrapType::SpikedPit),
            "SpikedPit should not appear at difficulty 1"
        );
        assert!(
            !types_seen.contains(&TrapType::Landmine),
            "Landmine should not appear at difficulty 1"
        );
        assert!(
            !types_seen.contains(&TrapType::Web),
            "Web should not appear at difficulty 1"
        );
        assert!(
            !types_seen.contains(&TrapType::StatueTrap),
            "StatueTrap should not appear at difficulty 1"
        );
        assert!(
            !types_seen.contains(&TrapType::PolyTrap),
            "PolyTrap should not appear at difficulty 1"
        );
        assert!(
            !types_seen.contains(&TrapType::FireTrap),
            "FireTrap should not appear outside Gehennom"
        );

        // Should see basic traps.
        assert!(
            types_seen.contains(&TrapType::ArrowTrap),
            "ArrowTrap should appear at difficulty 1"
        );
        assert!(
            types_seen.contains(&TrapType::Pit),
            "Pit should appear at difficulty 1"
        );
        assert!(
            types_seen.contains(&TrapType::BearTrap),
            "BearTrap should appear at difficulty 1"
        );
    }

    #[test]
    fn test_trap_type_pool_deep() {
        // At difficulty 10 (not Gehennom), advanced traps should appear.
        let mut types_seen = std::collections::HashSet::new();
        for seed in 0..20_000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            if let Some(tt) = traptype_rnd(10, false, false, false, &mut rng) {
                types_seen.insert(tt);
            }
        }
        // Should see advanced traps.
        assert!(
            types_seen.contains(&TrapType::SpikedPit),
            "SpikedPit should appear at difficulty 10"
        );
        assert!(
            types_seen.contains(&TrapType::Landmine),
            "Landmine should appear at difficulty 10"
        );
        assert!(
            types_seen.contains(&TrapType::Web),
            "Web should appear at difficulty 10"
        );
        assert!(
            types_seen.contains(&TrapType::StatueTrap),
            "StatueTrap should appear at difficulty 10"
        );
        assert!(
            types_seen.contains(&TrapType::PolyTrap),
            "PolyTrap should appear at difficulty 10"
        );
        // Still no fire trap outside Gehennom.
        assert!(
            !types_seen.contains(&TrapType::FireTrap),
            "FireTrap should not appear outside Gehennom at difficulty 10"
        );
    }

    #[test]
    fn test_trap_fire_trap_gehennom_only() {
        // Fire traps only appear in Gehennom.
        let mut fire_seen = false;
        for seed in 0..10_000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            if let Some(TrapType::FireTrap) =
                traptype_rnd(15, true, false, false, &mut rng)
            {
                fire_seen = true;
                break;
            }
        }
        assert!(
            fire_seen,
            "FireTrap should appear in Gehennom at high difficulty"
        );
    }

    #[test]
    fn test_trap_never_random_portal_vibrating() {
        // MagicPortal, VibratingSquare, TrappedDoor, TrappedChest
        // should never be randomly generated.
        for seed in 0..50_000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            if let Some(tt) = traptype_rnd(30, true, false, false, &mut rng) {
                assert!(
                    !matches!(
                        tt,
                        TrapType::MagicPortal
                            | TrapType::VibratingSquare
                            | TrapType::TrappedDoor
                            | TrapType::TrappedChest
                    ),
                    "Should never randomly generate {:?}",
                    tt
                );
            }
        }
    }

    #[test]
    fn test_trap_no_teleport_on_noteleport_level() {
        // TeleportTrap and LevelTeleport should not appear on
        // no-teleport levels.
        for seed in 0..10_000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            if let Some(tt) = traptype_rnd(10, false, true, false, &mut rng) {
                assert!(
                    !matches!(tt, TrapType::TeleportTrap | TrapType::LevelTeleport),
                    "Teleport traps should not appear on no-teleport levels"
                );
            }
        }
    }

    #[test]
    fn test_trap_hole_rarity() {
        // Hole has only 1/7 chance to be kept when rolled.
        // Over 10000 seeds, holes should be rare.
        let mut hole_count = 0u32;
        let mut total_traps = 0u32;
        for seed in 0..10_000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            if let Some(tt) = traptype_rnd(5, false, false, false, &mut rng) {
                total_traps += 1;
                if tt == TrapType::Hole {
                    hole_count += 1;
                }
            }
        }
        // Hole is rolled 1/25 of the time, then kept 1/7 = ~0.57% of all traps.
        let pct = hole_count as f64 / total_traps.max(1) as f64;
        assert!(
            pct < 0.03,
            "Holes should be very rare (<3%), got {:.1}%",
            pct * 100.0
        );
    }

    // ── Trap placement quantity ─────────────────────────────────

    #[test]
    fn test_trap_room_trap_count_geometric() {
        // room_trap_count follows geometric distribution.
        // At low difficulty, x = max(2, 8-0) = 8, so P(trap) = 1/8.
        // Expected count = (1/8) / (1 - 1/8) = 1/7 ~ 0.14 per room.
        let mut total = 0u32;
        let n = 10_000u32;
        for seed in 0..n {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            total += room_trap_count(1, &mut rng);
        }
        let avg = total as f64 / n as f64;
        // Should be roughly 0.14 (1/7)
        assert!(
            avg < 0.5,
            "Low difficulty avg trap count should be < 0.5, got {:.2}",
            avg
        );
    }

    #[test]
    fn test_trap_maze_trap_count_range() {
        // maze_trap_count should be in [7, 12].
        let mut min_count = u32::MAX;
        let mut max_count = 0u32;
        for seed in 0..1000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let count = maze_trap_count(&mut rng);
            min_count = min_count.min(count);
            max_count = max_count.max(count);
        }
        assert_eq!(min_count, 7, "Maze trap count min should be 7");
        assert_eq!(max_count, 12, "Maze trap count max should be 12");
    }

    // =================================================================
    // H.3: Branch Logic
    // =================================================================

    #[test]
    fn test_dungeon_branch_configs_count() {
        let configs = branch_configs();
        // Should have configs for all 8 branches.
        assert_eq!(
            configs.len(),
            8,
            "Should have 8 branch configs"
        );
    }

    #[test]
    fn test_dungeon_branch_main_depth_range() {
        // Main dungeon: 25 + rn2(5) = [25, 29] levels.
        let mut min_levels = u32::MAX;
        let mut max_levels = 0u32;
        for seed in 0..1000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let count = branch_level_count(DungeonBranch::Main, &mut rng);
            min_levels = min_levels.min(count);
            max_levels = max_levels.max(count);
        }
        assert!(
            min_levels >= 25,
            "Main dungeon min levels should be >= 25, got {}",
            min_levels
        );
        assert!(
            max_levels <= 29,
            "Main dungeon max levels should be <= 29, got {}",
            max_levels
        );
    }

    #[test]
    fn test_dungeon_branch_mines_entry_depth() {
        // Mines entry: base=2, range=3 => depth in [2, 5].
        let mut min_entry = u32::MAX;
        let mut max_entry = 0u32;
        for seed in 0..1000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let entry = mines_entry_depth(&mut rng);
            min_entry = min_entry.min(entry);
            max_entry = max_entry.max(entry);
        }
        assert!(
            min_entry >= 2,
            "Mines entry min should be >= 2, got {}",
            min_entry
        );
        assert!(
            max_entry <= 5,
            "Mines entry max should be <= 5, got {}",
            max_entry
        );
        assert_eq!(min_entry, 2, "Should hit mines entry depth 2");
        assert_eq!(max_entry, 5, "Should hit mines entry depth 5");
    }

    #[test]
    fn test_dungeon_branch_mines_level_count() {
        // Mines: 8 + rn2(2) = [8, 9] levels.
        let mut min_levels = u32::MAX;
        let mut max_levels = 0u32;
        for seed in 0..1000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let count = branch_level_count(DungeonBranch::Mines, &mut rng);
            min_levels = min_levels.min(count);
            max_levels = max_levels.max(count);
        }
        assert_eq!(min_levels, 8, "Mines min levels should be 8");
        assert_eq!(max_levels, 9, "Mines max levels should be 9");
    }

    #[test]
    fn test_dungeon_branch_sokoban_fixed_levels() {
        // Sokoban: 4 + rn2(0) = always 4 levels.
        for seed in 0..100 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let count = branch_level_count(DungeonBranch::Sokoban, &mut rng);
            assert_eq!(count, 4, "Sokoban should always have 4 levels");
        }
    }

    #[test]
    fn test_dungeon_branch_gehennom_properties() {
        let configs = branch_configs();
        let geh = configs
            .iter()
            .find(|c| c.branch == DungeonBranch::Gehennom)
            .unwrap();
        assert!(geh.mazelike, "Gehennom should be mazelike");
        assert!(geh.hellish, "Gehennom should be hellish");
        assert_eq!(geh.base_levels, 20, "Gehennom base levels should be 20");
    }

    #[test]
    fn test_dungeon_branch_quest_level_count() {
        // Quest: 5 + rn2(2) = [5, 6] levels.
        let mut min_levels = u32::MAX;
        let mut max_levels = 0u32;
        for seed in 0..1000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let count = branch_level_count(DungeonBranch::Quest, &mut rng);
            min_levels = min_levels.min(count);
            max_levels = max_levels.max(count);
        }
        assert_eq!(min_levels, 5, "Quest min levels should be 5");
        assert_eq!(max_levels, 6, "Quest max levels should be 6");
    }

    #[test]
    fn test_dungeon_branch_ludios_single_level() {
        // Fort Ludios: 1 + rn2(0) = always 1 level.
        for seed in 0..100 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let count = branch_level_count(DungeonBranch::FortLudios, &mut rng);
            assert_eq!(count, 1, "Fort Ludios should always have 1 level");
        }
    }

    #[test]
    fn test_dungeon_branch_vlads_tower_fixed() {
        // Vlad's Tower: 3 + rn2(0) = always 3 levels.
        for seed in 0..100 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let count =
                branch_level_count(DungeonBranch::VladsTower, &mut rng);
            assert_eq!(count, 3, "Vlad's Tower should always have 3 levels");
        }
    }

    #[test]
    fn test_dungeon_branch_endgame_fixed() {
        // Elemental Planes: 6 + rn2(0) = always 6 levels.
        for seed in 0..100 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let count = branch_level_count(DungeonBranch::Endgame, &mut rng);
            assert_eq!(count, 6, "Endgame should always have 6 levels");
        }
    }

    #[test]
    fn test_dungeon_branch_all_branches_present() {
        // Every DungeonBranch variant should have a config.
        let configs = branch_configs();
        let branches: Vec<DungeonBranch> = vec![
            DungeonBranch::Main,
            DungeonBranch::Mines,
            DungeonBranch::Sokoban,
            DungeonBranch::Quest,
            DungeonBranch::FortLudios,
            DungeonBranch::Gehennom,
            DungeonBranch::VladsTower,
            DungeonBranch::Endgame,
        ];
        for branch in &branches {
            assert!(
                configs.iter().any(|c| c.branch == *branch),
                "Missing config for {:?}",
                branch
            );
        }
    }

    #[test]
    fn test_dungeon_branch_mazelike_flags() {
        let configs = branch_configs();
        // Main dungeon is NOT mazelike.
        let main = configs
            .iter()
            .find(|c| c.branch == DungeonBranch::Main)
            .unwrap();
        assert!(!main.mazelike, "Main dungeon should not be mazelike");

        // Mines IS mazelike.
        let mines = configs
            .iter()
            .find(|c| c.branch == DungeonBranch::Mines)
            .unwrap();
        assert!(mines.mazelike, "Mines should be mazelike");

        // Sokoban IS mazelike.
        let soko = configs
            .iter()
            .find(|c| c.branch == DungeonBranch::Sokoban)
            .unwrap();
        assert!(soko.mazelike, "Sokoban should be mazelike");
    }

    // =================================================================
    // Maze generation tests
    // =================================================================

    #[test]
    fn test_maze_is_connected() {
        // All corridor/floor cells in a generated maze must be reachable
        // from the first corridor cell (flood fill).
        for seed in 0..20u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let map = generate_maze(
                LevelMap::DEFAULT_WIDTH,
                LevelMap::DEFAULT_HEIGHT,
                &mut rng,
            );

            let start = find_first_walkable(&map);
            assert!(
                start.is_some(),
                "seed {}: maze should have walkable cells",
                seed
            );
            let reachable = flood_fill(&map, start.unwrap());

            // Count unreachable corridor/floor cells.
            let mut unreachable = 0usize;
            for y in 0..map.height {
                for x in 0..map.width {
                    let t = map.cells[y][x].terrain;
                    if is_passable(t) && !reachable[y][x] {
                        unreachable += 1;
                    }
                }
            }
            assert_eq!(
                unreachable, 0,
                "seed {}: maze has {} unreachable passable cells",
                seed, unreachable
            );
        }
    }

    #[test]
    fn test_gehennom_has_lava() {
        // A Gehennom level should contain lava terrain.
        for seed in 0..10u64 {
            let mut rng = Pcg64::seed_from_u64(seed + 100);
            let level = generate_gehennom_level(5, &mut rng);
            let lava_count = (0..level.map.height)
                .flat_map(|y| (0..level.map.width).map(move |x| (x, y)))
                .filter(|&(x, y)| level.map.cells[y][x].terrain == Terrain::Lava)
                .count();
            assert!(
                lava_count > 0,
                "seed {}: Gehennom level should have lava, got 0",
                seed
            );
        }
    }

    #[test]
    fn test_gehennom_is_mazelike() {
        // A Gehennom level should have no rectangular rooms (rooms vec is
        // empty) and should be composed of corridors/walls.
        let mut rng = test_rng();
        let level = generate_gehennom_level(5, &mut rng);
        assert!(
            level.rooms.is_empty(),
            "Gehennom level should be mazelike (no rooms), got {} rooms",
            level.rooms.len()
        );

        // Verify there are corridors.
        let corridor_count = (0..level.map.height)
            .flat_map(|y| (0..level.map.width).map(move |x| (x, y)))
            .filter(|&(x, y)| level.map.cells[y][x].terrain == Terrain::Corridor)
            .count();
        assert!(
            corridor_count > 50,
            "Gehennom level should have many corridors, got {}",
            corridor_count
        );
    }

    // =================================================================
    // Vault tests
    // =================================================================

    #[test]
    fn test_vault_placement_creates_2x2_floor() {
        let mut rng = test_rng();
        let mut map = LevelMap::new_standard();
        let rooms = Vec::new();

        let vault = try_place_vault(&mut map, &rooms, &mut rng);
        assert!(vault.is_some(), "Should be able to place a vault on empty map");

        let v = vault.unwrap();
        // Interior should be 2x2 floor.
        for dy in 0..2 {
            for dx in 0..2 {
                let pos = Position::new((v.x + dx) as i32, (v.y + dy) as i32);
                assert_eq!(
                    map.get(pos).unwrap().terrain,
                    Terrain::Floor,
                    "Vault interior at ({}, {}) should be Floor",
                    v.x + dx,
                    v.y + dy
                );
            }
        }
    }

    #[test]
    fn test_vault_has_surrounding_walls() {
        let mut rng = test_rng();
        let mut map = LevelMap::new_standard();
        let rooms = Vec::new();

        let vault = try_place_vault(&mut map, &rooms, &mut rng).unwrap();
        let vx = vault.x;
        let vy = vault.y;

        // All cells around the 2x2 floor should be walls.
        for x in vx.saturating_sub(1)..=vx + 2 {
            for y in vy.saturating_sub(1)..=vy + 2 {
                let inside = x >= vx && x < vx + 2 && y >= vy && y < vy + 2;
                let pos = Position::new(x as i32, y as i32);
                if inside {
                    assert_eq!(map.get(pos).unwrap().terrain, Terrain::Floor);
                } else {
                    assert_eq!(
                        map.get(pos).unwrap().terrain,
                        Terrain::Wall,
                        "Wall expected at ({}, {})",
                        x,
                        y
                    );
                }
            }
        }
    }

    #[test]
    fn test_vault_isolated_from_rooms() {
        let mut rng = Pcg64::seed_from_u64(99);
        let mut map = LevelMap::new_standard();

        // Place a level first.
        let level = generate_level(5, &mut rng);
        // Copy the level's map.
        for y in 0..map.height {
            for x in 0..map.width {
                map.cells[y][x] = level.map.cells[y][x];
            }
        }

        let vault = try_place_vault(&mut map, &level.rooms, &mut rng);
        // Vault placement may fail if the map is too crowded; that's ok.
        if let Some(v) = vault {
            // Verify vault interior is Floor.
            let interior_ok = (0..2).all(|dy| {
                (0..2).all(|dx| {
                    map.get(Position::new((v.x + dx) as i32, (v.y + dy) as i32))
                        .is_some_and(|c| c.terrain == Terrain::Floor)
                })
            });
            assert!(interior_ok, "Vault interior should be Floor");
        }
    }

    #[test]
    fn test_vault_deterministic() {
        let mut map_a = LevelMap::new_standard();
        let mut map_b = LevelMap::new_standard();
        let rooms = Vec::new();

        let vault_a = try_place_vault(&mut map_a, &rooms, &mut Pcg64::seed_from_u64(42));
        let vault_b = try_place_vault(&mut map_b, &rooms, &mut Pcg64::seed_from_u64(42));

        assert_eq!(vault_a, vault_b, "Same seed should produce same vault");
    }

    // =================================================================
    // Shop type tests
    // =================================================================

    #[test]
    fn test_choose_shop_type_shallow_mostly_general() {
        let mut general_count = 0u32;
        let total = 1000u32;
        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            if choose_shop_type(2, &mut rng) == ShopType::General {
                general_count += 1;
            }
        }
        let pct = general_count as f64 / total as f64;
        assert!(
            pct > 0.80,
            "At depth 2, general shops should dominate (>80%), got {:.1}%",
            pct * 100.0
        );
    }

    #[test]
    fn test_choose_shop_type_deep_has_specialty() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..5000u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            seen.insert(choose_shop_type(15, &mut rng));
        }
        // At depth 15, we should see specialty shops.
        assert!(seen.contains(&ShopType::Armor), "Should see Armor shop");
        assert!(seen.contains(&ShopType::Weapons), "Should see Weapons shop");
        assert!(seen.contains(&ShopType::Potions), "Should see Potions shop");
        assert!(seen.contains(&ShopType::Wands), "Should see Wands shop");
        assert!(seen.contains(&ShopType::Rings), "Should see Rings shop");
    }

    #[test]
    fn test_choose_shop_type_all_types_possible() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..10_000u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            seen.insert(choose_shop_type(20, &mut rng));
        }
        assert!(seen.len() >= 10, "Should see at least 10 shop types at depth 20, got {}", seen.len());
    }

    // =================================================================
    // Temple room tests
    // =================================================================

    #[test]
    fn test_configure_temple_room_places_altar() {
        let mut rng = test_rng();
        let mut map = LevelMap::new_standard();
        let room = Room {
            x: 10,
            y: 5,
            width: 6,
            height: 4,
            lit: true,
        };

        // Carve room floor first.
        for y in room.y..=room.bottom() {
            for x in room.x..=room.right() {
                map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
            }
        }

        let (altar_pos, alignment) = configure_temple_room(&mut map, &room, &mut rng);
        assert_eq!(
            map.get(altar_pos).unwrap().terrain,
            Terrain::Altar,
            "Temple room should have an altar at center"
        );
        assert!(
            matches!(
                alignment,
                AltarAlignment::Lawful | AltarAlignment::Neutral | AltarAlignment::Chaotic
            ),
            "Alignment should be one of the three main alignments"
        );
    }

    #[test]
    fn test_temple_altar_at_center() {
        let mut rng = test_rng();
        let mut map = LevelMap::new_standard();
        let room = Room {
            x: 20,
            y: 5,
            width: 8,
            height: 6,
            lit: true,
        };

        let (altar_pos, _) = configure_temple_room(&mut map, &room, &mut rng);
        let (cx, cy) = room.center();
        assert_eq!(altar_pos.x, cx as i32);
        assert_eq!(altar_pos.y, cy as i32);
    }

    #[test]
    fn test_temple_alignment_distribution() {
        let mut lawful = 0u32;
        let mut neutral = 0u32;
        let mut chaotic = 0u32;
        let total = 3000u32;

        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            let mut map = LevelMap::new_standard();
            let room = Room { x: 10, y: 5, width: 6, height: 4, lit: true };
            let (_, alignment) = configure_temple_room(&mut map, &room, &mut rng);
            match alignment {
                AltarAlignment::Lawful => lawful += 1,
                AltarAlignment::Neutral => neutral += 1,
                AltarAlignment::Chaotic => chaotic += 1,
                AltarAlignment::None => {}
            }
        }

        // Each should be ~33%.
        for (name, count) in [("Lawful", lawful), ("Neutral", neutral), ("Chaotic", chaotic)] {
            let pct = count as f64 / total as f64;
            assert!(
                pct > 0.25 && pct < 0.42,
                "{} should be ~33%, got {:.1}%",
                name,
                pct * 100.0
            );
        }
    }

    // =================================================================
    // Special room population tests
    // =================================================================

    #[test]
    fn test_populate_court_places_throne() {
        let mut rng = test_rng();
        let mut map = LevelMap::new_standard();
        let room = Room { x: 10, y: 5, width: 8, height: 5, lit: true };
        for y in room.y..=room.bottom() {
            for x in room.x..=room.right() {
                map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
            }
        }

        populate_special_room(&mut map, &room, SpecialRoomType::Court, &mut rng);

        let (cx, cy) = room.center();
        assert_eq!(
            map.get(Position::new(cx as i32, cy as i32)).unwrap().terrain,
            Terrain::Throne,
        );
    }

    #[test]
    fn test_populate_morgue_places_graves() {
        let mut rng = test_rng();
        let mut map = LevelMap::new_standard();
        let room = Room { x: 10, y: 5, width: 10, height: 6, lit: false };
        for y in room.y..=room.bottom() {
            for x in room.x..=room.right() {
                map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
            }
        }

        populate_special_room(&mut map, &room, SpecialRoomType::Morgue, &mut rng);

        let grave_count = (room.y..=room.bottom())
            .flat_map(|y| (room.x..=room.right()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                map.get(Position::new(x as i32, y as i32))
                    .is_some_and(|c| c.terrain == Terrain::Grave)
            })
            .count();
        assert!(
            grave_count >= 3,
            "Morgue should have >= 3 graves, got {}",
            grave_count
        );
    }

    #[test]
    fn test_populate_swamp_has_pools() {
        let mut rng = test_rng();
        let mut map = LevelMap::new_standard();
        let room = Room { x: 10, y: 5, width: 10, height: 6, lit: false };
        for y in room.y..=room.bottom() {
            for x in room.x..=room.right() {
                map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
            }
        }

        populate_special_room(&mut map, &room, SpecialRoomType::Swamp, &mut rng);

        let pool_count = (room.y..=room.bottom())
            .flat_map(|y| (room.x..=room.right()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                map.get(Position::new(x as i32, y as i32))
                    .is_some_and(|c| c.terrain == Terrain::Pool)
            })
            .count();
        let total = room.width * room.height;
        assert!(
            pool_count > 0,
            "Swamp should have pools"
        );
        assert!(
            pool_count < total,
            "Swamp should not be entirely pools"
        );
    }

    #[test]
    fn test_populate_temple_places_altar() {
        let mut rng = test_rng();
        let mut map = LevelMap::new_standard();
        let room = Room { x: 10, y: 5, width: 6, height: 4, lit: true };
        for y in room.y..=room.bottom() {
            for x in room.x..=room.right() {
                map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
            }
        }

        populate_special_room(&mut map, &room, SpecialRoomType::Temple, &mut rng);

        let (cx, cy) = room.center();
        assert_eq!(
            map.get(Position::new(cx as i32, cy as i32)).unwrap().terrain,
            Terrain::Altar,
        );
    }

    // =================================================================
    // Room floor count test
    // =================================================================

    #[test]
    fn test_room_floor_count() {
        let mut map = LevelMap::new_standard();
        let room = Room { x: 5, y: 5, width: 4, height: 3, lit: true };
        // No floor yet.
        assert_eq!(room_floor_count(&map, &room), 0);

        // Carve the room.
        carve_room(&mut map, &room);
        // Interior is 4x3 = 12 floor tiles.
        assert_eq!(room_floor_count(&map, &room), 12);
    }

    // =================================================================
    // Shop room configuration test
    // =================================================================

    #[test]
    fn test_configure_shop_room_has_door() {
        let mut rng = test_rng();
        let mut map = LevelMap::new_standard();
        let room = Room { x: 10, y: 5, width: 6, height: 4, lit: true };
        carve_room(&mut map, &room);

        configure_shop_room(&mut map, &room, &mut rng);

        // Count closed doors on the room's walls.
        let mut door_count = 0;
        let lx = room.x as i32 - 1;
        let ly = room.y as i32 - 1;
        let hx = room.right() as i32 + 1;
        let hy = room.bottom() as i32 + 1;

        for x in room.x..=room.right() {
            if map.get(Position::new(x as i32, ly)).is_some_and(|c| c.terrain == Terrain::DoorClosed) {
                door_count += 1;
            }
            if map.get(Position::new(x as i32, hy)).is_some_and(|c| c.terrain == Terrain::DoorClosed) {
                door_count += 1;
            }
        }
        for y in room.y..=room.bottom() {
            if map.get(Position::new(lx, y as i32)).is_some_and(|c| c.terrain == Terrain::DoorClosed) {
                door_count += 1;
            }
            if map.get(Position::new(hx, y as i32)).is_some_and(|c| c.terrain == Terrain::DoorClosed) {
                door_count += 1;
            }
        }
        assert!(
            door_count >= 1,
            "Shop room should have at least 1 door, got {}",
            door_count
        );
    }

    // =================================================================
    // Room helper tests
    // =================================================================

    #[test]
    fn test_room_center() {
        let room = Room { x: 10, y: 5, width: 6, height: 4, lit: true };
        assert_eq!(room.center(), (13, 7));
    }

    #[test]
    fn test_room_right_bottom() {
        let room = Room { x: 10, y: 5, width: 6, height: 4, lit: true };
        assert_eq!(room.right(), 15);
        assert_eq!(room.bottom(), 8);
    }

    #[test]
    fn test_room_contains() {
        let room = Room { x: 10, y: 5, width: 6, height: 4, lit: true };
        assert!(room.contains(10, 5));
        assert!(room.contains(15, 8));
        assert!(room.contains(12, 7));
        assert!(!room.contains(9, 5));
        assert!(!room.contains(16, 5));
        assert!(!room.contains(10, 4));
        assert!(!room.contains(10, 9));
    }

    #[test]
    fn test_room_overlaps_with_margin() {
        let a = Room { x: 10, y: 5, width: 5, height: 3, lit: true };
        let b = Room { x: 20, y: 5, width: 5, height: 3, lit: true };
        assert!(!a.overlaps_with_margin(&b, 2), "Distant rooms should not overlap");

        let c = Room { x: 16, y: 5, width: 3, height: 3, lit: true };
        assert!(a.overlaps_with_margin(&c, 2), "Close rooms should overlap with margin=2");
    }

    // =================================================================
    // Room population plan tests
    // =================================================================

    /// Helper: create a room and carve it into a fresh map.
    fn setup_room_map(x: usize, y: usize, w: usize, h: usize) -> (LevelMap, Room) {
        let mut map = LevelMap::new_standard();
        let room = Room { x, y, width: w, height: h, lit: true };
        carve_room(&mut map, &room);
        (map, room)
    }

    #[test]
    fn test_populate_beehive_has_queen_and_bees() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        let pop = plan_room_population(&mut map, &room, SpecialRoomType::Beehive, &mut rng);

        // Should have monsters on floor tiles.
        assert!(!pop.monsters.is_empty(), "Beehive should have monsters");
        // All monsters should be ant class 'a' (bees).
        assert!(
            pop.monsters.iter().all(|m| m.0 == 'a'),
            "All beehive monsters should be class 'a'"
        );
        // First monster (queen) should be at center.
        let (cx, cy) = room.center();
        assert_eq!(
            (pop.monsters[0].1, pop.monsters[0].2),
            (cx, cy),
            "Queen bee should be at room center"
        );
        // Should have royal jelly objects.
        assert!(
            pop.objects.iter().any(|o| o.0 == '%'),
            "Beehive should have royal jelly (food objects)"
        );
        // All monsters should be asleep.
        assert!(
            pop.monsters.iter().all(|m| m.3),
            "All beehive monsters should be asleep"
        );
    }

    #[test]
    fn test_populate_zoo_has_monsters_and_gold() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        let pop = plan_room_population(&mut map, &room, SpecialRoomType::Zoo, &mut rng);

        assert!(!pop.monsters.is_empty(), "Zoo should have monsters");
        assert!(!pop.objects.is_empty(), "Zoo should have gold objects");
        // All monsters should be random class '?' and asleep.
        assert!(
            pop.monsters.iter().all(|m| m.0 == '?' && m.3),
            "Zoo monsters should be random class and asleep"
        );
        // Should have gold objects.
        assert!(
            pop.objects.iter().all(|o| o.0 == '$'),
            "Zoo objects should be gold"
        );
    }

    #[test]
    fn test_populate_barracks_has_soldiers() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        let pop = plan_room_population(&mut map, &room, SpecialRoomType::Barracks, &mut rng);

        assert!(!pop.monsters.is_empty(), "Barracks should have monsters");
        // All should be soldier class '@'.
        assert!(
            pop.monsters.iter().all(|m| m.0 == '@'),
            "Barracks monsters should be soldier class '@'"
        );
        // All asleep.
        assert!(
            pop.monsters.iter().all(|m| m.3),
            "Barracks soldiers should be asleep"
        );
        // Some chests on ~5% of tiles.
        // With 48 tiles, expect ~2-3 chests.
        // But it's random, so just check the objects are chests.
        assert!(
            pop.objects.iter().all(|o| o.0 == '('),
            "Barracks objects should be chests"
        );
    }

    #[test]
    fn test_populate_leprechaun_hall() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        let pop = plan_room_population(
            &mut map, &room, SpecialRoomType::LeprechaunHall, &mut rng,
        );

        assert!(!pop.monsters.is_empty(), "Leprechaun hall should have monsters");
        assert!(
            pop.monsters.iter().all(|m| m.0 == 'l'),
            "Leprechaun hall monsters should be class 'l'"
        );
        assert!(
            pop.objects.iter().all(|o| o.0 == '$'),
            "Leprechaun hall objects should be gold"
        );
    }

    #[test]
    fn test_populate_cockatrice_nest() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        let pop = plan_room_population(
            &mut map, &room, SpecialRoomType::CockatriceNest, &mut rng,
        );

        assert!(!pop.monsters.is_empty(), "Cockatrice nest should have monsters");
        assert!(
            pop.monsters.iter().all(|m| m.0 == 'c'),
            "Cockatrice nest monsters should be class 'c'"
        );
        // Should have statues.
        assert!(
            pop.objects.iter().any(|o| o.0 == '`'),
            "Cockatrice nest should have statues"
        );
    }

    #[test]
    fn test_populate_anthole() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        let pop = plan_room_population(&mut map, &room, SpecialRoomType::Anthole, &mut rng);

        assert!(!pop.monsters.is_empty(), "Anthole should have monsters");
        assert!(
            pop.monsters.iter().all(|m| m.0 == 'a'),
            "Anthole monsters should be ant class 'a'"
        );
        assert!(
            pop.objects.iter().any(|o| o.0 == '%'),
            "Anthole should have food objects"
        );
    }

    #[test]
    fn test_populate_morgue_has_undead_and_graves() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 10, 6);

        let pop = plan_room_population(&mut map, &room, SpecialRoomType::Morgue, &mut rng);

        assert!(!pop.monsters.is_empty(), "Morgue should have undead");
        assert!(
            pop.monsters.iter().all(|m| m.0 == 'Z'),
            "Morgue monsters should be zombie class 'Z'"
        );

        // Should have graves on the map.
        let grave_count = (room.y..=room.bottom())
            .flat_map(|y| (room.x..=room.right()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                map.get(Position::new(x as i32, y as i32))
                    .is_some_and(|c| c.terrain == Terrain::Grave)
            })
            .count();
        assert!(
            grave_count > 0,
            "Morgue should have grave terrain"
        );
    }

    #[test]
    fn test_populate_swamp_has_pools_and_eels() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 10, 6);

        let pop = plan_room_population(&mut map, &room, SpecialRoomType::Swamp, &mut rng);

        // Check pool terrain.
        let pool_count = (room.y..=room.bottom())
            .flat_map(|y| (room.x..=room.right()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                map.get(Position::new(x as i32, y as i32))
                    .is_some_and(|c| c.terrain == Terrain::Pool)
            })
            .count();
        assert!(pool_count > 0, "Swamp should have pool terrain");

        // Should have some monsters (eels ';' or fungi 'F').
        assert!(
            !pop.monsters.is_empty(),
            "Swamp should have creatures"
        );
        assert!(
            pop.monsters.iter().all(|m| m.0 == ';' || m.0 == 'F'),
            "Swamp monsters should be eels or fungi"
        );
    }

    #[test]
    fn test_populate_court_has_throne_and_courtiers() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 5);

        let pop = plan_room_population(&mut map, &room, SpecialRoomType::Court, &mut rng);

        // Throne at center.
        let (cx, cy) = room.center();
        assert_eq!(
            map.get(Position::new(cx as i32, cy as i32)).unwrap().terrain,
            Terrain::Throne,
        );
        // First monster is the king 'K'.
        assert_eq!(pop.monsters[0].0, 'K', "First monster should be king");
        // Rest are courtiers 'o'.
        assert!(
            pop.monsters[1..].iter().all(|m| m.0 == 'o'),
            "Remaining court monsters should be courtiers"
        );
    }

    #[test]
    fn test_populate_temple_has_altar_and_priest() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 6, 4);

        let pop = plan_room_population(&mut map, &room, SpecialRoomType::Temple, &mut rng);

        let (cx, cy) = room.center();
        assert_eq!(
            map.get(Position::new(cx as i32, cy as i32)).unwrap().terrain,
            Terrain::Altar,
        );
        // Should have priest.
        assert!(
            pop.monsters.iter().any(|m| m.0 == 'p'),
            "Temple should have a priest"
        );
    }

    #[test]
    fn test_room_population_struct_fields() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 6, 4);

        let pop = plan_room_population(&mut map, &room, SpecialRoomType::Zoo, &mut rng);

        assert_eq!(pop.room_type, SpecialRoomType::Zoo);
        assert_eq!(pop.room, room);
        // All monster positions should be inside the room.
        for (_, mx, my, _, _) in &pop.monsters {
            assert!(
                room.contains(*mx, *my),
                "Monster at ({}, {}) should be inside room",
                mx, my
            );
        }
        // All object positions should be inside the room.
        for (_, ox, oy) in &pop.objects {
            assert!(
                room.contains(*ox, *oy),
                "Object at ({}, {}) should be inside room",
                ox, oy
            );
        }
    }

    #[test]
    fn test_populate_oroom_is_empty() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 6, 4);

        let pop = plan_room_population(&mut map, &room, SpecialRoomType::ORoom, &mut rng);

        assert!(pop.monsters.is_empty(), "ORoom should have no monsters");
        assert!(pop.objects.is_empty(), "ORoom should have no objects");
    }

    // =================================================================
    // Themed room tests
    // =================================================================

    #[test]
    fn test_themed_room_pillared_hall() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        apply_theme(&mut map, &room, ThemeRoom::PillaredHall, &mut rng);

        // Should have some stone pillars inside the room (not on edges).
        let pillar_count = (room.y..=room.bottom())
            .flat_map(|y| (room.x..=room.right()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                map.get(Position::new(x as i32, y as i32))
                    .is_some_and(|c| c.terrain == Terrain::Stone)
            })
            .count();
        assert!(
            pillar_count > 0,
            "Pillared hall should have stone pillars"
        );
    }

    #[test]
    fn test_themed_room_garden() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        apply_theme(&mut map, &room, ThemeRoom::Garden, &mut rng);

        // Should have a fountain at center.
        let (cx, cy) = room.center();
        assert_eq!(
            map.get(Position::new(cx as i32, cy as i32)).unwrap().terrain,
            Terrain::Fountain,
            "Garden should have fountain at center"
        );

        // Should have some trees.
        let tree_count = (room.y..=room.bottom())
            .flat_map(|y| (room.x..=room.right()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                map.get(Position::new(x as i32, y as i32))
                    .is_some_and(|c| c.terrain == Terrain::Tree)
            })
            .count();
        assert!(
            tree_count > 0,
            "Garden should have trees"
        );
    }

    #[test]
    fn test_themed_room_graveyard_corner() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        apply_theme(&mut map, &room, ThemeRoom::GraveyardCorner, &mut rng);

        let grave_count = (room.y..=room.bottom())
            .flat_map(|y| (room.x..=room.right()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                map.get(Position::new(x as i32, y as i32))
                    .is_some_and(|c| c.terrain == Terrain::Grave)
            })
            .count();
        assert!(
            grave_count >= 2 && grave_count <= 4,
            "Graveyard corner should have 2-4 graves, got {}",
            grave_count
        );
    }

    #[test]
    fn test_themed_room_flooded() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        apply_theme(&mut map, &room, ThemeRoom::FloodedRoom, &mut rng);

        let pool_count = (room.y..=room.bottom())
            .flat_map(|y| (room.x..=room.right()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                map.get(Position::new(x as i32, y as i32))
                    .is_some_and(|c| c.terrain == Terrain::Pool)
            })
            .count();
        assert!(
            pool_count > 0,
            "Flooded room should have pools"
        );
    }

    #[test]
    fn test_themed_room_minor_throne() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        apply_theme(&mut map, &room, ThemeRoom::MinorThrone, &mut rng);

        let (cx, cy) = room.center();
        assert_eq!(
            map.get(Position::new(cx as i32, cy as i32)).unwrap().terrain,
            Terrain::Throne,
            "Minor throne room should have throne at center"
        );
    }

    #[test]
    fn test_themed_room_lava_room() {
        let mut rng = test_rng();
        let (mut map, room) = setup_room_map(10, 5, 8, 6);

        apply_theme(&mut map, &room, ThemeRoom::LavaRoom, &mut rng);

        let lava_count = (room.y..=room.bottom())
            .flat_map(|y| (room.x..=room.right()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                map.get(Position::new(x as i32, y as i32))
                    .is_some_and(|c| c.terrain == Terrain::Lava)
            })
            .count();
        assert!(
            lava_count > 0,
            "Lava room should have lava tiles"
        );
    }

    #[test]
    fn test_maybe_apply_theme_probability() {
        // Over many seeds, ~15% of calls should apply a theme.
        let room = Room { x: 10, y: 5, width: 8, height: 6, lit: true };
        let mut themed_count = 0u32;
        let total = 10_000u32;

        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            let mut map = LevelMap::new_standard();
            carve_room(&mut map, &room);
            if maybe_apply_theme(&mut map, &room, 15, &mut rng).is_some() {
                themed_count += 1;
            }
        }

        let pct = themed_count as f64 / total as f64;
        assert!(
            pct > 0.10 && pct < 0.22,
            "Theme probability should be ~15%, got {:.1}%",
            pct * 100.0
        );
    }

    #[test]
    fn test_maybe_apply_theme_depth_gating() {
        // At depth 1, LavaRoom should never appear (requires depth > 10).
        let room = Room { x: 10, y: 5, width: 8, height: 6, lit: true };
        let mut lava_seen = false;

        for seed in 0..10_000u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let mut map = LevelMap::new_standard();
            carve_room(&mut map, &room);
            if let Some(ThemeRoom::LavaRoom) = maybe_apply_theme(&mut map, &room, 1, &mut rng) {
                lava_seen = true;
                break;
            }
        }
        assert!(
            !lava_seen,
            "LavaRoom should not appear at depth 1"
        );
    }

    // =================================================================
    // Integration: generate_level with special/themed rooms
    // =================================================================

    #[test]
    fn test_generate_level_still_valid_with_special_rooms() {
        // Verify that levels are still valid (connected, stairs present)
        // after adding special room and themed room integration.
        for seed in 0..50u64 {
            let depth = (seed % 25 + 1) as u8;
            let mut rng = Pcg64::seed_from_u64(seed + 5000);
            let level = generate_level(depth, &mut rng);

            assert!(!level.rooms.is_empty(), "seed {}: should have rooms", seed);
            assert!(
                level.down_stairs.is_some(),
                "seed {}: should have down stairs",
                seed
            );

            // Connectivity check.
            if let Some(start) = find_first_walkable(&level.map) {
                let reachable = flood_fill(&level.map, start);
                for (i, room) in level.rooms.iter().enumerate() {
                    let ok = (room.y..=room.bottom())
                        .any(|y| (room.x..=room.right()).any(|x| reachable[y][x]));
                    assert!(
                        ok,
                        "seed {}: room {} unreachable after special room integration",
                        seed, i
                    );
                }
            }
        }
    }

    #[test]
    fn test_generate_level_deep_may_have_special_terrain() {
        // At high depths, some levels should have non-floor terrain in rooms
        // (thrones, altars, graves, pools, etc.) from special rooms or themes.
        let mut has_special_terrain = false;
        for seed in 0..200u64 {
            let mut rng = Pcg64::seed_from_u64(seed + 9000);
            let level = generate_level(20, &mut rng);

            for room in &level.rooms {
                for y in room.y..=room.bottom() {
                    for x in room.x..=room.right() {
                        let t = level.map.cells[y][x].terrain;
                        if matches!(
                            t,
                            Terrain::Throne
                                | Terrain::Altar
                                | Terrain::Grave
                                | Terrain::Pool
                                | Terrain::Fountain
                                | Terrain::Tree
                                | Terrain::Lava
                                | Terrain::Stone
                        ) {
                            has_special_terrain = true;
                        }
                    }
                }
            }
            if has_special_terrain {
                break;
            }
        }
        assert!(
            has_special_terrain,
            "Over 200 seeds at depth 20, at least one level should have special terrain"
        );
    }
}
