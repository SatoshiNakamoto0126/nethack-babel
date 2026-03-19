mod config;
mod game_start;
#[allow(dead_code)]
mod recording;
mod save;
#[allow(dead_code)]
mod server;

use std::io::{self, BufRead, Write};
use std::path::Path;

use anyhow::{Context, Result};
use clap::Parser;
use rand::Rng;
use rand::SeedableRng;
use rand_pcg::Pcg64;

use nethack_babel_data::{
    BucStatus, Color as NhColor, GameData, MonsterDef, ObjectClass, ObjectCore, ObjectDef,
    load_game_data,
};
use nethack_babel_engine::action::{Direction, PlayerAction, Position};
use nethack_babel_engine::conduct::ConductState;
use nethack_babel_engine::dungeon::{LevelMap, Terrain};
use nethack_babel_engine::end::format_conduct_summary;
use nethack_babel_engine::equipment::EquipmentSlots;
use nethack_babel_engine::event::{DeathCause, EngineEvent};
use nethack_babel_engine::fov::FovMap;
use nethack_babel_engine::inventory::Inventory;
use nethack_babel_engine::map_gen::{Room, generate_level};
use nethack_babel_engine::role::{Race as EngineRace, Role as EngineRole};
use nethack_babel_engine::topten::{Leaderboard, LeaderboardEntry, default_leaderboard_path};
use nethack_babel_engine::turn::resolve_turn;
use nethack_babel_engine::world::{
    ArmorClass, Attributes, DisplaySymbol, Encumbrance, EncumbranceLevel, ExperienceLevel,
    GameWorld, HitPoints, Monster, MonsterIdentity, MovementPoints, NORMAL_SPEED, Name, Nutrition,
    Peaceful, Positioned, Power, Speed,
};

use nethack_babel_i18n::locale::{LanguageManifest, LocaleManager};
use nethack_babel_tui::{
    App, DisplayCell, InventoryI18n, InventoryItem, MAP_COLS, MAP_ROWS, MapView, Menu, MenuHow,
    MenuItem, MenuResult, MessageUrgency, StatusLine, TermColor, TuiMessages, TuiPort, WindowPort,
    make_inventory_item,
};

#[derive(Parser)]
#[command(
    name = "nethack-babel",
    version,
    about = "NetHack Babel - A modern Rust reimplementation of NetHack"
)]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "~/.config/nethack-babel/config.toml")]
    config: String,

    /// Language override
    #[arg(short, long)]
    language: Option<String>,

    /// Start in wizard/debug mode
    #[arg(short = 'D', long)]
    debug: bool,

    /// Specify character name
    #[arg(short = 'u', long)]
    name: Option<String>,

    /// Path to the data directory
    #[arg(long, default_value = "data")]
    data_dir: String,

    /// Use text-mode fallback instead of the ratatui TUI
    #[arg(long)]
    text: bool,

    /// Start in server mode, listening for SSH/telnet connections.
    /// Optionally specify bind address as addr:port (default: 0.0.0.0:2323).
    #[arg(long, value_name = "ADDR:PORT")]
    server: Option<Option<String>>,

    /// Record the game session to an asciinema v2 compatible file
    #[arg(long, value_name = "FILE")]
    record: Option<String>,

    /// Replay a previously recorded session
    #[arg(long, value_name = "FILE")]
    replay: Option<String>,

    /// Choose player role (e.g. Valkyrie, Wizard, Rogue)
    #[arg(long)]
    role: Option<String>,

    /// Choose player race (e.g. Human, Elf, Dwarf, Gnome, Orc)
    #[arg(long)]
    race: Option<String>,
}

// ---------------------------------------------------------------------------
// Terrain rendering
// ---------------------------------------------------------------------------

/// Map a terrain type to its ASCII display character.
fn terrain_char(terrain: Terrain) -> char {
    match terrain {
        Terrain::Stone => ' ',
        Terrain::Wall => '#',
        Terrain::Floor => '.',
        Terrain::Corridor => '#',
        Terrain::DoorOpen => '|',
        Terrain::DoorClosed => '+',
        Terrain::DoorLocked => '+',
        Terrain::StairsUp => '<',
        Terrain::StairsDown => '>',
        Terrain::Altar => '_',
        Terrain::Fountain => '{',
        Terrain::Throne => '\\',
        Terrain::Sink => '#',
        Terrain::Grave => '|',
        Terrain::Pool => '}',
        Terrain::Moat => '}',
        Terrain::Lava => '}',
        Terrain::Ice => '.',
        Terrain::Air => ' ',
        Terrain::Cloud => '#',
        Terrain::Water => '}',
        Terrain::Tree => 'T',
        Terrain::IronBars => '#',
        Terrain::Drawbridge => '.',
        Terrain::MagicPortal => '\\',
    }
}

/// Map a NetHack 16-color constant to an RGB TermColor.
fn nh_color_to_term(color: NhColor) -> TermColor {
    match color {
        NhColor::Black => TermColor::Rgb(40, 40, 40),
        NhColor::Red => TermColor::Rgb(170, 0, 0),
        NhColor::Green => TermColor::Rgb(0, 170, 0),
        NhColor::Brown => TermColor::Rgb(170, 85, 0),
        NhColor::Blue => TermColor::Rgb(0, 0, 170),
        NhColor::Magenta => TermColor::Rgb(170, 0, 170),
        NhColor::Cyan => TermColor::Rgb(0, 170, 170),
        NhColor::Gray => TermColor::Rgb(170, 170, 170),
        NhColor::NoColor => TermColor::Rgb(170, 170, 170),
        NhColor::Orange => TermColor::Rgb(255, 165, 0),
        NhColor::BrightGreen => TermColor::Rgb(85, 255, 85),
        NhColor::Yellow => TermColor::Rgb(255, 255, 85),
        NhColor::BrightBlue => TermColor::Rgb(85, 85, 255),
        NhColor::BrightMagenta => TermColor::Rgb(255, 85, 255),
        NhColor::BrightCyan => TermColor::Rgb(85, 255, 255),
        NhColor::White => TermColor::Rgb(255, 255, 255),
    }
}

/// Map terrain to a foreground TermColor for the 16-color scheme.
fn terrain_fg_color(terrain: Terrain) -> TermColor {
    match terrain {
        Terrain::Floor => TermColor::Rgb(170, 170, 170), // gray
        Terrain::Corridor => TermColor::Rgb(170, 170, 170), // gray
        Terrain::Wall => TermColor::Rgb(170, 170, 170),  // gray
        Terrain::Stone => TermColor::Rgb(170, 170, 170), // gray
        Terrain::DoorOpen => TermColor::Rgb(170, 85, 0), // brown
        Terrain::DoorClosed => TermColor::Rgb(170, 85, 0), // brown/yellow
        Terrain::DoorLocked => TermColor::Rgb(170, 85, 0), // brown/yellow
        Terrain::StairsUp => TermColor::Rgb(255, 255, 255), // white
        Terrain::StairsDown => TermColor::Rgb(255, 255, 255), // white
        Terrain::Altar => TermColor::Rgb(170, 170, 170), // gray
        Terrain::Fountain => TermColor::Rgb(0, 0, 170),  // blue
        Terrain::Throne => TermColor::Rgb(255, 255, 85), // yellow
        Terrain::Sink => TermColor::Rgb(170, 170, 170),  // gray
        Terrain::Grave => TermColor::Rgb(170, 170, 170), // gray
        Terrain::Pool => TermColor::Rgb(0, 0, 170),      // blue
        Terrain::Moat => TermColor::Rgb(0, 0, 170),      // blue
        Terrain::Water => TermColor::Rgb(0, 0, 170),     // blue
        Terrain::Lava => TermColor::Rgb(170, 0, 0),      // red
        Terrain::Ice => TermColor::Rgb(85, 255, 255),    // bright cyan
        Terrain::Air => TermColor::Rgb(85, 255, 255),    // bright cyan
        Terrain::Cloud => TermColor::Rgb(170, 170, 170), // gray
        Terrain::Tree => TermColor::Rgb(0, 170, 0),      // green
        Terrain::IronBars => TermColor::Rgb(0, 170, 170), // cyan
        Terrain::Drawbridge => TermColor::Rgb(170, 85, 0), // brown
        Terrain::MagicPortal => TermColor::Rgb(255, 85, 255), // bright magenta
    }
}

