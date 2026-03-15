# 魔杖/射线/爆炸系统机制规格

> 源版本: NetHack 3.7 (`zap.c` r1.584, `explode.c` r1.122, `apply.c`, `read.c`, `dig.c`)
> 提取日期: 2026-03-14

---

## 1. 魔杖分类

每种魔杖在 `objects.h` 中有一个 `oc_dir` 字段，取值为三种之一:

| oc_dir 值 | 数值 | 行为 |
|-----------|------|------|
| NODIR | 1 | 无方向，不询问方向，直接生效 |
| IMMEDIATE | 2 | 方向性射线，直线前进，不反弹，可穿透多个目标(每个 -3 射程) |
| RAY | 3 | 方向性射线，直线前进，可在墙壁/门上反弹，穿过怪物时减少射程 |

### 1.1 NODIR 魔杖列表

| 魔杖 | 常量名 | 效果概述 |
|------|--------|---------|
| 光明之杖 | WAN_LIGHT | 照亮所在房间；对精灵(gremlin)形态造成伤害 |
| 暗门探测之杖 | WAN_SECRET_DOOR_DETECTION | 揭示附近暗门/暗道 |
| 启蒙之杖 | WAN_ENLIGHTENMENT | 显示角色状态信息(enlightenment) |
| 召唤怪物之杖 | WAN_CREATE_MONSTER | 创建 1 个怪物(1/23 概率创建 2..8 个) |
| 许愿之杖 | WAN_WISHING | 获得一次许愿(若 Luck + rn2(5) < 0 则失败) |

### 1.2 IMMEDIATE 魔杖列表

| 魔杖 | 常量名 |
|------|--------|
| 无效之杖 | WAN_NOTHING |
| 打击之杖 | WAN_STRIKING |
| 隐身之杖 | WAN_MAKE_INVISIBLE |
| 减速之杖 | WAN_SLOW_MONSTER |
| 加速之杖 | WAN_SPEED_MONSTER |
| 不死回转之杖 | WAN_UNDEAD_TURNING |
| 变形之杖 | WAN_POLYMORPH |
| 抵消之杖 | WAN_CANCELLATION |
| 传送之杖 | WAN_TELEPORTATION |
| 开锁之杖 | WAN_OPENING |
| 关门之杖 | WAN_LOCKING |
| 探索之杖 | WAN_PROBING |

**IMMEDIATE 射程**: `rn1(8, 6)` = 随机 6..13 格。射线沿直线行进，对路径上每个怪物调用 `bhitm()`，对路径上每堆物品调用 `bhito()`。**可穿透多个目标**: 每命中一个怪物 `range -= 3`, 命中物品堆 `range -= 1`。射线遇到墙壁/关闭的门停止。**不反弹**。途中还对每格调用 `zap_map()` 处理地形效果(暗门/陷阱/门/刻字等)。注意: 如果 `bhitm()` 返回非零(仅 SPE_KNOCK 击退时)，射线提前终止。

### 1.3 RAY 魔杖列表

| 魔杖 | 常量名 | ZT 类型 | 伤害骰(nd) |
|------|--------|---------|-----------|
| 掘洞之杖 | WAN_DIGGING | (特殊) | 无直接伤害 |
| 魔法飞弹之杖 | WAN_MAGIC_MISSILE | ZT_MAGIC_MISSILE (0) | 2 |
| 火焰之杖 | WAN_FIRE | ZT_FIRE (1) | 6 |
| 冰霜之杖 | WAN_COLD | ZT_COLD (2) | 6 |
| 沉睡之杖 | WAN_SLEEP | ZT_SLEEP (3) | 6 |
| 死亡之杖 | WAN_DEATH | ZT_DEATH (4) | 6 |
| 闪电之杖 | WAN_LIGHTNING | ZT_LIGHTNING (5) | 6 |

RAY 魔杖的射线通过 `dobuzz()` 处理，可反弹、可被反射。

---

## 2. 射线传播算法 (dobuzz)

### 2.1 基本参数

```
输入:
  type: 0..9=玩家魔杖, 10..19=玩家法术, 20..29=玩家龙息
        -10..-19=怪物法术, -20..-29=怪物龙息, -30..-39=怪物魔杖
  nd: 伤害骰数量
  sx, sy: 起始坐标
  dx, dy: 方向增量 (-1, 0, 或 1 各轴)

初始化:
  range = rn1(7, 7)  // 随机 7..13
  如果 dx == 0 且 dy == 0:  // 垂直射线
    range = 1
```

### 2.2 主循环伪代码

```
WHILE range > 0:
  range -= 1
  sx += dx; sy += dy

  IF 不在地图内 OR 地形 == STONE:
    GOTO make_bounce

  // 如果不是火球且不是毒气，处理地形效果
  IF NOT fireball AND NOT poison_gas:
    range += zap_over_floor(sx, sy, ...)  // 可能返回负值减少射程

  // 击中怪物
  IF 怪物在 (sx,sy):
    IF 火球: BREAK (火球在击中处爆炸)
    IF zap_hit(怪物AC, spell_type):
      IF mon_reflects(怪物):
        dx = -dx; dy = -dy  // 反射: 反转方向
      ELSE:
        dmg = zhitm(怪物, type, nd, &被摧毁的护甲)
        // 处理伤害、死亡等
      range -= 2  // zap_hit 成功即消耗，无论反射还是实际命中
    ELSE:
      miss  // 未命中

  // 击中玩家
  ELSE IF 玩家在 (sx,sy) AND range >= 0:
    IF 骑乘 AND 1/3 概率 AND 坐骑无反射:
      作为怪物命中处理
    ELSE IF zap_hit(玩家AC, 0):
      range -= 2  // zap_hit 成功即消耗，在反射检查之前
      IF Reflecting:
        dx = -dx; dy = -dy  // 反射
      ELSE:
        zhitu(type, nd, ...)  // 受到伤害

  // 墙壁/关闭的门 -> 反弹
  IF 不可通行位置 OR (关闭的门 AND range >= 0):
    make_bounce:
    bchance = 不在地图内或石头 ? 10
            : 矿坑层且是墙壁 ? 20
            : 75  // 默认

    IF 火球:
      sx = 上一步位置; sy = 上一步位置
      BREAK  // 火球在障碍物前爆炸
    ELSE:
      bounce_dir(sx, sy, &dx, &dy, bchance)
```

### 2.3 反弹方向算法 (bounce_dir)

```
输入: sx, sy (当前位置), ddx, ddy (当前方向), bounceback (1/n 原路返回概率)

IF ddx == 0 OR ddy == 0 OR (bounceback > 0 AND rn2(bounceback) == 0):
  // 对角线分量缺失或随机原路返回
  ddx = -ddx
  ddy = -ddy
ELSE:
  // 尝试确定哪个轴可以反弹
  bounce = 0
  lsy = sy - ddy  // 横向移动后的位置
  lsx = sx - ddx  // 纵向移动后的位置

  IF (sx, lsy) 是可通行位置:
    bounce = 1  // 可以反转 Y 轴
  IF (lsx, sy) 是可通行位置:
    IF bounce == 0 OR rn2(2):
      bounce = 2  // 可以反转 X 轴

  SWITCH bounce:
    case 0: ddx = -ddx; ddy = -ddy  // 原路返回
    case 1: ddy = -ddy              // 只反转 Y
    case 2: ddx = -ddx              // 只反转 X
```

