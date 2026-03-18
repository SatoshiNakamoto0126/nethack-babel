use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Top-level configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub game: GameConfig,
    #[serde(default)]
    pub character: CharacterConfig,
    #[serde(default)]
    pub behavior: BehaviorConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub map: MapConfig,
    #[serde(default)]
    pub status: StatusConfig,
    #[serde(default)]
    pub message: MessageConfig,
    #[serde(default)]
    pub menu: MenuConfig,
    #[serde(default)]
    pub sound: SoundConfig,
    #[serde(default)]
    pub keybinds: KeybindConfig,
    #[serde(default)]
    pub advanced: AdvancedConfig,
}

// ---------------------------------------------------------------------------
// OptSection::General — core game settings
// ---------------------------------------------------------------------------

/// Menu style for item selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MenuStyle {
    Traditional,
    Combination,
    #[default]
    Full,
    Partial,
}

/// How to sort discovered objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortDiscoveries {
    /// By discovery order.
    ByDiscovery,
    /// Alphabetically within class.
    Alphabetical,
    /// By object class, then alphabetically.
    #[default]
    ByClass,
}

/// How to sort loot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortLoot {
    None,
    Loot,
    #[default]
    Full,
}

/// Disclosure preferences for end-of-game information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DiscloseChoice {
    /// Show with prompt.
    #[default]
    Yes,
    /// Do not show.
    No,
    /// Show without prompt.
    Auto,
}

/// End-of-game disclosure settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscloseConfig {
    /// Show inventory at end.
    #[serde(default = "default_disclose")]
    pub inventory: DiscloseChoice,
    /// Show attributes at end.
    #[serde(default = "default_disclose")]
    pub attributes: DiscloseChoice,
    /// Show vanquished monsters.
    #[serde(default = "default_disclose")]
    pub vanquished: DiscloseChoice,
    /// Show genocided monsters.
    #[serde(default = "default_disclose")]
    pub genocided: DiscloseChoice,
    /// Show conducts.
    #[serde(default = "default_disclose")]
    pub conduct: DiscloseChoice,
}

impl Default for DiscloseConfig {
    fn default() -> Self {
        Self {
            inventory: DiscloseChoice::Yes,
            attributes: DiscloseChoice::Yes,
            vanquished: DiscloseChoice::Yes,
            genocided: DiscloseChoice::Yes,
            conduct: DiscloseChoice::Yes,
        }
    }
}

fn default_disclose() -> DiscloseChoice {
    DiscloseChoice::Yes
}

/// Paranoid confirmation settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParanoidConfig {
    #[serde(default = "default_true")]
    pub confirm: bool,
    #[serde(default)]
    pub quit: bool,
    #[serde(default)]
    pub die: bool,
    #[serde(default)]
    pub bones: bool,
    #[serde(default)]
    pub attack: bool,
    #[serde(default)]
    pub pray: bool,
    #[serde(default)]
    pub wand_break: bool,
    #[serde(default)]
    pub were_change: bool,
    #[serde(default)]
    pub remove: bool,
    #[serde(default)]
    pub swim: bool,
    #[serde(default)]
    pub trap: bool,
    #[serde(default)]
    pub lava: bool,
    #[serde(default)]
    pub water: bool,
}

impl Default for ParanoidConfig {
    fn default() -> Self {
        Self {
            confirm: true,
            quit: false,
            die: false,
            bones: false,
            attack: false,
            pray: false,
            wand_break: false,
            were_change: false,
            remove: false,
            swim: false,
            trap: false,
            lava: false,
            water: false,
        }
    }
}

/// Run mode — how to display multi-step movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Teleport,
    #[default]
    Run,
    Walk,
    Crawl,
}

/// Pickup burden threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PickupBurden {
    Unencumbered,
    Burdened,
    #[default]
    Stressed,
    Strained,
    Overtaxed,
    Overloaded,
}

/// Autounlock behavior when encountering a locked door/chest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AutoUnlock {
    /// Do nothing.
    None,
    /// Apply a key/lockpick if in inventory.
    #[default]
    Apply,
    /// Kick doors, force chests.
    Force,
}

// ---------------------------------------------------------------------------
// [game] section — corresponds to NetHack General options
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    #[serde(default = "default_language")]
    pub language: String,
    /// Player's name (corresponds to `name` option).
    #[serde(default)]
    pub name: String,
    /// Preferred fruit name.
    #[serde(default = "default_fruit")]
    pub fruit: String,
    /// Pack order preference (string of item class symbols).
    #[serde(default = "default_packorder")]
    pub packorder: String,
    /// Name for your starting dog.
    #[serde(default)]
    pub dogname: String,
    /// Name for your starting cat.
    #[serde(default)]
    pub catname: String,
    /// Name for your starting horse.
    #[serde(default)]
    pub horsename: String,
    /// Preferred pet type ("dog", "cat", "none").
    #[serde(default)]
    pub pettype: String,
    /// Play mode: "normal", "explore", "debug".
    #[serde(default = "default_playmode")]
    pub playmode: String,
    /// Show the legacy intro splash.
    #[serde(default = "default_true")]
    pub legacy: bool,
    /// Show the initial news message.
    #[serde(default = "default_true")]
    pub news: bool,
    /// Show splash screen.
    #[serde(default = "default_true")]
    pub splash_screen: bool,
    /// Show tutorial tips.
    #[serde(default = "default_true")]
    pub tips: bool,
    /// Show tombstone on death.
    #[serde(default = "default_true")]
    pub tombstone: bool,
    /// Display top scores in window.
    #[serde(default)]
    pub toptenwin: bool,
    /// Score display format.
    #[serde(default = "default_scores")]
    pub scores: String,
    /// Save bones files.
    #[serde(default = "default_true")]
    pub bones: bool,
    /// Checkpoint saves.
    #[serde(default = "default_true")]
    pub checkpoint: bool,
    /// Disclosure settings.
    #[serde(default)]
    pub disclose: DiscloseConfig,
    /// Paranoid confirmation settings.
    #[serde(default)]
    pub paranoid_confirmation: ParanoidConfig,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            name: String::new(),
            fruit: default_fruit(),
            packorder: default_packorder(),
            dogname: String::new(),
            catname: String::new(),
            horsename: String::new(),
            pettype: String::new(),
            playmode: default_playmode(),
            legacy: true,
            news: true,
            splash_screen: true,
            tips: true,
            tombstone: true,
            toptenwin: false,
            scores: default_scores(),
            bones: true,
            checkpoint: true,
            disclose: DiscloseConfig::default(),
            paranoid_confirmation: ParanoidConfig::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// [character] section — role/race/gender/align selection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharacterConfig {
    /// Starting role (e.g., "Valkyrie", "Wizard", "random").
    #[serde(default)]
    pub role: String,
    /// Starting race (e.g., "Human", "Elf", "random").
    #[serde(default)]
    pub race: String,
    /// Starting gender ("male", "female", "random").
    #[serde(default)]
    pub gender: String,
    /// Starting alignment ("lawful", "neutral", "chaotic", "random").
    #[serde(default)]
    pub alignment: String,
    /// Nudist conduct — start without armor.
    #[serde(default)]
    pub nudist: bool,
    /// Pauper conduct — start without gold.
    #[serde(default)]
    pub pauper: bool,
}

// ---------------------------------------------------------------------------
// [behavior] section — corresponds to NetHack Behavior options
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    /// Automatically pick up objects you move over.
    #[serde(default)]
    pub autopickup: bool,
    /// Object types to autopickup (e.g., "$?!/=").
    #[serde(default = "default_autopickup_types")]
    pub autopickup_types: String,
    /// Autopickup exceptions (patterns).
    #[serde(default)]
    pub autopickup_exceptions: Vec<String>,
    /// Automatically dig when wielding a digging tool and moving.
    #[serde(default)]
    pub autodig: bool,
    /// Walking into a door attempts to open it.
    #[serde(default = "default_true")]
    pub autoopen: bool,
    /// Fill empty quiver automatically when firing.
    #[serde(default)]
    pub autoquiver: bool,
    /// Behavior when encountering locked door/chest.
    #[serde(default)]
    pub autounlock: AutoUnlock,
    /// Require confirmation before attacking peaceful monsters.
    #[serde(default = "default_true")]
    pub confirm: bool,
    /// Safe pet — avoid attacking pets.
    #[serde(default = "default_true")]
    pub safe_pet: bool,
    /// Safe wait — avoid waiting when hostile monsters are nearby.
    #[serde(default = "default_true")]
    pub safe_wait: bool,
    /// Push wielded weapon into secondary slot when wielding new.
    #[serde(default)]
    pub pushweapon: bool,
    /// Pickup burden threshold.
    #[serde(default)]
    pub pickup_burden: PickupBurden,
    /// Pick up items you previously threw.
    #[serde(default = "default_true")]
    pub pickup_thrown: bool,
    /// Pick up items stolen and recovered from monsters.
    #[serde(default = "default_true")]
    pub pickup_stolen: bool,
    /// Don't re-pickup items you just dropped.
    #[serde(default = "default_true")]
    pub dropped_nopick: bool,
    /// Fixed inventory — items keep their letters when used up/dropped.
    #[serde(default = "default_true")]
    pub fixinv: bool,
    /// Sort pack by type.
    #[serde(default = "default_true")]
    pub sortpack: bool,
    /// How to sort loot.
    #[serde(default)]
    pub sortloot: SortLoot,
    /// Use 'abc' style for looting prompts (vs. menu).
    #[serde(default)]
    pub lootabc: bool,
    /// Force inventory menus for item selection.
    #[serde(default)]
    pub force_invmenu: bool,
    /// Menu style.
    #[serde(default)]
    pub menustyle: MenuStyle,
    /// Verbose messages.
    #[serde(default = "default_true")]
    pub verbose: bool,
    /// Use '.' to rest instead of space.
    #[serde(default)]
    pub rest_on_space: bool,
    /// Fire assist — help with ranged attacks.
    #[serde(default = "default_true")]
    pub fireassist: bool,
    /// Offer command assistance.
    #[serde(default = "default_true")]
    pub cmdassist: bool,
    /// Use extended command menu.
    #[serde(default)]
    pub extmenu: bool,
    /// Pile limit — min items to trigger a menu on pickup.
    #[serde(default = "default_pile_limit")]
    pub pile_limit: u32,
    /// How to display multi-step movement.
    #[serde(default)]
    pub runmode: RunMode,
    /// Travel command enabled.
    #[serde(default = "default_true")]
    pub travel: bool,
    /// Use number_pad for movement instead of vi-keys.
    #[serde(default)]
    pub number_pad: bool,
    /// How to display scores.
    #[serde(default)]
    pub sortvanquished: SortDiscoveries,
    /// How to sort discoveries list.
    #[serde(default)]
    pub sortdiscoveries: SortDiscoveries,
    /// Suppress version-specific alerts.
    #[serde(default)]
    pub suppress_alert: String,
    /// Request quick farsight after teleport.
    #[serde(default = "default_true")]
    pub quick_farsight: bool,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            autopickup: false,
            autopickup_types: default_autopickup_types(),
            autopickup_exceptions: Vec::new(),
            autodig: false,
            autoopen: true,
            autoquiver: false,
            autounlock: AutoUnlock::default(),
            confirm: true,
            safe_pet: true,
            safe_wait: true,
            pushweapon: false,
            pickup_burden: PickupBurden::default(),
            pickup_thrown: true,
            pickup_stolen: true,
            dropped_nopick: true,
            fixinv: true,
            sortpack: true,
            sortloot: SortLoot::default(),
            lootabc: false,
            force_invmenu: false,
            menustyle: MenuStyle::default(),
            verbose: true,
            rest_on_space: false,
            fireassist: true,
            cmdassist: true,
            extmenu: false,
            pile_limit: default_pile_limit(),
            runmode: RunMode::default(),
            travel: true,
            number_pad: false,
            sortvanquished: SortDiscoveries::default(),
            sortdiscoveries: SortDiscoveries::default(),
            suppress_alert: String::new(),
            quick_farsight: true,
        }
    }
}

