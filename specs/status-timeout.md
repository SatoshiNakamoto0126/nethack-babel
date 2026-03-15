# Status & Timeout 机制规格

> 源文件: `include/prop.h`, `include/youprop.h`, `include/you.h`, `src/timeout.c`, `src/potion.c`, `src/attrib.c`, `src/mkobj.c`, `src/dig.c`, `src/eat.c`

---

## 1. Property 系统概览

NetHack 的"属性"(property)系统是一个统一的框架,管理英雄身上的 68 种效果。每种属性由 `struct prop` 的三个字段组成:

```
struct prop {
    long extrinsic;   // 来自装备的位掩码
    long blocked;     // 被装备阻止的位掩码
    long intrinsic;   // 内在属性 = 超时计时器 | 永久标志
};
```

英雄拥有 `u.uprops[LAST_PROP + 1]` 数组 (69 个槽,索引 0 未用)。

### 1.1 来源分类

| 来源类型 | 存储位置 | 含义 |
|---------|---------|------|
| **Extrinsic** | `prop.extrinsic` | 来自穿戴/持有物品的位掩码 (`W_ARM`, `W_RINGL`, `W_ART` 等) |
| **Blocked** | `prop.blocked` | 同样的位掩码格式,表示某装备阻止了该属性 |
| **Timed** | `prop.intrinsic & TIMEOUT` | 低 24 位 (`0x00FFFFFF`),剩余回合数,最大 16,777,215 |
| **FROMEXPER** | `prop.intrinsic & 0x01000000` | 来自角色职业的经验等级 |
| **FROMRACE** | `prop.intrinsic & 0x02000000` | 来自种族的经验等级 |
| **FROMOUTSIDE** | `prop.intrinsic & 0x04000000` | 来自尸体、祈祷、王座等外部来源 |
| **FROMFORM** | `prop.intrinsic & 0x10000000` | 来自变身后的怪物形态 |
| **I_SPECIAL** | `prop.intrinsic & 0x20000000` | 属性可控 (如祝福的悬浮药水允许 `>` 提前结束) |

`INTRINSIC` 宏 = `FROMOUTSIDE | FROMRACE | FROMEXPER` (永久性来源的合集)。

### 1.2 Extrinsic 位掩码定义

```
W_ARM   = 0x00000001  // 身体铠甲
W_ARMC  = 0x00000002  // 斗篷
W_ARMH  = 0x00000004  // 头盔
W_ARMS  = 0x00000008  // 盾牌
W_ARMG  = 0x00000010  // 手套
W_ARMF  = 0x00000020  // 靴子
W_ARMU  = 0x00000040  // 衬衫
W_WEP   = 0x00000100  // 主手武器
W_QUIVER= 0x00000200  // 弹药
W_SWAPWEP=0x00000400  // 副手武器
W_ART   = 0x00001000  // 携带神器 (非穿戴)
W_ARTI  = 0x00002000  // 激活神器
W_AMUL  = 0x00010000  // 护身符
W_RINGL = 0x00020000  // 左手戒指
W_RINGR = 0x00040000  // 右手戒指
W_TOOL  = 0x00080000  // 眼镜/蒙眼布
W_SADDLE= 0x00100000  // 鞍
W_BALL  = 0x00200000  // 铁球
W_CHAIN = 0x00400000  // 铁链
```

### 1.3 有效性判定宏模式

每种属性有对应的 `Xxx` 宏,基本模式为:

```
#define Xxx  (HXxx || EXxx)             // 普通属性
#define Xxx  ((HXxx || EXxx) && !BXxx)  // 有阻止机制的属性
```

特殊情况:
- `Very_fast` = `(HFast & ~INTRINSIC) || EFast` — 只有 timed 或 extrinsic 才算"非常快"
- `Blind` = `(HBlinded || EBlinded) && !BBlinded` — Eyes of the Overworld 可阻止
- `Hallucination` = `HHallucination && !Halluc_resistance`
- `Sick`, `Stoned`, `Strangled`, `Vomiting`, `Glib`, `Slimed` — 仅有 intrinsic (纯超时, 无 extrinsic)

### 1.4 超时操作原语

```pseudocode
fn itimeout(val: long) -> long:
    if val >= 0x00FFFFFF: return 0x00FFFFFF
    if val < 1: return 0
    return val

fn set_itimeout(which: &mut long, val: long):
    *which &= ~TIMEOUT          // 清除低 24 位
    *which |= itimeout(val)     // 设置新值 (钳位后)

fn incr_itimeout(which: &mut long, incr: int):
    set_itimeout(which, (*which & TIMEOUT) + incr)
```

`set_itimeout` 是**替换**语义,`incr_itimeout` 是**累加**语义。两者都自动钳位到 `[0, 0x00FFFFFF]`。

---

## 2. 全部 68 种属性

### 2.1 属性总表

