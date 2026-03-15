//! Color definitions for enhanced terminal display.
//!
//! Provides True Color mappings for terrain, messages, and BUC status,
//! with dimming for explored-but-not-visible tiles.  Also provides
//! NetHack's classic 16-color system (`NHColor`), monster/object class
//! color mappings, and status-line highlighting rules.

use nethack_babel_engine::dungeon::Terrain;

use crate::port::{MessageUrgency, TermColor};

// ---------------------------------------------------------------------------
// NetHack 16-color system (matches C include/color.h)
// ---------------------------------------------------------------------------

/// Terminal colors matching NetHack's 16-color system.
///
/// Values correspond to the `CLR_*` constants in C NetHack's `color.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NHColor {
    Black = 0,
    Red = 1,
    Green = 2,
    Brown = 3,          // on IBM, low-intensity yellow is brown
    Blue = 4,
    Magenta = 5,
    Cyan = 6,
    Gray = 7,           // low-intensity white (default fg)
    NoColor = 8,        // use terminal default
    Orange = 9,         // bright red
    BrightGreen = 10,
    Yellow = 11,
    BrightBlue = 12,
    BrightMagenta = 13,
    BrightCyan = 14,
    White = 15,
}

impl NHColor {
    /// Whether this color is in the bright (8-15) range.
    pub fn is_bright(self) -> bool {
        (self as u8) >= 8 && self != NHColor::NoColor
    }
}

// ---------------------------------------------------------------------------
// Monster class → color
// ---------------------------------------------------------------------------

/// Map a monster display class character to its default NHColor.
///
/// Based on the per-class color assignments in C NetHack's `display.c` and
/// the `HI_DOMESTIC` / `HI_LORD` / `HI_OVERLORD` aliases from `color.h`.
pub fn monster_class_color(class: char) -> NHColor {
    match class {
        '@' => NHColor::White,         // human (HI_DOMESTIC)
        'a' => NHColor::Brown,         // ant
        'b' => NHColor::Blue,          // blob
        'c' => NHColor::Yellow,        // cockatrice
        'd' => NHColor::Brown,         // dog / canine
        'D' => NHColor::Red,           // dragon (base; individuals vary)
        'e' => NHColor::BrightBlue,    // eye / sphere
        'f' => NHColor::Brown,         // feline
        'g' => NHColor::Green,         // gremlin / gargoyle
        'h' => NHColor::Brown,         // humanoid
        'H' => NHColor::Brown,         // giant
        'i' => NHColor::BrightCyan,    // imp
        'j' => NHColor::Blue,          // jelly
        'k' => NHColor::Red,           // kobold
        'l' => NHColor::BrightGreen,   // leprechaun
        'm' => NHColor::Brown,         // mimic
        'n' => NHColor::Red,           // nymph
        'o' => NHColor::Red,           // orc
        'p' => NHColor::Green,         // piercer
        'q' => NHColor::Brown,         // quadruped
        'r' => NHColor::Brown,         // rodent
        's' => NHColor::Brown,         // spider
        't' => NHColor::White,         // trapper / lurker
        'u' => NHColor::Green,         // unicorn / horse
        'v' => NHColor::Red,           // vortex
        'w' => NHColor::Brown,         // worm
        'x' => NHColor::Brown,         // xan / grid bug
        'y' => NHColor::Green,         // light
        'z' => NHColor::Green,         // zruty
        'A' => NHColor::BrightGreen,   // angel
        'B' => NHColor::Brown,         // bat
        'C' => NHColor::White,         // centaur
        'E' => NHColor::Yellow,        // elemental
        'F' => NHColor::Green,         // fungus
        'G' => NHColor::BrightBlue,    // gnome
        'J' => NHColor::Brown,         // jabberwock
        'K' => NHColor::Magenta,       // keystone kop
        'L' => NHColor::BrightMagenta, // lich
        'M' => NHColor::Brown,         // mummy
        'N' => NHColor::Red,           // naga
        'O' => NHColor::Gray,          // ogre
        'P' => NHColor::Green,         // pudding
        'Q' => NHColor::Brown,         // quantum mechanic
        'R' => NHColor::Red,           // rust monster
        'S' => NHColor::Green,         // snake
        'T' => NHColor::Brown,         // troll
        'U' => NHColor::Gray,          // umber hulk
        'V' => NHColor::Red,           // vampire
        'W' => NHColor::Gray,          // wraith
        'X' => NHColor::Brown,         // xorn
        'Y' => NHColor::Brown,         // yeti / ape
        'Z' => NHColor::Gray,          // zombie
        '&' => NHColor::Red,           // demon
        ':' => NHColor::Cyan,          // sea monster
        ';' => NHColor::Blue,          // eel
        '\'' => NHColor::White,        // golem
        _ => NHColor::NoColor,
    }
}

