//! Game start flow: role, race, alignment selection and starting setup.
//!
//! This module handles the character creation sequence shown before the
//! dungeon is generated.  When CLI args supply a role/race/alignment the
//! corresponding menu is skipped.

use nethack_babel_engine::combat::SkillLevel;
use nethack_babel_engine::pets::{self, Role};
use nethack_babel_engine::world::{Attributes, HitPoints, Power};
use nethack_babel_i18n::LocaleManager;
use nethack_babel_tui::{Menu, MenuHow, MenuItem, MenuResult, WindowPort};

// ---------------------------------------------------------------------------
// Role / Race / Alignment definitions
// ---------------------------------------------------------------------------

/// All 13 NetHack roles with their FTL message key suffix.
pub const ALL_ROLES: &[(&str, &str, Role)] = &[
    ("Archeologist", "role-archeologist", Role::Archeologist),
    ("Barbarian", "role-barbarian", Role::Barbarian),
    ("Caveperson", "role-caveperson", Role::Caveperson),
    ("Healer", "role-healer", Role::Healer),
    ("Knight", "role-knight", Role::Knight),
    ("Monk", "role-monk", Role::Monk),
    ("Priest", "role-priest", Role::Priest),
    ("Ranger", "role-ranger", Role::Ranger),
    ("Rogue", "role-rogue", Role::Rogue),
    ("Samurai", "role-samurai", Role::Samurai),
    ("Tourist", "role-tourist", Role::Tourist),
    ("Valkyrie", "role-valkyrie", Role::Valkyrie),
    ("Wizard", "role-wizard", Role::Wizard),
];

/// Player races.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Race {
    Human,
    Elf,
    Dwarf,
    Gnome,
    Orc,
}

impl Race {
    /// English name (for CLI arg parsing and internal use).
    pub fn name(self) -> &'static str {
        match self {
            Race::Human => "Human",
            Race::Elf => "Elf",
            Race::Dwarf => "Dwarf",
            Race::Gnome => "Gnome",
            Race::Orc => "Orc",
        }
    }

    /// FTL message key for this race.
    fn ftl_key(self) -> &'static str {
        match self {
            Race::Human => "race-human",
            Race::Elf => "race-elf",
            Race::Dwarf => "race-dwarf",
            Race::Gnome => "race-gnome",
            Race::Orc => "race-orc",
        }
    }

    /// Translated display name.
    pub fn display_name(self, locale: &LocaleManager) -> String {
        locale.translate(self.ftl_key(), None)
    }
}

/// Player alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Lawful,
    Neutral,
    Chaotic,
}

impl Alignment {
    /// English name (for CLI arg parsing and internal use).
    pub fn name(self) -> &'static str {
        match self {
            Alignment::Lawful => "Lawful",
            Alignment::Neutral => "Neutral",
            Alignment::Chaotic => "Chaotic",
        }
    }

    /// FTL message key for this alignment.
    fn ftl_key(self) -> &'static str {
        match self {
            Alignment::Lawful => "alignment-lawful",
            Alignment::Neutral => "alignment-neutral",
            Alignment::Chaotic => "alignment-chaotic",
        }
    }

    /// Translated display name.
    pub fn display_name(self, locale: &LocaleManager) -> String {
        locale.translate(self.ftl_key(), None)
    }
}

/// Valid races for each role, following NetHack 3.7.
pub fn valid_races(role: Role) -> &'static [Race] {
    match role {
        Role::Archeologist => &[Race::Human, Race::Dwarf, Race::Gnome],
        Role::Barbarian => &[Race::Human, Race::Orc],
        Role::Caveperson => &[Race::Human, Race::Dwarf, Race::Gnome],
        Role::Healer => &[Race::Human, Race::Gnome],
        Role::Knight => &[Race::Human],
        Role::Monk => &[Race::Human],
        Role::Priest => &[Race::Human, Race::Elf],
        Role::Ranger => &[Race::Human, Race::Elf, Race::Gnome, Race::Orc],
        Role::Rogue => &[Race::Human, Race::Orc],
        Role::Samurai => &[Race::Human],
        Role::Tourist => &[Race::Human],
        Role::Valkyrie => &[Race::Human, Race::Dwarf],
        Role::Wizard => &[Race::Human, Race::Elf, Race::Gnome, Race::Orc],
    }
}