| # | 枚举名 | 类别 | 正常游戏可 timed? | 正常 timed 来源 | 超时到期行为 |
|---|--------|------|-------------------|----------------|-------------|
| 1 | `FIRE_RES` | 抗性 | 仅 explore/wizard | 熔岩救命后临时 | "temporary ability to survive burning has ended" |
| 2 | `COLD_RES` | 抗性 | 否 | — | 静默移除 |
| 3 | `SLEEP_RES` | 抗性 | 否 | — | 静默移除 |
| 4 | `DISINT_RES` | 抗性 | 否 | — | 静默移除 |
| 5 | `SHOCK_RES` | 抗性 | 否 | — | 静默移除 |
| 6 | `POISON_RES` | 抗性 | 否 | — | 静默移除 |
| 7 | `ACID_RES` | 抗性 | 是 | 吃酸性尸体 | 消息; 吃酸性尸体途中自动续延 1 回合 |
| 8 | `STONE_RES` | 抗性 | 是 | 吃石化尸体 | 消息; 吃途中续延; 检查手持石化尸体 |
| 9 | `DRAIN_RES` | 抗性 | 否 | — | 静默移除 |
| 10 | `SICK_RES` | 抗性 | 否 | — | 静默移除 |
| 11 | `INVULNERABLE` | 特殊 | 是 | 祈祷 | 静默移除; 祈祷期间跳过致命超时处理 |
| 12 | `ANTIMAGIC` | 抗性 | 否 | — | 静默移除 |
| 13 | `STUNNED` | 状态异常 | 是 | 多种 | `make_stunned(0, TRUE)` "feel a bit steadier" |
| 14 | `CONFUSION` | 状态异常 | 是 | 多种 | `make_confused(0, TRUE)` "feel less confused" |
| 15 | `BLINDED` | 状态异常 | 是 | 多种 | `make_blinded(0, TRUE)` 恢复视觉 |
| 16 | `DEAF` | 状态异常 | 是 | 雷暴等 | `make_deaf(0, TRUE)` "can hear again" |
| 17 | `SICK` | **致命** | 是 | 怪物攻击/腐烂食物 | 死亡或 CON 检定恢复(仅食物中毒) |
| 18 | `STONED` | **致命** | 是 | 接触石化 | 5 回合后石化死亡 |
| 19 | `STRANGLED` | **致命** | 是 | 勒颈项链 | 5 回合窒息死亡 |
| 20 | `VOMITING` | 状态异常 | 是 | 多种 | 约 15 回合呕吐序列 |
| 21 | `GLIB` | 状态异常 | 是 | 油/血 | `make_glib(0)` |
| 22 | `SLIMED` | **致命** | 是 | 绿色黏液 | 10 回合后变成绿色黏液死亡 |
| 23 | `HALLUC` | 状态异常 | 是 | 药水等 | `make_hallucinated(0, TRUE, 0)` 恢复 |
| 24 | `HALLUC_RES` | 抗性 | 否 | — | 静默移除 |
| 25 | `FUMBLING` | 状态异常 | 是 | 诅咒靴/冰面 | 滑倒; 自循环重设 `rnd(20)` 回合 |
| 26 | `WOUNDED_LEGS` | 状态异常 | 是 | 陷阱等 | `heal_legs(0)` |
| 27 | `SLEEPY` | 状态异常 | 是 | 诅咒戒指等 | 入睡; 自循环重设 `rnd(100)` 回合 |
| 28 | `HUNGER` | 状态异常 | 否 | — | 静默移除 |
| 29 | `SEE_INVIS` | 感知 | 是 | 药水等 | 重绘 |
| 30 | `TELEPAT` | 感知 | 否 | — | 静默移除 |
| 31 | `WARNING` | 感知 | 否 | — | 静默移除 |
| 32 | `WARN_OF_MON` | 感知 | 否 | — | 清除 `warntype.species` |
| 33 | `WARN_UNDEAD` | 感知 | 否 | — | 静默移除 |
| 34 | `SEARCHING` | 感知 | 否 | — | 静默移除 |
| 35 | `CLAIRVOYANT` | 感知 | 是 | 药水等 | 静默移除 |
| 36 | `INFRAVISION` | 感知 | 否 | — | 静默移除 |
| 37 | `DETECT_MONSTERS` | 感知 | 是 | 药水等 | `see_monsters()` 重绘 |
| 38 | `BLND_RES` | 抗性 | 否 | — | 静默移除 |
| 39 | `ADORNED` | 外观 | 否 | — | 静默移除 |
| 40 | `INVIS` | 外观 | 是 | 药水等 | 消息 "no longer invisible" |
| 41 | `DISPLACED` | 外观 | 是 | 吃 displacer beast 尸体 | `toggle_displacement()` 消息 |
| 42 | `STEALTH` | 外观 | 否 | — | 静默移除 |
| 43 | `AGGRAVATE_MONSTER` | 外观 | 否 | — | 静默移除 |
| 44 | `CONFLICT` | 外观 | 否 | — | 静默移除 |
| 45 | `JUMPING` | 移动 | 否 | — | 静默移除 |
| 46 | `TELEPORT` | 移动 | 是 | 药水/食物等 | 静默移除 |
| 47 | `TELEPORT_CONTROL` | 移动 | 否 | — | 静默移除 |
| 48 | `LEVITATION` | 移动 | 是 | 药水等 | `float_down()` 下降 |
| 49 | `FLYING` | 移动 | 否 | — | "You land." + `spoteffects()` |
| 50 | `WWALKING` | 移动 | 仅 explore/wizard | 熔岩救命后临时 | "temporary ability to walk on liquid has ended" |
| 51 | `SWIMMING` | 移动 | 否 | — | 静默移除 |
| 52 | `MAGICAL_BREATHING` | 移动 | 是 | 祈祷(毒气区域) | 如在毒气区: "cough" |
| 53 | `PASSES_WALLS` | 移动 | 是 | 祈祷(卡墙) | "hemmed in again" 或 "back to normal" |
| 54 | `SLOW_DIGESTION` | 物理 | 否 | — | 静默移除 |
| 55 | `HALF_SPDAM` | 物理 | 否 | — | 静默移除 |
| 56 | `HALF_PHDAM` | 物理 | 否 | — | 静默移除 |
| 57 | `REGENERATION` | 物理 | 否 | — | 静默移除 |
| 58 | `ENERGY_REGENERATION` | 物理 | 否 | — | 静默移除 |
| 59 | `PROTECTION` | 物理 | 否 | — | 静默移除 |
| 60 | `PROT_FROM_SHAPE_CHANGERS` | 物理 | 否 | — | `restartcham()` |
| 61 | `POLYMORPH` | 物理 | 是 | 药水/陷阱 | 静默移除 |
| 62 | `POLYMORPH_CONTROL` | 物理 | 否 | — | 静默移除 |
| 63 | `UNCHANGING` | 物理 | 否 | — | 静默移除 |
| 64 | `FAST` | 物理 | 是 | 药水/食物 | "feel yourself slow down [a bit]" |
| 65 | `REFLECTING` | 物理 | 否 | — | 静默移除 |
| 66 | `FREE_ACTION` | 物理 | 否 | — | 静默移除 |
| 67 | `FIXED_ABIL` | 物理 | 否 | — | 静默移除 |
| 68 | `LIFESAVED` | 物理 | 否 | — | 静默移除 |

