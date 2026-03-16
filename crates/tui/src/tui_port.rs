//! Concrete [`WindowPort`] implementation backed by ratatui + crossterm.
//!
//! `TuiPort` owns the terminal handle and manages raw mode, the alternate
//! screen buffer, and all rendering through ratatui's immediate-mode API.

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use nethack_babel_engine::action::{Direction, Position};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::Terminal;

use unicode_width::UnicodeWidthChar;

use crate::colors::{message_color, to_ratatui_color};
use crate::input::{crossterm_to_input, map_direction_key};
use crate::port::{
    InputEvent, MapView, Menu, MenuHow, MenuResult, MessageUrgency,
    StatusLine, WindowPort, MAP_COLS, MAP_ROWS,
};
use crate::widgets::{MapWidget, StatusWidget};

// ---------------------------------------------------------------------------
// Terminal lifecycle
// ---------------------------------------------------------------------------

/// Initialize the terminal for TUI rendering: raw mode, alternate screen,
/// and hidden cursor.
pub fn init_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

/// Restore the terminal to its original state: show cursor, leave alternate
/// screen, and disable raw mode.
pub fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> io::Result<()> {
    terminal::disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        cursor::Show
    )?;
    terminal.show_cursor()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// TuiPort
// ---------------------------------------------------------------------------

/// Concrete TUI window port that renders through ratatui and reads input
/// through crossterm.
pub struct TuiPort {
    terminal: Terminal<CrosstermBackend<Stdout>>,

    // Cached render state so that draw() can reference them.
    map: MapView,
    cursor: (i16, i16),
    status: StatusLine,

    // Message display state.
    current_message: Option<String>,
    current_urgency: MessageUrgency,
    show_more: bool,

    // Full message history for Ctrl+P scrollback.
    message_history: Vec<String>,
    /// Maximum history entries to keep.
    max_history: usize,
}

impl TuiPort {
    /// Create a new `TuiPort`.  Does NOT initialize the terminal yet --
    /// call [`WindowPort::init`] for that.
    ///
    /// `terminal` should come from [`init_terminal()`].
    pub fn new(
        terminal: Terminal<CrosstermBackend<Stdout>>,
    ) -> Self {
        Self {
            terminal,
            map: MapView::new(),
            cursor: (0, 0),
            status: StatusLine::default(),
            current_message: None,
            current_urgency: MessageUrgency::Normal,
            show_more: false,
            message_history: Vec::new(),
            max_history: 200,
        }
    }

    /// Create a `TuiPort` by initializing the terminal internally.
    pub fn create() -> io::Result<Self> {
        let terminal = init_terminal()?;
        Ok(Self::new(terminal))
    }

    // ── Internal drawing ────────────────────────────────────────

    /// Perform a full frame draw of map, messages, and status.
    fn draw_frame(&mut self) {
        // Capture all the state we need before the closure borrows self.
        let map = &self.map;
        let cursor = self.cursor;
        let status = &self.status;
        let current_message = &self.current_message;
        let current_urgency = self.current_urgency;
        let show_more = self.show_more;

        let _ = self.terminal.draw(|frame| {
            let area = frame.area();

            // Layout:
            //   [message area: 2 lines]
            //   [map area: 21 lines]
            //   [status area: 2 lines]
            //
            // If the terminal is too small, we shrink the map.
            let chunks = Layout::vertical([
                Constraint::Length(2),  // messages
                Constraint::Min(1),    // map (fills remaining)
                Constraint::Length(2), // status bar
            ])
            .split(area);

            let msg_area = chunks[0];
            let map_area = chunks[1];
            let status_area = chunks[2];

            // -- Messages --
            render_message_line(
                frame,
                msg_area,
                current_message.as_deref(),
                current_urgency,
                show_more,
            );

            // -- Map --
            let map_widget = MapWidget::new(map, cursor);
            frame.render_widget(map_widget, map_area);

            // -- Status --
            let status_widget = StatusWidget::new(status);
            frame.render_widget(status_widget, status_area);
        });
    }

    /// Read one crossterm event, blocking until one arrives.
    fn read_crossterm_key(&self) -> event::KeyEvent {
        loop {
            if let Ok(Event::Key(key)) = event::read() {
                return key;
            }
            // Ignore non-key events (mouse, resize, paste, etc.) and
            // loop to get an actual key.
        }
    }

