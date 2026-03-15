//! Pager, look, and whatis system.
//!
//! Provides text descriptions for everything the player can look at or query:
//! terrain, monsters, objects, traps, engravings, and help topics.
//! Corresponds to C NetHack's `pager.c` and parts of `insight.c`.

/// Describe what's at a specific map position.
/// This is the core of the `:` (look here) and `;` (look there) commands.
pub fn describe_position(
    terrain: &str,
    monster_name: Option<&str>,
    object_names: &[String],
    trap_name: Option<&str>,
    engraving: Option<&str>,
    is_lit: bool,
    is_explored: bool,
) -> Vec<String> {
    let mut descriptions = Vec::new();

    if !is_explored {
        descriptions.push("You see nothing here.".to_string());
        return descriptions;
    }

    // Monster first
    if let Some(mon) = monster_name {
        descriptions.push(format!("You see {} here.", mon));
    }

    // Objects
    match object_names.len() {
        0 => {}
        1 => descriptions.push(format!("You see {} here.", object_names[0])),
        n => descriptions.push(format!("You see {} objects here.", n)),
    }

    // Trap
    if let Some(trap) = trap_name {
        descriptions.push(format!("There is {} here.", trap));
    }

    // Engraving
    if let Some(text) = engraving {
        descriptions.push(format!("You read: \"{}\".", text));
    }

    // Terrain
    descriptions.push(describe_terrain(terrain, is_lit));

    descriptions
}

/// Describe terrain type for the look command.
pub fn describe_terrain(terrain: &str, _is_lit: bool) -> String {
    let base = match terrain {
        "floor" => "the floor of a room",
        "corridor" => "a corridor",
        "stairs_up" => "a staircase up",
        "stairs_down" => "a staircase down",
        "closed_door" => "a closed door",
        "open_door" => "an open door",
        "locked_door" => "a locked door",
        "wall" | "horizontal_wall" | "vertical_wall" => "a wall",
        "fountain" => "a fountain",
        "throne" => "an opulent throne",
        "altar" => "an altar",
        "pool" => "a pool of water",
        "moat" => "a moat",
        "lava" => "molten lava",
        "tree" => "a tree",
        "iron_bars" => "a set of iron bars",
        "grave" => "a grave",
        "drawbridge_raised" => "a raised drawbridge",
        "drawbridge_lowered" => "a lowered drawbridge",
        "ice" => "a sheet of ice",
        "air" => "open air",
        "cloud" => "a cloud",
        "stone" => "solid stone",
        "secret_door" => "a wall", // secret doors look like walls
        _ => "something unknown",
    };
    format!("There is {} here.", base)
}

/// Describe a monster in detail (for far-look / whatis on a monster).
pub fn describe_monster(
    name: &str,
    level: i32,
    is_peaceful: bool,
    is_tame: bool,
    hp_status: &str,
) -> Vec<String> {
    let mut desc = Vec::new();
    desc.push(format!("{}:", name));

    if is_tame {
        desc.push("  It is tame.".to_string());
    } else if is_peaceful {
        desc.push("  It is peaceful.".to_string());
    }

    desc.push(format!("  Level {}, {}.", level, hp_status));
    desc
}

/// Describe a monster's HP status from its current/max HP.
pub fn hp_status_description(current: i32, max: i32) -> &'static str {
    let ratio = current as f64 / max.max(1) as f64;
    if ratio >= 0.9 {
        "healthy"
    } else if ratio >= 0.5 {
        "injured"
    } else if ratio >= 0.25 {
        "badly injured"
    } else {
        "near death"
    }
}

/// Describe an object in detail.
pub fn describe_object(
    name: &str,
    object_class: char,
    is_identified: bool,
    weight: i32,
) -> Vec<String> {
    let mut desc = Vec::new();
    desc.push(format!("{}:", name));

    let class_name = match object_class {
        ')' => "weapon",
        '[' => "armor",
        '=' => "ring",
        '"' => "amulet",
        '(' => "tool",
        '%' => "food",
        '!' => "potion",
        '?' => "scroll",
        '+' => "spellbook",
        '/' => "wand",
        '$' => "coins",
        '*' => "gem",
        _ => "item",
    };
    desc.push(format!("  Class: {}", class_name));

    if is_identified {
        desc.push(format!("  Weight: {}", weight));
    }

    desc
}

