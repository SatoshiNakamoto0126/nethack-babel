# CLAUDE.md

This file provides guidance to Claude Code when working with the NetHack Babel codebase.

## Project Overview

NetHack Babel — a Rust reimplementation of NetHack 3.7 with native i18n support. 6-crate workspace, ~133K LOC Rust, 3,984 tests. Aligns to original NetHack behavior using extracted specs from the C source at `/Users/hz/Downloads/NetHack/`.

## Environment

- **Rust nightly** required (`rust-toolchain.toml` pins the version)
- macOS / Linux / WSL2
- No external C dependencies (pure Rust, no mlua)

## Commands

```bash
# Build
cargo build

# Run (TUI mode, default English)
cargo run -- --data-dir data

# Run with Chinese locale
cargo run -- --data-dir data --language zh_CN

# Run in wizard mode (debug — create monsters, wish, teleport)
cargo run -- --data-dir data -D

# Test (full workspace)
cargo test --workspace

# Test single crate
cargo test -p nethack-babel-engine
cargo test -p nethack-babel-i18n

# Test single function
cargo test -p nethack-babel-engine -- test_combat_monk_armor_penalty

# Lint
cargo clippy --workspace -- -D warnings
```

## Workspace Layout

```
crates/
  engine/    — Game logic (80 source files, ~133K LOC):
               combat, monsters, items, dungeon, turn loop, spells, potions,
               scrolls, wands, traps, equipment, polyself, steed, religion,
               shop, hunger, status, pets, artifacts, explode, objnam,
               vault, minion, priest, pager, o_init, mondata, region, ...
  i18n/      — Localization: NounPhrase, doname(), Classifier, MessageComposer, LocaleManager
  data/      — TOML data loading: MonsterDef, ObjectDef, GameData, LevelDefinition
  tui/       — Terminal UI: App state, input mapping, WindowPort trait, ratatui, colors
  cli/       — Binary entry point: main loop, FOV, map view, event→message, save/load, config
  audio/     — Sound system (stub)

data/
  monsters/  — Monster definition TOML (394 monsters)
  items/     — Object definition TOML (430 items across 11 files)
  dungeons/  — Dungeon topology + special level TOML (gehennom/, sokoban/, bigroom/)
  quests/    — 13 role-specific quest TOML files
  content/   — Rumors, epitaphs, encyclopedia text
  help/      — Help text files (help.txt, cmdhelp.txt, opthelp.txt, history.txt)
  locale/
    en/      — English (manifest.toml, messages.ftl — 1,624 keys)
    zh-CN/   — Simplified Chinese
    zh-TW/   — Traditional Chinese
    de/      — German (partial)
    fr/      — French (partial)

specs/       — 29 extracted spec files from NetHack C source (formulas + test vectors)
tests/       — TEST_ORACLES.md (verification strategies), formula/, integration/, snapshots/
```

## Architecture

### ECS (hecs)
Game state uses `hecs::World` wrapped in `GameWorld`. Components: `Player`, `Monster`, `Positioned`, `HitPoints`, `Speed`, `Name`, `StatusEffects`, `Intrinsics`, `PlayerCombat`, `CreationOrder`, `AppearanceTable`, etc. Spawn via tuple bundles — hecs has a 16-component tuple limit; use `insert_one()` for overflow.

### Turn Loop
`resolve_turn()` in `turn.rs` is the main dispatcher. Takes `PlayerAction` + `&mut GameWorld` + `&mut Rng`, returns `Vec<EngineEvent>`. Per-turn systems called at new-turn boundary: HP/PW regen, hunger, status effect ticking (stoning/sliming/sickness countdowns), polymorph timer, light source fuel, gas cloud ticking, attribute exercise. Monster turns run after player action, sorted by speed desc then `CreationOrder` asc (Decision D2).

### i18n Pipeline
- **Fluent (.ftl)** for message templates (combat messages, UI strings)
- **TOML** for entity name translations (monsters.toml, objects.toml)
- **Classifier** for CJK counter words (classifiers.toml → `Classifier::load_from_toml()`)
- `LocaleManager` provides `translate()`, `translate_monster_name()`, `translate_object_name()`
- `NounPhrase` renders items differently for English vs CJK (articles vs classifiers)
- `doname()` / `doname_locale()` for item display names

### Event System
Engine functions return `Vec<EngineEvent>`. CLI layer converts events to display messages via `events_to_messages()` (needs `&GameWorld` for entity name lookup). No IO in the engine crate.

## Spec-First Development (Decision D5)

Specs live in `specs/*.md`. Each contains exact formulas extracted from NetHack C source + test vectors. When implementing a feature:
1. Read the relevant spec
2. Implement the formulas exactly (including known bugs, marked `[疑似 bug]`)
3. Write tests using the spec's test vectors
4. Test name format: `test_{system}_{feature}_{scenario}`

## Key Design Decisions