    /// Read one crossterm event with an optional timeout. Returns None
    /// on timeout.
    #[allow(dead_code)]
    fn read_crossterm_event_timeout(
        &self,
        timeout: Duration,
    ) -> Option<Event> {
        if event::poll(timeout).unwrap_or(false) {
            event::read().ok()
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: write a string into a buffer row with CJK-aware column tracking
// ---------------------------------------------------------------------------

/// Write `text` into `buf` at row `y`, starting at column `start_x`.
/// Advances by the Unicode display width of each character (2 for CJK).
/// Returns the column position after the last character written.
fn put_str(
    buf: &mut ratatui::buffer::Buffer,
    text: &str,
    start_x: u16,
    y: u16,
    right_bound: u16,
    style: Style,
) -> u16 {
    let mut col = start_x;
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0) as u16;
        if col + w > right_bound {
            break;
        }
        let cell = &mut buf[(col, y)];
        cell.set_char(ch);
        cell.set_style(style);
        col += w;
    }
    col
}

// ---------------------------------------------------------------------------
// Helper: compute display width of a string (CJK-aware)
// ---------------------------------------------------------------------------

fn display_width(s: &str) -> u16 {
    s.chars()
        .map(|c| c.width().unwrap_or(0) as u16)
        .sum()
}

// ---------------------------------------------------------------------------
// Helper: render a message line into a frame area
// ---------------------------------------------------------------------------

fn render_message_line(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    message: Option<&str>,
    urgency: MessageUrgency,
    show_more: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let buf = frame.buffer_mut();

    // Clear the message area first.
    let clear_style = Style::default();
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            cell.set_char(' ');
            cell.set_style(clear_style);
        }
    }

    if let Some(msg) = message {
        let fg = to_ratatui_color(message_color(urgency));
        let style = Style::default().fg(fg);

        let y = area.y;
        let end_col = put_str(buf, msg, area.x, y, area.right(), style);

        // --More-- indicator
        if show_more {
            let more_text = "--More--";
            let more_style = Style::default()
                .fg(ratatui::style::Color::Black)
                .bg(ratatui::style::Color::White)
                .add_modifier(Modifier::BOLD);

            put_str(buf, more_text, end_col + 1, y, area.right(), more_style);
        }
    }
}

// ---------------------------------------------------------------------------
// WindowPort implementation
// ---------------------------------------------------------------------------

impl WindowPort for TuiPort {
    fn init(&mut self) {
        // Terminal is already initialized via create() / new().
        // Do a first clear draw.
        let _ = self.terminal.clear();
        self.draw_frame();
    }

    fn shutdown(&mut self) {
        let _ = restore_terminal(&mut self.terminal);
    }

    // ── Map rendering ────────────────────────────────────────────

    fn render_map(&mut self, map: &MapView, cursor: (i16, i16)) {
        self.map = map.clone();
        self.cursor = cursor;
        self.draw_frame();
    }

    fn render_status(&mut self, status: &StatusLine) {
        self.status = status.clone();
        self.draw_frame();
    }

    // ── Messages ─────────────────────────────────────────────────

    fn show_message(&mut self, msg: &str, urgency: MessageUrgency) {
        self.current_message = Some(msg.to_string());
        self.current_urgency = urgency;
        self.show_more = false;

        // Add to history.
        self.message_history.push(msg.to_string());
        if self.message_history.len() > self.max_history {
            self.message_history
                .drain(..self.message_history.len() - self.max_history);
        }

        self.draw_frame();
    }

    fn show_more_prompt(&mut self) -> bool {
        self.show_more = true;
        self.draw_frame();

        // Wait for any key. Escape returns false (skip remaining).
        let key = self.read_crossterm_key();
        self.show_more = false;

        !matches!(key.code, KeyCode::Esc)
    }

    fn show_message_history(&mut self, messages: &[String]) {
        let display_messages: Vec<&str> = if messages.is_empty() {
            // Fall back to internal history.
            self.message_history
                .iter()
                .rev()
                .take(20)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|s| s.as_str())
                .collect()
        } else {
            messages.iter().map(|s| s.as_str()).collect()
        };