/// Valid alignments for a given role + race combination.
pub fn valid_alignments(role: Role, _race: Race) -> &'static [Alignment] {
    match role {
        Role::Archeologist => &[Alignment::Lawful, Alignment::Neutral],
        Role::Barbarian => &[Alignment::Neutral, Alignment::Chaotic],
        Role::Caveperson => &[Alignment::Lawful, Alignment::Neutral],
        Role::Healer => &[Alignment::Neutral],
        Role::Knight => &[Alignment::Lawful],
        Role::Monk => &[Alignment::Lawful, Alignment::Neutral, Alignment::Chaotic],
        Role::Priest => &[Alignment::Lawful, Alignment::Neutral, Alignment::Chaotic],
        Role::Ranger => &[Alignment::Neutral, Alignment::Chaotic],
        Role::Rogue => &[Alignment::Chaotic],
        Role::Samurai => &[Alignment::Lawful],
        Role::Tourist => &[Alignment::Neutral],
        Role::Valkyrie => &[Alignment::Lawful, Alignment::Neutral],
        Role::Wizard => &[Alignment::Neutral, Alignment::Chaotic],
    }
}

// ---------------------------------------------------------------------------
// Starting stats per role
// ---------------------------------------------------------------------------

/// Base starting stats for a given role.
pub fn starting_stats(role: Role) -> (Attributes, HitPoints, Power) {
    let (str_, dex, con, int, wis, cha, hp, pw) = match role {
        Role::Archeologist => (7, 10, 7, 7, 7, 7, 11, 1),
        Role::Barbarian => (16, 15, 16, 7, 7, 6, 14, 1),
        Role::Caveperson => (10, 7, 8, 7, 7, 6, 14, 1),
        Role::Healer => (7, 7, 11, 7, 13, 7, 11, 4),
        Role::Knight => (13, 7, 14, 7, 7, 17, 14, 4),
        Role::Monk => (10, 8, 7, 7, 8, 7, 12, 2),
        Role::Priest => (7, 7, 7, 7, 10, 7, 12, 4),
        Role::Ranger => (13, 13, 13, 7, 7, 6, 13, 1),
        Role::Rogue => (7, 10, 7, 7, 7, 10, 10, 1),
        Role::Samurai => (10, 8, 17, 7, 7, 8, 16, 1),
        Role::Tourist => (7, 7, 7, 10, 6, 7, 8, 1),
        Role::Valkyrie => (10, 7, 10, 7, 7, 7, 14, 1),
        Role::Wizard => (7, 7, 7, 10, 7, 7, 10, 4),
    };

    let attrs = Attributes {
        strength: str_,
        strength_extra: 0,
        dexterity: dex,
        constitution: con,
        intelligence: int,
        wisdom: wis,
        charisma: cha,
    };
    let hit_points = HitPoints {
        current: hp,
        max: hp,
    };
    let power = Power {
        current: pw,
        max: pw,
    };

    (attrs, hit_points, power)
}

/// Apply racial stat adjustments to base attributes.
pub fn apply_race_adjustments(attrs: &mut Attributes, race: Race) {
    match race {
        Race::Human => {}
        Race::Elf => {
            attrs.intelligence = attrs.intelligence.saturating_add(1);
            attrs.wisdom = attrs.wisdom.saturating_add(1);
            attrs.constitution = attrs.constitution.saturating_sub(1);
        }
        Race::Dwarf => {
            attrs.strength = attrs.strength.saturating_add(1);
            attrs.constitution = attrs.constitution.saturating_add(1);
            attrs.charisma = attrs.charisma.saturating_sub(1);
        }
        Race::Gnome => {
            attrs.intelligence = attrs.intelligence.saturating_add(1);
            attrs.strength = attrs.strength.saturating_sub(1);
        }
        Race::Orc => {
            attrs.strength = attrs.strength.saturating_add(1);
            attrs.constitution = attrs.constitution.saturating_add(1);
            attrs.intelligence = attrs.intelligence.saturating_sub(1);
            attrs.charisma = attrs.charisma.saturating_sub(2);
        }
    }
}

