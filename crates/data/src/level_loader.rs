//! Loader for TOML-based level definitions and dungeon topology.

use std::path::Path;

use crate::level_schema::*;

/// Load a level definition from a TOML string.
pub fn load_level_from_str(toml_str: &str) -> Result<LevelDefinition, String> {
    toml::from_str(toml_str).map_err(|e| format!("Failed to parse level TOML: {}", e))
}

/// Load a level definition from a file path.
pub fn load_level_from_file(path: &Path) -> Result<LevelDefinition, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read level file {}: {}", path.display(), e))?;
    load_level_from_str(&content)
}

/// Load dungeon topology from a TOML string.
pub fn load_topology_from_str(toml_str: &str) -> Result<DungeonTopology, String> {
    toml::from_str(toml_str).map_err(|e| format!("Failed to parse topology TOML: {}", e))
}

/// Load dungeon topology from a file path.
pub fn load_topology_from_file(path: &Path) -> Result<DungeonTopology, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read topology file {}: {}", path.display(), e))?;
    load_topology_from_str(&content)
}

/// Parse an ASCII map string into a 2D grid of terrain characters.
/// Returns (width, height, grid) where grid[y][x] is the char at that position.
pub fn parse_ascii_map(map_data: &str) -> (usize, usize, Vec<Vec<char>>) {
    let lines: Vec<&str> = map_data.lines().filter(|l| !l.is_empty()).collect();
    let height = lines.len();
    let width = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let grid: Vec<Vec<char>> = lines
        .iter()
        .map(|line| {
            let mut row: Vec<char> = line.chars().collect();
            // Pad to width
            row.resize(width, ' ');
            row
        })
        .collect();
    (width, height, grid)
}

/// Map an ASCII character from a level map to a terrain type name.
pub fn ascii_to_terrain(ch: char) -> &'static str {
    match ch {
        '.' => "floor",
        '#' => "corridor",
        '-' | '|' => "wall",
        '+' => "door_closed",
        'S' => "door_secret",
        '{' => "fountain",
        '}' => "pool",
        '\\' => "throne",
        '_' => "altar",
        '<' => "stairs_up",
        '>' => "stairs_down",
        '^' => "trap",
        'L' => "lava",
        'P' => "pool",
        'T' => "tree",
        'F' => "iron_bars",
        'W' => "water",
        'B' => "drawbridge_raised",
        'C' => "cloud",
        'A' => "air",
        ' ' => "stone",
        _ => "stone",
    }
}

/// Embedded level data for levels that don't need external TOML files.
/// Returns the TOML string for a given special level name.
pub fn get_embedded_level(name: &str) -> Option<&'static str> {
    match name {
        "valley" => Some(VALLEY_TOML),
        "asmodeus" => Some(ASMODEUS_TOML),
        "baalzebub" => Some(BAALZEBUB_TOML),
        "juiblex" => Some(JUIBLEX_TOML),
        "orcus" => Some(ORCUS_TOML),
        "fakewiz1" => Some(FAKEWIZ1_TOML),
        "fakewiz2" => Some(FAKEWIZ2_TOML),
        _ => None,
    }
}

// Embedded TOML definitions for key Gehennom levels
// These are derived from the NetHack C source level definitions

const VALLEY_TOML: &str = r#"
[level]
name = "valley"
branch = "Gehennom"
flags = ["mazelevel", "noteleport", "hardfloor", "nommap", "noautomap"]

