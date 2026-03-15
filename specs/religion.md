# NetHack 3.7 宗教与祈祷机制规格

> 源码版本: NetHack-3.7 分支, pray.c rev 1.244, attrib.c, role.c, rnd.c, timeout.c, do.c
> 提取日期: 2026-03-14

---

## 1. 阵营系统 (Alignment)

### 1.1 阵营类型

```
A_LAWFUL  =  1  (秩序)
A_NEUTRAL =  0  (中立)
A_CHAOTIC = -1  (混沌)
A_NONE    = -128 (无阵营, Moloch 专用)
```

### 1.2 阵营记录 (alignment record)

`u.ualign.record` 是整型值, 表示玩家与自身神灵的关系.

- 初始值: 由职业 `initrecord` 字段决定 (所有职业为 10)
- 上限 `ALIGNLIM`:

```
ALIGNLIM = 10 + (moves / 200)
```

即: 随游戏进行, 可达到的上限不断增长 (每 200 回合 +1).

- 下溢无下限 (可负到 int 极限), `adjalign(n)` 实现:

```pseudocode
fn adjalign(n: i32):
    new = record + n
    if n < 0:
        if new < record:
            record = new         // 正常下降, 无下限
        abuse += (-n) as u32     // 累计恶行记录 (触发复仇女神机制)
    else if new > record:
        record = min(new, ALIGNLIM)  // 上限为 ALIGNLIM
```

### 1.3 阵营等级阈值 (用于祈祷/满意度)

```
PIOUS    = 20   // 虔诚
DEVOUT   = 14   // 忠诚
FERVENT  = 9    // 热忱
STRIDENT = 4    // 坚定
```

这些阈值在 `pleased()` 中决定神灵好感度措辞和赐福力度.

### 1.4 神怒 (ugangr)

`u.ugangr` 计数器表示神怒积累. 大于 0 时 `can_pray()` 判定为 p_type=1 (受罚).

- 增加: 祭祀宠物 (+3), 祭假圣器 (+3), 在非己阵营祭坛被拒 (+3), 同种族牺牲但非混沌 (+3), `gods_upset()` (+1 己神)
- 减少 (gods_upset): 当 `gods_upset(g_align)` 中 `g_align != ualign.type` 且 `ugangr > 0` 时, `ugangr--` (其他神被激怒反而减少自己神的怒气)
- 减少 (献祭): 在己阵营祭坛献祭有价值尸体时:

```pseudocode
reduction = value * (chaotic? 2 : 3) / MAXVALUE   // MAXVALUE = 24
ugangr = max(0, ugangr - reduction)
```

#### gods_upset() 完整逻辑

```pseudocode
fn gods_upset(g_align):
    if g_align == ualign.type:
        ugangr++              // 自己的神被激怒, 增加怒气
    else if ugangr > 0:
        ugangr--              // 其他神被激怒, 自己的神消气一点
    angrygods(g_align)
```

### 1.5 阵营转换

在非己阵营祭坛献祭可触发转换 (需同时满足):
- 当前 `ugod_is_angry()` (record < 0) **或** 祭坛为 Gehennom 的无阵营祭坛
- 从未转换过 (`ualignbase[A_CURRENT] == ualignbase[A_ORIGINAL]`)
- 祭坛不是无阵营 (A_NONE)

转换后果:
- `change_luck(-3)`
- `ublesscnt += 300`
- 阵营永久改变

---

## 2. 祈祷机制 (Prayer)

### 2.1 祈祷目标

```pseudocode
p_aligntyp =
    if on_altar(): 祭坛的阵营
    else: u.ualign.type (玩家阵营)
```

无论在哪里祈祷, 你的**本身神灵**回应祈祷. 在其他神灵的祭坛上祈祷由你自己的神回应, 这就是为什么会有坏事发生.

### 2.2 祈祷冷却 (ublesscnt)

`u.ublesscnt` 是回合倒数计时器, 大于 0 时祈祷"太早".

**冷却条件对照**:

| 玩家状态 | 祈祷判定条件 |
|---------|------------|
| 严重困境 (trouble > 0) | ublesscnt > 200 → 太早 |
| 轻微困扰 (trouble < 0) | ublesscnt > 100 → 太早 |
| 无困境 (trouble == 0) | ublesscnt > 0 → 太早 |

**冷却重置值**:

| 场景 | 新冷却值 |
|------|---------|
| 祈祷成功 (pleased) | `rnz(350)` |
| 受罚 (angrygods) | `rnz(300)`, 取 max(现有, 新值) |
| 太早祈祷失败 | ublesscnt += `rnz(250)` |
| 被称王 (crowned) | +kick_on_butt * `rnz(1000)` |
| 半神状态 (udemigod) | +kick_on_butt * `rnz(1000)` |
| 获得圣器赐予 | `rnz(300 + 50 * nartifacts)` |

其中 `kick_on_butt = (udemigod ? 1 : 0) + (crowned ? 1 : 0)`, 最大为 2.

**反自动化机制**: 当 `moves > 100000` 时:

```pseudocode
incr = (moves - 100000) / 100
ublesscnt += incr  // 上限 LARGEST_INT
```

### 2.3 rnz 函数 (随机化冷却值)