---

## 3. 每回合超时递减逻辑 (`nh_timeout`)

`nh_timeout()` 在每个玩家回合调用一次。完整处理流程:

```pseudocode
fn nh_timeout():
    // === Phase 1: 运气衰减 ===
    baseluck = 0
    if full_moon: baseluck += 1
    if friday_13: baseluck -= 1
    if killed_quest_leader: baseluck -= 4
    if is_archeologist and wearing_fedora: baseluck += 1

    if uluck != baseluck
       and moves % (have_amulet_or_angry_god ? 300 : 600) == 0:
        time_luck = stone_luck(false)   // 仅 blessed/cursed
        nostone = !carrying(LUCKSTONE) and !stone_luck(true)  // true=也算 uncursed
        if uluck > baseluck and (nostone or time_luck < 0): uluck -= 1
        if uluck < baseluck and (nostone or time_luck > 0): uluck += 1

    // 无幸运石: 运气总是向 baseluck 衰减
    // 未诅咒幸运石: 正负运气都不衰减
    // 祝福幸运石: 负运气恢复, 正运气不衰减
    // 诅咒幸运石: 正运气衰减, 负运气不恢复

    if invulnerable: return  // 跳过所有致命处理

    // === Phase 2: 对话序列 (在递减前!) ===
    if Stoned:     stoned_dialogue()
    if Slimed:     slime_dialogue()
    if Vomiting:   vomiting_dialogue()
    if Strangled:  choke_dialogue()
    if Sick:       sickness_dialogue()
    if HLevitation & TIMEOUT: levitation_dialogue()
    if HPasses_walls & TIMEOUT: phaze_dialogue()
    if HMagical_breathing & TIMEOUT: region_dialogue()
    if HSleepy & TIMEOUT: sleep_dialogue()

    // === Phase 3: 变身超时 ===
    if u.mtimedone and --u.mtimedone == 0:
        if Unchanging: u.mtimedone = rnd(100 * youmonst.data.mlevel + 1)
        else if is_were: you_unwere(false)
        else: rehumanize()

    // === Phase 4: 杂项递减 ===
    if u.ucreamed: u.ucreamed -= 1         // 奶油糊脸
    if u.usptime:                           // 法术保护消散
        if --u.usptime == 0 and u.uspellprot:
            u.usptime = u.uspmtime
            u.uspellprot -= 1
            find_ac()
    if u.ugallop: if --u.ugallop == 0: "stops galloping"

    // === Phase 5: 所有属性超时递减 ===
    was_flying = Flying
    for each property p in uprops[0..LAST_PROP]:
        if (p.intrinsic & TIMEOUT) != 0:
            p.intrinsic -= 1              // 递减 TIMEOUT 部分
            if (p.intrinsic & TIMEOUT) == 0:
                handle_timeout_expiry(p)  // 见各属性处理

    // === Phase 6: 对象计时器 ===
    run_timers()
```

**关键设计**: 对话序列在递减之前执行。对话函数看到的是递减前的值。例如 `vomiting_dialogue()` 读取 `Vomiting & TIMEOUT` 然后使用 `v - 1` 来匹配消息。

---

## 4. 状态效果叠加规则

### 4.1 `set_itimeout` (替换) 语义的属性

所有 `make_*` 函数内部都使用 `set_itimeout`,即**替换**当前超时值:

| 属性 | 函数 | 内部操作 |
|------|------|---------|
| CONFUSION | `make_confused(xtime, talk)` | `set_itimeout(&HConfusion, xtime)` |
| STUNNED | `make_stunned(xtime, talk)` | `set_itimeout(&HStun, xtime)` |
| BLINDED | `make_blinded(xtime, talk)` | `set_itimeout(&HBlinded, xtime)` |
| DEAF | `make_deaf(xtime, talk)` | `set_itimeout(&HDeaf, xtime)` |
| HALLUC | `make_hallucinated(xtime, talk, mask)` | `set_itimeout(&HHallucination, xtime)` |
| SICK | `make_sick(xtime, cause, talk, type)` | `set_itimeout(&Sick, xtime)` |
| STONED | `make_stoned(xtime, msg, killedby, killername)` | `set_itimeout(&Stoned, xtime)` |
| SLIMED | `make_slimed(xtime, msg)` | `set_itimeout(&Slimed, xtime)` |
| VOMITING | `make_vomiting(xtime, talk)` | `set_itimeout(&Vomiting, xtime)` |
| GLIB | `make_glib(xtime)` | `set_itimeout(&Glib, xtime)` |

### 4.2 调用方实现的累加

虽然 `make_*` 内部是替换,但调用方常传入 `已有值 + 新增值`,实现事实上的累加:

```pseudocode
// 呕吐时附加混乱 (timeout.c:vomiting_dialogue)
make_confused((HConfusion & TIMEOUT) + d(2,4), FALSE)  // 读旧值, 加上新值

// 呕吐时附加眩晕 (timeout.c:vomiting_dialogue)
make_stunned((HStun & TIMEOUT) + d(2,4), FALSE)
```

### 4.3 `incr_itimeout` (纯累加) 的使用场景

| 场景 | 调用 |
|------|------|
| 雷暴致聋 | `incr_itimeout(&HDeaf, rn1(20, 30))` |
| SLEEPY 到期重设 | `incr_itimeout(&HSleepy, rnd(100))` |
| FUMBLING 到期重设 | `incr_itimeout(&HFumbling, rnd(20))` |

### 4.4 致命状态的叠加规则

- **STONED**: 始终设为 5。调用者通常检查 `!Stoned`,已在石化中不会重复施加。
- **SLIMED**: 始终设为 10。调用者通常检查 `!Slimed`。
- **SICK**: **替换**当前值。已有 SICK 时再次感染会替换倒计时(通常缩短: `Sick/3+1`)。
- **STRANGLED**: 始终设为 5 (勒颈项链)。

