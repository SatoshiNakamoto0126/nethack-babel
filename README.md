# NetHack Babel

**A modern Rust reimplementation of NetHack 3.7 with built-in multilingual support.**

![CI](https://github.com/user/nethack-babel/actions/workflows/ci.yml/badge.svg)
[![License: NGPL](https://img.shields.io/badge/License-NGPL-blue.svg)](LICENSE)
![Rust: nightly](https://img.shields.io/badge/Rust-nightly-orange.svg)
![Tests: 1195](https://img.shields.io/badge/tests-1195_passing-brightgreen.svg)

<!-- TODO: Add terminal screenshot -->

## Overview

NetHack Babel is a ground-up reimplementation of [NetHack 3.7](https://github.com/NetHack/NetHack) in Rust, preserving formula-level accuracy while adopting a modern architecture. It replaces NetHack's global state and manual memory management with an ECS-based design (hecs), separates game logic from all IO, and defines all game content — monsters, items, dungeons — in TOML data files rather than compiled-in tables. The engine emits typed events instead of formatted strings, enabling built-in trilingual support (English, Simplified Chinese, Traditional Chinese) via Project Fluent without any changes to game logic. The result is a NetHack that is easier to extend, test, translate, and port to new frontends.

## Features

- **True Color terminal rendering** — ratatui-based TUI with BUC-colored inventory, syntax-highlighted messages, and minimap
- **Built-in trilingual support** — English, Simplified Chinese (简体中文), and Traditional Chinese (繁體中文) via [Project Fluent](https://projectfluent.org/), hot-switchable at runtime with `O` key
- **CJK-aware item naming** — Chinese counter words (量词) system: "3把匕首" instead of "3 daggers"; BUC prefix: "祝福的+2长剑"
- **Data-driven architecture** — 383 monsters, 369 items, and level parameters defined in TOML; modify content without recompiling
- **ECS-based game state** — hecs entity-component-system with explicit turn resolution and typed events
- **Formula-precise mechanics** — 28 mechanism specs (31,274 lines) extracted from the original C source; 1,195 tests verify fidelity
- **Deep alignment** — combat formulas, potion/scroll/wand BUC matrices, shop pricing, prayer mechanics, monster AI, hunger system, trap damage — all match original NetHack behavior including documented edge cases
- **Cross-platform** — macOS, Linux, Windows
- **Save/load with anti-savescumming** — bincode serialization with versioned headers and integrity checks
- **Bones system** — death leaves a ghost and cursed items for future characters to find
- **Deterministic replay** — explicit RNG threading; same seed + same inputs = same game
- **Planned** — Steam integration (cloud saves, achievements, Workshop), SSH multiplayer server, asciinema recording

## Quick Start

```sh
git clone https://github.com/user/nethack-babel.git
cd nethack-babel
cargo run -- --data-dir data
```

### Play in Chinese

```sh
cargo run -- --data-dir data --language zh_CN    # Simplified Chinese
cargo run -- --data-dir data --language zh_TW    # Traditional Chinese
```

Or press `O` in-game to switch languages without restarting.

Rust nightly is required and will be selected automatically via `rust-toolchain.toml`. If you use [rustup](https://rustup.rs/), no manual setup is needed.

## Controls

| Key | Action | Key | Action |
|-----|--------|-----|--------|
| `h` `j` `k` `l` | Move (vi-keys) | `y` `u` `b` `n` | Move diagonally |
| Arrow keys | Move (cardinal) | `.` | Wait one turn |
| `i` | Inventory | `,` | Pick up item |
| `d` | Drop | `e` | Eat |
| `q` | Quaff (drink) | `r` | Read (scroll) |
| `z` | Zap (wand) | `f` | Fire (ranged) |
| `w` | Wield weapon | `W` | Wear armor |
| `T` | Take off armor | `P` | Put on accessory |
| `R` | Remove accessory | `s` | Search |
| `<` | Go upstairs | `>` | Go downstairs |
| `o` | Open door | `c` | Close door |
| `p` | Pay shopkeeper | `O` | Options / language |
| `S` | Save and quit | `Ctrl+C` | Quit |
| `?` | Help | `Ctrl+P` | Message history |
| `#` | Extended commands | `0`-`9` | Count prefix |

### Extended Commands

Press `#` then type a command name (Tab for completion):

| Command | Action | Command | Action |
|---------|--------|---------|--------|
| `pray` | Pray to your god | `loot` | Loot a container |
| `enhance` | Enhance weapon skills | `name` | Name an item |
| `dip` | Dip item in liquid | `ride` | Mount a steed |
| `offer` | Sacrifice at altar | `quit` | Quit the game |

## Architecture

NetHack Babel is a Cargo workspace with six crates. Dependencies flow strictly downward; the engine never performs IO.

| Crate | Role | LOC | Tests |
|-------|------|-----|-------|
| `engine` | Pure game logic — combat, monsters, items, dungeon, turn loop, 27 modules | 47,000 | 1,059 |
| `data` | TOML schema definitions and loaders for monsters, items, dungeons | 3,920 | 23 |
| `i18n` | Fluent-based localization, item naming (doname), CJK classifiers | 3,644 | 66 |
| `tui` | Terminal UI built on ratatui + crossterm | 2,696 | 13 |
| `audio` | Sound effects via rodio, triggered by engine events | 340 | 10 |
| `cli` | Binary entry point — config, save/load, main loop orchestration | 3,147 | 24 |

```
                     cli
                  /  |  \  \
                /    |    \   \
             tui   i18n  audio  save
               \     |    /
                \    |   /
                 engine
                   |
                  data
```

## Game Systems

| System | Tests | Key Features |
|--------|-------|-------------|
| Melee combat | 66 | Full hit/damage chain, negative AC roll, backstab, two-weapon, Monk martial arts |
| Monster AI | 34 | Flee logic, door interaction, fly/swim/phase, teleport, covetous harass |
| Monster generation | — | Depth scaling, group spawn, initial equipment, spawn rate timers |
| Dungeon generation | 58 | Room/corridor, 13 special room types, 8 dungeon branches, stair reachability |
| Traps (25 types) | 33 | Placement by depth, damage formulas, detection, avoidance, trap-specific effects |
| Potions (26 types) | 51 | Full BUC × confused matrix, healing formulas, acid resistance, see invisible |
| Scrolls (23 types) | 40 | Identify rn2(5), enchant weapon high-spe, genocide BUC×confused, confuse monster |
| Wands (24 types) | 25 | Death vs undead, recharge explosion, self-zap, wresting, fire/cold cross-resist |
| Artifacts (33) | — | Special attacks, defenses, invoke effects |
| Shop system | 94 | Full pricing pipeline, charisma table, price ID, theft, credit, 12 shop types, Kops |
| Religion / prayer | 103 | Prayer timeout, success chain, effect priority, Gehennom rule, crowning, luck -13..+13 |
| Pet system | 69 | Food quality, combat AI balk threshold, Pet Sematary revival, cross-level, leash |
| Hunger / eating | 98 | Ring/amulet/regen hunger, corpse spoilage, racial modifiers, fainting, starvation |
| Status effects | 45 | 11 timed effects with decay, intrinsics from corpses, confusion direction randomization |
| Bones system | 25 | Death snapshot, ghost behavior, item cursing/downgrade, anti-cheat |
| Identification | — | BUC testing, appearance shuffling, price ID |
| Score & XP | 41 | Monster XP formula (8 bonus categories), level thresholds, score calculation |
| Conduct tracking | — | 13 standard conducts + Elberethless |
| RNG verification | 8 | 10K-sample statistical tests for key probability events |
| Classic exploits | 20 | Elbereth, pudding farming, price ID, Excalibur dip, unicorn horn |
| Item naming (i18n) | 66 | doname pipeline, insta snapshots for all object classes, CJK leak guard |
| Save / load | — | Bincode serialization, versioned headers |
| FOV | — | Recursive shadowcasting |
| Multi-level dungeon | — | Level caching, monster preservation, stair transitions |

## i18n System

NetHack Babel separates all player-visible text from game logic:

- **Message templates** — [Project Fluent](https://projectfluent.org/) `.ftl` files for combat messages, UI strings
- **Entity name translations** — TOML files mapping English monster/object names to Chinese (`monsters.toml`, `objects.toml`)
- **Counter words (量词)** — `classifiers.toml` maps object classes and specific items to the correct Chinese measure word
- **Item naming** — `doname()` / `doname_locale()` pipeline produces "祝福的+2长剑" (CJK) or "a blessed +2 long sword" (English)
- **Hot-switchable** — Press `O` in-game, no restart required

### Adding a new language

1. Create `data/locale/<code>/manifest.toml` with language metadata
2. Add `messages.ftl` with Fluent message translations
3. Optionally add `monsters.toml`, `objects.toml` for entity name translations
4. For CJK languages, add `classifiers.toml` with counter word mappings

## Mechanism Specifications

The `specs/` directory contains 28 mechanism specifications totaling 31,274 lines, extracted from the original NetHack C source, reviewed, and verified. Each spec documents the exact formulas, constants, and edge cases for a game subsystem and includes test vectors with precise input/output pairs.

| Spec | Source | Lines | System |
|------|--------|-------|--------|
| melee-combat | uhitm.c | 1,393 | Hit/damage formulas |
| monster-attack | mhitu.c | 1,323 | Monster attack types and effects |
| wand-ray | zap.c | 1,313 | Wand effects, ray propagation |
| item-naming | objnam.c | 1,260 | doname() pipeline |
| shop | shk.c | 879 | Pricing, shopkeeper AI |
| religion | pray.c | 1,061 | Prayer, sacrifice, luck |
| trap | trap.c | 928 | Trap types, damage, avoidance |
| hunger | eat.c | 724 | Nutrition, eating, corpse effects |
| ... | | | (28 total) |

## Building

### Prerequisites

Rust nightly is the only requirement:

```sh
# Build
cargo build

# Run all 1,195 tests
cargo test --workspace

# Run the game
cargo run -- --data-dir data

# Run with Chinese
cargo run -- --data-dir data --language zh_CN

# Text-mode fallback (no TUI)
cargo run -- --data-dir data --text

# Lint
cargo clippy --workspace --all-targets

# Release build (optimized, stripped)
cargo build --release
```

## Project Status

The game engine is feature-complete through Phase 4 of the alignment plan. All core systems — combat, items, monsters, dungeon generation, pets, religion, traps, shops, hunger, status effects, identification, conducts, bones, save/load, and the terminal UI — are implemented and verified against the original NetHack source with 1,195 passing tests.

See [ALIGNMENT_REPORT_PHASE2.md](ALIGNMENT_REPORT_PHASE2.md) for the detailed alignment report and [DIFFERENCES.md](DIFFERENCES.md) for known deviations from NetHack 3.7 behavior.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for build instructions, code style, testing conventions, and how to add monsters, items, translations, and new game mechanics.

## License

[NetHack General Public License (NGPL)](LICENSE).

Third-party crate dependencies are restricted to MIT, Apache-2.0, BSD, or LGPL licenses, enforced by `cargo-deny`.

## Acknowledgments

NetHack Babel builds on decades of work by the [NetHack DevTeam](https://nethack.org/). The original NetHack source is available at [github.com/NetHack/NetHack](https://github.com/NetHack/NetHack).

Key Rust dependencies: [hecs](https://crates.io/crates/hecs) (ECS), [ratatui](https://crates.io/crates/ratatui) (terminal UI), [fluent](https://crates.io/crates/fluent) (i18n), [rodio](https://crates.io/crates/rodio) (audio), [rand](https://crates.io/crates/rand) / [rand_pcg](https://crates.io/crates/rand_pcg) (deterministic RNG), [bincode](https://crates.io/crates/bincode) (save files), [insta](https://crates.io/crates/insta) (snapshot testing).