```pseudocode
fn rnz(i: i32) -> i32:
    x = i as i64
    tmp = 1000 + rn2(1000)       // 1000..1999
    tmp *= rne(4)                 // rne(4): 几何分布 1..max(ulevel/3,5)
    if rn2(2):
        x = x * tmp / 1000       // 放大
    else:
        x = x * 1000 / tmp       // 缩小
    return x as i32
```

`rnz(350)` 的典型范围: 约 175~700, 但极端情况可达 ~3500+.

### 2.4 祈祷类型判定 (p_type)

```pseudocode
fn can_pray() -> p_type:
    // 特殊情况
    if is_demon(hero) && (p_aligntyp == LAWFUL || p_aligntyp != NEUTRAL):
        return 拒绝 (恶魔只能向混沌或 Moloch 祈祷)
        // [疑似 bug] 条件 `A_LAWFUL || != A_NEUTRAL` 等价于
        // `A_LAWFUL || (A_CHAOTIC || A_NONE)`
        // = 除 A_NEUTRAL 外都拒绝.
        // 注释说 "ok if chaotic or none", 但实际上 A_NONE 时条件为
        // (0!=1 || 0!=0) = (true || false) = true → 也被拒绝.
        // [疑似 bug] 实际逻辑: 恶魔只允许向 A_NEUTRAL 祈祷 (即 p_aligntyp==0),
        // 这与注释 "ok if chaotic or none" 矛盾.

    // Moloch 祭坛
    if p_aligntyp == A_NONE:
        p_type = -2

    // 冷却检查 (见 2.2 节表格)
    else if 冷却未过:
        p_type = 0  // 太早

    // 态度检查
    else if Luck < 0 || ugangr > 0 || alignment < 0:
        p_type = 1  // 受罚

    // 非己阵营祭坛且条件 OK
    else if on_altar() && ualign.type != p_aligntyp:
        p_type = 2  // 跨阵营

    // 正常
    else:
        p_type = 3  // 成功

    // 不死覆盖
    if is_undead(hero) && !Inhell:
        if p_aligntyp == LAWFUL || (p_aligntyp == NEUTRAL && !rn2(10)):
            p_type = -1  // 不死生物在非混沌下危险
```

其中 `alignment` 的计算:

```pseudocode
alignment =
    if ualign.type != 0 && ualign.type == -p_aligntyp:
        -ualign.record           // 对立阵营祭坛 → 取反
    else if ualign.type != p_aligntyp:
        ualign.record / 2        // 不同阵营祭坛 → 减半
    else:
        ualign.record            // 同阵营 → 原值
```

### 2.5 祈祷过程

1. 玩家输入 `#pray`
2. 若 `ParanoidPray`, 要求确认
3. 破坏无神论戒律 (`u.uconduct.gnostic++`)
4. `can_pray(TRUE)` 判定 p_type
5. `nomul(-3)` 冻结 3 回合, 设置 `prayer_done` 为完成回调
6. **若 p_type == 3 且非 Gehennom**: 玩家获得无敌光环 (`uinvulnerable = TRUE`)

### 2.6 祈祷结果 (prayer_done)

#### p_type == -2 (Moloch 祭坛)

```
- 听到恶魔笑声, 唤醒附近怪物
- adjalign(-2)
- 若非 Gehennom: "Nothing else happens."
- 若在 Gehennom: 进入 Inhell 处理
```

#### p_type == -1 (不死形态)

```
- 神叱责, 强制还原人形
- losehp(rnd(20))
```

#### Inhell (Gehennom 中)

```
- "Since you are in Gehennom, <god> can't help you."
- if ualign.record <= 0 || rnl(ualign.record):
    angrygods(ualign.type)
```

#### p_type == 0 (太早)

```
- 在非己祭坛: 尝试诅咒水
- ublesscnt += rnz(250)
- change_luck(-3)
- gods_upset(ualign.type) → ugangr++ 并 angrygods
```

#### p_type == 1 (受罚)

```
- 在非己祭坛: 尝试诅咒水
- angrygods(ualign.type)
```

#### p_type == 2 (非己阵营祭坛, 但条件达标)

```
- 尝试诅咒水 (water_prayer(FALSE))
- 若有水被诅咒:
    ublesscnt += rnz(250), change_luck(-3), gods_upset
- 否则:
    pleased(alignment)
```

#### p_type == 3 (己阵营, 条件好)

```
- 在祭坛上:
    - 尝试复活宠物尸体/雕像 (pray_revive)
    - 祝福祭坛上的水 (water_prayer(TRUE))
- pleased(alignment)
```

### 2.7 困境系统 (Trouble)

`in_trouble()` 按优先级返回最严重的困境.

**严重困境 (正值, 按优先级递减)**:

| 值 | 困境 | 条件 |
|---|------|------|
| 14 | STONED | 正在石化 |
| 13 | SLIMED | 正在黏液化 |
| 12 | STRANGLED | 被绞杀 |
| 11 | LAVA | 陷在岩浆中 |
| 10 | SICK | 生病 |
| 9 | STARVING | 饥饿状态 >= WEAK |
| 8 | REGION | 在毒云中 |
| 7 | HIT | HP 严重过低 (见下) |
| 6 | LYCANTHROPE | 感染了兽化诅咒 |
| 5 | COLLAPSING | 严重超负荷且 STR 损失 > 3 |
| 4 | STUCK_IN_WALL | 卡在墙里 |
| 3 | CURSED_LEVITATION | 被诅咒悬浮 |
| 2 | UNUSEABLE_HANDS | 无法使用双手 |
| 1 | CURSED_BLINDFOLD | 被诅咒眼罩 |