/// Character enlightenment — show all player attributes and intrinsics.
/// This is the #enlightenment / insight system from C.
pub fn enlightenment(
    attributes: &[(String, String)],
    intrinsics: &[String],
    conducts: &[String],
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("Current Attributes:".to_string());
    lines.push(String::new());

    for (name, value) in attributes {
        lines.push(format!("  You have {} {}.", value, name));
    }

    if !intrinsics.is_empty() {
        lines.push(String::new());
        lines.push("Intrinsics:".to_string());
        for intr in intrinsics {
            lines.push(format!("  You have {}.", intr));
        }
    }

    if !conducts.is_empty() {
        lines.push(String::new());
        lines.push("Conducts:".to_string());
        for conduct in conducts {
            lines.push(format!("  You have been {}.", conduct));
        }
    }

    lines
}

/// Describe a trap by type name.
pub fn describe_trap(trap_type: &str) -> String {
    match trap_type {
        "pit" => "a pit".to_string(),
        "spiked pit" => "a spiked pit".to_string(),
        "bear trap" => "a bear trap".to_string(),
        "web" => "a web".to_string(),
        "teleport trap" => "a teleportation trap".to_string(),
        "level teleport" => "a level teleporter".to_string(),
        "fire trap" => "a fire trap".to_string(),
        "sleep gas trap" => "a sleep gas trap".to_string(),
        "rust trap" => "a rust trap".to_string(),
        "anti-magic" => "an anti-magic field".to_string(),
        "land mine" => "a land mine".to_string(),
        "rolling boulder" => "a rolling boulder trap".to_string(),
        "squeaky board" => "a squeaky board".to_string(),
        "arrow trap" => "an arrow trap".to_string(),
        "dart trap" => "a dart trap".to_string(),
        "falling rock trap" => "a falling rock trap".to_string(),
        "magic portal" => "a magic portal".to_string(),
        "vibrating square" => "a vibrating square".to_string(),
        "polymorph trap" => "a polymorph trap".to_string(),
        "magic trap" => "a magic trap".to_string(),
        "statue trap" => "a statue trap".to_string(),
        "hole" => "a hole".to_string(),
        "trapdoor" => "a trapdoor".to_string(),
        _ => format!("a {} trap", trap_type),
    }
}

