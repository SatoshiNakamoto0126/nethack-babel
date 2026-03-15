# Monster AI

> 来源：`src/monmove.c`、`src/wizard.c`、`src/mon.c`、`src/mthrowu.c`、`src/track.c`、`src/mondata.c`、`include/monst.h`、`include/monflag.h`、`include/mfndpos.h`

---

## 1. 概览：每回合 Monster 决策流

每个游戏 tick，`moveloop_core()` 调用 `movemon()`，对地图上每个存活怪物执行 `movemon_singlemon()`。怪物获得移动的条件是其 `movement` 点数 >= `NORMAL_SPEED (12)`，每次行动消耗 12 点。

### 1.1 移动点数计算 (`mcalcmove`)

```
base_mmove = permonst.mmove     // 物种基础速度

if mspeed == MSLOW:
    if base_mmove < 12:
        mmove = (2 * base_mmove + 1) / 3     // 损失约 1/3
    else:
        mmove = 4 + base_mmove / 3            // 损失约 2/3
elif mspeed == MFAST:
    mmove = (4 * base_mmove + 2) / 3         // 增加约 1/3

// 随机取整到 NORMAL_SPEED 的倍数（防止玩家预测 free turn）
mmove_adj = mmove % NORMAL_SPEED
mmove -= mmove_adj
if rn2(NORMAL_SPEED) < mmove_adj:
    mmove += NORMAL_SPEED
```

**示例：** speed=12 正常怪物每 tick 获得 12 点，刚好行动一次。speed=18 的怪物每 tick 获得 12 或 24（概率各 50%），平均 18/12 = 1.5 次行动。

---

## 2. 主决策循环 (`dochug`)

`dochug()` 分四阶段执行：

### Phase 1: 预处理

```pseudocode
fn dochug(mtmp):
    // STRAT_ARRIVE 一次性特殊动作
    if mtmp.mstrategy & STRAT_ARRIVE:
        mtmp.mstrategy &= ~STRAT_ARRIVE

    // WAITFORU: 看到玩家或受伤时取消等待
    if mtmp.mstrategy & STRAT_WAITFORU:
        if m_canseeu(mtmp) or mtmp.mhp < mtmp.mhpmax:
            mtmp.mstrategy &= ~STRAT_WAITFORU

    // 冻结/等待的怪物不行动
    if !mtmp.mcanmove or (mtmp.mstrategy & STRAT_WAITMASK):
        // STRAT_CLOSE 的怪物靠近时会对话 (quest_talk)
        return 0

    // 尝试唤醒沉睡怪物（详见 §3）
    if mtmp.msleeping and !disturb(mtmp):
        return 0

    // 混乱恢复：1/50 概率
    if mtmp.mconf and !rn2(50): mtmp.mconf = 0
    // 眩晕恢复：1/10 概率
    if mtmp.mstun and !rn2(10): mtmp.mstun = 0

    // 逃跑中的怪物 1/40 概率随机传送
    if mtmp.mflee and !rn2(40) and can_teleport(mdat) and !mtmp.iswiz:
        rloc(mtmp)
        return 0

    // m_respond: 尖叫怪(Shrieker)、美杜莎凝视、复仇女神(Erinys)aggravate
    m_respond(mtmp)

    // 逃跑勇气恢复：满血 + 非定时逃跑 + 1/25 概率
    if mtmp.mflee and !mtmp.mfleetim and mtmp.mhp == mtmp.mhpmax and !rn2(25):
        mtmp.mflee = 0
```

### Phase 2: 特殊移动与动作

```pseudocode
    set_apparxy(mtmp)  // 决定怪物认为玩家在哪（§5）

    // 贪婪怪物策略（§8）
    if is_covetous(mdat):
        tactics(mtmp)
        if mtmp.mstate: return 0  // 传送用掉了回合
        set_apparxy(mtmp)

    // 计算距离/恐惧
    distfleeck(mtmp, &inrange, &nearby, &scared)

    // 使用防御/杂项物品
    if find_defensive(mtmp, FALSE): use_defensive(mtmp)
    elif find_misc(mtmp): use_misc(mtmp)

    // 恶魔勒索
    if nearby and mdat.msound == MS_BRIBE and mtmp.mpeaceful:
        demon_talk(mtmp)

    // 守卫巡逻
    if is_watch(mdat): watch_on_duty(mtmp)
    // 夺心魔精神冲击：1/20 概率
    elif is_mind_flayer(mdat) and !rn2(20): mind_blast(mtmp)

    // 武器切换（近战距离 dist2 <= 8 时）
    if (!mtmp.mpeaceful or Conflict) and inrange and dist2 <= 8:
        if mtmp.weapon_check == NEED_WEAPON:
            mtmp.weapon_check = NEED_HTH_WEAPON
            mon_wield_item(mtmp)
```

### Phase 3: 实际移动

移动条件（满足任一即进入 `m_move`）：

