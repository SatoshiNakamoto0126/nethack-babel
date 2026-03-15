# NetHack Babel — Player's Guidebook

[中文版指南](GUIDEBOOK_zh.md)

## 1. Introduction

NetHack Babel is a faithful reimplementation of NetHack 3.7 in Rust, with 99.8% coverage of the original's gameplay systems and 3,984 passing tests. If you've played NetHack before, everything works as you expect — the same formulas, the same tricks, the same deaths. If you're new, welcome to one of the deepest roguelikes ever made.

Your goal: descend through the Dungeons of Doom, retrieve the Amulet of Yendor from the bottom, ascend through the Elemental Planes, and offer the Amulet at the correct altar on the Astral Plane. You will die many times before succeeding. That's normal.

## 2. Getting Started

### Starting the Game

```sh
cargo run -- --data-dir data                      # English
cargo run -- --data-dir data --language zh_CN      # Simplified Chinese
cargo run -- --data-dir data --language zh_TW      # Traditional Chinese
```

Press `O` during gameplay to switch languages without restarting.

### The Screen

```
  Messages appear here (top line)
  ┌──────────────────────────────────────┐
  │  #####                               │
  │  #...#         The map (80×21)       │
  │  #.@.#         @ = you               │
  │  #...#         . = floor             │
  │  ###+####      # = wall / corridor   │
  │  #......#      + = door              │
  │  ########      < > = stairs          │
  └──────────────────────────────────────┘
  Status line 1: Name  St:18 Dx:14 Co:16 In:10 Wi:8  Ch:12
  Status line 2: Dlvl:1 $:0 HP:16(16) Pw:4(4) AC:10 Xp:1 T:1
```

### Your First Game

1. Move with `hjkl` (vi-keys) or arrow keys. Diagonal movement: `yubn`.
2. Walk into monsters to attack them.
3. Press `,` to pick up items. Press `i` to see your inventory.
4. Press `e` to eat food when hungry. Press `q` to quaff potions.
5. Press `>` on a `>` tile to descend to the next level.
6. Press `S` to save your game.

## 3. Movement & Actions

### Basic Movement

| Key | Direction | Key | Direction |
|-----|-----------|-----|-----------|
| `h` | West | `l` | East |
| `j` | South | `k` | North |
| `y` | NW | `u` | NE |
| `b` | SW | `n` | SE |
| `.` | Wait (rest) | `s` | Search (find hidden doors/traps) |

**Shift + direction** = move until interrupted (run). Useful for crossing long corridors.

**Count prefix**: type a number before a command to repeat it. `20s` searches 20 times.

### Stairs

- `<` on a `<` tile: go up one level
- `>` on a `>` tile: go down one level

The dungeon remembers each level you've visited. Monsters you left behind will still be there when you return.

## 4. Combat

### Melee

Walk into a monster to attack it. Your chance to hit depends on:

- Your experience level and attributes (Strength, Dexterity)
- Your weapon's enchantment and skill level
- The target's armor class (lower AC = harder to hit)
- Your luck (ranges from -13 to +13)

### Weapon Skills

You gain proficiency as you fight: Restricted → Basic → Skilled → Expert. Higher skill means better to-hit and damage bonuses. Press `#enhance` to spend skill points.

### Important Combat Mechanics

- **Negative AC**: When a monster's AC is below 0, you must beat an additional roll to hit
- **Silver weapons**: Deal +d20 bonus damage to silver-hating monsters (demons, undead)
- **Blessed weapons**: Deal +d4 bonus damage to undead and demons
- **Backstab**: Rogues deal bonus damage when attacking from behind while invisible
- **Monks**: Fight best unarmed — wearing armor imposes a severe -20 hit penalty

## 5. Items

### Item Types

| Symbol | Type | Use Key |
|--------|------|---------|
| `)` | Weapons | `w` wield |
| `[` | Armor | `W` wear, `T` take off |
| `!` | Potions | `q` quaff |
| `?` | Scrolls | `r` read |
| `/` | Wands | `z` zap |
| `=` | Rings | `P` put on, `R` remove |
| `"` | Amulets | `P` put on, `R` remove |
| `(` | Tools | `a` apply |
| `%` | Food | `e` eat |
| `+` | Spellbooks | `r` read (to learn) |
| `*` | Gems | `,` pick up |
| `$` | Gold | Automatic |

### BUC Status (Blessed / Uncursed / Cursed)

Every item has a BUC status that modifies its effects:

- **Blessed** (B): Enhanced effects. Blessed potions of healing restore more HP, blessed scrolls of identify reveal your whole inventory.
- **Uncursed** (U): Normal effects.
- **Cursed** (C): Weakened or reversed effects. Cursed potions of gain level *lose* a level. Cursed equipment cannot be removed.