**轻微困扰 (负值)**:

| 值 | 困境 | 条件 |
|---|------|------|
| -1 | PUNISHED | 被铁球束缚 |
| -2 | FUMBLING | 笨拙状态 |
| -3 | CURSED_ITEMS | 有诅咒装备 |
| -4 | SADDLE | 坐骑鞍被诅咒 |
| -5 | BLIND | 暂时失明/失聪 |
| -6 | POISONED | 属性被毒降低 |
| -7 | WOUNDED_LEGS | 腿伤 |
| -8 | HUNGRY | 饥饿状态 >= HUNGRY |
| -9 | STUNNED | 眩晕 |
| -10 | CONFUSED | 混乱 |
| -11 | HALLUCINATION | 幻觉 |

#### HP 严重过低判定 (critically_low_hp)

```pseudocode
fn critically_low_hp(only_if_injured: bool) -> bool:
    curhp = if Upolyd: u.mh else: u.uhp
    maxhp = if Upolyd: u.mhmax else: u.uhpmax
    if only_if_injured && curhp >= maxhp: return false

    hplim = 15 * u.ulevel
    if maxhp > hplim: maxhp = hplim

    divisor = match xlev_to_rank(ulevel):   // maps 1..30 → 0..8
        0 | 1 => 5     // level 1-5
        2 | 3 => 6     // level 6-13
        4 | 5 => 7     // level 14-21
        6 | 7 => 8     // level 22-29
        _     => 9     // level 30+

    return curhp <= 5 || curhp * divisor <= maxhp
```

### 2.8 pleased() — 神灵满意的响应

#### 修复行动等级

```pseudocode
fn pleased():
    trouble = in_trouble()

    if on_altar() && p_aligntyp != ualign.type:
        adjalign(-1)    // 在非己祭坛祈祷, 微扣阵营
        return
    if ualign.record < 2 && trouble <= 0:
        adjalign(1)     // 给点面子

    // 无困境 + 虔诚 → 可能获得额外恩惠
    if !trouble && ualign.record >= DEVOUT:
        if p_trouble == 0:   // 祈祷开始时也没困境
            pat_on_head = 1

    else:
        prayer_luck = max(Luck, -1)
        action = rn1(prayer_luck + (on_altar? 3 + on_shrine : 2), 1)
        // = rn2(prayer_luck + altar_bonus) + 1
        // altar_bonus: 不在祭坛=2, 祭坛=3, 圣殿=4

        if !on_altar():
            action = min(action, 3)    // 不在祭坛上限 3
        if ualign.record < STRIDENT:   // record < 4
            action = if record > 0 || !rnl(2): 1 else: 0

        match min(action, 5):
            5 => pat_on_head = 1; 修所有困境
            4 => 修所有困境
            3 => 修最严重 + 修所有严重困境 (最多10个)
            2 => 修所有严重困境 (最多9个)
            1 => 修最严重困境 (如果是严重的)
            0 => 无动于衷
```

#### pat_on_head (额外恩惠)

当所有困境已解决且有额外恩惠时:

```pseudocode
match rn2((Luck + 6) >> 1):     // 0 .. (Luck+6)/2 - 1
    0 => 无
    1 => 修复/祝福武器, 修复锈蚀
    2 => 金色光芒: 恢复失去的等级 或 +5 maxhp, 恢复满血/力量/饱食
    3 => 告知城堡曲调 (最多两次, 之后 fallthrough)
    4 => 蓝光: 解除所有诅咒物品
    5 => 赐予内在能力 (按优先级: Telepathy → Speed → Stealth → Protection)
    6 => 赐予法术书
    7|8 => if record >= PIOUS(20) && 未称王: gcrownu() (称王)
           else: 赐予法术书
```

#### give_spell() — 赐予法术书

pat_on_head case 6 (及 case 7|8 不满足称王条件时) 调用 `give_spell()`:

```pseudocode
fn give_spell():
    trycnt = ulevel + 1
    otmp = mkobj(SPBOOK_no_NOVEL, TRUE)   // 随机法术书 (排除小说)

    // 重试 trycnt 次, 偏好未知/遗忘的、非受限技能的法术
    while --trycnt > 0:
        if otmp.otyp != SPE_BLANK_PAPER:
            if known_spell(otmp.otyp) <= spe_Unknown      // 未知或遗忘
               && !P_RESTRICTED(spell_skilltype(otmp.otyp)):  // 非受限技能
                break   // 找到合适的法术
        else:  // 空白纸
            if !objects[SPE_BLANK_PAPER].oc_name_known
               || carrying(MAGIC_MARKER):
                break   // 未鉴定空白纸, 或有魔法标记笔
        otmp.otyp = rnd_class(bases[SPBOOK_CLASS], SPE_BLANK_PAPER)

    // 25% 概率直接学会法术 (而非获得法术书)
    if otmp.otyp != SPE_BLANK_PAPER
       && !rn2(4)
       && known_spell(otmp.otyp) != spe_Fresh:   // 非刚学过
        spe_let = force_learn_spell(otmp.otyp)
        if spe_let != '\0':
            // 直接学会, 销毁书
            显示 "Divine knowledge of <spell> fills your mind!"
        obfree(otmp)
    else:
        // 获得实体法术书
        bless(otmp)
        if otmp.otyp == SPE_BLANK_PAPER || !rn2(100):
            makeknown(otmp.otyp)    // 鉴定空白纸或 1% 鉴定其他
        放置在脚下
```

