# Architecture

Technical architecture overview for NetHack Babel.

## Workspace Structure

NetHack Babel is a Cargo workspace with six crates (~42,000 lines of Rust). Dependencies flow strictly downward:

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

| Crate    | Path             | Purpose |
|----------|------------------|---------|
| `data`   | `crates/data`    | TOML schema definitions, ECS component structs, compile-time const tables, and data file loaders. No game logic. |
| `engine` | `crates/engine`  | Pure game logic. Zero IO, zero filesystem access. Receives data definitions and player input, returns game events. Requires **Rust nightly** for `gen_blocks`. |
| `i18n`   | `crates/i18n`    | Fluent-based localization. Composes translated messages from engine events. |
| `tui`    | `crates/tui`     | Terminal UI built on ratatui + crossterm. Handles rendering and input. |
| `audio`  | `crates/audio`   | Sound effects via rodio. Triggered by engine events. |
| `cli`    | `crates/cli`     | Binary entry point. Config parsing, save/load, main loop orchestration. |

### Dependency Rules

- `data` depends on nothing (except serde, toml, bitflags).
- `engine` depends only on `data` (for type definitions).
- `i18n`, `tui`, `audio` depend on `engine` and `data`.
- `cli` depends on everything.
- **The engine never depends on any IO crate.** This is the central architectural invariant.

### Nightly Toolchain Requirement

The workspace requires Rust nightly, pinned via `rust-toolchain.toml`:

```toml
[toolchain]
channel = "nightly"
```

The only unstable feature used is `gen_blocks` (enabled in `crates/engine/src/lib.rs` via `#![feature(gen_blocks)]`). This feature allows writing lazy iterators with `gen { ... yield value; ... }` syntax instead of manual state machines. See "Gen Blocks" below for details.

## Data Flow

The game follows a strict unidirectional data flow:

```
 1. STARTUP
    cli loads TOML files (data crate)
      -> MonsterDef[], ObjectDef[], DungeonDef[]

 2. INITIALIZATION
    cli creates the game world
      -> engine::GameWorld::new(player_start)
    cli generates the first level
      -> engine::map_gen::generate_level(depth, &mut rng)

 3. MAIN LOOP (each iteration)
    input    = tui::wait_for_input()           // blocks for player input
    action   = input::map_key(input)           // convert to PlayerAction
    events   = engine::resolve_turn(           // pure game logic
                 &mut world, action, &mut rng)
    messages = i18n::compose_all(&events)      // localize event messages
    tui::render(&world, &messages)             // draw the game state
    audio::play_events(&events)                // play sound effects

 4. SAVE/LOAD
    save::save_game(&world, path)              // serialize via bincode
    world = save::load_game(path)              // deserialize
```

Key property: the engine never calls into tui, i18n, or audio. It returns `Vec<EngineEvent>` (or `impl Iterator<Item = EngineEvent>` via gen blocks), and the callers decide what to do with them.

## ECS Component Model

Game state is stored as entities with typed components in a `hecs::World`, wrapped by `engine::world::GameWorld`.

### Player Entity

The player is a single entity with these components:

| Component          | Type                   | Purpose |
|--------------------|------------------------|---------|
| `Player`           | marker (unit struct)   | Identifies the player entity |
| `Positioned`       | `Position`             | Map coordinates |
| `HitPoints`        | `{ current, max }`    | Health |
| `Power`            | `{ current, max }`    | Mana |
| `ExperienceLevel`  | `u8`                   | Character level |
| `Attributes`       | 6 stats + str_extra    | STR/DEX/CON/INT/WIS/CHA |
| `ArmorClass`       | `i32`                  | Defense rating |
| `Speed`            | `u32`                  | Base movement speed |
| `MovementPoints`   | `i32`                  | Accumulated movement budget |
| `EncumbranceLevel` | enum                   | Weight-capacity tier |
| `HeroSpeedBonus`   | enum                   | Fast/VeryFast intrinsic |
| `Nutrition`        | `i32`                  | Hunger counter |
| `Name`             | `String`               | Display name |

### Monster Entities

Monsters share most components with the player but use `Monster` instead of `Player` as a marker. Pets additionally have the `Tame` marker and a `PetState` component.

### Item Entities

Items use components from the `data` crate:

| Component       | Purpose |
|-----------------|---------|
| `ObjectCore`    | Type ID, class, quantity, weight, inventory letter |
| `BucStatus`     | Blessed/uncursed/cursed state and bknown flag |
| `KnowledgeState`| Per-instance identification flags (known, dknown, rknown, etc.) |
| `ObjectLocation`| Where the item is (floor, inventory, monster inventory, etc.) |
| `Enchantment`   | +/- enchantment value (spe) |
| `Erosion`       | Rust/corrosion/fireproofing state |
| `ObjectExtra`   | Optional individual name, container contents |

### Component Access Pattern

```rust
// Reading a component
let hp = world.get_component::<HitPoints>(entity).unwrap();
println!("HP: {}/{}", hp.current, hp.max);

// Mutating a component
if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
    hp.current -= damage;
}

// Querying all entities with a component
for (entity, pos) in world.query::<Positioned>().iter() {
    // ...
}
```

## Event System

The engine communicates exclusively through `EngineEvent`, a flat enum with ~40 variants covering all observable game actions:

```
 EngineEvent
   Combat:      MeleeHit, MeleeMiss, RangedHit, RangedMiss, ExtraDamage, ...
   Items:       ItemPickedUp, ItemDropped, ItemWielded, ItemIdentified, ...
   State:       EntityMoved, EntityDied, HungerChange, LevelUp, HpChange, ...
   Environment: DoorOpened, DoorClosed, TrapTriggered, TrapRevealed, ...
   Dungeon:     LevelChanged, MonsterGenerated
   Messages:    Message { text: String }
   Control:     TurnEnd, GameOver
```

### Event Consumers

| Consumer    | What it does with events |
|-------------|--------------------------|
| `tui`       | Renders map changes, displays messages, plays animations |
| `i18n`      | Translates `Message` events and combat events into localized strings |
| `audio`     | Maps events to sound effects (e.g., `MeleeHit` -> sword clang) |
| `analytics` | Aggregates fine-grained events into `AnalyticsEvent` summaries |
| `save`      | Records events for replay (planned) |

### Analytics Layer

The `event::summarize()` function aggregates fine-grained events into higher-level `AnalyticsEvent` types (combat rounds, floor changes, quest progress) for statistics and replay.

## Gen Blocks

The engine uses Rust's unstable `gen_blocks` feature to provide **iterator-based API alternatives** alongside traditional Vec-returning functions. Gen blocks allow multi-phase game logic to be expressed as lazy iterators using `yield`, without writing manual state machine structs.

### Three Gen Block Functions

| Function | Module | Vec equivalent | Purpose |
|----------|--------|----------------|---------|
| `turn_events_gen()` | `turn.rs` | `resolve_turn()` | Yields events lazily as the turn resolves through player action, monster turns, and new-turn processing phases |
| `trace_ray_gen()` | `wands.rs` | `trace_ray()` | Yields each `RayCell` as a wand/spell ray propagates step by step, including bounces |
| `visible_cells_gen()` | `fov.rs` | `FovMap::compute()` | Yields `(x, y)` coordinates of visible cells lazily from shadowcasting |

### Example: Gen Block vs Vec API

```rust
// Vec-based: collects all events into a Vec, returns them at once
let events: Vec<EngineEvent> = resolve_turn(&mut world, action, &mut rng);

// Gen-block-based: yields events lazily as an iterator
let events_iter = turn_events_gen(&mut world, action, &mut rng);
for event in events_iter {
    // Process each event as it's produced
}
```

Both APIs produce identical output. The gen block variants are tested for equivalence against their Vec counterparts (e.g., `trace_ray_gen_matches_trace_ray`, `visible_cells_gen_matches_compute`).

### Why Gen Blocks

Turn resolution involves multiple sequential phases (player action, monster turns, end-of-turn processing), each of which can produce a variable number of events. Without gen blocks, expressing this as an iterator requires either:
1. Collecting all phases into a Vec first (losing laziness), or
2. Writing a complex manual state machine with an enum tracking which phase is active.

Gen blocks give the readability of imperative code with the laziness of iterators:

```rust
pub fn turn_events_gen<'a, R: Rng>(...) -> impl Iterator<Item = EngineEvent> + 'a {
    gen move {
        // Phase 1: player action
        for event in resolve_player_action(...) { yield event; }
        // Phase 2: monster turns
        for event in resolve_monster_turns(...) { yield event; }
        // Phase 3: end-of-turn
        for event in process_new_turn(...) { yield event; }
    }
}
```

## Const Tables

