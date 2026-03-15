# NetHack 3.7 Artifact System Specification

Source: `include/artilist.h`, `include/artifact.h`, `src/artifact.c`, `src/uhitm.c`, `src/fountain.c`, `src/pray.c`, `src/objnam.c`, `src/do_name.c`, `src/dothrow.c`, `src/questpgr.c`, `src/role.c`

---

## 1. Artifact 定义表

### 1.1 数据结构

每个 artifact 由 `struct artifact` 定义：

```
artifact:
  otyp:       基础物品类型
  name:       名称字符串
  spfx:       挥舞/穿戴时特殊效果 (位掩码)
  cspfx:      仅携带时特殊效果 (位掩码)
  mtype:      目标怪物类型/符号/标志
  attk:       攻击属性 {adtyp, damn, damd}
  defn:       防御属性 {adtyp}
  cary:       携带属性 {adtyp}
  inv_prop:   激活能力编号
  alignment:  所属阵营 (A_LAWFUL / A_NEUTRAL / A_CHAOTIC / A_NONE)
  role:       关联职业 (PM_xxx / NON_PM)
  race:       关联种族 (PM_xxx / NON_PM)
  gen_spe:    生成时 spe 偏移量
  gift_value: 祭坛赠礼最低祭品价值 (通常 = 怪物难度 + 1)
  cost:       售价 (若 0 则使用 100 * 基础物品价格)
  acolor:     发光颜色
```

### 1.2 SPFX 标志位

| 标志 | 值 | 含义 |
|---|---|---|
| SPFX_NOGEN | 0x00000001 | 不随机生成，只能通过神赐/特殊途径获得 |
| SPFX_RESTR | 0x00000002 | 受限——不能通过命名获得 |
| SPFX_INTEL | 0x00000004 | 自我意志——智能神器 |
| SPFX_SPEAK | 0x00000008 | 能说话（耳语谣言） |
| SPFX_SEEK  | 0x00000010 | 帮助搜索 [定义但未在代码中引用，疑似遗留标志] |
| SPFX_WARN  | 0x00000020 | 警告危险/特定怪物 |
| SPFX_ATTK  | 0x00000040 | 具有特殊攻击 |
| SPFX_DEFN  | 0x00000080 | 具有特殊防御 |
| SPFX_DRLI  | 0x00000100 | 吸取等级 |
| SPFX_SEARCH | 0x00000200 | 帮助搜索 |
| SPFX_BEHEAD | 0x00000400 | 斩首/腰斩 |
| SPFX_HALRES | 0x00000800 | 幻觉抗性 |
| SPFX_ESP    | 0x00001000 | 心灵感应 |
| SPFX_STLTH  | 0x00002000 | 潜行 |
| SPFX_REGEN  | 0x00004000 | 生命恢复 |
| SPFX_EREGEN | 0x00008000 | 魔力恢复 |
| SPFX_HSPDAM | 0x00010000 | 半法术伤害 |
| SPFX_HPHDAM | 0x00020000 | 半物理伤害 |
| SPFX_TCTRL  | 0x00040000 | 传送控制 |
| SPFX_LUCK   | 0x00080000 | 增加运气（如幸运石） |
| SPFX_DMONS  | 0x00100000 | 对特定怪物种类加成 |
| SPFX_DCLAS  | 0x00200000 | 对特定怪物符号加成 |
| SPFX_DFLAG1 | 0x00400000 | 对 mflags1 标志加成 |
| SPFX_DFLAG2 | 0x00800000 | 对 mflags2 标志加成 |
| SPFX_DALIGN | 0x01000000 | 对非本阵营怪物加成 |
| SPFX_DBONUS | 0x01F00000 | 攻击加成掩码（以上5种的组合） |
| SPFX_XRAY   | 0x02000000 | X 射线视觉（范围 3） |
| SPFX_REFLECT | 0x04000000 | 反射 |
| SPFX_PROTECT | 0x08000000 | 保护 |

### 1.3 完整 Artifact 列表

**约定**：`PHYS(a,b)` = 物理伤害，命中骰 d(a), 伤害骰 d(b)。`DRLI(a,b)` = 等级吸取。`COLD/FIRE/ELEC/STUN/POIS(a,b)` = 相应元素攻击。`DFNS(c)` = 穿戴时防御 c 类型。`CARY(c)` = 携带时防御 c 类型。

#### 普通神器