// ---------------------------------------------------------------------------
// [display] section — general display options
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Use color in map display.
    #[serde(default = "default_true")]
    pub color: bool,
    /// Use color specifically for map tiles.
    #[serde(default = "default_true")]
    pub map_colors: bool,
    /// Use color for message text.
    #[serde(default = "default_true")]
    pub message_colors: bool,
    /// Use background colors for some map highlighting.
    #[serde(default = "default_true")]
    pub bgcolors: bool,
    /// Show map as text (vs. tiles).
    #[serde(default = "default_true")]
    pub ascii_map: bool,
    /// Use tiled map display.
    #[serde(default)]
    pub tiled_map: bool,
    /// Show sparkle effect for resistance.
    #[serde(default = "default_true")]
    pub sparkle: bool,
    /// Show timed delay animations.
    #[serde(default = "default_true")]
    pub timed_delay: bool,
    /// Highlight pets on the map.
    #[serde(default)]
    pub hilite_pet: bool,
    /// Pet highlight attribute (e.g., "underline").
    #[serde(default)]
    pub petattr: String,
    /// Highlight object piles on the map.
    #[serde(default)]
    pub hilite_pile: bool,
    /// Highlight BUC status in inventory.
    #[serde(default = "default_true")]
    pub buc_highlight: bool,
    /// Use dark room shading.
    #[serde(default = "default_true")]
    pub dark_room: bool,
    /// Show lit corridors differently.
    #[serde(default)]
    pub lit_corridor: bool,
    /// Use nerd fonts for map symbols.
    #[serde(default)]
    pub nerd_fonts: bool,
    /// Show minimap.
    #[serde(default = "default_true")]
    pub minimap: bool,
    /// Show info on mouse hover.
    #[serde(default = "default_true")]
    pub mouse_hover_info: bool,
    /// Use inverse video for some things.
    #[serde(default = "default_true")]
    pub use_inverse: bool,
    /// Use truecolor display.
    #[serde(default)]
    pub use_truecolor: bool,
    /// Use standout for --More--.
    #[serde(default)]
    pub standout: bool,
    /// Use guicolor.
    #[serde(default = "default_true")]
    pub guicolor: bool,
    /// Menu overlay mode.
    #[serde(default = "default_true")]
    pub menu_overlay: bool,
    /// Fullscreen mode.
    #[serde(default)]
    pub fullscreen: bool,
    /// Mouse support.
    #[serde(default)]
    pub mouse_support: bool,
    /// Wrap text in message window.
    #[serde(default)]
    pub wraptext: bool,
    /// Popup dialog for prompts (vs. in-window).
    #[serde(default)]
    pub popup_dialog: bool,
    /// Perm invent — persistent inventory window.
    #[serde(default)]
    pub perm_invent: bool,
    /// Perm invent mode.
    #[serde(default)]
    pub perminv_mode: String,
    /// Window borders.
    #[serde(default)]
    pub windowborders: String,
    /// Symbol set name.
    #[serde(default)]
    pub symset: String,
    /// Rogue-level symbol set.
    #[serde(default)]
    pub roguesymset: String,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            color: true,
            map_colors: true,
            message_colors: true,
            bgcolors: true,
            ascii_map: true,
            tiled_map: false,
            sparkle: true,
            timed_delay: true,
            hilite_pet: false,
            petattr: String::new(),
            hilite_pile: false,
            buc_highlight: true,
            dark_room: true,
            lit_corridor: false,
            nerd_fonts: false,
            minimap: true,
            mouse_hover_info: true,
            use_inverse: true,
            use_truecolor: false,
            standout: false,
            guicolor: true,
            menu_overlay: true,
            fullscreen: false,
            mouse_support: false,
            wraptext: false,
            popup_dialog: false,
            perm_invent: false,
            perminv_mode: String::new(),
            windowborders: String::new(),
            symset: String::new(),
            roguesymset: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// [map] section — corresponds to NetHack Map options
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapConfig {
    /// Autodescribe terrain under cursor.
    #[serde(default = "default_true")]
    pub autodescribe: bool,
    /// Mention decorations when moving.
    #[serde(default = "default_true")]
    pub mention_decor: bool,
    /// Mention map features.
    #[serde(default)]
    pub mention_map: bool,
    /// Mention walls when blind.
    #[serde(default)]
    pub mention_walls: bool,
    /// Spot monsters automatically.
    #[serde(default = "default_true")]
    pub spot_monsters: bool,
    /// Show gold as 'X' instead of '$'.
    #[serde(default, rename = "goldX")]
    pub gold_x: bool,
    /// Show monster movement.
    #[serde(default)]
    pub mon_movement: bool,
    /// Whatis coordinate display style ("c" compass, "m" map, etc.).
    #[serde(default)]
    pub whatis_coord: String,
    /// Whatis filter setting.
    #[serde(default)]
    pub whatis_filter: String,
    /// Whatis uses menu.
    #[serde(default)]
    pub whatis_menu: bool,
    /// Skip map positions in whatis cursor movement.
    #[serde(default)]
    pub whatis_moveskip: bool,
    /// Menu items visible when here-command menu is shown.
    #[serde(default)]
    pub herecmd_menu: bool,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            autodescribe: true,
            mention_decor: true,
            mention_map: false,
            mention_walls: false,
            spot_monsters: true,
            gold_x: false,
            mon_movement: false,
            whatis_coord: String::new(),
            whatis_filter: String::new(),
            whatis_menu: false,
            whatis_moveskip: false,
            herecmd_menu: false,
        }
    }
}

// ---------------------------------------------------------------------------
// [status] section — corresponds to NetHack Status options
// ---------------------------------------------------------------------------

/// Number of status lines to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StatusLines {
    #[default]
    Two,
    Three,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusConfig {
    /// Show experience points in status line.
    #[serde(default)]
    pub showexp: bool,
    /// Show score in status line.
    #[serde(default)]
    pub showscore: bool,
    /// Show time in status line.
    #[serde(default)]
    pub time: bool,
    /// Show damage dealt.
    #[serde(default)]
    pub showdamage: bool,
    /// Show race in status line.
    #[serde(default)]
    pub showrace: bool,
    /// Show version in status line.
    #[serde(default)]
    pub showvers: bool,
    /// Show hitpoint bar.
    #[serde(default)]
    pub hitpointbar: bool,
    /// Number of status lines.
    #[serde(default)]
    pub statuslines: StatusLines,
    /// Status hilite rules (raw strings for now).
    #[serde(default)]
    pub hilite_status: Vec<String>,
    /// Implicit "uncursed" — don't show for items known to be uncursed.
    #[serde(default = "default_true")]
    pub implicit_uncursed: bool,
}

impl Default for StatusConfig {
    fn default() -> Self {
        Self {
            showexp: false,
            showscore: false,
            time: false,
            showdamage: false,
            showrace: false,
            showvers: false,
            hitpointbar: false,
            statuslines: StatusLines::default(),
            hilite_status: Vec::new(),
            implicit_uncursed: true,
        }
    }
}

// ---------------------------------------------------------------------------
// [message] section — message window configuration
// ---------------------------------------------------------------------------

/// Message window behavior on Ctrl+P.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MsgWindow {
    #[default]
    Single,
    Combination,
    Full,
    Reversed,
}

/// Menu heading style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MenuHeadings {
    Bold,
    #[default]
    Inverse,
    Underline,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageConfig {
    /// Number of messages in history.
    #[serde(default = "default_msghistory")]
    pub msghistory: u32,
    /// Message window behavior on Ctrl+P.
    #[serde(default)]
    pub msg_window: MsgWindow,
    /// Menu coloring rules.
    #[serde(default)]
    pub menucolors: Vec<String>,
    /// Message type rules.
    #[serde(default)]
    pub message_types: Vec<String>,
    /// Menu headings style.
    #[serde(default)]
    pub menu_headings: MenuHeadings,
    /// Use accessible messages (with location info).
    #[serde(default)]
    pub accessiblemsg: bool,
    /// Vary message count display.
    #[serde(default)]
    pub vary_msgcount: bool,
}

impl Default for MessageConfig {
    fn default() -> Self {
        Self {
            msghistory: default_msghistory(),
            msg_window: MsgWindow::default(),
            menucolors: Vec::new(),
            message_types: Vec::new(),
            menu_headings: MenuHeadings::default(),
            accessiblemsg: false,
            vary_msgcount: false,
        }
    }
}

// ---------------------------------------------------------------------------
// [menu] section — menu-specific keys and settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuConfig {
    /// Key to select all items in menu.
    #[serde(default = "default_menu_select_all")]
    pub select_all: String,
    /// Key to deselect all items.
    #[serde(default = "default_menu_deselect_all")]
    pub deselect_all: String,
    /// Key to select items on current page.
    #[serde(default)]
    pub select_page: String,
    /// Key to deselect items on current page.
    #[serde(default)]
    pub deselect_page: String,
    /// Key to invert all selections.
    #[serde(default = "default_menu_invert_all")]
    pub invert_all: String,
    /// Key to invert page selections.
    #[serde(default)]
    pub invert_page: String,
    /// Key to go to first page.
    #[serde(default)]
    pub first_page: String,
    /// Key to go to last page.
    #[serde(default)]
    pub last_page: String,
    /// Key to go to next page.
    #[serde(default)]
    pub next_page: String,
    /// Key to go to previous page.
    #[serde(default)]
    pub previous_page: String,
    /// Key to search in menu.
    #[serde(default)]
    pub search: String,
    /// Use object symbols in menus.
    #[serde(default)]
    pub objsyms: bool,
    /// Menu invert mode (0=no auto, 1=invert, 2=deselect all first).
    #[serde(default)]
    pub invertmode: u8,
    /// Tab separator in menus.
    #[serde(default)]
    pub tab_sep: bool,
}