// ---------------------------------------------------------------------------
// Object class → color
// ---------------------------------------------------------------------------

/// Map an object display class character to its default NHColor.
///
/// Based on the `HI_*` material aliases in C `color.h` and the per-class
/// defaults used in `display.c`.
pub fn object_class_color(class: char) -> NHColor {
    match class {
        ')' => NHColor::Cyan,          // weapon  (HI_METAL)
        '[' => NHColor::Cyan,          // armor   (HI_METAL)
        '=' => NHColor::Brown,         // ring
        '"' => NHColor::Orange,        // amulet
        '(' => NHColor::Cyan,          // tool    (HI_METAL)
        '%' => NHColor::Brown,         // food    (HI_ORGANIC)
        '!' => NHColor::BrightMagenta, // potion
        '?' => NHColor::White,         // scroll  (HI_PAPER)
        '+' => NHColor::BrightBlue,    // spellbook
        '/' => NHColor::Cyan,          // wand
        '$' => NHColor::Yellow,        // gold    (HI_GOLD)
        '*' => NHColor::White,         // gem / rock
        '`' => NHColor::Gray,          // boulder / statue
        '0' => NHColor::Cyan,          // iron ball
        '_' => NHColor::Cyan,          // iron chain
        _ => NHColor::NoColor,
    }
}

// ---------------------------------------------------------------------------
// NHColor → TermColor conversion
// ---------------------------------------------------------------------------

/// Convert an `NHColor` to the TUI's `TermColor` representation.
///
/// Maps the 16 classic NetHack colors to their closest terminal equivalents
/// using the standard 256-color indexed palette (0-15).
pub fn nhcolor_to_term(color: NHColor) -> TermColor {
    match color {
        NHColor::NoColor => TermColor::Default,
        other => TermColor::Indexed(other as u8),
    }
}

/// Convert an `NHColor` to a `ratatui::style::Color`.
///
/// Uses `Indexed(n)` to map exactly to the standard 16-color terminal
/// palette (indices 0-15), which matches the ANSI color ordering that
/// C NetHack's tty port relies on.
pub fn nhcolor_to_ratatui(color: NHColor) -> ratatui::style::Color {
    match color {
        NHColor::NoColor => ratatui::style::Color::Reset,
        other => ratatui::style::Color::Indexed(other as u8),
    }
}

// ---------------------------------------------------------------------------
// Status-line highlighting (hilite_status)
// ---------------------------------------------------------------------------

/// A field on the status line that can be highlighted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusField {
    Hp,
    HpMax,
    Pw,
    PwMax,
    Ac,
    Level,
    Gold,
    Str,
    Dex,
    Con,
    Int,
    Wis,
    Cha,
    Alignment,
    Hunger,
    Encumbrance,
    DungeonLevel,
    Experience,
    Time,
}

/// Condition under which a status highlight fires.
#[derive(Debug, Clone)]
pub enum HighlightCondition {
    /// Value is below this fraction of its maximum (e.g. 0.25 = 25%).
    PercentBelow(f32),
    /// Value changed since the last render.
    Changed,
    /// Always highlight.
    Always,
    /// Value equals a specific string (e.g. "Hungry").
    Equals(String),
}

/// A single status-field highlight rule.
#[derive(Debug, Clone)]
pub struct StatusHighlight {
    pub field: StatusField,
    pub condition: HighlightCondition,
    pub color: NHColor,
    pub bold: bool,
}