| # | 名称 | 基础物品 | spfx | cspfx | mtype | 攻击 | 防御 | 携带 | Invoke | 阵营 | 职业 | 种族 | gen_spe | gift_value | 价格 |
|---|------|---------|------|-------|-------|------|------|------|--------|------|------|------|---------|------------|------|
| 1 | Excalibur | LONG_SWORD | NOGEN,RESTR,SEEK,DEFN,INTEL,SEARCH | - | - | PHYS(5,10) | DRLI(0,0) | - | - | Lawful | Knight | - | 0 | 10 | 4000 |
| 2 | Stormbringer | RUNESWORD | RESTR,ATTK,DEFN,INTEL,DRLI | - | - | DRLI(5,2) | DRLI(0,0) | - | - | Chaotic | - | - | 0 | 9 | 8000 |
| 3 | Mjollnir | WAR_HAMMER | RESTR,ATTK | - | - | ELEC(5,24) | - | - | - | Neutral | Valkyrie | - | 0 | 8 | 4000 |
| 4 | Cleaver | BATTLE_AXE | RESTR | - | - | PHYS(3,6) | - | - | - | Neutral | Barbarian | - | 0 | 8 | 1500 |
| 5 | Grimtooth | ORCISH_DAGGER | RESTR,WARN,DFLAG2 | - | M2_ELF | PHYS(2,6) | POIS(0,0) | - | FLING_POISON | Chaotic | - | Orc | 0 | 5 | 1200 |
| 6 | Orcrist | ELVEN_BROADSWORD | WARN,DFLAG2 | - | M2_ORC | PHYS(5,0) | - | - | - | Chaotic | - | Elf | 3 | 4 | 2000 |
| 7 | Sting | ELVEN_DAGGER | WARN,DFLAG2 | - | M2_ORC | PHYS(5,0) | - | - | - | Chaotic | - | Elf | 3 | 1 | 800 |
| 8 | Magicbane | ATHAME | RESTR,ATTK,DEFN | - | - | STUN(3,4) | DFNS(AD_MAGM) | - | - | Neutral | Wizard | - | 0 | 7 | 3500 |
| 9 | Frost Brand | LONG_SWORD | RESTR,ATTK,DEFN | - | - | COLD(5,0) | COLD(0,0) | - | SNOWSTORM | None | - | - | 0 | 9 | 3000 |
| 10 | Fire Brand | LONG_SWORD | RESTR,ATTK,DEFN | - | - | FIRE(5,0) | FIRE(0,0) | - | FIRESTORM | None | - | - | 0 | 5 | 3000 |
| 11 | Dragonbane | BROADSWORD | RESTR,DCLAS,REFLECT | - | S_DRAGON | PHYS(5,0) | - | - | - | None | - | - | 2 | 5 | 500 |
| 12 | Demonbane | SILVER_MACE | RESTR,DFLAG2 | - | M2_DEMON | PHYS(5,0) | - | - | BANISH | Lawful | Cleric | - | 1 | 3 | 2500 |
| 13 | Werebane | SILVER_SABER | RESTR,DFLAG2 | - | M2_WERE | PHYS(5,0) | DFNS(AD_WERE) | - | - | None | - | - | 1 | 4 | 1500 |
| 14 | Grayswandir | SILVER_SABER | RESTR,HALRES | - | - | PHYS(5,0) | - | - | - | Lawful | - | - | 0 | 10 | 8000 |
| 15 | Giantslayer | LONG_SWORD | RESTR,DFLAG2 | - | M2_GIANT | PHYS(5,0) | - | - | - | Neutral | - | - | 2 | 4 | 200 |
| 16 | Ogresmasher | WAR_HAMMER | RESTR,DCLAS | - | S_OGRE | PHYS(5,0) | - | - | - | None | - | - | 2 | 1 | 200 |
| 17 | Trollsbane | MORNING_STAR | RESTR,DCLAS,REGEN | - | S_TROLL | PHYS(5,0) | - | - | - | None | - | - | 2 | 1 | 200 |
| 18 | Vorpal Blade | LONG_SWORD | RESTR,BEHEAD | - | - | PHYS(5,1) | - | - | - | Neutral | - | - | 1 | 5 | 4000 |
| 19 | Snickersnee | KATANA | RESTR | - | - | PHYS(0,8) | - | - | - | Lawful | Samurai | - | 0 | 8 | 1200 |
| 20 | Sunsword | LONG_SWORD | RESTR,DFLAG2 | - | M2_UNDEAD | PHYS(5,0) | DFNS(AD_BLND) | - | BLINDING_RAY | Lawful | - | - | 0 | 6 | 1500 |

#### 任务神器（SPFX_NOGEN | SPFX_RESTR | SPFX_INTEL，gen_spe=0，gift_value=12）

| # | 名称 | 基础物品 | 额外 spfx | cspfx | mtype | 攻击 | 防御 | 携带 | Invoke | 阵营 | 职业 |
|---|------|---------|-----------|-------|-------|------|------|------|--------|------|------|
| 21 | The Orb of Detection | CRYSTAL_BALL | - | ESP,HSPDAM | - | - | - | CARY(AD_MAGM) | INVIS | Lawful | Archeologist |
| 22 | The Heart of Ahriman | LUCKSTONE | - | STLTH | - | PHYS(5,0) | - | - | LEVITATION | Neutral | Barbarian |
| 23 | The Sceptre of Might | MACE | DALIGN | - | - | PHYS(5,0) | DFNS(AD_MAGM) | - | CONFLICT | Lawful | Cave Dweller |
| 24 | The Staff of Aesculapius | QUARTERSTAFF | ATTK,DRLI,REGEN | - | - | DRLI(0,0) | DRLI(0,0) | - | HEALING | Neutral | Healer |
| 25 | The Magic Mirror of Merlin | MIRROR | SPEAK | ESP | - | - | - | CARY(AD_MAGM) | - | Lawful | Knight |
| 26 | The Eyes of the Overworld | LENSES | XRAY | - | - | - | DFNS(AD_MAGM) | - | ENLIGHTENING | Neutral | Monk |
| 27 | The Mitre of Holiness | HELM_OF_BRILLIANCE | DFLAG2,PROTECT | - | M2_UNDEAD | - | - | CARY(AD_FIRE) | ENERGY_BOOST | Lawful | Cleric |
| 28 | The Longbow of Diana | BOW | REFLECT | ESP | - | PHYS(5,0) | - | - | CREATE_AMMO | Chaotic | Ranger |
| 29 | The Master Key of Thievery | SKELETON_KEY | SPEAK | WARN,TCTRL,HPHDAM | - | - | - | - | UNTRAP | Chaotic | Rogue |
| 30 | The Tsurugi of Muramasa | TSURUGI | BEHEAD,LUCK,PROTECT | - | - | PHYS(0,8) | - | - | - | Lawful | Samurai |
| 31 | The Platinum Yendorian Express Card | CREDIT_CARD | DEFN | ESP,HSPDAM | - | - | - | CARY(AD_MAGM) | CHARGE_OBJ | Neutral | Tourist |
| 32 | The Orb of Fate | CRYSTAL_BALL | LUCK | WARN,HSPDAM,HPHDAM | - | - | - | - | LEV_TELE | Neutral | Valkyrie |
| 33 | The Eye of the Aethiopica | AMULET_OF_ESP | - | EREGEN,HSPDAM | - | - | DFNS(AD_MAGM) | - | CREATE_PORTAL | Neutral | Wizard |

---

## 2. 神器特殊攻击

### 2.1 命中加成公式 (`spec_abon`)

```
IF artifact.attk.damn > 0 AND spec_applies(artifact, target):
    bonus = rnd(artifact.attk.damn)   // 即 1..damn
ELSE:
    bonus = 0
```

此加成加到命中骰上。

### 2.2 伤害加成公式 (`spec_dbon`)

