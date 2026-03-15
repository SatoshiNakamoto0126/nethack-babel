# NetHack 3.7 -- 怪物攻击玩家机制规格

> 提取自 `src/mhitu.c`, `src/uhitm.c`, `src/mcastu.c`,
> `include/monattk.h`, `include/permonst.h`, `include/prop.h`

---

## 1. 攻击矩阵总览

每个怪物种族 (`permonst`) 拥有一个长度为 NATTK = 6 的攻击槽数组 `mattk[6]`。每个攻击槽包含四个字段:

| 字段 | 含义 | 示例值 |
|------|------|--------|
| `aatyp` | 攻击方式 | AT_CLAW, AT_BITE, AT_WEAP, ... |
| `adtyp` | 伤害类型 | AD_PHYS, AD_FIRE, AD_DRLI, ... |
| `damn` | 伤害骰子数 | 1, 2, 3, ... |
| `damd` | 每个骰子面数 | 4, 6, 8, ... |

基础伤害 = `d(damn, damd)`。如果 `damn == 0 && damd == 0`，则基础伤害为 0。

---

## 2. 主流程: `mattacku()`

**入口**: `mattacku(mtmp)` -- 怪物 mtmp 尝试攻击玩家。

### 2.1 预处理

1. 计算距离变量:
   - `ranged = (mdistu(mtmp) > 3)` -- 是否远程（距离平方 > 3）
   - `range2 = !monnear(mtmp, mtmp->mux, mtmp->muy)` -- 怪物认为自己是否远程
   - `foundyou = u_at(mtmp->mux, mtmp->muy)` -- 怪物是否瞄准了正确位置
   - `youseeit = canseemon(mtmp)`

2. 如果不远程, `nomul(0)` 打断玩家动作

3. 如果在水下且怪物不会游泳 => 不攻击

4. 如果玩家被吞噬, 只有吞噬者能攻击

5. 如果骑乘中且怪物与坐骑相邻 (`m_next2u(mtmp)`), 所有怪物有 25% 概率 (`!rn2(4)`) 攻击坐骑代替玩家; 兽人提升至 50% (`!rn2(2)`)

### 2.2 命中判定公式

```
tmp = AC_VALUE(u.uac) + 10 + mtmp->m_lev
    + (multi < 0 ? 4 : 0)                    // 玩家行动不能时 +4
    - (player_invisible_to_monster ? 2 : 0)   // 怪物看不见玩家 -2
    - (mtmp->mtrapped ? 2 : 0)                // 怪物被困 -2
if tmp <= 0: tmp = 1                          // 最小值 1

AC_VALUE(AC) = AC >= 0 ? AC : -rnd(-AC)
// 即: 正AC直接用, 负AC随机取 1..|AC|
```

命中判定: `tmp > rnd(20 + i)` 其中 i 是当前攻击槽索引（0-5）。

- 如果 `tmp == rnd(20+i)` => 近失 (near miss)
- 如果 `tmp > j` => 命中, 调用 `hitmu()`
- 否则 => 未命中 `missmu()`

**注意**: 后续攻击槽难度递增——`rnd(20+i)` 随 i 增大而增大。

### 2.3 武器攻击 (AT_WEAP) 额外命中修正

```
hittmp = hitval(mon_currwep, &youmonst)   // 武器对玩家的命中加值
tmp += hittmp
// 命中判定后减去 hittmp, 避免跨攻击累积
```

### 2.4 拥抱攻击 (AT_HUGS) 自动命中条件

拥抱攻击（通常是第 3 个攻击槽, i >= 2）仅在前两个攻击都命中时才自动命中:

```
if (i >= 2 && sum[i-1] != 0 && sum[i-2] != 0) || mtmp == u.ustuck:
    自动命中, 调用 hitmu()
```

### 2.5 攻击方式循环

对每个攻击槽 i (0..5):

```
for i in 0..NATTK-1:
    if DEADMONSTER(mtmp): return 1   // 反击致死

    mattk = getmattk(mtmp, &youmonst, i, sum, &alt_attk)

    // 跳过条件
    if u.uswallow && mattk.aatyp != AT_ENGL: skip
    if skipnonmagc && mattk.aatyp != AT_MAGC: skip
    if skipdrin && mattk is AT_TENT+AD_DRIN: skip

    switch mattk.aatyp:
        AT_CLAW, AT_KICK, AT_BITE, AT_STNG, AT_TUCH, AT_BUTT, AT_TENT:
            近战命中判定 (见 2.2)
        AT_HUGS:
            自动命中判定 (见 2.4)
        AT_GAZE:
            调用 gazemu() (远程/近程均可)
        AT_EXPL:
            调用 explmu() (近程自动)
        AT_ENGL:
            吞噬命中判定 (近程)
        AT_BREA:
            仅远程: breamu()
        AT_SPIT:
            仅远程: spitmu()
        AT_WEAP:
            远程: thrwmu()
            近程: 命中判定 + hitmu()
        AT_MAGC:
            远程: buzzmu() (直线弹道法术)
            近程: castmu() (施法)

    // 命中后有 1/10 概率唤醒睡眠玩家
    // 如果攻击者死亡 (M_ATTK_AGR_DIED), 返回 1
    // 如果攻击者传送走 (M_ATTK_AGR_DONE), break
```

---

## 3. `getmattk()` -- 攻击替换逻辑

在处理每个攻击槽前, `getmattk()` 可能替换攻击:

### 3.1 SEDUCE=0 系统选项

如果 `!SYSOPT_SEDUCE`:
- 第一个攻击是 AD_SSEX => 全部 6 个攻击替换为 `c_sa_no[]` (非色诱版本)
- 其它攻击是 AD_SSEX => 仅该攻击替换为 AD_DRLI

### 3.2 连续疾病/饥饿攻击限制

如果当前攻击的 adtyp 是 AD_DISE / AD_PEST / AD_FAMN, 且与上一个攻击相同, 且上一个攻击命中:
=> 替换为 AD_STUN

### 3.3 吸取能量按比例调整 (AD_DREN)

```
ulev = max(u.ulevel, 6)
if u.uen <= 5 * ulev && damn > 1:
    damn -= 1
    if u.uenmax <= 2 * ulev && damd > 3:
        damd -= 3
elif u.uen > 12 * ulev:
    damn += 1
    if u.uenmax > 20 * ulev:
        damd += 3
```

