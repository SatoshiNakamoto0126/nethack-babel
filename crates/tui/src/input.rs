//! Key mapping from crossterm events to game actions.
//!
//! Implements the traditional NetHack keybindings: vi-keys for movement,
//! Shift+vi-key for long movement, and the standard command alphabet.
//! Also provides the complete list of 162 extended commands and 33 wizard
//! mode debug commands from NetHack 3.7.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nethack_babel_engine::action::{Direction, PlayerAction};

// ---------------------------------------------------------------------------
// Prompt kinds for keys that need further input
// ---------------------------------------------------------------------------

/// What kind of follow-up prompt a key requires before it can produce
/// a [`PlayerAction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    /// The key needs an inventory letter (e.g. wield, wear, drop).
    Item {
        /// Short prompt string shown to the player.
        prompt: &'static str,
        /// The command tag, used by [`App::build_prompted_action`] to
        /// construct the correct [`PlayerAction`] variant.
        command: ItemCommand,
    },
    /// The key needs a direction (e.g. open door, close door, fight).
    Direction {
        prompt: &'static str,
        command: DirectionCommand,
    },
    /// The key needs an inventory letter *followed by* a direction
    /// (e.g. throw, zap wand).
    ItemThenDirection {
        item_prompt: &'static str,
        dir_prompt: &'static str,
        command: ItemDirectionCommand,
    },
}

/// Commands that require a single inventory letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemCommand {
    Drop,
    Wield,
    Wear,
    TakeOff,
    PutOn,
    Remove,
    Apply,
    Quiver,
    Invoke,
    Offer,
    Rub,
    Dip,
    Tip,
    Force,
}

/// Commands that require a direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectionCommand {
    Open,
    Close,
    Fight,
    Kick,
    Chat,
    Untrap,
    Run,
    Rush,
}

/// Commands that require an inventory letter and then a direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemDirectionCommand {
    Throw,
    ZapWand,
}

/// Check whether a key event requires a follow-up prompt.
///
/// Returns `Some(PromptKind)` if the key is a known command that needs
/// further input, or `None` if it either maps directly via [`map_key`]
/// or is completely unknown.
pub fn key_needs_prompt(key: KeyEvent) -> Option<PromptKind> {
    let mods = key.modifiers & (KeyModifiers::SHIFT | KeyModifiers::CONTROL);

    match (mods, key.code) {
        // ── Item-only prompts ──────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('d')) => Some(PromptKind::Item {
            prompt: "Drop what? [a-zA-Z or ?*]",
            command: ItemCommand::Drop,
        }),
        (KeyModifiers::NONE, KeyCode::Char('w')) => Some(PromptKind::Item {
            prompt: "Wield what? [a-zA-Z or - for bare hands]",
            command: ItemCommand::Wield,
        }),
        (KeyModifiers::NONE, KeyCode::Char('W'))
        | (KeyModifiers::SHIFT, KeyCode::Char('W' | 'w')) => Some(PromptKind::Item {
            prompt: "Wear what? [a-zA-Z or ?*]",
            command: ItemCommand::Wear,
        }),
        (KeyModifiers::NONE, KeyCode::Char('T'))
        | (KeyModifiers::SHIFT, KeyCode::Char('T' | 't')) => Some(PromptKind::Item {
            prompt: "Take off what? [a-zA-Z or ?*]",
            command: ItemCommand::TakeOff,
        }),
        (KeyModifiers::NONE, KeyCode::Char('P'))
        | (KeyModifiers::SHIFT, KeyCode::Char('P' | 'p')) => Some(PromptKind::Item {
            prompt: "Put on what? [a-zA-Z or ?*]",
            command: ItemCommand::PutOn,
        }),
        (KeyModifiers::NONE, KeyCode::Char('R'))
        | (KeyModifiers::SHIFT, KeyCode::Char('R' | 'r')) => Some(PromptKind::Item {
            prompt: "Remove what? [a-zA-Z or ?*]",
            command: ItemCommand::Remove,
        }),
        (KeyModifiers::NONE, KeyCode::Char('a')) => Some(PromptKind::Item {
            prompt: "Apply what? [a-zA-Z or ?*]",
            command: ItemCommand::Apply,
        }),
        (KeyModifiers::NONE, KeyCode::Char('Q'))
        | (KeyModifiers::SHIFT, KeyCode::Char('Q' | 'q')) => Some(PromptKind::Item {
            prompt: "Ready what? [a-zA-Z or ?*]",
            command: ItemCommand::Quiver,
        }),

        // ── Direction-only prompts ─────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('o')) => Some(PromptKind::Direction {
            prompt: "In what direction?",
            command: DirectionCommand::Open,
        }),
        (KeyModifiers::NONE, KeyCode::Char('c')) => Some(PromptKind::Direction {
            prompt: "In what direction?",
            command: DirectionCommand::Close,
        }),
        (KeyModifiers::NONE, KeyCode::Char('F'))
        | (KeyModifiers::SHIFT, KeyCode::Char('F' | 'f')) => Some(PromptKind::Direction {
            prompt: "In what direction?",
            command: DirectionCommand::Fight,
        }),
        (KeyModifiers::NONE, KeyCode::Char('g')) => Some(PromptKind::Direction {
            prompt: "Rush in what direction?",
            command: DirectionCommand::Rush,
        }),
        (KeyModifiers::NONE, KeyCode::Char('G'))
        | (KeyModifiers::SHIFT, KeyCode::Char('G' | 'g')) => Some(PromptKind::Direction {
            prompt: "Run in what direction?",
            command: DirectionCommand::Run,
        }),
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => Some(PromptKind::Direction {
            prompt: "In what direction?",
            command: DirectionCommand::Kick,
        }),

        // ── Item + direction prompts ───────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('t')) => Some(PromptKind::ItemThenDirection {
            item_prompt: "Throw what? [a-zA-Z or ?*]",
            dir_prompt: "In what direction?",
            command: ItemDirectionCommand::Throw,
        }),
        (KeyModifiers::NONE, KeyCode::Char('z')) => Some(PromptKind::ItemThenDirection {
            item_prompt: "Zap what? [a-zA-Z or ?*]",
            dir_prompt: "In what direction?",
            command: ItemDirectionCommand::ZapWand,
        }),

        _ => None,
    }
}

/// Prompt mapping for `#extended` commands that need follow-up input.
///
/// This uses the same [`PromptKind`] flow as key-based commands so both
/// input paths share one action-construction pipeline.
pub fn extended_command_needs_prompt(name: &str) -> Option<PromptKind> {
    match name.trim().to_lowercase().as_str() {
        // ── Direction-only prompts ─────────────────────────────
        "open" => Some(PromptKind::Direction {
            prompt: "In what direction?",
            command: DirectionCommand::Open,
        }),
        "close" => Some(PromptKind::Direction {
            prompt: "In what direction?",
            command: DirectionCommand::Close,
        }),
        "kick" => Some(PromptKind::Direction {
            prompt: "In what direction?",
            command: DirectionCommand::Kick,
        }),
        "chat" => Some(PromptKind::Direction {
            prompt: "In what direction?",
            command: DirectionCommand::Chat,
        }),
        "fight" => Some(PromptKind::Direction {
            prompt: "In what direction?",
            command: DirectionCommand::Fight,
        }),
        "untrap" => Some(PromptKind::Direction {
            prompt: "In what direction?",
            command: DirectionCommand::Untrap,
        }),
        "run" => Some(PromptKind::Direction {
            prompt: "Run in what direction?",
            command: DirectionCommand::Run,
        }),
        "rush" => Some(PromptKind::Direction {
            prompt: "Rush in what direction?",
            command: DirectionCommand::Rush,
        }),

        // ── Item-only prompts ──────────────────────────────────
        "wear" => Some(PromptKind::Item {
            prompt: "Wear what? [a-zA-Z or ?*]",
            command: ItemCommand::Wear,
        }),
        "wield" => Some(PromptKind::Item {
            prompt: "Wield what? [a-zA-Z or - for bare hands]",
            command: ItemCommand::Wield,
        }),
        "takeoff" => Some(PromptKind::Item {
            prompt: "Take off what? [a-zA-Z or ?*]",
            command: ItemCommand::TakeOff,
        }),
        "puton" => Some(PromptKind::Item {
            prompt: "Put on what? [a-zA-Z or ?*]",
            command: ItemCommand::PutOn,
        }),
        "remove" => Some(PromptKind::Item {
            prompt: "Remove what? [a-zA-Z or ?*]",
            command: ItemCommand::Remove,
        }),
        "drop" => Some(PromptKind::Item {
            prompt: "Drop what? [a-zA-Z or ?*]",
            command: ItemCommand::Drop,
        }),
        "apply" => Some(PromptKind::Item {
            prompt: "Apply what? [a-zA-Z or ?*]",
            command: ItemCommand::Apply,
        }),
        "quiver" => Some(PromptKind::Item {
            prompt: "Ready what? [a-zA-Z or ?*]",
            command: ItemCommand::Quiver,
        }),
        "invoke" => Some(PromptKind::Item {
            prompt: "Invoke what? [a-zA-Z or ?*]",
            command: ItemCommand::Invoke,
        }),
        "rub" => Some(PromptKind::Item {
            prompt: "Rub what? [a-zA-Z or ?*]",
            command: ItemCommand::Rub,
        }),
        "tip" => Some(PromptKind::Item {
            prompt: "Tip what? [a-zA-Z or ?*]",
            command: ItemCommand::Tip,
        }),
        "offer" => Some(PromptKind::Item {
            prompt: "Offer what? [a-zA-Z or ?*]",
            command: ItemCommand::Offer,
        }),
        "force" => Some(PromptKind::Item {
            prompt: "Force lock with what? [a-zA-Z or ?*]",
            command: ItemCommand::Force,
        }),

        // ── Item + direction prompts ───────────────────────────
        "throw" => Some(PromptKind::ItemThenDirection {
            item_prompt: "Throw what? [a-zA-Z or ?*]",
            dir_prompt: "In what direction?",
            command: ItemDirectionCommand::Throw,
        }),
        "zap" => Some(PromptKind::ItemThenDirection {
            item_prompt: "Zap what? [a-zA-Z or ?*]",
            dir_prompt: "In what direction?",
            command: ItemDirectionCommand::ZapWand,
        }),
        _ => None,
    }
}

