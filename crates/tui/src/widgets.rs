//! Ratatui widgets for rendering the game display.
//!
//! Three core widgets: the dungeon map, the two-line status bar, and the
//! message area with `--More--` support.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use crate::colors::to_ratatui_color;
use crate::port::{MapView, StatusLine, MAP_COLS, MAP_ROWS};

// ---------------------------------------------------------------------------
// MapWidget
// ---------------------------------------------------------------------------

/// Renders the dungeon map as a grid of colored characters.
///
/// Each cell is drawn with its own fg/bg color. The cursor position is
/// highlighted with a reversed style.
pub struct MapWidget<'a> {
    pub map: &'a MapView,
    pub cursor: (i16, i16),
}

impl<'a> MapWidget<'a> {
    pub fn new(map: &'a MapView, cursor: (i16, i16)) -> Self {
        Self { map, cursor }
    }
}

impl Widget for MapWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let max_rows = (area.height as usize).min(MAP_ROWS);
        let max_cols = (area.width as usize).min(MAP_COLS);

        for row in 0..max_rows {
            for col in 0..max_cols {
                let cell = &self.map.cells[row][col];
                let x = area.x + col as u16;
                let y = area.y + row as u16;

                if x >= area.right() || y >= area.bottom() {
                    continue;
                }

                let fg = to_ratatui_color(cell.fg);
                let bg = to_ratatui_color(cell.bg);

                let is_cursor =
                    col as i16 == self.cursor.0 && row as i16 == self.cursor.1;

                let mut style = Style::default().fg(fg).bg(bg);
                if cell.bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if is_cursor {
                    style = style.add_modifier(Modifier::REVERSED);
                }

                let buf_cell = &mut buf[(x, y)];
                buf_cell.set_char(cell.ch);
                buf_cell.set_style(style);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// StatusWidget
// ---------------------------------------------------------------------------

/// Renders the two-line status bar at the bottom of the screen.
///
/// Line 1: character name, title, stats, alignment.
/// Line 2: dungeon level, gold, HP, Pw, AC, Xp, turn count, status effects.
pub struct StatusWidget<'a> {
    pub status: &'a StatusLine,
}

impl<'a> StatusWidget<'a> {
    pub fn new(status: &'a StatusLine) -> Self {
        Self { status }
    }
}

impl Widget for StatusWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        // Line 1
        let line1_y = area.y;
        let style = Style::default()
            .fg(ratatui::style::Color::White)
            .bg(ratatui::style::Color::DarkGray);
        render_status_line(buf, area.x, line1_y, area.width, &self.status.line1, style);

        // Line 2
        if area.height >= 2 {
            let line2_y = area.y + 1;
            render_status_line(
                buf,
                area.x,
                line2_y,
                area.width,
                &self.status.line2,
                style,
            );
        }
    }
}

/// Write a single status line into the buffer, truncating or padding to fit.
fn render_status_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    text: &str,
    style: Style,
) {
    let chars: Vec<char> = text.chars().collect();
    for col in 0..width {
        let bx = x + col;
        let ch = chars.get(col as usize).copied().unwrap_or(' ');
        let cell = &mut buf[(bx, y)];
        cell.set_char(ch);
        cell.set_style(style);
    }
}

// ---------------------------------------------------------------------------
// MessageWidget
// ---------------------------------------------------------------------------

/// Renders the message area, typically at the top of the screen.
///
/// Shows the most recent messages. When `show_more` is true, appends
/// `--More--` to signal that additional messages are waiting.
pub struct MessageWidget<'a> {
    pub messages: &'a [String],
    pub show_more: bool,
}

impl<'a> MessageWidget<'a> {
    pub fn new(messages: &'a [String], show_more: bool) -> Self {
        Self {
            messages,
            show_more,
        }
    }
}

impl Widget for MessageWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let max_lines = area.height as usize;
        let style = Style::default();
        let more_style = Style::default()
            .fg(ratatui::style::Color::Black)
            .bg(ratatui::style::Color::White)
            .add_modifier(Modifier::BOLD);

        // Show the last `max_lines` messages.
        let start = if self.messages.len() > max_lines {
            self.messages.len() - max_lines
        } else {
            0
        };

        for (i, msg) in self.messages[start..].iter().enumerate() {
            if i >= max_lines {
                break;
            }
            let y = area.y + i as u16;
            let chars: Vec<char> = msg.chars().collect();
            for col in 0..area.width {
                let bx = area.x + col;
                let ch = chars.get(col as usize).copied().unwrap_or(' ');
                let cell = &mut buf[(bx, y)];
                cell.set_char(ch);
                cell.set_style(style);
            }
        }

        // Append --More-- indicator if needed.
        if self.show_more {
            let more_text = "--More--";
            // Find the line to append to: the last message line, or line 0.
            let msg_count = self.messages[start..].len().min(max_lines);
            let target_line = if msg_count > 0 { msg_count - 1 } else { 0 };
            let target_y = area.y + target_line as u16;

            // Find where the message text ends on that line.
            let msg_len = if let Some(msg) = self.messages.last() {
                (msg.chars().count() as u16).min(area.width)
            } else {
                0
            };

            // Place --More-- after the message text (with a space separator).
            let more_x = area.x + msg_len + 1;
            for (j, ch) in more_text.chars().enumerate() {
                let bx = more_x + j as u16;
                if bx >= area.right() {
                    break;
                }
                let cell = &mut buf[(bx, target_y)];
                cell.set_char(ch);
                cell.set_style(more_style);
            }
        }
    }
}
