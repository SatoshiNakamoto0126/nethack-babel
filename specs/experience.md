# NetHack 3.7 经验系统机制规格

> 源码版本: NetHack-3.7 分支, exper.c rev 1.62, attrib.c rev 1.134, role.c rev 1.107, botl.c, makemon.c
> 提取日期: 2026-03-14

---

## 1. 经验等级阈值表

函数 `newuexp(lev)` 返回**累计**经验值阈值: 当 `u.uexp >= newuexp(u.ulevel)` 时升级。

### 1.1 公式

```
newuexp(lev):
    if lev < 1:  return 0
    if lev < 10: return 10 * 2^lev
    if lev < 20: return 10000 * 2^(lev - 10)
    else:        return 10000000 * (lev - 19)
```

### 1.2 完整表 (全 30 级)

| 等级 | 升到下一级所需 XP (`newuexp(lev)`) | 到达本级最低 XP (`newuexp(lev-1)`) |
|------|-----------------------------------|------------------------------------|
| 1    | 20                                | 0                                  |
| 2    | 40                                | 20                                 |
| 3    | 80                                | 40                                 |
| 4    | 160                               | 80                                 |
| 5    | 320                               | 160                                |
| 6    | 640                               | 320                                |
| 7    | 1,280                             | 640                                |
| 8    | 2,560                             | 1,280                              |
| 9    | 5,120                             | 2,560                              |
| 10   | 10,000                            | 5,120                              |
| 11   | 20,000                            | 10,000                             |
| 12   | 40,000                            | 20,000                             |
| 13   | 80,000                            | 40,000                             |
| 14   | 160,000                           | 80,000                             |
| 15   | 320,000                           | 160,000                            |
| 16   | 640,000                           | 320,000                            |
| 17   | 1,280,000                         | 640,000                            |
| 18   | 2,560,000                         | 1,280,000                          |
| 19   | 5,120,000                         | 2,560,000                          |
| 20   | 10,000,000                        | 5,120,000                          |
| 21   | 20,000,000                        | 10,000,000                         |
| 22   | 30,000,000                        | 20,000,000                         |
| 23   | 40,000,000                        | 30,000,000                         |
| 24   | 50,000,000                        | 40,000,000                         |
| 25   | 60,000,000                        | 50,000,000                         |
| 26   | 70,000,000                        | 60,000,000                         |
| 27   | 80,000,000                        | 70,000,000                         |
| 28   | 90,000,000                        | 80,000,000                         |
| 29   | 100,000,000                       | 90,000,000                         |
| 30   | 110,000,000                       | 100,000,000                        |

注: `newuexp(0)` 返回 0, 用于 `newuexp(u.ulevel - 1)` 当 u.ulevel == 1 的特殊情况。30 级是 MAXULEV 硬上限, `newuexp(30)` = 110,000,000 仅用于 XP 上限比较。

---

## 2. 怪物经验值计算

函数: `experience(mtmp, nk)` (src/exper.c:84-166)

### 2.1 基础值

```
base = 1 + m_lev^2
```

`m_lev` 是怪物的**当前等级** (由 `adj_lev()` 在创建时计算, 可能不同于 `mons[].mlevel`)。

### 2.2 AC 加成

```
mac = find_mac(mtmp)    // 怪物有效 AC (考虑装备)
if mac < 3:
    if mac < 0:
        bonus = (7 - mac) * 2
    else:     // mac in [0, 2]
        bonus = (7 - mac) * 1
```

示例: AC 2 给 +5, AC 0 给 +7, AC -5 给 +24。

### 2.3 速度加成

```
NORMAL_SPEED = 12

if mmove > NORMAL_SPEED:
    if mmove > 18:    // > 3/2 * NORMAL_SPEED
        bonus += 5
    else:
        bonus += 3
```

### 2.4 特殊攻击方式加成

对每个攻击槽 (NATTK = 6):

```
for each attack slot:
    aatyp = attack type
    if aatyp > AT_BUTT (4):
        if aatyp == AT_WEAP (254):  bonus += 5
        elif aatyp == AT_MAGC (255): bonus += 10
        else:                        bonus += 3
```

值 > AT_BUTT 的攻击方式:
- AT_TUCH (5), AT_STNG (6), AT_HUGS (7): +3
- AT_SPIT (10), AT_ENGL (11), AT_BREA (12), AT_EXPL (13), AT_BOOM (14), AT_GAZE (15), AT_TENT (16): +3
- AT_WEAP (254): +5
- AT_MAGC (255): +10

### 2.5 特殊伤害类型加成

对每个攻击槽:

```
adtyp = damage type

if adtyp > AD_PHYS (0) AND adtyp < AD_BLND (11):
    // AD_MAGM(1)..AD_SPC2(10): 魔法飞弹/火/冰/睡眠/解离/电/毒str/酸/spare1/spare2
    bonus += 2 * m_lev
elif adtyp == AD_DRLI (15) OR adtyp == AD_STON (18) OR adtyp == AD_SLIM (40):
    // 吸取生命等级 / 石化 / 变绿色黏怪
    bonus += 50
elif adtyp != AD_PHYS (0):
    // 其他非物理伤害类型 (AD_BLND 及以上, 排除 DRLI/STON/SLIM)
    bonus += m_lev

// 超重伤害加成 (适用于所有伤害类型, 含 AD_PHYS)
if (damd * damn) > 23:
    bonus += m_lev

// 鳗鱼溺水特殊加成
if adtyp == AD_WRAP AND monster_class == S_EEL AND 玩家非水生(NOT Amphibious):
    bonus += 1000
```

### 2.6 Extra Nasty 加成

```
if extra_nasty(ptr):   // M2_NASTY 标志位
    bonus += 7 * m_lev
```

### 2.7 高等级加成

```
if m_lev > 8:
    bonus += 50
```

### 2.8 邮件守护进程覆盖

```
if monster is PM_MAIL_DAEMON:
    total = 1  // 覆盖所有其他计算
```

仅在 `MAIL_STRUCTURES` 定义时生效。

### 2.9 复活/克隆怪物递减

仅当怪物具有 `mrevived` 或 `mcloned` 标志时生效。`nk` 为该怪物类型的总击杀数 (含本次), 存于 `mvitals[mndx].died`。

```
// 递减区间:
//   击杀  1..20:   完整 XP
//   击杀 21..40:   XP / 2
//   击杀 41..80:   XP / 4
//   击杀 81..120:  XP / 8
//   击杀 121..180: XP / 16
//   击杀 181..240: XP / 32
//   击杀 241..255+: XP / 64

tmp2 = 20
i = 0
while nk > tmp2 AND tmp > 1:
    tmp = (tmp + 1) / 2      // 整数除法, 向上取整的折半
    nk -= tmp2
    if i is odd:
        tmp2 += 20
    i += 1
```

区间大小循环: 20, 20, 40, 40, 60, 60... (`tmp2` 仅在 `i` 为奇数时增加 20)。

---

## 3. 怪物难度计算

### 3.1 怪物等级调整: `adj_lev()` (src/makemon.c:2007-2039)