[map]
halign = "center"
valign = "center"
data = """
---------------------------------------------------------------------------
|...S.|..|.....|  |.....-|      |................|   |...............| |...|
|.....+..|.....|  |......|      |................|   |...............| |...|
|...S.|..|.....|  |......+######+................+###+...............| |...|
|-----|..|----+|  |------|      |.................S   |...............|.|...|
      |..|    |S  S      |      |................|   |-+-------------|.|...|
      |..+    |.  .|     |      |................|   | |             |.|...|
      |..|    |.  .|     |      |-----+----------|   | |             |.|...|
      |..|    |.  .|     |      |     |              | |             |.|...|
      |..| |---.  .---|  |      |     |           |--| |          |---.---|
      |..| |........  |  |      |     |           |    |          |.......|
      |..| |........  |  |-+--+-|     |           |    |----------|.......|
      |..| |........  |  |......|     |    -------+----+----      |.......|
      |..| |..--------|  |......|     |    |...........|   |      |.......|
      |..| |..|          |......|     |    |...........|   |      |.......|
      |..| |..|  |-------|......|     |----|...........|   |------|.......|
      |..| |..+  +.......+.....+     +....+...........|          |.......|
      |..| |..|  |-------|......|     |----|...........|----------|.......|
      |..| |..|          |......|     |    |...........|                  |
      |..|  ---          |------|     |     -----------                   |
       --                                                                |

"""

[[regions]]
area = [0, 0, 74, 20]
lit = false

[[stairs]]
direction = "up"
x = 1
y = 1

[[stairs]]
direction = "down"
x = 72
y = 18

[[monsters]]
id = "Orcus"
x = 50
y = 14

[[objects]]
id = "wand of death"
x = 50
y = 14
chance = 50
"#;

const ASMODEUS_TOML: &str = r#"
[level]
name = "asmodeus"
branch = "Gehennom"
flags = ["mazelevel", "noteleport", "hardfloor"]

[map]
halign = "center"
valign = "center"
data = """
------      ------
|....|      |....|
|....+######+....|
|....|      |....|
------      ------
  ##          ##
  ##          ##
------      ------
|....|      |....|
|....+######+....|
|....|      |....|
------      ------
"""

[[regions]]
area = [0, 0, 17, 11]
lit = false

[[stairs]]
direction = "up"
x = 2
y = 1

[[stairs]]
direction = "down"
x = 14
y = 10

[[monsters]]
id = "Asmodeus"
x = 14
y = 4
"#;

const BAALZEBUB_TOML: &str = r#"
[level]
name = "baalzebub"
branch = "Gehennom"
flags = ["mazelevel", "noteleport", "hardfloor"]

[map]
halign = "center"
valign = "center"
data = """
---------------------
|...................|
|.-----+----+-----.|
|.|....|....|....|.|
|.|....|....|....|.|
|.|....|....|....|.|
|.------+--------.|
|...................|
---------------------
"""

[[regions]]
area = [0, 0, 20, 8]
lit = false

[[stairs]]
direction = "up"
x = 1
y = 1

[[stairs]]
direction = "down"
x = 19
y = 7

[[monsters]]
id = "Baalzebub"
x = 10
y = 4
"#;

const JUIBLEX_TOML: &str = r#"
[level]
name = "juiblex"
branch = "Gehennom"
flags = ["mazelevel", "noteleport", "hardfloor"]

[map]
halign = "center"
valign = "center"
data = """
-----------
|.........|
|.}}}}}}|.|
|.}}}}}}|.|
|.}}}}}}|.|
|.-------.|
|.........|
|.--------+
|.........|
-----------
"""

[[regions]]
area = [0, 0, 10, 9]
lit = false

[[stairs]]
direction = "up"
x = 1
y = 1

[[stairs]]
direction = "down"
x = 9
y = 8

[[monsters]]
id = "Juiblex"
x = 5
y = 3
"#;

const ORCUS_TOML: &str = r#"
[level]
name = "orcus"
branch = "Gehennom"
flags = ["mazelevel", "noteleport", "hardfloor"]

[map]
halign = "center"
valign = "center"
data = """
--------------------------------------------
|..........|.......|.........|.............|
|..........+.......+.........+.............|
|..........|.......|.........|.............|
|----------+-------+---------+-------------|
|.........................................S|
|.........................................S|
|..........|.......|.........|.............|
|..........+.......+.........+.............|
|..........|.......|.........|.............|
--------------------------------------------
"""

[[regions]]
area = [0, 0, 43, 10]
lit = false

[[stairs]]
direction = "up"
x = 1
y = 1

