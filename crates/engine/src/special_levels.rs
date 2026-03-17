//! Hand-coded special level generators.
//!
//! Generates fixed-layout special levels that deviate from the standard
//! random room-and-corridor algorithm in [`crate::map_gen`].  These
//! correspond to the Lua-defined special levels in classic NetHack 3.7
//! (`dat/*.lua`), but are expressed as pure Rust for the Babel
//! reimplementation.

use rand::Rng;

use crate::action::Position;
use crate::dungeon::{LevelMap, Terrain};
use crate::map_gen::{GeneratedLevel, Room};
use crate::quest::{
    quest_artifact_for_role, quest_enemies_for_role, quest_guardian_for_role,
    quest_leader_for_role, quest_nemesis_for_role,
};
use crate::role::Role;

use nethack_babel_data::level_loader::{
    ascii_to_terrain, get_embedded_level, load_level_from_str, parse_ascii_map,
};
use nethack_babel_data::level_schema::{LevelDefinition, MapDefinition};

// ═══════════════════════════════════════════════════════════════════════════
// Sokoban
// ═══════════════════════════════════════════════════════════════════════════

/// Flags carried alongside a generated special level.
#[derive(Debug, Clone, Default)]
pub struct SpecialLevelFlags {
    /// Digging is forbidden on this level (e.g. Sokoban).
    pub no_dig: bool,
    /// Teleporting is forbidden on this level.
    pub no_teleport: bool,
    /// Prayer does not work on this level (e.g. Gehennom, Sanctum).
    pub no_prayer: bool,
    /// This level is part of the endgame sequence.
    pub is_endgame: bool,
}

/// Result of a special level generator — the level plus extra flags.
#[derive(Debug)]
pub struct SpecialLevel {
    pub generated: GeneratedLevel,
    pub flags: SpecialLevelFlags,
}

/// Monster placement directive produced by special-level population planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialMonsterSpawn {
    pub name: String,
    pub pos: Option<Position>,
    pub chance: u32,
    pub peaceful: Option<bool>,
    pub asleep: Option<bool>,
}

/// Object placement directive produced by special-level population planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialObjectSpawn {
    pub name: String,
    pub pos: Option<Position>,
    pub chance: u32,
    pub quantity: Option<u32>,
}

/// Planned population payload for a generated special level.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecialLevelPopulation {
    pub monsters: Vec<SpecialMonsterSpawn>,
    pub objects: Vec<SpecialObjectSpawn>,
}

impl SpecialLevelPopulation {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.monsters.is_empty() && self.objects.is_empty()
    }
}

/// Which reward sits on the top floor of Sokoban.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SokobanReward {
    BagOfHolding,
    AmuletOfReflection,
}

// ---------------------------------------------------------------------------
// Sokoban puzzle definitions
// ---------------------------------------------------------------------------

/// A Sokoban puzzle is a compact string map plus metadata.
struct SokobanPuzzle {
    /// ASCII map where:
    ///   '#' = wall, '.' = floor, '0' = boulder, '^' = hole/trap,
    ///   '<' = stairs up, '>' = stairs down, ' ' = stone
    map: &'static [&'static str],
    width: usize,
    height: usize,
}

/// Four Sokoban levels, from bottom (easiest) to top (hardest).
/// These are simplified versions of the classic NetHack Sokoban puzzles.
const SOKOBAN_PUZZLES: [SokobanPuzzle; 4] = [
    // Level 1 (bottom, easiest)
    SokobanPuzzle {
        map: &[
            "          ------",
            "          |....|",
            "          |.0..|",
            "-------   |....|",
            "|.....| --|....|",
            "|.0.0.|  .|.0..|",
            "|..0..| ---.0..|",
            "|.0.0.| |....0.|",
            "|.....----.0...|",
            "|..0...........|",
            "|.....----.--..|",
            "----->|   |.0..|",
            "      |   |....|",
            "      -----<---",
        ],
        width: 17,
        height: 14,
    },
    // Level 2
    SokobanPuzzle {
        map: &[
            "  ----          ",
            "  |..|          ",
            "  |..---        ",
            "  |.0..|        ",
            "  |.0.0|        ",
            "--|.0.0|--      ",
            "|..0.0.0.|      ",
            "|........|      ",
            "|..0.0.0.----   ",
            "|--..0......|   ",
            "  |...0.0.0.|   ",
            "  |...------<-  ",
            "  ------->      ",
        ],
        width: 16,
        height: 13,
    },
    // Level 3
    SokobanPuzzle {
        map: &[
            "  --------      ",
            "  |......|      ",
            "  |.0..0.|      ",
            "--|.0..0.|--    ",
            "|...0..0...|   ",
            "|...0..0...|   ",
            "|..0..0..0.|   ",
            "---..0.0..---  ",
            "  |.0..0..|    ",
            "  |..0..0.|    ",
            "  |...>>..|-   ",
            "  |.......|.|  ",
            "  |..0.0.0..|  ",
            "  |........<|  ",
            "  -----------  ",
        ],
        width: 16,
        height: 15,
    },
    // Level 4 (top, hardest — has the reward)
    SokobanPuzzle {
        map: &[
            "  ---------     ",
            "  |.......|-    ",
            "  |.......+.|   ",
            "  |..^^^^.|.|   ",
            "  |..^^^^.|.|   ",
            "--|..^^^^.|.|   ",
            "|.+..^^^^.|.|   ",
            "|.|-......|.|   ",
            "|.||.0..0.|.|   ",
            "|.||.0..0.+.|   ",
            "|.||..0.0.|.|   ",
            "|.||.0..0.|.|   ",
            "|.||.0..0.|-|   ",
            "|.||..0.0.|     ",
            "|.||..>.0.|     ",
            "|.||......|     ",
            "|.||------|     ",
            "|.|..<|         ",
            "|.+...|         ",
            "|-----|         ",
        ],
        width: 16,
        height: 20,
    },
];

/// Generate a Sokoban level.
///
/// `level_num` selects which puzzle (0 = bottom/easiest, 3 = top/hardest).
/// Values outside `0..4` are clamped.
///
/// The returned level has `no_dig` set to `true`.
pub fn generate_sokoban(level_num: u8, rng: &mut impl Rng) -> SpecialLevel {
    let idx = (level_num.min(3)) as usize;
    let puzzle = &SOKOBAN_PUZZLES[idx];

    // Use a map big enough to hold the puzzle centered in the standard area.
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    // Offset to roughly center the puzzle on the map.
    let ox = (map_w.saturating_sub(puzzle.width)) / 2;
    let oy = (map_h.saturating_sub(puzzle.height)) / 2;

    let mut up_stairs: Option<Position> = None;
    let mut down_stairs: Option<Position> = None;
    let mut boulder_positions: Vec<Position> = Vec::new();
    let mut rooms: Vec<Room> = Vec::new();

    // Track the bounding box of all floor tiles to create a pseudo-room.
    let mut min_fx = map_w;
    let mut max_fx = 0usize;
    let mut min_fy = map_h;
    let mut max_fy = 0usize;

    for (row, line) in puzzle.map.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            let mx = ox + col;
            let my = oy + row;
            if mx >= map_w || my >= map_h {
                continue;
            }
            let pos = Position::new(mx as i32, my as i32);
            match ch {
                '#' | '-' | '|' => {
                    map.set_terrain(pos, Terrain::Wall);
                }
                '+' => {
                    // Door (closed).
                    map.set_terrain(pos, Terrain::DoorClosed);
                }
                '.' => {
                    map.set_terrain(pos, Terrain::Floor);
                    min_fx = min_fx.min(mx);
                    max_fx = max_fx.max(mx);
                    min_fy = min_fy.min(my);
                    max_fy = max_fy.max(my);
                }
                '0' => {
                    // Boulder on floor — we mark floor, track boulder
                    // separately (object layer).
                    map.set_terrain(pos, Terrain::Floor);
                    boulder_positions.push(pos);
                    min_fx = min_fx.min(mx);
                    max_fx = max_fx.max(mx);
                    min_fy = min_fy.min(my);
                    max_fy = max_fy.max(my);
                }
                '^' => {
                    // Hole/pit trap — we use Floor and note that objects
                    // layer should place a trap here.
                    map.set_terrain(pos, Terrain::Floor);
                    min_fx = min_fx.min(mx);
                    max_fx = max_fx.max(mx);
                    min_fy = min_fy.min(my);
                    max_fy = max_fy.max(my);
                }
                '<' => {
                    map.set_terrain(pos, Terrain::StairsUp);
                    up_stairs = Some(pos);
                    min_fx = min_fx.min(mx);
                    max_fx = max_fx.max(mx);
                    min_fy = min_fy.min(my);
                    max_fy = max_fy.max(my);
                }
                '>' => {
                    map.set_terrain(pos, Terrain::StairsDown);
                    down_stairs = Some(pos);
                    min_fx = min_fx.min(mx);
                    max_fx = max_fx.max(mx);
                    min_fy = min_fy.min(my);
                    max_fy = max_fy.max(my);
                }
                _ => {
                    // Space or unknown — leave as stone.
                }
            }
        }
    }

    // Create a single pseudo-room encompassing all floor area.
    if max_fx >= min_fx && max_fy >= min_fy {
        rooms.push(Room {
            x: min_fx,
            y: min_fy,
            width: max_fx - min_fx + 1,
            height: max_fy - min_fy + 1,
            lit: true,
        });
    }

    // If this is the top level (level 4), place a reward marker.
    // The actual item creation is handled by the caller; we just note
    // the reward type for the caller via the RNG coin flip.
    let _reward = if idx == 3 {
        if rng.random_bool(0.5) {
            Some(SokobanReward::BagOfHolding)
        } else {
            Some(SokobanReward::AmuletOfReflection)
        }
    } else {
        None
    };

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs,
            down_stairs,
        },
        flags: SpecialLevelFlags {
            no_dig: true,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

/// Count boulders in a Sokoban puzzle definition (for testing).
pub fn sokoban_boulder_count(level_num: u8) -> usize {
    let idx = level_num.min(3) as usize;
    let puzzle = &SOKOBAN_PUZZLES[idx];
    puzzle
        .map
        .iter()
        .flat_map(|line| line.chars())
        .filter(|&c| c == '0')
        .count()
}

// ═══════════════════════════════════════════════════════════════════════════
// Gnomish Mines
// ═══════════════════════════════════════════════════════════════════════════

/// Generate a Mines level with irregular cavern-style rooms, scattered
/// gems/gold, and gnomish inhabitants.
///
/// `depth` is the absolute dungeon depth (used for lighting and loot
/// scaling).  The returned level uses the standard 80x21 dimensions.
pub fn generate_mines_level(depth: u8, rng: &mut impl Rng) -> GeneratedLevel {
    let mut map = LevelMap::new_standard();
    let w = map.width;
    let h = map.height;

    // 1. Place irregular cavern-like rooms.
    let room_count = rng.random_range(4..=7u32) as usize;
    let mut rooms: Vec<Room> = Vec::with_capacity(room_count);

    for _ in 0..room_count * 40 {
        if rooms.len() >= room_count {
            break;
        }
        if let Some(room) = try_place_cavern_room(w, h, &rooms, rng) {
            rooms.push(room);
        }
    }

    rooms.sort_by_key(|r| r.x);

    // 2. Carve cavern rooms — irregular floor shapes.
    for room in &rooms {
        carve_cavern_room(&mut map, room, rng);
    }

    // 3. Connect rooms with winding tunnels.
    connect_mines_rooms(&mut map, &rooms, rng);

    // 4. Scatter gold and gem markers on floor tiles.
    scatter_mines_features(&mut map, rng);

    // 5. Place stairs.
    let (up_stairs, down_stairs) = place_mines_stairs(&mut map, &rooms, depth, rng);

    GeneratedLevel {
        map,
        rooms,
        up_stairs,
        down_stairs,
    }
}

/// Try to place a cavern room with somewhat larger, irregular dimensions.
fn try_place_cavern_room(
    map_w: usize,
    map_h: usize,
    existing: &[Room],
    rng: &mut impl Rng,
) -> Option<Room> {
    let width = rng.random_range(4..=10u32) as usize;
    let height = rng.random_range(3..=6u32) as usize;

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
        lit: rng.random_bool(0.3), // Mines are mostly dark.
    };

    for existing_room in existing {
        if room.overlaps_with_margin(existing_room, 2) {
            return None;
        }
    }

    Some(room)
}

/// Carve an irregular cavern room: start with the rectangular interior,
/// then randomly nibble corners and edges to make it look natural.
fn carve_cavern_room(map: &mut LevelMap, room: &Room, rng: &mut impl Rng) {
    // First lay down walls around the bounding box.
    let lx = room.x as i32 - 1;
    let ly = room.y as i32 - 1;
    let hx = room.right() as i32 + 1;
    let hy = room.bottom() as i32 + 1;

    for x in lx..=hx {
        for y in ly..=hy {
            let pos = Position::new(x, y);
            if !map.in_bounds(pos) {
                continue;
            }
            let on_edge = x == lx || x == hx || y == ly || y == hy;
            if on_edge {
                map.set_terrain(pos, Terrain::Wall);
            } else {
                // Interior: carve floor, but randomly skip corner cells
                // to make it irregular.
                let rx = (x - lx) as usize;
                let ry = (y - ly) as usize;
                let rw = (hx - lx) as usize;
                let rh = (hy - ly) as usize;
                let is_corner = (rx <= 1 || rx >= rw - 1) && (ry <= 1 || ry >= rh - 1);
                if is_corner && rng.random_bool(0.4) {
                    map.set_terrain(pos, Terrain::Wall);
                } else {
                    map.set_terrain(pos, Terrain::Floor);
                }
            }
        }
    }
}

/// Connect mines rooms with corridors.
fn connect_mines_rooms(map: &mut LevelMap, rooms: &[Room], rng: &mut impl Rng) {
    if rooms.len() < 2 {
        return;
    }

    // Sequential connection ensures full connectivity.
    for i in 0..rooms.len() - 1 {
        dig_mines_corridor(map, &rooms[i], &rooms[i + 1], rng);
    }

    // A couple of extra connections for variety.
    if rooms.len() > 2 {
        let extras = rng.random_range(1..=3u32);
        for _ in 0..extras {
            let a = rng.random_range(0..rooms.len());
            let mut b = rng.random_range(0..rooms.len().saturating_sub(1));
            if b >= a {
                b += 1;
            }
            dig_mines_corridor(map, &rooms[a], &rooms[b], rng);
        }
    }
}

/// Dig a corridor between two rooms (L-shaped, same as standard).
fn dig_mines_corridor(map: &mut LevelMap, a: &Room, b: &Room, rng: &mut impl Rng) {
    let (ax, ay) = (
        rng.random_range(a.x..=a.right()) as i32,
        rng.random_range(a.y..=a.bottom()) as i32,
    );
    let (bx, by) = (
        rng.random_range(b.x..=b.right()) as i32,
        rng.random_range(b.y..=b.bottom()) as i32,
    );

    if rng.random_bool(0.5) {
        dig_line(map, ax, bx, ay, true);
        dig_line(map, ay, by, bx, false);
    } else {
        dig_line(map, ay, by, ax, false);
        dig_line(map, ax, bx, by, true);
    }
}

/// Dig a horizontal or vertical line of corridor.
fn dig_line(map: &mut LevelMap, from: i32, to: i32, fixed: i32, horizontal: bool) {
    let (lo, hi) = if from < to { (from, to) } else { (to, from) };
    for v in lo..=hi {
        let pos = if horizontal {
            Position::new(v, fixed)
        } else {
            Position::new(fixed, v)
        };
        if let Some(cell) = map.get(pos)
            && matches!(cell.terrain, Terrain::Stone | Terrain::Wall)
        {
            map.set_terrain(pos, Terrain::Corridor);
        }
    }
}

/// Scatter some fountain features in the mines (gold/gems are items, so
/// we just place a few fountains as terrain markers for flavor).
fn scatter_mines_features(map: &mut LevelMap, rng: &mut impl Rng) {
    let fountain_count = rng.random_range(0..=2u32);
    let mut placed = 0u32;
    for _ in 0..fountain_count * 50 {
        if placed >= fountain_count {
            break;
        }
        let x = rng.random_range(2..map.width - 2) as i32;
        let y = rng.random_range(2..map.height - 2) as i32;
        let pos = Position::new(x, y);
        if let Some(cell) = map.get(pos)
            && cell.terrain == Terrain::Floor
        {
            map.set_terrain(pos, Terrain::Fountain);
            placed += 1;
        }
    }
}

/// Place up/down stairs in the mines.
fn place_mines_stairs(
    map: &mut LevelMap,
    rooms: &[Room],
    depth: u8,
    rng: &mut impl Rng,
) -> (Option<Position>, Option<Position>) {
    if rooms.is_empty() {
        return (None, None);
    }

    let mut up_pos = None;
    if depth > 1 {
        let idx = rng.random_range(0..rooms.len());
        let pos = random_floor_pos(&rooms[idx], rng);
        map.set_terrain(pos, Terrain::StairsUp);
        up_pos = Some(pos);
    }

    let down_idx = if rooms.len() > 1 {
        let mut i = rng.random_range(0..rooms.len());
        if let Some(up) = up_pos {
            for _ in 0..20 {
                if !rooms[i].contains(up.x as usize, up.y as usize) {
                    break;
                }
                i = rng.random_range(0..rooms.len());
            }
        }
        i
    } else {
        0
    };
    let down = random_floor_pos(&rooms[down_idx], rng);
    map.set_terrain(down, Terrain::StairsDown);

    (up_pos, Some(down))
}