怪物在创建时, `m_lev` 由 `adj_lev(ptr)` 计算 (非直接使用 `ptr->mlevel`):

```
adj_lev(ptr):
    // 特殊: 巫师
    if ptr == Wizard_of_Yendor:
        tmp = ptr->mlevel + 已击杀次数
        return min(tmp, 49)

    // 特殊: "超级"恶魔 (mlevel > 49)
    if ptr->mlevel > 49:
        return 50

    tmp = ptr->mlevel

    // 根据地下城难度调整
    tmp2 = level_difficulty() - tmp
    if tmp2 < 0:
        tmp -= 1        // 怪物等级高于地下城难度: -1
    else:
        tmp += tmp2 / 5 // 否则: 每 5 点差距 +1

    // 根据玩家等级调整
    tmp2 = u.ulevel - ptr->mlevel
    if tmp2 > 0:
        tmp += tmp2 / 4  // 每 4 级差距 +1

    // 上限: 基础等级的 1.5 倍
    upper = min(3 * ptr->mlevel / 2, 49)

    return clamp(tmp, 0, upper)
```

### 3.2 地下城难度: `level_difficulty()` (src/dungeon.c:2021)

```
level_difficulty():
    if 在终盘:
        return depth(sanctum_level) + u.ulevel / 2
    elif 携带护符:
        return deepest_lev_reached(FALSE)
    else:
        res = depth(u.uz)
        if builds_up(u.uz):   // 某些向上建造的分支
            res += 2 * (dungeons[u.uz.dnum].entry_lev - u.uz.dlevel + 1)
        return res
```

### 3.3 怪物 HP 计算: `newmonhp()` (src/makemon.c:1014)

```
mon.m_lev = adj_lev(ptr)

if is_golem(ptr):
    hp = golemhp(mndx)              // 固定值
elif is_rider(ptr):
    hp = d(10, 8)                    // 10d8
elif ptr->mlevel > 49:
    hp = 2 * (ptr->mlevel - 6)      // 编码的固定 HP
    m_lev = hp / 4                   // 重新近似等级
elif S_DRAGON 且为成年龙:
    hp = 终盘 ? 8 * m_lev : 4 * m_lev + d(m_lev, 4)
elif m_lev == 0:
    hp = rnd(4)
else:
    hp = d(m_lev, 8)                 // 标准: m_lev 个 d8
    if is_home_elemental(ptr):
        hp *= 3
```

---

## 4. 升级流程 (`pluslvl`)

由 `newexplevel()` 在 `u.uexp >= newuexp(u.ulevel)` 且 `u.ulevel < MAXULEV (30)` 时触发。

### 4.1 HP 增加

函数 `newhp()` (src/attrib.c:1079-1142)。

#### 初始化 (等级 0)

```
hp = role.hpadv.infix + race.hpadv.infix
if role.hpadv.inrnd > 0: hp += rnd(role.hpadv.inrnd)
if race.hpadv.inrnd > 0: hp += rnd(race.hpadv.inrnd)
// 初始化时无体质调整
```

#### 后续升级

```
if ulevel < role.xlev:
    hp = role.hpadv.lofix + race.hpadv.lofix
    if role.hpadv.lornd > 0: hp += rnd(role.hpadv.lornd)
    if race.hpadv.lornd > 0: hp += rnd(race.hpadv.lornd)
else:
    hp = role.hpadv.hifix + race.hpadv.hifix
    if role.hpadv.hirnd > 0: hp += rnd(role.hpadv.hirnd)
    if race.hpadv.hirnd > 0: hp += rnd(race.hpadv.hirnd)

// 体质加成
con = ACURR(A_CON)
if con <= 3:   conplus = -2
elif con <= 6:  conplus = -1
elif con <= 14: conplus = 0
elif con <= 16: conplus = +1
elif con == 17: conplus = +2
elif con == 18: conplus = +3
else:           conplus = +4   // con >= 19

hp += conplus
if hp <= 0: hp = 1
```

#### 30 级后限流

```
if ulevel >= MAXULEV:
    lim = 5 - uhpmax / 300
    lim = max(lim, 1)
    if hp > lim: hp = lim
```

uhpmax 达到 1200 后, HP 增长恒为 1。

#### 增量记录

对 `ulevel < MAXULEV`: 存入 `u.uhpinc[u.ulevel]`, 用于未来等级吸取时回退。

### 4.2 HP/PW 各职业种族数值表

格式: `{ infix, inrnd, lofix, lornd, hifix, hirnd }`
- `infix + rnd(inrnd)`: 初始化
- `lofix + rnd(lornd)`: 低于 xlev 时每级增量
- `hifix + rnd(hirnd)`: 达到或高于 xlev 时每级增量
- 注意: `inrnd/lornd/hirnd` 为 0 时跳过随机部分

#### 职业 HP 进阶

| 职业         | 初始fix | 初始rnd | 低fix | 低rnd | 高fix | 高rnd | xlev |
|-------------|---------|---------|-------|-------|-------|-------|------|
| Archeologist | 11      | 0       | 0     | 8     | 1     | 0     | 14   |
| Barbarian    | 14      | 0       | 0     | 10    | 2     | 0     | 10   |
| Caveman      | 14      | 0       | 0     | 8     | 2     | 0     | 10   |
| Healer       | 11      | 0       | 0     | 8     | 1     | 0     | 20   |
| Knight       | 14      | 0       | 0     | 8     | 2     | 0     | 10   |
| Monk         | 12      | 0       | 0     | 8     | 1     | 0     | 10   |
| Priest       | 12      | 0       | 0     | 8     | 1     | 0     | 10   |
| Rogue        | 10      | 0       | 0     | 8     | 1     | 0     | 11   |
| Ranger       | 13      | 0       | 0     | 6     | 1     | 0     | 12   |
| Samurai      | 13      | 0       | 0     | 8     | 1     | 0     | 11   |
| Tourist      | 8       | 0       | 0     | 8     | 0     | 0     | 14   |
| Valkyrie     | 14      | 0       | 0     | 8     | 2     | 0     | 10   |
| Wizard       | 10      | 0       | 0     | 8     | 1     | 0     | 12   |

#### 职业 PW (能量) 进阶

| 职业         | 初始fix | 初始rnd | 低fix | 低rnd | 高fix | 高rnd |
|-------------|---------|---------|-------|-------|-------|-------|
| Archeologist | 1       | 0       | 0     | 1     | 0     | 1     |
| Barbarian    | 1       | 0       | 0     | 1     | 0     | 1     |
| Caveman      | 1       | 0       | 0     | 1     | 0     | 1     |
| Healer       | 1       | 4       | 0     | 1     | 0     | 2     |
| Knight       | 1       | 4       | 0     | 1     | 0     | 2     |
| Monk         | 2       | 2       | 0     | 2     | 0     | 2     |
| Priest       | 4       | 3       | 0     | 2     | 0     | 2     |
| Rogue        | 1       | 0       | 0     | 1     | 0     | 1     |
| Ranger       | 1       | 0       | 0     | 1     | 0     | 1     |
| Samurai      | 1       | 0       | 0     | 1     | 0     | 1     |
| Tourist      | 1       | 0       | 0     | 1     | 0     | 1     |
| Valkyrie     | 1       | 0       | 0     | 1     | 0     | 1     |
| Wizard       | 4       | 3       | 0     | 2     | 0     | 3     |