/// Dim a TermColor for remembered-but-not-visible cells.
fn dim_color(c: TermColor) -> TermColor {
    match c {
        TermColor::Rgb(r, g, b) => TermColor::Rgb(r / 3, g / 3, b / 3),
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Build MapView from world state
// ---------------------------------------------------------------------------

/// Build a `MapView` from the current world state, FOV, and entity positions.
fn build_map_view(world: &GameWorld, fov: &FovMap) -> MapView {
    let map = &world.dungeon().current_level;
    let mut view = MapView::new();

    // 1. Render terrain.
    for row in 0..map.height.min(MAP_ROWS) {
        for col in 0..map.width.min(MAP_COLS) {
            let cell = &map.cells[row][col];
            let visible = fov.is_visible(col as i32, row as i32);
            let explored = cell.explored || visible;

            if !explored {
                // Unseen: leave as space (default).
                continue;
            }

            let ch = terrain_char(cell.terrain);
            let fg = terrain_fg_color(cell.terrain);

            let (fg, bg) = if visible {
                (fg, TermColor::Rgb(0, 0, 0))
            } else {
                // Remembered but not currently visible: dimmed.
                (dim_color(fg), TermColor::Rgb(0, 0, 0))
            };

            view.set(
                col,
                row,
                DisplayCell {
                    ch,
                    fg,
                    bg,
                    bold: false,
                },
            );
        }
    }

    // 2. Overlay monsters (only if visible in FOV).
    for (_entity, (pos, _monster, dsym)) in world
        .ecs()
        .query::<(&Positioned, &Monster, &DisplaySymbol)>()
        .iter()
    {
        let x = pos.0.x as usize;
        let y = pos.0.y as usize;
        if y < MAP_ROWS && x < MAP_COLS && fov.is_visible(x as i32, y as i32) {
            let fg = nh_color_to_term(dsym.color);
            view.set(
                x,
                y,
                DisplayCell {
                    ch: dsym.symbol,
                    fg,
                    bg: TermColor::Rgb(0, 0, 0),
                    bold: false,
                },
            );
        }
    }

    // 3. Overlay the player (always visible).
    if let Some(pos) = world.get_component::<Positioned>(world.player()) {
        let x = pos.0.x as usize;
        let y = pos.0.y as usize;
        if y < MAP_ROWS && x < MAP_COLS {
            view.set(
                x,
                y,
                DisplayCell {
                    ch: '@',
                    fg: TermColor::Rgb(255, 255, 255),
                    bg: TermColor::Rgb(0, 0, 0),
                    bold: true,
                },
            );
        }
    }

    view
}

// ---------------------------------------------------------------------------
// Build StatusLine from world state
// ---------------------------------------------------------------------------

/// Get the hunger label text (empty string for NotHungry).
fn hunger_label(nutrition: i32, locale: &LocaleManager) -> String {
    let key = if nutrition > 1000 {
        "status-satiated"
    } else if nutrition > 150 {
        return String::new();
    } else if nutrition > 50 {
        "status-hungry"
    } else if nutrition > 0 {
        "status-weak"
    } else {
        "status-fainting"
    };
    locale.translate(key, None)
}

/// Build the two-line status bar from world state.
fn build_status(world: &GameWorld, locale: &LocaleManager) -> StatusLine {
    let player = world.player();

    let name = world
        .get_component::<Name>(player)
        .map(|n| n.0.clone())
        .unwrap_or_else(|| "Player".to_string());

    let attrs = world
        .get_component::<Attributes>(player)
        .map(|a| *a)
        .unwrap_or_default();

    let str_display = if attrs.strength == 18 && attrs.strength_extra > 0 {
        if attrs.strength_extra == 100 {
            "18/**".to_string()
        } else {
            format!("18/{:02}", attrs.strength_extra)
        }
    } else {
        format!("{}", attrs.strength)
    };

    let xlvl = world
        .get_component::<ExperienceLevel>(player)
        .map(|x| x.0)
        .unwrap_or(1);

    // Line 1: Name the Title  St:xx Dx:xx Co:xx In:xx Wi:xx Ch:xx
    let l_str = locale.translate("stat-label-str", None);
    let l_dx = locale.translate("stat-label-dex", None);
    let l_co = locale.translate("stat-label-con", None);
    let l_in = locale.translate("stat-label-int", None);
    let l_wi = locale.translate("stat-label-wis", None);
    let l_ch = locale.translate("stat-label-cha", None);
    let line1 = format!(
        "{name}    {l_str}:{str_display} {l_dx}:{} {l_co}:{} {l_in}:{} {l_wi}:{} {l_ch}:{}",
        attrs.dexterity, attrs.constitution, attrs.intelligence, attrs.wisdom, attrs.charisma,
    );

    // Branch-aware depth label.
    let dlvl_display = match world.dungeon().branch {
        nethack_babel_engine::dungeon::DungeonBranch::Main => {
            format!("{}", world.dungeon().depth)
        }
        nethack_babel_engine::dungeon::DungeonBranch::Mines => {
            format!("Mines:{}", world.dungeon().depth)
        }
        nethack_babel_engine::dungeon::DungeonBranch::Sokoban => {
            format!("Sokbn:{}", world.dungeon().depth)
        }
        nethack_babel_engine::dungeon::DungeonBranch::Quest => {
            format!("Quest:{}", world.dungeon().depth)
        }
        nethack_babel_engine::dungeon::DungeonBranch::Gehennom => {
            format!("Geh:{}", world.dungeon().depth)
        }
        nethack_babel_engine::dungeon::DungeonBranch::VladsTower => {
            format!("Vlad:{}", world.dungeon().depth)
        }
        nethack_babel_engine::dungeon::DungeonBranch::FortLudios => "Knox".to_string(),
        nethack_babel_engine::dungeon::DungeonBranch::Endgame => match world.dungeon().depth {
            1 => "Earth".to_string(),
            2 => "Air".to_string(),
            3 => "Fire".to_string(),
            4 => "Water".to_string(),
            5 => "Astral".to_string(),
            _ => format!("End:{}", world.dungeon().depth),
        },
    };

    let hp = world
        .get_component::<HitPoints>(player)
        .map(|h| *h)
        .unwrap_or(HitPoints { current: 0, max: 0 });
    let pw = world
        .get_component::<Power>(player)
        .map(|p| *p)
        .unwrap_or(Power { current: 0, max: 0 });
    let ac = world
        .get_component::<ArmorClass>(player)
        .map(|a| a.0)
        .unwrap_or(10);
    let turn = world.turn();
    let nutrition = world
        .get_component::<Nutrition>(player)
        .map(|n| n.0)
        .unwrap_or(900);
    let hunger = hunger_label(nutrition, locale);

    let gold = nethack_babel_engine::items::get_inventory(world, player)
        .into_iter()
        .filter(|(_, core)| core.object_class == ObjectClass::Coin)
        .map(|(_, core)| i64::from(core.quantity.max(0)))
        .sum::<i64>();

    // Status effects.
    let mut effects = Vec::new();
    {
        use nethack_babel_engine::status;
        if status::is_blind(world, player) {
            effects.push("Blind");
        }
        if status::is_confused(world, player) {
            effects.push("Conf");
        }
        if status::is_stunned(world, player) {
            effects.push("Stun");
        }
        if status::is_hallucinating(world, player) {
            effects.push("Hallu");
        }
        if status::is_levitating(world, player) {
            effects.push("Lev");
        }
        if status::is_sick(world, player) {
            effects.push("Ill");
        }
    }
    let encumbrance = world
        .get_component::<EncumbranceLevel>(player)
        .map(|e| e.0)
        .unwrap_or(Encumbrance::Unencumbered);
    if let Some(label) = match encumbrance {
        Encumbrance::Unencumbered => None,
        Encumbrance::Burdened => Some("Burdened"),
        Encumbrance::Stressed => Some("Stressed"),
        Encumbrance::Strained => Some("Strained"),
        Encumbrance::Overtaxed => Some("Overtaxed"),
        Encumbrance::Overloaded => Some("Overloaded"),
    } {
        effects.push(label);
    }

    // Line 2: Dlvl:X $:G HP:C(M) Pw:C(M) AC:A Xp:L T:T [Status]
    let l_dlvl = locale.translate("stat-label-dlvl", None);
    let l_gold = locale.translate("stat-label-gold", None);
    let l_hp = locale.translate("stat-label-hp", None);
    let l_pw = locale.translate("stat-label-pw", None);
    let l_ac = locale.translate("stat-label-ac", None);
    let l_xp = locale.translate("stat-label-xp", None);
    let l_turn = locale.translate("stat-label-turn", None);
    let mut line2 = format!(
        "{l_dlvl}:{dlvl_display} {l_gold}:{gold} {l_hp}:{}({}) {l_pw}:{}({}) {l_ac}:{ac} {l_xp}:{xlvl} {l_turn}:{turn}",
        hp.current, hp.max, pw.current, pw.max,
    );
    if !hunger.is_empty() {
        line2.push(' ');
        line2.push_str(&hunger);
    }
    for effect in &effects {
        line2.push(' ');
        line2.push_str(effect);
    }

    StatusLine { line1, line2 }
}

fn contextual_chat_target(world: &GameWorld, direction: Direction) -> Option<hecs::Entity> {
    let player = world.player();
    let player_pos = world.get_component::<Positioned>(player).map(|pos| pos.0)?;
    let target_pos = player_pos.step(direction);
    world
        .ecs()
        .query::<(&Monster, &Positioned, &HitPoints)>()
        .iter()
        .find_map(|(entity, (_, pos, hp))| {
            (pos.0 == target_pos && hp.current > 0).then_some(entity)
        })
}

fn contextual_chat_target_def(
    world: &GameWorld,
    direction: Direction,
) -> Option<(hecs::Entity, MonsterDef)> {
    let entity = contextual_chat_target(world, direction)?;
    let identity = world.get_component::<MonsterIdentity>(entity)?;
    let def = world
        .monster_catalog()
        .iter()
        .find(|def| def.id == identity.0)?
        .clone();
    Some((entity, def))
}

fn player_gold_amount(world: &GameWorld) -> i64 {
    let player = world.player();
    let Some(items) = world
        .get_component::<Inventory>(player)
        .map(|inv| inv.items.clone())
    else {
        return 0;
    };

    items
        .into_iter()
        .filter_map(|item| world.get_component::<ObjectCore>(item))
        .filter(|core| core.object_class == ObjectClass::Coin)
        .map(|core| i64::from(core.quantity.max(0)))
        .sum()
}

fn is_peaceful_oracle_chat(world: &GameWorld, direction: Direction) -> bool {
    contextual_chat_target(world, direction).is_some_and(|entity| {
        world
            .get_component::<Name>(entity)
            .is_some_and(|name| name.0.eq_ignore_ascii_case("Oracle"))
            && world.get_component::<Peaceful>(entity).is_some()
    })
}

fn is_peaceful_demon_bribe_chat(world: &GameWorld, direction: Direction) -> bool {
    contextual_chat_target_def(world, direction).is_some_and(|(entity, def)| {
        def.sound == nethack_babel_data::schema::MonsterSound::Bribe
            && world.get_component::<Peaceful>(entity).is_some()
    })
}

fn oracle_major_price(world: &GameWorld) -> i32 {
    let player = world.player();
    500 + 50
        * i32::from(
            world
                .get_component::<ExperienceLevel>(player)
                .map(|lvl| lvl.0)
                .unwrap_or(1),
        )
}

fn prompt_oracle_consultation_menu(
    port: &mut impl WindowPort,
    world: &GameWorld,
    direction: Direction,
) -> Option<PlayerAction> {
    let major_price = oracle_major_price(world);
    let menu = Menu {
        title: "Consult The Oracle".to_string(),
        items: vec![
            MenuItem {
                accelerator: 'a',
                text: "Minor consultation (50 zorkmids)".to_string(),
                selected: false,
                selectable: true,
                group: None,
            },
            MenuItem {
                accelerator: 'b',
                text: format!("Major consultation ({major_price} zorkmids)"),
                selected: false,
                selectable: true,
                group: None,
            },
            MenuItem {
                accelerator: 'c',
                text: "Cancel".to_string(),
                selected: false,
                selectable: true,
                group: None,
            },
        ],
        how: MenuHow::PickOne,
    };

    match port.show_menu(&menu) {
        MenuResult::Selected(indices) if !indices.is_empty() => match indices[0] {
            0 => Some(PlayerAction::ConsultOracle {
                direction,
                major: false,
            }),
            1 => Some(PlayerAction::ConsultOracle {
                direction,
                major: true,
            }),
            _ => None,
        },
        _ => None,
    }
}

fn prompt_tui_demon_bribe(
    port: &mut impl WindowPort,
    world: &GameWorld,
    direction: Direction,
) -> Option<PlayerAction> {
    if nethack_babel_engine::status::is_deaf(world, world.player()) {
        return Some(PlayerAction::BribeDemon {
            direction,
            amount: 0,
        });
    }

    let gold = player_gold_amount(world);
    let prompt = format!("How much will you offer? [0..{gold}] (blank to refuse, Esc to cancel)");
    let line = port.get_line(&prompt)?;
    let amount = line.trim().parse::<i64>().unwrap_or(0);
    Some(PlayerAction::BribeDemon { direction, amount })
}

fn contextualize_tui_action(
    port: &mut impl WindowPort,
    world: &GameWorld,
    action: PlayerAction,
) -> Option<PlayerAction> {
    match action {
        PlayerAction::Chat { direction } if is_peaceful_oracle_chat(world, direction) => {
            prompt_oracle_consultation_menu(port, world, direction)
        }
        PlayerAction::Chat { direction } if is_peaceful_demon_bribe_chat(world, direction) => {
            prompt_tui_demon_bribe(port, world, direction)
        }
        _ => Some(action),
    }
}

fn prompt_text_oracle_consultation(
    stdin: &io::Stdin,
    stdout: &io::Stdout,
    world: &GameWorld,
    direction: Direction,
) -> Result<Option<PlayerAction>> {
    let major_price = oracle_major_price(world);
    println!("  Consult the Oracle:");
    println!("    a - minor consultation (50 zorkmids)");
    println!("    b - major consultation ({major_price} zorkmids)");
    println!("    q - cancel");
    print!("  Choice> ");
    stdout.lock().flush()?;

    let mut line = String::new();
    let bytes_read = stdin.lock().read_line(&mut line)?;
    if bytes_read == 0 {
        return Ok(None);
    }

    let choice = line
        .trim()
        .chars()
        .next()
        .unwrap_or('q')
        .to_ascii_lowercase();
    let action = match choice {
        'a' | 'm' => Some(PlayerAction::ConsultOracle {
            direction,
            major: false,
        }),
        'b' | 'M' | 'g' => Some(PlayerAction::ConsultOracle {
            direction,
            major: true,
        }),
        _ => None,
    };
    Ok(action)
}

fn prompt_text_demon_bribe(
    stdin: &io::Stdin,
    stdout: &io::Stdout,
    world: &GameWorld,
    direction: Direction,
) -> Result<Option<PlayerAction>> {
    if nethack_babel_engine::status::is_deaf(world, world.player()) {
        return Ok(Some(PlayerAction::BribeDemon {
            direction,
            amount: 0,
        }));
    }

    let gold = player_gold_amount(world);
    println!("  How much will you offer? [0..{gold}]");
    println!("    blank or invalid input counts as refusing");
    print!("  Offer> ");
    stdout.lock().flush()?;

    let mut line = String::new();
    let bytes_read = stdin.lock().read_line(&mut line)?;
    if bytes_read == 0 {
        return Ok(None);
    }

    let trimmed = line.trim();
    let amount = trimmed.parse::<i64>().unwrap_or(0);
    Ok(Some(PlayerAction::BribeDemon { direction, amount }))
}

fn contextualize_text_action(
    stdin: &io::Stdin,
    stdout: &io::Stdout,
    world: &GameWorld,
    action: PlayerAction,
) -> Result<Option<PlayerAction>> {
    match action {
        PlayerAction::Chat { direction } if is_peaceful_oracle_chat(world, direction) => {
            prompt_text_oracle_consultation(stdin, stdout, world, direction)
        }
        PlayerAction::Chat { direction } if is_peaceful_demon_bribe_chat(world, direction) => {
            prompt_text_demon_bribe(stdin, stdout, world, direction)
        }
        _ => Ok(Some(action)),
    }
}

// ---------------------------------------------------------------------------
// Update FOV and explored flags
// ---------------------------------------------------------------------------

/// Compute FOV from the player's position and mark explored cells.
///
/// When the player is blind, FOV is restricted to radius 1 (feel
/// adjacent tiles only). If the blind player has telepathy, monster
/// positions are additionally marked as visible.
fn update_fov(world: &mut GameWorld, fov: &mut FovMap) {
    let player = world.player();
    let player_pos = world
        .get_component::<Positioned>(player)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    let is_blind = nethack_babel_engine::status::is_blind(world, player);

    if is_blind {
        // Blind: can only feel adjacent tiles (radius 1).
        fov.compute_blind(player_pos);

        // Telepathy + blind: sense monster positions through walls.
        let has_telepathy = nethack_babel_engine::status::has_intrinsic_telepathy(world, player);
        if has_telepathy {
            // Collect monster positions.
            let monster_positions: Vec<(i32, i32)> = world
                .ecs()
                .query::<(&Positioned, &nethack_babel_engine::world::Monster)>()
                .iter()
                .filter(|(e, _)| *e != player)
                .map(|(_, (p, _))| (p.0.x, p.0.y))
                .collect();
            fov.mark_positions(&monster_positions);
        }
    } else {
        let map = &world.dungeon().current_level;
        let width = map.width;
        let height = map.height;

        // We need a snapshot of the terrain to pass to the FovMap closure.
        let cells_snapshot: Vec<Vec<bool>> = map
            .cells
            .iter()
            .map(|row| row.iter().map(|c| c.terrain.is_opaque()).collect())
            .collect();

        fov.compute(player_pos, 12, |x, y| {
            if x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height {
                cells_snapshot[y as usize][x as usize]
            } else {
                true
            }
        });
    }

    // Mark newly visible cells as explored.
    let map = &mut world.dungeon_mut().current_level;
    for row in 0..map.height {
        for col in 0..map.width {
            if fov.is_visible(col as i32, row as i32) {
                map.cells[row][col].explored = true;
                map.cells[row][col].visible = true;
            } else {
                map.cells[row][col].visible = false;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Locale initialization
// ---------------------------------------------------------------------------

/// Discover all locale directories under `data_dir/locale/`, load their
/// manifests and FTL message files, then activate the requested language.
fn init_locale_manager(data_dir: &Path, language: &str) -> Result<LocaleManager> {
    let mut locale = LocaleManager::new();
    let locale_dir = data_dir.join("locale");

    if locale_dir.is_dir() {
        for entry in std::fs::read_dir(&locale_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let code = entry.file_name().to_string_lossy().to_string();

            // Read manifest.toml
            let manifest_path = entry.path().join("manifest.toml");
            if !manifest_path.exists() {
                continue;
            }
            let manifest_str = std::fs::read_to_string(&manifest_path)
                .with_context(|| format!("reading {}", manifest_path.display()))?;
            let manifest: LanguageManifest = toml::from_str(&manifest_str)
                .with_context(|| format!("parsing {}", manifest_path.display()))?;

            // Read messages.ftl
            let messages_path = entry.path().join("messages.ftl");
            if !messages_path.exists() {
                continue;
            }
            let messages_str = std::fs::read_to_string(&messages_path)
                .with_context(|| format!("reading {}", messages_path.display()))?;

            if let Err(e) = locale.load_locale(&code, manifest, &[("messages.ftl", &messages_str)])
            {
                tracing::warn!("Failed to load locale '{}': {}", code, e);
                continue;
            }

            // Load entity name translations (monsters.toml, objects.toml).
            // Either file is optional; default to empty translations if missing.
            let monsters_path = entry.path().join("monsters.toml");
            let objects_path = entry.path().join("objects.toml");
            if monsters_path.exists() || objects_path.exists() {
                let monsters_str = if monsters_path.exists() {
                    std::fs::read_to_string(&monsters_path)
                        .with_context(|| format!("reading {}", monsters_path.display()))?
                } else {
                    "[translations]\n".to_string()
                };
                let objects_str = if objects_path.exists() {
                    std::fs::read_to_string(&objects_path)
                        .with_context(|| format!("reading {}", objects_path.display()))?
                } else {
                    "[translations]\n".to_string()
                };
                if let Err(e) = locale.load_entity_translations(&code, &monsters_str, &objects_str)
                {
                    tracing::warn!("Failed to load entity translations for '{}': {}", code, e);
                }
            }

            // Load classifier (classifiers.toml).
            let clf_path = entry.path().join("classifiers.toml");
            if clf_path.exists() {
                let clf_str = std::fs::read_to_string(&clf_path)
                    .with_context(|| format!("reading {}", clf_path.display()))?;
                if let Err(e) = locale.load_classifier(&clf_str) {
                    tracing::warn!("Failed to load classifiers for '{}': {}", code, e);
                }
            }
        }
    }

    // Normalize: CLI may use "zh_CN" but locale dirs use "zh-CN".
    let normalized = language.replace('_', "-");
    if locale.set_language(&normalized).is_err() {
        tracing::info!(
            "Language '{}' not available, falling back to English",
            normalized
        );
        let _ = locale.set_language("en");
    }

    Ok(locale)
}

// ---------------------------------------------------------------------------
// Event-to-message conversion
// ---------------------------------------------------------------------------

/// Convert engine events to display messages with urgency, using the locale
/// for translatable strings and the world for entity name lookups.
fn events_to_messages(
    events: &[EngineEvent],
    locale: &LocaleManager,
    world: &GameWorld,
) -> Vec<(String, MessageUrgency)> {
    use fluent::FluentArgs;

    let mut messages = Vec::new();
    for event in events {
        match event {
            EngineEvent::Message { key, args } => {
                let mut fluent_args = FluentArgs::new();
                for (k, v) in args {
                    fluent_args.set(k.clone(), v.clone());
                }
                let translated = locale.translate(key, Some(&fluent_args));
                // If translation returns the key itself, use it as-is
                messages.push((translated, MessageUrgency::Normal));
            }
            EngineEvent::HpChange { amount, .. } => {
                if *amount > 0 {
                    let mut args = FluentArgs::new();
                    args.set("amount", *amount as i64);
                    let msg = locale.translate("event-hp-gained", Some(&args));
                    let text = format!("{} (+{amount})", msg);
                    messages.push((text, MessageUrgency::Healing));
                } else if *amount < 0 {
                    let mut args = FluentArgs::new();
                    args.set("amount", (*amount as i64).abs());
                    let msg = locale.translate("event-hp-lost", Some(&args));
                    let text = format!("{} ({amount} hp)", msg);
                    messages.push((text, MessageUrgency::Damage));
                }
            }
            EngineEvent::PwChange { amount, .. } if *amount > 0 => {
                let mut args = FluentArgs::new();
                args.set("amount", *amount as i64);
                let msg = locale.translate("event-pw-gained", Some(&args));
                messages.push((format!("{} (+{amount})", msg), MessageUrgency::Healing));
            }
            EngineEvent::HungerChange { new_level, .. } => {
                let urgency = match new_level {
                    nethack_babel_engine::event::HungerLevel::Weak
                    | nethack_babel_engine::event::HungerLevel::Fainting
                    | nethack_babel_engine::event::HungerLevel::Fainted
                    | nethack_babel_engine::event::HungerLevel::Starved => MessageUrgency::Danger,
                    _ => MessageUrgency::Normal,
                };
                // Map HungerLevel to FTL key.
                let key = match new_level {
                    nethack_babel_engine::event::HungerLevel::Satiated => "hunger-satiated",
                    nethack_babel_engine::event::HungerLevel::NotHungry => "hunger-not-hungry",
                    nethack_babel_engine::event::HungerLevel::Hungry => "hunger-hungry",
                    nethack_babel_engine::event::HungerLevel::Weak => "hunger-weak",
                    nethack_babel_engine::event::HungerLevel::Fainting => "hunger-fainting",
                    nethack_babel_engine::event::HungerLevel::Fainted => "hunger-fainting",
                    nethack_babel_engine::event::HungerLevel::Starved => "hunger-starved",
                };
                let text = locale.translate(key, None);
                messages.push((text, urgency));
            }
            EngineEvent::DoorOpened { .. } => {
                let text = locale.translate("door-opened", None);
                messages.push((text, MessageUrgency::Normal));
            }
            EngineEvent::DoorClosed { .. } => {
                let text = locale.translate("door-closed", None);
                messages.push((text, MessageUrgency::Normal));
            }
            EngineEvent::DoorBroken { .. } => {
                let text = locale.translate("door-broken", None);
                messages.push((text, MessageUrgency::Normal));
            }
            EngineEvent::MeleeHit {
                attacker,
                defender,
                weapon,
                damage,
            } => {
                let att_name = entity_display_name(*attacker, world, locale);
                let def_name = entity_display_name(*defender, world, locale);
                match weapon {
                    Some(w) => {
                        let wpn_en = world.entity_name(*w);
                        let wpn_name = locale.translate_object_name(&wpn_en).to_string();
                        let mut args = FluentArgs::new();
                        args.set("attacker", att_name.clone());
                        args.set("defender", def_name.clone());
                        args.set("weapon", wpn_name.clone());
                        let msg = locale.translate("melee-hit-weapon", Some(&args));
                        let text = format!("{} ({damage})", msg);
                        messages.push((text, MessageUrgency::Normal));
                    }
                    None => {
                        let mut args = FluentArgs::new();
                        args.set("attacker", att_name.clone());
                        args.set("defender", def_name.clone());
                        let msg = locale.translate("melee-hit-bare", Some(&args));
                        let text = format!("{} ({damage})", msg);
                        messages.push((text, MessageUrgency::Normal));
                    }
                }
            }
            EngineEvent::MeleeMiss {
                attacker, defender, ..
            } => {
                let att_name = entity_display_name(*attacker, world, locale);
                let def_name = entity_display_name(*defender, world, locale);
                let mut args = FluentArgs::new();
                args.set("attacker", att_name.clone());
                args.set("defender", def_name.clone());
                let text = locale.translate("melee-miss", Some(&args));
                messages.push((text, MessageUrgency::Normal));
            }
            EngineEvent::EntityDied { entity, .. } => {
                let name = entity_display_name(*entity, world, locale);
                let mut args = FluentArgs::new();
                args.set("entity", name.clone());
                let text = locale.translate("entity-killed", Some(&args));
                messages.push((text, MessageUrgency::Normal));
            }
            EngineEvent::LevelUp { new_level, .. } => {
                let mut args = FluentArgs::new();
                args.set("level", *new_level as i64);
                let text = locale.translate("level-up", Some(&args));
                messages.push((text, MessageUrgency::System));
            }
            EngineEvent::ItemPickedUp {
                actor,
                item,
                quantity,
            } => {
                let actor_name = entity_display_name(*actor, world, locale);
                let item_en = world.entity_name(*item);
                let item_name = locale.translate_object_name(&item_en).to_string();
                let mut args = FluentArgs::new();
                args.set("actor", actor_name.clone());
                args.set("item", item_name.clone());
                args.set("quantity", *quantity as i64);
                let text = locale.translate("item-picked-up", Some(&args));
                messages.push((text, MessageUrgency::Normal));
            }
            EngineEvent::ItemDropped { actor, item } => {
                let actor_name = entity_display_name(*actor, world, locale);
                let item_en = world.entity_name(*item);
                let item_name = locale.translate_object_name(&item_en).to_string();
                let mut args = FluentArgs::new();
                args.set("actor", actor_name.clone());
                args.set("item", item_name.clone());
                let text = locale.translate("item-dropped", Some(&args));
                messages.push((text, MessageUrgency::Normal));
            }
            EngineEvent::ItemWielded { actor, item } => {
                let actor_name = entity_display_name(*actor, world, locale);
                let item_en = world.entity_name(*item);
                let item_name = locale.translate_object_name(&item_en).to_string();
                let mut args = FluentArgs::new();
                args.set("actor", actor_name.clone());
                args.set("item", item_name.clone());
                let text = locale.translate("item-wielded", Some(&args));
                messages.push((text, MessageUrgency::Normal));
            }
            EngineEvent::ItemWorn { actor, item } => {
                let actor_name = entity_display_name(*actor, world, locale);
                let item_en = world.entity_name(*item);
                let item_name = locale.translate_object_name(&item_en).to_string();
                let mut args = FluentArgs::new();
                args.set("actor", actor_name.clone());
                args.set("item", item_name.clone());
                let text = locale.translate("item-worn", Some(&args));
                messages.push((text, MessageUrgency::Normal));
            }
            EngineEvent::ItemRemoved { actor, item } => {
                let actor_name = entity_display_name(*actor, world, locale);
                let item_en = world.entity_name(*item);
                let item_name = locale.translate_object_name(&item_en).to_string();
                let mut args = FluentArgs::new();
                args.set("actor", actor_name.clone());
                args.set("item", item_name.clone());
                let text = locale.translate("item-removed", Some(&args));
                messages.push((text, MessageUrgency::Normal));
            }
            EngineEvent::GameOver { score, .. } => {
                let mut args = FluentArgs::new();
                args.set("score", *score as i64);
                let text = locale.translate("game-over", Some(&args));
                messages.push((text, MessageUrgency::Danger));
            }
            // Silent events.
            EngineEvent::EntityMoved { .. } | EngineEvent::TurnEnd { .. } => {}
            _ => {}
        }
    }
    messages
}

/// Display name for an entity in the events_to_messages context.
/// Returns "You" (or translated equivalent) for the player, and a
/// translated monster name for non-player entities.
fn entity_display_name(entity: hecs::Entity, world: &GameWorld, locale: &LocaleManager) -> String {
    if world.is_player(entity) {
        locale.translate("you", None)
    } else {
        let en_name = world.entity_name(entity);
        locale.translate_monster_name(&en_name).to_string()
    }
}

// ---------------------------------------------------------------------------
// End-game: tombstone, disclosure, leaderboard
// ---------------------------------------------------------------------------

/// Handle full game-over sequence: tombstone, disclosure, leaderboard.
fn handle_game_over(world: &GameWorld, port: &mut TuiPort, cause: &DeathCause, score: u64) {
    let player = world.player();
    let player_name = world.entity_name(player);

    let level = world
        .get_component::<ExperienceLevel>(player)
        .map(|l| l.0)
        .unwrap_or(1);

    let (hp, maxhp) = world
        .get_component::<HitPoints>(player)
        .map(|h| (h.current, h.max))
        .unwrap_or((0, 0));

    let turns = world.turn();

    let death_reason = match cause {
        DeathCause::KilledBy { killer_name } => format!("killed by {}", killer_name),
        DeathCause::Starvation => "starved to death".to_string(),
        DeathCause::Poisoning => "died of poisoning".to_string(),
        DeathCause::Petrification => "turned to stone".to_string(),
        DeathCause::Drowning => "drowned".to_string(),
        DeathCause::Burning => "burned to death".to_string(),
        DeathCause::Disintegration => "disintegrated".to_string(),
        DeathCause::Sickness => "died of sickness".to_string(),
        DeathCause::Strangulation => "strangled".to_string(),
        DeathCause::Falling => "fell to death".to_string(),
        DeathCause::CrushedByBoulder => "crushed by a boulder".to_string(),
        DeathCause::Quit => "quit".to_string(),
        DeathCause::Escaped => "escaped".to_string(),
        DeathCause::Ascended => "ascended".to_string(),
        DeathCause::Trickery => "died by trickery".to_string(),
    };

    // -- Tombstone --
    let epitaph = format!("{}, level {} adventurer", player_name, level);
    let info = format!(
        "{} -- Score: {} -- Turns: {} -- HP: {}/{}",
        death_reason, score, turns, hp, maxhp
    );
    port.render_tombstone(&epitaph, &info);

    // -- Disclosure: conducts --
    let conducts = world
        .get_component::<ConductState>(player)
        .map(|state| (*state).clone())
        .unwrap_or_default();
    let conduct_lines = format_conduct_summary(&conducts);
    let conduct_text = conduct_lines.join("\n");
    port.show_text("Voluntary challenges", &conduct_text);

    // -- Leaderboard --
    let (role, race, gender, alignment) = player_identity_strings(world, player);
    let lb_path = default_leaderboard_path();
    let mut lb = Leaderboard::load(&lb_path);
    let entry = LeaderboardEntry {
        rank: 0,
        score: score as i64,
        player_name: player_name.clone(),
        role,
        race,
        gender,
        alignment,
        death_cause: death_reason,
        dungeon_level: format!("Dlvl:{}", level),
        experience_level: level as u32,
        turns: turns as u64,
        timestamp: chrono_timestamp(),
    };
    lb.add_entry(entry);
    let _ = lb.save(&lb_path);

    // Display top 10.
    let top_lines = lb.format_top(10);
    let top_text = top_lines.join("\n");
    port.show_text("Top Scores", &top_text);
}

fn player_identity_strings(
    world: &GameWorld,
    player: hecs::Entity,
) -> (String, String, String, String) {
    let Some(identity) = world.get_component::<nethack_babel_data::PlayerIdentity>(player) else {
        return (
            "Adventurer".to_string(),
            "Human".to_string(),
            "male".to_string(),
            "neutral".to_string(),
        );
    };

    let role = EngineRole::from_id(identity.role)
        .map(|r| r.name().to_string())
        .unwrap_or_else(|| "Adventurer".to_string());
    let race = EngineRace::from_id(identity.race)
        .map(|r| r.name().to_string())
        .unwrap_or_else(|| "Human".to_string());
    let gender = match identity.gender {
        nethack_babel_data::Gender::Male => "male",
        nethack_babel_data::Gender::Female => "female",
        nethack_babel_data::Gender::Neuter => "neuter",
    }
    .to_string();
    let alignment = match identity.alignment {
        nethack_babel_data::Alignment::Lawful => "lawful",
        nethack_babel_data::Alignment::Neutral => "neutral",
        nethack_babel_data::Alignment::Chaotic => "chaotic",
    }
    .to_string();

    (role, race, gender, alignment)
}

/// Simple timestamp without requiring chrono crate.
fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Convert to YYYYMMDD-style string.
    // Simple approach: just use the unix timestamp as a string.
    format!("{}", secs)
}

// ---------------------------------------------------------------------------
// Monster spawning
// ---------------------------------------------------------------------------

/// Pick random monsters appropriate for the given depth and spawn them as
/// entities in the world. Places them in rooms other than the player's
/// starting room.
fn spawn_monsters(
    world: &mut GameWorld,
    data: &GameData,
    rooms: &[Room],
    player_room_idx: Option<usize>,
    depth: u8,
    rng: &mut impl Rng,
) {
    // Filter monsters appropriate for this depth.
    let eligible: Vec<&MonsterDef> = data
        .monsters
        .iter()
        .filter(|m| m.base_level >= 0 && m.base_level <= (depth as i8 + 2))
        .collect();

    if eligible.is_empty() || rooms.is_empty() {
        return;
    }

    let monster_count = rng.random_range(3u32..=8u32).min(rooms.len() as u32 * 2);

    for _ in 0..monster_count {
        // Pick a room that is not the player's starting room.
        let room_idx = if rooms.len() > 1 {
            loop {
                let idx = rng.random_range(0..rooms.len());
                if Some(idx) != player_room_idx {
                    break idx;
                }
            }
        } else {
            0
        };

        let room = &rooms[room_idx];

        // Pick a random position inside the room.
        let x = rng.random_range(room.x..=room.right()) as i32;
        let y = rng.random_range(room.y..=room.bottom()) as i32;
        let pos = Position::new(x, y);

        // Pick a random eligible monster.
        let mon_idx = rng.random_range(0..eligible.len());
        let mon_def = eligible[mon_idx];

        // Spawn the entity with appropriate components.
        let base_hp = (mon_def.base_level as i32).max(1) * 2 + rng.random_range(0..4);
        let speed = (mon_def.speed as u32).max(1);

        world.spawn((
            Monster,
            Positioned(pos),
            HitPoints {
                current: base_hp,
                max: base_hp,
            },
            Speed(speed),
            MovementPoints(NORMAL_SPEED as i32),
            Name(mon_def.names.male.clone()),
            DisplaySymbol {
                symbol: mon_def.symbol,
                color: mon_def.color,
            },
        ));
    }
}

/// Find which room index contains the given position, if any.
fn find_room_containing(rooms: &[Room], pos: Position) -> Option<usize> {
    rooms
        .iter()
        .position(|r| r.contains(pos.x as usize, pos.y as usize))
}

// ---------------------------------------------------------------------------
// ASCII rendering (text-mode fallback)
// ---------------------------------------------------------------------------

/// Render the current map state to stdout with entities overlaid.
fn render_map_ascii(world: &GameWorld) {
    let map = &world.dungeon().current_level;

    // Build a character grid from terrain.
    let mut grid: Vec<Vec<char>> = Vec::with_capacity(map.height);
    for y in 0..map.height {
        let mut row = Vec::with_capacity(map.width);
        for x in 0..map.width {
            let ch = terrain_char(map.cells[y][x].terrain);
            row.push(ch);
        }
        grid.push(row);
    }

    // Overlay monsters.
    for (_entity, (pos, name, _monster)) in
        world.ecs().query::<(&Positioned, &Name, &Monster)>().iter()
    {
        let x = pos.0.x as usize;
        let y = pos.0.y as usize;
        if y < map.height && x < map.width {
            let ch = name.0.chars().next().unwrap_or('M');
            grid[y][x] = ch;
        }
    }

    // Overlay the player.
    if let Some(pos) = world.get_component::<Positioned>(world.player()) {
        let x = pos.0.x as usize;
        let y = pos.0.y as usize;
        if y < map.height && x < map.width {
            grid[y][x] = '@';
        }
    }

    // Print the grid.
    for row in &grid {
        let line: String = row.iter().collect();
        println!("{}", line);
    }
}

/// Render the status bar in text mode.
fn render_status_text(world: &GameWorld) {
    let player = world.player();
    let hp = world
        .get_component::<HitPoints>(player)
        .map(|h| format!("HP:{}/{}", h.current, h.max))
        .unwrap_or_else(|| "HP:??".to_string());

    let pos = world
        .get_component::<Positioned>(player)
        .map(|p| format!("({},{})", p.0.x, p.0.y))
        .unwrap_or_else(|| "(?,?)".to_string());

    let depth = world.dungeon().depth;
    let turn = world.turn();

    println!(
        "Dlvl:{depth}  {hp}  T:{turn}  Pos:{pos}  [hjklyubn=move .=wait <=up >=down q=quit ?=help]"
    );
}

/// Display events as text messages.
fn display_events_text(events: &[EngineEvent], locale: &LocaleManager, world: &GameWorld) {
    for (text, _) in events_to_messages(events, locale, world) {
        println!("  {text}");
    }
}

// ---------------------------------------------------------------------------
// Input parsing (text mode)
// ---------------------------------------------------------------------------

fn parse_direction_token(token: &str) -> Option<Direction> {
    match token.trim().to_ascii_lowercase().as_str() {
        "h" | "w" | "west" => Some(Direction::West),
        "j" | "s" | "south" => Some(Direction::South),
        "k" | "n" | "north" => Some(Direction::North),
        "l" | "e" | "east" => Some(Direction::East),
        "y" | "nw" | "northwest" | "north-west" => Some(Direction::NorthWest),
        "u" | "ne" | "northeast" | "north-east" => Some(Direction::NorthEast),
        "b" | "sw" | "southwest" | "south-west" => Some(Direction::SouthWest),
        "m" | "se" | "southeast" | "south-east" => Some(Direction::SouthEast),
        "." | "self" | "here" => Some(Direction::Self_),
        "<" | "up" => Some(Direction::Up),
        ">" | "down" => Some(Direction::Down),
        _ => None,
    }
}

fn parse_inventory_letter_token(token: &str) -> Option<char> {
    token
        .trim()
        .chars()
        .find(|c| c.is_ascii_alphabetic())
        .map(|c| {
            if c.is_ascii_lowercase() {
                c
            } else {
                c.to_ascii_uppercase()
            }
        })
}

fn parse_position_tokens(x_token: &str, y_token: &str) -> Option<Position> {
    let x = x_token.trim().parse::<i32>().ok()?;
    let y = y_token.trim().parse::<i32>().ok()?;
    Some(Position::new(x, y))
}

fn parse_spell_token(token: &str) -> Option<nethack_babel_engine::action::SpellId> {
    let ch = token
        .trim()
        .chars()
        .find(|c| c.is_ascii_alphabetic())?
        .to_ascii_lowercase();
    Some(nethack_babel_engine::action::SpellId(
        (ch as u8).saturating_sub(b'a'),
    ))
}

fn parse_text_mode_contextual_command(input: &str, world: &GameWorld) -> Option<PlayerAction> {
    let trimmed = input.trim();
    let lower_trimmed = trimmed.to_ascii_lowercase();

    if lower_trimmed.starts_with("annotate ") {
        let text = trimmed[9..].trim();
        if !text.is_empty() {
            return Some(PlayerAction::Annotate {
                text: text.to_string(),
            });
        }
    }
    if lower_trimmed.starts_with("engrave ") {
        let text = trimmed[8..].trim();
        if !text.is_empty() {
            return Some(PlayerAction::Engrave {
                text: text.to_string(),
            });
        }
    }
    if lower_trimmed.starts_with("call ") {
        let rest = trimmed[5..].trim();
        let mut rest_parts = rest.splitn(2, ' ');
        let class = rest_parts
            .next()
            .and_then(|s| s.chars().find(|c| c.is_ascii_graphic()));
        let name = rest_parts.next().map(str::trim).filter(|s| !s.is_empty());
        if let (Some(class), Some(name)) = (class, name) {
            return Some(PlayerAction::CallType {
                class,
                name: name.to_string(),
            });
        }
    }
    if lower_trimmed.starts_with("name level ") {
        let name = trimmed[11..].trim();
        if !name.is_empty() {
            return Some(PlayerAction::Name {
                target: nethack_babel_engine::action::NameTarget::Level,
                name: name.to_string(),
            });
        }
    }
    if lower_trimmed.starts_with("name item ") {
        let rest = trimmed[10..].trim();
        let mut rest_parts = rest.splitn(2, ' ');
        let item = rest_parts.next().and_then(|tok| {
            let letter = parse_inventory_letter_token(tok)?;
            nethack_babel_engine::inventory::find_by_letter(world, world.player(), letter)
        });
        let name = rest_parts.next().map(str::trim).filter(|s| !s.is_empty());
        if let (Some(item), Some(name)) = (item, name) {
            return Some(PlayerAction::Name {
                target: nethack_babel_engine::action::NameTarget::Item { item },
                name: name.to_string(),
            });
        }
    }
    if lower_trimmed.starts_with("name monster ") {
        let rest = trimmed[13..].trim();
        let mut rest_parts = rest.splitn(3, ' ');
        let x_token = rest_parts.next();
        let y_token = rest_parts.next();
        let name = rest_parts.next().map(str::trim).filter(|s| !s.is_empty());
        if let (Some(x_token), Some(y_token), Some(name)) = (x_token, y_token, name)
            && let Some(position) = parse_position_tokens(x_token, y_token)
        {
            return Some(PlayerAction::Name {
                target: nethack_babel_engine::action::NameTarget::MonsterAt { position },
                name: name.to_string(),
            });
        }
    }

    let mut parts = trimmed.split_whitespace();
    let cmd = parts.next()?.to_ascii_lowercase();
    let player = world.player();

    let item_by_token = |token: &str| {
        let letter = parse_inventory_letter_token(token)?;
        nethack_babel_engine::inventory::find_by_letter(world, player, letter)
    };

    match cmd.as_str() {
        "drop" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Drop { item })
        }
        "wield" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Wield { item })
        }
        "wear" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Wear { item })
        }
        "takeoff" | "take-off" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::TakeOff { item })
        }
        "puton" | "put-on" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::PutOn { item })
        }
        "remove" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Remove { item })
        }
        "apply" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Apply { item })
        }
        "rub" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Rub { item })
        }
        "tip" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Tip { item })
        }
        "invoke" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::InvokeArtifact { item })
        }
        "offer" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Offer { item: Some(item) })
        }
        "force" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::ForceLock { item })
        }
        "adjust" => {
            let item = parts.next().and_then(item_by_token)?;
            let new_letter = parts.next().and_then(parse_inventory_letter_token)?;
            Some(PlayerAction::Adjust { item, new_letter })
        }
        "eat" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Eat { item: Some(item) })
        }
        "quaff" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Quaff { item: Some(item) })
        }
        "read" => {
            let item = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Read { item: Some(item) })
        }
        "dip" => {
            let item = parts.next().and_then(item_by_token)?;
            let into = parts.next().and_then(item_by_token)?;
            Some(PlayerAction::Dip { item, into })
        }
        "throw" => {
            let item = parts.next().and_then(item_by_token)?;
            let direction = parts.next().and_then(parse_direction_token)?;
            Some(PlayerAction::Throw { item, direction })
        }
        "zap" => {
            let item = parts.next().and_then(item_by_token)?;
            let direction = parts.next().and_then(parse_direction_token);
            Some(PlayerAction::ZapWand { item, direction })
        }
        "cast" => {
            let spell = parts.next().and_then(parse_spell_token)?;
            let direction = parts.next().and_then(parse_direction_token);
            Some(PlayerAction::CastSpell { spell, direction })
        }
        "open" => {
            let direction = parts.next().and_then(parse_direction_token)?;
            Some(PlayerAction::Open { direction })
        }
        "close" => {
            let direction = parts.next().and_then(parse_direction_token)?;
            Some(PlayerAction::Close { direction })
        }
        "kick" => {
            let direction = parts.next().and_then(parse_direction_token)?;
            Some(PlayerAction::Kick { direction })
        }
        "chat" => {
            let direction = parts.next().and_then(parse_direction_token)?;
            Some(PlayerAction::Chat { direction })
        }
        "fight" => {
            let direction = parts.next().and_then(parse_direction_token)?;
            Some(PlayerAction::FightDirection { direction })
        }
        "run" => {
            let direction = parts.next().and_then(parse_direction_token)?;
            Some(PlayerAction::RunDirection { direction })
        }
        "rush" => {
            let direction = parts.next().and_then(parse_direction_token)?;
            Some(PlayerAction::RushDirection { direction })
        }
        "untrap" => {
            let direction = parts.next().and_then(parse_direction_token)?;
            Some(PlayerAction::Untrap { direction })
        }
        "travel" | "retravel" => {
            let x_token = parts.next()?;
            let y_token = parts.next()?;
            let destination = parse_position_tokens(x_token, y_token)?;
            Some(PlayerAction::Travel { destination })
        }
        "jump" => {
            let x_token = parts.next()?;
            let y_token = parts.next()?;
            let position = parse_position_tokens(x_token, y_token)?;
            Some(PlayerAction::Jump { position })
        }
        "lookat" | "glance" => {
            let x_token = parts.next()?;
            let y_token = parts.next()?;
            let position = parse_position_tokens(x_token, y_token)?;
            Some(PlayerAction::LookAt { position })
        }
        "whatis" | "showtrap" => {
            let x_token = parts.next()?;
            let y_token = parts.next()?;
            let position = parse_position_tokens(x_token, y_token)?;
            Some(PlayerAction::WhatIs {
                position: Some(position),
            })
        }
        "knownclass" => {
            let class = parts
                .next()
                .and_then(|s| s.chars().find(|c| c.is_ascii_graphic()))?;
            Some(PlayerAction::KnownClass { class })
        }
        _ => None,
    }
}