```pseudocode
    should_move = (
        !nearby                         // 不在玩家旁边
        or mtmp.mflee                   // 正在逃跑
        or scared                       // 被 Elbereth/scare monster 吓到
        or mtmp.mconf                   // 混乱
        or mtmp.mstun                   // 眩晕
        or (mtmp.minvis and !rn2(3))    // 隐身 2/3 概率移动
        or (leprechaun 且玩家没金币)
        or (is_wanderer and !rn2(4))    // 游荡者 3/4 概率移动
        or (Conflict and !mtmp.iswiz)
        or (!mtmp.mcansee and !rn2(4))  // 失明 3/4 概率移动
        or mtmp.mpeaceful               // 和平怪物总是移动
    )
```

移动前可能施放非定向法术（距离 <= 49, `castmu` with `FALSE, FALSE`），之后调用 `m_move()`。

移动后如果不在玩家旁边但有远程攻击能力，仍可在同一回合发起远程攻击：

```pseudocode
    if moved and !nearby:
        if ranged_attk_available(mtmp) or attacktype(AT_WEAP) or find_offensive(mtmp):
            // 继续到 Phase 4 的攻击判断
```

### Phase 4: 标准攻击

```pseudocode
    if status != MMOVE_DONE and (!mtmp.mpeaceful or Conflict):
        if (inrange and !scared) or panicattk:
            mattacku(mtmp)
```

`panicattk` = 怪物被吓到且无路可逃 (MMOVE_NOMOVES + scared)。

---

## 3. 唤醒机制 (`disturb`)

沉睡怪物被唤醒的条件（全部 AND）：

```
条件 1: couldsee(mtmp.mx, mtmp.my)                     // 在视线内
条件 2: mdistu(mtmp) <= 100                              // 距离 <= 10 格（dist2 <= 100）
条件 3: !Stealth OR (ettin AND rn2(10))                  // 潜行无效或对 Ettin 9/10
条件 4: !(nymph OR jabberwock OR leprechaun) OR !rn2(50) // 这些怪只 1/50 概率醒
条件 5: Aggravate_monster                                // 激怒属性一定醒
         OR (dog OR human)                                // 狗/人类一定醒
         OR (!rn2(7) AND 非伪装家具/物品)                   // 1/7 概率醒
```

---

## 4. 恐惧与逃跑

### 4.1 `distfleeck` — 恐惧判定

```pseudocode
fn distfleeck(mtmp) -> (inrange, nearby, scared):
    inrange = dist2(mtmp.mx, mtmp.my, mtmp.mux, mtmp.muy) <= BOLT_LIM^2  // <= 64
    nearby  = inrange AND monnear(mtmp, mtmp.mux, mtmp.muy)  // 切比雪夫距离 <= 1

    // 确定怪物看到的恐惧源位置
    if !mtmp.mcansee or (Invis and !perceives(mdat)):
        scary_pos = (mtmp.mux, mtmp.muy)    // 怪物认为玩家在的位置
    else:
        scary_pos = (u.ux, u.uy)            // 玩家真实位置

    scared = nearby AND (
        onscary(scary_pos, mtmp)            // Elbereth/scare monster scroll
        OR (flees_light AND !rn2(5))        // Gremlin 怕光（4/5 概率）
        OR (!peaceful AND in_your_sanctuary) // 在你的神殿
    )

    if scared:
        flee_time = rnd(rn2(7) ? 10 : 100)
        // rn2(7): 6/7 概率 → rnd(10) = 1..10
        //         1/7 概率 → rnd(100) = 1..100
        monflee(mtmp, flee_time, TRUE, TRUE)
```

### 4.2 `monflee` — 设置逃跑

```pseudocode
fn monflee(mtmp, fleetime, first, fleemsg):
    if mtmp == u.ustuck: release_hero(mtmp)

    if !first or !mtmp.mflee:
        if fleetime == 0:
            mtmp.mfleetim = 0               // 永久逃跑（直到勇气恢复）
        elif !mtmp.mflee or mtmp.mfleetim:
            fleetime += mtmp.mfleetim
            if fleetime == 1: fleetime = 2   // 最小 2 回合
            mtmp.mfleetim = min(fleetime, 127)

        mtmp.mflee = 1

    mon_track_clear(mtmp)
```

`mfleetim` 每回合递减 1（在 `m_calcdistress` 中），减到 0 时 `mflee` 清零。

### 4.3 `onscary` — Elbereth / Scare Monster 判定

**完全免疫（不受任何恐惧影响）：**
- Wizard of Yendor (`mtmp.iswiz`)
- 守序仆从 (lawful minion)
- 天使 (Angel)
- Riders (Death, Famine, Pestilence)

**免疫地面恐惧源（Elbereth、scroll of scare monster）：**
- 人类 (`S_HUMAN`)
- 独特怪物 (`unique_corpstat`)

**特定场景免疫：**
- 店主在自己店里
- 牧师在自己神殿里

**Elbereth 额外限制（不影响 scroll of scare monster）：**
- 需要玩家在 Elbereth 格子上（或 displacement 映像在该格）
- 店主、守卫免疫
- 怪物失明则免疫
- 和平怪物免疫
- Minotaur 免疫
- 在 Gehennom 或 Endgame 中无效

**吸血鬼在祭坛上受到恐惧。**