### 4.5 SICK 双类型系统

SICK 有两个子类型,以位掩码存储在 `u.usick_type`:

```
SICK_VOMITABLE    = 0x01  // 食物中毒 (可通过呕吐治愈)
SICK_NONVOMITABLE = 0x02  // 疾病 (如来自怪物攻击)
SICK_ALL          = 0x03
```

施加时: `u.usick_type |= type` (OR 累加类型位)。

治愈某种类型时:
```pseudocode
u.usick_type &= ~type
if u.usick_type != 0:      // 仅部分治愈
    set_itimeout(&Sick, Sick * 2)  // 剩余时间翻倍 (近似)
else:
    Sick = 0                // 完全治愈
```

**[疑似 bug]**: 当 Sick 值很大时,`Sick * 2` 可能超出 24 位上限。但 `set_itimeout` 内部的 `itimeout()` 会将其截断到 `0x00FFFFFF`。这在极端情况下可能导致意外的长病程。

---

## 5. 致命倒计时序列

### 5.1 石化 (STONED) — 5 回合

初始设定: `make_stoned(5L, ...)` → `set_itimeout(&Stoned, 5)`

对话在递减前执行,使用 `i = Stoned & TIMEOUT`:

| 回合 (i) | 消息 | 副效果 |
|----------|------|--------|
| 5 | "You are slowing down." | `HFast = 0` (丢失内在速度); 中止多步移动 |
| 4 | "Your limbs are stiffening." | 中止活动 (除非正在开能救命的锡罐 — Popeye 机制); 中止多步移动 |
| 3 | "Your limbs have turned to stone." | `nomul(-3)` 无法移动 3 回合; 治愈 wounded legs |
| 2 | "You have turned to stone." | 延长 HDeaf 至少 5 回合; `make_vomiting(0, FALSE)` 清除呕吐; `make_slimed(0, NULL)` 清除黏液化 |
| 1 | "You are a statue." | (仅消息) |
| 0 | — | `done_timeout(STONING, STONED)` → 死亡 |

每回合 `exercise(A_DEX, FALSE)`。

nolimbs 形态: "limbs" 替换为 "extremities"。

**Popeye 机制**: 如果正在开罐头 (`opentin`), 且罐头已知含 lizard 或 acidic 怪物肉, 在 `i == 4` 时不中止开罐操作。

**中断方法**:
- 吃酸性尸体或 lizard 尸体 (直接治愈)
- 吃含 lizard/acidic 的罐头
- 多态为有 Stone_resistance 的生物
- 祈祷
- 酸性药水 (potion of acid)

### 5.2 黏液化 (SLIMED) — 10 回合

初始设定: `make_slimed(10L, ...)` → `set_itimeout(&Slimed, 10)`

对话使用 `t = Slimed & TIMEOUT`, `i = t / 2`。消息仅在 `t % 2 != 0` (奇数) 时显示:

| t | i | 消息 | 副效果 |
|---|---|------|--------|
| 9 | 4 | "You are turning a little [green]." | — |
| 7 | 3 | "Your limbs are getting oozy." | `HFast = 0` 失去速度; 中止活动 (除非 Popeye); 中止多步 |
| 5 | 2 | "Your skin begins to peel away." | 延长 HDeaf 至少 5 回合 |
| 3 | 1 | "You are turning into [a green slime]." | 如果同时 Stoned: `make_stoned(0, NULL)` 清除石化 |
| 1 | 0 | "You have become [a green slime]." | 设置外观为 PM_GREEN_SLIME; `newsym()` |
| 0 | — | — | `slimed_to_death()`: `polymon(PM_GREEN_SLIME)` 然后 `done_timeout(TURNED_SLIME, SLIMED)` |

偶数 t (10, 8, 6, 4, 2): 无消息,仅执行 switch(i) 的副效果。

每回合 `exercise(A_DEX, FALSE)`。

**到期后 life-saved 的特殊处理**: 如果绿色黏液已被灭绝 (`G_GENOD`), life-saved 后立即再次 `done(GENOCIDED)` — "Unfortunately, green slime has been genocided..."

**中断方法**:
- `burn_away_slime()` — 任何火焰伤害 (消息: "The slime that covers you is burned away!")
- 多态为火焰怪物
- 祈祷
- 吃能让你变身成火焰怪物的罐头 (Popeye: `polyfood(otin)`)

### 5.3 疾病 (SICK) — 可变回合

**初始设定**:
- 怪物攻击 (SICK_NONVOMITABLE): `Sick ? Sick/3 + 1 : rn1(ACURR(A_CON), 20)` — 首次 `[20, 20+CON-1]` 回合; 已生病则当前/3+1
- 食物中毒 (SICK_VOMITABLE): 通常 `Sick > 1 ? Sick - 1 : 1`
- Unicorn horn 治愈失败: `Sick ? Sick/3 + 1 : rn1(ACURR(A_CON), 20)` (同怪物攻击)

对话使用 `j = Sick & TIMEOUT`, `i = j / 2`, 仅在 `j % 2 != 0` 时显示:

| i | 消息 |
|---|------|
| 3 | "Your illness feels worse." (食物中毒: "sickness" 代替 "illness") |
| 2 | "Your illness is severe." |
| 1 | "You are at Death's door." (幻觉时追加: "He/She is inviting you in.") |

每回合 `exercise(A_CON, FALSE)`。

**超时到 0 时**:
```pseudocode
if (usick_type & SICK_NONVOMITABLE) == 0   // 纯食物中毒
   and rn2(100) < ACURR(A_CON):             // CON% 概率恢复
    "You have recovered from your illness."
    make_sick(0, NULL, FALSE, SICK_ALL)
    exercise(A_CON, FALSE)
    adjattrib(A_CON, -1, 1)                  // 永久失去 1 点 CON
else:
    "You die from your illness."
    done_timeout(POISONING, SICK)
    u.usick_type = 0
```

**关键**: 只有**纯食物中毒**(SICK_NONVOMITABLE 位未设)才有 CON% 自动恢复机会。含疾病类型则到 0 必死。