/// Map a crossterm key event to a player action.
///
/// Returns `None` for keys that require further input (e.g. `o` needs a
/// direction prompt) or that have no mapping at all.
pub fn map_key(key: KeyEvent) -> Option<PlayerAction> {
    // Strip all modifiers except SHIFT and CONTROL for matching purposes.
    let mods = key.modifiers & (KeyModifiers::SHIFT | KeyModifiers::CONTROL);

    match (mods, key.code) {
        // ── Movement: hjklyubn ───────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('h')) => Some(PlayerAction::Move {
            direction: Direction::West,
        }),
        (KeyModifiers::NONE, KeyCode::Char('j')) => Some(PlayerAction::Move {
            direction: Direction::South,
        }),
        (KeyModifiers::NONE, KeyCode::Char('k')) => Some(PlayerAction::Move {
            direction: Direction::North,
        }),
        (KeyModifiers::NONE, KeyCode::Char('l')) => Some(PlayerAction::Move {
            direction: Direction::East,
        }),
        (KeyModifiers::NONE, KeyCode::Char('y')) => Some(PlayerAction::Move {
            direction: Direction::NorthWest,
        }),
        (KeyModifiers::NONE, KeyCode::Char('u')) => Some(PlayerAction::Move {
            direction: Direction::NorthEast,
        }),
        (KeyModifiers::NONE, KeyCode::Char('b')) => Some(PlayerAction::Move {
            direction: Direction::SouthWest,
        }),
        (KeyModifiers::NONE, KeyCode::Char('n')) => Some(PlayerAction::Move {
            direction: Direction::SouthEast,
        }),

        // ── Long movement: HJKLYUBN (Shift) ─────────────────────
        // crossterm delivers Shift+letter as KeyCode::Char('H') with SHIFT modifier,
        // or on some terminals as uppercase char without SHIFT flag.
        (KeyModifiers::SHIFT, KeyCode::Char('H' | 'h')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::West,
        }),
        (KeyModifiers::SHIFT, KeyCode::Char('J' | 'j')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::South,
        }),
        (KeyModifiers::SHIFT, KeyCode::Char('K' | 'k')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::North,
        }),
        (KeyModifiers::SHIFT, KeyCode::Char('L' | 'l')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::East,
        }),
        (KeyModifiers::SHIFT, KeyCode::Char('Y' | 'y')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::NorthWest,
        }),
        (KeyModifiers::SHIFT, KeyCode::Char('U' | 'u')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::NorthEast,
        }),
        (KeyModifiers::SHIFT, KeyCode::Char('B' | 'b')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::SouthWest,
        }),
        (KeyModifiers::SHIFT, KeyCode::Char('N' | 'n')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::SouthEast,
        }),
        // Also catch uppercase char with NONE modifier (some terminals).
        (KeyModifiers::NONE, KeyCode::Char('H')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::West,
        }),
        (KeyModifiers::NONE, KeyCode::Char('J')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::South,
        }),
        (KeyModifiers::NONE, KeyCode::Char('K')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::North,
        }),
        (KeyModifiers::NONE, KeyCode::Char('L')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::East,
        }),
        (KeyModifiers::NONE, KeyCode::Char('Y')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::NorthWest,
        }),
        (KeyModifiers::NONE, KeyCode::Char('U')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::NorthEast,
        }),
        (KeyModifiers::NONE, KeyCode::Char('B')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::SouthWest,
        }),
        (KeyModifiers::NONE, KeyCode::Char('N')) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::SouthEast,
        }),

        // ── Wait and Search ──────────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('.')) => Some(PlayerAction::Rest),
        (KeyModifiers::NONE, KeyCode::Char('s')) => Some(PlayerAction::Search),

        // ── Items ────────────────────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('i')) => Some(PlayerAction::ViewInventory),
        (KeyModifiers::NONE, KeyCode::Char('I'))
        | (KeyModifiers::SHIFT, KeyCode::Char('I' | 'i')) => Some(PlayerAction::ViewEquipped),
        (KeyModifiers::NONE, KeyCode::Char(',')) => Some(PlayerAction::PickUp),
        (KeyModifiers::NONE, KeyCode::Char('e')) => Some(PlayerAction::Eat { item: None }),
        (KeyModifiers::NONE, KeyCode::Char('q')) => Some(PlayerAction::Quaff { item: None }),
        (KeyModifiers::NONE, KeyCode::Char('r')) => Some(PlayerAction::Read { item: None }),
        (KeyModifiers::NONE, KeyCode::Char('f')) => Some(PlayerAction::Fire),

        // d — drop (single item, needs item prompt; return None to signal that)
        // D — drop multiple
        (KeyModifiers::NONE, KeyCode::Char('d')) => None, // needs item prompt
        (KeyModifiers::NONE, KeyCode::Char('D'))
        | (KeyModifiers::SHIFT, KeyCode::Char('D' | 'd')) => None, // needs multi-item prompt

        // z — zap wand (needs item + optional direction)
        (KeyModifiers::NONE, KeyCode::Char('z')) => None, // needs item prompt

        // Z — cast spell (needs spell selection + optional direction)
        (KeyModifiers::NONE, KeyCode::Char('Z'))
        | (KeyModifiers::SHIFT, KeyCode::Char('Z' | 'z')) => None, // needs spell prompt

        // w — wield weapon
        (KeyModifiers::NONE, KeyCode::Char('w')) => None, // needs item prompt

        // W — wear armor
        (KeyModifiers::NONE, KeyCode::Char('W'))
        | (KeyModifiers::SHIFT, KeyCode::Char('W' | 'w')) => None, // needs item prompt

        // T — take off armor
        (KeyModifiers::NONE, KeyCode::Char('T'))
        | (KeyModifiers::SHIFT, KeyCode::Char('T' | 't')) => None, // needs item prompt

        // P — put on accessory
        (KeyModifiers::NONE, KeyCode::Char('P'))
        | (KeyModifiers::SHIFT, KeyCode::Char('P' | 'p')) => None, // needs item prompt

        // R — remove accessory
        (KeyModifiers::NONE, KeyCode::Char('R'))
        | (KeyModifiers::SHIFT, KeyCode::Char('R' | 'r')) => None, // needs item prompt

        // a — apply (use) item
        (KeyModifiers::NONE, KeyCode::Char('a')) => None, // needs item prompt

        // t — throw
        (KeyModifiers::NONE, KeyCode::Char('t')) => None, // needs item + direction prompt

        // Q — select ammo for quiver
        (KeyModifiers::NONE, KeyCode::Char('Q'))
        | (KeyModifiers::SHIFT, KeyCode::Char('Q' | 'q')) => None, // needs item prompt

        // x — swap primary/secondary weapons
        (KeyModifiers::NONE, KeyCode::Char('x')) => Some(PlayerAction::Swap),

        // X — toggle two-weapon combat
        (KeyModifiers::NONE, KeyCode::Char('X'))
        | (KeyModifiers::SHIFT, KeyCode::Char('X' | 'x')) => Some(PlayerAction::ToggleTwoWeapon),

        // F — fight in direction (melee attack)
        (KeyModifiers::NONE, KeyCode::Char('F'))
        | (KeyModifiers::SHIFT, KeyCode::Char('F' | 'f')) => None, // needs direction prompt

        // E — engrave
        (KeyModifiers::NONE, KeyCode::Char('E'))
        | (KeyModifiers::SHIFT, KeyCode::Char('E' | 'e')) => None, // needs text prompt

        // C — call / name
        (KeyModifiers::NONE, KeyCode::Char('C'))
        | (KeyModifiers::SHIFT, KeyCode::Char('C' | 'c')) => None, // needs naming target

        // A — take off all armor
        (KeyModifiers::NONE, KeyCode::Char('A'))
        | (KeyModifiers::SHIFT, KeyCode::Char('A' | 'a')) => Some(PlayerAction::TakeOffAll),

        // @ — toggle autopickup
        (KeyModifiers::NONE, KeyCode::Char('@')) => Some(PlayerAction::Options),

        // ── Stairs ───────────────────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('<')) | (KeyModifiers::SHIFT, KeyCode::Char('<')) => {
            Some(PlayerAction::GoUp)
        }
        (KeyModifiers::NONE, KeyCode::Char('>')) | (KeyModifiers::SHIFT, KeyCode::Char('>')) => {
            Some(PlayerAction::GoDown)
        }

        // ── Doors ────────────────────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('o')) => None, // needs direction prompt
        (KeyModifiers::NONE, KeyCode::Char('c')) => None, // needs direction prompt

        // ── Kick (Ctrl+D) ────────────────────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => None, // needs direction prompt

        // ── Look / information ───────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char(':')) => Some(PlayerAction::LookHere),
        (KeyModifiers::NONE, KeyCode::Char(';')) => None, // needs position prompt (far look)
        (KeyModifiers::NONE, KeyCode::Char('/')) => None, // whatis — symbol lookup
        (KeyModifiers::NONE, KeyCode::Char('?')) => Some(PlayerAction::Help),
        (KeyModifiers::NONE, KeyCode::Char('\\')) => Some(PlayerAction::ViewDiscoveries),
        (KeyModifiers::NONE, KeyCode::Char('`')) => None, // knownclass — needs class prompt
        (KeyModifiers::NONE, KeyCode::Char('^')) => None, // showtrap — needs position
        (KeyModifiers::NONE, KeyCode::Char('&')) => Some(PlayerAction::Help), // whatdoes
        (KeyModifiers::NONE, KeyCode::Char('v')) => Some(PlayerAction::ShowVersion), // versionshort

        // ── See equipment sub-commands ───────────────────────────
        // * — show all equipment
        (KeyModifiers::NONE, KeyCode::Char('*')) => Some(PlayerAction::ViewEquipped),

        // ── Spellbook ────────────────────────────────────────────
        // '+' — show spells
        (KeyModifiers::NONE, KeyCode::Char('+')) | (KeyModifiers::SHIFT, KeyCode::Char('+')) => {
            Some(PlayerAction::ViewEquipped)
        }

        // ── Show gold ────────────────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('$')) => Some(PlayerAction::ViewInventory),

        // ── Message history: Ctrl+P ──────────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => Some(PlayerAction::ShowHistory),

        // ── Ctrl+X — show attributes ─────────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('x')) => Some(PlayerAction::Attributes),

        // ── Ctrl+O — overview ────────────────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('o')) => Some(PlayerAction::DungeonOverview),

        // ── Ctrl+T — teleport ────────────────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('t')) => None, // teleport, needs implementation

        // ── Ctrl+Z — suspend ─────────────────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('z')) => None, // suspend

        // ── Ctrl+R — redraw ──────────────────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('r')) => None, // redraw screen

        // ── Ctrl+A — repeat ──────────────────────────────────────
        (KeyModifiers::CONTROL, KeyCode::Char('a')) => None, // repeat last command

        // ── Payment ──────────────────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('p')) => Some(PlayerAction::Pay),

        // ── Travel (underscore) ──────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('_')) | (KeyModifiers::SHIFT, KeyCode::Char('_')) => {
            None
        } // needs position prompt

        // ── Shell (!) ────────────────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('!')) => None, // shell escape

        // ── Perminv (|) ──────────────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('|')) => None, // persistent inventory scroll

        // ── Extended command prefix ──────────────────────────────
        // '#' is the extended command prefix — the app layer should
        // prompt for the full command name.
        (KeyModifiers::NONE, KeyCode::Char('#')) => None, // extended command prefix

        // ── Options (language switch) ──────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('O'))
        | (KeyModifiers::SHIFT, KeyCode::Char('O' | 'o')) => Some(PlayerAction::Options),

        // ── Session control ──────────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Char('S'))
        | (KeyModifiers::SHIFT, KeyCode::Char('S' | 's')) => Some(PlayerAction::SaveAndQuit),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(PlayerAction::Quit),

        // ── Escape ───────────────────────────────────────────────
        (_, KeyCode::Esc) => None, // cancel / close

        // ── Arrow key movement ───────────────────────────────────
        (KeyModifiers::NONE, KeyCode::Up) => Some(PlayerAction::Move {
            direction: Direction::North,
        }),
        (KeyModifiers::NONE, KeyCode::Down) => Some(PlayerAction::Move {
            direction: Direction::South,
        }),
        (KeyModifiers::NONE, KeyCode::Left) => Some(PlayerAction::Move {
            direction: Direction::West,
        }),
        (KeyModifiers::NONE, KeyCode::Right) => Some(PlayerAction::Move {
            direction: Direction::East,
        }),
        (KeyModifiers::SHIFT, KeyCode::Up) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::North,
        }),
        (KeyModifiers::SHIFT, KeyCode::Down) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::South,
        }),
        (KeyModifiers::SHIFT, KeyCode::Left) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::West,
        }),
        (KeyModifiers::SHIFT, KeyCode::Right) => Some(PlayerAction::MoveUntilInterrupt {
            direction: Direction::East,
        }),

        // ── No match ─────────────────────────────────────────────
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Extended commands — complete list from NetHack 3.7 cmd.c extcmdlist[]
// ---------------------------------------------------------------------------