// ---------------------------------------------------------------------------
// Starting skills per role
// ---------------------------------------------------------------------------

/// Starting skill levels for each role.
///
/// Reference: `src/u_init.c` — `Skill_*[]` arrays.
pub fn starting_skills(role: Role) -> Vec<(String, SkillLevel)> {
    match role {
        Role::Archeologist => vec![
            ("pick-axe".into(), SkillLevel::Basic),
            ("sling".into(), SkillLevel::Basic),
        ],
        Role::Barbarian => vec![
            ("two-handed sword".into(), SkillLevel::Basic),
            ("axe".into(), SkillLevel::Basic),
        ],
        Role::Caveperson => vec![
            ("club".into(), SkillLevel::Basic),
            ("sling".into(), SkillLevel::Basic),
        ],
        Role::Healer => vec![
            ("knife".into(), SkillLevel::Basic),
            ("healing spells".into(), SkillLevel::Basic),
        ],
        Role::Knight => vec![
            ("long sword".into(), SkillLevel::Basic),
            ("lance".into(), SkillLevel::Basic),
            ("riding".into(), SkillLevel::Basic),
        ],
        Role::Monk => vec![
            ("martial arts".into(), SkillLevel::Basic),
        ],
        Role::Priest => vec![
            ("mace".into(), SkillLevel::Basic),
            ("clerical spells".into(), SkillLevel::Basic),
        ],
        Role::Ranger => vec![
            ("dagger".into(), SkillLevel::Basic),
            ("bow".into(), SkillLevel::Basic),
        ],
        Role::Rogue => vec![
            ("short sword".into(), SkillLevel::Basic),
            ("dagger".into(), SkillLevel::Skilled),
        ],
        Role::Samurai => vec![
            ("long sword".into(), SkillLevel::Basic),
            ("bow".into(), SkillLevel::Basic),
            ("martial arts".into(), SkillLevel::Basic),
        ],
        Role::Tourist => vec![
            ("dart".into(), SkillLevel::Basic),
        ],
        Role::Valkyrie => vec![
            ("long sword".into(), SkillLevel::Basic),
            ("dagger".into(), SkillLevel::Basic),
        ],
        Role::Wizard => vec![
            ("quarterstaff".into(), SkillLevel::Basic),
            ("attack spells".into(), SkillLevel::Basic),
        ],
    }
}

// ---------------------------------------------------------------------------
// Carry capacity
// ---------------------------------------------------------------------------

/// Maximum carry capacity based on strength and constitution.
///
/// Simplified formula; the real NetHack calculation is in `src/hack.c`
/// `max_carr_cap()` and uses `25*(adj_str-3)` for str <= 18 plus
/// extra-strength bonuses.
pub fn initial_carry_capacity(strength: i32, constitution: i32) -> i32 {
    let str_bonus = if strength >= 25 {
        1250
    } else if strength >= 19 {
        1000
    } else if strength >= 18 {
        500 + (strength - 18) * 50
    } else {
        strength * 25
    };
    str_bonus + constitution * 10
}

// ---------------------------------------------------------------------------
// Character selection result
// ---------------------------------------------------------------------------