### 3.4 mspec_used 冷却期

如果怪物的 `mspec_used > 0` 且攻击是 AT_ENGL / AT_HUGS / AD_STCK / AD_POLY:
- 如果原攻击有元素伤害 (AD_ACID/ELEC/COLD/FIRE): 替换为 AT_TUCH + 原 adtyp, 1d6
- 否则: 替换为 AT_CLAW + AD_PHYS, 1d6
- 如果原 damd == 0 (如地衣): AT_TUCH, 0d0

### 3.5 武器攻击的非物理伤害强制替换

条件: indx == 0, 怪物有 AT_WEAP + 非 AD_PHYS 攻击, 且:
- 怪物被取消 (mcan), 或
- 武器是石化尸体/Stormbringer/Vorpal Blade

=> 替换 adtyp 为 AD_PHYS

### 3.6 巫妖冷触攻击替换

如果目标有冷抗性, 巫妖的 AT_TUCH + AD_COLD 替换为 AD_PHYS, damn 减半:

```
damn = (damn + 1) / 2
if damd == 10: damd = 6    // 巫妖: 1d10 -> 1d6
```

### 3.7 元素位面加倍

如果怪物在其家乡元素位面:
```
damn *= 2
```

---

## 4. `hitmu()` -- 命中后伤害处理

### 4.1 基础伤害计算

```
damage = d(mattk.damn, mattk.damd)
if (is_undead(mdat) || is_vampshifter(mtmp)) && midnight():
    damage += d(mattk.damn, mattk.damd)    // 午夜不死生物双倍伤害
```

### 4.2 伤害类型分派

调用 `mhitm_adtyping()` 处理各伤害类型 (见第 5 节)。

### 4.3 击退 (knockback)

调用 `mhitm_knockback()` 尝试击退。

### 4.4 负 AC 减伤

```
if damage > 0 && u.uac < 0:
    damage -= rnd(-u.uac)       // 随机减去 1..|AC|
    if damage < 1: damage = 1   // 最小伤害 1
```

### 4.5 Half_physical_damage 减伤

```
if Half_physical_damage:
    damage = (damage + 1) / 2
// 也适用于: 牧师角色穿戴圣冠 (Mitre of Holiness) 且敌人怕祝福
```

### 4.6 永久伤害 (Death 的生命力吸取)

当 `permdmg` 标志为真时 (仅 Death 的 AD_DETH):

```
permdmg = rn2(damage / 2 + 1)
if Upolyd || u.uhpmax > 25 * u.ulevel:
    permdmg = damage
elif u.uhpmax > 10 * u.ulevel:
    permdmg += damage / 2
elif u.uhpmax > 5 * u.ulevel:
    permdmg += damage / 4
// else: permdmg 保持 rn2(damage/2+1)

lowerlimit = Upolyd ? min(youmonst.data.mlevel, u.ulevel) : minuhpmax(1)
if hpmax - permdmg > lowerlimit:
    hpmax -= permdmg
elif hpmax > lowerlimit:
    hpmax = lowerlimit
```

### 4.7 被动反击

如果伤害 > 0, 调用 `passiveum()` (见第 8 节)。

---

## 5. 伤害类型详细规格

以下列出每种伤害类型在"怪物打玩家" (mdef == youmonst) 场景下的效果。

### AD_PHYS (0) -- 物理伤害

**无武器攻击** (非 AT_WEAP):
- AT_HUGS + AD_PHYS: 首次 50% 概率抓住玩家 (`set_ustuck`)。已抓住则 "You are being crushed."
  - 先检查 `u_slip_free()`: 润滑衣物/油皮斗篷可避免
  - `sticks(youmonst.data)` 为真则无效
- 其它: 显示攻击消息, 正常物理伤害

**武器攻击** (AT_WEAP):
- 石化尸体: damage = 1, 玩家可能变石头
- `damage += dmgval(weapon, youmonst)` -- 武器伤害加值
- 力量手套 (Gauntlets of Power): `damage += rn1(4, 3)` 即 3..6
- 如果 `damage <= 0: damage = 1`
- 神器武器: 调用 `artifact_hit()`, 可能有额外效果
- 银武器 + 玩家恨银: 额外灼伤消息
- 铁/金属武器 + 玩家是布丁: 可能分裂
- 中毒武器: 如果 `mhitu_dieroll <= 5` (5/20 概率), 调用 `poisoned(buf, A_STR, name, 10, FALSE)`
  - `hpdamchance = 10` 意味着 50% 概率 HP 伤害, 50% 概率 STR 伤害
- `rustm(&youmonst, weapon)` -- 武器可能锈蚀玩家装备

### AD_FIRE (2) -- 火焰伤害

```
hitmsg()
if !mhitm_mgc_atk_negated():   // MC 检查
    "You're on fire!" (或类似)
    if completelyburns(youmonst.data):   // 纸/稻草魔像
        rehumanize()
        return
    elif Fire_resistance:
        damage = 0
    // 高等级怪物可能烧毁物品: if m_lev > rn2(20)
    // burn_away_slime()
else:
    damage = 0
```

### AD_COLD (3) -- 冰霜伤害

```
hitmsg()
if !mhitm_mgc_atk_negated():
    if Cold_resistance:
        damage = 0
    // 可能冻坏药水等
else:
    damage = 0
```

### AD_ELEC (6) -- 闪电伤害

```
hitmsg()
if !mhitm_mgc_atk_negated():
    if Shock_resistance:
        damage = 0
    // 可能破坏戒指/魔杖
else:
    damage = 0
```

### AD_ACID (8) -- 酸液伤害

```
hitmsg()
if !mhitm_mgc_atk_negated():
    if Acid_resistance:
        damage = 0
    // 可能腐蚀护甲
else:
    damage = 0
```

### AD_DRST (7) / AD_DRDX (30) / AD_DRCO (31) -- 毒素 (降 STR/DEX/CON)

```
hitmsg()
ptmp = AD_DRST -> A_STR, AD_DRDX -> A_DEX, AD_DRCO -> A_CON
if !negated && !rn2(8):    // 1/8 概率触发
    poisoned(buf, ptmp, name, 30, FALSE)
    // poisoned() 内部: rn2(30) 概率 HP 伤害, 否则属性伤害
```