关键点:
- 重试次数 = `ulevel + 1`, 高等级玩家更可能获得合适法术
- 偏好: 未知/遗忘法术 > 已知法术, 可用技能 > 受限技能
- 25% 概率直接学会 (无需阅读), 但已熟知 (`spe_Fresh`) 的不会直接学
- 法术书总是被祝福

### 2.9 angrygods() — 神灵愤怒的后果

```pseudocode
fn angrygods(resp_god):
    if Inhell: resp_god = A_NONE
    u.ublessed = 0    // 失去神圣保护

    // 计算最大愤怒等级
    if resp_god != ualign.type:
        maxanger = ualign.record / 2 + (Luck>0? -Luck/3 : -Luck)
    else:
        maxanger = 3 * ugangr + ((Luck>0 || record>=STRIDENT)? -Luck/3 : -Luck)
    maxanger = clamp(maxanger, 1, 15)

    match rn2(maxanger):
        0|1 => "You feel that <god> is displeased."
        2|3 => 失去 WIS, 失去经验等级
        4|5 => 黑光诅咒: 50% attrcurse 或 rndcurse
        6   => 被惩罚 (铁球); 若已有则 fallthrough 到 4|5
        7|8 => 召唤爪牙 (summon_minion)
        9+  => god_zaps_you (闪电+分解光线, 可能致死)

    // 重置冷却
    new_ublesscnt = rnz(300)
    ublesscnt = max(ublesscnt, new_ublesscnt)
```

#### god_zaps_you 详细流程

1. 闪电打击:
   - 若被吞噬: 打吞噬者
   - 否则: 打玩家 (反射 → 无效; 电抗 → 无效; 否则 → `fry_by_god` 致死)
2. 广角分解光线:
   - 若被吞噬: 分解吞噬者
   - 否则: 依次摧毁盾牌、斗篷、盔甲、内衣, 然后分解玩家 (分解抗 → 存活)
3. 在星界层/圣殿: 额外召唤 3 个爪牙, "Destroy them, my servants!"

---

## 3. 祭祀机制 (Sacrifice)

### 3.1 可祭祀物品

只有**尸体** (`CORPSE`)、**真圣器** (`AMULET_OF_YENDOR`)、**假圣器** (`FAKE_AMULET_OF_YENDOR`) 可祭祀. 其他物品: "Nothing happens."

前提: 必须站在祭坛上, 不被吞噬, 未混乱/眩晕.

### 3.2 尸体祭祀值 (sacrifice_value)

```pseudocode
fn sacrifice_value(otmp) -> i32:
    if otmp.corpsenm == PM_ACID_BLOB
       || moves <= peek_at_iced_corpse_age(otmp) + 50:
        value = mons[corpsenm].difficulty + 1
        if otmp.oeaten:
            value = eaten_stat(value, otmp)    // 按吃掉比例折算
    else:
        value = 0    // 太旧, 无价值
    return value
```

关键: 尸体必须**不超过 50 回合** (或是酸液团), 否则价值为 0.

### 3.3 祭祀评估 (eval_offering)

```pseudocode
fn eval_offering(otmp, altaralign) -> i32:
    value = sacrifice_value(otmp)
    if value == 0: return 0

    if is_undead(mon):
        if ualign.type != CHAOTIC
           || (mon == WRAITH && uconduct.unvegetarian):
            value += 1   // 不死生物加成
    else if is_unicorn(mon):
        uni_align = sgn(mon.maligntyp)
        if uni_align == altaralign:
            // 祭与祭坛同阵营的独角兽 → 侮辱
            adjattrib(A_WIS, -1)
            return -1
        else if ualign.type == altaralign:
            // 祭非己阵营独角兽于己阵营祭坛 → 大善
            adjalign(5)
            value += 3
        else if uni_align == ualign.type:
            // 祭己阵营独角兽于非己祭坛 → 神怒, 可能转换
            ualign.record = -1
            value = 1
        else:
            // 其他情况
            value += 3
    return value
```

### 3.4 祭祀流程总览

```
dosacrifice()
  ├─ 真圣器 → offer_real_amulet() (游戏结束判定)
  ├─ 假圣器 → offer_fake_amulet()
  ├─ 尸体 → offer_corpse()
  └─ 其他 → "Nothing happens."
```

### 3.5 尸体祭祀详细流程 (offer_corpse)

1. **破坏无神论戒律** (`gnostic++`)
2. **同种族尸体** → `sacrifice_your_race()`:
   - 非混沌: adjalign(-5), ugangr += 3, adjattrib(WIS, -1), change_luck(-5), angrygods
   - 混沌: adjalign(+5)
   - 在混沌/无阵营祭坛: 召唤恶魔领主
   - 在其他祭坛: 祭坛被血染成混沌
   - 高祭坛 (且非混沌献混沌): desecrate_altar → god_zaps_you

3. **前宠物尸体**: adjalign(-3), 获得永久 Aggravate Monster, 然后 gods_upset