#### 种族 HP 进阶

| 种族   | 初始fix | 初始rnd | 低fix | 低rnd | 高fix | 高rnd |
|--------|---------|---------|-------|-------|-------|-------|
| Human  | 2       | 0       | 0     | 2     | 1     | 0     |
| Elf    | 1       | 0       | 0     | 1     | 1     | 0     |
| Dwarf  | 4       | 0       | 0     | 3     | 2     | 0     |
| Gnome  | 1       | 0       | 0     | 1     | 0     | 0     |
| Orc    | 1       | 0       | 0     | 1     | 0     | 0     |

#### 种族 PW (能量) 进阶

| 种族   | 初始fix | 初始rnd | 低fix | 低rnd | 高fix | 高rnd |
|--------|---------|---------|-------|-------|-------|-------|
| Human  | 1       | 0       | 2     | 0     | 2     | 0     |
| Elf    | 2       | 0       | 3     | 0     | 3     | 0     |
| Dwarf  | 0       | 0       | 0     | 0     | 0     | 0     |
| Gnome  | 2       | 0       | 2     | 0     | 2     | 0     |
| Orc    | 1       | 0       | 1     | 0     | 1     | 0     |

### 4.3 PW (能量/法力) 增加

函数 `newpw()` (src/exper.c:43-81)。

#### 初始化 (等级 0)

```
en = role.enadv.infix + race.enadv.infix
if role.enadv.inrnd > 0: en += rnd(role.enadv.inrnd)
if race.enadv.inrnd > 0: en += rnd(race.enadv.inrnd)
```

#### 后续升级

```
enrnd = ACURR(A_WIS) / 2   // 整数除法
if ulevel < role.xlev:
    enrnd += role.enadv.lornd + race.enadv.lornd
    enfix = role.enadv.lofix + race.enadv.lofix
else:
    enrnd += role.enadv.hirnd + race.enadv.hirnd
    enfix = role.enadv.hifix + race.enadv.hifix

en = enermod(rn1(enrnd, enfix))
// rn1(x, y) = rn2(x) + y = random [y, y+x-1]
```

#### enermod (职业能量乘数)

```
enermod(en):
    Cleric, Wizard:               return 2 * en
    Healer, Knight:               return (3 * en) / 2   // 整数除法
    Barbarian, Valkyrie:          return (3 * en) / 4   // 整数除法
    其他 (Arc, Cav, Mon, Pri, Rog, Ran, Sam, Tou):
                                  return en
```

[疑似 bug] `enermod` 中 Priest (PM_CLERIC) 获得 2x 乘数, 但 Priest 的职业代码为 PM_CLERIC, switch 分支确认匹配。这不是 bug, 但容易混淆因为 `case PM_CLERIC` 实际对应 Priest 职业。

#### 30 级后限流

```
if ulevel >= MAXULEV:
    lim = 4 - uenmax / 200
    lim = max(lim, 1)
    if en > lim: en = lim
```

uenmax 达到 600 后, PW 增长恒为 1。

#### 下限

```
if en <= 0: en = 1
```

### 4.4 等级递增与 XP 处理

```
if ulevel < MAXULEV:
    if incremental (来自 XP 积累):
        // 限制 XP 不超过下一级阈值
        tmp = newuexp(ulevel + 1)
        if uexp >= tmp: uexp = tmp - 1
    else:
        // 来自药水/幽灵/愿望: 设置 XP 为当前等级阈值
        uexp = newuexp(ulevel)

    ulevel += 1
    显示 "Welcome [back] to experience level {ulevel}."
    if ulevelmax < ulevel: ulevelmax = ulevel
    adjabil(ulevel - 1, ulevel)  // 获得固有能力
    // 检查是否达到新称号, 记录成就
```

关键: 每次 `newexplevel()` 调用最多升**一级**。升级后 XP 被截断到下一级阈值以下 (增量模式)。调用者需重新检查。

### 4.5 升级获得的固有能力

函数 `adjabil()` (src/attrib.c:1005-1074)。达到指定等级时获得, 降到该等级以下时失去。

#### 职业固有能力

| 职业         | 等级 | 能力             |
|-------------|------|-----------------|
| Archeologist | 1    | Searching       |
|              | 5    | Stealth         |
|              | 10   | Fast            |
| Barbarian    | 1    | Poison resistance |
|              | 7    | Fast            |
|              | 15   | Stealth         |
| Caveman      | 7    | Fast            |
|              | 15   | Warning         |
| Healer       | 1    | Poison resistance |
|              | 15   | Warning         |
| Knight       | 7    | Fast            |
| Monk         | 1    | Fast, Sleep res., See invisible |
|              | 3    | Poison resistance |
|              | 5    | Stealth         |
|              | 7    | Warning         |
|              | 9    | Searching       |
|              | 11   | Fire resistance |
|              | 13   | Cold resistance |
|              | 15   | Shock resistance |
|              | 17   | Teleport control |
| Priest       | 15   | Warning         |
|              | 20   | Fire resistance |
| Ranger       | 1    | Searching       |
|              | 7    | Stealth         |
|              | 15   | See invisible   |
| Rogue        | 1    | Stealth         |
|              | 10   | Searching       |
| Samurai      | 1    | Fast            |
|              | 15   | Stealth         |
| Tourist      | 10   | Searching       |
|              | 20   | Poison resistance |
| Valkyrie     | 1    | Cold resistance |
|              | 3    | Stealth         |
|              | 7    | Fast            |
| Wizard       | 15   | Warning         |
|              | 17   | Teleport control |

#### 种族固有能力

| 种族  | 等级 | 能力             |
|-------|------|-----------------|
| Elf   | 1    | Infravision     |
|       | 4    | Sleep resistance |
| Dwarf | 1    | Infravision     |
| Gnome | 1    | Infravision     |
| Orc   | 1    | Infravision     |
|       | 1    | Poison resistance |
| Human | (无) |                 |

注意: 1 级能力使用 `FROMEXPER | FROMOUTSIDE` 掩码, **不会**因等级吸取而丢失。

### 4.6 武器技能槽

```
adjabil() 同时调用:
    升级时: add_weapon_skill(newlevel - oldlevel)   // 每级 +1 槽
    降级时: lose_weapon_skill(oldlevel - newlevel)  // 每级 -1 槽
```

---

## 5. 降级流程 (`losexp`)

src/exper.c:206-291

### 5.1 生命吸取抗性检查

```
if caller != "#levelchange" (巫师模式命令):
    if resists_drli(youmonst): return  // 无效果
```

### 5.2 等级 > 1

```
显示 "Goodbye level {ulevel}."
ulevel -= 1
adjabil(ulevel + 1, ulevel)  // 移除固有能力
```