**注意**: `bchance` 值表示原路返回的 1/n 概率:
- 石头/地图外: 1/10 (10% 概率原路返回，否则正常反弹)
- 矿坑墙壁: 1/20
- 通常墙壁: 1/75

### 2.4 火球特殊处理

火球(SPE_FIREBALL 即 `type == ZT_SPELL(ZT_FIRE)`)不在射线路径上造成伤害。当射线碰到怪物或墙壁时，在该位置爆炸:

```
explode(sx, sy, type, d(12, 6), 0, EXPL_FIERY)
```

伤害: `d(12, 6)` (12d6)，类型为火焰，3x3 范围。

---

## 3. 命中判定 (zap_hit)

```
输入: ac (目标 AC), type (英雄法术类型，用于技能加成)

chance = rn2(20)  // 0..19
spell_bonus = type ? spell_hit_bonus(type) : 0

IF chance == 0:
  // 5% 的小概率即使裸体也能闪避
  RETURN rnd(10) < ac + spell_bonus

ac = AC_VALUE(ac)  // 限制: 至少为 -128 + 10 = 有效 AC
RETURN (3 - chance < ac + spell_bonus)
```

**`spell_hit_bonus(skill)` 详细公式** (仅英雄法术，`type != 0` 时调用):

```
hit_bon = 0
SWITCH P_SKILL(spell_skilltype(skill)):
  Restricted/Unskilled: hit_bon = -4
  Basic:                hit_bon = 0
  Skilled:              hit_bon = +2
  Expert:               hit_bon = +3

dex = ACURR(A_DEX)
IF dex < 4:      hit_bon -= 3
ELSE IF dex < 6: hit_bon -= 2
ELSE IF dex < 8: hit_bon -= 1
ELSE IF dex < 14: hit_bon -= 0
ELSE:            hit_bon += dex - 14

RETURN hit_bon
```

范围: 技能 -4..+3, DEX -3..+(DEX-14)。魔杖 zap 时 `type == 0`，`spell_bonus` 固定为 0。

**解读**: 基本命中阈值为 `ac + spell_bonus > 3 - chance`。chance 范围 0..19，所以:
- AC 10 (无甲): 只有 chance == 0 时才可能 miss
- AC -10: chance > 13 时 miss

---

## 4. 反射机制

### 4.1 玩家反射源 (Reflecting 宏)

检查顺序(外层到内层):
1. **盾牌**: 反射之盾 (SHIELD_OF_REFLECTION) -> `EReflecting & W_ARMS`
2. **武器**: 反射属性的神器武器 -> `EReflecting & W_WEP`
3. **护符**: 反射之护符 (AMULET_OF_REFLECTION) -> `EReflecting & W_AMUL`
4. **身甲**: 银龙鳞甲/银龙鳞片 -> `EReflecting & W_ARM`
5. **固有属性**: `HReflecting` (来自某些多态形态或神器)

### 4.2 怪物反射源 (mon_reflects)

检查顺序:
1. 盾牌 == SHIELD_OF_REFLECTION
2. 武器是有反射属性的神器 (`arti_reflects()`)
3. 护符 == AMULET_OF_REFLECTION
4. 身甲 == SILVER_DRAGON_SCALES 或 SILVER_DRAGON_SCALE_MAIL
5. 怪物种族 == PM_SILVER_DRAGON 或 PM_CHROMATIC_DRAGON

### 4.3 反射时的行为

- RAY 类型射线: 方向完全反转 (`dx = -dx; dy = -dy`)
- 怪物反射: 射线继续以相反方向传播
- 玩家反射: 射线继续以相反方向传播
- **反射也消耗额外射程**: `range -= 2` 在 `zap_hit()` 成功时无条件执行，无论目标是否反射。对怪物，`range -= 2` 在 `mon_reflects` if/else 块之外 (zap.c line 4937)；对玩家，`range -= 2` 在 `Reflecting` 检查之前 (zap.c line 4949)
- IMMEDIATE 类型射线**无法**被反射 (没有 `mon_reflects` 检查)
- 毒气射线: 反射后不在该格留下毒气云

### 4.4 不可反射的效果

- 所有 IMMEDIATE 魔杖/法术 (打击、减速、传送等)
- 掘洞射线 (WAN_DIGGING / SPE_DIG) -- 属于 RAY 但走 `zap_dig()` 专用路径
- NODIR 魔杖

---

## 5. 每种射线的伤害公式 (zhitm - 对怪物)

所有伤害公式中 `nd` = 伤害骰数量。

| 射线类型 | 基础伤害 | 特殊规则 |
|---------|---------|---------|
| ZT_MAGIC_MISSILE | d(nd, 6) | 魔法抗性(resists_magm)完全免疫 |
| ZT_FIRE | d(nd, 6) | 火焰抗性免疫; 冰霜抗性者额外 +7 伤害; 可烧毁护甲和物品 |
| ZT_COLD | d(nd, 6) | 冰霜抗性免疫; 火焰抗性者额外 +d(nd,3) 伤害; 1/3 概率摧毁物品 |
| ZT_SLEEP | 0 伤害 | 沉睡效果: `sleep_monst(mon, d(nd, 25), ...)` |
| ZT_DEATH(死亡) | mon.mhp + 1 | 非生物/恶魔/魔法抗性免疫; Death 怪物恢复 HP; 无豁免 |
| ZT_DEATH(分解) | MAGIC_COOKIE (1000) 或盾/甲被摧毁 | 分解抗性免疫; 先摧毁盾牌，再身甲+披风，最后杀死 |
| ZT_LIGHTNING | d(nd, 6) | 电击抗性免疫伤害但仍可致盲; nd > 2 时致盲 rnd(50) 回合; 1/3 概率摧毁物品 |
| ZT_POISON_GAS | d(nd, 6) | 毒素抗性免疫 |
| ZT_ACID | d(nd, 6) | 酸液抗性免疫; 1/6 概率腐蚀武器; 1/6 概率腐蚀护甲 |

### 5.1 伤害修正

```
// 骑士双倍伤害(仅英雄法术)
IF is_hero_spell(type) AND Role == Knight AND 有探索圣物:
  tmp *= 2

// 抗性减半(怪物有豁免骰)
IF tmp > 0 AND type >= 0 AND resist(mon, ...):
  tmp /= 2
```

### 5.2 法术伤害加成 (spell_damage_bonus)

```
输入: dmg (基础伤害)

IF Int <= 9:
  IF dmg > 1: dmg = max(1, dmg - 3)  // -3 但不低于 1
ELSE IF Int <= 13 OR level < 5:
  不变
ELSE IF Int <= 18:
  dmg += 1
ELSE IF Int <= 24 OR level < 14:
  dmg += 2
ELSE:  // Int >= 25
  dmg += 3
```

### 5.3 对英雄的伤害 (zhitu)

大致与 zhitm 相同，但:
- Half_spell_damage 对魔杖和法术(abstyp < 20)减半: `dam = (dam + 1) / 2`
- 龙息不受 Half_spell_damage 影响
- 死亡射线: 非生物/恶魔形态或魔法抗性可存活，否则直接死亡
- 分解龙息: 先检查分解抗性，再检查物品保护(inventory_resistance_check)，然后盾牌->身甲->死亡
- 闪电还会致盲: `flashburn(d(nd, 50), TRUE)` — 注意: 这个致盲效果在命中/未命中/反射判定之后无条件执行 (zap.c line 4974)，即使射线未命中 (whiz by) 或被反射，只要玩家在射线路径上就会致盲