/// Parse a text-mode command from stdin into a `PlayerAction`.
fn parse_command(input: &str, wizard_mode: bool) -> Option<PlayerAction> {
    let mut trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix('#') {
        trimmed = rest.trim_start();
    }
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if wizard_mode {
        if lower.starts_with("wizwish ") {
            let wish_text = trimmed[8..].trim();
            if !wish_text.is_empty() {
                return Some(PlayerAction::WizWish {
                    wish_text: wish_text.to_string(),
                });
            }
        }
        if lower.starts_with("wizgenesis ") {
            let monster_name = trimmed[11..].trim();
            if !monster_name.is_empty() {
                return Some(PlayerAction::WizGenesis {
                    monster_name: monster_name.to_string(),
                });
            }
        }
        if lower.starts_with("wizlevelport ") {
            let depth_text = trimmed[13..].trim();
            if let Ok(depth) = depth_text.parse::<i32>() {
                return Some(PlayerAction::WizLevelTeleport { depth });
            }
        }
    }

    match lower.as_str() {
        "h" => Some(PlayerAction::Move {
            direction: Direction::West,
        }),
        "j" => Some(PlayerAction::Move {
            direction: Direction::South,
        }),
        "k" => Some(PlayerAction::Move {
            direction: Direction::North,
        }),
        "l" => Some(PlayerAction::Move {
            direction: Direction::East,
        }),
        "y" => Some(PlayerAction::Move {
            direction: Direction::NorthWest,
        }),
        "u" => Some(PlayerAction::Move {
            direction: Direction::NorthEast,
        }),
        "b" => Some(PlayerAction::Move {
            direction: Direction::SouthWest,
        }),
        "n" => Some(PlayerAction::Move {
            direction: Direction::SouthEast,
        }),
        "." | "wait" | "rest" => Some(PlayerAction::Rest),
        "s" | "search" => Some(PlayerAction::Search),
        "<" | "up" => Some(PlayerAction::GoUp),
        ">" | "down" => Some(PlayerAction::GoDown),
        "," | "pickup" | "pick" => Some(PlayerAction::PickUp),
        "i" | "inv" | "inventory" => Some(PlayerAction::ViewInventory),
        "eq" | "equip" | "equipment" => Some(PlayerAction::ViewEquipped),
        "p" | "pray" => Some(PlayerAction::Pray),
        "eat" => Some(PlayerAction::Eat { item: None }),
        "quaff" | "drink" => Some(PlayerAction::Quaff { item: None }),
        "read" => Some(PlayerAction::Read { item: None }),
        "offer" => Some(PlayerAction::Offer { item: None }),
        "look" | "lookhere" | ":" => Some(PlayerAction::LookHere),
        "whatis" => Some(PlayerAction::WhatIs { position: None }),
        "discoveries" | "discover" | "known" | "genocided" => Some(PlayerAction::ViewDiscoveries),
        "conduct" => Some(PlayerAction::ViewConduct),
        "vanquished" => Some(PlayerAction::Vanquished),
        "chronicle" => Some(PlayerAction::Chronicle),
        "overview" | "dungeonoverview" => Some(PlayerAction::DungeonOverview),
        "terrain" => Some(PlayerAction::ViewTerrain),
        "attributes" | "attr" => Some(PlayerAction::Attributes),
        "options" | "optionsfull" | "saveoptions" | "autopickup" => Some(PlayerAction::Options),
        "ride" => Some(PlayerAction::Ride),
        "swap" => Some(PlayerAction::Swap),
        "wipe" => Some(PlayerAction::Wipe),
        "turnundead" | "turn-undead" | "turn" => Some(PlayerAction::TurnUndead),
        "enhance" | "enhanceskill" => Some(PlayerAction::EnhanceSkill),
        "fire" => Some(PlayerAction::Fire),
        "sit" => Some(PlayerAction::Sit),
        "loot" => Some(PlayerAction::Loot),
        "pay" => Some(PlayerAction::Pay),
        "monster" => Some(PlayerAction::Monster),
        "takeoffall" | "take-off-all" => Some(PlayerAction::TakeOffAll),
        "twoweapon" | "two-weapon" | "2weapon" => Some(PlayerAction::ToggleTwoWeapon),
        "save" => Some(PlayerAction::Save),
        "savequit" | "save-and-quit" | "saveandquit" => Some(PlayerAction::SaveAndQuit),
        "inventtype" | "showspells" | "seeall" | "seearmor" | "seeamulet" | "seerings"
        | "seetools" | "seeweapon" => Some(PlayerAction::ViewEquipped),
        "showgold" => Some(PlayerAction::ViewInventory),
        "redraw" => Some(PlayerAction::Redraw),
        "version" | "versionshort" | "v" => Some(PlayerAction::ShowVersion),
        "whatdoes" => Some(PlayerAction::Help),
        "history" => Some(PlayerAction::ShowHistory),
        "help" | "?" => Some(PlayerAction::Help),
        "wizidentify" if wizard_mode => Some(PlayerAction::WizIdentify),
        "wizmap" if wizard_mode => Some(PlayerAction::WizMap),
        "wizdetect" if wizard_mode => Some(PlayerAction::WizDetect),
        "wizwhere" if wizard_mode => Some(PlayerAction::WizWhere),
        "wizkill" if wizard_mode => Some(PlayerAction::WizKill),
        _ => None,
    }
}

