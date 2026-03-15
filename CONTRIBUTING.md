# Contributing to NetHack Babel

Thank you for your interest in contributing to NetHack Babel. This guide covers how to build, test, and submit changes.

## Prerequisites

- **Rust 2024 edition** (1.85+) via [rustup](https://rustup.rs/)
- **Git** for version control
- **cargo-deny** (optional, for license auditing): `cargo install cargo-deny`

## Building

```sh
# Clone the repository
git clone https://github.com/user/nethack-babel.git
cd nethack-babel

# Build the entire workspace
cargo build

# Build in release mode
cargo build --release

# Run the game (text-mode fallback)
cargo run

# Run with options
cargo run -- --data-dir data --language en -u Hero
```

## Testing

```sh
# Run all workspace tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p nethack-babel-engine

# Run a specific test by name
cargo test -p nethack-babel-engine melee_hit

# Run tests with output visible
cargo test --workspace -- --nocapture
```

Tests are organized by crate:
- **engine** tests: unit tests for every game system (combat, movement, hunger, etc.), located inline in each `src/*.rs` file
- **data** tests: schema validation and TOML loading tests
- **integration** tests: in `tests/` (placeholder, to be populated)
- **formula** tests: test vectors validating specific NetHack formulas in `tests/formula/`

## Code Style

NetHack Babel follows the Rust ecosystem conventions with some project-specific guidelines:

### Formatting

- Run `cargo fmt` before committing. The CI pipeline enforces this.
- 4-space indentation, no tabs.
- Maximum line length: soft limit of 100 characters (rustfmt default).

### Naming

- Types: `PascalCase` (`GameWorld`, `EngineEvent`)
- Functions and methods: `snake_case` (`resolve_turn`, `grant_movement`)
- Constants: `SCREAMING_SNAKE_CASE` (`NORMAL_SPEED`, `BOLT_LIM`)
- Modules: `snake_case` matching the file name

### Documentation

- All public items should have a doc comment (`///` or `//!`).
- Module-level comments use `//!` at the top of the file.
- Reference the NetHack source file or spec when implementing a specific formula (e.g., `/// Matches NetHack's u_calc_moveamt()`).

### Engine Conventions

- **Zero IO in the engine crate.** The `engine` crate must never perform file IO, print to stdout, or access the filesystem. All output flows through `Vec<EngineEvent>`.
- **Explicit RNG.** Every function that needs randomness takes `&mut impl Rng` as a parameter. Never use a global RNG.
- **Pure functions where possible.** Prefer functions that take `&GameWorld` and return results over methods that mutate state implicitly.
- **Collect-then-apply pattern.** When iterating over ECS entities and mutating them, collect the data you need first, then apply mutations in a second pass. This avoids borrow-checker issues with hecs.

### Linting

```sh
# Run clippy with all targets
cargo clippy --workspace --all-targets

# Check license compliance
cargo deny check licenses
```

## PR Workflow

1. **Fork and branch.** Create a feature branch from `main`.
2. **Make your changes.** Follow the code style above.
3. **Test.** Run `cargo test --workspace` and `cargo clippy --workspace`.
4. **Commit.** Write clear commit messages describing what changed and why.
5. **Open a PR.** Target the `main` branch. Include:
   - A summary of what the PR does
   - Which NetHack subsystem it implements or modifies
   - Any known deviations from NetHack 3.7 behavior (add to `DIFFERENCES.md`)
6. **Review.** Address feedback. Keep commits clean (squash fixups).

## How to Add New Monsters

Monster definitions live in TOML files under `data/monsters/`.

1. Open `data/monsters/base.toml`.
2. Add a new `[[monster]]` entry at the end (or at the appropriate position by ID):

```toml
[[monster]]
id = 384
name = "new monster"
symbol = "N"
color = "Red"
base_level = 5
speed = 12
armor_class = 5
magic_resistance = 0
alignment = 0
difficulty = 6
generation_flags = ["Genocidable"]
frequency = 2
corpse_weight = 400
corpse_nutrition = 100
sound = "Growl"
size = "Medium"
flags = ["Hostile", "Carnivore"]

[[monster.attacks]]
type = "Claw"
damage_type = "Physical"
dice = "1d6"
```

3. The `id` must be unique and sequential.
4. Run `cargo test -p nethack-babel-data` to verify the file parses correctly.
5. If the monster has special behavior, implement it in `crates/engine/src/monster_ai.rs`.

## How to Add New Items

Item definitions live in TOML files under `data/items/`, organized by object class:

- `weapons.toml`, `armor.toml`, `potions.toml`, `scrolls.toml`, `rings.toml`, `wands.toml`, `amulets.toml`, `tools.toml`, `food.toml`, `gems.toml`, `spellbooks.toml`

1. Open the appropriate file for the item's class.
2. Add a new entry following the existing format.
3. Ensure the `id` is unique across all item files.
4. If the item has a use effect, implement the effect in the corresponding engine module (`potions.rs`, `scrolls.rs`, `wands.rs`, etc.).
5. Run `cargo test -p nethack-babel-data` to validate.

## How to Add Translations

Translations use [Project Fluent](https://projectfluent.org/) (`.ftl` files).

### Adding a new language

1. Create a new directory under `data/locale/` with the language code:
   ```
   data/locale/ja/
   ```

2. Create `manifest.toml`:
   ```toml
   [language]
   code = "ja"
   name = "Japanese"
   native_name = "日本語"
   ```

3. Create `messages.ftl` with translations:
   ```ftl
   # messages.ftl - Japanese translations
   welcome = ダンジョンへようこそ！
   you-hit = { $target }に当たった！
   you-miss = { $target }を外した。
   ```

4. For CJK languages, create `classifiers.toml` if the language uses measure words/classifiers:
   ```toml
   [classifiers]
   default = "個"
   weapon = "把"
   scroll = "巻"
   ```

### Translation keys

- Message keys correspond to `EngineEvent` variants.
- The `MessageComposer` in `crates/i18n/src/composer.rs` maps events to Fluent message IDs.
- Variables like `$target`, `$damage`, `$amount` are passed as Fluent arguments.

### Testing translations

```sh
# Run with a specific language
cargo run -- --language ja

# Verify Fluent files parse correctly
cargo test -p nethack-babel-i18n
```

## How to Add New Game Mechanics

New game mechanics go in the engine crate (`crates/engine/src/`).

### Module pattern

Each subsystem is a module following this pattern:

1. **Create a new file** `crates/engine/src/my_system.rs`.

2. **Add the module** to `crates/engine/src/lib.rs`:
   ```rust
   pub mod my_system;
   ```

3. **Structure the module**:
   ```rust
   //! Brief description of the subsystem.
   //!
   //! Reference: `specs/my-system.md`

   use rand::Rng;
   use crate::event::EngineEvent;
   use crate::world::GameWorld;

   // --- Constants (matching NetHack source) ---

   const SOME_THRESHOLD: i32 = 42;

   // --- Public API ---

   /// Resolve the main action for this subsystem.
   pub fn resolve_action(
       world: &mut GameWorld,
       rng: &mut impl Rng,
   ) -> Vec<EngineEvent> {
       let mut events = Vec::new();
       // ... implementation ...
       events
   }

   // --- Internal helpers ---

   fn helper_function() { /* ... */ }

   // --- Tests ---

   #[cfg(test)]
   mod tests {
       use super::*;
       // ... test functions ...
   }
   ```

4. **Wire it into the turn loop** in `crates/engine/src/turn.rs` if the mechanic runs every turn.

5. **Add events** to `crates/engine/src/event.rs` if the mechanic produces new event types.

6. **Add translations** for any new messages in `data/locale/en/messages.ftl`.

### Key design rules

- **No IO in the engine.** Return `Vec<EngineEvent>` instead of printing.
- **No global RNG.** Accept `&mut impl Rng` as a parameter.
- **Test with deterministic seeds.** Use `Pcg64::seed_from_u64(N)` in tests.
- **Document NetHack reference.** Note which C source file and function the implementation mirrors.
- **Update DIFFERENCES.md** if the behavior deviates from NetHack 3.7.

## License

By contributing, you agree that your contributions will be licensed under the NetHack General Public License (NGPL). All third-party dependencies must be MIT, Apache-2.0, BSD, or LGPL licensed (enforced by `cargo-deny`).