---

## 6. IMMEDIATE 魔杖效果详解 (bhitm)

### 6.1 打击之杖 (WAN_STRIKING)

```
命中判定: u.uswallow ? 自动命中 : rnd(20) < 10 + find_mac(mon)
伤害: d(2, 12)
双倍: 骑士+探索圣物时 *= 2
法术版(SPE_FORCE_BOLT): 额外 spell_damage_bonus()
魔法抗性: 完全免疫(shieldeff + "Boing!")
```

### 6.2 减速之杖 (WAN_SLOW_MONSTER)

- 调用 `mon_adjust_speed(mon, -1, obj)`
- 如果被吞噬且吞噬者是旋风类: 强制喷出
- 可被 resist() 豁免

### 6.3 加速之杖 (WAN_SPEED_MONSTER)

- 调用 `mon_adjust_speed(mon, +1, obj)`
- 友好手势(不激怒和平怪物)
- 可被 resist() 豁免

### 6.4 不死回转之杖 (WAN_UNDEAD_TURNING)

- 复活地面上的尸体 (`unturn_dead()`)
- 对不死生物: 伤害 `rnd(8)`, 骑士双倍, 法术版有 spell_damage_bonus
- 不死生物若未 resist: 逃跑 (`monflee`)

### 6.5 变形之杖 (WAN_POLYMORPH)

- 魔法抗性免疫
- 1/25 概率系统冲击致死 (自然变形者除外)
- 否则 `newcham()` 变为随机形态
- 长虫特殊处理: 防止同一次 zap 反复变形

### 6.6 抵消之杖 (WAN_CANCELLATION)

- 调用 `cancel_monst()`: 设置 `mcan = 1`
- 粘土魔像: 死亡
- 变形者: 强制回原形
- 可被 resist() 豁免

### 6.7 传送之杖 (WAN_TELEPORTATION)

- 调用 `u_teleport_mon()` 传送怪物

### 6.8 隐身之杖 (WAN_MAKE_INVISIBLE)

- 调用 `mon_set_minvis(mon, FALSE)`

### 6.9 开锁之杖 (WAN_OPENING)

- 被吞噬: 释放
- 释放陷阱中的怪物
- 法术版(SPE_KNOCK): 小体型怪物被击退 `rnd(2)` 格

### 6.10 关门之杖 (WAN_LOCKING)

- 关闭怪物所在陷阱 (`closeholdingtrap`)

### 6.11 探索之杖 (WAN_PROBING)

- 显示怪物状态和背包

### 6.12 无效之杖 (WAN_NOTHING)

- 无效果

---

## 7. NODIR 魔杖效果详解 (zapnodir)

### 7.1 光明之杖 (WAN_LIGHT)

- 照亮房间 (`litroom(TRUE, obj)`)
- 对精灵形态的英雄造成少量伤害 (`lightdamage`)

### 7.2 暗门探测之杖 (WAN_SECRET_DOOR_DETECTION)

- 调用 `findit()` 揭示暗门和暗道

### 7.3 召唤怪物之杖 (WAN_CREATE_MONSTER)

```
数量 = rn2(23) == 0 ? rn1(7, 2) : 1
       // 1/23 概率产生 2..8 个, 否则 1 个
```

### 7.4 许愿之杖 (WAN_WISHING)

```
IF Luck + rn2(5) < 0:
  "Unfortunately, nothing happens."  // 许愿失败
ELSE:
  makewish()  // 正常许愿
```

### 7.5 启蒙之杖 (WAN_ENLIGHTENMENT)

- 显示完整的角色状态信息

---

## 8. 魔杖充能系统

### 8.1 初始充能 (mkobj.c)

```
IF otyp == WAN_WISHING:
  spe = 1                     // 固定 1 次
ELSE IF oc_dir == NODIR:
  spe = rn1(5, 11) = 11..15  // NODIR 魔杖: 11 到 15 次
ELSE:
  spe = rn1(5, 4) = 4..8     // IMMEDIATE 和 RAY 魔杖: 4 到 8 次
```

| 魔杖类别 | 充能范围 | 期望值 |
|---------|---------|-------|
| 许愿之杖 | 1 (固定) | 1 |
| NODIR (光明, 暗门探测, 启蒙, 召唤怪物) | 11..15 | 13 |
| IMMEDIATE (打击, 减速, 加速, 变形, 抵消, 传送, 开锁, 关门, 探索, 隐身, 不死回转, 无效) | 4..8 | 6 |
| RAY (魔法飞弹, 火焰, 冰霜, 沉睡, 死亡, 闪电, 掘洞) | 4..8 | 6 |

BUC 状态: `blessorcurse(obj, 17)` -- 1/17 概率诅咒或祝福 (各约 5.9%), 否则未诅咒。
`recharged` 字段初始为 0。

### 8.2 使用判定 (zappable)

```
zappable(wand):
  IF wand.spe < 0:
    RETURN 0  // 已耗尽
  IF wand.spe == 0 AND rn2(121) != 0:  // WAND_WREST_CHANCE = 121
    RETURN 0  // 无充能，未成功榨取
  IF wand.spe == 0:
    "You wrest one last charge from the worn-out wand."
  wand.spe -= 1
  RETURN 1
```

**关键细节**:
- `spe == 0` 时有 1/121 的概率榨取最后一次充能
- 榨取成功后 `spe` 变为 -1
- `spe < 0` 的魔杖无法再使用
- `spe < 0` 时会 "turn to dust" 并销毁

---

## 9. 魔杖充能 (recharge)

### 9.1 充能上限

```
lim = WAN_WISHING ? 1
    : oc_dir != NODIR ? 8   // IMMEDIATE 和 RAY
    : 15                     // NODIR
```

### 9.2 过充爆炸概率

```
n = obj.recharged  // 之前充能次数 (0..7, 3-bit field)

IF n > 0 AND (obj == WAN_WISHING OR n^3 > rn2(343)):
  爆炸!

// 概率表:
// n=0: 0% (从不爆炸)
// n=1: 1/343 = 0.29%
// n=2: 8/343 = 2.33%
// n=3: 27/343 = 7.87%
// n=4: 64/343 = 18.66%
// n=5: 125/343 = 36.44%
// n=6: 216/343 = 62.97%
// n=7: 343/343 = 100%

// 许愿之杖: 只要 n > 0 就必定爆炸
```

### 9.3 充能量

```
IF 诅咒:
  stripspe(obj)  // spe 设为 0 或 -1
ELSE:
  IF lim == 1 (许愿之杖):
    n = 1
  ELSE:
    n = rn1(5, lim + 1 - 5)  // = rn1(5, lim - 4) = (lim-4)..(lim)
    IF 不是祝福:
      n = rnd(n)  // 1..n

  IF obj.spe < n:
    obj.spe = n
  ELSE:
    obj.spe += 1  // 如果当前已经达到或超过随机值，只 +1

// 许愿之杖 spe > 3 时强制爆炸 (当前不可达，但代码存在)
```

### 9.4 充能爆炸伤害 (wand_explode)