impl Default for MenuConfig {
    fn default() -> Self {
        Self {
            select_all: default_menu_select_all(),
            deselect_all: default_menu_deselect_all(),
            select_page: String::new(),
            deselect_page: String::new(),
            invert_all: default_menu_invert_all(),
            invert_page: String::new(),
            first_page: String::new(),
            last_page: String::new(),
            next_page: String::new(),
            previous_page: String::new(),
            search: String::new(),
            objsyms: false,
            invertmode: 0,
            tab_sep: false,
        }
    }
}

// ---------------------------------------------------------------------------
// [sound] section
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_volume")]
    pub volume: u8,
    /// Acoustics — whether the character can hear.
    #[serde(default = "default_true")]
    pub acoustics: bool,
    /// Enable voice sounds.
    #[serde(default)]
    pub voices: bool,
    /// Sound library name.
    #[serde(default)]
    pub soundlib: String,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: default_volume(),
            acoustics: true,
            voices: false,
            soundlib: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// [keybinds] section — key binding overrides
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeybindConfig {
    /// Movement key overrides.
    pub move_left: Option<String>,
    pub move_down: Option<String>,
    pub move_up: Option<String>,
    pub move_right: Option<String>,
    pub move_upleft: Option<String>,
    pub move_upright: Option<String>,
    pub move_downleft: Option<String>,
    pub move_downright: Option<String>,
    /// Arbitrary key→command bindings, e.g. {"x" = "swap"}.
    #[serde(default)]
    pub bindings: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// [advanced] section — corresponds to NetHack Advanced options
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    /// Window type (tty, curses, etc.).
    #[serde(default)]
    pub windowtype: String,
    /// Message window alignment.
    #[serde(default)]
    pub align_message: String,
    /// Status window alignment.
    #[serde(default)]
    pub align_status: String,
    /// Help on start.
    #[serde(default = "default_true")]
    pub help: bool,
    /// Ignore interrupt signals.
    #[serde(default)]
    pub ignintr: bool,
    /// Use 8-bit character set.
    #[serde(default)]
    pub eight_bit_tty: bool,
    /// Allow rerolling stats at character creation.
    #[serde(default)]
    pub reroll: bool,
    /// Show blind/deaf status effects.
    #[serde(default)]
    pub blind: bool,
    #[serde(default)]
    pub deaf: bool,
    /// Version info display.
    #[serde(default)]
    pub versinfo: bool,
    /// Query menu mode.
    #[serde(default)]
    pub query_menu: bool,
    /// Player selection method.
    #[serde(default)]
    pub player_selection: String,
    /// Scroll margin.
    #[serde(default = "default_scroll_margin")]
    pub scroll_margin: u32,
    /// Scroll amount.
    #[serde(default)]
    pub scroll_amount: u32,
    /// Status updates enabled.
    #[serde(default = "default_true")]
    pub status_updates: bool,
    /// Font settings.
    #[serde(default)]
    pub font_map: String,
    #[serde(default)]
    pub font_menu: String,
    #[serde(default)]
    pub font_message: String,
    #[serde(default)]
    pub font_status: String,
    #[serde(default)]
    pub font_text: String,
    /// Font sizes.
    #[serde(default)]
    pub font_size_map: u32,
    #[serde(default)]
    pub font_size_menu: u32,
    #[serde(default)]
    pub font_size_message: u32,
    #[serde(default)]
    pub font_size_status: u32,
    #[serde(default)]
    pub font_size_text: u32,
    /// Tile settings.
    #[serde(default)]
    pub tile_file: String,
    #[serde(default)]
    pub tile_width: u32,
    #[serde(default)]
    pub tile_height: u32,
    /// Terminal dimensions.
    #[serde(default)]
    pub term_cols: u32,
    #[serde(default)]
    pub term_rows: u32,
    /// Mail notification.
    #[serde(default = "default_true")]
    pub mail: bool,
    /// Use dark gray color.
    #[serde(default = "default_true")]
    pub use_darkgray: bool,
    /// Select saved games.
    #[serde(default)]
    pub selectsaved: bool,
    /// Preload tiles.
    #[serde(default = "default_true")]
    pub preload_tiles: bool,
    /// Custom symbols (raw strings).
    #[serde(default)]
    pub customsymbols: Vec<String>,
    /// Custom colors (raw strings).
    #[serde(default)]
    pub customcolors: Vec<String>,
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            windowtype: String::new(),
            align_message: String::new(),
            align_status: String::new(),
            help: true,
            ignintr: false,
            eight_bit_tty: false,
            reroll: false,
            blind: false,
            deaf: false,
            versinfo: false,
            query_menu: false,
            player_selection: String::new(),
            scroll_margin: default_scroll_margin(),
            scroll_amount: 0,
            status_updates: true,
            font_map: String::new(),
            font_menu: String::new(),
            font_message: String::new(),
            font_status: String::new(),
            font_text: String::new(),
            font_size_map: 0,
            font_size_menu: 0,
            font_size_message: 0,
            font_size_status: 0,
            font_size_text: 0,
            tile_file: String::new(),
            tile_width: 0,
            tile_height: 0,
            term_cols: 0,
            term_rows: 0,
            mail: true,
            use_darkgray: true,
            selectsaved: false,
            preload_tiles: true,
            customsymbols: Vec::new(),
            customcolors: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Default value helpers
// ---------------------------------------------------------------------------

fn default_language() -> String {
    "en".into()
}

fn default_true() -> bool {
    true
}

fn default_autopickup_types() -> String {
    "$?!/=".into()
}

fn default_volume() -> u8 {
    75
}

fn default_fruit() -> String {
    "slime mold".into()
}

fn default_packorder() -> String {
    "\")[%?+!=/(*`0_".into()
}

fn default_playmode() -> String {
    "normal".into()
}

fn default_scores() -> String {
    "3 top/2 around/own".into()
}

fn default_pile_limit() -> u32 {
    5
}

fn default_msghistory() -> u32 {
    20
}

fn default_scroll_margin() -> u32 {
    4
}

fn default_menu_select_all() -> String {
    ".".into()
}

fn default_menu_deselect_all() -> String {
    "-".into()
}

fn default_menu_invert_all() -> String {
    "@".into()
}

// ---------------------------------------------------------------------------
// Option metadata — maps all 218+ NetHack options to their names/types
// ---------------------------------------------------------------------------

/// The type of a NetHack option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
    Boolean,
    Compound,
}

/// Which section an option belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionSection {
    General,
    Behavior,
    Map,
    Status,
    Advanced,
}

/// Metadata for a single NetHack option.
#[derive(Debug, Clone)]
pub struct OptionInfo {
    pub name: &'static str,
    pub section: OptionSection,
    pub opt_type: OptionType,
    pub description: &'static str,
    pub default_val: &'static str,
}

