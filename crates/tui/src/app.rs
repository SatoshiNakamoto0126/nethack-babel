//! Main application loop that ties the window port, input mapping, and
//! rendering together.

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hecs::Entity;
use nethack_babel_engine::action::{Direction, NameTarget, PlayerAction, Position, SpellId};

use crate::input::{
    DirectionCommand, ItemCommand, ItemDirectionCommand, PromptKind, complete_extended_command,
    extended_command_needs_prompt, key_needs_prompt, map_direction_key, map_extended_command,
    map_key, regular_commands,
};
use crate::port::{InputEvent, InputKeyCode, MapView, MessageUrgency, StatusLine, WindowPort};

/// A single message in the history, along with its urgency.
#[derive(Debug, Clone)]
pub struct MessageEntry {
    pub text: String,
    pub urgency: MessageUrgency,
}

/// Message log that tracks history and pending display state.
#[derive(Debug, Clone, Default)]
pub struct MessageLog {
    /// All messages accumulated this session (for history).
    entries: Vec<MessageEntry>,
    /// Index into `entries` of the first un-displayed message.
    display_offset: usize,
}

impl MessageLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a message into the log.
    pub fn push(&mut self, text: String, urgency: MessageUrgency) {
        self.entries.push(MessageEntry { text, urgency });
    }

    /// Return messages that have not yet been shown (since last
    /// `mark_displayed`).
    pub fn pending(&self) -> &[MessageEntry] {
        &self.entries[self.display_offset..]
    }

    /// Mark all current messages as displayed.
    pub fn mark_displayed(&mut self) {
        self.display_offset = self.entries.len();
    }

    /// Whether there are unseen messages.
    pub fn has_pending(&self) -> bool {
        self.display_offset < self.entries.len()
    }

    /// Access the full history as strings (for Ctrl+P display).
    pub fn history_strings(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.text.clone()).collect()
    }

    /// Return the last N history entries as strings.
    pub fn recent_history(&self, n: usize) -> Vec<String> {
        let start = self.entries.len().saturating_sub(n);
        self.entries[start..]
            .iter()
            .map(|e| e.text.clone())
            .collect()
    }

    /// Total number of messages stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Pre-translated common UI messages.
///
/// Populated from the locale manager before the game loop starts.
/// The TUI crate does not depend on `i18n`, so these strings are
/// injected by the caller.
#[derive(Debug, Clone)]
pub struct TuiMessages {
    pub command_descriptions: HashMap<String, String>,
    pub empty_handed: String,
    pub never_mind: String,
    pub no_such_item: String,
    pub not_implemented: String,
    pub no_previous_command: String,
    pub commands_title: String,
    pub wizard_mode_disabled: String,
    pub direction_prompt: String,
    pub direction_prompt_optional: String,
    pub direction_prompt_run: String,
    pub direction_prompt_rush: String,
    pub direction_help_title: String,
    pub direction_help_body: String,
    pub item_prompt_drop: String,
    pub item_prompt_wield: String,
    pub item_prompt_wear: String,
    pub item_prompt_take_off: String,
    pub item_prompt_put_on: String,
    pub item_prompt_remove: String,
    pub item_prompt_apply: String,
    pub item_prompt_ready: String,
    pub item_prompt_throw: String,
    pub item_prompt_zap: String,
    pub item_prompt_invoke: String,
    pub item_prompt_rub: String,
    pub item_prompt_tip: String,
    pub item_prompt_offer: String,
    pub item_prompt_force_lock: String,
    pub item_prompt_dip: String,
    pub item_prompt_dip_into: String,
    pub item_prompt_name_item: String,
    pub item_prompt_adjust_item: String,
    pub text_prompt_wish: String,
    pub text_prompt_create_monster: String,
    pub text_prompt_teleport_level: String,
    pub text_prompt_annotate_level: String,
    pub text_prompt_engrave: String,
    pub text_prompt_call_class: String,
    pub text_prompt_call_name: String,
    pub text_prompt_known_class: String,
    pub text_prompt_cast_spell: String,
    pub text_prompt_name_target: String,
    pub text_prompt_name_level: String,
    pub text_prompt_call_monster: String,
    pub text_prompt_name_it: String,
    pub text_prompt_assign_inventory_letter: String,
    pub position_prompt_travel: String,
    pub position_prompt_jump: String,
    pub position_prompt_inspect: String,
    pub position_prompt_look: String,
    pub position_prompt_name_monster: String,
}

impl Default for TuiMessages {
    fn default() -> Self {
        Self {
            command_descriptions: HashMap::new(),
            empty_handed: "You are empty handed.".to_string(),
            never_mind: "Never mind.".to_string(),
            no_such_item: "You don't have that item.".to_string(),
            not_implemented: "Not yet implemented.".to_string(),
            no_previous_command: "No previous command to repeat.".to_string(),
            commands_title: "Commands".to_string(),
            wizard_mode_disabled: "Wizard mode is not enabled.".to_string(),
            direction_prompt: "In what direction?".to_string(),
            direction_prompt_optional: "In what direction? (Esc for none)".to_string(),
            direction_prompt_run: "Run in what direction?".to_string(),
            direction_prompt_rush: "Rush in what direction?".to_string(),
            direction_help_title: "Direction Keys".to_string(),
            direction_help_body: "h left, j down, k up, l right\ny northwest, u northeast, b southwest, n southeast\n. self, < up, > down\nEsc cancel, ? show this help".to_string(),
            item_prompt_drop: "Drop what? [a-zA-Z or ?*]".to_string(),
            item_prompt_wield: "Wield what? [a-zA-Z or - for bare hands]".to_string(),
            item_prompt_wear: "Wear what? [a-zA-Z or ?*]".to_string(),
            item_prompt_take_off: "Take off what? [a-zA-Z or ?*]".to_string(),
            item_prompt_put_on: "Put on what? [a-zA-Z or ?*]".to_string(),
            item_prompt_remove: "Remove what? [a-zA-Z or ?*]".to_string(),
            item_prompt_apply: "Apply what? [a-zA-Z or ?*]".to_string(),
            item_prompt_ready: "Ready what? [a-zA-Z or ?*]".to_string(),
            item_prompt_throw: "Throw what? [a-zA-Z or ?*]".to_string(),
            item_prompt_zap: "Zap what? [a-zA-Z or ?*]".to_string(),
            item_prompt_invoke: "Invoke what? [a-zA-Z or ?*]".to_string(),
            item_prompt_rub: "Rub what? [a-zA-Z or ?*]".to_string(),
            item_prompt_tip: "Tip what? [a-zA-Z or ?*]".to_string(),
            item_prompt_offer: "Offer what? [a-zA-Z or ?*]".to_string(),
            item_prompt_force_lock: "Force lock with what? [a-zA-Z or ?*]".to_string(),
            item_prompt_dip: "Dip what? [a-zA-Z or ?*]".to_string(),
            item_prompt_dip_into: "Dip into what? [a-zA-Z or ?*]".to_string(),
            item_prompt_name_item: "Name which item? [a-zA-Z or ?*]".to_string(),
            item_prompt_adjust_item: "Adjust which item? [a-zA-Z or ?*]".to_string(),
            text_prompt_wish: "Wish for what?".to_string(),
            text_prompt_create_monster: "Create which monster?".to_string(),
            text_prompt_teleport_level: "Teleport to which dungeon level?".to_string(),
            text_prompt_annotate_level: "Annotate this level with:".to_string(),
            text_prompt_engrave: "Engrave what?".to_string(),
            text_prompt_call_class: "Call which class letter?".to_string(),
            text_prompt_call_name: "Call it what?".to_string(),
            text_prompt_known_class: "Known which class letter?".to_string(),
            text_prompt_cast_spell: "Cast which spell letter?".to_string(),
            text_prompt_name_target: "Name target ([i]tem/[m]onster/[l]evel)?".to_string(),
            text_prompt_name_level: "Name this level:".to_string(),
            text_prompt_call_monster: "Call this monster what?".to_string(),
            text_prompt_name_it: "Name it what?".to_string(),
            text_prompt_assign_inventory_letter: "Assign new inventory letter:".to_string(),
            position_prompt_travel: "Travel to where?".to_string(),
            position_prompt_jump: "Jump to where?".to_string(),
            position_prompt_inspect: "Inspect which position?".to_string(),
            position_prompt_look: "Look at which position?".to_string(),
            position_prompt_name_monster: "Name which monster position?".to_string(),
        }
    }
}

/// Top-level application state for the TUI client.
///
/// Owns the cached display state (map, status, messages) that the cli
/// crate populates from the game world.  Coordinates rendering through a
/// [`WindowPort`] backend and translates input into [`PlayerAction`]s.
#[derive(Debug, Clone)]
pub struct App {
    /// Cached map state for rendering.
    pub map: MapView,
    /// Bottom status bar data.
    pub status: StatusLine,
    /// Message history and pending display queue.
    pub messages: MessageLog,
    /// Whether a `--More--` prompt is currently active.
    pub show_more: bool,
    /// Whether the application loop should keep running.
    pub running: bool,
    /// Cursor position on the map (col, row).
    pub cursor: (i16, i16),
    /// Whether the map needs a full redraw on the next render cycle.
    map_needs_redraw: bool,
    /// Accumulated count prefix for repeating actions (e.g. "20s" to
    /// search 20 times).  Typing digits before a command sets this.
    pub count_prefix: Option<u32>,
    /// Mapping from inventory letters to ECS entities.
    ///
    /// The main loop should update this before each call to
    /// [`get_player_action`] so that item-prompting commands can
    /// resolve a letter press directly to the corresponding entity.
    pub inventory_letters: HashMap<char, Entity>,
    /// Pre-translated UI messages (injected from locale manager).
    pub messages_i18n: TuiMessages,
    /// Last non-meta action for repeat (`^A` / `#repeat`).
    last_repeatable_action: Option<PlayerAction>,
    /// Whether wizard/debug commands are enabled.
    wizard_mode: bool,
}

impl App {
    /// Create a new application state with defaults.
    pub fn new() -> Self {
        Self {
            map: MapView::new(),
            status: StatusLine::default(),
            messages: MessageLog::new(),
            show_more: false,
            running: true,
            cursor: (0, 0),
            map_needs_redraw: true,
            count_prefix: None,
            inventory_letters: HashMap::new(),
            messages_i18n: TuiMessages::default(),
            last_repeatable_action: None,
            wizard_mode: false,
        }
    }

    /// Enable/disable wizard command input paths.
    pub fn set_wizard_mode(&mut self, enabled: bool) {
        self.wizard_mode = enabled;
    }