/// Pick a random interior position in a room.
fn random_floor_pos(room: &Room, rng: &mut impl Rng) -> Position {
    Position::new(
        rng.random_range(room.x..=room.right()) as i32,
        rng.random_range(room.y..=room.bottom()) as i32,
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// Oracle Level
// ═══════════════════════════════════════════════════════════════════════════

/// Generate the Oracle level.
///
/// Features a central Delphi room with fountains and the Oracle NPC.
/// Surrounded by standard dungeon rooms connected by corridors.
pub fn generate_oracle_level(rng: &mut impl Rng) -> GeneratedLevel {
    let mut map = LevelMap::new_standard();
    let w = map.width;
    let h = map.height;

    // 1. Create the central Delphi room.
    let delphi_w = 11usize;
    let delphi_h = 5usize;
    let delphi_x = (w - delphi_w) / 2;
    let delphi_y = (h - delphi_h) / 2;

    let delphi = Room {
        x: delphi_x,
        y: delphi_y,
        width: delphi_w,
        height: delphi_h,
        lit: true, // Delphi is always lit.
    };

    // Carve the Delphi room with walls.
    carve_oracle_room(&mut map, &delphi);

    // Place fountains in a symmetric pattern inside Delphi.
    // Four fountains at the corners of an inner rectangle.
    let fx1 = delphi_x + 2;
    let fx2 = delphi_x + delphi_w - 3;
    let fy1 = delphi_y + 1;
    let fy2 = delphi_y + delphi_h - 2;

    for &(fx, fy) in &[(fx1, fy1), (fx2, fy1), (fx1, fy2), (fx2, fy2)] {
        let pos = Position::new(fx as i32, fy as i32);
        map.set_terrain(pos, Terrain::Fountain);
    }

    // The Oracle NPC sits at the center of Delphi.
    // (Entity placement is handled by the caller; we just note the
    // position.)
    let _oracle_pos = Position::new(
        (delphi_x + delphi_w / 2) as i32,
        (delphi_y + delphi_h / 2) as i32,
    );

    // 2. Place surrounding rooms.
    let mut rooms: Vec<Room> = vec![delphi.clone()];
    let target_rooms = rng.random_range(3..=5u32) as usize;

    for _ in 0..target_rooms * 30 {
        if rooms.len() > target_rooms {
            break;
        }
        let rw = rng.random_range(3..=8u32) as usize;
        let rh = rng.random_range(3..=5u32) as usize;
        let min_x = 2usize;
        let min_y = 2usize;
        let max_x = w.saturating_sub(rw + 2);
        let max_y = h.saturating_sub(rh + 2);
        if max_x < min_x || max_y < min_y {
            continue;
        }
        let rx = rng.random_range(min_x..=max_x);
        let ry = rng.random_range(min_y..=max_y);
        let candidate = Room {
            x: rx,
            y: ry,
            width: rw,
            height: rh,
            lit: rng.random_bool(0.6),
        };

        let overlaps = rooms.iter().any(|r| candidate.overlaps_with_margin(r, 2));
        if !overlaps {
            carve_oracle_room(&mut map, &candidate);
            rooms.push(candidate);
        }
    }

    // 3. Connect all rooms — Delphi is rooms[0].
    connect_oracle_rooms(&mut map, &rooms, rng);

    // 4. Place doors at room/corridor junctions.
    place_oracle_doors(&mut map, &rooms, rng);

    // 5. Stairs (Oracle level always has both up and down).
    let up_pos = place_stair_in_room(&mut map, &rooms, Terrain::StairsUp, None, rng);
    let down_pos = place_stair_in_room(&mut map, &rooms, Terrain::StairsDown, up_pos, rng);

    GeneratedLevel {
        map,
        rooms,
        up_stairs: up_pos,
        down_stairs: down_pos,
    }
}

/// Carve a rectangular room with walls (same as standard).
fn carve_oracle_room(map: &mut LevelMap, room: &Room) {
    let lx = room.x as i32 - 1;
    let ly = room.y as i32 - 1;
    let hx = room.right() as i32 + 1;
    let hy = room.bottom() as i32 + 1;

    // Walls
    for x in lx..=hx {
        map.set_terrain(Position::new(x, ly), Terrain::Wall);
        map.set_terrain(Position::new(x, hy), Terrain::Wall);
    }
    for y in ly..=hy {
        map.set_terrain(Position::new(lx, y), Terrain::Wall);
        map.set_terrain(Position::new(hx, y), Terrain::Wall);
    }

    // Interior floor
    for y in room.y..=room.bottom() {
        for x in room.x..=room.right() {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }
}

/// Connect Oracle rooms.
fn connect_oracle_rooms(map: &mut LevelMap, rooms: &[Room], rng: &mut impl Rng) {
    if rooms.len() < 2 {
        return;
    }

    // Connect every room to Delphi (rooms[0]) to guarantee connectivity.
    for i in 1..rooms.len() {
        dig_oracle_corridor(map, &rooms[0], &rooms[i], rng);
    }

    // Sequential connections between surrounding rooms.
    for i in 1..rooms.len() - 1 {
        if rng.random_bool(0.5) {
            dig_oracle_corridor(map, &rooms[i], &rooms[i + 1], rng);
        }
    }
}

/// Dig an L-shaped corridor.
fn dig_oracle_corridor(map: &mut LevelMap, a: &Room, b: &Room, rng: &mut impl Rng) {
    let ax = rng.random_range(a.x..=a.right()) as i32;
    let ay = rng.random_range(a.y..=a.bottom()) as i32;
    let bx = rng.random_range(b.x..=b.right()) as i32;
    let by = rng.random_range(b.y..=b.bottom()) as i32;

    if rng.random_bool(0.5) {
        dig_line(map, ax, bx, ay, true);
        dig_line(map, ay, by, bx, false);
    } else {
        dig_line(map, ay, by, ax, false);
        dig_line(map, ax, bx, by, true);
    }
}

/// Place doors where corridors meet room walls (simplified).
fn place_oracle_doors(map: &mut LevelMap, rooms: &[Room], rng: &mut impl Rng) {
    for room in rooms {
        let lx = room.x as i32 - 1;
        let ly = room.y as i32 - 1;
        let hx = room.right() as i32 + 1;
        let hy = room.bottom() as i32 + 1;

        for x in room.x..=room.right() {
            try_place_oracle_door(map, Position::new(x as i32, ly), rng);
            try_place_oracle_door(map, Position::new(x as i32, hy), rng);
        }
        for y in room.y..=room.bottom() {
            try_place_oracle_door(map, Position::new(lx, y as i32), rng);
            try_place_oracle_door(map, Position::new(hx, y as i32), rng);
        }
    }
}

fn try_place_oracle_door(map: &mut LevelMap, pos: Position, _rng: &mut impl Rng) {
    let cell = match map.get(pos) {
        Some(c) => c,
        None => return,
    };
    if cell.terrain != Terrain::Corridor {
        return;
    }
    let has_floor = [(0i32, -1i32), (0, 1), (-1, 0), (1, 0)]
        .iter()
        .any(|&(dx, dy)| {
            map.get(Position::new(pos.x + dx, pos.y + dy))
                .is_some_and(|c| c.terrain == Terrain::Floor)
        });
    if has_floor {
        map.set_terrain(pos, Terrain::DoorOpen);
    }
}

/// Place a stair in one of the rooms (not Delphi if possible).
fn place_stair_in_room(
    map: &mut LevelMap,
    rooms: &[Room],
    terrain: Terrain,
    avoid: Option<Position>,
    rng: &mut impl Rng,
) -> Option<Position> {
    if rooms.is_empty() {
        return None;
    }

    // Prefer non-Delphi rooms (index > 0).
    let start = if rooms.len() > 1 { 1 } else { 0 };
    let mut idx = rng.random_range(start..rooms.len());

    // Avoid placing in the same room as `avoid`.
    if let Some(av) = avoid {
        for _ in 0..20 {
            if !rooms[idx].contains(av.x as usize, av.y as usize) {
                break;
            }
            idx = rng.random_range(start..rooms.len());
        }
    }

    let pos = random_floor_pos(&rooms[idx], rng);
    map.set_terrain(pos, terrain);
    Some(pos)
}

// ═══════════════════════════════════════════════════════════════════════════
// Minetown
// ═══════════════════════════════════════════════════════════════════════════

/// Describes what kind of special room a room in Minetown is.
///
/// Stored per-room in the `room_types` vector that parallels
/// `GeneratedLevel.rooms`.  The caller uses these tags to spawn
/// appropriate entities (shopkeepers, priests, watchmen, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinetownRoomType {
    /// General store shop room.
    GeneralStore,
    /// Specialty shop room.
    SpecialtyShop,
    /// Temple with aligned altar and priest.
    Temple,
    /// Ordinary building / residence.
    Ordinary,
}

/// Result of the Minetown generator.
///
/// In addition to the standard `GeneratedLevel`, Minetown carries
/// per-room type annotations so the caller knows where to spawn
/// shopkeepers, priests, watchmen, etc.
#[derive(Debug)]
pub struct MinetownLevel {
    pub generated: GeneratedLevel,
    /// Parallel to `generated.rooms` -- one entry per room.
    pub room_types: Vec<MinetownRoomType>,
}

/// Generate the Minetown special level.
///
/// Layout: a walled town area in the center of the map containing a
/// general store, 1-2 specialty shops, a temple with an altar, and
/// ordinary buildings.  The perimeter wall has 2-3 entrances.
/// Stairs up/down are placed outside the town walls.
pub fn generate_minetown(rng: &mut impl Rng) -> MinetownLevel {
    let mut map = LevelMap::new_standard();
    let w = map.width;
    let h = map.height;

    // Town occupies a central rectangle, leaving margin for corridors.
    let town_w = rng.random_range(40..=55u32) as usize;
    let town_h = rng.random_range(12..=16u32) as usize;
    let town_x = (w - town_w) / 2;
    let town_y = (h - town_h) / 2;

    // Draw town perimeter walls.
    for x in town_x..=(town_x + town_w) {
        let px = x as i32;
        map.set_terrain(Position::new(px, town_y as i32), Terrain::Wall);
        map.set_terrain(Position::new(px, (town_y + town_h) as i32), Terrain::Wall);
    }
    for y in town_y..=(town_y + town_h) {
        let py = y as i32;
        map.set_terrain(Position::new(town_x as i32, py), Terrain::Wall);
        map.set_terrain(Position::new((town_x + town_w) as i32, py), Terrain::Wall);
    }

    // Fill town interior with floor.
    for y in (town_y + 1)..(town_y + town_h) {
        for x in (town_x + 1)..(town_x + town_w) {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }

    // Place 2-3 entrances in the town wall.
    let entrance_count = rng.random_range(2..=3u32);
    for _ in 0..entrance_count {
        let side = rng.random_range(0..4u32);
        let pos = match side {
            0 => {
                // top
                let ex = rng.random_range((town_x + 2)..(town_x + town_w - 1));
                Position::new(ex as i32, town_y as i32)
            }
            1 => {
                // bottom
                let ex = rng.random_range((town_x + 2)..(town_x + town_w - 1));
                Position::new(ex as i32, (town_y + town_h) as i32)
            }
            2 => {
                // left
                let ey = rng.random_range((town_y + 2)..(town_y + town_h - 1));
                Position::new(town_x as i32, ey as i32)
            }
            _ => {
                // right
                let ey = rng.random_range((town_y + 2)..(town_y + town_h - 1));
                Position::new((town_x + town_w) as i32, ey as i32)
            }
        };
        map.set_terrain(pos, Terrain::DoorOpen);
    }

    // Place buildings inside the town.
    let mut rooms: Vec<Room> = Vec::new();
    let mut room_types: Vec<MinetownRoomType> = Vec::new();

    // Building grid: divide the interior into a 2-row layout.
    let interior_x = town_x + 2;
    let interior_y = town_y + 2;
    let interior_w = town_w - 4;
    let interior_h = town_h - 4;

    // Target: 4-6 buildings.
    let num_buildings = rng.random_range(4..=6u32) as usize;
    let mut placed = 0usize;

    for _ in 0..num_buildings * 40 {
        if placed >= num_buildings {
            break;
        }
        let bw = rng.random_range(4..=8u32) as usize;
        let bh = rng.random_range(3..=5u32) as usize;
        let max_bx = interior_x + interior_w.saturating_sub(bw);
        let max_by = interior_y + interior_h.saturating_sub(bh);
        if max_bx <= interior_x || max_by <= interior_y {
            continue;
        }
        let bx = rng.random_range(interior_x..=max_bx);
        let by = rng.random_range(interior_y..=max_by);

        let candidate = Room {
            x: bx,
            y: by,
            width: bw,
            height: bh,
            lit: true,
        };

        let overlaps = rooms.iter().any(|r| candidate.overlaps_with_margin(r, 2));
        if overlaps {
            continue;
        }

        // Carve building walls and floor.
        carve_walled_room(&mut map, &candidate);

        // Place a door on a random wall of the building.
        place_building_door(&mut map, &candidate, rng);

        rooms.push(candidate);
        placed += 1;
    }

    // Assign room types: first is general store, next 1-2 are specialty
    // shops, one is temple, rest are ordinary.
    for (i, _room) in rooms.iter().enumerate() {
        let rtype = if i == 0 {
            MinetownRoomType::GeneralStore
        } else if i == 1 || (i == 2 && rooms.len() > 3) {
            MinetownRoomType::SpecialtyShop
        } else if (i == 2 && rooms.len() <= 3) || i == 3 {
            MinetownRoomType::Temple
        } else {
            MinetownRoomType::Ordinary
        };
        room_types.push(rtype);
    }

    // Place altar in temple room.
    if let Some(temple_idx) = room_types
        .iter()
        .position(|t| *t == MinetownRoomType::Temple)
    {
        let temple = &rooms[temple_idx];
        let (cx, cy) = temple.center();
        map.set_terrain(Position::new(cx as i32, cy as i32), Terrain::Altar);
    }

    // Place fountain in town square (open area).
    let sq_x = (town_x + town_w / 2) as i32;
    let sq_y = (town_y + town_h / 2) as i32;
    let sq_pos = Position::new(sq_x, sq_y);
    if map.get(sq_pos).is_some_and(|c| c.terrain == Terrain::Floor) {
        map.set_terrain(sq_pos, Terrain::Fountain);
    }

    // Place stairs outside the town perimeter.
    let up_stairs = place_stair_outside_town(&mut map, town_x, town_y, town_w, town_h, rng);
    let down_stairs = place_stair_outside_town(&mut map, town_x, town_y, town_w, town_h, rng);

    // Connect stairs to town via corridors.
    if let Some(up) = up_stairs {
        dig_toward_town(&mut map, up, town_x, town_y, town_w, town_h, rng);
    }
    if let Some(down) = down_stairs {
        dig_toward_town(&mut map, down, town_x, town_y, town_w, town_h, rng);
    }

    MinetownLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs,
            down_stairs,
        },
        room_types,
    }
}

/// Carve a room with walls on the perimeter and floor inside.
fn carve_walled_room(map: &mut LevelMap, room: &Room) {
    let lx = room.x as i32 - 1;
    let ly = room.y as i32 - 1;
    let hx = room.right() as i32 + 1;
    let hy = room.bottom() as i32 + 1;

    for x in lx..=hx {
        for y in ly..=hy {
            let pos = Position::new(x, y);
            if !map.in_bounds(pos) {
                continue;
            }
            let on_edge = x == lx || x == hx || y == ly || y == hy;
            if on_edge {
                map.set_terrain(pos, Terrain::Wall);
            } else {
                map.set_terrain(pos, Terrain::Floor);
            }
        }
    }
}

/// Place a door on a random wall of a building.
fn place_building_door(map: &mut LevelMap, room: &Room, rng: &mut impl Rng) {
    let side = rng.random_range(0..4u32);
    let pos = match side {
        0 => {
            // top wall
            let dx = rng.random_range(room.x..=room.right()) as i32;
            Position::new(dx, room.y as i32 - 1)
        }
        1 => {
            // bottom wall
            let dx = rng.random_range(room.x..=room.right()) as i32;
            Position::new(dx, room.bottom() as i32 + 1)
        }
        2 => {
            // left wall
            let dy = rng.random_range(room.y..=room.bottom()) as i32;
            Position::new(room.x as i32 - 1, dy)
        }
        _ => {
            // right wall
            let dy = rng.random_range(room.y..=room.bottom()) as i32;
            Position::new(room.right() as i32 + 1, dy)
        }
    };
    if map.in_bounds(pos) {
        map.set_terrain(pos, Terrain::DoorClosed);
    }
}

/// Place a stair outside the town perimeter on stone.
fn place_stair_outside_town(
    map: &mut LevelMap,
    town_x: usize,
    town_y: usize,
    town_w: usize,
    town_h: usize,
    rng: &mut impl Rng,
) -> Option<Position> {
    for _ in 0..200 {
        let x = rng.random_range(1..map.width - 1) as i32;
        let y = rng.random_range(1..map.height - 1) as i32;
        let pos = Position::new(x, y);

        // Must be outside town walls.
        let ux = x as usize;
        let uy = y as usize;
        if ux >= town_x && ux <= town_x + town_w && uy >= town_y && uy <= town_y + town_h {
            continue;
        }

        if map.get(pos).is_some_and(|c| c.terrain == Terrain::Stone) {
            map.set_terrain(pos, Terrain::StairsUp);
            return Some(pos);
        }
    }
    None
}

/// Dig a corridor from a stair position toward the nearest town entrance.
fn dig_toward_town(
    map: &mut LevelMap,
    stair: Position,
    town_x: usize,
    town_y: usize,
    town_w: usize,
    town_h: usize,
    rng: &mut impl Rng,
) {
    let target_x = rng.random_range((town_x + 2)..(town_x + town_w - 1)) as i32;
    let target_y = if stair.y < town_y as i32 {
        town_y as i32
    } else {
        (town_y + town_h) as i32
    };

    if rng.random_bool(0.5) {
        dig_line(map, stair.x, target_x, stair.y, true);
        dig_line(map, stair.y, target_y, target_x, false);
    } else {
        dig_line(map, stair.y, target_y, stair.x, false);
        dig_line(map, stair.x, target_x, target_y, true);
    }

    let entry = Position::new(target_x, target_y);
    if map.in_bounds(entry) {
        map.set_terrain(entry, Terrain::DoorOpen);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// The Castle
// ═══════════════════════════════════════════════════════════════════════════

/// Generate the Castle level.
///
/// The castle is a large rectangular fortress with a drawbridge entrance,
/// internal rooms, and a chest containing the wand of wishing.  Soldiers
/// and dragons inhabit the castle.
///
/// Layout:
/// - Large outer wall occupying most of the map width
/// - Moat surrounding the castle on three sides
/// - Drawbridge on the south wall
/// - Internal partition creating sub-rooms
/// - Music note puzzle for drawbridge (handled by caller)
pub fn generate_castle(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    let w = map.width;
    let h = map.height;

    // Castle dimensions: nearly full width, ~14 rows tall.
    let castle_w = rng.random_range(50..=65u32) as usize;
    let castle_h = rng.random_range(11..=15u32) as usize;
    let castle_x = (w - castle_w) / 2;
    let castle_y = (h - castle_h) / 2;

    // 1. Place moat around the castle (1-cell wide ring).
    let moat_x = castle_x.saturating_sub(1);
    let moat_y = castle_y.saturating_sub(1);
    let moat_w = castle_w + 2;
    let moat_h = castle_h + 2;

    for x in moat_x..=(moat_x + moat_w) {
        for y in moat_y..=(moat_y + moat_h) {
            let on_moat_edge =
                x == moat_x || x == moat_x + moat_w || y == moat_y || y == moat_y + moat_h;
            if on_moat_edge {
                let pos = Position::new(x as i32, y as i32);
                if map.in_bounds(pos) {
                    map.set_terrain(pos, Terrain::Moat);
                }
            }
        }
    }

    // 2. Castle outer walls.
    for x in castle_x..=(castle_x + castle_w) {
        let px = x as i32;
        map.set_terrain(Position::new(px, castle_y as i32), Terrain::Wall);
        map.set_terrain(
            Position::new(px, (castle_y + castle_h) as i32),
            Terrain::Wall,
        );
    }
    for y in castle_y..=(castle_y + castle_h) {
        let py = y as i32;
        map.set_terrain(Position::new(castle_x as i32, py), Terrain::Wall);
        map.set_terrain(
            Position::new((castle_x + castle_w) as i32, py),
            Terrain::Wall,
        );
    }

    // 3. Fill castle interior with floor.
    for y in (castle_y + 1)..(castle_y + castle_h) {
        for x in (castle_x + 1)..(castle_x + castle_w) {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }

    // 4. Internal partition wall (horizontal, dividing castle into halves).
    let partition_y = castle_y + castle_h / 2;
    for x in castle_x..=(castle_x + castle_w) {
        map.set_terrain(Position::new(x as i32, partition_y as i32), Terrain::Wall);
    }
    // Doors in the partition.
    let door_count = rng.random_range(2..=4u32);
    for _ in 0..door_count {
        let dx = rng.random_range((castle_x + 2)..(castle_x + castle_w - 1)) as i32;
        map.set_terrain(Position::new(dx, partition_y as i32), Terrain::DoorClosed);
    }

    // 5. Drawbridge on the south wall.
    let drawbridge_x = castle_x + castle_w / 2;
    let drawbridge_pos = Position::new(drawbridge_x as i32, (castle_y + castle_h) as i32);
    map.set_terrain(drawbridge_pos, Terrain::Drawbridge);
    // Also open the moat tile below the drawbridge.
    let moat_below = Position::new(drawbridge_x as i32, (castle_y + castle_h + 1) as i32);
    if map.in_bounds(moat_below) {
        map.set_terrain(moat_below, Terrain::Drawbridge);
    }

    // 6. Build rooms list.
    let upper_room = Room {
        x: castle_x + 1,
        y: castle_y + 1,
        width: castle_w.saturating_sub(2),
        height: (partition_y - castle_y).saturating_sub(1),
        lit: true,
    };
    let lower_room = Room {
        x: castle_x + 1,
        y: partition_y + 1,
        width: castle_w.saturating_sub(2),
        height: (castle_y + castle_h).saturating_sub(partition_y + 1),
        lit: true,
    };
    let rooms = vec![upper_room, lower_room];

    // 7. Place stairs.
    // Stairs up inside the castle (upper half).
    let up_pos = random_floor_in_rect(
        &mut map,
        castle_x + 2,
        castle_y + 1,
        castle_w.saturating_sub(4),
        (partition_y - castle_y).saturating_sub(1),
        Terrain::StairsUp,
        rng,
    );
    // Stairs down outside the castle (below moat).
    let down_pos =
        place_stair_outside_castle(&mut map, castle_x, castle_y, castle_w, castle_h, rng);

    // Dig corridor from stairs down to drawbridge approach.
    if let Some(down) = down_pos {
        let approach_y = (castle_y + castle_h + 2).min(h - 1) as i32;
        dig_line(&mut map, down.x, drawbridge_x as i32, down.y, true);
        dig_line(&mut map, down.y, approach_y, drawbridge_x as i32, false);
    }

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: up_pos,
            down_stairs: down_pos,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: false,
            ..Default::default()
        },
    }
}

/// Place a stair tile inside a rectangular area.
fn random_floor_in_rect(
    map: &mut LevelMap,
    rx: usize,
    ry: usize,
    rw: usize,
    rh: usize,
    terrain: Terrain,
    rng: &mut impl Rng,
) -> Option<Position> {
    if rw == 0 || rh == 0 {
        return None;
    }
    for _ in 0..100 {
        let x = rng.random_range(rx..(rx + rw)) as i32;
        let y = rng.random_range(ry..(ry + rh)) as i32;
        let pos = Position::new(x, y);
        if map.get(pos).is_some_and(|c| c.terrain == Terrain::Floor) {
            map.set_terrain(pos, terrain);
            return Some(pos);
        }
    }
    None
}

/// Place a stair outside the castle moat.
fn place_stair_outside_castle(
    map: &mut LevelMap,
    castle_x: usize,
    castle_y: usize,
    castle_w: usize,
    castle_h: usize,
    rng: &mut impl Rng,
) -> Option<Position> {
    // Place below the castle (south).
    let target_y = castle_y + castle_h + 3;
    if target_y >= map.height {
        return None;
    }
    let target_x = rng.random_range((castle_x + 2)..(castle_x + castle_w - 1));
    let pos = Position::new(target_x as i32, target_y as i32);
    if map.in_bounds(pos) {
        map.set_terrain(pos, Terrain::StairsDown);
        return Some(pos);
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════════
// Medusa's Island
// ═══════════════════════════════════════════════════════════════════════════

/// Generate Medusa's Island.
///
/// An island surrounded by water with Medusa as the boss monster.
/// Statues of former adventurers dot the island.
///
/// `variant` selects between two layout variants (0 or 1).
pub fn generate_medusa(variant: u8, rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    let w = map.width;
    let h = map.height;

    // Fill entire map with water first.
    for y in 0..h {
        for x in 0..w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Water);
        }
    }

    let variant = variant % 2;

    let (island_x, island_y, island_w, island_h) = if variant == 0 {
        // Variant A: central rectangular island.
        let iw = rng.random_range(20..=30u32) as usize;
        let ih = rng.random_range(8..=12u32) as usize;
        let ix = (w - iw) / 2;
        let iy = (h - ih) / 2;
        (ix, iy, iw, ih)
    } else {
        // Variant B: offset island, slightly smaller.
        let iw = rng.random_range(18..=25u32) as usize;
        let ih = rng.random_range(7..=10u32) as usize;
        let ix = (w - iw) / 3;
        let iy = (h - ih) / 2;
        (ix, iy, iw, ih)
    };

    // Carve island: walls on perimeter, floor inside.
    for y in island_y..=(island_y + island_h) {
        for x in island_x..=(island_x + island_w) {
            let pos = Position::new(x as i32, y as i32);
            let on_edge = x == island_x
                || x == island_x + island_w
                || y == island_y
                || y == island_y + island_h;
            if on_edge {
                map.set_terrain(pos, Terrain::Wall);
            } else {
                map.set_terrain(pos, Terrain::Floor);
            }
        }
    }

    // Internal partition: divide the island into two sections.
    let mid_x = island_x + island_w / 2;
    for y in island_y..=(island_y + island_h) {
        map.set_terrain(Position::new(mid_x as i32, y as i32), Terrain::Wall);
    }
    // Door in partition.
    let door_y = rng.random_range((island_y + 1)..(island_y + island_h)) as i32;
    map.set_terrain(Position::new(mid_x as i32, door_y), Terrain::DoorClosed);

    // Place statues (represented as Grave terrain -- closest available
    // terrain to "statue" without adding a new variant).
    let statue_count = rng.random_range(3..=6u32);
    for _ in 0..statue_count {
        let sx = rng.random_range((island_x + 1)..(island_x + island_w)) as i32;
        let sy = rng.random_range((island_y + 1)..(island_y + island_h)) as i32;
        let pos = Position::new(sx, sy);
        if map.get(pos).is_some_and(|c| c.terrain == Terrain::Floor) {
            map.set_terrain(pos, Terrain::Grave);
        }
    }

    // Build rooms list.
    let left_room = Room {
        x: island_x + 1,
        y: island_y + 1,
        width: (mid_x - island_x).saturating_sub(1),
        height: island_h.saturating_sub(1),
        lit: false, // Medusa's lair is dark.
    };
    let right_room = Room {
        x: mid_x + 1,
        y: island_y + 1,
        width: (island_x + island_w).saturating_sub(mid_x + 1),
        height: island_h.saturating_sub(1),
        lit: false,
    };
    let rooms = vec![left_room, right_room];

    // Stairs up inside the island (left half).
    let up_pos = random_floor_in_rect(
        &mut map,
        island_x + 1,
        island_y + 1,
        (mid_x - island_x).saturating_sub(1),
        island_h.saturating_sub(1),
        Terrain::StairsUp,
        rng,
    );

    // Stairs down inside the island (right half).
    let down_pos = random_floor_in_rect(
        &mut map,
        mid_x + 1,
        island_y + 1,
        (island_x + island_w).saturating_sub(mid_x + 1),
        island_h.saturating_sub(1),
        Terrain::StairsDown,
        rng,
    );

    // Small secondary island (variant B only).
    if variant == 1 {
        let si_x = island_x + island_w + 5;
        let si_y = island_y + 2;
        let si_w = 6usize;
        let si_h = 4usize;
        if si_x + si_w < w && si_y + si_h < h {
            for y in si_y..=(si_y + si_h) {
                for x in si_x..=(si_x + si_w) {
                    let pos = Position::new(x as i32, y as i32);
                    let on_edge = x == si_x || x == si_x + si_w || y == si_y || y == si_y + si_h;
                    if on_edge {
                        map.set_terrain(pos, Terrain::Wall);
                    } else {
                        map.set_terrain(pos, Terrain::Floor);
                    }
                }
            }
        }
    }

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: up_pos,
            down_stairs: down_pos,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: false,
            ..Default::default()
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Vlad's Tower
// ═══════════════════════════════════════════════════════════════════════════

/// Which level within Vlad's Tower (1 = bottom, 3 = top).
pub type VladLevel = u8;

/// Generate a single level of Vlad's Tower.
///
/// Vlad's Tower consists of 3 small tower levels connected by stairs.
/// - Level 1 (bottom): entrance from Gehennom, stairs up
/// - Level 2 (middle): stairs up and down
/// - Level 3 (top): Vlad's lair, stairs down only; Candelabrum here
///
/// The tower is a small rectangular area (roughly 20x10) centered on the map.
pub fn generate_vlad_tower(level: VladLevel, rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    // Tower dimensions.
    let tower_w = rng.random_range(12..=18u32) as usize;
    let tower_h = rng.random_range(7..=10u32) as usize;
    let ox = (map_w - tower_w) / 2;
    let oy = (map_h - tower_h) / 2;

    // Carve walls around the tower.
    for x in ox.saturating_sub(1)..=(ox + tower_w) {
        for y in oy.saturating_sub(1)..=(oy + tower_h) {
            let pos = Position::new(x as i32, y as i32);
            if !map.in_bounds(pos) {
                continue;
            }
            let on_edge = x == ox.saturating_sub(1)
                || x == ox + tower_w
                || y == oy.saturating_sub(1)
                || y == oy + tower_h;
            if on_edge {
                map.set_terrain(pos, Terrain::Wall);
            } else if x >= ox && x < ox + tower_w && y >= oy && y < oy + tower_h {
                map.set_terrain(pos, Terrain::Floor);
            }
        }
    }

    // Add some internal walls to create rooms within the tower.
    let mid_x = ox + tower_w / 2;
    for y in oy..oy + tower_h {
        let pos = Position::new(mid_x as i32, y as i32);
        map.set_terrain(pos, Terrain::Wall);
    }
    // Leave a doorway in the internal wall.
    let door_y = oy + tower_h / 2;
    map.set_terrain(
        Position::new(mid_x as i32, door_y as i32),
        Terrain::DoorClosed,
    );

    let rooms = vec![Room {
        x: ox,
        y: oy,
        width: tower_w,
        height: tower_h,
        lit: true,
    }];

    // Place stairs.
    let mut up_stairs = None;
    let mut down_stairs = None;

    let level_num = level.clamp(1, 3);

    // Bottom level (1): stairs up only (entrance from Gehennom is a portal).
    // Middle level (2): both stairs.
    // Top level (3): stairs down only.
    if level_num <= 2 {
        // Stairs up — right side of tower.
        let ux = ox + tower_w - 2;
        let uy = oy + 1;
        let pos = Position::new(ux as i32, uy as i32);
        map.set_terrain(pos, Terrain::StairsUp);
        up_stairs = Some(pos);
    }

    if level_num >= 2 {
        // Stairs down — left side of tower.
        let dx = ox + 1;
        let dy = oy + tower_h - 2;
        let pos = Position::new(dx as i32, dy as i32);
        map.set_terrain(pos, Terrain::StairsDown);
        down_stairs = Some(pos);
    }

    // Scatter graves on the floor for undead atmosphere.
    let grave_count = rng.random_range(2..=5u32);
    let mut placed = 0u32;
    for _ in 0..grave_count * 20 {
        if placed >= grave_count {
            break;
        }
        let gx = rng.random_range(ox..ox + tower_w) as i32;
        let gy = rng.random_range(oy..oy + tower_h) as i32;
        let pos = Position::new(gx, gy);
        if let Some(cell) = map.get(pos)
            && cell.terrain == Terrain::Floor
        {
            map.set_terrain(pos, Terrain::Grave);
            placed += 1;
        }
    }

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs,
            down_stairs,
        },
        flags: SpecialLevelFlags {
            no_dig: true,
            no_teleport: true,
            no_prayer: true,
            is_endgame: false,
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Wizard's Tower
// ═══════════════════════════════════════════════════════════════════════════

/// Generate the Wizard's Tower level.
///
/// The Wizard of Yendor resides here.  Features a magic portal back to
/// the main dungeon and contains the real/fake Amulet of Yendor.
pub fn generate_wizard_tower(_rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    // Tower: central area with multiple rooms.
    let tower_w = 24usize;
    let tower_h = 13usize;
    let ox = (map_w - tower_w) / 2;
    let oy = (map_h - tower_h) / 2;

    // Outer walls.
    for x in ox.saturating_sub(1)..=(ox + tower_w) {
        for y in oy.saturating_sub(1)..=(oy + tower_h) {
            let pos = Position::new(x as i32, y as i32);
            if !map.in_bounds(pos) {
                continue;
            }
            let on_edge = x == ox.saturating_sub(1)
                || x == ox + tower_w
                || y == oy.saturating_sub(1)
                || y == oy + tower_h;
            if on_edge {
                map.set_terrain(pos, Terrain::Wall);
            } else if x >= ox && x < ox + tower_w && y >= oy && y < oy + tower_h {
                map.set_terrain(pos, Terrain::Floor);
            }
        }
    }

    // Internal dividing walls creating 4 rooms.
    let mid_x = ox + tower_w / 2;
    let mid_y = oy + tower_h / 2;
    for y in oy..oy + tower_h {
        map.set_terrain(Position::new(mid_x as i32, y as i32), Terrain::Wall);
    }
    for x in ox..ox + tower_w {
        map.set_terrain(Position::new(x as i32, mid_y as i32), Terrain::Wall);
    }
    // Doorways in internal walls.
    let door1_y = (oy + mid_y) / 2;
    map.set_terrain(
        Position::new(mid_x as i32, door1_y as i32),
        Terrain::DoorOpen,
    );
    let door2_y = (mid_y + oy + tower_h) / 2;
    map.set_terrain(
        Position::new(mid_x as i32, door2_y as i32),
        Terrain::DoorOpen,
    );
    let door3_x = (ox + mid_x) / 2;
    map.set_terrain(
        Position::new(door3_x as i32, mid_y as i32),
        Terrain::DoorOpen,
    );
    let door4_x = (mid_x + ox + tower_w) / 2;
    map.set_terrain(
        Position::new(door4_x as i32, mid_y as i32),
        Terrain::DoorOpen,
    );

    // Magic portal in the bottom-right room.
    let portal_x = mid_x + 2;
    let portal_y = mid_y + 2;
    map.set_terrain(
        Position::new(portal_x as i32, portal_y as i32),
        Terrain::MagicPortal,
    );

    let rooms = vec![Room {
        x: ox,
        y: oy,
        width: tower_w,
        height: tower_h,
        lit: true,
    }];

    // Stairs up (to reach from below).
    let up_pos = Position::new((ox + 2) as i32, (oy + 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: true,
            no_teleport: true,
            no_prayer: true,
            is_endgame: false,
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Sanctum (Temple of Moloch)
// ═══════════════════════════════════════════════════════════════════════════

/// Generate the Sanctum level — the Temple of Moloch.
///
/// The High Priest of Moloch guards the real Amulet of Yendor here.
/// Features:
/// - Central temple with Altar of Moloch
/// - Moat surrounding the temple
/// - Stairs up from Gehennom
pub fn generate_sanctum(_rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    // Fill with walls.
    for y in 0..map_h {
        for x in 0..map_w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
        }
    }

    // Central temple area.
    let temple_w = 20usize;
    let temple_h = 9usize;
    let tx = (map_w - temple_w) / 2;
    let ty = (map_h - temple_h) / 2;

    // Moat ring around the temple (2 cells wide).
    for x in (tx.saturating_sub(3))..=(tx + temple_w + 2) {
        for y in (ty.saturating_sub(3))..=(ty + temple_h + 2) {
            let pos = Position::new(x as i32, y as i32);
            if !map.in_bounds(pos) {
                continue;
            }
            let in_moat = x >= tx.saturating_sub(2)
                && x <= tx + temple_w + 1
                && y >= ty.saturating_sub(2)
                && y <= ty + temple_h + 1;
            let in_temple = x >= tx.saturating_sub(1)
                && x <= tx + temple_w
                && y >= ty.saturating_sub(1)
                && y <= ty + temple_h;
            if in_temple {
                // Will be carved below.
            } else if in_moat {
                map.set_terrain(pos, Terrain::Moat);
            }
        }
    }

    // Carve the temple itself.
    for x in tx.saturating_sub(1)..=(tx + temple_w) {
        for y in ty.saturating_sub(1)..=(ty + temple_h) {
            let pos = Position::new(x as i32, y as i32);
            if !map.in_bounds(pos) {
                continue;
            }
            let on_edge = x == tx.saturating_sub(1)
                || x == tx + temple_w
                || y == ty.saturating_sub(1)
                || y == ty + temple_h;
            if on_edge {
                map.set_terrain(pos, Terrain::Wall);
            } else if x >= tx && x < tx + temple_w && y >= ty && y < ty + temple_h {
                map.set_terrain(pos, Terrain::Floor);
            }
        }
    }

    // Place the Altar of Moloch at the center.
    let altar_x = tx + temple_w / 2;
    let altar_y = ty + temple_h / 2;
    map.set_terrain(
        Position::new(altar_x as i32, altar_y as i32),
        Terrain::Altar,
    );

    // Bridge across the moat (one cell wide on the south side).
    let bridge_x = tx + temple_w / 2;
    for y_off in 0..=3 {
        let pos = Position::new(bridge_x as i32, (ty + temple_h + y_off) as i32);
        if map.in_bounds(pos) {
            map.set_terrain(pos, Terrain::Floor);
        }
    }

    // Corridor leading from the bridge to the edge of the map.
    for y in (ty + temple_h + 3)..map_h - 1 {
        let pos = Position::new(bridge_x as i32, y as i32);
        if map.in_bounds(pos) {
            map.set_terrain(pos, Terrain::Corridor);
        }
    }

    // Stairs up at the bottom edge.
    let stair_pos = Position::new(bridge_x as i32, (map_h - 2) as i32);
    map.set_terrain(stair_pos, Terrain::StairsUp);

    let rooms = vec![Room {
        x: tx,
        y: ty,
        width: temple_w,
        height: temple_h,
        lit: true,
    }];

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(stair_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: true,
            no_teleport: true,
            no_prayer: true,
            is_endgame: false,
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Elemental Planes
// ═══════════════════════════════════════════════════════════════════════════

/// Which elemental plane to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementalPlane {
    Earth,
    Air,
    Fire,
    Water,
}

/// Generate an Elemental Plane level.
///
/// Each plane has characteristic terrain and a magic portal to the next
/// plane in sequence: Earth -> Air -> Fire -> Water -> Astral.
pub fn generate_elemental_plane(plane: ElementalPlane, rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    match plane {
        ElementalPlane::Earth => generate_earth_plane(&mut map, map_w, map_h, rng),
        ElementalPlane::Air => generate_air_plane(&mut map, map_w, map_h, rng),
        ElementalPlane::Fire => generate_fire_plane(&mut map, map_w, map_h, rng),
        ElementalPlane::Water => generate_water_plane(&mut map, map_w, map_h, rng),
    }

    // Place magic portal to the next plane.
    let portal_pos = find_random_floor(&map, rng);
    if let Some(pos) = portal_pos {
        map.set_terrain(pos, Terrain::MagicPortal);
    }

    // Place stairs up (entry from previous plane or Sanctum).
    let stair_pos = find_random_floor(&map, rng);
    if let Some(pos) = stair_pos {
        map.set_terrain(pos, Terrain::StairsUp);
    }

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: Vec::new(),
            up_stairs: stair_pos,
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: true,
            no_teleport: true,
            no_prayer: false,
            is_endgame: true,
        },
    }
}

/// Earth plane: dense stone with tunnels carved through.
fn generate_earth_plane(map: &mut LevelMap, w: usize, h: usize, rng: &mut impl Rng) {
    // Fill with stone.
    for y in 0..h {
        for x in 0..w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Stone);
        }
    }

    // Carve winding tunnels using random walks.
    let num_tunnels = rng.random_range(6..=10u32);
    for _ in 0..num_tunnels {
        let mut cx = rng.random_range(2..w - 2) as i32;
        let mut cy = rng.random_range(2..h - 2) as i32;
        let length = rng.random_range(20..=60u32);
        for _ in 0..length {
            map.set_terrain(Position::new(cx, cy), Terrain::Corridor);
            // Random direction.
            match rng.random_range(0..4u32) {
                0 => cx = (cx + 1).min(w as i32 - 2),
                1 => cx = (cx - 1).max(1),
                2 => cy = (cy + 1).min(h as i32 - 2),
                _ => cy = (cy - 1).max(1),
            }
        }
    }

    // Ensure a few open caverns.
    for _ in 0..3 {
        let cx = rng.random_range(5..w - 5) as i32;
        let cy = rng.random_range(3..h - 3) as i32;
        let rw = rng.random_range(3..=6u32) as i32;
        let rh = rng.random_range(2..=4u32) as i32;
        for y in (cy - rh)..=(cy + rh) {
            for x in (cx - rw)..=(cx + rw) {
                let pos = Position::new(x, y);
                if map.in_bounds(pos) {
                    map.set_terrain(pos, Terrain::Floor);
                }
            }
        }
    }
}

/// Air plane: open space everywhere.
fn generate_air_plane(map: &mut LevelMap, w: usize, h: usize, rng: &mut impl Rng) {
    // Fill with Air terrain.
    for y in 0..h {
        for x in 0..w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Air);
        }
    }

    // Scatter some clouds.
    let cloud_count = rng.random_range(20..=40u32);
    for _ in 0..cloud_count {
        let cx = rng.random_range(0..w) as i32;
        let cy = rng.random_range(0..h) as i32;
        map.set_terrain(Position::new(cx, cy), Terrain::Cloud);
    }

    // A few small floor platforms for items/monsters to stand on.
    for _ in 0..5 {
        let cx = rng.random_range(3..w - 3) as i32;
        let cy = rng.random_range(2..h - 2) as i32;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let pos = Position::new(cx + dx, cy + dy);
                if map.in_bounds(pos) {
                    map.set_terrain(pos, Terrain::Floor);
                }
            }
        }
    }
}

/// Fire plane: lava everywhere with some narrow stone paths.
fn generate_fire_plane(map: &mut LevelMap, w: usize, h: usize, rng: &mut impl Rng) {
    // Fill with lava.
    for y in 0..h {
        for x in 0..w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Lava);
        }
    }

    // Carve stone paths through the lava.
    let num_paths = rng.random_range(4..=8u32);
    for _ in 0..num_paths {
        let mut cx = rng.random_range(2..w - 2) as i32;
        let mut cy = rng.random_range(2..h - 2) as i32;
        let length = rng.random_range(15..=40u32);
        for _ in 0..length {
            map.set_terrain(Position::new(cx, cy), Terrain::Floor);
            match rng.random_range(0..4u32) {
                0 => cx = (cx + 1).min(w as i32 - 2),
                1 => cx = (cx - 1).max(1),
                2 => cy = (cy + 1).min(h as i32 - 2),
                _ => cy = (cy - 1).max(1),
            }
        }
    }

    // A few open areas.
    for _ in 0..3 {
        let cx = rng.random_range(5..w - 5) as i32;
        let cy = rng.random_range(3..h - 3) as i32;
        for dy in -2i32..=2 {
            for dx in -3i32..=3 {
                let pos = Position::new(cx + dx, cy + dy);
                if map.in_bounds(pos) {
                    map.set_terrain(pos, Terrain::Floor);
                }
            }
        }
    }
}

