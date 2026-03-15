# NetHack 3.7 施法系统完整机制规格

> 源文件版本: spell.c $NHDT-Revision: 1.185$, zap.c, role.c, u_init.c, objects.h, spell.h, skills.h, you.h, objclass.h

---

## 目录

1. [数据结构](#1-数据结构)
2. [法术学习](#2-法术学习reading-spellbooks)
3. [法术记忆衰减](#3-法术记忆衰减)
4. [施法成功率公式](#4-施法成功率公式)
5. [能量 (Pw) 消耗公式](#5-能量-pw-消耗公式)
6. [护甲施法惩罚](#6-护甲施法惩罚)
7. [法术技能系统](#7-法术技能系统)
8. [法术效果](#8-法术效果)
9. [特殊法术](#9-特殊法术)
10. [遗忘 (amnesia)](#10-遗忘-amnesia)
11. [测试向量](#11-测试向量)

---

## 1. 数据结构

### 1.1 法术槽 (`struct spell`)

```
struct spell {
    short sp_id;   // 法术 ID = 对应的 object otyp (SPE_xxx); NO_SPELL (0) = 空槽
    xint16 sp_lev; // 法术等级 (1..7)
    int sp_know;   // 记忆值, 0 = 遗忘, KEEN (20000) = 满记忆
};
```

- 总法术槽数: `MAXSPELL = LAST_SPELL - FIRST_SPELL + 1` (SPE_BLANK_PAPER - SPE_DIG + 1)
  - 因为包含 SPE_BLANK_PAPER (无实际法术), 所以即使学满所有法术也至少有一个空终止槽
- 存储在 `svs.spl_book[MAXSPELL + 1]`

### 1.2 法术书对象属性 (objects.h SPELL 宏)

```
SPELL(name, desc, sub, prob, delay, level, mgc, dir, color, sn)
```

关键字段:
- `oc_delay`: 基础阅读延迟 (turns per unit)
- `oc_level` (= `oc_oc2`): 法术等级 1..7
- `oc_skill` (= `oc_subtyp`): 法术学派 (`P_ATTACK_SPELL`..`P_MATTER_SPELL`)

### 1.3 全部法术表

| 法术 | 学派 | 等级 | 延迟 | 方向 |
|------|------|------|------|------|
| dig | matter | 5 | 6 | RAY |
| magic missile | attack | 2 | 2 | RAY |
| fireball | attack | 4 | 4 | RAY |
| cone of cold | attack | 4 | 7 | RAY |
| sleep | enchantment | 3 | 1 | RAY |
| finger of death | attack | 7 | 10 | RAY |
| light | divination | 1 | 1 | NODIR |
| detect monsters | divination | 1 | 1 | NODIR |
| healing | healing | 1 | 2 | IMMEDIATE |
| knock | matter | 1 | 1 | IMMEDIATE |
| force bolt | attack | 1 | 2 | IMMEDIATE |
| confuse monster | enchantment | 1 | 2 | IMMEDIATE |
| cure blindness | healing | 2 | 2 | IMMEDIATE |
| drain life | attack | 2 | 2 | IMMEDIATE |
| slow monster | enchantment | 2 | 2 | IMMEDIATE |
| wizard lock | matter | 2 | 3 | IMMEDIATE |
| create monster | clerical | 2 | 3 | NODIR |
| detect food | divination | 2 | 3 | NODIR |
| cause fear | enchantment | 3 | 3 | NODIR |
| clairvoyance | divination | 3 | 3 | NODIR |
| cure sickness | healing | 3 | 3 | NODIR |
| charm monster | enchantment | 5 | 3 | IMMEDIATE |
| haste self | escape | 3 | 4 | NODIR |
| detect unseen | divination | 3 | 4 | NODIR |
| levitation | escape | 4 | 4 | NODIR |
| extra healing | healing | 3 | 5 | IMMEDIATE |
| restore ability | healing | 4 | 5 | NODIR |
| invisibility | escape | 4 | 5 | NODIR |
| detect treasure | divination | 4 | 5 | NODIR |
| remove curse | clerical | 3 | 5 | NODIR |
| magic mapping | divination | 5 | 7 | NODIR |
| identify | divination | 3 | 6 | NODIR |
| turn undead | clerical | 6 | 8 | IMMEDIATE |
| polymorph | matter | 6 | 8 | IMMEDIATE |
| teleport away | escape | 6 | 6 | IMMEDIATE |
| create familiar | clerical | 6 | 7 | NODIR |
| cancellation | matter | 7 | 8 | IMMEDIATE |
| protection | clerical | 1 | 3 | NODIR |
| jumping | escape | 1 | 3 | IMMEDIATE |
| stone to flesh | healing | 3 | 1 | IMMEDIATE |
| chain lightning | attack | 2 | 4 | NODIR |

---

## 2. 法术学习 (Reading Spellbooks)

### 2.1 阅读前提

在 `study_book()` 中, 按以下顺序检查:

1. **催眠书 (dull spellbook)**: 如果法术书描述为 "dull" 且角色无 Sleep_resistance 且不 Confused:
   ```
   dullbook = rnd(25) - ACURR(A_WIS)
   // 如果之前被打断过并且是同一本书:
   dullbook -= rnd(spell_level)
   if dullbook > 0:
       fall_asleep(-(dullbook + rnd(2 * spell_level)))
   ```

2. **空白书**: `SPE_BLANK_PAPER` 直接返回, 不消耗时间
3. **小说 (novel)**: `SPE_NOVEL` 触发致敬文本, 首次 +20 经验值

### 2.2 阅读时间计算

基于法术等级 (`oc_level`) 和书本延迟 (`oc_delay`):

```
match spell_level:
    1 | 2 => delay = -oc_delay
    3 | 4 => delay = -(spell_level - 1) * oc_delay
    5 | 6 => delay = -spell_level * oc_delay
    7     => delay = -8 * oc_delay
```

delay 为负值, 代表需要占据的回合数. 例如 magic missile: level=2, delay=2, 故阅读需 2 回合.
finger of death: level=7, delay=10, 故阅读需 80 回合.

**透镜 (lenses) 加速**: 每回合 50% 几率跳过一个延迟 tick, 等效缩短约 1/3 阅读时间:
```
if delay > 0 && ublindf && ublindf->otyp == LENSES && rn2(2):
    delay++  // 负向计数, ++ 使其更快到 0
```

### 2.3 阅读成功/失败

#### 2.3.1 祝福状态决定基础分支

- **被祝福 (blessed)**: 自动成功, 跳过难度检查
- **被诅咒 (cursed)**: 自动失败 (`too_hard = TRUE`)
- **未诅咒 (uncursed)**: 进入难度检查

#### 2.3.2 未诅咒书本难度检查

```
read_ability = ACURR(A_INT) + 4 + u.ulevel / 2
             - 2 * spell_level
             + (wearing_lenses ? 2 : 0)
```

- 巫师 (Wizard) 在 `read_ability < 20` 时获得警告:
  - `< 12`: "very difficult"
  - `12..19`: "difficult"
- 然后: `if rnd(20) > read_ability => too_hard = TRUE`

#### 2.3.3 失败后果 (`too_hard = TRUE`)

1. 触发 `cursed_book()` 效果 (见下)
2. 之后有 2/3 几率书本粉碎 ("crumbles to dust"), 加上 `cursed_book()` 返回 TRUE 的爆炸情况也销毁书本

#### 2.3.4 `cursed_book()` 效果

根据 `rn2(spell_level)` (0..level-1):

| rn2 结果 | 效果 |
|----------|------|
| 0 | 传送 (`tele()`) |
| 1 | 激怒怪物 (`aggravate()`) |
| 2 | 致盲 250+rn1(100) 回合 |
| 3 | 偷金币 (`take_gold()`) |
| 4 | 混乱 16+rn1(7) 回合 |
| 5 | 接触毒药 (戴手套腐蚀手套; 否则受毒素伤害) |
| 6 | 爆炸: 无魔法抗性则 `2*rnd(10)+5` 伤害, 并返回 TRUE (书本销毁) |
| default (>=7) | 随机诅咒 (`rndcurse()`) |

> **注意**: 等级 1 法术永远 rn2(1)=0, 只会传送. 等级 7 法术所有效果都可能.

#### 2.3.5 困惑状态阅读 (`confused_book()`)

- 1/3 几率撕毁书本 (SPE_BOOK_OF_THE_DEAD 豁免)
- 2/3 几率仅浪费时间

#### 2.3.6 成功学习 (`learn()`)

分两种情况:

**已知法术 (刷新):**
```
if spestudied > MAX_SPELL_STUDY (= 3):
    // 书本已被读取超过 3+1=4 次 (学习+刷新共计)
    "This spellbook is too faint to be read any more."
    书变成 SPE_BLANK_PAPER
    spestudied = rn2(old_spestudied)
else:
    sp_know = KEEN + 1 (= 20001)
    spestudied++
    exercise(A_WIS, TRUE)  // 额外智慧锻炼
```

**新法术:**
```
if spestudied >= MAX_SPELL_STUDY (= 3):
    // 多态产物, 太模糊
    书变成 SPE_BLANK_PAPER
else:
    spl_book[i].sp_id = booktype
    spl_book[i].sp_lev = spell_level
    sp_know = KEEN + 1 (= 20001)
    spestudied++
```

**关于 `spestudied` 和 `MAX_SPELL_STUDY`:**
- `MAX_SPELL_STUDY = 3`
- 已知法术检查 `> MAX_SPELL_STUDY` (即 `> 3`, 第 5 次起才太模糊)
- 新法术检查 `>= MAX_SPELL_STUDY` (即 `>= 3`, 第 4 次起就太模糊)
- 正常新书 `spestudied = 0`, 可学习 1 次 + 刷新 3 次 = 总计 4 次阅读
- 多态书可能 `spestudied > 0`, 减少可用次数

### 2.4 记忆刷新阈值

在 `study_book()` 中, 如果已知法术且 `sp_know > KEEN / 10` (= 2000 回合):
```
"You know [spell] quite well already."
"Refresh your memory anyway? [yn]"
```
可选择是否继续. 低于 2000 回合剩余记忆时, 不弹确认.

### 2.5 初始法术 (`initialspell()`)

游戏开始时通过起始物品法术书调用:
```
sp_know = KEEN (= 20000)  // 不加 1, 因为不经历"当回合衰减"
```

### 2.6 强制学习 (`force_learn_spell()`)

由神赐等机制调用:
```
if spell is blank/BotD/already Fresh: return '\0'
sp_know = KEEN (= 20000)
```

---

## 3. 法术记忆衰减

### 3.1 衰减机制

在 `age_spells()` 中, 每经过一个 moveloop 迭代:

```
for each known spell i:
    if sp_know > 0:
        sp_know--
```

- 每游戏回合衰减 1 点
- **不受** 角色速度/休息/昏迷状态影响 -- 基于时间流逝而非角色动作
- `KEEN = 20000` 回合 = 满记忆
- 阅读时设为 `KEEN + 1 = 20001` (补偿阅读当回合的衰减)

### 3.2 记忆状态分级

| sp_know 范围 | 状态 | 施法效果 |
|-------------|------|---------|
| = KEEN (20000) | 满记忆 | 正常施法 |
| > KEEN/10 (2000) | Fresh | 正常施法; 不允许"太熟了"刷新跳过 |
| KEEN/20 (1000) < k <= KEEN/10 (2000) | 渐渐模糊 | "Your recall of this spell is gradually fading." |
| KEEN/40 (500) < k <= KEEN/20 (1000) | 知识模糊 | "Your knowledge of this spell is growing faint." |
| KEEN/200 (100) < k <= KEEN/40 (500) | 难以回忆 | "You have difficulty remembering the spell." |
| 0 < k <= KEEN/200 (100) | 勉强回忆 | "You strain to recall the spell." |
| k <= 0 | 遗忘 | 施法失败 (backfire), 无法正常施法 |

### 3.3 记忆保留显示精度

在法术列表中显示的记忆百分比精度取决于对应学派技能:

```
percent = (sp_know - 1) / (KEEN / 100) + 1  // 即 1..100
accuracy = match skill:
    Expert       => 2
    Skilled      => 5
    Basic        => 10
    Unskilled/Restricted => 25
// 向上取整到 accuracy 的倍数
displayed = accuracy * ((percent - 1) / accuracy + 1)
// 显示为 "(displayed - accuracy + 1)% - displayed%"
```

---

## 4. 施法成功率公式

### 4.1 总体流程 (`percent_success()`)

成功率计算分为两部分: **内在能力 (splcaster)** 和 **学习能力 (chance)**, 最终合并.

### 4.2 内在能力 (splcaster)

```
splcaster = urole.spelbase

// 护甲惩罚 (详见第 6 节)
if uarm && is_metallic(uarm) && !paladin_bonus:
    if uarmc && uarmc.otyp == ROBE:
        splcaster += urole.spelarmr / 2
    else:
        splcaster += urole.spelarmr
else if uarmc && uarmc.otyp == ROBE:
    splcaster -= urole.spelarmr  // 长袍奖励

if uarms:  // 任何盾牌
    splcaster += urole.spelshld

if uwep && uwep.otyp == QUARTERSTAFF:
    splcaster -= 3  // 法杖小奖励

if !paladin_bonus:
    if uarmh && is_metallic(uarmh):
        splcaster += 4   // uarmhbon
    if uarmg && is_metallic(uarmg):
        splcaster += 6   // uarmgbon
    if uarmf && is_metallic(uarmf):
        splcaster += 2   // uarmfbon

// 角色特殊法术奖励
if spell == urole.spelspec:
    splcaster += urole.spelsbon  // 所有角色为 -4

// 治疗系法术奖励
if spell in {HEALING, EXTRA_HEALING, CURE_BLINDNESS,
             CURE_SICKNESS, RESTORE_ABILITY, REMOVE_CURSE}:
    splcaster += urole.spelheal

// 上限
splcaster = min(splcaster, 20)
```

其中 `paladin_bonus` = 骑士 (Knight) 施放 clerical 法术时为 TRUE, 豁免金属护甲和头盔/手套/靴子惩罚.

### 4.3 学习能力 (chance)

```
// 基础: 基于角色法术属性
statused = ACURR(urole.spelstat)  // INT 或 WIS, 取决于角色
chance = 11 * statused / 2        // 整数除法

// 难度计算
skill = max(P_SKILL(spell_skilltype), P_UNSKILLED) - 1
// skill: Restricted/Unskilled => 0, Basic => 1, Skilled => 2, Expert => 3
difficulty = (spell_level - 1) * 4 - (skill * 6 + u.ulevel / 3 + 1)

if difficulty > 0:
    chance -= isqrt(900 * difficulty + 2000)
    // isqrt 为整数平方根
else:
    learning = 15 * (-difficulty) / spell_level
    chance += min(learning, 20)

// 裁剪
chance = clamp(chance, 0, 120)
```

### 4.4 重盾惩罚

```
if uarms && weight(uarms) > 30:  // > small shield (weight 30)
    if spell == urole.spelspec:
        chance /= 2
    else:
        chance /= 4
```

### 4.5 最终合并

```
chance = chance * (20 - splcaster) / 15 - splcaster
chance = clamp(chance, 0, 100)
```

### 4.6 施法判定

```
if Confused || rnd(100) > chance:
    "You fail to cast the spell correctly."
    u.uen -= energy / 2
    return  // 失败但消耗半能量和 1 回合
```

---

## 5. 能量 (Pw) 消耗公式

### 5.1 基础消耗

```
energy = SPELL_LEV_PW(spell_level) = spell_level * 5
// 范围: 5 (level 1) 到 35 (level 7)
```

### 5.2 Amulet of Yendor 惩罚

如果携带 Amulet 且 `u.uen >= energy`:
```
"You feel the amulet draining your energy away."
u.uen -= rnd(2 * energy)  // 立即消耗 1..2*energy
if u.uen < 0: u.uen = 0
// 之后仍需检查 energy > u.uen
```

### 5.3 能量不足

```
if energy > u.uen:
    "You don't have enough energy to cast that spell."
    // 附加信息: "yet" (从未达到过) / "anymore" (曾达到过但下降了)
    // 不消耗回合 (除非 Amulet 已消耗了能量)
```

### 5.4 饥饿消耗

```
hungr = energy * 2

// 巫师 (Wizard) 智力减免:
if Role == Wizard:
    intell = ACURR(A_INT)
else:
    intell = 10  // 非巫师统一为 10

match intell:
    >= 17 => hungr = 0
    16    => hungr /= 4
    15    => hungr /= 2
    <= 14 => // 无减免

// 安全边界: 不使角色饿晕
if hungr > u.uhunger - 3:
    hungr = u.uhunger - 3

morehungry(hungr)
```

- `SPE_DETECT_FOOD` 豁免饥饿消耗
- `u.uhunger <= 10` 时禁止施法 (detect food 除外)
- `ACURR(A_STR) < 4` 时禁止施法 (restore ability 除外)
- 负重 (Overloaded/Overtaxed) 也阻止施法

### 5.5 遗忘法术的 backfire 消耗

```
if sp_know <= 0:
    u.uen -= rnd(energy)  // 1..energy 随机
    // 加上混乱/眩晕效果
```

### 5.6 失败施法消耗

```
if confused || rnd(100) > chance:
    u.uen -= energy / 2
```

### 5.7 成功施法消耗

```
u.uen -= energy  // 全额消耗
```

---

## 6. 护甲施法惩罚

### 6.1 角色数据表

从 `roles[]` 提取的法术相关参数:

| 角色 | spelbase | spelheal | spelshld | spelarmr | spelstat | spelspec | spelsbon |
|------|----------|----------|----------|----------|----------|----------|----------|
| Arc  | 5 | 0 | 2 | 10 | A_INT | magic mapping | -4 |
| Bar  | 14 | 0 | 0 | 8 | A_INT | haste self | -4 |
| Cav  | 12 | 0 | 1 | 8 | A_INT | dig | -4 |
| Hea  | 3 | -3 | 2 | 10 | A_WIS | cure sickness | -4 |
| Kni  | 8 | -2 | 0 | 9 | A_WIS | turn undead | -4 |
| Mon  | 8 | -2 | 2 | 20 | A_WIS | restore ability | -4 |
| Pri  | 3 | -2 | 2 | 10 | A_WIS | remove curse | -4 |
| Rog  | 8 | 0 | 1 | 9 | A_INT | detect treasure | -4 |
| Ran  | 9 | 2 | 1 | 10 | A_INT | invisibility | -4 |
| Sam  | 10 | 0 | 0 | 8 | A_INT | clairvoyance | -4 |
| Tou  | 5 | 1 | 2 | 10 | A_INT | charm monster | -4 |
| Val  | 10 | -2 | 0 | 9 | A_WIS | cone of cold | -4 |
| Wiz  | 1 | 0 | 3 | 10 | A_INT | magic missile | -4 |

**说明**:
- `spelbase`: 基础施法惩罚 (越高越差). Wizard=1 最佳, Barbarian=14 最差
- `spelheal`: 治疗系法术额外修正 (负 = 奖励). Healer=-3 最佳
- `spelshld`: 盾牌惩罚. Wizard=3 最大, Bar/Kni/Sam/Val=0 无惩罚
- `spelarmr`: 金属身体护甲惩罚. Monk=20 最大 (故意设计: 修道士不该穿甲)
- `spelsbon`: 所有角色 = -4 (对角色特殊法术的奖励)

### 6.2 惩罚明细

#### 身体护甲 (body armor, `uarm`)

仅在 `is_metallic(uarm)` 时生效:

```
if !paladin_bonus:
    if wearing_robe_as_cloak:
        splcaster += spelarmr / 2  // 长袍减半惩罚
    else:
        splcaster += spelarmr      // 全额惩罚
```

不穿金属甲但穿长袍:
```
splcaster -= spelarmr  // 奖励! 长袍本身降低施法难度
```

#### 盾牌 (`uarms`)

```
splcaster += spelshld  // 任何盾牌都会增加惩罚
```

此外, 重盾还有 chance 减半/四分之一 (见 4.4):
```
if weight(uarms) > 30:  // > small shield
    chance /= 4  // 或 /= 2 如果是角色特殊法术
```

#### 头盔 (`uarmh`)

```
if is_metallic(uarmh):
    splcaster += 4  // uarmhbon
```

Helm of brilliance 是非金属的 (despite the name), 所以不受惩罚.

#### 手套 (`uarmg`)

```
if is_metallic(uarmg):
    splcaster += 6  // uarmgbon, 最大的附件惩罚
```

#### 靴子 (`uarmf`)

```
if is_metallic(uarmf):
    splcaster += 2  // uarmfbon
```

#### 骑士 (Knight) 豁免

当 `Role == Knight && spell_school == P_CLERIC_SPELL` 时:
- 身体护甲金属惩罚 **跳过**
- 头盔/手套/靴子金属惩罚 **跳过**
- 盾牌惩罚 (`spelshld`) 仍然生效 (骑士 spelshld=0 所以无影响)
- 重盾 chance 惩罚仍然生效

#### 法杖 (Quarterstaff)

```
if wielding quarterstaff:
    splcaster -= 3  // 小奖励
```

---

## 7. 法术技能系统

### 7.1 技能等级

```
P_ISRESTRICTED = 0  // 无法提升
P_UNSKILLED    = 1  // 可提升
P_BASIC        = 2
P_SKILLED      = 3
P_EXPERT       = 4
```

### 7.2 各角色法术学派技能上限

从 `Skill_X[]` 表提取 (未列出 = P_RESTRICTED):

| 角色 | Attack | Healing | Divination | Enchantment | Clerical | Escape | Matter |
|------|--------|---------|------------|-------------|----------|--------|--------|
| Arc  | Basic  | Basic   | Expert     | -           | -        | -      | Basic  |
| Bar  | Basic  | -       | -          | -           | -        | Basic  | -      |
| Cav  | Basic  | -       | -          | -           | -        | -      | Skilled|
| Hea  | -      | Expert  | -          | -           | -        | -      | -      |
| Kni  | Skilled| Skilled | -          | -           | Skilled  | -      | -      |
| Mon  | Basic  | Expert  | Basic      | Basic       | Skilled  | Skilled| Basic  |
| Pri  | -      | Expert  | Expert     | -           | Expert   | -      | -      |
| Rog  | -      | -       | Skilled    | -           | -        | Skilled| Skilled|
| Ran  | -      | Basic   | Expert     | -           | -        | Basic  | -      |
| Sam  | Basic  | -       | Basic      | -           | Skilled  | -      | -      |
| Tou  | -      | -       | Basic      | Basic       | -        | Skilled| -      |
| Val  | Basic  | -       | -          | -           | -        | Basic  | -      |
| Wiz  | Expert | Skilled | Expert     | Skilled     | Skilled  | Expert | Expert |

### 7.3 技能对施法的影响

1. **成功率**: `difficulty` 公式中 `skill * 6` (skill 0..3)
2. **法术效果增强**: 多个法术在 `role_skill >= P_SKILLED` 时获得 blessed 效果
3. **记忆精度显示**: 技能越高, 显示的保留百分比越精确
4. **特殊**: fireball/cone of cold 在 Skilled+ 时变为可瞄准的多次爆炸

### 7.4 技能经验获取

成功施法后:
```
use_skill(spell_school, spell_level)
```

即每次成功施法为对应学派累积 `spell_level` 点技能经验.

### 7.5 巫师法术书识别

Wizard 角色根据法术学派技能自动识别相应等级的法术书:

```
match P_SKILL(school):
    Unskilled => 识别 level <= 1 (非 pauper 时; pauper 为 0)
    Basic     => 识别 level <= 3
    Skilled   => 识别 level <= 5
    Expert+   => 识别 level <= 7 (全部)
```

---

## 8. 法术效果

### 8.1 效果分类

法术效果实现基于三类复用:

- **Wand 效果** (`weffects()`): 大部分 RAY/IMMEDIATE 法术
- **Scroll 效果** (`seffects()`): remove curse, confuse monster, detect food, cause fear, identify, charm monster, magic mapping, create monster
- **Potion 效果** (`peffects()`): haste self, detect treasure, detect monsters, levitation, restore ability, invisibility

### 8.2 技能 >= Skilled 时的 blessed 效果

以下法术在 `role_skill >= P_SKILLED` 时, pseudo object 设为 blessed=1:
- healing, extra healing (wand-like)
- remove curse, confuse monster, detect food, cause fear, identify, charm monster (scroll-like)
- haste self, detect treasure, detect monsters, levitation, restore ability (potion-like)
- clairvoyance (自定义: blessed 时同时探测怪物)

### 8.3 Attack 学派

#### magic missile (SPE_MAGIC_MISSILE)
- 方向: RAY
- 等级: 2, Pw: 10
- 伤害: 通过 `weffects()` -> `zhitm()` -> `spell_damage_bonus()`

#### fireball (SPE_FIREBALL)
- 等级: 4, Pw: 20
- `role_skill >= P_SKILLED` 时: 可瞄准 (throwspell), 产生 `rnd(8)+1` 次爆炸
  - 每次爆炸伤害: `spell_damage_bonus(u.ulevel / 2 + 1)`
  - 后续爆炸位置在目标点周围 `rnd(3)-2` 偏移, 越界则回弹到中心
- `role_skill < P_SKILLED` 时: 作为普通 wand RAY

#### cone of cold (SPE_CONE_OF_COLD)
- 等级: 4, Pw: 20
- 与 fireball 完全相同的技能增强逻辑

#### finger of death (SPE_FINGER_OF_DEATH)
- 等级: 7, Pw: 35
- 方向: RAY
- 效果: 死亡射线 (通过 `weffects()`)

#### force bolt (SPE_FORCE_BOLT)
- 等级: 1, Pw: 5
- 方向: IMMEDIATE
- 物理伤害 (half_phys 减免适用于自我施法)

#### drain life (SPE_DRAIN_LIFE)
- 等级: 2, Pw: 10
- 方向: IMMEDIATE

#### chain lightning (SPE_CHAIN_LIGHTNING)
- 等级: 2, Pw: 10
- 方向: NODIR (自动向 8 方向传播)
- 特殊机制:
  - 从施法者向 8 个方向传播, 初始 strength=2
  - 每步 strength-1
  - 命中非电抗性怪物: strength 恢复至 3 (链接)
  - 命中电抗性怪物: strength=0 (终止)
  - 避开和平/驯化怪物
  - 链接传播时如果 `strength >= 2` 且 `u.uen > 0`: 额外消耗 1 Pw
  - 上限: `CHAIN_LIGHTNING_LIMIT = 100` 个格子
  - 不穿透墙壁、关闭的门

#### `spell_damage_bonus()` 公式

```
// 基于 ACURR(A_INT) 和 u.ulevel
match intell:
    <= 9            => dmg = max(dmg - 3, 1) if dmg > 1, else dmg
    10..13 or lvl<5 => dmg (无修改)
    14..18          => dmg + 1
    19..24 or lvl<14=> dmg + 2
    25              => dmg + 3
```

### 8.4 Healing 学派

#### healing (SPE_HEALING)
- 等级: 1, Pw: 5
- Skilled+ 时 blessed, 增强回复量
- 可指向其他生物 (IMMEDIATE)

#### extra healing (SPE_EXTRA_HEALING)
- 等级: 3, Pw: 15
- Skilled+ 时 blessed

#### cure blindness (SPE_CURE_BLINDNESS)
- 等级: 2, Pw: 10
- 无方向, `healup(0, 0, FALSE, TRUE)` -- 仅治愈失明

#### cure sickness (SPE_CURE_SICKNESS)
- 等级: 3, Pw: 15
- 无方向, `healup(0, 0, TRUE, FALSE)` + 清除 slime

#### restore ability (SPE_RESTORE_ABILITY)
- 等级: 4, Pw: 20
- Skilled+ blessed potion 效果

#### stone to flesh (SPE_STONE_TO_FLESH)
- 等级: 3, Pw: 15
- IMMEDIATE, wand 效果

### 8.5 Divination 学派

#### light (SPE_LIGHT)
- 等级: 1, Pw: 5
- NODIR, wand 效果

#### detect monsters (SPE_DETECT_MONSTERS)
- 等级: 1, Pw: 5
- NODIR, potion 效果; Skilled+ blessed

#### detect food (SPE_DETECT_FOOD)
- 等级: 2, Pw: 10
- NODIR, scroll 效果; Skilled+ blessed
- **特殊**: 不消耗饥饿, 即使 `uhunger <= 10` 也可施放

#### detect unseen (SPE_DETECT_UNSEEN)
- 等级: 3, Pw: 15
- NODIR, wand 效果

#### clairvoyance (SPE_CLAIRVOYANCE)
- 等级: 3, Pw: 15
- NODIR, `do_vicinity_map(pseudo)`; Skilled+ blessed (同时探测怪物)
- BClairvoyant 时被 cornuthaum 阻挡

#### detect treasure (SPE_DETECT_TREASURE)
- 等级: 4, Pw: 20
- NODIR, potion 效果; Skilled+ blessed

#### magic mapping (SPE_MAGIC_MAPPING)
- 等级: 5, Pw: 25
- NODIR, scroll 效果 (不受 skill 影响 blessed)

#### identify (SPE_IDENTIFY)
- 等级: 3, Pw: 15
- NODIR, scroll 效果; Skilled+ blessed

### 8.6 Enchantment 学派

#### confuse monster (SPE_CONFUSE_MONSTER)
- 等级: 1, Pw: 5
- IMMEDIATE, scroll 效果; Skilled+ blessed

#### sleep (SPE_SLEEP)
- 等级: 3, Pw: 15
- RAY, wand 效果

#### slow monster (SPE_SLOW_MONSTER)
- 等级: 2, Pw: 10
- IMMEDIATE, wand 效果

#### cause fear (SPE_CAUSE_FEAR)
- 等级: 3, Pw: 15
- NODIR, scroll 效果; Skilled+ blessed

#### charm monster (SPE_CHARM_MONSTER)
- 等级: 5, Pw: 25
- IMMEDIATE, scroll 效果; Skilled+ blessed

### 8.7 Clerical 学派

#### protection (SPE_PROTECTION)
- 等级: 1, Pw: 5
- NODIR, 自定义 `cast_protection()`
- 机制:
  ```
  loglev = floor(log2(u.ulevel)) + 1  // 1..5
  natac = u.uac + u.uspellprot  // 还原 protection 前的自然 AC
  natac_scaled = (10 - natac) / 10  // 整数除法, 转换正向并缩小

  gain = loglev - u.uspellprot / (4 - min(3, natac_scaled))

  if gain > 0:
      u.uspellprot += gain
      u.uspmtime = (skill == P_EXPERT) ? 20 : 10
      if u.usptime == 0:
          u.usptime = u.uspmtime
      find_ac()
  ```
  - 效果: 降低 AC (通过 `u.uspellprot`)
  - 持续时间: `u.usptime` 倒计时, 每 `u.uspmtime` 回合减少 1 点 uspellprot
  - Expert skill: 衰减周期 20 回合 vs 普通 10 回合

#### remove curse (SPE_REMOVE_CURSE)
- 等级: 3, Pw: 15
- NODIR, scroll 效果; Skilled+ blessed

#### create monster (SPE_CREATE_MONSTER)
- 等级: 2, Pw: 10
- NODIR, scroll 效果 (不受 skill 影响 blessed)

#### turn undead (SPE_TURN_UNDEAD)
- 等级: 6, Pw: 30
- IMMEDIATE, wand 效果

#### create familiar (SPE_CREATE_FAMILIAR)
- 等级: 6, Pw: 30
- NODIR, `make_familiar(NULL, u.ux, u.uy, FALSE)`

### 8.8 Escape 学派

#### haste self (SPE_HASTE_SELF)
- 等级: 3, Pw: 15
- NODIR, potion 效果; Skilled+ blessed

#### levitation (SPE_LEVITATION)
- 等级: 4, Pw: 20
- NODIR, potion 效果 (不受 skill 影响 blessed)

#### invisibility (SPE_INVISIBILITY)
- 等级: 4, Pw: 20
- NODIR, potion 效果 (不受 skill 影响 blessed)

#### teleport away (SPE_TELEPORT_AWAY)
- 等级: 6, Pw: 30
- IMMEDIATE, wand 效果

#### jumping (SPE_JUMPING)
- 等级: 1, Pw: 5
- IMMEDIATE, `jump(max(role_skill, 1))`
  - 跳跃距离取决于 skill level

### 8.9 Matter 学派

#### knock (SPE_KNOCK)
- 等级: 1, Pw: 5
- IMMEDIATE, wand 效果

#### wizard lock (SPE_WIZARD_LOCK)
- 等级: 2, Pw: 10
- IMMEDIATE, wand 效果

#### dig (SPE_DIG)
- 等级: 5, Pw: 25
- RAY, wand 效果

#### polymorph (SPE_POLYMORPH)
- 等级: 6, Pw: 30
- IMMEDIATE, wand 效果

#### cancellation (SPE_CANCELLATION)
- 等级: 7, Pw: 35
- IMMEDIATE, wand 效果

---

## 9. 特殊法术

### 9.1 死者之书 (Book of the Dead)

非施法, 而是阅读效果 (`deadbook()`):

- **在祭坛位置 (invocation_pos)** 且不在楼梯上:
  - 被诅咒: 符文打乱, 无效果
  - 无蜡烛台或钟: 寒意/声音提示
  - 蜡烛台 (7 支蜡烛, 点燃, 未诅咒) + 钟 (最近 5 回合内敲过, 未诅咒): 成功召唤 (mkinvokearea)
  - 其中之一被诅咒: 失败, 跳转到 raise_dead
  - 缺少条件: 只是提示

- **非祭坛位置**:
  - 被诅咒: 复活死者 (可能生成 master lich/nalfeshnee + 附近不死)
  - 被祝福: 安抚视线内不死生物 (同阵营+相邻 => 驯化)
  - 未诅咒: 三种风味文本之一

### 9.2 遗忘法术的 Backfire

`spell_backfire(spell)`:

```
duration = (spell_level + 1) * 3   // 6..24

match rn2(10):
    0..3 (40%):  Confusion += duration
    4..6 (30%):  Confusion += 2*duration/3, Stun += duration/3
    7..8 (20%):  Stun += 2*duration/3, Confusion += duration/3
    9    (10%):  Stun += duration
```

### 9.3 施法前提检查

在 `rejectcasting()` 和 `spelleffects_check()` 中:

1. **Stunned**: 无法施法
2. **无法吟唱** (`!can_chant()`): 无法施法 (例如无嘴)
3. **双手不自由** (`!freehand()`): 无法施法 (武器+盾焊接; 双手武器焊接)
   - **例外**: 持有 quarterstaff 时仍可施法
4. **饥饿**: `uhunger <= 10` 时禁止 (detect food 除外)
5. **力量**: `ACURR(A_STR) < 4` 时禁止 (restore ability 除外)
6. **负重**: `check_capacity()` -- Overtaxed/Overloaded

### 9.4 Fireball/Cone of Cold 技能增强 (throwspell)

当 `role_skill >= P_SKILLED`:
- 使用 `throwspell()` 选择目标位置
- 距离上限: `distmin(u, target) <= 10`
- 必须可见目标点
- 不能穿过实体墙壁

产生 `rnd(8)+1` (2..9) 次爆炸:
```
for n in 1..rnd(8)+1:
    if target == self:
        zapyourself(pseudo, TRUE)
    else:
        explode(target, spell_damage_bonus(u.ulevel / 2 + 1))
    // 后续目标偏移:
    target.x = original.x + rnd(3) - 2  // -1, 0, +1
    target.y = original.y + rnd(3) - 2
    if !valid_target: revert to original center
```

### 9.5 ^T 传送与法术系统交互

`tport_spell()` 在 wizard mode 中管理临时 teleport away 法术槽:
- `ADD_SPELL`: 临时插入 SPE_TELEPORT_AWAY 到法术列表
- `HIDE_SPELL`: 暂时隐藏已知的 teleport away
- `REMOVESPELL` / `UNHIDESPELL`: 恢复

---

## 10. 遗忘 (Amnesia)

### 10.1 `losespells()` -- 卷轴/陷阱触发

```
n = 已知法术数

nzap = rn2(n + 1)  // 0..n
if Confused:
    i = rn2(n + 1)
    nzap = max(nzap, i)  // 混乱取较差结果

if nzap > 1 && !rnl(7):   // 好运减免 [疑似 bug: rnl(7) 返回 0 概率受 Luck 影响]
    nzap = rnd(nzap)      // 1..nzap, 平均减半

// Fisher-Yates 风格等概率遗忘:
for i = 0; nzap > 0; i++:
    if rn2(n - i) < nzap:
        sp_know[i] = 0
        exercise(A_WIS, FALSE)
        nzap--
```

> [疑似 bug] `rnl(7)` 的好运检查: 当 Luck=0 时 `rnl(7)` 返回 `rn2(7)` 即 0..6, 失败条件 `!rnl(7)` 为 `!0` 即仅当 `rnl(7)==0` (1/7 概率). 当 Luck 高时 `rnl()` 倾向返回更低值, 使 `!rnl(7)` 更常为 TRUE, 这符合 "好运减免" 的设计意图. 但当 Luck < 0 时 `rnl()` 倾向高值, 减免更不可能, 这也是合理的. 实际上这不是 bug, 是正确行为.

### 10.2 记忆衰减 vs 遗忘

- `losespells()` 将 sp_know 设为 0 (完全遗忘, 但法术仍在列表中)
- `age_spells()` 每回合自然衰减 1 点
- 遗忘的法术可通过重读法术书恢复 (`incrnknow` -> sp_know = KEEN + 1)

---

## 11. 测试向量

### 11.1 施法成功率 (`percent_success`)

以下测试用例均为确定性计算 (无随机因素).

**参数格式**: `(spelbase, spelheal, spelshld, spelarmr, spelstat_attr, statused_value, spell_level, skill_level, u_level, spell_is_healing, spell_is_special, metal_body, robe_cloak, shield, heavy_shield, metal_helm, metal_gloves, metal_boots, paladin_bonus, quarterstaff)`

简化: 仅列出关键参数差异.

#### 测试 1: Wizard, 裸装, magic missile, Expert attack, INT 18, level 14

```
输入:
  spelbase=1, statused=18 (INT), spell_level=2, skill=Expert(3)
  u_level=14, 无护甲, 无盾, 无法杖
  spell_is_special=TRUE (magic missile), spelsbon=-4
  spell_is_healing=FALSE

计算 splcaster:
  splcaster = 1 + (-4) = -3  (special spell bonus)
  splcaster = min(-3, 20) = -3

计算 chance:
  chance = 11 * 18 / 2 = 99
  difficulty = (2-1)*4 - (3*6 + 14/3 + 1) = 4 - (18 + 4 + 1) = 4 - 23 = -19
  learning = 15 * 19 / 2 = 142
  chance += min(142, 20) = 20
  chance = 99 + 20 = 119
  chance = clamp(119, 0, 120) = 119

最终:
  chance = 119 * (20 - (-3)) / 15 - (-3) = 119 * 23 / 15 + 3 = 182 + 3 = 185
  chance = clamp(185, 0, 100) = 100

输出: 100%
```

#### 测试 2: Barbarian, 裸装, haste self, Basic escape, INT 7, level 1

```
输入:
  spelbase=14, statused=7 (INT), spell_level=3, skill=Basic(1)
  u_level=1, spell_is_special=TRUE, spelsbon=-4
  spell_is_healing=FALSE

splcaster = 14 + (-4) = 10

chance = 11 * 7 / 2 = 38  (整数除法: 77/2=38)
difficulty = (3-1)*4 - (1*6 + 1/3 + 1) = 8 - (6 + 0 + 1) = 8 - 7 = 1
chance -= isqrt(900*1 + 2000) = isqrt(2900) = 53
chance = 38 - 53 = -15
chance = clamp(-15, 0, 120) = 0

最终:
  chance = 0 * (20 - 10) / 15 - 10 = 0 - 10 = -10
  chance = clamp(-10, 0, 100) = 0

输出: 0%
```

#### 测试 3: Priest, robe, no shield, remove curse, Expert clerical, WIS 18, level 10

```
输入:
  spelbase=3, spelheal=-2, spelshld=2, spelarmr=10, statused=18 (WIS)
  spell_level=3, skill=Expert(3), u_level=10
  spell_is_special=TRUE (remove curse), spelsbon=-4
  spell_is_healing=TRUE (remove curse 在治疗列表中)
  robe_cloak=TRUE, no metal body armor

splcaster = 3 + (-10) [robe bonus] + (-4) [special] + (-2) [healing] = -13
splcaster = min(-13, 20) = -13

chance = 11 * 18 / 2 = 99
difficulty = (3-1)*4 - (3*6 + 10/3 + 1) = 8 - (18 + 3 + 1) = 8 - 22 = -14
learning = 15 * 14 / 3 = 70
chance += min(70, 20) = 20
chance = 99 + 20 = 119
chance = clamp(119, 0, 120) = 119

最终:
  chance = 119 * (20 - (-13)) / 15 - (-13) = 119 * 33 / 15 + 13
        = 261 + 13 = 274
  chance = clamp(274, 0, 100) = 100

输出: 100%
```

#### 测试 4: Ranger, no armor, invisibility (special), Basic escape, INT 13, level 5

```
输入:
  spelbase=9, spelheal=2, spelarmr=10, statused=13 (INT)
  spell_level=4, skill=Basic(1), u_level=5
  spell_is_special=TRUE, spelsbon=-4, spell_is_healing=FALSE

splcaster = 9 + (-4) = 5

chance = 11 * 13 / 2 = 71
difficulty = (4-1)*4 - (1*6 + 5/3 + 1) = 12 - (6 + 1 + 1) = 12 - 8 = 4
chance -= isqrt(900*4 + 2000) = isqrt(5600) = 74
chance = 71 - 74 = -3
chance = clamp(-3, 0, 120) = 0

最终:
  chance = 0 * (20 - 5) / 15 - 5 = -5
  chance = clamp(-5, 0, 100) = 0

输出: 0%
```

#### 测试 5: Wizard, full metal (chain mail, iron helm, gauntlets of power, iron shoes), no robe, no shield, magic missile, Expert, INT 18, level 20

```
splcaster = 1 + 10 [metal body] + 4 [metal helm] + 6 [metal gloves]
          + 2 [metal boots] + (-4) [special] = 19

chance = 11 * 18 / 2 = 99
difficulty = (2-1)*4 - (3*6 + 20/3 + 1) = 4 - (18 + 6 + 1) = 4 - 25 = -21
learning = 15 * 21 / 2 = 157
chance += min(157, 20) = 20
chance = 99 + 20 = 119
chance = clamp(119, 0, 120) = 119

最终:
  chance = 119 * (20 - 19) / 15 - 19 = 119 / 15 - 19 = 7 - 19 = -12
  chance = clamp(-12, 0, 100) = 0

输出: 0%
```

#### 测试 6: Wizard, metal body + robe, no shield, magic missile, Expert, INT 18, level 20

```
splcaster = 1 + 10/2 [metal body + robe = spelarmr/2] + (-4) [special] = 2

chance = 119 (same as test 5)

最终:
  chance = 119 * (20 - 2) / 15 - 2 = 119 * 18 / 15 - 2 = 142 - 2 = 140
  chance = clamp(140, 0, 100) = 100

输出: 100%
```

#### 测试 7: Knight, chain mail, iron helm, large shield (wt 100), turn undead (clerical), Skilled, WIS 14, level 8

```
paladin_bonus = TRUE (Knight + clerical spell)
splcaster = 8 + 0 [shield: spelshld=0] + (-4) [special: turn undead] = 4
// 身体/头盔/手套/靴子惩罚全部豁免

chance = 11 * 14 / 2 = 77
difficulty = (6-1)*4 - (2*6 + 8/3 + 1) = 20 - (12 + 2 + 1) = 20 - 15 = 5
chance -= isqrt(900*5 + 2000) = isqrt(6500) = 80
chance = 77 - 80 = -3
chance = clamp(-3, 0, 120) = 0

// 重盾 (weight 100 > 30): 对特殊法术 chance /= 2
chance = 0 / 2 = 0

最终:
  chance = 0 * (20 - 4) / 15 - 4 = -4
  chance = clamp(-4, 0, 100) = 0

输出: 0%
// 骑士等级 8 施放 6 级法术仍然非常困难
```

#### 测试 8: Healer, robe, no shield, cure sickness (special+healing), Expert healing, WIS 18, level 14

```
spelbase=3, spelheal=-3, spelsbon=-4, spelarmr=10
splcaster = 3 + (-10) [robe] + (-4) [special] + (-3) [healing] = -14
splcaster = min(-14, 20) = -14

chance = 11 * 18 / 2 = 99
difficulty = (3-1)*4 - (3*6 + 14/3 + 1) = 8 - (18 + 4 + 1) = -15
learning = 15 * 15 / 3 = 75
chance += min(75, 20) = 20
chance = 99 + 20 = 119
chance = clamp(119, 0, 120) = 119

最终:
  chance = 119 * (20 - (-14)) / 15 - (-14) = 119 * 34 / 15 + 14
        = 269 + 14 = 283
  chance = clamp(283, 0, 100) = 100

输出: 100%
```

#### 测试 9: Tourist, no armor, charm monster (special), Basic enchantment, INT 10, level 3

```
spelbase=5, spelsbon=-4
splcaster = 5 + (-4) = 1

chance = 11 * 10 / 2 = 55
difficulty = (5-1)*4 - (1*6 + 3/3 + 1) = 16 - (6 + 1 + 1) = 16 - 8 = 8
chance -= isqrt(900*8 + 2000) = isqrt(9200) = 95
chance = 55 - 95 = -40
chance = clamp(-40, 0, 120) = 0

最终:
  chance = 0 * (20 - 1) / 15 - 1 = -1
  chance = clamp(-1, 0, 100) = 0

输出: 0%
```

#### 测试 10: Wizard, robe, no other armor, light (level 1 div), Expert div, INT 20, level 30

```
spelbase=1, spell_is_special=FALSE, spell_is_healing=FALSE
splcaster = 1 + (-10) [robe] = -9
splcaster = min(-9, 20) = -9

chance = 11 * 20 / 2 = 110
difficulty = (1-1)*4 - (3*6 + 30/3 + 1) = 0 - (18 + 10 + 1) = -29
learning = 15 * 29 / 1 = 435
chance += min(435, 20) = 20
chance = 110 + 20 = 130
chance = clamp(130, 0, 120) = 120

最终:
  chance = 120 * (20 - (-9)) / 15 - (-9) = 120 * 29 / 15 + 9 = 232 + 9 = 241
  chance = clamp(241, 0, 100) = 100

输出: 100%
```

### 11.2 边界条件

#### 边界 1: splcaster 恰好等于 20 (上限)

```
Barbarian, full metal, no robe, shield, iron helm, iron gloves, iron boots, non-special spell
spelbase=14, spelarmr=8, spelshld=0
splcaster = 14 + 8 [metal body] + 0 [shield] + 4 [helm] + 6 [gloves] + 2 [boots] = 34
splcaster = min(34, 20) = 20

任何 chance 值:
  final = chance * (20 - 20) / 15 - 20 = 0 - 20 = -20
  final = clamp(-20, 0, 100) = 0

输出: 0%  (splcaster=20 意味着任何法术都不可能成功)
```

#### 边界 2: 阅读 read_ability 恰好为 20

```
Wizard 检查: read_ability >= 20 时不显示 difficulty 警告
read_ability = ACURR(A_INT) + 4 + u.ulevel/2 - 2*spell_level + lenses
例: INT=18, level=14, spell_level=7, no lenses
read_ability = 18 + 4 + 7 - 14 = 15 < 20 => 显示 "difficult" 警告

INT=18, level=14, spell_level=7, lenses=TRUE
read_ability = 18 + 4 + 7 - 14 + 2 = 17 < 20 => still warns

INT=18, level=20, spell_level=5, no lenses
read_ability = 18 + 4 + 10 - 10 = 22 >= 20 => 不警告
```

#### 边界 3: 法术书 spestudied 边界

```
已知法术:
  spestudied = 3: 可刷新 (> 3 is false, 3 is not > 3)
  spestudied = 4: 太模糊, 变为 blank paper (4 > 3 is true)

新法术:
  spestudied = 2: 可学习 (>= 3 is false)
  spestudied = 3: 太模糊, 变为 blank paper (3 >= 3 is true)
```

#### 边界 4: Pw 消耗边界

```
spell_level=1: energy = 5
spell_level=7: energy = 35

Amulet drain: u.uen -= rnd(2*energy)
  level 7: drain 1..70, 如果 u.uen=35, drain 后可能为 35-70=-35 => clamped to 0
  之后 energy(35) > u.uen(0) => "don't have enough energy"
```

#### 边界 5: 记忆衰减 -- sp_know = 1 vs sp_know = 0

```
sp_know = 1: 仍可施法, 但 "You strain to recall the spell." (<=100)
  施法成功后, 下一回合 age_spells 将其减到 0
sp_know = 0: "Your knowledge of this spell is twisted." => backfire
  消耗 rnd(energy), 产生混乱/眩晕, 无法术效果
```

#### 边界 6: difficulty = 0 恰好

```
difficulty = (spell_level - 1) * 4 - (skill * 6 + u_level / 3 + 1) = 0

此时走 else 分支:
  learning = 15 * 0 / spell_level = 0
  chance += 0

即 difficulty=0 等同于 "无惩罚无奖励"
```

---

## 附注: 疑似问题

1. **[疑似 bug]** `cast_chain_lightning()` 中当 `u.uswallow` 时直接 return, 注释说 "TODO: damage the engulfer", 即被吞噬时 chain lightning 完全无效果, 但仍消耗 Pw. 这是已知 TODO 而非 bug.

2. **[疑似 bug]** `spelleffects_check()` 中 `energy > u.uen` 的检查: 当 Amulet drain 后设了 `res = ECMD_TIME`, 但如果之后 `energy > u.uen` 的 `return TRUE` 路径不设 `*res`, 它保留了之前设的 `ECMD_TIME`. 然而仔细看代码, `return TRUE` 不设 `*res` 意味着调用者使用已设的 `res` 值, 这个值可能是 `ECMD_TIME` (Amulet case) 或 `ECMD_OK` (initial). 代码中 Amulet 分支设 `*res = ECMD_TIME` 后, `energy > u.uen` 分支无设置意味着隐式继承, 设计意图正确 (Amulet 消耗了回合).

3. **[疑似 bug]** `percent_success()` 中 `spellev(spell)` 在 difficulty<0 分支做除数: `15 * -difficulty / spellev(spell)`. 如果 `spellev(spell)` 为 0 会导致除零. 但实际中所有法术等级为 1..7, protection/jumping 等级 1 是最低的, 所以不会发生. SPE_BLANK_PAPER 等级为 0 但它不会出现在 spl_book 中 (或说 sp_id 不会是 SPE_BLANK_PAPER -- 除非 bug). 属于理论上的防御性编程缺失.

4. **[疑似 bug]** 在 `study_book()` 中, `read_ability` 公式: `ACURR(A_INT) + 4 + u.ulevel / 2 - 2 * spell_level + lenses_bonus`. 这里只用 INT, 不管角色的 spelstat 是 WIS 还是 INT. 这意味着高 WIS 低 INT 的 Priest/Knight 阅读高级法术书时更容易失败, 即使他们是该法术学派的专家. 这可能是有意设计 (阅读理解力用 INT) 也可能是疏忽.