```
n = obj.spe + chg  // chg 为充能来源提供的值 (recharge: rnd(lim))
IF n < 2: n = 2

k = SWITCH obj.otyp:
  WAN_WISHING: 12
  WAN_CANCELLATION, WAN_DEATH, WAN_POLYMORPH, WAN_UNDEAD_TURNING: 10
  WAN_COLD, WAN_FIRE, WAN_LIGHTNING, WAN_MAGIC_MISSILE: 8
  WAN_NOTHING: 4
  其他: 6

dmg = d(n, k)
losehp(Maybe_Half_Phys(dmg), "exploding wand", ...)
```

---

## 10. 诅咒魔杖反噬 (backfire)

```
// dozap() 中: 诅咒魔杖 zap 时
IF obj.cursed AND rn2(100) == 0:  // WAND_BACKFIRE_CHANCE = 100, 即 1% 概率
  backfire(obj)

backfire(obj):
  dmg = d(obj.spe + 2, 6)
  losehp(Maybe_Half_Phys(dmg), "exploding wand", ...)
  useupall(obj)  // 魔杖销毁
```

---

## 11. 魔杖自爆 (do_break_wand / apply)

### 11.1 前置条件

- 需要有手(`!nohands`)
- 需要空手(`freehand`)
- 力量要求: 易碎(balsa/glass) >= 5, 其他 >= 10
- 需确认(paranoid_query)

### 11.2 充能复原

```
// 调用 zappable() 消耗一次充能，然后恢复
obj.spe++
// 如果是榨取的最后一次 (spe 从 -1 到 0 再到 0 后的处理):
IF obj.spe == 0:
  obj.spe = rnd(3)  // 给予 1..3 充能
```

### 11.3 基础伤害

```
dmg = obj.spe * 4  // 基于剩余充能
```

### 11.4 分类处理

**无爆炸效果组** (只显示 "But nothing else happens..."):
- WAN_OPENING (除非被吞噬时释放)
- WAN_WISHING
- WAN_NOTHING
- WAN_LOCKING
- WAN_PROBING
- WAN_ENLIGHTENMENT
- WAN_SECRET_DOOR_DETECTION

**大型爆炸组** (调用 `broken_wand_explode` -> `explode()`):

| 魔杖 | 爆炸伤害 | 爆炸类型 |
|------|---------|---------|
| WAN_DEATH | dmg * 4 = spe * 16 | EXPL_MAGICAL |
| WAN_LIGHTNING | dmg * 4 = spe * 16 | EXPL_MAGICAL |
| WAN_FIRE | dmg * 2 = spe * 8 | EXPL_FIERY |
| WAN_COLD | dmg * 2 = spe * 8 | EXPL_FROSTY |
| WAN_MAGIC_MISSILE | dmg = spe * 4 | EXPL_MAGICAL |

这些爆炸调用 `explode()` (3x3 范围)，然后魔杖销毁，**不再有逐格效果**。

**逐格效果组** (先 `explode()` 再遍历周围 9 格):

| 魔杖 | 特殊处理 |
|------|---------|
| WAN_STRIKING | dmg 重算为 `d(1 + spe, 6)`, 显示 "A wall of force..." |
| WAN_CANCELLATION | 对周围怪物/物品施加效果 |
| WAN_POLYMORPH | 对周围怪物/物品施加效果 |
| WAN_TELEPORTATION | 对周围怪物/物品施加效果 |
| WAN_UNDEAD_TURNING | 对周围怪物/物品施加效果 |
| WAN_DIGGING | 在周围挖坑/洞 |
| WAN_CREATE_MONSTER | 在玩家附近创建怪物 |
| WAN_SLOW_MONSTER | 对周围怪物施加效果 |
| WAN_SPEED_MONSTER | 对周围怪物施加效果 |
| WAN_MAKE_INVISIBLE | 对周围怪物施加效果 |
| WAN_LIGHT | 照亮房间 |

逐格效果组的爆炸伤害:
```
explode(ox, oy, -(obj.otyp), rnd(dmg), WAND_CLASS, EXPL_MAGICAL)
// 即 rnd(spe * 4) 伤害，WAND_CLASS，魔法爆炸
```

### 11.5 逐格遍历顺序

```
FOR i = 0 TO 8:  // N_DIRS = 8, 加上自身位置
  x = obj.ox + xdir[i]
  y = obj.oy + ydir[i]
  // xdir/ydir: 8 个方向 + (0,0) 在最后
  // 玩家自身最后处理
```

### 11.6 折断后对英雄的额外伤害

对英雄调用 `zapyourself(obj, FALSE)`:
- `ordinary == FALSE` 影响某些魔杖的伤害计算
- 打击之杖: `d(1 + spe, 6)` (而非正常 zap 的 `d(2, 12)`)
- 光明之杖: `d(spe, 25)` 致盲
- 沉睡之杖: 沉睡 `rnd(50)` 回合

---

## 12. 爆炸系统 (explode)

### 12.1 范围

始终为 3x3 (中心点 + 8 个相邻格)。每个格独立检查。

### 12.2 伤害

```
输入: dam (基础伤害值)

对怪物:
  IF 抗性(shieldeff):
    仅物品损毁伤害(itemdmg) + golemeffects
  ELSE:
    mdam = dam
    IF resist(mon, olet, 0, FALSE):
      mdam = (dam + 1) / 2  // 豁免成功减半(向上取整)
    IF 被抓取 AND 怪物是抓取者 AND 在爆炸半径内:
      mdam *= 2  // 抓取者双倍伤害
    IF resists_cold(mon) AND adtyp == AD_FIRE:
      mdam *= 2  // 冰霜抗性怪物对火焰双倍脆弱
    IF resists_fire(mon) AND adtyp == AD_COLD:
      mdam *= 2  // 火焰抗性怪物对冰霜双倍脆弱
    mon.mhp -= mdam + itemdmg

对英雄:
  damu = dam  // 初始值
  IF olet == WAND_CLASS:  // 折断魔杖的爆炸
    SWITCH Role:
      Cleric, Monk, Wizard: damu /= 5
      Healer, Knight: damu /= 2
      其他: 不变
  IF Invulnerable: damu = 0
  ELSE IF adtyp == AD_PHYS OR adtyp == AD_ACID:
    damu = Maybe_Half_Phys(damu)
  IF uhurt == 2 (无抗性):
    IF 正在抓取怪物: damu *= 2
    // 注意: 英雄没有"冰火互克"双倍伤害 [疑似 bug 或有意为之，源代码有注释 "why not?"]
    u.uhp -= damu
```

### 12.3 噪音

```
noise = dam * dam
IF noise < 50: noise = 50
IF 被吞噬: noise = (noise + 3) / 4
wake_nearto(x, y, noise)
```

### 12.4 分解射线爆炸特殊规则

当 `olet == WAND_CLASS` 且 `adtyp == AD_DISN` (死亡之杖折断):
- 对怪物: 检查 `nonliving` 或 `is_demon` 或 `is_vampshifter` (而非 `resists_disint`)
- 对英雄: 检查 `nonliving` 或 `is_demon`
- 这与分解龙息(检查 `Disint_resistance`)不同

---

## 13. 吞噬时使用魔杖的特殊规则

### 13.1 IMMEDIATE 魔杖 (weffects -> bhitm)

被吞噬时(u.uswallow)，IMMEDIATE 魔杖直接作用于吞噬者:
```
bhitm(u.ustuck, obj)
```
不会有 "穿透" 或方向选择。