```
IF artifact 无攻击 (adtyp == AD_PHYS AND damn == 0 AND damd == 0):
    spec_dbon_applies = FALSE
    return 0
ELSE IF artifact == Grimtooth:
    spec_dbon_applies = TRUE  // [特例: 忽略 spec_applies，对所有目标有效]
ELSE:
    spec_dbon_applies = spec_applies(artifact, target)

IF spec_dbon_applies:
    IF artifact.attk.damd > 0:
        return rnd(artifact.attk.damd)   // 即 1..damd
    ELSE:
        return max(base_weapon_damage, 1)  // 翻倍基础伤害（至少 1）
ELSE:
    return 0
```

**关键解读**：当 `damd == 0` 且 `spec_dbon_applies` 时，返回 `max(tmp, 1)`，其中 `tmp` 是已计算的基础武器伤害。这意味着有效伤害翻倍。适用于 Frost Brand（COLD 5,0）、Fire Brand（FIRE 5,0）、Dragonbane、Demonbane、Werebane、Grayswandir、Giantslayer、Ogresmasher、Trollsbane、Sunsword、Orcrist、Sting 等 `damd=0` 的 bane 武器。

### 2.3 spec_applies 匹配逻辑

```
IF artifact.spfx & (SPFX_DBONUS | SPFX_ATTK) == 0:
    return (attk.adtyp == AD_PHYS)  // 无特殊：仅物理攻击适用

IF SPFX_DMONS: 目标 == 指定怪物种类
IF SPFX_DCLAS: 目标符号 == mtype (如 S_DRAGON, S_OGRE, S_TROLL)
IF SPFX_DFLAG1: 目标.mflags1 & mtype != 0
IF SPFX_DFLAG2: 目标.mflags2 & mtype != 0
    // 若目标是玩家: 检查种族标志或狼人状态
IF SPFX_DALIGN: 目标阵营 != 神器阵营
IF SPFX_ATTK:
    // 检查目标是否对该元素攻击有抗性
    // 有抗性 -> FALSE, 无抗性 -> TRUE
    // 元素类型: FIRE, COLD, ELEC, MAGM, STUN, DRST, DRLI, STON
```

### 2.4 各 Artifact 具体攻击效果

| 神器 | 攻击骰 | 对谁生效 | 附加效果 |
|------|--------|---------|---------|
| Excalibur | d5 命中 + d10 伤害 | 所有目标 (AD_PHYS) | 搜索、吸取防御 |
| Stormbringer | d5 命中 + d2 伤害 + 吸取等级 | 无吸取抗性目标 | 见 2.5 |
| Mjollnir | d5 命中 + d24 伤害 | 无电抗性目标 | 1/5 概率摧毁物品 |
| Cleaver | d3 命中 + d6 伤害 | 所有目标 | 攻击三格（见 2.6） |
| Grimtooth | d2 命中 + d6 伤害 | **所有目标**（但警告只针对精灵）| 永久附毒 |
| Orcrist | d5 命中 + max(基础,1) 伤害 | 兽人 (M2_ORC) | 警告兽人(蓝色光芒) |
| Sting | d5 命中 + max(基础,1) 伤害 | 兽人 (M2_ORC) | 警告兽人(蓝色光芒) |
| Magicbane | d3 命中 + d4 伤害 | 无魔抗目标 | 见 2.7 |
| Frost Brand | d5 命中 + max(基础,1) 伤害 | 无冰抗目标 | 1/4 概率摧毁物品 |
| Fire Brand | d5 命中 + max(基础,1) 伤害 | 无火抗目标 | 1/4 概率摧毁物品，融化粘液 |
| Dragonbane | d5 命中 + max(基础,1) 伤害 | S_DRAGON 类 | - |
| Demonbane | d5 命中 + max(基础,1) 伤害 | M2_DEMON | - |
| Werebane | d5 命中 + max(基础,1) 伤害 | M2_WERE | - |
| Grayswandir | d5 命中 + max(基础,1) 伤害 | 所有目标 (AD_PHYS) | 幻觉抗性；银材质 |
| Giantslayer | d5 命中 + max(基础,1) 伤害 | M2_GIANT | - |
| Ogresmasher | d5 命中 + max(基础,1) 伤害 | S_OGRE 类 | - |
| Trollsbane | d5 命中 + max(基础,1) 伤害 | S_TROLL 类 | 生命恢复 |
| Vorpal Blade | d5 命中 + d1 伤害 | 所有目标 | 见 2.8 |
| Snickersnee | 0 命中 + d8 伤害 | 所有目标 (AD_PHYS) | - |
| Sunsword | d5 命中 + max(基础,1) 伤害 | M2_UNDEAD | 发光; 致盲抗性 |
| Heart of Ahriman | d5 命中 + max(基础,1) 伤害 | 所有目标 (AD_PHYS) | - |
| Sceptre of Might | d5 命中 + max(基础,1) 伤害 | 非本阵营 (DALIGN) | - |
| Staff of Aesculapius | 0 命中 + 吸取等级 | 无吸取抗性目标 | 见 2.5 |
| Longbow of Diana | d5 命中 + max(基础,1) 伤害 | 所有目标 (AD_PHYS) | 仅用于发射箭矢 |
| Tsurugi of Muramasa | 0 命中 + d8 伤害 | 所有目标 (AD_PHYS) | 见 2.9 |

### 2.5 等级吸取 (SPFX_DRLI: Stormbringer / Staff of Aesculapius)

`artifact_hit` 中处理:

**对怪物 (youattack):**
```
drain = monhp_per_lvl(mdef)     // 通常 1d8
// 限制: 如果 mhpmax - drain <= m_lev 则减少 drain
IF mdef.m_lev == 0:
    dmg = 2 * mdef.mhp + 200    // 致命伤害
ELSE:
    dmg += drain
    mdef.mhpmax -= drain
    mdef.m_lev -= 1

// 攻击者治疗: (drain + 1) / 2（向上取整）
IF youattack: healup(heal_amount)
ELSE: healmon(magr, heal_amount)
```

**对玩家 (youdefend):**
```
losexp("life drainage")    // 失去一个经验等级
// 攻击者治疗: (|old_uhpmax - new_uhpmax| + 1) / 2
```

