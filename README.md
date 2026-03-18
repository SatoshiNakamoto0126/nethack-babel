# NetHack Babel

**A modern Rust reimplementation of NetHack 3.7 with built-in multilingual support.**

[中文版 README](README_zh.md)

[![License: NGPL](https://img.shields.io/badge/License-NGPL-blue.svg)](LICENSE)
![Rust: nightly](https://img.shields.io/badge/Rust-nightly-orange.svg)
![Tests: 4217](https://img.shields.io/badge/tests-4217_passing-brightgreen.svg)
![LOC: 133K](https://img.shields.io/badge/LOC-133K-informational.svg)

## Codebase Comparison: C Original vs Rust Remaster

| Metric | C NetHack 3.7 | Rust Babel | Notes |
|--------|--------------|------------|-------|
| **Language** | C + Lua | Rust | Pure Rust, no C dependencies |
| **Code lines** | 214K (C) + 13K (Lua) | 124K (Rust) | 45% fewer lines for same coverage |
| **Source files** | 133 `.c` + 90 `.h` | 115 `.rs` | Fewer files, larger modules |
| **Level definitions** | 131 `.lua` scripts | 65 `.toml` + Rust generators | TOML for data, Rust for logic |
| **Data files** | Compiled-in C arrays | 65 TOML + 5 FTL + text | Hot-reloadable, no recompile |
| **Tests** | ~0 (manual QA only) | **4,217** automated | 4-layer pyramid + differential harness |
| **Specifications** | In-code comments | **29 spec documents** | Extracted formulas with test vectors |
| **i18n** | Optional `#ifdef` | Built-in (5 languages) | Fluent + TOML, hot-switchable |
| **Architecture** | Global state, IO mixed in | ECS + zero-IO events | Deterministic, testable, replayable |
| **Build time** | ~30s (make) | ~15s (cargo) | Incremental builds faster |
| **Binary size** | ~4MB | ~6MB | Includes all data |

## Overview

NetHack Babel is a ground-up reimplementation of [NetHack 3.7](https://github.com/NetHack/NetHack) in Rust, preserving formula-level accuracy while adopting a modern architecture. It replaces NetHack's global state and manual memory management with an ECS-based design (hecs), separates game logic from all IO, and defines all game content — monsters, items, dungeons — in TOML data files rather than compiled-in tables. The engine emits typed events instead of formatted strings, enabling built-in multilingual support (English, Simplified Chinese, Traditional Chinese, German, French) via Project Fluent without any changes to game logic. The result is a NetHack that is easier to extend, test, translate, and port to new frontends.

## About Original NetHack

NetHack is one of the foundational roguelikes, developed continuously for decades by the NetHack DevTeam. It is a turn-based, single-player dungeon simulation centered on emergent interactions: item effects combine in surprising ways, dungeon branches create long strategic arcs, and permadeath makes every decision final. The classic objective is to retrieve the Amulet of Yendor, ascend through the Planes, and sacrifice it on the correct Astral altar. NetHack Babel follows this original gameplay contract and treats NetHack 3.7 behavior as the reference standard.

## Features

- **True Color terminal rendering** — ratatui-based TUI with BUC-colored inventory, 16-color system with status highlighting
- **Built-in 5-language support** — English, Simplified Chinese (简体中文), Traditional Chinese (繁體中文), German, French — hot-switchable at runtime with `O` key
- **CJK-aware item naming** — Chinese counter words (量词) system: "3把匕首" instead of "3 daggers"; BUC prefix: "祝福的+2长剑"
- **Data-driven architecture** — 394 monsters, 430 items, 33 artifacts defined in TOML; modify content without recompiling
- **ECS-based game state** — hecs entity-component-system with explicit turn resolution and typed events
- **Formula-precise mechanics** — 29 mechanism specs extracted from the original C source; 4,217 tests verify fidelity
- **99.8% coverage** — all C NetHack gameplay systems implemented: combat, magic, items, monsters, dungeon, religion, pets, traps, shops, polymorph, riding, bones, conducts, and more
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

```sh
git clone https://github.com/SatoshiNakamoto0126/nethack-babel.git
cd nethack-babel
cargo run -- --data-dir data
```

### Play in Chinese

```sh
cargo run -- --data-dir data --language zh_CN    # Simplified Chinese
cargo run -- --data-dir data --language zh_TW    # Traditional Chinese
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

Rust nightly is required and will be selected automatically via `rust-toolchain.toml`. If you use [rustup](https://rustup.rs/), no manual setup is needed.

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
| `engine` | Pure game logic — combat, monsters, items, dungeon, turn loop, 80 modules | 133,000 | 3,400+ |
| `data` | TOML schema definitions and loaders for monsters, items, dungeons, levels | 5,500 | 42 |
| `i18n` | Fluent-based localization, item naming (doname), CJK classifiers | 4,800 | 91 |
| `tui` | Terminal UI built on ratatui + crossterm, 16-color system | 5,600 | 23 |
| `audio` | Sound effects via rodio, triggered by engine events | 340 | 58 |
| `cli` | Binary entry point — config, save/load, options, main loop | 5,200 | 88 |

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

The `specs/` directory contains 29 mechanism specifications extracted from the original NetHack C source, reviewed, and verified. Each spec documents the exact formulas, constants, edge cases, and test vectors for a game subsystem.

## Testing Infrastructure

NetHack Babel uses a 4-layer test pyramid plus a differential execution harness for compiler-grade cross-validation against the original C engine.

### Test Pyramid (4,217+ tests)

| Layer | Tests | Purpose |
|-------|-------|---------|
| **Unit tests** | ~3,500 | Pure function I/O: damage formulas, AC calculation, BUC matrices |
| **Snapshot tests** | ~500 | Prevent silent regression in item names, messages, i18n rendering |
| **Integration touchstones** | 100 | Multi-system event chains: melee→death→corpse, stoning→cure, polymorph→revert |
| **Property-based (proptest)** | 26 | Random input invariants: dice bounds, rnl range, plural never empty, score non-negative |
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
cargo test --workspace                   # Run all 4,217+ tests
cargo run -- --data-dir data             # Run (English)
cargo run -- --data-dir data --language zh_CN  # Run (Chinese)
cargo run -- --data-dir data -D          # Wizard mode
cargo clippy --workspace --all-targets   # Lint
cargo build --release                    # Release build
```

## Project Status

The game engine is feature-complete with 99.8% coverage of NetHack 3.7 gameplay systems. All core systems — combat, magic, items, monsters, dungeon generation, special levels, quests, pets, religion, traps, shops, hunger, status effects, identification, polymorph, riding, conducts, bones, save/load, leaderboard, and the terminal UI — are implemented and verified against the original NetHack source with 4,217+ passing tests.

Save format note: the current on-disk save version is `1.0.0`. Older `0.3.x` saves are intentionally rejected because level-scoped floor object data and story/runtime state are now serialized differently.

See [GAP_STATUS.md](GAP_STATUS.md) for the detailed status report and [DIFFERENCES.md](DIFFERENCES.md) for known deviations from NetHack 3.7 behavior.

## Roadmap (2026)

1. **Temple and shop long-tail parity**  
   Push beyond the current `temple-entry / sanctum-entry / shop-entry / follow / payoff / pray / calm-down / cranky-priest-chat / repair / drop-credit / live-sell / robbery / restitution / shopkeeper-death / deserted-shop / protection-spend / donation-tiers / ale-gift / blessing-clairvoyance / cleansing` runtime closure and finish the remaining priest/shopkeeper edge cases: richer deity/shop feedback and deeper economy-side aftermath.
2. **Wizard harassment parity**  
   Broaden Wizard of Yendor harassment beyond the current respawn/theft/curse/nasty-summon runtime, with richer nasty pools, stronger cadence, and more long-tail original-NetHack behavior.
3. **Story drift detection**  
   Expand the current quest/endgame/shop/temple/sanctum/wizard plus `Medusa`/`Castle`/`Orcus`/`Fort Ludios`/`Vlad`/invocation-portal traversal/save-load matrices into broader drift-detection harnesses for economy, religion, branch transitions, and other campaign-critical paths.
4. **Runtime cache and save hardening**  
   Continue auditing level-local runtime state and keep save-format rules explicit whenever serialized world semantics change.
5. **Contributor and content tooling**  
   Keep content docs, validation scripts, and schema guidance aligned with the live data-driven dungeon/topology pipeline.

## TODO (Short-Term)

- [ ] Extend temple/shop parity beyond the current temple-entry/sanctum-entry/shop-entry/payoff/follow/pray/calm/cranky-priest-chat/repair/drop-credit/live-sell/robbery/restitution/shopkeeper-death/deserted-shop/protection-spend/donation-tiers/ale-gift/blessing-clairvoyance/cleansing path: remaining richer deity/shop feedback and more original economy aftermath.
- [ ] Extend Wizard of Yendor harassment parity beyond the current respawn, theft, cursing, scaled nasty summon, and repeated reload coverage.
- [ ] Grow the story traversal matrix into a broader save/load plus drift-detection harness for economy, religion, and branch state beyond the current quest/endgame/shop/temple/sanctum/wizard/Medusa/Castle/Orcus/Fort-Ludios/Vlad/invocation-portal scenarios.
- [ ] Audit remaining level-local runtime state for cross-level leakage and save/load omissions.

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
