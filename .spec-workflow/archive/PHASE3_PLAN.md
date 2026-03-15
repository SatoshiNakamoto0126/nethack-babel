# Phase 3: Gap Closure Implementation Plan

**Baseline**: 1,269 tests, ~60K LOC, 10/10 touchstone scenarios passing
**Goal**: Close the ~115K LOC gap to make the game fully playable
**Approach**: 5 sub-phases, each with 4 parallel agents, state checkpoint after each

---

## Phase 3A: Equipment + Inventory + Save (Unblocks Playability)

**Without this phase, players cannot interact with items at all.**

### Agent 3A-1: Equipment System
**Files**: `crates/engine/src/equipment.rs` (new), `crates/engine/src/world.rs`
- Multi-slot equipment: Weapon, OffHand, Helmet, Cloak, BodyArmor, Shield, Gloves, Boots, Ring_L, Ring_R, Amulet
- `EquipmentSlots` component on player entity
- `equip_item()` / `unequip_item()` — slot validation, two-hand weapon logic
- Cursed equipment: cannot remove, -1 enchant sticks
- Wield/Wear/TakeOff/PutOn/Remove action handlers in turn.rs
- AC recalculation from worn armor
- Equipment bonuses applied to combat (weapon damage dice, armor AC)
- ~15 tests

### Agent 3A-2: Inventory System
**Files**: `crates/engine/src/inventory.rs` (new), `crates/engine/src/turn.rs`
- `Inventory` component: Vec<Entity> with 52-slot limit (a-zA-Z)
- `add_to_inventory()` / `remove_from_inventory()` — letter assignment, merging stackable items
- PickUp action: prompt item selection when multiple items on floor
- Drop action: remove from inventory, place on floor
- ViewInventory action: return inventory listing as events
- Autopickup: filter by configured item classes
- ~15 tests

### Agent 3A-3: Save/Load Wiring
**Files**: `crates/cli/src/save.rs`, `crates/cli/src/main.rs`
- Wire SaveAndQuit → actual save_game() call
- Wire game startup → check for save file → restore_game()
- Save: serialize GameWorld (ECS snapshot), dungeon cache, turn counter
- Restore: deserialize, rebuild ECS world, resume
- Anti-savescum: delete save file on successful load
- ~8 tests

### Agent 3A-4: Item Interaction Wiring
**Files**: `crates/engine/src/turn.rs`, `crates/tui/src/app.rs`
- Wire 'd' key → item selection prompt → Drop action
- Wire 'w' key → weapon selection prompt → Wield action
- Wire 'W' key → armor selection prompt → Wear action
- Wire 'T'/'R' key → equipped item selection → TakeOff/Remove
- Wire 'z' key → wand selection prompt → ZapWand action
- Wire 'q'/'e'/'r' with item selection when item=None
- Item selection menu via WindowPort::select_item()
- ~10 tests

**Phase 3A exit criteria**: Player can pick up items, view inventory, equip/unequip, save/load game. ~48 new tests.

---

## Phase 3B: Role/Race + Status Enforcement

### Agent 3B-1: Role & Race System
**Files**: `crates/engine/src/role.rs` (new), `crates/data/`
- Role enum: Archeologist, Barbarian, Caveperson, Healer, Knight, Monk, Priest, Ranger, Rogue, Samurai, Tourist, Valkyrie, Wizard
- Race enum: Human, Elf, Dwarf, Gnome, Orc
- Starting inventory per role (from u_init.c)
- Starting attributes per role/race
- Skill restrictions per role
- Racial abilities (elf sleep resist, orc poison resist, etc.)
- Role titles by level
- ~20 tests

### Agent 3B-2: Status Effect Enforcement
**Files**: `crates/engine/src/status.rs`, `crates/engine/src/turn.rs`, `crates/engine/src/fov.rs`
- Blindness → restrict FOV to radius 0 (only feel adjacent), enable telepathy if intrinsic
- Confusion → randomize movement direction (already has maybe_confuse_direction, wire it)
- Stun → reduce to-hit, random movement
- Hallucination → scramble monster/item display names
- Levitation → block floor trap triggers, block stair use (unless controlled), no pickup from floor
- Paralysis → skip player turn entirely
- ~15 tests

### Agent 3B-3: Attribute System
**Files**: `crates/engine/src/attributes.rs` (new)
- exercise/abuse system (STR/DEX/CON/INT/WIS/CHA grow with use)
- Attribute caps per race
- Attribute drain (from monsters) and restore (from potion/prayer)
- Strength 18/xx for fighters
- Attribute effects on gameplay (CHA on shop prices already done, STR/DEX on combat done)
- ~12 tests

### Agent 3B-4: Game Start Flow
**Files**: `crates/cli/src/main.rs`, `crates/tui/src/app.rs`
- Role/race selection menu at game start
- Generate starting inventory based on role
- Set starting attributes based on role+race
- Set starting pet (cat/dog based on role)
- Set alignment based on role
- ~8 tests

**Phase 3B exit criteria**: Player selects Valkyrie/Human, starts with long sword + shield, blindness/confusion actually affect gameplay. ~55 new tests.

---

## Phase 3C: Spells + Tools + Engraving + Dip

### Agent 3C-1: Spellcasting System
**Files**: `crates/engine/src/spells.rs` (new)
- SpellBook component: known spells with memory decay timer
- Learn spell by reading spellbook (takes turns, intelligence check)
- Cast spell: select from known spells, direction if applicable
- Spell effects: force bolt, healing, detect monsters, identify, fireball, cone of cold, sleep, magic missile, etc.
- Spell failure rate based on role + armor penalty
- ~20 tests