/// Water plane: underwater level.
fn generate_water_plane(map: &mut LevelMap, w: usize, h: usize, rng: &mut impl Rng) {
    // Fill with water.
    for y in 0..h {
        for x in 0..w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Water);
        }
    }

    // Create bubble-like air pockets (floor areas).
    let bubble_count = rng.random_range(8..=15u32);
    for _ in 0..bubble_count {
        let cx = rng.random_range(3..w - 3) as i32;
        let cy = rng.random_range(2..h - 2) as i32;
        let radius = rng.random_range(1..=3i32);
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= radius * radius + 1 {
                    let pos = Position::new(cx + dx, cy + dy);
                    if map.in_bounds(pos) {
                        map.set_terrain(pos, Terrain::Floor);
                    }
                }
            }
        }
    }

    // Connect bubbles with corridors.
    for _ in 0..bubble_count {
        let mut cx = rng.random_range(2..w - 2) as i32;
        let mut cy = rng.random_range(2..h - 2) as i32;
        let length = rng.random_range(10..=25u32);
        for _ in 0..length {
            let pos = Position::new(cx, cy);
            if map.in_bounds(pos)
                && let Some(cell) = map.get(pos)
                && cell.terrain == Terrain::Water
            {
                map.set_terrain(pos, Terrain::Corridor);
            }
            match rng.random_range(0..4u32) {
                0 => cx = (cx + 1).min(w as i32 - 2),
                1 => cx = (cx - 1).max(1),
                2 => cy = (cy + 1).min(h as i32 - 2),
                _ => cy = (cy - 1).max(1),
            }
        }
    }
}