/// The complete result of character creation.
#[derive(Debug, Clone)]
pub struct CharacterChoice {
    pub role: Role,
    pub role_name: String,
    pub race: Race,
    pub alignment: Alignment,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Translate a role's display name
// ---------------------------------------------------------------------------

fn role_display_name(ftl_key: &str, en_name: &str, locale: &LocaleManager) -> String {
    let translated = locale.translate(ftl_key, None);
    if translated == ftl_key {
        en_name.to_string()
    } else {
        translated
    }
}

// ---------------------------------------------------------------------------
// Menu-based selection (TUI mode)
// ---------------------------------------------------------------------------

/// Run the full character creation flow using window port menus.
pub fn select_character(
    port: &mut impl WindowPort,
    role_arg: Option<&str>,
    race_arg: Option<&str>,
    name_arg: Option<&str>,
    locale: &LocaleManager,
) -> Option<CharacterChoice> {
    // 1. Role selection.
    let (role, role_name) = if let Some(r) = role_arg {
        match parse_role(r) {
            Some((en_name, ftl_key, role)) => {
                (role, role_display_name(ftl_key, en_name, locale))
            }
            None => select_role(port, locale)?,
        }
    } else {
        select_role(port, locale)?
    };

    // 2. Race selection.
    let races = valid_races(role);
    let race = if let Some(r) = race_arg {
        match parse_race(r) {
            Some(race) if races.contains(&race) => race,
            _ => select_race(port, role, races, locale)?,
        }
    } else if races.len() == 1 {
        races[0]
    } else {
        select_race(port, role, races, locale)?
    };

    // 3. Alignment selection.
    let alignments = valid_alignments(role, race);
    let alignment = if alignments.len() == 1 {
        alignments[0]
    } else {
        select_alignment(port, alignments, locale)?
    };

    // 4. Name prompt.
    let name = if let Some(n) = name_arg {
        n.to_string()
    } else {
        let default_name = role_name.clone();
        let mut args = fluent::FluentArgs::new();
        args.set("default", default_name.clone());
        let prompt = locale.translate("chargen-who-are-you", Some(&args));
        match port.get_line(&prompt) {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => default_name,
        }
    };

    Some(CharacterChoice {
        role,
        role_name,
        race,
        alignment,
        name,
    })
}

/// Show the role selection menu.
fn select_role(port: &mut impl WindowPort, locale: &LocaleManager) -> Option<(Role, String)> {
    let items: Vec<MenuItem> = ALL_ROLES
        .iter()
        .enumerate()
        .map(|(i, (_en_name, ftl_key, _role))| MenuItem {
            accelerator: (b'a' + i as u8) as char,
            text: role_display_name(ftl_key, _en_name, locale),
            selected: false,
            selectable: true,
            group: None,
        })
        .collect();

    let menu = Menu {
        title: locale.translate("chargen-pick-role", None),
        items,
        how: MenuHow::PickOne,
    };

    match port.show_menu(&menu) {
        MenuResult::Selected(indices) => {
            let idx = *indices.first()?;
            if idx < ALL_ROLES.len() {
                let (en_name, ftl_key, role) = ALL_ROLES[idx];
                Some((role, role_display_name(ftl_key, en_name, locale)))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Show the race selection menu.
fn select_race(
    port: &mut impl WindowPort,
    _role: Role,
    races: &[Race],
    locale: &LocaleManager,
) -> Option<Race> {
    let items: Vec<MenuItem> = races
        .iter()
        .enumerate()
        .map(|(i, race)| MenuItem {
            accelerator: (b'a' + i as u8) as char,
            text: race.display_name(locale),
            selected: false,
            selectable: true,
            group: None,
        })
        .collect();

    let menu = Menu {
        title: locale.translate("chargen-pick-race", None),
        items,
        how: MenuHow::PickOne,
    };

    match port.show_menu(&menu) {
        MenuResult::Selected(indices) => {
            let idx = *indices.first()?;
            if idx < races.len() {
                Some(races[idx])
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Show the alignment selection menu.
fn select_alignment(
    port: &mut impl WindowPort,
    alignments: &[Alignment],
    locale: &LocaleManager,
) -> Option<Alignment> {
    let items: Vec<MenuItem> = alignments
        .iter()
        .enumerate()
        .map(|(i, align)| MenuItem {
            accelerator: (b'a' + i as u8) as char,
            text: align.display_name(locale),
            selected: false,
            selectable: true,
            group: None,
        })
        .collect();

    let menu = Menu {
        title: locale.translate("chargen-pick-alignment", None),
        items,
        how: MenuHow::PickOne,
    };

    match port.show_menu(&menu) {
        MenuResult::Selected(indices) => {
            let idx = *indices.first()?;
            if idx < alignments.len() {
                Some(alignments[idx])
            } else {
                None
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// CLI arg parsing helpers
// ---------------------------------------------------------------------------

/// Parse a role name from a CLI argument (case-insensitive, prefix match).
pub fn parse_role(input: &str) -> Option<(&'static str, &'static str, Role)> {
    let lower = input.to_lowercase();
    for (name, ftl_key, role) in ALL_ROLES {
        if name.to_lowercase() == lower || name.to_lowercase().starts_with(&lower) {
            return Some((name, ftl_key, *role));
        }
    }
    None
}

/// Parse a race name from a CLI argument (case-insensitive, prefix match).
pub fn parse_race(input: &str) -> Option<Race> {
    let lower = input.to_lowercase();
    let all = [Race::Human, Race::Elf, Race::Dwarf, Race::Gnome, Race::Orc];
    for race in &all {
        if race.name().to_lowercase() == lower || race.name().to_lowercase().starts_with(&lower) {
            return Some(*race);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Apply character choice to a GameWorld
// ---------------------------------------------------------------------------

/// Apply the selected role/race/alignment to the player entity in the world.
pub fn apply_character_choice(
    world: &mut nethack_babel_engine::world::GameWorld,
    choice: &CharacterChoice,
) {
    let player = world.player();

    let (mut attrs, hp, pw) = starting_stats(choice.role);
    apply_race_adjustments(&mut attrs, choice.race);

    if let Some(mut a) = world.get_component_mut::<Attributes>(player) {
        *a = attrs;
    }
    if let Some(mut h) = world.get_component_mut::<HitPoints>(player) {
        *h = hp;
    }
    if let Some(mut p) = world.get_component_mut::<Power>(player) {
        *p = pw;
    }
    if let Some(mut n) = world.get_component_mut::<nethack_babel_engine::world::Name>(player) {
        n.0 = choice.name.clone();
    }
}

/// Spawn the starting pet based on the chosen role.
pub fn spawn_starting_pet(
    world: &mut nethack_babel_engine::world::GameWorld,
    role: Role,
    rng: &mut impl rand::Rng,
) -> Vec<nethack_babel_engine::event::EngineEvent> {
    let (_pet_entity, events) = pets::init_pet(world, role, rng);
    events
}

/// Simple text-mode character creation (with i18n support).
pub fn select_character_text(
    role_arg: Option<&str>,
    race_arg: Option<&str>,
    name_arg: Option<&str>,
    locale: &LocaleManager,
) -> CharacterChoice {
    use std::io::{self, BufRead, Write};

    // 1. Role
    let (role, role_name) = if let Some(r) = role_arg {
        parse_role(r)
            .map(|(en, ftl, role)| (role, role_display_name(ftl, en, locale)))
            .unwrap_or_else(|| {
                eprintln!("Unknown role '{}', defaulting to Valkyrie.", r);
                (Role::Valkyrie, role_display_name("role-valkyrie", "Valkyrie", locale))
            })
    } else {
        println!("{}", locale.translate("chargen-pick-role", None));
        for (i, (en_name, ftl_key, _)) in ALL_ROLES.iter().enumerate() {
            println!(
                "  {} - {}",
                (b'a' + i as u8) as char,
                role_display_name(ftl_key, en_name, locale)
            );
        }
        print!("> ");
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input).ok();
        let ch = input.trim().chars().next().unwrap_or('l');
        let idx = if ch.is_ascii_lowercase() {
            (ch as u8 - b'a') as usize
        } else {
            11 // Valkyrie
        };
        if idx < ALL_ROLES.len() {
            let (en_name, ftl_key, role) = ALL_ROLES[idx];
            (role, role_display_name(ftl_key, en_name, locale))
        } else {
            (Role::Valkyrie, role_display_name("role-valkyrie", "Valkyrie", locale))
        }
    };

    // 2. Race
    let races = valid_races(role);
    let race = if let Some(r) = race_arg {
        parse_race(r)
            .filter(|rc| races.contains(rc))
            .unwrap_or(races[0])
    } else if races.len() == 1 {
        races[0]
    } else {
        println!("{}", locale.translate("chargen-pick-race", None));
        for (i, r) in races.iter().enumerate() {
            println!("  {} - {}", (b'a' + i as u8) as char, r.display_name(locale));
        }
        print!("> ");
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input).ok();
        let ch = input.trim().chars().next().unwrap_or('a');
        let idx = if ch.is_ascii_lowercase() {
            (ch as u8 - b'a') as usize
        } else {
            0
        };
        if idx < races.len() {
            races[idx]
        } else {
            races[0]
        }
    };

    // 3. Alignment
    let alignments = valid_alignments(role, race);
    let alignment = if alignments.len() == 1 {
        alignments[0]
    } else {
        println!("{}", locale.translate("chargen-pick-alignment", None));
        for (i, a) in alignments.iter().enumerate() {
            println!("  {} - {}", (b'a' + i as u8) as char, a.display_name(locale));
        }
        print!("> ");
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input).ok();
        let ch = input.trim().chars().next().unwrap_or('a');
        let idx = if ch.is_ascii_lowercase() {
            (ch as u8 - b'a') as usize
        } else {
            0
        };
        if idx < alignments.len() {
            alignments[idx]
        } else {
            alignments[0]
        }
    };

    // 4. Name
    let name = if let Some(n) = name_arg {
        n.to_string()
    } else {
        let mut args = fluent::FluentArgs::new();
        args.set("default", role_name.clone());
        let prompt = locale.translate("chargen-who-are-you", Some(&args));
        print!("{} > ", prompt);
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input).ok();
        let trimmed = input.trim();
        if trimmed.is_empty() {
            role_name.clone()
        } else {
            trimmed.to_string()
        }
    };

    CharacterChoice {
        role,
        role_name,
        race,
        alignment,
        name,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_role_exact() {
        let (_, _, role) = parse_role("Valkyrie").unwrap();
        assert_eq!(role, Role::Valkyrie);
    }

    #[test]
    fn test_parse_role_case_insensitive() {
        let (_, _, role) = parse_role("wizard").unwrap();
        assert_eq!(role, Role::Wizard);
    }

    #[test]
    fn test_parse_role_prefix() {
        let (_, _, role) = parse_role("bar").unwrap();
        assert_eq!(role, Role::Barbarian);
    }

    #[test]
    fn test_parse_role_invalid() {
        assert!(parse_role("InvalidRole").is_none());
    }

    #[test]
    fn test_parse_race_exact() {
        let race = parse_race("Elf").unwrap();
        assert_eq!(race, Race::Elf);
    }

    #[test]
    fn test_parse_race_case_insensitive() {
        let race = parse_race("dwarf").unwrap();
        assert_eq!(race, Race::Dwarf);
    }

    #[test]
    fn test_parse_race_invalid() {
        assert!(parse_race("Hobbit").is_none());
    }

    #[test]
    fn test_starting_stats_differ_by_role() {
        let (bar_attrs, bar_hp, _) = starting_stats(Role::Barbarian);
        let (wiz_attrs, wiz_hp, _) = starting_stats(Role::Wizard);
        assert!(bar_attrs.strength > wiz_attrs.strength);
        assert!(wiz_attrs.intelligence > bar_attrs.intelligence);
        assert_ne!(bar_hp.max, wiz_hp.max);
    }

    #[test]
    fn test_starting_stats_barbarian_is_strong() {
        let (attrs, hp, _) = starting_stats(Role::Barbarian);
        assert_eq!(attrs.strength, 16);
        assert_eq!(hp.max, 14);
    }

    #[test]
    fn test_race_adjustments_elf() {
        let (mut attrs, _, _) = starting_stats(Role::Wizard);
        let base_int = attrs.intelligence;
        let base_con = attrs.constitution;
        apply_race_adjustments(&mut attrs, Race::Elf);
        assert_eq!(attrs.intelligence, base_int + 1);
        assert_eq!(attrs.constitution, base_con - 1);
    }

    #[test]
    fn test_race_adjustments_human_no_change() {
        let (mut attrs, _, _) = starting_stats(Role::Valkyrie);
        let before = attrs;
        apply_race_adjustments(&mut attrs, Race::Human);
        assert_eq!(attrs.strength, before.strength);
        assert_eq!(attrs.dexterity, before.dexterity);
    }

    #[test]
    fn test_valid_races_knight_human_only() {
        let races = valid_races(Role::Knight);
        assert_eq!(races, &[Race::Human]);
    }

    #[test]
    fn test_valid_races_wizard_multiple() {
        let races = valid_races(Role::Wizard);
        assert!(races.len() > 1);
        assert!(races.contains(&Race::Human));
        assert!(races.contains(&Race::Elf));
    }

    #[test]
    fn test_valid_alignments_rogue_chaotic_only() {
        let aligns = valid_alignments(Role::Rogue, Race::Human);
        assert_eq!(aligns, &[Alignment::Chaotic]);
    }

    #[test]
    fn test_valid_alignments_monk_all_three() {
        let aligns = valid_alignments(Role::Monk, Race::Human);
        assert_eq!(aligns.len(), 3);
    }

    #[test]
    fn test_all_roles_have_valid_starting_stats() {
        for (_, _, role) in ALL_ROLES {
            let (attrs, hp, pw) = starting_stats(*role);
            assert!(attrs.strength >= 3, "strength too low for {:?}", role);
            assert!(attrs.strength <= 25, "strength too high for {:?}", role);
            assert!(hp.max > 0, "hp should be positive for {:?}", role);
            assert!(pw.max > 0, "pw should be positive for {:?}", role);
        }
    }

    #[test]
    fn test_apply_character_choice() {
        use nethack_babel_engine::action::Position;
        use nethack_babel_engine::world::GameWorld;

        let mut world = GameWorld::new(Position::new(40, 10));
        let choice = CharacterChoice {
            role: Role::Barbarian,
            role_name: "Barbarian".to_string(),
            race: Race::Orc,
            alignment: Alignment::Chaotic,
            name: "Conan".to_string(),
        };

        apply_character_choice(&mut world, &choice);

        let player = world.player();
        let attrs = world
            .get_component::<Attributes>(player)
            .expect("player should have attributes");
        assert_eq!(attrs.strength, 17);

        let name = world.entity_name(player);
        assert_eq!(name, "Conan");
    }

    #[test]
    fn test_starting_inventory_not_empty() {
        use nethack_babel_engine::action::Position;
        use nethack_babel_engine::world::GameWorld;

        let mut world = GameWorld::new(Position::new(40, 10));
        let choice = CharacterChoice {
            role: Role::Tourist,
            role_name: "Tourist".to_string(),
            race: Race::Human,
            alignment: Alignment::Neutral,
            name: "Visitor".to_string(),
        };

        apply_character_choice(&mut world, &choice);

        let player = world.player();
        let hp = world
            .get_component::<HitPoints>(player)
            .expect("player should have HP");
        assert_eq!(hp.max, 8);
        assert_eq!(hp.current, 8);
    }

    #[test]
    fn test_select_character_with_all_cli_args() {
        use nethack_babel_engine::action::{Direction, Position};
        use nethack_babel_tui::{
            InputEvent, MapView, Menu, MenuResult, MessageUrgency, StatusLine, WindowPort,
        };

        struct PanicPort;
        impl WindowPort for PanicPort {
            fn init(&mut self) {}
            fn shutdown(&mut self) {}
            fn render_map(&mut self, _: &MapView, _: (i16, i16)) {}
            fn render_status(&mut self, _: &StatusLine) {}
            fn show_message(&mut self, _: &str, _: MessageUrgency) {}
            fn show_more_prompt(&mut self) -> bool { true }
            fn show_message_history(&mut self, _: &[String]) {}
            fn show_menu(&mut self, _: &Menu) -> MenuResult {
                panic!("Menu should not be shown when CLI args provide all values");
            }
            fn show_text(&mut self, _: &str, _: &str) {}
            fn get_key(&mut self) -> InputEvent { InputEvent::None }
            fn ask_direction(&mut self, _: &str) -> Option<Direction> { None }
            fn ask_position(&mut self, _: &str) -> Option<Position> { None }
            fn ask_yn(&mut self, _: &str, _: &str, d: char) -> char { d }
            fn get_line(&mut self, _: &str) -> Option<String> { None }
            fn render_tombstone(&mut self, _: &str, _: &str) {}
            fn delay(&mut self, _: u32) {}
            fn bell(&mut self) {}
        }

        let locale = LocaleManager::new();
        let mut port = PanicPort;
        let choice = select_character(
            &mut port,
            Some("Rogue"),
            Some("Human"),
            Some("Shadow"),
            &locale,
        )
        .expect("should get character choice");

        assert_eq!(choice.role, Role::Rogue);
        assert_eq!(choice.race, Race::Human);
        assert_eq!(choice.alignment, Alignment::Chaotic);
        assert_eq!(choice.name, "Shadow");
    }

    // ── Starting skills tests ──────────────────────────────────────────

    #[test]
    fn test_starting_skills_valkyrie() {
        let skills = starting_skills(Role::Valkyrie);
        assert!(skills.iter().any(|(n, _)| n == "long sword"));
        assert!(skills.iter().any(|(n, _)| n == "dagger"));
    }

    #[test]
    fn test_starting_skills_wizard() {
        let skills = starting_skills(Role::Wizard);
        assert!(skills.iter().any(|(n, l)| n == "quarterstaff" && *l == SkillLevel::Basic));
        assert!(skills.iter().any(|(n, l)| n == "attack spells" && *l == SkillLevel::Basic));
    }

    #[test]
    fn test_starting_skills_knight_has_riding() {
        let skills = starting_skills(Role::Knight);
        assert!(skills.iter().any(|(n, _)| n == "riding"));
        assert!(skills.iter().any(|(n, _)| n == "lance"));
    }

    #[test]
    fn test_starting_skills_samurai_martial_arts() {
        let skills = starting_skills(Role::Samurai);
        assert!(skills.iter().any(|(n, _)| n == "martial arts"));
        assert!(skills.iter().any(|(n, _)| n == "bow"));
    }

    #[test]
    fn test_starting_skills_rogue_dagger_skilled() {
        let skills = starting_skills(Role::Rogue);
        let dagger = skills.iter().find(|(n, _)| n == "dagger");
        assert!(dagger.is_some());
        assert_eq!(dagger.unwrap().1, SkillLevel::Skilled);
    }

    #[test]
    fn test_starting_skills_all_roles_nonempty() {
        for (_, _, role) in ALL_ROLES {
            let skills = starting_skills(*role);
            assert!(!skills.is_empty(), "role {:?} should have starting skills", role);
        }
    }

    // ── Carry capacity tests ───────────────────────────────────────────

    #[test]
    fn test_initial_carry_capacity_basic() {
        // STR 10, CON 10: 10*25 + 10*10 = 250 + 100 = 350
        assert_eq!(initial_carry_capacity(10, 10), 350);
    }

    #[test]
    fn test_initial_carry_capacity_high_str() {
        // STR 25: 1250 + CON 18*10 = 1250 + 180 = 1430
        assert_eq!(initial_carry_capacity(25, 18), 1430);
    }

    #[test]
    fn test_initial_carry_capacity_str_19() {
        // STR 19: 1000 + CON 10*10 = 1100
        assert_eq!(initial_carry_capacity(19, 10), 1100);
    }

    #[test]
    fn test_initial_carry_capacity_str_18() {
        // STR 18: 500 + 0 + CON 10*10 = 600
        assert_eq!(initial_carry_capacity(18, 10), 600);
    }
}