/// Help topic lookup for the `?` command.
pub fn help_topic(topic: &str) -> Option<&'static str> {
    match topic {
        "commands" => Some(
            "Movement: hjkl/yubn or numpad\n\
             Actions: e(eat), q(quaff), r(read), z(zap), w(wield), W(wear)\n\
             Info: i(inventory), /(whatis), :(look)\n\
             Other: s(search), o(open), c(close), >(down), <(up)",
        ),
        "options" => Some("Set options with OPTIONS= in .nethackrc or press O in-game."),
        "symbols" => {
            Some("@=you .=floor #=corridor <=upstairs >=downstairs +=door |/-=wall")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- describe_position ----

    #[test]
    fn test_describe_position_unexplored() {
        let result = describe_position("floor", None, &[], None, None, false, false);
        assert_eq!(result, vec!["You see nothing here."]);
    }

    #[test]
    fn test_describe_position_empty_floor() {
        let result = describe_position("floor", None, &[], None, None, true, true);
        assert_eq!(result, vec!["There is the floor of a room here."]);
    }

    #[test]
    fn test_describe_position_with_monster() {
        let result = describe_position(
            "floor",
            Some("a grid bug"),
            &[],
            None,
            None,
            true,
            true,
        );
        assert_eq!(result[0], "You see a grid bug here.");
        assert_eq!(result.len(), 2); // monster + terrain
    }

    #[test]
    fn test_describe_position_with_single_object() {
        let result = describe_position(
            "corridor",
            None,
            &["a long sword".to_string()],
            None,
            None,
            true,
            true,
        );
        assert_eq!(result[0], "You see a long sword here.");
        assert_eq!(result[1], "There is a corridor here.");
    }

    #[test]
    fn test_describe_position_with_multiple_objects() {
        let objects = vec![
            "a long sword".to_string(),
            "a shield".to_string(),
            "a helmet".to_string(),
        ];
        let result = describe_position("floor", None, &objects, None, None, true, true);
        assert_eq!(result[0], "You see 3 objects here.");
    }

    #[test]
    fn test_describe_position_with_trap() {
        let result = describe_position(
            "floor",
            None,
            &[],
            Some("a pit"),
            None,
            true,
            true,
        );
        assert_eq!(result[0], "There is a pit here.");
    }

    #[test]
    fn test_describe_position_with_engraving() {
        let result = describe_position(
            "floor",
            None,
            &[],
            None,
            Some("Elbereth"),
            true,
            true,
        );
        assert_eq!(result[0], "You read: \"Elbereth\".");
    }

    #[test]
    fn test_describe_position_everything() {
        let result = describe_position(
            "floor",
            Some("a goblin"),
            &["a dagger".to_string()],
            Some("a bear trap"),
            Some("Elbereth"),
            true,
            true,
        );
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], "You see a goblin here.");
        assert_eq!(result[1], "You see a dagger here.");
        assert_eq!(result[2], "There is a bear trap here.");
        assert_eq!(result[3], "You read: \"Elbereth\".");
        assert_eq!(result[4], "There is the floor of a room here.");
    }

    // ---- describe_terrain ----

    #[test]
    fn test_describe_terrain_floor() {
        assert_eq!(
            describe_terrain("floor", true),
            "There is the floor of a room here."
        );
    }

    #[test]
    fn test_describe_terrain_corridor() {
        assert_eq!(
            describe_terrain("corridor", false),
            "There is a corridor here."
        );
    }

    #[test]
    fn test_describe_terrain_stairs() {
        assert_eq!(
            describe_terrain("stairs_up", true),
            "There is a staircase up here."
        );
        assert_eq!(
            describe_terrain("stairs_down", true),
            "There is a staircase down here."
        );
    }

    #[test]
    fn test_describe_terrain_doors() {
        assert_eq!(
            describe_terrain("closed_door", true),
            "There is a closed door here."
        );
        assert_eq!(
            describe_terrain("open_door", true),
            "There is an open door here."
        );
        assert_eq!(
            describe_terrain("locked_door", true),
            "There is a locked door here."
        );
    }

    #[test]
    fn test_describe_terrain_walls() {
        let expected = "There is a wall here.";
        assert_eq!(describe_terrain("wall", true), expected);
        assert_eq!(describe_terrain("horizontal_wall", true), expected);
        assert_eq!(describe_terrain("vertical_wall", true), expected);
    }

    #[test]
    fn test_describe_terrain_special() {
        assert_eq!(
            describe_terrain("fountain", true),
            "There is a fountain here."
        );
        assert_eq!(
            describe_terrain("throne", true),
            "There is an opulent throne here."
        );
        assert_eq!(
            describe_terrain("altar", true),
            "There is an altar here."
        );
        assert_eq!(
            describe_terrain("lava", true),
            "There is molten lava here."
        );
        assert_eq!(
            describe_terrain("pool", true),
            "There is a pool of water here."
        );
        assert_eq!(
            describe_terrain("ice", true),
            "There is a sheet of ice here."
        );
    }

    #[test]
    fn test_describe_terrain_secret_door() {
        // Secret doors should look like walls
        assert_eq!(
            describe_terrain("secret_door", true),
            "There is a wall here."
        );
    }

    #[test]
    fn test_describe_terrain_unknown() {
        assert_eq!(
            describe_terrain("xyzzy", true),
            "There is something unknown here."
        );
    }

    // ---- describe_monster ----

    #[test]
    fn test_describe_monster_hostile() {
        let desc = describe_monster("giant rat", 1, false, false, "healthy");
        assert_eq!(desc.len(), 2);
        assert_eq!(desc[0], "giant rat:");
        assert_eq!(desc[1], "  Level 1, healthy.");
    }

    #[test]
    fn test_describe_monster_peaceful() {
        let desc = describe_monster("shopkeeper", 12, true, false, "healthy");
        assert_eq!(desc.len(), 3);
        assert_eq!(desc[0], "shopkeeper:");
        assert_eq!(desc[1], "  It is peaceful.");
        assert_eq!(desc[2], "  Level 12, healthy.");
    }

    #[test]
    fn test_describe_monster_tame() {
        let desc = describe_monster("little dog", 2, false, true, "injured");
        assert_eq!(desc.len(), 3);
        assert_eq!(desc[0], "little dog:");
        assert_eq!(desc[1], "  It is tame.");
        assert_eq!(desc[2], "  Level 2, injured.");
    }

    #[test]
    fn test_describe_monster_tame_overrides_peaceful() {
        // If both tame and peaceful, tame takes precedence
        let desc = describe_monster("kitten", 2, true, true, "healthy");
        assert_eq!(desc.len(), 3);
        assert_eq!(desc[1], "  It is tame.");
    }

    // ---- hp_status_description ----

    #[test]
    fn test_hp_status_healthy() {
        assert_eq!(hp_status_description(10, 10), "healthy");
        assert_eq!(hp_status_description(9, 10), "healthy");
    }

    #[test]
    fn test_hp_status_injured() {
        assert_eq!(hp_status_description(8, 10), "injured");
        assert_eq!(hp_status_description(5, 10), "injured");
    }

    #[test]
    fn test_hp_status_badly_injured() {
        assert_eq!(hp_status_description(4, 10), "badly injured");
        assert_eq!(hp_status_description(3, 10), "badly injured");
    }

    #[test]
    fn test_hp_status_near_death() {
        assert_eq!(hp_status_description(2, 10), "near death");
        assert_eq!(hp_status_description(1, 10), "near death");
    }

    #[test]
    fn test_hp_status_zero_max() {
        // Edge case: max HP is 0 — should not panic
        assert_eq!(hp_status_description(0, 0), "near death");
    }

    // ---- describe_object ----

    #[test]
    fn test_describe_object_identified() {
        let desc = describe_object("a long sword", ')', true, 40);
        assert_eq!(desc.len(), 3);
        assert_eq!(desc[0], "a long sword:");
        assert_eq!(desc[1], "  Class: weapon");
        assert_eq!(desc[2], "  Weight: 40");
    }

    #[test]
    fn test_describe_object_unidentified() {
        let desc = describe_object("a bubbly potion", '!', false, 20);
        assert_eq!(desc.len(), 2);
        assert_eq!(desc[0], "a bubbly potion:");
        assert_eq!(desc[1], "  Class: potion");
    }

    #[test]
    fn test_describe_object_classes() {
        assert!(describe_object("x", '[', false, 0)[1].contains("armor"));
        assert!(describe_object("x", '=', false, 0)[1].contains("ring"));
        assert!(describe_object("x", '"', false, 0)[1].contains("amulet"));
        assert!(describe_object("x", '(', false, 0)[1].contains("tool"));
        assert!(describe_object("x", '%', false, 0)[1].contains("food"));
        assert!(describe_object("x", '?', false, 0)[1].contains("scroll"));
        assert!(describe_object("x", '+', false, 0)[1].contains("spellbook"));
        assert!(describe_object("x", '/', false, 0)[1].contains("wand"));
        assert!(describe_object("x", '$', false, 0)[1].contains("coins"));
        assert!(describe_object("x", '*', false, 0)[1].contains("gem"));
        assert!(describe_object("x", '^', false, 0)[1].contains("item")); // unknown class
    }

    // ---- enlightenment ----

    #[test]
    fn test_enlightenment_basic() {
        let attrs = vec![
            ("strength".to_string(), "18".to_string()),
            ("dexterity".to_string(), "14".to_string()),
        ];
        let intrinsics = vec!["fire resistance".to_string()];
        let conducts = vec!["vegetarian".to_string()];

        let lines = enlightenment(&attrs, &intrinsics, &conducts);
        assert_eq!(lines[0], "Current Attributes:");
        assert_eq!(lines[1], "");
        assert_eq!(lines[2], "  You have 18 strength.");
        assert_eq!(lines[3], "  You have 14 dexterity.");
        assert_eq!(lines[4], "");
        assert_eq!(lines[5], "Intrinsics:");
        assert_eq!(lines[6], "  You have fire resistance.");
        assert_eq!(lines[7], "");
        assert_eq!(lines[8], "Conducts:");
        assert_eq!(lines[9], "  You have been vegetarian.");
    }

    #[test]
    fn test_enlightenment_no_intrinsics_or_conducts() {
        let attrs = vec![("strength".to_string(), "10".to_string())];
        let lines = enlightenment(&attrs, &[], &[]);
        assert_eq!(lines.len(), 3); // header, blank, one attribute
        assert_eq!(lines[2], "  You have 10 strength.");
    }

    // ---- describe_trap ----

    #[test]
    fn test_describe_trap_known_types() {
        assert_eq!(describe_trap("pit"), "a pit");
        assert_eq!(describe_trap("spiked pit"), "a spiked pit");
        assert_eq!(describe_trap("bear trap"), "a bear trap");
        assert_eq!(describe_trap("web"), "a web");
        assert_eq!(describe_trap("teleport trap"), "a teleportation trap");
        assert_eq!(describe_trap("level teleport"), "a level teleporter");
        assert_eq!(describe_trap("fire trap"), "a fire trap");
        assert_eq!(describe_trap("sleep gas trap"), "a sleep gas trap");
        assert_eq!(describe_trap("rust trap"), "a rust trap");
        assert_eq!(describe_trap("anti-magic"), "an anti-magic field");
        assert_eq!(describe_trap("land mine"), "a land mine");
        assert_eq!(describe_trap("rolling boulder"), "a rolling boulder trap");
        assert_eq!(describe_trap("squeaky board"), "a squeaky board");
        assert_eq!(describe_trap("arrow trap"), "an arrow trap");
        assert_eq!(describe_trap("dart trap"), "a dart trap");
        assert_eq!(describe_trap("falling rock trap"), "a falling rock trap");
        assert_eq!(describe_trap("magic portal"), "a magic portal");
        assert_eq!(describe_trap("vibrating square"), "a vibrating square");
        assert_eq!(describe_trap("polymorph trap"), "a polymorph trap");
        assert_eq!(describe_trap("magic trap"), "a magic trap");
        assert_eq!(describe_trap("statue trap"), "a statue trap");
        assert_eq!(describe_trap("hole"), "a hole");
        assert_eq!(describe_trap("trapdoor"), "a trapdoor");
    }

    #[test]
    fn test_describe_trap_unknown() {
        assert_eq!(describe_trap("mystery"), "a mystery trap");
    }

    // ---- help_topic ----

    #[test]
    fn test_help_topic_commands() {
        let help = help_topic("commands");
        assert!(help.is_some());
        assert!(help.unwrap().contains("Movement"));
        assert!(help.unwrap().contains("hjkl"));
    }

    #[test]
    fn test_help_topic_options() {
        let help = help_topic("options");
        assert!(help.is_some());
        assert!(help.unwrap().contains("OPTIONS="));
    }

    #[test]
    fn test_help_topic_symbols() {
        let help = help_topic("symbols");
        assert!(help.is_some());
        assert!(help.unwrap().contains("@=you"));
    }

    #[test]
    fn test_help_topic_unknown() {
        assert!(help_topic("nonexistent").is_none());
    }
}