4. **value == 0**: "Nothing happens." (太旧)

5. **value < 0** (独角兽侮辱): gods_upset 或 desecrate_altar

6. **高祭坛 + 非己阵营**: desecrate_altar

7. **非己阵营祭坛**: `offer_different_alignment_altar()`:
   - 神怒或 (无阵营祭坛且在 Gehennom) 时:
     - 可首次转换阵营 (见 1.5): 需 `ualignbase[A_CURRENT] == ualignbase[A_ORIGINAL]` 且 `altaralign != A_NONE`
     - **已转换过或无阵营祭坛 → 被拒绝**:
       - `ugangr += 3`
       - `adjalign(-5)`
       - `change_luck(-5)`
       - `adjattrib(WIS, -2)`
       - 若非 Gehennom: `angrygods(ualign.type)`
   - 非神怒时: 有 `rn2(8+ulevel) > 5` (即 `(2+ulevel)/(8+ulevel)` 成功率) 机会转换祭坛
   - 成功: 祭坛变为己方, change_luck(+1)
   - 失败: change_luck(-1)

8. **己阵营祭坛, 正常祭祀**: 按优先级依次处理:

```pseudocode
if ugangr > 0:
    减少神怒 (见 1.4)
else if ugod_is_angry():          // record < 0
    adjalign(min(value, min(MAXVALUE, -record)))
else if ublesscnt > 0:
    减少冷却:
    ublesscnt -= value * (chaotic? 500 : 300) / MAXVALUE
    ublesscnt = max(0, ublesscnt)
else:
    // 尝试赐予神器, 然后增加运气
    if bestow_artifact(value): return
    luck_increase = value * LUCKMAX / (MAXVALUE * 2)
    // = value * 10 / 48 ≈ value / 5
    if uluck > value:
        luck_increase = 0
    else if uluck + luck_increase > value:
        luck_increase = value - uluck
    change_luck(luck_increase)
    if uluck < 0: uluck = 0
```

### 3.6 神器赐予概率 (bestow_artifact)

```pseudocode
fn bestow_artifact(max_giftvalue) -> bool:
    nartifacts = 已存在的神器数量
    do_bestow = ulevel > 2 && uluck >= 0
    if do_bestow:
        do_bestow = !rn2(6 + 2 * ugifts * nartifacts)
        // 概率 = 1 / (6 + 2 * ugifts * nartifacts)
    if do_bestow:
        otmp = mk_artifact(NULL, a_align(ux, uy), max_giftvalue, TRUE)
        if otmp:
            // 非负 spe, 不诅咒, 防锈蚀
            ugifts++
            ublesscnt = rnz(300 + 50 * nartifacts)
            return true
    return false
```

| ugifts | nartifacts | 概率 |
|--------|-----------|------|
| 0 | 任意 | 1/6 = 16.7% |
| 1 | 1 | 1/8 = 12.5% |
| 1 | 3 | 1/12 = 8.3% |
| 2 | 5 | 1/26 = 3.8% |
| 3 | 7 | 1/48 = 2.1% |

### 3.7 真圣器祭祀 (offer_real_amulet)

必须在高祭坛上 (AM_SANCTUM 标志).

| 祭坛阵营 | 结果 |
|---------|------|
| A_NONE (Moloch 圣殿) | record 降至 -99, 被杀死, 再被分解 |
| 非己阵营高祭坛 | adjalign(-99), 逃脱 (ESCAPED) |
| 己阵营高祭坛 | **飞升 (ASCENDED)** |

### 3.8 假圣器祭祀

```pseudocode
if !highaltar && !otmp.known:
    offer_too_soon()    // 不知是假的, 当作太早
elif !otmp.known:       // 高祭坛, 首次
    change_luck(-1), 发现是假的
else:                   // 明知是假的还敢献
    change_luck(-3), adjalign(-1), ugangr += 3
    gods_upset 或 desecrate_altar
```

---

## 4. 称王 (Crowning)

### 4.1 触发条件

在 `pleased()` 的 pat_on_head 分支, case 7|8:

```
ualign.record >= PIOUS (20) && u.uevent.uhand_of_elbereth == 0
```

### 4.2 固有赐予

称王赐予以下**永久外在属性** (FROMOUTSIDE):
- See_invisible
- Fire_resistance
- Cold_resistance
- Shock_resistance
- Sleep_resistance
- Poison_resistance

另外赐予 **1 个额外武器技能点**.

### 4.3 按阵营分类

#### 秩序 (Lawful) — "The Hand of Elbereth"

- 若持长剑且非神器 → 转化为 Excalibur
- 解锁 P_LONG_SWORD 技能
- 武器被祝福, 修复锈蚀, spe 至少 1

#### 中立 (Neutral) — "Envoy of Balance"

- 若已持 Vorpal Blade → 描述效果
- 若 Vorpal Blade 不存在 → 赐予 Vorpal Blade (spe=1)
- 解锁 P_LONG_SWORD 技能

#### 混沌 (Chaotic) — "chosen to steal souls"

- 若已持 Stormbringer → 描述效果
- 若 Stormbringer 不存在 → 赐予 Stormbringer (spe=1)
- 解锁 P_BROAD_SWORD 技能

#### 职业特殊赐予 (优先于武器赐予)

