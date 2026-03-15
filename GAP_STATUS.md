# NetHack Babel -- Remaster Status (2026-03-15)

## Current Metrics

- **Tests**: 3,729 passing (21 failures in i18n noun_phrase — WIP Fluent grammar)
- **Rust LOC**: 150K across 111 source files (120K code, 12K comments)
- **Engine LOC**: 132K across 80 modules
- **Data**: 11.6K lines TOML (monsters + items + dungeons + quests + locale)
- **Fluent Entries**: 1,623 English keys; zh-CN 1,140, zh-TW 1,140, de 492, fr 405
- **Coverage**: ~99% of C NetHack gameplay systems implemented

## Implemented Systems (Complete)

### Engine Modules by Size

| Module | LOC | C Equivalent | Notes |
|--------|-----|-------------|-------|
| special_levels.rs | 7,323 | sp_lev.c | All special levels: Castle, Medusa, Oracle, Sokoban (8), Mines, Gehennom, Planes, Astral, Quests, Big Room (9), Vlad, Wizard Tower |
| monster_ai.rs | 4,622 | mon.c, monst.c | Full AI decision tree, spellcasting, demon lords |
| potions.rs | 4,402 | potion.c | All potion effects, alchemy, BUC variants |
| combat.rs | 4,188 | uhitm.c, weapon.c | Melee/ranged/engulf/breath/gaze, 12 damage types, to-hit formulas |
| hunger.rs | 4,172 | eat.c | Corpse intrinsics (16+ types), tin mechanics, nutrition |
| map_gen.rs | 3,948 | mkroom.c, mkmaze.c | Room/maze/corridor generation, special rooms (13 types) |
| turn.rs | 3,791 | allmain.c | Game loop, action dispatch, speed system |
| wands.rs | 3,683 | zap.c | All wand types, beam bouncing, item interaction |
| scrolls.rs | 3,546 | read.c | All scroll effects, BUC variants, scroll of identify |
| religion.rs | 3,513 | pray.c, minion.c | Prayer, sacrifice, altar, crowning, minion summoning |
| identification.rs | 3,507 | -- | Multi-layer: appearance, BUC, enchantment, naming |
| traps.rs | 3,408 | trap.c | All trap types, player + monster interactions |
| shop.rs | 3,324 | shk.c | Pricing, charisma, theft, Kop spawning, shopkeeper AI |
| spells.rs | 3,167 | spell.c | 40 spell types, casting failure, spell memory |
| pets.rs | 2,835 | dog.c, dogmove.c | Loyalty, hunger, AI, combat, taming |
| equipment.rs | 2,792 | do_wear.c | 12 slots, 50+ intrinsic types from worn items |
| conduct.rs | 2,666 | -- | 13 standard + Elberethless |
| status.rs | 2,299 | timeout.c | 25+ timed effects with expiration callbacks |
| inventory.rs | 2,280 | invent.c | Container management, bag of holding, item stacking |
| artifacts.rs | 2,162 | artifact.c | 33 artifacts, invoke powers, alignment restrictions |
| ranged.rs | 2,095 | dothrow.c | Projectile mechanics, multishot, returning weapons |
| role.rs | 2,000 | role.c, u_init.c | 13 roles, 5 races, starting inventory, stat ranges |
| polyself.rs | 1,933 | polyself.c | Form abilities, system shock, armor breaking |
| dungeon.rs | 1,908 | dungeon.c | 8 branches, topology, level transitions |
| end.rs | 1,832 | end.c | Death handling, DYWYPI, score calculation |
| movement.rs | 1,754 | hack.c | Walk/run/travel, door interaction, diagonal rules |
| tools.rs | 1,651 | apply.c | All usable tools, horn/lamp/mirror/stethoscope/etc. |
| apply.rs | 1,635 | apply.c | Application dispatch for complex tools |
| muse.rs | 1,620 | muse.c | Monster item usage AI |
| mkobj.rs | 1,592 | mkobj.c | Object generation with frequency tables |
| npc.rs | 1,557 | sounds.c | NPC dialog, shopkeeper chat, quest leader |
| dip.rs | 1,547 | -- | Alchemy (4 recipes), Excalibur dip, poison coating |
| bones.rs | 1,467 | bones.c | Ghost generation, item degradation |
| monmove.rs | 1,383 | monmove.c | Monster pathfinding, fleeing, pursuit |
| do_actions.rs | 1,343 | do.c | Miscellaneous player actions |
| makemon.rs | 1,336 | makemon.c | Monster placement, difficulty scaling |
| quest.rs | 1,330 | quest.c, questpgr.c | 13 quest roles with dialog, leader, nemesis |
| topten.rs | 1,312 | topten.c | JSON leaderboard, top 100 |
| dig.rs | 1,312 | dig.c | Multi-turn digging, floor/wall/corridor, pickaxe |
| mhitu.rs | 1,246 | mhitu.c | Monster-hits-you attacks |
| attributes.rs | 1,245 | attrib.c | STR/DEX/CON/INT/WIS/CHA exercise and growth |
| region.rs | 1,200 | region.c | Gas clouds, poison regions |
| music.rs | 1,194 | music.c | All instruments, ambient sounds |
| teleport.rs | 1,151 | teleport.c | Random/controlled teleport, level teleport |
| environment.rs | 1,139 | fountain.c, sit.c | Fountain effects, throne, kick, containers |
| wish.rs | 1,119 | -- | Wish parsing, artifact restrictions |
| pickup.rs | 1,114 | pickup.c | Floor item pickup, autopickup |
| engrave.rs | 1,043 | engrave.c | Engraving types, Elbereth, degradation |
| items.rs | 1,036 | -- | Item property queries |
| detect.rs | 1,002 | detect.c | Monster/object/trap detection, scroll/potion/spell |
| mondata.rs | 928 | mondata.c | 60+ monster predicate functions |
| mhitm.rs | 890 | mhitm.c | Monster-vs-monster combat |
| steed.rs | 874 | steed.c | Mount/dismount, riding skill, steed AI |
| exper.rs | 855 | exper.c | XP calculation, level advancement |
| fountain.rs | 818 | fountain.c | Fountain quaffing and dipping |
| objnam.rs | 764 | objnam.c | doname/xname/an/the/plural/erosion |
| were.rs | 732 | were.c | Lycanthropy, form-shifting, silver vulnerability |
| steal.rs | 705 | steal.c | Nymph theft, foocubus interaction |
| worm.rs | 700 | worm.c | Long worm segments, tail mechanics |
| sit.rs | 693 | sit.c | Throne effects, cockatrice egg |
| ball.rs | 691 | ball.c | Iron ball drag mechanics, punishment |
| mcastu.rs | 665 | mcastu.c | Monster spellcasting (clerical + arcane) |
| light.rs | 661 | light.c | Light source radius, lamp/candle/sunsword |
| worn.rs | 647 | worn.c | Intrinsic-granting from worn items |
| dbridge.rs | 638 | dbridge.c | Drawbridge state machine, entity crush |
| pager.rs | 636 | pager.c | Symbol lookup, object/monster descriptions |
| fov.rs | 596 | vision.c | Field-of-view ray casting |
| explode.rs | 582 | explode.c | 7 types, 3x3 blast, item destruction |
| lock.rs | 558 | lock.c | Lock picking, key/credit card/lockpick |
| event.rs | 521 | -- | Event bus for cross-system communication |
| o_init.rs | 519 | o_init.c | Appearance shuffling, per-game randomized descriptions |
| priest.rs | 484 | priest.c | Temple mechanics, protection purchase |
| vault.rs | 467 | vault.c | Guard NPC, gold collection |
| minion.rs | 429 | minion.c | Angel/demon summoning on prayer |
| world.rs | 414 | -- | Game world state container |
| write.rs | 384 | write.c | Magic marker, scroll writing |
| symbols.rs | 260 | drawing.c | Map symbol definitions |
| action.rs | 235 | cmd.c | Action/command enumeration |
| rumors.rs | 225 | rumors.c | True/false rumors, Oracle consultations |
| lib.rs | 88 | -- | Crate root, module declarations |