/// Return the default highlight rules, matching C NetHack's built-in
/// `hilite_status` defaults.
pub fn default_status_highlights() -> Vec<StatusHighlight> {
    vec![
        // HP below 50% → yellow
        StatusHighlight {
            field: StatusField::Hp,
            condition: HighlightCondition::PercentBelow(0.50),
            color: NHColor::Yellow,
            bold: false,
        },
        // HP below 25% → red + bold
        StatusHighlight {
            field: StatusField::Hp,
            condition: HighlightCondition::PercentBelow(0.25),
            color: NHColor::Red,
            bold: true,
        },
        // Pw below 50% → yellow
        StatusHighlight {
            field: StatusField::Pw,
            condition: HighlightCondition::PercentBelow(0.50),
            color: NHColor::Yellow,
            bold: false,
        },
        // Pw below 25% → orange + bold
        StatusHighlight {
            field: StatusField::Pw,
            condition: HighlightCondition::PercentBelow(0.25),
            color: NHColor::Orange,
            bold: true,
        },
        // Hunger states
        StatusHighlight {
            field: StatusField::Hunger,
            condition: HighlightCondition::Equals("Hungry".to_string()),
            color: NHColor::Yellow,
            bold: false,
        },
        StatusHighlight {
            field: StatusField::Hunger,
            condition: HighlightCondition::Equals("Weak".to_string()),
            color: NHColor::Orange,
            bold: true,
        },
        StatusHighlight {
            field: StatusField::Hunger,
            condition: HighlightCondition::Equals("Fainting".to_string()),
            color: NHColor::Red,
            bold: true,
        },
        StatusHighlight {
            field: StatusField::Hunger,
            condition: HighlightCondition::Equals("Starving".to_string()),
            color: NHColor::Red,
            bold: true,
        },
        // Encumbrance
        StatusHighlight {
            field: StatusField::Encumbrance,
            condition: HighlightCondition::Equals("Burdened".to_string()),
            color: NHColor::Yellow,
            bold: false,
        },
        StatusHighlight {
            field: StatusField::Encumbrance,
            condition: HighlightCondition::Equals("Stressed".to_string()),
            color: NHColor::Orange,
            bold: false,
        },
        StatusHighlight {
            field: StatusField::Encumbrance,
            condition: HighlightCondition::Equals("Strained".to_string()),
            color: NHColor::Orange,
            bold: true,
        },
        StatusHighlight {
            field: StatusField::Encumbrance,
            condition: HighlightCondition::Equals("Overtaxed".to_string()),
            color: NHColor::Red,
            bold: true,
        },
        StatusHighlight {
            field: StatusField::Encumbrance,
            condition: HighlightCondition::Equals("Overloaded".to_string()),
            color: NHColor::Red,
            bold: true,
        },
        // Gold changed
        StatusHighlight {
            field: StatusField::Gold,
            condition: HighlightCondition::Changed,
            color: NHColor::Yellow,
            bold: true,
        },
    ]
}

/// Evaluate a highlight condition against a numeric value with a known max.
///
/// Returns `true` if the condition matches.
pub fn highlight_matches_numeric(condition: &HighlightCondition, value: i32, max: i32) -> bool {
    match condition {
        HighlightCondition::PercentBelow(pct) => {
            max > 0 && (value as f32) < (*pct * max as f32)
        }
        HighlightCondition::Changed => false, // caller must track state
        HighlightCondition::Always => true,
        HighlightCondition::Equals(_) => false,
    }
}