### 4.4 逃跑中的勇气恢复

```
// 每回合在 dochug Phase 1 检查
if mtmp.mflee AND mtmp.mfleetim == 0 AND mtmp.mhp == mtmp.mhpmax AND !rn2(25):
    mtmp.mflee = 0
```

条件：非定时逃跑 + 满血 + 每回合 1/25 概率。

### 4.5 贪婪怪物的 HP 阈值 (`strategy`)

```pseudocode
fn strategy(mtmp) -> strat:
    ratio = (mtmp.mhp * 3) / mtmp.mhpmax   // 整除，结果 0-3

    switch ratio:
        case 0:                              // HP < 33.3%
            return STRAT_HEAL
        case 1:                              // HP 33.3%-66.6%
            if mdat != PM_WIZARD_OF_YENDOR:
                return STRAT_HEAL
            // Wizard 在此区间仍尝试进攻
            dstrat = STRAT_HEAL
        case 2:                              // HP 66.6%-99.9%
            dstrat = STRAT_HEAL
        case 3:                              // HP = 100%
            dstrat = STRAT_NONE

    // 然后按优先级尝试获取 invocation artifacts...
    // 如果什么都拿不到，返回 dstrat
```

**[疑似 bug]** `case 1` 中 Wizard 的 fallthrough 使得 `dstrat = STRAT_HEAL`，但随后如果有 target 就会去抢夺而不是治疗。这意味着 33%-66% HP 的 Wizard 在有目标时会冒险进攻。这似乎是 intentional design（"the wiz is less cautious"），但 fallthrough 到 `case 2` 设置 `dstrat = STRAT_HEAL` 意味着如果没有目标仍会选择治疗，行为一致。

---

## 5. 目标感知：`set_apparxy`

怪物将目标位置存储在 `(mux, muy)` 中——这是怪物认为玩家所在的位置。

```pseudocode
fn set_apparxy(mtmp):
    // 宠物、抓住你的怪物、已知你位置的怪物
    if mtmp.mtame or mtmp == u.ustuck or u_at(mux, muy):
        mtmp.mux = u.ux; mtmp.muy = u.uy
        return

    notseen = !mtmp.mcansee or (Invis and !perceives(mdat))
    notthere = Displaced and mdat != PM_DISPLACER_BEAST

    if Underwater:       displ = 1
    elif notseen:
        if mdat == PM_XORN and umoney > 0:
            displ = 0    // Xorn 闻到金子，精确定位
        else:
            displ = 1
    elif notthere:
        displ = couldsee(mux, muy) ? 2 : 1
    else:
        displ = 0        // 直接看到，精确定位

    if displ == 0:
        mtmp.mux = u.ux; mtmp.muy = u.uy
        return

    // 破解隐身/位移术：有概率直接定位
    gotu = notseen ? !rn2(3)        // 隐身：1/3 概率看穿
         : notthere ? !rn2(4)       // 位移：1/4 概率看穿
         : FALSE

    if !gotu:
        // 在玩家周围 displ 范围内随机选一个有效位置
        // 最多尝试 200 次
        loop (max 200):
            mx = u.ux - displ + rn2(2 * displ + 1)
            my = u.uy - displ + rn2(2 * displ + 1)
            // 过滤不可达位置
        mtmp.mux = mx; mtmp.muy = my
    else:
        mtmp.mux = u.ux; mtmp.muy = u.uy
```

---

## 6. 追踪算法

### 6.1 接近方向 (`appr`)

`appr` 决定怪物移动倾向：

| 值 | 含义 |
|----|------|
| +1 | 向目标靠近 |
| -1 | 远离目标（逃跑） |
| 0 | 随机移动 |
| -2 | 保持特定距离范围 |

```pseudocode
// 基础值
appr = mtmp.mflee ? -1 : 1

// 混乱或正在吞噬玩家 → 随机
if mtmp.mconf or engulfing_u: appr = 0

// 丧失追踪能力的情况 → appr = 0
if !mtmp.mcansee
   or (should_see and Invis and !perceives and rn2(11))  // 隐身时 10/11 失去追踪
   or player_disguised_as_object
   or (mtmp.mpeaceful and !mtmp.isshk)                   // 和平非店主
   or (stalker/bat/light and !rn2(3)):                    // 1/3 概率失去追踪
    appr = 0

// Leprechaun 拿了更多金子就跑
if appr == 1 and leppie_avoidance(mtmp): appr = -1

// 有远程攻击的敌对怪物试图保持距离（§11）
appr = m_balks_at_approaching(appr, mtmp, ...)
```

### 6.2 英雄轨迹追踪 (`track.c`)

```pseudocode
UTSZ = 100  // 轨迹缓冲区大小

fn settrack():
    // 每回合记录玩家位置到环形缓冲区
    utrack[utpnt] = (u.ux, u.uy)
    utpnt = (utpnt + 1) % UTSZ

fn gettrack(x, y) -> coord or NULL:
    // 从最新轨迹倒序搜索
    // 返回 distmin(x,y,track)==1 的最近轨迹点
    // distmin==0 返回 NULL（已经在轨迹上）
```