### Other Crates

| Crate | LOC | Purpose |
|-------|-----|---------|
| cli | 8,055 | Game startup, config parsing, RC files, 40+ options |
| tui | 5,629 | Terminal UI, status bar, message log, menus |
| i18n | 4,625 | Fluent-based i18n, noun phrase grammar, CJK support |
| data | 4,956 | TOML data loading, monster/item/dungeon definitions |
| audio | 340 | Sound event dispatch |

### Data Layer

- **394 monsters** (data/monsters/base.toml — 11.6K lines)
- **430 objects** across 11 categories: weapons (71), armor (82), tools (50), spellbooks (44), gems (36), food (33), rings (28), potions (26), wands (24), scrolls (23), amulets (13)
- **33 artifacts** with invoke powers and alignment restrictions
- **13 quest roles** with dialog (data/quests/)
- **8 dungeon branches** + topology (data/dungeons/dungeon_topology.toml)
- **8 Sokoban variants**, 9 Big Room variants, 7 Gehennom levels

### Special Levels (ALL implemented)

Castle, Medusa, Oracle, Rogue level, Sokoban (8 variants), Minetown,
Mines End, Valley of the Dead, Asmodeus, Baalzebub, Juiblex, Orcus,
Fake Wizard (x2), Wizard Tower (x3), Sanctum, Vlad's Tower (x3),
Fort Ludios, Earth/Air/Fire/Water Planes, Astral Plane, 13 Quest sets,
Big Room (9 variants)

### Localization

- 5 locales: en, zh-CN, zh-TW, de, fr
- Fluent-based message system (1,623 English keys)
- Noun phrase grammar engine: articles, plurals, BUC, enchantment, erosion
- CJK display width, classifiers (zh-CN/zh-TW)
- Object and monster name translation via TOML

## Remaining Work (<1%)

### Cross-System Integration (wiring gaps)

- Polymorph trap -> actual polyself transformation (polyself.rs exists but trap trigger not wired)
- Vault guard spawn on vault entry (vault.rs exists but spawn trigger not wired)
- Minion entity creation on prayer (event fires but entity not spawned)
- Appearance table -> full display pipeline (o_init.rs shuffles but TUI doesn't consume all fields)

### Polish

- Wizard mode: WizMap/WizDetect functional; WizGenesis/WizWish/WizKill emit events only
- Some edge cases in eating/stair interactions
- i18n noun phrase grammar: 21 test failures in Fluent gender/case agreement (de/fr)
- End-to-end balance testing (30 years of C tuning not yet replicated)

### Excluded by Design

- Non-TUI window systems (X11/Qt/curses) -- TUI only
- Lua integration (replaced by TOML + Rust)
- Mail daemon (optional in C too)
- Platform-specific code (VMS/DOS/Amiga)
- RCS-style version stamps ($NHDT-Date$ etc.)