    fn is_repeatable_action(action: &PlayerAction) -> bool {
        !matches!(
            action,
            PlayerAction::Help
                | PlayerAction::ShowHistory
                | PlayerAction::Options
                | PlayerAction::ViewInventory
                | PlayerAction::ViewEquipped
                | PlayerAction::ViewDiscoveries
                | PlayerAction::ViewConduct
                | PlayerAction::DungeonOverview
                | PlayerAction::ViewTerrain
                | PlayerAction::ShowVersion
                | PlayerAction::Attributes
                | PlayerAction::LookAt { .. }
                | PlayerAction::LookHere
                | PlayerAction::Redraw
                | PlayerAction::Quit
                | PlayerAction::Save
                | PlayerAction::SaveAndQuit
        )
    }

    /// Remember the latest action eligible for repeat (`^A` / `#repeat`).
    pub fn remember_repeatable_action(&mut self, action: &PlayerAction) {
        if Self::is_repeatable_action(action) {
            self.last_repeatable_action = Some(action.clone());
        }
    }

    fn repeat_last_action(&self, port: &mut impl WindowPort) -> Option<PlayerAction> {
        if let Some(action) = &self.last_repeatable_action {
            return Some(action.clone());
        }
        port.show_message(
            &self.messages_i18n.no_previous_command,
            MessageUrgency::Normal,
        );
        None
    }

    fn show_request_command_menu(&self, port: &mut impl WindowPort) {
        let mut commands = regular_commands();
        commands.sort_by(|a, b| a.name.cmp(b.name));
        let lines: Vec<String> = commands
            .iter()
            .map(|cmd| {
                if let Some(description) = self.messages_i18n.command_descriptions.get(cmd.name) {
                    format!("#{} - {}", cmd.name, description)
                } else {
                    format!("#{}", cmd.name)
                }
            })
            .collect();
        port.show_text(&self.messages_i18n.commands_title, &lines.join("\n"));
    }

    fn resolve_tui_only_extended_command(
        &self,
        port: &mut impl WindowPort,
        command_name: &str,
    ) -> Option<Option<PlayerAction>> {
        match command_name.trim().to_lowercase().as_str() {
            "repeat" => Some(self.repeat_last_action(port)),
            "reqmenu" => {
                self.show_request_command_menu(port);
                Some(None)
            }
            "herecmdmenu" | "therecmdmenu" => {
                self.show_request_command_menu(port);
                Some(None)
            }
            "perminv" => Some(Some(PlayerAction::ViewInventory)),
            "exploremode" => {
                port.show_message(&self.messages_i18n.not_implemented, MessageUrgency::Normal);
                Some(None)
            }
            "shell" | "suspend" => {
                port.show_message(&self.messages_i18n.not_implemented, MessageUrgency::Normal);
                Some(None)
            }
            "wizidentify" => self.wizard_action(port, PlayerAction::WizIdentify),
            "wizmap" => self.wizard_action(port, PlayerAction::WizMap),
            "wizdetect" => self.wizard_action(port, PlayerAction::WizDetect),
            "wizwhere" => self.wizard_action(port, PlayerAction::WizWhere),
            "wizkill" => self.wizard_action(port, PlayerAction::WizKill),
            "wizwish" => {
                let wish = self
                    .get_localized_line(port, "Wish for what?")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                match wish {
                    Some(wish_text) => {
                        self.wizard_action(port, PlayerAction::WizWish { wish_text })
                    }
                    None => Some(None),
                }
            }
            "wizgenesis" => {
                let monster = self
                    .get_localized_line(port, "Create which monster?")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                match monster {
                    Some(monster_name) => {
                        self.wizard_action(port, PlayerAction::WizGenesis { monster_name })
                    }
                    None => Some(None),
                }
            }
            "wizlevelport" => {
                let depth = self
                    .get_localized_line(port, "Teleport to which dungeon level?")
                    .and_then(|s| s.trim().parse::<i32>().ok());
                match depth {
                    Some(depth) => {
                        self.wizard_action(port, PlayerAction::WizLevelTeleport { depth })
                    }
                    None => Some(None),
                }
            }
            "wizborn" | "wizcast" | "wizcustom" | "wizfliplevel" | "wizintrinsic"
            | "wizloaddes" | "wizloadlua" | "wizmakemap" | "wizrumorcheck" | "wizseenv"
            | "wizsmell" | "wiztelekinesis" | "wmode" | "debugfuzzer" | "levelchange"
            | "lightsources" | "migratemons" | "panic" | "polyself" | "stats" | "timeout"
            | "vision" => self.wizard_unimplemented(port),
            _ => None,
        }
    }

    fn wizard_action(
        &self,
        port: &mut impl WindowPort,
        action: PlayerAction,
    ) -> Option<Option<PlayerAction>> {
        if self.wizard_mode {
            Some(Some(action))
        } else {
            port.show_message(
                &self.messages_i18n.wizard_mode_disabled,
                MessageUrgency::Normal,
            );
            Some(None)
        }
    }

    fn wizard_unimplemented(&self, port: &mut impl WindowPort) -> Option<Option<PlayerAction>> {
        if self.wizard_mode {
            port.show_message(&self.messages_i18n.not_implemented, MessageUrgency::Normal);
        } else {
            port.show_message(
                &self.messages_i18n.wizard_mode_disabled,
                MessageUrgency::Normal,
            );
        }
        Some(None)
    }

    fn resolve_wizard_ctrl_action(
        &self,
        port: &mut impl WindowPort,
        code: InputKeyCode,
        modifiers: crate::port::InputModifiers,
    ) -> Option<Option<PlayerAction>> {
        if !modifiers.ctrl || modifiers.alt {
            return None;
        }

        match code {
            InputKeyCode::Char('e' | 'E') => self.wizard_action(port, PlayerAction::WizDetect),
            InputKeyCode::Char('f' | 'F') => self.wizard_action(port, PlayerAction::WizMap),
            InputKeyCode::Char('g' | 'G') => {
                let monster = self
                    .get_localized_line(port, "Create which monster?")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                match monster {
                    Some(monster_name) => {
                        self.wizard_action(port, PlayerAction::WizGenesis { monster_name })
                    }
                    None => Some(None),
                }
            }
            InputKeyCode::Char('i' | 'I') => self.wizard_action(port, PlayerAction::WizIdentify),
            InputKeyCode::Char('v' | 'V') => {
                let depth = self
                    .get_localized_line(port, "Teleport to which dungeon level?")
                    .and_then(|s| s.trim().parse::<i32>().ok());
                match depth {
                    Some(depth) => {
                        self.wizard_action(port, PlayerAction::WizLevelTeleport { depth })
                    }
                    None => Some(None),
                }
            }
            InputKeyCode::Char('w' | 'W') => {
                let wish = self
                    .get_localized_line(port, "Wish for what?")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                match wish {
                    Some(wish_text) => {
                        self.wizard_action(port, PlayerAction::WizWish { wish_text })
                    }
                    None => Some(None),
                }
            }
            _ => None,
        }
    }

    /// Push new messages into the log.  The next call to
    /// [`display_pending_messages`] will show them through the port.
    pub fn push_messages(&mut self, messages: &[String]) {
        for msg in messages {
            self.messages.push(msg.clone(), MessageUrgency::Normal);
        }
    }

    /// Push a single message with a specific urgency.
    pub fn push_message(&mut self, text: impl Into<String>, urgency: MessageUrgency) {
        self.messages.push(text.into(), urgency);
    }

    /// Display all pending (un-shown) messages through the port, handling
    /// `--More--` when multiple messages are queued.
    ///
    /// Each message is shown individually. If there are more messages
    /// waiting after the current one, a `--More--` prompt is shown and
    /// the method blocks until the player presses a key.
    pub fn display_pending_messages(&mut self, port: &mut impl WindowPort) {
        let pending: Vec<MessageEntry> = self.messages.pending().to_vec();

        for (i, entry) in pending.iter().enumerate() {
            let is_last = i == pending.len() - 1;

            // Show the message.
            port.show_message(&entry.text, entry.urgency);

            // If there are more messages waiting, show --More--.
            if !is_last {
                self.show_more = true;
                let cont = port.show_more_prompt();
                self.show_more = false;
                if !cont {
                    // Player pressed Escape -- skip remaining messages.
                    break;
                }
            }
        }

        self.messages.mark_displayed();
    }

    /// Display one or more messages through the port, handling `--More--`
    /// when the message buffer would overflow the display.
    ///
    /// This is the "push and display immediately" convenience method.
    pub fn display_messages(&mut self, port: &mut impl WindowPort, messages: &[String]) {
        self.push_messages(messages);
        self.display_pending_messages(port);
    }

    /// Handle the '#' extended command: read a command name from the
    /// player, then dispatch it via [`map_extended_command`].
    ///
    /// Shows a "# " prompt and accepts text input with backspace and
    /// tab-completion support.  Returns `None` if the player cancels
    /// (Escape) or enters an unrecognised command.
    pub fn get_extended_command(&mut self, port: &mut impl WindowPort) -> Option<PlayerAction> {
        port.show_message("# ", MessageUrgency::Normal);

        let mut buffer = String::new();
        loop {
            let event = port.get_key();
            match event {
                InputEvent::Key {
                    code: InputKeyCode::Char(c),
                    ..
                } => {
                    buffer.push(c);
                    port.show_message(&format!("# {}", buffer), MessageUrgency::Normal);
                }
                InputEvent::Key {
                    code: InputKeyCode::Enter,
                    ..
                } => {
                    break;
                }
                InputEvent::Key {
                    code: InputKeyCode::Escape,
                    ..
                } => {
                    return None;
                }
                InputEvent::Key {
                    code: InputKeyCode::Backspace,
                    ..
                } => {
                    buffer.pop();
                    port.show_message(&format!("# {}", buffer), MessageUrgency::Normal);
                }
                InputEvent::Key {
                    code: InputKeyCode::Tab,
                    ..
                } => {
                    if let Some(completed) = complete_extended_command(&buffer) {
                        buffer = completed;
                        port.show_message(&format!("# {}", buffer), MessageUrgency::Normal);
                    }
                }
                _ => {}
            }
        }

        if let Some(action) = self.resolve_tui_only_extended_command(port, &buffer) {
            return action;
        }

        if let Some(prompt_kind) = extended_command_needs_prompt(&buffer)
            && let Some(action) = self.resolve_prompt_kind(port, prompt_kind)
        {
            return Some(action);
        }

        if let Some(action) = self.resolve_extended_custom_prompt_action(port, &buffer) {
            return action;
        }

        map_extended_command(&buffer)
    }

