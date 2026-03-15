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

use nethack_babel_data::{load_game_data, Color as NhColor, GameData, MonsterDef, ObjectClass};
use nethack_babel_engine::action::{Direction, PlayerAction, Position};
use nethack_babel_engine::dungeon::{LevelMap, Terrain};
use nethack_babel_engine::event::EngineEvent;
use nethack_babel_engine::fov::FovMap;
use nethack_babel_engine::map_gen::{generate_level, Room};
use nethack_babel_engine::turn::resolve_turn;
use nethack_babel_engine::world::{
    Attributes, ArmorClass, DisplaySymbol, ExperienceLevel,
    GameWorld, HitPoints, Monster, MovementPoints, Name, Nutrition,
    Positioned, Power, Speed, NORMAL_SPEED,
};

use nethack_babel_i18n::locale::{LanguageManifest, LocaleManager};
use nethack_babel_tui::{
    App, DisplayCell, InventoryI18n, MapView, Menu, MenuHow, MenuItem, MenuResult,
    MessageUrgency, StatusLine, TermColor, TuiMessages, TuiPort, WindowPort,
    MAP_COLS, MAP_ROWS,
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
    /// (Phase 5 stub -- not yet implemented)
    #[arg(long, value_name = "ADDR:PORT")]
    server: Option<Option<String>>,

    /// Record the game session to an asciinema v2 compatible file
    #[arg(long, value_name = "FILE")]
    record: Option<String>,

    /// Replay a previously recorded session (Phase 5 stub)
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
        Terrain::Floor => TermColor::Rgb(170, 170, 170),        // gray
        Terrain::Corridor => TermColor::Rgb(170, 170, 170),     // gray
        Terrain::Wall => TermColor::Rgb(170, 170, 170),         // gray
        Terrain::Stone => TermColor::Rgb(170, 170, 170),        // gray
        Terrain::DoorOpen => TermColor::Rgb(170, 85, 0),        // brown
        Terrain::DoorClosed => TermColor::Rgb(170, 85, 0),      // brown/yellow
        Terrain::DoorLocked => TermColor::Rgb(170, 85, 0),      // brown/yellow
        Terrain::StairsUp => TermColor::Rgb(255, 255, 255),     // white
        Terrain::StairsDown => TermColor::Rgb(255, 255, 255),   // white
        Terrain::Altar => TermColor::Rgb(170, 170, 170),        // gray
        Terrain::Fountain => TermColor::Rgb(0, 0, 170),         // blue
        Terrain::Throne => TermColor::Rgb(255, 255, 85),        // yellow
        Terrain::Sink => TermColor::Rgb(170, 170, 170),         // gray
        Terrain::Grave => TermColor::Rgb(170, 170, 170),        // gray
        Terrain::Pool => TermColor::Rgb(0, 0, 170),             // blue
        Terrain::Moat => TermColor::Rgb(0, 0, 170),             // blue
        Terrain::Water => TermColor::Rgb(0, 0, 170),            // blue
        Terrain::Lava => TermColor::Rgb(170, 0, 0),             // red
        Terrain::Ice => TermColor::Rgb(85, 255, 255),           // bright cyan
        Terrain::Air => TermColor::Rgb(85, 255, 255),           // bright cyan
        Terrain::Cloud => TermColor::Rgb(170, 170, 170),        // gray
        Terrain::Tree => TermColor::Rgb(0, 170, 0),             // green
        Terrain::IronBars => TermColor::Rgb(0, 170, 170),       // cyan
        Terrain::Drawbridge => TermColor::Rgb(170, 85, 0),      // brown
        Terrain::MagicPortal => TermColor::Rgb(255, 85, 255),   // bright magenta
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
        attrs.dexterity,
        attrs.constitution,
        attrs.intelligence,
        attrs.wisdom,
        attrs.charisma,
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
        nethack_babel_engine::dungeon::DungeonBranch::Endgame => {
            match world.dungeon().depth {
                1 => "Earth".to_string(),
                2 => "Air".to_string(),
                3 => "Fire".to_string(),
                4 => "Water".to_string(),
                5 => "Astral".to_string(),
                _ => format!("End:{}", world.dungeon().depth),
            }
        }
    };

    let hp = world
        .get_component::<HitPoints>(player)
        .map(|h| *h)
        .unwrap_or(HitPoints {
            current: 0,
            max: 0,
        });
    let pw = world
        .get_component::<Power>(player)
        .map(|p| *p)
        .unwrap_or(Power {
            current: 0,
            max: 0,
        });
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

    // Gold: TODO: read actual gold from player inventory/wallet component.
    let gold = 0i64;

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

    StatusLine {
        line1,
        line2,
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
            if x >= 0
                && y >= 0
                && (x as usize) < width
                && (y as usize) < height
            {
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
                    tracing::warn!(
                        "Failed to load entity translations for '{}': {}",
                        code,
                        e
                    );
                }
            }

            // Load classifier (classifiers.toml).
            let clf_path = entry.path().join("classifiers.toml");
            if clf_path.exists() {
                let clf_str = std::fs::read_to_string(&clf_path)
                    .with_context(|| format!("reading {}", clf_path.display()))?;
                if let Err(e) = locale.load_classifier(&clf_str) {
                    tracing::warn!(
                        "Failed to load classifiers for '{}': {}",
                        code,
                        e
                    );
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
            EngineEvent::HpChange {
                amount, ..
            } => {
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
                messages.push((
                    format!("{} (+{amount})", msg),
                    MessageUrgency::Healing,
                ));
            }
            EngineEvent::HungerChange {
                new_level, ..
            } => {
                let urgency = match new_level {
                    nethack_babel_engine::event::HungerLevel::Weak
                    | nethack_babel_engine::event::HungerLevel::Fainting
                    | nethack_babel_engine::event::HungerLevel::Fainted
                    | nethack_babel_engine::event::HungerLevel::Starved => {
                        MessageUrgency::Danger
                    }
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
                        let wpn_name =
                            locale.translate_object_name(&wpn_en).to_string();
                        let mut args = FluentArgs::new();
                        args.set("attacker", att_name.clone());
                        args.set("defender", def_name.clone());
                        args.set("weapon", wpn_name.clone());
                        let msg =
                            locale.translate("melee-hit-weapon", Some(&args));
                        let text = format!("{} ({damage})", msg);
                        messages.push((text, MessageUrgency::Normal));
                    }
                    None => {
                        let mut args = FluentArgs::new();
                        args.set("attacker", att_name.clone());
                        args.set("defender", def_name.clone());
                        let msg =
                            locale.translate("melee-hit-bare", Some(&args));
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
                let item_name =
                    locale.translate_object_name(&item_en).to_string();
                let mut args = FluentArgs::new();
                args.set("actor", actor_name.clone());
                args.set("item", item_name.clone());
                args.set("quantity", *quantity as i64);
                let text = locale.translate("item-picked-up", Some(&args));
                messages.push((text, MessageUrgency::Normal));
            }
            EngineEvent::GameOver { score, .. } => {
                let mut args = FluentArgs::new();
                args.set("score", *score as i64);
                let text = locale.translate("game-over", Some(&args));
                messages.push((text, MessageUrgency::Danger));
            }
            // Silent events.
            EngineEvent::EntityMoved { .. }
            | EngineEvent::TurnEnd { .. } => {}
            _ => {}
        }
    }
    messages
}