**中断方法**:
- Unicorn horn (随机成功)
- 吃 lizard 尸体
- Eucalyptus leaf
- 祈祷
- 全愈药水 (potion of full healing / extra healing)
- 呕吐 (仅治愈 SICK_VOMITABLE 部分)

### 5.4 窒息 (STRANGLED) — 5 回合

初始设定: 佩戴勒颈项链时 `Strangled` 设为约 5-6 回合。

两套对话文本; 选择条件: Breathless 或 `!rn2(50)` (2% 概率) 使用 `choke_texts2`:

**choke_texts** (标准):

| i | 消息 |
|---|------|
| 5 | "You find it hard to breathe." |
| 4 | "You're gasping for air." |
| 3 | "You can no longer breathe." |
| 2 | "You're turning [blue]." |
| 1 | "You suffocate." |

**choke_texts2** (替代):

| i | 消息 |
|---|------|
| 5 | "Your [neck] is becoming constricted." |
| 4 | "Your blood is having trouble reaching your brain." |
| 3 | "The pressure on your [neck] increases." |
| 2 | "Your consciousness is fading." |
| 1 | "You suffocate." |

到 0: `done_timeout(DIED, STRANGLED)`, killer = "strangulation" (或 "suffocation" 如被埋葬)。

每回合 `exercise(A_STR, FALSE)`。

**到期后 life-saved**: 如果仍佩戴勒颈项链: "Your amulet vanishes!" → `useup(uamul)`。

### 5.5 呕吐 (VOMITING) — 约 15 回合

初始设定: 取决于来源,典型值约 14-20 回合。

对话使用 `v = Vomiting & TIMEOUT`, 匹配 `v - 1` (递减尚未发生):

| v-1 | 事件 |
|-----|------|
| 14 | "You are feeling mildly nauseated." |
| 11 | "You feel slightly confused." (已混乱: "slightly more confused") |
| 9 | `make_confused(HConfusion_timeout + d(2,4), FALSE)` |
| 8 | "You can't seem to think straight." |
| 6 | `make_stunned(HStun_timeout + d(2,4), FALSE)` + 中止活动; **FALLTHROUGH** 到 case 9: `make_confused(...)` |
| 5 | "You feel incredibly sick." |
| 2 | "You are about to vomit." (幻觉: "about to hurl!") |
| 0 | `morehungry(20)` + `vomit()` → `nomul(-2)` |

每回合 `exercise(A_CON, FALSE)`。

**case 6 FALLTHROUGH**: 当 v-1 == 6 时,先执行 stunned,然后 fall through 到 case 9 执行 confused。这是有意设计 — 后期症状更严重。

---

## 6. 属性锻炼/滥用计时器系统 (attrib.c)

### 6.1 概述

6 个属性 (STR, INT, WIS, DEX, CON, CHA) 中只有 4 个可锻炼:
- **可锻炼**: STR, WIS, DEX, CON
- **不可锻炼**: INT, CHA (exercise() 直接 return)

锻炼系统通过 `u.aexe[A_MAX]` (AEXE) 数组跟踪累积的锻炼/滥用值。

### 6.2 exercise() — 累积锻炼值

```pseudocode
fn exercise(attr: int, positive: bool):
    if attr == A_INT or attr == A_CHA: return
    if Upolyd and attr != A_WIS: return   // 变身中不锻炼身体属性

    if abs(AEXE(attr)) < AVAL:  // AVAL = 50, 累积上限
        if positive:
            // 收益递减 Part I: 属性越高越难锻炼
            AEXE(attr) += (rn2(19) > ACURR(attr)) ? 1 : 0
            // 属性 3: 概率 = 15/19 ≈ 79%
            // 属性 10: 概率 = 8/19 ≈ 42%
            // 属性 18: 概率 = 0/19 = 0%
        else:
            AEXE(attr) -= rn2(2)  // 滥用: 50% 概率 -1
```

### 6.3 exerper() — 周期性自动锻炼/滥用

每 10 回合 (`moves % 10 == 0`) 基于饥饿和负重:

```pseudocode
// 饥饿检查
match hunger_state:
    SATIATED:    exercise(A_DEX, false); if Monk: exercise(A_WIS, false)
    NOT_HUNGRY:  exercise(A_CON, true)
    WEAK:        exercise(A_STR, false); if Monk: exercise(A_WIS, true)
    FAINTING:    exercise(A_CON, false)

// 负重检查
match encumbrance:
    MOD_ENCUMBER: exercise(A_STR, true)
    HVY_ENCUMBER: exercise(A_STR, true); exercise(A_DEX, false)
    EXT_ENCUMBER: exercise(A_DEX, false); exercise(A_CON, false)
```

每 5 回合 (`moves % 5 == 0`) 基于状态:

```pseudocode
if (HClairvoyant & (INTRINSIC|TIMEOUT)) and !BClairvoyant: exercise(A_WIS, true)
if HRegeneration: exercise(A_STR, true)
if Sick or Vomiting: exercise(A_CON, false)
if Confusion or Hallucination: exercise(A_WIS, false)
if (Wounded_legs and !steed) or Fumbling or HStun: exercise(A_DEX, false)
```

### 6.4 exerchk() — 属性变化检定

在 `context.next_attrib_check` 时刻触发。间隔 = `rn1(200, 800)` = `[800, 999]` 回合。

```pseudocode
fn exerchk():
    exerper()  // 先执行周期性累积

    if moves >= context.next_attrib_check and !multi:
        for each attr i in 0..A_MAX:
            ax = AEXE(i)
            if ax == 0: continue

            mod_val = sgn(ax)  // +1 or -1

            // 属性上下限
            lolim = ATTRMIN(i)              // 通常 3
            hilim = min(ATTRMAX(i), 18)     // 锻炼不能超过 18

            if (ax < 0 and ABASE(i) <= lolim) or (ax > 0 and ABASE(i) >= hilim):
                goto decay

            if Upolyd and i != A_WIS: goto decay

            // 收益递减 Part III: 概率检定
            threshold = (i != A_WIS) ? abs(ax) * 2 / 3 : abs(ax)
            if rn2(AVAL) > threshold:  // AVAL = 50
                goto decay             // 检定失败

            // 检定成功: 改变属性
            if adjattrib(i, mod_val, -1):
                AEXE(i) = 0  // 成功后归零
                // "You must have been exercising diligently." 等

        decay:
            // 收益递减 Part II: 累积值每次检定衰减一半
            AEXE(i) = (abs(ax) / 2) * mod_val

        context.next_attrib_check += rn1(200, 800)
```