### 13.2 RAY 魔杖 (dobuzz 中的 u.uswallow)

```
IF u.uswallow AND type >= 0:  // 英雄的射线
  tmp = zhitm(u.ustuck, type, nd, &otmp)
  // 正常伤害处理
  IF tmp == MAGIC_COOKIE:  // 分解
    u.ustuck.mhp = 0  // "只是打了个洞"
  IF DEADMONSTER(u.ustuck):
    killed(u.ustuck)
  RETURN  // 被吞噬时射线不传播
```

**注意**: 怪物射线 (`type < 0`) 在吞噬状态下直接返回不执行。

### 13.3 掘洞之杖 (zap_dig 中的 u.uswallow)

```
IF 吞噬者非旋风类:
  IF 消化类: "You pierce <mon>'s stomach wall!"
  IF 独特怪物: mhp = (mhp + 1) / 2
  ELSE: mhp = 1
  expels(mon, ...)  // 强制喷出
```

### 13.4 NODIR 魔杖

被吞噬时 NODIR 魔杖正常工作(光明、许愿等不涉及方向)。

---

## 14. 地形/物品效果 (zap_over_floor)

### 14.1 火焰射线 (ZT_FIRE)

| 目标 | 效果 | 射程修正 |
|------|------|---------|
| 蛛网(陷阱) | 烧毁 | 0 |
| 冰面 | 融化为水 | 0 |
| 水池(POOL) | 蒸发 -> 变成 ROOM + PIT; 产生蒸汽云 `rnd(5)` 范围 | -3 |
| 护城河/深水 | 部分蒸发; 产生蒸汽云 | 0 |
| 喷泉 | 干涸; 产生蒸汽云 `rnd(3)` | -1 |
| 地面卷轴/法术书 | 1/3 概率逐个烧毁 (SCR_FIRE 和 SPE_FIREBALL 免疫) | 0 |
| 暗门 | 揭示 | 0 |
| 关闭的门 | 完全烧毁 (D_NODOOR) | -1000 (射程终止) |

### 14.2 冰霜射线 (ZT_COLD)

| 目标 | 效果 | 射程修正 |
|------|------|---------|
| 水池/护城河 | 结冰(ICE/ICED_MOAT); 启动融化计时器 | -3 |
| 岩浆 | 冷却固化为 ROOM | -3 |
| 深水(WATER) | 暂时结冰然后恢复 | -1000 (射程终止) |
| 岩浆墙 + rn2(chance) | 暂时结冰然后恢复 | -1000 |
| 已有冰面 | 延长融化计时器 | 0 |
| 暗门 | 揭示 | 0 |
| 关闭的门 | 冻碎 (D_NODOOR) | -1000 |

### 14.3 闪电射线 (ZT_LIGHTNING)

| 目标 | 效果 | 射程修正 |
|------|------|---------|
| 铁栏 | 1/10 概率融化(若非 W_NONDIGGABLE) | -3 |
| 暗门 | 揭示 | 0 |
| 关闭的门 | 劈裂 (D_BROKEN) | -1000 |

### 14.4 酸液射线 (ZT_ACID)

| 目标 | 效果 | 射程修正 |
|------|------|---------|
| 铁栏 | 必然腐蚀(若非 W_NONDIGGABLE) | -3 |

### 14.5 毒气射线 (ZT_POISON_GAS)

- 在可通行位置创建 1x1 毒气云: `create_gas_cloud(x, y, 1, 8)` (范围 1, 伤害 8)

### 14.6 死亡/分解射线 (ZT_DEATH)

- 分解龙息对关闭的门: 完全分解 (D_NODOOR)
- 死亡射线/法术: 不影响门 (进入 default 分支)

---

## 15. 物品销毁 (destroy_items / maybe_destroy_item)

### 15.1 火焰 (AD_FIRE)

| 物品类别 | 效果 | 伤害 | 每件销毁概率 |
|---------|------|------|------------|
| 药水 (非油) | 沸腾爆炸 | rnd(6) | 1/3 |
| 油药水 | 点燃爆炸 | rnd(6) | 1/3 |
| 卷轴 | 燃烧 | 1 | 1/3 |
| 法术书 | 燃烧 (死者之书免疫) | 1 | 1/3 |
| 绿色泥怪球 | 爆炸 | (owt+19)/20 | 1/3 |

火焰抗性保护非药水类物品(但药水仍然会被破坏)。

### 15.2 冰霜 (AD_COLD)

| 物品类别 | 效果 | 伤害 | 每件销毁概率 |
|---------|------|------|------------|
| 药水 | 冻裂 | rnd(4) | 1/3 |

### 15.3 电击 (AD_ELEC)

| 物品类别 | 效果 | 伤害 | 每件销毁概率 |
|---------|------|------|------------|
| 戒指 | 碎裂消失 | 0 | 1/3 (带电戒指 2/3 概率充能而非销毁) |
| 魔杖 | 爆炸碎裂 | rnd(10) | 1/3 |

电击抗性保护非戒指类物品。
戴着非金属手套的已装备戒指免疫；电击抗性戒指免疫。

### 15.4 保护机制

- `inventory_resistance_check(dmgtyp)` 检查外穿物品是否提供对应抗性
- `obj_resists(obj, prob1, prob2)`: 神器和某些特殊物品有额外抗性

---

## 16. RAY 魔杖的 nd (伤害骰数) 参数

```
// weffects() 中:
WAN_MAGIC_MISSILE: nd = 2
WAN_FIRE:          nd = 6
WAN_COLD:          nd = 6
WAN_SLEEP:         nd = 6
WAN_DEATH:         nd = 6
WAN_LIGHTNING:     nd = 6

// 法术版:
SPE_MAGIC_MISSILE..SPE_FINGER_OF_DEATH: nd = u.ulevel / 2 + 1
```

---

## 17. 自我 zap (zapyourself)

当英雄对自己使用魔杖 (方向为 `.` 即 dx=dy=dz=0):

| 魔杖 | 效果 |
|------|------|
| WAN_STRIKING | d(2, 12) 伤害 (Antimagic 免疫, "Boing!") |
| WAN_LIGHTNING | d(12, 6) 伤害 + flashburn(rnd(100)) |
| WAN_FIRE | d(12, 6) 伤害 + 烧甲 + 销毁物品 + 治愈史莱姆 |
| WAN_COLD | d(12, 6) 伤害 + 销毁物品 |
| WAN_MAGIC_MISSILE | d(4, 6) 伤害 (Antimagic 免疫) |
| WAN_DEATH | 直接死亡 (非生物/恶魔免疫) |
| WAN_SLEEP | 沉睡 rnd(50) 回合 (Sleep_resistance 免疫) |
| WAN_POLYMORPH | 自我变形 (Unchanging 免疫) |
| WAN_CANCELLATION | 取消自身效果 (粘土魔像死亡) |
| WAN_TELEPORTATION | 传送 |
| WAN_SPEED_MONSTER | 加速 rn1(25, 50) = 50..74 回合 |
| WAN_SLOW_MONSTER | 减速 |
| WAN_MAKE_INVISIBLE | 隐身 rn1(15, 31) = 31..45 回合 |
| WAN_PROBING | 检查自身状态和背包 |
| WAN_OPENING | 释放被抓取/解除惩罚/开箱 |
| WAN_LOCKING | 关闭陷阱/锁箱 |
| WAN_UNDEAD_TURNING | unturn_you() |
| WAN_DIGGING | 无效果 |
| WAN_NOTHING | 无效果 |