### 2.6 Cleaver 三格劈砍

当挥舞 Cleaver 时，`hitum_cleave()` 替代普通攻击：
- 攻击 3 格：主目标 + 左右各一个相邻格
- 交替顺时针/逆时针扫描
- 不能与双持同时使用
- 不能在吞噬/缠绕/无对角线移动时使用
- 不享受背刺或击碎武器的附加效果
- 如果主目标被第一击杀死（如气孢爆炸），第二击跳过已死目标

### 2.7 Magicbane 特殊效果

Magicbane 的魔法效果与附魔值有反向依赖。`MB_MAX_DIEROLL = 8`，只有 dieroll <= 8 时触发特殊效果。

```
scare_dieroll = MB_MAX_DIEROLL / 2  = 4
IF spe >= 3: scare_dieroll /= 2^(spe/3)
IF NOT spec_dbon_applies: dieroll += 1

do_stun = (max(spe, 0) < rn2(spec_dbon_applies ? 11 : 7))

// 伤害累积（每个级别额外 +1d4）:
始终: dmg += 1d4                    // 探测: 总 (2..3)d4
IF do_stun: dmg += 1d4             // 眩晕: 总 (3..4)d4
IF dieroll <= scare_dieroll: dmg += 1d4  // 恐惧: 总 (3..5)d4
IF dieroll <= scare_dieroll/2: dmg += 1d4  // 取消: 总 (4..6)d4
```

效果优先级 (高到低): 取消 > 恐惧 > 眩晕 > 探测

- **取消**: cancel_monst()，玩家失去 1 max Pw；若攻击有魔法攻击的怪物，攻击者获得 1 max Pw
- **恐惧**: 玩家瘫痪 3 回合(如有魔抗则抵抗)；怪物逃跑 3 回合(50%概率抵抗)
- **眩晕**: 玩家 +3 眩晕计时；怪物设 mstun
- **探测**: spe==0 或 !rn2(3*|spe|) 时探测怪物

所有情况下额外 1/12 概率造成混乱（玩家: +4 回合混乱计时；怪物: 设 mconf）。

### 2.8 Vorpal Blade 斩首

触发条件: `dieroll == 1` 或目标是 Jabberwock

**对怪物:**
- 无头/非头部命中/吞噬中: "misses wildly"，伤害设为 0
- 非实体/无定形: "slices through neck"，但不造成额外致命伤害
- 其他: `dmg = 2 * mdef.mhp + 200`，即死

**对玩家:**
- 无头: "misses wildly"，伤害设为 0
- 非实体/无定形: "slices through neck"，仅普通伤害
- 其他: `dmg = 2 * (Upolyd ? u.mh : u.uhp) + 200`，即死

### 2.9 Tsurugi of Muramasa 腰斩

触发条件: `dieroll == 1`

**对怪物 (非吞噬中):**
- 大型怪物 (bigmonst): `dmg *= 2`（伤害翻倍），不致命
- 小型怪物: `dmg = 2 * mdef.mhp + 200`，即死，"cuts in half"

**吞噬中:**
- "slice wide open"，`dmg = 2 * mdef.mhp + 200`，即死

**对玩家:**
- 大型形态: `dmg *= 2`（伤害翻倍），不致命
- 小型形态: `dmg = 2 * (Upolyd ? u.mh : u.uhp) + 200`，即死

---

## 3. 神器防御属性

防御效果来自两个来源：`defn` 字段（穿戴/挥舞时）和 `cary` 字段（携带时）。

### 3.1 defn 防御（穿戴时生效）

| 神器 | defn.adtyp | 效果 |
|------|-----------|------|
| Excalibur | AD_DRLI | 吸取抗性 |
| Stormbringer | AD_DRLI | 吸取抗性 |
| Grimtooth | AD_DRST | 毒素抗性 |
| Magicbane | AD_MAGM | 魔法抗性 |
| Frost Brand | AD_COLD | 冰冻抗性 |
| Fire Brand | AD_FIRE | 火焰抗性 |
| Werebane | AD_WERE | 狼人抗性 |
| Staff of Aesculapius | AD_DRLI | 吸取抗性 |
| Sceptre of Might | AD_MAGM | 魔法抗性 |
| Eyes of the Overworld | AD_MAGM | 魔法抗性 |
| Sunsword | AD_BLND | 致盲抗性 (特殊处理: 仅挥舞时) |
| Eye of the Aethiopica | AD_MAGM | 魔法抗性 |

### 3.2 cary 防御（携带时生效）

| 神器 | cary.adtyp | 效果 |
|------|-----------|------|
| Orb of Detection | AD_MAGM | 魔法抗性 |
| Magic Mirror of Merlin | AD_MAGM | 魔法抗性 |
| Mitre of Holiness | AD_FIRE | 火焰抗性 |
| Platinum Yendorian Express Card | AD_MAGM | 魔法抗性 |

### 3.3 spfx 授予的被动属性（挥舞/穿戴时生效）

| spfx | 属性 | 拥有此 spfx 的神器 |
|------|------|-------------------|
| SPFX_SEARCH | 搜索 | Excalibur |
| SPFX_HALRES | 幻觉抗性 | Grayswandir |
| SPFX_ESP | 心灵感应 | (仅在 cspfx 中) |
| SPFX_REGEN | 生命恢复 | Trollsbane, Staff of Aesculapius |
| SPFX_REFLECT | 反射 (仅挥舞时) | Dragonbane, Longbow of Diana |
| SPFX_PROTECT | 保护 | Tsurugi of Muramasa, Mitre of Holiness |
| SPFX_XRAY | X射线视觉(范围3) | Eyes of the Overworld |
| SPFX_SEEK | [定义但未在代码中实现，无实际效果] | Excalibur |
| SPFX_DRLI | 吸取等级(攻击) | Stormbringer, Staff of Aesculapius |
| SPFX_BEHEAD | 斩首/腰斩 | Vorpal Blade, Tsurugi of Muramasa |

### 3.4 cspfx 授予的被动属性（携带时生效）