/// Display name for an entity in the events_to_messages context.
/// Returns "You" (or translated equivalent) for the player, and a
/// translated monster name for non-player entities.
fn entity_display_name(
    entity: hecs::Entity,
    world: &GameWorld,
    locale: &LocaleManager,
) -> String {
    if world.is_player(entity) {
        locale.translate("you", None)
    } else {
        let en_name = world.entity_name(entity);
        locale.translate_monster_name(&en_name).to_string()
    }
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
    for (_entity, (pos, name, _monster)) in world
        .ecs()
        .query::<(&Positioned, &Name, &Monster)>()
        .iter()
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
fn display_events_text(events: &[EngineEvent]) {
    for event in events {
        match event {
            EngineEvent::EntityMoved { .. } | EngineEvent::TurnEnd { .. } => {}
            EngineEvent::Message { key, args } => {
                // In text-only mode, just show the key (no locale available here)
                if args.is_empty() {
                    println!("  {key}");
                } else {
                    let arg_str: Vec<String> = args.iter().map(|(k, v)| format!("{k}={v}")).collect();
                    println!("  {key} ({})", arg_str.join(", "));
                }
            }
            EngineEvent::HpChange {
                amount, source, ..
            } => {
                println!("  [HP change: {amount:+} ({source:?})]");
            }
            EngineEvent::PwChange { amount, .. } => {
                println!("  [PW regen: +{amount}]");
            }
            EngineEvent::HungerChange {
                old, new_level, ..
            } => {
                println!("  You feel {new_level:?}. (was {old:?})");
            }
            EngineEvent::DoorOpened { .. } => println!("  The door opens."),
            EngineEvent::DoorClosed { .. } => println!("  The door closes."),
            EngineEvent::DoorBroken { .. } => println!("  The door crashes open!"),
            EngineEvent::MeleeHit { damage, .. } => {
                println!("  You hit! ({damage} damage)");
            }
            EngineEvent::MeleeMiss { .. } => println!("  You miss."),
            EngineEvent::EntityDied { cause, .. } => {
                println!("  Something dies. ({cause:?})");
            }
            EngineEvent::GameOver { cause, score } => {
                println!("  *** GAME OVER *** Cause: {cause:?}, Score: {score}");
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Input parsing (text mode)
// ---------------------------------------------------------------------------

/// Parse a single-character command from stdin into a PlayerAction.
fn parse_command(input: &str) -> Option<PlayerAction> {
    let ch = input.trim().chars().next()?;
    match ch {
        'h' => Some(PlayerAction::Move {
            direction: Direction::West,
        }),
        'j' => Some(PlayerAction::Move {
            direction: Direction::South,
        }),
        'k' => Some(PlayerAction::Move {
            direction: Direction::North,
        }),
        'l' => Some(PlayerAction::Move {
            direction: Direction::East,
        }),
        'y' => Some(PlayerAction::Move {
            direction: Direction::NorthWest,
        }),
        'u' => Some(PlayerAction::Move {
            direction: Direction::NorthEast,
        }),
        'b' => Some(PlayerAction::Move {
            direction: Direction::SouthWest,
        }),
        'n' => Some(PlayerAction::Move {
            direction: Direction::SouthEast,
        }),
        '.' => Some(PlayerAction::Rest),
        's' => Some(PlayerAction::Search),
        '<' => Some(PlayerAction::GoUp),
        '>' => Some(PlayerAction::GoDown),
        _ => None,
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
fn try_load_save(player_name: &str) -> Option<(GameWorld, u32, i32, [u8; 32])> {
    let save_path = save::save_file_path(player_name);
    if !save_path.exists() {
        return None;
    }
    match save::load_game(&save_path) {
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

fn create_world(
    data: &GameData,
    rng: &mut Pcg64,
    player_name: Option<&str>,
) -> WorldSetup {
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

    let mut world = GameWorld::new(player_start);

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
    spawn_monsters(
        &mut world,
        data,
        &level.rooms,
        player_room_idx,
        depth,
        rng,
    );

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

fn show_game_options(
    port: &mut TuiPort,
    cfg: &mut config::Config,
    locale: &LocaleManager,
    config_path: &str,
) {
    loop {
        let items = vec![
            bool_item('a', &locale.translate("opt-autopickup", None), cfg.behavior.autopickup),
            MenuItem {
                accelerator: 'b',
                text: format!("{}: {}",
                    locale.translate("opt-autopickup-types", None),
                    cfg.behavior.autopickup_types),
                selected: false, selectable: true, group: None,
            },
            bool_item('c', &locale.translate("opt-legacy", None), cfg.game.legacy),
        ];
        let menu = Menu {
            title: locale.translate("ui-options-game", None),
            items,
            how: MenuHow::PickOne,
        };
        match port.show_menu(&menu) {
            MenuResult::Selected(indices) if !indices.is_empty() => {
                match indices[0] {
                    0 => cfg.behavior.autopickup = !cfg.behavior.autopickup,
                    1 => {
                        let prompt = format!("{}: ",
                            locale.translate("opt-autopickup-types", None));
                        if let Some(val) = port.get_line(&prompt)
                            && !val.is_empty()
                        {
                            cfg.behavior.autopickup_types = val;
                        }
                    }
                    2 => cfg.game.legacy = !cfg.game.legacy,
                    _ => {}
                }
                let _ = config::save_config(cfg, config_path);
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
            bool_item('a', &locale.translate("opt-map-colors", None), cfg.display.map_colors),
            bool_item('b', &locale.translate("opt-message-colors", None), cfg.display.message_colors),
            bool_item('c', &locale.translate("opt-buc-highlight", None), cfg.display.buc_highlight),
            bool_item('d', &locale.translate("opt-minimap", None), cfg.display.minimap),
            bool_item('e', &locale.translate("opt-mouse-hover", None), cfg.display.mouse_hover_info),
            bool_item('f', &locale.translate("opt-nerd-fonts", None), cfg.display.nerd_fonts),
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
            bool_item('a', &locale.translate("opt-sound-enabled", None), cfg.sound.enabled),
            MenuItem {
                accelerator: 'b',
                text: format!("{}: {}", locale.translate("opt-volume", None), cfg.sound.volume),
                selected: false, selectable: true, group: None,
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
                        let prompt = format!("{} (0-100): ",
                            locale.translate("opt-volume", None));
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

fn show_language_menu(
    port: &mut TuiPort,
    locale: &mut LocaleManager,
    app: &mut App,
) {
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

fn run_tui_mode(
    world: &mut GameWorld,
    _data: &GameData,
    rng: &mut Pcg64,
    locale: &mut LocaleManager,
    cfg: &mut config::Config,
    config_path: &str,
    legacy_info: Option<(&str, &str)>,
) -> Result<()> {
    install_panic_hook();

    let mut port = TuiPort::create()
        .map_err(|e| anyhow::anyhow!("Failed to initialize TUI: {e}"))?;
    port.init();

    let mut app = App::new();
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
            format!("NetHack Babel v{} -- {}", env!("CARGO_PKG_VERSION"), welcome)
        };
        app.push_message(text, MessageUrgency::System);
    }

    // Show legacy intro narrative if enabled.
    if cfg.game.legacy && let Some((deity, role)) = legacy_info {
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
                    "help-move", "", "help-move-diagram", "",
                    "help-attack", "help-wait", "help-search",
                    "help-inventory", "help-pickup", "help-drop",
                    "help-stairs-up", "help-stairs-down",
                    "help-eat", "help-quaff", "help-read",
                    "help-wield", "help-wear", "help-remove", "help-zap",
                    "help-options", "help-look", "help-history",
                    "", "help-shift-run", "help-arrows",
                    "", "help-symbols-title",
                ];
                let symbol_keys = [
                    "help-symbol-player", "help-symbol-floor",
                    "help-symbol-corridor", "help-symbol-door-closed",
                    "help-symbol-door-open", "help-symbol-stairs-up",
                    "help-symbol-stairs-down", "help-symbol-water",
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
                nethack_babel_tui::show_inventory(
                    &mut port, &[], Some(&inv_i18n),
                );
                continue;
            }
            Some(PlayerAction::ViewEquipped) => {
                let title = locale.translate("ui-equipment-title", None);
                let body = locale.translate("ui-equipment-empty", None);
                port.show_text(&title, &body);
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
                app.push_message(locale.translate("ui-save-prompt", None), MessageUrgency::System);
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
                        app.push_message(
                            format!("Save failed: {e}"),
                            MessageUrgency::System,
                        );
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
                        app.push_message(locale.translate("ui-save-success", None), MessageUrgency::System);
                    }
                    Err(e) => {
                        app.push_message(
                            format!("Save failed: {e}"),
                            MessageUrgency::System,
                        );
                    }
                }
                continue;
            }
            Some(PlayerAction::Options) => {
                // Top-level options menu loop
                loop {
                    let lang_label = locale.manifest_for(locale.current_language())
                        .map(|m| m.name.clone())
                        .unwrap_or_default();
                    let items = vec![
                        MenuItem {
                            accelerator: 'a',
                            text: locale.translate("ui-options-game", None),
                            selected: false, selectable: true, group: None,
                        },
                        MenuItem {
                            accelerator: 'b',
                            text: locale.translate("ui-options-display", None),
                            selected: false, selectable: true, group: None,
                        },
                        MenuItem {
                            accelerator: 'c',
                            text: locale.translate("ui-options-sound", None),
                            selected: false, selectable: true, group: None,
                        },
                        MenuItem {
                            accelerator: 'd',
                            text: format!("{}: {}",
                                locale.translate("ui-select-language", None),
                                lang_label),
                            selected: false, selectable: true, group: None,
                        },
                    ];
                    let menu = Menu {
                        title: locale.translate("ui-options-title", None),
                        items,
                        how: MenuHow::PickOne,
                    };
                    match port.show_menu(&menu) {
                        MenuResult::Selected(indices) if !indices.is_empty() => {
                            match indices[0] {
                                0 => show_game_options(&mut port, cfg, locale, config_path),
                                1 => show_display_options(&mut port, cfg, locale, config_path),
                                2 => show_sound_options(&mut port, cfg, locale, config_path),
                                3 => show_language_menu(&mut port, locale, &mut app),
                                _ => {}
                            }
                        }
                        _ => break,
                    }
                }
                continue;
            }
            Some(a) => a,
            None => continue, // Unmapped key or Escape.
        };

        // Resolve the turn.
        let events = resolve_turn(world, action, rng);

        // Convert events to messages.
        let messages = events_to_messages(&events, locale, world);
        for (text, urgency) in &messages {
            app.push_message(text.clone(), *urgency);
        }

        // Check for game over.
        for event in &events {
            if let EngineEvent::GameOver { cause, score } = event {
                game_over = true;
                // Show tombstone.
                let epitaph = world.entity_name(world.player());
                let info = format!("Cause: {cause:?} -- Score: {score}");
                port.render_tombstone(&epitaph, &info);
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
    _data: &GameData,
    rng: &mut Pcg64,
    recorder: &mut Option<recording::GameRecorder>,
    locale: &LocaleManager,
) -> Result<()> {
    println!();
    println!("{}", locale.translate("event-dungeon-welcome", None));
    println!("Commands: h/j/k/l/y/u/b/n = move, . = wait, < = up, > = down, q = quit, ? = help");
    println!();

    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut game_over = false;

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

        if trimmed == "q" || trimmed == "quit" {
            println!("{}", locale.translate("ui-goodbye", None));
            break;
        }
        if trimmed == "?" || trimmed == "help" {
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
            println!("  <  = go up stairs");
            println!("  >  = go down stairs");
            println!("  q  = quit the game");
            println!("  ?  = show this help");
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

        let action = match parse_command(trimmed) {
            Some(a) => a,
            None => {
                if !trimmed.is_empty() {
                    let mut args = fluent::FluentArgs::new();
                    args.set("key", trimmed.to_string());
                    println!("  {}", locale.translate("ui-unknown-command", Some(&args)));
                }
                continue;
            }
        };

        let events = resolve_turn(world, action, rng);
        display_events_text(&events);

        for event in &events {
            if matches!(event, EngineEvent::GameOver { .. }) {
                game_over = true;
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

    // -- Handle --server (Phase 5 stub) ------------------------------------
    if let Some(ref server_opt) = cli.server {
        let addr = server_opt
            .as_deref()
            .unwrap_or(server::DEFAULT_BIND_ADDR);
        let srv = server::GameServer::new(addr, server::DEFAULT_MAX_CONNECTIONS);
        match srv.start() {
            Ok(()) => return Ok(()),
            Err(server::ServerError::NotImplemented) => {
                println!("Server mode not yet implemented.");
                return Ok(());
            }
            Err(e) => anyhow::bail!("Server error: {e}"),
        }
    }

    // -- Handle --replay (Phase 5 stub) -----------------------------------
    if let Some(ref replay_path) = cli.replay {
        let path = std::path::PathBuf::from(replay_path);
        match recording::replay_session(&path) {
            Ok(()) => return Ok(()),
            Err(recording::RecordingError::ReplayNotImplemented) => {
                println!("Replay mode not yet implemented.");
                return Ok(());
            }
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

    // 7. Try to restore from a save file, or create a new world.
    //    If creating a new world, run character selection first.
    let player_name_for_save = cli.name.as_deref().unwrap_or("you");
    let (mut world, legacy_info) = if let Some((restored_world, turn, depth, rng_state)) =
        try_load_save(player_name_for_save)
    {
        // Re-seed the RNG from the saved state.
        rng = Pcg64::from_seed(rng_state);
        if cli.text {
            println!(
                "Restored saved game: turn {turn}, depth {depth}."
            );
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
            let mut port = TuiPort::create()
                .map_err(|e| anyhow::anyhow!("Failed to initialize TUI: {e}"))?;
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
            println!("{}", locale.translate("event-player-role", Some(&role_args)));
            println!();
        }

        // Compute deity name for legacy intro.
        let deity_name = {
            use nethack_babel_engine::religion::{god_name, roles};
            use nethack_babel_engine::pets::Role;
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

        let setup = create_world(
            &data,
            &mut rng,
            Some(&character_choice.name),
        );
        let mut new_world = setup.world;

        // Apply role/race stats and name to the player.
        game_start::apply_character_choice(&mut new_world, &character_choice);

        // Spawn the starting pet.
        let _pet_events =
            game_start::spawn_starting_pet(&mut new_world, character_choice.role, &mut rng);

        (new_world, Some((deity_name, role_name)))
    };

    // 8. Run in the appropriate mode.
    let legacy_ref = legacy_info
        .as_ref()
        .map(|(d, r)| (d.as_str(), r.as_str()));
    if cli.text {
        run_text_mode(&mut world, &data, &mut rng, &mut recorder, &locale)?;
    } else {
        run_tui_mode(&mut world, &data, &mut rng, &mut locale, &mut cfg, &cli.config, legacy_ref)?;
    }

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
