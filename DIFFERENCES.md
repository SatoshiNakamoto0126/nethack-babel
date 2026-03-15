# Architectural Differences: C NetHack 3.7 vs Rust Babel

## Purpose
This document is the authoritative record of intentional divergences from C NetHack.
When behavior differs, this is the "court of last resort" for determining if it's a bug or a design decision.

## Legend
- **ARCH**: Fundamental architecture change (language, framework, data format)
- **IMPL**: Implementation detail differs (same behavior, different code path)
- **DELTA**: Intentional behavior change (documented reason)
- **TODO**: Known gap, will be closed

## Divergences

### Architecture (ARCH)
| Feature | C NetHack | Rust Babel | Type | Reason |
|---------|-----------|------------|------|--------|
| Language | C (~290K LOC) | Rust (~132K LOC), nightly with `gen_blocks` | ARCH | Memory safety, algebraic types, trait-based polymorphism; gen blocks for lazy iterator APIs |
| State management | Global structs (ga-gz, sva-svy) via macros (`gm.moves`, `svl.level`) | hecs ECS components (`HitPoints`, `Positioned`, etc.) in `GameWorld` | ARCH | Eliminates implicit coupling, simplifies save/load, no manual reinit between games |
| Engine I/O | IO mixed in game logic (`pline()`, file reads, save writes) | Zero-IO engine, `Vec<EngineEvent>` / `impl Iterator<Item = EngineEvent>` | ARCH | Headless testing, replay recording, alternate frontends |
| RNG | Global `rn2()`/`rnd()`/`rnl()` shared by game and UI | Explicit `&mut impl Rng`, no global RNG | ARCH | Deterministic replay from seed; same inputs + same seed = same game |
| Data definition | C arrays/macros (`objects[]`, `mons[]`), recompile to change | TOML files under `data/`, parsed at load time with validation | ARCH | No recompile for content changes; type-checked schema at load time |
| Data validation | Runtime `switch` tables (`sbon()`, `dbon()`, `newuexp()`) | `const fn` compile-time tables with const assertions in `const_tables.rs` | ARCH | Eliminates wrong-table-value bugs; zero runtime cost |
| Level scripts | Embedded Lua interpreter (131 `.lua` files in `dat/`) | TOML + hand-coded Rust in `special_levels.rs` | ARCH | No C dependency (mlua); type-safe, testable; scripting API may come later |
| Save format | Custom portable SF with field-level serialization, `sfctool`/`sfexpasc`/`sftags` | bincode + `NBSV` header (magic, version tuple, save reason), serde derive | ARCH | Compact, fast, automatic derive; not interchangeable with C saves |
| I18N | Lua language packs, `nhgettext()` hash lookup, `i18n_grammar.c` dispatch | Project Fluent (`.ftl`) via `fluent` crate, `MessageComposer` | ARCH | Fluent handles pluralization/gender natively; robust tooling |
| Window system | tty/curses/X11/Qt/win32 (5 window ports) | ratatui + crossterm (single TUI) | ARCH | Single cross-platform terminal UI with Unicode and true color |
| Build system | Makefiles + `setup.sh` + hints files | Cargo workspace | ARCH | Standard Rust toolchain, no setup step |

