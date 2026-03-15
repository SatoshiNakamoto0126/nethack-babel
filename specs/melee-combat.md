# NetHack 3.7 近战战斗机制规格

> 源码版本: NetHack-3.7 分支, uhitm.c rev 1.477, weapon.c rev 1.128
> 提取日期: 2026-03-14

---

## 1. 命中判定总览

近战命中使用 **d20 对抗系统**: 计算 `roll_to_hit` 值, 投掷 `dieroll = rnd(20)`, 命中条件为:

```
mhit = (roll_to_hit > dieroll) OR 被吞噬中(u.uswallow)
```

被吞噬时自动命中 (不需要投骰).

> 注意: 与 D&D 不同, **没有 natural 20 自动命中规则**. 如果 `roll_to_hit <= 0`, 除被吞噬外绝不可能命中.

---

## 2. THAC0 / roll_to_hit 完整公式

`find_roll_to_hit()` (uhitm.c:364-427) 计算如下:

```
roll_to_hit =
    1
  + abon()                          // 力量+敏捷命中加成
  + find_mac(target)                // 目标 AC (注意: AC 越低越好, 负数=好甲)
  + u.uhitinc                       // Ring of Increase Accuracy 加成(含吃掉戒指获得的内在)
  + luck_bonus                      // 运气加成
  + level_bonus                     // 等级加成
  + monster_state_bonus             // 目标状态加成
  + role_race_bonus                 // 职业/种族加成
  + encumbrance_penalty             // 负重惩罚
  + trap_penalty                    // 陷阱惩罚
  + weapon_bonus                    // 武器加成 (hitval + weapon_hit_bonus)
```

### 2.1 各分项详解

#### 2.1.1 力量+敏捷命中加成 abon()

(weapon.c:950-984)

**如果变形中 (Upolyd)**: `return adj_lev(&mons[u.umonnum]) - 3`

**否则, 先算力量部分 sbon**:

| 力量范围 | sbon |
|---------|------|
| STR < 6 | -2 |
| 6 <= STR < 8 | -1 |
| 8 <= STR < 17 | 0 |
| 17 <= STR <= 18/50 | +1 |
| 18/51 <= STR < 18/100 | +2 |
| STR >= 18/100 (包括 18/** 和 19+) | +3 |

**低等级调整**: 如果 `u.ulevel < 3`, sbon 额外 +1.

**再加敏捷部分**:

| 敏捷范围 | 最终返回值 |
|---------|-----------|
| DEX < 4 | sbon - 3 |
| 4 <= DEX < 6 | sbon - 2 |
| 6 <= DEX < 8 | sbon - 1 |
| 8 <= DEX < 14 | sbon |
| DEX >= 14 | sbon + (DEX - 14) |

> 注: STR 使用 NetHack 内部编码, `STR18(x) = 18 + x` (attrib.h:36), 所以 18/50 的内部值 = 68, 18/100 的内部值 = 118. 纯粹的 18 (无 /xx) 内部值就是 18, 满足 `<= STR18(50)` 所以 sbon = 1.

#### 2.1.2 目标 AC: find_mac()

(worn.c:709-728)

```
base = monster.data.ac          // 怪物基础 AC (种族固有)
for each worn item:
    if item == AMULET_OF_GUARDING:
        base -= 2               // 固定值, 不受侵蚀影响
    else:
        base -= ARM_BONUS(item) // 正值, 减去 = AC 变好(更低)
if abs(base) > AC_MAX (99):
    base = sgn(base) * AC_MAX
return base
```

> 注意: `find_mac` 返回的是实际 AC 值. 在 roll_to_hit 公式中是**直接加**这个值. AC 为负时有利于防御者 (减小 roll_to_hit), AC 为正时有利于攻击者.

#### 2.1.3 运气加成

```
luck_bonus = sgn(Luck) * ((abs(Luck) + 2) / 3)
```

其中 `Luck = u.uluck + u.moreluck`, 范围 [-13, +13] (但 u.uluck 被限制在 [-10, +10], moreluck 由幸运石提供 +3 或 -3).

精确展开:

| Luck | sgn | abs | (abs+2)/3 | 结果 |
|------|-----|-----|-----------|------|
| -13 | -1 | 13 | 5 | -5 |
| -10 | -1 | 10 | 4 | -4 |
| -9 | -1 | 9 | 3 | -3 |
| -8 | -1 | 8 | 3 | -3 |
| -7 | -1 | 7 | 3 | -3 |
| -6 | -1 | 6 | 2 | -2 |
| -5 | -1 | 5 | 2 | -2 |
| -4 | -1 | 4 | 2 | -2 |
| -3 | -1 | 3 | 1 | -1 |
| -2 | -1 | 2 | 1 | -1 |
| -1 | -1 | 1 | 1 | -1 |
| 0 | 0 | 0 | 0 | 0 |
| 1 | 1 | 1 | 1 | +1 |
| 2 | 1 | 2 | 1 | +1 |
| 3 | 1 | 3 | 1 | +1 |
| 4 | 1 | 4 | 2 | +2 |
| 5 | 1 | 5 | 2 | +2 |
| 6 | 1 | 6 | 2 | +2 |
| 7 | 1 | 7 | 3 | +3 |
| 8 | 1 | 8 | 3 | +3 |
| 9 | 1 | 9 | 3 | +3 |
| 10 | 1 | 10 | 4 | +4 |
| 13 | 1 | 13 | 5 | +5 |

#### 2.1.4 等级加成

```
level_bonus = maybe_polyd(youmonst.data.mlevel, u.ulevel)
```

非变形时 = 玩家等级 `u.ulevel` (1-30); 变形时 = 当前形态怪物等级 `mlevel`.

#### 2.1.5 目标状态加成

| 条件 | 加成 |
|------|-----|
| 目标 stunned (mstun) | +2 |
| 目标 fleeing (mflee) | +2 |
| 目标 sleeping (msleeping) | +2 |
| 目标 paralyzed (!mcanmove) | +4 |

这些加成可叠加.

#### 2.1.6 职业/种族加成

**武僧 (Monk, 非变形)**:
- 穿着身体护甲 (uarm != NULL): `roll_to_hit -= urole.spelarmr` (值为 **20**)
  - 此惩罚赋值给 `role_roll_penalty`, 在 miss 时用于区别 "因穿甲而 miss" vs "本来就会 miss"
- 不穿甲, 不持武器, 不持盾 (`!uarm && !uwep && !uarms`): `roll_to_hit += (u.ulevel / 3) + 2`

**精灵 vs 兽人**: 如果玩家是精灵 (或变形为精灵) 且目标是兽人: `roll_to_hit += 1`

#### 2.1.7 负重/陷阱惩罚

```
encumbrance = near_capacity()   // 0=Unencumbered, 1=Burdened, 2=Stressed,
                                // 3=Strained, 4=Overtaxed, 5=Overloaded
if encumbrance > 0:
    roll_to_hit -= (encumbrance * 2) - 1   // 即 1,3,5,7,9

if u.utrap:     // 被困在陷阱中
    roll_to_hit -= 3
```

| 负重等级 | 惩罚 |
|---------|------|
| Unencumbered (0) | 0 |
| Burdened (1) | -1 |
| Stressed (2) | -3 |
| Strained (3) | -5 |
| Overtaxed (4) | -7 |
| Overloaded (5) | -9 |

#### 2.1.8 武器加成

仅在 `aatyp == AT_WEAP` 或 `AT_CLAW` 时适用:

```
if weapon != NULL:
    roll_to_hit += hitval(weapon, target)
roll_to_hit += weapon_hit_bonus(weapon)  // weapon 可以为 NULL (裸手)
```

特殊: 如果 `aatyp == AT_KICK` 且有武术加成 (martial_bonus), 也加 `weapon_hit_bonus(NULL)`.

---

## 3. hitval() 武器对目标命中加成

(weapon.c:148-187)

```
tmp = 0
if is_weapon:   // oclass == WEAPON_CLASS 或 is_weptool
    tmp += weapon.spe              // 附魔值

tmp += objects[weapon.otyp].oc_hitbon  // 武器固有命中加成 (每种武器不同)

// 特定武器对特定怪物的加成
if is_weapon AND weapon.blessed AND target_hates_blessings:
    // hates_blessings = is_undead OR is_demon (含 vampshifter)
    tmp += 2

if is_spear AND target.mlet in {S_XORN, S_DRAGON, S_JABBERWOCK, S_NAGA, S_GIANT}:
    tmp += 2

if weapon == TRIDENT AND target is swimmer:
    if target on water tile: tmp += 4
    else if target.mlet == S_EEL or S_SNAKE: tmp += 2

if is_pick AND (target passes_walls AND thick_skinned):  // xorn, earth elemental
    tmp += 2

if weapon is artifact:
    tmp += spec_abon(weapon, target)
    // = rnd(artifact.attk.damn) if spec_applies, else 0
```

---

## 4. weapon_hit_bonus() 武器技能命中加成

(weapon.c:1539-1631)

### 4.1 普通武器技能

| 技能等级 | 命中加成 |
|---------|---------|
| Restricted | -4 |
| Unskilled | -4 |
| Basic | 0 |
| Skilled | +2 |
| Expert | +3 |

### 4.2 双武器战斗 (使用双武器且攻击武器是 uwep 或 uswapwep)

取 `min(P_SKILL(TWO_WEAPON_COMBAT), P_SKILL(weapon_type))`:

| 有效技能 | 命中加成 |
|---------|---------|
| Restricted/Unskilled | -9 |
| Basic | -7 |
| Skilled | -5 |
| Expert | -3 |

### 4.3 裸手/武术 (weapon == NULL, type == P_BARE_HANDED_COMBAT)

公式:
```
bonus = max(P_SKILL(P_BARE_HANDED_COMBAT), P_UNSKILLED) - 1  // unskilled => 0
bonus = ((bonus + 2) * (martial_bonus ? 2 : 1)) / 2
```

| 技能等级 | 裸手 (b.h.) | 武术 (m.a.) |
|---------|------------|------------|
| Unskilled (1) | +1 | n/a |
| Basic (2) | +1 | +3 |
| Skilled (3) | +2 | +4 |
| Expert (4) | +2 | +5 |
| Master (5) | +3 | +6 |
| Grand Master (6) | +3 | +7 |

> 武术仅限武僧和武士 (Monk, Samurai), 由 `martial_bonus()` 宏判定 (skills.h:81).

### 4.4 骑乘惩罚 (额外叠加)

| 骑术技能 | 额外命中调整 |
|---------|------------|
| Restricted/Unskilled | -2 |
| Basic | -1 |
| Skilled | 0 |
| Expert | 0 |

使用双武器骑乘时额外再 -2.

---

## 5. 伤害计算

### 5.1 武器基础伤害: dmgval()

(weapon.c:215-356)

```
if target is big (bigmonst):
    tmp = rnd(objects[weapon].oc_wldam)  // 大型怪物伤害骰
    + extra damage per weapon type (见下表)
else:
    tmp = rnd(objects[weapon].oc_wsdam)  // 小型怪物伤害骰
    + extra damage per weapon type (见下表)

if is_weapon:
    tmp += weapon.spe         // 附魔加成
    if tmp < 0: tmp = 0       // 负附魔不能产生负伤害

if material <= LEATHER AND thick_skinned(target):
    tmp = 0                   // 皮甲以下软武器打不动厚皮怪

if target == Shade AND !shade_glare(weapon):
    tmp = 0                   // 影子免疫大多数物理伤害
```

#### 额外伤害修正 (小型怪物 -- !bigmonst)

| 武器类型 | 额外伤害 |
|---------|---------|
| IRON_CHAIN, CROSSBOW_BOLT, MACE, SILVER_MACE, WAR_HAMMER, FLAIL, SPETUM, TRIDENT | +1 |
| BATTLE_AXE, BARDICHE, BILL_GUISARME, GUISARME, LUCERN_HAMMER, MORNING_STAR, RANSEUR, BROADSWORD, ELVEN_BROADSWORD, RUNESWORD, VOULGE | +rnd(4) |
| ACID_VENOM | +rnd(6) |

#### 额外伤害修正 (大型怪物 -- bigmonst)

| 武器类型 | 额外伤害 |
|---------|---------|
| IRON_CHAIN, CROSSBOW_BOLT, MORNING_STAR, PARTISAN, RUNESWORD, ELVEN_BROADSWORD, BROADSWORD | +1 |
| FLAIL, RANSEUR, VOULGE | +rnd(4) |
| ACID_VENOM, HALBERD, SPETUM | +rnd(6) |
| BATTLE_AXE, BARDICHE, TRIDENT | +d(2,4) |
| TSURUGI, DWARVISH_MATTOCK, TWO_HANDED_SWORD | +d(2,6) |

#### 重铁球特殊规则

```
if weapon == HEAVY_IRON_BALL AND tmp > 0:
    base_weight = objects[HEAVY_IRON_BALL].oc_weight  // 480
    if weapon.owt > base_weight:
        wt = (weapon.owt - base_weight) / WT_IRON_BALL_INCR  // WT_IRON_BALL_INCR = 160
        tmp += rnd(4 * wt)
        if tmp > 25: tmp = 25   // 硬上限 25
```

### 5.2 特殊伤害类型 (在 dmgval 内)

在 dmgval() 中, 对武器/宝石/球/链:

| 条件 | 额外伤害 |
|------|---------|
| 祝福 (blessed) vs 厌恶祝福的怪物 (不死/恶魔/vampshifter) | +rnd(4) |
| 斧 (is_axe) vs 木质怪物 (is_wooden) | +rnd(4) |
| **银质 (SILVER) vs 厌银怪物** | **+rnd(20)** |
| 神器光源 (artifact_light, lamplit) vs 厌光怪物 (Gremlin) | +rnd(8) |

> 减半规则: 如果武器是神器且 `spec_dbon(weapon, target, 25) >= 25` (即神器会给双倍伤害), 上述 bonus 减半: `bonus = (bonus + 1) / 2`. 这是为了让"先加 bonus, 再双倍"的等效结果正确.

### 5.3 侵蚀减免

```
if tmp > 0:
    tmp -= greatest_erosion(weapon)   // max(oeroded, oeroded2), 范围 0-3
    if tmp < 1: tmp = 1              // 侵蚀不能让伤害低于 1 (但前提 tmp > 0)
```

### 5.4 力量伤害加成: dbon()

(weapon.c:988-1011)

**变形中 (Upolyd)**: 返回 0.

| 力量范围 | 伤害加成 |
|---------|---------|
| STR < 6 | -1 |
| 6 <= STR < 16 | 0 |
| 16 <= STR < 18 | +1 |
| STR == 18 (18 整, 内部值 18) | +2 |
| 18 < STR <= 18/75 (内部值 19-93) | +3 |
| 18/76 <= STR <= 18/90 (内部值 94-108) | +4 |
| 18/91 <= STR < 18/100 (内部值 109-117) | +5 |
| STR >= 18/100 (内部值 >= 118, 含 19+) | +6 |

#### 力量加成的双手/双武器修正

(uhitm.c:1461-1469, hmon_hitmon_dmg_recalc)

```
strbonus = dbon()
absbonus = abs(strbonus)

if twohits (双武器或双拳攻击):
    strbonus = ((3 * absbonus + 2) / 4) * sgn(strbonus)    // 约 3/4
else if melee AND uwep AND bimanual(uwep):
    strbonus = ((3 * absbonus + 1) / 2) * sgn(strbonus)    // 约 3/2
```

> 设计意图: 双武器两击总力量 = 2 * 3/4 = 3/2, 与双手武器持平.

完整映射表:

| dbon() | 双武器每手 | 双手武器 | 单手 |
|--------|----------|---------|------|
| +6 | +5 | +9 | +6 |
| +5 | +4 | +8 | +5 |
| +4 | +3 | +6 | +4 |
| +3 | +2 | +5 | +3 |
| +2 | +2 | +3 | +2 |
| +1 | +1 | +2 | +1 |
| 0 | 0 | 0 | 0 |
| -1 | -1 | -2 | -1 |

验算 dbon=4, 双武器: `((3*4+2)/4)*1 = 14/4 = 3`. 双手: `((3*4+1)/2)*1 = 13/2 = 6`.

> 注意: 力量加成不用于投掷+发射器组合 (ammo_and_launcher). 投掷仅获得 u.udaminc.

### 5.5 增伤戒指/内在加成

```
dmgbonus += u.udaminc   // 来自 Ring of Increase Damage 或吃掉它获得的内在
```

双武器时两次攻击均获得此加成 (不减半). 双手武器也获得全额.

### 5.6 武器技能伤害加成: weapon_dam_bonus()

(weapon.c:1638-1724)

#### 普通武器

| 技能等级 | 伤害加成 |
|---------|---------|
| Restricted/Unskilled | -2 |
| Basic | 0 |
| Skilled | +1 |
| Expert | +2 |

#### 双武器

取 `min(P_SKILL(TWO_WEAPON_COMBAT), P_SKILL(weapon_type))`:

| 有效技能 | 伤害加成 |
|---------|---------|
| Restricted/Unskilled | -3 |
| Basic | -1 |
| Skilled | 0 |
| Expert | +1 |

#### 裸手/武术

公式:
```
bonus = max(P_SKILL(P_BARE_HANDED_COMBAT), P_UNSKILLED) - 1
bonus = ((bonus + 1) * (martial_bonus ? 3 : 1)) / 2
```

| 技能等级 | 裸手 (b.h.) | 武术 (m.a.) |
|---------|------------|------------|
| Unskilled (1) | 0 | n/a |
| Basic (2) | +1 | +3 |
| Skilled (3) | +1 | +4 |
| Expert (4) | +2 | +6 |
| Master (5) | +2 | +7 |
| Grand Master (6) | +3 | +9 |

#### 骑乘伤害加成

| 骑术技能 | 伤害加成 |
|---------|---------|
| Restricted/Unskilled | 0 |
| Basic | 0 |
| Skilled | +1 |
| Expert | +2 |

(不适用于双武器战斗)

### 5.7 总伤害公式

```
total_dmg = base_damage                   // dmgval() 或裸手伤害
          + u.udaminc                     // 增伤戒指
          + strength_bonus (经双手/双武器修正) // dbon() + 修正
          + weapon_dam_bonus(weapon)      // 技能伤害加成

if total_dmg < 1:
    total_dmg = 1    // 命中保底 1 点 (除了 Shade 和特殊物品)
```

> Shade (影子) 特殊: 如果不是用银质/祝福/神器光/特殊方式攻击, 伤害为 0. 保底规则对 Shade 为: `dmg = (get_dmg_bonus AND !mon_is_shade) ? 1 : 0`.

### 5.8 伤害上限

无硬上限. 伤害直接从怪物 HP 扣除: `mon->mhp -= dmg`.

---

## 6. 裸手攻击规则

### 6.1 基础伤害

(uhitm.c:837-882, hmon_hitmon_barehands)

```
if target == Shade:
    dmg = 0
else:
    dmg = rnd(martial_bonus ? 4 : 2)   // 武术: 1d4, 普通: 1d2
```

### 6.2 手套/银戒指额外伤害

```
if wearing gloves (uarmg):
    dmg += special_dmgval(hero, target, W_ARMG, &silverhit)
    // 祝福手套: +rnd(4) vs 不死/恶魔
    // 银手套: +rnd(20) vs 厌银怪物
else:
    // 检查银戒指
    if single_attack (twohits == 0):
        检查左右手两枚戒指 (但银伤害不叠加, 只取一次 rnd(20))
    if first_of_two (twohits == 1):
        只检查右手戒指 (W_RINGR)
    if second_of_two (twohits == 2):
        只检查左手戒指 (W_RINGL)
    if third_or_later (twohits >= 3, 仅变形):
        不再检查戒指
```

### 6.3 武僧 (Monk) 特殊

- **武僧不穿甲、不持武器、不持盾时**: roll_to_hit 额外 `+ (u.ulevel / 3) + 2`
- 武僧穿甲: roll_to_hit 惩罚 `-20` (spelarmr = 20)
- 武僧和武士使用武术 (martial_bonus):
  - 裸手伤害骰从 1d2 变为 **1d4**
  - 技能命中和伤害加成大幅提高 (见上表)
- 有概率施展裸手震慑 (stagger, 见 6.5)

### 6.4 双拳攻击: double_punch()

(uhitm.c:735-754)

条件: `!uwep && !uarms && P_SKILL(P_BARE_HANDED_COMBAT) > P_BASIC`

```
chance = (skill_level - P_BASIC)   // 1..4
success = chance > rn2(5)          // rn2(5) = 0..4
```

| 技能等级 | 双拳概率 |
|---------|---------|
| Unskilled/Basic | 0% (不检查) |
| Skilled (3) | 20% (1 > rn2(5), 即 rn2(5)==0) |
| Expert (4) | 40% |
| Master (5) | 60% |
| Grand Master (6) | 80% |

> 双拳攻击时, 力量加成使用 3/4 修正 (与双武器相同). 第一击检查右手戒指, 第二击检查左手戒指.

### 6.5 裸手震慑 (stagger)

(uhitm.c:1569-1585)

条件: 裸手 (`unarmed = !uwep && !uarm && !uarms`), 伤害 > 1, 非投掷, 非变形, 非冲刺, 非双武器.

```
if rnd(100) < P_SKILL(P_BARE_HANDED_COMBAT)
   AND !bigmonst(target)
   AND !thick_skinned(target):
    目标被击退 (mhurtle), 可能触发陷阱致死
```

概率: Unskilled 1%, Basic 2%, Skilled 3%, Expert 4%, Master 5%, Grand Master 6%.

---

## 7. 特殊攻击机制

### 7.1 背刺 (Backstab)

(uhitm.c:920-965)

**条件** (全部必须满足):
- 玩家是盗贼 (Rogue), 非变形 (`!Upolyd`)
- 近战攻击 (`hand_to_hand`)
- 伤害 > 1 (`train_weapon_skill == TRUE`)
- 目标不是被缠住的怪物 (`mon != u.ustuck`)
- 非双武器 (`!u.twoweap`)
- 非 Cleaver 神器
- 目标可背刺 (`backstabbable`): 非无定形/旋风/非实体/非 blob/非眼球/非真菌, **能看见目标** (`canseemon`), 且目标在逃跑 (`mflee`) 或无助 (`helpless`)

**效果**: `dmg += rnd(u.ulevel)` (额外 1 到玩家等级的随机伤害)

### 7.2 治疗者解剖知识

(uhitm.c:948-952)

条件: 玩家是治疗者 (Healer), 近战 (`hand_to_hand`), 武器是刀类 (`objects[otyp].oc_skill == P_KNIFE`).

```
dmg += min(3, mvitals[monsndx(mon->data)].died / 6)
```

每杀死 6 只同种怪物增加 1 点, 上限 3.

### 7.3 马上冲刺 (Jousting)

(uhitm.c:2098-2129)

**条件**: 骑乘中 (`u.usteed`), 使用长矛 (`weapon_type(obj) == P_LANCE`), 目标非 `u.ustuck`, 非 Fumbling, 非 Stunned, 武器是 uwep 或 (uswapwep 且 twoweap), 非被困 (`!u.utrap`).

```
skill_rating = P_SKILL(weapon_type(obj))   // lance skill
if twoweap: skill_rating = min(skill_rating, P_SKILL(TWO_WEAPON_COMBAT))
if skill_rating == P_ISRESTRICTED: skill_rating = P_UNSKILLED

joust_dieroll = rn2(5)  // 0-4
if joust_dieroll < skill_rating:
    if joust_dieroll == 0
       AND rnl(50) == 49     // luck-adjusted, ~2% base
       AND !unsolid(target)
       AND !obj_resists(obj, 0, 100):
        return -1  // 冲刺成功但长矛碎裂
    return 1       // 冲刺成功
else:
    return 0       // 冲刺失败, 普通攻击
```

冲刺成功概率:

| 技能 | 概率 |
|------|-----|
| Unskilled (1) | 20% |
| Basic (2) | 40% |
| Skilled (3) | 60% |
| Expert (4) | 80% |

**冲刺额外伤害**: `dmg += d(2, 10)` 如果是 uwep; `d(2, 2)` 如果是 uswapwep.

冲刺还会将目标击退 (mhurtle), 可能触发陷阱.

### 7.4 武器粉碎 (Weapon Shatter)

(uhitm.c:966-1013)

**条件**:
- dieroll == 2 (5% 概率; dieroll==1 保留给 Vorpal/Tsurugi 斩首)
- 使用 uwep, 武器类为 WEAPON_CLASS
- 双手武器 (bimanual) 或 (武士 + 刀 katana + 无盾)
- 武器技能 >= Skilled (`P_SKILL(wtype) >= P_SKILLED`)
- 目标持有非脆弱 (`!is_flimsy`) 武器
- 目标武器通过 `obj_resists(monwep, 50 + 15*(attacker_erosion - defender_erosion), 100)` 检查失败

**效果**: 粉碎目标武器, 目标有 75% 概率逃跑.

有效概率约 2.5% (5% dieroll * ~50% obj_resists 基础, 受双方侵蚀差影响).

### 7.5 Cleaver (清道夫) 三连击

(uhitm.c:650-731)

Cleaver 神器会攻击三个目标: 主目标一侧、主目标、主目标另一侧. 攻击方向在顺时针/逆时针间交替 (静态变量 `clockwise`). 每次独立计算 roll_to_hit.

不能与双武器同时使用. 不能在被吞噬中、被缠住、或对角移动受限 (`NODIAG`) 时使用.

Cleaver 三连击**排除**背刺和武器粉碎检查.

### 7.6 毒素

(uhitm.c:1509-1538)

- **应用条件**:
  - 涂毒武器 (`opoisoned && is_poisonable`): 所有命中均有效
  - 永久性毒素 (`permapoisoned`): 仅 `dieroll <= 5` 时有效 (25%)
- **效果**: 如果目标不抗毒 (`!resists_poison`):
  - 90% 概率: `dmg += rnd(6)`
  - 10% 概率: **即死** (poiskilled)
- 涂毒武器使用后有概率消耗毒素: `!rn2(max(2, 10 - weapon.owt/10))`
- 永久毒素 (permapoisoned) 不会被消耗
- 武士使用毒武器会失去阵营记录; 守序玩家失去 1 点阵营

### 7.7 神器特殊伤害

#### spec_abon (命中加成)

(artifact.c:1083-1094)

```
if artifact has attack (attk.damn != 0) AND spec_applies(artifact, target):
    return rnd(attk.damn)
else:
    return 0
```

#### spec_dbon (伤害加成)

(artifact.c:1098-1116)

```
if artifact has no attack: spec_dbon_applies = FALSE
else if Grimtooth: spec_dbon_applies = TRUE  // 对所有目标
else: spec_dbon_applies = spec_applies(artifact, target)

if spec_dbon_applies:
    if attk.damd != 0: return rnd(attk.damd)
    else: return max(current_dmg, 1)   // 即双倍伤害
```

#### artifact_hit (特殊效果)

(artifact.c:1460-1651)

在 `hmon_hitmon_weapon_melee` 中调用, 发生在基础伤害之后:

1. **先叠加 spec_dbon**: `*dmgptr += spec_dbon(otmp, mdef, *dmgptr)`
2. **元素攻击** (Fire/Cold/Elec/MagicMissile): 消息 + 可能破坏目标物品
3. **Magicbane** (AD_STUN, dieroll <= 4): 特殊 Mb_hit 逻辑
4. **斩首** (SPFX_BEHEAD):
   - **Tsurugi of Muramasa** (dieroll == 1): 非大型怪物即死; 大型怪物 `*dmgptr *= 2`; 被吞噬时即死
   - **Vorpal Blade** (dieroll == 1 或目标为 Jabberwock): 有头且非无形/非实体 -> 即死; 无头 -> miss (dmg = 0)
5. **吸取等级** (SPFX_DRLI): 有概率吸取 1-2 级

> 注: "dieroll == 1" 对应 D&D 的 "natural 20", 因为 NetHack 反转了投骰逻辑.

---

## 8. 命中判定完整流程伪代码

```
FUNCTION do_attack(target):
    // 前置检查
    IF attack_checks(target, uwep): RETURN TRUE  // 取消攻击
    IF Upolyd AND noattacks: "no way to attack"; RETURN
    IF check_capacity OR overexertion: RETURN     // 负重过重或体力透支
    IF twoweap AND !can_twoweapon: untwoweapon()

    exercise(A_STR, TRUE)  // 锻炼力量
    u_wipe_engr(3)         // 擦除脚下雕刻

    // 小妖精闪避 (1/7 概率, 非冰冻/非无助/非困惑/能看见)
    IF target is leprechaun AND special conditions:
        "miss wildly and stumble forwards"; RETURN FALSE

    IF Upolyd:
        hmonas(target)     // 使用怪物形态攻击序列
    ELSE:
        hitum(target, youmonst.data.mattk)  // 使用人形攻击

FUNCTION hitum(target, attack):
    IF wielding Cleaver AND !twoweap AND !swallowed AND !stuck AND !NODIAG:
        RETURN hitum_cleave(target, attack)  // 三连击

    twohits = (uwep ? twoweap : double_punch()) ? 1 : 0

    // --- 第一次攻击 ---
    roll = find_roll_to_hit(target, AT_WEAP, uwep, ...)
    mon_maybe_unparalyze(target)  // 10% 概率解除麻痹
    dieroll = rnd(20)
    hit = (roll > dieroll) OR swallowed
    IF hit: exercise(A_DEX, TRUE)  // 锻炼敏捷

    alive = known_hitum(target, uwep, &hit, roll, armorpenalty, attack, dieroll)
    passive(target, uwep, hit, alive, AT_WEAP, ...)

    // --- 第二次攻击 (如果有) ---
    IF twohits AND alive AND target still adjacent AND !override_confirmation
       AND multi >= 0 AND !life-saved:
        twohits = 2
        roll = find_roll_to_hit(target, AT_WEAP, uswapwep, ...)
        mon_maybe_unparalyze(target)
        dieroll = rnd(20)
        hit = (roll > dieroll) OR swallowed
        alive = known_hitum(target, uswapwep, ...)
        IF hit: passive(target, uswapwep, ...)

    twohits = 0

FUNCTION hmon_hitmon(target, weapon, thrown, dieroll):
    // 1. 计算基础伤害
    IF weapon == NULL:
        dmg = rnd(martial_bonus ? 4 : 2)  // 裸手
        加手套/银戒指伤害
    ELSE IF weapon is WEAPON/WEPTOOL/GEM:
        IF melee: dmg = dmgval(weapon, target)  // 含附魔, 材质, 特殊
        ELSE: dmg = rnd(2)  // 非正规武器使用 (发射器/近身用飞弹等)
        加背刺/治疗者/冲刺/神器/银质 特殊伤害
    ELSE IF weapon is POTION:
        potionhit 效果
    ELSE:
        dmg = weight-based 或 specific item 伤害

    // 2. 加成汇总 (仅在 dmg > 0 且 get_dmg_bonus 为真时)
    IF dmg > 0:
        dmg += u.udaminc + 力量加成(经双手/双武器修正) + 技能伤害加成

    // 3. 毒素
    IF ispoisoned AND !resists_poison:
        90%: dmg += rnd(6)
        10%: 即死

    // 4. 保底
    IF dmg < 1:
        dmg = (get_dmg_bonus AND !Shade) ? 1 : 0

    // 5. 冲刺额外
    IF jousting:
        dmg += d(2, 10)  // 或 d(2, 2) 如果非 uwep
        击退目标
    ELSE IF unarmed AND dmg > 1 AND !thrown AND !Upolyd:
        可能震慑
    ELSE IF armed AND dmg > 1 AND !thrown AND !Upolyd AND !twoweap AND uwep:
        可能击退 (knockback)

    // 6. 应用伤害
    target.mhp -= dmg
```

---

## 9. 每回合多次攻击

### 9.1 人形 (非变形) 时

**最多 2 次攻击**, 来源:
- **双武器战斗**: `u.twoweap == TRUE`, 第一次用 uwep, 第二次用 uswapwep
- **裸手双拳** (double_punch): 无武器无盾, 技能 > Basic, 概率 20%-80%

第二次攻击不会在以下情况发生:
- Stormbringer 主动攻击 (override_confirmation)
- 被被动反击麻痹 (`multi < 0`)
- 被被动反击杀死并生命挽救 (`u.umortality` 增加)
- 目标已死 (`!malive`)
- 目标被击退不在原位 (`m_at(x, y) != mon`)

### 9.2 变形 (Upolyd) 时

使用 `hmonas()`, 遍历当前形态的 **NATTK** (最多 6) 个攻击条目.

- AT_WEAP 攻击: 使用 uwep/uswapwep, 可交替使用 (简化双武器, 需满足多项条件)
- AT_CLAW: 如果有武器且未使用过武器, 当作 AT_WEAP
- AT_TUCH: 巫妖形态时可使用武器
- 双手武器只允许一次 AT_WEAP 攻击 (前次命中 + bimanual -> skip)
- 每次攻击独立计算 roll_to_hit 和 dieroll
- 多 AT_WEAP 攻击时, `twohits` 递增, 影响力量加成和戒指检查
- 目标被击退或死亡后, 后续攻击自动跳过

---

## 10. 武器技能系统

### 10.1 技能等级

(skills.h)

| 等级 | 内部值 | 适用范围 |
|------|-------|---------|
| Restricted (P_ISRESTRICTED) | 0 | 无法使用 |
| Unskilled (P_UNSKILLED) | 1 | 所有可用技能起始 |
| Basic (P_BASIC) | 2 | 所有技能 |
| Skilled (P_SKILLED) | 3 | 所有技能 |
| Expert (P_EXPERT) | 4 | 所有技能 |
| Master (P_MASTER) | 5 | 仅 Bare-Handed/Martial Arts |
| Grand Master (P_GRAND_MASTER) | 6 | 仅 Bare-Handed/Martial Arts |

### 10.2 练习与升级

每次造成 > 1 点伤害的命中, 对当前武器技能 (或双武器技能) 加 1 点练习值.

```
use_skill(skill, 1)   // P_ADVANCE(skill) += 1
```

升级条件 (weapon.c:1150-1163):

```
can_advance(skill) =
    !P_RESTRICTED(skill)
    AND P_SKILL(skill) < P_MAX_SKILL(skill)
    AND u.skills_advanced < P_SKILL_LIMIT (60)
    AND P_ADVANCE(skill) >= practice_needed_to_advance(P_SKILL(skill))
    AND u.weapon_slots >= slots_required(skill)
```

#### 练习值阈值

(skills.h:106)

```
practice_needed_to_advance(level) = level * level * 20
```

| 当前等级 | 需要练习值 |
|---------|-----------|
| Unskilled (1) | 20 |
| Basic (2) | 80 |
| Skilled (3) | 180 |
| Expert (4) | 320 |
| Master (5) | 500 |

> 注: 初始化时, 各技能的 P_ADVANCE 被设为 `practice_needed_to_advance(P_SKILL - 1)`, 即刚好满足前一级的阈值.

#### 技能槽消耗

(weapon.c:1126-1147)

**武器技能和双武器技能** (`skill <= P_LAST_WEAPON || skill == P_TWO_WEAPON_COMBAT`):

```
slots_required(skill) = P_SKILL(skill)  // 当前等级
```

| 升级路径 | 消耗槽数 |
|---------|---------|
| Unskilled -> Basic | 1 |
| Basic -> Skilled | 2 |
| Skilled -> Expert | 3 |

**裸手/武术技能**:

```
slots_required(skill) = (P_SKILL(skill) + 1) / 2
```

| 升级路径 | 消耗槽数 |
|---------|---------|
| Unskilled -> Basic | 1 |
| Basic -> Skilled | 1 |
| Skilled -> Expert | 2 |
| Expert -> Master | 2 |
| Master -> Grand Master | 3 |

#### 技能槽获取

每升一级获得 1 个技能槽 (`add_weapon_skill(1)`). 某些事件 (如失去等级) 会失去技能槽.

总上限: `P_SKILL_LIMIT = 60` 次升级.

### 10.3 技能初始化

(weapon.c:1732-1805, skill_init)

1. 所有技能设为 Restricted
2. 角色初始物品中的武器, 对应技能设为 Basic (弹药除外)
3. 根据职业定义的 `class_skill[]` 数组设置每个技能的 `max_skill`
4. 未列出的技能保持 Restricted; 列出但初始为 Restricted 的设为 Unskilled
5. 如果裸手 max_skill > Expert, 起始裸手为 Basic (武僧)
6. 如果起始宠物是马, 骑术为 Basic

### 10.4 最大技能限制

每个职业对每种武器技能有不同的 max_skill. 例如:
- 武僧: 裸手可达 Grand Master, 大多数武器只有 Basic 或 Restricted
- 战士: 多数武器可达 Expert, 裸手最高 Expert
- 盗贼: 短剑/匕首 Expert, 其他武器多为 Skilled 或 Basic

具体值由 `src/u_init.c` 中各职业的 `Skill_X[]` 数组定义.

---

## 11. 被动防御 (passive)

(uhitm.c, passive 函数)

当玩家攻击怪物后, 怪物 AT_NONE 攻击条目中的 adtyp 触发被动反击.

被动伤害: `d(damn, damd)` 或 `d(mon_level + 1, damd)` (如果 damn == 0).

### 11.1 始终触发 (即使怪物已死)

| 类型 | 效果 |
|------|------|
| AD_FIRE | 侵蚀武器 (1/6 概率) |
| AD_ACID | 溅射酸液, 无抗性时受 `d(damn, damd)` 伤害; 1/30 概率腐蚀护甲; 侵蚀武器 |
| AD_STON | 石化: 如果无手套(裸手攻击)/无靴子(踢)/无头盔(头撞), 且无石化抗性 -> **死亡** |
| AD_RUST | 生锈武器 |
| AD_CORR | 腐蚀武器 |
| AD_MAGM | 魔法飞弹 (Oracle 特有): 无魔法抗性时受 `d(damn, damd)` 伤害 |
| AD_ENCH | 去附魔武器 (解魔兽) |

### 11.2 仅在怪物存活时触发 (2/3 概率, `!mon->mcan && rn2(3)`)

| 类型 | 效果 |
|------|------|
| AD_PLYS (浮游眼) | 如果能看到怪物且怪物能看到你: 无自由行动 -> 麻痹; 有反射则反弹 |
| AD_PLYS (其他) | 无自由行动 -> 冻结 |
| AD_COLD (棕色霉) | 无抗性: 受 `d(damn, damd)` 寒冷伤害, 怪物吸收热量恢复 HP |
| AD_STUN (黄色霉) | 眩晕 |
| AD_FIRE (火蚁等) | 无抗性: 受 `d(damn, damd)` 火焰伤害 |
| AD_ELEC (电鳗等) | 无抗性: 受 `d(damn, damd)` 电击伤害 |

---

## 12. 测试向量

以下测试用例给出精确的确定性输入和预期输出. 随机函数用固定值标注.

### 约定

- `rnd(N)` 返回 1..N 的随机值, 测试中固定为指定值
- `rn2(N)` 返回 0..N-1 的随机值
- 所有未提及的状态为默认值 (0/false/无)
- AC_MAX = 99

---

### TV1: 基本命中计算 -- 1 级战士, 长剑 +0, 打 AC 10 的怪物

**输入**:
- u.ulevel = 1, STR = 16, DEX = 10, Luck = 0
- 武器: long sword +0, Basic skill, oc_hitbon = 0
- 目标: AC 10 怪物, 正常状态
- 负重: Unencumbered, 非骑乘, 非变形
- u.uhitinc = 0

**计算**:
```
abon(): STR=16 -> sbon=0; ulevel<3 -> sbon=1; DEX=10 -> return 1
find_mac: 10
luck_bonus: sgn(0)*((0+2)/3) = 0
level_bonus: 1
hitval: spe=0, oc_hitbon=0 -> 0
weapon_hit_bonus: Basic -> 0
```
`roll_to_hit = 1 + 1 + 10 + 0 + 0 + 1 + 0 + 0 = 13`

**命中条件**: dieroll <= 12 (60% 命中率)

---

### TV2: 高级武僧裸手 -- 10 级武僧, 无装备, Grand Master, 打 AC 0 睡眠目标

**输入**:
- u.ulevel = 10, STR = 18/50 (内部值 68), DEX = 18, Luck = 5
- 无武器, 无盾, 无甲. P_BARE_HANDED_COMBAT = P_GRAND_MASTER (6)
- 目标: AC 0 怪物, sleeping
- 负重: Unencumbered
- u.uhitinc = 0

**计算**:
```
abon(): STR=68 -> 17<=68<=STR18(50)=68 -> sbon=1; ulevel>=3; DEX=18 -> 1+(18-14)=5
find_mac: 0
luck_bonus: sgn(5)*((5+2)/3) = 1*2 = 2
level_bonus: 10
monster_state: sleeping +2
monk_bonus: !uarm, !uwep, !uarms -> +(10/3)+2 = 3+2 = 5
weapon_hit_bonus(NULL): martial_arts, GrandMaster(6):
  bonus = max(6,1)-1 = 5; ((5+2)*2)/2 = 7
```
`roll_to_hit = 1 + 5 + 0 + 0 + 2 + 10 + 2 + 5 + 7 = 32`

**命中条件**: dieroll <= 31, 即**自动命中** (dieroll 最大 20)

**伤害** (假设 rnd(4) = 3, double_punch 激活, 第一击):
- 基础: rnd(4) = 3 (武术)
- dbon(): STR=68, `68 <= STR18(75)=93` -> +3
- 双拳 twohits=1: strbonus = ((3*3+2)/4)*1 = (11)/4 = 2
- u.udaminc = 0
- weapon_dam_bonus: GrandMaster: ((5+1)*3)/2 = 9

第一击伤害 = 3 + 0 + 2 + 9 = **14**

---

### TV3: 双武器战斗 -- 银军刀 +3 和 银匕首 +2, Expert, 打恶魔 AC -5

**输入**:
- u.ulevel = 15, STR = 18/100 (内部值 118), DEX = 16, Luck = 10
- uwep: silver saber +3, Expert P_SABER, blessed, 非神器
- uswapwep: silver dagger +2, Expert P_DAGGER, blessed
- P_TWO_WEAPON_COMBAT = Expert
- 目标: 恶魔, AC -5, hates_blessings=TRUE, hates_silver=TRUE, 正常状态
- u.uhitinc = 2, u.udaminc = 0

**第一击 roll_to_hit**:
```
abon(): STR>=118 -> sbon=3; DEX=16 -> 3+(16-14) = 5
find_mac: -5
luck_bonus: sgn(10)*((10+2)/3) = 4
level_bonus: 15
hitval(saber): spe=3 + oc_hitbon(saber=0) + blessed_vs_demon=+2 = 5
weapon_hit_bonus: two-weapon, min(Expert,Expert)=Expert -> -3
```
`roll_to_hit = 1 + 5 + (-5) + 2 + 4 + 15 + 5 + (-3) = 24`

**命中**: 自动命中

**第一击伤害** (假设 rnd(8)=5 即 saber vs small 1d8; rnd(4)=2 blessed; rnd(20)=15 silver):
```
dmgval: rnd(8)=5, +spe=3 -> 8; blessed +rnd(4)=2; silver +rnd(20)=15 -> 8+2+15 = 25
  (非神器, 无 spec_dbon 减半)
  erosion: 0 -> no deduction
u.udaminc = 0
strbonus: dbon()=6, twohits -> ((3*6+2)/4)*1 = 20/4 = 5
weapon_dam_bonus: two-weapon Expert -> +1
总伤害 = 25 + 0 + 5 + 1 = 31
```

---

### TV4: 背刺 -- 15 级盗贼, 匕首 +5, 目标逃跑中

**输入**:
- Role = Rogue, u.ulevel = 15, !Upolyd
- uwep: dagger +5, Expert, 近战, non-twoweap
- 目标: 逃跑中 (mflee), 非无定形/旋风等, canseemon, mon != u.ustuck
- dmgval 返回 8 (假设 rnd(4)=3, +spe=5)

**效果**: `dmg += rnd(15)` (假设 rnd(15) = 10)

dmg after backstab = 8 + 10 = 18, 再加力量+技能+udaminc.

---

### TV5: 边界条件 -- 负附魔武器, 伤害保底

**输入**:
- uwep: short sword -5, Basic skill
- dmgval 计算: rnd(6)=1, +spe=-5 -> -4, clamp to 0 (因为 `if tmp < 0: tmp = 0`)
- 目标: 非 Shade, 非 thick_skinned
- 无特殊伤害 (非祝福, 非银质等)

**dmgval 返回 0**

然后在 hmon_hitmon: dmg == 0, **不进入 dmg_recalc** (条件是 `if dmg > 0`).

```
dmg < 1: get_dmg_bonus=TRUE, !Shade -> dmg = 1
```

**最终伤害: 1** (保底)

> [疑似 bug，如实记录原始行为] dmg == 0 时跳过 dmg_recalc 意味着力量加成、技能加成、u.udaminc 都不会生效. 如果 u.udaminc >= 5 (足以把 dmgval=0 变成正数), 这个正数也不会被应用. 保底直接给 1. 这不像是 bug, 而是设计意图: 基础伤害为 0 的武器不配获得加成.

---

### TV6: 边界条件 -- Shade 免疫

**输入**:
- uwep: long sword +0 (非银, 非祝福, 非神器光源)
- 目标: Shade

**计算**:
```
dmgval: ptr == &mons[PM_SHADE] && !shade_glare(obj) -> tmp = 0
```

```
hmon_hitmon: dmg = 0
dmg < 1: get_dmg_bonus=TRUE, mon_is_shade=TRUE -> dmg = 0
```

**最终伤害: 0** (物理无效, shade_miss 消息 "harmlessly passes through")

---

### TV7: 银质裸手 -- 银戒指 vs 吸血鬼

**输入**:
- 无武器, 无手套
- 右手戴银戒指 (silver material), twohits = 0 (单次攻击)
- 目标: 吸血鬼 (hates_silver = true)
- u.ulevel = 5, STR = 16, 非武术

**计算**:
```
hmon_hitmon_barehands:
    dmg = rnd(2)  // 假设 = 1
    spcdmgflg = W_RINGR | W_RINGL (twohits==0, 检查两手)
    special_dmgval: 右手银戒指, hates_silver -> bonus += rnd(20) (假设 = 14)
    (左手无戒指, 无额外)
    dmg = 1 + 14 = 15

dmg_recalc (dmg > 0):
    u.udaminc = 0
    strbonus = dbon() = 1 (STR=16)
    weapon_dam_bonus(NULL): P_BARE_HANDED_COMBAT = Unskilled(1):
        bonus = ((0+1)*1)/2 = 0
    dmg = 15 + 0 + 1 + 0 = 16
```

**最终伤害: 16**

---

### TV8: 冲刺 -- 骑乘使用 lance +2, Skilled

**输入**:
- 骑乘中, uwep = lance +2, Skilled (3), non-twoweap
- 目标: AC 5, 正常状态, 非 u.ustuck, 小型怪物
- 非 Fumbling, 非 Stunned, 非 trapped

**冲刺判定**: joust_dieroll = rn2(5), 命中条件: < 3, 概率 60%

假设 rn2(5) = 1, 冲刺成功:
```
基础 dmgval: rnd(6)=4 (lance vs small 1d6), +spe=2 -> 6
jousting_dmg = d(2, 10)  // 假设 = 14 (uwep)
total before bonuses = 6
加成: dbon + u.udaminc + weapon_dam_bonus
jousting: dmg += 14 -> 6 + 加成 + 14
```

冲刺还会将目标击退 1 格.

---

### TV9: 边界条件 -- 最大负重惩罚 + 陷阱, 无法命中

**输入**:
- Overloaded (near_capacity = 5), stuck in trap (u.utrap = 1)
- u.ulevel = 1, STR = 10, DEX = 10, Luck = 0, u.uhitinc = 0
- uwep = dagger +0, Unskilled
- 目标 AC = 10

**计算**:
```
abon(): STR=10 -> sbon=0; ulevel<3 -> sbon=1; DEX=10 -> return 1
find_mac: 10
luck: 0
level: 1
encumbrance: -(5*2-1) = -9
trap: -3
hitval: spe=0 + oc_hitbon(dagger: 具体值取决于objects数组) = 0 (假设)
weapon_hit_bonus: Unskilled -> -4
```
`roll_to_hit = 1 + 1 + 10 + 0 + 0 + 1 - 9 - 3 + 0 - 4 = -3`

**命中条件**: -3 > dieroll, 永远不成立 (dieroll >= 1). **必定 miss**.

---

### TV10: 变形攻击 -- 多攻击形态

**输入**:
- Upolyd as 某 3-attack 怪物: {AT_CLAW/AD_PHYS, AT_CLAW/AD_PHYS, AT_BITE/AD_PHYS}
- uwep = long sword +2 (第一个 AT_CLAW 会转为 AT_WEAP 使用武器)
- 后续 AT_CLAW 和 AT_BITE 为徒手

**流程**:
1. 第 1 次 AT_CLAW -> 有 uwep 且未用过武器 -> goto use_weapon -> 用 long sword 攻击
2. 第 2 次 AT_CLAW -> weapon_used=TRUE, 不再转 weapon -> 徒手爪击
3. AT_BITE -> 徒手咬击

每次独立计算 roll_to_hit 和 dieroll. 银戒指: 第 1 击(武器)不适用, 第 2 击检查右手 (odd_claw toggled), 第 3 击检查左手 (odd_claw toggled again).

`multi_weap` 计数 AT_WEAP 类型攻击; 如果 > 1 则 `twohits` 递增.

---

### TV11: 武僧穿甲惩罚 -- 精确数值

**输入**:
- Role = Monk, u.ulevel = 5, STR = 16, DEX = 14
- 穿着 ring mail (uarm != NULL)
- uwep = quarterstaff +1, Basic
- 目标 AC = 5, 正常状态
- Luck = 0, u.uhitinc = 0, Unencumbered

**计算**:
```
abon(): STR=16 -> sbon=0; ulevel>=3; DEX=14 -> 0+(14-14)=0
find_mac: 5
luck: 0
level: 5
monk penalty: uarm != NULL -> -20 (spelarmr=20), role_roll_penalty = 20
hitval: spe=1 + oc_hitbon(quarterstaff=0) = 1
weapon_hit_bonus: Basic -> 0
```
`roll_to_hit = 1 + 0 + 5 + 0 + 0 + 5 - 20 + 1 + 0 = -8`

**必定 miss** (惩罚 -20 极其严厉)

> miss 消息: `missum()` 检查 `rollneeded + armorpenalty > dieroll` 即 `-8 + 20 > dieroll` 即 `12 > dieroll`. 如果 dieroll <= 11, 显示 "Your armor is rather cumbersome..." (因穿甲而 miss, 不穿甲本来能命中).

---

### TV12: 边界条件 -- 力量 18 整 (非 18/xx) 的 abon/dbon

**输入**:
- STR = 18 (内部值 = 18, 即 STR18(0) = 18, 但实际 `STR18(0) = 18+0 = 18`)

**abon()**: `str < 17` 不满足; `str <= STR18(50)` 即 `18 <= 68` -> sbon = 1
**dbon()**: `str < 18` 不满足; `str == 18` -> return +2

> 注意: 这里 `STR18(50) = 68`, 而 STR = 18 确实 <= 68. 18 整和 18/01-18/50 都得到 sbon=1. 区别在于 dbon: STR=18 -> +2, 而 STR=18/01(=19) 已经进入 `str <= STR18(75)` 分支 -> +3.

---

### TV13: 双手武器力量加成 -- 精确验算

**输入**:
- STR = 18/100 (内部值 118), dbon() = 6
- uwep = two-handed sword (bimanual), melee, single-weapon (not twoweap)

**力量修正**:
```
absbonus = 6
strbonus = ((3 * 6 + 1) / 2) * 1 = 19 / 2 = 9   // C 整数除法
```

完整映射:

| dbon() 原值 | 3*abs+1 | /2 (C 整除) | 最终 |
|------------|---------|------------|------|
| +6 | 19 | 9 | +9 |
| +5 | 16 | 8 | +8 |
| +4 | 13 | 6 | +6 |
| +3 | 10 | 5 | +5 |
| +2 | 7 | 3 | +3 |
| +1 | 4 | 2 | +2 |
| 0 | - | - | 0 |
| -1 | 4 | 2 | -2 |

---

### TV14: 边界条件 -- Luck = -10 (持咒幸运石), 对命中的影响

**输入**:
- u.uluck = -10, u.moreluck = -3 (持诅咒幸运石)
- Luck = -10 + (-3) = -13

**运气加成**: `sgn(-13) * ((13+2)/3) = -1 * 5 = -5`

这是运气加成可达到的最负极端值.

---

### TV15: 边界条件 -- 双武器 unskilled 的极端惩罚

**输入**:
- u.ulevel = 1, STR = 10, DEX = 10, Luck = 0
- uwep: short sword +0, Unskilled P_SHORT_SWORD
- uswapwep: dagger +0, Unskilled P_DAGGER
- P_TWO_WEAPON_COMBAT = Unskilled
- u.uhitinc = 0
- 目标 AC = 0

**计算**:
```
abon(): STR=10 -> sbon=0; ulevel<3 -> +1; DEX=10 -> return 1
find_mac: 0
luck: 0
level: 1
hitval(short_sword): spe=0 + oc_hitbon = 0 (假设)
weapon_hit_bonus: two-weapon, min(Unskilled, Unskilled) = Unskilled -> -9
```
`roll_to_hit = 1 + 1 + 0 + 0 + 0 + 1 + 0 + (-9) = -6`

**必定 miss** (Unskilled 双武器几乎不可能命中)

**伤害 (假设命中)**: dmgval=rnd(6)+0-2(skill)=假设3-2=1, weapon_dam_bonus: -3
`dmg = 1 + 0 + 0 + (-3) = -2` -> clamp to 1 (保底).

---

## 13. 关键常量汇总

| 常量 | 值 | 来源 |
|------|---|------|
| AC_MAX | 99 | you.h |
| LUCKMAX | 10 | you.h (u.uluck 上限) |
| LUCKMIN | -10 | you.h (u.uluck 下限) |
| LUCKADD | 3 | you.h (幸运石 moreluck 绝对值) |
| NATTK | 6 | permonst.h (怪物最大攻击数) |
| P_UNSKILLED | 1 | skills.h |
| P_BASIC | 2 | skills.h |
| P_SKILLED | 3 | skills.h |
| P_EXPERT | 4 | skills.h |
| P_MASTER | 5 | skills.h |
| P_GRAND_MASTER | 6 | skills.h |
| P_SKILL_LIMIT | 60 | skills.h (最大技能升级次数) |
| Monk spelarmr | 20 | role.c (穿甲命中/施法惩罚) |
| martial_bonus | Monk or Samurai | skills.h:81 |
| STR18(x) | 18 + x | attrib.h:36 |
| WT_IRON_BALL_INCR | 160 | weight.h:18 |
| HMON_MELEE | 0 | hack.h:557 |
| HMON_THROWN | 1 | hack.h:558 |
| HMON_KICKED | 2 | hack.h:559 |
| HMON_APPLIED | 3 | hack.h:560 |

---

## 14. 注意事项与标注

1. **无自然 20 自动命中**: 与 D&D 不同, NetHack 的 d20 系统没有 "natural 20 always hits" 规则. 如果 roll_to_hit <= 0, 除被吞噬外不可能命中.

2. **AC 方向**: NetHack AC 越低越好 (与 AD&D 一致). `find_mac` 返回实际 AC, 在 roll_to_hit 中直接相加, 所以负 AC 会降低命中率.

3. [疑似 bug，如实记录原始行为] **武僧穿甲惩罚 spelarmr=20 极其严厉**: 这个值与法术施放惩罚共用同一字段, 对武僧命中的影响远超其他职业. 导致穿甲武僧几乎无法命中任何目标. 这似乎是有意为之的设计 -- 鼓励武僧不穿甲.

4. **双武器力量加成不对称**: 双武器每击用 3/4 力量, 双手武器用 3/2 力量. 双武器两击总共 3/2, 与双手武器持平. 这是刻意平衡.

5. **银伤害 rnd(20) 减半**: 在 dmgval() 中, 如果武器是神器且给双倍伤害 (spec_dbon >= 25), 银/祝福/斧/光源 bonus 会减半 `(bonus+1)/2`. 但裸手银戒指伤害走 hmon_hitmon_barehands -> special_dmgval, 不经过此减半.

6. **背刺与双武器互斥**: 代码明确检查 `!u.twoweap` 才允许背刺.

7. **Cleaver 与背刺/武器粉碎互斥**: `is_art(obj, ART_CLEAVER)` 检查阻止了这些特殊效果.

8. [疑似 bug，如实记录原始行为] **dmg == 0 跳过加成**: 当 dmgval 返回 0 (如负附魔), `hmon_hitmon_dmg_recalc` 不被调用 (条件 `if hmd.dmg > 0`), 因此 u.udaminc、力量加成、技能加成都不会生效, 即使它们本可以使伤害为正. 直接走保底 dmg = 1.

9. **Fumbling 不影响命中判定**: `Fumbling` 属性不会降低 roll_to_hit. 它仅影响冲刺 (joust 返回 0) 和其他非战斗判定. 战斗中没有"fumble miss"机制.

10. **mon_maybe_unparalyze**: 每次攻击前, 被麻痹的目标有 10% 概率 (`!rn2(10)`) 恢复行动能力. 这发生在命中判定之前, 但恢复后该次攻击仍享有 `!mcanmove` 的 +4 命中加成 (因为状态检查在之前已完成).
    [疑似 bug，如实记录原始行为] 实际顺序: find_roll_to_hit (含 +4 加成) -> mon_maybe_unparalyze -> dieroll. 所以如果 unparalyze 成功, 攻击者仍获得 +4 加成, 但这只影响当前这一击.