`poisoned()` 函数详细流程:
- 如果 Poison_resistance: 仅 "The poison doesn't seem to affect you."
- 否则:
  - 有 `hpdamchance` 参数 (这里是 30) 表示 `rn2(hpdamchance)` 概率造成 HP 伤害
  - HP 伤害 = `rn1(10, 6)` 即 6..15
  - 属性伤害 = `losestr(rnd(damage))` 等

### AD_DRIN (32) -- 吸取智力 (Mind Flayer)

```
hitmsg()
if defends(AD_DRIN, uwep) || !has_head(youmonst.data):
    "You don't seem harmed."
    skipdrin = TRUE   // 跳过后续 DRIN 攻击
    return
if u_slip_free():     // 润滑头盔
    return
if uarmh && rn2(8):   // 头盔 7/8 概率挡住
    "Your helmet blocks the attack to your head."
    return

// 负 AC 不减此伤害!
if Half_physical_damage:
    damage = (damage + 1) / 2
mdamageu(magr, damage)
damage = 0   // 不再第二次扣血

if !uarmh || uarmh.otyp != DUNCE_CAP:
    eat_brains() -- 吸取大脑, 可能致死
    // 如果致死, skipdrin = TRUE

adjattrib(A_INT, -rnd(2), FALSE)    // 降 1-2 点 INT
if !rn2(5): losespells()             // 20% 概率忘记法术
if !rn2(5): drain_weapon_skill(rnd(2))  // 20% 概率降低武器技能
```

### AD_DRLI (15) -- 吸取生命等级

```
hitmsg()
if !rn2(3) && !Drain_resistance && !negated:   // 1/3 概率
    losexp("life drainage")
```

### AD_DREN (16) -- 吸取魔法能量

```
hitmsg()
if !negated && !rn2(4):   // 1/4 概率
    drain_en(damage, FALSE)
    // drain_en 从 u.uen 扣除, 溢出部分扣 u.uenmax
damage = 0   // 不造成 HP 伤害
```

### AD_STUN (12) -- 眩晕

```
hitmsg()
if !mtmp.mcan && !rn2(4):   // 1/4 概率
    make_stunned(HStun_timeout + damage, TRUE)
    damage /= 2   // 眩晕后物理伤害减半
```

### AD_SLOW (13) -- 减速

```
hitmsg()
if !negated && HFast && !rn2(4):   // 1/4 概率 (需要当前有速度)
    u_slow_down()   // 清除内在速度
```

### AD_PLYS (14) -- 麻痹

```
hitmsg()
if multi >= 0 && !rn2(3) && !negated:   // 1/3 概率
    if Free_action:
        "You momentarily stiffen."
    else:
        nomul(-rnd(10))   // 麻痹 1-10 回合
```

### AD_SLEE (4) -- 催眠

```
hitmsg()
if multi >= 0 && !rn2(5) && !negated:   // 1/5 概率
    if Sleep_resistance: return
    fall_asleep(-rnd(10), TRUE)   // 睡眠 1-10 回合
```

### AD_CONF (25) -- 混乱

```
hitmsg()
if !mtmp.mcan && !rn2(4) && !mtmp.mspec_used:
    mtmp.mspec_used += damage + rn2(6)
    make_confused(HConfusion + damage, FALSE)
damage = 0   // 不造成 HP 伤害
```

### AD_BLND (11) -- 致盲

```
if can_blnd(magr, mdef, aatyp, NULL):
    make_blinded(BlindedTimeout + damage, FALSE)
damage = 0
```

### AD_STON (18) -- 石化

```
hitmsg()
if !rn2(3):   // 1/3 概率触发
    if magr.mcan:
        仅消息 ("You hear a cough")
    else:
        // 嘶嘶声消息
        if !rn2(10) || flags.moonphase == NEW_MOON:   // 1/10 或新月
            do_stone_u():
                if !Stoned && !Stone_resistance:
                    if poly_when_stoned: polymon(PM_STONE_GOLEM)
                    else: make_stoned(5, ...)  // 5 回合后石化
```

**石化概率计算**: 1/3 进入外层, 再 1/10 (或新月 100%) 实际石化。
非新月有效概率: 1/3 * 1/10 = 1/30 每次攻击。

### AD_STCK (19) -- 粘附

```
hitmsg()
if !negated && !u.ustuck && !sticks(youmonst.data):
    set_ustuck(magr)   // 粘住玩家
```

不造成额外伤害; 伤害来自 damn/damd 基础骰。

### AD_WRAP (28) -- 缠绕 (鳗鱼等)

```
if (!magr.mcan || u.ustuck == magr) && !sticks(youmonst.data):
    if !u.ustuck && !rn2(10):   // 1/10 概率缠住
        if u_slip_free(): damage = 0
        else: set_ustuck(magr); "X coils/swings itself around you!"
    elif u.ustuck == magr:
        // 已缠住 -- 可能溺水
        if is_pool(magr.mx, magr.my) && !Swimming && !Amphibious && !Breathless:
            "X drowns you..."
            done(DROWNING)
        elif AT_HUGS:
            "You are being crushed."
    else:
        damage = 0; "X brushes against you/your leg."
else:
    damage = 0
```

### AD_SGLD (20) -- 偷金

```
hitmsg()
if youmonst.data.mlet == magr.data.mlet: return   // 同族不偷
if !magr.mcan: stealgold(magr)
```

### AD_SITM (21) / AD_SEDU (22) -- 偷物品 / 色诱偷物

```
// 如果玩家自身是色诱系怪物: 怪物逃跑
// 如果怪物 mcan: "plain <foo> tries to charm you" + 2/3 概率逃跑
// 否则: steal(magr, buf) -- 偷一件物品, 怪物逃跑
```

### AD_SSEX (35) -- 色诱 (延伸)

```
if SYSOPT_SEDUCE:
    if could_seduce() == 1 && !magr.mcan:
        doseduce()   // 完整色诱流程 (见第 6 节)
else:
    退化为 AD_SEDU
```

### AD_TLPT (23) -- 传送