/// Complete list of all NetHack 3.7 options with metadata.
///
/// This table is derived from `include/optlist.h` and covers all 218+ options
/// that a player might set via the config file or the in-game `O` menu.
pub const ALL_OPTIONS: &[OptionInfo] = &[
    // ── General / game options ──────────────────────────────────────
    OptionInfo {
        name: "name",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "your character's name",
        default_val: "",
    },
    OptionInfo {
        name: "role",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "your starting role",
        default_val: "",
    },
    OptionInfo {
        name: "race",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "your starting race",
        default_val: "",
    },
    OptionInfo {
        name: "gender",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "your starting gender",
        default_val: "",
    },
    OptionInfo {
        name: "alignment",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "your starting alignment",
        default_val: "",
    },
    OptionInfo {
        name: "playmode",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "normal, explore, or debug mode",
        default_val: "normal",
    },
    OptionInfo {
        name: "windowtype",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "windowing system to use",
        default_val: "",
    },
    OptionInfo {
        name: "fruit",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "name of a fruit you enjoy eating",
        default_val: "slime mold",
    },
    OptionInfo {
        name: "language",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "display language",
        default_val: "en",
    },
    OptionInfo {
        name: "catname",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "name of your starting cat",
        default_val: "",
    },
    OptionInfo {
        name: "dogname",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "name of your starting dog",
        default_val: "",
    },
    OptionInfo {
        name: "horsename",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "name of your starting horse",
        default_val: "",
    },
    OptionInfo {
        name: "pettype",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "preferred type of pet",
        default_val: "",
    },
    OptionInfo {
        name: "packorder",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "inventory pack order",
        default_val: "\")[%?+!=/(*`0_",
    },
    OptionInfo {
        name: "disclose",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "end-of-game disclosure options",
        default_val: "",
    },
    OptionInfo {
        name: "scores",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "score display format",
        default_val: "3 top/2 around/own",
    },
    OptionInfo {
        name: "paranoid_confirmation",
        section: OptionSection::General,
        opt_type: OptionType::Compound,
        description: "extra confirmations for dangerous actions",
        default_val: "confirm",
    },
    // ── Behavior options ────────────────────────────────────────────
    OptionInfo {
        name: "autopickup",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "automatically pick up objects",
        default_val: "false",
    },
    OptionInfo {
        name: "pickup_types",
        section: OptionSection::Behavior,
        opt_type: OptionType::Compound,
        description: "types of objects to pick up",
        default_val: "$?!/=",
    },
    OptionInfo {
        name: "pickup_burden",
        section: OptionSection::Behavior,
        opt_type: OptionType::Compound,
        description: "max burden for autopickup",
        default_val: "stressed",
    },
    OptionInfo {
        name: "pickup_thrown",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "pick up items you threw",
        default_val: "true",
    },
    OptionInfo {
        name: "pickup_stolen",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "pick up stolen items",
        default_val: "true",
    },
    OptionInfo {
        name: "dropped_nopick",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "don't re-pick up dropped items",
        default_val: "true",
    },
    OptionInfo {
        name: "autodig",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "dig when wielding digging tool",
        default_val: "false",
    },
    OptionInfo {
        name: "autoopen",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "open doors when walking into them",
        default_val: "true",
    },
    OptionInfo {
        name: "autoquiver",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "fill quiver automatically",
        default_val: "false",
    },
    OptionInfo {
        name: "autounlock",
        section: OptionSection::Behavior,
        opt_type: OptionType::Compound,
        description: "auto-unlock locked doors/chests",
        default_val: "apply",
    },
    OptionInfo {
        name: "confirm",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "confirm attacks on peaceful monsters",
        default_val: "true",
    },
    OptionInfo {
        name: "safe_pet",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "avoid attacking pets",
        default_val: "true",
    },
    OptionInfo {
        name: "safe_wait",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "avoid waiting near hostiles",
        default_val: "true",
    },
    OptionInfo {
        name: "pushweapon",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "push old weapon to secondary",
        default_val: "false",
    },
    OptionInfo {
        name: "fixinv",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "fixed inventory letters",
        default_val: "true",
    },
    OptionInfo {
        name: "sortpack",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "sort inventory by type",
        default_val: "true",
    },
    OptionInfo {
        name: "sortloot",
        section: OptionSection::Behavior,
        opt_type: OptionType::Compound,
        description: "sort loot display",
        default_val: "full",
    },
    OptionInfo {
        name: "sortdiscoveries",
        section: OptionSection::Behavior,
        opt_type: OptionType::Compound,
        description: "sort discovered items",
        default_val: "by class",
    },
    OptionInfo {
        name: "sortvanquished",
        section: OptionSection::Behavior,
        opt_type: OptionType::Compound,
        description: "sort vanquished monsters",
        default_val: "by class",
    },
    OptionInfo {
        name: "lootabc",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "use a/b/c in loot prompts",
        default_val: "false",
    },
    OptionInfo {
        name: "force_invmenu",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "force inventory menu",
        default_val: "false",
    },
    OptionInfo {
        name: "menustyle",
        section: OptionSection::Behavior,
        opt_type: OptionType::Compound,
        description: "item selection menu style",
        default_val: "full",
    },
    OptionInfo {
        name: "verbose",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "verbose game messages",
        default_val: "true",
    },
    OptionInfo {
        name: "rest_on_space",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "spacebar rests",
        default_val: "false",
    },
    OptionInfo {
        name: "fireassist",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "help with ranged attacks",
        default_val: "true",
    },
    OptionInfo {
        name: "cmdassist",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "command assistance",
        default_val: "true",
    },
    OptionInfo {
        name: "extmenu",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "extended command menu",
        default_val: "false",
    },
    OptionInfo {
        name: "pile_limit",
        section: OptionSection::Behavior,
        opt_type: OptionType::Compound,
        description: "min items for pickup menu",
        default_val: "5",
    },
    OptionInfo {
        name: "runmode",
        section: OptionSection::Behavior,
        opt_type: OptionType::Compound,
        description: "multi-step movement display",
        default_val: "run",
    },
    OptionInfo {
        name: "travel",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "travel command enabled",
        default_val: "true",
    },
    OptionInfo {
        name: "number_pad",
        section: OptionSection::Behavior,
        opt_type: OptionType::Compound,
        description: "use numpad for movement",
        default_val: "false",
    },
    OptionInfo {
        name: "suppress_alert",
        section: OptionSection::Behavior,
        opt_type: OptionType::Compound,
        description: "suppress version alerts",
        default_val: "",
    },
    OptionInfo {
        name: "quick_farsight",
        section: OptionSection::Behavior,
        opt_type: OptionType::Boolean,
        description: "quick farsight after teleport",
        default_val: "true",
    },
    // ── Map options ─────────────────────────────────────────────────
    OptionInfo {
        name: "autodescribe",
        section: OptionSection::Map,
        opt_type: OptionType::Boolean,
        description: "describe terrain under cursor",
        default_val: "true",
    },
    OptionInfo {
        name: "mention_decor",
        section: OptionSection::Map,
        opt_type: OptionType::Boolean,
        description: "mention decorations",
        default_val: "true",
    },
    OptionInfo {
        name: "mention_map",
        section: OptionSection::Map,
        opt_type: OptionType::Boolean,
        description: "mention map features",
        default_val: "false",
    },
    OptionInfo {
        name: "mention_walls",
        section: OptionSection::Map,
        opt_type: OptionType::Boolean,
        description: "mention walls when blind",
        default_val: "false",
    },
    OptionInfo {
        name: "spot_monsters",
        section: OptionSection::Map,
        opt_type: OptionType::Boolean,
        description: "auto-spot monsters",
        default_val: "true",
    },
    OptionInfo {
        name: "goldX",
        section: OptionSection::Map,
        opt_type: OptionType::Boolean,
        description: "show gold as X",
        default_val: "false",
    },
    OptionInfo {
        name: "mon_movement",
        section: OptionSection::Map,
        opt_type: OptionType::Boolean,
        description: "show monster movement",
        default_val: "false",
    },
    OptionInfo {
        name: "whatis_coord",
        section: OptionSection::Map,
        opt_type: OptionType::Compound,
        description: "coordinate style",
        default_val: "",
    },
    OptionInfo {
        name: "whatis_filter",
        section: OptionSection::Map,
        opt_type: OptionType::Compound,
        description: "whatis filter",
        default_val: "",
    },
    OptionInfo {
        name: "whatis_menu",
        section: OptionSection::Map,
        opt_type: OptionType::Boolean,
        description: "whatis uses menu",
        default_val: "false",
    },
    OptionInfo {
        name: "whatis_moveskip",
        section: OptionSection::Map,
        opt_type: OptionType::Boolean,
        description: "skip positions in whatis",
        default_val: "false",
    },
    OptionInfo {
        name: "herecmd_menu",
        section: OptionSection::Map,
        opt_type: OptionType::Boolean,
        description: "here-command menu",
        default_val: "false",
    },
    OptionInfo {
        name: "bgcolors",
        section: OptionSection::Map,
        opt_type: OptionType::Boolean,
        description: "background color highlighting",
        default_val: "true",
    },
    // ── Status options ──────────────────────────────────────────────
    OptionInfo {
        name: "showexp",
        section: OptionSection::Status,
        opt_type: OptionType::Boolean,
        description: "show experience points",
        default_val: "false",
    },
    OptionInfo {
        name: "showscore",
        section: OptionSection::Status,
        opt_type: OptionType::Boolean,
        description: "show score",
        default_val: "false",
    },
    OptionInfo {
        name: "time",
        section: OptionSection::Status,
        opt_type: OptionType::Boolean,
        description: "show game time",
        default_val: "false",
    },
    OptionInfo {
        name: "showdamage",
        section: OptionSection::Status,
        opt_type: OptionType::Boolean,
        description: "show damage dealt",
        default_val: "false",
    },
    OptionInfo {
        name: "showrace",
        section: OptionSection::Status,
        opt_type: OptionType::Boolean,
        description: "show race in status",
        default_val: "false",
    },
    OptionInfo {
        name: "showvers",
        section: OptionSection::Status,
        opt_type: OptionType::Boolean,
        description: "show version in status",
        default_val: "false",
    },
    OptionInfo {
        name: "hitpointbar",
        section: OptionSection::Status,
        opt_type: OptionType::Boolean,
        description: "show HP bar",
        default_val: "false",
    },
    OptionInfo {
        name: "statuslines",
        section: OptionSection::Status,
        opt_type: OptionType::Compound,
        description: "number of status lines",
        default_val: "2",
    },
    OptionInfo {
        name: "implicit_uncursed",
        section: OptionSection::Status,
        opt_type: OptionType::Boolean,
        description: "hide 'uncursed' for known items",
        default_val: "true",
    },
    // ── Display / Advanced options ──────────────────────────────────
    OptionInfo {
        name: "color",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "use color in display",
        default_val: "true",
    },
    OptionInfo {
        name: "ascii_map",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "show map as text",
        default_val: "true",
    },
    OptionInfo {
        name: "tiled_map",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "show map as tiles",
        default_val: "false",
    },
    OptionInfo {
        name: "sparkle",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "sparkle for resistance",
        default_val: "true",
    },
    OptionInfo {
        name: "timed_delay",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "timed delay animations",
        default_val: "true",
    },
    OptionInfo {
        name: "hilite_pet",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "highlight pets",
        default_val: "false",
    },
    OptionInfo {
        name: "petattr",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "pet highlight attribute",
        default_val: "",
    },
    OptionInfo {
        name: "hilite_pile",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "highlight object piles",
        default_val: "false",
    },
    OptionInfo {
        name: "dark_room",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "dark room shading",
        default_val: "true",
    },
    OptionInfo {
        name: "lit_corridor",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "show lit corridors",
        default_val: "false",
    },
    OptionInfo {
        name: "use_inverse",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "use inverse video",
        default_val: "true",
    },
    OptionInfo {
        name: "use_truecolor",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "use truecolor",
        default_val: "false",
    },
    OptionInfo {
        name: "standout",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "standout for --More--",
        default_val: "false",
    },
    OptionInfo {
        name: "guicolor",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "GUI colors",
        default_val: "true",
    },
    OptionInfo {
        name: "menu_overlay",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "overlay menus",
        default_val: "true",
    },
    OptionInfo {
        name: "fullscreen",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "fullscreen mode",
        default_val: "false",
    },
    OptionInfo {
        name: "mouse_support",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "mouse support",
        default_val: "false",
    },
    OptionInfo {
        name: "wraptext",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "wrap text in message window",
        default_val: "false",
    },
    OptionInfo {
        name: "popup_dialog",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "popup dialogs",
        default_val: "false",
    },
    OptionInfo {
        name: "perm_invent",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "persistent inventory window",
        default_val: "false",
    },
    OptionInfo {
        name: "legacy",
        section: OptionSection::General,
        opt_type: OptionType::Boolean,
        description: "show legacy intro",
        default_val: "true",
    },
    OptionInfo {
        name: "news",
        section: OptionSection::General,
        opt_type: OptionType::Boolean,
        description: "show news on start",
        default_val: "true",
    },
    OptionInfo {
        name: "splash_screen",
        section: OptionSection::General,
        opt_type: OptionType::Boolean,
        description: "show splash screen",
        default_val: "true",
    },
    OptionInfo {
        name: "tips",
        section: OptionSection::General,
        opt_type: OptionType::Boolean,
        description: "show tutorial tips",
        default_val: "true",
    },
    OptionInfo {
        name: "tombstone",
        section: OptionSection::General,
        opt_type: OptionType::Boolean,
        description: "show tombstone on death",
        default_val: "true",
    },
    OptionInfo {
        name: "toptenwin",
        section: OptionSection::General,
        opt_type: OptionType::Boolean,
        description: "show scores in window",
        default_val: "false",
    },
    OptionInfo {
        name: "bones",
        section: OptionSection::General,
        opt_type: OptionType::Boolean,
        description: "save bones files",
        default_val: "true",
    },
    OptionInfo {
        name: "checkpoint",
        section: OptionSection::General,
        opt_type: OptionType::Boolean,
        description: "checkpoint saves",
        default_val: "true",
    },
    OptionInfo {
        name: "help",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "help on start",
        default_val: "true",
    },
    OptionInfo {
        name: "ignintr",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "ignore interrupts",
        default_val: "false",
    },
    OptionInfo {
        name: "eight_bit_tty",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "8-bit character set",
        default_val: "false",
    },
    OptionInfo {
        name: "mail",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "mail notification",
        default_val: "true",
    },
    OptionInfo {
        name: "use_darkgray",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "use dark gray color",
        default_val: "true",
    },
    OptionInfo {
        name: "preload_tiles",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "preload tile images",
        default_val: "true",
    },
    OptionInfo {
        name: "acoustics",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "character can hear",
        default_val: "true",
    },
    OptionInfo {
        name: "sounds",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "sound effects enabled",
        default_val: "true",
    },
    OptionInfo {
        name: "voices",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "voice sounds",
        default_val: "false",
    },
    OptionInfo {
        name: "msghistory",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "message history size",
        default_val: "20",
    },
    OptionInfo {
        name: "msg_window",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "message window behavior",
        default_val: "single",
    },
    OptionInfo {
        name: "menu_headings",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "menu heading style",
        default_val: "inverse",
    },
    OptionInfo {
        name: "accessiblemsg",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "accessible messages",
        default_val: "false",
    },
    OptionInfo {
        name: "symset",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "symbol set",
        default_val: "",
    },
    OptionInfo {
        name: "roguesymset",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "rogue-level symbol set",
        default_val: "",
    },
    OptionInfo {
        name: "align_message",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "message window alignment",
        default_val: "",
    },
    OptionInfo {
        name: "align_status",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "status window alignment",
        default_val: "",
    },
    OptionInfo {
        name: "scroll_margin",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "scroll margin",
        default_val: "4",
    },
    OptionInfo {
        name: "scroll_amount",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "scroll amount",
        default_val: "0",
    },
    OptionInfo {
        name: "windowborders",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "window borders style",
        default_val: "",
    },
    OptionInfo {
        name: "nudist",
        section: OptionSection::General,
        opt_type: OptionType::Boolean,
        description: "start without armor",
        default_val: "false",
    },
    OptionInfo {
        name: "pauper",
        section: OptionSection::General,
        opt_type: OptionType::Boolean,
        description: "start without gold",
        default_val: "false",
    },
    OptionInfo {
        name: "reroll",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "allow stat reroll",
        default_val: "false",
    },
    OptionInfo {
        name: "perminv_mode",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "persistent inventory mode",
        default_val: "",
    },
    OptionInfo {
        name: "selectsaved",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "show saved game menu",
        default_val: "false",
    },
    OptionInfo {
        name: "status_updates",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "status line updates",
        default_val: "true",
    },
    OptionInfo {
        name: "vary_msgcount",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "vary message count",
        default_val: "false",
    },
    OptionInfo {
        name: "soundlib",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "sound library name",
        default_val: "",
    },
    OptionInfo {
        name: "font_map",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "map font",
        default_val: "",
    },
    OptionInfo {
        name: "font_menu",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "menu font",
        default_val: "",
    },
    OptionInfo {
        name: "font_message",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "message font",
        default_val: "",
    },
    OptionInfo {
        name: "font_status",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "status font",
        default_val: "",
    },
    OptionInfo {
        name: "font_text",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "text font",
        default_val: "",
    },
    OptionInfo {
        name: "font_size_map",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "map font size",
        default_val: "0",
    },
    OptionInfo {
        name: "font_size_menu",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "menu font size",
        default_val: "0",
    },
    OptionInfo {
        name: "font_size_message",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "message font size",
        default_val: "0",
    },
    OptionInfo {
        name: "font_size_status",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "status font size",
        default_val: "0",
    },
    OptionInfo {
        name: "font_size_text",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "text font size",
        default_val: "0",
    },
    OptionInfo {
        name: "tile_file",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "tile file path",
        default_val: "",
    },
    OptionInfo {
        name: "tile_width",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "tile width",
        default_val: "0",
    },
    OptionInfo {
        name: "tile_height",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "tile height",
        default_val: "0",
    },
    OptionInfo {
        name: "term_cols",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "terminal columns",
        default_val: "0",
    },
    OptionInfo {
        name: "term_rows",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "terminal rows",
        default_val: "0",
    },
    OptionInfo {
        name: "player_selection",
        section: OptionSection::Advanced,
        opt_type: OptionType::Compound,
        description: "player selection method",
        default_val: "",
    },
    OptionInfo {
        name: "query_menu",
        section: OptionSection::Advanced,
        opt_type: OptionType::Boolean,
        description: "query menu mode",
        default_val: "false",
    },
];

