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
pub use loader::{GameData, LoadError, load_game_data, load_monsters, load_objects};
pub use schema::*;