### Agent 3C-2: Tool Use (apply.c)
**Files**: `crates/engine/src/tools.rs` (new)
- Unicorn horn: cure confusion/stun/hallucination/blind (already have function, wire to Apply)
- Stethoscope: detect monster HP through walls
- Pick-axe/mattock: dig walls/floors
- Key/lockpick/credit card: unlock doors/chests
- Lamp/lantern: light source toggle
- Tinning kit: create tins from corpses
- Crystal ball: intelligence-based divination
- Camera: blind adjacent monsters
- Mirror: reflect gaze attacks
- Whistle: call pets
- ~25 tests

### Agent 3C-3: Engraving System
**Files**: `crates/engine/src/engrave.rs` (new)
- Engrave text on floor (dust/blade/fire/lightning)
- Durability: dust=1 step, blade=5 steps, fire/lightning=permanent
- Elbereth: scares most monsters (check before monster attacks)
- Monster Elbereth avoidance in AI
- Conduct tracking (Elberethless)
- ~12 tests

### Agent 3C-4: Dipping + Alchemy
**Files**: `crates/engine/src/dip.rs` (new)
- Dip item into potion → alchemy (potion combinations)
- Dip into fountain → Excalibur (lawful+level5+longsword) — wire existing function
- Dip into holy water → bless item
- Dip weapon into potion of sickness → poison weapon
- Alchemy recipes (from spec)
- ~15 tests

**Phase 3C exit criteria**: Player can cast spells, use tools, engrave Elbereth, create holy water. ~72 new tests.

---

## Phase 3D: Dungeon Branches + Monster Full Attacks

### Agent 3D-1: Special Level Generation
**Files**: `crates/engine/src/special_levels.rs`, `crates/engine/src/map_gen.rs`
- Gnomish Mines levels (3-5 variants)
- Minetown layout (with shops, temple)
- Sokoban puzzles (4 levels × 2 variants each)
- Oracle level
- Big Room
- The Castle (with drawbridge)
- Medusa's Island (2 variants)

### Agent 3D-2: Gehennom + Endgame
**Files**: `crates/engine/src/special_levels.rs`, `crates/engine/src/dungeon.rs`
- Maze generation for Gehennom
- Vlad's Tower (3 levels)
- Wizard's Tower
- Sanctum
- Elemental Planes (Earth, Air, Fire, Water)
- Astral Plane (3 altars)

### Agent 3D-3: Monster Full Attack Types
**Files**: `crates/engine/src/combat.rs`, `crates/engine/src/monster_ai.rs`
- Engulf (swallow): enter u.uswallow state, internal damage per turn, escape check
- Breath weapon: fire/cold/lightning/acid/poison/disintegration/sleep
- Active gaze (Medusa stone gaze with reflection)
- Touch attacks (cockatrice stone touch, lich level drain)
- Sting attacks (poison, disease)
- Monster ranged: throw items at player (mthrowu.c)
- Monster spellcasting: buzz spells at player (mcastu.c)

### Agent 3D-4: Dungeon Branch Transitions
**Files**: `crates/engine/src/dungeon.rs`, `crates/engine/src/turn.rs`
- Branch entrance detection (stairs/portal to Mines at depth 3-5, etc.)
- Branch-specific monster generation
- Portal mechanics (Fort Ludios, Quest)
- Magic portal for endgame transition
- Level feeling messages ("You have a strange feeling...")

**Phase 3D exit criteria**: Full dungeon from level 1 to Astral Plane traversable, monsters use full attack repertoire. ~60 new tests.

---

## Phase 3E: Polish & Remaining Systems

### Agent 3E-1: Environment Interactions
- Fountain: quaff (wishes, water moccasins, stat changes), dip (Excalibur wired)
- Throne: sit (wish, stat change, genocide, throne room effects)
- Sink: quaff, kick (ring, foocubus, pudding)
- Kicking: doors (break/force), monsters, items

### Agent 3E-2: Lock/Dig/Container
- Lock picking: keys, lockpicks, credit card, door/chest unlock
- Digging: pick-axe through walls/floors, wand of digging
- Containers: bags, chests — put in, take out, bag of holding, bag of tricks

### Agent 3E-3: Polymorph + Teleport + Ride
- Polymorph self: form selection, attribute changes, timeout, system shock
- Controlled teleportation: position selection UI
- Level teleport: depth selection
- Riding: mount/dismount, mounted combat, steed management

### Agent 3E-4: Quest + NPC + Display
- Quest system: leader, nemesis, quest artifact
- Priest NPC: temple interactions, protection
- Vault guard: gold deposit dialogue
- Wizard harassment: double trouble, steal amulet
- Status hilites on status bar
- Options system expansion

**Phase 3E exit criteria**: All major C systems have Rust equivalents. Remaining gaps are cosmetic or rare edge cases.

---

## Execution Protocol

1. Before each phase: save state to `SESSION_CHECKPOINT_3X.md`
2. Run `/compact` between phases
3. After each phase: `cargo test --workspace`, record count
4. After each phase: update `PHASE3_PLAN.md` with results
5. Use worktree isolation for agents when possible to avoid conflicts

## Expected Growth

| Phase | Tests (projected) | LOC (projected) |
|-------|------------------|-----------------|
| 3A | ~1,320 | ~65K |
| 3B | ~1,375 | ~70K |
| 3C | ~1,447 | ~77K |
| 3D | ~1,507 | ~85K |
| 3E | ~1,570 | ~92K |
