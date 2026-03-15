# NetHack 3.7 -- 物品生成系统机制规格

> 提取自 `src/mkobj.c`, `src/o_init.c`, `src/rnd.c`, `src/shknam.c`, `src/mon.c`, `src/objnam.c`, `src/dungeon.c`, `include/objects.h`, `include/objclass.h`, `include/obj.h`
> NetHack 3.7.0 work-in-progress (截至 2025-11 源码)

---

## 目录

1. [随机数原语](#1-随机数原语)
2. [物品类选择](#2-物品类选择)
3. [类内物品选择 (oc_prob)](#3-类内物品选择-oc_prob)
4. [宝石概率与深度关系](#4-宝石概率与深度关系)
5. [外观随机化](#5-外观随机化)
6. [BUC 状态分配](#6-buc-状态分配)
7. [附魔值生成](#7-附魔值生成)
8. [充能数生成](#8-充能数生成)
9. [堆叠数量生成](#9-堆叠数量生成)
10. [侵蚀与涂油](#10-侵蚀与涂油)
11. [神器生成](#11-神器生成)
12. [容器内容物生成](#12-容器内容物生成)
13. [特殊物品初始化](#13-特殊物品初始化)
14. [地下城难度与深度效应](#14-地下城难度与深度效应)
15. [死亡掉落生成](#15-死亡掉落生成)
16. [商店物品栏生成](#16-商店物品栏生成)
17. [测试向量](#17-测试向量)

---

## 1. 随机数原语

以下原语贯穿整个物品生成系统，必须先理解。

| 函数 | 语义 | 范围 |
|------|------|------|
| `rn2(x)` | 均匀随机 | `[0, x)` |
| `rnd(x)` | 均匀随机 | `[1, x]` |
| `rn1(x, y)` | `rn2(x) + y` | `[y, y+x-1]` |
| `d(n, x)` | n 个 x 面骰子之和 | `[n, n*x]` |
| `rne(x)` | 几何分布，受等级上限约束 | `[1, utmp]` |
| `rnz(i)` | "everyone's favorite" 非对称扰动 | 大致围绕 `i` |
| `rnl(x)` | Luck 修正的均匀随机 | `[0, x)` |

### rne(x) 详细算法

```
utmp = (hero_level < 15) ? 5 : hero_level / 3
result = 1
WHILE result < utmp AND rn2(x) == 0:
    result += 1
RETURN result
```

- 参数 `x` 为几何分布的"成功概率分母"
- 每次循环有 `1/x` 概率继续增加
- 上界为 `utmp`
- 对于 `rne(3)`: P(1) = 2/3, P(2) = 2/9, P(3) = 2/27, ..., 受上限截断

### rnz(i) 详细算法

```
x = i (作为 long)
tmp = 1000 + rn2(1000)          -- [1000, 1999]
tmp *= rne(4)                    -- 乘以 [1, utmp] 的几何值
IF rn2(2) == 0:
    x = x * tmp / 1000          -- 放大
ELSE:
    x = x * 1000 / tmp          -- 缩小
RETURN x (截断为 int)
```

### rnl(x) 详细算法

```
adjustment = Luck
IF x <= 15:
    adjustment = (abs(adjustment) + 1) / 3 * sign(adjustment)
i = RND(x)
IF adjustment != 0 AND rn2(37 + abs(adjustment)) != 0:
    i -= adjustment
    clamp i to [0, x-1]
RETURN i
```

好运使结果趋向 0，坏运使结果趋向 x-1。

---

## 2. 物品类选择

当调用 `mkobj(RANDOM_CLASS, artif)` 时，先从以下概率表中选择物品类。

### 2.1 普通关卡 (mkobjprobs)

| 物品类 | 权重 | 概率 |
|--------|------|------|
| WEAPON_CLASS | 10 | 10% |
| ARMOR_CLASS | 10 | 10% |
| FOOD_CLASS | 20 | 20% |
| TOOL_CLASS | 8 | 8% |
| GEM_CLASS | 8 | 8% |
| POTION_CLASS | 16 | 16% |
| SCROLL_CLASS | 16 | 16% |
| SPBOOK_CLASS | 4 | 4% |
| WAND_CLASS | 4 | 4% |
| RING_CLASS | 3 | 3% |
| AMULET_CLASS | 1 | 1% |
| **总计** | **100** | |

选择算法：
```
tprob = rnd(100)       -- [1, 100]
FOR EACH entry IN table:
    tprob -= entry.weight
    IF tprob <= 0:
        RETURN entry.class
```

### 2.2 Rogue 关卡 (rogueprobs)

| 物品类 | 权重 |
|--------|------|
| WEAPON_CLASS | 12 |
| ARMOR_CLASS | 12 |
| FOOD_CLASS | 22 |
| POTION_CLASS | 22 |
| SCROLL_CLASS | 22 |
| WAND_CLASS | 5 |
| RING_CLASS | 5 |
| **总计** | **100** |

注意：Rogue 关卡不生成 TOOL, GEM, SPBOOK, AMULET。

### 2.3 Gehennom (hellprobs)

| 物品类 | 权重 |
|--------|------|
| WEAPON_CLASS | 20 |
| ARMOR_CLASS | 20 |
| FOOD_CLASS | 16 |
| TOOL_CLASS | 12 |
| GEM_CLASS | 10 |
| POTION_CLASS | 1 |
| SCROLL_CLASS | 1 |
| WAND_CLASS | 8 |
| RING_CLASS | 8 |
| AMULET_CLASS | 4 |
| **总计** | **100** |

关键差异：Gehennom 中卷轴和药水极为罕见 (各 1%)，武器/盔甲权重翻倍，戒指/护身符大幅提升。

### 2.4 容器内容物 (boxiprobs)

| 物品类 | 权重 |
|--------|------|
| GEM_CLASS | 18 |
| FOOD_CLASS | 15 |
| POTION_CLASS | 18 |
| SCROLL_CLASS | 18 |
| SPBOOK_CLASS | 12 |
| COIN_CLASS | 7 |
| WAND_CLASS | 6 |
| RING_CLASS | 5 |
| AMULET_CLASS | 1 |
| **总计** | **100** |

注意：容器中不出现 WEAPON, ARMOR, TOOL。

### 2.5 表选择逻辑

```
IF Is_rogue_level:
    table = rogueprobs
ELSE IF Inhell:
    table = hellprobs
ELSE:
    table = mkobjprobs
```

---

## 3. 类内物品选择 (oc_prob)

选定物品类后，在该类所有物品中按 `oc_prob` 权重选择具体物品。

算法 (`mkobj` 中)：
```
total = oclass_prob_totals[class]  -- 预计算的该类 oc_prob 之和
prob = rnd(total)                  -- [1, total]
i = bases[class]                   -- 该类第一个物品的索引
WHILE (prob -= objects[i].oc_prob) > 0:
    i += 1
RETURN i
```

### 3.1 各类 oc_prob 总权重

以下为 `objects.h` 中定义的 `oc_prob` 值汇总（不含 oc_prob=0 的物品，0 概率物品不会被随机生成）。

#### WEAPON_CLASS (武器)

总权重 = 1000（所有武器 oc_prob 之和）

代表性物品：
- arrow: 55, crossbow bolt: 55, dart: 60
- long sword: 50, spear: 50, dagger: 30
- mace: 40, axe: 40, flail: 40, war hammer: 15
- two-handed sword: 22, short sword: 8
- shuriken: 35, boomerang: 15
- bow: 24, sling: 40, crossbow: 45
- athame: 0, scalpel: 0 (不随机生成)
- tsurugi: 0, runesword: 0 (仅限神器基底)

#### ARMOR_CLASS (盔甲)

总权重 = 1000

代表性物品 (suit)：
- leather armor: 82, studded leather: 72, ring mail: 72, scale mail: 72
- chain mail: 72, banded mail: 72, splint mail: 62
- plate mail: 44, bronze plate mail: 25, crystal plate mail: 10
- 龙鳞甲/龙鳞: 全部 0 (不随机生成)

头盔/手套/靴子/斗篷/盾牌 中各项也有独立 oc_prob。

#### RING_CLASS (戒指)

所有戒指的 oc_prob 均为 **1**，共 28 种戒指，总权重 = 28。每种戒指等概率。

#### WAND_CLASS (魔杖)

总权重 = 1000

代表性：
- wand of light: 95, wand of striking: 75
- wand of digging: 55, wand of magic missile: 50
- wand of wishing: 5, wand of death: 5
- wand of nothing: 25

#### POTION_CLASS (药水)

总权重 = 1000

代表性：
- potion of healing: 115, potion of water: 80
- potion of gain ability: 40, potion of speed: 40
- potion of full healing: 10, potion of polymorph: 10

#### SCROLL_CLASS (卷轴)

总权重 = 1000

代表性：
- scroll of identify: 180, scroll of light: 90
- scroll of enchant weapon: 80, scroll of remove curse: 65
- scroll of enchant armor: 63
- scroll of genocide: 15, scroll of charging: 15
- blank paper: 28

#### FOOD_CLASS (食物)

总权重 = 1000

代表性：
- food ration: 380, tripe ration: 140
- egg: 85, tin: 75, slime mold: 75
- fortune cookie: 55
- corpse: 0 (不随机生成，由怪物死亡产生)

#### SPBOOK_CLASS (魔法书)

总权重 = 1000 (含 novel=1，不含 Book of the Dead=0)

代表性：
- spellbook of confuse monster: 49, spellbook of magic missile: 45
- spellbook of light: 45, spellbook of detect monsters: 43
- spellbook of finger of death: 5
- blank paper: 18, novel: 1

**SPBOOK_no_NOVEL 特殊模式**：用 `rnd_class(bases[SPBOOK_CLASS], SPE_BLANK_PAPER)` 选择，排除 novel 和 Book of the Dead。`rnd_class` 使用与上述相同的 oc_prob 加权算法。

#### AMULET_CLASS (护身符)

总权重 = 1000

代表性：
- amulet of ESP: 120, amulet of strangulation: 115
- amulet of restful sleep: 115, amulet versus poison: 115, amulet of change: 115
- amulet of life saving: 75, amulet of reflection: 75
- amulet of flying: 60, amulet of unchanging: 60
- fake amulet: 0, Amulet of Yendor: 0 (不随机生成)

#### TOOL_CLASS (工具)

总权重 = 1000

代表性：
- tin whistle: 100, skeleton key: 80
- leash: 65, lock pick: 60
- blindfold: 50, towel: 50
- large box: 40, chest: 35, sack: 35
- oil lamp: 45, mirror: 45, magic whistle: 30
- tallow candle: 20, pick-axe: 20, bag of holding: 20, bag of tricks: 20
- Candelabrum of Invocation: 0, Bell of Opening: 0 (不随机生成)

#### GEM_CLASS (宝石)

宝石的 oc_prob 是**动态的**，随地下城深度变化（详见第 4 节）。

初始时（深度 0）：
- 所有真宝石 oc_prob 按公式重算（见第 4 节）
- 9 种玻璃宝石: 每种 76-77
- luckstone: 10, loadstone: 10, flint: 10, touchstone: 8
- rock: 100

### 3.2 rnd_class 算法

当需要从连续 otyp 范围 `[first, last]` 中按 oc_prob 选择时使用：

```
sum = SUM(objects[i].oc_prob FOR i IN [first, last])
IF sum == 0:
    RETURN rn1(last - first + 1, first)  -- 等概率
x = rnd(sum)
FOR i = first TO last:
    IF (x -= objects[i].oc_prob) <= 0:
        RETURN i
-- fallback (first == last)
RETURN first
```

用途：SPBOOK_no_NOVEL 选择、容器中替换 ROCK 为宝石、bag_of_holding 中替换 WAN_CANCELLATION。

---

## 4. 宝石概率与深度关系

函数 `setgemprobs(d_level *)` 在每次进入新关卡时调用 (`oinit()`)。

### 算法

```
lev = ledger_no(dlev)    -- 绝对关卡序号，clamp 到 maxledgerno()
first = bases[GEM_CLASS]  -- 第一个宝石的 objects[] 索引

-- 步骤 1: 低价宝石清零
FOR j = 0 TO (9 - lev/3 - 1):
    objects[first + j].oc_prob = 0
first += j     -- first 现在指向第一个非零概率的真宝石

-- 步骤 2: 重新分配剩余真宝石的概率
FOR j = first TO LAST_REAL_GEM:
    objects[j].oc_prob = (171 + j - first) / (LAST_REAL_GEM + 1 - first)

-- 步骤 3: 重算 GEM_CLASS 总权重 (包括石头)
sum = 0
FOR j = bases[GEM_CLASS] TO bases[GEM_CLASS+1] - 1:
    sum += objects[j].oc_prob
oclass_prob_totals[GEM_CLASS] = sum
```

### 解读

- `lev/3` 决定有多少种低价宝石（数组前部）概率被设为 0
- 深度 0 时：`9 - 0 = 9` 种最低价宝石清零

[疑似 bug，如实记录原始行为] 当 `lev = 0` (即 `dlev` 为 NULL 时的初始化)，`9 - 0/3 = 9`，意味着前 9 种宝石全部概率为 0。此时从第 10 种宝石 (amber, 索引偏移 9) 开始才有概率。真宝石共 22 种 (dilithium crystal 到 jade)，所以浅层只能随机得到较低价的宝石。随着深度增加，更高价的宝石逐渐解锁。

- `lev = 3` 时：`9 - 1 = 8` 种清零
- `lev = 6` 时：`9 - 2 = 7` 种清零
- `lev = 27+` 时：`9 - 9 = 0` 种清零，所有真宝石均可出现

概率公式 `(171 + j - first) / (LAST_REAL_GEM + 1 - first)` 使得数组靠后的宝石（更低价）概率更高。171 这个常数保证分母整除时总和合理。

---

## 5. 外观随机化

每局游戏开始时 `init_objects()` 调用 `shuffle_all()` 对以下物品类的描述进行洗牌。

### 5.1 整类洗牌

| 类别 | 范围 | 是否洗牌材质 |
|------|------|-------------|
| AMULET_CLASS | 第一个到最后一个魔法护身符（不含 fake/real AoY） | 是 |
| POTION_CLASS | 第一个到 POT_WATER - 1（不含 water） | 是 |
| RING_CLASS | 整类 | 是 |
| SCROLL_CLASS | 第一个到最后一个魔法卷轴（不含 mail/blank paper） | 是 |
| SPBOOK_CLASS | 第一个到最后一个魔法书（不含 blank paper/novel/Book of the Dead） | 是 |
| WAND_CLASS | 整类 | 是 |
| VENOM_CLASS | 整类 | 是 |

### 5.2 子类洗牌 (盔甲)

| 子类 | 范围 | 是否洗牌材质 |
|------|------|-------------|
| 头盔 | HELMET 到 HELM_OF_TELEPATHY | 否 |
| 手套 | LEATHER_GLOVES 到 GAUNTLETS_OF_DEXTERITY | 否 |
| 斗篷 | CLOAK_OF_PROTECTION 到 CLOAK_OF_DISPLACEMENT | 否 |
| 靴子 | SPEED_BOOTS 到 LEVITATION_BOOTS | 否 |

### 5.3 洗牌算法 (shuffle)

```
-- 统计可洗牌物品数 (oc_name_known == 0 的)
num_to_shuffle = COUNT(j in [o_low, o_high] WHERE NOT objects[j].oc_name_known)
IF num_to_shuffle < 2: RETURN

FOR j = o_low TO o_high:
    IF objects[j].oc_name_known: CONTINUE
    DO:
        i = j + rn2(o_high - j + 1)
    WHILE objects[i].oc_name_known
    SWAP objects[j].oc_descr_idx, objects[i].oc_descr_idx
    SWAP objects[j].oc_tough, objects[i].oc_tough
    SWAP objects[j].oc_color, objects[i].oc_color
    IF domaterial:
        SWAP objects[j].oc_material, objects[i].oc_material
```

已知名称（`oc_name_known=1`）的物品不参与洗牌，保持固定描述。

[疑似 bug] 这个洗牌算法不是标准 Fisher-Yates shuffle。它从 `j` 到 `o_high` 范围随机选择（含 `j` 自身），但跳过 `oc_name_known` 条目时使用 do-while 重试。当 `oc_name_known` 的物品嵌入范围中间时，概率分布不完全均匀，但实际影响极小。

### 5.4 宝石颜色随机化

函数 `randomize_gem_colors()` 额外处理：
- 绿松石 (turquoise): 50% 概率变为蓝色 (复制 sapphire 的描述)
- 海蓝宝石 (aquamarine): 50% 概率变为蓝色
- 萤石 (fluorite): 25% 紫色(保持), 25% 蓝色, 25% 白色, 25% 绿色

### 5.5 其他特殊随机化

- `WAN_NOTHING` 的 `oc_dir` 被随机设为 `NODIR` 或 `IMMEDIATE` (50/50)

---

## 6. BUC 状态分配

### 6.1 blessorcurse(otmp, chance)

核心函数，大多数物品类使用此函数：

```
IF otmp 已经是 blessed 或 cursed: RETURN (不改变)
IF rn2(chance) == 0:          -- 1/chance 概率触发
    IF rn2(2) == 0: curse(otmp)
    ELSE: bless(otmp)
-- 否则保持 uncursed (两个标志均为 0)
```

结果概率：
- P(cursed) = 1/(2 * chance)
- P(blessed) = 1/(2 * chance)
- P(uncursed) = 1 - 1/chance

### 6.2 curse(otmp) 函数

```
IF otmp.oclass == COIN_CLASS: RETURN  -- 金币不可诅咒
otmp.blessed = 0
otmp.cursed = 1
-- 附加效果: 双手武器解除、副手诅咒检查、运气物品更新、bag_of_holding 重量更新
-- figurine: 如果 carried 且 corpsenm 有效，附加变形计时器
-- spellbook: 中断阅读
```

### 6.3 各类别的 BUC 分配

#### WEAPON_CLASS

```
IF rn2(11) == 0:           -- 1/11 概率
    spe = rne(3)           -- 正附魔
    blessed = rn2(2)       -- 50% blessed, 50% uncursed
ELSE IF rn2(10) == 0:      -- (10/11)*(1/10) = 1/11 概率
    curse(otmp)
    spe = -rne(3)          -- 负附魔
ELSE:                      -- 约 9/11 概率
    blessorcurse(otmp, 10) -- 1/20 blessed, 1/20 cursed, 9/10 uncursed
```

综合概率约为：
- P(blessed) ~= 1/22 + (9/11)*(1/20) = 约 8.6%
- P(cursed) ~= 1/11 + (9/11)*(1/20) = 约 13.2%
- P(uncursed) ~= 78.2%

#### ARMOR_CLASS

```
IF rn2(10) != 0 AND (是 fumble_boots/levitation_boots/helm_of_opposite_alignment
                      /gauntlets_of_fumbling OR rn2(11) == 0):
    curse(otmp)
    spe = -rne(3)
ELSE IF rn2(10) == 0:
    blessed = rn2(2)
    spe = rne(3)
ELSE:
    blessorcurse(otmp, 10)
```

对于"坏"盔甲 (fumble boots 等): 90% 被诅咒，10% 走第二/第三分支。
对于普通盔甲: 约 1/11 (经过 rn2(10)!=0 且 rn2(11)==0) 被诅咒。

#### FOOD_CLASS

- tin: `blessorcurse(otmp, 10)` -- P(B)=P(C)=5%, P(U)=90%
- 其他食物: 无 BUC 处理（默认 uncursed）

#### POTION_CLASS / SCROLL_CLASS

`blessorcurse(otmp, 4)` -- P(B)=P(C)=12.5%, P(U)=75%

例外: `SCR_MAIL` 不做 blessorcurse。

#### SPBOOK_CLASS (魔法书)

`blessorcurse(otmp, 17)` -- P(B)=P(C) ~= 2.9%, P(U) ~= 94.1%

#### WAND_CLASS (魔杖)

`blessorcurse(otmp, 17)` -- 与魔法书相同

#### RING_CLASS (戒指)

- 带电荷的戒指 (oc_charged): `blessorcurse(otmp, 3)` -- P(B)=P(C) ~= 16.7%, P(U) ~= 66.7%
  - 此外若 `spe < 0` 且 `rn2(5) != 0`: 额外 80% 概率 curse
- 不带电荷的戒指:
  - teleportation/polymorph/aggravate monster/hunger: 90% cursed (`rn2(10) != 0`)
  - 其他: 10% 概率进入 curse 分支（rn2(10) != 0 且 rn2(9) == 0）

#### AMULET_CLASS (护身符)

- strangulation/change/restful sleep: 90% cursed
- 其他: `blessorcurse(otmp, 10)` -- 5% blessed, 5% cursed, 90% uncursed

#### TOOL_CLASS (工具)

| 工具子类 | BUC 规则 |
|----------|----------|
| 蜡烛 (tallow/wax) | `blessorcurse(otmp, 5)` -- 10% B, 10% C |
| 灯笼/油灯 | `blessorcurse(otmp, 5)` |
| 魔灯 | `blessorcurse(otmp, 2)` -- 25% B, 25% C |
| 油脂罐 | `blessorcurse(otmp, 10)` |
| 水晶球 | `blessorcurse(otmp, 2)` |
| 小雕像 | `blessorcurse(otmp, 4)` |
| 其他工具 | 无 BUC 处理 (默认 uncursed) |

#### GEM_CLASS (宝石)

- loadstone: 强制 `curse(otmp)`
- 其他: 无 BUC 处理 (默认 uncursed)

---

## 7. 附魔值生成

### WEAPON_CLASS

```
IF rn2(11) == 0:              -- ~9.1% 概率
    spe = rne(3)              -- [1, utmp], 几何分布 p=1/3
ELSE IF rn2(10) == 0:         -- ~8.3% 概率 (全部物品的 10/11 * 1/10)
    spe = -rne(3)
ELSE:
    spe = 0
```

### ARMOR_CLASS

```
IF (坏盔甲 OR rn2(11)==0) AND rn2(10)!=0:
    spe = -rne(3)
ELSE IF rn2(10) == 0:
    spe = rne(3)
ELSE:
    spe = 0
```

### RING_CLASS (带 oc_charged 的戒指)

```
IF rn2(10) != 0:              -- 90% 走入此分支
    IF rn2(10) != 0 AND bcsign(otmp) != 0:
        spe = bcsign(otmp) * rne(3)   -- blessed => +, cursed => -
    ELSE:
        spe = rn2(2) ? rne(3) : -rne(3)  -- 50/50 正负
ELSE:
    spe = 0
-- 之后，如果 spe == 0:
    spe = rn2(4) - rn2(3)    -- [-2, 3], 使 +0 变少见
-- 如果 spe < 0 且 rn2(5) != 0: curse
```

`rn2(4) - rn2(3)` 的分布：

| 值 | 概率 |
|----|------|
| -2 | 1/12 |
| -1 | 2/12 |
| 0 | 3/12 |
| 1 | 3/12 |
| 2 | 2/12 |
| 3 | 1/12 |

### rne(3) 分布 (hero_level < 15 时, utmp=5)

| 值 | P(值) |
|----|-------|
| 1 | 2/3 |
| 2 | 2/9 |
| 3 | 2/27 |
| 4 | 2/81 |
| 5 | 1/81 (截断: 原 2/243 + 余项) |

精确: P(k) = (1/3)^(k-1) * (2/3) for k < utmp; P(utmp) = (1/3)^(utmp-1)

---

## 8. 充能数生成

### WAND_CLASS (魔杖)

```
IF otyp == WAN_WISHING:
    spe = 1                    -- 固定 1 发
ELSE IF objects[otyp].oc_dir == NODIR:
    spe = rn1(5, 11)          -- [11, 15]
ELSE:
    spe = rn1(5, 4)           -- [4, 8]
```

NODIR 魔杖: light, secret door detection, enlightenment, create monster, wishing
其余为 IMMEDIATE 或 RAY 类。

注意: `WAN_NOTHING` 的 `oc_dir` 在游戏初始化时被随机设为 NODIR 或 IMMEDIATE，因此其充能范围取决于该局的随机结果。

### TOOL_CLASS (工具充能)

| 工具 | spe 公式 | 范围 |
|------|----------|------|
| expensive camera | `rn1(70, 30)` | [30, 99] |
| tinning kit | `rn1(70, 30)` | [30, 99] |
| magic marker | `rn1(70, 30)` | [30, 99] |
| can of grease | `rn1(21, 5)` | [5, 25] |
| crystal ball | `rn1(5, 3)` | [3, 7] |
| horn of plenty | `rn1(18, 3)` | [3, 20] |
| bag of tricks | `rn1(18, 3)` | [3, 20] |
| Bell of Opening | 固定 3 | 3 |
| magic flute / magic harp / frost horn / fire horn / drum of earthquake | `rn1(5, 4)` | [4, 8] |

### 灯具

| 工具 | age (燃油量) | spe |
|------|-------------|-----|
| tallow candle | `20 * oc_cost = 20*10 = 200` | 1 |
| wax candle | `20 * oc_cost = 20*20 = 400` | 1 |
| brass lantern | `rn1(500, 1000)` => [1000, 1499] | 1 |
| oil lamp | `rn1(500, 1000)` => [1000, 1499] | 1 |
| magic lamp | 无特殊 age | 1 (表示精灵还在) |

---

## 9. 堆叠数量生成

### WEAPON_CLASS

```
IF is_multigen(otmp):    -- 弹药和投掷武器: arrow, dart, shuriken, boomerang 等
    quan = rn1(6, 6)     -- [6, 11]
ELSE:
    quan = 1
```

`is_multigen` 定义: `oc_skill >= -P_SHURIKEN && oc_skill <= -P_BOW`
包括: arrow, elven arrow, orcish arrow, silver arrow, ya, crossbow bolt, dart, shuriken, boomerang

### FOOD_CLASS

| 食物 | 数量 |
|------|------|
| kelp frond | `rnd(2)` => [1, 2] |
| 非 corpse/meat_ring/kelp_frond 且 rn2(6)==0 | `quan = 2` (约 16.7% 概率翻倍) |
| glob | 固定 1 (重量变化代替数量) |
| 其他 | 1 |

### GEM_CLASS

| 宝石 | 数量 |
|------|------|
| rock | `rn1(6, 6)` => [6, 11] |
| loadstone | 1 |
| luckstone | 1 |
| 其他宝石: rn2(6)==0 | 2 (约 16.7% 概率) |
| 其他宝石: 默认 | 1 |

### TOOL_CLASS

| 工具 | 数量 |
|------|------|
| tallow candle | `1 + (rn2(2) ? rn2(7) : 0)` => 50%: 1, 50%: [1,7] |
| wax candle | 同上 |
| 其他 | 1 |

蜡烛数量分布：
- P(1) = 1/2 + 1/2 * 1/7 = 4/7
- P(2) = 1/2 * 1/7 = 1/14
- P(3) = 1/14, P(4) = 1/14, P(5) = 1/14, P(6) = 1/14, P(7) = 1/14

---

## 10. 侵蚀与涂油

函数 `mkobj_erosions(otmp)` 在物品初始化末尾调用。

### 前提条件 (may_generate_eroded)

以下物品**不会**生成侵蚀：
- 初始英雄物品栏 (moves <= 1 且不在 mklev 中)
- 已经是 erodeproof 的
- 不可侵蚀的 (erosion_matters 返回 false 或 is_damageable 返回 false)
- worm tooth / unicorn horn
- 神器

### 侵蚀算法

```
IF rn2(100) == 0:                 -- 1% 概率
    otmp.oerodeproof = 1          -- 防蚀
ELSE:
    -- 主要侵蚀 (rust/burn/crack)
    IF rn2(80) == 0 AND (is_flammable OR is_rustprone OR is_crackable):
        DO:
            oeroded += 1
        WHILE oeroded < 3 AND rn2(9) == 0
    -- 次要侵蚀 (rot/corrode)
    IF rn2(80) == 0 AND (is_rottable OR is_corrodeable):
        DO:
            oeroded2 += 1
        WHILE oeroded2 < 3 AND rn2(9) == 0

-- 涂油 (独立于上述分支)
IF rn2(1000) == 0:
    otmp.greased = 1              -- 0.1% 概率
```

侵蚀等级分布 (条件: 已触发 1/80)：
- 等级 1: P = 8/9 (约 88.9%)
- 等级 2: P = 1/9 * 8/9 (约 9.9%)
- 等级 3: P = 1/81 (约 1.2%)

注意：erodeproof 和 eroded 互斥。如果 rn2(100)==0 命中，则得到 erodeproof 且不会有任何侵蚀。否则两种侵蚀通道独立尝试。

### 毒涂

```
IF is_poisonable(otmp) AND rn2(100) == 0:
    otmp.opoisoned = 1            -- 1% 概率
```

另外，`permapoisoned` 的物品 (如 Grimtooth 神器) 始终设置 opoisoned = 1。

---

## 11. 神器生成

### 11.1 武器随机生成神器

```
IF artif AND rn2(20 + 10 * nartifact_exist()) == 0:
    otmp = mk_artifact(otmp, A_NONE, 99, TRUE)
```

- 第一个神器: P = 1/20
- 已存在 1 个: P = 1/30
- 已存在 2 个: P = 1/40
- 已存在 n 个: P = 1/(20 + 10n)

### 11.2 盔甲随机生成神器

```
IF artif AND rn2(40 + 10 * nartifact_exist()) == 0:
    otmp = mk_artifact(otmp, A_NONE, 99, TRUE)
```

- 第一个: P = 1/40
- 已存在 n 个: P = 1/(40 + 10n)

### 11.3 唯一物品自动神器

```
IF objects[otyp].oc_unique AND NOT otmp.oartifact:
    otmp = mk_artifact(otmp, A_NONE, 99, FALSE)
```

唯一物品 (如 Candelabrum, Bell of Opening, Book of the Dead) 自动尝试关联神器。

### 11.4 mk_artifact 选择算法

1. 遍历所有神器列表
2. 排除：已存在的、SPFX_NOGEN 标记的、唯一物品请求时排除非唯一的
3. 非对齐请求 (A_NONE): 只接受 otyp 匹配的候选
4. 从合格候选中等概率随机选一个
5. 如果没有候选，返回原物品不变

### 11.5 nartifact_exist()

遍历 `artiexist[1..NROFARTIFACTS]` 数组，计数 `.exists` 为 true 的条目。该计数同时影响武器和盔甲的神器生成概率，意味着任何途径（愿望、特殊关卡、随机生成）产生的神器都会降低后续随机生成神器的概率。

---

## 12. 容器内容物生成

函数 `mkbox_cnts(box)` 在容器初始化时调用。

### 12.1 内容物最大数量

| 容器 | 上限 n |
|------|--------|
| ice box | 20 |
| chest (locked) | 7 |
| chest (unlocked) | 5 |
| large box (locked) | 5 |
| large box (unlocked) | 3 |
| sack / oilskin sack (初始物品栏) | 0 |
| sack / oilskin sack (非初始) | 1 |
| bag of holding | 1 |
| 其他 | 0 |

实际数量: `rn2(n + 1)` => [0, n]

### 12.2 内容物选择

- ice box: 始终生成 corpse（`mksobj(CORPSE, TRUE, FALSE)`），age 设为 0，停止 ROT_CORPSE/REVIVE_MON/SHRINK_GLOB 计时器
- 其他容器: 使用 `boxiprobs` 概率表（见第 2.4 节）

### 12.3 特殊处理

- **金币数量**: `rnd(level_difficulty() + 2) * rnd(75)`，注释称这是 2.5x 关卡通常量
- **如果生成了 ROCK**: 替换为 `rnd_class(DILITHIUM_CRYSTAL, LOADSTONE)` 范围内的随机宝石
  - 若数量 > 2，减为 1
- **bag of holding 内**：
  - 不能包含 bag of holding 或 bag of tricks => 替换为 sack (spe=0)
  - 不能包含 wand of cancellation => 替换为 `rnd_class(WAN_LIGHT, WAN_LIGHTNING)` 范围内的随机魔杖

### 12.4 Schroedinger's Box

`LARGE_BOX` 在 `spe == 1` 时为 Schroedinger's Box（由 `SchroedingersBox(o)` 宏检测）。这不是通过 `mkbox_cnts` 设置的，而是在特殊关卡 Lua 脚本中设置。Royal coffers (`spe == 2`) 也是通过特殊关卡设定。

---

## 13. 特殊物品初始化

### 13.1 尸体 (CORPSE)

```
tryct = 50
DO:
    corpsenm = undead_to_corpse(rndmonnum())
WHILE (mvitals[corpsenm].mvflags & G_NOCORPSE) AND tryct-- > 0
IF tryct == 0: corpsenm = PM_HUMAN
```

尸体腐烂计时器: `start_corpse_timeout(body)`
- 蜥蜴/地衣: 不腐烂，不复活
- Rider: `REVIVE_MON` 计时器，6-67 回合 (Death 6+, 其他 12+)
- 巨魔: 每回合 1/TROLL_REVIVE_CHANCE 概率复活，2-TAINT_AGE 回合内
- 僵尸化: `rn1(15, 5)` => [5, 19] 回合
- 普通: `ROT_AGE - age + rnz(rot_adjust) - rot_adjust` 回合

### 13.2 蛋 (EGG)

```
corpsenm = NON_PM    -- 默认普通蛋
IF rn2(3) == 0:      -- 33% 概率尝试产生特定种类蛋
    FOR tryct = 200 DOWNTO 1:
        mndx = can_be_hatched(rndmonnum())
        IF mndx != NON_PM AND NOT dead_species(mndx):
            corpsenm = mndx
            BREAK
```

### 13.3 罐头 (TIN)

```
IF rn2(6) == 0:                -- 16.7% 菠菜
    set_tin_variety(otmp, SPINACH_TIN)
ELSE:                          -- 83.3% 尝试肉罐头
    FOR tryct = 200 DOWNTO 1:
        mndx = undead_to_corpse(rndmonnum())
        IF mons[mndx].cnutrit AND NOT G_NOCORPSE:
            corpsenm = mndx
            BREAK
blessorcurse(otmp, 10)
```

### 13.4 箱子/大箱子

```
olocked = rn2(5) != 0         -- 80% 上锁
otrapped = rn2(10) == 0       -- 10% 有陷阱
tknown = otrapped AND rn2(100) == 0  -- 有陷阱时 1% 可见
```

### 13.5 小雕像 (FIGURINE)

```
DO:
    corpsenm = rndmonnum_adj(5, 10)   -- 比当前难度高 5-10 的怪物
WHILE is_human(mons[corpsenm]) AND tryct++ < 30
blessorcurse(otmp, 4)
```

### 13.6 性别 (尸体/雕像/小雕像)

在 `mksobj` 中，corpsenm 确定后统一设置性别：

```
IF is_neuter(ptr): spe = CORPSTAT_NEUTER
ELSE IF is_female(ptr): spe = CORPSTAT_FEMALE
ELSE IF is_male(ptr): spe = CORPSTAT_MALE
ELSE: spe = rn2(2) ? CORPSTAT_FEMALE : CORPSTAT_MALE  -- 50/50
```

### 13.7 雕像 (STATUE)

```
corpsenm = rndmonnum()
IF NOT verysmall(mons[corpsenm]) AND rn2(level_difficulty()/2 + 10) > 10:
    -- 在雕像内放一本魔法书 (不含 novel)
    add_to_container(otmp, mkobj(SPBOOK_no_NOVEL, FALSE))
```

深度越深，雕像内含书的概率越高：
- 深度 1: P(含书) = 0 (rn2(10) 无法 > 10)，即 `level_difficulty()/2 + 10 = 10`，`rn2(10)` 范围 [0,9]，不可能 > 10
- 深度 2: `rn2(11)` 中 > 10 的仅有值不存在（因为 rn2(11) 范围 [0,10]），所以 P = 0
- 深度 4: `rn2(12)` 中 11 > 10，P = 1/12
- 深度 20: `rn2(20)` 中 11..19 > 10，P = 9/20 = 45%
- 深度 40: `rn2(30)` 中 11..29 > 10，P = 19/30 ~= 63%

### 13.8 Glob (粘液团)

```
globby = 1
quan = 1                -- 固定数量为 1
owt = objects[otyp].oc_weight   -- 初始重量 20
known = dknown = 1
corpsenm = PM_GRAY_OOZE + (otyp - GLOB_OF_GRAY_OOZE)
start_glob_timeout(otmp, 0)     -- 23-27 回合后缩小
```

缩小计时器: 每 ~25 回合减少 1 重量单位，初始 20 => 约 500 回合后消失。

### 13.9 药水特殊处理

所有非 `POT_OIL` 的药水在 `mksobj` 中统一将 `corpsenm` 字段 (重载为 `fromsink`) 设为 0（覆盖 NON_PM 初始值），POT_OIL 的 `age` 设为 `MAX_OIL_IN_FLASK` (400)。

### 13.10 Candy Bar

`assign_candy_wrapper(otmp)` 为糖果棒设置 `spe` (包装纸索引)。

---

## 14. 地下城难度与深度效应

### 14.1 level_difficulty() 函数

```
IF In_endgame:
    res = depth(sanctum_level) + hero_level / 2
ELSE IF hero_has_amulet:
    res = deepest_level_reached
ELSE:
    res = depth(u.uz)
    IF builds_up(u.uz):    -- 如 Sokoban, Vlad's Tower
        res += 2 * (entry_lev - u.uz.dlevel + 1)
```

示例 (Sokoban, 入口在地牢第 9 层, entry_lev=8):
- 在 dlevel 8: `8 + 2*(8-8+1) = 10`
- 在 dlevel 7: `7 + 2*(8-7+1) = 11`
- 在 dlevel 5 (顶层): `5 + 2*(8-5+1) = 13`

### 14.2 深度对物品生成的具体影响

| 机制 | 深度影响 | 公式/引用 |
|------|---------|-----------|
| **宝石类型分布** | 深度越深，更高价宝石解锁 | `setgemprobs`: 前 `9 - lev/3` 种清零 |
| **容器金币** | 深度越深，金币更多 | `rnd(level_difficulty()+2) * rnd(75)` |
| **雕像含书概率** | 深度越深，含书率更高 | `rn2(level_difficulty()/2 + 10) > 10` |
| **Rogue 关卡物品类** | 使用 rogueprobs 表 | 无 tool/gem/spbook/amulet |
| **Gehennom 物品类** | 使用 hellprobs 表 | 极少 potion/scroll |
| **小雕像怪物难度** | 比当前难度高 5-10 | `rndmonnum_adj(5, 10)` |
| **尸体怪物类型** | 间接通过 `rndmonnum()` | 使用 `rndmonst_adj(0, 0)` 选深度适当的怪物 |
| **商店 mimic 概率** | 深度越深越高 | `rn2(100) < depth(&u.uz)` |

### 14.3 Rogue 关卡特殊限制

- `LEVEL_SPECIFIC_NOCORPSE` 宏在 Rogue 关卡返回 true，因此**不会生成任何尸体或死亡掉落物品**
- 使用 `rogueprobs` 概率表（见 2.2）

### 14.4 deathdrops 关卡标志

`level.flags.deathdrops` 默认为 true。某些特殊关卡可通过 Lua 设为 false，此时 `LEVEL_SPECIFIC_NOCORPSE` 返回 true，完全禁止尸体和死亡掉落。

---

## 15. 死亡掉落生成

怪物死亡时的物品掉落由 `xkilled()` (玩家击杀) 和 `mondied()` (其他原因死亡) 处理。

### 15.1 怪物物品栏掉落

所有怪物死亡时 (`mondead` -> `m_detach` -> `relobj`)，其携带的全部物品 (`minvent`) 掉落到死亡地点。这包括:
- 怪物自带的武器/盔甲
- 从英雄或其他怪物偷取的物品
- 特殊关卡脚本赋予的物品

### 15.2 尸体生成 (corpse_chance)

`corpse_chance(mon, magr, was_swallowed)` 决定是否生成尸体:

```
-- 永远不生成尸体的:
IF mon is Vlad or a lich: RETURN FALSE   -- 身体化为灰尘
IF mon has AT_BOOM attack: 爆炸处理, RETURN FALSE  -- 如 gas spore
IF LEVEL_SPECIFIC_NOCORPSE(mdat): RETURN FALSE  -- Rogue 关卡 / deathdrops=false

-- 总是生成尸体的:
IF bigmonst(mdat) OR mdat == PM_LIZARD (且未被 clone):
    RETURN TRUE
IF is_golem(mdat) OR is_mplayer(mdat) OR is_rider(mdat) OR mon.isshk:
    RETURN TRUE

-- 概率生成:
tmp = 2 + ((mdat.geno & G_FREQ) < 2) + verysmall(mdat)
RETURN rn2(tmp) == 0
```

概率分析:
- 普通频率 (G_FREQ >= 2)、非极小怪物: P(尸体) = 1/2
- 稀有频率 (G_FREQ < 2)、非极小: P(尸体) = 1/3
- 普通频率、极小怪物: P(尸体) = 1/3
- 稀有且极小: P(尸体) = 1/4

### 15.3 特殊死亡掉落 (make_corpse)

根据怪物类型，`make_corpse()` 决定掉落什么:

#### 龙类 (dragon)

```
IF rn2(mon.mrevived ? 20 : 3) == 0:
    -- 掉落龙鳞 (GRAY_DRAGON_SCALES + offset)
    spe = 0, 无 BUC
PLUS: 正常尸体
```

- 首次击杀: P(龙鳞) = 1/3
- 复活过的龙: P(龙鳞) = 1/20

#### 独角兽

```
IF mon.mrevived AND rn2(2) != 0:
    -- 角化为灰尘 (不掉落)
ELSE:
    -- 掉落 unicorn horn (通过 mksobj, init=TRUE)
    IF mon.mrevived: degraded_horn = 1  -- 退化的角
PLUS: 正常尸体
```

#### Golem 类

| Golem 类型 | 掉落物 | 数量 |
|------------|--------|------|
| iron golem | iron chain | d(2,6) 个 |
| glass golem | 随机 glass gem | d(2,4) 个 |
| clay golem | rock | rn2(20)+50 个 |
| stone golem | 自身雕像 (无 CORPSTAT_INIT) | 1 |
| wood golem | quarterstaff/shield/club/spear/boomerang 混合 | d(2,4) 个 |
| rope golem | leash/bullwhip/grappling hook 混合 | rn2(3) 个 (可能为 0) |
| leather golem | leather armor/cloak/saddle | d(2,4) 个 |
| gold golem | gold: `200 - rnl(101)` | 1 堆 |
| paper golem | blank paper scroll | rnd(4) 个 |

Golem 不留下尸体，只留下构成材料。

#### 木质 Golem 选择算法

```
FOR EACH of d(2,4) items:
    IF rn2(2): QUARTERSTAFF
    ELSE IF rn2(3): SMALL_SHIELD
    ELSE IF rn2(3): CLUB
    ELSE IF rn2(3): ELVEN_SPEAR
    ELSE: BOOMERANG
```

概率: QUARTERSTAFF=1/2, SMALL_SHIELD=1/3, CLUB=1/9, ELVEN_SPEAR=1/27, BOOMERANG=1/27

[疑似 bug] 概率不完全均匀:
- SMALL_SHIELD 实际概率 = 1/2 * 2/3 = 1/3
- 但 CLUB = 1/2 * 1/3 * 2/3 = 2/18 ~= 1/9
- 设计意图可能是等概率，但嵌套 rn2 导致不等

#### 吸血鬼/木乃伊/僵尸

掉落对应的"活人"尸体 (`undead_to_corpse`)，age 减去 `TAINT_AGE + 1`（已腐烂的旧尸体）。

#### 粘液怪 (ooze/pudding/slime)

掉落对应的 glob（不是尸体），并与相邻同类 glob 合并。

### 15.4 额外宝物掉落 (xkilled 独有)

仅当**玩家击杀**怪物时 (`xkilled`)，有额外随机宝物:

```
IF rn2(6) == 0                             -- 1/6 概率
   AND NOT (mvitals[mndx].mvflags & G_NOCORPSE)
   AND (x != u.ux OR y != u.uy)           -- 不在英雄脚下
   AND mdat.mlet != S_KOP                  -- 不是 Kop
   AND NOT mtmp.mcloned:                   -- 不是克隆体
    otmp = mkobj(RANDOM_CLASS, TRUE)
    -- 过滤:
    IF otmp.oclass == FOOD_CLASS AND NOT (mdat.mflags2 & M2_COLLECT) AND NOT artifact:
        delobj(otmp)                       -- 不从非收集怪物掉落食物
    ELSE IF mdat.msize < MZ_HUMAN AND otyp != FIGURINE
            AND (otmp.owt > 30 OR objects[otyp].oc_big):
        delobj(otmp)                       -- 小怪物不掉落大物品
    ELSE:
        place_object(otmp, x, y)
```

关键限制:
- **食物过滤 (3.7 新增)**: 不收集食物的怪物不会掉落随机食物，防止后期营养过剩和前期刷怪
- **体型过滤**: 小于人类体型的怪物不掉落重于 30 或 `oc_big` (双手/笨重) 的物品
- 例外: figurine 不受体型限制

### 15.5 mondied vs xkilled

| 特征 | mondied | xkilled |
|------|---------|---------|
| 调用者 | 非玩家击杀 (怪物互杀、环境等) | 玩家击杀 |
| 额外宝物掉落 | 无 | 有 (1/6 概率) |
| 尸体生成 | 通过 corpse_chance | 通过 corpse_chance |
| 特殊尸体处理 | 通过 make_corpse | 通过 make_corpse |
| 击杀记录/对齐惩罚 | 无 | 有 |
| 经验值 | 无 | 有 |

---

## 16. 商店物品栏生成

### 16.1 商店类型概率

在 `shtypes[]` 数组中定义，概率总和 = 100:

| 商店类型 | 概率 | symb | 主要物品类 |
|----------|------|------|-----------|
| general store | 42 | RANDOM_CLASS | 任意 |
| used armor dealership | 14 | ARMOR_CLASS | 90% 盔甲, 10% 武器 |
| second-hand bookstore | 10 | SCROLL_CLASS | 90% 卷轴, 10% 魔法书 |
| liquor emporium | 10 | POTION_CLASS | 100% 药水 |
| antique weapons outlet | 5 | WEAPON_CLASS | 90% 武器, 10% 盔甲 |
| delicatessen | 5 | FOOD_CLASS | 83% 食物, 5% 果汁, 4% 酒, 5% 水, 3% 冰箱 |
| jewelers | 3 | RING_CLASS | 85% 戒指, 10% 宝石, 5% 护身符 |
| quality apparel (wand shop) | 3 | WAND_CLASS | 90% 魔杖, 5% 手套, 5% 精灵斗篷 |
| hardware store | 3 | TOOL_CLASS | 100% 工具 |
| rare books | 3 | SPBOOK_CLASS | 90% 魔法书, 10% 卷轴 |
| health food store | 2 | FOOD_CLASS | 70% 素食, 20% 果汁, 4% 治疗水, 3% 满治疗水, 2% 食物侦测卷, 1% 蜂王浆 |
| **总计** | **100** | | |

### 16.2 特殊关卡商店 (概率 = 0, 仅通过 Lua 脚本创建)

| 商店类型 | iprobs |
|----------|--------|
| lighting store | 30% wax candle, 44% tallow candle, 5% brass lantern, 9% oil lamp, 3% magic lamp, 5% POT_OIL, 2% WAN_LIGHT, 1% SCR_LIGHT, 1% SPE_LIGHT |

### 16.3 物品生成算法 (mkshobj_at)

对商店房间中的每个合法格子调用 `mkshobj_at`:

```
-- 3.6 tribute: 书店特殊处理
IF mkspecl AND (shop_name == "rare books" OR "second-hand bookstore"):
    mksobj_at(SPE_NOVEL, sx, sy, FALSE, FALSE)
    RETURN

-- Mimic 替代
IF rn2(100) < depth(&u.uz) AND NOT MON_AT(sx, sy):
    -- 生成 mimic 替代物品
ELSE:
    atype = get_shop_item(shop_index)
    IF atype == VEGETARIAN_CLASS:
        mkveggy_at(sx, sy)          -- 素食物品
    ELSE IF atype < 0:
        mksobj_at(-atype, sx, sy, TRUE, TRUE)  -- 特定物品类型
    ELSE:
        mkobj_at(atype, sx, sy, TRUE)           -- 随机该类物品
```

### 16.4 get_shop_item 算法

```
FOR j = rnd(100), i = 0; (j -= shp.iprobs[i].iprob) > 0; i++:
    continue
RETURN shp.iprobs[i].itype
```

返回值:
- 正值: 物品类 (使用 `mkobj_at` 生成随机该类物品)
- 负值: 取反后为特定 otyp (使用 `mksobj_at` 生成)
- `VEGETARIAN_CLASS`: 特殊伪类

### 16.5 素食商店物品选择 (shkveg)

```
-- 收集所有 FOOD_CLASS 中满足 veggy_item 的物品
FOR each food item i:
    IF veggy_item(NULL, i):    -- 材质为 VEGGY, 或是 EGG
        ok[j++] = i
        maxprob += objects[i].oc_prob
-- 按 oc_prob 加权随机选择
prob = rnd(maxprob)
RETURN ok[找到的索引]
```

`veggy_item` 判断: `objects[otyp].oc_material == VEGGY` 或 `otyp == EGG`

### 16.6 商店 Mimic 概率

```
P(mimic) = min(depth(&u.uz), 99) / 100
```

深度 1: 1%, 深度 50: 50%, 深度 100+: 99%

### 16.7 一格一物品 + tribute

- 商店中每个合法格子放一个物品
- 合法格子: 不在门口行、在房间内、非边缘
- 如果 tribute 系统启用且 bookstock 未设置: 随机一个格子放 SPE_NOVEL

### 16.8 saleable 检查

`saleable(shkp, obj)` 判断物品是否属于该商店类型:
- general store (RANDOM_CLASS): 所有物品都 saleable
- 其他商店: 检查 `iprobs` 表中的 `itype`
  - 正 itype: 匹配 `obj.oclass`
  - 负 itype: 匹配 `-obj.otyp`
  - VEGETARIAN_CLASS: 匹配 `veggy_item(obj, 0)`

---

## 17. 测试向量

以下测试向量假设 hero_level < 15 (utmp=5 for rne), 深度 1, 非 Rogue/Gehennom 关卡, 除非另行说明。

### TV-1: 物品类选择 -- 边界: tprob = 1

```
输入: rnd(100) 返回 1
过程: 1 - 10 (WEAPON) = -9 <= 0
输出: WEAPON_CLASS
```

### TV-2: 物品类选择 -- 边界: tprob = 100

```
输入: rnd(100) 返回 100
过程: 100-10=90, 90-10=80, 80-20=60, 60-8=52, 52-8=44,
      44-16=28, 28-16=12, 12-4=8, 8-4=4, 4-3=1, 1-1=0 <= 0
输出: AMULET_CLASS
```

### TV-3: 物品类选择 -- tprob = 97 (RING_CLASS 边界)

```
输入: rnd(100) 返回 97
过程: 97-10=87, 87-10=77, 77-20=57, 57-8=49, 49-8=41,
      41-16=25, 25-16=9, 9-4=5, 5-4=1, 1-3=-2 <= 0
输出: RING_CLASS
```

### TV-4: blessorcurse(otmp, 10) -- blessed

```
输入: rn2(10) 返回 0, rn2(2) 返回 1
过程: rn2(10)==0 触发; rn2(2)!=0 => bless
输出: blessed=1, cursed=0
```

### TV-5: blessorcurse(otmp, 10) -- uncursed

```
输入: rn2(10) 返回 3
过程: rn2(10)!=0 => 不触发
输出: blessed=0, cursed=0
```

### TV-6: rne(3) -- 最大值 (hero_level=1, utmp=5)

```
输入: rn2(3) 连续返回 0, 0, 0, 0 (四次)
过程: tmp=1, rn2(3)==0 => 2; rn2(3)==0 => 3; rn2(3)==0 => 4;
      rn2(3)==0 => 5; 5 >= utmp(5), 循环终止
输出: 5
```

### TV-7: 魔杖充能 -- wand of wishing

```
输入: otyp = WAN_WISHING
输出: spe = 1 (固定)
```

### TV-8: 魔杖充能 -- NODIR 类 (wand of light)

```
输入: otyp = WAN_LIGHT, rn2(5) 返回 3
过程: spe = rn1(5, 11) = rn2(5) + 11 = 3 + 11
输出: spe = 14
```

### TV-9: 魔杖充能 -- RAY 类 (wand of fire)

```
输入: otyp = WAN_FIRE, rn2(5) 返回 0
过程: spe = rn1(5, 4) = rn2(5) + 4 = 0 + 4
输出: spe = 4
```

### TV-10: 武器堆叠 -- arrow (multigen)

```
输入: otyp = ARROW, rn2(6) 返回 5
过程: is_multigen => true; quan = rn1(6, 6) = rn2(6) + 6 = 5 + 6
输出: quan = 11
```

### TV-11: 武器堆叠 -- long sword (non-multigen)

```
输入: otyp = LONG_SWORD
输出: quan = 1
```

### TV-12: 戒指附魔 -- 边界: spe 从 0 被修正

```
输入: 带 oc_charged 的戒指, rn2(10)=0 (第一个分支不触发), 然后 spe=0,
      rn2(4)=0, rn2(3)=2
过程: 第一分支 rn2(10)==0 => spe=0; spe==0 触发修正:
      spe = rn2(4) - rn2(3) = 0 - 2 = -2
      spe < 0, rn2(5)=1 (!= 0) => curse
输出: spe = -2, cursed = 1
```

### TV-13: 侵蚀生成 -- 边界: 最大侵蚀

```
输入: 铁质武器, moves > 1, not erodeproof, not artifact
      rn2(100) = 5 (非 0, 不防蚀)
      rn2(80) = 0 (触发主侵蚀)
      rn2(9) = 0 (oeroded 升到 2), rn2(9) = 0 (oeroded 升到 3)
      -- oeroded 已达上限 3, 循环终止
输出: oeroded = 3 (最大侵蚀)
```

### TV-14: 宝石深度 -- 边界: 深度 0, lev = 0

```
输入: lev = 0
过程: j 从 0 到 8 (共 9 种), 全部 oc_prob = 0
      第 10 种 (amber) 开始有概率
      剩余 13 种真宝石, 概率按 (171 + offset) / 13 分配
输出: dilithium 到 obsidian 的 oc_prob = 0; amber 到 jade 有非零概率
```

### TV-15: 蜡烛数量 -- 边界: 最大

```
输入: otyp = TALLOW_CANDLE, rn2(2) = 1, rn2(7) = 6
过程: quan = 1 + (true ? rn2(7) : 0) = 1 + 6 = 7
输出: quan = 7
```

### TV-16: 蜡烛数量 -- 边界: 最小

```
输入: otyp = WAX_CANDLE, rn2(2) = 0
过程: quan = 1 + (false ? rn2(7) : 0) = 1 + 0 = 1
输出: quan = 1
```

### TV-17: 容器金币 -- 深度 1

```
输入: level_difficulty() = 1, rnd(3) = 2, rnd(75) = 50
过程: quan = rnd(1+2) * rnd(75) = 2 * 50 = 100
输出: quan = 100 gold pieces
```

### TV-18: 神器概率 -- 已有 3 个神器

```
输入: nartifact_exist() = 3, rn2(50) = 0
过程: P = 1/(20 + 10*3) = 1/50; rn2(50)==0 => 触发
输出: 尝试生成神器 (mk_artifact 被调用)
```

### TV-19: 死亡掉落 -- 小怪物不掉大物品

```
输入: mdat.msize = MZ_SMALL, xkilled 额外宝物触发 (rn2(6)=0),
      mkobj 生成 TWO_HANDED_SWORD (owt=150, oc_big=1)
过程: mdat.msize < MZ_HUMAN AND otyp != FIGURINE AND oc_big => true
输出: delobj(otmp) -- 物品被删除，不掉落
```

### TV-20: 死亡掉落 -- 食物过滤

```
输入: mdat.mflags2 未含 M2_COLLECT, xkilled 额外宝物触发,
      mkobj 生成 FOOD_RATION (oclass == FOOD_CLASS), 非 artifact
过程: oclass == FOOD_CLASS AND NOT M2_COLLECT AND NOT artifact => true
输出: delobj(otmp) -- 食物被删除，不掉落
```

### TV-21: 龙鳞掉落 -- 首次击杀 vs 复活

```
输入 A: PM_RED_DRAGON, mrevived=0, rn2(3)=0
输出 A: 掉落 RED_DRAGON_SCALES (P=1/3)

输入 B: PM_RED_DRAGON, mrevived=1, rn2(20)=5
输出 B: 不掉落龙鳞 (rn2(20)!=0)
```

### TV-22: 尸体概率 -- 边界: 小型稀有怪物

```
输入: mdat.geno & G_FREQ = 1 (< 2), verysmall=TRUE, 非 big/golem/mplayer/rider/shk
过程: tmp = 2 + 1 + 1 = 4
输出: P(尸体) = 1/4 (rn2(4)==0)
```

### TV-23: 商店 -- delicatessen 物品选择

```
输入: shop_type = delicatessen, rnd(100) = 84
过程: 84-83=1, 1-5=-4 <= 0
      iprobs[1] = {5, -POT_FRUIT_JUICE}
输出: 生成 POT_FRUIT_JUICE (特定物品, atype = -POT_FRUIT_JUICE)
```

### TV-24: 商店 -- mimic 概率边界

```
输入 A: depth = 1, rn2(100) = 0 (0 < 1)
输出 A: mimic 生成 (P=1%)

输入 B: depth = 1, rn2(100) = 1 (1 >= 1)
输出 B: 正常物品生成

输入 C: depth = 100, rn2(100) = 99 (99 < 100)
输出 C: mimic 生成 (P=99%)
```

### TV-25: 容器 -- bag of holding 不含 bag of holding

```
输入: box.otyp = BAG_OF_HOLDING, 内容物随机生成 BAG_OF_HOLDING
过程: Is_mbag(otmp) => true
      otmp.otyp = SACK, otmp.spe = 0, 重算 owt
输出: 内容物变为 SACK (替代 BAG_OF_HOLDING)
```

### TV-26: 雕像含书 -- 边界: 深度 2 (不可能含书)

```
输入: level_difficulty() = 2, corpsenm 对应非极小怪物
过程: rn2(2/2 + 10) = rn2(11), 范围 [0,10], 不可能 > 10
输出: 雕像不含书 (P=0)
```

---

## 附录 A: 各类 oc_prob 总和验证

以下总和从 `objects.h` 中逐项相加得出，应全部等于 1000（NetHack 的设计约定）。

| 类别 | 总和 | 备注 |
|------|------|------|
| WEAPON_CLASS | 1000 | |
| ARMOR_CLASS | 1000 | |
| FOOD_CLASS | 1000 | 不含 corpse(0), glob(0) 等 |
| POTION_CLASS | 1000 | 含 water(80) |
| SCROLL_CLASS | 1000 | 含 blank paper(28), 不含 mail(0), 不含 extra labels(0) |
| SPBOOK_CLASS | 1000 | 含 blank paper(18) + novel(1), 不含 Book of the Dead(0) |
| WAND_CLASS | 1000 | 不含 extra wands(0) |
| RING_CLASS | 28 | 每种 1, 共 28 种 |
| AMULET_CLASS | 1000 | 不含 fake(0)/real(0) AoY |
| TOOL_CLASS | 1000 | 不含 Candelabrum(0)/Bell of Opening(0)/unicorn horn(0)/land mine(0)/beartrap(0) |
| GEM_CLASS | 动态 | 随深度变化 |
| COIN_CLASS | 1000 | 只有 gold piece |

注: RING_CLASS 总和为 28 (非 1000) 是 objects.h 中的既定设计，`mkobj` 使用 `oclass_prob_totals[class]` 即该总和作为 `rnd` 的上界，每种戒指等概率。

---

## 附录 B: 疑似 Bug 与不确定之处

1. **[疑似 bug] 洗牌算法非标准**: `shuffle()` 使用的随机选择 `j + rn2(o_high - j + 1)` 配合 do-while 跳过 `oc_name_known` 条目，不是标准 Fisher-Yates shuffle。当固定描述物品嵌入范围中间时，周围物品的概率分布可能略有偏差。实际影响因 `oc_name_known` 条目极少而微乎其微。

2. **[疑似 bug] 木质 Golem 掉落概率不均**: 使用嵌套 `rn2` 导致 QUARTERSTAFF (1/2) 远高于 BOOMERANG (1/27)。如果设计意图是等概率，应改用数组+rn2(5)。但也可能是有意设计（偏好常见木质物品）。

3. **宝石概率公式 171 常数**: `(171 + j - first) / (LAST_REAL_GEM + 1 - first)` 中 171 的来源不明确，可能是为了让概率总和在整数除法下接近某个目标值。整数除法意味着实际概率会有舍入误差。

4. **WAN_NOTHING 的 oc_dir 影响**: 由于 `oc_dir` 在初始化时被随机化，`WAN_NOTHING` 的初始充能在 [4,8] 和 [11,15] 之间二选一，这是有意设计。

5. **permapoisoned 仅限 Grimtooth**: `permapoisoned(obj)` 函数（定义在 `artifact.c`）仅对神器 Grimtooth 返回 true。普通武器的 1% 毒涂概率通过 `is_poisonable` + `rn2(100)==0` 处理。

6. **RING_CLASS 总和**: 28 种戒指各 oc_prob=1 是 objects.h 的设计。如果未来版本增减戒指，总和会变化。mkobj 对此正确处理（使用动态计算的总和）。

7. **死亡掉落食物过滤 (3.7 新增)**: `xkilled` 中对非 `M2_COLLECT` 怪物的食物掉落做了删除处理。注释明确指出这是为了防止后期营养过剩和前期刷怪。此过滤不适用于 `mondied` (非玩家击杀)。

8. **容器中 ROCK 替换**: `mkbox_cnts` 中如果 `boxiprobs` 生成了 GEM_CLASS 且选中 ROCK，会反复调用 `rnd_class(DILITHIUM_CRYSTAL, LOADSTONE)` 直到不是 ROCK。如果所有宝石 oc_prob 为 0（不可能在正常情况下发生），这将是死循环。