---

## 18. 掘洞之杖特殊处理 (zap_dig)

掘洞之杖是 RAY 类型但**不走 dobuzz() 路径**，而是调用专用的 `zap_dig()`:

### 18.1 被吞噬时

- 非旋风类吞噬者: 几乎杀死 (独特怪物 HP 减半, 其他 HP=1), 然后喷出

### 18.2 向上/向下

- 向上或在楼梯上: 石头掉落 `rnd(硬头盔 ? 2 : 6)` 伤害
- 向下: 挖洞 (`dighole()`)

### 18.3 水平方向

```
digdepth = rn1(18, 8)  // 8..25
WHILE digdepth > 0:
  移动到下一格
  挖掘墙壁/门等地形
```

---

## 19. 刻字鉴定魔杖 (engrave.c)

使用魔杖在地上刻字是一种经典的鉴定方法。每种魔杖刻字时产生不同的效果:

### 19.1 效果分类

| 魔杖 | 刻字效果 | 鉴定提示 |
|------|---------|---------|
| **WAN_FIRE** | 刻字类型=BURN; "Flames fly from the wand." | 直接识别: "This wand is a wand of fire!" |
| **WAN_LIGHTNING** | 刻字类型=BURN; "Lightning arcs from the wand." + 致盲 | 直接识别: "This wand is a wand of lightning!" |
| **WAN_DIGGING** | 刻字类型=ENGRAVE; "Gravel flies up from the floor." | 直接识别: "This wand is a wand of digging!" |
| **WAN_POLYMORPH** | 已有刻字变为随机文字 | 有刻字时才可见 |
| **WAN_CANCELLATION** | 已有刻字消失: "The engraving vanishes!" | 有刻字时才可见 |
| **WAN_MAKE_INVISIBLE** | 已有刻字消失: "The engraving vanishes!" | 有刻字时才可见 |
| **WAN_TELEPORTATION** | 已有刻字消失: "The engraving vanishes!" | 有刻字时才可见 |
| **WAN_COLD** | "A few ice cubes drop from the wand." | 如果已有 BURN 类型刻字则消除 |
| **WAN_STRIKING** | "The wand unsuccessfully fights your attempt to write!" | 不产生实际刻字 |
| **WAN_SLOW_MONSTER** | "The bugs on the floor slow down!" | 纯文字提示 |
| **WAN_SPEED_MONSTER** | "The bugs on the floor speed up!" | 纯文字提示 |
| **WAN_MAGIC_MISSILE** | "The floor is riddled by bullet holes!" | 纯文字提示 |
| **WAN_SLEEP / WAN_DEATH** | "The bugs on the floor stop moving!" | 两者不可区分 |
| **WAN_NOTHING** | 无效果(DUST 类型刻字) | 区分: 消耗充能但无任何消息 |
| **WAN_UNDEAD_TURNING** | 无效果(DUST 类型刻字) | |
| **WAN_OPENING** | 无效果(DUST 类型刻字) | |
| **WAN_LOCKING** | 无效果(DUST 类型刻字) | |
| **WAN_PROBING** | 无效果(DUST 类型刻字) | |
| **NODIR 魔杖** | 触发 `zapnodir()` 正常效果 | 光明照亮房间, 召唤产生怪物等 |

### 19.2 刻字使用充能

- 与正常 zap 相同: 调用 `zappable()` 消耗充能
- **诅咒反噬**: 与 zap 相同的 1/100 概率调用 `wand_explode()` (注意: 此处是 `wand_explode(obj, 0)` 而非 `backfire()`, `chg=0` 会被调整为 2)
- 榨取(wrest)在刻字时也会出现
- 充能耗尽 (`spe < 0`): 魔杖化为灰尘

### 19.3 鉴定策略

最高效的鉴定方式:
1. 先在地上刻一个 `Elbereth` (用手指, DUST 类型)
2. 用待鉴定魔杖 zap 地面
3. 观察效果:
   - 三种魔杖直接自报门户: 火焰/闪电/掘洞
   - 刻字消失: 抵消/隐身/传送/冰霜(仅对 BURN 类刻字)/变形(文字变化)
   - 明确的文字提示: 打击/减速/加速/魔法飞弹/沉睡+死亡(不可区分)
   - 无任何效果(刻字不变, 无消息): 无效/不死回转/开锁/关门/探索
   - NODIR 魔杖直接触发效果: 立即可识别

---

## 20. 怪物使用魔杖 (muse.c)

### 20.1 进攻性魔杖 (find_offensive)

怪物在满足以下条件时会使用进攻性魔杖:
- 非和平、非动物、非无脑
- 英雄未被吞噬
- 与英雄对齐 (`lined_up()`)

**反射感知**: 如果怪物已见过英雄使用反射 (`m_seenres(M_SEEN_REFL)`), 或怪物与英雄相邻 (`monnear`), 则**跳过所有 RAY 类魔杖**。

进攻性魔杖优先级(后选覆盖先选):

| 魔杖 | 常量 | 额外条件 |
|------|------|---------|
| WAN_DEATH | MUSE_WAN_DEATH | 英雄无已知魔法抗性 |
| WAN_SLEEP | MUSE_WAN_SLEEP | 英雄未处于多重动作中(multi >= 0), 无已知睡眠抗性 |
| WAN_FIRE | MUSE_WAN_FIRE | 英雄无已知火焰抗性 |
| WAN_COLD | MUSE_WAN_COLD | 英雄无已知冰霜抗性 |
| WAN_LIGHTNING | MUSE_WAN_LIGHTNING | 英雄无已知电击抗性 |
| WAN_MAGIC_MISSILE | MUSE_WAN_MAGIC_MISSILE | 英雄无已知魔法抗性 |
| WAN_STRIKING | MUSE_WAN_STRIKING | 英雄无已知魔法抗性; **不受反射跳过** |
| WAN_UNDEAD_TURNING | MUSE_WAN_UNDEAD_TURNING | 特殊条件; **不受反射跳过** |
| WAN_TELEPORTATION | MUSE_WAN_TELEPORTATION | 英雄无 Teleport_control; **不受反射跳过**; 用于打破英雄的有利位置 |

### 20.2 防御性魔杖 (find_defensive)

怪物在 HP 低于阈值时搜索防御手段:
- HP 阈值: `mhp * fraction >= mhpmax`, 其中 `fraction = ulevel < 10 ? 5 : ulevel < 14 ? 4 : 3`

| 魔杖 | 常量 | 用途 |
|------|------|------|
| WAN_TELEPORTATION | MUSE_WAN_TELEPORTATION_SELF | 自我传送逃跑 |
| WAN_UNDEAD_TURNING | MUSE_WAN_UNDEAD_TURNING | 对抗英雄的石化武器(cockatrice corpse) |

### 20.3 杂项魔杖 (find_misc)

怪物非战斗时的自我增强:

| 魔杖 | 常量 | 效果 |
|------|------|------|
| WAN_MAKE_INVISIBLE | MUSE_WAN_MAKE_INVISIBLE | 自我隐身 (如果尚未隐身) |
| WAN_SPEED_MONSTER | MUSE_WAN_SPEED_MONSTER | 自我加速 (如果尚未很快) |
| WAN_POLYMORPH | MUSE_WAN_POLYMORPH | 自我变形 (低 HP 时尝试) |

