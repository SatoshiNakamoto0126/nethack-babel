#![feature(gen_blocks)]
//! `nethack-babel-engine` — pure game logic for NetHack Babel.
//!
//! This crate has **zero IO**.  It defines the ECS-backed game world, the
//! player action vocabulary, the event system, dungeon representation,
//! field-of-view computation, combat resolution, and the turn loop.
//!
//! All rendering, input handling, and persistence live in sibling crates.

pub mod action;
pub mod artifacts;
pub mod attributes;
pub mod bones;
pub mod combat;
pub mod conduct;
pub mod detect;
pub mod dig;
pub mod dip;
pub mod dungeon;
pub mod end;
pub mod engrave;
pub mod exper;
pub mod explode;
pub mod environment;
pub mod equipment;
pub mod event;
pub mod fov;
pub mod hunger;
pub mod identification;
pub mod inventory;
pub mod items;
pub mod lock;
pub mod makemon;
pub mod map_gen;
pub mod mkobj;
pub mod mhitm;
pub mod mhitu;
pub mod monmove;
pub mod monster_ai;
pub mod movement;
pub mod npc;
pub mod o_init;
pub mod objnam;
pub mod pager;
pub mod pets;
pub mod polyself;
pub mod potions;
pub mod quest;
pub mod religion;
pub mod rumors;
pub mod scrolls;
pub mod turn;
pub mod ranged;
pub mod role;
pub mod shop;
pub mod spells;
pub mod status;
pub mod teleport;
pub mod tools;
pub mod traps;
pub mod special_levels;
pub mod wands;
pub mod topten;
pub mod wish;
pub mod worm;
pub mod world;
pub mod worn;
pub mod region;
pub mod write;
pub mod light;
pub mod music;
pub mod steed;
pub mod dbridge;
pub mod ball;
pub mod symbols;
pub mod apply;
pub mod fountain;
pub mod steal;
pub mod were;
pub mod mcastu;
pub mod muse;
pub mod pickup;
pub mod sit;
pub mod do_actions;
pub mod vault;
pub mod minion;
pub mod mondata;
pub mod priest;
