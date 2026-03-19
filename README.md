# NetHack Babel

**A modern Rust reimplementation of NetHack 3.7 with built-in multilingual support.**

[中文版 README](README_zh.md)

[![License: NGPL](https://img.shields.io/badge/License-NGPL-blue.svg)](LICENSE)
![Rust: nightly](https://img.shields.io/badge/Rust-nightly-orange.svg)
![Tests: 4667](https://img.shields.io/badge/tests-4667_passing-brightgreen.svg)
![LOC: 211K](https://img.shields.io/badge/LOC-211K_Rust-informational.svg)

## Codebase Comparison: C Original vs Rust Remaster

| Metric | C NetHack 3.7 | Rust Babel | Notes |
|--------|--------------|------------|-------|
| **Language** | C + Lua | Rust | Pure Rust, no C dependencies |
| **Code lines** | 424K (`.c/.h`) + 24K (`.lua`) | 211K (`.rs`) + 14K locale + 26K TOML | Rust splits gameplay, i18n, and data instead of compiling tables into the binary |
| **Source files** | 590 `.c/.h` | 115 `.rs` | Rust workspace uses fewer gameplay files despite broader automated coverage |
| **Level definitions** | 167 `.lua` scripts | 65 `.toml` + Rust generators | TOML for data, Rust for logic |
| **Data files** | Compiled-in tables + dat assets | 65 TOML + 20 locale files + embedded text | Release artifact now ships as a single executable |
| **Tests** | ~0 (manual QA only) | **4,667** automated | Unit + integration + property + differential layers |
| **Specifications** | In-code comments | **29 spec documents** | Extracted formulas with test vectors |
| **i18n** | Optional `#ifdef` | Built-in (5 languages) | Fluent + TOML, hot-switchable |
| **Architecture** | Global state, IO mixed in | ECS + zero-IO events | Deterministic, testable, replayable |
| **Binary size** | ~4MB | ~5.8MB | Single executable release asset with embedded data and locales |

## Overview

NetHack Babel is a ground-up reimplementation of [NetHack 3.7](https://github.com/NetHack/NetHack) in Rust, preserving formula-level accuracy while adopting a modern architecture. It replaces NetHack's global state and manual memory management with an ECS-based design (hecs), separates game logic from all IO, and defines all game content — monsters, items, dungeons — in TOML data files rather than compiled-in tables. The engine emits typed events instead of formatted strings, enabling built-in multilingual support (English, Simplified Chinese, Traditional Chinese, German, French) via Project Fluent without any changes to game logic. The result is a NetHack that is easier to extend, test, translate, and port to new frontends.

## Current Scale

Measured on 2026-03-20 from the checked-in repository:

- **115 Rust source files / 210,956 lines** across the six workspace crates
- **20 locale files / 13,888 lines** of Fluent + TOML translation content
- **65 data TOML files / 25,639 lines** for monsters, items, levels, and manifests
- **320 tracked files / 305,257 total lines** in the repository
- **29 mechanism specs + 29 review notes** under `specs/`
- **4,667 automated tests** in the workspace test suite
- **394 monsters / 430 items** loaded by the current embedded runtime assets

## About Original NetHack

NetHack is one of the foundational roguelikes, developed continuously for decades by the NetHack DevTeam. It is a turn-based, single-player dungeon simulation centered on emergent interactions: item effects combine in surprising ways, dungeon branches create long strategic arcs, and permadeath makes every decision final. The classic objective is to retrieve the Amulet of Yendor, ascend through the Planes, and sacrifice it on the correct Astral altar. NetHack Babel follows this original gameplay contract and treats NetHack 3.7 behavior as the reference standard.

## Features

- **True Color terminal rendering** — ratatui-based TUI with BUC-colored inventory, 16-color system with status highlighting
- **Built-in 5-language support** — English, Simplified Chinese (简体中文), Traditional Chinese (繁體中文), German, French — hot-switchable at runtime with `O` key
- **CJK-aware item naming** — Chinese counter words (量词) system: "3把匕首" instead of "3 daggers"; BUC prefix: "祝福的+2长剑"
- **Data-driven architecture** — 394 monsters, 430 items, 33 artifacts defined in TOML; modify content without recompiling
- **ECS-based game state** — hecs entity-component-system with explicit turn resolution and typed events
- **Formula-precise mechanics** — 29 mechanism specs extracted from the original C source; 4,667 tests verify fidelity
- **Broad gameplay coverage** — the main campaign, special levels, quests, endgame, shops, religion, pets, traps, polymorph, riding, bones, conducts, replay, and save/load are all live runtime systems
- **Per-game appearance shuffling** — each game randomizes potion colors, scroll labels, ring materials
- **Complete special levels** — 30+ generators: Sokoban (8 puzzles), Castle, Medusa, all Gehennom levels, Vlad's Tower, Wizard Tower, Sanctum, Elemental Planes, Astral Plane, 13 role-specific quest branches
- **13 playable roles** — Archeologist, Barbarian, Caveman, Healer, Knight, Monk, Priest, Ranger, Rogue, Samurai, Tourist, Valkyrie, Wizard — each with unique quest, starting inventory, and rank titles
- **Cross-platform** — macOS, Linux, Windows
- **Save/load with anti-savescumming** — bincode serialization with versioned headers
- **Bones system** — death leaves a ghost and cursed items for future characters
- **Leaderboard** — JSON-persisted top-100 score board
- **Deterministic replay** — explicit RNG threading; same seed + same inputs = same game
- **Replay + server runtime modes (MVP)** — replay `.cast` sessions and run a TCP multi-connection server endpoint
- **Wizard mode** — debug commands: create monsters, grant wishes, reveal/detect the current level, inspect special-level topology, wipe the current level, and level-teleport

## Quick Start

Official release artifacts are published as a single executable per tag. Release binaries do **not** need a separate `data/` directory; source checkouts still use the repository `data/` tree.

### Release Binary

```sh
./nethack-babel-v0.1.4-aarch64-apple-darwin --language zh-CN
```

### Source Checkout

```sh
git clone https://github.com/SatoshiNakamoto0126/nethack-babel.git
cd nethack-babel
cargo run -- --data-dir data
```

### Play in Chinese

The official release binary is distributed as a single executable with embedded locale/data assets. In a source checkout, keep passing `--data-dir data` as shown here.

```sh
cargo run -- --data-dir data --language zh-CN    # Simplified Chinese
cargo run -- --data-dir data --language zh-TW    # Traditional Chinese
```

Or press `O` in-game to switch languages without restarting.

### Wizard Mode (Debug)

```sh
cargo run -- --data-dir data -D
```

Then use Ctrl+W (wish), Ctrl+F (map), Ctrl+G (genesis), Ctrl+I (identify all).

### Replay / Server Modes

```sh
cargo run -- --replay ./session.cast
cargo run -- --server 127.0.0.1:2323
```

Rust nightly is required for source builds and is selected automatically via `rust-toolchain.toml`. If you use [rustup](https://rustup.rs/), no manual setup is needed.
The release workflow now publishes a single executable asset per tag instead of a tarball containing `data/`.

## Controls

| Key | Action | Key | Action |
|-----|--------|-----|--------|
| `h` `j` `k` `l` | Move (vi-keys) | `y` `u` `b` `n` | Move diagonally |
| Arrow keys | Move (cardinal) | `.` | Wait one turn |
| `i` | Inventory | `,` | Pick up item |
| `d` | Drop | `e` | Eat |
| `q` | Quaff (drink) | `r` | Read (scroll) |
| `z` | Zap (wand) | `Z` | Cast spell |
| `f` | Fire (ranged) | `t` | Throw |
| `w` | Wield weapon | `W` | Wear armor |
| `T` | Take off armor | `P` | Put on accessory |
| `R` | Remove accessory | `s` | Search |
| `<` | Go upstairs | `>` | Go downstairs |
| `o` | Open door | `c` | Close door |
| `k` | Kick | `a` | Apply (tool) |
| `p` | Pay shopkeeper | `O` | Options / language |
| `S` | Save and quit | `Ctrl+C` | Quit |
| `?` | Help | `Ctrl+P` | Message history |
| `#` | Extended commands | `F` | Force fight |

### Extended Commands

Press `#` then type a command name (Tab for completion):

| Command | Action | Command | Action |
|---------|--------|---------|--------|
| `pray` | Pray to your god | `loot` | Loot a container |
| `enhance` | Enhance weapon skills | `name` | Name an item |
| `dip` | Dip item in liquid | `ride` | Mount a steed |
| `offer` | Sacrifice at altar | `invoke` | Invoke artifact |
| `sit` | Sit down | `jump` | Jump |
| `turn` | Turn undead | `untrap` | Disarm trap |
| `wipe` | Wipe face | `swap` | Swap weapons |
| `known` | List identified items | `vanquished` | List killed monsters |
| `conduct` | View conducts | `overview` | Dungeon overview |
| `wait` | Wait (explicit) | `call` | Name item type |

## Architecture

NetHack Babel is a Cargo workspace with six crates. Dependencies flow strictly downward; the engine never performs IO.

| Crate | Role | LOC | Tests |
|-------|------|-----|-------|
| `engine` | Pure game logic — combat, monsters, items, dungeon, turn loop, 80+ modules | 170,847 | 3,800+ |
| `data` | TOML schema definitions and loaders for monsters, items, dungeons, levels | 4,753 | 44 |
| `i18n` | Fluent-based localization, item naming (doname), CJK classifiers | 7,209 | 91 |
| `tui` | Terminal UI built on ratatui + crossterm, 16-color system | 8,794 | 172 |
| `audio` | Sound effects via rodio, triggered by engine events | 340 | 58 |
| `cli` | Binary entry point — config, save/load, options, main loop | 19,013 | 195 |

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

| System | Status | Key Features |
|--------|--------|-------------|
| Melee combat | Complete | Full hit/damage chain, negative AC roll, backstab, two-weapon, Monk martial arts |
| Ranged combat | Complete | Thrown/fired projectiles, ammunition, launchers |
| Spells (40+) | Complete | All spell effects: fireball, drain life, finger of death, healing, detect, polymorph |
| Wands (24) | Complete | Ray bouncing, self-zap, object hitting, recharge explosion |
| Potions (26) | Complete | Full BUC × confused matrix, splash effects on throw |
| Scrolls (23) | Complete | Identify, enchant, genocide, teleport — all with BUC variants |
| Artifacts (33) | Complete | Special attacks, defenses, invoke effects |
| Equipment | Complete | 12 slots, 50+ intrinsic types from rings/amulets/boots/cloaks/helms/gloves |
| Monster AI | Complete | Flee logic, spellcasting, item use, demon lord special behaviors |
| Dungeon (8 branches) | Complete | Main, Mines, Sokoban, Quest, Fort Ludios, Gehennom, Vlad, Endgame |
| 30+ special levels | Complete | All Gehennom demon lairs, Wizard Tower, Sanctum, Elemental Planes, Astral |
| 13 quest sets | Complete | Role-specific maps, leaders, nemeses, artifacts |
| 13 room types | Complete | Shops, temples, zoos, barracks, beehives, morgues, thrones, swamps |
| Traps (25) | Complete | All types with player + monster interactions |
| Shop system | Complete | Pricing, charisma, price ID, theft, credit, Kops |
| Religion / prayer | Complete | Prayer, sacrifice, crowning, minion summoning, guardian angel |
| Pet system | Complete | Loyalty, hunger, combat AI, cross-level following |
| Hunger / eating | Complete | Corpse intrinsics (16+ types), tin mechanics, choking, cannibalism |
| Polymorph | Complete | Form abilities, system shock, armor breaking, steed dismount |
| Riding | Complete | Mounted speed, combat modifier, water traversal |
| Status effects | Complete | 25+ timed effects with expiration, progressive stoning/sliming/sickness |
| Explosions | Complete | 7 types, 3×3 blast, item destruction |
| Ball & chain | Complete | Drag mechanics, punishment |
| Drawbridge | Complete | State machine, entity crush/dodge |
| Vault / Priest / Minion | Complete | Guard NPC, temple mechanics, divine summoning |
| Detection / Light / FOV | Complete | Search, magic mapping, light source fuel, shadowcasting |
| Bones system | Complete | Ghost generation, item cursing |
| Conduct tracking | Complete | 13 standard + Elberethless |
| Score & leaderboard | Complete | JSON-persisted top 100, DYWYPI disclosure |
| Identification | Complete | Multi-layer (appearance, BUC, enchant), per-game shuffled appearances |
| Object naming | Complete | doname/xname/an/the/plural/erosion pipeline |
| Attribute exercise | Complete | STR/DEX/CON/INT/WIS/CHA growth from actions |
| Options system | Complete | 40+ settings, RC file parsing |
| Help system | Complete | 4 help files, symbol lookup (whatis) |
| Music / sounds | Complete | All instruments, monster vocalizations, ambient sounds |

## i18n System

NetHack Babel separates all player-visible text from game logic:

- **1,624 message templates** — [Project Fluent](https://projectfluent.org/) `.ftl` files
- **Entity name translations** — TOML files for monster/object names in each language
- **Counter words (量词)** — `classifiers.toml` for CJK measure words
- **Item naming** — `doname()` pipeline: "祝福的+2长剑" (CJK) or "a blessed +2 long sword" (English)
- **Hot-switchable** — Press `O` in-game, no restart required

### Adding a new language

1. Create `data/locale/<code>/manifest.toml` with language metadata
2. Add `messages.ftl` with Fluent message translations
3. Optionally add `monsters.toml`, `objects.toml` for entity name translations
4. For CJK languages, add `classifiers.toml` with counter word mappings

## Mechanism Specifications

The `specs/` directory contains 29 mechanism specifications extracted from the original NetHack C source, reviewed and paired with 29 review notes. Each spec documents the exact formulas, constants, edge cases, and test vectors for a game subsystem.

## Testing Infrastructure

NetHack Babel uses a 4-layer test pyramid plus a differential execution harness for compiler-grade cross-validation against the original C engine.

### Test Pyramid (4,667+ tests)

| Layer | Tests | Purpose |
|-------|-------|---------|
| **Unit tests** | ~3,900 | Pure function I/O: damage formulas, AC calculation, BUC matrices |
| **Snapshot tests** | ~500 | Prevent silent regression in item names, messages, i18n rendering |
| **Integration touchstones** | 100+ | Multi-system event chains: melee→death→corpse, stoning→cure, polymorph→revert |
| **Property-based (proptest)** | 32 | Random input invariants: dice bounds, rnl range, plural never empty, score non-negative |
| **Monte Carlo (10K samples)** | 21 | Probability distributions: hit rates, dice means, choking 1/20, fountain wish 1/4000 |

### Differential Execution Harness

An industrial-grade cross-validation framework that lets the C and Rust engines "talk to each other":

1. **C-Side Observer**: Instrumented C NetHack (`#ifdef DIFF_TEST`) dumps per-turn state to JSONL — player HP/position/AC/status, RNG call log, inventory weight, 14-bit status bitmask, monster count
2. **Rust-Side Replay**: Loads C recordings, replays actions via `resolve_turn()`, compares observable state
3. **Fuzzing Pipeline**: `scripts/fuzz_c_recordings.sh` generates thousands of random recordings to hunt divergences automatically

```bash
# Generate a C recording
bash scripts/generate_c_recording.sh movement 20
# (legacy form still supported)
bash scripts/generate_c_recording.sh 12345 movement 20

# Run differential tests
cargo test -p nethack-babel-engine --test differential

# Fuzz: generate 100 random recordings × 200 turns each
bash scripts/fuzz_c_recordings.sh 100 200
```

When a divergence is found (e.g., "C says player is Punished, Rust says not"), the exact turn, RNG calls, and state delta are reported for targeted debugging.

The harness has been validated end-to-end: a 20-turn C recording was successfully generated and consumed by the Rust differential tests.
Nightly replay/property/Monte-Carlo regression is automated in
`.github/workflows/nightly-differential.yml` (with optional C-side corpus refresh if `NETHACK_DIR` is configured in repository variables).

## Building

### Prerequisites

Rust nightly is the only requirement:

```sh
cargo build                              # Build
cargo test --workspace                   # Run all 4,667+ tests
cargo run -- --data-dir data             # Run (English)
cargo run -- --data-dir data --language zh-CN  # Run (Chinese)
cargo run -- --data-dir data -D          # Wizard mode
cargo clippy --workspace --all-targets   # Lint
cargo build --release                    # Release build
```

For the GitHub release binary, download the single executable asset attached to the tag release. Source checkouts still use the repository `data/` tree when run locally.

## Project Status

The project is now in the "long-tail parity" phase rather than the "missing core systems" phase. The main campaign, quest branches, special levels, endgame, shops, religion, pets, bones, replay, save/load, leaderboard, and multilingual TUI are all playable and exercised by 4,667 automated tests. The remaining gaps are concentrated in original-NetHack texture and cadence: Wizard of Yendor covetous behavior, `sounds.c` conversational and ambient edge cases, and a few choice-heavy interactions where UX still lags behind the C original.

Save format note: the current on-disk save version is `1.0.0`. Older `0.3.x` saves are intentionally rejected because level-scoped floor object data and story/runtime state are now serialized differently.

See [GAP_STATUS.md](GAP_STATUS.md) for the detailed status report and [DIFFERENCES.md](DIFFERENCES.md) for known deviations from NetHack 3.7 behavior.

## Roadmap (2026)

1. **Wizard of Yendor fidelity pass**  
   Continue porting the remaining `wizard.c` texture: fuller covetous targeting, more original `intervene()` cadence, and tighter `nasty()` candidate selection so late-game pressure feels closer to vanilla NetHack.
2. **NPC voice and ambient texture pass**  
   Keep closing the gap with `sounds.c`, especially the remaining `MS_VAMPIRE`, `MS_IMITATE`, `MS_TRUMPET`, full-moon/were timing, and a few room ambience cases such as swamp and barracks.
3. **Shop, Oracle, and demon interaction UX**  
   Preserve the engine behavior that already exists, but improve the player-facing prompt flow so consultations, bribes, pricing, and warnings feel as complete as the original C interaction model.
4. **Drift harness expansion**  
   Grow the current story/save matrices into broader campaign-level drift detection for economy, religion, branch state, and endgame pressure, so parity improvements stay stable across save/load and replay.

## TODO (Short-Term)

- [ ] Finish the remaining `wizard.c` covetous and intervention long-tail, especially cadence and target selection details.
- [ ] Close the highest-signal `sounds.c` gaps: vampire lines, imitate/trumpet side effects, and remaining ambience texture.
- [ ] Tighten shop/oracle/demon prompt UX so player-facing interaction quality matches the engine parity already implemented underneath.
- [ ] Keep expanding story/save drift coverage for wizard, religion, economy, and branch-transition edge cases.

## Documentation

- [Player's Guidebook (English)](doc/GUIDEBOOK.md) — Complete gameplay guide
- [玩家指南（中文）](doc/GUIDEBOOK_zh.md) — 完整游戏攻略
- [Content Contributor Guide](docs/Content-Contributor-Guide.md) — Data/content editing workflow and validation checklist
- [DIFFERENCES.md](DIFFERENCES.md) — Architectural divergences from C NetHack
- [CONTRIBUTING.md](CONTRIBUTING.md) — Build instructions, code style, testing

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for build instructions, code style, testing conventions, and how to add monsters, items, translations, and new game mechanics.

## License

[NetHack General Public License (NGPL)](LICENSE).

Third-party crate dependencies are restricted to MIT, Apache-2.0, BSD, or LGPL licenses, enforced by `cargo-deny`.

## Acknowledgments

NetHack Babel builds on decades of work by the [NetHack DevTeam](https://nethack.org/). The original NetHack source is available at [github.com/NetHack/NetHack](https://github.com/NetHack/NetHack).

Key Rust dependencies: [hecs](https://crates.io/crates/hecs) (ECS), [ratatui](https://crates.io/crates/ratatui) (terminal UI), [fluent](https://crates.io/crates/fluent) (i18n), [rodio](https://crates.io/crates/rodio) (audio), [rand](https://crates.io/crates/rand) / [rand_pcg](https://crates.io/crates/rand_pcg) (deterministic RNG), [bincode](https://crates.io/crates/bincode) (save files), [insta](https://crates.io/crates/insta) (snapshot testing).
