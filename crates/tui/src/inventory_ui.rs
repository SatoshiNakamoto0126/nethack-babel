//! Inventory display and item selection UI for the TUI.
//!
//! Provides functions to show the player's inventory grouped by object class,
//! with BUC-status color highlighting, and to present item selection menus
//! for commands like drop, throw, etc.

use std::collections::HashMap;

use nethack_babel_data::{BucStatus, ObjectClass, ObjectCore};

use crate::colors::BucLabel;
use crate::port::{Menu, MenuHow, MenuItem, MenuResult, TermColor, WindowPort};

// ---------------------------------------------------------------------------
// Localized strings for inventory display
// ---------------------------------------------------------------------------

/// Pre-translated strings for inventory UI.
///
/// The CLI layer populates this from `LocaleManager` and passes it through
/// so the TUI crate doesn't need an i18n dependency.
#[derive(Debug, Clone)]
pub struct InventoryI18n {
    /// Maps ObjectClass to translated header (e.g. "Weapons" / "武器").
    pub class_headers: HashMap<ObjectClass, String>,
    /// Inventory window title (e.g. "Inventory" / "物品栏").
    pub title: String,
    /// Empty inventory message (e.g. "You are not carrying anything.").
    pub empty_message: String,
    /// BUC marker for blessed items in list view (e.g. "[B]" / "[祝]").
    pub buc_marker_blessed: String,
    /// BUC marker for cursed items in list view (e.g. "[C]" / "[咒]").
    pub buc_marker_cursed: String,
    /// BUC tag for blessed items in selection menu (e.g. "(blessed)").
    pub buc_tag_blessed: String,
    /// BUC tag for cursed items in selection menu (e.g. "(cursed)").
    pub buc_tag_cursed: String,
    /// BUC tag for uncursed items in selection menu (e.g. "(uncursed)").
    pub buc_tag_uncursed: String,
    /// Fallback header for unknown classes (e.g. "Other" / "其他").
    pub other_header: String,
    /// Pickup menu title (e.g. "Pick up what?" / "捡起什么？").
    pub pickup_title: String,
}

impl Default for InventoryI18n {
    fn default() -> Self {
        let mut class_headers = HashMap::new();
        for &(class, name) in &[
            (ObjectClass::Weapon, "Weapons"),
            (ObjectClass::Armor, "Armor"),
            (ObjectClass::Ring, "Rings"),
            (ObjectClass::Amulet, "Amulets"),
            (ObjectClass::Tool, "Tools"),
            (ObjectClass::Food, "Comestibles"),
            (ObjectClass::Potion, "Potions"),
            (ObjectClass::Scroll, "Scrolls"),
            (ObjectClass::Spellbook, "Spellbooks"),
            (ObjectClass::Wand, "Wands"),
            (ObjectClass::Coin, "Coins"),
            (ObjectClass::Gem, "Gems/Stones"),
            (ObjectClass::Rock, "Rocks"),
            (ObjectClass::Ball, "Iron balls"),
            (ObjectClass::Chain, "Chains"),
            (ObjectClass::Venom, "Venom"),
        ] {
            class_headers.insert(class, name.to_string());
        }

        Self {
            class_headers,
            title: "Inventory".to_string(),
            empty_message: "You are not carrying anything.".to_string(),
            buc_marker_blessed: "[B]".to_string(),
            buc_marker_cursed: "[C]".to_string(),
            buc_tag_blessed: "(blessed)".to_string(),
            buc_tag_cursed: "(cursed)".to_string(),
            buc_tag_uncursed: "(uncursed)".to_string(),
            other_header: "Other".to_string(),
            pickup_title: "Pick up what?".to_string(),
        }
    }
}

impl InventoryI18n {
    fn class_header(&self, class: ObjectClass) -> &str {
        self.class_headers
            .get(&class)
            .map(|s| s.as_str())
            .unwrap_or(&self.other_header)
    }
}

/// Canonical ordering of object classes for inventory display.
/// Matches the traditional NetHack inventory group order.
const CLASS_ORDER: &[ObjectClass] = &[
    ObjectClass::Coin,
    ObjectClass::Amulet,
    ObjectClass::Weapon,
    ObjectClass::Armor,
    ObjectClass::Ring,
    ObjectClass::Tool,
    ObjectClass::Food,
    ObjectClass::Potion,
    ObjectClass::Scroll,
    ObjectClass::Spellbook,
    ObjectClass::Wand,
    ObjectClass::Gem,
    ObjectClass::Rock,
    ObjectClass::Ball,
    ObjectClass::Chain,
    ObjectClass::Venom,
];

// ---------------------------------------------------------------------------
// Inventory item descriptor
// ---------------------------------------------------------------------------