- **法师** (非持 Vorpal Blade/Stormbringer, 未持 SPE_FINGER_OF_DEATH): 赐予 SPE_FINGER_OF_DEATH
- **僧侣** (未持武器神器, 未持 SPE_RESTORE_ABILITY): 赐予 SPE_RESTORE_ABILITY

---

## 5. 祭坛机制

### 5.1 BUC 鉴定 (doaltarobj)

将物品丢在祭坛上:

```pseudocode
fn doaltarobj(obj):
    if Blind: return   // 看不见则无反馈

    if obj.oclass != COIN_CLASS:
        gnostic++   // 破坏无神论戒律

    if obj.blessed || obj.cursed:
        "There is a <amber|black> flash as <item> hits the altar."
        if !Hallucination: obj.bknown = true
    else:
        "<Item> lands on the altar."
        if obj.oclass != COIN_CLASS: obj.bknown = true
```

- 祝福: 琥珀色闪光 (amber)
- 诅咒: 黑色闪光 (black)
- 无状态 (uncursed): 无闪光, 但仍可识别
- 金币: 永远无 BUC 状态, 无反馈

### 5.2 水祈祷 (water_prayer)

在祭坛上祈祷, 祭坛上的 `POT_WATER` 会被影响:

- 成功祈祷 (p_type==3, 己阵营): **祝福**水
- 失败祈祷 (p_type==0/1/2): **诅咒**水

### 5.3 转换祭坛

通过在非己阵营祭坛献祭尸体, 有机会转换祭坛 (见 3.5.7).

成功率: `rn2(8 + ulevel) > 5`, 即 `(2 + ulevel) / (8 + ulevel)`.

| ulevel | 成功率 |
|--------|-------|
| 1 | 3/9 = 33% |
| 5 | 7/13 = 54% |
| 10 | 12/18 = 67% |
| 15 | 17/23 = 74% |
| 20 | 22/28 = 79% |
| 30 | 32/38 = 84% |

### 5.4 亵渎祭坛 (desecrate_altar)

高祭坛不能转换. 尝试亵渎 (在高祭坛献不当祭品, 或故意摧毁祭坛):

```pseudocode
fn desecrate_altar(highaltar, altaralign):
    if altaralign == ualign.type:
        adjalign(-20)
        ugangr += 5
    "You feel the air around you grow charged..."
    god_zaps_you(altaralign)
```

### 5.5 altar_wrath — 破坏祭坛的后果

踢、挖掘等破坏祭坛时:

```pseudocode
fn altar_wrath(x, y):
    altaralign = a_align(x, y)
    if ualign.type == altaralign && record > -rn2(4):
        "How darest thou desecrate my altar!"
        adjattrib(WIS, -1)
        record--
    else:
        "Thou shalt pay, infidel!"
        if Luck > -5 && rn2(Luck + 6):
            change_luck(rn2(20) ? -1 : -2)
```

---

## 6. 神灵名称

### 6.1 按职业的三位一体神系

| 职业 | 秩序 (lgod) | 中立 (ngod) | 混沌 (cgod) | 神话体系 |
|-----|------------|------------|------------|---------|
| Archeologist | Quetzalcoatl | Camaxtli | Huhetotl | 中美洲 |
| Barbarian | Mitra | Crom | Set | 海博利亚 |
| Caveman | Anu | *Ishtar | Anshar | 巴比伦 |
| Healer | *Athena | Hermes | Poseidon | 希腊 |
| Knight | Lugh | *Brigit | Manannan Mac Lir | 凯尔特 |
| Monk | Shan Lai Ching | Chih Sung-tzu | Huan Ti | 中国 |
| Priest | (随机借用其他职业的神系) | | | |
| Rogue | Issek | Mog | Kos | 涅吾 (Nehwon) |
| Ranger | Mercury | *Venus | Mars | 罗马/行星 |
| Samurai | *Amaterasu Omikami | Raijin | Susanowo | 日本 |
| Tourist | Blind Io | *The Lady | Offler | 碟形世界 |
| Valkyrie | Tyr | Odin | Loki | 北欧 |
| Wizard | Ptah | Thoth | Anhur | 埃及 |

带 `*` 前缀的名字在源码中以 `_` 开头 (如 `"_Ishtar"`), 表示该神为女性, `align_gtitle()` 返回 "goddess" 而非 "god".

### 6.2 特殊规则

- **Moloch** (`A_NONE`): 不属于任何神系, Gehennom 的叛逆之神
- **Priest 职业**: `lgod/ngod/cgod` 为 `0`, 在初始化时从随机其他职业借用神系 (`flags.pantheon`)
- 在 **Gehennom** 中, 所有神灵消息来自 Moloch

---

## 7. 运气系统 (Luck)

### 7.1 数据结构

```
u.uluck: schar      // 基础运气, -10..10 (LUCKMIN..LUCKMAX)
u.moreluck: schar   // 运气石加成, -3/0/+3 (LUCKADD)
Luck = u.uluck + u.moreluck    // 总有效运气, -13..13
```

### 7.2 运气石效果 (set_moreluck)

```pseudocode
fn set_moreluck():
    luckbon = stone_luck(true)   // 包含 uncursed
    if !luckbon && !carrying(LUCKSTONE):
        moreluck = 0
    else if luckbon >= 0:
        moreluck = +3            // 祝福或未诅咒运气石
    else:
        moreluck = -3            // 诅咒运气石
```

