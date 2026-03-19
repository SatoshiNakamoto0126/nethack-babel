//! Player action vocabulary, position, and direction types.
//!
//! Defines the complete set of player inputs that the engine can process,
//! along with the spatial primitives (position, direction) used throughout
//! the game logic.

use hecs::Entity;
use serde::{Deserialize, Serialize};

/// A position on a level map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    #[inline]
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Return the position offset by one step in the given direction.
    #[inline]
    pub fn step(self, dir: Direction) -> Self {
        let (dx, dy) = dir.delta();
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

/// Cardinal and inter-cardinal directions plus vertical and self.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
    Up,
    Down,
    Self_,
}

impl Direction {
    /// Map direction to (dx, dy) offset. Up/Down/Self_ map to (0,0).
    #[inline]
    pub fn delta(self) -> (i32, i32) {
        match self {
            Direction::North => (0, -1),
            Direction::South => (0, 1),
            Direction::East => (1, 0),
            Direction::West => (-1, 0),
            Direction::NorthEast => (1, -1),
            Direction::NorthWest => (-1, -1),
            Direction::SouthEast => (1, 1),
            Direction::SouthWest => (-1, 1),
            Direction::Up | Direction::Down | Direction::Self_ => (0, 0),
        }
    }

    /// Whether this direction is a diagonal (NE, NW, SE, SW).
    #[inline]
    pub fn is_diagonal(self) -> bool {
        matches!(
            self,
            Direction::NorthEast
                | Direction::NorthWest
                | Direction::SouthEast
                | Direction::SouthWest
        )
    }

    /// All eight planar movement directions.
    pub const PLANAR: [Direction; 8] = [
        Direction::North,
        Direction::NorthEast,
        Direction::East,
        Direction::SouthEast,
        Direction::South,
        Direction::SouthWest,
        Direction::West,
        Direction::NorthWest,
    ];
}

/// Identifies a known spell by index in the spell book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpellId(pub u8);

/// What the player wants to name/annotate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NameTarget {
    /// Name an individual item.
    Item { item: Entity },
    /// Call an item class (e.g., "a potion called healing").
    ItemClass { class: char },
    /// Annotate the current dungeon level.
    Level,
    /// Name a monster.
    Monster { entity: Entity },
    /// Name the monster at a map position.
    MonsterAt { position: Position },
}

/// All possible player inputs that advance (or query) the game state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerAction {
    // ── Movement ──────────────────────────────────────────────
    Move {
        direction: Direction,
    },
    MoveUntilInterrupt {
        direction: Direction,
    },
    FightDirection {
        direction: Direction,
    },
    RunDirection {
        direction: Direction,
    },
    RushDirection {
        direction: Direction,
    },
    MoveNoPickup {
        direction: Direction,
    },
    Rest,
    Wait,
    Search,

    // ── Items ─────────────────────────────────────────────────
    PickUp,
    Drop {
        item: Entity,
    },
    DropMultiple {
        items: Vec<Entity>,
    },
    Eat {
        item: Option<Entity>,
    },
    Quaff {
        item: Option<Entity>,
    },
    Read {
        item: Option<Entity>,
    },
    ZapWand {
        item: Entity,
        direction: Option<Direction>,
    },
    CastSpell {
        spell: SpellId,
        direction: Option<Direction>,
    },
    Wear {
        item: Entity,
    },
    TakeOff {
        item: Entity,
    },
    TakeOffAll,
    Wield {
        item: Entity,
    },
    PutOn {
        item: Entity,
    },
    Remove {
        item: Entity,
    },
    Apply {
        item: Entity,
    },
    Throw {
        item: Entity,
        direction: Direction,
    },
    Fire,

    // ── Interaction ───────────────────────────────────────────
    Open {
        direction: Direction,
    },
    Close {
        direction: Direction,
    },
    Kick {
        direction: Direction,
    },
    ForceLock {
        item: Entity,
    },

    // ── Extended commands ─────────────────────────────────────
    Pray,
    Offer {
        item: Option<Entity>,
    },
    Chat {
        direction: Direction,
    },
    ConsultOracle {
        direction: Direction,
        major: bool,
    },
    Loot,
    EnhanceSkill,
    Dip {
        item: Entity,
        into: Entity,
    },
    Ride,
    Engrave {
        text: String,
    },
    Name {
        target: NameTarget,
        name: String,
    },
    Adjust {
        item: Entity,
        new_letter: char,
    },
    Sit,
    Jump {
        position: Position,
    },
    Untrap {
        direction: Direction,
    },
    TurnUndead,
    Swap,
    Wipe,
    Tip {
        item: Entity,
    },
    Rub {
        item: Entity,
    },
    InvokeArtifact {
        item: Entity,
    },
    Monster,

    // ── Meta / UI queries ────────────────────────────────────
    ViewInventory,
    ViewEquipped,
    ViewDiscoveries,
    ViewConduct,
    DungeonOverview,
    ViewTerrain,
    ShowVersion,
    Annotate {
        text: String,
    },
    Attributes,
    LookAt {
        position: Position,
    },
    LookHere,
    Help,
    ShowHistory,
    CallType {
        class: char,
        name: String,
    },
    KnownItems,
    KnownClass {
        class: char,
    },
    Vanquished,
    Chronicle,
    Glance {
        direction: Direction,
    },
    Redraw,
    WhatIs {
        position: Option<Position>,
    },

    // ── Stairs ────────────────────────────────────────────────
    GoUp,
    GoDown,

    // ── Special ───────────────────────────────────────────────
    Travel {
        destination: Position,
    },
    Pay,
    ToggleTwoWeapon,

    // ── Session control ─────────────────────────────────────
    Save,
    Quit,
    SaveAndQuit,

    // ── Options / language switch ─────────────────────────
    Options,

    // ── Wizard mode commands (debug only) ─────────────────
    /// #genesis -- create a monster by name.
    WizGenesis {
        monster_name: String,
    },
    /// #wish -- wish for an item.
    WizWish {
        wish_text: String,
    },
    /// #identify -- identify all inventory items.
    WizIdentify,
    /// #map -- reveal entire map.
    WizMap,
    /// #levelchange -- jump to a specific dungeon depth.
    WizLevelTeleport {
        depth: i32,
    },
    /// #detect -- detect all monsters, objects, and traps.
    WizDetect,
    /// #where -- show special level locations.
    WizWhere,
    /// #kill -- kill a target monster.
    WizKill,
}