/// Metadata for an extended command.
#[derive(Debug, Clone, Copy)]
pub struct ExtendedCommand {
    /// The command name as typed after '#'.
    pub name: &'static str,
    /// The default key binding (empty string if no default key).
    pub default_key: &'static str,
    /// Short description of the command.
    pub description: &'static str,
    /// Whether this command autocompletes when typing.
    pub autocomplete: bool,
    /// Whether this is a wizard-mode-only command.
    pub wizard_only: bool,
}

/// Context for request-command style menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandMenuKind {
    Request,
    Here,
    There,
}

/// Complete list of all extended commands from NetHack 3.7.
///
/// This includes 99 regular extended commands and 33 wizard-mode commands,
/// sorted alphabetically within each category.
pub const EXTENDED_COMMANDS: &[ExtendedCommand] = &[
    // ── Regular extended commands (available to all players) ──────
    ExtendedCommand {
        name: "#",
        default_key: "#",
        description: "enter and perform an extended command",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "?",
        default_key: "M-?",
        description: "list all extended commands",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "adjust",
        default_key: "M-a",
        description: "adjust inventory letters",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "annotate",
        default_key: "M-A",
        description: "name current level",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "apply",
        default_key: "a",
        description: "apply (use) a tool",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "attributes",
        default_key: "C-x",
        description: "show your attributes",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "autopickup",
        default_key: "@",
        description: "toggle autopickup on/off",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "call",
        default_key: "C",
        description: "name a monster, object, or object type",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "cast",
        default_key: "Z",
        description: "zap (cast) a spell",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "chat",
        default_key: "M-c",
        description: "talk to someone",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "chronicle",
        default_key: "",
        description: "show journal of major events",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "close",
        default_key: "c",
        description: "close a door",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "conduct",
        default_key: "M-C",
        description: "list voluntary challenges maintained",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "dip",
        default_key: "M-d",
        description: "dip an object into something",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "down",
        default_key: ">",
        description: "go down a staircase",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "drop",
        default_key: "d",
        description: "drop an item",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "droptype",
        default_key: "D",
        description: "drop specific item types",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "eat",
        default_key: "e",
        description: "eat something",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "engrave",
        default_key: "E",
        description: "engrave writing on the floor",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "enhance",
        default_key: "M-e",
        description: "advance or check skills",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "exploremode",
        default_key: "M-X",
        description: "enter explore mode",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "fight",
        default_key: "F",
        description: "force fight in a direction",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "fire",
        default_key: "f",
        description: "fire ammunition from quiver",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "force",
        default_key: "M-f",
        description: "force a lock",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "genocided",
        default_key: "M-g",
        description: "list genocided/extinct monsters",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "glance",
        default_key: ";",
        description: "show what a map symbol is",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "help",
        default_key: "?",
        description: "give a help message",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "herecmdmenu",
        default_key: "",
        description: "show menu of commands for here",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "history",
        default_key: "V",
        description: "show game history",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "inventory",
        default_key: "i",
        description: "show your inventory",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "inventtype",
        default_key: "I",
        description: "show inventory by item class",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "invoke",
        default_key: "M-i",
        description: "invoke an object's powers",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "jump",
        default_key: "M-j",
        description: "jump to another location",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "kick",
        default_key: "C-d",
        description: "kick something",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "known",
        default_key: "\\",
        description: "show discovered object types",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "knownclass",
        default_key: "`",
        description: "show discovered types for one class",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "look",
        default_key: ":",
        description: "look at what is here",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "lookaround",
        default_key: "",
        description: "describe what you can see",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "loot",
        default_key: "M-l",
        description: "loot a box on the floor",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "monster",
        default_key: "M-m",
        description: "use monster's special ability",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "name",
        default_key: "M-n",
        description: "name a monster or object",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "offer",
        default_key: "M-o",
        description: "offer a sacrifice to the gods",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "open",
        default_key: "o",
        description: "open a door",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "options",
        default_key: "O",
        description: "show option settings",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "optionsfull",
        default_key: "",
        description: "show all options, change them",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "overview",
        default_key: "C-o",
        description: "summary of explored dungeon",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "pay",
        default_key: "p",
        description: "pay your shopping bill",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "perminv",
        default_key: "|",
        description: "scroll persistent inventory",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "pickup",
        default_key: ",",
        description: "pick up things here",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "pray",
        default_key: "M-p",
        description: "pray to the gods for help",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "prevmsg",
        default_key: "C-p",
        description: "view recent game messages",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "puton",
        default_key: "P",
        description: "put on an accessory",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "quaff",
        default_key: "q",
        description: "quaff (drink) something",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "quit",
        default_key: "",
        description: "exit without saving",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "quiver",
        default_key: "Q",
        description: "select ammunition for quiver",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "read",
        default_key: "r",
        description: "read a scroll or spellbook",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "redraw",
        default_key: "C-r",
        description: "redraw screen",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "remove",
        default_key: "R",
        description: "remove an accessory",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "repeat",
        default_key: "C-a",
        description: "repeat a previous command",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "reqmenu",
        default_key: "m",
        description: "request menu or modify command",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "retravel",
        default_key: "C-_",
        description: "travel to previous destination",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "ride",
        default_key: "M-R",
        description: "mount or dismount a steed",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "rub",
        default_key: "M-r",
        description: "rub a lamp or a stone",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "run",
        default_key: "G",
        description: "run until something interesting",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "rush",
        default_key: "g",
        description: "rush until something interesting",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "save",
        default_key: "S",
        description: "save the game and exit",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "saveoptions",
        default_key: "",
        description: "save the game configuration",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "search",
        default_key: "s",
        description: "search for traps and secret doors",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "seeall",
        default_key: "*",
        description: "show all equipment in use",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "seeamulet",
        default_key: "\"",
        description: "show amulet currently worn",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "seearmor",
        default_key: "[",
        description: "show armor currently worn",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "seerings",
        default_key: "=",
        description: "show ring(s) currently worn",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "seetools",
        default_key: "(",
        description: "show tools currently in use",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "seeweapon",
        default_key: ")",
        description: "show weapon currently wielded",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "shell",
        default_key: "!",
        description: "enter a sub-shell",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "showgold",
        default_key: "$",
        description: "show gold, shop credit/debt",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "showspells",
        default_key: "+",
        description: "list and reorder known spells",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "showtrap",
        default_key: "^",
        description: "describe an adjacent trap",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "sit",
        default_key: "M-s",
        description: "sit down",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "suspend",
        default_key: "C-z",
        description: "push game to background",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "swap",
        default_key: "x",
        description: "swap wielded and secondary weapons",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "takeoff",
        default_key: "T",
        description: "take off one piece of armor",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "takeoffall",
        default_key: "A",
        description: "remove all armor",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "teleport",
        default_key: "C-t",
        description: "teleport around the level",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "terrain",
        default_key: "",
        description: "view map without obstructions",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "therecmdmenu",
        default_key: "",
        description: "menu of commands for adjacent spot",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "throw",
        default_key: "t",
        description: "throw something",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "tip",
        default_key: "M-T",
        description: "empty a container",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "travel",
        default_key: "_",
        description: "travel to a map location",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "turn",
        default_key: "M-t",
        description: "turn undead away",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "twoweapon",
        default_key: "X",
        description: "toggle two-weapon combat",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "untrap",
        default_key: "M-u",
        description: "untrap something",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "up",
        default_key: "<",
        description: "go up a staircase",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "vanquished",
        default_key: "M-V",
        description: "list vanquished monsters",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "version",
        default_key: "M-v",
        description: "list compile time options",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "versionshort",
        default_key: "v",
        description: "show version and build date",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "wait",
        default_key: ".",
        description: "rest one move",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "wear",
        default_key: "W",
        description: "wear a piece of armor",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "whatdoes",
        default_key: "&",
        description: "tell what a command does",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "whatis",
        default_key: "/",
        description: "show what a symbol is",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "wield",
        default_key: "w",
        description: "wield a weapon",
        autocomplete: false,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "wipe",
        default_key: "M-w",
        description: "wipe off your face",
        autocomplete: true,
        wizard_only: false,
    },
    ExtendedCommand {
        name: "zap",
        default_key: "z",
        description: "zap a wand",
        autocomplete: false,
        wizard_only: false,
    },
    // ── Wizard mode commands ─────────────────────────────────────
    ExtendedCommand {
        name: "wizborn",
        default_key: "",
        description: "show stats of monsters created",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizcast",
        default_key: "",
        description: "cast any spell",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizcustom",
        default_key: "",
        description: "show customized glyphs",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizdetect",
        default_key: "C-e",
        description: "reveal hidden things nearby",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizfliplevel",
        default_key: "",
        description: "flip the level",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizgenesis",
        default_key: "C-g",
        description: "create a monster",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizidentify",
        default_key: "C-i",
        description: "identify all items",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizintrinsic",
        default_key: "",
        description: "set an intrinsic",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizkill",
        default_key: "",
        description: "slay a monster",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizlevelport",
        default_key: "C-v",
        description: "teleport to another level",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizloaddes",
        default_key: "",
        description: "load a des-file lua script",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizloadlua",
        default_key: "",
        description: "load and execute a lua script",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizmakemap",
        default_key: "",
        description: "recreate the current level",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizmap",
        default_key: "C-f",
        description: "map the level",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizrumorcheck",
        default_key: "",
        description: "verify rumor boundaries",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizseenv",
        default_key: "",
        description: "show map seen vectors",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizsmell",
        default_key: "",
        description: "smell monster",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wiztelekinesis",
        default_key: "",
        description: "telekinesis",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizwhere",
        default_key: "",
        description: "show special level locations",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wizwish",
        default_key: "C-w",
        description: "wish for something",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "wmode",
        default_key: "",
        description: "show wall modes",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "debugfuzzer",
        default_key: "",
        description: "start the fuzz tester",
        autocomplete: false,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "levelchange",
        default_key: "",
        description: "change experience level",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "lightsources",
        default_key: "",
        description: "show mobile light sources",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "migratemons",
        default_key: "",
        description: "show migrating monsters",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "panic",
        default_key: "",
        description: "test panic routine (fatal)",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "polyself",
        default_key: "",
        description: "polymorph self",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "stats",
        default_key: "",
        description: "show memory statistics",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "timeout",
        default_key: "",
        description: "look at timeout queue",
        autocomplete: true,
        wizard_only: true,
    },
    ExtendedCommand {
        name: "vision",
        default_key: "",
        description: "show vision array",
        autocomplete: true,
        wizard_only: true,
    },
];