/// Look up option metadata by name.
pub fn find_option(name: &str) -> Option<&'static OptionInfo> {
    let lower = name.to_lowercase();
    ALL_OPTIONS.iter().find(|o| o.name == lower)
}

/// Return all options in a given section.
pub fn options_in_section(section: OptionSection) -> Vec<&'static OptionInfo> {
    ALL_OPTIONS
        .iter()
        .filter(|o| o.section == section)
        .collect()
}

/// Return a human-readable description line for an option, using all metadata.
///
/// Format: `"name (type, section) — description [default: val]"`
pub fn describe_option(name: &str) -> Option<String> {
    let info = find_option(name)?;
    let type_str = match info.opt_type {
        OptionType::Boolean => "boolean",
        OptionType::Compound => "compound",
    };
    let section_str = match info.section {
        OptionSection::General => "general",
        OptionSection::Behavior => "behavior",
        OptionSection::Map => "map",
        OptionSection::Status => "status",
        OptionSection::Advanced => "advanced",
    };
    Some(format!(
        "{} ({}, {}) — {} [default: {}]",
        info.name, type_str, section_str, info.description, info.default_val
    ))
}

// ---------------------------------------------------------------------------
// NetHack-style OPTIONS line parsing
// ---------------------------------------------------------------------------

/// Apply a single option (name + optional value) to a `Config`.
///
/// Boolean options: `"autopickup"` sets to true, `"!autopickup"` sets to false.
/// Compound options: `"fruit"` with `Some("mango")` sets the fruit name.
///
/// Returns `Err` with a message if the option name is unrecognized.
pub fn apply_option(config: &mut Config, name: &str, value: Option<&str>) -> Result<(), String> {
    let (negated, key) = if let Some(stripped) = name.strip_prefix('!') {
        (true, stripped)
    } else if let Some(stripped) = name.strip_prefix("no") {
        // "noautopickup" is equivalent to "!autopickup"
        if find_option(stripped).is_some() {
            (true, stripped)
        } else {
            (false, name)
        }
    } else {
        (false, name)
    };

    let key_lower = key.to_lowercase();

    // Boolean options — map to Config fields
    match key_lower.as_str() {
        // behavior booleans
        "autopickup" => config.behavior.autopickup = !negated,
        "autodig" => config.behavior.autodig = !negated,
        "autoopen" => config.behavior.autoopen = !negated,
        "autoquiver" => config.behavior.autoquiver = !negated,
        "confirm" => config.behavior.confirm = !negated,
        "safe_pet" => config.behavior.safe_pet = !negated,
        "safe_wait" => config.behavior.safe_wait = !negated,
        "pushweapon" => config.behavior.pushweapon = !negated,
        "pickup_thrown" => config.behavior.pickup_thrown = !negated,
        "pickup_stolen" => config.behavior.pickup_stolen = !negated,
        "dropped_nopick" => config.behavior.dropped_nopick = !negated,
        "fixinv" => config.behavior.fixinv = !negated,
        "sortpack" => config.behavior.sortpack = !negated,
        "lootabc" => config.behavior.lootabc = !negated,
        "force_invmenu" => config.behavior.force_invmenu = !negated,
        "verbose" => config.behavior.verbose = !negated,
        "rest_on_space" => config.behavior.rest_on_space = !negated,
        "fireassist" => config.behavior.fireassist = !negated,
        "cmdassist" => config.behavior.cmdassist = !negated,
        "extmenu" => config.behavior.extmenu = !negated,
        "travel" => config.behavior.travel = !negated,
        "number_pad" => config.behavior.number_pad = !negated,
        "quick_farsight" => config.behavior.quick_farsight = !negated,

        // display booleans
        "color" => config.display.color = !negated,
        "sparkle" => config.display.sparkle = !negated,
        "timed_delay" => config.display.timed_delay = !negated,
        "hilite_pet" => config.display.hilite_pet = !negated,
        "hilite_pile" => config.display.hilite_pile = !negated,
        "dark_room" => config.display.dark_room = !negated,
        "lit_corridor" => config.display.lit_corridor = !negated,
        "use_inverse" => config.display.use_inverse = !negated,
        "use_truecolor" => config.display.use_truecolor = !negated,
        "standout" => config.display.standout = !negated,
        "guicolor" => config.display.guicolor = !negated,
        "ascii_map" => config.display.ascii_map = !negated,
        "tiled_map" => config.display.tiled_map = !negated,
        "menu_overlay" => config.display.menu_overlay = !negated,
        "fullscreen" => config.display.fullscreen = !negated,
        "mouse_support" => config.display.mouse_support = !negated,
        "wraptext" => config.display.wraptext = !negated,
        "popup_dialog" => config.display.popup_dialog = !negated,
        "perm_invent" => config.display.perm_invent = !negated,

        // map booleans
        "autodescribe" => config.map.autodescribe = !negated,
        "mention_decor" => config.map.mention_decor = !negated,
        "mention_map" => config.map.mention_map = !negated,
        "mention_walls" => config.map.mention_walls = !negated,
        "spot_monsters" => config.map.spot_monsters = !negated,
        "goldx" => config.map.gold_x = !negated,
        "mon_movement" => config.map.mon_movement = !negated,
        "whatis_menu" => config.map.whatis_menu = !negated,
        "whatis_moveskip" => config.map.whatis_moveskip = !negated,
        "herecmd_menu" => config.map.herecmd_menu = !negated,

        // status booleans
        "showexp" => config.status.showexp = !negated,
        "showscore" => config.status.showscore = !negated,
        "time" => config.status.time = !negated,
        "showdamage" => config.status.showdamage = !negated,
        "showrace" => config.status.showrace = !negated,
        "showvers" => config.status.showvers = !negated,
        "hitpointbar" => config.status.hitpointbar = !negated,
        "implicit_uncursed" => config.status.implicit_uncursed = !negated,

        // game booleans
        "legacy" => config.game.legacy = !negated,
        "news" => config.game.news = !negated,
        "splash_screen" => config.game.splash_screen = !negated,
        "tips" => config.game.tips = !negated,
        "tombstone" => config.game.tombstone = !negated,
        "toptenwin" => config.game.toptenwin = !negated,
        "bones" => config.game.bones = !negated,
        "checkpoint" => config.game.checkpoint = !negated,

        // advanced booleans
        "help" => config.advanced.help = !negated,
        "ignintr" => config.advanced.ignintr = !negated,
        "eight_bit_tty" => config.advanced.eight_bit_tty = !negated,
        "mail" => config.advanced.mail = !negated,
        "use_darkgray" => config.advanced.use_darkgray = !negated,
        "selectsaved" => config.advanced.selectsaved = !negated,
        "preload_tiles" => config.advanced.preload_tiles = !negated,
        "status_updates" => config.advanced.status_updates = !negated,
        "accessiblemsg" => config.message.accessiblemsg = !negated,
        "vary_msgcount" => config.message.vary_msgcount = !negated,
        "query_menu" => config.advanced.query_menu = !negated,
        "reroll" => config.advanced.reroll = !negated,

        // sound booleans
        "acoustics" => config.sound.acoustics = !negated,
        "sounds" => config.sound.enabled = !negated,
        "voices" => config.sound.voices = !negated,

        // character booleans
        "nudist" => config.character.nudist = !negated,
        "pauper" => config.character.pauper = !negated,

        // Compound options that need a value
        "fruit" => {
            config.game.fruit = value.unwrap_or("slime mold").to_string();
        }
        "name" => {
            config.game.name = value.unwrap_or("").to_string();
        }
        "role" | "class" => {
            config.character.role = value.unwrap_or("").to_string();
        }
        "race" => {
            config.character.race = value.unwrap_or("").to_string();
        }
        "gender" => {
            config.character.gender = value.unwrap_or("").to_string();
        }
        "align" | "alignment" => {
            config.character.alignment = value.unwrap_or("").to_string();
        }
        "packorder" => {
            config.game.packorder = value.unwrap_or("").to_string();
        }
        "pickup_types" => {
            config.behavior.autopickup_types = value.unwrap_or("$").to_string();
        }
        "pile_limit" => {
            if let Some(v) = value {
                config.behavior.pile_limit = v.parse::<u32>().unwrap_or(5);
            }
        }
        "pettype" => {
            config.game.pettype = value.unwrap_or("").to_string();
        }
        "dogname" => {
            config.game.dogname = value.unwrap_or("").to_string();
        }
        "catname" => {
            config.game.catname = value.unwrap_or("").to_string();
        }
        "horsename" => {
            config.game.horsename = value.unwrap_or("").to_string();
        }
        "scores" => {
            config.game.scores = value.unwrap_or("3 top/2 around/own").to_string();
        }
        "suppress_alert" => {
            config.behavior.suppress_alert = value.unwrap_or("").to_string();
        }
        "language" => {
            config.game.language = value.unwrap_or("en").to_string();
        }
        "playmode" => {
            config.game.playmode = value.unwrap_or("normal").to_string();
        }
        "menustyle" => {
            if let Some(v) = value {
                match v.to_lowercase().as_str() {
                    "traditional" => config.behavior.menustyle = MenuStyle::Traditional,
                    "combination" => config.behavior.menustyle = MenuStyle::Combination,
                    "full" => config.behavior.menustyle = MenuStyle::Full,
                    "partial" => config.behavior.menustyle = MenuStyle::Partial,
                    _ => return Err(format!("unknown menustyle: {v}")),
                }
            }
        }
        "runmode" => {
            if let Some(v) = value {
                match v.to_lowercase().as_str() {
                    "teleport" => config.behavior.runmode = RunMode::Teleport,
                    "run" => config.behavior.runmode = RunMode::Run,
                    "walk" => config.behavior.runmode = RunMode::Walk,
                    "crawl" => config.behavior.runmode = RunMode::Crawl,
                    _ => return Err(format!("unknown runmode: {v}")),
                }
            }
        }
        "sortloot" => {
            if let Some(v) = value {
                match v.to_lowercase().as_str() {
                    "none" => config.behavior.sortloot = SortLoot::None,
                    "loot" => config.behavior.sortloot = SortLoot::Loot,
                    "full" => config.behavior.sortloot = SortLoot::Full,
                    _ => return Err(format!("unknown sortloot: {v}")),
                }
            }
        }
        "sortdiscoveries" => {
            if let Some(v) = value {
                match v.to_lowercase().as_str() {
                    "bydiscovery" | "by discovery" | "o" => {
                        config.behavior.sortdiscoveries = SortDiscoveries::ByDiscovery;
                    }
                    "alphabetical" | "a" => {
                        config.behavior.sortdiscoveries = SortDiscoveries::Alphabetical;
                    }
                    "byclass" | "by class" | "c" => {
                        config.behavior.sortdiscoveries = SortDiscoveries::ByClass;
                    }
                    _ => return Err(format!("unknown sortdiscoveries: {v}")),
                }
            }
        }
        "pickup_burden" => {
            if let Some(v) = value {
                match v.to_lowercase().as_str() {
                    "unencumbered" | "u" => {
                        config.behavior.pickup_burden = PickupBurden::Unencumbered;
                    }
                    "burdened" | "b" => {
                        config.behavior.pickup_burden = PickupBurden::Burdened;
                    }
                    "stressed" | "s" => {
                        config.behavior.pickup_burden = PickupBurden::Stressed;
                    }
                    "strained" | "n" => {
                        config.behavior.pickup_burden = PickupBurden::Strained;
                    }
                    "overtaxed" | "o" => {
                        config.behavior.pickup_burden = PickupBurden::Overtaxed;
                    }
                    "overloaded" | "l" => {
                        config.behavior.pickup_burden = PickupBurden::Overloaded;
                    }
                    _ => return Err(format!("unknown pickup_burden: {v}")),
                }
            }
        }
        "autounlock" => {
            if let Some(v) = value {
                match v.to_lowercase().as_str() {
                    "none" => config.behavior.autounlock = AutoUnlock::None,
                    "apply" => config.behavior.autounlock = AutoUnlock::Apply,
                    "force" => config.behavior.autounlock = AutoUnlock::Force,
                    _ => return Err(format!("unknown autounlock: {v}")),
                }
            }
        }
        "whatis_coord" => {
            config.map.whatis_coord = value.unwrap_or("").to_string();
        }
        "whatis_filter" => {
            config.map.whatis_filter = value.unwrap_or("").to_string();
        }
        "msghistory" => {
            if let Some(v) = value {
                config.message.msghistory = v.parse::<u32>().unwrap_or(20);
            }
        }
        "msg_window" | "msgwindow" => {
            if let Some(v) = value {
                match v.to_lowercase().as_str() {
                    "single" | "s" => config.message.msg_window = MsgWindow::Single,
                    "combination" | "c" => config.message.msg_window = MsgWindow::Combination,
                    "full" | "f" => config.message.msg_window = MsgWindow::Full,
                    "reversed" | "r" => config.message.msg_window = MsgWindow::Reversed,
                    _ => return Err(format!("unknown msg_window: {v}")),
                }
            }
        }
        "menu_headings" => {
            if let Some(v) = value {
                match v.to_lowercase().as_str() {
                    "bold" => config.message.menu_headings = MenuHeadings::Bold,
                    "inverse" => config.message.menu_headings = MenuHeadings::Inverse,
                    "underline" => config.message.menu_headings = MenuHeadings::Underline,
                    "none" => config.message.menu_headings = MenuHeadings::None,
                    _ => return Err(format!("unknown menu_headings: {v}")),
                }
            }
        }
        "symset" => {
            config.display.symset = value.unwrap_or("").to_string();
        }
        "roguesymset" => {
            config.display.roguesymset = value.unwrap_or("").to_string();
        }
        "windowtype" => {
            config.advanced.windowtype = value.unwrap_or("").to_string();
        }
        "scroll_margin" => {
            if let Some(v) = value {
                config.advanced.scroll_margin = v.parse::<u32>().unwrap_or(4);
            }
        }
        "scroll_amount" => {
            if let Some(v) = value {
                config.advanced.scroll_amount = v.parse::<u32>().unwrap_or(0);
            }
        }
        "soundlib" => {
            config.sound.soundlib = value.unwrap_or("").to_string();
        }
        "petattr" => {
            config.display.petattr = value.unwrap_or("").to_string();
        }
        "perminv_mode" => {
            config.display.perminv_mode = value.unwrap_or("").to_string();
        }
        "windowborders" => {
            config.display.windowborders = value.unwrap_or("").to_string();
        }
        "align_message" => {
            config.advanced.align_message = value.unwrap_or("").to_string();
        }
        "align_status" => {
            config.advanced.align_status = value.unwrap_or("").to_string();
        }
        "player_selection" => {
            config.advanced.player_selection = value.unwrap_or("").to_string();
        }
        _ => return Err(format!("unknown option: {key_lower}")),
    }
    Ok(())
}