### 20.4 怪物 zap 机制

```
mzapwand(mtmp, otmp, self):
  IF otmp.spe < 1: impossible() 并返回  // 不应发生
  otmp.spe -= 1
  // 显示 zap 消息(根据可见性)
```

怪物使用 RAY 魔杖时:
```
// 进攻性 RAY 魔杖:
mzapwand(mtmp, otmp, FALSE)
buzz(-(30 + BZ_OFS_WAN(otyp)), 6, mx, my, dx, dy)
// type = -30..-39 范围 -> 怪物魔杖编码
// nd = 6 (与英雄相同，WAN_MAGIC_MISSILE 除外)
```

怪物使用 IMMEDIATE 魔杖时:
```
// 进攻性:
mbhit(mtmp, rn1(8, 6), mbhitm, bhito, otmp)
// 射程同英雄: 6..13 格

// 防御性(自我传送等):
mzapwand(mtmp, otmp, TRUE)
// 直接对自身施加效果
```

**关键区别**: 怪物从不使用 NODIR 魔杖 (不许愿、不召唤怪物)。怪物也从不折断魔杖。

---

## 21. current_wand 安全机制

`gc.current_wand` 在 zap 开始时设为 `obj`，在结束后清除。目的:
- 如果 zap 途中触发连锁反应(例如击中神殿牧师 -> 神罚 -> 闪电 -> destroy_items)导致魔杖被销毁，`current_wand` 被设为 NULL，调用方检查后不会再操作已销毁的魔杖。

---

## 22. 测试向量

### TV-1: 火焰之杖基础伤害

```
输入: 英雄 zap WAN_FIRE 命中无抗性怪物
参数: nd=6, type=ZT_WAND(ZT_FIRE)=1
预期: 伤害 = d(6,6), 范围 6..36
抗性: 火焰抗性怪物伤害 = 0 + shieldeff
冰霜抗性怪物: d(6,6) + 7 额外伤害
```

### TV-2: 死亡之杖即死

```
输入: 英雄 zap WAN_DEATH 命中普通怪物(mhp=50)
预期: tmp = 50 + 1 = 51, 怪物必死
抗性: 非生物(zombie等) -> shieldeff, tmp=0
      恶魔 -> shieldeff, tmp=0
      魔法抗性 -> shieldeff, tmp=0
```

### TV-3: 分解龙息 vs 护甲

```
输入: 分解龙息 (type=ZT_BREATH(ZT_DEATH)=24) 命中持盾怪物
预期:
  无分解抗性 + 有盾牌: 盾牌被摧毁, 怪物存活
  无分解抗性 + 无盾有甲: 身甲+披风被摧毁, 怪物存活
  无分解抗性 + 无甲无盾: tmp=MAGIC_COOKIE(1000), 披风+衬衫销毁, 怪物死亡
  有分解抗性: shieldeff, tmp=0
```

### TV-4: 魔杖充能爆炸概率边界

```
输入: recharged=0 的魔杖充能
预期: 0^3 = 0, 条件 n > 0 为 false -> 绝不爆炸

输入: recharged=7 的魔杖充能
预期: 7^3 = 343, rn2(343) 范围 0..342, 343 > 任何值 -> 必定爆炸

输入: recharged=1 的许愿之杖充能
预期: n > 0 且 otyp == WAN_WISHING -> 必定爆炸 (跳过概率检查)

输入: recharged=0 的许愿之杖充能
预期: n == 0 -> 不爆炸; 充能量 = 1; spe 设为 max(1, 当前spe+1)
```

### TV-5: 榨取最后一次充能边界

```
输入: spe=0 的魔杖 zap
预期: rn2(121) == 0 的概率 = 1/121 ≈ 0.83%
  成功: "You wrest one last charge", spe 变为 -1, 魔杖可用此次
  失败: nothing_happens, spe 不变

输入: spe=-1 的魔杖 zap
预期: spe < 0 -> 直接返回 0, nothing_happens
  zap 后检查 spe < 0: "turn to dust", 魔杖销毁
```

### TV-6: 诅咒魔杖反噬

```
输入: 诅咒的 WAN_FIRE(spe=5) zap
反噬概率: 1/100 = 1%
反噬伤害: d(5+2, 6) = d(7,6), 范围 7..42
无反噬时: 正常 zap, spe 变为 4
```

### TV-7: 折断死亡之杖

```
输入: 折断 WAN_DEATH(spe=4)
步骤:
  zappable() 消耗 1 充能: spe=3, 返回后 spe++ -> spe=4
  dmg = 4 * 4 = 16
  调用 broken_wand_explode(obj, 16 * 4, EXPL_MAGICAL)
  即 explode(ux, uy, -(WAN_DEATH), 64, WAND_CLASS, EXPL_MAGICAL)
  爆炸类型: AD_DISN (因为 abs(type) % 10 == 4)
  对角色(法师): damu = 64 / 5 = 12
  对角色(战士): damu = 64
```

### TV-8: 反射后射线继续 (含 range 消耗)

```
输入: 英雄 zap WAN_FIRE 命中持银龙鳞甲的怪物, 当前 range=9
预期:
  zap_hit 成功 -> mon_reflects 返回 TRUE
  dx = -dx, dy = -dy (方向反转)
  range -= 2 -> range 变为 7 (反射也消耗额外射程)
  射线继续以反转方向传播, 剩余射程 7

源码依据:
  怪物: range -= 2 位于 mon_reflects if/else 块之外 (zap.c line 4937),
        在 zap_hit() 成功的整个分支末尾, 反射和实际命中均执行
  玩家: range -= 2 位于 Reflecting 检查之前 (zap.c line 4949),
        zap_hit() 成功后立即执行
```

### TV-9: 火球爆炸范围和伤害

```
输入: 英雄等级 10 施放 SPE_FIREBALL 命中墙前的位置
参数: type = ZT_SPELL(ZT_FIRE) = 11, nd = 10/2+1 = 6
射线阶段: 火球沿射线方向前进, 遇怪物或墙时停止 (墙: 退一格)
爆炸: explode(sx, sy, 11, d(12, 6), 0, EXPL_FIERY)
  注意: 爆炸伤害永远是 d(12, 6) 而非 d(nd, 6)
  范围: 3x3 (sx-1..sx+1, sy-1..sy+1)
  对英雄若在范围内: 按 explode() 逻辑处理
```

### TV-10: 被吞噬时 zap RAY 魔杖

```
输入: 被紫虫吞噬, zap WAN_COLD
参数: type = ZT_WAND(ZT_COLD) = 2, nd = 6
预期:
  tmp = zhitm(u.ustuck, 2, 6, &otmp)
  如果紫虫无冰霜抗性: d(6,6) 伤害
  如果紫虫有火焰抗性: 额外 d(6,3)
  "The bolt of cold rips into the purple worm!"
  射线不传播, 直接返回
```

### TV-11: 许愿之杖 Luck 边界条件