/// Look up an extended command by name.
pub fn find_extended_command(name: &str) -> Option<&'static ExtendedCommand> {
    let lower = name.to_lowercase();
    EXTENDED_COMMANDS.iter().find(|c| c.name == lower)
}

/// Return all regular (non-wizard) extended commands.
pub fn regular_commands() -> Vec<&'static ExtendedCommand> {
    EXTENDED_COMMANDS
        .iter()
        .filter(|c| !c.wizard_only)
        .collect()
}

fn commands_named(names: &[&str]) -> Vec<&'static ExtendedCommand> {
    names
        .iter()
        .filter_map(|name| find_extended_command(name))
        .collect()
}

/// Return the command list for the given request-command menu flavor.
pub fn command_menu_commands(kind: CommandMenuKind) -> Vec<&'static ExtendedCommand> {
    match kind {
        CommandMenuKind::Request => {
            let mut commands = regular_commands();
            commands.sort_by(|a, b| a.name.cmp(b.name));
            commands
        }
        CommandMenuKind::Here => commands_named(&[
            "pickup",
            "loot",
            "look",
            "search",
            "offer",
            "pray",
            "drop",
            "inventory",
            "sit",
            "travel",
            "jump",
            "whatis",
        ]),
        CommandMenuKind::There => commands_named(&[
            "look", "glance", "whatis", "travel", "jump", "open", "close", "kick", "chat", "fight",
            "throw", "zap", "untrap",
        ]),
    }
}

/// Return all wizard-mode-only commands.
pub fn wizard_commands() -> Vec<&'static ExtendedCommand> {
    EXTENDED_COMMANDS.iter().filter(|c| c.wizard_only).collect()
}

/// Return all extended commands that support autocompletion.
pub fn autocomplete_commands() -> Vec<&'static ExtendedCommand> {
    EXTENDED_COMMANDS
        .iter()
        .filter(|c| c.autocomplete)
        .collect()
}

/// Map an extended command name to a PlayerAction.
///
/// Extended commands are typed after the '#' prefix (e.g., "#pray", "#loot").
/// Map an extended command name to a [`PlayerAction`].
///
/// Returns `None` for commands that require further prompting (direction,
/// item selection, text input, or position selection). For direction/item/
/// item+direction commands, use [`extended_command_needs_prompt`] first and
/// dispatch through the shared prompt flow in `app.rs`.
///
/// Also returns `None` for unknown/unrecognized command names.
pub fn map_extended_command(name: &str) -> Option<PlayerAction> {
    match name.trim().to_lowercase().as_str() {
        // ── Commands that produce an action directly ─────────────
        "pray" => Some(PlayerAction::Pray),
        "loot" => Some(PlayerAction::Loot),
        "ride" => Some(PlayerAction::Ride),
        "enhance" => Some(PlayerAction::EnhanceSkill),
        "quit" => Some(PlayerAction::Quit),
        "save" => Some(PlayerAction::Save),
        "help" | "?" => Some(PlayerAction::Help),
        "search" => Some(PlayerAction::Search),
        "pickup" => Some(PlayerAction::PickUp),
        "inventory" | "inv" => Some(PlayerAction::ViewInventory),
        "inventtype" => Some(PlayerAction::ViewEquipped),
        "pay" => Some(PlayerAction::Pay),
        "options" | "optionsfull" => Some(PlayerAction::Options),
        "history" => Some(PlayerAction::ShowHistory),
        "fire" => Some(PlayerAction::Fire),
        "eat" => Some(PlayerAction::Eat { item: None }),
        "quaff" | "drink" => Some(PlayerAction::Quaff { item: None }),
        "read" => Some(PlayerAction::Read { item: None }),
        "known" | "discoveries" => Some(PlayerAction::ViewDiscoveries),
        "conduct" => Some(PlayerAction::ViewConduct),
        "sit" => Some(PlayerAction::Sit),
        "attributes" => Some(PlayerAction::Attributes),
        "overview" => Some(PlayerAction::DungeonOverview),
        "two-weapon" | "twoweapon" => Some(PlayerAction::ToggleTwoWeapon),
        "swap" => Some(PlayerAction::Swap),
        "version" | "versionshort" => Some(PlayerAction::ShowVersion),
        "wipe" | "wipeoff" => Some(PlayerAction::Wipe),
        "look" | "lookaround" => Some(PlayerAction::LookHere),
        "wait" => Some(PlayerAction::Rest),
        "down" => Some(PlayerAction::GoDown),
        "up" => Some(PlayerAction::GoUp),
        "turn" => Some(PlayerAction::TurnUndead),
        "chronicle" => Some(PlayerAction::Chronicle),
        "vanquished" => Some(PlayerAction::Vanquished),
        "genocided" => Some(PlayerAction::ViewDiscoveries),
        "saveoptions" => Some(PlayerAction::Options),
        "seeall" | "seearmor" | "seeamulet" | "seerings" | "seetools" | "seeweapon" => {
            Some(PlayerAction::ViewEquipped)
        }
        "showgold" => Some(PlayerAction::ViewInventory),
        "showspells" => Some(PlayerAction::ViewEquipped),
        "whatdoes" => Some(PlayerAction::Help),
        "autopickup" => Some(PlayerAction::Options),
        "prevmsg" => Some(PlayerAction::ShowHistory),

        // ── Commands needing direction prompt ────────────────────
        // Dispatched by `extended_command_needs_prompt`.
        "open" => None,
        "close" => None,
        "kick" => None,
        "chat" => None,

        // ── Commands needing item prompt ─────────────────────────
        // Dispatched by `extended_command_needs_prompt`.
        "wear" => None,
        "wield" => None,
        "takeoff" => None,
        "takeoffall" => Some(PlayerAction::TakeOffAll),
        "puton" => None,
        "remove" => None,
        "drop" | "droptype" => None,
        "apply" => None,
        "quiver" => None,
        "invoke" => None,
        "rub" => None,
        "tip" => None,
        "offer" => None,
        "force" => None,

        // ── Commands needing item + direction prompt ─────────────
        // Dispatched by `extended_command_needs_prompt`.
        "throw" => None,
        "zap" => None,
        "cast" => None,
        "dip" => None,

        // ── Commands needing text/position prompt ────────────────
        "name" | "naming" | "call" => None,
        "adjust" => None,
        "engrave" => None,
        "annotate" => None,
        "travel" => None,
        "whatis" | "glance" => None,
        "jump" => None,
        "showtrap" => None,
        "retravel" => None,

        // ── TUI-layer / system commands ──────────────────────────
        "redraw" => Some(PlayerAction::Redraw),
        "repeat" => None,            // handled by TUI layer directly
        "reqmenu" => None,           // prefix command, no standalone action
        "run" | "rush" => None,      // handled via direction prompt dispatch
        "shell" | "suspend" => None, // system-level, not applicable
        "perminv" => None,           // handled by TUI layer directly

        // ── Unimplemented ────────────────────────────────────────
        "untrap" => None,
        "monster" => Some(PlayerAction::Monster),
        "terrain" => Some(PlayerAction::ViewTerrain),
        "herecmdmenu" | "therecmdmenu" => None,
        "exploremode" => None,

        // ── Wizard mode commands ─────────────────────────────────
        "wizwish" => None,
        "wizmap" => None,
        "wizgenesis" => None,
        "wizidentify" => None,
        "wizdetect" => None,
        "wizlevelport" => None,
        "wizloadlua" | "wizloaddes" => None,
        "wizmakemap" | "wizfliplevel" => None,

        _ => None,
    }
}