追踪条件 (`can_track`): 怪物有眼睛 (`haseyes`) 或玩家持有 Excalibur。

```pseudocode
// 在 m_move 中
if !should_see and can_track(ptr):
    cp = gettrack(omx, omy)
    if cp:
        ggx = cp.x; ggy = cp.y   // 跟着玩家足迹走
```

### 6.3 怪物自身轨迹 (`mtrack`)

怪物记住最近 4 个 (`MTSZ=4`) 位置，避免走回头路：

```pseudocode
// 选择移动位置时
for j in 0..min(MTSZ, cnt-1):
    if (nx, ny) == mtmp.mtrack[j]:
        if rn2(4 * (cnt - j)):  // 越近期的位置越倾向避开
            skip this position
```

### 6.4 `mfndpos` — 位置搜索

`mfndpos()` 检查怪物周围 8 个方向 + 当前位置，返回可移动到的位置列表及其属性标志。

关键标志由 `mon_allowflags()` 计算：

| 标志 | 条件 |
|------|------|
| `ALLOW_U` | 非和平，或 Conflict 生效 |
| `ALLOW_M` | 宠物 |
| `ALLOW_MDISP` | `is_displacer` (在 mfndpos 内部判断) |
| `OPENDOOR` | 有手且非极小体型 |
| `UNLOCKDOOR` | 能开门 + 持有钥匙/信用卡/开锁器，或是 Wizard/Rider |
| `BUSTDOOR` | 巨人 |
| `ALLOW_DIG` | 可挖掘（非 Rogue 关） |
| `ALLOW_WALL` | 穿墙 |
| `ALLOW_ROCK` | 穿墙或可扔石头或可碎石头 |
| `ALLOW_BARS` | 可穿过铁栏 |
| `NOTONL` | 独角兽在可传送层回避玩家直线 |
| `ALLOW_SSM` | 人类、Minotaur、店主、牧师等忽略 scare monster |
| `NOGARLIC` | 非鬼的不死生物/吸血鬼避开大蒜 |

### 6.5 移动选择

```pseudocode
for each position (nx, ny) in mfndpos results:
    ndist = dist2(nx, ny, ggx, ggy)

    // 回避玩家踢过的位置（和平/宠物）
    if m_avoid_kicked_loc: skip

    // 检查怪物轨迹避免走回头路
    // ...

    nearer = (ndist < nidist)

    // 选择最佳位置
    if (appr == 1 and nearer)           // 靠近：选更近的
       or (appr == -1 and !nearer)      // 逃跑：选更远的
       or (appr == 0 and !rn2(++chcnt)) // 随机：等概率
       or (appr == -2 and range-based)  // 保持距离
       or (mmoved == NOTHING):          // 第一个可移动位置
        select this position
```

### 6.6 Shortsighted 关卡

```pseudocode
if !mtmp.mpeaceful and level.flags.shortsighted:
    if nidist > (couldsee(nix,niy) ? 144 : 36) and appr == 1:
        appr = 0  // 太远则放弃追踪
```

视线内 > 12 格(144=12^2)，视线外 > 6 格(36=6^2)，怪物不再主动追踪。

---

## 7. 门处理

### 7.1 能力判定

```
can_open = !(nohands(ptr) || verysmall(ptr))
// 有手 AND 非极小体型

can_unlock = (can_open AND monhaskey(mtmp, TRUE))
             OR mtmp.iswiz OR is_rider(ptr)
// monhaskey: 持有 skeleton key / lock pick / credit card

doorbuster = is_giant(ptr)
```

### 7.2 门交互（`postmov` 中）

怪物移动到门格后，按优先级处理：

```pseudocode
if amorphous(ptr):
    // 从门下流过（雾云、光等）
    message: "flows/oozes under the door"

elif door.LOCKED and can_unlock:
    if door.TRAPPED and has_magic_key:
        disarm trap  // 无消息
    if trapped:
        door → D_NODOOR; mb_trapped() // 可能死亡
    else:
        door → D_ISOPEN
        message: "unlocks and opens a door"

elif door.CLOSED and can_open:
    if trapped:
        door → D_NODOOR; mb_trapped()
    else:
        door → D_ISOPEN
        message: "opens a door"

elif (door.LOCKED or door.CLOSED):
    // doorbuster (giant) 或其他情况
    if trapped or (LOCKED and !rn2(2)):
        door → D_NODOOR     // 完全摧毁
    else:
        door → D_BROKEN     // 破门
    message: "smashes down a door"
```

### 7.3 吸血鬼过门

吸血鬼如果不是 amorphous 形态，在遇到关门时会变成雾云 (fog cloud) 形态流过：

```pseudocode
if is_vampshifter(mtmp) and !amorphous(mdat):
    if IS_DOOR and (LOCKED or CLOSED) and can_fog(mtmp):
        vamp_shift(mtmp, PM_FOG_CLOUD)
```

`can_fog` 条件：fog cloud 未被灭绝 + `is_vampshifter` + 无 Protection_from_shape_changers + 无过大物品阻止通过。

### 7.4 挖掘过门

如果怪物能挖掘 (`can_tunnel`) 且需要镐 (`needspick`)：

