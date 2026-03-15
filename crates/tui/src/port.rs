//! WindowPort trait and supporting types for the display backend interface.

use nethack_babel_engine::action::{Direction, Position};

// ---------------------------------------------------------------------------
// Display types
// ---------------------------------------------------------------------------

/// A single cell in the map display.
#[derive(Debug, Clone, Copy)]
pub struct DisplayCell {
    pub ch: char,
    pub fg: TermColor,
    pub bg: TermColor,
    pub bold: bool,
}

impl Default for DisplayCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: TermColor::Default,
            bg: TermColor::Default,
            bold: false,
        }
    }
}

/// Terminal color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermColor {
    Default,
    Rgb(u8, u8, u8),
    Indexed(u8),
}

// ---------------------------------------------------------------------------
// Map view
// ---------------------------------------------------------------------------

/// The dimensions of a NetHack level (COLNO x ROWNO).
pub const MAP_COLS: usize = 80;
pub const MAP_ROWS: usize = 21;

/// A 2-D grid of display cells representing the visible map.
#[derive(Debug, Clone)]
pub struct MapView {
    pub cells: [[DisplayCell; MAP_COLS]; MAP_ROWS],
}

impl Default for MapView {
    fn default() -> Self {
        Self {
            cells: [[DisplayCell::default(); MAP_COLS]; MAP_ROWS],
        }
    }
}

impl MapView {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a cell at the given (col, row) position.
    pub fn get(&self, col: usize, row: usize) -> Option<&DisplayCell> {
        self.cells.get(row).and_then(|r| r.get(col))
    }

    /// Set a cell at the given (col, row) position.
    pub fn set(&mut self, col: usize, row: usize, cell: DisplayCell) {
        if row < MAP_ROWS && col < MAP_COLS {
            self.cells[row][col] = cell;
        }
    }
}

// ---------------------------------------------------------------------------
// Status line
// ---------------------------------------------------------------------------

/// The two-line status bar shown at the bottom of the screen.
///
/// ```text
/// Line 1: [Name] the [Title]  St:18/01 Dx:16 Co:18 In:7 Wi:14 Ch:10  [Align]
/// Line 2: Dlvl:10 $:1234 HP:85(120) Pw:23(45) AC:-3 Xp:12/123456 T:4567 [Status]
/// ```
#[derive(Debug, Clone, Default)]
pub struct StatusLine {
    pub line1: String,
    pub line2: String,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Urgency level for display messages, controlling color and behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum MessageUrgency {
    #[default]
    Normal,
    Damage,
    Healing,
    Danger,
    NpcDialogue,
    System,
}


// ---------------------------------------------------------------------------
// Menus
// ---------------------------------------------------------------------------

/// A single menu item.
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Accelerator key (e.g. 'a', 'b', ...).
    pub accelerator: char,
    /// Display text for the item.
    pub text: String,
    /// Whether the item is currently selected.
    pub selected: bool,
    /// Whether the item is selectable at all (false for group headers).
    pub selectable: bool,
    /// Optional group heading this item belongs to.
    pub group: Option<String>,
}

/// A menu to present to the player.
#[derive(Debug, Clone)]
pub struct Menu {
    pub title: String,
    pub items: Vec<MenuItem>,
    pub how: MenuHow,
}

/// Menu selection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuHow {
    /// Display only, no selection.
    None,
    /// Select exactly one item.
    PickOne,
    /// Select any number of items.
    PickAny,
}

/// The result of a menu interaction.
#[derive(Debug, Clone)]
pub enum MenuResult {
    /// Player cancelled the menu.
    Cancelled,
    /// Player made no selection (for display-only menus).
    Nothing,
    /// One or more selected item indices.
    Selected(Vec<usize>),
}

// ---------------------------------------------------------------------------
// Input events
// ---------------------------------------------------------------------------

/// An input event from the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    /// A key press.
    Key {
        code: InputKeyCode,
        modifiers: InputModifiers,
    },
    /// Mouse click at (column, row).
    Mouse {
        col: u16,
        row: u16,
        button: MouseButton,
    },
    /// Terminal was resized to (width, height).
    Resize { width: u16, height: u16 },
    /// No event (timeout or unknown).
    None,
}

/// Key code for input events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputKeyCode {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    Insert,
    F(u8),
}

/// Modifier keys held during an input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl InputModifiers {
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
    };

    pub const SHIFT: Self = Self {
        shift: true,
        ctrl: false,
        alt: false,
    };

    pub const CTRL: Self = Self {
        shift: false,
        ctrl: true,
        alt: false,
    };

    pub const ALT: Self = Self {
        shift: false,
        ctrl: false,
        alt: true,
    };
}

/// Mouse button for mouse events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

// ---------------------------------------------------------------------------
// WindowPort trait
// ---------------------------------------------------------------------------

/// Abstract window port interface.
///
/// Each display backend (TUI, GUI, test shim, etc.) implements this trait.
/// The engine communicates with the player exclusively through this interface.
pub trait WindowPort {
    /// Initialize the display backend. Called once at startup.
    fn init(&mut self);

    /// Shut down the display backend. Called once on exit.
    fn shutdown(&mut self);

    // ── Map rendering ────────────────────────────────────────────

    /// Render the dungeon map with the cursor at the given (col, row).
    fn render_map(&mut self, map: &MapView, cursor: (i16, i16));

    /// Render the two-line status bar.
    fn render_status(&mut self, status: &StatusLine);

    // ── Messages ─────────────────────────────────────────────────

    /// Display a message with the given urgency.
    fn show_message(&mut self, msg: &str, urgency: MessageUrgency);

    /// Show a `--More--` prompt. Returns `false` if the user pressed Escape.
    fn show_more_prompt(&mut self) -> bool;

    /// Show the scrollable message history.
    fn show_message_history(&mut self, messages: &[String]);

    // ── Menus ────────────────────────────────────────────────────

    /// Present a menu and return the player's selection.
    fn show_menu(&mut self, menu: &Menu) -> MenuResult;

    /// Show a block of text (e.g. help screen, long description).
    fn show_text(&mut self, title: &str, content: &str);

    // ── Input ────────────────────────────────────────────────────

    /// Wait for and return the next input event.
    fn get_key(&mut self) -> InputEvent;

    /// Prompt the player for a direction (8 directions + up/down).
    fn ask_direction(&mut self, prompt: &str) -> Option<Direction>;

    /// Prompt the player to select a position on the map.
    fn ask_position(&mut self, prompt: &str) -> Option<Position>;

    /// Ask a yes/no (or custom choice) question.
    /// `choices` contains the valid characters; `default` is returned on Enter.
    fn ask_yn(&mut self, prompt: &str, choices: &str, default: char) -> char;

    /// Prompt for a free-form text string. Returns `None` if cancelled.
    fn get_line(&mut self, prompt: &str) -> Option<String>;

    // ── Special ──────────────────────────────────────────────────

    /// Render the death tombstone / epitaph screen.
    fn render_tombstone(&mut self, epitaph: &str, death_info: &str);

    /// Delay for the given number of milliseconds.
    fn delay(&mut self, ms: u32);

    /// Ring the terminal bell.
    fn bell(&mut self);
}