/// Tab completion for extended command names.
///
/// Given a partial command string, returns the completed command if there
/// is a unique match, or the longest common prefix if multiple commands
/// share the same prefix (and it is longer than `partial`).
/// Returns `None` if no commands match or the common prefix is not longer
/// than the input.
pub fn complete_extended_command(partial: &str) -> Option<String> {
    let lower = partial.to_lowercase();
    let matches: Vec<&ExtendedCommand> = EXTENDED_COMMANDS
        .iter()
        .filter(|c| c.autocomplete && c.name.starts_with(&lower))
        .collect();
    if matches.len() == 1 {
        Some(matches[0].name.to_string())
    } else {
        if matches.is_empty() {
            return None;
        }
        let first = matches[0].name;
        let prefix_len = first.len().min(
            matches
                .iter()
                .skip(1)
                .map(|m| {
                    first
                        .chars()
                        .zip(m.name.chars())
                        .take_while(|(a, b)| a == b)
                        .count()
                })
                .min()
                .unwrap_or(first.len()),
        );
        let prefix: String = first.chars().take(prefix_len).collect();
        if prefix.len() > partial.len() {
            Some(prefix)
        } else {
            None
        }
    }
}

/// Map a key event to a direction, used for direction prompts
/// (open/close door, kick, etc.).
pub fn map_direction_key(key: KeyEvent) -> Option<Direction> {
    match key.code {
        KeyCode::Char('h') | KeyCode::Left => Some(Direction::West),
        KeyCode::Char('j') | KeyCode::Down => Some(Direction::South),
        KeyCode::Char('k') | KeyCode::Up => Some(Direction::North),
        KeyCode::Char('l') | KeyCode::Right => Some(Direction::East),
        KeyCode::Char('y') => Some(Direction::NorthWest),
        KeyCode::Char('u') => Some(Direction::NorthEast),
        KeyCode::Char('b') => Some(Direction::SouthWest),
        KeyCode::Char('n') => Some(Direction::SouthEast),
        KeyCode::Char('<') => Some(Direction::Up),
        KeyCode::Char('>') => Some(Direction::Down),
        KeyCode::Char('.') => Some(Direction::Self_),
        _ => None,
    }
}

/// Convert a crossterm `KeyEvent` into our `InputEvent`.
pub fn crossterm_to_input(key: KeyEvent) -> crate::port::InputEvent {
    let modifiers = crate::port::InputModifiers {
        shift: key.modifiers.contains(KeyModifiers::SHIFT),
        ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
        alt: key.modifiers.contains(KeyModifiers::ALT),
    };

    let code = match key.code {
        KeyCode::Char(c) => crate::port::InputKeyCode::Char(c),
        KeyCode::Enter => crate::port::InputKeyCode::Enter,
        KeyCode::Esc => crate::port::InputKeyCode::Escape,
        KeyCode::Backspace => crate::port::InputKeyCode::Backspace,
        KeyCode::Tab => crate::port::InputKeyCode::Tab,
        KeyCode::Up => crate::port::InputKeyCode::Up,
        KeyCode::Down => crate::port::InputKeyCode::Down,
        KeyCode::Left => crate::port::InputKeyCode::Left,
        KeyCode::Right => crate::port::InputKeyCode::Right,
        KeyCode::Home => crate::port::InputKeyCode::Home,
        KeyCode::End => crate::port::InputKeyCode::End,
        KeyCode::PageUp => crate::port::InputKeyCode::PageUp,
        KeyCode::PageDown => crate::port::InputKeyCode::PageDown,
        KeyCode::Delete => crate::port::InputKeyCode::Delete,
        KeyCode::Insert => crate::port::InputKeyCode::Insert,
        KeyCode::F(n) => crate::port::InputKeyCode::F(n),
        _ => return crate::port::InputEvent::None,
    };

    crate::port::InputEvent::Key { code, modifiers }
}

// ---------------------------------------------------------------------------
// Full key binding table — all default key→command assignments
// ---------------------------------------------------------------------------

/// A single key binding entry: key description to command name.
#[derive(Debug, Clone, Copy)]
pub struct KeyBinding {
    pub key: &'static str,
    pub command: &'static str,
    pub description: &'static str,
}

