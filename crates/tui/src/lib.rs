//! Terminal UI crate for NetHack Babel.
//!
//! Provides the TUI rendering backend using ratatui + crossterm, including:
//!
//! - [`port::WindowPort`] — abstract interface between engine and display
//! - [`tui_port::TuiPort`] — concrete terminal backend implementation
//! - [`app::App`] — main application loop and message management
//! - [`input`] — key mapping from crossterm events to game actions
//! - [`colors`] — True Color definitions for terrain, messages, and BUC
//! - [`widgets`] — ratatui widgets for map, status bar, and messages

pub mod app;
pub mod colors;
pub mod input;
pub mod inventory_ui;
pub mod port;
pub mod tui_port;
pub mod widgets;

// Re-export the primary public types for convenience.
pub use app::{App, TuiMessages};
pub use colors::{
    buc_color, buc_color_from_status, default_status_highlights, highlight_matches_numeric,
    highlight_matches_string, message_color, monster_class_color, nhcolor_to_ratatui,
    nhcolor_to_term, object_class_color, terrain_color, BucLabel, HighlightCondition, NHColor,
    StatusField, StatusHighlight,
};
pub use inventory_ui::{
    make_inventory_item, pickup_menu, select_items, show_inventory, BucKnowledge,
    InventoryI18n, InventoryItem,
};
pub use port::{
    DisplayCell, InputEvent, InputKeyCode, InputModifiers, MapView, Menu, MenuHow,
    MenuItem, MenuResult, MessageUrgency, MouseButton, StatusLine, TermColor,
    WindowPort, MAP_COLS, MAP_ROWS,
};
pub use tui_port::{init_terminal, restore_terminal, TuiPort};