```
hitmsg()
if !negated:
    tele()   // 传送玩家
    // 伤害限制: 不能致死
    if (Half_physical_damage ? (damage-1)/2 : damage) >= current_hp:
        damage = current_hp - 1
        if Half_physical_damage: damage *= 2
        if damage < 1: damage = 1; hp += 1   // 确保存活
```

### AD_RUST (24) -- 锈蚀

```
hitmsg()
if !magr.mcan:
    if completelyrusts(youmonst.data):  // 铁魔像
        rehumanize()
        return
    erode_armor(&youmonst, ERODE_RUST)
```

不造成 HP 伤害(但基础骰仍然适用)。

### AD_CORR (42) -- 腐蚀

```
hitmsg()
if !magr.mcan:
    erode_armor(&youmonst, ERODE_CORRODE)
```

### AD_DCAY (34) -- 腐朽

```
hitmsg()
if !magr.mcan:
    if completelyrots(youmonst.data):  // 木/皮魔像
        rehumanize()
        return
    erode_armor(&youmonst, ERODE_ROT)
```

### AD_ENCH (41) -- 去附魔

```
hitmsg()
if !negated:
    obj = some_armor(&youmonst) 或随机选择 ring/amulet/blindfold
    if obj: drain_item(obj, FALSE)  // 降低附魔值 -1
```

### AD_SLIM (40) -- 变绿史莱姆

```
hitmsg()
if negated: "You escape harm."; return
if flaming(youmonst.data): damage = 0; "The slime burns away!"
elif Unchanging || noncorporeal || youmonst is green slime: damage = 0
elif !Slimed:
    make_slimed(10, ...)   // 10 回合后变史莱姆
```

### AD_POLY (43) -- 变形

```
hitmsg()
if HP > adjusted_damage && !negated:
    mon_poly(magr, &youmonst, damage)   // 随机变形
```

### AD_WERE (29) -- 传染兽化症

```
hitmsg()
if !rn2(4) && u.ulycn == NON_PM && !Protection_from_shape_changers
   && !defends(AD_WERE, uwep) && !negated:
    "You feel feverish."
    set_ulycn(monsndx(magr.data))   // 感染兽化症
```

### AD_HEAL (27) -- 护士治疗

```
if magr.mcan || (Upolyd && touch_petrifies): 普通攻击
elif 玩家不穿甲、不持武器:
    HP += rnd(7)
    1/7 概率 hpmax++ (上限: 5 * ulevel + d(2 * ulevel, 10))
    1/13 概率护士消失
    1/3 概率 exercise(A_STR/A_CON, TRUE)
    治愈疾病
    damage = 0
else:
    普通攻击 (伤害来自基础骰)
```

### AD_DETH (37) -- 死神之触

```
"Death reaches out with its deadly touch."
if is_undead(youmonst.data):
    damage = (damage + 1) / 2
    "Was that the touch of death?"
    return
switch rn2(20):
    case 19/18/17:
        if !Antimagic:
            touch_of_death():
                dmg = 50 + d(8,6)     // 50 + 8..48 = 58..98
                drain = dmg / 2
                Upolyd: u.mh = 0, rehumanize()
                else: 如果 drain >= uhpmax => 直接死亡
                      否则: uhpmax -= drain, losehp(dmg)
            damage = 0; return
        // 有 Antimagic 则 fallthrough
    case 16..5:
        "Your life force draining away..."
        permdmg = 1   // 触发永久HP减少 (见 4.6)
        return
    case 4..0:
        "Lucky for you, it didn't work!"
        damage = 0
```

### AD_PEST (38) -- 瘟疫之触

```
"You feel fever and chills."
diseasemu():
    if Sick_resistance: "slight illness"; return FALSE
    else: make_sick(duration, name, TRUE, SICK_NONVOMITABLE)
          duration = Sick ? Sick/3+1 : rn1(ACURR(A_CON), 20) 即 20..20+CON-1
// 加上正常物理伤害
```

### AD_FAMN (39) -- 饥荒之触

```
"Your body shrivels."
exercise(A_CON, FALSE)
if !is_fainted(): morehungry(rn1(40, 40))   // 增加 40..79 饥饿
// 加上正常物理伤害
```

### AD_SAMU (252) -- 窃取护符

```
hitmsg()
if !rn2(20):   // 1/20 概率
    stealamulet(magr)   // 偷取任务物品/护符/祈祷道具
```

### AD_CURS (253) -- 随机诅咒

```
hitmsg()
if magr is Gremlin && !night(): return   // 小鬼白天无效
if !magr.mcan && !rn2(10):   // 1/10 概率
    if youmonst is clay golem:
        "Some writing vanishes from your head!"
        rehumanize()
    else:
        mon_give_prop(magr, attrcurse())
        // attrcurse(): 随机诅咒一种属性/能力
```

### AD_DISE (33) -- 疾病

```
hitmsg()
if !diseasemu(magr.data):
    damage = 0
// diseasemu: 同 AD_PEST 的 diseasemu() 流程
```

### AD_LEGS (17) -- 腿部攻击 (蠼螋)

```
side = 随机左/右
if (usteed || Levitation || Flying) && !is_flyer(magr):
    "X tries to reach your leg!"; damage = 0
elif magr.mcan:
    "X nuzzles against your leg!"; damage = 0
else:
    靴子减伤逻辑:
    - 低靴/铁靴: 50% "pricks exposed part"
    - 其它靴子: 1/5 "pricks through"
    - 其余: "scratches boot"; damage = 0; return
    - 无靴: "pricks your leg"

    set_wounded_legs(side, rnd(60 - ACURR(A_DEX)))
```

### AD_DGST (26) -- 消化

通过 `gulpmu()` 处理, 在 `mhitm_adtyping` 中 damage = 0。
(实际消化逻辑在吞噬机制中, 见第 7 节)

### AD_HALU (36) -- 致幻

对玩家: damage = 0 (仅用于 AT_EXPL 爆炸型致幻)

### AD_RBRE (242) -- 随机吐息

在 breamu() 中处理, 随机选择火/冰/电/酸/毒气吐息。

### AD_CLRC (240) / AD_SPEL (241) -- 牧师/法师法术

在 castmu() 中处理 (见第 9 节)。

---

## 6. 色诱机制: `doseduce()`

条件: `could_seduce() == 1` (异性) && `SYSOPT_SEDUCE` && 怪物未取消