### 5.3 等级 == 1 (致命或非致命)

```
if drainer != NULL:
    // 致命! 因生命吸取死亡
    死亡原因 = drainer
    // 如果被生命保留, 继续以下流程

if ulevel 仍 == 1 (非生命保留后升级的情况):
    uexp = 0
```

`drainer` 为 NULL (如神怒) 时, 等级吸取**永不致命**; 仅将 1 级角色的 XP 重置为 0。

### 5.4 HP 减少

```
uhpmin = minuhpmax(10)        // max(ulevel, 10)
num = uhpinc[ulevel]          // 取回该等级存储的 HP 增量
uhpmax -= num
if uhpmax < uhpmin: uhpmax = uhpmin      // 下限
if uhpmax > old_uhpmax: uhpmax = old_uhpmax  // 防止增加

uhp -= num
if uhp < 1: uhp = 1
if uhp > uhpmax: uhp = uhpmax
```

### 5.5 PW 减少

```
num = ueninc[ulevel]          // 取回该等级存储的 PW 增量
uenmax -= num
if uenmax < 0: uenmax = 0
uen -= num
if uen < 0: uen = 0
if uen > uenmax: uen = uenmax
```

### 5.6 经验值调整

```
if uexp > 0:
    uexp = newuexp(ulevel) - 1
```

将经验值设为当前 (已降低的) 等级的阈值减 1, 即处于新等级范围的顶端。

### 5.7 变身形态

若变身中 (`Upolyd`):
```
num = monhp_per_lvl(youmonst)   // 通常 1d8
mhmax -= num
mh -= num
if mh <= 0: rehumanize()
```

### 5.8 经验丢失情景

| 情景 | 来源 | drainer 参数 | 致命? | 绕过抗性? |
|------|------|-------------|------|----------|
| 生命吸取攻击 (AD_DRLI) | mhitu.c, uhitm.c, zap.c, artifact.c | 攻击者名称 | 是 (等级1) | 否 |
| 魅魔/夜魔过度消耗 | mhitu.c | "exhaustion" | 是 (等级1) | 否 |
| 神怒 | pray.c | NULL | 否 | 是 |
| 坐在王座上 | sit.c | "a bad experience..." | 是 (等级1) | 否 |
| 火陷阱 (体质过低) | trap.c, attrib.c | NULL | 否 | 是 |
| 巫师模式 | wizcmds.c | "#levelchange" | 否 | 是 (特殊) |

---

## 6. 等级恢复

### 6.1 恢复能力药水 (`POT_RESTORE_ABILITY`)

src/potion.c:643-693

```
if 诅咒: "Ulch! This makes you feel mediocre!" (无效果)
if 未诅咒或祝福:
    // 1. 恢复所有降低到最大值以下的属性
    for 每个属性 (从随机起点遍历 A_MAX 个):
        if ABASE(i) < AMAX(i):
            ABASE(i) = AMAX(i)
            // 重置属性滥用值 (但不重置锻炼值)

    // 2. 恢复丢失的等级
    if 是药水 (非法术) AND ulevel < ulevelmax:
        do:
            pluslvl(FALSE)   // 获得等级, 非增量模式
        while ulevel < ulevelmax AND 药水是祝福的
```

关键区别:
- **未祝福药水**: 恢复**一个**丢失的等级
- **祝福药水**: 恢复**所有**丢失的等级 (循环直到 `ulevel == ulevelmax`)
- **法术** (`SPE_RESTORE_ABILITY`): **不恢复**等级, 仅恢复属性
- `ulevelmax` 记录历史最高等级 (但坐王座失败会减少 `ulevelmax`)

### 6.2 恢复能力法术

与药水使用同一函数 `peffect_restore_ability()`, 但法术使用临时法术书对象, 其 `otyp == SPE_RESTORE_ABILITY` (不是 `POT_RESTORE_ABILITY`), 因此不进入等级恢复分支。

法术的祝福状态取决于治疗技能等级: 技能足够高时法术书临时对象被标记为祝福。

### 6.3 增益等级药水 (`POT_GAIN_LEVEL`)

src/potion.c:1082-1110

```
if 诅咒:
    // 物理意义上的 "up a level": 传送到上一层
elif 未诅咒或祝福:
    pluslvl(FALSE)
    uexp = rndexp(TRUE)  // 设置随机 XP
```

### 6.4 食用幽灵尸体

src/eat.c:1141-1143

```
case PM_WRAITH:
    pluslvl(FALSE)
```

`pluslvl(FALSE)` 模式 (非增量):
- 显示 "You feel more experienced."
- XP 设为 `newuexp(ulevel)` (当前等级阈值的底部)
- 升一级

---

## 7. 属性锻炼/滥用系统

src/attrib.c:486-677

### 7.1 概述

属性通过"锻炼"(exercise) 和"滥用"(abuse) 机制随时间变化。只有 4 个属性参与: **Str, Wis, Dex, Con**。Int 和 Cha **不能**通过锻炼改变。

变身中 (`Upolyd`) 只有 Wis 能被锻炼; 其他物理属性的锻炼被忽略。

### 7.2 锻炼值积累

函数 `exercise(i, inc_or_dec)` (src/attrib.c:488-518):

```
exercise(i, inc_or_dec):
    if i == A_INT or i == A_CHA: return   // 不能锻炼
    if Upolyd and i != A_WIS: return      // 变身中不能锻炼物理属性

    if abs(AEXE(i)) < AVAL (50):
        if inc_or_dec (锻炼):
            // 递减收益法则 Part I:
            // 高属性值时更难增加: 属性3时79%成功, 属性18时0%成功
            AEXE(i) += (rn2(19) > ACURR(i)) ? 1 : 0
        else (滥用):
            // 50% 概率减少
            AEXE(i) -= rn2(2)    // 0 或 1
```

AEXE(i) 的范围被限制在 [-AVAL, +AVAL] 即 [-50, +50] 之间。

### 7.3 周期性触发

函数 `exerper()` (src/attrib.c:520-583), 由 `exerchk()` 在每个游戏回合调用:

#### 每 10 回合: 饥饿检查

```
if moves % 10 == 0:
    switch 饥饿状态:
        SATIATED (>1000):
            exercise(A_DEX, FALSE)       // 滥用敏捷
            if Monk: exercise(A_WIS, FALSE)  // 僧侣: 滥用智慧
        NOT_HUNGRY (>150):
            exercise(A_CON, TRUE)        // 锻炼体质
        WEAK (>50):
            exercise(A_STR, FALSE)       // 滥用力量
            if Monk: exercise(A_WIS, TRUE)   // 僧侣: 锻炼智慧 (禁食)
        FAINTING/FAINTED:
            exercise(A_CON, FALSE)       // 滥用体质
```

#### 每 10 回合: 负重检查

```
    switch near_capacity():
        MOD_ENCUMBER:
            exercise(A_STR, TRUE)        // 锻炼力量
        HVY_ENCUMBER:
            exercise(A_STR, TRUE)
            exercise(A_DEX, FALSE)       // 滥用敏捷
        EXT_ENCUMBER:
            exercise(A_DEX, FALSE)
            exercise(A_CON, FALSE)       // 滥用体质
```