| 神器 | cspfx 属性 |
|------|-----------|
| Orb of Detection | ESP, HSPDAM |
| Heart of Ahriman | STLTH |
| Magic Mirror of Merlin | ESP |
| Longbow of Diana | ESP |
| Master Key of Thievery | WARN, TCTRL, HPHDAM |
| Platinum Yendorian Express Card | ESP, HSPDAM |
| Orb of Fate | WARN, HSPDAM, HPHDAM |
| Eye of the Aethiopica | EREGEN, HSPDAM |

### 3.5 Sunsword 特殊发光

Sunsword 挥舞时发光（`artifact_light()` 返回 TRUE）。此行为在核心代码中硬编码处理，而非通过 artifact 字段。挥舞时同时授予致盲抗性（`EBlnd_resist |= W_WEP`）。

### 3.6 神器免疫侵蚀 (`arti_immune`)

如果神器的 `attk.adtyp`、`defn.adtyp` 或 `cary.adtyp` 匹配侵蚀类型，则该神器免疫该类型的侵蚀。`AD_PHYS` 不免疫。

例如：Frost Brand (attk=COLD, defn=COLD) 免疫冰冻侵蚀; Fire Brand (attk=FIRE, defn=FIRE) 免疫火焰侵蚀。

---

## 4. 神器 Invoke 能力

### 4.1 冷却时间

```
IF obj.age > current_moves:
    // 检查是否能以 Pw 代替等待
    pw_cost = arti_invoke_cost_pw(obj)   // FLING_POISON 和 BLINDING_RAY: SPELL_LEV_PW(5) = 25 Pw
                                          // 其他: -1 (不能用Pw代替)
    IF pw_cost < 0 OR u.uen < pw_cost:
        "artifact is ignoring you"
        obj.age += d(3,10)               // 再次尝试会增加冷却
        return
    ELSE:
        u.uen -= pw_cost                 // 消耗 Pw 代替等待
ELSE:
    obj.age = current_moves + rnz(100)   // 设置冷却
```

`rnz(100)` 的期望值约 100，但可能显著偏大或偏小（见 rnz 算法）。

**inv_prop <= LAST_PROP 的 toggle 类能力**（CONFLICT, LEVITATION, INVIS）：
- 开启时检查 age 冷却；若冷却中则失败
- 关闭时设置 `obj.age = current_moves + rnz(100)`

### 4.2 各 Invoke 效果

| inv_prop | 神器 | 效果 |
|----------|------|------|
| INVIS | Orb of Detection | 切换隐形 (toggle) |
| LEVITATION | Heart of Ahriman | 切换漂浮 (toggle) |
| CONFLICT | Sceptre of Might | 切换冲突 (toggle) |
| HEALING | Staff of Aesculapius | 恢复 (uhpmax+1-uhp)/2 HP；治疗疾病、粘液、失明 |
| ENERGY_BOOST | Mitre of Holiness | 恢复 min((uenmax+1-uen)/2, 120) Pw；至少恢复到满（若差值<12） |
| ENLIGHTENING | Eyes of the Overworld | 显示完整角色信息 (魔法鉴定) |
| UNTRAP | Master Key of Thievery | 执行 #untrap 操作 |
| CHARGE_OBJ | Platinum Yendorian Express Card | 充能选中物品；blessed且角色是Tourist(或无职业限制): +1充能; cursed: -1; 否则: 0 |
| LEV_TELE | Orb of Fate | 层级传送 |
| CREATE_PORTAL | Eye of the Aethiopica | 打开到已探索地牢的传送门（不能带护符，不能在终局） |
| CREATE_AMMO | Longbow of Diana | 创建箭矢；blessed: spe>=0, +1d10数量; cursed: spe<=0; normal: +1d5数量 |
| BANISH | Demonbane | 驱逐视野内的恶魔/小鬼到 Gehennom；boss(dprince+2概率, dlord+1概率)更难驱逐；在任务中未杀boss则+10概率不被驱逐 |
| FLING_POISON | Grimtooth | 投掷毒液（50%致盲毒液/50%酸毒液）; 消耗 25 Pw |
| SNOWSTORM | Frost Brand | 施放 cone of cold，以 Expert 技能等级 |
| FIRESTORM | Fire Brand | 施放 fireball，以 Expert 技能等级 |
| BLINDING_RAY | Sunsword | 向方向发射致盲光线；向上/下则照亮当前位置；向自己则 blessed:15伤害, uncursed:10, cursed:5 的闪光灼伤; 消耗 25 Pw |

---

## 5. 触碰效果 (Blast Damage)

当不合适的生物尝试接触（捡起/挥舞）神器时 (`touch_artifact`):

### 5.1 条件判定

```
self_willed = (spfx & SPFX_INTEL) != 0

IF 玩家:
    badclass = self_willed AND (职业不匹配 OR 种族不匹配)
    badalign = (SPFX_RESTR) AND alignment != A_NONE
               AND (神器阵营 != 玩家阵营 OR 玩家 alignment record < 0)
ELSE IF 非 covetous 且非 mplayer:
    badclass = self_willed AND role != NON_PM AND 不是 Excalibur
    badalign = (SPFX_RESTR) AND alignment != A_NONE
               AND (神器阵营 != 怪物阵营)
ELSE (covetous/mplayer):
    badclass = badalign = FALSE

// 检查 bane
IF NOT badalign:
    badalign = bane_applies(artifact, monster)
```

### 5.2 伤害

触发条件: `(badclass OR badalign) AND self_willed` 或 `badalign AND (非玩家 OR !rn2(4))`

对怪物：返回 0（不允许接触）

对玩家：
```
dmg = d(Antimagic ? 2 : 4, self_willed ? 10 : 4)
// 即:
//   有魔抗 + 智能神器: 2d10 (2-20)
//   无魔抗 + 智能神器: 4d10 (4-40)
//   有魔抗 + 非智能:   2d4  (2-8)
//   无魔抗 + 非智能:   4d4  (4-16)

// 银质材料额外伤害
IF 物品材质是银 AND 玩家厌恶银:
    tmp = rnd(10)
    dmg += Maybe_Half_Phys(tmp)    // 半物理伤害减免适用
```