流程:
1. 尝试脱掉玩家所有防具 (通过 `mayberem()`)
   - 高 CHA 时给确认提示, 低 CHA 时强制脱
   - 脱掉装备可能导致传送 (失去悬浮) => 中断
2. 如果仍有甲/斗篷: 怪物遗憾地离开
3. 否则进入"time stands still..."
4. 结果判定:

```
attr_tot = ACURR(A_CHA) + ACURR(A_INT)
if rn2(35) > min(attr_tot, 32):   // 失败
    switch rn2(5):
        0: u.uen = 0; u.uenmax -= rnd(Half_phys ? 5 : 10)
        1: adjattrib(A_CON, -1)
        2: adjattrib(A_WIS, -1)
        3: if !Drain_resistance: losexp("overexertion")
        4: losehp(Maybe_Half_Phys(rn1(10, 6)))
else:   // 成功
    mtmp.mspec_used = rnd(100)
    switch rn2(5):
        0: u.uenmax += rnd(5); u.uen = u.uenmax
        1: adjattrib(A_CON, +1)
        2: adjattrib(A_WIS, +1)
        3: pluslvl()   // 升一级
        4: u.uhp = u.uhpmax; u.mh = u.mhmax   // 满血
```

5. 金钱: 如果 CHA < rn2(20) 且非驯服怪物, 收费 `rnd(money+10) + 500` (和平怪物 /5)
6. 1/25 概率怪物被取消

---

## 7. 吞噬机制: `gulpmu()`

### 7.1 初始吞噬

条件: 怪物未已吞噬 + engulf_target 检查通过 + 非坑中有巨石

```
// 强制下马
set_ustuck(magr)
u.uswallow = 1

// 吞噬持续时间
if AD_DGST:
    tim_tmp = ACURR(A_CON) + 10 - u.uac + rn2(20)
    if tim_tmp < 0: tim_tmp = 0    // 防止极高 AC 导致负值
    tim_tmp = tim_tmp / mtmp.m_lev + 3
    // 高 CON + 低 AC (好甲) = 更长消化时间 = 更多时间逃脱
elif other:
    tim_tmp = rnd(mtmp.m_lev + 10 / 2)
    // [疑似 bug: 运算优先级问题, 10/2=5, 实际是 rnd(m_lev + 5)]

u.uswldtim = max(tim_tmp, 2)
```

**运算优先级注意**: `tim_tmp = rnd((int) mtmp->m_lev + 10 / 2)` 中 `10 / 2` 因 C 运算优先级先算 = 5, 所以实际是 `rnd(m_lev + 5)`, 而非 `rnd((m_lev + 10) / 2)`. 注释说 "higher level attacker takes longer to eject hero", 与实际行为一致; 这可能是有意写法而非 bug, 但写法容易引起混淆。

### 7.2 吞噬中的每回合伤害

```
tmp = d(mattk.damn, mattk.damd)
u.uswldtim -= 1

switch mattk.adtyp:
    AD_DGST:
        if Slow_digestion:
            u.uswldtim = 0; tmp = 0
        elif u.uswldtim == 0:
            "X totally digests you!"
            tmp = u.uhp   // 致死伤害
            if Half_physical_damage: tmp *= 2   // "sorry" -- 确保致死
        else:
            "X digests you!" + exercise(A_STR)

    AD_PHYS:
        Fog cloud: if Amphibious/Breathless && !flaming: tmp = 0
        Other: "You are pummeled with debris!"

    AD_ACID:
        if Acid_resistance: tmp = 0

    AD_BLND:
        if can_blnd(): make_blinded(tmp); tmp = 0

    AD_ELEC/AD_COLD/AD_FIRE:
        if !mtmp.mcan && rn2(2):   // 50% 概率触发
            检查对应抗性; 有抗性 tmp = 0
        else: tmp = 0

    AD_DISE:
        if !diseasemu(): tmp = 0

    AD_DREN:
        if !mtmp.mcan && rn2(4):   // 75% 概率
            drain_en(tmp, FALSE)
        tmp = 0

// 物理伤害类型: AC 减伤 + Half_physical_damage
if physical_damage:
    if u.uac < 0: tmp -= rnd(-u.uac); if tmp < 0: tmp = 1
    tmp = Maybe_Half_Phys(tmp)
```

### 7.3 排出条件

- 石化体质: 怪物急忙吐出
- `u.uswldtim == 0`: 被吐出 (Slow_digestion 特例)
- `youmonst.data.msize >= MZ_HUGE`: 变成巨型直接被吐出

---

## 8. 被动攻击 (Passive Defense): `passiveum()`

当玩家受到非零伤害攻击后, 检查玩家当前形态的被动攻击 (AT_NONE 或 AT_BOOM)。

查找攻击: 扫描玩家当前形态的 mattk[], 找到第一个 `aatyp == AT_NONE` 或 `aatyp == AT_BOOM`。

伤害计算:
```
if oldu_mattk.damn > 0:
    tmp = d(damn, damd)
elif oldu_mattk.damd > 0:
    tmp = d(mlevel + 1, damd)
else:
    tmp = 0
```

### 被动攻击效果 (不需要仍为变形状态):

**AD_ACID**: 50% 概率溅射酸液伤害。1/30 概率腐蚀怪物护甲, 1/6 概率腐蚀怪物武器。

**AD_STON** (鸡蛇): 检查怪物护甲保护:
- `protector = attk_protection(mattk.aatyp)` -- 攻击方式对应的护甲部位
- 如果怪物缺少对应护甲 => 石化
- 武器等同手套保护

**AD_ENCH** (去附魔者): 如果怪物持有武器 => `drain_item(weapon, TRUE)` 去附魔

### 被动攻击效果 (需要仍为变形状态, 2/3 概率触发):

**AD_PHYS + AT_BOOM**: "You explode!" => rehumanize() => assess_dmg()

**AD_PLYS** (浮眼):
- 如果是浮眼: 1/4 概率 tmp = 127
- 检查怪物能否看见 + 有眼 + 2/3 概率 + 能看见你
- 如果全部满足: "X is frozen by your gaze!" 麻痹 tmp 回合
- (怪物可能用魔法反射护盾反弹)

**AD_COLD** (棕霉/蓝果冻): 冻伤怪物, 玩家回复 HP = (tmp + rn2(2)) / 2。如果 mhmax 超过 mlevel*8+8, 可能分裂。