#### 每 5 回合: 状态检查

```
if moves % 5 == 0:
    if 千里眼 (Clairvoyant, intrinsic): exercise(A_WIS, TRUE)
    if 再生 (Regeneration): exercise(A_STR, TRUE)
    if 生病或呕吐: exercise(A_CON, FALSE)
    if 混乱或幻觉: exercise(A_WIS, FALSE)
    if 腿伤(无坐骑)或摸索或眩晕: exercise(A_DEX, FALSE)
```

### 7.4 属性变化检定

函数 `exerchk()` (src/attrib.c:597-677):

```
exerchk():
    exerper()   // 先积累锻炼值

    if moves >= context.next_attrib_check AND not multi (未被延迟):
        for 每个属性 i:
            ax = AEXE(i)
            if ax == 0: continue   // 无积累

            mod_val = sgn(ax)   // +1 或 -1

            // 检查是否已达上下限
            lolim = ATTRMIN(i)           // 通常 3
            hilim = min(ATTRMAX(i), 18)  // 锻炼上限为 18, 即使种族最大值更高
            if (ax < 0 ? ABASE(i) <= lolim : ABASE(i) >= hilim):
                goto nextattrib          // 已在极限

            // 变身中不能改变非智慧属性
            if Upolyd and i != A_WIS:
                goto nextattrib

            // 递减收益法则 Part III:
            // 检定是否真的改变属性
            if i != A_WIS:
                threshold = abs(ax) * 2 / 3
            else:
                threshold = abs(ax)      // 智慧更容易改变
            if rn2(AVAL=50) > threshold:
                goto nextattrib          // 未通过检定

            // 改变属性
            if adjattrib(i, mod_val, -1):
                AEXE(i) = 0              // 成功则归零积累
                显示解释消息

        nextattrib:
            // 递减收益法则 Part II: 积累值衰减
            AEXE(i) = (abs(ax) / 2) * mod_val

        // 设置下次检查时间
        context.next_attrib_check += rn1(200, 800)
        // 即 rn2(200) + 800 = 随机 [800, 999] 回合后
```

### 7.5 关键设计要点

1. **锻炼上限 18**: 即使种族允许属性超过 18 (如矮人 STR 可达 18/100), 锻炼也只能将 ABASE 提升到 18。超过 18 需要其他途径 (药水、愿望等)。

2. **智慧优势**: 智慧检定使用完整的 `abs(ax)` 作为阈值, 而其他属性使用 `abs(ax) * 2 / 3`, 使智慧更容易通过锻炼获得。

3. **衰减**: 无论是否通过检定, 积累值都减半 (向下取整)。这意味着长时间不活动时, 积累的锻炼/滥用效果会逐渐消失。

4. **检查间隔**: 800-999 回合之间随机, 约每 900 回合检查一次属性变化。

5. **不可锻炼属性**: Intelligence (A_INT) 和 Charisma (A_CHA) 完全不受锻炼系统影响。

### 7.6 常见锻炼触发 (非 `exerper`)

除了周期性检查, 以下行为也调用 `exercise()`:

| 行为 | 属性 | 锻炼(+)/滥用(-) |
|------|------|-----------------|
| 成功搜索 | Wis | + |
| 成功铸法 | Wis | + |
| 铸法失败 | Wis | - |
| 成功使用门 | Dex | + |
| 跳跃 | Str, Dex | + |
| 挖掘 | Str | + |
| 成功踢门/箱 | Dex, Str | + |
| 踢墙 (受伤) | Str | - |
| 祈祷 | Wis | + |
| 阅读/鉴定 | Wis | + |
| 使用千里眼 | Wis | + |
| 中毒 | Con | - |
| 呕吐 | Con | - |

---

## 8. 称号系统

### 8.1 等级到称号等级映射

函数: `xlev_to_rank(xlev)` (src/botl.c:301-314)

```
xlev_to_rank(xlev):
    if xlev <= 2:  return 0
    if xlev <= 30: return (xlev + 2) / 4   // 整数除法
    else:          return 8
```

| 称号等级 | 等级范围 | 反向映射 (rank_to_xlev) |
|---------|---------|------------------------|
| 0       | 1--2    | 1                      |
| 1       | 3--5    | 3                      |
| 2       | 6--9    | 6                      |
| 3       | 10--13  | 10                     |
| 4       | 14--17  | 14                     |
| 5       | 18--21  | 18                     |
| 6       | 22--25  | 22                     |
| 7       | 26--29  | 26                     |
| 8       | 30      | 30                     |

### 8.2 全 13 职业称号

每个职业有 9 个称号 (0-8)。部分有性别变体 (F 列)。

#### Archeologist
| 等级 | M | F |
|------|---|---|
| 0 | Digger | -- |
| 1 | Field Worker | -- |
| 2 | Investigator | -- |
| 3 | Exhumer | -- |
| 4 | Excavator | -- |
| 5 | Spelunker | -- |
| 6 | Speleologist | -- |
| 7 | Collector | -- |
| 8 | Curator | -- |

#### Barbarian
| 等级 | M | F |
|------|---|---|
| 0 | Plunderer | Plunderess |
| 1 | Pillager | -- |
| 2 | Bandit | -- |
| 3 | Brigand | -- |
| 4 | Raider | -- |
| 5 | Reaver | -- |
| 6 | Slayer | -- |
| 7 | Chieftain | Chieftainess |
| 8 | Conqueror | Conqueress |

#### Caveman / Cavewoman
| 等级 | M | F |
|------|---|---|
| 0 | Troglodyte | -- |
| 1 | Aborigine | -- |
| 2 | Wanderer | -- |
| 3 | Vagrant | -- |
| 4 | Wayfarer | -- |
| 5 | Roamer | -- |
| 6 | Nomad | -- |
| 7 | Rover | -- |
| 8 | Pioneer | -- |

#### Healer
| 等级 | M | F |
|------|---|---|
| 0 | Rhizotomist | -- |
| 1 | Empiric | -- |
| 2 | Embalmer | -- |
| 3 | Dresser | -- |
| 4 | Medicus ossium | Medica ossium |
| 5 | Herbalist | -- |
| 6 | Magister | Magistra |
| 7 | Physician | -- |
| 8 | Chirurgeon | -- |

#### Knight
| 等级 | M | F |
|------|---|---|
| 0 | Gallant | -- |
| 1 | Esquire | -- |
| 2 | Bachelor | -- |
| 3 | Sergeant | -- |
| 4 | Knight | -- |
| 5 | Banneret | -- |
| 6 | Chevalier | Chevaliere |
| 7 | Seignieur | Dame |
| 8 | Paladin | -- |

#### Monk
| 等级 | M | F |
|------|---|---|
| 0 | Candidate | -- |
| 1 | Novice | -- |
| 2 | Initiate | -- |
| 3 | Student of Stones | -- |
| 4 | Student of Waters | -- |
| 5 | Student of Metals | -- |
| 6 | Student of Winds | -- |
| 7 | Student of Fire | -- |
| 8 | Master | -- |