/// Complete default key binding table for NetHack 3.7.
///
/// This covers the standard 33 single-key commands, 8 vi-keys for movement,
/// 8 shifted vi-keys for long movement, Ctrl key commands, and Meta/Alt
/// extended command shortcuts.
pub const DEFAULT_KEYBINDINGS: &[KeyBinding] = &[
    // ── Movement (vi-keys) ──────────────────────────────────────
    KeyBinding {
        key: "h",
        command: "movewest",
        description: "move west",
    },
    KeyBinding {
        key: "j",
        command: "movesouth",
        description: "move south",
    },
    KeyBinding {
        key: "k",
        command: "movenorth",
        description: "move north",
    },
    KeyBinding {
        key: "l",
        command: "moveeast",
        description: "move east",
    },
    KeyBinding {
        key: "y",
        command: "movenorthwest",
        description: "move northwest",
    },
    KeyBinding {
        key: "u",
        command: "movenortheast",
        description: "move northeast",
    },
    KeyBinding {
        key: "b",
        command: "movesouthwest",
        description: "move southwest",
    },
    KeyBinding {
        key: "n",
        command: "movesoutheast",
        description: "move southeast",
    },
    // ── Long movement (Shift+vi-keys) ───────────────────────────
    KeyBinding {
        key: "H",
        command: "movewest",
        description: "run west",
    },
    KeyBinding {
        key: "J",
        command: "movesouth",
        description: "run south",
    },
    KeyBinding {
        key: "K",
        command: "movenorth",
        description: "run north",
    },
    KeyBinding {
        key: "L",
        command: "moveeast",
        description: "run east",
    },
    KeyBinding {
        key: "Y",
        command: "movenorthwest",
        description: "run northwest",
    },
    KeyBinding {
        key: "U",
        command: "movenortheast",
        description: "run northeast",
    },
    KeyBinding {
        key: "B",
        command: "movesouthwest",
        description: "run southwest",
    },
    KeyBinding {
        key: "N",
        command: "movesoutheast",
        description: "run southeast",
    },
    // ── Single-key commands ─────────────────────────────────────
    KeyBinding {
        key: ".",
        command: "wait",
        description: "rest one move",
    },
    KeyBinding {
        key: "s",
        command: "search",
        description: "search for traps/secret doors",
    },
    KeyBinding {
        key: "i",
        command: "inventory",
        description: "show inventory",
    },
    KeyBinding {
        key: "I",
        command: "inventtype",
        description: "show inventory by class",
    },
    KeyBinding {
        key: ",",
        command: "pickup",
        description: "pick up items",
    },
    KeyBinding {
        key: "d",
        command: "drop",
        description: "drop an item",
    },
    KeyBinding {
        key: "D",
        command: "droptype",
        description: "drop specific types",
    },
    KeyBinding {
        key: "e",
        command: "eat",
        description: "eat something",
    },
    KeyBinding {
        key: "q",
        command: "quaff",
        description: "quaff something",
    },
    KeyBinding {
        key: "r",
        command: "read",
        description: "read a scroll/spellbook",
    },
    KeyBinding {
        key: "w",
        command: "wield",
        description: "wield a weapon",
    },
    KeyBinding {
        key: "W",
        command: "wear",
        description: "wear armor",
    },
    KeyBinding {
        key: "T",
        command: "takeoff",
        description: "take off armor",
    },
    KeyBinding {
        key: "P",
        command: "puton",
        description: "put on accessory",
    },
    KeyBinding {
        key: "R",
        command: "remove",
        description: "remove accessory",
    },
    KeyBinding {
        key: "a",
        command: "apply",
        description: "apply (use) a tool",
    },
    KeyBinding {
        key: "t",
        command: "throw",
        description: "throw something",
    },
    KeyBinding {
        key: "f",
        command: "fire",
        description: "fire from quiver",
    },
    KeyBinding {
        key: "z",
        command: "zap",
        description: "zap a wand",
    },
    KeyBinding {
        key: "Z",
        command: "cast",
        description: "cast a spell",
    },
    KeyBinding {
        key: "o",
        command: "open",
        description: "open a door",
    },
    KeyBinding {
        key: "c",
        command: "close",
        description: "close a door",
    },
    KeyBinding {
        key: "p",
        command: "pay",
        description: "pay shopping bill",
    },
    KeyBinding {
        key: "Q",
        command: "quiver",
        description: "select quiver ammo",
    },
    KeyBinding {
        key: "x",
        command: "swap",
        description: "swap weapons",
    },
    KeyBinding {
        key: "X",
        command: "twoweapon",
        description: "toggle two-weapon",
    },
    KeyBinding {
        key: "A",
        command: "takeoffall",
        description: "remove all armor",
    },
    KeyBinding {
        key: "C",
        command: "call",
        description: "name something",
    },
    KeyBinding {
        key: "E",
        command: "engrave",
        description: "engrave on floor",
    },
    KeyBinding {
        key: "F",
        command: "fight",
        description: "force fight",
    },
    KeyBinding {
        key: "O",
        command: "options",
        description: "show options",
    },
    KeyBinding {
        key: "S",
        command: "save",
        description: "save and exit",
    },
    KeyBinding {
        key: "V",
        command: "history",
        description: "show game history",
    },
    KeyBinding {
        key: "v",
        command: "versionshort",
        description: "show version",
    },
    // ── Symbol-key commands ─────────────────────────────────────
    KeyBinding {
        key: "<",
        command: "up",
        description: "go up stairs",
    },
    KeyBinding {
        key: ">",
        command: "down",
        description: "go down stairs",
    },
    KeyBinding {
        key: ":",
        command: "look",
        description: "look here",
    },
    KeyBinding {
        key: ";",
        command: "glance",
        description: "far look",
    },
    KeyBinding {
        key: "/",
        command: "whatis",
        description: "identify symbol",
    },
    KeyBinding {
        key: "?",
        command: "help",
        description: "help",
    },
    KeyBinding {
        key: "\\",
        command: "known",
        description: "discoveries",
    },
    KeyBinding {
        key: "`",
        command: "knownclass",
        description: "class discoveries",
    },
    KeyBinding {
        key: "^",
        command: "showtrap",
        description: "describe trap",
    },
    KeyBinding {
        key: "&",
        command: "whatdoes",
        description: "what does key do",
    },
    KeyBinding {
        key: "@",
        command: "autopickup",
        description: "toggle autopickup",
    },
    KeyBinding {
        key: "#",
        command: "#",
        description: "extended command",
    },
    KeyBinding {
        key: "*",
        command: "seeall",
        description: "show all equipment",
    },
    KeyBinding {
        key: "$",
        command: "showgold",
        description: "show gold",
    },
    KeyBinding {
        key: "+",
        command: "showspells",
        description: "show spells",
    },
    KeyBinding {
        key: ")",
        command: "seeweapon",
        description: "show wielded weapon",
    },
    KeyBinding {
        key: "[",
        command: "seearmor",
        description: "show worn armor",
    },
    KeyBinding {
        key: "=",
        command: "seerings",
        description: "show worn rings",
    },
    KeyBinding {
        key: "\"",
        command: "seeamulet",
        description: "show worn amulet",
    },
    KeyBinding {
        key: "(",
        command: "seetools",
        description: "show used tools",
    },
    KeyBinding {
        key: "!",
        command: "shell",
        description: "shell escape",
    },
    KeyBinding {
        key: "|",
        command: "perminv",
        description: "persistent inventory",
    },
    KeyBinding {
        key: "_",
        command: "travel",
        description: "travel to location",
    },
    KeyBinding {
        key: "m",
        command: "reqmenu",
        description: "request menu",
    },
    KeyBinding {
        key: "g",
        command: "rush",
        description: "rush prefix",
    },
    KeyBinding {
        key: "G",
        command: "run",
        description: "run prefix",
    },
    // ── Ctrl key commands ───────────────────────────────────────
    KeyBinding {
        key: "C-a",
        command: "repeat",
        description: "repeat command",
    },
    KeyBinding {
        key: "C-c",
        command: "quit",
        description: "quit/interrupt",
    },
    KeyBinding {
        key: "C-d",
        command: "kick",
        description: "kick",
    },
    KeyBinding {
        key: "C-e",
        command: "wizdetect",
        description: "detect (wiz)",
    },
    KeyBinding {
        key: "C-f",
        command: "wizmap",
        description: "map level (wiz)",
    },
    KeyBinding {
        key: "C-g",
        command: "wizgenesis",
        description: "create monster (wiz)",
    },
    KeyBinding {
        key: "C-i",
        command: "wizidentify",
        description: "identify all (wiz)",
    },
    KeyBinding {
        key: "C-o",
        command: "overview",
        description: "dungeon overview",
    },
    KeyBinding {
        key: "C-p",
        command: "prevmsg",
        description: "previous messages",
    },
    KeyBinding {
        key: "C-r",
        command: "redraw",
        description: "redraw screen",
    },
    KeyBinding {
        key: "C-t",
        command: "teleport",
        description: "teleport",
    },
    KeyBinding {
        key: "C-v",
        command: "wizlevelport",
        description: "level teleport (wiz)",
    },
    KeyBinding {
        key: "C-w",
        command: "wizwish",
        description: "wish (wiz)",
    },
    KeyBinding {
        key: "C-x",
        command: "attributes",
        description: "show attributes",
    },
    KeyBinding {
        key: "C-z",
        command: "suspend",
        description: "suspend game",
    },
    KeyBinding {
        key: "C-_",
        command: "retravel",
        description: "retravel",
    },
    // ── Meta/Alt key commands ───────────────────────────────────
    KeyBinding {
        key: "M-?",
        command: "?",
        description: "list extended commands",
    },
    KeyBinding {
        key: "M-a",
        command: "adjust",
        description: "adjust inventory",
    },
    KeyBinding {
        key: "M-A",
        command: "annotate",
        description: "name level",
    },
    KeyBinding {
        key: "M-c",
        command: "chat",
        description: "talk to someone",
    },
    KeyBinding {
        key: "M-C",
        command: "conduct",
        description: "show conducts",
    },
    KeyBinding {
        key: "M-d",
        command: "dip",
        description: "dip an object",
    },
    KeyBinding {
        key: "M-e",
        command: "enhance",
        description: "enhance skills",
    },
    KeyBinding {
        key: "M-f",
        command: "force",
        description: "force a lock",
    },
    KeyBinding {
        key: "M-g",
        command: "genocided",
        description: "genocided list",
    },
    KeyBinding {
        key: "M-i",
        command: "invoke",
        description: "invoke item power",
    },
    KeyBinding {
        key: "M-j",
        command: "jump",
        description: "jump",
    },
    KeyBinding {
        key: "M-l",
        command: "loot",
        description: "loot a box",
    },
    KeyBinding {
        key: "M-m",
        command: "monster",
        description: "monster ability",
    },
    KeyBinding {
        key: "M-n",
        command: "name",
        description: "name something",
    },
    KeyBinding {
        key: "M-o",
        command: "offer",
        description: "offer sacrifice",
    },
    KeyBinding {
        key: "M-p",
        command: "pray",
        description: "pray",
    },
    KeyBinding {
        key: "M-r",
        command: "rub",
        description: "rub lamp/stone",
    },
    KeyBinding {
        key: "M-R",
        command: "ride",
        description: "mount/dismount",
    },
    KeyBinding {
        key: "M-s",
        command: "sit",
        description: "sit down",
    },
    KeyBinding {
        key: "M-t",
        command: "turn",
        description: "turn undead",
    },
    KeyBinding {
        key: "M-T",
        command: "tip",
        description: "empty container",
    },
    KeyBinding {
        key: "M-u",
        command: "untrap",
        description: "untrap",
    },
    KeyBinding {
        key: "M-v",
        command: "version",
        description: "version info",
    },
    KeyBinding {
        key: "M-V",
        command: "vanquished",
        description: "vanquished list",
    },
    KeyBinding {
        key: "M-w",
        command: "wipe",
        description: "wipe face",
    },
    KeyBinding {
        key: "M-X",
        command: "exploremode",
        description: "explore mode",
    },
];

/// Find a key binding by key name.
pub fn find_binding(key: &str) -> Option<&'static KeyBinding> {
    DEFAULT_KEYBINDINGS.iter().find(|b| b.key == key)
}