**AD_STUN** (黄霉): 怪物被眩晕, tmp = 0。

**AD_FIRE** (红霉): 灼伤怪物。

**AD_ELEC**: 电击怪物。

---

## 9. 怪物法术攻击

### 9.1 施法框架: `castmu()`

```
// 法术选择
spellnum = rn2(ml)   // ml = monster level
if AD_SPEL: spellnum = choose_magic_spell(spellnum)
if AD_CLRC: spellnum = choose_clerical_spell(spellnum)

// 施法失败检查
if mtmp.mcan || mtmp.mspec_used || !ml: cursetxt(); return MISS

// 冷却设置
mtmp.mspec_used = (m_lev < 8) ? (10 - m_lev) : 2

// 施法失败率 (fumble)
if rn2(ml * 10) < (mtmp.mconf ? 100 : 20):
    "The air crackles..."; return MISS

// 伤害计算
if foundyou:
    if mattk.damd > 0:
        dmg = d(ml/2 + mattk.damn, mattk.damd)
    else:
        dmg = d(ml/2 + 1, 6)
    if Half_spell_damage: dmg = (dmg + 1) / 2
```

### 9.2 法师法术列表 (AD_SPEL)

| 法术值 | 法术 | 效果 |
|--------|------|------|
| 24-20 | MGC_DEATH_TOUCH | 如果非不死/非恶魔 && rn2(m_lev) > 12 && !Antimagic: touch_of_death() (50 + 8d6 伤害 + 等量 HP max 减少) |
| 19-18 | MGC_CLONE_WIZ | 仅 Wizard of Yendor: 复制自身 |
| 17-15 | MGC_SUMMON_MONS | nasty(): 召唤危险怪物 |
| 14-13 | MGC_AGGRAVATION | aggravate(): 唤醒所有怪物 |
| 12-10 | MGC_CURSE_ITEMS | rndcurse(): 随机诅咒一件物品 |
| 9-8 | MGC_DESTRY_ARMR | Antimagic 挡住, 否则 destroy_arm() 摧毁一件护甲 |
| 7-6 | MGC_WEAKEN_YOU | Antimagic 挡住, 否则 losestr(rnd(m_lev - 6)) |
| 5-4 | MGC_DISAPPEAR | 怪物隐身 |
| 3 | MGC_STUN_YOU | Antimagic/Free_action: 1 回合; 否则 d(DEX<12 ? 6 : 4, 4) 回合 |
| 2 | MGC_HASTE_SELF | 怪物加速 |
| 1 | MGC_CURE_SELF | 怪物回复 3d6 HP |
| 0 | MGC_PSI_BOLT | Antimagic: dmg/2; 否则直接伤害 |

选择逻辑: `spellval = rn2(ml)`, 如果 > 24 则 `while (spellval > 24 && rn2(25)) spellval = rn2(spellval)` 循环缩小。

### 9.3 牧师法术列表 (AD_CLRC)

| 法术值 | 法术 | 效果 |
|--------|------|------|
| 15-14 | 2/3 CLC_OPEN_WOUNDS, 1/3 CLC_GEYSER | (见下) |
| 13 | CLC_GEYSER | d(8,6) 物理伤害, Half_phys 减半 |
| 12 | CLC_FIRE_PILLAR | d(8,6) 火伤, Fire_res 免疫, Half_spell 减半; 烧甲+烧物品 |
| 11 | CLC_LIGHTNING | d(8,6) 电伤, Reflecting/Shock_res 免疫; 致盲 rnd(100) 回合 |
| 10-9 | CLC_CURSE_ITEMS | 同法师版 |
| 8 | CLC_INSECTS | 召唤 max(3, rnd(m_lev/2)) 只虫子/蛇 |
| 7-6 | CLC_BLIND_YOU | 致盲 Half_spell ? 100 : 200 回合 |
| 5-4 | CLC_PARALYZE | Antimagic/Free_action: 1 回合; 否则 4+m_lev 回合, Half_spell 减半 |
| 3-2 | CLC_CONFUSE_YOU | Antimagic 挡住; 否则 make_confused(m_lev), Half_spell 减半 |
| 1 | CLC_CURE_SELF | 怪物回复 3d6 HP |
| 0 | CLC_OPEN_WOUNDS | Antimagic: dmg/2; 否则直接伤害 |

### 9.4 远程法术: `buzzmu()`

条件: `BZ_VALID_ADTYP(mattk.adtyp)` (即 AD_MAGM/FIRE/COLD/SLEE/DISN/ELEC/DRST/ACID)

```
if !mtmp.mcan && lined_up(mtmp) && rn2(3):   // 2/3 概率
    buzz(spell_type, mattk.damn, mx, my, dx, dy)
    // 直线弹道, 碰到玩家时检查抗性
```

---

## 10. 防御检查: 魔法取消 (MC)

`mhitm_mgc_atk_negated(magr, mdef, verbosely)`:

```
if magr.mcan: return TRUE   // 怪物被取消
armpro = magic_negation(mdef)   // 0..3
negated = !(rn2(10) >= 3 * armpro)
// 即: armpro=0 -> 0% 抵消
//     armpro=1 -> 30% 抵消
//     armpro=2 -> 60% 抵消
//     armpro=3 -> 90% 抵消
```

`magic_negation(mon)` 计算:
```
mc = max(所有穿戴护甲的 a_can 值)

if 有外在 Protection:
    mc += (护符是 Amulet_of_Guarding ? 2 : 1)
    if mc > 3: mc = 3
elif mc < 1:
    if 有内在 Protection (祈祷/法术): mc = 1

return mc   // 范围 0..3
```

### MC 适用的攻击类型

MC 检查 (`mhitm_mgc_atk_negated`) 适用于:
AD_FIRE, AD_COLD, AD_ELEC, AD_ACID, AD_DRLI, AD_DREN, AD_DRST/DRDX/DRCO,
AD_PLYS, AD_SLEE, AD_SLIM, AD_SLOW, AD_TLPT, AD_POLY, AD_STCK, AD_WERE,
AD_ENCH (直接检查 negated, 不通过此函数)