/// Evaluate a highlight condition against a string value.
///
/// Returns `true` if the condition matches.
pub fn highlight_matches_string(condition: &HighlightCondition, value: &str) -> bool {
    match condition {
        HighlightCondition::Equals(expected) => value == expected,
        HighlightCondition::Changed => false, // caller must track state
        HighlightCondition::Always => true,
        HighlightCondition::PercentBelow(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Terrain colors
// ---------------------------------------------------------------------------

/// Map a terrain type to (foreground, background) colors.
///
/// `lit` indicates whether the tile is on a lit square; `in_fov` indicates
/// whether the tile is currently in the player's field of view.  Explored
/// but out-of-FOV tiles are rendered in dimmed colors.
pub fn terrain_color(terrain: Terrain, lit: bool, in_fov: bool) -> (TermColor, TermColor) {
    let (fg, bg) = match terrain {
        // Stone / walls — gray on dark gray
        Terrain::Stone => (TermColor::Rgb(100, 100, 100), TermColor::Rgb(30, 30, 30)),
        Terrain::Wall => (TermColor::Rgb(160, 160, 160), TermColor::Rgb(50, 50, 50)),

        // Floors / corridors — brown/tan
        Terrain::Floor => {
            if lit {
                (TermColor::Rgb(180, 160, 120), TermColor::Rgb(40, 35, 25))
            } else {
                (TermColor::Rgb(120, 110, 80), TermColor::Rgb(30, 25, 18))
            }
        }
        Terrain::Corridor => (TermColor::Rgb(140, 130, 100), TermColor::Rgb(35, 30, 22)),

        // Doors — yellow-brown
        Terrain::DoorOpen => (TermColor::Rgb(200, 170, 80), TermColor::Rgb(60, 45, 20)),
        Terrain::DoorClosed => (TermColor::Rgb(180, 140, 60), TermColor::Rgb(60, 45, 20)),
        Terrain::DoorLocked => (TermColor::Rgb(180, 140, 60), TermColor::Rgb(60, 45, 20)),

        // Water / pool — blue
        Terrain::Pool => (TermColor::Rgb(80, 140, 220), TermColor::Rgb(15, 30, 60)),
        Terrain::Moat => (TermColor::Rgb(60, 120, 200), TermColor::Rgb(10, 25, 55)),
        Terrain::Water => (TermColor::Rgb(70, 130, 210), TermColor::Rgb(12, 28, 58)),

        // Lava — red-orange
        Terrain::Lava => (TermColor::Rgb(255, 100, 30), TermColor::Rgb(80, 20, 5)),

        // Ice — light blue
        Terrain::Ice => (TermColor::Rgb(180, 220, 255), TermColor::Rgb(40, 60, 80)),

        // Tree — green
        Terrain::Tree => (TermColor::Rgb(50, 160, 50), TermColor::Rgb(15, 40, 15)),

        // Fountain — bright blue
        Terrain::Fountain => (TermColor::Rgb(100, 180, 255), TermColor::Rgb(20, 40, 70)),

        // Altar — white / light
        Terrain::Altar => (TermColor::Rgb(240, 240, 240), TermColor::Rgb(60, 60, 60)),

        // Throne — golden
        Terrain::Throne => (TermColor::Rgb(220, 200, 80), TermColor::Rgb(50, 45, 20)),

        // Sink — light gray
        Terrain::Sink => (TermColor::Rgb(180, 190, 200), TermColor::Rgb(40, 42, 45)),

        // Grave — dark gray
        Terrain::Grave => (TermColor::Rgb(120, 120, 130), TermColor::Rgb(30, 30, 35)),

        // Stairs — white on dark
        Terrain::StairsUp => (TermColor::Rgb(240, 240, 240), TermColor::Rgb(40, 40, 40)),
        Terrain::StairsDown => (TermColor::Rgb(240, 240, 240), TermColor::Rgb(40, 40, 40)),

        // Iron bars — cyan
        Terrain::IronBars => (TermColor::Rgb(100, 200, 220), TermColor::Rgb(25, 50, 55)),

        // Drawbridge — brown
        Terrain::Drawbridge => (TermColor::Rgb(160, 120, 60), TermColor::Rgb(45, 35, 18)),

        // Air / cloud — very light
        Terrain::Air => (TermColor::Rgb(200, 220, 255), TermColor::Rgb(20, 25, 40)),
        Terrain::Cloud => (TermColor::Rgb(220, 220, 230), TermColor::Rgb(50, 50, 55)),

        // Magic portal — bright magenta
        Terrain::MagicPortal => (TermColor::Rgb(255, 80, 255), TermColor::Rgb(60, 15, 60)),
    };

    if in_fov {
        (fg, bg)
    } else {
        // Dimmed version for explored-but-not-visible tiles.
        (dim_color(fg), dim_color(bg))
    }
}

/// Dim a color by reducing its brightness by roughly half.
fn dim_color(color: TermColor) -> TermColor {
    match color {
        TermColor::Rgb(r, g, b) => TermColor::Rgb(r / 2, g / 2, b / 2),
        TermColor::Indexed(i) => TermColor::Indexed(i),
        TermColor::Default => TermColor::Default,
    }
}

// ---------------------------------------------------------------------------
// Message urgency colors
// ---------------------------------------------------------------------------

/// Map message urgency to a foreground color.
pub fn message_color(urgency: MessageUrgency) -> TermColor {
    match urgency {
        MessageUrgency::Damage => TermColor::Rgb(255, 80, 80),       // red
        MessageUrgency::Healing => TermColor::Rgb(80, 255, 80),      // green
        MessageUrgency::Danger => TermColor::Rgb(255, 165, 0),       // orange
        MessageUrgency::NpcDialogue => TermColor::Rgb(0, 255, 255),  // cyan
        MessageUrgency::System => TermColor::Rgb(160, 160, 160),     // gray
        MessageUrgency::Normal => TermColor::Default,
    }
}

// ---------------------------------------------------------------------------
// BUC status colors
// ---------------------------------------------------------------------------

/// Blessed / Uncursed / Cursed label for items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucLabel {
    Blessed,
    Uncursed,
    Cursed,
}

/// Map BUC status to a foreground color.
pub fn buc_color(buc: BucLabel) -> TermColor {
    match buc {
        BucLabel::Blessed => TermColor::Rgb(0, 220, 220),    // cyan
        BucLabel::Cursed => TermColor::Rgb(220, 50, 50),     // red
        BucLabel::Uncursed => TermColor::Rgb(220, 220, 220), // white
    }
}

/// Map a `BucStatus` component to a foreground color for display.
///
/// When the BUC status is not known to the player (`bknown == false`),
/// returns `TermColor::Default` (no special highlighting).
pub fn buc_color_from_status(buc: &nethack_babel_data::BucStatus) -> TermColor {
    if !buc.bknown {
        return TermColor::Default;
    }
    if buc.blessed {
        buc_color(BucLabel::Blessed)
    } else if buc.cursed {
        buc_color(BucLabel::Cursed)
    } else {
        buc_color(BucLabel::Uncursed)
    }
}

// ---------------------------------------------------------------------------
// TermColor → ratatui conversion
// ---------------------------------------------------------------------------

/// Convert our TermColor to a ratatui Color.
pub fn to_ratatui_color(color: TermColor) -> ratatui::style::Color {
    match color {
        TermColor::Default => ratatui::style::Color::Reset,
        TermColor::Rgb(r, g, b) => ratatui::style::Color::Rgb(r, g, b),
        TermColor::Indexed(i) => ratatui::style::Color::Indexed(i),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nhcolor_values_match_c_constants() {
        assert_eq!(NHColor::Black as u8, 0);
        assert_eq!(NHColor::Red as u8, 1);
        assert_eq!(NHColor::Green as u8, 2);
        assert_eq!(NHColor::Brown as u8, 3);
        assert_eq!(NHColor::Blue as u8, 4);
        assert_eq!(NHColor::Magenta as u8, 5);
        assert_eq!(NHColor::Cyan as u8, 6);
        assert_eq!(NHColor::Gray as u8, 7);
        assert_eq!(NHColor::NoColor as u8, 8);
        assert_eq!(NHColor::Orange as u8, 9);
        assert_eq!(NHColor::BrightGreen as u8, 10);
        assert_eq!(NHColor::Yellow as u8, 11);
        assert_eq!(NHColor::BrightBlue as u8, 12);
        assert_eq!(NHColor::BrightMagenta as u8, 13);
        assert_eq!(NHColor::BrightCyan as u8, 14);
        assert_eq!(NHColor::White as u8, 15);
    }

    #[test]
    fn nhcolor_is_bright() {
        assert!(!NHColor::Black.is_bright());
        assert!(!NHColor::Red.is_bright());
        assert!(!NHColor::Gray.is_bright());
        assert!(!NHColor::NoColor.is_bright()); // NoColor is 8 but not "bright"
        assert!(NHColor::Orange.is_bright());
        assert!(NHColor::White.is_bright());
        assert!(NHColor::Yellow.is_bright());
    }

    #[test]
    fn monster_class_known_mappings() {
        assert_eq!(monster_class_color('@'), NHColor::White);
        assert_eq!(monster_class_color('D'), NHColor::Red);
        assert_eq!(monster_class_color('L'), NHColor::BrightMagenta);
        assert_eq!(monster_class_color('V'), NHColor::Red);
        assert_eq!(monster_class_color('Z'), NHColor::Gray);
        assert_eq!(monster_class_color('&'), NHColor::Red);
        assert_eq!(monster_class_color(':'), NHColor::Cyan);
        assert_eq!(monster_class_color('d'), NHColor::Brown);
        assert_eq!(monster_class_color('g'), NHColor::Green);
    }

    #[test]
    fn monster_class_unknown_returns_nocolor() {
        assert_eq!(monster_class_color('!'), NHColor::NoColor);
        assert_eq!(monster_class_color('#'), NHColor::NoColor);
        assert_eq!(monster_class_color('1'), NHColor::NoColor);
    }

    #[test]
    fn object_class_known_mappings() {
        assert_eq!(object_class_color(')'), NHColor::Cyan);
        assert_eq!(object_class_color('['), NHColor::Cyan);
        assert_eq!(object_class_color('$'), NHColor::Yellow);
        assert_eq!(object_class_color('!'), NHColor::BrightMagenta);
        assert_eq!(object_class_color('?'), NHColor::White);
        assert_eq!(object_class_color('+'), NHColor::BrightBlue);
        assert_eq!(object_class_color('%'), NHColor::Brown);
    }

    #[test]
    fn object_class_unknown_returns_nocolor() {
        assert_eq!(object_class_color('@'), NHColor::NoColor);
        assert_eq!(object_class_color('z'), NHColor::NoColor);
    }

    #[test]
    fn nhcolor_to_term_conversion() {
        assert_eq!(nhcolor_to_term(NHColor::NoColor), TermColor::Default);
        assert_eq!(nhcolor_to_term(NHColor::Red), TermColor::Indexed(1));
        assert_eq!(nhcolor_to_term(NHColor::White), TermColor::Indexed(15));
        assert_eq!(nhcolor_to_term(NHColor::Black), TermColor::Indexed(0));
    }

    #[test]
    fn nhcolor_to_ratatui_conversion() {
        assert_eq!(nhcolor_to_ratatui(NHColor::NoColor), ratatui::style::Color::Reset);
        assert_eq!(nhcolor_to_ratatui(NHColor::Red), ratatui::style::Color::Indexed(1));
        assert_eq!(nhcolor_to_ratatui(NHColor::White), ratatui::style::Color::Indexed(15));
        assert_eq!(nhcolor_to_ratatui(NHColor::Yellow), ratatui::style::Color::Indexed(11));
        assert_eq!(nhcolor_to_ratatui(NHColor::Brown), ratatui::style::Color::Indexed(3));
    }

    #[test]
    fn default_highlights_not_empty() {
        let highlights = default_status_highlights();
        assert!(highlights.len() >= 5, "expected at least 5 default highlights");

        // Verify HP highlights exist
        let hp_highlights: Vec<_> = highlights
            .iter()
            .filter(|h| h.field == StatusField::Hp)
            .collect();
        assert_eq!(hp_highlights.len(), 2, "expected 2 HP highlights");
    }

    #[test]
    fn highlight_numeric_percent_below() {
        let cond = HighlightCondition::PercentBelow(0.5);
        assert!(highlight_matches_numeric(&cond, 40, 100));  // 40% < 50%
        assert!(!highlight_matches_numeric(&cond, 60, 100)); // 60% >= 50%
        assert!(highlight_matches_numeric(&cond, 0, 100));   // 0% < 50%
        assert!(!highlight_matches_numeric(&cond, 50, 100)); // 50% is not < 50%

        let cond25 = HighlightCondition::PercentBelow(0.25);
        assert!(highlight_matches_numeric(&cond25, 10, 100));
        assert!(!highlight_matches_numeric(&cond25, 30, 100));

        // Edge case: max is 0
        assert!(!highlight_matches_numeric(&cond, 0, 0));
    }

    #[test]
    fn highlight_string_equals() {
        let cond = HighlightCondition::Equals("Hungry".to_string());
        assert!(highlight_matches_string(&cond, "Hungry"));
        assert!(!highlight_matches_string(&cond, "Weak"));
        assert!(!highlight_matches_string(&cond, "hungry")); // case sensitive

        let always = HighlightCondition::Always;
        assert!(highlight_matches_string(&always, "anything"));

        // PercentBelow doesn't match strings
        let pct = HighlightCondition::PercentBelow(0.5);
        assert!(!highlight_matches_string(&pct, "50"));
    }
}