/// A displayable inventory item, combining identity and BUC status.
///
/// The TUI layer constructs these from engine data without depending on
/// the ECS world directly.
#[derive(Debug, Clone)]
pub struct InventoryItem {
    /// Inventory letter (e.g. 'a', 'B').
    pub letter: char,
    /// Display name of the item (already formatted with quantity, enchantment, etc.).
    pub name: String,
    /// Object class for grouping.
    pub class: ObjectClass,
    /// BUC status for color coding.
    pub buc: BucKnowledge,
    /// Quantity (for display; 0 means use name as-is).
    pub quantity: i32,
}

/// What the player knows about an item's BUC status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucKnowledge {
    /// BUC status is known: blessed.
    Blessed,
    /// BUC status is known: uncursed.
    Uncursed,
    /// BUC status is known: cursed.
    Cursed,
    /// BUC status is not yet known to the player.
    Unknown,
}

impl BucKnowledge {
    /// Derive BUC knowledge from the component data.
    pub fn from_buc(buc: &BucStatus) -> Self {
        if !buc.bknown {
            BucKnowledge::Unknown
        } else if buc.blessed {
            BucKnowledge::Blessed
        } else if buc.cursed {
            BucKnowledge::Cursed
        } else {
            BucKnowledge::Uncursed
        }
    }

    /// Map to a foreground color for display.
    pub fn color(self) -> TermColor {
        match self {
            BucKnowledge::Blessed => TermColor::Rgb(0, 220, 220),  // cyan
            BucKnowledge::Cursed => TermColor::Rgb(220, 50, 50),   // red
            BucKnowledge::Uncursed => TermColor::Rgb(220, 220, 220), // white
            BucKnowledge::Unknown => TermColor::Default,
        }
    }
}

// ---------------------------------------------------------------------------
// Build InventoryItem from engine components
// ---------------------------------------------------------------------------

/// Construct an `InventoryItem` from an `ObjectCore` and `BucStatus`.
///
/// `display_name` should be the fully-formatted item name as the player
/// would see it (including enchantment, erosion, etc.).
pub fn make_inventory_item(
    core: &ObjectCore,
    buc: &BucStatus,
    display_name: &str,
) -> InventoryItem {
    InventoryItem {
        letter: core.inv_letter.unwrap_or('?'),
        name: display_name.to_string(),
        class: core.object_class,
        buc: BucKnowledge::from_buc(buc),
        quantity: core.quantity,
    }
}

// ---------------------------------------------------------------------------
// Inventory display (read-only view)
// ---------------------------------------------------------------------------

/// Show the player's inventory in a grouped, scrollable text view.
///
/// Items are grouped by object class with colored headers, and each item
/// line is colored according to its BUC status.
///
/// When `i18n` is `None`, English defaults are used.
pub fn show_inventory(
    port: &mut impl WindowPort,
    items: &[InventoryItem],
    i18n: Option<&InventoryI18n>,
) {
    let default_i18n = InventoryI18n::default();
    let strings = i18n.unwrap_or(&default_i18n);

    if items.is_empty() {
        port.show_text(&strings.title, &strings.empty_message);
        return;
    }

    // Build grouped display lines.
    let mut lines = Vec::new();

    for &class in CLASS_ORDER {
        let class_items: Vec<&InventoryItem> =
            items.iter().filter(|it| it.class == class).collect();
        if class_items.is_empty() {
            continue;
        }

        // Group header.
        lines.push(format!("  {}", strings.class_header(class)));

        for item in &class_items {
            let buc_prefix = match item.buc {
                BucKnowledge::Blessed => format!(" {}", strings.buc_marker_blessed),
                BucKnowledge::Cursed => format!(" {}", strings.buc_marker_cursed),
                BucKnowledge::Uncursed | BucKnowledge::Unknown => String::new(),
            };
            lines.push(format!(
                "  {} - {}{}",
                item.letter, item.name, buc_prefix
            ));
        }

        // Blank line between groups.
        lines.push(String::new());
    }

    // Also handle items whose class is not in CLASS_ORDER.
    let other_items: Vec<&InventoryItem> = items
        .iter()
        .filter(|it| !CLASS_ORDER.contains(&it.class))
        .collect();
    if !other_items.is_empty() {
        lines.push(format!("  {}", strings.other_header));
        for item in &other_items {
            lines.push(format!("  {} - {}", item.letter, item.name));
        }
        lines.push(String::new());
    }

    let content = lines.join("\n");
    port.show_text(&strings.title, &content);
}

// ---------------------------------------------------------------------------
// Item selection menu (for drop, throw, etc.)
// ---------------------------------------------------------------------------