```pseudocode
if closed_door(nix, niy):
    weapon_check = NEED_PICK_OR_AXE
elif IS_TREE:
    weapon_check = NEED_AXE
elif IS_STWALL:
    weapon_check = NEED_PICK_AXE

if weapon_check >= NEED_PICK_AXE:
    mon_wield_item(mtmp)  // 切换到镐/斧，消耗本回合
```

---

## 8. 贪婪怪物行为 (Covetous)

### 8.1 触发条件

`M3_COVETOUS = M3_WANTSAMUL | M3_WANTSBELL | M3_WANTSBOOK | M3_WANTSCAND | M3_WANTSARTI`

拥有这些 mflags3 的怪物包括：Wizard of Yendor、各种 demon princes、quest nemeses。

### 8.2 策略选择 (`strategy`)

优先级（gate 已开启后）：
1. Amulet of Yendor
2. Quest Artifact
3. Book of the Dead
4. Bell of Opening
5. Candelabrum of Invocation

gate 未开启时：
1. Amulet of Yendor
2. Book of the Dead
3. Bell of Opening
4. Candelabrum of Invocation
5. Quest Artifact

目标查找顺序：
1. 玩家持有 → `STRAT_PLAYER`，goal = 玩家位置
2. 地面上有 → `STRAT_GROUND`，goal = 物品位置
3. 其他怪物持有 → `STRAT_MONSTR`，goal = 该怪物位置

### 8.3 战术执行 (`tactics`)

```pseudocode
fn tactics(mtmp):
    strat = strategy(mtmp)

    switch strat:
        STRAT_HEAL:
            // 治疗/撤退
            if u.uswallow and u.ustuck == mtmp:
                expels(mtmp)   // 先吐出玩家
            choose_stairs(&sx, &sy, mtmp.m_id % 2)  // 随机选向前或向后的楼梯
            mtmp.mavenge = 1   // 治疗期间仍会攻击

            if In_W_tower or (iswiz and 无楼梯 and 没有护符):
                if !rn2(3 + mtmp.mhp / 10):
                    rloc(mtmp)  // 随机传送
            elif sx,sy 有效 and 不在楼梯上:
                mnearto(mtmp, sx, sy)  // 传送到楼梯附近

            // 如果远离玩家 (dist > BOLT_LIM^2=64) 且 HP <= max-8:
            if distu(mx,my) > 64 and mtmp.mhp <= mtmp.mhpmax - 8:
                healmon(mtmp, rnd(8), 0)  // 每回合恢复 1-8 HP
                return 1

        STRAT_NONE:  // 骚扰
            if !rn2(!mtmp.mflee ? 5 : 33):  // 不逃跑时 1/5，逃跑时 1/33
                mnexto(mtmp)   // 传送到玩家附近

        default:  // 抢夺
            if player_on_target or STRAT_PLAYER:
                mnearto(mtmp, tx, ty)  // 传送到目标附近
            elif STRAT_GROUND:
                rloc_to(mtmp, tx, ty)  // 直接传送到物品位置并拾取
            elif STRAT_MONSTR:
                mnearto(mtmp, tx, ty)  // 传送到持有者附近
```

### 8.4 Wizard of Yendor 骚扰

玩家击杀 Wizard 后设 `u.uevent.udemigod = TRUE`，初始倒计时 `u.udg_cnt = rn1(250, 50)` = 50..299 回合（`wizard.c`）。倒计时到 0 时调用 `intervene()`，之后重置为较短的重复倒计时 `u.udg_cnt = rn1(200, 50)` = 50..249 回合（`allmain.c`），意味着骚扰随时间加速：

| 骰子 (rn2(6)) | 效果 |
|----------------|------|
| 0 | "You feel vaguely nervous." (无操作) |
| 1 | 同上 |
| 2 | 随机诅咒 (`rndcurse`) |
| 3 | `aggravate()` — 唤醒所有怪物，取消 WAITFORU |
| 4 | `nasty()` — 召唤一组高级怪物 |
| 5 | `resurrect()` — 复活 Wizard |

Astral 层只会 roll rnd(4)=1..4（无 case 0 和 5）。

### 8.5 covetous 移动中攻击其他怪物

在 `m_move` 中，covetous 怪物如果目标格有其他怪物且相邻（`dist2 <= 2`），会直接攻击该怪物：

```pseudocode
if intruder and intruder != mtmp and dist2(mtmp, goal) <= 2:
    mattackm(mtmp, intruder)
```

---

## 9. 和平怪物行为

### 9.1 店主 (Shopkeeper)

- 移动由 `shk_move()` 处理（不走 `m_move` 主流程）
- 在自己店内时忽略 Elbereth / scare monster
- `can_open = TRUE`，可以正常开门
- 守店巡逻，追踪盗窃者

### 9.2 牧师 (Priest)

- 移动由 `pri_move()` 处理
- 在自己神殿内时忽略 Elbereth / scare monster
- 拥有 `ALLOW_SSM | ALLOW_SANCT` 标志
- 碎石能力：`m_can_break_boulder` 在 `!mtmp.mspec_used` 时为 TRUE