MC **不**检查的:
AD_PHYS, AD_STUN (用 `mcan` 直接检查), AD_CONF (用 `mcan` + `mspec_used`),
AD_STON (用 `mcan`), AD_WRAP (用 `mcan`), AD_BLND (用 `can_blnd`),
AD_SGLD, AD_SEDU, AD_DRIN, AD_CURS, AD_SAMU, AD_FAMN, AD_PEST, AD_DETH

---

## 11. 凝视攻击: `gazemu()`

凝视不使用标准命中判定, 条件各异:

### 通用取消条件

```
if m_seenres(mtmp, ...): return MISS   // 怪物已知玩家有抗性
is_medusa = (mtmp.data == PM_MEDUSA)
// 检查: Hallucination 75% 概率使凝视无效
// Unaware (睡眠/昏迷) 使凝视无效 (除非可反射)
```

### AD_STON 石化凝视 (Medusa)

```
if cancelled || !mtmp.mcansee: 无效消息
if Reflecting && couldsee && is_medusa:
    反射回去 => 怪物石化
elif canseemon && couldsee && !Stone_resistance && !Unaware:
    "You meet X's gaze."
    poly_when_stoned => PM_STONE_GOLEM
    否则: done(STONING)
```

### AD_CONF 混乱凝视

```
if mcanseeu && !mspec_used && rn2(5):   // 4/5 概率尝试
    if !cancelled:
        conf_duration = d(3, 4)
        mspec_used += conf_duration + rn2(6)
        make_confused(HConfusion + conf_duration)
```

### AD_STUN 眩晕凝视

```
if mcanseeu && !mspec_used && rn2(5):
    if !cancelled:
        stun_duration = d(2, 6)
        mspec_used += stun_duration + rn2(6)
        make_stunned(HStun + stun_duration)
```

### AD_BLND 致盲凝视 (Archon)

```
if canseemon && !resists_blnd && distance <= BOLT_LIM^2:
    if !cancelled:
        blnd = d(damn, damd)
        make_blinded(blnd); 如果仍能看见则 make_stunned(max(old, rnd(3)))
```

### AD_FIRE 火焰凝视

```
if mcanseeu && !mspec_used && rn2(5):
    if !cancelled:
        dmg = d(2, 6)
        if Fire_resistance: dmg = 0; ugolemeffects(AD_FIRE, d(12, 6))
        if m_lev > rn2(20): burnarmor()
        if m_lev > rn2(20): destroy_items(AD_FIRE, orig_dmg)
```

### AD_SLEE 催眠凝视 (仅 Beholder, 未完成)

```
if mcanseeu && multi >= 0 && !rn2(5) && !Sleep_resistance:
    fall_asleep(-rnd(10), TRUE)
```

### AD_SLOW 减速凝视 (仅 Beholder, 未完成)

```
if mcanseeu && HFast && !defended && !rn2(4):
    u_slow_down()
```

---

## 12. 爆炸攻击: `explmu()`

```
if mtmp.mcan: return MISS   // 取消怪物不爆炸
tmp = d(damn, damd)
not_affected = defended(mtmp, adtyp)

switch adtyp:
    AD_COLD/AD_FIRE/AD_ELEC:
        mon_explodes(mtmp, mattk)   // 使用 explode() 产生区域效果
        怪物死亡 (除非生命救济)

    AD_BLND:
        if !resists_blnd:
            if mon_visible || rnd(tmp/2) > u.ulevel:
                make_blinded(tmp)

    AD_HALU:
        if !Blind && 能看见:
            make_hallucinated(HHallucination + tmp)
        怪物先死 (mondead), 再致幻

怪物爆炸后: wake_nearto(7*7 范围)
```

---

## 13. 润滑/油脂保护: `u_slip_free()`

适用于: AT_HUGS (拥抱), AD_WRAP (缠绕), AD_DRIN (脑汁吸取)

```
// AT_ENGL + AD_WRAP 不受润滑保护
obj = uarmc ? uarmc : uarm ? uarm : uarmu
if AD_DRIN: obj = uarmh   // 吸脑检查头盔

if obj && (obj.greased || obj.otyp == OILSKIN_CLOAK):
    if !obj.cursed || rn2(3):   // 诅咒物品 1/3 概率失败
        "X slips off your greased/slippery cloak!"
        if obj.greased && !rn2(2): 油脂磨损
        return TRUE
return FALSE
```

---

## 14. 怪物召唤

### 恶魔召唤 (非 Balrog, 非好色恶魔)

```
if !rn2(Inhell ? 10 : 16):   // 地狱 1/10, 其它 1/16
    msummon(mtmp)
```

### 兽人召唤

```
if is_human(mdat):   // 人形
    if !Protection_from_shape_changers && !rn2(5 - night()*2):
        new_were(mtmp)   // 变兽形
else:   // 兽形
    if Protection_from_shape_changers || !rn2(30):
        new_were(mtmp)   // 变回人形
if !rn2(10):
    were_summon()   // 召唤同类动物
```

---

## 15. 关键随机函数参考

```
rn2(x)   = 均匀随机 [0, x-1]
rnd(x)   = 均匀随机 [1, x]
d(n, x)  = n 个 rnd(x) 的和 [n, n*x]
rn1(x,y) = rn2(x) + y = [y, x+y-1]
```

---

## 16. 测试向量

以下给出精确的输入/输出对, 用于验证实现正确性。

### TV-1: 基础命中判定

```
输入: u.uac = 0, mtmp.m_lev = 5, multi >= 0, 怪物可见, 怪物未被困
      攻击槽 i = 0
计算: tmp = AC_VALUE(0) + 10 + 5 = 15
判定: 需要 tmp > rnd(20), 即 15 > rnd(20)
结果: rnd(20) 返回 1..14 时命中 (14/20 = 70%), 15 时近失, 16..20 未命中
```

### TV-2: 负 AC 命中判定

```
输入: u.uac = -10, mtmp.m_lev = 10, multi < 0, 怪物可见, 怪物未被困
计算: AC_VALUE(-10) = -rnd(10) 即 [-10, -1]
      tmp = [-10..-1] + 10 + 10 + 4 = [14..23]
      tmp <= 0 不会发生 (最小 14)
判定: 需要 tmp > rnd(20), 即 14..23 > rnd(20)
结果: 最差情况 14 > rnd(20), 仍有 13/20 命中率
```