**关键公式汇总**:
- 锻炼增量概率 (正): `(18 - ACURR) / 18` (近似)
- 滥用增量概率 (负): 50%
- 检定成功概率 (非 WIS): `(|AEXE| * 2/3) / 50`
- 检定成功概率 (WIS): `|AEXE| / 50`
- AEXE 每次检定衰减一半 (无论是否成功)
- 检定间隔: `[800, 999]` 回合

### 6.5 临时属性系统 (ATEMP/ATIME)

`u.atemp[A_MAX]` 和 `u.atime[A_MAX]` 用于临时属性变化。

**注意**: `restore_attrib()` 函数在源码中标注为 `(not used)`,ATIME 从未被设为非零值。此系统是历史遗留。

当前仅使用 ATEMP (不通过 ATIME 倒计时):
- STR: 饥饿导致的 -1 (`Weak` 状态时)
- DEX: wounded legs 导致的 -1

这两者各有独立的恢复机制,不依赖 `restore_attrib()`。

---

## 7. 法术保护消散

### 7.1 设置

`cast_protection()` 设置:
- `u.uspellprot`: 当前 AC 加成层数
- `u.uspmtime`: 每层消散间隔 (Expert Protection 技能: 20, 其他: 10)
- `u.usptime`: 当前层剩余倒计时

增益公式:
```pseudocode
gain = log2(ulevel) - uspellprot / (4 - min(3, natac_factor))
// natac_factor = (10 - natural_ac) / 10, 向下取整
```

### 7.2 消散逻辑

每回合 (`nh_timeout` 中):
```pseudocode
if u.usptime > 0:
    u.usptime -= 1
    if u.usptime == 0 and u.uspellprot > 0:
        u.usptime = u.uspmtime      // 重设倒计时
        u.uspellprot -= 1            // 减少一层
        find_ac()                    // 重算 AC
        // "The golden haze around you becomes less dense."
        // 或 "The golden haze around you disappears."
```

---

## 8. 光源计时器

光源使用通用对象计时器系统 (`BURN_OBJECT`),不走属性超时。

### 8.1 燃料数据模型

```
obj.age     = 剩余燃料回合数
obj.lamplit = 是否点亮 (0/1)
obj.spe     = 蜡烛台的蜡烛数; 其他灯未使用
```

### 8.2 灯类型与检查点

**油灯/铜灯** (初始 age ≈ 1500):

检查点: 150, 100, 50, 25, 0

| 剩余 age | 油灯消息 | 铜灯消息 |
|----------|---------|---------|
| 150 | "flickers" | "lantern is getting dim" |
| 100 | "flickers" | "lantern is getting dim" |
| 50 | "flickers considerably" | "lantern is getting dim" |
| 25 | "seems about to go out" | "lantern is getting dim" |
| 0 | "has gone out" | "lantern has run out of power" |

**蜡烛** (tallow: 初始 200; wax: 初始 400):

检查点: 75, 15, 0

| 剩余 age | 消息 |
|----------|------|
| 75 | "candle(s) getting short" |
| 15 | "candle's flame(s) flicker(s) low!" |
| 0 | "consumed!" / "flame(s) die(s)" → 物品销毁 |

**油壶** (POT_OIL):
- `timer = age` (稀释后: `timer = 3*age/4, 进位`)
- 光照半径 = 1 (极微弱)
- 耗尽后物品销毁

**魔法灯**: 不使用计时器,永不熄灭 (`spe=1` 时)。

**神器光源**: 不使用计时器,永远点亮。

### 8.3 `begin_burn` 计时器设置

```pseudocode
// 油灯/铜灯: 分段到下一个检查点
if age > 150: turns = age - 150
elif age > 100: turns = age - 100
elif age > 50:  turns = age - 50
elif age > 25:  turns = age - 25
else:           turns = age

// 蜡烛: 分段到下一个检查点
if age > 75:  turns = age - 75
elif age > 15: turns = age - 15
else:          turns = age

start_timer(turns, TIMER_OBJECT, BURN_OBJECT, obj)
obj.age -= turns   // 预扣: 到期时 age 正好等于检查点值
```

### 8.4 离线处理

```pseudocode
if timeout != svm.moves:  // 玩家不在该层时超时
    how_long = svm.moves - timeout
    if how_long >= obj.age:
        obj.age = 0
        end_burn(obj, false)  // 熄灭, 可能销毁物品
    else:
        obj.age -= how_long
        begin_burn(obj, true) // 重设计时器到下一个检查点
```

---

## 9. 蛋孵化计时器

### 9.1 孵化时间计算

```pseudocode
MAX_EGG_HATCH_TIME = 200

fn attach_egg_hatch_timeout(egg, when):
    stop_timer(HATCH_EGG, egg)

    if when == 0:  // 随机孵化时间
        for i in 151..=200:
            if rnd(i) > 150:
                when = i
                break
        // 第一次成功的期望位置: i ≈ 153-157
        // 200 回合内孵化的累积概率 > 99.9993%

    if when > 0:
        start_timer(when, TIMER_OBJECT, HATCH_EGG, egg)
```

### 9.2 孵化结果

```pseudocode
hatchcount = rnd(egg.quan)  // 栈中部分孵化
yours = egg.spe or (!flags.female and carried(egg) and !rn2(2))

for each hatching:
    mon = makemon(...)
    if (yours and !silent) or (carried(egg) and dragon):
        tamedog(mon)   // 驯服
        if carried and !dragon: mon.mtame = 20
```