### 5.3 无法抓握

若 `badclass AND badalign AND self_willed`：
- 物品 "evades your grasp"（未持有）或 "is beyond your control"（已持有）
- 返回 0，无法使用

---

## 6. Stormbringer 嗜血行为

Stormbringer 有特殊的反社会行为（硬编码于 `uhitm.c`）：

- 当挥舞 Stormbringer 面对和平怪物时，自动设置 `override_confirmation = TRUE`，跳过确认提示
- 面对宠物时，若挥舞 Stormbringer 则不执行正常的换位行为，而是强制攻击
- 攻击时显示 "Your bloodthirsty blade attacks!"

---

## 7. 职业特定任务神器

| 职业 | 任务神器 |
|------|---------|
| Archeologist | The Orb of Detection |
| Barbarian | The Heart of Ahriman |
| Cave Dweller | The Sceptre of Might |
| Healer | The Staff of Aesculapius |
| Knight | The Magic Mirror of Merlin |
| Monk | The Eyes of the Overworld |
| Cleric (Priest) | The Mitre of Holiness |
| Ranger | The Longbow of Diana |
| Rogue | The Master Key of Thievery |
| Samurai | The Tsurugi of Muramasa |
| Tourist | The Platinum Yendorian Express Card |
| Valkyrie | The Orb of Fate |
| Wizard | The Eye of the Aethiopica |

所有任务神器都具有 `SPFX_NOGEN | SPFX_RESTR | SPFX_INTEL`。

`hack_artifacts()` 在游戏初始化时将当前角色对应的任务神器的阵营和职业设为当前角色的阵营和职业。

---

## 8. 愿望获得神器的规则

源码位于 `objnam.c` 的 `readobjnam()` 末尾。

### 8.1 流程

```
1. 匹配名字到 artifact_name()
2. 通过 oname(otmp, name, ONAME_WISH) 尝试创建
3. 如果创建成功:
   - otmp.quan = 1
   - u.uconduct.wisharti++（无论是否最终获得）

4. 失败检查:
   IF is_quest_artifact(otmp):
       // 任务神器绝不能通过愿望获得
       强制失败
   ELSE IF otmp.oartifact AND rn2(nartifact_exist()) > 1:
       // 已存在 N 个神器时，有 (N-2)/N 概率失败
       // N=1: 0/1 > 1 永假 -> 总是成功
       // N=2: rn2(2) 即 {0,1}, 无一 > 1 -> 总是成功
       // N=3: rn2(3) 即 {0,1,2}, 只有 2>1 -> 1/3 概率失败
       // N=4: {0,1,2,3}, 2个>1 -> 2/4 = 1/2 概率失败
       // 一般: (N-2)/N 概率失败
       强制失败

5. 失败时:
   - artifact_exists(otmp, name, FALSE) 取消创建
   - obfree(otmp) 释放对象
   - 显示 "you feel something in your hands, but it disappears!"
   - 许愿仍然被消耗
```

### 8.2 总结

- **任务神器**: 不能愿望获得（无论如何）
- **SPFX_NOGEN 神器** (如 Excalibur): 不能愿望获得（因为 SPFX_RESTR 阻止命名，但 NOGEN 本身不直接影响愿望——注意 oname() 中的逻辑是，如果该 artifact 已存在则失败；如果不存在且物品类型匹配，则可以创建）

[修正: SPFX_NOGEN 不直接阻止愿望。阻止愿望的是 rn2(nartifact_exist()) > 1 的概率检查。如果该神器的 otyp 匹配且不存在，oname() 会创建它。但 Excalibur 同时是 SPFX_NOGEN | SPFX_RESTR | SPFX_INTEL 且 role=Knight，其任务神器标记不在此处生效（Excalibur 不是任务神器——Knight 的任务神器是 Magic Mirror of Merlin）。因此 Excalibur **可以**被愿望获得（受概率检查）。]

- **成功概率**: 取决于游戏中已存在的神器总数 N:
  - N <= 2: 必定成功
  - N = 3: 2/3 成功
  - N = k (k>=3): 2/k 成功
- **Wizard 模式**: 跳过概率检查
- **同一物品类型**: 如果你愿望一个已存在的神器，`exist_artifact()` 返回 TRUE，`oname()` 不会将其变为神器，你只会得到一个同名的普通物品

---

## 9. 祭坛赠礼系统

源码位于 `pray.c` 的 `bestow_artifact()`。

### 9.1 赠礼条件

```
do_bestow = (u.ulevel > 2) AND (u.uluck >= 0)

IF do_bestow:
    nartifacts = nartifact_exist()
    do_bestow = !rn2(6 + 2 * u.ugifts * nartifacts)
    // 概率 = 1 / (6 + 2 * 已收赠礼数 * 已存在神器数)
    // 第一次赠礼(ugifts=0): 1/6
    // 第二次(ugifts=1, nartifacts>=1): 1/(6+2*N)
```

### 9.2 候选选择 (`mk_artifact`)

```
FOR each artifact a:
    IF a.exists: SKIP
    IF a.spfx & SPFX_NOGEN: SKIP  // 任务神器等不能被赠礼
    IF a.gift_value > max_giftvalue AND 不是角色专属: SKIP

    IF 角色专属 (Role_if(a.role)):
        // 强制选择此神器（覆盖列表，只留这一个）
        BREAK

    // 非角色专属的检查:
    IF (a.alignment == 祭坛阵营 OR a.alignment == A_NONE)
       AND (a.race == NON_PM OR 种族不敌对):

        // 阵营匹配 or (无阵营且已有赠礼) or (无阵营且1/3概率):
        // 技能兼容性检查:
        //   1/4 概率跳过技能检查
        //   或 max_skill >= SKILLED
        //   或 (max_skill >= BASIC AND 1/2 概率)
        IF 通过上述检查: 加入候选列表
        ELSE IF 候选列表空: 加入备选列表
```

### 9.3 赠礼后处理

- `spe` 调整: `spe + gen_spe`，限制在 [-10, 10) 范围内
- `spe < 0` 时提升到 0
- 取消诅咒
- 设为防侵蚀
- `u.ugifts++`
- `u.ublesscnt = rnz(300 + 50 * nartifacts)` — 增加下次祝福等待时间
- 解锁武器技能