### TV-3: 火焰伤害 + 抗性

```
输入: 怪物 AD_FIRE 攻击, damn=2, damd=6, 玩家有 Fire_resistance
      MC negation 未触发
结果: hitmsg(), "You're on fire!", "The fire doesn't feel hot!", damage = 0
```

### TV-4: 午夜不死双倍伤害

```
输入: 怪物是吸血鬼, damn=1, damd=6, midnight() 为真
      d(1,6) 第一次 = 4, d(1,6) 第二次 = 3
结果: damage = 4 + 3 = 7
```

### TV-5: 负 AC 减伤 (边界: damage 降到 1)

```
输入: 命中后 damage = 3, u.uac = -20
计算: damage -= rnd(20); rnd(20) 返回 15 => damage = 3 - 15 = -12
      damage < 1 => damage = 1
结果: 最终伤害 = 1
```

### TV-6: MC 3 护甲的否定概率

```
输入: armpro = 3
计算: negated = !(rn2(10) >= 9)
      rn2(10) 返回 0..8 时 negated = TRUE (9/10 = 90%)
      rn2(10) 返回 9 时 negated = FALSE (1/10 = 10%)
结果: 90% 概率否定魔法攻击
```

### TV-7: Death 的永久 HP 减少

```
输入: Death 攻击, damage = 20, u.uhpmax = 60, u.ulevel = 5
      rn2(20) 返回 17 (case 17, !Antimagic)
结果: touch_of_death() 执行, dmg = 50 + d(8,6)
      假设 d(8,6) = 30 => dmg = 80, drain = 40
      u.uhpmax = 60, drain(40) < 60 => uhpmax = max(60-40, minuhpmax(3)) = 20
      losehp(80)
```

### TV-8: Death 生命力吸取 (permdmg 路径)

```
输入: Death 攻击, damage = 20, u.uhpmax = 30, u.ulevel = 5
      rn2(20) 返回 10 (case 16..5 => permdmg 路径)
计算: permdmg = rn2(20/2+1) = rn2(11)
      u.uhpmax = 30 > 25*5=125? 否. > 10*5=50? 否. > 5*5=25? 是.
      permdmg += damage/4 = 5
      假设 rn2(11) = 7 => permdmg = 12
      lowerlimit = minuhpmax(1) (通常 = u.ulevel)
      hpmax = max(30 - 12, 5) = 18
然后: Half_physical_damage 检查, AC 减伤, mdamageu(damage)
```

### TV-9: Mind Flayer 吸脑 -- 头盔挡住 (边界)

```
输入: Mind Flayer AT_TENT+AD_DRIN, 玩家戴头盔, rn2(8) 返回 0
计算: rn2(8) = 0, 条件 rn2(8) 不满足 (需要非0), 所以不挡住
结果: 吸脑生效

输入: 同上, rn2(8) 返回 7
计算: rn2(8) = 7 != 0, 满足, 头盔挡住
结果: "Your helmet blocks the attack to your head."
概率: 7/8 挡住, 1/8 通过
```

### TV-10: 石化攻击概率 (非新月)

```
输入: Cockatrice 攻击, flags.moonphase != NEW_MOON
外层: !rn2(3) = 1/3 进入
内层: !rn2(10) = 1/10 实际石化
总概率: 1/3 * 1/10 = 1/30 每次攻击
结果: 30 次攻击平均触发一次石化流程 (还需通过 Stone_resistance 等检查)
```

### TV-11: 石化攻击概率 (新月, 边界)

```
输入: Cockatrice 攻击, flags.moonphase == NEW_MOON
外层: !rn2(3) = 1/3 进入
内层: (!rn2(10) || moonphase==NEW_MOON) = TRUE (新月必定通过)
总概率: 1/3 每次攻击
结果: 新月时石化概率大幅提升到 1/3
```

### TV-12: 吞噬消化时间 (AD_DGST, 边界)

```
输入: ACURR(A_CON) = 18, u.uac = -5, rn2(20) = 10, mtmp.m_lev = 15
计算: tim_tmp = (18 + 10 - (-5) + 10) / 15 + 3
      = (18 + 10 + 5 + 10) / 15 + 3
      = 43 / 15 + 3
      = 2 + 3 = 5
      u.uswldtim = max(5, 2) = 5
结果: 5 回合后被完全消化 (除非逃脱)
```

---

## 17. 已知特殊行为和注意事项

1. **攻击槽递增难度**: `rnd(20+i)` 使得第 6 个攻击槽命中阈值为 rnd(25), 比第 1 个 rnd(20) 显著更难命中。这是有意设计, 防止多攻击怪物过于强力。

2. **skipnonmagc 机制**: 如果怪物攻击了错误位置 (wildmiss), 后续所有非法术攻击都跳过, 减少冗余的 miss 消息。

3. **skipdrin 机制**: Mind Flayer 的多次 AD_DRIN 攻击, 如果首次吸脑导致目标没有头/死亡/忘记法术, 后续 DRIN 攻击全部跳过。

4. **mhitu_dieroll**: 武器攻击的命中骰 (`rnd(20+i)`) 会被保存到 `gm.mhitu_dieroll`, 供 `artifact_hit()` 使用 (判断是否触发神器特效)。

5. **运算优先级注意**: `gulpmu()` 中非消化吞噬的持续时间计算 `rnd((int) mtmp->m_lev + 10 / 2)` 因 C 运算优先级实际是 `rnd(m_lev + 5)` 而非 `rnd((m_lev + 10) / 2)`。行为与注释一致, 可能是有意写法但容易引起混淆。

6. **吞噬时 Half_physical_damage 特例**: 当 `u.uswldtim == 0` 且 AD_DGST 消化完成时, `tmp = u.uhp`, 然后 `if Half_physical_damage: tmp *= 2`。这确保即使有半物理伤害, 消化仍然致死。

7. **护士治疗条件**: 玩家必须完全不穿甲 (无 uarm, uarmc, uarmu, uarms, uarmg, uarmf, uarmh) 且不持武器。满足条件时护士治疗而非伤害。

8. **色诱 CHA+INT 封顶**: `min(attr_tot, 32)` 确保 CHA + INT 超过 32 时不再提高成功率。最高成功率 = 1 - 2/35 ≈ 94.3%。