| # | Decision | Rule |
|---|----------|------|
| D1 | Message alignment | Combat/death/status messages: verbatim match original. Help/UI text: can differ. |
| D2 | Same-speed ordering | `CreationOrder(u64)` component, sort speed desc → creation asc |
| D3 | Level validation | Feature signature diff against original Lua levels |
| D4 | RNG verification | 10K-sample statistical tests for key probability events |
| D5 | Spec-first | Extract spec → write tests → implement |
| D6 | doname guard | `insta` snapshot tests for all object classes, CJK leak guard |

### D7: Testing Pyramid

Mandatory test categories:

- **Monte Carlo (statistical)**: Probability-dependent logic (combat hit rates, trap chances, loot drops) — 10K-sample tests, verify within 2σ of C formula.
- **Mock RNG (deterministic)**: ECS state machines (polymorph→revert, stoning countdown, hunger stages) — fixed RNG seeds, cover all state transitions.
- **Golden Master**: Capture C engine outputs as test oracles when feasible (see `tests/TEST_ORACLES.md`).
- **Property-based**: proptest/quickcheck for serialization roundtrips, item name parsing, map generation validity.

Target: 5,000+ tests (currently 3,984).

### D8: Entity Hard Limits

Prevent OOM/entity explosion:
- Maximum entities per level: 500 (monsters + items + effects)
- Monster generation capped at 120 per level
- Summoning spells must check entity count before creating
- If entity cap reached, log warning and reject creation
- Matches C NetHack's MAXMONNO and similar constants

## Parallel Agent File Ownership

When running multiple agents, partition by file to avoid conflicts:

| Owner | Files |
|-------|-------|
| Combat | `combat.rs`, `ranged.rs`, `mhitu.rs`, `mhitm.rs` |
| Items | `potions.rs`, `scrolls.rs`, `wands.rs`, `items.rs`, `mkobj.rs`, `objnam.rs` |
| Monsters | `monster_ai.rs`, `mcastu.rs`, `muse.rs`, `monmove.rs`, `makemon.rs`, `mondata.rs` |
| Dungeon | `map_gen.rs`, `dungeon.rs`, `traps.rs`, `special_levels.rs` |
| Systems | `hunger.rs`, `religion.rs`, `shop.rs`, `status.rs`, `attributes.rs` |
| NPCs | `pets.rs`, `vault.rs`, `minion.rs`, `priest.rs`, `npc.rs` |
| Mechanics | `polyself.rs`, `steed.rs`, `explode.rs`, `ball.rs`, `dbridge.rs`, `region.rs` |
| Actions | `action.rs`, `do_actions.rs`, `equipment.rs`, `apply.rs`, `dig.rs`, `lock.rs` |
| Display | `pager.rs`, `symbols.rs`, `o_init.rs`, `fov.rs`, `light.rs`, `detect.rs` |
| i18n | `i18n/*.rs` |
| TUI | `tui/*.rs` (including `colors.rs`) |
| CLI | `cli/*.rs` (including `config.rs`, `save.rs`, `game_start.rs`) |

`world.rs` and `turn.rs` are shared — minimize concurrent changes to these.

## Gotchas

- **classifiers.toml format**: Must use flat `[class]`/`[name]` sections, not hierarchical. `Classifier::load_from_toml()` expects `default = "个"` at top level.
- **hecs 16-tuple limit**: Player entity exceeds 16 components. Use `world.spawn(first_15)` then `world.insert_one(entity, extra)`.
- **`events_to_messages()` needs `&GameWorld`**: Entity names in events are `hecs::Entity` handles, not strings.
- **`doname()` creates a throwaway `LocaleManager`**: Use `doname_locale()` with an explicit locale.
- **Snapshot tests**: Run `INSTA_UPDATE=always cargo test -p nethack-babel-i18n` to update.
- **RNG in tests**: Use `Pcg64::seed_from_u64(N)` for deterministic tests. Different seeds per test function.
- **FTL multiline syntax**: Continuation lines MUST start with whitespace. Select expressions: ALL inner lines indented. A bare letter without `=` crashes the Fluent parser.
- **AppearanceTable**: `GameWorld::new()` creates an `AppearanceTable` from the RNG seed. Without this, all games show identical unidentified item descriptions.
- **Deserialization safety**: Size limits (>10MB reject), nesting depth limits (>32 reject), string length limits (>10K reject).
- **Cross-system event consumption**: `StatusApplied { Polymorphed }` must trigger `polyself::polymorph_self()`. Events without consumers are silent failures.
- **Steed + polymorph**: `polymorph_self()` must dismount first. `movement.rs` uses steed speed when mounted. `combat.rs` applies `mounted_combat_modifier()`.
- **Blind + scrolls**: `read_scroll()` caller must check blindness first — blind players cannot read.
- **Stunned/confused + wands**: Wand direction is randomized when stunned or confused.