fn is_repeatable_text_action(action: &PlayerAction) -> bool {
    !matches!(
        action,
        PlayerAction::ViewInventory
            | PlayerAction::ViewEquipped
            | PlayerAction::ViewDiscoveries
            | PlayerAction::ViewConduct
            | PlayerAction::DungeonOverview
            | PlayerAction::ViewTerrain
            | PlayerAction::ShowVersion
            | PlayerAction::Attributes
            | PlayerAction::LookAt { .. }
            | PlayerAction::LookHere
            | PlayerAction::Help
            | PlayerAction::ShowHistory
            | PlayerAction::KnownItems
            | PlayerAction::KnownClass { .. }
            | PlayerAction::Vanquished
            | PlayerAction::Chronicle
            | PlayerAction::Glance { .. }
            | PlayerAction::Redraw
            | PlayerAction::WhatIs { .. }
            | PlayerAction::Options
            | PlayerAction::Save
            | PlayerAction::Quit
            | PlayerAction::SaveAndQuit
    )
}

#[cfg(test)]
mod text_input_tests {
    use super::*;
    use nethack_babel_data::{KnowledgeState, ObjectLocation, ObjectTypeId};

    fn spawn_inventory_item(world: &mut GameWorld, letter: char) -> hecs::Entity {
        world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(0),
                object_class: ObjectClass::Weapon,
                quantity: 1,
                weight: 10,
                age: 0,
                inv_letter: Some(letter),
                artifact: None,
            },
            BucStatus {
                cursed: false,
                blessed: false,
                bknown: false,
            },
            KnowledgeState {
                known: false,
                dknown: false,
                rknown: false,
                cknown: false,
                lknown: false,
                tknown: false,
            },
            ObjectLocation::Inventory,
        ))
    }

    #[test]
    fn parse_text_mode_pickup_and_inventory_commands() {
        assert!(matches!(
            parse_command(",", false),
            Some(PlayerAction::PickUp)
        ));
        assert!(matches!(
            parse_command("pickup", false),
            Some(PlayerAction::PickUp)
        ));
        assert!(matches!(
            parse_command("inventory", false),
            Some(PlayerAction::ViewInventory)
        ));
        assert!(matches!(
            parse_command("equipment", false),
            Some(PlayerAction::ViewEquipped)
        ));
    }

    #[test]
    fn parse_text_mode_wait_and_pray_commands() {
        assert!(matches!(
            parse_command(".", false),
            Some(PlayerAction::Rest)
        ));
        assert!(matches!(
            parse_command("wait", false),
            Some(PlayerAction::Rest)
        ));
        assert!(matches!(
            parse_command("pray", false),
            Some(PlayerAction::Pray)
        ));
    }

    #[test]
    fn parse_text_mode_extended_non_wizard_commands() {
        assert!(matches!(
            parse_command("look", false),
            Some(PlayerAction::LookHere)
        ));
        assert!(matches!(
            parse_command("conduct", false),
            Some(PlayerAction::ViewConduct)
        ));
        assert!(matches!(
            parse_command("discoveries", false),
            Some(PlayerAction::ViewDiscoveries)
        ));
        assert!(matches!(
            parse_command("attributes", false),
            Some(PlayerAction::Attributes)
        ));
        assert!(matches!(
            parse_command("twoweapon", false),
            Some(PlayerAction::ToggleTwoWeapon)
        ));
        assert!(matches!(
            parse_command("turnundead", false),
            Some(PlayerAction::TurnUndead)
        ));
        assert!(matches!(
            parse_command("offer", false),
            Some(PlayerAction::Offer { item: None })
        ));
        assert!(matches!(
            parse_command("eat", false),
            Some(PlayerAction::Eat { item: None })
        ));
        assert!(matches!(
            parse_command("whatis", false),
            Some(PlayerAction::WhatIs { position: None })
        ));
        assert!(matches!(
            parse_command("takeoffall", false),
            Some(PlayerAction::TakeOffAll)
        ));
        assert!(matches!(
            parse_command("save", false),
            Some(PlayerAction::Save)
        ));
        assert!(matches!(
            parse_command("savequit", false),
            Some(PlayerAction::SaveAndQuit)
        ));
        assert!(matches!(
            parse_command("known", false),
            Some(PlayerAction::ViewDiscoveries)
        ));
        assert!(matches!(
            parse_command("vanquished", false),
            Some(PlayerAction::Vanquished)
        ));
        assert!(matches!(
            parse_command("chronicle", false),
            Some(PlayerAction::Chronicle)
        ));
        assert!(matches!(
            parse_command("options", false),
            Some(PlayerAction::Options)
        ));
    }

    #[test]
    fn parse_text_mode_hash_prefixed_commands() {
        assert!(matches!(
            parse_command("#pray", false),
            Some(PlayerAction::Pray)
        ));
        assert!(matches!(
            parse_command("#drink", false),
            Some(PlayerAction::Quaff { item: None })
        ));
        assert!(matches!(
            parse_command("#wizmap", true),
            Some(PlayerAction::WizMap)
        ));
        assert!(parse_command("#wizmap", false).is_none());
    }

    #[test]
    fn repeatable_text_action_filter_excludes_ui_actions() {
        assert!(is_repeatable_text_action(&PlayerAction::Rest));
        assert!(is_repeatable_text_action(&PlayerAction::Move {
            direction: Direction::East,
        }));
        assert!(!is_repeatable_text_action(&PlayerAction::ViewInventory));
        assert!(!is_repeatable_text_action(&PlayerAction::Save));
        assert!(!is_repeatable_text_action(&PlayerAction::SaveAndQuit));
    }

    #[test]
    fn parse_text_mode_wizard_commands_gate_on_debug_mode() {
        assert!(matches!(
            parse_command("wizmap", true),
            Some(PlayerAction::WizMap)
        ));
        assert!(matches!(
            parse_command("wizidentify", true),
            Some(PlayerAction::WizIdentify)
        ));
        assert!(parse_command("wizmap", false).is_none());
        assert!(parse_command("wizidentify", false).is_none());
    }

    #[test]
    fn parse_text_mode_wizard_argument_commands() {
        assert!(matches!(
            parse_command("wizwish blessed +2 silver saber", true),
            Some(PlayerAction::WizWish { wish_text }) if wish_text == "blessed +2 silver saber"
        ));
        assert!(matches!(
            parse_command("wizgenesis arch-lich", true),
            Some(PlayerAction::WizGenesis { monster_name }) if monster_name == "arch-lich"
        ));
        assert!(matches!(
            parse_command("wizlevelport 9", true),
            Some(PlayerAction::WizLevelTeleport { depth: 9 })
        ));
        assert!(parse_command("wizwish silver dragon", false).is_none());
    }

    #[test]
    fn parse_direction_token_aliases() {
        assert_eq!(parse_direction_token("h"), Some(Direction::West));
        assert_eq!(
            parse_direction_token("north-east"),
            Some(Direction::NorthEast)
        );
        assert_eq!(parse_direction_token(">"), Some(Direction::Down));
        assert_eq!(parse_direction_token("."), Some(Direction::Self_));
        assert_eq!(parse_direction_token("bogus"), None);
    }

    #[test]
    fn parse_autopickup_types_maps_known_symbols() {
        let classes = parse_autopickup_types("$?!/=");
        assert_eq!(
            classes,
            vec![
                ObjectClass::Coin,
                ObjectClass::Scroll,
                ObjectClass::Potion,
                ObjectClass::Wand,
                ObjectClass::Ring,
            ]
        );
    }

    #[test]
    fn parse_autopickup_types_dedups_and_skips_unknown() {
        let classes = parse_autopickup_types("$x$$!");
        assert_eq!(classes, vec![ObjectClass::Coin, ObjectClass::Potion]);
    }

    #[test]
    fn apply_runtime_options_wires_autopickup_to_dungeon_state() {
        let mut world = GameWorld::new(Position::new(1, 1));
        let mut cfg = config::Config::default();
        cfg.behavior.autopickup = false;
        cfg.behavior.autopickup_types = "$?".to_string();

        apply_runtime_options(&mut world, &cfg);

        assert!(!world.dungeon().autopickup_enabled);
        assert_eq!(
            world.dungeon().autopickup_classes,
            vec![ObjectClass::Coin, ObjectClass::Scroll]
        );
    }

    #[test]
    fn parse_text_mode_contextual_item_and_direction_commands() {
        let mut world = GameWorld::new(Position::new(1, 1));
        let item_a = spawn_inventory_item(&mut world, 'a');
        let item_b = spawn_inventory_item(&mut world, 'b');

        assert!(matches!(
            parse_text_mode_contextual_command("drop a", &world),
            Some(PlayerAction::Drop { item }) if item == item_a
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("open h", &world),
            Some(PlayerAction::Open {
                direction: Direction::West
            })
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("throw a l", &world),
            Some(PlayerAction::Throw { item, direction: Direction::East }) if item == item_a
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("zap a k", &world),
            Some(PlayerAction::ZapWand { item, direction: Some(Direction::North) }) if item == item_a
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("dip a b", &world),
            Some(PlayerAction::Dip { item, into }) if item == item_a && into == item_b
        ));
        assert!(parse_text_mode_contextual_command("drop z", &world).is_none());
    }

    #[test]
    fn parse_text_mode_contextual_text_and_position_commands() {
        let mut world = GameWorld::new(Position::new(1, 1));
        let item_a = spawn_inventory_item(&mut world, 'a');

        assert!(matches!(
            parse_text_mode_contextual_command("annotate minetown branch", &world),
            Some(PlayerAction::Annotate { text }) if text == "minetown branch"
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("engrave Elbereth", &world),
            Some(PlayerAction::Engrave { text }) if text == "Elbereth"
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("call ! healing", &world),
            Some(PlayerAction::CallType { class: '!', name }) if name == "healing"
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("name level sokoban", &world),
            Some(PlayerAction::Name { target: nethack_babel_engine::action::NameTarget::Level, name }) if name == "sokoban"
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("name item a Excalibur", &world),
            Some(PlayerAction::Name { target: nethack_babel_engine::action::NameTarget::Item { item }, name })
                if item == item_a && name == "Excalibur"
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("name monster 3 4 foo", &world),
            Some(PlayerAction::Name { target: nethack_babel_engine::action::NameTarget::MonsterAt { position }, name })
                if position == Position::new(3, 4) && name == "foo"
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("travel 7 8", &world),
            Some(PlayerAction::Travel { destination }) if destination == Position::new(7, 8)
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("retravel 7 8", &world),
            Some(PlayerAction::Travel { destination }) if destination == Position::new(7, 8)
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("jump 2 9", &world),
            Some(PlayerAction::Jump { position }) if position == Position::new(2, 9)
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("lookat 5 6", &world),
            Some(PlayerAction::LookAt { position }) if position == Position::new(5, 6)
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("whatis 10 11", &world),
            Some(PlayerAction::WhatIs { position: Some(position) }) if position == Position::new(10, 11)
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("showtrap 10 11", &world),
            Some(PlayerAction::WhatIs { position: Some(position) }) if position == Position::new(10, 11)
        ));
        assert!(matches!(
            parse_text_mode_contextual_command("cast b h", &world),
            Some(PlayerAction::CastSpell { spell, direction: Some(Direction::West) })
                if spell.0 == 1
        ));
    }
}

