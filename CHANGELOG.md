# Changelog

All notable changes to NetHack Babel will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [0.1.0] - 2026-03-16

### Added
- Complete NetHack 3.7 reimplementation in Rust (133K LOC, 4,103+ tests)
- 80+ engine modules covering all gameplay systems
- Terminal UI (ratatui + crossterm)
- 5 languages: English, Simplified Chinese, Traditional Chinese, German (partial), French (partial)
- Per-game randomized item appearances (potion colors, scroll labels, etc.)
- 394 monsters, 430 items, 33 artifacts (TOML data files)
- 30+ special level generators (Sokoban, Castle, Gehennom, Planes, etc.)
- 13 playable roles with unique quests and rank titles
- Save/load with anti-savescum protection
- JSON-persisted leaderboard (top 100)
- Wizard mode (-D flag) with debug commands
- 4-layer test pyramid: unit, snapshot, integration touchstone, property-based
- Differential execution test harness (C->JSONL->Rust replay)
- 29 mechanism specification documents extracted from C source

### Architecture
- ECS-based game state (hecs)
- Zero-IO engine design (pure logic, events only)
- Explicit RNG threading for deterministic replay
- Project Fluent (.ftl) for message localization
- TOML data files for all game content
