#![feature(gen_blocks)]
//! `nethack-babel-engine` — pure game logic for NetHack Babel.
//!
//! This crate has **zero IO**.  It defines the ECS-backed game world, the
//! player action vocabulary, the event system, dungeon representation,
//! field-of-view computation, combat resolution, and the turn loop.
//!
//! All rendering, input handling, and persistence live in sibling crates.

pub mod action;
pub mod apply;
pub mod artifacts;
pub mod attributes;
pub mod ball;
pub mod bones;
pub mod combat;
pub mod conduct;
pub mod dbridge;
pub mod detect;
pub mod dig;
pub mod dip;
pub mod do_actions;
pub mod dungeon;
pub mod end;
pub mod engrave;
pub mod environment;
pub mod equipment;
pub mod event;
pub mod exper;
pub mod explode;
pub mod fountain;
pub mod fov;
pub mod hunger;
pub mod identification;
pub mod inventory;
pub mod items;
pub mod light;
pub mod lock;
pub mod makemon;
pub mod map_gen;
pub mod mcastu;
pub mod mhitm;
pub mod mhitu;
pub mod minion;
pub mod mkobj;
pub mod mondata;
pub mod monmove;
pub mod monster_ai;
pub mod movement;
pub mod muse;
pub mod music;
pub mod npc;
pub mod o_init;
pub mod objnam;
pub mod pager;
pub mod pets;
pub mod pickup;
pub mod polyself;
pub mod potions;
pub mod priest;
pub mod quest;
pub mod ranged;
pub mod region;
pub mod religion;
pub mod role;
pub mod rumors;
pub mod scrolls;
pub mod shop;
pub mod sit;
pub mod special_levels;
pub mod spells;
pub mod status;
pub mod steal;
pub mod steed;
pub mod symbols;
pub mod teleport;
pub mod tools;
pub mod topten;
pub mod traps;
pub mod turn;
pub mod vault;
pub mod wands;
pub mod were;
pub mod wish;
pub mod world;
pub mod worm;
pub mod worn;
pub mod write;
