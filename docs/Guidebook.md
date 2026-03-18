# NetHack Babel -- Guidebook

*Version 0.1.0*

---

**Contents:**
[1. Introduction](#1-introduction) --
[2. Getting Started](#2-getting-started) --
[3. Movement](#3-movement) --
[4. Combat](#4-combat) --
[5. Items](#5-items) --
[6. Dungeon Features](#6-dungeon-features) --
[7. Your Character](#7-your-character) --
[8. Pets](#8-pets) --
[9. Death and Scoring](#9-death-and-scoring) --
[10. Tips for New Players](#10-tips-for-new-players) --
[11. Language Support](#11-language-support) --
[12. Command Reference](#12-command-reference) --
[13. Symbol Reference](#13-symbol-reference) --
[14. Original NetHack at a Glance](#14-original-nethack-at-a-glance) --
[15. Project Roadmap and TODO](#15-project-roadmap-and-todo)

---

## 1. Introduction

### What Is NetHack Babel?

NetHack Babel is a modern reimplementation of the classic roguelike
NetHack, written from the ground up in Rust. It aims for formula-level
accuracy with the original game while bringing a modern architecture,
true-color terminal rendering, and built-in multilingual support.

NetHack is a single-player dungeon exploration game. You control an
adventurer who descends into a randomly generated dungeon to retrieve
the Amulet of Yendor. Along the way, you will fight monsters, collect
treasure, solve puzzles, and die -- often and memorably. Every game
generates a new dungeon, and death is permanent.

If you have played NetHack before, you will feel right at home. If you
have never played, welcome -- you are about to discover one of the most
intricate games ever made.

### How It Differs from the Original

NetHack Babel is based on NetHack 3.7 and preserves its core mechanics.
Combat formulas, monster behavior, item effects, and dungeon generation
all follow the original rules as closely as possible. The key differences
are:

- **Trilingual.** All game text is available in English, Simplified
  Chinese, and Traditional Chinese. Switch languages at any time
  without restarting.
- **Modern terminal rendering.** True-color via ratatui with colored
  messages, BUC highlighting, and a clean three-panel layout.
- **Data-driven.** Monsters, objects, and dungeons live in external
  TOML files for easy modding and content creation.
- **Deterministic replay.** Same seed and same inputs produce the same
  game, enabling bug reports, speedrun verification, and AI research.

From a player's perspective, NetHack Babel plays like NetHack. The
differences are in quality of life, not in gameplay rules.

## 2. Getting Started

### Launching

From the source repository:

    cargo run -- --data-dir data

With a compiled binary, run it from a directory containing `data/`, or pass the path explicitly:

    nethack-babel --data-dir /path/to/data

**Command-line options:**

| Flag | Effect |
|------|--------|
| `--data-dir PATH` | Path to game data directory (default: `data`) |
| `-u NAME` / `--name NAME` | Set your character's name |
| `-l LANG` / `--language LANG` | Display language: `en`, `zh-CN`, `zh-TW` |
| `-D` / `--debug` | Start in wizard (debug) mode |
| `--text` | Plain text-mode fallback instead of TUI |
| `--record FILE` | Record session to file |

### Wizard Mode Commands

When launched with `-D` / `--debug`, text-mode input also accepts:

- `wizgenesis <monster>`: spawn the named monster adjacent to the hero.
- `wizwish <wish>`: materialize a wished-for item, applying the same wish parser and restrictions as the engine.
- `wizidentify`: mark carried items as fully known at the instance level.
- `wizmap`: reveal the full current map.
- `wizdetect`: detect all monsters, objects, and traps on the current level.
- `wizwhere`: print the current location and the resolved special-level topology for this run.
- `wizkill`: remove every live monster on the current level.
- `wizlevelport <depth>`: level-teleport to the requested dungeon depth.

### The Screen

    +------------------------------------------------------------+
    | Messages appear here.                                      |  <- Message line
    |      ------           --------                             |
    |      |....|           |......|                             |
    |      |..@.|####       |......|                             |
    |      |....|   #####---|......|                             |
    |      ------           --------                             |  <- Map
    | Player  St:16 Dx:12 Co:14 In:10 Wi:11 Ch:9                |  <- Status line 1
    | Dlvl:1 $:0 HP:16(16) Pw:4(4) AC:10 Xp:1 T:1              |  <- Status line 2
    +------------------------------------------------------------+

- **Message area** (top): Game messages. Press `Ctrl+P` to review past messages. `--More--` appears when multiple messages queue up.
- **Map** (middle): The dungeon. Unexplored areas are blank; explored but not visible areas are dimmed.
- **Status bar** (bottom): Your vital statistics. See section 7.

## 3. Movement

### Basic and Diagonal Movement

Vi-style keys map to eight compass directions:

      y k u       NW  N  NE
      h . l        W  .   E
      b j n       SW  S  SE

Arrow keys work for cardinal directions. Press `.` to wait one turn.

Diagonal movement (`y`, `u`, `b`, `n`) is essential for efficient navigation and escaping monsters.

### Running

Hold **Shift** + a movement key to run in that direction. Your character keeps moving until interrupted by a junction, doorway, monster, or map edge. Shift+Arrow also works for cardinal directions.

### Stairs

- `<` on an up staircase to climb up.
- `>` on a down staircase to descend.

The deeper you go, the harder the monsters -- but the better the treasure.

### Doors

- `o` + direction: **open** a door.
- `c` + direction: **close** a door.

Locked doors can be kicked open (`#kick`), but kicking can injure you, and shopkeepers do not appreciate property damage.

## 4. Combat

### Melee

Move into a monster to attack it. The game uses your wielded weapon
(or bare hands if nothing is wielded). You will see messages describing
the outcome:

- "You hit the kobold!" -- your attack connected.
- "You miss the kobold." -- your attack missed.
- "The kobold hits you!" -- it struck you back.

Combat formulas account for weapon skill, strength, dexterity, and
encumbrance, matching the original NetHack. Different weapon types
use different damage dice -- a two-handed sword hits harder than a
dagger, but daggers are lighter and can be thrown.

Press `F` + direction to **force-attack** in that direction. This is
useful when you want to attack a peaceful monster or when you are not
sure something is standing on a particular tile.

### Weapons and Armor

- `w`: **Wield** a weapon from your inventory.
- `W`: **Wear** armor. `T`: **Take off** armor you are wearing.
- `P`: **Put on** a ring or amulet. `R`: **Remove** a ring or amulet.

Armor Class (AC) starts at 10 (unarmored). **Lower AC is better.** A
suit of plate mail might bring it down to 3; magical armor can push it
into negative numbers. Every point of AC matters -- invest in good
armor early.

### Ranged Combat

- `t`: **Throw** an item. You will be prompted to select an item from
  inventory and a direction. Daggers, darts, spears, and other
  projectiles work well as thrown weapons.
- `f`: **Fire** readied projectile (quivered ammo with a wielded
  launcher, such as arrows with a bow).

Ranged attacks let you soften up dangerous monsters before they reach
melee range -- an important tactic against monsters that can kill you
in one or two hits.

## 5. Items

### Basic Operations

| Key | Action |
|-----|--------|
| `,` | Pick up items from the floor |
| `d` / `D` | Drop one / multiple items |
| `i` | View inventory |
| `I` | View equipped items |

### Using Items

| Key | Action |
|-----|--------|
| `e` | Eat food |
| `q` | Quaff (drink) a potion |
| `r` | Read a scroll or spellbook |
| `z` | Zap a wand |
| `Z` | Cast a spell |
| `a` | Apply (use) a tool |

### Item Identification

Many items start unidentified ("a bubbly potion", "a scroll labeled ZELGO MER"). Ways to identify:

- **Use them** -- risky but informative.
- **Price identification** -- shopkeepers reveal base prices.
- **Scrolls of identify** -- safe and reliable.
- **Altars** -- drop items on an altar to learn BUC status.

### BUC Status (Blessed / Uncursed / Cursed)

Every item has a BUC status. Blessed items are enhanced, uncursed work normally, cursed items are weakened or harmful. Cursed equipment cannot be removed once worn.

In the TUI, known BUC is color-coded: **Blessed** = cyan, **Uncursed** = white, **Cursed** = red.

## 6. Dungeon Features

### Shops

Some rooms are shops, run by a shopkeeper. When you enter a shop:

- Items on the floor are merchandise. Pick one up and the shopkeeper
  will quote a price.
- Press `p` to **pay** for items you are carrying.
- You can sell your own items by dropping them in the shop.
- **Do not steal.** Shopkeepers are among the strongest creatures in
  the game and will attack thieves. Do not damage shop walls or
  inventory either.

### Altars

Altars (`_`) let you test BUC status (drop items on them), pray
(`#pray`), and sacrifice corpses (`#offer`). Sacrificing to your god
improves your standing and may earn artifact weapon gifts.

### Fountains

Fountains (`{`) offer random effects when quaffed from -- sometimes
treasure, sometimes water monsters. Dipping items in fountains may
also produce interesting results.

### Traps

Traps are hidden until discovered. Press `s` to **search** adjacent
squares. You may need to search the same spot multiple times -- luck
affects your chances.

Common trap types:

| Trap | Effect |
|------|--------|
| Pit / spiked pit | You fall in, taking damage |
| Arrow / dart trap | A projectile shoots at you |
| Bear trap | You are held in place for several turns |
| Teleportation trap | You are teleported to a random location |
| Falling rock trap | A rock falls on your head |
| Squeaky board | Makes noise, alerting monsters |
| Trap door | You fall to the next level |

### Special Levels

- **Gnomish Mines** -- a branch of cavern-style levels with gnomes,
  dwarves, scattered gems, and useful tools. A good source of early
  equipment.
- **Sokoban** -- a branch containing block-pushing puzzles. Each floor
  has a prize at the top. Push boulders into holes to clear a path.
  Do not go back down once you enter -- it angers your god.
- **The Oracle** -- a special level with a central room surrounded by
  fountains. The Oracle offers consultations for a fee.

## 7. Your Character

### Status Bar Explained

**Line 1:** `Player  St:16 Dx:12 Co:14 In:10 Wi:11 Ch:9`

| Stat | Full Name | Affects |
|------|-----------|---------|
| St | Strength | Melee damage, carrying capacity |
| Dx | Dexterity | To-hit chance, AC bonus |
| Co | Constitution | HP gained per level |
| In | Intelligence | Spell success rate |
| Wi | Wisdom | Power (spell energy) gain |
| Ch | Charisma | Shop prices, some interactions |

Strength 18 is special: exceptional strength shows as 18/01 through 18/** (18/100).

**Line 2:** `Dlvl:3 $:42 HP:24(30) Pw:8(12) AC:5 Xp:3 T:512 Hungry`

- **Dlvl**: dungeon depth. **$**: gold. **HP**: hit points (current/max).
- **Pw**: power for spells (current/max). **AC**: armor class (lower = better).
- **Xp**: experience level. **T**: turn count. Status conditions follow.

### Hunger

| State | Meaning |
|-------|---------|
| Satiated | Overfed -- eating more risks choking |
| *(blank)* | Normal |
| Hungry | Eat soon |
| Weak | Combat impaired, eat now |
| Fainting | About to pass out |
| Starved | Dead |

Eat with `e`. Monster corpses can be eaten but some are poisonous.

### Alignment and Prayer

Your character follows a Lawful, Neutral, or Chaotic god. Pray (`#pray`) when desperate -- your god may heal you, cure illness, or feed you. But do not pray more than once every 300-500 turns, and do not pray when your god is angry.

## 8. Pets

You begin the game with a tame animal -- typically a kitten or a little
dog, depending on your role. Your pet is a valuable companion that
fights alongside you and provides useful information about items.

**Feeding and loyalty:**

- Pets get hungry over time. Feed them by dropping food near them.
  Dogs and cats prefer meat (tripe rations, corpses).
- A well-fed pet is a loyal pet. A starving pet may turn feral and
  eventually attack you.

**Using your pet:**

- Your pet will fight monsters near you, gaining experience and
  becoming stronger over time.
- **Pets avoid cursed items.** If your pet refuses to step on an item
  on the ground, that item is very likely cursed. This is one of the
  most reliable early-game ways to test items.
- Your pet follows you between dungeon levels if it is standing
  adjacent to you when you use the stairs.

**Caring for your pet:**

- Do not hit your pet -- attacks reduce tameness and may cause it to
  turn hostile.
- Stay within a few squares of your pet. Pets that are too far away
  may become lost.
- If your pet dies, you can tame a new animal using a tripe ration
  or magic (scroll of taming, spell of charm monster).

## 9. Death and Scoring

### Permadeath

When you die in NetHack Babel, it is permanent. There is no reload, no
quicksave, no checkpoint. The game displays a tombstone showing your
cause of death and final score. Death comes in many forms -- monsters,
starvation, poison, petrification, and dozens more. Each death teaches
you something; each new game generates a fresh dungeon.

### Scoring

Your score is calculated from several factors:

    4 * experience + exploration + gold + 1000 * artifacts
      + 5000 * conducts + 50000 * ascension

### Conducts

Optional self-imposed challenges that increase your score:

| Conduct | Restriction |
|---------|-------------|
| Foodless | Never ate |
| Vegan | No animal products |
| Vegetarian | No meat |
| Atheist | Never prayed or sacrificed |
| Weaponless | Never killed with a wielded weapon |
| Pacifist | Never killed directly |
| Illiterate | Never read scrolls/spellbooks |
| Wishless | Never wished |
| Genocideless | Never genocided |
| Nudist | Never wore armor |
| Elberethless | Never wrote Elbereth (bonus) |

### The Goal

The ultimate goal is to descend deep into the dungeon, find the Amulet
of Yendor, and carry it back up through the Elemental Planes to offer
it to your god. This is called **ascension**. Do not worry about winning
right away -- focus on exploring, learning, and surviving longer each
time.

## 10. Tips for New Players

1. **Always carry food.** Starvation is a top killer. Eat safe corpses (lichens, jackals); hoard rations.
2. **Identify before using.** Unknown potions/scrolls can be lethal. Use pets, altars, and price ID to test items safely.
3. **Elbereth saves lives.** Engrave it (`#engrave`) and most monsters will not attack you while you stand on it.
4. **Do not anger shopkeepers.** Pay for everything. If you accidentally pick something up, put it back.
5. **Pray when desperate** -- but not more than once every 300+ turns, and only when genuinely in danger.
6. **Fight in corridors.** Only one monster can reach you at a time, versus being surrounded in a room.
7. **Use ranged attacks.** Weaken monsters before they reach melee range.
8. **Know when to run.** Use stairs to escape to a different level if a fight is unwinnable.
9. **Search dead ends.** Secret doors are common. Press `s` multiple times on suspicious walls.
10. **Read the messages.** The game tells you important things. Press `Ctrl+P` to review.

## 11. Language Support

### Setting Your Language

Command line: `nethack-babel --language zh-CN`

Config file (`~/.config/nethack-babel/config.toml`):

    [game]
    language = "zh-CN"

### Available Languages

| Code | Language |
|------|----------|
| `en` | English |
| `zh-CN` | Simplified Chinese (简体中文) |
| `zh-TW` | Traditional Chinese (繁體中文) |

Translation files use Project Fluent format (`.ftl`) in `data/locale/`. When a translation is missing, the game falls back to the parent language (zh-TW falls back to zh-CN, which falls back to en). CJK double-width characters are correctly measured for terminal alignment.

## 12. Command Reference

### Movement

| Key | Action | Key | Action |
|-----|--------|-----|--------|
| `h` / Left | West | `H` / Shift+Left | Run west |
| `j` / Down | South | `J` / Shift+Down | Run south |
| `k` / Up | North | `K` / Shift+Up | Run north |
| `l` / Right | East | `L` / Shift+Right | Run east |
| `y` | Northwest | `Y` | Run northwest |
| `u` | Northeast | `U` | Run northeast |
| `b` | Southwest | `B` | Run southwest |
| `n` | Southeast | `N` | Run southeast |
| `.` | Wait/rest | `s` | Search |
| `<` | Go up stairs | `>` | Go down stairs |

### Items and Equipment

| Key | Action | Key | Action |
|-----|--------|-----|--------|
| `,` | Pick up | `d` / `D` | Drop one / many |
| `i` | Inventory | `I` | Equipment |
| `e` | Eat | `q` | Quaff potion |
| `r` | Read | `z` | Zap wand |
| `a` | Apply tool | `Z` | Cast spell |
| `w` | Wield weapon | `f` | Fire projectile |
| `W` | Wear armor | `T` | Take off armor |
| `P` | Put on ring/amulet | `R` | Remove ring/amulet |
| `t` | Throw | `F` | Force-attack |

### Interaction and Information

| Key | Action |
|-----|--------|
| `o` + direction | Open door |
| `c` + direction | Close door |
| `p` | Pay shopkeeper |
| `:` | Look at floor |
| `;` | Far look |
| `/` | Identify symbol |
| `?` | Help |
| `Ctrl+P` | Message history |
| `#` | Extended command prefix |
| `Escape` | Cancel |

### Extended Commands

`#pray` `#offer` `#chat` `#loot` `#dip` `#ride` `#enhance` `#name` `#adjust` `#kick` `#twoweapon`

## 13. Symbol Reference

### Dungeon Features

| Symbol | Meaning | Symbol | Meaning |
|--------|---------|--------|---------|
| `.` | Floor / ice | `#` | Corridor / wall |
| `+` | Closed/locked door | `\|` | Open door / grave |
| `<` | Stairs up | `>` | Stairs down |
| `{` | Fountain | `}` | Water / pool / lava |
| `_` | Altar | `\` | Throne |
| `T` | Tree | ` ` | Stone / air |

### Monsters

| Symbol | Type | Symbol | Type |
|--------|------|--------|------|
| `@` | You (the player) | `a` | Ants, insects |
| `b` | Blob | `c` | Cockatrice |
| `d` | Dog, jackal, wolf | `e` | Floating eye |
| `f` | Cat (feline) | `g` | Gnome, gremlin |
| `h` | Humanoid | `i` | Imp, minor demon |
| `j` | Jelly | `k` | Kobold |
| `l` | Leprechaun | `m` | Mimic |
| `n` | Nymph | `o` | Orc |
| `r` | Rodent | `s` | Spider, scorpion |
| `u` | Unicorn, horse | `v` | Vortex |
| `w` | Worm | `z` | Zombie |
| `A` | Angel | `B` | Bat |
| `C` | Centaur | `D` | Dragon |
| `E` | Elemental | `F` | Fungus, mold |
| `H` | Giant | `L` | Lich |
| `M` | Mummy | `N` | Naga |
| `O` | Ogre | `S` | Snake |
| `T` | Troll | `V` | Vampire |
| `W` | Wraith, wight | `Z` | Large zombie |
| `&` | Demon (major) | `'` | Golem |
| `:` | Lizard | `;` | Sea monster |

Colors distinguish monsters of the same letter. A red `D` is a red dragon; a white `D` is a white dragon. Pay attention to color -- it can mean the difference between a manageable fight and instant death.

## 14. Original NetHack at a Glance

Original NetHack is a long-running, community-maintained roguelike known for deep system interactions and strict permadeath. It rewards planning, experimentation, and risk management over reflexes. The game world is procedural, but the rules are consistent enough that mastery comes from understanding interactions.

The core campaign flow in both original NetHack and NetHack Babel is:

1. Descend through the Dungeons of Doom and side branches.
2. Retrieve the Amulet of Yendor.
3. Ascend through the Elemental Planes.
4. Offer the Amulet on the correct Astral altar.

## 15. Project Roadmap and TODO

### Roadmap (2026)

1. Push temple/shop parity past the current `temple-entry / sanctum-entry / shop-entry / follow / payoff / pray / calm-down / cranky-priest-chat / repair / drop-credit / live-sell / robbery / restitution / protection-spend / donation-tiers / ale-gift / blessing-clairvoyance / cleansing` runtime closure and finish the remaining richer deity/shop feedback and economy aftermath.
2. Expand Wizard of Yendor harassment parity beyond the current respawn/theft/curse/scaled nasty-summon runtime and repeated regression coverage.
3. Grow traversal and save/load matrices beyond the current quest/endgame/shop/temple/sanctum/wizard plus `Medusa`/`Castle`/`Orcus`/`Fort Ludios`/`Vlad`/invocation-portal coverage into broader drift-detection harnesses for economy, religion, branch transitions, and other campaign-critical paths.
4. Continue hardening level-local runtime caches and save compatibility rules as serialized world state evolves.
5. Keep contributor tooling, data docs, and Guidebook/README parity aligned with live implementation.

### Active TODO

- [ ] Extend temple/shop parity beyond the current temple-entry/sanctum-entry/shop-entry/payoff/follow/pray/calm/cranky-priest-chat/repair/drop-credit/live-sell/robbery/restitution/protection-spend/donation-tiers/ale-gift/blessing-clairvoyance/cleansing path: remaining richer deity/shop feedback and more original economy aftermath.
- [ ] Extend Wizard of Yendor harassment parity beyond the current respawn, theft, cursing, scaled nasty summon, and repeated reload coverage.
- [ ] Expand traversal/save-load matrices into stronger drift-detection coverage for economy, religion, and branch-state regressions beyond the current quest/endgame/shop/temple/sanctum/wizard/Medusa/Castle/Orcus/Fort-Ludios/Vlad/invocation-portal matrix.
- [ ] Audit remaining level-local runtime state for cross-level leakage and serialization gaps.

### Save Format Policy

The current save format version is `1.0.0`. Older `0.3.x` saves are intentionally incompatible and will be rejected on load. The break was required to serialize level-scoped floor object locations and newer quest/runtime state without silently misloading cross-level data.

---

*NetHack Babel is free software under the NetHack General Public License (NGPL). Based on NetHack by the NetHack DevTeam.*

*"The DevTeam thinks of everything."*