#### Priest / Priestess
| 等级 | M | F |
|------|---|---|
| 0 | Aspirant | -- |
| 1 | Acolyte | -- |
| 2 | Adept | -- |
| 3 | Priest | Priestess |
| 4 | Curate | -- |
| 5 | Canon | Canoness |
| 6 | Lama | -- |
| 7 | Patriarch | Matriarch |
| 8 | High Priest | High Priestess |

#### Rogue
| 等级 | M | F |
|------|---|---|
| 0 | Footpad | -- |
| 1 | Cutpurse | -- |
| 2 | Rogue | -- |
| 3 | Pilferer | -- |
| 4 | Robber | -- |
| 5 | Burglar | -- |
| 6 | Filcher | -- |
| 7 | Magsman | Magswoman |
| 8 | Thief | -- |

#### Ranger
| 等级 | M | F |
|------|---|---|
| 0 | Tenderfoot | -- |
| 1 | Lookout | -- |
| 2 | Trailblazer | -- |
| 3 | Reconnoiterer | Reconnoiteress |
| 4 | Scout | -- |
| 5 | Arbalester | -- |
| 6 | Archer | -- |
| 7 | Sharpshooter | -- |
| 8 | Marksman | Markswoman |

#### Samurai
| 等级 | M | F |
|------|---|---|
| 0 | Hatamoto | -- |
| 1 | Ronin | -- |
| 2 | Ninja | Kunoichi |
| 3 | Joshu | -- |
| 4 | Ryoshu | -- |
| 5 | Kokushu | -- |
| 6 | Daimyo | -- |
| 7 | Kuge | -- |
| 8 | Shogun | -- |

#### Tourist
| 等级 | M | F |
|------|---|---|
| 0 | Rambler | -- |
| 1 | Sightseer | -- |
| 2 | Excursionist | -- |
| 3 | Peregrinator | Peregrinatrix |
| 4 | Traveler | -- |
| 5 | Journeyer | -- |
| 6 | Voyager | -- |
| 7 | Explorer | -- |
| 8 | Adventurer | -- |

#### Valkyrie
| 等级 | M | F |
|------|---|---|
| 0 | Stripling | -- |
| 1 | Skirmisher | -- |
| 2 | Fighter | -- |
| 3 | Man-at-arms | Woman-at-arms |
| 4 | Warrior | -- |
| 5 | Swashbuckler | -- |
| 6 | Hero | Heroine |
| 7 | Champion | -- |
| 8 | Lord | Lady |

#### Wizard
| 等级 | M | F |
|------|---|---|
| 0 | Evoker | -- |
| 1 | Conjurer | -- |
| 2 | Thaumaturge | -- |
| 3 | Magician | -- |
| 4 | Enchanter | Enchantress |
| 5 | Sorcerer | Sorceress |
| 6 | Necromancer | -- |
| 7 | Wizard | -- |
| 8 | Mage | -- |

### 8.3 称号成就记录

达到新称号等级 (高于之前) 时, 记录成就 `ACH_RNK1` 到 `ACH_RNK8` (0 级在 1 级时不计为成就)。英雄在该时刻为女性时, 成就值取反 (负数)。

---

## 9. 经验获取来源

### 9.1 击杀怪物 (主要来源)

```
tmp = experience(mtmp, mvitals[mndx].died)
more_experienced(tmp, 0)
newexplevel()
```

### 9.2 旅行者拍照怪物

```
if 职业 == Tourist AND 非初始宠物 AND 非虫尾:
    more_experienced(experience(mtmp, 0), 0)  // nk=0: 无递减
    newexplevel()
```

### 9.3 旅行者探索新层

```
if 职业 == Tourist:
    more_experienced(level_difficulty(), 0)
    newexplevel()
```

### 9.4 其他来源

| 来源 | XP 量 | 分数加成 | 条件 |
|------|-------|---------|------|
| 击杀怪物 | `experience(mtmp, nk)` | 0 | 总是 |
| 旅行者拍照 | `experience(mtmp, 0)` | 0 | Tourist, 相机 |
| 旅行者新层 | `level_difficulty()` | 0 | Tourist, 进入新层 |
| 修理吱呀地板 | 1 | 5 | 拆除陷阱 |
| 拆除箱子陷阱 | 8 | 0 | 拆除陷阱 |
| 拆除门陷阱 | 8 | 0 | 拆除陷阱 |
| 吃狗粮 | 1 | 0 | 非 Cave/Orc/Tourist |
| 喝脏泉水 | 1 | 0 | 泉水 |
| 咨询神谕 (初次小) | ~5 或 ~2 | u_pay/50 | 首次小咨询 |
| 咨询神谕 (初次大) | ~100 或 ~40 | u_pay/50 | 首次大咨询 |
| 阅读小说段落 | 20 | 0 | 首次 Discworld 段落 |
| 治疗师治疗宠物 | min(delta, healamt) | 0 | Healer, 治疗驯服怪 |
| 薛定谔的猫 (死亡) | 20 | 10 | 开箱, 猫已死 |
| 物品鉴定 | 0 | 10 | 卷轴/药水/魔杖/刻写鉴定 |

### 9.5 分数计算 (`more_experienced`)

```
more_experienced(exper, rexp):
    u.uexp += exper                    // 经验值
    u.urexp += 4 * exper + rexp        // 分数

    // 溢出保护: 环绕时限制为 LONG_MAX
    if 新值 < 0 AND 增量 > 0:
        值 = LONG_MAX
```

每个击杀 XP 值 4 分。

### 9.6 新手标志

```
if urexp >= 1000 (Wizard) 或 2000 (其他):
    flags.beginner = FALSE
```

---

## 10. `rndexp` -- 变身/药水随机经验

```
rndexp(gaining):
    minexp = (ulevel == 1) ? 0 : newuexp(ulevel - 1)
    maxexp = newuexp(ulevel)
    diff = maxexp - minexp

    // 缩放 diff 以适合 rn2() 参数范围
    factor = 1
    while diff >= LARGEST_INT:
        diff /= 2
        factor *= 2

    result = minexp + factor * rn2(diff)

    // 30 级时获取: 加上当前超额 XP
    if ulevel == MAXULEV AND gaining:
        result += (uexp - minexp)
        if result < uexp:  // 溢出检查
            result = uexp

    return result
```

在当前等级范围内产生均匀分布的随机 XP 值。

---

## 11. 等级上限与溢出

- **MAXULEV = 30**: 最大经验等级
- `newexplevel()` 仅在 `ulevel < MAXULEV` 时触发 `pluslvl(TRUE)`
- 经验值在 30 级后继续累积, 无上限 (除 `LONG_MAX` 溢出保护)
- 增量升级时 XP 被截断到 `newuexp(ulevel + 1) - 1` 以防跳级
- 30 级后 HP/PW 增长被限流 (见 4.1 和 4.3 节)

---

## 12. 测试向量

### 12.1 经验阈值表