    /// Update the inventory letter mapping from a list of
    /// `(entity, letter)` pairs.  The main loop should call this
    /// each time the inventory changes so that item-prompting
    /// commands resolve correctly.
    pub fn update_inventory_letters(&mut self, items: impl IntoIterator<Item = (Entity, char)>) {
        self.inventory_letters.clear();
        for (entity, letter) in items {
            self.inventory_letters.insert(letter, entity);
        }
    }

    /// Prompt the player to select an inventory item by letter.
    ///
    /// Displays `prompt` in the message area and waits for the player
    /// to press a letter key (a-zA-Z), `-` (bare hands/nothing), or
    /// `*` (show full inventory).  Returns the typed character, or
    /// `None` if cancelled with Escape.
    pub fn prompt_inventory_item(&self, port: &mut impl WindowPort, prompt: &str) -> Option<char> {
        port.show_message(self.localize_prompt(prompt), MessageUrgency::Normal);
        loop {
            let event = port.get_key();
            match event {
                InputEvent::Key {
                    code: InputKeyCode::Char(c),
                    ..
                } if c.is_ascii_alphabetic() || c == '-' || c == '*' || c == '?' => {
                    return Some(c);
                }
                InputEvent::Key {
                    code: InputKeyCode::Escape,
                    ..
                } => {
                    return None;
                }
                _ => continue,
            }
        }
    }

    fn localize_prompt<'a>(&'a self, prompt: &'a str) -> &'a str {
        match prompt {
            "Drop what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_drop,
            "Wield what? [a-zA-Z or - for bare hands]" => &self.messages_i18n.item_prompt_wield,
            "Wear what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_wear,
            "Take off what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_take_off,
            "Put on what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_put_on,
            "Remove what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_remove,
            "Apply what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_apply,
            "Ready what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_ready,
            "Throw what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_throw,
            "Zap what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_zap,
            "Invoke what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_invoke,
            "Rub what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_rub,
            "Tip what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_tip,
            "Offer what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_offer,
            "Force lock with what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_force_lock,
            "Dip what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_dip,
            "Dip into what? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_dip_into,
            "Name which item? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_name_item,
            "Adjust which item? [a-zA-Z or ?*]" => &self.messages_i18n.item_prompt_adjust_item,
            "In what direction?" => &self.messages_i18n.direction_prompt,
            "In what direction? (Esc for none)" => &self.messages_i18n.direction_prompt_optional,
            "Run in what direction?" => &self.messages_i18n.direction_prompt_run,
            "Rush in what direction?" => &self.messages_i18n.direction_prompt_rush,
            "Wish for what?" => &self.messages_i18n.text_prompt_wish,
            "Create which monster?" => &self.messages_i18n.text_prompt_create_monster,
            "Teleport to which dungeon level?" => &self.messages_i18n.text_prompt_teleport_level,
            "Travel to where?" => &self.messages_i18n.position_prompt_travel,
            "Jump to where?" => &self.messages_i18n.position_prompt_jump,
            "Inspect which position?" => &self.messages_i18n.position_prompt_inspect,
            "Look at which position?" => &self.messages_i18n.position_prompt_look,
            "Annotate this level with:" => &self.messages_i18n.text_prompt_annotate_level,
            "Engrave what?" => &self.messages_i18n.text_prompt_engrave,
            "Call which class letter?" => &self.messages_i18n.text_prompt_call_class,
            "Call it what?" => &self.messages_i18n.text_prompt_call_name,
            "Known which class letter?" => &self.messages_i18n.text_prompt_known_class,
            "Cast which spell letter?" => &self.messages_i18n.text_prompt_cast_spell,
            "Name target ([i]tem/[m]onster/[l]evel)?" => {
                &self.messages_i18n.text_prompt_name_target
            }
            "Name this level:" => &self.messages_i18n.text_prompt_name_level,
            "Name which monster position?" => &self.messages_i18n.position_prompt_name_monster,
            "Call this monster what?" => &self.messages_i18n.text_prompt_call_monster,
            "Name it what?" => &self.messages_i18n.text_prompt_name_it,
            "Assign new inventory letter:" => {
                &self.messages_i18n.text_prompt_assign_inventory_letter
            }
            _ => prompt,
        }
    }

    fn get_localized_line(&self, port: &mut impl WindowPort, prompt: &str) -> Option<String> {
        port.get_line(self.localize_prompt(prompt))
    }

    fn get_localized_position(&self, port: &mut impl WindowPort, prompt: &str) -> Option<Position> {
        port.ask_position(self.localize_prompt(prompt))
    }

    /// Prompt the player for a direction.
    ///
    /// Displays `prompt` in the message area and waits for a
    /// directional key (vi-keys, arrow keys, `<`, `>`, `.`).
    /// Returns `None` if cancelled with Escape.
    pub fn prompt_direction(&self, port: &mut impl WindowPort, prompt: &str) -> Option<Direction> {
        let prompt = self.localize_prompt(prompt);
        port.show_message(prompt, MessageUrgency::Normal);
        loop {
            let event = port.get_key();
            match event {
                InputEvent::Key { code, modifiers } => {
                    // Convert to crossterm KeyEvent for map_direction_key.
                    let ct_code = match code {
                        InputKeyCode::Char(c) => crossterm::event::KeyCode::Char(c),
                        InputKeyCode::Up => crossterm::event::KeyCode::Up,
                        InputKeyCode::Down => crossterm::event::KeyCode::Down,
                        InputKeyCode::Left => crossterm::event::KeyCode::Left,
                        InputKeyCode::Right => crossterm::event::KeyCode::Right,
                        InputKeyCode::Escape => return None,
                        _ => continue,
                    };
                    if matches!(ct_code, crossterm::event::KeyCode::Char('?')) {
                        port.show_text(
                            &self.messages_i18n.direction_help_title,
                            &self.messages_i18n.direction_help_body,
                        );
                        port.show_message(prompt, MessageUrgency::Normal);
                        continue;
                    }
                    let mut ct_mods = crossterm::event::KeyModifiers::empty();
                    if modifiers.shift {
                        ct_mods |= crossterm::event::KeyModifiers::SHIFT;
                    }
                    let key_event = crossterm::event::KeyEvent::new(ct_code, ct_mods);
                    if let Some(dir) = map_direction_key(key_event) {
                        return Some(dir);
                    }
                    // Not a direction key — ignore and keep waiting.
                }
                _ => continue,
            }
        }
    }

    /// Build a [`PlayerAction`] for an item command given the inventory
    /// letter the player typed.
    ///
    /// Returns `None` if the letter doesn't correspond to any known
    /// inventory item (and the player is warned).
    fn build_item_action(
        &self,
        port: &mut impl WindowPort,
        letter: char,
        command: ItemCommand,
    ) -> Option<PlayerAction> {
        // '-' means bare hands / nothing equipped — only valid for Wield.
        if letter == '-' {
            return match command {
                ItemCommand::Wield => {
                    // Wield bare hands: engine needs *some* Entity.
                    // We signal "empty hands" with a message for now
                    // and skip the action since there's no sentinel entity.
                    port.show_message(&self.messages_i18n.empty_handed, MessageUrgency::Normal);
                    None
                }
                _ => {
                    port.show_message(&self.messages_i18n.never_mind, MessageUrgency::Normal);
                    None
                }
            };
        }

        // '*' or '?' could open an inventory browser in the future.
        if letter == '*' || letter == '?' {
            port.show_message(&self.messages_i18n.not_implemented, MessageUrgency::Normal);
            return None;
        }

        let entity = match self.inventory_letters.get(&letter) {
            Some(&e) => e,
            None => {
                port.show_message(&self.messages_i18n.no_such_item, MessageUrgency::Normal);
                return None;
            }
        };

        Some(match command {
            ItemCommand::Drop => PlayerAction::Drop { item: entity },
            ItemCommand::Wield => PlayerAction::Wield { item: entity },
            ItemCommand::Wear => PlayerAction::Wear { item: entity },
            ItemCommand::TakeOff => PlayerAction::TakeOff { item: entity },
            ItemCommand::PutOn => PlayerAction::PutOn { item: entity },
            ItemCommand::Remove => PlayerAction::Remove { item: entity },
            ItemCommand::Apply => PlayerAction::Apply { item: entity },
            ItemCommand::Quiver => PlayerAction::Apply { item: entity }, // placeholder
            ItemCommand::Invoke => PlayerAction::InvokeArtifact { item: entity },
            ItemCommand::Offer => PlayerAction::Offer { item: Some(entity) },
            ItemCommand::Rub => PlayerAction::Rub { item: entity },
            ItemCommand::Dip => PlayerAction::Apply { item: entity }, // placeholder
            ItemCommand::Tip => PlayerAction::Tip { item: entity },
            ItemCommand::Force => PlayerAction::ForceLock { item: entity },
        })
    }

    /// Build a [`PlayerAction`] for a direction command.
    fn build_direction_action(direction: Direction, command: DirectionCommand) -> PlayerAction {
        match command {
            DirectionCommand::Open => PlayerAction::Open { direction },
            DirectionCommand::Close => PlayerAction::Close { direction },
            DirectionCommand::Fight => PlayerAction::FightDirection { direction },
            DirectionCommand::Kick => PlayerAction::Kick { direction },
            DirectionCommand::Chat => PlayerAction::Chat { direction },
            DirectionCommand::Untrap => PlayerAction::Untrap { direction },
            DirectionCommand::Run => PlayerAction::RunDirection { direction },
            DirectionCommand::Rush => PlayerAction::RushDirection { direction },
        }
    }

    /// Build a [`PlayerAction`] for an item-then-direction command.
    fn build_item_direction_action(
        &self,
        port: &mut impl WindowPort,
        letter: char,
        direction: Direction,
        command: ItemDirectionCommand,
    ) -> Option<PlayerAction> {
        if letter == '*' || letter == '?' {
            port.show_message(&self.messages_i18n.not_implemented, MessageUrgency::Normal);
            return None;
        }

        let entity = match self.inventory_letters.get(&letter) {
            Some(&e) => e,
            None => {
                port.show_message(&self.messages_i18n.no_such_item, MessageUrgency::Normal);
                return None;
            }
        };

        Some(match command {
            ItemDirectionCommand::Throw => PlayerAction::Throw {
                item: entity,
                direction,
            },
            ItemDirectionCommand::ZapWand => PlayerAction::ZapWand {
                item: entity,
                direction: Some(direction),
            },
        })
    }