Dip items in holy water (blessed clear water) to bless them. An altar lets you test BUC by dropping items on it.

### Identification

Items start unidentified. You can identify them by:
- **Using them** — quaff that unknown potion (risky!)
- **Scroll of identify** — safe but limited
- **Price ID** — shop prices reveal item identity (a classic strategy)
- **Altar testing** — dropping items on altars reveals BUC status

### Item Naming in Chinese

When playing in Chinese, items display with proper Chinese grammar:

| English | 简体中文 |
|---------|---------|
| a long sword | 长剑 |
| a blessed +2 long sword | 祝福的+2长剑 |
| 3 daggers | 3把匕首 |
| a potion of healing | 治疗药水 |

Counter words (量词) are applied automatically: 把 for weapons, 瓶 for potions, 张 for scrolls, etc.

## 6. Monsters

### Monster Speed

Monsters and the player act based on movement points. Faster monsters get more actions per turn. Same-speed monsters act in the order they were created (first come, first served).

### Monster AI

Monsters are not mindless:
- **Intelligent monsters** open doors, use wands, wear armor, drink potions
- **Wounded monsters** (HP < 25%) may flee
- **Covetous monsters** (Wizard of Yendor) teleport to steal key items
- **Flying monsters** cross water and lava
- **Phasing monsters** move through walls

### Pets

Your starting pet (cat or dog) follows you, fights enemies, and picks up items. Keep it fed to maintain loyalty — starving pets go feral. Pets can follow you between levels if they're adjacent when you use stairs.

**Classic strategy**: Pets can steal items from shops by standing on them and being displaced out of the shop.

## 7. The Dungeon

### Dungeon Branches

| Branch | Depth | Features |
|--------|-------|----------|
| Main Dungeon | 1-29 | The main path down |
| Gnomish Mines | 3-5 entry | Minetown, shops, gems |
| Sokoban | From Mines | Boulder puzzles, guaranteed loot |
| Quest | Mid-game | Role-specific, artifact reward |
| Fort Ludios | Random | Vault full of gold |
| Gehennom | Below Castle | Demons, mazes, no prayer |
| Vlad's Tower | In Gehennom | Candelabrum of Invocation |
| Endgame | After ascent | Elemental Planes + Astral |

### Special Rooms

Rooms can be: shops, temples, throne rooms, zoos, barracks, beehives, morgues, swamps, leprechaun halls, cockatrice nests, antholes.

### Traps

25 trap types lurk in the dungeon. Search (`s`) to find hidden traps. Some notable ones:
- **Pit / spiked pit** — fall damage, may be poisoned
- **Teleport trap** — warps you to a random location
- **Polymorph trap** — transforms you into a random creature
- **Anti-magic field** — drains your power
- **Landmine** — heavy damage and stun

## 8. Hunger & Eating

You consume 1 nutrition per turn, plus extra for worn rings, regeneration, and the Amulet of Yendor. Hunger levels:

| Level | Nutrition | Effect |
|-------|-----------|--------|
| Satiated | > 1000 | Risk of choking if you eat more |
| Not Hungry | 150-1000 | Normal |
| Hungry | 50-150 | Warning message |
| Weak | 1-50 | -1 Strength, impaired combat |
| Fainting | ≤ 0 | Random fainting, near death |
| Starved | Deep negative | Death |

**Corpses** are a major food source but have risks: old corpses cause food poisoning, some grant intrinsic abilities (telepathy from floating eye, poison resistance from killer bee).

## 9. Religion & Prayer

### Prayer

Press `#pray` when in desperate need. Your god may:
1. Fix damaged attributes
2. Cure disease and sickness
3. End starvation
4. Grant useful items

Prayer has a cooldown. Success depends on your alignment record, luck, and whether you're on a co-aligned altar. **Prayer never works in Gehennom.**

### Sacrifice

Kill monsters and offer their corpses on altars with `#offer`. Benefits:
- Improve alignment and luck
- Receive artifact gifts (first gift is guaranteed)
- Convert altar alignment (risky)

### Luck

Luck ranges from -13 to +13. It affects combat, prayer, and many random events. Luck naturally decays toward 0 every 600 turns. Carry a luckstone (blessed or uncursed) to prevent decay.

## 10. Shops

Shopkeepers track everything. When you pick up an item in a shop, you owe money. Pay with `p`.

**Pricing** depends on: base item cost × BUC modifier × your Charisma × Tourist penalty.

**Price identification**: Astute players can deduce item identity from the shopkeeper's asking price. This is a core NetHack strategy.

**Theft**: If you leave a shop without paying, the shopkeeper attacks. Keystone Kops may be summoned.

## 11. Status Effects