        // Render history as a scrollable text block.
        let content = display_messages.join("\n");
        self.show_text("Message History", &content);
    }

    // ── Menus ────────────────────────────────────────────────────

    fn show_menu(&mut self, menu: &Menu) -> MenuResult {
        let items = &menu.items;
        if items.is_empty() {
            return MenuResult::Nothing;
        }

        let mut selected: Vec<bool> = items.iter().map(|it| it.selected).collect();

        loop {
            // Draw the menu.
            let title = &menu.title;
            let sel = &selected;
            let _ = self.terminal.draw(|frame| {
                let area = frame.area();
                let buf = frame.buffer_mut();

                // Clear area.
                let clear_style = Style::default();
                for y in area.y..area.bottom() {
                    for x in area.x..area.right() {
                        let cell = &mut buf[(x, y)];
                        cell.set_char(' ');
                        cell.set_style(clear_style);
                    }
                }

                // Title
                let title_style = Style::default()
                    .fg(ratatui::style::Color::Yellow)
                    .add_modifier(Modifier::BOLD);
                put_str(buf, title, area.x, area.y, area.right(), title_style);

                // Items
                let normal_style = Style::default();
                let selected_style = Style::default()
                    .fg(ratatui::style::Color::Green)
                    .add_modifier(Modifier::BOLD);
                let header_style = Style::default()
                    .fg(ratatui::style::Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED);

                for (idx, item) in items.iter().enumerate() {
                    let y = area.y + 2 + idx as u16;
                    if y >= area.bottom() {
                        break;
                    }

                    let style = if !item.selectable {
                        header_style
                    } else if sel[idx] {
                        selected_style
                    } else {
                        normal_style
                    };

                    let line = if item.selectable {
                        let mark = if sel[idx] { '+' } else { '-' };
                        format!("{} {} {}", item.accelerator, mark, item.text)
                    } else {
                        item.text.clone()
                    };

                    put_str(buf, &line, area.x + 1, y, area.right(), style);
                }

                // Footer hint
                let hint = match menu.how {
                    MenuHow::None => "(press any key to continue)",
                    MenuHow::PickOne => "(type letter to select, Esc to cancel)",
                    MenuHow::PickAny => {
                        "(type letters to toggle, Enter to confirm, Esc to cancel)"
                    }
                };
                let hint_y = area.bottom().saturating_sub(1);
                let hint_style = Style::default()
                    .fg(ratatui::style::Color::DarkGray);
                put_str(buf, hint, area.x, hint_y, area.right(), hint_style);
            });

            // Wait for input.
            let key = self.read_crossterm_key();

            match menu.how {
                MenuHow::None => {
                    // Any key dismisses.
                    return MenuResult::Nothing;
                }
                MenuHow::PickOne => {
                    if matches!(key.code, KeyCode::Esc) {
                        return MenuResult::Cancelled;
                    }
                    if let KeyCode::Char(c) = key.code {
                        // Find item with this accelerator.
                        for (idx, item) in items.iter().enumerate() {
                            if item.selectable && item.accelerator == c {
                                return MenuResult::Selected(vec![idx]);
                            }
                        }
                    }
                }
                MenuHow::PickAny => {
                    if matches!(key.code, KeyCode::Esc) {
                        return MenuResult::Cancelled;
                    }
                    if matches!(key.code, KeyCode::Enter) {
                        let picked: Vec<usize> = selected
                            .iter()
                            .enumerate()
                            .filter_map(|(i, &s)| if s { Some(i) } else { None })
                            .collect();
                        return if picked.is_empty() {
                            MenuResult::Nothing
                        } else {
                            MenuResult::Selected(picked)
                        };
                    }
                    if let KeyCode::Char(c) = key.code {
                        for (idx, item) in items.iter().enumerate() {
                            if item.selectable && item.accelerator == c {
                                selected[idx] = !selected[idx];
                            }
                        }
                    }
                }
            }
        }
    }

    fn show_text(&mut self, title: &str, content: &str) {
        let lines: Vec<&str> = content.lines().collect();
        let mut scroll: usize = 0;

        loop {
            let _ = self.terminal.draw(|frame| {
                let area = frame.area();
                let buf = frame.buffer_mut();

                // Clear
                let clear_style = Style::default();
                for y in area.y..area.bottom() {
                    for x in area.x..area.right() {
                        let cell = &mut buf[(x, y)];
                        cell.set_char(' ');
                        cell.set_style(clear_style);
                    }
                }

                // Title
                let title_style = Style::default()
                    .fg(ratatui::style::Color::Yellow)
                    .add_modifier(Modifier::BOLD);
                put_str(buf, title, area.x, area.y, area.right(), title_style);

                // Content lines with scroll.
                let content_height = (area.height as usize).saturating_sub(3);
                let text_style = Style::default();
                for (i, line) in lines.iter().skip(scroll).enumerate() {
                    if i >= content_height {
                        break;
                    }
                    let y = area.y + 2 + i as u16;
                    put_str(buf, line, area.x, y, area.right(), text_style);
                }

                // Footer
                let hint = "(Space/PgDn: next, b/PgUp: prev, q/Esc: close)";
                let hint_y = area.bottom().saturating_sub(1);
                let hint_style = Style::default()
                    .fg(ratatui::style::Color::DarkGray);
                put_str(buf, hint, area.x, hint_y, area.right(), hint_style);
            });

            let key = self.read_crossterm_key();
            let content_height = self
                .terminal
                .size()
                .map(|s| (s.height as usize).saturating_sub(3))
                .unwrap_or(20);

            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => break,
                KeyCode::Char(' ') | KeyCode::PageDown => {
                    scroll = (scroll + content_height).min(
                        lines.len().saturating_sub(content_height),
                    );
                }
                KeyCode::Char('b') | KeyCode::PageUp => {
                    scroll = scroll.saturating_sub(content_height);
                }
                KeyCode::Down | KeyCode::Char('j')
                    if scroll < lines.len().saturating_sub(content_height) =>
                {
                    scroll += 1;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    scroll = scroll.saturating_sub(1);
                }
                KeyCode::Home => {
                    scroll = 0;
                }
                KeyCode::End => {
                    scroll = lines.len().saturating_sub(content_height);
                }
                _ => {}
            }
        }

        // Redraw normal game screen after closing text view.
        self.draw_frame();
    }

    // ── Input ────────────────────────────────────────────────────

    fn get_key(&mut self) -> InputEvent {
        let key = self.read_crossterm_key();
        crossterm_to_input(key)
    }

    fn ask_direction(&mut self, prompt: &str) -> Option<Direction> {
        self.show_message(prompt, MessageUrgency::Normal);
        let key = self.read_crossterm_key();
        let result = map_direction_key(key);
        // Clear prompt.
        self.current_message = None;
        self.draw_frame();
        result
    }

    fn ask_position(&mut self, prompt: &str) -> Option<Position> {
        self.show_message(prompt, MessageUrgency::Normal);

        let mut pos_x = self.cursor.0;
        let mut pos_y = self.cursor.1;

        loop {
            // Highlight the target position by temporarily moving cursor.
            let old_cursor = self.cursor;
            self.cursor = (pos_x, pos_y);
            self.draw_frame();
            self.cursor = old_cursor;

            let key = self.read_crossterm_key();
            match key.code {
                KeyCode::Esc => {
                    self.current_message = None;
                    self.draw_frame();
                    return None;
                }
                KeyCode::Enter | KeyCode::Char('.') => {
                    self.current_message = None;
                    self.draw_frame();
                    return Some(Position::new(pos_x as i32, pos_y as i32));
                }
                _ => {
                    if let Some(dir) = map_direction_key(key) {
                        let (dx, dy) = dir.delta();
                        pos_x += dx as i16;
                        pos_y += dy as i16;
                        // Clamp to map bounds.
                        pos_x = pos_x.clamp(0, (MAP_COLS - 1) as i16);
                        pos_y = pos_y.clamp(0, (MAP_ROWS - 1) as i16);
                    }
                }
            }
        }
    }

    fn ask_yn(
        &mut self,
        prompt: &str,
        choices: &str,
        default: char,
    ) -> char {
        let display = format!("{} [{}] ", prompt, choices);
        self.show_message(&display, MessageUrgency::Normal);

        loop {
            let key = self.read_crossterm_key();
            match key.code {
                KeyCode::Enter => {
                    self.current_message = None;
                    self.draw_frame();
                    return default;
                }
                KeyCode::Esc => {
                    self.current_message = None;
                    self.draw_frame();
                    return default;
                }
                KeyCode::Char(c) if choices.contains(c) => {
                    self.current_message = None;
                    self.draw_frame();
                    return c;
                }
                // Invalid key -- keep waiting.
                _ => {}
            }
        }
    }

    fn get_line(&mut self, prompt: &str) -> Option<String> {
        let mut input = String::new();

        loop {
            let display = format!("{}{}_", prompt, input);
            self.current_message = Some(display);
            self.current_urgency = MessageUrgency::Normal;
            self.show_more = false;
            self.draw_frame();

            let key = self.read_crossterm_key();
            match key.code {
                KeyCode::Enter => {
                    self.current_message = None;
                    self.draw_frame();
                    return if input.is_empty() {
                        None
                    } else {
                        Some(input)
                    };
                }
                KeyCode::Esc => {
                    self.current_message = None;
                    self.draw_frame();
                    return None;
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) if input.len() < 80 => {
                    input.push(c);
                }
                _ => {}
            }
        }
    }

    // ── Special ──────────────────────────────────────────────────

    fn render_tombstone(&mut self, epitaph: &str, death_info: &str) {
        // Split death_info on " -- " to display each field on its own line.
        let info_lines: Vec<&str> = death_info.split(" -- ").collect();

        let _ = self.terminal.draw(|frame| {
            let area = frame.area();
            let buf = frame.buffer_mut();

            // Clear
            let clear_style = Style::default();
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    let cell = &mut buf[(x, y)];
                    cell.set_char(' ');
                    cell.set_style(clear_style);
                }
            }

            // Traditional NetHack tombstone ASCII art
            let tombstone = [
                "            ----------",
                "           /          \\",
                "          /    REST    \\",
                "         /      IN      \\",
                "        /     PEACE      \\",
                "       /                  \\",
                "       |                  |",
                "       |                  |",
                "       |                  |",
                "       |                  |",
                "       |                  |",
                "      *|     *  *  *      |*",
                "  _____| ___________      |_____",
            ];

            let stone_style = Style::default()
                .fg(ratatui::style::Color::White);
            let text_style = Style::default()
                .fg(ratatui::style::Color::Yellow);

            let start_y = area.y + 1;
            let center_x = area.x + area.width / 2;

            // Draw tombstone frame
            for (i, line) in tombstone.iter().enumerate() {
                let y = start_y + i as u16;
                if y >= area.bottom() {
                    break;
                }
                let w = display_width(line);
                let line_start = center_x.saturating_sub(w / 2);
                put_str(buf, line, line_start, y, area.right(), stone_style);
            }

            // Epitaph text centered inside the tombstone (on the blank rows)
            let text_start_y = start_y + 6; // first "|" row
            if text_start_y < area.bottom() {
                let w = display_width(epitaph);
                let epi_start = center_x.saturating_sub(w / 2);
                put_str(buf, epitaph, epi_start, text_start_y, area.right(), text_style);
            }

            // Death info lines on subsequent rows inside the stone
            let info_style = Style::default()
                .fg(ratatui::style::Color::Red);
            for (i, line) in info_lines.iter().enumerate() {
                let y = text_start_y + 1 + i as u16;
                if y >= area.bottom() || y >= text_start_y + 4 {
                    break;
                }
                let w = display_width(line);
                let line_start = center_x.saturating_sub(w / 2);
                put_str(buf, line, line_start, y, area.right(), info_style);
            }

            // "Press any key" at bottom
            let footer = "(Press any key to continue)";
            let footer_y = area.bottom().saturating_sub(1);
            let footer_style = Style::default()
                .fg(ratatui::style::Color::DarkGray);
            let w = display_width(footer);
            let footer_start = center_x.saturating_sub(w / 2);
            put_str(buf, footer, footer_start, footer_y, area.right(), footer_style);
        });

        // Wait for any key.
        let _ = self.read_crossterm_key();
    }

    fn delay(&mut self, ms: u32) {
        std::thread::sleep(Duration::from_millis(ms as u64));
    }

    fn bell(&mut self) {
        let _ = execute!(io::stdout(), crossterm::style::Print('\x07'));
    }
}

impl Drop for TuiPort {
    fn drop(&mut self) {
        // Best-effort terminal cleanup on drop.
        let _ = restore_terminal(&mut self.terminal);
    }
}