- 自己的蛋在同层孵化 → 宠物 (mtame=20)
- 龙蛋在背包中孵化 → 宠物 (标准 tamedog)
- 剩余蛋: 重设短孵化时间 `rnd(12)` 回合

### 9.3 过期蛋判定

```pseudocode
#define stale_egg(egg)  ((svm.moves - egg.age) > 2 * MAX_EGG_HATCH_TIME)
// 超过 400 回合为 "stale" — 不会孵化
```

---

## 10. 尸体老化

### 10.1 关键常量

```
TAINT_AGE           = 50     // 尸体变质的回合数 (吃了可能食物中毒)
ROT_AGE             = 250    // 尸体腐烂消失的回合数
TROLL_REVIVE_CHANCE = 37     // 巨魔每回合复活概率 1/37
```

### 10.2 尸体计时器设置 (`start_corpse_timeout`)

```pseudocode
fn start_corpse_timeout(body):
    // Lizard 和 Lichen 永不腐烂/复活
    if corpsenm == PM_LIZARD or PM_LICHEN: return

    action = ROT_CORPSE
    rot_adjust = in_mklev ? 25 : 10
    age = max(moves, 1) - body.age    // 尸体已存在回合数
    if age > ROT_AGE(250):
        when = rot_adjust
    else:
        when = ROT_AGE - age
    when += rnz(rot_adjust) - rot_adjust  // 随机偏移

    // Rider 特殊处理
    if is_rider(corpsenm):
        action = REVIVE_MON
        when = rider_revival_time(body, false)

    // 巨魔特殊处理
    elif mlet == S_TROLL:
        for age in 2..=TAINT_AGE(50):
            if !rn2(37):          // 每回合 1/37 概率
                action = REVIVE_MON
                when = age
                break
        // 50 回合内复活概率 ≈ 1 - (36/37)^49 ≈ 74.1%

    // 僵尸化
    elif zombify and zombie_form(...) != NON_PM and !body.norevive:
        action = ZOMBIFY_MON
        when = rn1(15, 5)         // [5, 19] 回合

    start_timer(when, TIMER_OBJECT, action, body)
```

### 10.3 Rider 复活时间

```pseudocode
fn rider_revival_time(body, retry):
    minturn = retry ? 3 : (corpsenm == PM_DEATH ? 6 : 12)
    for when in minturn..67:
        if !rn2(3): break    // 每回合 1/3 概率停止
    return when
    // 几何分布: 期望值 ≈ minturn + 1.5 * (1/(1-2/3)) ≈ minturn + 3
    // 最大 67 回合 (保证上限)
```

### 10.4 尸体品质判定

```pseudocode
corpse_age = moves - obj.age

if corpse_age > TAINT_AGE(50):
    // 尸体已变质 — 吃了可能食物中毒
    // 例外: acid blob, lizard, lichen 等
```

冰面保鲜: `peek_at_iced_corpse_age()` 减去冰冻时间,使有效年龄更年轻。

### 10.5 尸体销毁

`rot_corpse()` 触发时:
- 在地上: `newsym()` 更新显示,可能暴露隐藏怪物
- 在背包: "Your [corpse] rots away." + 移除装备
- 在怪物背包: 清除装备标志
- 最终调用 `rot_organic()` 实际销毁对象

---

## 11. 通用对象计时器系统

### 11.1 架构

计时器存储在全局有序链表 `gt.timer_base` 中,按到期时间从近到远排序。

```
struct timer_element {
    long timeout;         // 绝对到期时间 (创建时 moves + when)
    short kind;           // TIMER_OBJECT | TIMER_LEVEL | etc.
    short func_index;     // ROT_ORGANIC, BURN_OBJECT, etc.
    anything arg;         // 指向对象的指针或坐标
    timer_element *next;
};
```

### 11.2 计时器类型枚举

| 枚举 | 类型 | 说明 |
|------|------|------|
| ROT_ORGANIC (0) | OBJECT | 埋葬有机物腐烂 |
| ROT_CORPSE (1) | OBJECT | 尸体腐烂消失 |
| REVIVE_MON (2) | OBJECT | 尸体复活 (巨魔、Rider) |
| ZOMBIFY_MON (3) | OBJECT | 尸体变僵尸 |
| BURN_OBJECT (4) | OBJECT | 光源燃烧 (有 cleanup 回调) |
| HATCH_EGG (5) | OBJECT | 蛋孵化 |
| FIG_TRANSFORM (6) | OBJECT | 小雕像变化 |
| SHRINK_GLOB (7) | OBJECT | 黏液团缩小 |
| MELT_ICE_AWAY (8) | LEVEL | 冰面融化 |

### 11.3 `run_timers()` 执行

```pseudocode
fn run_timers():
    while timer_base != null and timer_base.timeout <= moves:
        curr = 弹出首元素
        if curr.kind == TIMER_OBJECT: curr.arg.a_obj.timed -= 1
        调用 timeout_funcs[curr.func_index].f(&curr.arg, curr.timeout)
        释放 curr
```

### 11.4 计时器操作

```pseudocode
start_timer(when, kind, func_index, arg) -> bool:
    // 检查重复: 同一 (kind, func_index, arg) 不能有两个
    timeout = moves + when
    插入到有序链表
    if TIMER_OBJECT: arg.a_obj.timed++
    return true

stop_timer(func_index, arg) -> long:
    从链表移除匹配项
    调用 cleanup 回调 (如果有)
    return 剩余时间 (timeout - moves)

peek_timer(func_index, arg) -> long:
    返回到期时间 (绝对), 0 表示不存在
```

### 11.5 小雕像变化计时器

```pseudocode
when = rnd(9000) + 200    // [201, 9200] 回合
start_timer(when, TIMER_OBJECT, FIG_TRANSFORM, figurine)
```

---

## 12. 测试向量

### 石化倒计时