/// Find a random floor/corridor cell in the map (for portal/stair placement).
fn find_random_floor(map: &LevelMap, rng: &mut impl Rng) -> Option<Position> {
    // Collect all walkable cells.
    let mut candidates: Vec<Position> = Vec::new();
    for y in 1..map.height - 1 {
        for x in 1..map.width - 1 {
            let t = map.cells[y][x].terrain;
            if matches!(t, Terrain::Floor | Terrain::Corridor | Terrain::Air) {
                candidates.push(Position::new(x as i32, y as i32));
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let idx = rng.random_range(0..candidates.len());
    Some(candidates[idx])
}

// ═══════════════════════════════════════════════════════════════════════════
// Astral Plane
// ═══════════════════════════════════════════════════════════════════════════

/// Generate the Astral Plane — the final level.
///
/// Features:
/// - Three altars (Lawful, Neutral, Chaotic) in separate temple rooms
/// - Open central area connecting the temples
/// - Entry from the Plane of Water
/// - Offering the real Amulet of Yendor at the correct altar => Ascension
pub fn generate_astral_plane(rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    // Fill with floor to create an open battlefield.
    for y in 0..map_h {
        for x in 0..map_w {
            let pos = Position::new(x as i32, y as i32);
            let on_edge = x == 0 || x == map_w - 1 || y == 0 || y == map_h - 1;
            if on_edge {
                map.set_terrain(pos, Terrain::Wall);
            } else {
                map.set_terrain(pos, Terrain::Floor);
            }
        }
    }

    // Three temple rooms, evenly spaced across the map.
    let temple_w = 7usize;
    let temple_h = 5usize;
    let spacing = map_w / 4;

    let temple_positions = [
        (spacing - temple_w / 2, map_h / 2 - temple_h / 2), // Left (Lawful)
        (2 * spacing - temple_w / 2, map_h / 2 - temple_h / 2), // Center (Neutral)
        (3 * spacing - temple_w / 2, map_h / 2 - temple_h / 2), // Right (Chaotic)
    ];

    let mut rooms = Vec::new();

    for &(tx, ty) in &temple_positions {
        // Walls around the temple.
        for x in tx.saturating_sub(1)..=(tx + temple_w) {
            for y in ty.saturating_sub(1)..=(ty + temple_h) {
                let pos = Position::new(x as i32, y as i32);
                if !map.in_bounds(pos) {
                    continue;
                }
                let on_edge = x == tx.saturating_sub(1)
                    || x == tx + temple_w
                    || y == ty.saturating_sub(1)
                    || y == ty + temple_h;
                if on_edge {
                    map.set_terrain(pos, Terrain::Wall);
                } else if x >= tx && x < tx + temple_w && y >= ty && y < ty + temple_h {
                    map.set_terrain(pos, Terrain::Floor);
                }
            }
        }

        // Doorway on the south side.
        let door_x = tx + temple_w / 2;
        let door_y = ty + temple_h;
        map.set_terrain(
            Position::new(door_x as i32, door_y as i32),
            Terrain::DoorOpen,
        );

        // Altar at center of temple.
        let altar_x = tx + temple_w / 2;
        let altar_y = ty + temple_h / 2;
        map.set_terrain(
            Position::new(altar_x as i32, altar_y as i32),
            Terrain::Altar,
        );

        rooms.push(Room {
            x: tx,
            y: ty,
            width: temple_w,
            height: temple_h,
            lit: true,
        });
    }

    // Place stairs up (entry point) near the bottom center.
    let stair_pos = Position::new((map_w / 2) as i32, (map_h - 3) as i32);
    map.set_terrain(stair_pos, Terrain::StairsUp);

    // Place some cloud terrain for atmosphere.
    let cloud_count = rng.random_range(10..=25u32);
    for _ in 0..cloud_count {
        let cx = rng.random_range(1..map_w - 1) as i32;
        let cy = rng.random_range(1..map_h - 1) as i32;
        let pos = Position::new(cx, cy);
        if let Some(cell) = map.get(pos)
            && cell.terrain == Terrain::Floor
        {
            map.set_terrain(pos, Terrain::Cloud);
        }
    }

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(stair_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: true,
            no_teleport: true,
            no_prayer: false,
            is_endgame: true,
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Fort Ludios
// ═══════════════════════════════════════════════════════════════════════════

/// Generate the Fort Ludios level.
///
/// A large fortress filled with soldiers and gold, accessed via a portal.
///
/// Layout:
/// - Large rectangular fort in the center, surrounded by moat
/// - Internal barracks rooms with soldiers
/// - Throne room with the lieutenant
/// - Treasure room with gold
/// - Single entry via drawbridge from the south
pub fn generate_fort_ludios(rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    // Fort dimensions.
    let fort_w = rng.random_range(45..=60u32) as usize;
    let fort_h = rng.random_range(12..=16u32) as usize;
    let fort_x = (map_w - fort_w) / 2;
    let fort_y = (map_h - fort_h) / 2;

    // 1. Moat around the fort.
    let moat_x = fort_x.saturating_sub(1);
    let moat_y = fort_y.saturating_sub(1);
    let moat_w = fort_w + 2;
    let moat_h = fort_h + 2;

    for x in moat_x..=(moat_x + moat_w) {
        for y in moat_y..=(moat_y + moat_h) {
            let on_moat_edge =
                x == moat_x || x == moat_x + moat_w || y == moat_y || y == moat_y + moat_h;
            if on_moat_edge {
                let pos = Position::new(x as i32, y as i32);
                if map.in_bounds(pos) {
                    map.set_terrain(pos, Terrain::Moat);
                }
            }
        }
    }

    // 2. Fort outer walls.
    for x in fort_x..=(fort_x + fort_w) {
        map.set_terrain(Position::new(x as i32, fort_y as i32), Terrain::Wall);
        map.set_terrain(
            Position::new(x as i32, (fort_y + fort_h) as i32),
            Terrain::Wall,
        );
    }
    for y in fort_y..=(fort_y + fort_h) {
        map.set_terrain(Position::new(fort_x as i32, y as i32), Terrain::Wall);
        map.set_terrain(
            Position::new((fort_x + fort_w) as i32, y as i32),
            Terrain::Wall,
        );
    }

    // 3. Fill interior with floor.
    for y in (fort_y + 1)..(fort_y + fort_h) {
        for x in (fort_x + 1)..(fort_x + fort_w) {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }

    // 4. Internal walls: divide into barracks, throne room, and treasury.
    // Horizontal partition at 1/3 and 2/3 height.
    let row1 = fort_y + fort_h / 3;
    let row2 = fort_y + 2 * fort_h / 3;
    for x in fort_x..=(fort_x + fort_w) {
        map.set_terrain(Position::new(x as i32, row1 as i32), Terrain::Wall);
        map.set_terrain(Position::new(x as i32, row2 as i32), Terrain::Wall);
    }
    // Vertical partition in the top section.
    let col_mid = fort_x + fort_w / 2;
    for y in fort_y..=row1 {
        map.set_terrain(Position::new(col_mid as i32, y as i32), Terrain::Wall);
    }

    // Doors between sections.
    let door_positions = [
        (col_mid, (fort_y + row1) / 2),  // vertical partition door
        (fort_x + fort_w / 4, row1),     // top-left to middle
        (fort_x + 3 * fort_w / 4, row1), // top-right to middle
        (fort_x + fort_w / 3, row2),     // middle to bottom
        (fort_x + 2 * fort_w / 3, row2), // middle to bottom (second)
    ];
    for (dx, dy) in door_positions {
        let pos = Position::new(dx as i32, dy as i32);
        if map.in_bounds(pos) {
            map.set_terrain(pos, Terrain::DoorClosed);
        }
    }

    // 5. Throne in the center of the middle section (commander's room).
    let throne_x = fort_x + fort_w / 2;
    let throne_y = (row1 + row2) / 2;
    map.set_terrain(
        Position::new(throne_x as i32, throne_y as i32),
        Terrain::Throne,
    );

    // 6. Drawbridge on the south wall.
    let drawbridge_x = fort_x + fort_w / 2;
    let drawbridge_pos = Position::new(drawbridge_x as i32, (fort_y + fort_h) as i32);
    map.set_terrain(drawbridge_pos, Terrain::Drawbridge);
    // Open the moat below.
    let moat_below = Position::new(drawbridge_x as i32, (fort_y + fort_h + 1) as i32);
    if map.in_bounds(moat_below) {
        map.set_terrain(moat_below, Terrain::Drawbridge);
    }

    // 7. Build rooms.
    let top_left = Room {
        x: fort_x + 1,
        y: fort_y + 1,
        width: (col_mid - fort_x).saturating_sub(1),
        height: (row1 - fort_y).saturating_sub(1),
        lit: true,
    };
    let top_right = Room {
        x: col_mid + 1,
        y: fort_y + 1,
        width: (fort_x + fort_w).saturating_sub(col_mid + 1),
        height: (row1 - fort_y).saturating_sub(1),
        lit: true,
    };
    let middle = Room {
        x: fort_x + 1,
        y: row1 + 1,
        width: fort_w.saturating_sub(2),
        height: (row2 - row1).saturating_sub(1),
        lit: true,
    };
    let bottom = Room {
        x: fort_x + 1,
        y: row2 + 1,
        width: fort_w.saturating_sub(2),
        height: (fort_y + fort_h).saturating_sub(row2 + 1),
        lit: true,
    };
    let rooms = vec![top_left, top_right, middle, bottom];

    // 8. Magic portal for entry (in the bottom section).
    let portal_x = fort_x + fort_w / 3;
    let portal_y = row2 + 2;
    let portal_pos = Position::new(portal_x as i32, portal_y as i32);
    if map.in_bounds(portal_pos)
        && map
            .get(portal_pos)
            .is_some_and(|c| c.terrain == Terrain::Floor)
    {
        map.set_terrain(portal_pos, Terrain::MagicPortal);
    }

    // 9. Corridor from drawbridge south.
    let _approach_y = (fort_y + fort_h + 2).min(map_h - 1);
    for y in (fort_y + fort_h + 2)..map_h - 1 {
        let pos = Position::new(drawbridge_x as i32, y as i32);
        if map.in_bounds(pos) {
            map.set_terrain(pos, Terrain::Corridor);
        }
    }

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: None, // Accessed via portal, not stairs
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: false,
            ..Default::default()
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Special level dispatch
// ═══════════════════════════════════════════════════════════════════════════

/// Identifies a special level in the dungeon.
///
/// Used by the level generation dispatcher to select the correct generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialLevelId {
    OracleLevel,
    Minetown,
    MinesEnd,
    Sokoban(u8),
    Castle,
    Medusa(u8),
    FortLudios,
    VladsTower(u8),
    WizardTower,
    Sanctum,
    EarthPlane,
    AirPlane,
    FirePlane,
    WaterPlane,
    AstralPlane,
    /// Valley of the Dead (first Gehennom level).
    Valley,
    /// Big room variants.
    BigRoom(u8),
    /// Rogue-style level.
    Rogue,
    /// Asmodeus's Lair.
    Asmodeus,
    /// Baalzebub's Lair.
    Baalzebub,
    /// Juiblex's Swamp.
    Juiblex,
    /// Orcus Town.
    Orcus,
    /// Fake wizard towers.
    FakeWizard(u8),
    /// Wizard Tower level 2.
    WizardTower2,
    /// Wizard Tower level 3.
    WizardTower3,
    /// Quest home level.
    QuestStart,
    /// Quest locate level.
    QuestLocator,
    /// Quest nemesis level.
    QuestGoal,
    /// Quest filler levels.
    QuestFiller(u8),
}

fn room_center_pos(room: &Room) -> Position {
    let (cx, cy) = room.center();
    Position::new(cx as i32, cy as i32)
}

fn room_anchor_positions(room: &Room, exclude: &[Position]) -> Vec<Position> {
    let left = room.x as i32;
    let right = room.right() as i32;
    let top = room.y as i32;
    let bottom = room.bottom() as i32;
    let (cx, cy) = room.center();
    let candidates = [
        Position::new(left, top),
        Position::new(right, top),
        Position::new(left, bottom),
        Position::new(right, bottom),
        Position::new(cx as i32, top),
        Position::new(cx as i32, bottom),
        Position::new(left, cy as i32),
        Position::new(right, cy as i32),
    ];

    let mut anchors = Vec::with_capacity(candidates.len());
    for pos in candidates {
        if exclude.contains(&pos) || anchors.contains(&pos) {
            continue;
        }
        anchors.push(pos);
    }
    anchors
}

fn find_room_containing_pos(rooms: &[Room], pos: Position) -> Option<&Room> {
    rooms.iter().find(|room| {
        room.contains(
            usize::try_from(pos.x).unwrap_or_default(),
            usize::try_from(pos.y).unwrap_or_default(),
        )
    })
}

fn quest_enemy_spawns(
    rooms: &[Room],
    primary: &str,
    secondary: &str,
    total: usize,
) -> Vec<SpecialMonsterSpawn> {
    let mut anchors = Vec::new();
    for room in rooms {
        anchors.extend(room_anchor_positions(room, &[]));
        if anchors.len() >= total {
            break;
        }
    }
    anchors.truncate(total);
    anchors
        .into_iter()
        .enumerate()
        .map(|(idx, pos)| SpecialMonsterSpawn {
            name: if idx % 4 == 3 { secondary } else { primary }.to_string(),
            pos: Some(pos),
            chance: 100,
            peaceful: Some(false),
            asleep: Some(false),
        })
        .collect()
}

fn find_first_terrain(map: &LevelMap, terrain: Terrain) -> Option<Position> {
    for y in 0..map.height {
        for x in 0..map.width {
            let pos = Position::new(x as i32, y as i32);
            if map.get(pos).is_some_and(|cell| cell.terrain == terrain) {
                return Some(pos);
            }
        }
    }
    None
}

fn quest_role_from_name(role_name: Option<&str>) -> Option<Role> {
    match role_name?.trim().to_ascii_lowercase().as_str() {
        "archeologist" => Some(Role::Archeologist),
        "barbarian" => Some(Role::Barbarian),
        "caveman" | "caveperson" => Some(Role::Caveperson),
        "healer" => Some(Role::Healer),
        "knight" => Some(Role::Knight),
        "monk" => Some(Role::Monk),
        "priest" => Some(Role::Priest),
        "ranger" => Some(Role::Ranger),
        "rogue" => Some(Role::Rogue),
        "samurai" => Some(Role::Samurai),
        "tourist" => Some(Role::Tourist),
        "valkyrie" => Some(Role::Valkyrie),
        "wizard" => Some(Role::Wizard),
        _ => None,
    }
}

fn embedded_level_name_for_toml(id: SpecialLevelId) -> Option<&'static str> {
    match id {
        SpecialLevelId::Valley => Some("valley"),
        SpecialLevelId::Asmodeus => Some("asmodeus"),
        SpecialLevelId::Baalzebub => Some("baalzebub"),
        SpecialLevelId::Juiblex => Some("juiblex"),
        SpecialLevelId::Orcus => Some("orcus"),
        SpecialLevelId::FakeWizard(1) => Some("fakewiz1"),
        SpecialLevelId::FakeWizard(2) => Some("fakewiz2"),
        _ => None,
    }
}

fn aligned_map_offsets(map_def: &MapDefinition, map_w: usize, map_h: usize) -> (i32, i32) {
    let (mw, mh, _) = parse_ascii_map(&map_def.data);
    let offset_x = match map_def.halign.as_str() {
        "left" => 1,
        "right" => (map_w as i32 - mw as i32 - 1).max(1),
        _ => ((map_w as i32 - mw as i32) / 2).max(1),
    };
    let offset_y = match map_def.valign.as_str() {
        "top" => 1,
        "bottom" => (map_h as i32 - mh as i32 - 1).max(1),
        _ => ((map_h as i32 - mh as i32) / 2).max(1),
    };
    (offset_x, offset_y)
}

fn population_from_level_definition(def: &LevelDefinition) -> SpecialLevelPopulation {
    let mut pop = SpecialLevelPopulation::default();
    let has_map = def.map.is_some();
    let (offset_x, offset_y) = def
        .map
        .as_ref()
        .map(|map_def| {
            aligned_map_offsets(map_def, LevelMap::DEFAULT_WIDTH, LevelMap::DEFAULT_HEIGHT)
        })
        .unwrap_or((0, 0));

    for mon in &def.monsters {
        let Some(name) = mon
            .id
            .as_ref()
            .cloned()
            .or_else(|| mon.class.as_ref().map(|c| format!("class:{}", c)))
        else {
            continue;
        };

        let pos = match (mon.x, mon.y) {
            (Some(x), Some(y)) => {
                let local_x = if has_map { x - 1 } else { x };
                let local_y = if has_map { y - 1 } else { y };
                Some(Position::new(local_x + offset_x, local_y + offset_y))
            }
            _ => None,
        };

        pop.monsters.push(SpecialMonsterSpawn {
            name,
            pos,
            chance: mon.chance.min(100),
            peaceful: mon.peaceful,
            asleep: mon.asleep,
        });
    }

    for obj in &def.objects {
        let Some(name) = obj
            .id
            .as_ref()
            .cloned()
            .or_else(|| obj.class.as_ref().map(|c| format!("class:{}", c)))
        else {
            continue;
        };

        let pos = match (obj.x, obj.y) {
            (Some(x), Some(y)) => {
                let local_x = if has_map { x - 1 } else { x };
                let local_y = if has_map { y - 1 } else { y };
                Some(Position::new(local_x + offset_x, local_y + offset_y))
            }
            _ => None,
        };

        pop.objects.push(SpecialObjectSpawn {
            name,
            pos,
            chance: obj.chance.min(100),
            quantity: obj.quantity,
        });
    }

    pop
}

fn population_from_embedded_toml(level_name: &str) -> SpecialLevelPopulation {
    let Some(raw) = get_embedded_level(level_name) else {
        return SpecialLevelPopulation::default();
    };
    let Ok(def) = load_level_from_str(raw) else {
        return SpecialLevelPopulation::default();
    };
    population_from_level_definition(&def)
}

fn build_embedded_special_level(id: SpecialLevelId, rng: &mut impl Rng) -> Option<SpecialLevel> {
    let level_name = embedded_level_name_for_toml(id)?;
    let raw = get_embedded_level(level_name)?;
    let def = load_level_from_str(raw).ok()?;
    Some(build_level_from_toml(&def, rng))
}

/// Build population directives for a generated special level.
///
/// This bridges legacy hand-written terrain generators and data-driven
/// level content definitions by returning the actors/items that must be
/// present when the player first enters the level.
pub fn population_for_special_level(
    id: SpecialLevelId,
    generated: &GeneratedLevel,
) -> SpecialLevelPopulation {
    population_for_special_level_with_role(id, generated, None)
}

pub fn population_for_special_level_with_role(
    id: SpecialLevelId,
    generated: &GeneratedLevel,
    role_name: Option<&str>,
) -> SpecialLevelPopulation {
    match id {
        SpecialLevelId::OracleLevel => {
            let pos = generated.rooms.first().map(room_center_pos);
            SpecialLevelPopulation {
                monsters: vec![SpecialMonsterSpawn {
                    name: "Oracle".to_string(),
                    pos,
                    chance: 100,
                    peaceful: Some(true),
                    asleep: Some(false),
                }],
                objects: Vec::new(),
            }
        }
        SpecialLevelId::Medusa(_) => {
            let pos = generated
                .rooms
                .get(1)
                .or_else(|| generated.rooms.first())
                .map(room_center_pos);
            SpecialLevelPopulation {
                monsters: vec![SpecialMonsterSpawn {
                    name: "Medusa".to_string(),
                    pos,
                    chance: 100,
                    peaceful: Some(false),
                    asleep: Some(false),
                }],
                objects: Vec::new(),
            }
        }
        SpecialLevelId::Castle => {
            let pos = generated.rooms.first().map(room_center_pos);
            SpecialLevelPopulation {
                monsters: Vec::new(),
                objects: vec![SpecialObjectSpawn {
                    name: "wand of wishing".to_string(),
                    pos,
                    chance: 100,
                    quantity: Some(1),
                }],
            }
        }
        SpecialLevelId::FortLudios => {
            let mut monsters = Vec::new();
            let room_positions: Vec<Position> =
                generated.rooms.iter().map(room_center_pos).collect();
            let throne_pos = find_first_terrain(&generated.map, Terrain::Throne)
                .or_else(|| room_positions.get(2).copied())
                .or_else(|| room_positions.first().copied());

            monsters.push(SpecialMonsterSpawn {
                name: "lieutenant".to_string(),
                pos: throne_pos,
                chance: 100,
                peaceful: Some(false),
                asleep: Some(false),
            });

            for pos in room_positions.iter().take(2) {
                monsters.push(SpecialMonsterSpawn {
                    name: "soldier".to_string(),
                    pos: Some(*pos),
                    chance: 100,
                    peaceful: Some(false),
                    asleep: Some(false),
                });
            }
            if let Some(pos) = room_positions.get(3).copied().or(throne_pos) {
                monsters.push(SpecialMonsterSpawn {
                    name: "captain".to_string(),
                    pos: Some(pos),
                    chance: 100,
                    peaceful: Some(false),
                    asleep: Some(false),
                });
            }

            SpecialLevelPopulation {
                monsters,
                objects: Vec::new(),
            }
        }
        SpecialLevelId::VladsTower(3) => {
            let pos = generated.rooms.first().map(room_center_pos);
            SpecialLevelPopulation {
                monsters: vec![SpecialMonsterSpawn {
                    name: "Vlad the Impaler".to_string(),
                    pos,
                    chance: 100,
                    peaceful: Some(false),
                    asleep: Some(false),
                }],
                objects: vec![SpecialObjectSpawn {
                    name: "Candelabrum of Invocation".to_string(),
                    pos,
                    chance: 100,
                    quantity: Some(1),
                }],
            }
        }
        SpecialLevelId::WizardTower3 => {
            let pos = generated.rooms.first().map(room_center_pos);
            SpecialLevelPopulation {
                monsters: vec![SpecialMonsterSpawn {
                    name: "Wizard of Yendor".to_string(),
                    pos,
                    chance: 100,
                    peaceful: Some(false),
                    asleep: Some(false),
                }],
                objects: Vec::new(),
            }
        }
        SpecialLevelId::Sanctum => {
            let pos = generated.rooms.first().map(room_center_pos);
            SpecialLevelPopulation {
                monsters: vec![SpecialMonsterSpawn {
                    name: "high priest".to_string(),
                    pos,
                    chance: 100,
                    peaceful: Some(false),
                    asleep: Some(false),
                }],
                objects: Vec::new(),
            }
        }
        SpecialLevelId::QuestStart => {
            let Some(role) = quest_role_from_name(role_name) else {
                return SpecialLevelPopulation::default();
            };
            let Some(room) = generated.rooms.first() else {
                return SpecialLevelPopulation::default();
            };
            let leader_pos = room_center_pos(room);
            let mut monsters = vec![SpecialMonsterSpawn {
                name: quest_leader_for_role(role).to_string(),
                pos: Some(leader_pos),
                chance: 100,
                peaceful: Some(true),
                asleep: Some(false),
            }];
            monsters.extend(
                room_anchor_positions(room, &[leader_pos])
                    .into_iter()
                    .take(4)
                    .map(|pos| SpecialMonsterSpawn {
                        name: quest_guardian_for_role(role).to_string(),
                        pos: Some(pos),
                        chance: 100,
                        peaceful: Some(true),
                        asleep: Some(false),
                    }),
            );
            SpecialLevelPopulation {
                monsters,
                objects: Vec::new(),
            }
        }
        SpecialLevelId::QuestLocator => {
            let Some(role) = quest_role_from_name(role_name) else {
                return SpecialLevelPopulation::default();
            };
            let enemies = quest_enemies_for_role(role);
            SpecialLevelPopulation {
                monsters: quest_enemy_spawns(&generated.rooms, enemies.enemy1, enemies.enemy2, 6),
                objects: Vec::new(),
            }
        }
        SpecialLevelId::QuestGoal => {
            let Some(role) = quest_role_from_name(role_name) else {
                return SpecialLevelPopulation::default();
            };
            let pos = find_first_terrain(&generated.map, Terrain::Altar)
                .or_else(|| generated.rooms.first().map(room_center_pos));
            let Some(nemesis_pos) = pos else {
                return SpecialLevelPopulation::default();
            };
            let escort_room = find_room_containing_pos(&generated.rooms, nemesis_pos)
                .or_else(|| generated.rooms.first());
            let enemies = quest_enemies_for_role(role);
            let mut monsters = vec![SpecialMonsterSpawn {
                name: quest_nemesis_for_role(role).to_string(),
                pos: Some(nemesis_pos),
                chance: 100,
                peaceful: Some(false),
                asleep: Some(false),
            }];
            if let Some(room) = escort_room {
                let escort_names = [enemies.enemy1, enemies.enemy1, enemies.enemy2];
                monsters.extend(
                    room_anchor_positions(room, &[nemesis_pos])
                        .into_iter()
                        .zip(escort_names)
                        .map(|(pos, name)| SpecialMonsterSpawn {
                            name: name.to_string(),
                            pos: Some(pos),
                            chance: 100,
                            peaceful: Some(false),
                            asleep: Some(false),
                        }),
                );
            }
            SpecialLevelPopulation {
                monsters,
                objects: vec![SpecialObjectSpawn {
                    name: quest_artifact_for_role(role).to_string(),
                    pos: Some(nemesis_pos),
                    chance: 100,
                    quantity: Some(1),
                }],
            }
        }
        SpecialLevelId::QuestFiller(_) => {
            let Some(role) = quest_role_from_name(role_name) else {
                return SpecialLevelPopulation::default();
            };
            let enemies = quest_enemies_for_role(role);
            SpecialLevelPopulation {
                monsters: quest_enemy_spawns(&generated.rooms, enemies.enemy1, enemies.enemy2, 4),
                objects: Vec::new(),
            }
        }
        _ => {
            if let Some(name) = embedded_level_name_for_toml(id) {
                return population_from_embedded_toml(name);
            }
            SpecialLevelPopulation::default()
        }
    }
}

/// Check if a branch+depth pair corresponds to a known special level.
///
/// This is a simplified mapping; in full NetHack, the exact depths are
/// randomized per game and stored in the dungeon topology.
pub fn identify_special_level(
    branch: crate::dungeon::DungeonBranch,
    depth: i32,
) -> Option<SpecialLevelId> {
    use crate::dungeon::DungeonBranch;
    match branch {
        DungeonBranch::Main => {
            // Oracle is around depth 5-9 (simplified: depth 7).
            // Castle is at the bottom of the Main branch (depth 25).
            // Medusa is at depth 24.
            match depth {
                25 => Some(SpecialLevelId::Castle),
                24 => Some(SpecialLevelId::Medusa(0)),
                _ => None,
            }
        }
        DungeonBranch::Mines => {
            match depth {
                // Minetown is at depth 5 in the Mines.
                5 => Some(SpecialLevelId::Minetown),
                _ => None,
            }
        }
        DungeonBranch::Sokoban => {
            if depth >= 1 && depth <= 4 {
                Some(SpecialLevelId::Sokoban(depth as u8))
            } else {
                None
            }
        }
        DungeonBranch::FortLudios => Some(SpecialLevelId::FortLudios),
        DungeonBranch::VladsTower => {
            if depth >= 1 && depth <= 3 {
                Some(SpecialLevelId::VladsTower(depth as u8))
            } else {
                None
            }
        }
        DungeonBranch::Gehennom => match depth {
            1 => Some(SpecialLevelId::Valley),
            5 => Some(SpecialLevelId::Juiblex),
            7 => Some(SpecialLevelId::Asmodeus),
            10 => Some(SpecialLevelId::Baalzebub),
            12 => Some(SpecialLevelId::Orcus),
            14 => Some(SpecialLevelId::FakeWizard(1)),
            15 => Some(SpecialLevelId::FakeWizard(2)),
            17 => Some(SpecialLevelId::WizardTower),
            18 => Some(SpecialLevelId::WizardTower2),
            19 => Some(SpecialLevelId::WizardTower3),
            20 => Some(SpecialLevelId::Sanctum),
            _ => None,
        },
        DungeonBranch::Endgame => match depth {
            1 => Some(SpecialLevelId::EarthPlane),
            2 => Some(SpecialLevelId::AirPlane),
            3 => Some(SpecialLevelId::FirePlane),
            4 => Some(SpecialLevelId::WaterPlane),
            5 => Some(SpecialLevelId::AstralPlane),
            _ => None,
        },
        DungeonBranch::Quest => match depth {
            1 => Some(SpecialLevelId::QuestStart),
            4 => Some(SpecialLevelId::QuestLocator),
            7 => Some(SpecialLevelId::QuestGoal),
            d if d >= 2 => Some(SpecialLevelId::QuestFiller(d as u8)),
            _ => None,
        },
    }
}

/// Dispatch to the appropriate special level generator based on the level ID.
/// Returns `None` for level IDs whose generators are not yet implemented
/// (falls back to random level generation).
pub fn dispatch_special_level(
    id: SpecialLevelId,
    _role: Option<&str>,
    rng: &mut impl Rng,
) -> Option<SpecialLevel> {
    match id {
        SpecialLevelId::OracleLevel => {
            let level = generate_oracle_level(rng);
            Some(SpecialLevel {
                generated: level,
                flags: SpecialLevelFlags::default(),
            })
        }
        SpecialLevelId::Minetown => {
            let mt = generate_minetown(rng);
            Some(SpecialLevel {
                generated: mt.generated,
                flags: SpecialLevelFlags::default(),
            })
        }
        SpecialLevelId::MinesEnd => Some(generate_mines_end(rng.random_range(0..3u8), rng)),
        SpecialLevelId::Sokoban(n) => Some(generate_sokoban(n, rng)),
        SpecialLevelId::Castle => Some(generate_castle(rng)),
        SpecialLevelId::Medusa(v) => Some(generate_medusa(v, rng)),
        SpecialLevelId::FortLudios => Some(generate_fort_ludios(rng)),
        SpecialLevelId::VladsTower(n) => Some(generate_vlad_tower(n, rng)),
        SpecialLevelId::WizardTower => Some(generate_wizard_tower(rng)),
        SpecialLevelId::Sanctum => Some(generate_sanctum(rng)),
        SpecialLevelId::EarthPlane => Some(generate_elemental_plane(ElementalPlane::Earth, rng)),
        SpecialLevelId::AirPlane => Some(generate_elemental_plane(ElementalPlane::Air, rng)),
        SpecialLevelId::FirePlane => Some(generate_elemental_plane(ElementalPlane::Fire, rng)),
        SpecialLevelId::WaterPlane => Some(generate_elemental_plane(ElementalPlane::Water, rng)),
        SpecialLevelId::AstralPlane => Some(generate_astral_plane(rng)),
        SpecialLevelId::Valley
        | SpecialLevelId::Asmodeus
        | SpecialLevelId::Baalzebub
        | SpecialLevelId::Juiblex
        | SpecialLevelId::Orcus
        | SpecialLevelId::FakeWizard(1)
        | SpecialLevelId::FakeWizard(2) => build_embedded_special_level(id, rng),
        SpecialLevelId::BigRoom(v) => Some(generate_big_room(v, rng)),
        SpecialLevelId::Rogue => Some(generate_rogue_level(15, rng)),
        SpecialLevelId::FakeWizard(_) => Some(generate_fake_wizard(rng)),
        SpecialLevelId::WizardTower2 => Some(generate_wizard_tower_upper(2, rng)),
        SpecialLevelId::WizardTower3 => Some(generate_wizard_tower_upper(3, rng)),
        SpecialLevelId::QuestStart => Some(generate_quest_start(_role.unwrap_or("valkyrie"), rng)),
        SpecialLevelId::QuestLocator => {
            Some(generate_quest_locator(_role.unwrap_or("valkyrie"), rng))
        }
        SpecialLevelId::QuestGoal => Some(generate_quest_goal(_role.unwrap_or("valkyrie"), rng)),
        SpecialLevelId::QuestFiller(_) => {
            Some(generate_quest_filler(_role.unwrap_or("valkyrie"), rng))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Gehennom special levels
// ═══════════════════════════════════════════════════════════════════════════

/// Generate the Valley of the Dead — the first level of Gehennom.
///
/// Features a large room with an altar, surrounded by walls.
/// No-teleport zone; undead are prevalent here.
pub fn generate_valley(rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    // Fill with walls initially.
    for y in 0..map_h {
        for x in 0..map_w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
        }
    }

    // Central valley room.
    let room_w = rng.random_range(30..=45u32) as usize;
    let room_h = rng.random_range(10..=14u32) as usize;
    let rx = (map_w - room_w) / 2;
    let ry = (map_h - room_h) / 2;

    for y in (ry + 1)..(ry + room_h) {
        for x in (rx + 1)..(rx + room_w) {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }

    // Place an altar at the center.
    let altar_x = rx + room_w / 2;
    let altar_y = ry + room_h / 2;
    map.set_terrain(
        Position::new(altar_x as i32, altar_y as i32),
        Terrain::Altar,
    );

    // Place some graves (representing the dead).
    for i in 0..rng.random_range(4..=8u32) {
        let gx = rx + 2 + (i as usize * 3) % (room_w - 4);
        let gy = ry + 2 + (i as usize) % (room_h - 4);
        if map.cells[gy][gx].terrain == Terrain::Floor {
            map.set_terrain(Position::new(gx as i32, gy as i32), Terrain::Grave);
        }
    }

    // Corridor from south wall to map edge.
    let corridor_x = rx + room_w / 2;
    for y in (ry + room_h)..map_h.saturating_sub(1) {
        map.set_terrain(
            Position::new(corridor_x as i32, y as i32),
            Terrain::Corridor,
        );
    }

    // Corridor from north wall up.
    for y in 1..ry {
        map.set_terrain(
            Position::new(corridor_x as i32, y as i32),
            Terrain::Corridor,
        );
    }

    // Stairs.
    let up_pos = Position::new(corridor_x as i32, (map_h - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    let down_pos = Position::new(corridor_x as i32, 1);
    map.set_terrain(down_pos, Terrain::StairsDown);

    let rooms = vec![Room {
        x: rx,
        y: ry,
        width: room_w,
        height: room_h,
        lit: false,
    }];

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: true,
            is_endgame: false,
        },
    }
}

/// Generate Asmodeus's Lair.
///
/// A two-part room with Asmodeus at the center. Fire traps surround the lair.
pub fn generate_asmodeus(rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    for y in 0..map_h {
        for x in 0..map_w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
        }
    }

    // Lair dimensions.
    let lair_w = rng.random_range(22..=30u32) as usize;
    let lair_h = rng.random_range(10..=14u32) as usize;
    let ox = (map_w - lair_w) / 2;
    let oy = (map_h - lair_h) / 2;

    // Carve interior.
    for y in (oy + 1)..(oy + lair_h) {
        for x in (ox + 1)..(ox + lair_w) {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }

    // Internal partition creating two chambers.
    let mid_x = ox + lair_w / 2;
    for y in (oy + 1)..(oy + lair_h) {
        map.set_terrain(Position::new(mid_x as i32, y as i32), Terrain::Wall);
    }
    // Doorway.
    let door_y = oy + lair_h / 2;
    map.set_terrain(
        Position::new(mid_x as i32, door_y as i32),
        Terrain::DoorClosed,
    );

    // Corridors to edges.
    let corr_x = ox + lair_w / 4;
    for y in 1..oy {
        map.set_terrain(Position::new(corr_x as i32, y as i32), Terrain::Corridor);
    }
    // Opening into lair.
    map.set_terrain(Position::new(corr_x as i32, oy as i32), Terrain::DoorOpen);
    for y in (oy + lair_h)..map_h.saturating_sub(1) {
        map.set_terrain(Position::new(corr_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(corr_x as i32, (oy + lair_h) as i32),
        Terrain::DoorOpen,
    );

    // Stairs.
    let up_pos = Position::new(corr_x as i32, (map_h - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    let down_pos = Position::new(corr_x as i32, 1);
    map.set_terrain(down_pos, Terrain::StairsDown);

    let rooms = vec![Room {
        x: ox,
        y: oy,
        width: lair_w,
        height: lair_h,
        lit: false,
    }];

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: false,
            no_prayer: true,
            is_endgame: false,
        },
    }
}

/// Generate Baalzebub's Lair.
///
/// Connected corridors with fire traps; Baalzebub waits within.
pub fn generate_baalzebub(rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    for y in 0..map_h {
        for x in 0..map_w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
        }
    }

    // Central lair.
    let lair_w = rng.random_range(20..=28u32) as usize;
    let lair_h = rng.random_range(8..=12u32) as usize;
    let ox = (map_w - lair_w) / 2;
    let oy = (map_h - lair_h) / 2;

    for y in (oy + 1)..(oy + lair_h) {
        for x in (ox + 1)..(ox + lair_w) {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }

    // Some lava pools inside the lair.
    for i in 0..rng.random_range(3..=6u32) {
        let lx = ox + 2 + (i as usize * 4) % (lair_w - 4);
        let ly = oy + 2 + (i as usize * 2) % (lair_h - 4);
        if map.cells[ly][lx].terrain == Terrain::Floor {
            map.set_terrain(Position::new(lx as i32, ly as i32), Terrain::Lava);
        }
    }

    // Corridors north and south.
    let corr_x = ox + lair_w / 2;
    for y in 1..oy {
        map.set_terrain(Position::new(corr_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(Position::new(corr_x as i32, oy as i32), Terrain::DoorOpen);
    for y in (oy + lair_h)..map_h.saturating_sub(1) {
        map.set_terrain(Position::new(corr_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(corr_x as i32, (oy + lair_h) as i32),
        Terrain::DoorOpen,
    );

    let up_pos = Position::new(corr_x as i32, (map_h - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    let down_pos = Position::new(corr_x as i32, 1);
    map.set_terrain(down_pos, Terrain::StairsDown);

    let rooms = vec![Room {
        x: ox,
        y: oy,
        width: lair_w,
        height: lair_h,
        lit: false,
    }];

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: false,
            no_prayer: true,
            is_endgame: false,
        },
    }
}

/// Generate Juiblex's Swamp.
///
/// A swampy level with pools of water and swamp terrain.
pub fn generate_juiblex(rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    for y in 0..map_h {
        for x in 0..map_w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
        }
    }

    // Large swamp area.
    let swamp_w = rng.random_range(35..=50u32) as usize;
    let swamp_h = rng.random_range(12..=16u32) as usize;
    let sx = (map_w - swamp_w) / 2;
    let sy = (map_h - swamp_h) / 2;

    // Fill with a mix of floor and water.
    for y in (sy + 1)..(sy + swamp_h) {
        for x in (sx + 1)..(sx + swamp_w) {
            let terrain = if rng.random_range(0..=3u32) == 0 {
                Terrain::Water
            } else {
                Terrain::Floor
            };
            map.set_terrain(Position::new(x as i32, y as i32), terrain);
        }
    }

    // Ensure a walkable path through the center row.
    let center_y = sy + swamp_h / 2;
    for x in (sx + 1)..(sx + swamp_w) {
        map.set_terrain(Position::new(x as i32, center_y as i32), Terrain::Floor);
    }

    // Corridors.
    let corr_x = sx + swamp_w / 2;
    for y in 1..sy {
        map.set_terrain(Position::new(corr_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(Position::new(corr_x as i32, sy as i32), Terrain::Floor);
    for y in (sy + swamp_h)..map_h.saturating_sub(1) {
        map.set_terrain(Position::new(corr_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(corr_x as i32, (sy + swamp_h) as i32),
        Terrain::Floor,
    );

    let up_pos = Position::new(corr_x as i32, (map_h - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    let down_pos = Position::new(corr_x as i32, 1);
    map.set_terrain(down_pos, Terrain::StairsDown);

    let rooms = vec![Room {
        x: sx,
        y: sy,
        width: swamp_w,
        height: swamp_h,
        lit: false,
    }];

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: false,
            no_prayer: true,
            is_endgame: false,
        },
    }
}

/// Generate Orcus Town.
///
/// A town-like layout with graves and undead.
pub fn generate_orcus(rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    for y in 0..map_h {
        for x in 0..map_w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
        }
    }

    // Town area.
    let town_w = rng.random_range(35..=50u32) as usize;
    let town_h = rng.random_range(12..=16u32) as usize;
    let tx = (map_w - town_w) / 2;
    let ty = (map_h - town_h) / 2;

    // Carve out the town as floor.
    for y in (ty + 1)..(ty + town_h) {
        for x in (tx + 1)..(tx + town_w) {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }

    // Scatter graves throughout.
    for i in 0..rng.random_range(8..=15u32) {
        let gx = tx + 2 + (i as usize * 3) % (town_w - 4);
        let gy = ty + 2 + (i as usize * 2) % (town_h - 4);
        if map.cells[gy][gx].terrain == Terrain::Floor {
            map.set_terrain(Position::new(gx as i32, gy as i32), Terrain::Grave);
        }
    }

    // Place an altar.
    let altar_x = tx + town_w / 2;
    let altar_y = ty + town_h / 2;
    map.set_terrain(
        Position::new(altar_x as i32, altar_y as i32),
        Terrain::Altar,
    );

    // Corridors.
    let corr_x = tx + town_w / 2;
    for y in 1..ty {
        map.set_terrain(Position::new(corr_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(Position::new(corr_x as i32, ty as i32), Terrain::DoorOpen);
    for y in (ty + town_h)..map_h.saturating_sub(1) {
        map.set_terrain(Position::new(corr_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(corr_x as i32, (ty + town_h) as i32),
        Terrain::DoorOpen,
    );

    let up_pos = Position::new(corr_x as i32, (map_h - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    let down_pos = Position::new(corr_x as i32, 1);
    map.set_terrain(down_pos, Terrain::StairsDown);

    let rooms = vec![Room {
        x: tx,
        y: ty,
        width: town_w,
        height: town_h,
        lit: false,
    }];

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: false,
            no_prayer: true,
            is_endgame: false,
        },
    }
}

/// Generate a Fake Wizard Tower level.
///
/// A simple maze-like level that mimics the wizard tower.
pub fn generate_fake_wizard(rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    for y in 0..map_h {
        for x in 0..map_w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
        }
    }

    // Small tower structure.
    let tower_w = rng.random_range(16..=22u32) as usize;
    let tower_h = rng.random_range(8..=12u32) as usize;
    let ox = (map_w - tower_w) / 2;
    let oy = (map_h - tower_h) / 2;

    for y in (oy + 1)..(oy + tower_h) {
        for x in (ox + 1)..(ox + tower_w) {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }

    // Internal walls to create maze-like rooms.
    let mid_x = ox + tower_w / 2;
    for y in (oy + 1)..(oy + tower_h) {
        map.set_terrain(Position::new(mid_x as i32, y as i32), Terrain::Wall);
    }
    // Doorways.
    let door_y = oy + tower_h / 3;
    map.set_terrain(
        Position::new(mid_x as i32, door_y as i32),
        Terrain::DoorClosed,
    );
    let door_y2 = oy + 2 * tower_h / 3;
    map.set_terrain(
        Position::new(mid_x as i32, door_y2 as i32),
        Terrain::DoorClosed,
    );

    // Corridors.
    let corr_x = ox + tower_w / 4;
    for y in 1..oy {
        map.set_terrain(Position::new(corr_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(Position::new(corr_x as i32, oy as i32), Terrain::DoorOpen);
    for y in (oy + tower_h)..map_h.saturating_sub(1) {
        map.set_terrain(Position::new(corr_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(corr_x as i32, (oy + tower_h) as i32),
        Terrain::DoorOpen,
    );

    let up_pos = Position::new(corr_x as i32, (map_h - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    let down_pos = Position::new(corr_x as i32, 1);
    map.set_terrain(down_pos, Terrain::StairsDown);

    let rooms = vec![Room {
        x: ox,
        y: oy,
        width: tower_w,
        height: tower_h,
        lit: false,
    }];

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags {
            no_dig: true,
            no_teleport: true,
            no_prayer: true,
            is_endgame: false,
        },
    }
}

/// Generate an upper Wizard Tower level (levels 2 and 3).
///
/// Smaller than the main tower but with similar structure.
pub fn generate_wizard_tower_upper(level: u8, rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    // Fill with stone/wall.
    for y in 0..map_h {
        for x in 0..map_w {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
        }
    }

    // Tower is smaller than the main wizard tower.
    let tower_w = rng.random_range(16..=20u32) as usize;
    let tower_h = rng.random_range(8..=11u32) as usize;
    let ox = (map_w - tower_w) / 2;
    let oy = (map_h - tower_h) / 2;

    // Carve interior.
    for y in (oy + 1)..(oy + tower_h) {
        for x in (ox + 1)..(ox + tower_w) {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }

    // Stairs: level 2 has up and down, level 3 has down only.
    let up_pos = if level == 2 {
        let pos = Position::new((ox + 2) as i32, (oy + 2) as i32);
        map.set_terrain(pos, Terrain::StairsUp);
        Some(pos)
    } else {
        None
    };

    let down_pos = Position::new((ox + tower_w - 2) as i32, (oy + tower_h - 2) as i32);
    map.set_terrain(down_pos, Terrain::StairsDown);

    let rooms = vec![Room {
        x: ox,
        y: oy,
        width: tower_w,
        height: tower_h,
        lit: true,
    }];

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: up_pos,
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags {
            no_dig: true,
            no_teleport: true,
            no_prayer: true,
            is_endgame: false,
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TOML-to-level bridge
// ═══════════════════════════════════════════════════════════════════════════

/// Build a [`SpecialLevel`] from a TOML [`LevelDefinition`].
///
/// Converts the declarative TOML format into a playable level:
/// 1. Parse ASCII map into terrain grid
/// 2. Place stairs from map characters and explicit placements
/// 3. Set special level flags from the `flags` list
pub fn build_level_from_toml(def: &LevelDefinition, _rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    let mut up_stairs = None;
    let mut down_stairs = None;
    let mut rooms: Vec<Room> = Vec::new();

    // Parse the ASCII map if present.
    if let Some(ref map_def) = def.map {
        let (_mw, _mh, grid) = parse_ascii_map(&map_def.data);
        let (offset_x, offset_y) = aligned_map_offsets(map_def, map_w, map_h);

        let mut min_fx = map_w;
        let mut max_fx = 0usize;
        let mut min_fy = map_h;
        let mut max_fy = 0usize;

        for (gy, row) in grid.iter().enumerate() {
            for (gx, &ch) in row.iter().enumerate() {
                let x = offset_x + gx as i32;
                let y = offset_y + gy as i32;
                if x < 0 || y < 0 || x >= map_w as i32 || y >= map_h as i32 {
                    continue;
                }
                let pos = Position::new(x, y);

                let terrain = match ascii_to_terrain(ch) {
                    "floor" => Terrain::Floor,
                    "corridor" => Terrain::Corridor,
                    "wall" => Terrain::Wall,
                    "door_closed" => Terrain::DoorClosed,
                    "door_secret" => Terrain::DoorClosed,
                    "fountain" => Terrain::Fountain,
                    "grave" => Terrain::Grave,
                    "pool" => Terrain::Pool,
                    "throne" => Terrain::Throne,
                    "altar" => Terrain::Altar,
                    "stairs_up" => {
                        up_stairs = Some(pos);
                        Terrain::StairsUp
                    }
                    "stairs_down" => {
                        down_stairs = Some(pos);
                        Terrain::StairsDown
                    }
                    "lava" => Terrain::Lava,
                    "drawbridge_raised" => Terrain::Drawbridge,
                    "tree" => Terrain::Tree,
                    "iron_bars" => Terrain::IronBars,
                    "water" => Terrain::Water,
                    "cloud" => Terrain::Cloud,
                    "air" => Terrain::Air,
                    _ => Terrain::Stone,
                };
                map.set_terrain(pos, terrain);

                if terrain.is_walkable() {
                    min_fx = min_fx.min(x as usize);
                    max_fx = max_fx.max(x as usize);
                    min_fy = min_fy.min(y as usize);
                    max_fy = max_fy.max(y as usize);
                }
            }
        }

        // Create a pseudo-room encompassing the walkable area.
        if max_fx >= min_fx && max_fy >= min_fy {
            rooms.push(Room {
                x: min_fx,
                y: min_fy,
                width: max_fx - min_fx + 1,
                height: max_fy - min_fy + 1,
                lit: false,
            });
        }
    }

    // Process explicit stair placements (override map-based ones).
    let (placement_offset_x, placement_offset_y) = def
        .map
        .as_ref()
        .map(|map_def| aligned_map_offsets(map_def, map_w, map_h))
        .unwrap_or((0, 0));
    let has_map = def.map.is_some();
    for stair in &def.stairs {
        if let (Some(sx), Some(sy)) = (stair.x, stair.y) {
            let local_x = if has_map { sx - 1 } else { sx };
            let local_y = if has_map { sy - 1 } else { sy };
            let pos = Position::new(local_x + placement_offset_x, local_y + placement_offset_y);
            match stair.direction.as_str() {
                "up" => {
                    map.set_terrain(pos, Terrain::StairsUp);
                    up_stairs = Some(pos);
                }
                "down" => {
                    map.set_terrain(pos, Terrain::StairsDown);
                    down_stairs = Some(pos);
                }
                _ => {}
            }
        }
    }

    // Default stairs if none placed.
    if up_stairs.is_none() {
        let pos = Position::new(1, 1);
        map.set_terrain(pos, Terrain::StairsUp);
        up_stairs = Some(pos);
    }
    if down_stairs.is_none() {
        let pos = Position::new((map_w - 2) as i32, (map_h - 2) as i32);
        map.set_terrain(pos, Terrain::StairsDown);
        down_stairs = Some(pos);
    }

    // Parse flags.
    let mut flags = SpecialLevelFlags::default();
    for flag in &def.level.flags {
        match flag.as_str() {
            "noteleport" => flags.no_teleport = true,
            "nodig" => flags.no_dig = true,
            "noprayer" | "no_prayer" => flags.no_prayer = true,
            _ => {}
        }
    }

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs,
            down_stairs,
        },
        flags,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Big Room
// ═══════════════════════════════════════════════════════════════════════════

/// Generate a Big Room level — a single large open area.
///
/// Four variants:
/// - 0: Completely open
/// - 1: Pillar grid
/// - 2: Central pool
/// - 3: Cross-shaped corridors dividing the room
pub fn generate_big_room(variant: u8, rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    // Large room covering most of the map.
    let room_x1: usize = 2;
    let room_y1: usize = 2;
    let room_x2 = map_w - 3;
    let room_y2 = map_h - 3;

    // Walls around the perimeter.
    for x in room_x1..=room_x2 {
        map.set_terrain(Position::new(x as i32, room_y1 as i32), Terrain::Wall);
        map.set_terrain(Position::new(x as i32, room_y2 as i32), Terrain::Wall);
    }
    for y in room_y1..=room_y2 {
        map.set_terrain(Position::new(room_x1 as i32, y as i32), Terrain::Wall);
        map.set_terrain(Position::new(room_x2 as i32, y as i32), Terrain::Wall);
    }

    // Fill interior with floor.
    for y in (room_y1 + 1)..room_y2 {
        for x in (room_x1 + 1)..room_x2 {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }

    match variant % 4 {
        0 => { /* Completely open — nothing extra */ }
        1 => {
            // Pillar grid: place walls every 4 cells.
            for y in ((room_y1 + 3)..room_y2).step_by(4) {
                for x in ((room_x1 + 3)..room_x2).step_by(4) {
                    map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
                }
            }
        }
        2 => {
            // Central pool.
            let cx = (room_x1 + room_x2) / 2;
            let cy = (room_y1 + room_y2) / 2;
            let pool_r = rng.random_range(3..=5u32) as i32;
            for dy in -pool_r..=pool_r {
                for dx in -pool_r..=pool_r {
                    if dx * dx + dy * dy <= pool_r * pool_r {
                        let px = cx as i32 + dx;
                        let py = cy as i32 + dy;
                        if px > room_x1 as i32
                            && px < room_x2 as i32
                            && py > room_y1 as i32
                            && py < room_y2 as i32
                        {
                            map.set_terrain(Position::new(px, py), Terrain::Pool);
                        }
                    }
                }
            }
        }
        3 => {
            // Cross-shaped wall partitions with doorways.
            let cx = (room_x1 + room_x2) / 2;
            let cy = (room_y1 + room_y2) / 2;
            // Horizontal wall.
            for x in (room_x1 + 1)..room_x2 {
                if x != cx && x != cx + 1 {
                    map.set_terrain(Position::new(x as i32, cy as i32), Terrain::Wall);
                }
            }
            // Vertical wall.
            for y in (room_y1 + 1)..room_y2 {
                if y != cy && y != cy + 1 {
                    map.set_terrain(Position::new(cx as i32, y as i32), Terrain::Wall);
                }
            }
        }
        _ => {}
    }

    // Place stairs.
    let up_pos = Position::new((room_x1 + 2) as i32, (room_y1 + 1) as i32);
    let down_pos = Position::new((room_x2 - 2) as i32, (room_y2 - 1) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    let rooms = vec![Room {
        x: room_x1 + 1,
        y: room_y1 + 1,
        width: room_x2 - room_x1 - 1,
        height: room_y2 - room_y1 - 1,
        lit: true,
    }];

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Rogue level
// ═══════════════════════════════════════════════════════════════════════════

/// Generate a Rogue-style level: a 3x3 grid of rooms connected by corridors.
///
/// Emulates the look of the original Rogue game with simpler rooms.
pub fn generate_rogue_level(_depth: i32, rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    // 3x3 grid of rooms.
    let grid_cols = 3usize;
    let grid_rows = 3usize;
    let cell_w = map_w / grid_cols;
    let cell_h = map_h / grid_rows;

    let mut room_rects: Vec<(usize, usize, usize, usize)> = Vec::new(); // (x, y, w, h)
    let mut rooms: Vec<Room> = Vec::new();

    for gr in 0..grid_rows {
        for gc in 0..grid_cols {
            let cell_x = gc * cell_w;
            let cell_y = gr * cell_h;

            // Random room size within the cell.
            let rw = rng.random_range(4..=(cell_w - 2).max(4) as u32) as usize;
            let rh = rng.random_range(3..=(cell_h - 2).max(3) as u32) as usize;
            let rx = cell_x + rng.random_range(1..(cell_w - rw).max(2) as u32) as usize;
            let ry = cell_y + rng.random_range(1..(cell_h - rh).max(2) as u32) as usize;

            // Draw walls.
            for x in rx..=(rx + rw).min(map_w - 1) {
                let top = ry;
                let bot = (ry + rh).min(map_h - 1);
                map.set_terrain(Position::new(x as i32, top as i32), Terrain::Wall);
                map.set_terrain(Position::new(x as i32, bot as i32), Terrain::Wall);
            }
            for y in ry..=(ry + rh).min(map_h - 1) {
                map.set_terrain(Position::new(rx as i32, y as i32), Terrain::Wall);
                map.set_terrain(
                    Position::new((rx + rw).min(map_w - 1) as i32, y as i32),
                    Terrain::Wall,
                );
            }

            // Fill interior with floor.
            for y in (ry + 1)..(ry + rh).min(map_h) {
                for x in (rx + 1)..(rx + rw).min(map_w) {
                    map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
                }
            }

            // Interior top-left and size (for corridor connections).
            let ix = rx + 1;
            let iy = ry + 1;
            let iw = (rw - 1).max(1);
            let ih = (rh - 1).max(1);

            rooms.push(Room {
                x: ix,
                y: iy,
                width: iw,
                height: ih,
                lit: false,
            });
            room_rects.push((ix, iy, iw, ih));
        }
    }

    // Connect adjacent rooms with corridors (horizontal then vertical).
    for gr in 0..grid_rows {
        for gc in 0..grid_cols {
            let idx = gr * grid_cols + gc;
            let (rx, ry, rw, rh) = room_rects[idx];
            let cx = rx + rw / 2;
            let cy = ry + rh / 2;

            // Connect to the room to the right.
            if gc + 1 < grid_cols {
                let ridx = gr * grid_cols + gc + 1;
                let (rrx, rry, rrw, rrh) = room_rects[ridx];
                let rcx = rrx + rrw / 2;
                let rcy = rry + rrh / 2;

                let corr_y = cy.min(rcy);
                for x in cx..=rcx {
                    let pos = Position::new(x as i32, corr_y as i32);
                    if map.cells[corr_y][x].terrain == Terrain::Stone {
                        map.set_terrain(pos, Terrain::Corridor);
                    }
                }
                // Vertical segment if needed.
                if cy != rcy {
                    let (y_min, y_max) = if cy < rcy { (cy, rcy) } else { (rcy, cy) };
                    for y in y_min..=y_max {
                        let pos = Position::new(rcx as i32, y as i32);
                        if map.cells[y][rcx].terrain == Terrain::Stone {
                            map.set_terrain(pos, Terrain::Corridor);
                        }
                    }
                }
            }

            // Connect to the room below.
            if gr + 1 < grid_rows {
                let bidx = (gr + 1) * grid_cols + gc;
                let (brx, bry, brw, brh) = room_rects[bidx];
                let bcx = brx + brw / 2;
                let bcy = bry + brh / 2;

                let corr_x = cx.min(bcx);
                for y in cy..=bcy {
                    let pos = Position::new(corr_x as i32, y as i32);
                    if map.cells[y][corr_x].terrain == Terrain::Stone {
                        map.set_terrain(pos, Terrain::Corridor);
                    }
                }
                // Horizontal segment if needed.
                if cx != bcx {
                    let (x_min, x_max) = if cx < bcx { (cx, bcx) } else { (bcx, cx) };
                    for x in x_min..=x_max {
                        let pos = Position::new(x as i32, bcy as i32);
                        if map.cells[bcy][x].terrain == Terrain::Stone {
                            map.set_terrain(pos, Terrain::Corridor);
                        }
                    }
                }
            }
        }
    }

    // Place stairs in the first and last rooms.
    let (s0x, s0y, s0w, s0h) = room_rects[0];
    let up_pos = Position::new((s0x + s0w / 2) as i32, (s0y + s0h / 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    let last = room_rects.len() - 1;
    let (slx, sly, slw, slh) = room_rects[last];
    let down_pos = Position::new((slx + slw / 2) as i32, (sly + slh / 2) as i32);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Mines End
// ═══════════════════════════════════════════════════════════════════════════

/// Generate Mines End — the bottom level of the Gnomish Mines.
///
/// Three variants:
/// - 0: Large cavern with a luckstone
/// - 1: Open area with scattered gems
/// - 2: Cavern with a central pool
pub fn generate_mines_end(variant: u8, rng: &mut impl Rng) -> SpecialLevel {
    let map_w = LevelMap::DEFAULT_WIDTH;
    let map_h = LevelMap::DEFAULT_HEIGHT;
    let mut map = LevelMap::new(map_w, map_h);

    // Large cavern covering most of the map.
    let cav_x1: usize = 3;
    let cav_y1: usize = 2;
    let cav_x2 = map_w - 4;
    let cav_y2 = map_h - 3;

    // Walls around the perimeter.
    for x in cav_x1..=cav_x2 {
        map.set_terrain(Position::new(x as i32, cav_y1 as i32), Terrain::Wall);
        map.set_terrain(Position::new(x as i32, cav_y2 as i32), Terrain::Wall);
    }
    for y in cav_y1..=cav_y2 {
        map.set_terrain(Position::new(cav_x1 as i32, y as i32), Terrain::Wall);
        map.set_terrain(Position::new(cav_x2 as i32, y as i32), Terrain::Wall);
    }

    // Fill interior with floor.
    for y in (cav_y1 + 1)..cav_y2 {
        for x in (cav_x1 + 1)..cav_x2 {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }

    // Irregular edges: randomly replace some floor cells near walls with stone.
    for y in (cav_y1 + 1)..cav_y2 {
        for x in (cav_x1 + 1)..cav_x2 {
            let dist_x = (x - cav_x1).min(cav_x2 - x);
            let dist_y = (y - cav_y1).min(cav_y2 - y);
            let dist = dist_x.min(dist_y);
            if dist <= 2 && rng.random_bool(0.3) {
                map.set_terrain(Position::new(x as i32, y as i32), Terrain::Stone);
            }
        }
    }

    match variant % 3 {
        0 => {
            // Large open cavern — nothing extra beyond irregular edges.
        }
        1 => {
            // Scattered fountain clusters (representing gem-rich areas).
            for _ in 0..rng.random_range(3..=6u32) {
                let fx = rng.random_range((cav_x1 + 3) as u32..(cav_x2 - 3) as u32) as i32;
                let fy = rng.random_range((cav_y1 + 2) as u32..(cav_y2 - 2) as u32) as i32;
                let pos = Position::new(fx, fy);
                if map.cells[fy as usize][fx as usize].terrain == Terrain::Floor {
                    map.set_terrain(pos, Terrain::Fountain);
                }
            }
        }
        2 => {
            // Central underground pool.
            let cx = ((cav_x1 + cav_x2) / 2) as i32;
            let cy = ((cav_y1 + cav_y2) / 2) as i32;
            for dy in -3i32..=3 {
                for dx in -5i32..=5 {
                    if dx * dx / 4 + dy * dy <= 9 {
                        let px = cx + dx;
                        let py = cy + dy;
                        if px > cav_x1 as i32
                            && px < cav_x2 as i32
                            && py > cav_y1 as i32
                            && py < cav_y2 as i32
                        {
                            map.set_terrain(Position::new(px, py), Terrain::Pool);
                        }
                    }
                }
            }
        }
        _ => {}
    }

    // Stairs: up only (this is the bottom of the mines).
    let up_pos = Position::new(((cav_x1 + cav_x2) / 2) as i32, (cav_y1 + 1) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    let rooms = vec![Room {
        x: cav_x1 + 1,
        y: cav_y1 + 1,
        width: cav_x2 - cav_x1 - 1,
        height: cav_y2 - cav_y1 - 1,
        lit: true,
    }];

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms,
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags::default(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Quest levels
// ═══════════════════════════════════════════════════════════════════════════

// -- Helpers for quest level construction --

/// Draw a rectangular room (walls + floor interior) on the map.
/// Returns the Room struct (interior only, excluding walls).
fn draw_quest_room(map: &mut LevelMap, x: usize, y: usize, w: usize, h: usize, lit: bool) -> Room {
    let map_w = map.width;
    let map_h = map.height;
    // Walls
    for cx in x..=(x + w).min(map_w - 1) {
        map.set_terrain(Position::new(cx as i32, y as i32), Terrain::Wall);
        map.set_terrain(
            Position::new(cx as i32, (y + h).min(map_h - 1) as i32),
            Terrain::Wall,
        );
    }
    for cy in y..=(y + h).min(map_h - 1) {
        map.set_terrain(Position::new(x as i32, cy as i32), Terrain::Wall);
        map.set_terrain(
            Position::new((x + w).min(map_w - 1) as i32, cy as i32),
            Terrain::Wall,
        );
    }
    // Floor
    for cy in (y + 1)..(y + h).min(map_h) {
        for cx in (x + 1)..(x + w).min(map_w) {
            map.set_terrain(Position::new(cx as i32, cy as i32), Terrain::Floor);
        }
    }
    Room {
        x: x + 1,
        y: y + 1,
        width: w.saturating_sub(1).max(1),
        height: h.saturating_sub(1).max(1),
        lit,
    }
}

/// Fill rectangular area with a single terrain type.
fn fill_terrain(map: &mut LevelMap, x: usize, y: usize, w: usize, h: usize, terrain: Terrain) {
    let map_w = map.width;
    let map_h = map.height;
    for cy in y..(y + h).min(map_h) {
        for cx in x..(x + w).min(map_w) {
            map.set_terrain(Position::new(cx as i32, cy as i32), terrain);
        }
    }
}

/// Fill the entire map interior (rows 1..h-1, cols 1..w-1) with a terrain.
fn fill_map_interior(map: &mut LevelMap, terrain: Terrain) {
    let w = map.width;
    let h = map.height;
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            map.set_terrain(Position::new(x as i32, y as i32), terrain);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Quest Start — role dispatch
// ═══════════════════════════════════════════════════════════════════════════

/// Generate the quest home level (quest start), themed per role.
pub fn generate_quest_start(role: &str, rng: &mut impl Rng) -> SpecialLevel {
    match role {
        "valkyrie" => generate_valkyrie_start(rng),
        "wizard" => generate_wizard_quest_start(rng),
        "archeologist" => generate_archeologist_start(rng),
        "barbarian" => generate_barbarian_start(rng),
        "caveman" | "caveperson" => generate_caveman_start(rng),
        "healer" => generate_healer_start(rng),
        "knight" => generate_knight_start(rng),
        "monk" => generate_monk_start(rng),
        "priest" => generate_priest_start(rng),
        "ranger" => generate_ranger_start(rng),
        "rogue" => generate_rogue_quest_start(rng),
        "samurai" => generate_samurai_start(rng),
        "tourist" => generate_tourist_start(rng),
        _ => generate_generic_quest_start(rng),
    }
}

// -- Valkyrie: Norse hall with frozen river --
fn generate_valkyrie_start(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Floor);

    // Frozen river running east-west across the south part.
    let river_y = 15;
    for x in 1..map.width - 1 {
        let dy = if rng.random_bool(0.3) { 1 } else { 0 };
        let y = (river_y as i32 + dy).min(map.height as i32 - 2);
        map.set_terrain(Position::new(x as i32, y), Terrain::Ice);
    }

    // Great hall (central building).
    let bx = 20usize;
    let by = 3usize;
    let bw = 40usize;
    let bh = 10usize;
    let room = draw_quest_room(&mut map, bx, by, bw, bh, true);

    // War room partition.
    let part_x = bx + bw / 2;
    for y in by..(by + bh) {
        map.set_terrain(Position::new(part_x as i32, y as i32), Terrain::Wall);
    }
    map.set_terrain(
        Position::new(part_x as i32, (by + bh / 2) as i32),
        Terrain::DoorClosed,
    );

    // Fountain in the east chamber.
    map.set_terrain(
        Position::new((part_x + 5) as i32, (by + bh / 2) as i32),
        Terrain::Fountain,
    );

    // South entrance.
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh) as i32),
        Terrain::DoorClosed,
    );

    // Stairs.
    let up_pos = Position::new((bx + 5) as i32, (map.height - 2) as i32);
    let down_pos = Position::new((bx + bw - 5) as i32, (by + 1) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Wizard: Magic tower with library and summoning chambers --
fn generate_wizard_quest_start(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    // Dark surroundings.
    fill_map_interior(&mut map, Terrain::Floor);

    // Central tower.
    let tx = 25usize;
    let ty = 2usize;
    let tw = 30usize;
    let th = 16usize;
    let room = draw_quest_room(&mut map, tx, ty, tw, th, true);

    // Library alcove (inner room, east side).
    let lib_x = tx + tw - 10;
    let lib_y = ty + 2;
    draw_quest_room(&mut map, lib_x, lib_y, 8, 5, true);
    map.set_terrain(
        Position::new(lib_x as i32, (lib_y + 2) as i32),
        Terrain::DoorClosed,
    );

    // Summoning chamber (inner room, west side).
    let sum_x = tx + 2;
    let sum_y = ty + 2;
    draw_quest_room(&mut map, sum_x, sum_y, 8, 5, true);
    map.set_terrain(
        Position::new((sum_x + 8) as i32, (sum_y + 2) as i32),
        Terrain::DoorClosed,
    );
    // Altar in summoning chamber.
    map.set_terrain(
        Position::new((sum_x + 4) as i32, (sum_y + 3) as i32),
        Terrain::Altar,
    );

    // Fountain in main tower.
    map.set_terrain(
        Position::new((tx + tw / 2) as i32, (ty + th / 2) as i32),
        Terrain::Fountain,
    );

    // South entrance.
    map.set_terrain(
        Position::new((tx + tw / 2) as i32, (ty + th) as i32),
        Terrain::DoorClosed,
    );

    // Stairs.
    let up_pos = Position::new((tx + tw / 2) as i32, (map.height - 2) as i32);
    let down_pos = Position::new((tx + tw / 2) as i32, (ty + 1) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Archeologist: Desert camp with dig sites --
fn generate_archeologist_start(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Floor);

    // Camp building (central).
    let bx = 25usize;
    let by = 4usize;
    let bw = 30usize;
    let bh = 8usize;
    let room = draw_quest_room(&mut map, bx, by, bw, bh, true);
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh) as i32),
        Terrain::DoorClosed,
    );

    // Fountain inside.
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh / 2) as i32),
        Terrain::Fountain,
    );

    // Dig sites: scattered graves representing excavation pits.
    for i in 0..rng.random_range(5..=8u32) {
        let gx = 5 + (i as usize * 7) % 65;
        let gy = 14 + (i as usize * 3) % 5;
        if map.cells[gy][gx].terrain == Terrain::Floor {
            map.set_terrain(Position::new(gx as i32, gy as i32), Terrain::Grave);
        }
    }

    let up_pos = Position::new(10, (map.height - 2) as i32);
    let down_pos = Position::new(70, 1);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Barbarian: Wilderness camp near mountains --
fn generate_barbarian_start(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Floor);

    // Mountain wall along the north edge.
    for x in 1..map.width - 1 {
        let h = rng.random_range(1..=3u32) as usize;
        for y in 1..=h {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
        }
    }

    // Camp tent (central).
    let bx = 28usize;
    let by = 6usize;
    let bw = 24usize;
    let bh = 8usize;
    let room = draw_quest_room(&mut map, bx, by, bw, bh, true);
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh) as i32),
        Terrain::DoorClosed,
    );

    // Fountain (water source).
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh / 2) as i32),
        Terrain::Fountain,
    );

    // Scattered trees around camp.
    for _ in 0..rng.random_range(6..=12u32) {
        let tx = rng.random_range(2..map.width - 2) as i32;
        let ty = rng.random_range(5..map.height - 2) as i32;
        if map.cells[ty as usize][tx as usize].terrain == Terrain::Floor {
            map.set_terrain(Position::new(tx, ty), Terrain::Tree);
        }
    }

    let up_pos = Position::new(5, (map.height - 2) as i32);
    let down_pos = Position::new(75, (by + 1) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Caveman: Cave system with paintings --
fn generate_caveman_start(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    // Start with stone, carve out caves.
    // Large main cavern.
    let cx = 15usize;
    let cy = 3usize;
    let cw = 50usize;
    let ch = 14usize;

    // Irregular cave: fill interior and nibble edges.
    for y in cy..(cy + ch) {
        for x in cx..(cx + cw) {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
        }
    }
    // Nibble corners for cave-like shape.
    for y in cy..(cy + ch) {
        for x in cx..(cx + cw) {
            let dx = (x - cx).min(cx + cw - 1 - x);
            let dy = (y - cy).min(cy + ch - 1 - y);
            if (dx <= 1 || dy <= 1) && rng.random_bool(0.35) {
                map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
            }
        }
    }

    // Cave wall borders.
    for x in (cx.saturating_sub(1))..=(cx + cw).min(map.width - 1) {
        let yt = cy.saturating_sub(1);
        let yb = (cy + ch).min(map.height - 1);
        if map.cells[yt][x].terrain == Terrain::Stone {
            map.set_terrain(Position::new(x as i32, yt as i32), Terrain::Wall);
        }
        if map.cells[yb][x].terrain == Terrain::Stone {
            map.set_terrain(Position::new(x as i32, yb as i32), Terrain::Wall);
        }
    }

    // Fountain (pool).
    map.set_terrain(
        Position::new((cx + cw / 2) as i32, (cy + ch / 2) as i32),
        Terrain::Fountain,
    );

    let room = Room {
        x: cx + 2,
        y: cy + 2,
        width: cw - 4,
        height: ch - 4,
        lit: false,
    };

    let up_pos = Position::new((cx + 3) as i32, (cy + ch - 2) as i32);
    let down_pos = Position::new((cx + cw - 3) as i32, (cy + 1) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Healer: Temple of Epidaurus with gardens --
fn generate_healer_start(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Floor);

    // Temple building.
    let bx = 22usize;
    let by = 3usize;
    let bw = 36usize;
    let bh = 10usize;
    let room = draw_quest_room(&mut map, bx, by, bw, bh, true);
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh) as i32),
        Terrain::DoorClosed,
    );

    // Altar inside.
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + 2) as i32),
        Terrain::Altar,
    );

    // Fountains (healing pools) — two flanking the altar.
    map.set_terrain(
        Position::new((bx + bw / 2 - 4) as i32, (by + 2) as i32),
        Terrain::Fountain,
    );
    map.set_terrain(
        Position::new((bx + bw / 2 + 4) as i32, (by + 2) as i32),
        Terrain::Fountain,
    );

    // Gardens: trees around the temple exterior.
    for _ in 0..rng.random_range(10..=18u32) {
        let tx = rng.random_range(2..map.width - 2) as i32;
        let ty = rng.random_range(14..map.height - 2) as i32;
        if map.cells[ty as usize][tx as usize].terrain == Terrain::Floor {
            map.set_terrain(Position::new(tx, ty), Terrain::Tree);
        }
    }

    let up_pos = Position::new(5, (map.height - 2) as i32);
    let down_pos = Position::new(75, 1);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Knight: Castle with moat and drawbridge --
fn generate_knight_start(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Floor);

    // Castle.
    let cx = 20usize;
    let cy = 3usize;
    let cw = 40usize;
    let ch = 12usize;

    // Moat ring.
    for x in (cx.saturating_sub(1))..=(cx + cw + 1).min(map.width - 1) {
        for y in (cy.saturating_sub(1))..=(cy + ch + 1).min(map.height - 1) {
            let on_edge = x == cx.saturating_sub(1)
                || x == (cx + cw + 1).min(map.width - 1)
                || y == cy.saturating_sub(1)
                || y == (cy + ch + 1).min(map.height - 1);
            if on_edge {
                map.set_terrain(Position::new(x as i32, y as i32), Terrain::Moat);
            }
        }
    }

    let room = draw_quest_room(&mut map, cx, cy, cw, ch, true);

    // Internal partition.
    let part_y = cy + ch / 2;
    for x in cx..=(cx + cw) {
        map.set_terrain(Position::new(x as i32, part_y as i32), Terrain::Wall);
    }
    // Doors through partition.
    map.set_terrain(
        Position::new((cx + cw / 3) as i32, part_y as i32),
        Terrain::DoorClosed,
    );
    map.set_terrain(
        Position::new((cx + 2 * cw / 3) as i32, part_y as i32),
        Terrain::DoorClosed,
    );

    // Drawbridge on south wall.
    let db_x = cx + cw / 2;
    map.set_terrain(
        Position::new(db_x as i32, (cy + ch) as i32),
        Terrain::Drawbridge,
    );
    map.set_terrain(
        Position::new(db_x as i32, (cy + ch + 1).min(map.height - 1) as i32),
        Terrain::Drawbridge,
    );

    // Throne room.
    map.set_terrain(
        Position::new((cx + cw / 2) as i32, (cy + 2) as i32),
        Terrain::Throne,
    );

    // Fountain in courtyard.
    map.set_terrain(
        Position::new((cx + cw / 2) as i32, (part_y + 2) as i32),
        Terrain::Fountain,
    );

    let up_pos = Position::new(5, (map.height - 2) as i32);
    let down_pos = Position::new((cx + 2) as i32, (cy + 1) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Monk: Monastery with meditation gardens --
fn generate_monk_start(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Floor);

    // Monastery building.
    let bx = 22usize;
    let by = 2usize;
    let bw = 36usize;
    let bh = 12usize;
    let room = draw_quest_room(&mut map, bx, by, bw, bh, true);
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh) as i32),
        Terrain::DoorClosed,
    );

    // Meditation garden: inner courtyard.
    let gy = by + 3;
    let gx = bx + bw / 2 - 5;
    for y in gy..(gy + 5) {
        for x in gx..(gx + 10) {
            if rng.random_bool(0.3) {
                map.set_terrain(Position::new(x as i32, y as i32), Terrain::Tree);
            }
        }
    }

    // Fountain at garden center.
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (gy + 2) as i32),
        Terrain::Fountain,
    );

    // Altar.
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + 1) as i32),
        Terrain::Altar,
    );

    let up_pos = Position::new(5, (map.height - 2) as i32);
    let down_pos = Position::new(75, 1);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Priest: Cathedral with multiple altars --
fn generate_priest_start(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Floor);

    // Cathedral.
    let bx = 18usize;
    let by = 2usize;
    let bw = 44usize;
    let bh = 14usize;
    let room = draw_quest_room(&mut map, bx, by, bw, bh, true);

    // Grand entrance (double door).
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh) as i32),
        Terrain::DoorClosed,
    );
    map.set_terrain(
        Position::new((bx + bw / 2 + 1) as i32, (by + bh) as i32),
        Terrain::DoorClosed,
    );

    // Three altars in a row (lawful, neutral, chaotic).
    let altar_y = by + 2;
    map.set_terrain(
        Position::new((bx + bw / 4) as i32, altar_y as i32),
        Terrain::Altar,
    );
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, altar_y as i32),
        Terrain::Altar,
    );
    map.set_terrain(
        Position::new((bx + 3 * bw / 4) as i32, altar_y as i32),
        Terrain::Altar,
    );

    // Fountain.
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh / 2) as i32),
        Terrain::Fountain,
    );

    let up_pos = Position::new(5, (map.height - 2) as i32);
    let down_pos = Position::new(75, 1);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Ranger: Forest clearing with ranger lodge --
fn generate_ranger_start(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Tree);

    // Large forest clearing in the center.
    let clr_x = 15usize;
    let clr_y = 4usize;
    let clr_w = 50usize;
    let clr_h = 12usize;
    fill_terrain(&mut map, clr_x, clr_y, clr_w, clr_h, Terrain::Floor);

    // Ranger lodge inside the clearing.
    let lx = 30usize;
    let ly = 6usize;
    let lw = 20usize;
    let lh = 6usize;
    let room = draw_quest_room(&mut map, lx, ly, lw, lh, true);
    map.set_terrain(
        Position::new((lx + lw / 2) as i32, (ly + lh) as i32),
        Terrain::DoorClosed,
    );

    // Fountain.
    map.set_terrain(
        Position::new((lx + lw / 2) as i32, (ly + lh / 2) as i32),
        Terrain::Fountain,
    );

    // Paths through the forest (corridors).
    for y in 1..clr_y {
        map.set_terrain(
            Position::new((lx + lw / 2) as i32, y as i32),
            Terrain::Floor,
        );
    }
    for y in (clr_y + clr_h)..map.height - 1 {
        map.set_terrain(
            Position::new((lx + lw / 2) as i32, y as i32),
            Terrain::Floor,
        );
    }

    let up_pos = Position::new((lx + lw / 2) as i32, (map.height - 2) as i32);
    let down_pos = Position::new((lx + lw / 2) as i32, 1);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Rogue: Thieves' guild in underground tunnels --
fn generate_rogue_quest_start(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    // Guild hall (central).
    let bx = 25usize;
    let by = 4usize;
    let bw = 30usize;
    let bh = 10usize;
    let room = draw_quest_room(&mut map, bx, by, bw, bh, false);

    // Secret corridors leading to the guild.
    let mid_y = by + bh / 2;
    // West corridor.
    for x in 1..bx {
        map.set_terrain(Position::new(x as i32, mid_y as i32), Terrain::Corridor);
    }
    map.set_terrain(Position::new(bx as i32, mid_y as i32), Terrain::DoorClosed);
    // East corridor.
    for x in (bx + bw + 1)..map.width - 1 {
        map.set_terrain(Position::new(x as i32, mid_y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new((bx + bw) as i32, mid_y as i32),
        Terrain::DoorClosed,
    );

    // North corridor.
    let mid_x = bx + bw / 2;
    for y in 1..by {
        map.set_terrain(Position::new(mid_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(Position::new(mid_x as i32, by as i32), Terrain::DoorClosed);
    // South corridor.
    for y in (by + bh + 1)..map.height - 1 {
        map.set_terrain(Position::new(mid_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(mid_x as i32, (by + bh) as i32),
        Terrain::DoorClosed,
    );

    // Fountain inside.
    map.set_terrain(
        Position::new(mid_x as i32, (by + bh / 2) as i32),
        Terrain::Fountain,
    );

    let up_pos = Position::new(1, mid_y as i32);
    let down_pos = Position::new((map.width - 2) as i32, mid_y as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Samurai: Japanese castle with gardens --
fn generate_samurai_start(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Floor);

    // Castle.
    let bx = 20usize;
    let by = 2usize;
    let bw = 40usize;
    let bh = 12usize;
    let room = draw_quest_room(&mut map, bx, by, bw, bh, true);

    // Inner walls for rooms.
    let part_x = bx + bw / 2;
    for y in by..(by + bh) {
        map.set_terrain(Position::new(part_x as i32, y as i32), Terrain::Wall);
    }
    map.set_terrain(
        Position::new(part_x as i32, (by + bh / 2) as i32),
        Terrain::DoorClosed,
    );

    // South entrance.
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh) as i32),
        Terrain::DoorClosed,
    );

    // Zen garden: pool + trees in courtyard area.
    let gy = by + bh + 2;
    for _ in 0..rng.random_range(6..=10u32) {
        let tx = rng.random_range((bx + 2)..(bx + bw - 2)) as i32;
        let ty = rng.random_range(gy..(gy + 4).min(map.height - 2)) as i32;
        if map.cells[ty as usize][tx as usize].terrain == Terrain::Floor {
            map.set_terrain(Position::new(tx, ty), Terrain::Tree);
        }
    }
    // Pool.
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (gy + 1) as i32),
        Terrain::Pool,
    );

    // Fountain inside castle.
    map.set_terrain(
        Position::new((bx + bw / 4) as i32, (by + bh / 2) as i32),
        Terrain::Fountain,
    );

    let up_pos = Position::new(5, (map.height - 2) as i32);
    let down_pos = Position::new((bx + 2) as i32, (by + 1) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Tourist: City plaza with shops --
fn generate_tourist_start(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Floor);

    // Central plaza building (airline office).
    let bx = 25usize;
    let by = 4usize;
    let bw = 30usize;
    let bh = 8usize;
    let room = draw_quest_room(&mut map, bx, by, bw, bh, true);
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh) as i32),
        Terrain::DoorClosed,
    );

    // Shop stalls (small rooms flanking the plaza).
    let s1 = draw_quest_room(&mut map, 5, 4, 12, 6, true);
    map.set_terrain(Position::new(17, 7), Terrain::DoorClosed);

    let s2 = draw_quest_room(&mut map, 63, 4, 12, 6, true);
    map.set_terrain(Position::new(63, 7), Terrain::DoorClosed);

    // Fountain in the plaza.
    map.set_terrain(Position::new((bx + bw / 2) as i32, 14), Terrain::Fountain);

    let up_pos = Position::new(5, (map.height - 2) as i32);
    let down_pos = Position::new(75, 1);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room, s1, s2],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// -- Generic fallback --
fn generate_generic_quest_start(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Floor);

    let bw = rng.random_range(20..=30u32) as usize;
    let bh = rng.random_range(8..=12u32) as usize;
    let bx = (map.width - bw) / 2;
    let by = (map.height - bh) / 2;
    let room = draw_quest_room(&mut map, bx, by, bw, bh, true);
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh) as i32),
        Terrain::DoorClosed,
    );
    map.set_terrain(
        Position::new((bx + bw / 2) as i32, (by + bh / 2) as i32),
        Terrain::Fountain,
    );

    let up_pos = Position::new((bx + bw / 2) as i32, (map.height - 2) as i32);
    let down_pos = Position::new((bx + bw / 2) as i32, (by + 1) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Quest Locator — role dispatch
// ═══════════════════════════════════════════════════════════════════════════

/// Generate a quest locator level (middle quest level), themed per role.
pub fn generate_quest_locator(role: &str, rng: &mut impl Rng) -> SpecialLevel {
    // Locator levels vary by terrain theme but share a two-room structure.
    let mut map = LevelMap::new_standard();

    // Background terrain per role.
    let bg = match role {
        "ranger" => Terrain::Tree,
        "valkyrie" => Terrain::Ice,
        "caveman" | "caveperson" => Terrain::Stone,
        _ => Terrain::Stone,
    };
    // Fill for outdoor roles.
    if matches!(bg, Terrain::Tree | Terrain::Ice) {
        fill_map_interior(&mut map, bg);
    }

    // Two rooms connected by a corridor.
    let r1_x = 3usize;
    let r1_y = 3usize;
    let r1_w = rng.random_range(15..=20u32) as usize;
    let r1_h = rng.random_range(8..=12u32) as usize;

    let r2_x = map.width - 3 - rng.random_range(15..=20u32) as usize;
    let r2_y = 3usize;
    let r2_w = rng.random_range(15..=20u32) as usize;
    let r2_h = rng.random_range(8..=12u32) as usize;

    let room1 = draw_quest_room(&mut map, r1_x, r1_y, r1_w, r1_h, true);
    let room2 = draw_quest_room(&mut map, r2_x, r2_y, r2_w, r2_h, true);

    // Corridor connecting the rooms.
    let corr_y = (r1_y + r1_h / 2).min(map.height - 1);
    let corr_start = (r1_x + r1_w).min(map.width - 1);
    let corr_end = r2_x;
    for x in corr_start..=corr_end {
        let pos = Position::new(x as i32, corr_y as i32);
        let t = map.cells[corr_y][x].terrain;
        if matches!(
            t,
            Terrain::Stone | Terrain::Wall | Terrain::Tree | Terrain::Ice
        ) {
            map.set_terrain(pos, Terrain::Corridor);
        }
    }
    map.set_terrain(
        Position::new(corr_start as i32, corr_y as i32),
        Terrain::DoorClosed,
    );
    map.set_terrain(
        Position::new(corr_end as i32, corr_y as i32),
        Terrain::DoorClosed,
    );

    // Role-specific feature in room 1.
    match role {
        "valkyrie" => {
            // Ice pool.
            map.set_terrain(
                Position::new((r1_x + r1_w / 2) as i32, (r1_y + r1_h / 2) as i32),
                Terrain::Ice,
            );
        }
        "wizard" => {
            map.set_terrain(
                Position::new((r1_x + r1_w / 2) as i32, (r1_y + r1_h / 2) as i32),
                Terrain::Altar,
            );
        }
        "priest" => {
            map.set_terrain(
                Position::new((r1_x + r1_w / 2) as i32, (r1_y + r1_h / 2) as i32),
                Terrain::Altar,
            );
        }
        "knight" => {
            map.set_terrain(
                Position::new((r1_x + r1_w / 2) as i32, (r1_y + r1_h / 2) as i32),
                Terrain::Fountain,
            );
        }
        _ => {
            map.set_terrain(
                Position::new((r1_x + r1_w / 2) as i32, (r1_y + r1_h / 2) as i32),
                Terrain::Fountain,
            );
        }
    }

    // Stairs.
    let up_pos = Position::new((r1_x + 2) as i32, (r1_y + 1) as i32);
    let down_pos = Position::new((r2_x + 2) as i32, (r2_y + 1) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room1, room2],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Quest Goal — role dispatch
// ═══════════════════════════════════════════════════════════════════════════

/// Generate the quest goal level — the nemesis lair, themed per role.
pub fn generate_quest_goal(role: &str, rng: &mut impl Rng) -> SpecialLevel {
    match role {
        "valkyrie" => generate_valkyrie_goal(rng),
        "wizard" => generate_wizard_quest_goal(rng),
        "archeologist" => generate_archeologist_goal(rng),
        "barbarian" => generate_barbarian_goal(rng),
        "caveman" | "caveperson" => generate_caveman_goal(rng),
        "healer" => generate_healer_goal(rng),
        "knight" => generate_knight_goal(rng),
        "monk" => generate_monk_goal(rng),
        "priest" => generate_priest_goal(rng),
        "ranger" => generate_ranger_goal(rng),
        "rogue" => generate_rogue_quest_goal(rng),
        "samurai" => generate_samurai_goal(rng),
        "tourist" => generate_tourist_goal(rng),
        _ => generate_generic_quest_goal(rng),
    }
}

// -- Valkyrie goal: Icy cavern with lava (Lord Surtur's lair) --
fn generate_valkyrie_goal(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Ice);

    // Central lava chamber.
    let rx = 20usize;
    let ry = 3usize;
    let rw = 40usize;
    let rh = 14usize;
    let room = draw_quest_room(&mut map, rx, ry, rw, rh, true);

    // Lava pools inside.
    for _ in 0..rng.random_range(6..=12u32) {
        let lx = rng.random_range((rx + 2)..(rx + rw - 2)) as i32;
        let ly = rng.random_range((ry + 2)..(ry + rh - 2)) as i32;
        map.set_terrain(Position::new(lx, ly), Terrain::Lava);
    }

    // Nemesis position — altar at center.
    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    // Corridor from south.
    let entry_x = rx + rw / 2;
    for y in (ry + rh)..(map.height - 1) {
        map.set_terrain(Position::new(entry_x as i32, y as i32), Terrain::Floor);
    }
    map.set_terrain(
        Position::new(entry_x as i32, (ry + rh) as i32),
        Terrain::DoorClosed,
    );

    let up_pos = Position::new(entry_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Wizard goal: Dark tower top (Dark One's lair) --
fn generate_wizard_quest_goal(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    let rx = 25usize;
    let ry = 3usize;
    let rw = 30usize;
    let rh = 14usize;
    let room = draw_quest_room(&mut map, rx, ry, rw, rh, false);

    // Altar at center.
    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    // Pools of water around the edges (moats of darkness).
    for x in (rx + 2)..(rx + rw - 2) {
        map.set_terrain(Position::new(x as i32, (ry + 2) as i32), Terrain::Pool);
        map.set_terrain(Position::new(x as i32, (ry + rh - 2) as i32), Terrain::Pool);
    }

    let entry_x = rx + rw / 2;
    for y in (ry + rh)..(map.height - 1) {
        map.set_terrain(Position::new(entry_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(entry_x as i32, (ry + rh) as i32),
        Terrain::DoorClosed,
    );

    let up_pos = Position::new(entry_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Archeologist goal: Deep burial chamber --
fn generate_archeologist_goal(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    let rx = 20usize;
    let ry = 3usize;
    let rw = 40usize;
    let rh = 14usize;
    let room = draw_quest_room(&mut map, rx, ry, rw, rh, false);

    // Graves lining the chamber walls.
    for i in 0..8u32 {
        let gx = rx + 2 + (i as usize * 4) % (rw - 4);
        map.set_terrain(Position::new(gx as i32, (ry + 1) as i32), Terrain::Grave);
        map.set_terrain(
            Position::new(gx as i32, (ry + rh - 1) as i32),
            Terrain::Grave,
        );
    }

    // Altar at center (nemesis/artifact location).
    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    let entry_x = rx + rw / 2;
    for y in (ry + rh)..(map.height - 1) {
        map.set_terrain(Position::new(entry_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(entry_x as i32, (ry + rh) as i32),
        Terrain::DoorClosed,
    );

    let up_pos = Position::new(entry_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Barbarian goal: Dragon's lair (Thoth Amon) --
fn generate_barbarian_goal(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    let rx = 20usize;
    let ry = 3usize;
    let rw = 40usize;
    let rh = 14usize;
    let room = draw_quest_room(&mut map, rx, ry, rw, rh, false);

    // Lava pools.
    for _ in 0..rng.random_range(4..=8u32) {
        let lx = rng.random_range((rx + 2)..(rx + rw - 2)) as i32;
        let ly = rng.random_range((ry + 2)..(ry + rh - 2)) as i32;
        map.set_terrain(Position::new(lx, ly), Terrain::Lava);
    }

    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    let entry_x = rx + rw / 2;
    for y in (ry + rh)..(map.height - 1) {
        map.set_terrain(Position::new(entry_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(entry_x as i32, (ry + rh) as i32),
        Terrain::DoorClosed,
    );

    let up_pos = Position::new(entry_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Caveman goal: Underground lake (Chromatic Dragon) --
fn generate_caveman_goal(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    let rx = 15usize;
    let ry = 3usize;
    let rw = 50usize;
    let rh = 14usize;
    let room = draw_quest_room(&mut map, rx, ry, rw, rh, false);

    // Underground lake — pool in one half.
    for y in (ry + 2)..(ry + rh / 2) {
        for x in (rx + 2)..(rx + rw / 2) {
            map.set_terrain(Position::new(x as i32, y as i32), Terrain::Pool);
        }
    }

    map.set_terrain(
        Position::new((rx + 3 * rw / 4) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    let entry_x = rx + rw / 2;
    for y in (ry + rh)..(map.height - 1) {
        map.set_terrain(Position::new(entry_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(entry_x as i32, (ry + rh) as i32),
        Terrain::DoorClosed,
    );

    let up_pos = Position::new(entry_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Healer goal: Dark lab (Cyclops) --
fn generate_healer_goal(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    let rx = 22usize;
    let ry = 3usize;
    let rw = 36usize;
    let rh = 14usize;
    let room = draw_quest_room(&mut map, rx, ry, rw, rh, false);

    // Pools (poisoned water).
    for _ in 0..rng.random_range(3..=6u32) {
        let px = rng.random_range((rx + 2)..(rx + rw - 2)) as i32;
        let py = rng.random_range((ry + 2)..(ry + rh - 2)) as i32;
        map.set_terrain(Position::new(px, py), Terrain::Pool);
    }

    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    let entry_x = rx + rw / 2;
    for y in (ry + rh)..(map.height - 1) {
        map.set_terrain(Position::new(entry_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(entry_x as i32, (ry + rh) as i32),
        Terrain::DoorClosed,
    );

    let up_pos = Position::new(entry_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Knight goal: Ruined castle (Ixoth) --
fn generate_knight_goal(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    let rx = 20usize;
    let ry = 3usize;
    let rw = 40usize;
    let rh = 14usize;

    // Moat around the ruin.
    for x in (rx.saturating_sub(1))..=(rx + rw + 1).min(map.width - 1) {
        for y in (ry.saturating_sub(1))..=(ry + rh + 1).min(map.height - 1) {
            let on_edge = x == rx.saturating_sub(1)
                || x == (rx + rw + 1).min(map.width - 1)
                || y == ry.saturating_sub(1)
                || y == (ry + rh + 1).min(map.height - 1);
            if on_edge {
                map.set_terrain(Position::new(x as i32, y as i32), Terrain::Moat);
            }
        }
    }

    let room = draw_quest_room(&mut map, rx, ry, rw, rh, false);

    // Drawbridge entrance.
    let db_x = rx + rw / 2;
    map.set_terrain(
        Position::new(db_x as i32, (ry + rh) as i32),
        Terrain::Drawbridge,
    );
    map.set_terrain(
        Position::new(db_x as i32, (ry + rh + 1).min(map.height - 1) as i32),
        Terrain::Drawbridge,
    );

    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    // Corridor south of moat.
    for y in ((ry + rh + 2).min(map.height - 1))..(map.height - 1) {
        map.set_terrain(Position::new(db_x as i32, y as i32), Terrain::Corridor);
    }

    let up_pos = Position::new(db_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Monk goal: Corrupted temple (Master Kaen) --
fn generate_monk_goal(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    let rx = 22usize;
    let ry = 3usize;
    let rw = 36usize;
    let rh = 14usize;
    let room = draw_quest_room(&mut map, rx, ry, rw, rh, false);

    // Corrupted altars.
    map.set_terrain(
        Position::new((rx + rw / 3) as i32, (ry + 2) as i32),
        Terrain::Altar,
    );
    map.set_terrain(
        Position::new((rx + 2 * rw / 3) as i32, (ry + 2) as i32),
        Terrain::Altar,
    );

    // Central altar (nemesis position).
    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    let entry_x = rx + rw / 2;
    for y in (ry + rh)..(map.height - 1) {
        map.set_terrain(Position::new(entry_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(entry_x as i32, (ry + rh) as i32),
        Terrain::DoorClosed,
    );

    let up_pos = Position::new(entry_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Priest goal: Desecrated altar (Nalzok) --
fn generate_priest_goal(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    let rx = 22usize;
    let ry = 3usize;
    let rw = 36usize;
    let rh = 14usize;
    let room = draw_quest_room(&mut map, rx, ry, rw, rh, false);

    // Three desecrated altars.
    map.set_terrain(
        Position::new((rx + rw / 4) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );
    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );
    map.set_terrain(
        Position::new((rx + 3 * rw / 4) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    // Graves.
    for i in 0..6u32 {
        let gx = rx + 2 + (i as usize * 5) % (rw - 4);
        map.set_terrain(
            Position::new(gx as i32, (ry + rh - 2) as i32),
            Terrain::Grave,
        );
    }

    let entry_x = rx + rw / 2;
    for y in (ry + rh)..(map.height - 1) {
        map.set_terrain(Position::new(entry_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(entry_x as i32, (ry + rh) as i32),
        Terrain::DoorClosed,
    );

    let up_pos = Position::new(entry_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Ranger goal: Twisted forest (Scorpius) --
fn generate_ranger_goal(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();
    fill_map_interior(&mut map, Terrain::Tree);

    // Central clearing.
    let rx = 25usize;
    let ry = 5usize;
    let rw = 30usize;
    let rh = 10usize;
    fill_terrain(&mut map, rx, ry, rw, rh, Terrain::Floor);

    // Scattered trees inside for cover.
    for _ in 0..rng.random_range(3..=6u32) {
        let tx = rng.random_range((rx + 2)..(rx + rw - 2)) as i32;
        let ty = rng.random_range((ry + 2)..(ry + rh - 2)) as i32;
        map.set_terrain(Position::new(tx, ty), Terrain::Tree);
    }

    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    // Path south through forest.
    let path_x = rx + rw / 2;
    for y in (ry + rh)..map.height - 1 {
        map.set_terrain(Position::new(path_x as i32, y as i32), Terrain::Floor);
    }

    let up_pos = Position::new(path_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    let room = Room {
        x: rx + 1,
        y: ry + 1,
        width: rw - 2,
        height: rh - 2,
        lit: true,
    };

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Rogue goal: Vault (Master Assassin) --
fn generate_rogue_quest_goal(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    // Vault: small inner room inside a larger room.
    let rx = 25usize;
    let ry = 3usize;
    let rw = 30usize;
    let rh = 14usize;
    let room = draw_quest_room(&mut map, rx, ry, rw, rh, false);

    // Inner vault.
    draw_quest_room(&mut map, rx + 8, ry + 4, 14, 6, false);
    map.set_terrain(
        Position::new((rx + 8) as i32, (ry + 7) as i32),
        Terrain::DoorLocked,
    );

    // Altar inside vault.
    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    let entry_x = rx + rw / 2;
    for y in (ry + rh)..(map.height - 1) {
        map.set_terrain(Position::new(entry_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(entry_x as i32, (ry + rh) as i32),
        Terrain::DoorClosed,
    );

    let up_pos = Position::new(entry_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Samurai goal: Enemy fortress (Ashikaga Takauji) --
fn generate_samurai_goal(_rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    let rx = 20usize;
    let ry = 3usize;
    let rw = 40usize;
    let rh = 14usize;

    // Moat.
    for x in (rx.saturating_sub(1))..=(rx + rw + 1).min(map.width - 1) {
        for y in (ry.saturating_sub(1))..=(ry + rh + 1).min(map.height - 1) {
            let on_edge = x == rx.saturating_sub(1)
                || x == (rx + rw + 1).min(map.width - 1)
                || y == ry.saturating_sub(1)
                || y == (ry + rh + 1).min(map.height - 1);
            if on_edge {
                map.set_terrain(Position::new(x as i32, y as i32), Terrain::Moat);
            }
        }
    }

    let room = draw_quest_room(&mut map, rx, ry, rw, rh, false);

    // Drawbridge.
    let db_x = rx + rw / 2;
    map.set_terrain(
        Position::new(db_x as i32, (ry + rh) as i32),
        Terrain::Drawbridge,
    );
    map.set_terrain(
        Position::new(db_x as i32, (ry + rh + 1).min(map.height - 1) as i32),
        Terrain::Drawbridge,
    );

    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    for y in ((ry + rh + 2).min(map.height - 1))..(map.height - 1) {
        map.set_terrain(Position::new(db_x as i32, y as i32), Terrain::Corridor);
    }

    let up_pos = Position::new(db_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Tourist goal: Monster-infested ruins (Master of Thieves) --
fn generate_tourist_goal(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    let rx = 20usize;
    let ry = 3usize;
    let rw = 40usize;
    let rh = 14usize;
    let room = draw_quest_room(&mut map, rx, ry, rw, rh, false);

    // Scattered rubble (fountains and graves representing ruins).
    for _ in 0..rng.random_range(3..=6u32) {
        let fx = rng.random_range((rx + 2)..(rx + rw - 2)) as i32;
        let fy = rng.random_range((ry + 2)..(ry + rh - 2)) as i32;
        map.set_terrain(Position::new(fx, fy), Terrain::Grave);
    }

    map.set_terrain(
        Position::new((rx + rw / 2) as i32, (ry + rh / 2) as i32),
        Terrain::Altar,
    );

    let entry_x = rx + rw / 2;
    for y in (ry + rh)..(map.height - 1) {
        map.set_terrain(Position::new(entry_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(entry_x as i32, (ry + rh) as i32),
        Terrain::DoorClosed,
    );

    let up_pos = Position::new(entry_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// -- Generic quest goal fallback --
fn generate_generic_quest_goal(rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    let room_w = rng.random_range(30..=40u32) as usize;
    let room_h = rng.random_range(12..=16u32) as usize;
    let rx = (map.width - room_w) / 2;
    let ry = (map.height - room_h) / 2;
    let room = draw_quest_room(&mut map, rx, ry, room_w, room_h, true);

    map.set_terrain(
        Position::new((rx + room_w / 2) as i32, (ry + room_h / 2) as i32),
        Terrain::Altar,
    );

    let entry_x = rx + room_w / 2;
    for y in (ry + room_h)..(map.height - 1) {
        map.set_terrain(Position::new(entry_x as i32, y as i32), Terrain::Corridor);
    }
    map.set_terrain(
        Position::new(entry_x as i32, (ry + room_h) as i32),
        Terrain::DoorClosed,
    );

    let up_pos = Position::new(entry_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: None,
        },
        flags: SpecialLevelFlags {
            no_dig: false,
            no_teleport: true,
            no_prayer: false,
            is_endgame: false,
        },
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Quest Filler — role dispatch
// ═══════════════════════════════════════════════════════════════════════════

/// Generate a quest filler level, with role-appropriate background terrain.
pub fn generate_quest_filler(role: &str, rng: &mut impl Rng) -> SpecialLevel {
    let mut map = LevelMap::new_standard();

    // Role-specific background.
    match role {
        "ranger" => fill_map_interior(&mut map, Terrain::Tree),
        "valkyrie" => fill_map_interior(&mut map, Terrain::Ice),
        _ => {}
    }

    // Single room.
    let room_w = rng.random_range(20..=35u32) as usize;
    let room_h = rng.random_range(8..=14u32) as usize;
    let rx = (map.width - room_w) / 2;
    let ry = (map.height - room_h) / 2;
    let room = draw_quest_room(&mut map, rx, ry, room_w, room_h, true);

    // Corridors to map edges (north and south).
    let corr_x = rx + room_w / 2;
    for y in 1..ry {
        let pos = Position::new(corr_x as i32, y as i32);
        let t = map.cells[y][corr_x].terrain;
        if matches!(
            t,
            Terrain::Stone | Terrain::Wall | Terrain::Tree | Terrain::Ice
        ) {
            map.set_terrain(pos, Terrain::Corridor);
        }
    }
    map.set_terrain(Position::new(corr_x as i32, ry as i32), Terrain::DoorClosed);
    for y in ((ry + room_h).min(map.height - 1) + 1)..map.height.saturating_sub(1) {
        let pos = Position::new(corr_x as i32, y as i32);
        let t = map.cells[y][corr_x].terrain;
        if matches!(
            t,
            Terrain::Stone | Terrain::Wall | Terrain::Tree | Terrain::Ice
        ) {
            map.set_terrain(pos, Terrain::Corridor);
        }
    }
    map.set_terrain(
        Position::new(corr_x as i32, (ry + room_h).min(map.height - 1) as i32),
        Terrain::DoorClosed,
    );

    // Stairs.
    let up_pos = Position::new(corr_x as i32, 1);
    let down_pos = Position::new(corr_x as i32, (map.height - 2) as i32);
    map.set_terrain(up_pos, Terrain::StairsUp);
    map.set_terrain(down_pos, Terrain::StairsDown);

    SpecialLevel {
        generated: GeneratedLevel {
            map,
            rooms: vec![room],
            up_stairs: Some(up_pos),
            down_stairs: Some(down_pos),
        },
        flags: SpecialLevelFlags::default(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    use crate::makemon::{GoodPosFlags, goodpos};
    use crate::world::{GameWorld, Positioned};
    use nethack_babel_data::{GameData, MonsterDef, loader::load_game_data};

    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(42)
    }

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn test_game_data() -> &'static GameData {
        static DATA: OnceLock<GameData> = OnceLock::new();
        DATA.get_or_init(|| {
            load_game_data(&data_dir())
                .unwrap_or_else(|e| panic!("failed to load test game data: {}", e))
        })
    }

    fn resolve_monster_def_for_test(spec: &str) -> Option<&'static MonsterDef> {
        let spec = spec.trim();
        let data = test_game_data();
        if let Some(class_str) = spec.strip_prefix("class:") {
            let class = class_str.chars().next()?;
            return data
                .monsters
                .iter()
                .find(|def| def.symbol.eq_ignore_ascii_case(&class));
        }

        data.monsters.iter().find(|def| {
            def.names.male.eq_ignore_ascii_case(spec)
                || def
                    .names
                    .female
                    .as_ref()
                    .is_some_and(|female| female.eq_ignore_ascii_case(spec))
        })
    }

    fn make_population_test_world(map: LevelMap) -> GameWorld {
        let mut world = GameWorld::new(Position::new(5, 5));
        world.dungeon_mut().current_level = map;
        if let Some(mut player_pos) = world.get_component_mut::<Positioned>(world.player()) {
            player_pos.0 = Position::new(
                (LevelMap::DEFAULT_WIDTH - 1) as i32,
                (LevelMap::DEFAULT_HEIGHT - 1) as i32,
            );
        }
        world
    }

    // ── Sokoban tests ────────────────────────────────────────────────

    #[test]
    fn sokoban_has_correct_dimensions() {
        let mut rng = test_rng();
        for level in 0..4u8 {
            let sl = generate_sokoban(level, &mut rng);
            assert_eq!(sl.generated.map.width, LevelMap::DEFAULT_WIDTH);
            assert_eq!(sl.generated.map.height, LevelMap::DEFAULT_HEIGHT);
        }
    }

    #[test]
    fn sokoban_has_boulders() {
        // Each Sokoban level should have at least 4 boulders.
        for level in 0..4u8 {
            let count = sokoban_boulder_count(level);
            assert!(
                count >= 4,
                "Sokoban level {} has only {} boulders, expected >= 4",
                level,
                count
            );
        }
    }

    #[test]
    fn sokoban_has_no_dig_flag() {
        let mut rng = test_rng();
        let sl = generate_sokoban(0, &mut rng);
        assert!(sl.flags.no_dig, "Sokoban should have no_dig flag set");
        assert!(
            sl.flags.no_teleport,
            "Sokoban should have no_teleport flag set"
        );
    }

    #[test]
    fn sokoban_has_stairs() {
        let mut rng = test_rng();
        for level in 0..4u8 {
            let sl = generate_sokoban(level, &mut rng);
            assert!(
                sl.generated.up_stairs.is_some() || sl.generated.down_stairs.is_some(),
                "Sokoban level {} should have at least one staircase",
                level
            );
        }
    }

    #[test]
    fn sokoban_connectivity() {
        let mut rng = test_rng();
        for level in 0..4u8 {
            let sl = generate_sokoban(level, &mut rng);
            let map = &sl.generated.map;

            let start = find_first_walkable(map);
            assert!(
                start.is_some(),
                "Sokoban level {} has no walkable cells",
                level
            );
            let reachable = flood_fill(map, start.unwrap());

            // All floor cells should be reachable from the first walkable.
            let unreachable_floors = count_unreachable_floors(map, &reachable);
            // Allow a small number of unreachable cells from puzzle
            // geometry (trap holes that are intentionally isolated).
            assert!(
                unreachable_floors <= 10,
                "Sokoban level {} has {} unreachable floor cells",
                level,
                unreachable_floors
            );
        }
    }

    // ── Mines tests ──────────────────────────────────────────────────

    #[test]
    fn mines_has_rooms() {
        let mut rng = test_rng();
        let level = generate_mines_level(5, &mut rng);
        assert!(
            level.rooms.len() >= 3,
            "Mines should have >= 3 rooms, got {}",
            level.rooms.len()
        );
    }

    #[test]
    fn mines_connectivity() {
        let mut rng = test_rng();
        let level = generate_mines_level(5, &mut rng);
        let map = &level.map;

        let start = find_first_walkable(map).expect("Mines should have walkable cells");
        let reachable = flood_fill(map, start);

        // Every room should be reachable.
        for (i, room) in level.rooms.iter().enumerate() {
            let room_reachable =
                (room.y..=room.bottom()).any(|y| (room.x..=room.right()).any(|x| reachable[y][x]));
            assert!(room_reachable, "Mines room {} is not reachable", i);
        }
    }

    // ── Oracle tests ─────────────────────────────────────────────────

    #[test]
    fn oracle_has_delphi_with_fountains() {
        let mut rng = test_rng();
        let level = generate_oracle_level(&mut rng);
        let map = &level.map;

        // Count fountains — Delphi should have exactly 4.
        let fountain_count = (0..map.height)
            .flat_map(|y| (0..map.width).map(move |x| (x, y)))
            .filter(|&(x, y)| map.cells[y][x].terrain == Terrain::Fountain)
            .count();
        assert!(
            fountain_count >= 4,
            "Oracle level should have >= 4 fountains, got {}",
            fountain_count
        );
    }

    #[test]
    fn oracle_has_both_stairs() {
        let mut rng = test_rng();
        let level = generate_oracle_level(&mut rng);
        assert!(
            level.up_stairs.is_some(),
            "Oracle level should have up stairs"
        );
        assert!(
            level.down_stairs.is_some(),
            "Oracle level should have down stairs"
        );
    }

    #[test]
    fn oracle_connectivity() {
        let mut rng = test_rng();
        let level = generate_oracle_level(&mut rng);
        let map = &level.map;

        let start = find_first_walkable(map).expect("Oracle level should have walkable cells");
        let reachable = flood_fill(map, start);

        for (i, room) in level.rooms.iter().enumerate() {
            let room_reachable =
                (room.y..=room.bottom()).any(|y| (room.x..=room.right()).any(|x| reachable[y][x]));
            assert!(room_reachable, "Oracle room {} is not reachable", i);
        }
    }

    // ── Sokoban 1a/4a tests ────────────────────────────────────────

    #[test]
    fn test_sokoban_1a_layout() {
        // Sokoban level 0 (1a) should have the correct boulder count.
        let count = sokoban_boulder_count(0);
        // Count from the hardcoded puzzle: 0s in the map.
        assert!(
            count >= 8,
            "Sokoban 1a should have >= 8 boulders, got {}",
            count
        );

        // Verify the generated map has floor tiles at boulder positions.
        let mut rng = test_rng();
        let sl = generate_sokoban(0, &mut rng);
        let map = &sl.generated.map;

        // Count floor tiles (boulders are placed on floor).
        let floor_count = (0..map.height)
            .flat_map(|y| (0..map.width).map(move |x| (x, y)))
            .filter(|&(x, y)| map.cells[y][x].terrain == Terrain::Floor)
            .count();
        assert!(floor_count > 0, "Sokoban 1a should have floor tiles");
    }

    #[test]
    fn test_sokoban_4a_has_reward() {
        // Sokoban level 3 (4a, top) should have trap tiles for the
        // reward area.
        let puzzle = &SOKOBAN_PUZZLES[3];
        let trap_count = puzzle
            .map
            .iter()
            .flat_map(|line| line.chars())
            .filter(|&c| c == '^')
            .count();
        assert!(
            trap_count > 0,
            "Sokoban 4a should have trap ('^') tiles for the reward area, got {}",
            trap_count
        );

        // The reward RNG path should be exercised.
        let mut rng = test_rng();
        let sl = generate_sokoban(3, &mut rng);
        assert!(
            sl.generated.up_stairs.is_some(),
            "Sokoban 4a should have up stairs"
        );
    }

    // ── Mines cavern shape test ─────────────────────────────────────

    #[test]
    fn test_mines_level_has_caverns() {
        // Mines rooms should be irregular: at least some rooms should
        // have wall cells inside their bounding box (nibbled corners).
        let mut rng = test_rng();
        let level = generate_mines_level(5, &mut rng);
        let map = &level.map;

        let mut has_irregular = false;
        for room in &level.rooms {
            let total_interior = room.width * room.height;
            let floor_count = (room.y..=room.bottom())
                .flat_map(|y| (room.x..=room.right()).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    map.cells[y][x].terrain == Terrain::Floor
                        || map.cells[y][x].terrain == Terrain::Fountain
                })
                .count();
            // If fewer floor cells than total interior, corners were nibbled.
            if floor_count < total_interior {
                has_irregular = true;
                break;
            }
        }
        assert!(
            has_irregular,
            "At least one Mines room should have irregular (non-rectangular) shape"
        );
    }

    // ── Oracle fountain test ────────────────────────────────────────

    #[test]
    fn test_oracle_has_fountains() {
        let mut rng = test_rng();
        let level = generate_oracle_level(&mut rng);
        let map = &level.map;

        let fountain_count = count_terrain(map, Terrain::Fountain);
        assert_eq!(
            fountain_count, 4,
            "Oracle level should have exactly 4 fountains, got {}",
            fountain_count
        );
    }

    // ── Minetown tests ──────────────────────────────────────────────

    #[test]
    fn test_minetown_has_shop() {
        let mut rng = test_rng();
        let mt = generate_minetown(&mut rng);

        let shop_count = mt
            .room_types
            .iter()
            .filter(|t| {
                matches!(
                    t,
                    MinetownRoomType::GeneralStore | MinetownRoomType::SpecialtyShop
                )
            })
            .count();
        assert!(
            shop_count >= 1,
            "Minetown should have at least 1 shop room, got {}",
            shop_count
        );
    }

    #[test]
    fn minetown_has_temple() {
        let mut rng = test_rng();
        let mt = generate_minetown(&mut rng);

        let has_temple = mt.room_types.iter().any(|t| *t == MinetownRoomType::Temple);
        assert!(has_temple, "Minetown should have a temple");

        let map = &mt.generated.map;
        let altar_count = count_terrain(map, Terrain::Altar);
        assert!(
            altar_count >= 1,
            "Minetown temple should have at least 1 altar, got {}",
            altar_count
        );
    }

    #[test]
    fn minetown_has_buildings() {
        let mut rng = test_rng();
        let mt = generate_minetown(&mut rng);
        assert!(
            mt.generated.rooms.len() >= 3,
            "Minetown should have >= 3 buildings, got {}",
            mt.generated.rooms.len()
        );
    }

    #[test]
    fn minetown_has_stairs() {
        let mut rng = test_rng();
        let mt = generate_minetown(&mut rng);
        assert!(
            mt.generated.up_stairs.is_some(),
            "Minetown should have up stairs"
        );
        assert!(
            mt.generated.down_stairs.is_some(),
            "Minetown should have down stairs"
        );
    }

    // ── Castle tests ────────────────────────────────────────────────

    #[test]
    fn test_castle_has_drawbridge() {
        let mut rng = test_rng();
        let sl = generate_castle(&mut rng);
        let map = &sl.generated.map;

        let drawbridge_count = count_terrain(map, Terrain::Drawbridge);
        assert!(
            drawbridge_count >= 1,
            "Castle should have at least 1 drawbridge tile, got {}",
            drawbridge_count
        );
    }

    #[test]
    fn castle_has_moat() {
        let mut rng = test_rng();
        let sl = generate_castle(&mut rng);
        let map = &sl.generated.map;

        let moat_count = count_terrain(map, Terrain::Moat);
        assert!(
            moat_count >= 10,
            "Castle should have substantial moat, got {} tiles",
            moat_count
        );
    }

    #[test]
    fn castle_has_rooms() {
        let mut rng = test_rng();
        let sl = generate_castle(&mut rng);
        assert!(
            sl.generated.rooms.len() >= 2,
            "Castle should have >= 2 internal rooms, got {}",
            sl.generated.rooms.len()
        );
    }

    #[test]
    fn castle_has_stairs() {
        let mut rng = test_rng();
        let sl = generate_castle(&mut rng);
        assert!(
            sl.generated.up_stairs.is_some(),
            "Castle should have up stairs"
        );
        assert!(
            sl.generated.down_stairs.is_some(),
            "Castle should have down stairs"
        );
    }

    // ── Medusa tests ────────────────────────────────────────────────

    #[test]
    fn test_medusa_has_water() {
        let mut rng = test_rng();
        let sl = generate_medusa(0, &mut rng);
        let map = &sl.generated.map;

        let water_count = count_terrain(map, Terrain::Water);
        let total = map.width * map.height;
        assert!(
            water_count > total / 3,
            "Medusa's island should be surrounded by water ({} of {} cells)",
            water_count,
            total
        );
    }

    #[test]
    fn medusa_has_island_floor() {
        let mut rng = test_rng();
        let sl = generate_medusa(0, &mut rng);
        let map = &sl.generated.map;

        let floor_count = count_terrain(map, Terrain::Floor);
        assert!(
            floor_count >= 20,
            "Medusa's island should have floor tiles, got {}",
            floor_count
        );
    }

    #[test]
    fn medusa_has_statues() {
        let mut rng = test_rng();
        let sl = generate_medusa(0, &mut rng);
        let map = &sl.generated.map;

        // Statues are represented as Grave terrain.
        let statue_count = count_terrain(map, Terrain::Grave);
        assert!(
            statue_count >= 1,
            "Medusa's island should have statues (graves), got {}",
            statue_count
        );
    }

    #[test]
    fn medusa_has_stairs() {
        let mut rng = test_rng();
        for variant in 0..2u8 {
            let sl = generate_medusa(variant, &mut rng);
            assert!(
                sl.generated.up_stairs.is_some(),
                "Medusa variant {} should have up stairs",
                variant
            );
            assert!(
                sl.generated.down_stairs.is_some(),
                "Medusa variant {} should have down stairs",
                variant
            );
        }
    }

    // ── Shared test helpers ──────────────────────────────────────────

    fn is_passable(terrain: Terrain) -> bool {
        terrain.is_walkable() || matches!(terrain, Terrain::DoorClosed | Terrain::DoorLocked)
    }

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

    fn flood_fill(map: &LevelMap, start: (usize, usize)) -> Vec<Vec<bool>> {
        let mut visited = vec![vec![false; map.width]; map.height];
        let mut stack = vec![start];
        while let Some((x, y)) = stack.pop() {
            if visited[y][x] {
                continue;
            }
            if !is_passable(map.cells[y][x].terrain) {
                continue;
            }
            visited[y][x] = true;
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

    fn count_unreachable_floors(map: &LevelMap, reachable: &[Vec<bool>]) -> usize {
        let mut count = 0;
        for y in 0..map.height {
            for x in 0..map.width {
                if map.cells[y][x].terrain == Terrain::Floor && !reachable[y][x] {
                    count += 1;
                }
            }
        }
        count
    }

    fn count_terrain(map: &LevelMap, terrain: Terrain) -> usize {
        (0..map.height)
            .flat_map(|y| (0..map.width).map(move |x| (x, y)))
            .filter(|&(x, y)| map.cells[y][x].terrain == terrain)
            .count()
    }

    // ── Vlad's Tower tests ──────────────────────────────────────────

    #[test]
    fn test_vlad_tower_three_levels() {
        // Vlad's Tower has 3 levels.  Each should generate successfully
        // and be connected via stairs.
        let mut rng = test_rng();

        let level1 = generate_vlad_tower(1, &mut rng);
        let level2 = generate_vlad_tower(2, &mut rng);
        let level3 = generate_vlad_tower(3, &mut rng);

        // Level 1 (bottom): has stairs up, no stairs down.
        assert!(
            level1.generated.up_stairs.is_some(),
            "Vlad level 1 should have stairs up"
        );
        assert!(
            level1.generated.down_stairs.is_none(),
            "Vlad level 1 should NOT have stairs down"
        );

        // Level 2 (middle): has both stairs.
        assert!(
            level2.generated.up_stairs.is_some(),
            "Vlad level 2 should have stairs up"
        );
        assert!(
            level2.generated.down_stairs.is_some(),
            "Vlad level 2 should have stairs down"
        );

        // Level 3 (top): has stairs down, no stairs up.
        assert!(
            level3.generated.up_stairs.is_none(),
            "Vlad level 3 should NOT have stairs up"
        );
        assert!(
            level3.generated.down_stairs.is_some(),
            "Vlad level 3 should have stairs down"
        );

        // All levels should have no_dig and no_teleport.
        assert!(level1.flags.no_dig);
        assert!(level2.flags.no_teleport);
        assert!(level3.flags.no_prayer);
    }

    // ── Sanctum tests ───────────────────────────────────────────────

    #[test]
    fn test_sanctum_has_altar() {
        let mut rng = test_rng();
        let sanctum = generate_sanctum(&mut rng);
        let map = &sanctum.generated.map;

        let altar_count = count_terrain(map, Terrain::Altar);
        assert!(
            altar_count >= 1,
            "Sanctum should have at least 1 altar (Moloch), got {}",
            altar_count
        );

        // Should have moat terrain.
        let moat_count = count_terrain(map, Terrain::Moat);
        assert!(moat_count > 0, "Sanctum should have moat terrain, got 0");

        // Flags.
        assert!(sanctum.flags.no_prayer, "Sanctum should have no_prayer");
        assert!(sanctum.flags.no_dig, "Sanctum should have no_dig");
    }

    // ── Elemental Plane tests ───────────────────────────────────────

    #[test]
    fn test_elemental_plane_portals() {
        // Each elemental plane should have a magic portal.
        let planes = [
            ElementalPlane::Earth,
            ElementalPlane::Air,
            ElementalPlane::Fire,
            ElementalPlane::Water,
        ];

        for plane in &planes {
            let mut rng = test_rng();
            let level = generate_elemental_plane(*plane, &mut rng);
            let map = &level.generated.map;

            let portal_count = count_terrain(map, Terrain::MagicPortal);
            assert!(
                portal_count >= 1,
                "{:?} plane should have at least 1 magic portal, got {}",
                plane,
                portal_count
            );

            // Each plane should be marked as endgame.
            assert!(
                level.flags.is_endgame,
                "{:?} plane should have is_endgame flag",
                plane
            );
        }
    }

    #[test]
    fn test_elemental_plane_earth_has_stone() {
        let mut rng = test_rng();
        let level = generate_elemental_plane(ElementalPlane::Earth, &mut rng);
        let stone_count = count_terrain(&level.generated.map, Terrain::Stone);
        assert!(
            stone_count > 100,
            "Earth plane should be mostly stone, got {} stone cells",
            stone_count
        );
    }

    #[test]
    fn test_elemental_plane_fire_has_lava() {
        let mut rng = test_rng();
        let level = generate_elemental_plane(ElementalPlane::Fire, &mut rng);
        let lava_count = count_terrain(&level.generated.map, Terrain::Lava);
        assert!(
            lava_count > 100,
            "Fire plane should have much lava, got {} lava cells",
            lava_count
        );
    }

    #[test]
    fn test_elemental_plane_water_has_water() {
        let mut rng = test_rng();
        let level = generate_elemental_plane(ElementalPlane::Water, &mut rng);
        let water_count = count_terrain(&level.generated.map, Terrain::Water);
        assert!(
            water_count > 100,
            "Water plane should have much water, got {} water cells",
            water_count
        );
    }

    #[test]
    fn test_elemental_plane_air_has_air() {
        let mut rng = test_rng();
        let level = generate_elemental_plane(ElementalPlane::Air, &mut rng);
        let air_count = count_terrain(&level.generated.map, Terrain::Air);
        assert!(
            air_count > 100,
            "Air plane should have much air, got {} air cells",
            air_count
        );
    }

    // ── Astral Plane tests ──────────────────────────────────────────

    #[test]
    fn test_astral_has_three_altars() {
        let mut rng = test_rng();
        let astral = generate_astral_plane(&mut rng);
        let map = &astral.generated.map;

        let altar_count = count_terrain(map, Terrain::Altar);
        assert_eq!(
            altar_count, 3,
            "Astral Plane should have exactly 3 altars, got {}",
            altar_count
        );

        // Should have 3 temple rooms.
        assert_eq!(
            astral.generated.rooms.len(),
            3,
            "Astral Plane should have 3 temple rooms, got {}",
            astral.generated.rooms.len()
        );

        // Should be flagged as endgame.
        assert!(astral.flags.is_endgame, "Astral should be endgame");
        assert!(astral.flags.no_teleport, "Astral should be no-teleport");
    }

    #[test]
    fn test_astral_has_stairs() {
        let mut rng = test_rng();
        let astral = generate_astral_plane(&mut rng);
        assert!(
            astral.generated.up_stairs.is_some(),
            "Astral Plane should have stairs up (entry)"
        );
    }

    // ── Fort Ludios tests ──────────────────────────────────────────

    #[test]
    fn test_fort_ludios_has_moat() {
        let mut rng = test_rng();
        let sl = generate_fort_ludios(&mut rng);
        let map = &sl.generated.map;

        let moat_count = count_terrain(map, Terrain::Moat);
        assert!(
            moat_count >= 10,
            "Fort Ludios should have substantial moat, got {} tiles",
            moat_count
        );
    }

    #[test]
    fn test_fort_ludios_has_drawbridge() {
        let mut rng = test_rng();
        let sl = generate_fort_ludios(&mut rng);
        let map = &sl.generated.map;

        let drawbridge_count = count_terrain(map, Terrain::Drawbridge);
        assert!(
            drawbridge_count >= 1,
            "Fort Ludios should have a drawbridge, got {}",
            drawbridge_count
        );
    }

    #[test]
    fn test_fort_ludios_has_throne() {
        let mut rng = test_rng();
        let sl = generate_fort_ludios(&mut rng);
        let map = &sl.generated.map;

        let throne_count = count_terrain(map, Terrain::Throne);
        assert!(
            throne_count >= 1,
            "Fort Ludios should have a throne, got {}",
            throne_count
        );
    }

    #[test]
    fn test_fort_ludios_has_rooms() {
        let mut rng = test_rng();
        let sl = generate_fort_ludios(&mut rng);
        assert!(
            sl.generated.rooms.len() >= 3,
            "Fort Ludios should have >= 3 rooms, got {}",
            sl.generated.rooms.len()
        );
    }

    #[test]
    fn test_fort_ludios_has_portal() {
        let mut rng = test_rng();
        let sl = generate_fort_ludios(&mut rng);
        let map = &sl.generated.map;

        let portal_count = count_terrain(map, Terrain::MagicPortal);
        assert!(
            portal_count >= 1,
            "Fort Ludios should have a magic portal, got {}",
            portal_count
        );
    }

    #[test]
    fn test_fort_ludios_no_stairs() {
        // Fort Ludios is accessed via portal only.
        let mut rng = test_rng();
        let sl = generate_fort_ludios(&mut rng);
        assert!(
            sl.generated.up_stairs.is_none(),
            "Fort Ludios should have no up stairs (portal access only)"
        );
    }

    #[test]
    fn test_fort_ludios_flags() {
        let mut rng = test_rng();
        let sl = generate_fort_ludios(&mut rng);
        assert!(!sl.flags.no_dig, "Fort Ludios should allow digging");
        assert!(
            !sl.flags.no_teleport,
            "Fort Ludios should allow teleporting"
        );
        assert!(!sl.flags.is_endgame, "Fort Ludios is not endgame");
    }

    // ── Wizard Tower tests ─────────────────────────────────────────

    #[test]
    fn test_wizard_tower_has_portal() {
        let mut rng = test_rng();
        let sl = generate_wizard_tower(&mut rng);
        let map = &sl.generated.map;

        let portal_count = count_terrain(map, Terrain::MagicPortal);
        assert!(
            portal_count >= 1,
            "Wizard Tower should have a magic portal, got {}",
            portal_count
        );
    }

    #[test]
    fn test_wizard_tower_has_stairs_up() {
        let mut rng = test_rng();
        let sl = generate_wizard_tower(&mut rng);
        assert!(
            sl.generated.up_stairs.is_some(),
            "Wizard Tower should have stairs up"
        );
    }

    #[test]
    fn test_wizard_tower_flags() {
        let mut rng = test_rng();
        let sl = generate_wizard_tower(&mut rng);
        assert!(sl.flags.no_dig, "Wizard Tower should have no_dig");
        assert!(sl.flags.no_teleport, "Wizard Tower should have no_teleport");
        assert!(sl.flags.no_prayer, "Wizard Tower should have no_prayer");
    }

    // ── Special level ID dispatch tests ────────────────────────────

    #[test]
    fn test_identify_castle() {
        use crate::dungeon::DungeonBranch;
        assert_eq!(
            identify_special_level(DungeonBranch::Main, 25),
            Some(SpecialLevelId::Castle)
        );
    }

    #[test]
    fn test_identify_medusa() {
        use crate::dungeon::DungeonBranch;
        assert_eq!(
            identify_special_level(DungeonBranch::Main, 24),
            Some(SpecialLevelId::Medusa(0))
        );
    }

    #[test]
    fn test_identify_sokoban() {
        use crate::dungeon::DungeonBranch;
        for depth in 1..=4 {
            assert_eq!(
                identify_special_level(DungeonBranch::Sokoban, depth),
                Some(SpecialLevelId::Sokoban(depth as u8))
            );
        }
        assert_eq!(identify_special_level(DungeonBranch::Sokoban, 5), None);
    }

    #[test]
    fn test_identify_vlad() {
        use crate::dungeon::DungeonBranch;
        for depth in 1..=3 {
            assert_eq!(
                identify_special_level(DungeonBranch::VladsTower, depth),
                Some(SpecialLevelId::VladsTower(depth as u8))
            );
        }
    }

    #[test]
    fn test_identify_endgame() {
        use crate::dungeon::DungeonBranch;
        assert_eq!(
            identify_special_level(DungeonBranch::Endgame, 1),
            Some(SpecialLevelId::EarthPlane)
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Endgame, 5),
            Some(SpecialLevelId::AstralPlane)
        );
    }

    #[test]
    fn test_identify_fort_ludios() {
        use crate::dungeon::DungeonBranch;
        assert_eq!(
            identify_special_level(DungeonBranch::FortLudios, 1),
            Some(SpecialLevelId::FortLudios)
        );
    }

    #[test]
    fn test_identify_sanctum() {
        use crate::dungeon::DungeonBranch;
        assert_eq!(
            identify_special_level(DungeonBranch::Gehennom, 20),
            Some(SpecialLevelId::Sanctum)
        );
    }

    #[test]
    fn test_identify_wizard_tower() {
        use crate::dungeon::DungeonBranch;
        assert_eq!(
            identify_special_level(DungeonBranch::Gehennom, 17),
            Some(SpecialLevelId::WizardTower)
        );
    }

    #[test]
    fn test_identify_minetown() {
        use crate::dungeon::DungeonBranch;
        assert_eq!(
            identify_special_level(DungeonBranch::Mines, 5),
            Some(SpecialLevelId::Minetown)
        );
    }

    #[test]
    fn test_identify_regular_level_returns_none() {
        use crate::dungeon::DungeonBranch;
        assert_eq!(identify_special_level(DungeonBranch::Main, 10), None);
        assert_eq!(identify_special_level(DungeonBranch::Mines, 3), None);
        assert_eq!(identify_special_level(DungeonBranch::Gehennom, 3), None);
    }

    // ── Reproducibility tests ──────────────────────────────────────

    #[test]
    fn test_sokoban_reproducible() {
        let a = generate_sokoban(2, &mut Pcg64::seed_from_u64(100));
        let b = generate_sokoban(2, &mut Pcg64::seed_from_u64(100));
        for y in 0..a.generated.map.height {
            for x in 0..a.generated.map.width {
                assert_eq!(
                    a.generated.map.cells[y][x].terrain,
                    b.generated.map.cells[y][x].terrain,
                );
            }
        }
    }

    #[test]
    fn test_castle_reproducible() {
        let a = generate_castle(&mut Pcg64::seed_from_u64(200));
        let b = generate_castle(&mut Pcg64::seed_from_u64(200));
        assert_eq!(a.generated.rooms.len(), b.generated.rooms.len());
        assert_eq!(a.generated.up_stairs, b.generated.up_stairs);
        assert_eq!(a.generated.down_stairs, b.generated.down_stairs);
    }

    #[test]
    fn test_astral_reproducible() {
        let a = generate_astral_plane(&mut Pcg64::seed_from_u64(300));
        let b = generate_astral_plane(&mut Pcg64::seed_from_u64(300));
        assert_eq!(a.generated.rooms.len(), b.generated.rooms.len());
        let altar_a = count_terrain(&a.generated.map, Terrain::Altar);
        let altar_b = count_terrain(&b.generated.map, Terrain::Altar);
        assert_eq!(altar_a, altar_b);
    }

    // ── Multiple seed stability tests ──────────────────────────────

    #[test]
    fn test_castle_multiple_seeds() {
        for seed in 0..10u64 {
            let mut rng = Pcg64::seed_from_u64(seed + 500);
            let sl = generate_castle(&mut rng);
            assert!(
                sl.generated.rooms.len() >= 2,
                "seed {}: Castle should have rooms",
                seed
            );
            let moat = count_terrain(&sl.generated.map, Terrain::Moat);
            assert!(moat > 0, "seed {}: Castle should have moat", seed);
        }
    }

    #[test]
    fn test_medusa_multiple_seeds() {
        for seed in 0..10u64 {
            let mut rng = Pcg64::seed_from_u64(seed + 600);
            for variant in 0..2u8 {
                let sl = generate_medusa(variant, &mut rng);
                let water = count_terrain(&sl.generated.map, Terrain::Water);
                assert!(
                    water > 0,
                    "seed {}, variant {}: Medusa should have water",
                    seed,
                    variant
                );
            }
        }
    }

    #[test]
    fn test_vlad_multiple_seeds() {
        for seed in 0..10u64 {
            let mut rng = Pcg64::seed_from_u64(seed + 700);
            for level in 1..=3u8 {
                let sl = generate_vlad_tower(level, &mut rng);
                let floor = count_terrain(&sl.generated.map, Terrain::Floor);
                assert!(
                    floor > 10,
                    "seed {}, level {}: Vlad should have floor",
                    seed,
                    level
                );
            }
        }
    }

    #[test]
    fn test_elemental_planes_multiple_seeds() {
        let planes = [
            ElementalPlane::Earth,
            ElementalPlane::Air,
            ElementalPlane::Fire,
            ElementalPlane::Water,
        ];
        for seed in 0..5u64 {
            for plane in &planes {
                let mut rng = Pcg64::seed_from_u64(seed + 800);
                let sl = generate_elemental_plane(*plane, &mut rng);
                assert!(sl.flags.is_endgame);
                assert!(sl.flags.no_dig);
            }
        }
    }

    #[test]
    fn test_minetown_multiple_seeds() {
        for seed in 0..10u64 {
            let mut rng = Pcg64::seed_from_u64(seed + 900);
            let mt = generate_minetown(&mut rng);
            assert!(
                mt.generated.rooms.len() >= 3,
                "seed {}: Minetown should have buildings",
                seed
            );
            assert!(
                mt.room_types
                    .iter()
                    .any(|t| *t == MinetownRoomType::GeneralStore),
                "seed {}: Minetown should have a general store",
                seed
            );
        }
    }

    // ── Gehennom special level identification tests ─────────────────

    #[test]
    fn test_identify_gehennom_special_levels() {
        use crate::dungeon::DungeonBranch;
        assert_eq!(
            identify_special_level(DungeonBranch::Gehennom, 1),
            Some(SpecialLevelId::Valley)
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Gehennom, 5),
            Some(SpecialLevelId::Juiblex)
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Gehennom, 7),
            Some(SpecialLevelId::Asmodeus)
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Gehennom, 10),
            Some(SpecialLevelId::Baalzebub)
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Gehennom, 12),
            Some(SpecialLevelId::Orcus)
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Gehennom, 14),
            Some(SpecialLevelId::FakeWizard(1))
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Gehennom, 15),
            Some(SpecialLevelId::FakeWizard(2))
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Gehennom, 18),
            Some(SpecialLevelId::WizardTower2)
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Gehennom, 19),
            Some(SpecialLevelId::WizardTower3)
        );
    }

    #[test]
    fn test_identify_quest_levels() {
        use crate::dungeon::DungeonBranch;
        assert_eq!(
            identify_special_level(DungeonBranch::Quest, 1),
            Some(SpecialLevelId::QuestStart)
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Quest, 4),
            Some(SpecialLevelId::QuestLocator)
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Quest, 7),
            Some(SpecialLevelId::QuestGoal)
        );
        assert_eq!(
            identify_special_level(DungeonBranch::Quest, 3),
            Some(SpecialLevelId::QuestFiller(3))
        );
    }

    // ── Dispatch tests ──────────────────────────────────────────────

    #[test]
    fn test_dispatch_existing_generators() {
        let mut rng = test_rng();
        let result = dispatch_special_level(SpecialLevelId::Castle, None, &mut rng);
        assert!(result.is_some(), "Castle should dispatch successfully");
        let result = dispatch_special_level(SpecialLevelId::Valley, None, &mut rng);
        assert!(result.is_some(), "Valley should dispatch successfully");
    }

    #[test]
    fn test_dispatch_all_ids_return_some() {
        let mut rng = test_rng();
        let result = dispatch_special_level(SpecialLevelId::BigRoom(0), None, &mut rng);
        assert!(result.is_some(), "BigRoom should dispatch successfully");
        let result = dispatch_special_level(SpecialLevelId::QuestStart, None, &mut rng);
        assert!(result.is_some(), "QuestStart should dispatch successfully");
        let result = dispatch_special_level(SpecialLevelId::Rogue, None, &mut rng);
        assert!(result.is_some(), "Rogue should dispatch successfully");
        let result = dispatch_special_level(SpecialLevelId::MinesEnd, None, &mut rng);
        assert!(result.is_some(), "MinesEnd should dispatch successfully");
        let result = dispatch_special_level(SpecialLevelId::QuestLocator, None, &mut rng);
        assert!(
            result.is_some(),
            "QuestLocator should dispatch successfully"
        );
        let result = dispatch_special_level(SpecialLevelId::QuestGoal, None, &mut rng);
        assert!(result.is_some(), "QuestGoal should dispatch successfully");
        let result = dispatch_special_level(SpecialLevelId::QuestFiller(3), None, &mut rng);
        assert!(result.is_some(), "QuestFiller should dispatch successfully");
    }

    #[test]
    fn test_population_oracle_contains_oracle() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::OracleLevel, None, &mut rng)
            .expect("oracle level should generate");
        let pop = population_for_special_level(SpecialLevelId::OracleLevel, &sl.generated);
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("oracle")),
            "oracle population should include Oracle"
        );
    }

    #[test]
    fn test_population_medusa_contains_medusa() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::Medusa(0), None, &mut rng)
            .expect("medusa level should generate");
        let pop = population_for_special_level(SpecialLevelId::Medusa(0), &sl.generated);
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("medusa")),
            "medusa population should include Medusa"
        );
    }

    #[test]
    fn test_population_castle_contains_wand_of_wishing() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::Castle, None, &mut rng)
            .expect("castle level should generate");
        let pop = population_for_special_level(SpecialLevelId::Castle, &sl.generated);
        assert!(
            pop.objects
                .iter()
                .any(|o| o.name.eq_ignore_ascii_case("wand of wishing")),
            "castle population should include wand of wishing"
        );
    }

    #[test]
    fn test_population_vlad_top_contains_vlad_and_candelabrum() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::VladsTower(3), None, &mut rng)
            .expect("Vlad tower top should generate");
        let pop = population_for_special_level(SpecialLevelId::VladsTower(3), &sl.generated);
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("Vlad the Impaler")),
            "Vlad tower top population should include Vlad"
        );
        assert!(
            pop.objects
                .iter()
                .any(|o| o.name.eq_ignore_ascii_case("Candelabrum of Invocation")),
            "Vlad tower top population should include the Candelabrum"
        );
    }

    #[test]
    fn test_population_wizard_tower3_contains_wizard() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::WizardTower3, None, &mut rng)
            .expect("wizard tower 3 should generate");
        let pop = population_for_special_level(SpecialLevelId::WizardTower3, &sl.generated);
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("Wizard of Yendor")),
            "wizard tower 3 population should include the Wizard of Yendor"
        );
    }

    #[test]
    fn test_population_sanctum_contains_high_priest() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::Sanctum, None, &mut rng)
            .expect("sanctum should generate");
        let pop = population_for_special_level(SpecialLevelId::Sanctum, &sl.generated);
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("high priest")),
            "sanctum population should include the high priest"
        );
    }

    #[test]
    fn test_population_fakewiz2_contains_amulet() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::FakeWizard(2), None, &mut rng)
            .expect("fakewiz2 should generate");
        let pop = population_for_special_level(SpecialLevelId::FakeWizard(2), &sl.generated);
        assert!(
            pop.objects
                .iter()
                .any(|o| o.name.eq_ignore_ascii_case("Amulet of Yendor")),
            "fakewiz2 population should include the Amulet of Yendor"
        );
    }

    #[test]
    fn test_population_fort_ludios_has_garrison() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::FortLudios, None, &mut rng)
            .expect("Fort Ludios should generate");
        let pop = population_for_special_level(SpecialLevelId::FortLudios, &sl.generated);
        assert_eq!(
            pop.monsters
                .iter()
                .filter(|m| m.name.eq_ignore_ascii_case("soldier"))
                .count(),
            2,
            "Fort Ludios population should include two soldiers"
        );
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("lieutenant")),
            "Fort Ludios population should include its lieutenant"
        );
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("captain")),
            "Fort Ludios population should include its captain"
        );
    }

    #[test]
    fn test_population_quest_start_role_specific_leader() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::QuestStart, Some("wizard"), &mut rng)
            .expect("wizard quest start should generate");
        let pop = population_for_special_level_with_role(
            SpecialLevelId::QuestStart,
            &sl.generated,
            Some("wizard"),
        );
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("Neferet the Green")),
            "wizard quest start population should include Neferet the Green"
        );
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("apprentice")),
            "wizard quest start population should include apprentice guardians"
        );
    }

    #[test]
    fn test_population_quest_goal_role_specific_nemesis_and_artifact() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::QuestGoal, Some("wizard"), &mut rng)
            .expect("wizard quest goal should generate");
        let pop = population_for_special_level_with_role(
            SpecialLevelId::QuestGoal,
            &sl.generated,
            Some("wizard"),
        );
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("Dark One")),
            "wizard quest goal population should include the Dark One"
        );
        assert!(
            pop.objects
                .iter()
                .any(|o| o.name == "The Eye of the Aethiopica"),
            "wizard quest goal population should include the Eye artifact"
        );
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("vampire bat")),
            "wizard quest goal population should include wizard quest enemies"
        );
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("xorn")),
            "wizard quest goal population should include the secondary wizard quest enemy"
        );
    }

    #[test]
    fn test_population_quest_locator_role_specific_enemies() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::QuestLocator, Some("wizard"), &mut rng)
            .expect("wizard quest locator should generate");
        let pop = population_for_special_level_with_role(
            SpecialLevelId::QuestLocator,
            &sl.generated,
            Some("wizard"),
        );
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("vampire bat")),
            "wizard quest locator should include primary wizard quest enemies"
        );
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("xorn")),
            "wizard quest locator should include secondary wizard quest enemies"
        );
    }

    #[test]
    fn test_population_quest_filler_role_specific_enemies() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::QuestFiller(3), Some("wizard"), &mut rng)
            .expect("wizard quest filler should generate");
        let pop = population_for_special_level_with_role(
            SpecialLevelId::QuestFiller(3),
            &sl.generated,
            Some("wizard"),
        );
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("vampire bat")),
            "wizard quest filler should include primary wizard quest enemies"
        );
        assert!(
            pop.monsters
                .iter()
                .any(|m| m.name.eq_ignore_ascii_case("xorn")),
            "wizard quest filler should include secondary wizard quest enemies"
        );
    }

    #[test]
    fn test_population_quest_roles_cover_all_leaders_guardians_nemeses_enemies_and_artifacts() {
        for (idx, role) in Role::ALL.into_iter().enumerate() {
            let role_name = role.name().to_ascii_lowercase();
            let mut start_rng = Pcg64::seed_from_u64(7000 + idx as u64);
            let mut goal_rng = Pcg64::seed_from_u64(8000 + idx as u64);

            let start = dispatch_special_level(
                SpecialLevelId::QuestStart,
                Some(role_name.as_str()),
                &mut start_rng,
            )
            .unwrap_or_else(|| panic!("{} quest start should generate", role.name()));
            let start_pop = population_for_special_level_with_role(
                SpecialLevelId::QuestStart,
                &start.generated,
                Some(role_name.as_str()),
            );
            assert!(
                start_pop
                    .monsters
                    .iter()
                    .any(|m| m.name == quest_leader_for_role(role)),
                "{} quest start should include leader {}",
                role.name(),
                quest_leader_for_role(role)
            );
            assert!(
                start_pop
                    .monsters
                    .iter()
                    .any(|m| m.name == quest_guardian_for_role(role)),
                "{} quest start should include guardian {}",
                role.name(),
                quest_guardian_for_role(role)
            );

            let goal = dispatch_special_level(
                SpecialLevelId::QuestGoal,
                Some(role_name.as_str()),
                &mut goal_rng,
            )
            .unwrap_or_else(|| panic!("{} quest goal should generate", role.name()));
            let goal_pop = population_for_special_level_with_role(
                SpecialLevelId::QuestGoal,
                &goal.generated,
                Some(role_name.as_str()),
            );
            assert!(
                goal_pop
                    .monsters
                    .iter()
                    .any(|m| m.name == quest_nemesis_for_role(role)),
                "{} quest goal should include nemesis {}",
                role.name(),
                quest_nemesis_for_role(role)
            );
            let enemies = quest_enemies_for_role(role);
            assert!(
                goal_pop.monsters.iter().any(|m| m.name == enemies.enemy1),
                "{} quest goal should include primary enemy {}",
                role.name(),
                enemies.enemy1
            );
            assert!(
                goal_pop.monsters.iter().any(|m| m.name == enemies.enemy2),
                "{} quest goal should include secondary enemy {}",
                role.name(),
                enemies.enemy2
            );
            assert!(
                goal_pop
                    .objects
                    .iter()
                    .any(|o| o.name == quest_artifact_for_role(role)),
                "{} quest goal should include artifact {}",
                role.name(),
                quest_artifact_for_role(role)
            );
        }
    }

    #[test]
    fn test_population_quest_roles_cover_locator_and_filler_enemies() {
        for (idx, role) in Role::ALL.into_iter().enumerate() {
            let role_name = role.name().to_ascii_lowercase();
            let mut locator_rng = Pcg64::seed_from_u64(8100 + idx as u64);
            let mut filler_rng = Pcg64::seed_from_u64(8200 + idx as u64);
            let enemies = quest_enemies_for_role(role);

            let locator = dispatch_special_level(
                SpecialLevelId::QuestLocator,
                Some(role_name.as_str()),
                &mut locator_rng,
            )
            .unwrap_or_else(|| panic!("{} quest locator should generate", role.name()));
            let locator_pop = population_for_special_level_with_role(
                SpecialLevelId::QuestLocator,
                &locator.generated,
                Some(role_name.as_str()),
            );
            assert!(
                locator_pop
                    .monsters
                    .iter()
                    .any(|m| m.name == enemies.enemy1),
                "{} quest locator should include primary enemy {}",
                role.name(),
                enemies.enemy1
            );
            assert!(
                locator_pop
                    .monsters
                    .iter()
                    .any(|m| m.name == enemies.enemy2),
                "{} quest locator should include secondary enemy {}",
                role.name(),
                enemies.enemy2
            );

            let filler = dispatch_special_level(
                SpecialLevelId::QuestFiller(3),
                Some(role_name.as_str()),
                &mut filler_rng,
            )
            .unwrap_or_else(|| panic!("{} quest filler should generate", role.name()));
            let filler_pop = population_for_special_level_with_role(
                SpecialLevelId::QuestFiller(3),
                &filler.generated,
                Some(role_name.as_str()),
            );
            assert!(
                filler_pop.monsters.iter().any(|m| m.name == enemies.enemy1),
                "{} quest filler should include primary enemy {}",
                role.name(),
                enemies.enemy1
            );
            assert!(
                filler_pop.monsters.iter().any(|m| m.name == enemies.enemy2),
                "{} quest filler should include secondary enemy {}",
                role.name(),
                enemies.enemy2
            );
        }
    }

    #[test]
    fn test_special_level_population_census_key_levels_match_contracts() {
        struct CensusCase {
            id: SpecialLevelId,
            role: Option<&'static str>,
            expected_monsters: &'static [&'static str],
            expected_objects: &'static [&'static str],
            no_dig: bool,
            no_teleport: bool,
            no_prayer: bool,
        }

        let cases = [
            CensusCase {
                id: SpecialLevelId::OracleLevel,
                role: None,
                expected_monsters: &["Oracle"],
                expected_objects: &[],
                no_dig: false,
                no_teleport: false,
                no_prayer: false,
            },
            CensusCase {
                id: SpecialLevelId::Medusa(0),
                role: None,
                expected_monsters: &["Medusa"],
                expected_objects: &[],
                no_dig: false,
                no_teleport: false,
                no_prayer: false,
            },
            CensusCase {
                id: SpecialLevelId::Castle,
                role: None,
                expected_monsters: &[],
                expected_objects: &["wand of wishing"],
                no_dig: false,
                no_teleport: false,
                no_prayer: false,
            },
            CensusCase {
                id: SpecialLevelId::FortLudios,
                role: None,
                expected_monsters: &["soldier", "lieutenant", "captain"],
                expected_objects: &[],
                no_dig: false,
                no_teleport: false,
                no_prayer: false,
            },
            CensusCase {
                id: SpecialLevelId::Valley,
                role: None,
                expected_monsters: &["ghost"],
                expected_objects: &[],
                no_dig: false,
                no_teleport: true,
                no_prayer: true,
            },
            CensusCase {
                id: SpecialLevelId::Asmodeus,
                role: None,
                expected_monsters: &["Asmodeus"],
                expected_objects: &[],
                no_dig: false,
                no_teleport: true,
                no_prayer: true,
            },
            CensusCase {
                id: SpecialLevelId::Baalzebub,
                role: None,
                expected_monsters: &["Baalzebub"],
                expected_objects: &[],
                no_dig: false,
                no_teleport: true,
                no_prayer: true,
            },
            CensusCase {
                id: SpecialLevelId::Juiblex,
                role: None,
                expected_monsters: &["Juiblex"],
                expected_objects: &[],
                no_dig: false,
                no_teleport: true,
                no_prayer: true,
            },
            CensusCase {
                id: SpecialLevelId::Orcus,
                role: None,
                expected_monsters: &["Orcus"],
                expected_objects: &["wand of death"],
                no_dig: false,
                no_teleport: true,
                no_prayer: true,
            },
            CensusCase {
                id: SpecialLevelId::FakeWizard(2),
                role: None,
                expected_monsters: &["lich", "vampire lord", "kraken"],
                expected_objects: &["Amulet of Yendor"],
                no_dig: true,
                no_teleport: true,
                no_prayer: true,
            },
            CensusCase {
                id: SpecialLevelId::WizardTower3,
                role: None,
                expected_monsters: &["Wizard of Yendor"],
                expected_objects: &[],
                no_dig: true,
                no_teleport: true,
                no_prayer: true,
            },
            CensusCase {
                id: SpecialLevelId::Sanctum,
                role: None,
                expected_monsters: &["high priest"],
                expected_objects: &[],
                no_dig: true,
                no_teleport: true,
                no_prayer: true,
            },
            CensusCase {
                id: SpecialLevelId::QuestStart,
                role: Some("wizard"),
                expected_monsters: &["Neferet the Green", "apprentice"],
                expected_objects: &[],
                no_dig: false,
                no_teleport: false,
                no_prayer: false,
            },
            CensusCase {
                id: SpecialLevelId::QuestLocator,
                role: Some("wizard"),
                expected_monsters: &["vampire bat", "xorn"],
                expected_objects: &[],
                no_dig: false,
                no_teleport: false,
                no_prayer: false,
            },
            CensusCase {
                id: SpecialLevelId::QuestGoal,
                role: Some("wizard"),
                expected_monsters: &["Dark One", "vampire bat", "xorn"],
                expected_objects: &["The Eye of the Aethiopica"],
                no_dig: false,
                no_teleport: true,
                no_prayer: false,
            },
            CensusCase {
                id: SpecialLevelId::QuestFiller(3),
                role: Some("wizard"),
                expected_monsters: &["vampire bat", "xorn"],
                expected_objects: &[],
                no_dig: false,
                no_teleport: false,
                no_prayer: false,
            },
        ];

        for (idx, case) in cases.into_iter().enumerate() {
            let mut rng = Pcg64::seed_from_u64(90000 + idx as u64);
            let sl = dispatch_special_level(case.id, case.role, &mut rng)
                .unwrap_or_else(|| panic!("{:?} should generate", case.id));
            assert_eq!(
                sl.flags.no_dig, case.no_dig,
                "{:?} no_dig mismatch",
                case.id
            );
            assert_eq!(
                sl.flags.no_teleport, case.no_teleport,
                "{:?} no_teleport mismatch",
                case.id
            );
            assert_eq!(
                sl.flags.no_prayer, case.no_prayer,
                "{:?} no_prayer mismatch",
                case.id
            );

            let pop = population_for_special_level_with_role(case.id, &sl.generated, case.role);
            for expected in case.expected_monsters {
                assert!(
                    pop.monsters
                        .iter()
                        .any(|m| m.name.eq_ignore_ascii_case(expected)),
                    "{:?} population should include monster {}",
                    case.id,
                    expected
                );
            }
            for expected in case.expected_objects {
                assert!(
                    pop.objects
                        .iter()
                        .any(|o| o.name.eq_ignore_ascii_case(expected)),
                    "{:?} population should include object {}",
                    case.id,
                    expected
                );
            }
        }
    }

    #[test]
    fn test_embedded_population_positions_land_on_walkable_tiles() {
        let mut rng = test_rng();
        let ids = [
            SpecialLevelId::Valley,
            SpecialLevelId::Asmodeus,
            SpecialLevelId::Baalzebub,
            SpecialLevelId::Juiblex,
            SpecialLevelId::Orcus,
            SpecialLevelId::FakeWizard(1),
            SpecialLevelId::FakeWizard(2),
        ];

        for id in ids {
            let sl = dispatch_special_level(id, None, &mut rng)
                .unwrap_or_else(|| panic!("{:?} should generate", id));
            let pop = population_for_special_level(id, &sl.generated);
            let world = make_population_test_world(sl.generated.map.clone());

            for mon in &pop.monsters {
                if let Some(pos) = mon.pos {
                    let monster_def = resolve_monster_def_for_test(&mon.name)
                        .unwrap_or_else(|| panic!("{} should resolve for {:?}", mon.name, id));
                    assert!(
                        goodpos(&world, pos, Some(monster_def), GoodPosFlags::AVOID_MONSTER),
                        "{:?} monster {:?} should target a valid spawn tile, got {:?}",
                        id,
                        mon.name,
                        sl.generated.map.get(pos).map(|cell| cell.terrain)
                    );
                }
            }

            for obj in &pop.objects {
                if let Some(pos) = obj.pos {
                    assert!(
                        sl.generated
                            .map
                            .get(pos)
                            .is_some_and(|cell| cell.terrain.is_walkable()),
                        "{:?} object {:?} should target a walkable tile, got {:?}",
                        id,
                        obj.name,
                        sl.generated.map.get(pos).map(|cell| cell.terrain)
                    );
                }
            }
        }
    }

    #[test]
    fn test_population_gehennom_toml_contains_bosses() {
        let mut rng = test_rng();
        let ids_and_names = [
            (SpecialLevelId::Asmodeus, "Asmodeus"),
            (SpecialLevelId::Baalzebub, "Baalzebub"),
            (SpecialLevelId::Juiblex, "Juiblex"),
            (SpecialLevelId::Orcus, "Orcus"),
        ];

        for (id, expected_name) in ids_and_names {
            let sl = dispatch_special_level(id, None, &mut rng)
                .unwrap_or_else(|| panic!("{:?} should generate", id));
            let pop = population_for_special_level(id, &sl.generated);
            assert!(
                pop.monsters
                    .iter()
                    .any(|m| m.name.eq_ignore_ascii_case(expected_name)),
                "{:?} population should include {}",
                id,
                expected_name
            );
        }
    }

    #[test]
    fn test_dispatch_fakewiz_uses_embedded_toml_layout() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::FakeWizard(1), None, &mut rng)
            .expect("fakewiz1 should generate");
        let pool_count = count_terrain(&sl.generated.map, Terrain::Pool);
        assert!(
            pool_count > 0,
            "fakewiz1 dispatch should use embedded TOML water layout, got {} pool tiles",
            pool_count
        );
        assert!(sl.flags.no_dig, "fakewiz1 dispatch should preserve no_dig");
        assert!(
            sl.flags.no_prayer,
            "fakewiz1 dispatch should preserve no_prayer"
        );
    }

    #[test]
    fn test_dispatch_valley_uses_embedded_toml_layout() {
        let mut rng = test_rng();
        let sl = dispatch_special_level(SpecialLevelId::Valley, None, &mut rng)
            .expect("valley should generate");

        let altar_count = count_terrain(&sl.generated.map, Terrain::Altar);
        let grave_count = count_terrain(&sl.generated.map, Terrain::Grave);
        assert!(
            altar_count >= 1,
            "valley dispatch should preserve the altar"
        );
        assert!(
            grave_count >= 4,
            "valley dispatch should preserve grave terrain"
        );
        assert!(
            sl.flags.no_teleport,
            "valley dispatch should preserve noteleport"
        );
        assert!(
            sl.flags.no_prayer,
            "valley dispatch should preserve no_prayer"
        );
    }

    #[test]
    fn test_gehennom_generators_produce_valid_levels() {
        let mut rng = test_rng();
        let ids: Vec<(SpecialLevelId, &str)> = vec![
            (SpecialLevelId::Valley, "Valley"),
            (SpecialLevelId::Asmodeus, "Asmodeus"),
            (SpecialLevelId::Baalzebub, "Baalzebub"),
            (SpecialLevelId::Juiblex, "Juiblex"),
            (SpecialLevelId::Orcus, "Orcus"),
            (SpecialLevelId::FakeWizard(1), "FakeWizard(1)"),
            (SpecialLevelId::WizardTower2, "WizardTower2"),
            (SpecialLevelId::WizardTower3, "WizardTower3"),
        ];
        for (id, name) in &ids {
            let result = dispatch_special_level(*id, None, &mut rng);
            assert!(
                result.is_some(),
                "Generator for {} should produce a level",
                name
            );
            let level = result.unwrap();
            assert!(
                level.generated.up_stairs.is_some() || level.generated.down_stairs.is_some(),
                "{} should have at least one staircase",
                name
            );
        }
    }

    // ── Valley tests ────────────────────────────────────────────────

    #[test]
    fn test_valley_has_altar() {
        let mut rng = test_rng();
        let sl = generate_valley(&mut rng);
        let altar_count = count_terrain(&sl.generated.map, Terrain::Altar);
        assert!(
            altar_count >= 1,
            "Valley should have an altar, got {}",
            altar_count
        );
    }

    #[test]
    fn test_valley_has_graves() {
        let mut rng = test_rng();
        let sl = generate_valley(&mut rng);
        let grave_count = count_terrain(&sl.generated.map, Terrain::Grave);
        assert!(
            grave_count >= 4,
            "Valley should have graves, got {}",
            grave_count
        );
    }

    #[test]
    fn test_valley_flags() {
        let mut rng = test_rng();
        let sl = generate_valley(&mut rng);
        assert!(sl.flags.no_teleport, "Valley should be no-teleport");
        assert!(sl.flags.no_prayer, "Valley should be no-prayer");
    }

    // ── Orcus tests ─────────────────────────────────────────────────

    #[test]
    fn test_orcus_has_graves() {
        let mut rng = test_rng();
        let sl = generate_orcus(&mut rng);
        let grave_count = count_terrain(&sl.generated.map, Terrain::Grave);
        assert!(
            grave_count >= 4,
            "Orcus should have graves, got {}",
            grave_count
        );
    }

    // ── Juiblex tests ───────────────────────────────────────────────

    #[test]
    fn test_juiblex_has_water() {
        let mut rng = test_rng();
        let sl = generate_juiblex(&mut rng);
        let water_count = count_terrain(&sl.generated.map, Terrain::Water);
        assert!(
            water_count >= 10,
            "Juiblex should have water, got {}",
            water_count
        );
    }

    // ── Gehennom generator stability tests ───────────────────────────

    #[test]
    fn test_gehennom_generators_multiple_seeds() {
        for seed in 0..5u64 {
            let mut rng = Pcg64::seed_from_u64(seed + 1000);
            for id in [
                SpecialLevelId::Valley,
                SpecialLevelId::Asmodeus,
                SpecialLevelId::Baalzebub,
                SpecialLevelId::Juiblex,
                SpecialLevelId::Orcus,
                SpecialLevelId::FakeWizard(1),
                SpecialLevelId::FakeWizard(2),
                SpecialLevelId::WizardTower2,
                SpecialLevelId::WizardTower3,
            ] {
                let result = dispatch_special_level(id, None, &mut rng);
                assert!(result.is_some(), "seed {}: {:?} should generate", seed, id);
                let level = result.unwrap();
                let floor = count_terrain(&level.generated.map, Terrain::Floor);
                assert!(
                    floor > 10,
                    "seed {}: {:?} should have floor tiles",
                    seed,
                    id
                );
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // TOML-to-level bridge tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_build_level_from_toml_basic() {
        use nethack_babel_data::level_loader::load_level_from_str;
        let toml_str = r#"
[level]
name = "test_basic"

[map]
halign = "center"
valign = "center"
data = """
------
|....|
|....|
------
"""

[[stairs]]
direction = "up"
x = 2
y = 1

[[stairs]]
direction = "down"
x = 4
y = 2
"#;
        let def = load_level_from_str(toml_str).unwrap();
        let mut rng = test_rng();
        let sl = build_level_from_toml(&def, &mut rng);

        // Should have stairs.
        assert!(sl.generated.up_stairs.is_some());
        assert!(sl.generated.down_stairs.is_some());

        // Should have at least one room.
        assert!(!sl.generated.rooms.is_empty());
    }

    #[test]
    fn test_build_level_from_toml_with_stairs() {
        use nethack_babel_data::level_loader::load_level_from_str;
        let toml_str = r#"
[level]
name = "test_stairs"

[map]
halign = "center"
valign = "center"
data = """
------
|.<.>|
------
"""
"#;
        let def = load_level_from_str(toml_str).unwrap();
        let mut rng = test_rng();
        let sl = build_level_from_toml(&def, &mut rng);

        assert!(sl.generated.up_stairs.is_some());
        assert!(sl.generated.down_stairs.is_some());
    }

    #[test]
    fn test_build_level_from_toml_flags() {
        use nethack_babel_data::level_loader::load_level_from_str;
        let toml_str = r#"
[level]
name = "test_flags"
flags = ["noteleport", "nodig"]
"#;
        let def = load_level_from_str(toml_str).unwrap();
        let mut rng = test_rng();
        let sl = build_level_from_toml(&def, &mut rng);

        assert!(sl.flags.no_teleport);
        assert!(sl.flags.no_dig);
        assert!(!sl.flags.no_prayer);
    }

    #[test]
    fn test_build_level_from_toml_no_map() {
        use nethack_babel_data::level_loader::load_level_from_str;
        let toml_str = r#"
[level]
name = "empty"
"#;
        let def = load_level_from_str(toml_str).unwrap();
        let mut rng = test_rng();
        let sl = build_level_from_toml(&def, &mut rng);

        // Should still have default stairs.
        assert!(sl.generated.up_stairs.is_some());
        assert!(sl.generated.down_stairs.is_some());
    }

    #[test]
    fn test_build_level_from_toml_terrain_types() {
        use nethack_babel_data::level_loader::load_level_from_str;
        let toml_str = r#"
[level]
name = "test_terrain"

[map]
halign = "center"
valign = "center"
data = """
----
|G{|
----
"""
"#;
        let def = load_level_from_str(toml_str).unwrap();
        let mut rng = test_rng();
        let sl = build_level_from_toml(&def, &mut rng);

        let fountain_count = count_terrain(&sl.generated.map, Terrain::Fountain);
        let grave_count = count_terrain(&sl.generated.map, Terrain::Grave);
        assert!(fountain_count >= 1, "Should have a fountain");
        assert!(grave_count >= 1, "Should have a grave");
    }

    #[test]
    fn test_build_level_from_toml_embedded_valley() {
        use nethack_babel_data::level_loader::{get_embedded_level, load_level_from_str};
        let toml_str = get_embedded_level("valley").unwrap();
        let def = load_level_from_str(toml_str).unwrap();
        let mut rng = test_rng();
        let sl = build_level_from_toml(&def, &mut rng);

        assert!(sl.flags.no_teleport, "Valley TOML should set noteleport");
        assert!(sl.flags.no_prayer, "Valley TOML should set no_prayer");
        let floor = count_terrain(&sl.generated.map, Terrain::Floor);
        let altar_count = count_terrain(&sl.generated.map, Terrain::Altar);
        let grave_count = count_terrain(&sl.generated.map, Terrain::Grave);
        assert!(
            floor > 50,
            "Valley from TOML should have many floor tiles, got {}",
            floor
        );
        assert!(altar_count >= 1, "Valley from TOML should include an altar");
        assert!(grave_count >= 4, "Valley from TOML should include graves");
        let map_def = def.map.as_ref().expect("Valley TOML should include a map");
        let (offset_x, offset_y) =
            aligned_map_offsets(map_def, LevelMap::DEFAULT_WIDTH, LevelMap::DEFAULT_HEIGHT);
        assert_eq!(
            sl.generated.up_stairs,
            Some(Position::new(offset_x, offset_y)),
            "Valley up stairs should honor map alignment plus 1-based local coordinates"
        );
    }

    // ═══════════════════════════════════════════════════════════════════
    // Big Room tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_big_room_variant_0_open() {
        let mut rng = test_rng();
        let sl = generate_big_room(0, &mut rng);
        assert!(sl.generated.up_stairs.is_some());
        assert!(sl.generated.down_stairs.is_some());
        let floor = count_terrain(&sl.generated.map, Terrain::Floor);
        assert!(
            floor > 200,
            "Big room variant 0 should have lots of floor, got {}",
            floor
        );
    }

    #[test]
    fn test_big_room_variant_1_pillars() {
        let mut rng = test_rng();
        let sl = generate_big_room(1, &mut rng);
        assert!(sl.generated.up_stairs.is_some());
        // Pillar variant has some walls inside the room.
        let floor = count_terrain(&sl.generated.map, Terrain::Floor);
        assert!(
            floor > 100,
            "Pillar variant should still have plenty of floor"
        );
    }

    #[test]
    fn test_big_room_variant_2_pool() {
        let mut rng = test_rng();
        let sl = generate_big_room(2, &mut rng);
        let pool = count_terrain(&sl.generated.map, Terrain::Pool);
        assert!(
            pool > 0,
            "Pool variant should have pool tiles, got {}",
            pool
        );
    }

    #[test]
    fn test_big_room_variant_3_cross() {
        let mut rng = test_rng();
        let sl = generate_big_room(3, &mut rng);
        assert!(sl.generated.up_stairs.is_some());
        assert!(sl.generated.down_stairs.is_some());
    }

    #[test]
    fn test_big_room_all_variants_valid() {
        for v in 0..4u8 {
            let mut rng = Pcg64::seed_from_u64(v as u64 + 2000);
            let sl = generate_big_room(v, &mut rng);
            assert!(
                sl.generated.up_stairs.is_some() && sl.generated.down_stairs.is_some(),
                "BigRoom variant {} should have stairs",
                v
            );
            assert!(
                !sl.generated.rooms.is_empty(),
                "BigRoom variant {} should have a room",
                v
            );
        }
    }

    #[test]
    fn test_big_room_multiple_seeds() {
        for seed in 0..5u64 {
            for v in 0..4u8 {
                let mut rng = Pcg64::seed_from_u64(seed + 3000);
                let sl = generate_big_room(v, &mut rng);
                let floor = count_terrain(&sl.generated.map, Terrain::Floor);
                assert!(
                    floor > 50,
                    "seed {}, variant {}: Big room should have floor tiles",
                    seed,
                    v
                );
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Rogue level tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_rogue_level_has_rooms() {
        let mut rng = test_rng();
        let sl = generate_rogue_level(15, &mut rng);
        assert_eq!(
            sl.generated.rooms.len(),
            9,
            "Rogue level should have 9 rooms (3x3 grid)"
        );
    }

    #[test]
    fn test_rogue_level_has_stairs() {
        let mut rng = test_rng();
        let sl = generate_rogue_level(15, &mut rng);
        assert!(sl.generated.up_stairs.is_some());
        assert!(sl.generated.down_stairs.is_some());
    }

    #[test]
    fn test_rogue_level_has_corridors() {
        let mut rng = test_rng();
        let sl = generate_rogue_level(15, &mut rng);
        let corridor = count_terrain(&sl.generated.map, Terrain::Corridor);
        assert!(
            corridor > 0,
            "Rogue level should have corridor tiles, got {}",
            corridor
        );
    }

    #[test]
    fn test_rogue_level_multiple_seeds() {
        for seed in 0..10u64 {
            let mut rng = Pcg64::seed_from_u64(seed + 4000);
            let sl = generate_rogue_level(15, &mut rng);
            assert_eq!(sl.generated.rooms.len(), 9);
            let floor = count_terrain(&sl.generated.map, Terrain::Floor);
            assert!(
                floor > 20,
                "seed {}: Rogue should have floor, got {}",
                seed,
                floor
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Mines End tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_mines_end_variant_0() {
        let mut rng = test_rng();
        let sl = generate_mines_end(0, &mut rng);
        assert!(sl.generated.up_stairs.is_some());
        assert!(
            sl.generated.down_stairs.is_none(),
            "Mines End has no down stairs"
        );
        let floor = count_terrain(&sl.generated.map, Terrain::Floor);
        assert!(floor > 100, "Mines End should have floor, got {}", floor);
    }

    #[test]
    fn test_mines_end_variant_1_fountains() {
        let mut rng = test_rng();
        let sl = generate_mines_end(1, &mut rng);
        let fountain = count_terrain(&sl.generated.map, Terrain::Fountain);
        assert!(
            fountain > 0,
            "Mines End variant 1 should have fountains, got {}",
            fountain
        );
    }

    #[test]
    fn test_mines_end_variant_2_pool() {
        let mut rng = test_rng();
        let sl = generate_mines_end(2, &mut rng);
        let pool = count_terrain(&sl.generated.map, Terrain::Pool);
        assert!(
            pool > 0,
            "Mines End variant 2 should have pool, got {}",
            pool
        );
    }

    #[test]
    fn test_mines_end_all_variants_valid() {
        for v in 0..3u8 {
            let mut rng = Pcg64::seed_from_u64(v as u64 + 5000);
            let sl = generate_mines_end(v, &mut rng);
            assert!(
                sl.generated.up_stairs.is_some(),
                "MinesEnd variant {} should have up stairs",
                v
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Quest level tests
    // ═══════════════════════════════════════════════════════════════════

    const ALL_ROLES: [&str; 13] = [
        "valkyrie",
        "wizard",
        "archeologist",
        "barbarian",
        "caveman",
        "healer",
        "knight",
        "monk",
        "priest",
        "ranger",
        "rogue",
        "samurai",
        "tourist",
    ];

    #[test]
    fn test_quest_start_produces_valid_level() {
        let mut rng = test_rng();
        let sl = generate_quest_start("valkyrie", &mut rng);
        assert!(sl.generated.up_stairs.is_some());
        assert!(sl.generated.down_stairs.is_some());
        let floor = count_terrain(&sl.generated.map, Terrain::Floor);
        assert!(floor > 100, "Quest start should have floor, got {}", floor);
    }

    #[test]
    fn test_quest_start_has_fountain() {
        let mut rng = test_rng();
        let sl = generate_quest_start("valkyrie", &mut rng);
        let fountain = count_terrain(&sl.generated.map, Terrain::Fountain);
        assert!(fountain >= 1, "Quest start should have a fountain");
    }

    #[test]
    fn test_quest_start_all_13_roles() {
        for role in ALL_ROLES {
            let mut rng = Pcg64::seed_from_u64(42);
            let sl = generate_quest_start(role, &mut rng);
            assert!(
                sl.generated.up_stairs.is_some(),
                "Quest start for {} missing up stairs",
                role
            );
            assert!(
                sl.generated.down_stairs.is_some(),
                "Quest start for {} missing down stairs",
                role
            );
            let floor = count_terrain(&sl.generated.map, Terrain::Floor);
            assert!(
                floor > 20,
                "Quest start for {} should have floor, got {}",
                role,
                floor
            );
        }
    }

    #[test]
    fn test_quest_locator_produces_valid_level() {
        let mut rng = test_rng();
        let sl = generate_quest_locator("valkyrie", &mut rng);
        assert!(sl.generated.up_stairs.is_some());
        assert!(sl.generated.down_stairs.is_some());
        assert!(
            sl.generated.rooms.len() >= 2,
            "Quest locator should have at least 2 rooms"
        );
    }

    #[test]
    fn test_quest_locator_all_13_roles() {
        for role in ALL_ROLES {
            let mut rng = Pcg64::seed_from_u64(42);
            let sl = generate_quest_locator(role, &mut rng);
            assert!(
                sl.generated.up_stairs.is_some(),
                "Quest locator for {} missing up stairs",
                role
            );
            assert!(
                sl.generated.down_stairs.is_some(),
                "Quest locator for {} missing down stairs",
                role
            );
            assert!(
                sl.generated.rooms.len() >= 2,
                "Quest locator for {} should have >= 2 rooms",
                role
            );
        }
    }

    #[test]
    fn test_quest_goal_produces_valid_level() {
        let mut rng = test_rng();
        let sl = generate_quest_goal("valkyrie", &mut rng);
        assert!(sl.generated.up_stairs.is_some());
        assert!(
            sl.generated.down_stairs.is_none(),
            "Quest goal has no down stairs"
        );
        assert!(sl.flags.no_teleport, "Quest goal should be no-teleport");
    }

    #[test]
    fn test_quest_goal_has_altar() {
        let mut rng = test_rng();
        let sl = generate_quest_goal("valkyrie", &mut rng);
        let altar = count_terrain(&sl.generated.map, Terrain::Altar);
        assert!(altar >= 1, "Quest goal should have an altar");
    }

    #[test]
    fn test_quest_goal_all_13_roles() {
        for role in ALL_ROLES {
            let mut rng = Pcg64::seed_from_u64(42);
            let sl = generate_quest_goal(role, &mut rng);
            assert!(
                sl.generated.up_stairs.is_some(),
                "Quest goal for {} missing up stairs",
                role
            );
            assert!(
                sl.generated.down_stairs.is_none(),
                "Quest goal for {} should have no down stairs",
                role
            );
            assert!(
                sl.flags.no_teleport,
                "Quest goal for {} should be no-teleport",
                role
            );
            let altar = count_terrain(&sl.generated.map, Terrain::Altar);
            assert!(
                altar >= 1,
                "Quest goal for {} should have altar, got {}",
                role,
                altar
            );
        }
    }

    #[test]
    fn test_quest_filler_produces_valid_level() {
        let mut rng = test_rng();
        let sl = generate_quest_filler("valkyrie", &mut rng);
        assert!(sl.generated.up_stairs.is_some());
        assert!(sl.generated.down_stairs.is_some());
        let floor = count_terrain(&sl.generated.map, Terrain::Floor);
        assert!(floor > 20, "Quest filler should have floor, got {}", floor);
    }

    #[test]
    fn test_quest_filler_all_13_roles() {
        for role in ALL_ROLES {
            let mut rng = Pcg64::seed_from_u64(42);
            let sl = generate_quest_filler(role, &mut rng);
            assert!(
                sl.generated.up_stairs.is_some(),
                "Quest filler for {} missing up stairs",
                role
            );
            assert!(
                sl.generated.down_stairs.is_some(),
                "Quest filler for {} missing down stairs",
                role
            );
        }
    }

    #[test]
    fn test_quest_starts_are_distinct() {
        let mut rng1 = Pcg64::seed_from_u64(42);
        let mut rng2 = Pcg64::seed_from_u64(42);
        let val = generate_quest_start("valkyrie", &mut rng1);
        let wiz = generate_quest_start("wizard", &mut rng2);
        let mut differences = 0;
        for y in 0..21 {
            for x in 0..80 {
                let p = Position::new(x, y);
                if val.generated.map.get(p).map(|c| c.terrain)
                    != wiz.generated.map.get(p).map(|c| c.terrain)
                {
                    differences += 1;
                }
            }
        }
        assert!(
            differences > 50,
            "Valkyrie and Wizard quest starts should look different, only {} differences",
            differences
        );
    }

    #[test]
    fn test_quest_goals_are_distinct() {
        let mut rng1 = Pcg64::seed_from_u64(42);
        let mut rng2 = Pcg64::seed_from_u64(42);
        let val = generate_quest_goal("valkyrie", &mut rng1);
        let rog = generate_quest_goal("rogue", &mut rng2);
        let mut differences = 0;
        for y in 0..21 {
            for x in 0..80 {
                let p = Position::new(x, y);
                if val.generated.map.get(p).map(|c| c.terrain)
                    != rog.generated.map.get(p).map(|c| c.terrain)
                {
                    differences += 1;
                }
            }
        }
        assert!(
            differences > 20,
            "Valkyrie and Rogue quest goals should look different, only {} differences",
            differences
        );
    }

    #[test]
    fn test_quest_dispatch_passes_role() {
        let mut rng = Pcg64::seed_from_u64(42);
        let result = dispatch_special_level(SpecialLevelId::QuestStart, Some("wizard"), &mut rng);
        assert!(result.is_some());
    }

    #[test]
    fn test_quest_start_valkyrie_has_ice() {
        let mut rng = test_rng();
        let sl = generate_quest_start("valkyrie", &mut rng);
        let ice = count_terrain(&sl.generated.map, Terrain::Ice);
        assert!(ice > 0, "Valkyrie quest start should have ice");
    }

    #[test]
    fn test_quest_start_knight_has_moat() {
        let mut rng = test_rng();
        let sl = generate_quest_start("knight", &mut rng);
        let moat = count_terrain(&sl.generated.map, Terrain::Moat);
        assert!(moat > 0, "Knight quest start should have moat");
    }

    #[test]
    fn test_quest_start_ranger_has_trees() {
        let mut rng = test_rng();
        let sl = generate_quest_start("ranger", &mut rng);
        let trees = count_terrain(&sl.generated.map, Terrain::Tree);
        assert!(trees > 0, "Ranger quest start should have trees");
    }

    #[test]
    fn test_quest_start_priest_has_altars() {
        let mut rng = test_rng();
        let sl = generate_quest_start("priest", &mut rng);
        let altars = count_terrain(&sl.generated.map, Terrain::Altar);
        assert!(
            altars >= 3,
            "Priest quest start should have multiple altars, got {}",
            altars
        );
    }

    #[test]
    fn test_quest_goal_valkyrie_has_lava() {
        let mut rng = test_rng();
        let sl = generate_quest_goal("valkyrie", &mut rng);
        let lava = count_terrain(&sl.generated.map, Terrain::Lava);
        assert!(
            lava > 0,
            "Valkyrie quest goal should have lava (Surtur's lair)"
        );
    }

    #[test]
    fn test_quest_goal_knight_has_moat() {
        let mut rng = test_rng();
        let sl = generate_quest_goal("knight", &mut rng);
        let moat = count_terrain(&sl.generated.map, Terrain::Moat);
        assert!(moat > 0, "Knight quest goal should have moat");
    }

    #[test]
    fn test_quest_goal_rogue_has_locked_door() {
        let mut rng = test_rng();
        let sl = generate_quest_goal("rogue", &mut rng);
        let locked = count_terrain(&sl.generated.map, Terrain::DoorLocked);
        assert!(locked > 0, "Rogue quest goal should have locked vault door");
    }

    #[test]
    fn test_quest_levels_multiple_seeds() {
        for seed in 0..5u64 {
            let mut rng = Pcg64::seed_from_u64(seed + 6000);
            let sl = generate_quest_start("valkyrie", &mut rng);
            assert!(sl.generated.up_stairs.is_some());

            let mut rng = Pcg64::seed_from_u64(seed + 6100);
            let sl = generate_quest_locator("wizard", &mut rng);
            assert!(sl.generated.rooms.len() >= 2);

            let mut rng = Pcg64::seed_from_u64(seed + 6200);
            let sl = generate_quest_goal("ranger", &mut rng);
            assert!(sl.generated.up_stairs.is_some());

            let mut rng = Pcg64::seed_from_u64(seed + 6300);
            let sl = generate_quest_filler("tourist", &mut rng);
            assert!(sl.generated.up_stairs.is_some());
        }
    }

    #[test]
    fn test_quest_generic_fallback() {
        let mut rng = test_rng();
        let sl = generate_quest_start("unknown_role", &mut rng);
        assert!(sl.generated.up_stairs.is_some());
        assert!(sl.generated.down_stairs.is_some());

        let sl = generate_quest_goal("unknown_role", &mut rng);
        assert!(sl.generated.up_stairs.is_some());
        assert!(sl.generated.down_stairs.is_none());
        assert!(sl.flags.no_teleport);
    }

    // ═══════════════════════════════════════════════════════════════════
    // Dispatch completeness test
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_dispatch_no_more_none_returns() {
        let mut rng = test_rng();
        let all_ids = vec![
            SpecialLevelId::OracleLevel,
            SpecialLevelId::Minetown,
            SpecialLevelId::MinesEnd,
            SpecialLevelId::Sokoban(1),
            SpecialLevelId::Castle,
            SpecialLevelId::Medusa(0),
            SpecialLevelId::FortLudios,
            SpecialLevelId::VladsTower(1),
            SpecialLevelId::WizardTower,
            SpecialLevelId::Sanctum,
            SpecialLevelId::EarthPlane,
            SpecialLevelId::AirPlane,
            SpecialLevelId::FirePlane,
            SpecialLevelId::WaterPlane,
            SpecialLevelId::AstralPlane,
            SpecialLevelId::Valley,
            SpecialLevelId::BigRoom(0),
            SpecialLevelId::BigRoom(1),
            SpecialLevelId::BigRoom(2),
            SpecialLevelId::BigRoom(3),
            SpecialLevelId::Rogue,
            SpecialLevelId::Asmodeus,
            SpecialLevelId::Baalzebub,
            SpecialLevelId::Juiblex,
            SpecialLevelId::Orcus,
            SpecialLevelId::FakeWizard(1),
            SpecialLevelId::WizardTower2,
            SpecialLevelId::WizardTower3,
            SpecialLevelId::QuestStart,
            SpecialLevelId::QuestLocator,
            SpecialLevelId::QuestGoal,
            SpecialLevelId::QuestFiller(3),
        ];
        for id in &all_ids {
            let result = dispatch_special_level(*id, None, &mut rng);
            assert!(
                result.is_some(),
                "dispatch_special_level({:?}) should return Some, not None",
                id
            );
        }
    }
}