### 9.3 守卫 (Guard)

- 移动由 `gd_move()` 处理
- 免疫 Elbereth（`mtmp.isgd` 检查）
- 金库守卫引导玩家离开

### 9.4 Watch (City guards)

```pseudocode
fn watch_on_duty(mtmp):
    if mtmp.mpeaceful and in_town(u.ux+u.dx, u.uy+u.dy)
       and mtmp.mcansee and m_canseeu and !rn2(3):
        if player_picking_lock on locked_door:
            if already_warned:
                "Halt, thief!" → angry_guards()
            else:
                "Hey, stop picking that lock!"
                set D_WARNED
        elif player_digging:
            watch_dig(mtmp, ...)
```

### 9.5 和平怪物转敌对的触发条件

- **攻击和平怪物**：直接变敌对
- **盗窃店铺物品**：店主变敌对
- **在城镇撬锁被抓（第二次）**：守卫变敌对
- **攻击 quest leader**：所有 quest guardians 变敌对
- **Conflict 法术**：所有怪物临时敌对（检查 `resist_conflict`）

---

## 10. 按智能级别的策略差异

### 10.1 动物 (`M1_ANIMAL`) / 无脑 (`M1_MINDLESS`)

- 不会捡起物品（`mindless` 和 `is_animal` 跳过 `searches_for_item`）
- `M1_NOTAKE` 的怪物完全不拾取
- 躲藏在物品下方的怪物 (`hides_under`)：
  - 9/10 概率不离开隐藏位置（`rn2(10)` check in `m_move`）
  - 避开玩家可见的格子

### 10.2 人形怪物 / 武器使用者

- `AT_WEAP` 怪物在近战距离时切换武器（`NEED_HTH_WEAPON`）
- 困在陷阱中时倾向远程武器
- 可以开门 (`can_open`)
- 可以使用工具开锁 (`can_unlock`)

### 10.3 施法者

- 拥有 `AT_MAGC` 且 `AD_SPEL` 或 `AD_CLRC` 的攻击
- 非战斗移动时有机会施放非定向法术（距离 <= 7 格，`dist2 <= 49`）
- `mspec_used` 冷却期限制施法频率

### 10.4 Tengu（传送者）

```pseudocode
if ptr == PM_TENGU and !rn2(5) and !mtmp.mcan:
    if mtmp.mhp < 7 or mtmp.mpeaceful or rn2(2):
        rloc(mtmp)             // 随机传送
    else:
        mnexto(mtmp)           // 传送到玩家旁边
```

### 10.5 独角兽

- 在可传送层：设 `NOTONL` 标志，避免与玩家在同一直线
- 如果无路可走，50% 概率随机传送走

---

## 11. 远程攻击决策

### 11.1 距离攻击类型

`DISTANCE_ATTK_TYPE(atyp)` = `AT_SPIT || AT_BREA || AT_MAGC || AT_GAZE`

### 11.2 对齐检查 (`lined_up`)

攻击需要怪物与目标在一条直线上（水平、垂直或 45 度对角线），且路径上无阻挡。

```pseudocode
fn m_lined_up(mtarg, mtmp) -> bool:
    // 伪装中的玩家 24/25 概率不被发现
    if utarget and Upolyd and rn2(25):
        if u.uundetected or disguised: return FALSE

    return linedup(tx, ty, mtmp.mx, mtmp.my, boulder_handling)
```

Boulder 处理：
- 0 = 不考虑
- 1 = 忽略 boulder（`throws_rocks` 或持有 `WAN_STRIKING`）
- 2 = boulder 有概率阻挡（`rn2(2 + boulderspots) < 2`）

### 11.3 投掷决策 (`thrwmu`)

```pseudocode
fn thrwmu(mtmp):
    otmp = select_rwep(mtmp)        // 选择远程武器
    if !otmp: return

    // autoreturn weapon 有特殊射程检查
    if autoreturn_weapon(otmp):
        if dist2 > arw.range or !couldsee: return

    // 前进中的玩家 → 怪物尝试用投射物软化
    // 后退中的玩家 → 追击为主，但太远时投掷
    if !lined_up(mtmp): return
    if URETREATING and !always_toss:
        if rn2(BOLT_LIM - distmin(x, y, mtmp.mux, mtmp.muy)):
            return      // 越近越不愿投掷（想追上来肉搏）

    monshoot(mtmp, otmp, mwep)
```

### 11.4 保持距离 (`m_balks_at_approaching`)

只在以下条件下检查：非和平 + 距离 < 5 格 (`dist2 < 25`) + 能看到玩家

```pseudocode
fn m_balks_at_approaching(appr, mtmp, &distmin, &distmax) -> new_appr:
    // 有弓+箭 → 回避 (appr=-1)
    if m_has_launcher_and_ammo: return -1

    // 持长柄武器且在射程内 → 回避
    if MON_WEP is_pole and dist2 <= MON_POLE_DIST(5): return -1

    // 自动回收投掷武器 → 保持 [2^2, arw.range] 距离
    if autoreturn_weapon(mwep):
        return -2, distmin=4, distmax=arw.range

    // 有 breath/spit/gaze 且 (HP < 1/3 max 或 mspec_used == 0)
    if ranged_attk_available(mtmp):
        if mtmp.mhp < (mtmp.mhpmax + 1) / 3 or !mtmp.mspec_used:
            return -1

    return appr   // 不改变
```