/// Present an item selection menu from a list of inventory items.
///
/// `title` is the prompt (e.g. "What do you want to drop?").
/// `how` determines single vs. multi selection.
/// When `i18n` is `None`, English defaults are used for BUC tags and headers.
///
/// Returns the indices (into `items`) of selected items, or an empty vec
/// if cancelled.
pub fn select_items(
    port: &mut impl WindowPort,
    title: &str,
    items: &[InventoryItem],
    how: MenuHow,
    i18n: Option<&InventoryI18n>,
) -> Vec<usize> {
    let default_i18n = InventoryI18n::default();
    let strings = i18n.unwrap_or(&default_i18n);

    if items.is_empty() {
        return Vec::new();
    }

    // Build menu items grouped by class.
    let mut menu_items: Vec<MenuItem> = Vec::new();
    // Track which original index each selectable menu item maps to.
    let mut index_map: Vec<usize> = Vec::new();

    for &class in CLASS_ORDER {
        let class_indices: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, it)| it.class == class)
            .map(|(i, _)| i)
            .collect();

        if class_indices.is_empty() {
            continue;
        }

        let header = strings.class_header(class).to_string();

        // Group header (non-selectable).
        menu_items.push(MenuItem {
            accelerator: ' ',
            text: header.clone(),
            selected: false,
            selectable: false,
            group: None,
        });

        for &idx in &class_indices {
            let item = &items[idx];
            let buc_tag = match item.buc {
                BucKnowledge::Blessed => format!(" {}", strings.buc_tag_blessed),
                BucKnowledge::Cursed => format!(" {}", strings.buc_tag_cursed),
                BucKnowledge::Uncursed => format!(" {}", strings.buc_tag_uncursed),
                BucKnowledge::Unknown => String::new(),
            };
            menu_items.push(MenuItem {
                accelerator: item.letter,
                text: format!("{}{}", item.name, buc_tag),
                selected: false,
                selectable: true,
                group: Some(header.clone()),
            });
            index_map.push(idx);
        }
    }

    // Handle items not in CLASS_ORDER.
    let other_indices: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, it)| !CLASS_ORDER.contains(&it.class))
        .map(|(i, _)| i)
        .collect();
    if !other_indices.is_empty() {
        menu_items.push(MenuItem {
            accelerator: ' ',
            text: strings.other_header.clone(),
            selected: false,
            selectable: false,
            group: None,
        });
        for &idx in &other_indices {
            let item = &items[idx];
            menu_items.push(MenuItem {
                accelerator: item.letter,
                text: item.name.clone(),
                selected: false,
                selectable: true,
                group: Some(strings.other_header.clone()),
            });
            index_map.push(idx);
        }
    }

    let menu = Menu {
        title: title.to_string(),
        items: menu_items,
        how,
    };

    match port.show_menu(&menu) {
        MenuResult::Selected(sel_indices) => {
            let mut result = Vec::new();
            let mut selectable_counter = 0usize;
            for (menu_idx, menu_item) in menu.items.iter().enumerate() {
                if menu_item.selectable {
                    if sel_indices.contains(&menu_idx)
                        && let Some(&orig_idx) = index_map.get(selectable_counter) {
                            result.push(orig_idx);
                        }
                    selectable_counter += 1;
                }
            }
            result
        }
        MenuResult::Cancelled | MenuResult::Nothing => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Pickup menu (multiple floor items)
// ---------------------------------------------------------------------------

/// Present a pickup menu when multiple items are on the floor.
///
/// `floor_items` contains the display info for each item on the floor.
/// Returns indices of items the player wants to pick up.
pub fn pickup_menu(
    port: &mut impl WindowPort,
    floor_items: &[InventoryItem],
    i18n: Option<&InventoryI18n>,
) -> Vec<usize> {
    let default_i18n = InventoryI18n::default();
    let strings = i18n.unwrap_or(&default_i18n);

    if floor_items.is_empty() {
        return Vec::new();
    }

    // Single item: auto-pick-up, no menu needed.
    if floor_items.len() == 1 {
        return vec![0];
    }

    // Multiple items: show a PickAny menu.
    let mut items_with_letters: Vec<InventoryItem> = Vec::new();
    for (i, item) in floor_items.iter().enumerate() {
        let letter = if i < 26 {
            (b'a' + i as u8) as char
        } else if i < 52 {
            (b'A' + (i - 26) as u8) as char
        } else {
            '?'
        };
        let mut item_copy = item.clone();
        item_copy.letter = letter;
        items_with_letters.push(item_copy);
    }

    select_items(
        port,
        &strings.pickup_title,
        &items_with_letters,
        MenuHow::PickAny,
        i18n,
    )
}

// ---------------------------------------------------------------------------
// BUC color helper (convenience re-export)
// ---------------------------------------------------------------------------

/// Map a `BucKnowledge` to a `BucLabel` for use with the existing `buc_color`
/// function in `colors.rs`.  Returns `None` for `Unknown`.
pub fn buc_to_label(buc: BucKnowledge) -> Option<BucLabel> {
    match buc {
        BucKnowledge::Blessed => Some(BucLabel::Blessed),
        BucKnowledge::Cursed => Some(BucLabel::Cursed),
        BucKnowledge::Uncursed => Some(BucLabel::Uncursed),
        BucKnowledge::Unknown => None,
    }
}