### Implementation (IMPL)
| Feature | C NetHack | Rust Babel | Type | Reason |
|---------|-----------|------------|------|--------|
| Melee combat formulas | `uhitm.c`/`weapon.c` (`sbon()`/`dbon()`, skill bonuses, two-weapon, encumbrance) | Exact same formulas; compile-time lookup tables verified via const assertions | IMPL | Formula-level accuracy verified against C reference values |
| Ranged combat | `dothrow.c`/`mthrowu.c` (throwing, launchers, bounce logic) | Same core formulas; ray tracing uses same bounce-off-wall logic | IMPL | Core mechanics match; some ammo breakage edge cases simplified |
| FOV algorithm | Custom `vision.c` | Recursive shadowcasting (Bjorn Bergstrom / roguebasin) with relaxed Euclidean radius | IMPL | Same tiles +/-1 at edges; similar diagonal vision feel |
| Map generation | `mklev.c` with `smeq[]`, retry logic, corridor routing | Rooms-and-corridors (same broad logic, different retry/routing) | IMPL | Similar feel, not byte-identical; room counts (3-8), sizes, connectivity match |
| Monster AI | Scattered across `monmove.c`/`muse.c`/`mhitm.c`/`mhitu.c` (thousands of lines) | Multi-phase decision tree: covetous, defensive, flee, offensive, pickup, ranged, melee, pursue, doors | IMPL | Recognizable but centralized; covetous monsters teleport/steal correctly |
| Monster intelligence | Granular flags (`M1_ANIMAL`, `M1_MINDLESS`, `M1_HUMANOID`, etc.) | Three tiers (Animal, Humanoid, Intelligent) gating available behaviors | IMPL | Captures essential differences with less flag-checking boilerplate |
| Movement points | `u_calc_moveamt()`/`mcalcmove()` with stochastic rounding | Same formulas; integer division rounding may differ +/-1 due to Rust vs C semantics | IMPL | Speed-15 monsters may drift +/-1 turn over thousands of turns |
| Choking survival | 5% (1/20) survival; breathless immune; strangled blocks roll | Identical: `CHOKING_SURVIVAL_DENOM = 20`; verified by 10K-trial statistical test | IMPL | Direct translation |
| Pet AI/loyalty | `dogmove.c`/`dog.c`: loyalty, hunger, food appraisal, leash, multi-goal AI | `PetState` component with matching loyalty decay, training, food appraisal, leash | IMPL | Minor pathfinding differences possible |
| Trap system | All trap types with placement, detection, avoidance, triggering, escape | All trap types defined; placement, detection, avoidance, triggering, escape match | IMPL | One of the more complete subsystems |
| Potion/scroll/wand effects | All types with BUC-dependent effects | All 26 potions, 23 scrolls, all wands with BUC-dependent effects match | IMPL | Direct effects complete |
| Conduct tracking | 13 standard conducts, binary flags, per-action counting | 13 standard + Elberethless bonus; per-action counting; score formula matches | IMPL | Extra conduct is additive, not a divergence |
| Prayer/sacrifice | `pray.c`: alignment, prayer timeout, god anger, crowning, altar interactions | Alignment, prayer, sacrifice, crowning, luck mechanics match; prayer timeout/anger/crowning thresholds match | IMPL | Core prayer complete |

### Behavior Changes (DELTA)
| Feature | C NetHack | Rust Babel | Type | Reason |
|---------|-----------|------------|------|--------|
| Door luck adjustment | `rnl()` (luck-adjusted random) biases door success | Uniform rolls; luck not applied to doors | DELTA | Luck integration in progress; doors will be updated when complete |
| Score formula | `4*exp + rexp + gold` (simplified) | `4*exp + rexp + gold + 1000*artifacts + 5000*conducts + 50000*ascension` | DELTA | Extended formula rewards more dimensions of play |
| Gen blocks (nightly) | N/A (C) | Requires Rust nightly for `gen_blocks` feature | DELTA | Vec-returning stable fallbacks exist (`resolve_turn`, `trace_ray`, `FovMap::compute`) |

### Known Gaps (TODO)
| Feature | C NetHack | Rust Babel | Type | ETA |
|---------|-----------|------------|------|-----|
| Special levels | Dozens (Quest, Gehennom, Vlad, Castle, Medusa, Fort Ludios, Planes) | Sokoban, Gnomish Mines, Oracle only | TODO | Incremental |
| Quest system | 13 role-specific quest branches with leaders, nemeses, artifacts, text | Not implemented; quest artifacts defined but unobtainable via quest | TODO | Depends on special levels, alignment, role-specific NPCs |
| Lua level loading | Runtime `.lua` level definitions; moddable | No Lua interpreter; levels are Rust functions | TODO | Scripting API may be added later |
| Compound item interactions | Dipping, mixing potions, alchemy | Not implemented | TODO | After core items stable |
| Shop price markup | 25% random markup on unidentified items | Not implemented; simplified charisma table | TODO | Shop system refinement |
| Price ID UI | Shop prices narrow item identity | Backend `try_price_id()` exists; not exposed in UI | TODO | UI integration |
| Altar interactions | Converting altars, high-altar effects | Not implemented | TODO | After core prayer |
| Monster spellcasting | Full `buzzmu()` equivalent | Limited monster casting | TODO | Incremental |
| Vault guard AI | Full guard spawn/interaction on vault entry | Event only, not wired | TODO | Next session |
| Polymorph trap | Calls `polyself` | Event only, not wired | TODO | Next session |
| Minion spawning | Creates entity on prayer/sacrifice | Event only | TODO | Next session |
| Wizard mode | Full debug suite | 3/8 commands functional | TODO | Incremental |
| Shopkeeper AI | Full shopkeeper special behaviors | Partially implemented | TODO | Incremental |
| Monster equipping | Monsters equip found armor/weapons | Simplified | TODO | Incremental |