### 11.5 移动后攻击

怪物可以在同一回合移动并发起远程攻击：

```pseudocode
if moved and !nearby:
    if ranged_attk_available or AT_WEAP or find_offensive:
        // 不返回 0，继续到 Phase 4
        break   // falls through to attack phase
```

**[疑似 bug]** 注释说 "Monsters can move and then shoot on same turn; our hero can't. Is that fair?" 这是已知的不对称设计。

### 11.6 seen_resistance 智能

怪物记住玩家展示过的抗性 (`seen_resistance`)：

```pseudocode
// ranged_attk_available 检查
if DISTANCE_ATTK_TYPE(aatyp):
    typ = get_atkdam_type(adtyp)
    if m_seenres(mtmp, cvt_adtyp_to_mseenres(typ)) == 0:
        return TRUE     // 没见过抗性，会尝试此攻击
    // 如果见过抗性，跳过此攻击类型
```

---

## 12. 物品拾取

### 12.1 搜索范围

```
SQSRCHRADIUS = 5  // 切比雪夫距离
// 如果玩家距离 < 5 且敌对: radius -= 1 (变为 4)
// 雇佣兵(mercenary): radius = 1
// 在商店内: 24/25 概率跳过搜索（店主总跳过）
```

### 12.2 拾取意愿 (`mon_would_take_item`)

根据怪物种类和负载决定：

| 条件 | 重量限制 |
|------|----------|
| `!mindless && !is_animal && searches_for_item` | < 75% 最大负载 |
| `likes_gold` (金子) | < 95% |
| `likes_gems` (非矿物宝石) | < 85% |
| `likes_objs` (武器/防具/宝石/食物) | < 75% |
| `likes_magic` (魔法物品) | < 85% |
| `throws_rocks` (巨石，非 Sokoban) | < 50% |
| Gelatinous cube (非石/非球/非石化尸体) | 无限制 |

### 12.3 战斗中不拾取

```pseudocode
if appr == 1 and in_line:  // 正在追击且在直线上
    getitems = FALSE        // 不搜索物品
```

---

## 13. 特殊动作

### 13.1 Leprechaun 藏金

```pseudocode
fn leppie_stash(mtmp):
    if mtmp is leprechaun
       and !DEADMONSTER
       and !m_canseeu(mtmp)     // 看不到玩家
       and not in shop
       and on ROOM tile
       and no trap here
       and rn2(4):              // 3/4 概率
        drop gold
        bury it
```

### 13.2 蜘蛛结网

```pseudocode
fn maybe_spin_web(mtmp):
    if webmaker and !helpless and !mspec_used and !trap_here and soko_allow:
        prob = ((is_giant_spider ? 15 : 5) * (adjacent_walls + 1))
               - (3 * count_traps(WEB))
        if rn2(1000) < prob:
            maketrap(WEB)
            mtmp.mspec_used = d(4,4)  // 4-16 回合冷却
```

### 13.3 使用楼梯

怪物本身不主动使用楼梯（covetous 怪物通过传送到楼梯附近来"堵住"楼梯，而非走楼梯）。跨层追踪通过 `M2_STALK` 标志实现——当玩家换层时，stalker 怪物会跟随迁移。

### 13.4 隐藏

```pseudocode
// hides_under 类型（蛇、蜘蛛、穿刺者等）
if hides_under(ptr) and OBJ_AT(mx,my) and can_hide_under_obj:
    if rn2(10):      // 9/10 概率留在隐藏处不动
        return MMOVE_NOTHING

// 移动后重新隐藏
if hides_under(ptr) or eel:
    if mtmp.mundetected or (!helpless and rn2(5)):
        hideunder(mtmp)    // 4/5 概率重新隐藏
```

### 13.5 碎石

能碎石的怪物：Riders（无冷却）、店主、牧师、quest leader（有冷却 `mspec_used += rn1(20,10)` = 10..29 回合）。

### 13.6 Vrock 毒气

Vrock 开始逃跑时，如果 `mspec_used == 0`，释放毒气云：

```pseudocode
if mdat == PM_VROCK and !mspec_used:
    mspec_used = 75 + rn2(25)  // 75..99 回合冷却
    create_gas_cloud(mx, my, radius=5, dmg=8)
```

---

## 14. 位移与排挤

### 14.1 Monster Displacement

拥有 `M3_DISPLACES` 的怪物可以把其他怪物挤开。`should_displace()` 判断是否值得：

```pseudocode
fn should_displace(mtmp, data, ggx, ggy) -> bool:
    // 对比有位移和无位移的最短路径
    // 只有位移是唯一到达目标的方式，或位移路径更短时，才选择位移
    return shortest_with < shortest_without or no_without_options
```

### 14.2 不理想的位移目标

