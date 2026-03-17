# Content Contributor Guide

This repository is now mostly data-driven. Treat the `data/` tree as the canonical source for gameplay content, and use Rust only when a mechanic cannot be expressed cleanly as data.

## 1. Know the source of truth

- Monsters: `data/monsters/**/*.toml`
- Objects: `data/objects/**/*.toml`
- Dungeon topology: `data/dungeons/dungeon_topology.toml`
- Embedded Gehennom special levels: `data/dungeons/gehennom/*.toml`
- Hand-coded special generators: `crates/engine/src/special_levels.rs`

Do not maintain two competing definitions for the same level. The loader now `include_str!()`s the data files directly, so editing the TOML changes the embedded runtime data too.

## 2. When adding or changing monsters

- Prefer canonical NetHack names. Runtime special-level population resolves by catalog name.
- Preserve gendered aliases when they matter (`name_male`, `name_female`), because quest and special-level wiring may depend on them.
- If you rename a monster, grep for its name in:
  - `crates/engine/src/turn.rs`
  - `crates/engine/src/special_levels.rs`
  - `crates/engine/src/quest.rs`

## 3. When adding or changing objects

- Keep the object name parser in mind: wishes, special-level population, and quest artifacts resolve against catalog names.
- Test any renamed object through at least one of:
  - `wizwish <object name>`
  - special-level population
  - quest artifact generation

## 4. When adding or changing special levels

- If the level is data-driven, update the TOML in `data/dungeons/**` first.
- If the level is hand-generated in Rust, update both topology/population logic and its census tests.
- For population changes, verify:
  - first entry spawns the expected boss/artifact
  - revisit does not duplicate population
  - save/load preserves the level state

## 5. Minimum validation matrix

Run these before sending a PR:

```sh
cargo check -p nethack-babel
CARGO_HOME=/tmp/cargo-home cargo test -p nethack-babel-data
CARGO_HOME=/tmp/cargo-home cargo test -p nethack-babel-engine 'special_levels::tests::test_special_level_population_census_key_levels_match_contracts' -- --exact
CARGO_HOME=/tmp/cargo-home cargo test -p nethack-babel-engine 'turn::tests::test_entering_'
CARGO_HOME=/tmp/cargo-home cargo test -p nethack-babel-engine 'turn::tests::test_revisiting_'
CARGO_HOME=/tmp/cargo-home cargo test -p nethack-babel 'save::tests::round_trip_loaded_'
```

For wizard/debug changes, also run:

```sh
CARGO_HOME=/tmp/cargo-home cargo test -p nethack-babel-engine 'turn::tests::wiz_'
```

## 6. Review bar

- No duplicate content definitions.
- No name drift between data files and runtime resolvers.
- No special-level actor regressions.
- No save/load regressions for touched content.
