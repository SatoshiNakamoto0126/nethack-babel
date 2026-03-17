# Repository Guidelines

## Project Structure & Module Organization

This repository is a Rust workspace centered on `crates/`. The main crates are `crates/engine` (game logic), `crates/data` (TOML schemas/loaders), `crates/cli` (binary entry point), `crates/tui`, `crates/i18n`, and `crates/audio`. Runtime content lives under `data/` (`monsters/`, `items/`, `dungeons/`, `quests/`, `locale/`, `help/`). Specs and parity notes live in `specs/` and `doc/`. Tests live both in crate-local directories such as `crates/engine/tests` and top-level `tests/`.

## Build, Test, and Development Commands

- `cargo run -- --data-dir data` starts the game with the local data set.
- `cargo run -- --data-dir data --language zh_CN` runs with Simplified Chinese.
- `make build` builds the workspace in debug mode.
- `make check` runs fast type-checking with `cargo check --workspace`.
- `make test` runs the full test suite with `cargo test --workspace`.
- `cargo test -p nethack-babel-engine -- test_name` runs a focused engine test.
- `make clippy` runs CI-style linting with `cargo clippy --workspace -- -D warnings`.
- `make fmt` or `cargo fmt --all` formats the codebase.
- `cargo test -p nethack-babel-engine --test differential` runs the C-vs-Rust differential harness.

## Coding Style & Naming Conventions

Use Rust nightly and edition 2024. Format with `cargo fmt --all`; CI also expects clippy-clean code. Follow standard Rust naming: `snake_case` for modules/functions/tests, `CamelCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants. Keep `crates/engine` free of frontend IO; engine systems should return typed events (`Vec<EngineEvent>`), leaving rendering/audio to `cli`, `tui`, or `audio`.

## Testing Guidelines

Follow the repo’s spec-first flow: read `specs/*.md`, implement the exact formula/edge case, then add tests. Put unit tests next to the code they validate and use integration tests when behavior crosses modules. Name tests after behavior, e.g. `test_entering_medusa_spawns_medusa`. Run narrow tests first, then `make test`.

## Commit & Pull Request Guidelines

Recent history favors short imperative subjects with scope prefixes: `feat(engine): ...`, `feat(cli): ...`, `docs: ...`. Keep commits focused by crate or subsystem. PRs should explain gameplay impact, list touched crates/data paths, note the commands you ran, and mention any parity or save-format risk. Include screenshots or replay evidence for TUI-visible changes.

## Architecture & Contributor Notes

Use `data/` as the canonical runtime content directory and avoid editing generated output in `target/`. `GameWorld` uses `hecs`, which has a 16-component tuple spawn limit; for larger entities, spawn the base bundle and add extras with `insert_one()`. For deterministic tests, prefer `Pcg64::seed_from_u64(...)`. If you touch localized naming or message rendering, check snapshot and locale-sensitive behavior, not just English output.