```pseudocode
fn undesirable_disp(mtmp, x, y) -> bool:
    if is_pet:
        if trap and trap.tseen and rn2(40): return TRUE  // 宠物避开已知陷阱
        if cursed_object_at: return TRUE                  // 宠物避开诅咒物品
    else:
        if trap and rn2(40) and mon_knows_traps(ttyp): return TRUE

    if !accessible(x,y) and !(both_in_water): return TRUE
```

---

## 15. 再生

```pseudocode
fn mon_regen(mon, digest_meal):
    if moves % 20 == 0 or regenerates(mon.data):
        healmon(mon, 1, 0)        // 每 20 回合或每回合（regenerates）恢复 1 HP
    if mon.mspec_used:
        mon.mspec_used--           // 特殊能力冷却递减
```

Covetous 怪物在 `tactics/STRAT_HEAL` 中有额外治疗：远离玩家时每回合 `rnd(8)` = 1..8 HP。

---

## 测试向量

### 基础行为

| # | 输入场景 | 预期输出 |
|---|---------|---------|
| 1 | 普通怪物 speed=12, mspeed=NORMAL, m_moving=true | `mcalcmove` 返回 12 |
| 2 | speed=18, mspeed=NORMAL, m_moving=true | 返回 12 (概率 6/12=50%) 或 24 (概率 6/12=50%) |
| 3 | speed=12, mspeed=MSLOW | `mmove = 4 + 12/3 = 8`（`else` 分支，因为 12 不满足 `< 12`）; m_moving 后返回 0 (概率 8/12) 或 12 (概率 4/12) |
| 4 | speed=12, mspeed=MFAST | `mmove = (48+2)/3 = 16`; 返回 12 (概率 8/12) 或 24 (概率 4/12) |

### 恐惧/逃跑

| # | 输入场景 | 预期输出 |
|---|---------|---------|
| 5 | Wizard of Yendor 站在 Elbereth 上 | `onscary` 返回 FALSE（iswiz 免疫） |
| 6 | 普通 orc 在 Elbereth 旁, nearby=true | `scared=1`, flee_time = rnd(10) (6/7 概率) 或 rnd(100) (1/7 概率) |
| 7 | Minotaur 在 Elbereth 旁 | `onscary` 返回 FALSE（Minotaur 免疫 Elbereth） |
| 8 | 怪物 mflee=1, mfleetim=0, mhp=mhpmax | 每回合 1/25 概率恢复勇气 (`mflee→0`) |

### 边界条件

| # | 输入场景 | 预期输出 |
|---|---------|---------|
| 9 | **边界** covetous 怪物 HP 恰好 = mhpmax/3 (e.g. max=30, hp=10) | `(10*3)/30 = 1` → 非 Wizard 返回 STRAT_HEAL; Wizard fallthrough 到 case 2, dstrat=STRAT_HEAL 但可能被 target 覆盖 |
| 10 | **边界** covetous 怪物 HP = mhpmax/3 + 1 (e.g. max=30, hp=11) | `(11*3)/30 = 1` → 同上 |
| 11 | **边界** covetous 怪物 HP = mhpmax (e.g. max=30, hp=30) | `(30*3)/30 = 3` → dstrat=STRAT_NONE，只有找到目标才移动 |
| 12 | **边界** mfleetim 累加到 127 (e.g. 现有 120, 新增 10) | `min(130, 127) = 127`，被 cap 到 7 bit 上限 |

### 目标感知

| # | 输入场景 | 预期输出 |
|---|---------|---------|
| 13 | 玩家隐身 (Invis), 怪物无 see_invis | `notseen=TRUE`, `displ=1`; 1/3 概率直接定位，2/3 概率在玩家 ±1 范围随机 |
| 14 | 玩家 Displaced, 怪物能看到原位 | `displ=2`, 1/4 概率直接定位，3/4 概率在 ±2 范围随机 |
| 15 | Xorn 面对隐身但带金币的玩家 | `displ=0`，精确定位（闻到金子） |

### 远程攻击

| # | 输入场景 | 预期输出 |
|---|---------|---------|
| 16 | 怪物有 AT_BREA, 见过玩家 fire resist | `m_seenres != 0` → `ranged_attk_available` 对此攻击返回 FALSE |
| 17 | 持弓箭怪物, dist2=16 (4格), 能看到玩家 | `m_balks_at_approaching` 返回 -1（回避靠近） |
| 18 | 持 polearm 怪物, dist2=5 | dist2 <= MON_POLE_DIST(5) → 返回 -1（保持距离） |

---

## 附录：关键常量

| 常量 | 值 | 含义 |
|------|----|------|
| `NORMAL_SPEED` | 12 | 标准移动速度 |
| `BOLT_LIM` | 8 | 远程攻击最大距离 |
| `MON_POLE_DIST` | 5 | 怪物使用长柄武器的最大 dist2 |
| `MTSZ` | 4 | 怪物轨迹缓冲区大小 |
| `UTSZ` | 100 | 英雄轨迹缓冲区大小 |
| `SQSRCHRADIUS` | 5 | 物品搜索切比雪夫半径 |
| `mfleetim` 上限 | 127 | 7-bit Bitfield |