// ---------------------------------------------------------------------------
// Data directory resolution
// ---------------------------------------------------------------------------

/// Resolve the data directory path.
fn resolve_data_dir(specified: &str) -> Result<std::path::PathBuf> {
    let path = Path::new(specified);
    if path.is_dir() {
        return Ok(path.to_path_buf());
    }
    let cwd_data = Path::new("data");
    if specified != "data" && cwd_data.is_dir() {
        return Ok(cwd_data.to_path_buf());
    }
    if let Ok(home) = std::env::var("HOME") {
        let user_data = std::path::PathBuf::from(&home)
            .join(".nethack-babel")
            .join("data");
        if user_data.is_dir() {
            return Ok(user_data);
        }
    }
    let usr_local = Path::new("/usr/local/share/nethack-babel/data");
    if usr_local.is_dir() {
        return Ok(usr_local.to_path_buf());
    }
    let usr_share = Path::new("/usr/share/nethack-babel/data");
    if usr_share.is_dir() {
        return Ok(usr_share.to_path_buf());
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        let candidate = exe_dir.join("data");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        if let Some(parent) = exe_dir.parent() {
            let candidate = parent.join("data");
            if candidate.is_dir() {
                return Ok(candidate);
            }
        }
    }
    anyhow::bail!(
        "Data directory not found. Searched:\n\
         - {specified} (--data-dir)\n\
         - ./data/ (cwd)\n\
         - ~/.nethack-babel/data/ (user install)\n\
         - /usr/local/share/nethack-babel/data/\n\
         - /usr/share/nethack-babel/data/\n\
         - <executable-dir>/data/\n\
         Install data files or pass --data-dir."
    )
}