    /// Resolve a prompt kind into a concrete action using shared prompt logic.
    fn resolve_prompt_kind(
        &self,
        port: &mut impl WindowPort,
        prompt_kind: PromptKind,
    ) -> Option<PlayerAction> {
        match prompt_kind {
            PromptKind::Item { prompt, command } => {
                if let Some(letter) = self.prompt_inventory_item(port, prompt)
                    && let Some(action) = self.build_item_action(port, letter, command)
                {
                    return Some(action);
                }
                None
            }
            PromptKind::Direction { prompt, command } => self
                .prompt_direction(port, prompt)
                .map(|dir| Self::build_direction_action(dir, command)),
            PromptKind::ItemThenDirection {
                item_prompt,
                dir_prompt,
                command,
            } => {
                if let Some(letter) = self.prompt_inventory_item(port, item_prompt)
                    && let Some(dir) = self.prompt_direction(port, dir_prompt)
                    && let Some(action) =
                        self.build_item_direction_action(port, letter, dir, command)
                {
                    return Some(action);
                }
                None
            }
        }
    }

    /// Resolve extended commands that require text/position/custom prompts.
    ///
    /// Returns:
    /// - `Some(Some(action))` when a command is recognized and completed.
    /// - `Some(None)` when recognized but cancelled/invalid input.
    /// - `None` when this helper does not handle the command.
    fn resolve_extended_custom_prompt_action(
        &self,
        port: &mut impl WindowPort,
        command_name: &str,
    ) -> Option<Option<PlayerAction>> {
        let command = command_name.trim().to_lowercase();

        match command.as_str() {
            "travel" | "retravel" => Some(
                self.get_localized_position(port, "Travel to where?")
                    .map(|destination| PlayerAction::Travel { destination }),
            ),
            "jump" => Some(
                self.get_localized_position(port, "Jump to where?")
                    .map(|position| PlayerAction::Jump { position }),
            ),
            "whatis" | "showtrap" => Some(
                self.get_localized_position(port, "Inspect which position?")
                    .map(|position| PlayerAction::WhatIs {
                        position: Some(position),
                    }),
            ),
            "glance" => Some(
                self.get_localized_position(port, "Look at which position?")
                    .map(|position| PlayerAction::LookAt { position }),
            ),
            "annotate" => Some(
                self.get_localized_line(port, "Annotate this level with:")
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                    .map(|text| PlayerAction::Annotate { text }),
            ),
            "engrave" => Some(
                self.get_localized_line(port, "Engrave what?")
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                    .map(|text| PlayerAction::Engrave { text }),
            ),
            "call" => {
                let class = self
                    .get_localized_line(port, "Call which class letter?")
                    .and_then(|s| s.chars().find(|c| c.is_ascii_graphic()));
                let name = self
                    .get_localized_line(port, "Call it what?")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                Some(match (class, name) {
                    (Some(class), Some(name)) => Some(PlayerAction::CallType { class, name }),
                    _ => None,
                })
            }
            "knownclass" => Some(
                self.get_localized_line(port, "Known which class letter?")
                    .and_then(|s| s.chars().find(|c| c.is_ascii_graphic()))
                    .map(|class| PlayerAction::KnownClass { class }),
            ),
            "dip" => {
                let Some(item_letter) =
                    self.prompt_inventory_item(port, "Dip what? [a-zA-Z or ?*]")
                else {
                    return Some(None);
                };
                if item_letter == '*' || item_letter == '?' {
                    port.show_message(&self.messages_i18n.not_implemented, MessageUrgency::Normal);
                    return Some(None);
                }
                let Some(&item) = self.inventory_letters.get(&item_letter) else {
                    port.show_message(&self.messages_i18n.no_such_item, MessageUrgency::Normal);
                    return Some(None);
                };

                let Some(into_letter) =
                    self.prompt_inventory_item(port, "Dip into what? [a-zA-Z or ?*]")
                else {
                    return Some(None);
                };
                if into_letter == '*' || into_letter == '?' {
                    port.show_message(&self.messages_i18n.not_implemented, MessageUrgency::Normal);
                    return Some(None);
                }
                let Some(&into) = self.inventory_letters.get(&into_letter) else {
                    port.show_message(&self.messages_i18n.no_such_item, MessageUrgency::Normal);
                    return Some(None);
                };

                Some(Some(PlayerAction::Dip { item, into }))
            }
            "cast" => {
                let spell = self
                    .get_localized_line(port, "Cast which spell letter?")
                    .and_then(|s| s.chars().find(|c| c.is_ascii_alphabetic()))
                    .map(|c| c.to_ascii_lowercase() as u8 - b'a')
                    .map(SpellId);
                let Some(spell) = spell else {
                    return Some(None);
                };
                let direction = self.prompt_direction(port, "In what direction? (Esc for none)");
                Some(Some(PlayerAction::CastSpell { spell, direction }))
            }
            "name" | "naming" => {
                let target_kind = self
                    .get_localized_line(port, "Name target ([i]tem/[m]onster/[l]evel)?")
                    .and_then(|s| {
                        s.chars()
                            .find(|c| c.is_ascii_alphabetic())
                            .map(|c| c.to_ascii_lowercase())
                    })
                    .unwrap_or('i');

                if target_kind == 'l' {
                    let Some(name) = self
                        .get_localized_line(port, "Name this level:")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                    else {
                        return Some(None);
                    };
                    return Some(Some(PlayerAction::Name {
                        target: NameTarget::Level,
                        name,
                    }));
                }

                if target_kind == 'm' {
                    let Some(position) =
                        self.get_localized_position(port, "Name which monster position?")
                    else {
                        return Some(None);
                    };
                    let Some(name) = self
                        .get_localized_line(port, "Call this monster what?")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                    else {
                        return Some(None);
                    };
                    return Some(Some(PlayerAction::Name {
                        target: NameTarget::MonsterAt { position },
                        name,
                    }));
                }

                let Some(letter) =
                    self.prompt_inventory_item(port, "Name which item? [a-zA-Z or ?*]")
                else {
                    return Some(None);
                };
                let Some(&item) = self.inventory_letters.get(&letter) else {
                    port.show_message(&self.messages_i18n.no_such_item, MessageUrgency::Normal);
                    return Some(None);
                };
                let Some(name) = self
                    .get_localized_line(port, "Name it what?")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                else {
                    return Some(None);
                };
                Some(Some(PlayerAction::Name {
                    target: NameTarget::Item { item },
                    name,
                }))
            }
            "adjust" => {
                let Some(letter) =
                    self.prompt_inventory_item(port, "Adjust which item? [a-zA-Z or ?*]")
                else {
                    return Some(None);
                };
                let Some(&item) = self.inventory_letters.get(&letter) else {
                    port.show_message(&self.messages_i18n.no_such_item, MessageUrgency::Normal);
                    return Some(None);
                };
                let new_letter = self
                    .get_localized_line(port, "Assign new inventory letter:")
                    .and_then(|s| s.chars().find(|c| c.is_ascii_alphabetic()));
                Some(new_letter.map(|new_letter| PlayerAction::Adjust { item, new_letter }))
            }
            _ => None,
        }
    }

    /// Resolve key-driven commands that require custom prompts not covered by
    /// [`PromptKind`] (position selection, free-form text, class letters, etc.).
    fn resolve_key_custom_prompt_action(
        &self,
        port: &mut impl WindowPort,
        key_event: KeyEvent,
    ) -> Option<Option<PlayerAction>> {
        let mods = key_event.modifiers & (KeyModifiers::SHIFT | KeyModifiers::CONTROL);

        match (mods, key_event.code) {
            (KeyModifiers::NONE, KeyCode::Char(';')) => {
                self.resolve_extended_custom_prompt_action(port, "glance")
            }
            (KeyModifiers::NONE, KeyCode::Char('/')) => {
                self.resolve_extended_custom_prompt_action(port, "whatis")
            }
            (KeyModifiers::NONE, KeyCode::Char('^')) => {
                self.resolve_extended_custom_prompt_action(port, "showtrap")
            }
            (KeyModifiers::NONE, KeyCode::Char('`')) => {
                self.resolve_extended_custom_prompt_action(port, "knownclass")
            }
            (KeyModifiers::NONE, KeyCode::Char('E'))
            | (KeyModifiers::SHIFT, KeyCode::Char('E' | 'e')) => {
                self.resolve_extended_custom_prompt_action(port, "engrave")
            }
            (KeyModifiers::NONE, KeyCode::Char('C'))
            | (KeyModifiers::SHIFT, KeyCode::Char('C' | 'c')) => {
                self.resolve_extended_custom_prompt_action(port, "call")
            }
            (KeyModifiers::NONE, KeyCode::Char('Z'))
            | (KeyModifiers::SHIFT, KeyCode::Char('Z' | 'z')) => {
                self.resolve_extended_custom_prompt_action(port, "cast")
            }
            _ => None,
        }
    }