`confers_luck(obj)` 返回 true 的条件:
- `obj.otyp == LUCKSTONE`
- 或 obj 是带有 `SPFX_LUCK` 的神器

### 7.3 运气超时 (nh_timeout)

每 N 回合运气向基准值衰减:

```pseudocode
// 在 nh_timeout() 中, 每回合检查
baseluck = (full_moon? 1 : 0) - (friday13? 1 : 0) - (killed_quest_leader? 4 : 0)
           + (Archeologist && wearing_FEDORA? 1 : 0)

period = if u.uhave.amulet || u.ugangr: 300 else: 600

if moves % period == 0:
    time_luck = stone_luck(false)   // 只算 blessed
    nostone = !carrying(LUCKSTONE) && !stone_luck(true)

    if uluck > baseluck && (nostone || time_luck < 0):
        uluck--
    else if uluck < baseluck && (nostone || time_luck > 0):
        uluck++
```

**关键规则**:
- 无运气石: 正负运气都向 baseluck 衰减
- 祝福运气石: 阻止正运气衰减 (但负运气仍恢复)
- 诅咒运气石: 阻止负运气恢复 (但正运气仍衰减)
- 未诅咒运气石: 阻止双向衰减 (正不降, 负不升)

**衰减周期**: 正常 600 回合, 持圣器或神怒时 300 回合.

### 7.4 常见运气变化事件

| 事件 | Luck 变化 |
|------|----------|
| 独角兽角献祭 (非己阵营独角兽于己祭坛) | +1 (通过 change_luck) |
| 祭祀成功减少冷却后 luck < 0 时 | luck = 0 |
| 祈祷太早 | -3 |
| 假圣器 (首次, 未知) | -1 |
| 假圣器 (已知) | -3 |
| 转换阵营 | -3 |
| 转换祭坛成功 | +1 |
| 转换祭坛失败 | -1 |
| 同种族祭祀 (非混沌) | -5 |
| 同种族祭祀于无阵营祭坛 (混沌角色) | -2 |
| 同种族祭祀于混沌祭坛 (混沌角色) | +2 |
| 杀死 Quest 领袖 | baseluck -= 4 (永久) |
| 破坏祭坛 (非己阵营) | -1 (95%) 或 -2 (5%) |

### 7.5 祈祷对运气的要求

`can_pray()` 中: `(int) Luck < 0` → p_type = 1 (受罚). 即**有效运气 (含运气石加成) 必须 >= 0** 才能成功祈祷.

---

## 8. 无神论戒律 (Atheist Conduct)

`u.uconduct.gnostic` 计数器, 以下操作加 1:

| 操作 | 记录位置 |
|------|---------|
| 祈祷 (#pray) | `dopray()` |
| 祭祀尸体 (#offer) | `offer_corpse()` |
| 将物品丢在祭坛上 (非金币) | `doaltarobj()` |
| 使用 #turn (转化不死) | `doturn()` |

注意: 如果恶魔形态祈祷被拒 (is_demon 检查), **仍然**破坏戒律 (在 `can_pray` 之前已 `gnostic++`).

---

## 9. 转化不死 (#turn)

仅骑士和牧师可使用.

### 9.1 效果范围

```
range = BOLT_LIM + ulevel / 5    // 8 + ulevel/5, 范围 8..14
range_squared = range * range     // 用于距离比较
```

### 9.2 效果

对范围内可见的敌对不死/恶魔:

- 混乱时: "your voice falters", 解除怪物冻结
- 否则, 若怪物未抵抗:
  - 按怪物类别判定所需等级 (xlev):
    - S_ZOMBIE: 6
    - S_MUMMY: 8
    - S_WRAITH: 10
    - S_VAMPIRE: 12
    - S_GHOST: 14
    - S_LICH: 16
  - 若 ulevel >= xlev 且二次抵抗也失败:
    - 混沌: 怪物变和平
    - 其他: 杀死怪物
  - 否则: 怪物逃跑

### 9.3 后续

- `nomul(-(5 - (ulevel-1)/6))`: 瘫痪 1~5 回合
- 破坏无神论戒律
- 在 Gehennom 中: 无效, 只会 aggravate
- 神怒 > 6: "seems to ignore you", aggravate

---

## 10. 疑似 bug

### 10.1 恶魔祈祷条件 [疑似 bug]

`pray.c:2131-2132` 中的条件:

```pseudocode
if is_demon(hero) && (p_aligntyp == A_LAWFUL || p_aligntyp != A_NEUTRAL):
    // 拒绝祈祷
```

注释写 "ok if chaotic or none (Moloch)", 意为恶魔应该可以向混沌和 Moloch 祈祷. 但实际逻辑:
- `A_LAWFUL(1) || != A_NEUTRAL(0)` 对 A_CHAOTIC(-1) 求值: `(false || true) = true` → 被拒绝
- 对 A_NONE(-128) 求值: `(false || true) = true` → 被拒绝
- 仅对 A_NEUTRAL(0) 求值: `(false || false) = false` → 不被拒绝

**实际结果**: 恶魔只能向中立神祈祷, 与注释矛盾. 正确写法应为:

```pseudocode
if is_demon(hero) && (p_aligntyp != A_CHAOTIC && p_aligntyp != A_NONE):
    // 拒绝祈祷
```

或者原意可能是 `&&` 而非 `||`.

### 10.2 无神论戒律在 can_pray 前破坏

`pray.c:2221-2231` 中, `dopray()` 的执行顺序:

```pseudocode
// dopray() 中:
gnostic++                  // 先递增戒律计数
if gnostic 从 0 变为 1:
    livelog(...)            // 记录首次破坏戒律
if !can_pray(TRUE):        // 后检查, 恶魔可能被拒
    return                  // 即使被拒, 戒律已破坏
```

即使祈祷被 `can_pray` 拒绝 (如恶魔), 无神论戒律已被破坏. 代码注释也承认了这个问题:
> "breaking conduct should probably occur in can_pray()"

---

## 11. 测试向量

### 11.1 祈祷冷却判定

| # | trouble | ublesscnt | 预期 p_type (冷却) |
|---|---------|-----------|-------------------|
| 1 | STONED (14, 严重) | 201 | 0 (太早) |
| 2 | STONED (14, 严重) | 200 | 非 0 (不因冷却失败) |
| 3 | PUNISHED (-1, 轻微) | 101 | 0 (太早) |
| 4 | PUNISHED (-1, 轻微) | 100 | 非 0 (不因冷却失败) |
| 5 | 0 (无困境) | 1 | 0 (太早) |
| 6 | 0 (无困境) | 0 | 非 0 (不因冷却失败) |

### 11.2 尸体祭祀值

| # | corpsenm | difficulty | moves_since_death | oeaten | 预期 value |
|---|----------|-----------|-------------------|--------|-----------|
| 7 | acid blob | 1 | 999 | false | 2 (不看时间) |
| 8 | dragon (diff=20) | 20 | 30 | false | 21 |
| 9 | dragon (diff=20) | 20 | 51 | false | 0 (过期) |
| 10 | kobold (diff=0) | 0 | 10 | false | 1 |

### 11.3 神器赐予概率

| # | ulevel | uluck | ugifts | nartifacts | 预期 |
|---|--------|-------|--------|-----------|------|
| 11 | 2 | 5 | 0 | 0 | do_bestow=false (ulevel <= 2) |
| 12 | 3 | -1 | 0 | 0 | do_bestow=false (uluck < 0) |
| 13 | 10 | 3 | 0 | 2 | prob = 1/6 ≈ 16.7% |
| 14 | 10 | 3 | 2 | 4 | prob = 1/22 ≈ 4.5% |

### 11.4 边界条件

| # | 场景 | 预期 |
|---|------|------|
| 15 | ualign.record == 20, ALIGNLIM == 15 (moves=1000) | adjalign(+1) → record 仍为 15 (被 ALIGNLIM 限制) |
| 16 | uluck == LUCKMAX(10), change_luck(+5) | uluck 保持 10 (被 clamp) |
| 17 | 在 Gehennom 中成功祈祷 (p_type==3) | 不授予无敌; 结果: "can't help you" + 可能 angrygods |
| 18 | critically_low_hp: curhp=5, maxhp=100, ulevel=1 | hplim=15, maxhp 被限为 15; curhp<=5 → true |
| 19 | critically_low_hp: curhp=6, maxhp=30, ulevel=1 | hplim=15, maxhp 被限为 15; 6>5 且 6*5=30>15 → false |
| 20 | critically_low_hp: curhp=7, maxhp=30, ulevel=1 | hplim=15, maxhp 被限为 15; 7>5 且 7*5=35>15 → false |

### 11.5 运气超时

| # | 场景 | 预期 |
|---|------|------|
| 21 | uluck=5, 无运气石, moves%600==0, baseluck=0 | uluck 降到 4 |
| 22 | uluck=5, 祝福运气石, moves%600==0, baseluck=0 | uluck 保持 5 (祝福阻止正运气衰减) |
| 23 | uluck=-3, 诅咒运气石, moves%600==0, baseluck=0 | uluck 保持 -3 (诅咒阻止负运气恢复) |
| 24 | uluck=-3, 未诅咒运气石, moves%600==0, baseluck=0 | uluck 保持 -3 (未诅咒阻止双向衰减) |

---

## 附录 A: 关键常量汇总

| 常量 | 值 | 含义 |
|------|----|------|
| PIOUS | 20 | 称王所需最低 record |
| DEVOUT | 14 | pleased() 好感度阈值 |
| FERVENT | 9 | 好感度阈值 |
| STRIDENT | 4 | 最低 "满意" 阈值 |
| MAXVALUE | 24 | 祭祀值上限 (除 Wizard of Yendor) |
| LUCKMAX | 10 | u.uluck 上限 |
| LUCKMIN | -10 | u.uluck 下限 |
| LUCKADD | 3 | 运气石加成绝对值 |
| ALIGNLIM | 10 + moves/200 | alignment record 上限 |

## 附录 B: rnz 参考值

`rnz(N)` 的分布高度非线性 (由 rne(4) 的几何分布驱动).

对于 `rnz(350)` (pleased 后的冷却):
- 中位数约 350
- 50% 值落在 ~175 到 ~700 之间
- 极端高值可达数千 (概率极低)

对于 `rnz(300)` (angrygods 后的冷却):
- 类似分布, 中心在 300

`rne(4)`: 每次有 1/4 概率+1, 最大 max(ulevel/3, 5). 典型值 1~3.