fn object_class_from_option_symbol(symbol: char) -> Option<ObjectClass> {
    match symbol {
        ')' => Some(ObjectClass::Weapon),
        '[' => Some(ObjectClass::Armor),
        '=' => Some(ObjectClass::Ring),
        '"' => Some(ObjectClass::Amulet),
        '(' => Some(ObjectClass::Tool),
        '%' => Some(ObjectClass::Food),
        '!' => Some(ObjectClass::Potion),
        '?' => Some(ObjectClass::Scroll),
        '+' => Some(ObjectClass::Spellbook),
        '/' => Some(ObjectClass::Wand),
        '$' => Some(ObjectClass::Coin),
        '*' => Some(ObjectClass::Gem),
        '`' => Some(ObjectClass::Rock),
        '0' => Some(ObjectClass::Ball),
        '_' => Some(ObjectClass::Chain),
        '.' => Some(ObjectClass::Venom),
        _ => None,
    }
}

fn parse_autopickup_types(spec: &str) -> Vec<ObjectClass> {
    let mut classes = Vec::new();
    for symbol in spec.chars() {
        if let Some(class) = object_class_from_option_symbol(symbol)
            && !classes.contains(&class)
        {
            classes.push(class);
        }
    }
    classes
}

fn refresh_player_encumbrance(world: &mut GameWorld) {
    let player = world.player();
    let Some(attrs) = world.get_component::<Attributes>(player).map(|a| *a) else {
        return;
    };
    let carried = nethack_babel_engine::inventory::total_weight(world, player);
    let wounded_legs = if nethack_babel_engine::status::has_wounded_legs(world, player) {
        1
    } else {
        0
    };
    let carry_cap = nethack_babel_engine::inventory::carry_capacity(
        attrs.strength,
        attrs.constitution,
        nethack_babel_engine::status::is_levitating(world, player),
        false, // Flying state is not yet exposed by a dedicated status query.
        wounded_legs,
    );
    let level = nethack_babel_engine::inventory::encumbrance_level(carried, carry_cap);
    if let Some(mut enc) = world.get_component_mut::<EncumbranceLevel>(player) {
        enc.0 = level;
    }
}

fn apply_runtime_options(world: &mut GameWorld, cfg: &config::Config) {
    let autopickup_classes = parse_autopickup_types(&cfg.behavior.autopickup_types);
    world
        .dungeon_mut()
        .set_autopickup(cfg.behavior.autopickup, autopickup_classes);
    refresh_player_encumbrance(world);
}

// ---------------------------------------------------------------------------
// Save/Load helpers
// ---------------------------------------------------------------------------

/// Ensure the parent directory of a save file path exists.
fn ensure_save_dir(save_path: &Path) -> Result<()> {
    if let Some(parent) = save_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Try to load an existing save file for the given player name.
/// Returns `Some((world, turn, depth, rng_state))` if a save was found and
/// successfully loaded; `None` if no save file exists.
fn try_load_save(player_name: &str, data: &GameData) -> Option<(GameWorld, u32, i32, [u8; 32])> {
    let save_path = save::save_file_path(player_name);
    if !save::save_file_exists(&save_path) {
        return None;
    }
    match save::load_game_with_data(&save_path, data) {
        Ok(result) => Some(result),
        Err(e) => {
            eprintln!("Warning: failed to load save file: {e}");
            eprintln!("Starting a new game.");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// World creation (shared between TUI and text mode)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct WorldSetup {
    world: GameWorld,
    rooms: Vec<Room>,
    player_room_idx: Option<usize>,
}

fn create_world(data: &GameData, rng: &mut Pcg64, player_name: Option<&str>) -> WorldSetup {
    let depth: u8 = 1;
    let level = generate_level(depth, rng);

    // Determine player start position.
    let player_start = if let Some(up) = level.up_stairs {
        up
    } else if let Some(room) = level.rooms.first() {
        let (cx, cy) = room.center();
        Position::new(cx as i32, cy as i32)
    } else {
        Position::new(
            (LevelMap::DEFAULT_WIDTH / 2) as i32,
            (LevelMap::DEFAULT_HEIGHT / 2) as i32,
        )
    };

    let mut world = GameWorld::new_with_rng(player_start, rng);

    // Set the player's name if provided.
    if let Some(name) = player_name
        && let Some(mut n) = world.get_component_mut::<Name>(world.player())
    {
        n.0 = name.to_string();
    }

    // Install the generated level into the world's dungeon state.
    world.dungeon_mut().current_level = level.map;
    world.dungeon_mut().depth = depth as i32;

    // Spawn monsters.
    let player_room_idx = find_room_containing(&level.rooms, player_start);
    spawn_monsters(&mut world, data, &level.rooms, player_room_idx, depth, rng);

    WorldSetup {
        world,
        rooms: level.rooms,
        player_room_idx,
    }
}

// ---------------------------------------------------------------------------
// TUI mode
// ---------------------------------------------------------------------------

/// Install a panic hook that restores the terminal before printing the
/// panic message. This ensures the terminal is usable even after a crash.
fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Best-effort terminal restoration.
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        original_hook(panic_info);
    }));
}

// ---------------------------------------------------------------------------
// Options sub-menus
// ---------------------------------------------------------------------------

fn bool_item(accel: char, label: &str, value: bool) -> MenuItem {
    MenuItem {
        accelerator: accel,
        text: format!("[{}] {}", if value { "X" } else { " " }, label),
        selected: false,
        selectable: true,
        group: None,
    }
}

fn option_accelerator(index: usize) -> char {
    if index < 26 {
        (b'a' + index as u8) as char
    } else if index < 52 {
        (b'A' + (index - 26) as u8) as char
    } else {
        ' '
    }
}

fn show_game_options(
    port: &mut TuiPort,
    cfg: &mut config::Config,
    _locale: &LocaleManager,
    config_path: &str,
) {
    loop {
        let option_pairs = config::options_menu_items(cfg);

        let items: Vec<MenuItem> = option_pairs
            .iter()
            .enumerate()
            .map(|(i, (name, value))| {
                let accel = option_accelerator(i);
                // Look up description from the option metadata table.
                let desc_suffix = config::describe_option(name)
                    .map(|_d| "") // Description is available but we keep menu compact
                    .unwrap_or("");
                let text = if value == "true" || value == "false" {
                    let on = value == "true";
                    format!("[{}] {}{}", if on { "X" } else { " " }, name, desc_suffix)
                } else {
                    format!("{}: {}{}", name, value, desc_suffix)
                };
                MenuItem {
                    accelerator: accel,
                    text,
                    selected: false,
                    selectable: true,
                    group: None,
                }
            })
            .collect();

        let behavior_count = config::options_in_section(config::OptionSection::Behavior).len();
        let menu = Menu {
            title: format!(
                "Game Settings ({} behavior options available)",
                behavior_count
            ),
            items,
            how: MenuHow::PickOne,
        };

        match port.show_menu(&menu) {
            MenuResult::Selected(indices) if !indices.is_empty() => {
                let idx = indices[0];
                if idx < option_pairs.len() {
                    let (name, current_value) = &option_pairs[idx];

                    if current_value == "true" || current_value == "false" {
                        // Boolean option: toggle via apply_option using
                        // the "!name" (negate) convention
                        let toggle_name = if current_value == "true" {
                            format!("!{}", name)
                        } else {
                            name.clone()
                        };
                        let _ = config::apply_option(cfg, &toggle_name, None);
                    } else {
                        // Compound/string option: prompt for new value
                        let prompt = format!("{} [{}]: ", name, current_value);
                        if let Some(val) = port.get_line(&prompt)
                            && !val.is_empty()
                        {
                            let _ = config::apply_option(cfg, name, Some(&val));
                        }
                    }
                    let _ = config::save_config(cfg, config_path);
                }
            }
            _ => break,
        }
    }
}

fn show_display_options(
    port: &mut TuiPort,
    cfg: &mut config::Config,
    locale: &LocaleManager,
    config_path: &str,
) {
    loop {
        let items = vec![
            bool_item(
                'a',
                &locale.translate("opt-map-colors", None),
                cfg.display.map_colors,
            ),
            bool_item(
                'b',
                &locale.translate("opt-message-colors", None),
                cfg.display.message_colors,
            ),
            bool_item(
                'c',
                &locale.translate("opt-buc-highlight", None),
                cfg.display.buc_highlight,
            ),
            bool_item(
                'd',
                &locale.translate("opt-minimap", None),
                cfg.display.minimap,
            ),
            bool_item(
                'e',
                &locale.translate("opt-mouse-hover", None),
                cfg.display.mouse_hover_info,
            ),
            bool_item(
                'f',
                &locale.translate("opt-nerd-fonts", None),
                cfg.display.nerd_fonts,
            ),
        ];
        let menu = Menu {
            title: locale.translate("ui-options-display", None),
            items,
            how: MenuHow::PickOne,
        };
        match port.show_menu(&menu) {
            MenuResult::Selected(indices) if !indices.is_empty() => {
                match indices[0] {
                    0 => cfg.display.map_colors = !cfg.display.map_colors,
                    1 => cfg.display.message_colors = !cfg.display.message_colors,
                    2 => cfg.display.buc_highlight = !cfg.display.buc_highlight,
                    3 => cfg.display.minimap = !cfg.display.minimap,
                    4 => cfg.display.mouse_hover_info = !cfg.display.mouse_hover_info,
                    5 => cfg.display.nerd_fonts = !cfg.display.nerd_fonts,
                    _ => {}
                }
                let _ = config::save_config(cfg, config_path);
            }
            _ => break,
        }
    }
}

fn show_sound_options(
    port: &mut TuiPort,
    cfg: &mut config::Config,
    locale: &LocaleManager,
    config_path: &str,
) {
    loop {
        let items = vec![
            bool_item(
                'a',
                &locale.translate("opt-sound-enabled", None),
                cfg.sound.enabled,
            ),
            MenuItem {
                accelerator: 'b',
                text: format!(
                    "{}: {}",
                    locale.translate("opt-volume", None),
                    cfg.sound.volume
                ),
                selected: false,
                selectable: true,
                group: None,
            },
        ];
        let menu = Menu {
            title: locale.translate("ui-options-sound", None),
            items,
            how: MenuHow::PickOne,
        };
        match port.show_menu(&menu) {
            MenuResult::Selected(indices) if !indices.is_empty() => {
                match indices[0] {
                    0 => cfg.sound.enabled = !cfg.sound.enabled,
                    1 => {
                        let prompt = format!("{} (0-100): ", locale.translate("opt-volume", None));
                        if let Some(val) = port.get_line(&prompt)
                            && let Ok(v) = val.trim().parse::<u8>()
                            && v <= 100
                        {
                            cfg.sound.volume = v;
                        }
                    }
                    _ => {}
                }
                let _ = config::save_config(cfg, config_path);
            }
            _ => break,
        }
    }
}

fn show_language_menu(port: &mut TuiPort, locale: &mut LocaleManager, app: &mut App) {
    let mut langs: Vec<(String, String, String)> = locale
        .available_languages()
        .into_iter()
        .map(|code| {
            let manifest = locale.manifest_for(code);
            let name = manifest
                .map(|m| m.name.clone())
                .unwrap_or_else(|| code.to_string());
            let name_en = manifest
                .map(|m| m.name_en.clone())
                .unwrap_or_else(|| code.to_string());
            (code.to_string(), name, name_en)
        })
        .collect();
    langs.sort_by(|a, b| a.0.cmp(&b.0));

    let current = locale.current_language().to_string();
    let items: Vec<MenuItem> = langs
        .iter()
        .enumerate()
        .map(|(i, (code, name, name_en))| {
            let accel = (b'a' + i as u8) as char;
            let marker = if *code == current { "  [*]" } else { "" };
            let text = if name == name_en {
                format!("{}{}", name, marker)
            } else {
                format!("{} ({}){}", name, name_en, marker)
            };
            MenuItem {
                accelerator: accel,
                text,
                selected: *code == current,
                selectable: true,
                group: None,
            }
        })
        .collect();

    let menu = Menu {
        title: locale.translate("ui-select-language", None),
        items,
        how: MenuHow::PickOne,
    };

    if let MenuResult::Selected(indices) = port.show_menu(&menu)
        && let Some(&idx) = indices.first()
        && idx < langs.len()
    {
        let chosen = &langs[idx].0;
        if locale.set_language(chosen).is_ok() {
            let mut args = fluent::FluentArgs::new();
            args.set("language", langs[idx].1.clone());
            let text = locale.translate("config-language-set", Some(&args));
            app.push_message(text, MessageUrgency::System);
        }
    }
}

fn build_inventory_i18n(locale: &LocaleManager) -> InventoryI18n {
    let mut class_headers = std::collections::HashMap::new();
    let classes = [
        (ObjectClass::Weapon, "inv-class-weapon"),
        (ObjectClass::Armor, "inv-class-armor"),
        (ObjectClass::Ring, "inv-class-ring"),
        (ObjectClass::Amulet, "inv-class-amulet"),
        (ObjectClass::Tool, "inv-class-tool"),
        (ObjectClass::Food, "inv-class-food"),
        (ObjectClass::Potion, "inv-class-potion"),
        (ObjectClass::Scroll, "inv-class-scroll"),
        (ObjectClass::Spellbook, "inv-class-spellbook"),
        (ObjectClass::Wand, "inv-class-wand"),
        (ObjectClass::Coin, "inv-class-coin"),
        (ObjectClass::Gem, "inv-class-gem"),
        (ObjectClass::Rock, "inv-class-rock"),
        (ObjectClass::Ball, "inv-class-ball"),
        (ObjectClass::Chain, "inv-class-chain"),
        (ObjectClass::Venom, "inv-class-venom"),
    ];
    for (class, key) in &classes {
        class_headers.insert(*class, locale.translate(key, None));
    }
    InventoryI18n {
        class_headers,
        title: locale.translate("ui-inventory-title", None),
        empty_message: locale.translate("ui-inventory-empty", None),
        buc_marker_blessed: locale.translate("inv-buc-marker-blessed", None),
        buc_marker_cursed: locale.translate("inv-buc-marker-cursed", None),
        buc_tag_blessed: locale.translate("inv-buc-tag-blessed", None),
        buc_tag_cursed: locale.translate("inv-buc-tag-cursed", None),
        buc_tag_uncursed: locale.translate("inv-buc-tag-uncursed", None),
        other_header: locale.translate("inv-class-other", None),
        pickup_title: locale.translate("ui-pickup-title", None),
    }
}