    /// Wait for a key from the player and convert it to a [`PlayerAction`].
    ///
    /// Returns `None` for keys that have no direct mapping (e.g. Escape,
    /// or commands that require further prompting).
    ///
    /// Digits typed before a command accumulate in [`count_prefix`].
    /// The caller should read `count_prefix` after this method returns
    /// to determine how many times to repeat the action.
    pub fn get_player_action(&mut self, port: &mut impl WindowPort) -> Option<PlayerAction> {
        // Reset count prefix at the start of each action request.
        self.count_prefix = None;

        loop {
            let event = port.get_key();
            match event {
                InputEvent::Key { code, modifiers } => {
                    // Accumulate digit count prefix (unmodified digits only).
                    if let InputKeyCode::Char(c @ '0'..='9') = code
                        && !modifiers.shift
                        && !modifiers.ctrl
                        && !modifiers.alt
                    {
                        let digit = c as u32 - '0' as u32;
                        let current = self.count_prefix.unwrap_or(0);
                        self.count_prefix = Some(current.saturating_mul(10).saturating_add(digit));
                        // Show the accumulated count in the message
                        // area so the player has feedback.
                        port.show_message(
                            &format!("Count: {}", self.count_prefix.unwrap_or(0)),
                            MessageUrgency::Normal,
                        );
                        continue;
                    }

                    // Ctrl+A: repeat previous action.
                    if modifiers.ctrl
                        && !modifiers.alt
                        && matches!(code, InputKeyCode::Char('a' | 'A'))
                    {
                        if let Some(action) = self.repeat_last_action(port) {
                            return Some(action);
                        }
                        continue;
                    }

                    // Ctrl+Z: suspend is a system-level command.
                    if modifiers.ctrl
                        && !modifiers.alt
                        && matches!(code, InputKeyCode::Char('z' | 'Z'))
                    {
                        port.show_message(
                            &self.messages_i18n.not_implemented,
                            MessageUrgency::Normal,
                        );
                        continue;
                    }

                    // Shell escape: unsupported in this frontend.
                    if !modifiers.ctrl && !modifiers.alt && matches!(code, InputKeyCode::Char('!'))
                    {
                        port.show_message(
                            &self.messages_i18n.not_implemented,
                            MessageUrgency::Normal,
                        );
                        continue;
                    }

                    // perminv: temporary alias to inventory view.
                    if !modifiers.ctrl && !modifiers.alt && matches!(code, InputKeyCode::Char('|'))
                    {
                        return Some(PlayerAction::ViewInventory);
                    }

                    // reqmenu: show extended-command index.
                    if !modifiers.shift
                        && !modifiers.ctrl
                        && !modifiers.alt
                        && matches!(code, InputKeyCode::Char('m'))
                    {
                        self.show_request_command_menu(port);
                        continue;
                    }

                    // Ctrl+R: redraw request.
                    if modifiers.ctrl
                        && !modifiers.alt
                        && matches!(code, InputKeyCode::Char('r' | 'R'))
                    {
                        return Some(PlayerAction::Redraw);
                    }

                    if let Some(action) =
                        self.resolve_wizard_ctrl_action(port, code.clone(), modifiers)
                    {
                        if let Some(action) = action {
                            return Some(action);
                        }
                        continue;
                    }

                    // Handle '#' extended command prefix specially:
                    // instead of returning None, prompt for the full
                    // command name.
                    if code == InputKeyCode::Char('#')
                        && !modifiers.shift
                        && !modifiers.ctrl
                        && !modifiers.alt
                    {
                        return self.get_extended_command(port);
                    }

                    // Convert our InputEvent back to a crossterm KeyEvent
                    // for the map_key function.
                    let ct_code = match code {
                        InputKeyCode::Char(c) => crossterm::event::KeyCode::Char(c),
                        InputKeyCode::Enter => crossterm::event::KeyCode::Enter,
                        InputKeyCode::Escape => return None,
                        InputKeyCode::Backspace => crossterm::event::KeyCode::Backspace,
                        InputKeyCode::Tab => crossterm::event::KeyCode::Tab,
                        InputKeyCode::Up => crossterm::event::KeyCode::Up,
                        InputKeyCode::Down => crossterm::event::KeyCode::Down,
                        InputKeyCode::Left => crossterm::event::KeyCode::Left,
                        InputKeyCode::Right => crossterm::event::KeyCode::Right,
                        InputKeyCode::Home => crossterm::event::KeyCode::Home,
                        InputKeyCode::End => crossterm::event::KeyCode::End,
                        InputKeyCode::PageUp => crossterm::event::KeyCode::PageUp,
                        InputKeyCode::PageDown => crossterm::event::KeyCode::PageDown,
                        InputKeyCode::Delete => crossterm::event::KeyCode::Delete,
                        InputKeyCode::Insert => crossterm::event::KeyCode::Insert,
                        InputKeyCode::F(n) => crossterm::event::KeyCode::F(n),
                    };
                    let mut ct_mods = crossterm::event::KeyModifiers::empty();
                    if modifiers.shift {
                        ct_mods |= crossterm::event::KeyModifiers::SHIFT;
                    }
                    if modifiers.ctrl {
                        ct_mods |= crossterm::event::KeyModifiers::CONTROL;
                    }
                    if modifiers.alt {
                        ct_mods |= crossterm::event::KeyModifiers::ALT;
                    }
                    let key_event = crossterm::event::KeyEvent::new(ct_code, ct_mods);

                    // Check if this key needs a follow-up prompt
                    // (item selection, direction, or both).
                    if let Some(prompt_kind) = key_needs_prompt(key_event) {
                        if let Some(action) = self.resolve_prompt_kind(port, prompt_kind) {
                            return Some(action);
                        }
                        // Cancelled or invalid — loop again.
                        continue;
                    }

                    if let Some(action) = self.resolve_key_custom_prompt_action(port, key_event) {
                        if let Some(action) = action {
                            return Some(action);
                        }
                        // Cancelled or invalid — loop again.
                        continue;
                    }

                    if let Some(action) = map_key(key_event) {
                        return Some(action);
                    }
                    // Unmapped key -- continue waiting.
                }
                InputEvent::Resize { .. } => {
                    // The port should handle resize internally; we just
                    // continue waiting for a real input.
                }
                InputEvent::Mouse { .. } | InputEvent::None => {
                    // Ignore for now.
                }
            }
        }
    }

    /// Perform a full render cycle: map, status bar, and the latest
    /// message.
    pub fn render(&mut self, port: &mut impl WindowPort) {
        port.render_map(&self.map, self.cursor);
        port.render_status(&self.status);

        // Show the most recent message if any are unseen.
        if self.messages.has_pending() {
            let pending = self.messages.pending();
            if let Some(entry) = pending.last() {
                port.show_message(&entry.text, entry.urgency);
            }
        }
    }

    /// Perform a full render cycle using externally-provided map and
    /// status data (does not use the cached fields).
    pub fn render_with(
        &self,
        port: &mut impl WindowPort,
        map: &MapView,
        status: &StatusLine,
        cursor: (i16, i16),
    ) {
        port.render_map(map, cursor);
        port.render_status(status);

        // Show the most recent message if any are unseen.
        if self.messages.has_pending() {
            let pending = self.messages.pending();
            if let Some(entry) = pending.last() {
                port.show_message(&entry.text, entry.urgency);
            }
        }
    }

    /// Access the full message history as strings.
    pub fn message_history(&self) -> Vec<String> {
        self.messages.history_strings()
    }

    /// Show the message history through the port (for Ctrl+P).
    pub fn show_history(&self, port: &mut impl WindowPort) {
        let history = self.messages.recent_history(20);
        port.show_message_history(&history);
    }

    /// Mark the map as needing a full redraw.
    pub fn invalidate_map(&mut self) {
        self.map_needs_redraw = true;
    }

    /// Check and clear the redraw flag.
    pub fn take_redraw_flag(&mut self) -> bool {
        let flag = self.map_needs_redraw;
        self.map_needs_redraw = false;
        flag
    }

