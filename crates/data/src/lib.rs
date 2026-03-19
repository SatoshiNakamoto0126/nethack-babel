//! Game data schemas and definitions for NetHack Babel.
//!
//! This crate defines all static game data structures (monster species,
//! object types, terrain, traps, properties) and runtime ECS components
//! (object instances, player state, map cells, monster instances).

pub mod components;
pub mod const_tables;
pub mod level_loader;
pub mod level_schema;
pub mod loader;
pub mod schema;

pub use components::*;
pub use loader::{
    GameData, ITEM_FILE_NAMES, LoadError, load_embedded_game_data, load_game_data,
    load_game_data_from_sources, load_monsters, load_monsters_from_str, load_objects,
    load_objects_from_str,
};
pub use schema::*;