| Status | Duration | Cure |
|--------|----------|------|
| Confusion | Timed | Wait, unicorn horn |
| Blindness | Timed | Wait, carrot, unicorn horn |
| Stun | Timed | Wait, unicorn horn |
| Hallucination | Timed | Wait, unicorn horn |
| Levitation | Timed | Wait (can't use stairs!) |
| Stoning | 5 turns | Eat lizard corpse or acidic food |
| Sliming | 10 turns | Apply fire |
| Food Poisoning | Countdown | Prayer, unicorn horn |

The **unicorn horn** is the most versatile cure — it fixes confusion, stun, hallucination, and blindness with a single use (blessed always cures, uncursed ~33% chance, cursed never cures).

## 12. Classic Strategies

These time-tested strategies all work in NetHack Babel:

- **Elbereth**: Engrave "Elbereth" to scare most monsters. Burning it with a wand of fire makes it permanent.
- **Floating eye + telepathy**: Eat a floating eye corpse for permanent telepathy. Essential for finding monsters through walls.
- **Excalibur dip**: Dip a long sword in a fountain while Lawful and at least level 5 to receive Excalibur.
- **Pudding farming**: Hit a brown/black pudding with a negative-enchantment weapon to make it split. Repeat for infinite corpses and experience.
- **Price ID**: Use shopkeeper prices to identify potions, scrolls, and other items without risk.
- **Pet shop theft**: Position your pet on a shop item and displace it out of the shop. The pet "steals" the item for you.
- **Unicorn horn**: Rub a unicorn horn to cure nearly any negative status effect.

## 13. Conducts

NetHack tracks voluntary challenges called conducts. Maintaining a conduct through an entire game is a mark of skill:

| Conduct | Rule |
|---------|------|
| Foodless | Never eat |
| Vegan | Never eat animal products |
| Vegetarian | Never eat meat |
| Atheist | Never pray or sacrifice |
| Weaponless | Never wield a weapon in combat |
| Pacifist | Never directly kill a monster |
| Illiterate | Never read scrolls or spellbooks |
| Genocideless | Never use genocide |
| Polypileless | Never polymorph objects |
| Polyselfless | Never polymorph yourself |
| Wishless | Never wish for items |
| Artiwishless | Never wish for artifacts |
| Elberethless | Never write Elbereth |

## 14. Wizard Mode (Debug)

Start the game with `-D` flag to enable wizard mode:

```sh
cargo run -- --data-dir data -D
```

Wizard mode commands:

| Key | Command | Effect |
|-----|---------|--------|
| Ctrl+W | Wish | Wish for any item (e.g., "blessed +3 silver dragon scale mail") |
| Ctrl+F | Map | Reveal the entire current level |
| Ctrl+G | Genesis | Create a monster by name |
| Ctrl+I | Identify | Identify all items in your inventory |
| Ctrl+E | Detect | Reveal all monsters, objects, and traps |
| `#levelchange` | Level teleport | Jump to a specific dungeon level |
| `#where` | Where | Show locations of all special levels |

Wizard mode is invaluable for testing specific game mechanics and exploring content without the permadeath pressure.

## 15. Roles & Ranks

Each role has 9 rank titles earned as you gain experience levels:

| Role | Level 1 | Level 14 | Level 30 |
|------|---------|----------|----------|
| Archeologist | Digger | Excavator | Curator |
| Barbarian | Plunderer | Raider | Conqueror |
| Caveman | Troglodyte | Wayfarer | Pioneer |
| Healer | Rhizotomist | Medicus ossium | Chirurgeon |
| Knight | Gallant | Knight | Paladin |
| Monk | Candidate | Student of Waters | Master |
| Priest | Aspirant | Canon | High Priest |
| Ranger | Tenderfoot | Scout | Ranger |
| Rogue | Footpad | Robber | Thief |
| Samurai | Hatamoto | Ryoshu | Shogun |
| Tourist | Rambler | Traveler | Adventurer |
| Valkyrie | Stripling | Warrior | Lord |
| Wizard | Evoker | Enchanter | Mage |

Some roles have female-specific rank titles (e.g., Priestess instead of Priest).

## 16. Tips for New Players

1. **Don't eat everything** — tainted corpses kill. Check age with `;` (look).
2. **Identify before using** — unknown potions and scrolls can be deadly.
3. **Keep a unicorn horn** — it's the best status cure in the game.
4. **Pray when desperate** — but not too often, and never in Gehennom.
5. **Price ID in shops** — learn the price tables to identify items safely.
6. **Don't fight everything** — use Elbereth, corridors, and doors tactically.
7. **Pets are valuable** — a well-fed pet fights for you and steals from shops.
8. **Save your scrolls of identify** — use them on high-value unidentified items.
9. **Carry a lizard corpse** — it never rots and cures stoning.
10. **The DevTeam thinks of everything** — if you think "I wonder if I can...", the answer is probably yes.