| 输入: `newuexp(lev)` | 期望输出 |
|---------------------|---------|
| `newuexp(-1)` | 0 (边界: lev < 1) |
| `newuexp(0)` | 0 (边界: lev < 1) |
| `newuexp(1)` | 20 |
| `newuexp(9)` | 5120 (边界: lev=9, 仍用第一公式) |
| `newuexp(10)` | 10000 (边界: lev=10, 切换到第二公式) |
| `newuexp(19)` | 5120000 (边界: lev=19, 仍用第二公式) |
| `newuexp(20)` | 10000000 (边界: lev=20, 切换到第三公式) |
| `newuexp(30)` | 110000000 |

### 12.2 怪物经验值计算

**向量 A: 简单低级怪物**

输入:
- `m_lev = 3`, `mac = 10` (AC 10), `mmove = 12` (NORMAL_SPEED)
- 无攻击 (全部 AT_NONE/AD_PHYS)
- 无 extra_nasty, 非邮件守护进程, 非复活/克隆
- `nk` 无关 (非复活)

计算:
- base = 1 + 9 = 10
- AC 加成: mac=10, 不 < 3, 故 0
- 速度: mmove=12, 不 > 12, 故 0
- 攻击方式: 全 AT_NONE (0), 不 > AT_BUTT, 故 0
- 伤害类型: 全 AD_PHYS (0), 故 0
- Extra nasty: 否, 0
- 高等级: m_lev 3 <= 8, 0

期望: **10**

---

**向量 B: 中级 nasty 怪物, 含特殊攻击**

输入:
- `m_lev = 10`, `mac = -2`, `mmove = 15`
- 攻击: [AT_WEAP/AD_PHYS/2d8, AT_MAGC/AD_SPEL/0d0, AT_CLAW/AD_DRLI/1d6, AT_NONE/.., AT_NONE/.., AT_NONE/..]
- M2_NASTY 已设置
- 非复活, `m_lev > 8`

计算:
- base = 1 + 100 = 101
- AC: mac=-2, < 3 且 < 0, 故 (7 - (-2)) * 2 = 18
- 速度: 15 > 12 但不 > 18, 故 +3
- 攻击方式:
  - AT_WEAP (254) > AT_BUTT: +5
  - AT_MAGC (255) > AT_BUTT: +10
  - AT_CLAW (1) 不 > AT_BUTT: +0
- 伤害类型:
  - AD_PHYS (0): 无加成; 2*8=16, 不 > 23: 无重伤加成
  - AD_SPEL (241): > AD_PHYS 且不在 1-10 范围, 非 DRLI/STON/SLIM, != AD_PHYS: +m_lev = +10; 0*0=0, 不 >23
  - AD_DRLI (15): == AD_DRLI: +50; 1*6=6, 不 >23
- Extra nasty: 7 * 10 = 70
- 高等级: m_lev 10 > 8: +50

合计: 101 + 18 + 3 + 5 + 10 + 10 + 50 + 70 + 50 = **317**

---

**向量 C: 巨鳗溺水加成**

输入:
- `m_lev = 5`, `mac = 9` (不 < 3), `mmove = 12`, monster class = S_EEL
- 攻击: [AT_TUCH/AD_WRAP/2d6, AT_BITE/AD_PHYS/1d4, 其余 AT_NONE]
- 玩家非水生

计算:
- base = 1 + 25 = 26
- AC: mac=9, 不 < 3: 0
- 速度: 12, 不 > 12: 0
- 攻击方式: AT_TUCH(5) > AT_BUTT: +3; AT_BITE(2) 不 > AT_BUTT: 0
- 伤害类型:
  - AD_WRAP(28): != AD_PHYS, 不在 1-10, 非 DRLI/STON/SLIM: +m_lev = +5; 2*6=12, 不 >23
  - AD_WRAP + S_EEL + 非水生: +1000
  - AD_PHYS(0): 无加成; 1*4=4, 不 >23
- Extra nasty: 否
- 高等级: 5 <= 8: 0

合计: 26 + 3 + 5 + 1000 = **1034**

---

**向量 D: 复活怪物递减 (nk=45)**

输入: 基础 XP = 100, 怪物已复活, nk = 45

```
tmp = 100, nk = 45, tmp2 = 20, i = 0

迭代 0: nk(45) > tmp2(20): tmp = (100+1)/2 = 50, nk = 25, i=0 偶数: tmp2 不变. i=1
迭代 1: nk(25) > tmp2(20): tmp = (50+1)/2 = 25, nk = 5, i=1 奇数: tmp2 = 40. i=2
迭代 2: nk(5) <= tmp2(40): 停止
```

期望: **25**

---

**向量 E: 复活怪物大量击杀 (nk=200)**

输入: 基础 XP = 200, 怪物已复活, nk = 200

```
tmp=200, nk=200, tmp2=20, i=0
i=0: nk(200)>20: tmp=(200+1)/2=100, nk=180, 偶数: tmp2=20, i=1
i=1: nk(180)>20: tmp=(100+1)/2=50, nk=160, 奇数: tmp2=40, i=2
i=2: nk(160)>40: tmp=(50+1)/2=25, nk=120, 偶数: tmp2=40, i=3
i=3: nk(120)>40: tmp=(25+1)/2=13, nk=80, 奇数: tmp2=60, i=4
i=4: nk(80)>60: tmp=(13+1)/2=7, nk=20, 偶数: tmp2=60, i=5
i=5: nk(20)<=60: 停止
```

期望: **7**

---

**向量 F: 恰好在升级阈值 (边界)**

玩家 5 级, uexp = 320 (即 `newuexp(5)`)。
`newexplevel()` 检查: `uexp(320) >= newuexp(ulevel=5)` 即 `320 >= 320` = 真。
结果: 调用 `pluslvl(TRUE)`, 升到 6 级。XP 截断: 若 `uexp >= newuexp(6)=640`, 设为 639。因 320 < 640, 无需截断。

---

**向量 G: 1 级吸取有 drainer (致命, 边界)**

玩家 1 级, uexp = 15, 无吸取抗性。
调用 `losexp("vampire")`。
- `u.ulevel > 1` 为假
- `drainer` ("vampire") 非 NULL
- 玩家死亡, 原因 "vampire"。若被生命保留, uexp = 0。

期望: 死亡 (或生命保留后, ulevel 保持 1, uexp = 0)。

---

**向量 H: 1 级吸取无 drainer (非致命, 边界)**

玩家 1 级, uexp = 15, 无吸取抗性。
调用 `losexp(NULL)` (神怒)。
- `u.ulevel > 1` 为假, `drainer` 为 NULL, 故无消息, 不死亡
- `uexp = 0`

期望: ulevel 保持 1, uexp = 0, 不死亡。

---

**向量 I: 法师 PW 计算**

Wizard, 5 级, Human, WIS=16

```
ulevel = 5 (< xlev=12)
enrnd = 16/2 = 8
enrnd += role.enadv.lornd(2) + race.enadv.lornd(0) = 10
enfix = role.enadv.lofix(0) + race.enadv.lofix(2) = 2

raw = rn1(10, 2) = rn2(10) + 2 = [2, 11]
en = enermod(raw) = 2 * raw = [4, 22]
```