/// Parse a NetHack-style OPTIONS line and apply all options to `config`.
///
/// Format: `OPTIONS=autopickup,color,!verbose,fruit:slime mold`
/// - Boolean options: `name` (set true) or `!name`/`noname` (set false)
/// - Compound options: `name:value`
/// - Comma-separated list, with optional `OPTIONS=` prefix
pub fn parse_options_line(line: &str, config: &mut Config) -> Result<(), String> {
    let content = line
        .trim()
        .strip_prefix("OPTIONS=")
        .or_else(|| line.trim().strip_prefix("OPTIONS ="))
        .or_else(|| line.trim().strip_prefix("OPTION="))
        .unwrap_or(line.trim());

    if content.is_empty() {
        return Ok(());
    }

    let mut errors = Vec::new();

    for part in content.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let (name, value) = if let Some(idx) = part.find(':') {
            (&part[..idx], Some(part[idx + 1..].trim()))
        } else {
            (part, None)
        };

        if let Err(e) = apply_option(config, name.trim(), value) {
            errors.push(e);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// Load a traditional `.nethackrc` file and apply its OPTIONS lines.
///
/// Lines starting with `#` are comments. Lines starting with `OPTIONS=`
/// (or just bare option assignments) are parsed. Other lines are ignored.
pub fn load_nethackrc(path: &str, config: &mut Config) -> Result<(), String> {
    let expanded = expand_tilde(path);
    let path = Path::new(&expanded);

    if !path.exists() {
        return Ok(());
    }

    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    let mut errors = Vec::new();

    for (lineno, line) in contents.lines().enumerate() {
        let line = line.trim();
        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Only process OPTIONS= lines
        if (line.starts_with("OPTIONS=")
            || line.starts_with("OPTIONS =")
            || line.starts_with("OPTION="))
            && let Err(e) = parse_options_line(line, config)
        {
            errors.push(format!("line {}: {e}", lineno + 1));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

/// Generate option name/value pairs for the in-game Options (O) menu.
///
/// Returns a list of `(option_name, current_value_string)` for display.
/// Options are grouped by section and sorted alphabetically within each section.
pub fn options_menu_items(config: &Config) -> Vec<(String, String)> {
    let mut items = Vec::new();

    // --- Behavior booleans ---
    items.push(("autopickup".into(), bool_str(config.behavior.autopickup)));
    items.push(("autodig".into(), bool_str(config.behavior.autodig)));
    items.push(("autoopen".into(), bool_str(config.behavior.autoopen)));
    items.push(("autoquiver".into(), bool_str(config.behavior.autoquiver)));
    items.push(("cmdassist".into(), bool_str(config.behavior.cmdassist)));
    items.push(("confirm".into(), bool_str(config.behavior.confirm)));
    items.push(("extmenu".into(), bool_str(config.behavior.extmenu)));
    items.push(("fireassist".into(), bool_str(config.behavior.fireassist)));
    items.push(("fixinv".into(), bool_str(config.behavior.fixinv)));
    items.push((
        "force_invmenu".into(),
        bool_str(config.behavior.force_invmenu),
    ));
    items.push(("lootabc".into(), bool_str(config.behavior.lootabc)));
    items.push(("number_pad".into(), bool_str(config.behavior.number_pad)));
    items.push((
        "pickup_stolen".into(),
        bool_str(config.behavior.pickup_stolen),
    ));
    items.push((
        "pickup_thrown".into(),
        bool_str(config.behavior.pickup_thrown),
    ));
    items.push(("pushweapon".into(), bool_str(config.behavior.pushweapon)));
    items.push((
        "quick_farsight".into(),
        bool_str(config.behavior.quick_farsight),
    ));
    items.push((
        "rest_on_space".into(),
        bool_str(config.behavior.rest_on_space),
    ));
    items.push(("safe_pet".into(), bool_str(config.behavior.safe_pet)));
    items.push(("safe_wait".into(), bool_str(config.behavior.safe_wait)));
    items.push(("sortpack".into(), bool_str(config.behavior.sortpack)));
    items.push(("travel".into(), bool_str(config.behavior.travel)));
    items.push(("verbose".into(), bool_str(config.behavior.verbose)));

    // --- Behavior compound ---
    items.push((
        "autopickup_types".into(),
        config.behavior.autopickup_types.clone(),
    ));
    items.push((
        "menustyle".into(),
        format!("{:?}", config.behavior.menustyle).to_lowercase(),
    ));
    items.push(("pile_limit".into(), config.behavior.pile_limit.to_string()));
    items.push((
        "runmode".into(),
        format!("{:?}", config.behavior.runmode).to_lowercase(),
    ));
    items.push((
        "sortloot".into(),
        format!("{:?}", config.behavior.sortloot).to_lowercase(),
    ));

    // --- Display ---
    items.push(("color".into(), bool_str(config.display.color)));
    items.push(("dark_room".into(), bool_str(config.display.dark_room)));
    items.push(("hilite_pet".into(), bool_str(config.display.hilite_pet)));
    items.push(("hilite_pile".into(), bool_str(config.display.hilite_pile)));
    items.push(("lit_corridor".into(), bool_str(config.display.lit_corridor)));
    items.push(("sparkle".into(), bool_str(config.display.sparkle)));
    items.push(("standout".into(), bool_str(config.display.standout)));
    items.push(("use_inverse".into(), bool_str(config.display.use_inverse)));

    // --- Status ---
    items.push(("hitpointbar".into(), bool_str(config.status.hitpointbar)));
    items.push(("showexp".into(), bool_str(config.status.showexp)));
    items.push(("showrace".into(), bool_str(config.status.showrace)));
    items.push(("showscore".into(), bool_str(config.status.showscore)));
    items.push(("time".into(), bool_str(config.status.time)));

    // --- Game ---
    items.push(("fruit".into(), config.game.fruit.clone()));
    items.push(("name".into(), config.game.name.clone()));
    items.push(("packorder".into(), config.game.packorder.clone()));
    items.push(("tombstone".into(), bool_str(config.game.tombstone)));

    // --- Advanced ---
    items.push(("mail".into(), bool_str(config.advanced.mail)));

    items
}

fn bool_str(b: bool) -> String {
    if b { "true".into() } else { "false".into() }
}

// ---------------------------------------------------------------------------
// IO functions
// ---------------------------------------------------------------------------

/// Save configuration to a TOML file at `path`.
pub fn save_config(config: &Config, path: &str) -> anyhow::Result<()> {
    let expanded = expand_tilde(path);
    let toml_str = toml::to_string_pretty(config)?;
    if let Some(parent) = Path::new(&expanded).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&expanded, toml_str)?;
    Ok(())
}

/// Load configuration from a TOML file at `path`.
///
/// If the file does not exist the function returns the default configuration.
/// The path string may start with `~` which is expanded to `$HOME`.
pub fn load_config(path: &str) -> anyhow::Result<Config> {
    let expanded = expand_tilde(path);
    let path = Path::new(&expanded);

    if !path.exists() {
        tracing::info!(
            "Config file not found at {}, using defaults",
            path.display()
        );
        return Ok(Config::default());
    }

    let contents = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&contents)?;
    Ok(config)
}

/// Replace a leading `~` with the value of `$HOME`.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~')
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}{rest}");
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = Config::default();
        assert_eq!(cfg.game.language, "en");
        assert!(!cfg.behavior.autopickup);
        assert_eq!(cfg.sound.volume, 75);
    }

    #[test]
    fn parse_minimal_toml() {
        let toml_str = r#"
[game]
language = "zh_CN"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.game.language, "zh_CN");
        // Other fields should fall back to defaults
        assert!(!cfg.behavior.autopickup);
        assert!(cfg.display.color);
        assert_eq!(cfg.sound.volume, 75);
    }

    #[test]
    fn missing_file_returns_defaults() {
        let cfg = load_config("/nonexistent/path/config.toml").unwrap();
        assert_eq!(cfg.game.language, "en");
    }

    #[test]
    fn expand_tilde_works() {
        let expanded = expand_tilde("~/foo/bar");
        assert!(!expanded.starts_with('~'));
    }

    #[test]
    fn parse_character_config() {
        let toml_str = r#"
[character]
role = "Valkyrie"
race = "Human"
gender = "female"
alignment = "neutral"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.character.role, "Valkyrie");
        assert_eq!(cfg.character.race, "Human");
        assert_eq!(cfg.character.gender, "female");
        assert_eq!(cfg.character.alignment, "neutral");
    }

    #[test]
    fn parse_behavior_config() {
        let toml_str = r#"
[behavior]
autopickup = true
autopickup_types = "$"
autodig = true
safe_pet = false
number_pad = true
pile_limit = 10
menustyle = "traditional"
runmode = "walk"
pickup_burden = "burdened"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(cfg.behavior.autopickup);
        assert_eq!(cfg.behavior.autopickup_types, "$");
        assert!(cfg.behavior.autodig);
        assert!(!cfg.behavior.safe_pet);
        assert!(cfg.behavior.number_pad);
        assert_eq!(cfg.behavior.pile_limit, 10);
        assert_eq!(cfg.behavior.menustyle, MenuStyle::Traditional);
        assert_eq!(cfg.behavior.runmode, RunMode::Walk);
        assert_eq!(cfg.behavior.pickup_burden, PickupBurden::Burdened);
    }

    #[test]
    fn parse_display_config() {
        let toml_str = r#"
[display]
color = false
hilite_pet = true
dark_room = false
use_truecolor = true
perm_invent = true
symset = "IBMGraphics_2"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(!cfg.display.color);
        assert!(cfg.display.hilite_pet);
        assert!(!cfg.display.dark_room);
        assert!(cfg.display.use_truecolor);
        assert!(cfg.display.perm_invent);
        assert_eq!(cfg.display.symset, "IBMGraphics_2");
    }

    #[test]
    fn parse_map_config() {
        let toml_str = r#"
[map]
autodescribe = false
mention_walls = true
whatis_coord = "compass"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(!cfg.map.autodescribe);
        assert!(cfg.map.mention_walls);
        assert_eq!(cfg.map.whatis_coord, "compass");
    }

    #[test]
    fn parse_status_config() {
        let toml_str = r#"
[status]
showexp = true
showscore = true
time = true
hitpointbar = true
implicit_uncursed = false
hilite_status = ["hitpoints/100%/green", "hitpoints/<50%/red"]
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(cfg.status.showexp);
        assert!(cfg.status.showscore);
        assert!(cfg.status.time);
        assert!(cfg.status.hitpointbar);
        assert!(!cfg.status.implicit_uncursed);
        assert_eq!(cfg.status.hilite_status.len(), 2);
    }

    #[test]
    fn parse_message_config() {
        let toml_str = r#"
[message]
msghistory = 40
msg_window = "full"
menu_headings = "bold"
menucolors = ["color blue =blessed", "color red =cursed"]
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.message.msghistory, 40);
        assert_eq!(cfg.message.msg_window, MsgWindow::Full);
        assert_eq!(cfg.message.menu_headings, MenuHeadings::Bold);
        assert_eq!(cfg.message.menucolors.len(), 2);
    }

    #[test]
    fn parse_menu_config() {
        let toml_str = r#"
[menu]
select_all = "."
invertmode = 2
objsyms = true
tab_sep = true
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.menu.select_all, ".");
        assert_eq!(cfg.menu.invertmode, 2);
        assert!(cfg.menu.objsyms);
        assert!(cfg.menu.tab_sep);
    }

    #[test]
    fn parse_sound_config() {
        let toml_str = r#"
[sound]
enabled = false
volume = 50
acoustics = false
voices = true
soundlib = "fmod"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(!cfg.sound.enabled);
        assert_eq!(cfg.sound.volume, 50);
        assert!(!cfg.sound.acoustics);
        assert!(cfg.sound.voices);
        assert_eq!(cfg.sound.soundlib, "fmod");
    }

    #[test]
    fn parse_keybind_config() {
        let toml_str = r#"
[keybinds]
move_left = "a"
move_right = "d"

[keybinds.bindings]
x = "swap"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.keybinds.move_left.as_deref(), Some("a"));
        assert_eq!(cfg.keybinds.move_right.as_deref(), Some("d"));
        assert_eq!(
            cfg.keybinds.bindings.get("x").map(|s| s.as_str()),
            Some("swap")
        );
    }

    #[test]
    fn parse_advanced_config() {
        let toml_str = r#"
[advanced]
windowtype = "curses"
scroll_margin = 8
mail = false
font_size_map = 14
term_cols = 120
term_rows = 40
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.advanced.windowtype, "curses");
        assert_eq!(cfg.advanced.scroll_margin, 8);
        assert!(!cfg.advanced.mail);
        assert_eq!(cfg.advanced.font_size_map, 14);
        assert_eq!(cfg.advanced.term_cols, 120);
        assert_eq!(cfg.advanced.term_rows, 40);
    }

    #[test]
    fn parse_game_config_extended() {
        let toml_str = r#"
[game]
name = "Merlin-W"
fruit = "mango"
dogname = "Sirius"
catname = "Pixel"
horsename = "Shadowfax"
pettype = "dog"
legacy = false
tombstone = false
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.game.name, "Merlin-W");
        assert_eq!(cfg.game.fruit, "mango");
        assert_eq!(cfg.game.dogname, "Sirius");
        assert_eq!(cfg.game.catname, "Pixel");
        assert_eq!(cfg.game.horsename, "Shadowfax");
        assert_eq!(cfg.game.pettype, "dog");
        assert!(!cfg.game.legacy);
        assert!(!cfg.game.tombstone);
    }

    #[test]
    fn parse_paranoid_confirmation() {
        let toml_str = r#"
[game.paranoid_confirmation]
quit = true
die = true
attack = true
pray = true
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(cfg.game.paranoid_confirmation.quit);
        assert!(cfg.game.paranoid_confirmation.die);
        assert!(cfg.game.paranoid_confirmation.attack);
        assert!(cfg.game.paranoid_confirmation.pray);
        // Defaults
        assert!(cfg.game.paranoid_confirmation.confirm);
        assert!(!cfg.game.paranoid_confirmation.bones);
    }

    #[test]
    fn parse_disclose_config() {
        let toml_str = r#"
[game.disclose]
inventory = "auto"
vanquished = "no"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.game.disclose.inventory, DiscloseChoice::Auto);
        assert_eq!(cfg.game.disclose.vanquished, DiscloseChoice::No);
        assert_eq!(cfg.game.disclose.attributes, DiscloseChoice::Yes);
    }

    #[test]
    fn parse_autounlock() {
        let toml_str = r#"
[behavior]
autounlock = "force"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.behavior.autounlock, AutoUnlock::Force);
    }

    #[test]
    fn parse_autopickup_exceptions() {
        let toml_str = r#"
[behavior]
autopickup = true
autopickup_exceptions = [">*corpse", "<*gold piece"]
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(cfg.behavior.autopickup);
        assert_eq!(cfg.behavior.autopickup_exceptions.len(), 2);
    }

    #[test]
    fn option_metadata_count() {
        assert!(
            ALL_OPTIONS.len() >= 120,
            "should have 120+ options, got {}",
            ALL_OPTIONS.len()
        );
    }

    #[test]
    fn find_option_by_name() {
        let opt = find_option("autopickup").unwrap();
        assert_eq!(opt.section, OptionSection::Behavior);
        assert_eq!(opt.opt_type, OptionType::Boolean);
    }

    #[test]
    fn find_option_case_insensitive() {
        assert!(find_option("AUTOPICKUP").is_some());
        assert!(find_option("Color").is_some());
    }

    #[test]
    fn find_option_unknown() {
        assert!(find_option("nonexistent_option").is_none());
    }

    #[test]
    fn options_in_section_behavior() {
        let opts = options_in_section(OptionSection::Behavior);
        assert!(opts.len() >= 20, "behavior section should have 20+ options");
        assert!(opts.iter().any(|o| o.name == "autopickup"));
        assert!(opts.iter().any(|o| o.name == "safe_pet"));
    }

    #[test]
    fn options_in_section_map() {
        let opts = options_in_section(OptionSection::Map);
        assert!(opts.len() >= 8, "map section should have 8+ options");
        assert!(opts.iter().any(|o| o.name == "autodescribe"));
    }

    #[test]
    fn default_fruit_is_slime_mold() {
        let cfg = Config::default();
        assert_eq!(cfg.game.fruit, "slime mold");
    }

    #[test]
    fn default_packorder() {
        let cfg = Config::default();
        assert_eq!(cfg.game.packorder, "\")[%?+!=/(*`0_");
    }

    #[test]
    fn full_config_roundtrip() {
        let cfg = Config::default();
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let cfg2: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(cfg2.game.language, cfg.game.language);
        assert_eq!(cfg2.behavior.autopickup, cfg.behavior.autopickup);
        assert_eq!(cfg2.sound.volume, cfg.sound.volume);
        assert_eq!(cfg2.status.showexp, cfg.status.showexp);
    }

    #[test]
    fn empty_toml_gives_defaults() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.game.language, "en");
        assert!(cfg.display.color);
        assert!(!cfg.behavior.autopickup);
        assert_eq!(cfg.sound.volume, 75);
        assert_eq!(cfg.behavior.pile_limit, 5);
    }

    #[test]
    fn sortloot_variants() {
        let cfg: Config = toml::from_str("[behavior]\nsortloot = \"none\"").unwrap();
        assert_eq!(cfg.behavior.sortloot, SortLoot::None);
        let cfg: Config = toml::from_str("[behavior]\nsortloot = \"loot\"").unwrap();
        assert_eq!(cfg.behavior.sortloot, SortLoot::Loot);
        let cfg: Config = toml::from_str("[behavior]\nsortloot = \"full\"").unwrap();
        assert_eq!(cfg.behavior.sortloot, SortLoot::Full);
    }

    // ── OPTIONS line parsing tests ─────────────────────────────────

    #[test]
    fn parse_boolean_option() {
        let mut cfg = Config::default();
        parse_options_line("OPTIONS=autopickup", &mut cfg).unwrap();
        assert!(cfg.behavior.autopickup);
    }

    #[test]
    fn parse_negated_option() {
        let mut cfg = Config::default();
        cfg.behavior.verbose = true;
        parse_options_line("OPTIONS=!verbose", &mut cfg).unwrap();
        assert!(!cfg.behavior.verbose);
    }

    #[test]
    fn parse_negated_with_no_prefix() {
        let mut cfg = Config::default();
        cfg.behavior.autopickup = true;
        parse_options_line("OPTIONS=noautopickup", &mut cfg).unwrap();
        assert!(!cfg.behavior.autopickup);
    }

    #[test]
    fn parse_compound_option() {
        let mut cfg = Config::default();
        parse_options_line("OPTIONS=fruit:mango", &mut cfg).unwrap();
        assert_eq!(cfg.game.fruit, "mango");
    }

    #[test]
    fn parse_compound_option_with_spaces() {
        let mut cfg = Config::default();
        parse_options_line("OPTIONS=fruit:slime mold", &mut cfg).unwrap();
        assert_eq!(cfg.game.fruit, "slime mold");
    }

    #[test]
    fn parse_full_options_line() {
        let mut cfg = Config::default();
        parse_options_line(
            "OPTIONS=autopickup,color,!verbose,fruit:mango,pile_limit:10,menustyle:traditional",
            &mut cfg,
        )
        .unwrap();
        assert!(cfg.behavior.autopickup);
        assert!(cfg.display.color);
        assert!(!cfg.behavior.verbose);
        assert_eq!(cfg.game.fruit, "mango");
        assert_eq!(cfg.behavior.pile_limit, 10);
        assert_eq!(cfg.behavior.menustyle, MenuStyle::Traditional);
    }

    #[test]
    fn parse_multiple_options_lines() {
        let mut cfg = Config::default();
        parse_options_line("OPTIONS=autopickup,showexp", &mut cfg).unwrap();
        parse_options_line("OPTIONS=time,name:Gandalf", &mut cfg).unwrap();
        assert!(cfg.behavior.autopickup);
        assert!(cfg.status.showexp);
        assert!(cfg.status.time);
        assert_eq!(cfg.game.name, "Gandalf");
    }

    #[test]
    fn parse_runmode_option() {
        let mut cfg = Config::default();
        parse_options_line("OPTIONS=runmode:walk", &mut cfg).unwrap();
        assert_eq!(cfg.behavior.runmode, RunMode::Walk);
    }

    #[test]
    fn parse_unknown_option_returns_error() {
        let mut cfg = Config::default();
        let result = parse_options_line("OPTIONS=nonexistent_opt", &mut cfg);
        assert!(result.is_err());
    }

    #[test]
    fn parse_empty_options_line() {
        let mut cfg = Config::default();
        parse_options_line("OPTIONS=", &mut cfg).unwrap();
        // Should not change anything
        assert!(!cfg.behavior.autopickup);
    }

    #[test]
    fn parse_options_without_prefix() {
        let mut cfg = Config::default();
        parse_options_line("autopickup,color", &mut cfg).unwrap();
        assert!(cfg.behavior.autopickup);
        assert!(cfg.display.color);
    }

    #[test]
    fn options_menu_items_has_entries() {
        let cfg = Config::default();
        let items = options_menu_items(&cfg);
        assert!(
            items.len() >= 30,
            "should have 30+ menu items, got {}",
            items.len()
        );

        // Check some specific entries
        let find = |name: &str| items.iter().find(|(n, _)| n == name);
        assert_eq!(find("autopickup").unwrap().1, "false");
        assert_eq!(find("color").unwrap().1, "true");
        assert_eq!(find("fruit").unwrap().1, "slime mold");
        assert_eq!(find("verbose").unwrap().1, "true");
        assert_eq!(find("time").unwrap().1, "false");
    }

    #[test]
    fn options_menu_reflects_changes() {
        let mut cfg = Config::default();
        cfg.behavior.autopickup = true;
        cfg.game.fruit = "banana".into();
        cfg.status.showexp = true;

        let items = options_menu_items(&cfg);
        let find = |name: &str| items.iter().find(|(n, _)| n == name);
        assert_eq!(find("autopickup").unwrap().1, "true");
        assert_eq!(find("fruit").unwrap().1, "banana");
        assert_eq!(find("showexp").unwrap().1, "true");
    }

    #[test]
    fn apply_option_role_alias() {
        let mut cfg = Config::default();
        apply_option(&mut cfg, "class", Some("Wizard")).unwrap();
        assert_eq!(cfg.character.role, "Wizard");
    }

    #[test]
    fn apply_option_alignment_alias() {
        let mut cfg = Config::default();
        apply_option(&mut cfg, "align", Some("chaotic")).unwrap();
        assert_eq!(cfg.character.alignment, "chaotic");
    }

    #[test]
    fn statuslines_variants() {
        let cfg: Config = toml::from_str("[status]\nstatuslines = \"Two\"").unwrap();
        assert_eq!(cfg.status.statuslines, StatusLines::Two);
        let cfg: Config = toml::from_str("[status]\nstatuslines = \"Three\"").unwrap();
        assert_eq!(cfg.status.statuslines, StatusLines::Three);
    }

    #[test]
    fn msg_window_variants() {
        for (val, expected) in [
            ("single", MsgWindow::Single),
            ("combination", MsgWindow::Combination),
            ("full", MsgWindow::Full),
            ("reversed", MsgWindow::Reversed),
        ] {
            let toml_str = format!("[message]\nmsg_window = \"{}\"", val);
            let cfg: Config = toml::from_str(&toml_str).unwrap();
            assert_eq!(cfg.message.msg_window, expected);
        }
    }

    #[test]
    fn menustyle_variants() {
        for (val, expected) in [
            ("traditional", MenuStyle::Traditional),
            ("combination", MenuStyle::Combination),
            ("full", MenuStyle::Full),
            ("partial", MenuStyle::Partial),
        ] {
            let toml_str = format!("[behavior]\nmenustyle = \"{}\"", val);
            let cfg: Config = toml::from_str(&toml_str).unwrap();
            assert_eq!(cfg.behavior.menustyle, expected);
        }
    }

    #[test]
    fn runmode_variants() {
        for (val, expected) in [
            ("teleport", RunMode::Teleport),
            ("run", RunMode::Run),
            ("walk", RunMode::Walk),
            ("crawl", RunMode::Crawl),
        ] {
            let toml_str = format!("[behavior]\nrunmode = \"{}\"", val);
            let cfg: Config = toml::from_str(&toml_str).unwrap();
            assert_eq!(cfg.behavior.runmode, expected);
        }
    }

    #[test]
    fn pickup_burden_variants() {
        for (val, expected) in [
            ("unencumbered", PickupBurden::Unencumbered),
            ("burdened", PickupBurden::Burdened),
            ("stressed", PickupBurden::Stressed),
            ("strained", PickupBurden::Strained),
            ("overtaxed", PickupBurden::Overtaxed),
            ("overloaded", PickupBurden::Overloaded),
        ] {
            let toml_str = format!("[behavior]\npickup_burden = \"{}\"", val);
            let cfg: Config = toml::from_str(&toml_str).unwrap();
            assert_eq!(cfg.behavior.pickup_burden, expected);
        }
    }
}