    /// Signal the application to stop running.
    pub fn quit(&mut self) {
        self.running = false;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::{InputEvent, InputKeyCode, InputModifiers, Menu, MenuResult};
    use nethack_babel_engine::action::Position;
    use std::collections::VecDeque;

    // ── Mock WindowPort for testing ────────────────────────────────

    /// A test-only [`WindowPort`] that feeds scripted key events and
    /// records messages shown to the player.
    struct MockPort {
        /// Scripted key events returned by `get_key()`.
        keys: VecDeque<InputEvent>,
        /// Messages shown via `show_message()`.
        messages: Vec<String>,
        /// Scripted responses for `ask_position()`.
        positions: VecDeque<Option<Position>>,
        /// Scripted responses for `get_line()`.
        lines: VecDeque<Option<String>>,
        /// Captured text windows (`show_text`).
        shown_text: Vec<(String, String)>,
    }

    impl MockPort {
        fn new(keys: Vec<InputEvent>) -> Self {
            Self {
                keys: VecDeque::from(keys),
                messages: Vec::new(),
                positions: VecDeque::new(),
                lines: VecDeque::new(),
                shown_text: Vec::new(),
            }
        }

        fn key(c: char) -> InputEvent {
            InputEvent::Key {
                code: InputKeyCode::Char(c),
                modifiers: InputModifiers::NONE,
            }
        }

        fn escape() -> InputEvent {
            InputEvent::Key {
                code: InputKeyCode::Escape,
                modifiers: InputModifiers::NONE,
            }
        }
    }

    impl WindowPort for MockPort {
        fn init(&mut self) {}
        fn shutdown(&mut self) {}
        fn render_map(&mut self, _map: &MapView, _cursor: (i16, i16)) {}
        fn render_status(&mut self, _status: &StatusLine) {}
        fn show_message(&mut self, msg: &str, _urgency: MessageUrgency) {
            self.messages.push(msg.to_string());
        }
        fn show_more_prompt(&mut self) -> bool {
            true
        }
        fn show_message_history(&mut self, _messages: &[String]) {}
        fn show_menu(&mut self, _menu: &Menu) -> MenuResult {
            MenuResult::Cancelled
        }
        fn show_text(&mut self, title: &str, content: &str) {
            self.shown_text
                .push((title.to_string(), content.to_string()));
        }
        fn get_key(&mut self) -> InputEvent {
            self.keys.pop_front().unwrap_or(InputEvent::None)
        }
        fn ask_direction(&mut self, _prompt: &str) -> Option<Direction> {
            None
        }
        fn ask_position(&mut self, _prompt: &str) -> Option<Position> {
            self.positions.pop_front().flatten()
        }
        fn ask_yn(&mut self, _prompt: &str, _choices: &str, default: char) -> char {
            default
        }
        fn get_line(&mut self, _prompt: &str) -> Option<String> {
            self.lines.pop_front().flatten()
        }
        fn render_tombstone(&mut self, _epitaph: &str, _death_info: &str) {}
        fn delay(&mut self, _ms: u32) {}
        fn bell(&mut self) {}
    }

    // ── Existing tests ─────────────────────────────────────────────

    #[test]
    fn count_prefix_initial_state() {
        let app = App::new();
        assert!(
            app.count_prefix.is_none(),
            "count_prefix should start as None"
        );
    }

    #[test]
    fn count_prefix_can_be_set_and_read() {
        let mut app = App::new();
        // Simulate accumulating digits "2", "0" -> 20.
        app.count_prefix = Some(2);
        app.count_prefix = Some(app.count_prefix.unwrap_or(0).saturating_mul(10));
        assert_eq!(app.count_prefix, Some(20));
    }

    #[test]
    fn count_prefix_saturating_overflow() {
        let mut app = App::new();
        // Simulate typing many '9's — should saturate, not panic.
        app.count_prefix = Some(0);
        for _ in 0..15 {
            let current = app.count_prefix.unwrap_or(0);
            app.count_prefix = Some(current.saturating_mul(10).saturating_add(9));
        }
        // Should be u32::MAX after saturation.
        assert_eq!(app.count_prefix, Some(u32::MAX));
    }

    // ── prompt_inventory_item tests ────────────────────────────────

    #[test]
    fn prompt_inventory_item_returns_letter() {
        let app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('a')]);
        let result = app.prompt_inventory_item(&mut port, "Wield what?");
        assert_eq!(result, Some('a'));
    }

    #[test]
    fn prompt_inventory_item_returns_uppercase_letter() {
        let app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('Z')]);
        let result = app.prompt_inventory_item(&mut port, "Drop what?");
        assert_eq!(result, Some('Z'));
    }

    #[test]
    fn prompt_inventory_item_returns_dash() {
        let app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('-')]);
        let result = app.prompt_inventory_item(&mut port, "Wield what?");
        assert_eq!(result, Some('-'));
    }

    #[test]
    fn prompt_inventory_item_returns_star() {
        let app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('*')]);
        let result = app.prompt_inventory_item(&mut port, "Drop what?");
        assert_eq!(result, Some('*'));
    }

    #[test]
    fn prompt_inventory_item_returns_question_mark() {
        let app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('?')]);
        let result = app.prompt_inventory_item(&mut port, "Apply what?");
        assert_eq!(result, Some('?'));
    }

    #[test]
    fn prompt_inventory_item_escape_cancels() {
        let app = App::new();
        let mut port = MockPort::new(vec![MockPort::escape()]);
        let result = app.prompt_inventory_item(&mut port, "Wield what?");
        assert_eq!(result, None);
    }

    #[test]
    fn prompt_inventory_item_ignores_non_letter_keys() {
        let app = App::new();
        // First send a digit (ignored), then a valid letter.
        let mut port = MockPort::new(vec![
            MockPort::key('5'),
            MockPort::key('.'),
            MockPort::key('c'),
        ]);
        let result = app.prompt_inventory_item(&mut port, "Wield what?");
        assert_eq!(result, Some('c'));
    }

    #[test]
    fn prompt_inventory_item_shows_prompt() {
        let mut app = App::new();
        app.messages_i18n.item_prompt_wear = "穿戴哪一件？".to_string();
        let mut port = MockPort::new(vec![MockPort::key('b')]);
        app.prompt_inventory_item(&mut port, "Wear what? [a-zA-Z or ?*]");
        assert!(port.messages.contains(&"穿戴哪一件？".to_string()));
    }

    // ── prompt_direction tests ─────────────────────────────────────

    #[test]
    fn prompt_direction_returns_vi_key() {
        let app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('h')]);
        let result = app.prompt_direction(&mut port, "In what direction?");
        assert_eq!(result, Some(Direction::West));
    }

    #[test]
    fn prompt_direction_all_vi_keys() {
        let cases = [
            ('h', Direction::West),
            ('j', Direction::South),
            ('k', Direction::North),
            ('l', Direction::East),
            ('y', Direction::NorthWest),
            ('u', Direction::NorthEast),
            ('b', Direction::SouthWest),
            ('n', Direction::SouthEast),
            ('.', Direction::Self_),
            ('<', Direction::Up),
            ('>', Direction::Down),
        ];
        for (key, expected_dir) in cases {
            let app = App::new();
            let mut port = MockPort::new(vec![MockPort::key(key)]);
            let result = app.prompt_direction(&mut port, "Direction?");
            assert_eq!(
                result,
                Some(expected_dir),
                "key '{}' should map to {:?}",
                key,
                expected_dir
            );
        }
    }

    #[test]
    fn prompt_direction_escape_cancels() {
        let app = App::new();
        let mut port = MockPort::new(vec![MockPort::escape()]);
        let result = app.prompt_direction(&mut port, "In what direction?");
        assert_eq!(result, None);
    }

    #[test]
    fn prompt_direction_ignores_invalid_then_accepts_valid() {
        let app = App::new();
        // 'x' is not a direction key; 'j' is south.
        let mut port = MockPort::new(vec![MockPort::key('x'), MockPort::key('j')]);
        let result = app.prompt_direction(&mut port, "Direction?");
        assert_eq!(result, Some(Direction::South));
    }

    #[test]
    fn prompt_direction_question_mark_shows_help_then_accepts_valid() {
        let app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('?'), MockPort::key('l')]);
        let result = app.prompt_direction(&mut port, "In what direction?");
        assert_eq!(result, Some(Direction::East));
        assert_eq!(port.shown_text.len(), 1, "help text should be shown once");
        assert_eq!(port.shown_text[0].0, app.messages_i18n.direction_help_title);
        assert_eq!(port.shown_text[0].1, app.messages_i18n.direction_help_body);
        assert_eq!(
            port.messages,
            vec![
                app.messages_i18n.direction_prompt.clone(),
                app.messages_i18n.direction_prompt.clone()
            ]
        );
    }

    #[test]
    fn prompt_direction_localizes_common_prompt_text() {
        let mut app = App::new();
        app.messages_i18n.direction_prompt = "往哪个方向？".to_string();
        let mut port = MockPort::new(vec![MockPort::key('h')]);
        let result = app.prompt_direction(&mut port, "In what direction?");
        assert_eq!(result, Some(Direction::West));
        assert_eq!(port.messages, vec!["往哪个方向？".to_string()]);
    }

    #[test]
    fn prompt_direction_arrow_keys() {
        let app = App::new();
        let mut port = MockPort::new(vec![InputEvent::Key {
            code: InputKeyCode::Up,
            modifiers: InputModifiers::NONE,
        }]);
        let result = app.prompt_direction(&mut port, "Direction?");
        assert_eq!(result, Some(Direction::North));
    }

    // ── build_item_action tests ────────────────────────────────────

    #[test]
    fn build_item_action_valid_letter() {
        let mut app = App::new();
        // Create a dummy entity via hecs World.
        let mut ecs = hecs::World::new();
        let entity = ecs.spawn((42u32,));
        app.inventory_letters.insert('a', entity);

        let mut port = MockPort::new(vec![]);
        let result = app.build_item_action(&mut port, 'a', ItemCommand::Wield);
        assert!(matches!(
            result,
            Some(PlayerAction::Wield { item }) if item == entity
        ));
    }

    #[test]
    fn build_item_action_unknown_letter() {
        let app = App::new();
        let mut port = MockPort::new(vec![]);
        let result = app.build_item_action(&mut port, 'z', ItemCommand::Drop);
        assert!(result.is_none());
        assert!(port.messages.iter().any(|m| m.contains("don't have")));
    }

    #[test]
    fn build_item_action_dash_wield_is_none() {
        let app = App::new();
        let mut port = MockPort::new(vec![]);
        let result = app.build_item_action(&mut port, '-', ItemCommand::Wield);
        assert!(result.is_none());
        assert!(port.messages.iter().any(|m| m.contains("empty handed")));
    }

    #[test]
    fn build_item_action_dash_non_wield_is_none() {
        let app = App::new();
        let mut port = MockPort::new(vec![]);
        let result = app.build_item_action(&mut port, '-', ItemCommand::Drop);
        assert!(result.is_none());
        assert!(port.messages.iter().any(|m| m.contains("Never mind")));
    }

    #[test]
    fn build_item_action_star_is_none() {
        let app = App::new();
        let mut port = MockPort::new(vec![]);
        let result = app.build_item_action(&mut port, '*', ItemCommand::Wear);
        assert!(result.is_none());
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn build_item_action_all_commands() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let entity = ecs.spawn((1u32,));
        app.inventory_letters.insert('x', entity);

        let commands_and_expected: Vec<(ItemCommand, fn(Entity) -> PlayerAction)> = vec![
            (ItemCommand::Drop, |e| PlayerAction::Drop { item: e }),
            (ItemCommand::Wield, |e| PlayerAction::Wield { item: e }),
            (ItemCommand::Wear, |e| PlayerAction::Wear { item: e }),
            (ItemCommand::TakeOff, |e| PlayerAction::TakeOff { item: e }),
            (ItemCommand::PutOn, |e| PlayerAction::PutOn { item: e }),
            (ItemCommand::Remove, |e| PlayerAction::Remove { item: e }),
            (ItemCommand::Apply, |e| PlayerAction::Apply { item: e }),
            (ItemCommand::Force, |e| PlayerAction::ForceLock { item: e }),
        ];

        for (command, _make_expected) in &commands_and_expected {
            let mut port = MockPort::new(vec![]);
            let result = app.build_item_action(&mut port, 'x', *command);
            assert!(
                result.is_some(),
                "command {:?} should produce an action",
                command
            );
        }
    }

    // ── build_direction_action tests ───────────────────────────────

    #[test]
    fn build_direction_action_open() {
        let action = App::build_direction_action(Direction::North, DirectionCommand::Open);
        assert!(matches!(
            action,
            PlayerAction::Open {
                direction: Direction::North
            }
        ));
    }

    #[test]
    fn build_direction_action_close() {
        let action = App::build_direction_action(Direction::East, DirectionCommand::Close);
        assert!(matches!(
            action,
            PlayerAction::Close {
                direction: Direction::East
            }
        ));
    }

    #[test]
    fn build_direction_action_fight() {
        let action = App::build_direction_action(Direction::SouthWest, DirectionCommand::Fight);
        assert!(matches!(
            action,
            PlayerAction::FightDirection {
                direction: Direction::SouthWest
            }
        ));
    }

    #[test]
    fn build_direction_action_untrap() {
        let action = App::build_direction_action(Direction::West, DirectionCommand::Untrap);
        assert!(matches!(
            action,
            PlayerAction::Untrap {
                direction: Direction::West
            }
        ));
    }

    #[test]
    fn build_direction_action_run_and_rush() {
        let run = App::build_direction_action(Direction::NorthEast, DirectionCommand::Run);
        let rush = App::build_direction_action(Direction::South, DirectionCommand::Rush);
        assert!(matches!(
            run,
            PlayerAction::RunDirection {
                direction: Direction::NorthEast
            }
        ));
        assert!(matches!(
            rush,
            PlayerAction::RushDirection {
                direction: Direction::South
            }
        ));
    }

    // ── build_item_direction_action tests ──────────────────────────

    #[test]
    fn build_item_direction_action_throw() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let entity = ecs.spawn((1u32,));
        app.inventory_letters.insert('d', entity);

        let mut port = MockPort::new(vec![]);
        let result = app.build_item_direction_action(
            &mut port,
            'd',
            Direction::North,
            ItemDirectionCommand::Throw,
        );
        assert!(matches!(
            result,
            Some(PlayerAction::Throw {
                item,
                direction: Direction::North
            }) if item == entity
        ));
    }

    #[test]
    fn build_item_direction_action_zap() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let entity = ecs.spawn((1u32,));
        app.inventory_letters.insert('e', entity);

        let mut port = MockPort::new(vec![]);
        let result = app.build_item_direction_action(
            &mut port,
            'e',
            Direction::South,
            ItemDirectionCommand::ZapWand,
        );
        assert!(matches!(
            result,
            Some(PlayerAction::ZapWand {
                item,
                direction: Some(Direction::South)
            }) if item == entity
        ));
    }

    #[test]
    fn build_item_direction_action_unknown_letter() {
        let app = App::new();
        let mut port = MockPort::new(vec![]);
        let result = app.build_item_direction_action(
            &mut port,
            'z',
            Direction::East,
            ItemDirectionCommand::Throw,
        );
        assert!(result.is_none());
    }

    // ── update_inventory_letters tests ─────────────────────────────

    #[test]
    fn update_inventory_letters_replaces_map() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let e1 = ecs.spawn((1u32,));
        let e2 = ecs.spawn((2u32,));

        app.update_inventory_letters(vec![(e1, 'a'), (e2, 'b')]);
        assert_eq!(app.inventory_letters.len(), 2);
        assert_eq!(app.inventory_letters.get(&'a'), Some(&e1));
        assert_eq!(app.inventory_letters.get(&'b'), Some(&e2));

        // Second update should replace.
        let e3 = ecs.spawn((3u32,));
        app.update_inventory_letters(vec![(e3, 'c')]);
        assert_eq!(app.inventory_letters.len(), 1);
        assert_eq!(app.inventory_letters.get(&'c'), Some(&e3));
        assert!(!app.inventory_letters.contains_key(&'a'));
    }

    #[test]
    fn get_extended_command_open_direction_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('o'),
            MockPort::key('p'),
            MockPort::key('e'),
            MockPort::key('n'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
            MockPort::key('h'),
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Open {
                direction: Direction::West
            })
        ));
    }

    #[test]
    fn get_extended_command_throw_item_then_direction_flow() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let dagger = ecs.spawn((1u32,));
        app.inventory_letters.insert('a', dagger);
        let mut port = MockPort::new(vec![
            MockPort::key('t'),
            MockPort::key('h'),
            MockPort::key('r'),
            MockPort::key('o'),
            MockPort::key('w'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
            MockPort::key('a'),
            MockPort::key('l'),
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Throw {
                item,
                direction: Direction::East
            }) if item == dagger
        ));
    }

    #[test]
    fn get_extended_command_travel_position_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('t'),
            MockPort::key('r'),
            MockPort::key('a'),
            MockPort::key('v'),
            MockPort::key('e'),
            MockPort::key('l'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        let target = Position::new(12, 34);
        port.positions.push_back(Some(target));
        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Travel { destination }) if destination == target
        ));
    }

    #[test]
    fn get_extended_command_annotate_text_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('a'),
            MockPort::key('n'),
            MockPort::key('n'),
            MockPort::key('o'),
            MockPort::key('t'),
            MockPort::key('a'),
            MockPort::key('t'),
            MockPort::key('e'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        port.lines.push_back(Some("  mine level  ".to_string()));
        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Annotate { text }) if text == "mine level"
        ));
    }

    #[test]
    fn get_extended_command_knownclass_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('k'),
            MockPort::key('n'),
            MockPort::key('o'),
            MockPort::key('w'),
            MockPort::key('n'),
            MockPort::key('c'),
            MockPort::key('l'),
            MockPort::key('a'),
            MockPort::key('s'),
            MockPort::key('s'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        port.lines.push_back(Some("!".to_string()));
        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::KnownClass { class: '!' })
        ));
    }

    #[test]
    fn get_extended_command_adjust_flow() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let item = ecs.spawn((1u32,));
        app.inventory_letters.insert('a', item);
        let mut port = MockPort::new(vec![
            MockPort::key('a'),
            MockPort::key('d'),
            MockPort::key('j'),
            MockPort::key('u'),
            MockPort::key('s'),
            MockPort::key('t'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
            MockPort::key('a'),
        ]);
        port.lines.push_back(Some("b".to_string()));
        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Adjust {
                item: selected,
                new_letter: 'b'
            }) if selected == item
        ));
    }

    #[test]
    fn get_extended_command_name_flow() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let item = ecs.spawn((1u32,));
        app.inventory_letters.insert('a', item);
        let mut port = MockPort::new(vec![
            MockPort::key('n'),
            MockPort::key('a'),
            MockPort::key('m'),
            MockPort::key('e'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
            MockPort::key('a'),
        ]);
        port.lines.push_back(Some("i".to_string()));
        port.lines.push_back(Some("Excalibur".to_string()));
        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Name {
                target: NameTarget::Item { item: selected },
                name
            }) if selected == item && name == "Excalibur"
        ));
    }

    #[test]
    fn get_extended_command_name_level_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('n'),
            MockPort::key('a'),
            MockPort::key('m'),
            MockPort::key('e'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        port.lines.push_back(Some("l".to_string()));
        port.lines.push_back(Some("Mines Entry".to_string()));

        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Name {
                target: NameTarget::Level,
                name
            }) if name == "Mines Entry"
        ));
    }

    #[test]
    fn get_extended_command_name_monster_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('n'),
            MockPort::key('a'),
            MockPort::key('m'),
            MockPort::key('e'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        port.lines.push_back(Some("m".to_string()));
        port.lines.push_back(Some("Fluffy".to_string()));
        port.positions.push_back(Some(Position::new(12, 7)));

        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Name {
                target: NameTarget::MonsterAt { position },
                name
            }) if position == Position::new(12, 7) && name == "Fluffy"
        ));
    }

    #[test]
    fn get_extended_command_dip_two_items_flow() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let item_a = ecs.spawn((1u32,));
        let item_b = ecs.spawn((2u32,));
        app.inventory_letters.insert('a', item_a);
        app.inventory_letters.insert('b', item_b);
        let mut port = MockPort::new(vec![
            MockPort::key('d'),
            MockPort::key('i'),
            MockPort::key('p'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
            MockPort::key('a'),
            MockPort::key('b'),
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Dip { item, into }) if item == item_a && into == item_b
        ));
    }

    #[test]
    fn get_extended_command_cast_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('c'),
            MockPort::key('a'),
            MockPort::key('s'),
            MockPort::key('t'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
            MockPort::key('l'),
        ]);
        port.lines.push_back(Some("c".to_string()));
        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::CastSpell {
                spell: SpellId(2),
                direction: Some(Direction::East)
            })
        ));
    }

    #[test]
    fn get_player_action_hash_extended_open_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('#'),
            MockPort::key('o'),
            MockPort::key('p'),
            MockPort::key('e'),
            MockPort::key('n'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
            MockPort::key('j'),
        ]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Open {
                direction: Direction::South
            })
        ));
    }

    #[test]
    fn get_player_action_semicolon_glance_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![MockPort::key(';')]);
        let target = Position::new(7, 8);
        port.positions.push_back(Some(target));
        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::LookAt { position }) if position == target
        ));
    }

    #[test]
    fn get_player_action_backtick_knownclass_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('`')]);
        port.lines.push_back(Some("?".to_string()));
        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::KnownClass { class: '?' })
        ));
    }

    #[test]
    fn get_player_action_uppercase_e_engrave_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('E')]);
        port.lines.push_back(Some("Elbereth".to_string()));
        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Engrave { text }) if text == "Elbereth"
        ));
    }

    #[test]
    fn get_player_action_uppercase_z_cast_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            InputEvent::Key {
                code: InputKeyCode::Char('Z'),
                modifiers: InputModifiers::SHIFT,
            },
            MockPort::key('h'),
        ]);
        port.lines.push_back(Some("a".to_string()));
        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::CastSpell {
                spell: SpellId(0),
                direction: Some(Direction::West)
            })
        ));
    }

    // ── Full flow integration tests (get_player_action) ────────────

    #[test]
    fn get_player_action_wield_full_flow() {
        // Simulate: press 'w', then 'a' for the item letter.
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let sword = ecs.spawn((1u32,));
        app.inventory_letters.insert('a', sword);

        let mut port = MockPort::new(vec![
            MockPort::key('w'), // triggers wield prompt
            MockPort::key('a'), // selects item 'a'
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Wield { item }) if item == sword
        ));
    }

    #[test]
    fn get_player_action_drop_full_flow() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let item = ecs.spawn((1u32,));
        app.inventory_letters.insert('b', item);

        let mut port = MockPort::new(vec![MockPort::key('d'), MockPort::key('b')]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Drop { item: e }) if e == item
        ));
    }

    #[test]
    fn get_player_action_open_direction_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('o'), // open door
            MockPort::key('l'), // east direction
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Open {
                direction: Direction::East
            })
        ));
    }

    #[test]
    fn get_player_action_close_direction_flow() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('c'),
            MockPort::key('k'), // north
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Close {
                direction: Direction::North
            })
        ));
    }

    #[test]
    fn get_player_action_throw_item_then_direction() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let dagger = ecs.spawn((1u32,));
        app.inventory_letters.insert('c', dagger);

        let mut port = MockPort::new(vec![
            MockPort::key('t'), // throw
            MockPort::key('c'), // item letter
            MockPort::key('k'), // north
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Throw {
                item,
                direction: Direction::North
            }) if item == dagger
        ));
    }

    #[test]
    fn get_player_action_zap_item_then_direction() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let wand = ecs.spawn((1u32,));
        app.inventory_letters.insert('f', wand);

        let mut port = MockPort::new(vec![
            MockPort::key('z'), // zap
            MockPort::key('f'), // wand letter
            MockPort::key('j'), // south
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::ZapWand {
                item,
                direction: Some(Direction::South)
            }) if item == wand
        ));
    }

    #[test]
    fn get_player_action_wield_escape_cancels() {
        // Press 'w' then Escape -- should loop back and wait.
        // Then press '.' for Rest to terminate.
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('w'),
            MockPort::escape(),
            MockPort::key('.'), // rest
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::Rest)));
    }

    #[test]
    fn get_player_action_throw_escape_item_cancels() {
        // Press 't' then Escape during item prompt.
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('t'),
            MockPort::escape(),
            MockPort::key('.'),
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::Rest)));
    }

    #[test]
    fn get_player_action_throw_escape_direction_cancels() {
        // Press 't', valid item letter, then Escape during direction.
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let item = ecs.spawn((1u32,));
        app.inventory_letters.insert('a', item);

        let mut port = MockPort::new(vec![
            MockPort::key('t'),
            MockPort::key('a'),
            MockPort::escape(),
            MockPort::key('.'),
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::Rest)));
    }

    #[test]
    fn get_player_action_wear_full_flow() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let armor = ecs.spawn((1u32,));
        app.inventory_letters.insert('b', armor);

        let mut port = MockPort::new(vec![
            InputEvent::Key {
                code: InputKeyCode::Char('W'),
                modifiers: InputModifiers::SHIFT,
            },
            MockPort::key('b'),
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Wear { item }) if item == armor
        ));
    }

    #[test]
    fn get_player_action_takeoff_full_flow() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let armor = ecs.spawn((1u32,));
        app.inventory_letters.insert('c', armor);

        let mut port = MockPort::new(vec![
            InputEvent::Key {
                code: InputKeyCode::Char('T'),
                modifiers: InputModifiers::SHIFT,
            },
            MockPort::key('c'),
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::TakeOff { item }) if item == armor
        ));
    }

    #[test]
    fn get_player_action_puton_full_flow() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let ring = ecs.spawn((1u32,));
        app.inventory_letters.insert('d', ring);

        let mut port = MockPort::new(vec![
            InputEvent::Key {
                code: InputKeyCode::Char('P'),
                modifiers: InputModifiers::SHIFT,
            },
            MockPort::key('d'),
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::PutOn { item }) if item == ring
        ));
    }

    #[test]
    fn get_player_action_remove_full_flow() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let ring = ecs.spawn((1u32,));
        app.inventory_letters.insert('e', ring);

        let mut port = MockPort::new(vec![
            InputEvent::Key {
                code: InputKeyCode::Char('R'),
                modifiers: InputModifiers::SHIFT,
            },
            MockPort::key('e'),
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Remove { item }) if item == ring
        ));
    }

    #[test]
    fn get_player_action_apply_full_flow() {
        let mut app = App::new();
        let mut ecs = hecs::World::new();
        let tool = ecs.spawn((1u32,));
        app.inventory_letters.insert('g', tool);

        let mut port = MockPort::new(vec![MockPort::key('a'), MockPort::key('g')]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::Apply { item }) if item == tool
        ));
    }

    #[test]
    fn get_player_action_fight_direction() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            InputEvent::Key {
                code: InputKeyCode::Char('F'),
                modifiers: InputModifiers::SHIFT,
            },
            MockPort::key('h'), // west
        ]);

        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::FightDirection {
                direction: Direction::West
            })
        ));
    }

    #[test]
    fn get_player_action_g_rush_direction() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('g'), MockPort::key('n')]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::RushDirection {
                direction: Direction::SouthEast
            })
        ));
    }

    #[test]
    fn get_player_action_shift_g_run_direction() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            InputEvent::Key {
                code: InputKeyCode::Char('G'),
                modifiers: InputModifiers::SHIFT,
            },
            MockPort::key('u'),
        ]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::RunDirection {
                direction: Direction::NorthEast
            })
        ));
    }

    #[test]
    fn get_player_action_ctrl_a_repeats_last_action() {
        let mut app = App::new();
        app.remember_repeatable_action(&PlayerAction::Search);
        let mut port = MockPort::new(vec![InputEvent::Key {
            code: InputKeyCode::Char('a'),
            modifiers: InputModifiers::CTRL,
        }]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::Search)));
    }

    #[test]
    fn get_player_action_ctrl_a_without_history_shows_message_and_continues() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            InputEvent::Key {
                code: InputKeyCode::Char('a'),
                modifiers: InputModifiers::CTRL,
            },
            MockPort::key('.'),
        ]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::Rest)));
        assert!(
            port.messages
                .iter()
                .any(|m| m.contains("No previous command to repeat"))
        );
    }

    #[test]
    fn get_player_action_reqmenu_key_shows_command_index() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('m'), MockPort::key('.')]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::Rest)));
        assert!(
            port.shown_text
                .iter()
                .any(|(title, _)| title.contains("Commands"))
        );
    }

    #[test]
    fn get_player_action_shell_key_shows_not_implemented() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![MockPort::key('!'), MockPort::key('.')]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::Rest)));
        assert!(
            port.messages
                .iter()
                .any(|m| m == &app.messages_i18n.not_implemented)
        );
    }

    #[test]
    fn get_player_action_suspend_key_shows_not_implemented() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            InputEvent::Key {
                code: InputKeyCode::Char('z'),
                modifiers: InputModifiers::CTRL,
            },
            MockPort::key('.'),
        ]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::Rest)));
        assert!(
            port.messages
                .iter()
                .any(|m| m == &app.messages_i18n.not_implemented)
        );
    }

    #[test]
    fn get_player_action_perminv_key_maps_to_inventory_view() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![InputEvent::Key {
            code: InputKeyCode::Char('|'),
            modifiers: InputModifiers::NONE,
        }]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::ViewInventory)));
    }

    #[test]
    fn get_extended_command_repeat_uses_last_action() {
        let mut app = App::new();
        app.remember_repeatable_action(&PlayerAction::Search);
        let mut port = MockPort::new(vec![
            MockPort::key('r'),
            MockPort::key('e'),
            MockPort::key('p'),
            MockPort::key('e'),
            MockPort::key('a'),
            MockPort::key('t'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(matches!(action, Some(PlayerAction::Search)));
    }

    #[test]
    fn get_extended_command_reqmenu_shows_command_index() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('r'),
            MockPort::key('e'),
            MockPort::key('q'),
            MockPort::key('m'),
            MockPort::key('e'),
            MockPort::key('n'),
            MockPort::key('u'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(action.is_none());
        assert!(
            port.shown_text
                .iter()
                .any(|(title, _)| title.contains("Commands"))
        );
    }

    #[test]
    fn get_extended_command_perminv_maps_to_inventory_view() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('p'),
            MockPort::key('e'),
            MockPort::key('r'),
            MockPort::key('m'),
            MockPort::key('i'),
            MockPort::key('n'),
            MockPort::key('v'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(matches!(action, Some(PlayerAction::ViewInventory)));
    }

    #[test]
    fn get_extended_command_shell_shows_not_implemented() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('s'),
            MockPort::key('h'),
            MockPort::key('e'),
            MockPort::key('l'),
            MockPort::key('l'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(action.is_none());
        assert!(
            port.messages
                .iter()
                .any(|m| m == &app.messages_i18n.not_implemented)
        );
    }

    #[test]
    fn get_extended_command_wizmap_requires_wizard_mode() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('w'),
            MockPort::key('i'),
            MockPort::key('z'),
            MockPort::key('m'),
            MockPort::key('a'),
            MockPort::key('p'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(action.is_none());
        assert!(
            port.messages
                .iter()
                .any(|m| m == &app.messages_i18n.wizard_mode_disabled)
        );
    }

    #[test]
    fn get_extended_command_wizmap_in_wizard_mode() {
        let mut app = App::new();
        app.set_wizard_mode(true);
        let mut port = MockPort::new(vec![
            MockPort::key('w'),
            MockPort::key('i'),
            MockPort::key('z'),
            MockPort::key('m'),
            MockPort::key('a'),
            MockPort::key('p'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(matches!(action, Some(PlayerAction::WizMap)));
    }

    #[test]
    fn get_extended_command_wizwish_in_wizard_mode() {
        let mut app = App::new();
        app.set_wizard_mode(true);
        let mut port = MockPort::new(vec![
            MockPort::key('w'),
            MockPort::key('i'),
            MockPort::key('z'),
            MockPort::key('w'),
            MockPort::key('i'),
            MockPort::key('s'),
            MockPort::key('h'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        port.lines
            .push_back(Some("blessed +2 gray dragon scale mail".to_string()));
        let action = app.get_extended_command(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::WizWish { wish_text }) if wish_text == "blessed +2 gray dragon scale mail"
        ));
    }

    #[test]
    fn get_extended_command_unimplemented_wizard_command_in_wizard_mode() {
        let mut app = App::new();
        app.set_wizard_mode(true);
        let mut port = MockPort::new(vec![
            MockPort::key('w'),
            MockPort::key('i'),
            MockPort::key('z'),
            MockPort::key('l'),
            MockPort::key('o'),
            MockPort::key('a'),
            MockPort::key('d'),
            MockPort::key('l'),
            MockPort::key('u'),
            MockPort::key('a'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(action.is_none());
        assert!(
            port.messages
                .iter()
                .any(|m| m == &app.messages_i18n.not_implemented)
        );
    }

    #[test]
    fn get_extended_command_unimplemented_wizard_command_requires_wizard_mode() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('w'),
            MockPort::key('i'),
            MockPort::key('z'),
            MockPort::key('l'),
            MockPort::key('o'),
            MockPort::key('a'),
            MockPort::key('d'),
            MockPort::key('l'),
            MockPort::key('u'),
            MockPort::key('a'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(action.is_none());
        assert!(
            port.messages
                .iter()
                .any(|m| m == &app.messages_i18n.wizard_mode_disabled)
        );
    }

    #[test]
    fn get_player_action_ctrl_f_wizmap_in_wizard_mode() {
        let mut app = App::new();
        app.set_wizard_mode(true);
        let mut port = MockPort::new(vec![InputEvent::Key {
            code: InputKeyCode::Char('f'),
            modifiers: InputModifiers::CTRL,
        }]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::WizMap)));
    }

    #[test]
    fn get_player_action_ctrl_f_requires_wizard_mode() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            InputEvent::Key {
                code: InputKeyCode::Char('f'),
                modifiers: InputModifiers::CTRL,
            },
            MockPort::key('.'),
        ]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::Rest)));
        assert!(
            port.messages
                .iter()
                .any(|m| m == &app.messages_i18n.wizard_mode_disabled)
        );
    }

    #[test]
    fn get_player_action_ctrl_v_levelport_in_wizard_mode() {
        let mut app = App::new();
        app.set_wizard_mode(true);
        let mut port = MockPort::new(vec![InputEvent::Key {
            code: InputKeyCode::Char('v'),
            modifiers: InputModifiers::CTRL,
        }]);
        port.lines.push_back(Some("7".to_string()));
        let action = app.get_player_action(&mut port);
        assert!(matches!(
            action,
            Some(PlayerAction::WizLevelTeleport { depth: 7 })
        ));
    }

    #[test]
    fn get_player_action_ctrl_r_maps_to_redraw() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![InputEvent::Key {
            code: InputKeyCode::Char('r'),
            modifiers: InputModifiers::CTRL,
        }]);
        let action = app.get_player_action(&mut port);
        assert!(matches!(action, Some(PlayerAction::Redraw)));
    }

    #[test]
    fn get_extended_command_herecmdmenu_shows_command_index() {
        let mut app = App::new();
        let mut port = MockPort::new(vec![
            MockPort::key('h'),
            MockPort::key('e'),
            MockPort::key('r'),
            MockPort::key('e'),
            MockPort::key('c'),
            MockPort::key('m'),
            MockPort::key('d'),
            MockPort::key('m'),
            MockPort::key('e'),
            MockPort::key('n'),
            MockPort::key('u'),
            InputEvent::Key {
                code: InputKeyCode::Enter,
                modifiers: InputModifiers::NONE,
            },
        ]);
        let action = app.get_extended_command(&mut port);
        assert!(action.is_none());
        assert!(
            port.shown_text
                .iter()
                .any(|(title, _)| title.contains("Commands"))
        );
    }
}