```
输入: Luck = -4, zap WAN_WISHING
检查: Luck + rn2(5) = -4 + (0..4)
  rn2(5)=0: -4 < 0 -> 失败
  rn2(5)=1: -3 < 0 -> 失败
  rn2(5)=2: -2 < 0 -> 失败
  rn2(5)=3: -1 < 0 -> 失败
  rn2(5)=4:  0 >= 0 -> 成功
  失败概率: 4/5 = 80%

输入: Luck = 0, zap WAN_WISHING
检查: 0 + rn2(5) >= 0 -> 总是成功 (rn2(5) >= 0)
```

### TV-12: 沉睡射线持续时间

```
输入: WAN_SLEEP 命中怪物, nd=6
效果: sleep_monst(mon, d(6, 25), WAND_CLASS)
  沉睡持续: d(6, 25) = 6..150 回合

输入: WAN_SLEEP 自我 zap
效果: fall_asleep(-rnd(50), TRUE)
  沉睡持续: rnd(50) = 1..50 回合 (负号表示不可立即唤醒)
```

### TV-13: 折断 WAN_STRIKING 的伤害计算

```
输入: 折断 WAN_STRIKING(spe=3)
步骤:
  zappable() -> spe=2, 返回后 spe++ -> spe=3
  dmg = spe * 4 = 12 (但随后被覆盖)
  进入 case WAN_STRIKING:
    dmg = d(1 + 3, 6) = d(4, 6), 范围 4..24
  "A wall of force smashes down around you!"
  affects_objects = TRUE

  然后: explode(ox, oy, -(WAN_STRIKING), rnd(12), WAND_CLASS, EXPL_MAGICAL)
  注意: rnd(dmg) 而 dmg 此时为 d(4,6) 的结果, 假设为 15
  则 rnd(15) = 1..15

  [疑似 bug: dmg 在 case WAN_STRIKING 中被 d(1+spe, 6) 覆盖,
   但之后 explode() 用的是 rnd(dmg), 而 dmg 是本轮的 d(4,6) 的结果值,
   不是确定性的。但这不是 bug，只是两层随机。]
```

### TV-14: 刻字鉴定 -- 不可区分的魔杖对

```
输入: 用未知魔杖在有 DUST 刻字的地面刻字
场景1: WAN_SLEEP -> "The bugs on the floor stop moving!"
场景2: WAN_DEATH -> "The bugs on the floor stop moving!"
预期: 两者消息完全相同，无法通过刻字区分
      (需要其他方式: zap 怪物或使用风险)
```

### TV-15: 怪物反射感知影响魔杖选择

```
输入: 怪物持有 WAN_FIRE(spe=3) 和 WAN_STRIKING(spe=3)
场景1: 怪物未见过英雄反射 (m_seenres(M_SEEN_REFL) == 0), 非相邻
  预期: 可能选择 WAN_FIRE(进攻) 或 WAN_STRIKING(进攻)
场景2: 怪物已见过英雄反射 (m_seenres(M_SEEN_REFL) != 0), 非相邻
  预期: 跳过 WAN_FIRE; 仍可选择 WAN_STRIKING (IMMEDIATE, 不受反射限制)
场景3: 怪物与英雄相邻 (monnear == true)
  预期: 跳过 WAN_FIRE (reflection_skip); 仍可选择 WAN_STRIKING
```

### TV-16: 初始充能边界条件

```
输入: 新生成 WAN_WISHING
预期: spe = 1 (固定值, 非随机)

输入: 新生成 WAN_LIGHT (NODIR)
预期: spe = rn1(5, 11) = 11..15, 均匀分布

输入: 新生成 WAN_FIRE (RAY)
预期: spe = rn1(5, 4) = 4..8, 均匀分布

边界: spe 最小值 4 (RAY/IMMEDIATE), 最大值 15 (NODIR)
```

### TV-17: 怪物 zap 死亡之杖 vs 英雄反射

```
输入: 怪物首次 zap WAN_DEATH 命中有反射之盾的英雄
预期:
  zap_hit(u.uac, 0) -> 假设命中
  Reflecting == TRUE
  "But it reflects from your shield!"
  dx = -dx, dy = -dy (方向反转)
  怪物记住 M_SEEN_REFL
  下次: 怪物跳过 WAN_DEATH (reflection_skip == TRUE)

边界: 怪物相邻时 reflection_skip 也为 TRUE, 但此时 IMMEDIATE 魔杖仍可用
```

---

## 附录 A: ZT 类型编码

```
ZT_MAGIC_MISSILE = 0   (AD_MAGM - 1)
ZT_FIRE          = 1   (AD_FIRE - 1)
ZT_COLD          = 2   (AD_COLD - 1)
ZT_SLEEP         = 3   (AD_SLEE - 1)
ZT_DEATH         = 4   (AD_DISN - 1)
ZT_LIGHTNING     = 5   (AD_ELEC - 1)
ZT_POISON_GAS    = 6   (AD_DRST - 1)
ZT_ACID          = 7   (AD_ACID - 1)

ZT_WAND(x)   = x        // 0..9:   玩家魔杖
ZT_SPELL(x)  = 10 + x   // 10..19: 玩家法术
ZT_BREATH(x) = 20 + x   // 20..29: 玩家龙息

怪物编码: 取绝对值后恢复为英雄编码
  怪物魔杖: -39..-30 -> +30 -> -9..0 -> abs -> 0..9
  怪物法术: -19..-10 -> abs -> 10..19
  怪物龙息: -29..-20 -> abs -> 20..29
```

## 附录 B: flash_types 字符串表

```
索引 0..9 (魔杖):
  "magic missile", "bolt of fire", "bolt of cold",
  "sleep ray", "death ray", "bolt of lightning",
  "", "", "", ""

索引 10..19 (法术):
  "magic missile", "fireball", "cone of cold",
  "sleep ray", "finger of death", "bolt of lightning",
  "", "", "", ""

索引 20..29 (龙息):
  "blast of missiles", "blast of fire", "blast of frost",
  "blast of sleep gas", "blast of disintegration",
  "blast of lightning", "blast of poison gas",
  "blast of acid", "", ""
```

## 附录 C: 已知不确定点

1. **英雄爆炸无冰火互克**: `explode()` 中怪物有 `resists_cold + AD_FIRE -> *2` 和 `resists_fire + AD_COLD -> *2`，但英雄没有。源代码注释 `/* hero does not get same fire-resistant vs cold and cold-resistant vs fire double damage as monsters [why not?] */`。[可能是有意设计，也可能是遗留 bug，如实记录。]

2. **折断魔杖的 explode() 在逐格效果之前**: 源代码注释 `[TODO? This really ought to prevent the explosion from being fatal so that we never leave a bones file where none of the surrounding targets got affected yet.]` -- 如果爆炸致死英雄，周围目标还未受到逐格效果处理。[疑似 bug，如实记录原始行为]

3. **掘洞之杖折断的挖坑概率**: `rn2(obj.spe) < 3 || (!Can_dig_down && !candig)` 决定坑(PIT) vs 洞(HOLE)。当 spe 很高时几乎总是 HOLE，spe 低时倾向 PIT。

4. ~~**反射与 range 消耗**~~: 原记录有误。实际上 `range -= 2` 在 `zap_hit()` 成功时无条件执行——无论目标是否反射。对怪物，`range -= 2` 位于 `mon_reflects` if/else 块之后 (zap.c line 4937)；对玩家，`range -= 2` 在 `Reflecting` 检查之前 (zap.c line 4949)。反射并不"保存"射程。