[[stairs]]
direction = "down"
x = 42
y = 5

[[monsters]]
id = "Orcus"
x = 20
y = 5

[[objects]]
id = "wand of death"
x = 20
y = 5
chance = 75
"#;

const FAKEWIZ1_TOML: &str = r#"
[level]
name = "fakewiz1"
branch = "Gehennom"
flags = ["mazelevel", "noteleport", "hardfloor"]

[map]
halign = "center"
valign = "center"
data = """
-----------
|.........|
|.........|
|.........|
|....F....|
|.........|
|.........|
|.........|
-----------
"""

[[regions]]
area = [0, 0, 10, 8]
lit = false

[[stairs]]
direction = "up"
x = 1
y = 1

[[stairs]]
direction = "down"
x = 9
y = 7
"#;

const FAKEWIZ2_TOML: &str = r#"
[level]
name = "fakewiz2"
branch = "Gehennom"
flags = ["mazelevel", "noteleport", "hardfloor"]

[map]
halign = "center"
valign = "center"
data = """
-----------
|.........|
|.........|
|.........|
|....F....|
|.........|
|.........|
|.........|
-----------
"""

[[regions]]
area = [0, 0, 10, 8]
lit = false

[[stairs]]
direction = "up"
x = 1
y = 1

[[stairs]]
direction = "down"
x = 9
y = 7
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_level_definition() {
        let toml_str = r#"
[level]
name = "test_level"
branch = "Main"
flags = ["mazelevel", "noteleport"]

[map]
halign = "center"
valign = "center"
data = """
------
|....|
------
"""

[[monsters]]
id = "goblin"
x = 2
y = 1

[[objects]]
id = "gold piece"
x = 3
y = 1
chance = 50

[[stairs]]
direction = "up"
x = 1
y = 1

[[stairs]]
direction = "down"
x = 4
y = 1
"#;
        let level: LevelDefinition = toml::from_str(toml_str).unwrap();
        assert_eq!(level.level.name, "test_level");
        assert_eq!(level.level.flags, vec!["mazelevel", "noteleport"]);
        assert!(level.map.is_some());
        assert_eq!(level.monsters.len(), 1);
        assert_eq!(level.objects.len(), 1);
        assert_eq!(level.stairs.len(), 2);
    }

    #[test]
    fn test_parse_topology() {
        let toml_str = r#"
[[branches]]
name = "Main"
max_depth = 29
special_levels = [
    { name = "oracle", depth_range = [5, 9] },
    { name = "castle", depth = 25 },
]

[[connections]]
from_branch = "Main"
to_branch = "Mines"
entrance_range = [3, 5]
connection_type = "stairs"
"#;
        let topo: DungeonTopology = toml::from_str(toml_str).unwrap();
        assert_eq!(topo.branches.len(), 1);
        assert_eq!(topo.branches[0].name, "Main");
        assert_eq!(topo.branches[0].special_levels.len(), 2);
        assert_eq!(topo.connections.len(), 1);
    }

    #[test]
    fn test_parse_ascii_map() {
        let map = "---\n|.|\n---";
        let (w, h, grid) = parse_ascii_map(map);
        assert_eq!(w, 3);
        assert_eq!(h, 3);
        assert_eq!(grid[1][1], '.');
    }

    #[test]
    fn test_ascii_to_terrain() {
        assert_eq!(ascii_to_terrain('.'), "floor");
        assert_eq!(ascii_to_terrain('#'), "corridor");
        assert_eq!(ascii_to_terrain('-'), "wall");
        assert_eq!(ascii_to_terrain('+'), "door_closed");
        assert_eq!(ascii_to_terrain('{'), "fountain");
        assert_eq!(ascii_to_terrain('}'), "pool");
        assert_eq!(ascii_to_terrain('L'), "lava");
        assert_eq!(ascii_to_terrain(' '), "stone");
    }

    #[test]
    fn test_monster_placement_defaults() {
        let toml_str = r#"
[level]
name = "test"

[[monsters]]
id = "goblin"
"#;
        let level: LevelDefinition = toml::from_str(toml_str).unwrap();
        assert_eq!(level.monsters[0].chance, 100);
        assert!(level.monsters[0].x.is_none());
        assert!(level.monsters[0].y.is_none());
    }

    #[test]
    fn test_embedded_levels_parse() {
        for name in [
            "valley",
            "asmodeus",
            "baalzebub",
            "juiblex",
            "orcus",
            "fakewiz1",
            "fakewiz2",
        ] {
            let toml_str = get_embedded_level(name)
                .unwrap_or_else(|| panic!("Missing embedded level: {}", name));
            let level: LevelDefinition = toml::from_str(toml_str)
                .unwrap_or_else(|e| panic!("Failed to parse embedded level {}: {}", name, e));
            assert!(!level.level.name.is_empty(), "Level {} has empty name", name);
            assert!(level.map.is_some(), "Level {} has no map", name);
            assert!(!level.stairs.is_empty(), "Level {} has no stairs", name);
        }
    }

    #[test]
    fn test_roundtrip_level_definition() {
        let level = LevelDefinition {
            level: LevelHeader {
                name: "test".to_string(),
                branch: Some("Main".to_string()),
                flags: vec!["noteleport".to_string()],
                depth: Some(5),
            },
            map: None,
            regions: vec![],
            monsters: vec![MonsterPlacement {
                id: Some("goblin".to_string()),
                class: None,
                x: Some(5),
                y: Some(3),
                chance: 100,
                peaceful: None,
                asleep: None,
                align: None,
            }],
            objects: vec![],
            traps: vec![],
            doors: vec![],
            stairs: vec![StairsPlacement {
                direction: "up".to_string(),
                x: Some(1),
                y: Some(1),
            }],
            altars: vec![],
            engraving: vec![],
            shuffle_groups: vec![],
        };
        let toml_str = toml::to_string_pretty(&level).unwrap();
        let parsed: LevelDefinition = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.level.name, "test");
        assert_eq!(parsed.monsters.len(), 1);
    }

    #[test]
    fn test_load_dungeon_topology_file() {
        let topo_path =
            std::path::Path::new("/Users/hz/Downloads/nethack-babel/data/dungeons/dungeon_topology.toml");
        if topo_path.exists() {
            let topo = load_topology_from_file(topo_path).unwrap();
            assert!(topo.branches.len() >= 8, "Should have at least 8 branches");
            assert!(topo.branches.iter().any(|b| b.name == "Main"));
            let gehen = topo.branches.iter().find(|b| b.name == "Gehennom").unwrap();
            assert!(!gehen.special_levels.is_empty());
        }
    }

    #[test]
    fn test_load_gehennom_level_files() {
        let dir = std::path::Path::new("/Users/hz/Downloads/nethack-babel/data/dungeons/gehennom");
        if dir.exists() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                if entry.path().extension().map_or(false, |e| e == "toml") {
                    let level = load_level_from_file(&entry.path()).unwrap_or_else(|e| {
                        panic!("Failed to load {}: {}", entry.path().display(), e)
                    });
                    assert!(!level.level.name.is_empty());
                }
            }
        }
    }

    #[test]
    fn test_load_sokoban_level_files() {
        let dir = std::path::Path::new("/Users/hz/Downloads/nethack-babel/data/dungeons/sokoban");
        if dir.exists() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                if entry.path().extension().map_or(false, |e| e == "toml") {
                    let level = load_level_from_file(&entry.path()).unwrap_or_else(|e| {
                        panic!("Failed to load {}: {}", entry.path().display(), e)
                    });
                    assert!(!level.level.name.is_empty());
                    assert!(level.map.is_some());
                    assert!(!level.stairs.is_empty());
                }
            }
        }
    }

    #[test]
    fn test_ascii_map_padding() {
        let map = "----\n|.|\n----";
        let (w, h, grid) = parse_ascii_map(map);
        assert_eq!(w, 4);
        assert_eq!(h, 3);
        // Short row should be padded with spaces
        assert_eq!(grid[1].len(), 4);
        assert_eq!(grid[1][3], ' ');
    }

    #[test]
    fn test_empty_level_definition() {
        let toml_str = r#"
[level]
name = "empty"
"#;
        let level: LevelDefinition = toml::from_str(toml_str).unwrap();
        assert_eq!(level.level.name, "empty");
        assert!(level.map.is_none());
        assert!(level.monsters.is_empty());
        assert!(level.objects.is_empty());
        assert!(level.traps.is_empty());
        assert!(level.doors.is_empty());
        assert!(level.stairs.is_empty());
        assert!(level.altars.is_empty());
        assert!(level.engraving.is_empty());
        assert!(level.shuffle_groups.is_empty());
    }

    #[test]
    fn test_object_placement_full() {
        let toml_str = r#"
[level]
name = "test"

[[objects]]
id = "long sword"
x = 5
y = 3
chance = 75
cursed = true
enchantment = -3
identified = true
quantity = 1
"#;
        let level: LevelDefinition = toml::from_str(toml_str).unwrap();
        let obj = &level.objects[0];
        assert_eq!(obj.id.as_deref(), Some("long sword"));
        assert_eq!(obj.chance, 75);
        assert_eq!(obj.cursed, Some(true));
        assert_eq!(obj.enchantment, Some(-3));
        assert_eq!(obj.identified, Some(true));
    }

    #[test]
    fn test_trap_placement() {
        let toml_str = r#"
[level]
name = "test"

[[traps]]
type = "pit"
x = 3
y = 5
chance = 50
"#;
        let level: LevelDefinition = toml::from_str(toml_str).unwrap();
        assert_eq!(level.traps[0].trap_type, "pit");
        assert_eq!(level.traps[0].x, Some(3));
        assert_eq!(level.traps[0].chance, 50);
    }

    #[test]
    fn test_door_placement() {
        let toml_str = r#"
[level]
name = "test"

[[doors]]
state = "locked"
x = 5
y = 3
"#;
        let level: LevelDefinition = toml::from_str(toml_str).unwrap();
        assert_eq!(level.doors[0].state, "locked");
        assert_eq!(level.doors[0].x, 5);
    }

    #[test]
    fn test_altar_placement() {
        let toml_str = r#"
[level]
name = "test"

[[altars]]
align = "chaotic"
x = 10
y = 5
shrine = true
"#;
        let level: LevelDefinition = toml::from_str(toml_str).unwrap();
        assert_eq!(level.altars[0].align, "chaotic");
        assert!(level.altars[0].shrine);
    }

    #[test]
    fn test_region_definition() {
        let toml_str = r#"
[level]
name = "test"

[[regions]]
area = [0, 0, 20, 10]
lit = true
region_type = "shop"
irregular = true
"#;
        let level: LevelDefinition = toml::from_str(toml_str).unwrap();
        assert_eq!(level.regions[0].area, [0, 0, 20, 10]);
        assert_eq!(level.regions[0].lit, Some(true));
        assert_eq!(level.regions[0].region_type.as_deref(), Some("shop"));
        assert!(level.regions[0].irregular);
    }

    #[test]
    fn test_branch_with_flags() {
        let toml_str = r#"
[[branches]]
name = "VladsTower"
max_depth = 3
flags = ["going_up"]
special_levels = [
    { name = "vlad1", depth = 1 },
]
"#;
        let topo: DungeonTopology = toml::from_str(toml_str).unwrap();
        assert_eq!(topo.branches[0].flags, vec!["going_up"]);
    }

    #[test]
    fn test_connection_defaults() {
        let toml_str = r#"
[[branches]]
name = "Main"
max_depth = 29

[[connections]]
from_branch = "Main"
to_branch = "Mines"
entrance_range = [3, 5]
"#;
        let topo: DungeonTopology = toml::from_str(toml_str).unwrap();
        assert_eq!(topo.connections[0].connection_type, "stairs");
    }
}