fn inventory_display_name(
    world: &GameWorld,
    entity: hecs::Entity,
    core: &ObjectCore,
    obj_defs: &[ObjectDef],
) -> String {
    let base = world
        .get_component::<Name>(entity)
        .map(|name| name.0.clone())
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            nethack_babel_engine::items::object_def_for_core(obj_defs, core)
                .map(|def| def.name.clone())
        })
        .unwrap_or_else(|| format!("item(otyp={})", core.otyp.0));

    if core.quantity > 1 && !base.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!(
            "{} {}",
            core.quantity,
            nethack_babel_engine::identification::makeplural(&base)
        )
    } else {
        base
    }
}

fn build_inventory_items(world: &GameWorld, obj_defs: &[ObjectDef]) -> Vec<InventoryItem> {
    let unknown_buc = BucStatus {
        cursed: false,
        blessed: false,
        bknown: false,
    };

    nethack_babel_engine::items::get_inventory(world, world.player())
        .into_iter()
        .map(|(entity, core)| {
            let display_name = inventory_display_name(world, entity, &core, obj_defs);
            let buc = world
                .get_component::<BucStatus>(entity)
                .map(|b| (*b).clone())
                .unwrap_or_else(|| unknown_buc.clone());
            make_inventory_item(&core, &buc, &display_name)
        })
        .collect()
}

fn build_equipped_lines(world: &GameWorld, obj_defs: &[ObjectDef]) -> Vec<String> {
    let player = world.player();
    let Some(equip) = world.get_component::<EquipmentSlots>(player) else {
        return Vec::new();
    };

    let slots = [
        ("Weapon", equip.weapon),
        ("Off-hand", equip.off_hand),
        ("Helmet", equip.helmet),
        ("Cloak", equip.cloak),
        ("Body armor", equip.body_armor),
        ("Shield", equip.shield),
        ("Gloves", equip.gloves),
        ("Boots", equip.boots),
        ("Shirt", equip.shirt),
        ("Ring (left)", equip.ring_left),
        ("Ring (right)", equip.ring_right),
        ("Amulet", equip.amulet),
    ];

    slots
        .iter()
        .filter_map(|(label, maybe_entity)| {
            let entity = (*maybe_entity)?;
            let name = if let Some(core) = world.get_component::<ObjectCore>(entity) {
                inventory_display_name(world, entity, &core, obj_defs)
            } else {
                world.entity_name(entity)
            };
            Some(format!("{label}: {name}"))
        })
        .collect()
}

struct TuiRuntimeContext<'a> {
    locale: &'a mut LocaleManager,
    cfg: &'a mut config::Config,
    config_path: &'a str,
    legacy_info: Option<(&'a str, &'a str)>,
    wizard_mode: bool,
}

fn run_tui_mode(
    world: &mut GameWorld,
    data: &GameData,
    rng: &mut Pcg64,
    runtime: TuiRuntimeContext<'_>,
) -> Result<()> {
    install_panic_hook();

    let TuiRuntimeContext {
        locale,
        cfg,
        config_path,
        legacy_info,
        wizard_mode,
    } = runtime;

    // Auto-checkpoint config — saves every 100 turns in case of crash.
    let mut checkpoint_config =
        save::CheckpointConfig::with_interval(save::DEFAULT_CHECKPOINT_INTERVAL);

    let mut port =
        TuiPort::create().map_err(|e| anyhow::anyhow!("Failed to initialize TUI: {e}"))?;
    port.init();

    let mut app = App::new();
    app.set_wizard_mode(wizard_mode);
    app.messages_i18n = TuiMessages {
        empty_handed: locale.translate("ui-empty-handed", None),
        never_mind: locale.translate("ui-never-mind", None),
        no_such_item: locale.translate("ui-no-such-item", None),
        not_implemented: locale.translate("ui-not-implemented", None),
    };
    let map = &world.dungeon().current_level;
    let mut fov = FovMap::new(map.width, map.height);

    // Initial welcome message.
    {
        use fluent::FluentArgs;
        let mut args = FluentArgs::new();
        let player_name = world
            .get_component::<Name>(world.player())
            .map(|n| n.0.clone())
            .unwrap_or_else(|| "Adventurer".to_string());
        args.set("name", player_name);
        args.set("role", "".to_string());
        let welcome = locale.translate("welcome", Some(&args));
        let text = if welcome == "welcome" {
            format!(
                "NetHack Babel v{} -- Welcome to the dungeon!",
                env!("CARGO_PKG_VERSION")
            )
        } else {
            format!(
                "NetHack Babel v{} -- {}",
                env!("CARGO_PKG_VERSION"),
                welcome
            )
        };
        app.push_message(text, MessageUrgency::System);
    }

    // Show legacy intro narrative if enabled.
    if cfg.game.legacy
        && let Some((deity, role)) = legacy_info
    {
        use fluent::FluentArgs;
        let mut args = FluentArgs::new();
        args.set("deity", deity.to_string());
        args.set("role", role.to_string());
        let text = locale.translate("legacy-intro", Some(&args));
        if text != "legacy-intro" {
            let items: Vec<MenuItem> = text
                .lines()
                .map(|line| MenuItem {
                    accelerator: ' ',
                    text: line.to_string(),
                    selected: false,
                    selectable: false,
                    group: None,
                })
                .collect();
            let menu = Menu {
                title: "NetHack Babel".to_string(),
                items,
                how: MenuHow::None,
            };
            port.show_menu(&menu);
        }
    }

    let mut game_over = false;

    loop {
        app.update_inventory_letters(
            nethack_babel_engine::items::get_inventory(world, world.player())
                .into_iter()
                .filter_map(|(entity, core)| core.inv_letter.map(|letter| (entity, letter))),
        );

        // Update FOV.
        update_fov(world, &mut fov);

        // Build display data from world state.
        let map_view = build_map_view(world, &fov);
        let status = build_status(world, locale);

        // Update app state.
        app.map = map_view;
        app.status = status;

        // Set cursor to player position.
        if let Some(pos) = world.get_component::<Positioned>(world.player()) {
            app.cursor = (pos.0.x as i16, pos.0.y as i16);
        }

        // Render.
        app.render(&mut port);

        // Display any pending messages with --More-- handling.
        if app.messages.has_pending() {
            app.display_pending_messages(&mut port);
        }

        if game_over {
            // Wait for a key then exit.
            port.get_key();
            break;
        }

        // Get player input.
        let action = match app.get_player_action(&mut port) {
            Some(PlayerAction::ShowHistory) => {
                app.show_history(&mut port);
                continue;
            }
            Some(PlayerAction::Help) => {
                let title = locale.translate("help-title", None);
                let help_keys = [
                    "help-move",
                    "",
                    "help-move-diagram",
                    "",
                    "help-attack",
                    "help-wait",
                    "help-search",
                    "help-inventory",
                    "help-pickup",
                    "help-drop",
                    "help-stairs-up",
                    "help-stairs-down",
                    "help-eat",
                    "help-quaff",
                    "help-read",
                    "help-wield",
                    "help-wear",
                    "help-remove",
                    "help-zap",
                    "help-options",
                    "help-look",
                    "help-history",
                    "",
                    "help-shift-run",
                    "help-arrows",
                    "",
                    "help-symbols-title",
                ];
                let symbol_keys = [
                    "help-symbol-player",
                    "help-symbol-floor",
                    "help-symbol-corridor",
                    "help-symbol-door-closed",
                    "help-symbol-door-open",
                    "help-symbol-stairs-up",
                    "help-symbol-stairs-down",
                    "help-symbol-water",
                    "help-symbol-fountain",
                ];
                let mut lines: Vec<String> = help_keys
                    .iter()
                    .map(|k| {
                        if k.is_empty() {
                            String::new()
                        } else {
                            locale.translate(k, None)
                        }
                    })
                    .collect();
                for key in &symbol_keys {
                    lines.push(format!("  {}", locale.translate(key, None)));
                }
                port.show_text(&title, &lines.join("\n"));
                continue;
            }
            Some(PlayerAction::ViewInventory) => {
                let inv_i18n = build_inventory_i18n(locale);
                let items = build_inventory_items(world, &data.objects);
                nethack_babel_tui::show_inventory(&mut port, &items, Some(&inv_i18n));
                continue;
            }
            Some(PlayerAction::ViewEquipped) => {
                let title = locale.translate("ui-equipment-title", None);
                let lines = build_equipped_lines(world, &data.objects);
                if lines.is_empty() {
                    let body = locale.translate("ui-equipment-empty", None);
                    port.show_text(&title, &body);
                } else {
                    port.show_text(&title, &lines.join("\n"));
                }
                continue;
            }
            Some(PlayerAction::LookAt { position }) => {
                let map = &world.dungeon().current_level;
                let x = position.x as usize;
                let y = position.y as usize;
                if y < map.height && x < map.width {
                    let terrain = map.cells[y][x].terrain;
                    let mut args = fluent::FluentArgs::new();
                    args.set("terrain", format!("{:?}", terrain));
                    let text = locale.translate("event-you-see-here", Some(&args));
                    app.push_message(text, MessageUrgency::Normal);
                }
                continue;
            }
            Some(PlayerAction::WhatIs { position }) => {
                let target = position.or_else(|| {
                    world
                        .get_component::<Positioned>(world.player())
                        .map(|p| p.0)
                });
                if let Some(pos) = target {
                    let map = &world.dungeon().current_level;
                    let x = pos.x as usize;
                    let y = pos.y as usize;
                    if y < map.height && x < map.width {
                        let terrain = map.cells[y][x].terrain;
                        let mut args = fluent::FluentArgs::new();
                        args.set("terrain", format!("{:?}", terrain));
                        let text = locale.translate("event-you-see-here", Some(&args));
                        app.push_message(text, MessageUrgency::Normal);
                    }
                }
                continue;
            }
            Some(PlayerAction::LookHere) => {
                if let Some(pos) = world.get_component::<Positioned>(world.player()) {
                    let map = &world.dungeon().current_level;
                    let x = pos.0.x as usize;
                    let y = pos.0.y as usize;
                    if y < map.height && x < map.width {
                        let terrain = map.cells[y][x].terrain;
                        let mut args = fluent::FluentArgs::new();
                        args.set("terrain", format!("{:?}", terrain));
                        let text = locale.translate("event-you-see-here", Some(&args));
                        app.push_message(text, MessageUrgency::Normal);
                    }
                }
                continue;
            }
            Some(PlayerAction::Quit) => {
                // Ctrl+C — ask for confirmation
                let confirmed = port.ask_yn("Really quit?", "yn", 'n');
                if confirmed == 'y' {
                    break;
                }
                continue;
            }
            Some(PlayerAction::SaveAndQuit) => {
                // S — save and quit
                app.push_message(
                    locale.translate("ui-save-prompt", None),
                    MessageUrgency::System,
                );
                app.display_pending_messages(&mut port);

                let player_name = world.entity_name(world.player());
                let save_path = save::save_file_path(&player_name);
                // Generate a fresh seed from the current RNG to persist.
                let rng_state: [u8; 32] = rng.random();
                if let Err(e) = ensure_save_dir(&save_path) {
                    app.push_message(
                        format!("Failed to create save directory: {e}"),
                        MessageUrgency::System,
                    );
                    app.display_pending_messages(&mut port);
                    continue;
                }
                match save::save_game(world, &save_path, save::SaveReason::Quit, rng_state) {
                    Ok(()) => {
                        app.push_message(
                            locale.translate("ui-save-goodbye", None),
                            MessageUrgency::System,
                        );
                        app.display_pending_messages(&mut port);
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        break;
                    }
                    Err(e) => {
                        app.push_message(format!("Save failed: {e}"), MessageUrgency::System);
                        app.display_pending_messages(&mut port);
                        continue;
                    }
                }
            }
            Some(PlayerAction::Save) => {
                let player_name = world.entity_name(world.player());
                let save_path = save::save_file_path(&player_name);
                let rng_state: [u8; 32] = rng.random();
                if let Err(e) = ensure_save_dir(&save_path) {
                    app.push_message(
                        format!("Failed to create save directory: {e}"),
                        MessageUrgency::System,
                    );
                    continue;
                }
                match save::save_game(world, &save_path, save::SaveReason::Checkpoint, rng_state) {
                    Ok(()) => {
                        app.push_message(
                            locale.translate("ui-save-success", None),
                            MessageUrgency::System,
                        );
                    }
                    Err(e) => {
                        app.push_message(format!("Save failed: {e}"), MessageUrgency::System);
                    }
                }
                continue;
            }
            Some(PlayerAction::Options) => {
                // Top-level options menu loop
                loop {
                    let lang_label = locale
                        .manifest_for(locale.current_language())
                        .map(|m| m.name.clone())
                        .unwrap_or_default();
                    let items = vec![
                        MenuItem {
                            accelerator: 'a',
                            text: locale.translate("ui-options-game", None),
                            selected: false,
                            selectable: true,
                            group: None,
                        },
                        MenuItem {
                            accelerator: 'b',
                            text: locale.translate("ui-options-display", None),
                            selected: false,
                            selectable: true,
                            group: None,
                        },
                        MenuItem {
                            accelerator: 'c',
                            text: locale.translate("ui-options-sound", None),
                            selected: false,
                            selectable: true,
                            group: None,
                        },
                        MenuItem {
                            accelerator: 'd',
                            text: format!(
                                "{}: {}",
                                locale.translate("ui-select-language", None),
                                lang_label
                            ),
                            selected: false,
                            selectable: true,
                            group: None,
                        },
                    ];
                    let menu = Menu {
                        title: locale.translate("ui-options-title", None),
                        items,
                        how: MenuHow::PickOne,
                    };
                    match port.show_menu(&menu) {
                        MenuResult::Selected(indices) if !indices.is_empty() => match indices[0] {
                            0 => show_game_options(&mut port, cfg, locale, config_path),
                            1 => show_display_options(&mut port, cfg, locale, config_path),
                            2 => show_sound_options(&mut port, cfg, locale, config_path),
                            3 => show_language_menu(&mut port, locale, &mut app),
                            _ => {}
                        },
                        _ => break,
                    }
                }
                apply_runtime_options(world, cfg);
                continue;
            }
            Some(a) => a,
            None => continue, // Unmapped key or Escape.
        };

        let Some(action) = contextualize_tui_action(&mut port, world, action) else {
            continue;
        };

        app.remember_repeatable_action(&action);

        // Resolve the turn.
        let events = resolve_turn(world, action, rng);

        // Auto-checkpoint after each turn (if interval has elapsed).
        let player_name = world.entity_name(world.player());
        let rng_state: [u8; 32] = rng.random();
        save::maybe_checkpoint(world, &mut checkpoint_config, &player_name, rng_state);

        // Convert events to messages.
        let messages = events_to_messages(&events, locale, world);
        for (text, urgency) in &messages {
            app.push_message(text.clone(), *urgency);
        }

        // Check for game over (player death or GameOver event).
        for event in &events {
            let death_info = match event {
                EngineEvent::EntityDied { entity, cause, .. } if world.is_player(*entity) => {
                    Some((cause.clone(), 0u64))
                }
                EngineEvent::GameOver { cause, score } => Some((cause.clone(), *score)),
                _ => None,
            };
            if let Some((cause, score)) = death_info {
                game_over = true;
                handle_game_over(world, &mut port, &cause, score);
            }
        }
    }

    port.shutdown();
    Ok(())
}