期望范围: **4 到 22**

---

**向量 J: 野蛮人 PW 计算**

Barbarian, 12 级, Human, WIS=10

```
ulevel = 12 (>= xlev=10)
enrnd = 10/2 = 5
enrnd += role.enadv.hirnd(1) + race.enadv.hirnd(2) = 8
enfix = role.enadv.hifix(0) + race.enadv.hifix(2) = 2

raw = rn1(8, 2) = [2, 9]
en = enermod(raw) = (3 * raw) / 4
  raw=2: 6/4 = 1
  raw=9: 27/4 = 6
```

期望范围: **1 到 6**

---

**向量 K: 野蛮人矮人 HP 计算**

Barbarian Dwarf, 5 级, CON=18

```
ulevel = 5 (< xlev=10)
hp = role.hpadv.lofix(0) + race.hpadv.lofix(0) = 0
hp += rnd(role.hpadv.lornd=10) + rnd(race.hpadv.lornd=3)
   = [1,10] + [1,3] = [2, 13]
conplus = 3  (CON == 18)
hp += 3
范围: [5, 16]
```

期望范围: **5 到 16**

---

**向量 L: 旅行者侏儒高级 HP (边界)**

Tourist Gnome, 15 级, CON=7

```
ulevel = 15 (>= xlev=14)
hp = role.hpadv.hifix(0) + race.hpadv.hifix(0) = 0
rnd(role.hpadv.hirnd=0) -- 跳过 (hirnd=0)
rnd(race.hpadv.hirnd=0) -- 跳过 (hirnd=0)
hp = 0
conplus = 0  (CON 7, 在 7-14 范围)
hp += 0 = 0
hp <= 0, 故 hp = 1
```

期望: **1**

[疑似 bug] Tourist 是唯一高级 hirnd=0 (hifix=0) 的职业, Gnome/Orc 是唯一高级 hirnd=0 的种族。Tourist Gnome 和 Tourist Orc 在 >= xlev 后每级仅获得 1 HP (下限保护), 这是设计上的极端组合。

---

**向量 M: `adj_lev` 怪物等级调整**

输入: 怪物 mlevel=8, level_difficulty()=15, u.ulevel=12

```
tmp = 8
tmp2 = 15 - 8 = 7  (>= 0)
tmp += 7 / 5 = 1  => tmp = 9

tmp2 = 12 - 8 = 4  (> 0)
tmp += 4 / 4 = 1  => tmp = 10

upper = min(3 * 8 / 2, 49) = min(12, 49) = 12
tmp(10) <= upper(12), tmp(10) > 0

返回: 10
```

期望: **10**

---

**向量 N: `adj_lev` 上限截断 (边界)**

输入: 怪物 mlevel=5, level_difficulty()=30, u.ulevel=25

```
tmp = 5
tmp2 = 30 - 5 = 25  (>= 0)
tmp += 25 / 5 = 5  => tmp = 10

tmp2 = 25 - 5 = 20  (> 0)
tmp += 20 / 4 = 5  => tmp = 15

upper = min(3 * 5 / 2, 49) = min(7, 49) = 7
tmp(15) > upper(7)

返回: 7  (被截断到 1.5 * mlevel)
```

期望: **7**

---

**向量 O: 锻炼检定通过概率**

属性 A_STR, AEXE = 30 (锻炼积累):
```
threshold = abs(30) * 2 / 3 = 20
rn2(50) > 20 的概率 = 29/50 = 58%  (不通过)
rn2(50) <= 20 的概率 = 21/50 = 42% (通过)
```

属性 A_WIS, AEXE = 30:
```
threshold = abs(30) = 30
rn2(50) > 30 的概率 = 19/50 = 38%  (不通过)
rn2(50) <= 30 的概率 = 31/50 = 62% (通过)
```

期望: STR 通过率 42%, WIS 通过率 62% (智慧更容易)。

---

**向量 P: 恢复能力药水等级恢复 (边界)**

玩家曾达 15 级, 被吸取到 12 级, ulevelmax = 15。
喝祝福恢复能力药水:

```
do:
    pluslvl(FALSE)   // 升到 13
while ulevel(13) < ulevelmax(15) AND blessed: 继续
    pluslvl(FALSE)   // 升到 14
while ulevel(14) < ulevelmax(15) AND blessed: 继续
    pluslvl(FALSE)   // 升到 15
while ulevel(15) < ulevelmax(15): FALSE, 停止
```

期望: 等级恢复到 15 (等于 ulevelmax)。

未祝福的药水: 仅恢复一级 (到 13)。

---

## 附录 A: 关键常量

| 常量 | 值 | 位置 |
|------|---|------|
| MAXULEV | 30 | `include/global.h` |
| NORMAL_SPEED | 12 | `include/permonst.h` |
| NATTK | 6 | `include/permonst.h` |
| AT_BUTT | 4 | `include/monattk.h` |
| AT_WEAP | 254 | `include/monattk.h` |
| AT_MAGC | 255 | `include/monattk.h` |
| AD_PHYS | 0 | `include/monattk.h` |
| AD_BLND | 11 | `include/monattk.h` |
| AD_DRLI | 15 | `include/monattk.h` |
| AD_STON | 18 | `include/monattk.h` |
| AD_WRAP | 28 | `include/monattk.h` |
| AD_SLIM | 40 | `include/monattk.h` |
| M2_NASTY | 0x02000000 | `include/monflag.h` |
| AVAL | 50 | `src/attrib.c` (锻炼系统内部) |

## 附录 B: 攻击/伤害类型完整分类 (XP 计算用)

### 攻击方式 (aatyp) 对 XP 的影响

| 值 | 名称 | XP 加成 |
|----|------|--------|
| 0 | AT_NONE | 0 (不 > AT_BUTT) |
| 1 | AT_CLAW | 0 |
| 2 | AT_BITE | 0 |
| 3 | AT_KICK | 0 |
| 4 | AT_BUTT | 0 (不 > AT_BUTT, 是 == 而非 >) |
| 5-16 | AT_TUCH..AT_TENT | +3 |
| 254 | AT_WEAP | +5 |
| 255 | AT_MAGC | +10 |

### 伤害类型 (adtyp) 对 XP 的影响

| 值 | 名称 | XP 加成 |
|----|------|--------|
| 0 | AD_PHYS | 0 |
| 1-10 | AD_MAGM..AD_SPC2 | +2 * m_lev |
| 11-14 | AD_BLND..AD_PLYS | +m_lev |
| 15 | AD_DRLI | +50 |
| 16-17 | AD_DREN, AD_LEGS | +m_lev |
| 18 | AD_STON | +50 |
| 19-39 | AD_STCK..AD_FAMN | +m_lev |
| 40 | AD_SLIM | +50 |
| 41-43 | AD_ENCH..AD_POLY | +m_lev |
| 240-242 | AD_CLRC..AD_RBRE | +m_lev |