---

## 10. Excalibur 特殊获取方式

源码位于 `fountain.c` 的 `dipfountain()`。

### 10.1 条件

```
obj.otyp == LONG_SWORD
AND u.ulevel >= 5
AND !rn2(Role_if(PM_KNIGHT) ? 6 : 30)   // 骑士: 1/6, 其他: 1/30
AND obj.quan == 1
AND !obj.oartifact
AND !exist_artifact(LONG_SWORD, "Excalibur")
```

### 10.2 秩序阵营 (Lawful)

- 长剑变为 Excalibur
- blessed, 防侵蚀(oerodeproof)
- 侵蚀清除(oeroded = oeroded2 = 0)
- 喷泉消失
- 发现神器

### 10.3 非秩序阵营

- 长剑被诅咒
- 1/3 概率 spe 下降 1（不低于 -6）
- 清除防侵蚀
- 喷泉消失
- WIS 练习失败

### 10.4 hack_artifacts() 对 Excalibur 的特殊处理

```
IF 当前角色不是 Knight:
    Excalibur.role = NON_PM
    // 使得非骑士也能挥舞而不受职业限制的 blast
```

Excalibur 是唯一同时具有 SPFX_NOGEN（不随机生成）但可以通过喷泉获得的神器。

---

## 11. 神器命名规则

源码位于 `do_name.c` 和 `artifact.c` 的 `restrict_name()`。

### 11.1 `restrict_name()` 逻辑

```
FOR each artifact a:
    IF obj.otyp 与 a.otyp 相同类型:
        // "相同类型"判定:
        //   已鉴定: 只有精确匹配
        //   未鉴定: 同类同描述或同洗牌池的所有物品
        IF name 匹配 a.name:
            return (a.spfx & (SPFX_NOGEN | SPFX_RESTR)) != 0
                   OR obj.quan > 1
```

### 11.2 命名流程

```
1. 玩家输入名字
2. artifact_name() 模糊匹配（忽略空格和连字符差异）
3. restrict_name() 检查:
   - 如果匹配的神器有 SPFX_NOGEN 或 SPFX_RESTR → 名字被拒绝
   - 如果物品数量 > 1 → 名字被拒绝（不能把一叠匕首命名为 Sting）
   - 如果该神器已存在(exist_artifact) → 名字被拒绝
4. 如果名字被拒绝: 名字被 "slipped"（变为乱码字符串）
5. 如果通过: oname(obj, name, ONAME_VIA_NAMING) 创建神器
```

### 11.3 可通过命名获得的神器

只有同时满足以下条件的神器可以通过命名获得：
- 不含 `SPFX_NOGEN`
- 不含 `SPFX_RESTR`
- 该神器尚未存在
- 持有正确类型的单个物品

满足条件的神器：
- **Orcrist** (ELVEN_BROADSWORD) — 有 WARN, DFLAG2, 无 RESTR
- **Sting** (ELVEN_DAGGER) — 有 WARN, DFLAG2, 无 RESTR

所有其他非任务普通神器都包含 `SPFX_RESTR`，因此**不能通过命名获得**。

### 11.4 已有神器不可更名

如果物品已经是神器 (`obj->oartifact`)，尝试重命名时会得到 "{name} resists the attempt."

---

## 12. 防止掉落/被偷/被销毁的特殊规则

### 12.1 神器抵抗销毁

`obj_resists(obj, ochance, achance)`:
- 非神器: `rn2(100) < ochance` 概率抵抗
- 神器: `rn2(100) < achance` 概率抵抗
- 对于大部分销毁检查，`achance` 远高于 `ochance`（典型值如 2 vs 98 表示神器 98% 抵抗、普通物品 2% 抵抗）
- 不死鸟物品（Amulet of Yendor, Book of the Dead, Candelabrum, Bell of Opening, 骑士尸体）**总是**抵抗

### 12.2 智能神器 (SPFX_INTEL) 的特殊行为

智能神器（所有任务神器 + Excalibur + Stormbringer + Magicbane）在 `touch_artifact()` 中有额外的职业/种族检查。不匹配时：
- 怪物: 返回 0 (无法拾取)
- 玩家: 受到爆炸伤害，可能无法抓握

### 12.3 变形后重新检查 (`retouch_equipment`)

当玩家发生变化（阵营改变、狼化、变形）时，所有穿戴/携带的神器重新进行 `touch_artifact()` 检查。失败的物品：
- 被脱下/解除挥舞
- 根据调用者参数可能被丢弃
- 如果 invoke 能力正在激活中，也会被反转

---

## 13. Mjollnir 投掷机制

### 13.1 投掷条件
- 必须正在挥舞（`uwep`）
- 力量 >= 25 (`STR19(25)`)
- 必须是 Valkyrie 才能自动返回

### 13.2 返回判定（仅 Valkyrie）
```
IF rn2(100):  // 99% 概率返回
    IF NOT impaired AND rn2(100):  // 99% 概率接住
        "returns to your hand!" → 自动再挥舞
    ELSE:
        // 未接住
        IF rn2(2):  // 50% 概率无伤
            "lands at your feet"
        ELSE:
            dmg = rnd(3) + 额外 (artifact_hit 的闪电伤害)
            "hits your arm!"
            // 可能摧毁戒指/魔杖
ELSE:
    "fails to return!" → 留在目标位置
```

### 13.3 射程
```
range = (range + 1) / 2   // Mjollnir 射程减半（因为重）
```

---

## 14. 测试向量

### TV-1: Excalibur 基础伤害

```
输入: Excalibur 攻击一个普通怪物 (如 gnome)
spec_abon: rnd(5) = 1..5  (命中加成)
spec_dbon: rnd(10) = 1..10 (额外伤害)
总额外伤害: 1..10
spec_dbon_applies: TRUE (AD_PHYS, damn=5, damd=10, 对所有目标生效)
```

### TV-2: Grimtooth 对非精灵目标