The `data` crate includes a `const_tables` module (`crates/data/src/const_tables.rs`) that computes critical lookup tables entirely at compile time using `const fn`. This eliminates runtime computation costs and catches formula errors before the binary is produced.

### Tables

| Table | Type | Source | Purpose |
|-------|------|--------|---------|
| `STR_TO_HIT` | `[i8; 126]` | `weapon.c:sbon()` | Strength to-hit bonus, indexed by encoded STR value |
| `STR_DAMAGE` | `[i8; 126]` | `weapon.c:dbon()` | Strength damage bonus, indexed by encoded STR value |
| `XP_THRESHOLDS` | `[i64; 30]` | `exper.c:newuexp()` | Cumulative XP needed to advance past each level |
| `is_terrain_passable()` | `const fn(u8) -> bool` | `rm.h` terrain flags | Whether a terrain type allows walking |
| `is_terrain_opaque()` | `const fn(u8) -> bool` | `rm.h` terrain flags | Whether a terrain type blocks line of sight |

### Compile-Time Validation

All tables include `const` assertions that verify correctness at compile time:

```rust
// Monotonicity: XP thresholds are strictly increasing
const _: () = {
    let mut i = 1;
    while i < 30 {
        assert!(XP_THRESHOLDS[i] > XP_THRESHOLDS[i - 1]);
        i += 1;
    }
};

// Spot checks against known values from NetHack source
const _: () = {
    assert!(XP_THRESHOLDS[1] == 20);
    assert!(XP_THRESHOLDS[10] == 10_000);
    assert!(XP_THRESHOLDS[29] == 100_000_000);
};

// Cross-table consistency
const _: () = {
    assert!(STR_TO_HIT[encode_strength(18, 100)] == 3);
    assert!(STR_DAMAGE[encode_strength(18, 100)] == 7);
};
```

If any assertion fails, the project does not compile. This is a stronger guarantee than runtime tests alone.

## Key Design Decisions

### 1. Collect-Apply Pattern

When the engine needs to iterate over entities and mutate them (e.g., granting movement points to all monsters), it follows a two-pass pattern:

```rust
// Pass 1: Collect data (immutable borrow)
let mut additions: Vec<(Entity, i32)> = Vec::new();
for (entity, speed) in world.ecs().query::<&Speed>().iter() {
    additions.push((entity, calculate_movement(speed.0)));
}

// Pass 2: Apply mutations (mutable borrow)
for (entity, add) in additions {
    if let Some(mut mp) = world.get_component_mut::<MovementPoints>(entity) {
        mp.0 += add;
    }
}
```

This avoids borrow-checker conflicts with hecs, where you cannot iterate and mutate simultaneously.

### 2. No Global RNG

Every function that needs randomness accepts `&mut impl Rng`. This enables:
- **Deterministic replay**: same seed + same inputs = same game
- **Reproducible tests**: `Pcg64::seed_from_u64(42)` gives identical test runs
- **Parallel testing**: no shared mutable state between tests

### 3. Zero-IO Engine

The engine crate has no dependencies on `std::fs`, `std::io`, or any rendering library. Benefits:
- **Headless testing**: thousands of turns can be simulated in milliseconds
- **Multiple frontends**: TUI, GUI, web, or headless server can all use the same engine
- **Save/load isolation**: serialization is handled by the `cli` crate, not the engine

### 4. Data-Driven Definitions

All game content (monsters, objects, dungeon parameters) is defined in TOML files under `data/`. The engine receives parsed `MonsterDef[]` and `ObjectDef[]` structs and never reads files directly.

Benefits:
- Content can be modified without recompiling
- Data validation happens at load time with clear error messages
- Testing can use synthetic definitions without touching the filesystem

### 5. Typed Events over String Messages

Instead of calling `pline("You hit the %s!", monster_name)` like NetHack, the engine emits typed events:

```rust
EngineEvent::MeleeHit {
    attacker: player_entity,
    defender: monster_entity,
    weapon: Some(sword_entity),
    damage: 8,
}
```

The `i18n` crate then formats this into a localized message. This separation means:
- The engine does not need to know the current language
- Messages can be formatted differently per frontend (terse for TTY, verbose for GUI)
- Events can be replayed, analyzed, or streamed without parsing text

### 6. Iterator-Based and Vec-Based APIs

For key subsystems, the engine provides both a Vec-returning function and an iterator-returning gen block function:

| Subsystem | Vec API | Iterator API (gen block) |
|-----------|---------|--------------------------|
| Turn resolution | `resolve_turn() -> Vec<EngineEvent>` | `turn_events_gen() -> impl Iterator<Item = EngineEvent>` |
| Ray tracing | `trace_ray() -> RayPath` | `trace_ray_gen() -> impl Iterator<Item = RayCell>` |
| FOV computation | `FovMap::compute()` (marks bitmap) | `visible_cells_gen() -> impl Iterator<Item = (i32, i32)>` |

The Vec APIs are simpler and used by most callers. The iterator APIs enable lazy processing (e.g., stopping ray propagation early when a condition is met, or streaming events to a network client without buffering).

## Engine Module Map (26 modules)

```
engine/
  lib.rs            -- crate root, module declarations, #![feature(gen_blocks)]
  action.rs         -- PlayerAction enum, Position, Direction types
  artifacts.rs      -- 33 artifact definitions, special attack/defense/invoke
  combat.rs         -- melee combat formulas (to-hit, damage, two-weapon)
  conduct.rs        -- 14 voluntary conducts, achievements, scoring, scoreboard
  dungeon.rs        -- terrain types, map cell, level map, dungeon state
  event.rs          -- EngineEvent enum (~40 variants), analytics aggregation
  fov.rs            -- recursive shadowcasting field-of-view, visible_cells_gen
  hunger.rs         -- nutrition tracking, eating, corpse effects, choking
  identification.rs -- BUC testing, appearance shuffling, price ID, display names
  items.rs          -- item spawn, pickup, drop, inventory management, stacking
  map_gen.rs        -- procedural room-and-corridor dungeon generation
  monster_ai.rs     -- multi-phase monster decision tree and behavior
  movement.rs       -- player movement, door interaction, diagonal squeeze
  pets.rs           -- taming, loyalty, pet AI, hunger, leash mechanics
  potions.rs        -- 26 potion type effects with BUC variants
  ranged.rs         -- throwing, launcher mechanics, projectile paths
  religion.rs       -- alignment, prayer, sacrifice, crowning, luck
  scrolls.rs        -- 23 scroll type effects with BUC variants
  shop.rs           -- pricing, shopkeeper behavior, transactions, theft
  special_levels.rs -- Sokoban, Mines, Oracle level generators
  traps.rs          -- trap placement, detection, triggering, escape
  turn.rs           -- main turn loop, movement points, regen, turn_events_gen
  wands.rs          -- wand zapping, ray propagation, wand breaking, trace_ray_gen
  world.rs          -- GameWorld struct, ECS component definitions

data/
  lib.rs            -- crate root
  components.rs     -- ECS component struct definitions
  const_tables.rs   -- compile-time STR bonus, XP, terrain lookup tables
  loader.rs         -- TOML file loading and validation
  schema.rs         -- MonsterDef, ObjectDef, DungeonDef type definitions
```

## Test Suite

The workspace contains **560 tests** across 33 source files. Tests cover:

- Combat formula accuracy against C source reference values
- Const table correctness (runtime tests complementing compile-time assertions)
- Gen block / Vec API equivalence (e.g., `trace_ray_gen_matches_trace_ray`)
- Hunger/choking mechanics with statistical validation (10,000-trial Monte Carlo)
- Monster AI decision priorities
- Save/load round-trip integrity
- Pet behavior and loyalty mechanics
- Artifact properties and special attacks

All tests are deterministic via explicit RNG seeding (`Pcg64::seed_from_u64(42)`).

## Data File Layout

```
data/
  monsters/
    base.toml       -- 383 monster species definitions
  items/
    weapons.toml    -- weapon definitions
    armor.toml      -- armor definitions
    potions.toml    -- potion definitions
    scrolls.toml    -- scroll definitions
    rings.toml      -- ring definitions
    wands.toml      -- wand definitions
    amulets.toml    -- amulet definitions
    tools.toml      -- tool definitions
    food.toml       -- food definitions
    gems.toml       -- gem/stone definitions
    spellbooks.toml -- spellbook definitions
  locale/
    en/
      manifest.toml -- language metadata
      messages.ftl  -- English message templates (Fluent)
    zh-CN/
      manifest.toml
      messages.ftl  -- Simplified Chinese translations
      classifiers.toml -- CJK measure word definitions
    zh-TW/
      manifest.toml
      messages.ftl  -- Traditional Chinese translations
```
