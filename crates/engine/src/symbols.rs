//! Symbol identification — describe what glyphs mean on the map.
//!
//! Provides human-readable descriptions for terrain, monster classes,
//! and object classes. Used when the player presses `/` (whatis).

/// Describe a terrain type for the whatis command.
pub fn describe_terrain(terrain: &crate::dungeon::Terrain) -> &'static str {
    use crate::dungeon::Terrain;
    match terrain {
        Terrain::Stone => "solid stone",
        Terrain::Wall => "wall",
        Terrain::Floor => "floor of a room",
        Terrain::Corridor => "corridor",
        Terrain::DoorOpen => "open door",
        Terrain::DoorClosed => "closed door",
        Terrain::DoorLocked => "closed door",
        Terrain::StairsUp => "staircase up",
        Terrain::StairsDown => "staircase down",
        Terrain::Altar => "altar",
        Terrain::Fountain => "fountain",
        Terrain::Throne => "opulent throne",
        Terrain::Sink => "kitchen sink",
        Terrain::Grave => "headstone",
        Terrain::Pool => "pool of water",
        Terrain::Moat => "moat",
        Terrain::Lava => "molten lava",
        Terrain::Ice => "ice",
        Terrain::Air => "open air",
        Terrain::Cloud => "cloud",
        Terrain::Water => "water",
        Terrain::Tree => "tree",
        Terrain::IronBars => "set of iron bars",
        Terrain::Drawbridge => "drawbridge",
        Terrain::MagicPortal => "magic portal",
    }
}

/// Describe a monster class letter for the whatis command.
pub fn describe_monster_class(ch: char) -> &'static str {
    match ch {
        'a' => "ant or other insect",
        'b' => "blob",
        'c' => "cockatrice or similar",
        'd' => "canine (dog or wolf)",
        'e' => "floating eye or sphere",
        'f' => "feline (cat or tiger)",
        'g' => "gremlin or gargoyle",
        'h' => "humanoid",
        'i' => "imp or minor demon",
        'j' => "jelly",
        'k' => "kobold",
        'l' => "leprechaun",
        'm' => "mimic",
        'n' => "nymph",
        'o' => "orc",
        'p' => "piercer",
        'q' => "quadruped",
        'r' => "rodent",
        's' => "spider or scorpion",
        't' => "trapper or lurker above",
        'u' => "unicorn or horse",
        'v' => "vortex",
        'w' => "worm",
        'x' => "xan or grid bug",
        'y' => "apelike creature",
        'z' => "zombie",
        'A' => "angelic being",
        'B' => "bat or bird",
        'C' => "centaur",
        'D' => "dragon",
        'E' => "elemental",
        'F' => "fungus or mold",
        'G' => "gnome",
        'H' => "giant humanoid",
        'I' => "invisible stalker",
        'J' => "jabberwock",
        'K' => "Keystone Kop",
        'L' => "lich",
        'M' => "mummy",
        'N' => "naga",
        'O' => "ogre",
        'P' => "pudding or ooze",
        'Q' => "quantum mechanic",
        'R' => "rust monster or disenchanter",
        'S' => "snake",
        'T' => "troll",
        'U' => "umber hulk",
        'V' => "vampire",
        'W' => "wraith",
        'X' => "xorn",
        'Y' => "yeti or ape",
        'Z' => "major zombie",
        '&' => "demon or devil",
        '\'' => "golem",
        ':' => "sea monster",
        ';' => "long worm tail",
        '@' => "human or elf",
        _ => "unknown creature",
    }
}

/// Describe an object class symbol for the whatis command.
pub fn describe_object_class(ch: char) -> &'static str {
    match ch {
        ')' => "weapon",
        '[' => "suit of armor",
        '=' => "ring",
        '"' => "amulet",
        '(' => "useful item (tool)",
        '!' => "potion",
        '?' => "scroll",
        '+' => "spellbook",
        '/' => "wand",
        '$' => "pile of gold",
        '*' => "gem or rock",
        '%' => "piece of food",
        '0' => "iron ball",
        '_' => "altar or iron chain",
        '`' => "boulder or statue",
        '#' => "sink",
        '{' => "fountain",
        '}' => "pool or moat",
        '\\' => "opulent throne",
        '^' => "trap",
        '<' => "staircase up",
        '>' => "staircase down",
        '|' => "wall or open door",
        '-' => "wall or open door",
        '.' => "floor",
        _ => "unknown symbol",
    }
}

/// Describe a terrain type by its display character (for the `/` command lookup).
pub fn terrain_description(symbol: char) -> &'static str {
    match symbol {
        '.' => "floor of a room",
        '#' => "corridor",
        '<' => "staircase up",
        '>' => "staircase down",
        '+' => "door (closed)",
        '-' => "wall (horizontal)",
        '|' => "wall (vertical)",
        '{' => "fountain",
        '}' => "pool of water",
        '\\' => "opulent throne",
        '_' => "altar",
        '^' => "trap",
        'T' => "tree",
        _ => "unknown terrain",
    }
}

/// Full what-is lookup: given a map symbol, describe what it could be.
/// Returns a list of possible descriptions.
pub fn whatis_symbol(symbol: char) -> Vec<&'static str> {
    let mut results = Vec::new();

    // Check terrain
    let terrain = terrain_description(symbol);
    if terrain != "unknown terrain" {
        results.push(terrain);
    }

    // Check monster class
    let monster = describe_monster_class(symbol);
    if monster != "unknown creature" {
        results.push(monster);
    }

    // Check object class
    let object = describe_object_class(symbol);
    if object != "unknown symbol" {
        results.push(object);
    }

    if results.is_empty() {
        results.push("unknown symbol");
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monster_class_ant() {
        assert_eq!(describe_monster_class('a'), "ant or other insect");
    }

    #[test]
    fn test_monster_class_dragon() {
        assert_eq!(describe_monster_class('D'), "dragon");
    }

    #[test]
    fn test_monster_class_demon() {
        assert_eq!(describe_monster_class('&'), "demon or devil");
    }

    #[test]
    fn test_monster_class_unknown() {
        assert_eq!(describe_monster_class('$'), "unknown creature");
    }

    #[test]
    fn test_object_class_weapon() {
        assert_eq!(describe_object_class(')'), "weapon");
    }

    #[test]
    fn test_object_class_potion() {
        assert_eq!(describe_object_class('!'), "potion");
    }

    #[test]
    fn test_terrain_floor() {
        assert_eq!(terrain_description('.'), "floor of a room");
    }

    #[test]
    fn test_terrain_corridor() {
        assert_eq!(terrain_description('#'), "corridor");
    }

    #[test]
    fn test_whatis_at_sign() {
        let r = whatis_symbol('@');
        assert!(r.contains(&"human or elf"));
    }

    #[test]
    fn test_whatis_dot() {
        let r = whatis_symbol('.');
        assert!(r.contains(&"floor of a room"));
        assert!(r.contains(&"floor")); // object class also matches
    }

    #[test]
    fn test_whatis_unknown() {
        let r = whatis_symbol('\x01');
        assert_eq!(r, vec!["unknown symbol"]);
    }

    #[test]
    fn test_help_files_exist() {
        // Navigate from crate root to workspace root
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let help_dir = workspace.join("data/help");
        assert!(help_dir.join("help.txt").exists());
        assert!(help_dir.join("cmdhelp.txt").exists());
        assert!(help_dir.join("opthelp.txt").exists());
        assert!(help_dir.join("history.txt").exists());
    }
}