/// Find all key bindings for a given command name.
pub fn bindings_for_command(command: &str) -> Vec<&'static KeyBinding> {
    DEFAULT_KEYBINDINGS
        .iter()
        .filter(|b| b.command == command)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_extended_pray() {
        let action = map_extended_command("pray");
        assert!(matches!(action, Some(PlayerAction::Pray)));
    }

    #[test]
    fn test_map_extended_loot() {
        let action = map_extended_command("loot");
        assert!(matches!(action, Some(PlayerAction::Loot)));
    }

    #[test]
    fn test_map_extended_case_insensitive() {
        let action = map_extended_command("PRAY");
        assert!(matches!(action, Some(PlayerAction::Pray)));
    }

    #[test]
    fn test_map_extended_unknown() {
        let action = map_extended_command("foobar");
        assert!(action.is_none());
    }

    #[test]
    fn test_complete_extended_command_unique() {
        // "pr" should uniquely complete to "pray"
        let result = complete_extended_command("pr");
        assert_eq!(result, Some("pray".to_string()));
    }

    #[test]
    fn test_complete_extended_command_no_match() {
        let result = complete_extended_command("zzz");
        assert!(result.is_none());
    }

    #[test]
    fn test_map_extended_whitespace_trimmed() {
        let action = map_extended_command("  pray  ");
        assert!(matches!(action, Some(PlayerAction::Pray)));
    }

    #[test]
    fn test_map_extended_save() {
        let action = map_extended_command("save");
        assert!(matches!(action, Some(PlayerAction::Save)));
    }

    #[test]
    fn test_map_extended_enhance() {
        let action = map_extended_command("enhance");
        assert!(matches!(action, Some(PlayerAction::EnhanceSkill)));
    }

    // ── key_needs_prompt tests ─────────────────────────────────────

    fn make_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn make_shift_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT)
    }

    fn make_ctrl_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn prompt_item_keys() {
        // Keys that need just an item prompt.
        let item_keys = ['d', 'w', 'a'];
        for c in item_keys {
            let result = key_needs_prompt(make_key(c));
            assert!(
                matches!(result, Some(PromptKind::Item { .. })),
                "key '{}' should need an item prompt",
                c
            );
        }
    }

    #[test]
    fn prompt_shift_item_keys() {
        // Shifted keys that need an item prompt.
        let shift_keys = ['W', 'T', 'P', 'R'];
        for c in shift_keys {
            let result = key_needs_prompt(make_shift_key(c));
            assert!(
                matches!(result, Some(PromptKind::Item { .. })),
                "Shift+{} should need an item prompt",
                c
            );
        }
    }

    #[test]
    fn prompt_direction_keys() {
        let dir_keys = ['o', 'c'];
        for c in dir_keys {
            let result = key_needs_prompt(make_key(c));
            assert!(
                matches!(result, Some(PromptKind::Direction { .. })),
                "key '{}' should need a direction prompt",
                c
            );
        }
    }

    #[test]
    fn prompt_fight_direction() {
        let result = key_needs_prompt(make_shift_key('F'));
        assert!(matches!(
            result,
            Some(PromptKind::Direction {
                command: DirectionCommand::Fight,
                ..
            })
        ));
    }

    #[test]
    fn prompt_kick_direction() {
        let result = key_needs_prompt(make_ctrl_key('d'));
        assert!(matches!(
            result,
            Some(PromptKind::Direction {
                command: DirectionCommand::Kick,
                ..
            })
        ));
    }

    #[test]
    fn prompt_quiver_item() {
        let result = key_needs_prompt(make_shift_key('Q'));
        assert!(matches!(
            result,
            Some(PromptKind::Item {
                command: ItemCommand::Quiver,
                ..
            })
        ));
    }

    #[test]
    fn prompt_item_then_direction_keys() {
        let keys = ['t', 'z'];
        for c in keys {
            let result = key_needs_prompt(make_key(c));
            assert!(
                matches!(result, Some(PromptKind::ItemThenDirection { .. })),
                "key '{}' should need item+direction prompt",
                c
            );
        }
    }

    #[test]
    fn no_prompt_for_movement_keys() {
        let move_keys = ['h', 'j', 'k', 'l', 'y', 'u', 'b', 'n'];
        for c in move_keys {
            let result = key_needs_prompt(make_key(c));
            assert!(
                result.is_none(),
                "movement key '{}' should not need a prompt",
                c
            );
        }
    }

    #[test]
    fn no_prompt_for_simple_commands() {
        let simple = ['.', 's', 'i', ',', 'e', 'q', 'r', 'f', ':', '?', 'p'];
        for c in simple {
            let result = key_needs_prompt(make_key(c));
            assert!(
                result.is_none(),
                "simple command '{}' should not need a prompt",
                c
            );
        }
    }

    #[test]
    fn wield_command_tag() {
        let result = key_needs_prompt(make_key('w'));
        assert!(matches!(
            result,
            Some(PromptKind::Item {
                command: ItemCommand::Wield,
                ..
            })
        ));
    }

    #[test]
    fn drop_command_tag() {
        let result = key_needs_prompt(make_key('d'));
        assert!(matches!(
            result,
            Some(PromptKind::Item {
                command: ItemCommand::Drop,
                ..
            })
        ));
    }

    #[test]
    fn throw_command_tag() {
        let result = key_needs_prompt(make_key('t'));
        assert!(matches!(
            result,
            Some(PromptKind::ItemThenDirection {
                command: ItemDirectionCommand::Throw,
                ..
            })
        ));
    }

    #[test]
    fn zap_command_tag() {
        let result = key_needs_prompt(make_key('z'));
        assert!(matches!(
            result,
            Some(PromptKind::ItemThenDirection {
                command: ItemDirectionCommand::ZapWand,
                ..
            })
        ));
    }

    #[test]
    fn open_command_tag() {
        let result = key_needs_prompt(make_key('o'));
        assert!(matches!(
            result,
            Some(PromptKind::Direction {
                command: DirectionCommand::Open,
                ..
            })
        ));
    }

    #[test]
    fn close_command_tag() {
        let result = key_needs_prompt(make_key('c'));
        assert!(matches!(
            result,
            Some(PromptKind::Direction {
                command: DirectionCommand::Close,
                ..
            })
        ));
    }

    #[test]
    fn test_map_extended_discoveries() {
        let action = map_extended_command("discoveries");
        assert!(matches!(action, Some(PlayerAction::ViewDiscoveries)));
    }

    #[test]
    fn test_map_extended_known() {
        let action = map_extended_command("known");
        assert!(matches!(action, Some(PlayerAction::ViewDiscoveries)));
    }

    #[test]
    fn test_map_extended_eat() {
        let action = map_extended_command("eat");
        assert!(matches!(action, Some(PlayerAction::Eat { item: None })));
    }

    #[test]
    fn test_map_extended_quaff() {
        let action = map_extended_command("quaff");
        assert!(matches!(action, Some(PlayerAction::Quaff { item: None })));
    }

    #[test]
    fn test_map_extended_read() {
        let action = map_extended_command("read");
        assert!(matches!(action, Some(PlayerAction::Read { item: None })));
    }

    #[test]
    fn test_backslash_discoveries() {
        let key = KeyEvent::new(KeyCode::Char('\\'), KeyModifiers::NONE);
        let action = map_key(key);
        assert!(matches!(action, Some(PlayerAction::ViewDiscoveries)));
    }

    // ── Extended command metadata tests ────────────────────────────

    #[test]
    fn extended_commands_count() {
        assert!(
            EXTENDED_COMMANDS.len() >= 120,
            "should have 120+ extended commands, got {}",
            EXTENDED_COMMANDS.len()
        );
    }

    #[test]
    fn regular_commands_count() {
        let regular = regular_commands();
        assert!(
            regular.len() >= 90,
            "should have 90+ regular commands, got {}",
            regular.len()
        );
    }

    #[test]
    fn command_menu_here_is_contextual_subset() {
        let commands = command_menu_commands(CommandMenuKind::Here);
        let names: Vec<&str> = commands.iter().map(|c| c.name).collect();
        assert!(names.contains(&"pickup"));
        assert!(names.contains(&"offer"));
        assert!(names.contains(&"sit"));
        assert!(!names.contains(&"wizwish"));
        assert!(!names.contains(&"therecmdmenu"));
    }

    #[test]
    fn command_menu_there_is_contextual_subset() {
        let commands = command_menu_commands(CommandMenuKind::There);
        let names: Vec<&str> = commands.iter().map(|c| c.name).collect();
        assert!(names.contains(&"glance"));
        assert!(names.contains(&"chat"));
        assert!(names.contains(&"throw"));
        assert!(names.contains(&"untrap"));
        assert!(!names.contains(&"pray"));
        assert!(!names.contains(&"wizwish"));
    }

    #[test]
    fn wizard_commands_count() {
        let wiz = wizard_commands();
        assert!(
            wiz.len() >= 25,
            "should have 25+ wizard commands, got {}",
            wiz.len()
        );
        // All wizard commands should have wizard_only flag
        for cmd in &wiz {
            assert!(cmd.wizard_only, "{} should be wizard_only", cmd.name);
        }
    }

    #[test]
    fn find_extended_command_by_name() {
        let cmd = find_extended_command("pray").unwrap();
        assert_eq!(cmd.default_key, "M-p");
        assert!(cmd.autocomplete);
        assert!(!cmd.wizard_only);
    }

    #[test]
    fn find_wizard_command() {
        let cmd = find_extended_command("wizwish").unwrap();
        assert!(cmd.wizard_only);
        assert_eq!(cmd.default_key, "C-w");
    }

    #[test]
    fn find_extended_command_case_insensitive() {
        assert!(find_extended_command("PRAY").is_some());
        assert!(find_extended_command("Loot").is_some());
    }

    #[test]
    fn find_extended_command_unknown() {
        assert!(find_extended_command("nonexistent").is_none());
    }

    #[test]
    fn autocomplete_commands_are_subset() {
        let auto = autocomplete_commands();
        assert!(auto.len() >= 30);
        for cmd in &auto {
            assert!(cmd.autocomplete);
        }
    }

    // ── Key binding table tests ───────────────────────────────────

    #[test]
    fn keybinding_count() {
        assert!(
            DEFAULT_KEYBINDINGS.len() >= 100,
            "should have 100+ keybindings, got {}",
            DEFAULT_KEYBINDINGS.len()
        );
    }

    #[test]
    fn find_binding_by_key() {
        let binding = find_binding("h").unwrap();
        assert_eq!(binding.command, "movewest");
    }

    #[test]
    fn find_binding_ctrl() {
        let binding = find_binding("C-d").unwrap();
        assert_eq!(binding.command, "kick");
    }

    #[test]
    fn find_binding_meta() {
        let binding = find_binding("M-p").unwrap();
        assert_eq!(binding.command, "pray");
    }

    #[test]
    fn find_binding_unknown() {
        assert!(find_binding("C-q").is_none());
    }

    #[test]
    fn bindings_for_movement() {
        let binds = bindings_for_command("movewest");
        assert!(binds.len() >= 2, "west should have h and H bindings");
        assert!(binds.iter().any(|b| b.key == "h"));
        assert!(binds.iter().any(|b| b.key == "H"));
    }

    #[test]
    fn bindings_for_nonexistent_command() {
        let binds = bindings_for_command("nonexistent");
        assert!(binds.is_empty());
    }

    // ── map_key additional tests ──────────────────────────────────

    #[test]
    fn map_key_swap_weapons() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let action = map_key(key);
        assert!(matches!(action, Some(PlayerAction::Swap)));
    }

    #[test]
    fn map_key_ctrl_x_attributes() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        let action = map_key(key);
        assert!(matches!(action, Some(PlayerAction::Attributes)));
    }

    #[test]
    fn map_key_ctrl_o_overview() {
        let key = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
        let action = map_key(key);
        assert!(matches!(action, Some(PlayerAction::DungeonOverview)));
    }

    #[test]
    fn map_key_ctrl_p_history() {
        let key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        let action = map_key(key);
        assert!(matches!(action, Some(PlayerAction::ShowHistory)));
    }

    #[test]
    fn map_key_colon_look() {
        let key = KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE);
        let action = map_key(key);
        assert!(matches!(action, Some(PlayerAction::LookHere)));
    }

    #[test]
    fn map_key_at_sign_autopickup() {
        let key = KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE);
        let action = map_key(key);
        assert!(matches!(action, Some(PlayerAction::Options)));
    }

    #[test]
    fn map_key_star_equipment() {
        let key = KeyEvent::new(KeyCode::Char('*'), KeyModifiers::NONE);
        let action = map_key(key);
        assert!(matches!(action, Some(PlayerAction::ViewEquipped)));
    }

    #[test]
    fn map_key_dollar_gold() {
        let key = KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE);
        let action = map_key(key);
        assert!(matches!(action, Some(PlayerAction::ViewInventory)));
    }

    #[test]
    fn map_key_v_versionshort() {
        let key = KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE);
        let action = map_key(key);
        assert!(matches!(action, Some(PlayerAction::ShowVersion)));
    }

    #[test]
    fn map_key_upper_a_takeoffall() {
        let key = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT);
        let action = map_key(key);
        assert!(matches!(action, Some(PlayerAction::TakeOffAll)));
    }

    // ── map_extended_command additional coverage ───────────────────

    #[test]
    fn test_map_extended_look() {
        assert!(matches!(
            map_extended_command("look"),
            Some(PlayerAction::LookHere)
        ));
    }

    #[test]
    fn test_map_extended_wait() {
        assert!(matches!(
            map_extended_command("wait"),
            Some(PlayerAction::Rest)
        ));
    }

    #[test]
    fn test_map_extended_up_down() {
        assert!(matches!(
            map_extended_command("up"),
            Some(PlayerAction::GoUp)
        ));
        assert!(matches!(
            map_extended_command("down"),
            Some(PlayerAction::GoDown)
        ));
    }

    #[test]
    fn test_map_extended_conduct() {
        assert!(matches!(
            map_extended_command("conduct"),
            Some(PlayerAction::ViewConduct)
        ));
    }

    #[test]
    fn test_map_extended_vanquished() {
        assert!(matches!(
            map_extended_command("vanquished"),
            Some(PlayerAction::Vanquished)
        ));
    }

    #[test]
    fn test_map_extended_chronicle() {
        assert!(matches!(
            map_extended_command("chronicle"),
            Some(PlayerAction::Chronicle)
        ));
    }

    #[test]
    fn test_map_extended_saveoptions() {
        assert!(matches!(
            map_extended_command("saveoptions"),
            Some(PlayerAction::Options)
        ));
    }

    #[test]
    fn test_map_extended_see_commands() {
        for cmd in [
            "seeall",
            "seearmor",
            "seerings",
            "seeweapon",
            "seetools",
            "seeamulet",
        ] {
            assert!(
                matches!(map_extended_command(cmd), Some(PlayerAction::ViewEquipped)),
                "{} should map to ViewEquipped",
                cmd
            );
        }
    }

    #[test]
    fn test_map_extended_showgold() {
        assert!(matches!(
            map_extended_command("showgold"),
            Some(PlayerAction::ViewInventory)
        ));
    }

    #[test]
    fn test_map_extended_showspells() {
        assert!(matches!(
            map_extended_command("showspells"),
            Some(PlayerAction::ViewEquipped)
        ));
    }

    #[test]
    fn test_map_extended_prevmsg() {
        assert!(matches!(
            map_extended_command("prevmsg"),
            Some(PlayerAction::ShowHistory)
        ));
    }

    #[test]
    fn test_map_extended_autopickup() {
        assert!(matches!(
            map_extended_command("autopickup"),
            Some(PlayerAction::Options)
        ));
    }

    #[test]
    fn test_map_extended_new_direct_actions() {
        assert!(matches!(
            map_extended_command("pickup"),
            Some(PlayerAction::PickUp)
        ));
        assert!(matches!(
            map_extended_command("sit"),
            Some(PlayerAction::Sit)
        ));
        assert!(matches!(
            map_extended_command("attributes"),
            Some(PlayerAction::Attributes)
        ));
        assert!(matches!(
            map_extended_command("overview"),
            Some(PlayerAction::DungeonOverview)
        ));
        assert!(matches!(
            map_extended_command("version"),
            Some(PlayerAction::ShowVersion)
        ));
        assert!(matches!(
            map_extended_command("takeoffall"),
            Some(PlayerAction::TakeOffAll)
        ));
        assert!(matches!(
            map_extended_command("turn"),
            Some(PlayerAction::TurnUndead)
        ));
        assert!(matches!(
            map_extended_command("wipe"),
            Some(PlayerAction::Wipe)
        ));
        assert!(matches!(
            map_extended_command("swap"),
            Some(PlayerAction::Swap)
        ));
        assert!(matches!(
            map_extended_command("monster"),
            Some(PlayerAction::Monster)
        ));
        assert!(matches!(
            map_extended_command("terrain"),
            Some(PlayerAction::ViewTerrain)
        ));
        assert!(matches!(
            map_extended_command("redraw"),
            Some(PlayerAction::Redraw)
        ));
    }

    #[test]
    fn test_map_extended_knownclass_needs_prompt() {
        assert!(map_extended_command("knownclass").is_none());
    }

    #[test]
    fn test_complete_pray_from_autocomplete_commands() {
        // Only autocomplete-flagged commands should match
        let result = complete_extended_command("pr");
        assert_eq!(result, Some("pray".to_string()));
    }

    #[test]
    fn test_complete_wiz_prefix() {
        // "wizw" should match among autocomplete wizard commands
        let result = complete_extended_command("wizw");
        // wizwhere and wizwish — but only wizwhere has autocomplete
        assert_eq!(result, Some("wizwhere".to_string()));
    }

    #[test]
    fn test_extended_command_needs_prompt_direction_open() {
        let p = extended_command_needs_prompt("open");
        assert!(matches!(
            p,
            Some(PromptKind::Direction {
                command: DirectionCommand::Open,
                ..
            })
        ));
    }

    #[test]
    fn test_extended_command_needs_prompt_item_wear() {
        let p = extended_command_needs_prompt("wear");
        assert!(matches!(
            p,
            Some(PromptKind::Item {
                command: ItemCommand::Wear,
                ..
            })
        ));
    }

    #[test]
    fn test_extended_command_needs_prompt_item_then_direction_throw() {
        let p = extended_command_needs_prompt("throw");
        assert!(matches!(
            p,
            Some(PromptKind::ItemThenDirection {
                command: ItemDirectionCommand::Throw,
                ..
            })
        ));
    }

    #[test]
    fn test_extended_command_needs_prompt_run_and_untrap() {
        let run = extended_command_needs_prompt("run");
        let untrap = extended_command_needs_prompt("untrap");
        let fight = extended_command_needs_prompt("fight");
        assert!(matches!(
            run,
            Some(PromptKind::Direction {
                command: DirectionCommand::Run,
                ..
            })
        ));
        assert!(matches!(
            untrap,
            Some(PromptKind::Direction {
                command: DirectionCommand::Untrap,
                ..
            })
        ));
        assert!(matches!(
            fight,
            Some(PromptKind::Direction {
                command: DirectionCommand::Fight,
                ..
            })
        ));
    }

    #[test]
    fn test_key_needs_prompt_g_and_shift_g() {
        let rush = key_needs_prompt(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        let run = key_needs_prompt(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
        assert!(matches!(
            rush,
            Some(PromptKind::Direction {
                command: DirectionCommand::Rush,
                ..
            })
        ));
        assert!(matches!(
            run,
            Some(PromptKind::Direction {
                command: DirectionCommand::Run,
                ..
            })
        ));
    }

    #[test]
    fn test_extended_command_needs_prompt_unknown_none() {
        assert!(extended_command_needs_prompt("foobar").is_none());
    }
}