```
输入: Grimtooth 攻击一个 hill giant (非精灵)
spec_dbon_applies: TRUE (Grimtooth 特例绕过 spec_applies)
spec_abon: rnd(2) = 1..2
spec_dbon: rnd(6) = 1..6
输出: 命中加成 1..2, 额外伤害 1..6
```

### TV-3: Frost Brand 对有冰抗怪物

```
输入: Frost Brand 攻击 white dragon (有冰抗)
spec_applies: FALSE (目标有冰抗性)
spec_dbon_applies: FALSE
spec_abon: 0
spec_dbon: 0
输出: 无额外命中或伤害
```

### TV-4: Frost Brand 对无冰抗怪物，基础伤害 = 7

```
输入: Frost Brand 攻击 gnome (无冰抗), 基础武器伤害 tmp = 7
spec_applies: TRUE
spec_dbon_applies: TRUE
spec_abon: rnd(5) = 1..5
spec_dbon: damd == 0 → return max(7, 1) = 7
输出: 命中加成 1..5, 额外伤害 = 7 (有效翻倍)
```

### TV-5: Vorpal Blade 对 Jabberwock

```
输入: Vorpal Blade 攻击 Jabberwock, dieroll = 15 (非1)
因为目标是 Jabberwock → 仍然触发斩首
Jabberwock 有头, 非 noncorporeal/amorphous
输出: dmg = 2 * mdef.mhp + 200, 即死
```

### TV-6: Tsurugi 腰斩大型怪物

```
输入: Tsurugi of Muramasa 攻击 giant (bigmonst), dieroll = 1
输出: dmg *= 2 (翻倍, 不即死)
```

### TV-7: Touch blast — 智能神器 + 无魔抗

```
输入: Chaotic Wizard 尝试拾取 Excalibur (Lawful, INTEL)
badalign: TRUE (Chaotic != Lawful)
self_willed: TRUE (SPFX_INTEL)
dmg = d(4, 10) = 4..40 (无 Antimagic, 有 INTEL)
输出: 受到 4-40 点伤害, 可能无法抓握
```

### TV-8: Touch blast — 有魔抗 + 非智能神器

```
输入: Chaotic player (有 Antimagic) 尝试拾取 Demonbane (Lawful, 非 INTEL)
badalign: TRUE (Chaotic != Lawful)
self_willed: FALSE (无 SPFX_INTEL)
触发: badalign AND !rn2(4) → 25% 概率触发
IF 触发: dmg = d(2, 4) = 2..8
输出: 25% 概率受 2-8 伤害, 但始终可以拾取
```

### TV-9: 愿望神器 — 已存在 3 个神器时 (边界条件)

```
输入: 愿望 Grayswandir, nartifact_exist() = 3 (含新创建的 Grayswandir)
检查: rn2(3) > 1 → {0,1,2} 中只有 2 > 1
概率: 1/3 失败, 2/3 成功
输出: 2/3 概率获得 Grayswandir, 1/3 概率消失
```

### TV-10: 愿望神器 — 已存在 2 个神器时 (边界条件)

```
输入: 愿望 Fire Brand, nartifact_exist() = 2 (含新创建的)
检查: rn2(2) > 1 → {0,1} 无一 > 1
概率: 0% 失败
输出: 必定获得 Fire Brand
注: nartifact_exist() 在此时包含了刚创建的神器本身
```

### TV-11: Excalibur 喷泉 — 骑士 vs 非骑士 (边界条件)

```
输入A: Knight, level 5, 持长剑浸泡喷泉
概率: 1/6 触发 Excalibur 事件 (若尚不存在)
Knight 是 Lawful → 获得 blessed Excalibur

输入B: Valkyrie (Neutral), level 5, 持长剑浸泡喷泉
概率: 1/30 触发 Excalibur 事件
Valkyrie 默认 Neutral → 长剑被诅咒, 可能 spe-1

输入C: 任意角色, level 4, 持长剑浸泡喷泉
u.ulevel < 5 → 不触发 Excalibur 事件
```

### TV-12: Magicbane 效果 — spe=3, dieroll=2 (边界条件)

```
输入: Magicbane spe=3, dieroll=2, spec_dbon_applies=TRUE
scare_dieroll = 4 / 2^(3/3) = 4 / 2 = 2
dieroll(2) <= scare_dieroll(2) → 进入 scare
dieroll(2) <= scare_dieroll/2(1)? 2 <= 1 → 否, 不进入 cancel

do_stun: max(3,0)=3 < rn2(11), 约 8/11 概率 stun

额外伤害: +1d4 (base) + maybe 1d4 (stun) + 1d4 (scare) = (3..4)d4 或 (2..3)d4
attack_index: MB_INDEX_SCARE
效果: 怪物逃跑 3 回合 (50% 概率抵抗)
```

### TV-13: Stormbringer DRLI — 等级 0 怪物 (边界条件)

```
输入: Stormbringer 攻击等级 0 怪物 (如 grid bug, m_lev=0)
spec_dbon: rnd(2) = 1..2
DRLI 处理: mdef.m_lev == 0 → dmg = 2 * mdef.mhp + 200 (致命)
输出: 怪物被杀, 攻击者不回血 (drain=0 因为 mhpmax <= m_lev 条件限制)
```

[疑似 bug，如实记录原始行为: 当 m_lev=0 时，drain 计算中 `mhpmax - drain <= m_lev` 即 `mhpmax - drain <= 0`; 因为 drain = monhp_per_lvl(mdef) 通常为 1d8, 而 m_lev=0 的怪物 mhpmax 可能很小(如 grid bug 约 1-4)，drain 可能 > mhpmax 导致 drain 被设为 0 或 mhpmax-1。但因为 m_lev==0 走了直接致命的分支，drain 值在此路径上不被使用。攻击者也不会因致命路径而获得治疗。]

### TV-14: 赠礼概率 — 第一次 (边界条件)

```
输入: u.ugifts = 0, nartifact_exist() = 0, u.ulevel = 3, u.uluck = 0
do_bestow = TRUE (ulevel > 2, uluck >= 0)
概率 = 1 / (6 + 2*0*0) = 1/6
输出: 1/6 概率尝试生成赠礼 (若祭品价值足够)
```