| # | 输入 | 预期输出 |
|---|------|---------|
| TV-1 | `make_stoned(5L, ...)`, 5 回合不治疗, 无 Stone_resistance | 回合 5: "slowing down", HFast=0; 回合 4: "limbs stiffening"; 回合 3: "limbs turned to stone", nomul(-3); 回合 2: "turned to stone", 清除 Vomiting+Slimed; 递减到 0: `done(STONING)` |
| TV-2 | STONED=2 和 SLIMED=4 同时存在 | slime_dialogue 在 t=3 (i=1) 时: `make_stoned(0, NULL)` 清除石化。黏液化优先于石化。 |

### 疾病自然恢复 (边界条件)

| # | 输入 | 预期输出 |
|---|------|---------|
| TV-3 | Sick=1 到期, usick_type=SICK_VOMITABLE, CON=18 | `rn2(100) < 18`: 18% 恢复 (CON 永久-1), 82% 死亡 |
| TV-4 | Sick=1 到期, usick_type=SICK_VOMITABLE, CON=1 | `rn2(100) < 1`: 1% 恢复, 99% 死亡 |
| TV-5 | Sick=1 到期, usick_type=SICK_NONVOMITABLE, CON=25 | 无恢复机会, 100% 死亡 (不检定 CON) |
| TV-6 | Sick=1 到期, usick_type=SICK_ALL, CON=18 | `(usick_type & SICK_NONVOMITABLE) != 0` → 无恢复机会, 100% 死亡 |

### 运气衰减

| # | 输入 | 预期输出 |
|---|------|---------|
| TV-7 | uluck=5, 无 luckstone, 非满月非周五 13, moves % 600 == 0 | nostone=true, uluck > 0: uluck 减至 4 |
| TV-8 | uluck=5, 持有未诅咒 luckstone, moves % 600 == 0 | nostone=false, time_luck=0 (stone_luck(FALSE)=0), `FALSE or FALSE`=FALSE → uluck 不变 |

### 属性锻炼 (边界条件)

| # | 输入 | 预期输出 |
|---|------|---------|
| TV-9 | STR=18, exercise(A_STR, true) | 增量概率 = rn2(19) > 18 → 只有 rn2(19)返回18时成功 → 0/19 = 0%。**属性 18 时无法通过锻炼提升** |
| TV-10 | STR=3, exercise(A_STR, true) | 增量概率 = rn2(19) > 3 → 15/19 ≈ 79%。**属性 3 时锻炼效率最高** |
| TV-11 | AEXE(STR)=50, exercise(A_STR, true) | abs(AEXE) >= AVAL(50), 不再增加。锻炼值有上限 |

### 蛋孵化 (边界条件)

| # | 输入 | 预期输出 |
|---|------|---------|
| TV-12 | 创建蛋, when=0 (随机), i=151 | 首次: rnd(151) > 150 → 概率 1/151 ≈ 0.66%; 若失败继续到 i=152: rnd(152) > 150 → 2/152 ≈ 1.3%; 累积到 200 概率 > 99.999% |
| TV-13 | 蛋 age 距当前 > 400 回合 | `stale_egg()` 返回 true, 该蛋不会孵化 |

### 光源计时器

| # | 输入 | 预期输出 |
|---|------|---------|
| TV-14 | 新油灯 age=1500, begin_burn | age > 150: turns = 1500 - 150 = 1350; obj.age = 1500 - 1350 = 150; 首个 timer 在 1350 回合后触发 |
| TV-15 | 蜡烛 age=75, begin_burn | age > 75 为 false; age > 15 为 true: turns = 75 - 15 = 60; obj.age = 75 - 60 = 15; timer 在 60 回合后触发, 届时 age=15 显示 "flame flickers low" |

### 尸体腐烂 (边界条件)

| # | 输入 | 预期输出 |
|---|------|---------|
| TV-16 | 新鲜普通尸体 (age=moves), 非 mklev | when = 250 - 0 + noise ≈ 250 回合后 ROT_CORPSE |
| TV-17 | 巨魔尸体 | 2..50 回合窗口, 每回合 1/37 概率 REVIVE_MON; 若 49 回合均未命中 (概率 ≈ 25.9%) → ROT_CORPSE |
| TV-18 | Rider (Death) 尸体, 首次 | minturn=6; 之后每回合 1/3 break; 最迟 67; 期望 ≈ 9 回合复活 |

### 超时最大值边界

| # | 输入 | 预期输出 |
|---|------|---------|
| TV-19 | `incr_itimeout(&HBlinded, 0x01000000)`, 当前 timeout=0 | new = 0 + 0x01000000 = 16777216; `itimeout` 检测 >= 0x00FFFFFF → 截断到 16777215 |
| TV-20 | SICK 双类型部分治愈: Sick=0x007FFFFF, usick_type=SICK_ALL, 治愈 SICK_VOMITABLE | `Sick * 2 = 0x00FFFFFE`; `set_itimeout` 截断到 0x00FFFFFE (未溢出, 但接近上限) |

---

## 附录: `nh_timeout()` switch 中未处理的属性

以下属性在 `nh_timeout()` 的 switch-case 中没有专门的到期处理。它们的超时在正常游戏中不会自然到达 0(仅通过 `#wizintrinsic` 设置)。到期时进入 default 分支,属性静默消失:

COLD_RES, SLEEP_RES, DISINT_RES, SHOCK_RES, POISON_RES, DRAIN_RES, SICK_RES, ANTIMAGIC, HALLUC_RES, BLND_RES, HUNGER, TELEPAT, WARNING, WARN_UNDEAD, SEARCHING, INFRAVISION, ADORNED, STEALTH, AGGRAVATE_MONSTER, CONFLICT, JUMPING, TELEPORT_CONTROL, SWIMMING, SLOW_DIGESTION, HALF_SPDAM, HALF_PHDAM, REGENERATION, ENERGY_REGENERATION, PROTECTION, POLYMORPH_CONTROL, UNCHANGING, REFLECTING, FREE_ACTION, FIXED_ABIL, LIFESAVED, INVULNERABLE, TELEPORT, POLYMORPH, CLAIRVOYANT