// ---------------------------------------------------------------------------
// Text mode (fallback)
// ---------------------------------------------------------------------------

fn run_text_mode(
    world: &mut GameWorld,
    data: &GameData,
    rng: &mut Pcg64,
    recorder: &mut Option<recording::GameRecorder>,
    locale: &LocaleManager,
    wizard_mode: bool,
) -> Result<()> {
    // Text mode does not auto-checkpoint (user can save manually).
    let _checkpoint_config = save::CheckpointConfig::disabled();

    println!();
    println!("{}", locale.translate("event-dungeon-welcome", None));
    println!(
        "Commands: h/j/k/l/y/u/b/n move, . wait, s search, , pickup, i inv, eq equip, p pray, < up, > down, q quit, ? help"
    );
    println!();

    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut game_over = false;
    let mut last_repeatable_action: Option<PlayerAction> = None;

    loop {
        render_map_ascii(world);
        render_status_text(world);

        if game_over {
            println!("{}", locale.translate("ui-game-over-thanks", None));
            break;
        }

        print!("> ");
        stdout.lock().flush()?;

        let mut input = String::new();
        let bytes_read = stdin.lock().read_line(&mut input)?;
        if bytes_read == 0 {
            println!("{}", locale.translate("ui-goodbye", None));
            break;
        }

        let trimmed = input.trim();

        if let Some(rec) = recorder.as_mut() {
            rec.record_input(trimmed);
        }

        let command_input = trimmed
            .strip_prefix('#')
            .map(str::trim_start)
            .unwrap_or(trimmed);

        if command_input == "q" || command_input == "quit" {
            println!("{}", locale.translate("ui-goodbye", None));
            break;
        }
        if command_input == "?" || command_input == "help" {
            println!();
            println!("=== NetHack Babel Help ===");
            println!("Movement (vi-keys):");
            println!("  y k u     NW  N  NE");
            println!("  h . l      W  .   E");
            println!("  b j n     SW  S  SE");
            println!();
            println!("Commands:");
            println!("  .  = rest/wait one turn");
            println!("  s  = search adjacent squares");
            println!("  ,  = pick up items at your feet");
            println!("  i  = show inventory");
            println!("  eq = show equipped items");
            println!("  p  = pray");
            println!("  <  = go up stairs");
            println!("  >  = go down stairs");
            println!("  repeat = repeat last action");
            println!("  save = save game");
            println!("  savequit = save and quit");
            println!("  q  = quit the game");
            println!("  ?  = show this help");
            println!("  open <dir>, kick <dir>, chat <dir>, run <dir>");
            println!("  drop <letter>, wear/wield/apply <letter>, throw <letter> <dir>");
            println!("  zap <letter> [dir], dip <letter> <letter>, cast <spell> [dir]");
            println!("  travel <x> <y>, jump <x> <y>, lookat <x> <y>, whatis <x> <y>");
            println!("  annotate <text>, engrave <text>, call <class> <name>");
            println!();
            println!("Symbols:");
            println!("  @  = you (the player)");
            println!("  .  = floor");
            println!("  #  = corridor or wall");
            println!("  +  = closed/locked door");
            println!("  |  = open door");
            println!("  <  = stairs up");
            println!("  >  = stairs down");
            println!();
            continue;
        }

        let action = if let Some(a) = parse_text_mode_contextual_command(command_input, world) {
            a
        } else if command_input == "repeat" {
            if let Some(prev) = &last_repeatable_action {
                prev.clone()
            } else {
                println!("  Nothing to repeat.");
                continue;
            }
        } else {
            match parse_command(command_input, wizard_mode) {
                Some(a) => a,
                None => {
                    if !trimmed.is_empty() {
                        let mut args = fluent::FluentArgs::new();
                        args.set("key", command_input.to_string());
                        println!("  {}", locale.translate("ui-unknown-command", Some(&args)));
                    }
                    continue;
                }
            }
        };

        let Some(action) = contextualize_text_action(&stdin, &stdout, world, action)? else {
            continue;
        };

        if is_repeatable_text_action(&action) {
            last_repeatable_action = Some(action.clone());
        }

        if matches!(action, PlayerAction::ViewInventory) {
            let items = build_inventory_items(world, &data.objects);
            if items.is_empty() {
                println!("  You are not carrying anything.");
            } else {
                println!("  Inventory:");
                for item in &items {
                    println!("    {} - {}", item.letter, item.name);
                }
            }
            continue;
        }

        if matches!(action, PlayerAction::ViewEquipped) {
            let lines = build_equipped_lines(world, &data.objects);
            if lines.is_empty() {
                println!("  You have nothing equipped.");
            } else {
                println!("  Equipped:");
                for line in &lines {
                    println!("    {line}");
                }
            }
            continue;
        }

        if matches!(action, PlayerAction::Save) {
            let player_name = world.entity_name(world.player());
            let save_path = save::save_file_path(&player_name);
            let rng_state: [u8; 32] = rng.random();
            if let Err(e) = ensure_save_dir(&save_path) {
                println!("  Failed to create save directory: {e}");
                continue;
            }
            match save::save_game(world, &save_path, save::SaveReason::Checkpoint, rng_state) {
                Ok(()) => println!("  {}", locale.translate("ui-save-success", None)),
                Err(e) => println!("  Save failed: {e}"),
            }
            continue;
        }

        if matches!(action, PlayerAction::SaveAndQuit) {
            let player_name = world.entity_name(world.player());
            let save_path = save::save_file_path(&player_name);
            let rng_state: [u8; 32] = rng.random();
            if let Err(e) = ensure_save_dir(&save_path) {
                println!("  Failed to create save directory: {e}");
                continue;
            }
            match save::save_game(world, &save_path, save::SaveReason::Quit, rng_state) {
                Ok(()) => {
                    println!("  {}", locale.translate("ui-save-goodbye", None));
                    break;
                }
                Err(e) => println!("  Save failed: {e}"),
            }
            continue;
        }

        let events = resolve_turn(world, action, rng);
        display_events_text(&events, locale, world);

        for event in &events {
            match event {
                EngineEvent::GameOver { cause, score } => {
                    game_over = true;
                    println!("\n*** GAME OVER ***");
                    println!("Score: {score}");
                    println!("Cause: {cause:?}");
                }
                EngineEvent::EntityDied { entity, cause, .. } if world.is_player(*entity) => {
                    game_over = true;
                    println!("\n*** GAME OVER ***");
                    println!("Cause: {cause:?}");
                    println!("Turns: {}", world.turn());
                }
                _ => {}
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    // 1. Parse CLI args.
    let cli = Cli::parse();

    // -- Handle --server ----------------------------------------------------
    if let Some(ref server_opt) = cli.server {
        let addr = server_opt.as_deref().unwrap_or(server::DEFAULT_BIND_ADDR);
        let srv = server::GameServer::new(addr, server::DEFAULT_MAX_CONNECTIONS);
        srv.start()
            .map_err(|e| anyhow::anyhow!("Server error: {e}"))?;
        return Ok(());
    }

    // -- Handle --replay ---------------------------------------------------
    if let Some(ref replay_path) = cli.replay {
        let path = std::path::PathBuf::from(replay_path);
        match recording::replay_session(&path) {
            Ok(()) => return Ok(()),
            Err(e) => anyhow::bail!("Replay error: {e}"),
        }
    }

    // -- Prepare optional recorder ----------------------------------------
    let mut recorder = cli.record.as_ref().map(|path| {
        let r = recording::GameRecorder::new(path);
        if cli.text {
            println!("Recording session to: {path}");
        }
        r
    });

    // 2. Initialize tracing.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    // 3. Load config.
    let mut cfg = config::load_config(&cli.config)?;

    // Also try loading ~/.nethackrc for traditional NetHack-style options.
    if let Ok(home) = std::env::var("HOME") {
        let rc_path = format!("{}/.nethackrc", home);
        if std::path::Path::new(&rc_path).exists() {
            let _ = config::load_nethackrc(&rc_path, &mut cfg);
        }
    }

    if let Some(ref lang) = cli.language {
        cfg.game.language = lang.clone();
    }

    // 4. Load game data.
    let data_dir = resolve_data_dir(&cli.data_dir)?;
    let data = load_game_data(&data_dir)
        .with_context(|| format!("Failed to load game data from {}", data_dir.display()))?;

    if cli.text {
        println!("NetHack Babel v{}", env!("CARGO_PKG_VERSION"));
        println!();
        println!(
            "Loaded {} monsters, {} objects from {}",
            data.monsters.len(),
            data.objects.len(),
            data_dir.display()
        );
    }

    // 5. Initialize locale manager.
    let mut locale = init_locale_manager(&data_dir, &cfg.game.language)?;

    if cli.text {
        println!(
            "Language: {} ({})",
            locale.current_language(),
            locale.manifest().name
        );
    }

    // 6. Create RNG.
    let mut rng = Pcg64::from_os_rng();

    // 6b. Install panic save hook so the game is saved on crash.
    save::install_panic_hook();

    // 7. Try to restore from a save file, or create a new world.
    //    First try recovering from a crashed (panic) save, then try normal save.
    let player_name_for_save = cli.name.as_deref().unwrap_or("you");
    let recovered = match save::try_recover_with_data(player_name_for_save, &data) {
        Ok(Some(result)) => {
            tracing::info!("Recovered from panic save");
            Some(result)
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("Panic recovery failed: {e}");
            None
        }
    };
    let (mut world, legacy_info) = if let Some((restored_world, turn, depth, rng_state)) =
        recovered.or_else(|| try_load_save(player_name_for_save, &data))
    {
        // Re-seed the RNG from the saved state.
        rng = Pcg64::from_seed(rng_state);
        if cli.text {
            println!("Restored saved game: turn {turn}, depth {depth}.");
        }
        (restored_world, None)
    } else {
        // --- Character selection ---
        let character_choice = if cli.text {
            game_start::select_character_text(
                cli.role.as_deref(),
                cli.race.as_deref(),
                cli.name.as_deref(),
                &locale,
            )
        } else {
            // For TUI mode, we need a temporary port for the menus.
            // We init and shut down a TUI port just for character selection
            // (the main game loop will create its own).
            install_panic_hook();
            let mut port =
                TuiPort::create().map_err(|e| anyhow::anyhow!("Failed to initialize TUI: {e}"))?;
            port.init();

            let choice = game_start::select_character(
                &mut port,
                cli.role.as_deref(),
                cli.race.as_deref(),
                cli.name.as_deref(),
                &locale,
            );

            port.shutdown();

            match choice {
                Some(c) => c,
                None => {
                    // Player cancelled character creation.
                    return Ok(());
                }
            }
        };

        if cli.text {
            let mut role_args = fluent::FluentArgs::new();
            role_args.set("name", character_choice.name.clone());
            role_args.set("align", character_choice.alignment.name().to_string());
            role_args.set("race", character_choice.race.name().to_string());
            role_args.set("role", character_choice.role_name.clone());
            println!(
                "{}",
                locale.translate("event-player-role", Some(&role_args))
            );
            println!();
        }

        // Compute deity name for legacy intro.
        let deity_name = {
            use nethack_babel_engine::pets::Role;
            use nethack_babel_engine::religion::{god_name, roles};
            let role_idx = match character_choice.role {
                Role::Archeologist => roles::ARCHEOLOGIST,
                Role::Barbarian => roles::BARBARIAN,
                Role::Caveperson => roles::CAVEMAN,
                Role::Healer => roles::HEALER,
                Role::Knight => roles::KNIGHT,
                Role::Monk => roles::MONK,
                Role::Priest => roles::PRIEST,
                Role::Ranger => roles::RANGER,
                Role::Rogue => roles::ROGUE,
                Role::Samurai => roles::SAMURAI,
                Role::Tourist => roles::TOURIST,
                Role::Valkyrie => roles::VALKYRIE,
                Role::Wizard => roles::WIZARD,
            };
            let data_align = match character_choice.alignment {
                game_start::Alignment::Lawful => nethack_babel_data::Alignment::Lawful,
                game_start::Alignment::Neutral => nethack_babel_data::Alignment::Neutral,
                game_start::Alignment::Chaotic => nethack_babel_data::Alignment::Chaotic,
            };
            god_name(role_idx, data_align).to_string()
        };
        let role_name = character_choice.role_name.clone();

        let setup = create_world(&data, &mut rng, Some(&character_choice.name));
        let mut new_world = setup.world;
        new_world.set_spawn_catalogs(data.monsters.clone(), data.objects.clone());
        new_world.set_game_content(nethack_babel_engine::rumors::GameContent::load(&data_dir));

        // Apply role/race stats and name to the player.
        game_start::apply_character_choice(&mut new_world, &character_choice);

        // Spawn the starting pet.
        let _pet_events =
            game_start::spawn_starting_pet(&mut new_world, character_choice.role, &mut rng);

        (new_world, Some((deity_name, role_name)))
    };
    apply_runtime_options(&mut world, &cfg);

    // 8. Register the world for emergency save on panic.
    let player_name_for_panic = world.entity_name(world.player());
    let rng_state_for_panic: [u8; 32] = rng.random();
    // SAFETY: `world` lives until `unregister_panic_save()` below.
    unsafe {
        save::register_panic_save(&world, &player_name_for_panic, rng_state_for_panic);
    }

    // 9. Run in the appropriate mode.
    let legacy_ref = legacy_info.as_ref().map(|(d, r)| (d.as_str(), r.as_str()));
    if cli.text {
        run_text_mode(
            &mut world,
            &data,
            &mut rng,
            &mut recorder,
            &locale,
            cli.debug,
        )?;
    } else {
        run_tui_mode(
            &mut world,
            &data,
            &mut rng,
            TuiRuntimeContext {
                locale: &mut locale,
                cfg: &mut cfg,
                config_path: &cli.config,
                legacy_info: legacy_ref,
                wizard_mode: cli.debug,
            },
        )?;
    }

    // Unregister panic save before dropping the world.
    save::unregister_panic_save();

    // Save the recording if one was active.
    if let Some(ref rec) = recorder {
        match rec.save() {
            Ok(()) => {
                if cli.text {
                    println!("Session recorded to: {}", rec.output_path().display());
                }
            }
            Err(e) => eprintln!("Warning: failed to save recording: {e}"),
        }
    }

    Ok(())
}
