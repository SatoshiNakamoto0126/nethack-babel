# NetHack 3.7 物品鉴定系统机制规格

> 源码版本: NetHack-3.7 分支 (2025-04 快照)
> 主要源文件: `src/objnam.c`, `src/o_init.c`, `src/invent.c`, `src/shk.c`, `src/pager.c`, `src/do_name.c`, `src/read.c`, `src/zap.c`, `src/do_wear.c`, `src/do.c`, `src/engrave.c`, `src/insight.c`, `src/potion.c`
> 主要头文件: `include/obj.h`, `include/objclass.h`, `include/hack.h`, `include/objects.h`

---

## 1. 鉴定层级系统

NetHack 的物品鉴定是一个多层系统，分为**个体级标志**（存储在每个 `struct obj` 上）和**类别级标志**（存储在 `struct objclass objects[]` 全局数组上）。

### 1.1 个体级标志 (per-object bitfields)

每个物品实例 (`struct obj`) 上有以下鉴定相关位域:

| 位域 | 宽度 | 含义 | 设置条件 |
|------|------|------|----------|
| `dknown` | 1 bit | 外观描述已知 — 物品被近距离看到过 | 非盲时调用 `xname()`/`observe_object()` |
| `known` | 1 bit | 精确属性已知 — 附魔值、充能数等；某些无趣物品默认预设为 1 | 使用/鉴定后设置；若 `oc_uses_known==0` 则默认为 1 |
| `bknown` | 1 bit | BUC 状态（祝福/诅咒/无诅咒）已知 | 祭坛、宠物拾取、牧师角色、穿戴诅咒物品等 |
| `rknown` | 1 bit | 防锈/防火/防腐蚀状态已知 | 被侵蚀攻击、完全鉴定等 |
| `cknown` | 1 bit | 容器内容已知；锡罐已知内容类型 | 打开容器、完全鉴定 |
| `lknown` | 1 bit | 锁定/解锁状态已知（箱子） | 尝试开锁、完全鉴定 |
| `tknown` | 1 bit | 陷阱状态已知（箱子） | 检测到陷阱、完全鉴定 |

**注意**: 代码中定义了 `eknown`（效果已知——盲目时使用魔杖或佩戴戒指后知道效果但还没看到物品本身），但在 3.7 中用 `#if 0` 禁用，**未实装**。

### 1.2 类别级标志 (per-type fields in `objects[]`)

| 字段 | 含义 |
|------|------|
| `oc_name_known` | 该类型已被发现(discovered)——所有该类型的物品都显示真实名称 |
| `oc_uses_known` | 该类型的 `obj->known` 标志是否影响显示；为 0 时 `known` 总是预设为 1 |
| `oc_encountered` | 英雄至少观察到过一次该类型（控制发现列表中的出现） |
| `oc_uname` | 玩家给该类型起的"叫做"名称（字符串指针或 NULL） |
| `oc_name_idx` | 指向 `obj_descr[]` 中真实名称的索引 |
| `oc_descr_idx` | 指向 `obj_descr[]` 中外观描述的索引（游戏初始化时被 shuffle 打乱） |

### 1.3 `oc_uses_known` 的作用

```
IF oc_uses_known == 1:
    obj->known 控制是否显示附魔值/充能数
    （武器、护甲、戒指、魔杖、充能工具等）
IF oc_uses_known == 0:
    obj->known 总是预设为 1（如食物、金币等）
    xname() 不需要 known 标志来决定显示内容
```

在 `objects.h` 的宏定义中，`oc_uses_known` 的设置规则：
- `BITS(kn, mrg, chg, 0, mgc, chg, ...)` 中的 `spec` 参数同时设置 `oc_uses_known` 和 `oc_charged`
- 武器、护甲: `oc_uses_known = 0`（附魔值通过 `known` 位单独控制）
- 戒指（有数值的）: `oc_uses_known = 1`
- 魔杖: `oc_uses_known = 1`（控制充能数显示）

**关键逻辑**（`xname_flags()` 第 639 行）:
```
IF NOT oc_name_known AND oc_uses_known AND oc_unique:
    强制 obj->known = 0
```
这防止了未鉴定的独特物品通过冠词（"the" vs "a"）泄露身份信息。

---

## 2. 类别级鉴定 vs 个体级鉴定

### 2.1 类别级鉴定（discover / `oc_name_known`）

当一个类型被 discover 后，**所有**该类型的物品都显示真实名称。

**触发条件**:
- 调用 `makeknown(otyp)` — 这是一个宏，展开为 `discover_object(otyp, TRUE, TRUE, TRUE)`
- 使用卷轴/药水/魔杖产生可观察效果时
- 穿戴护甲/护符产生可观察效果时
- 鉴定卷轴/法术作用于物品时

**`discover_object()` 逻辑**:
```
FUNCTION discover_object(oindx, mark_as_known, mark_as_encountered, credit_hero):
    IF oindx < FIRST_OBJECT: RETURN  // 不发现通用对象

    IF (NOT oc_name_known AND mark_as_known)
       OR (NOT oc_encountered AND mark_as_encountered)
       OR (是武士角色 AND 有日语名称):

        将 oindx 加入 disco[] 发现列表

        IF mark_as_encountered:
            objects[oindx].oc_encountered = 1

        IF NOT oc_name_known AND mark_as_known:
            objects[oindx].oc_name_known = 1
            IF credit_hero: exercise(A_WIS, TRUE)  // 增加智慧经验
            IF 是宝石类: gem_learned()  // 可能影响未付款宝石的价格
            update_inventory()
```

### 2.2 个体级鉴定

每个物品实例有自己的 `known`, `bknown`, `rknown`, `cknown`, `lknown`, `tknown` 标志。这些标志独立于类别级 `oc_name_known`。

**完全鉴定**（`fully_identify_obj()`）:
```
FUNCTION fully_identify_obj(otmp):
    makeknown(otmp->otyp)           // 类别级发现
    IF otmp->oartifact:
        discover_artifact(otmp->oartifact)  // 加入神器发现列表
    observe_object(otmp)             // 设置 dknown, 标记 encountered
    otmp->known = 1
    otmp->bknown = 1
    otmp->rknown = 1
    set_cknown_lknown(otmp)         // 若适用设置 cknown, lknown
    IF otmp 是蛋 AND corpsenm != NON_PM:
        learn_egg_type(otmp->corpsenm)
```

### 2.3 `not_fully_identified()` — 判断物品是否还需要鉴定

```
FUNCTION not_fully_identified(otmp) -> boolean:
    IF otmp 是金币: RETURN FALSE  // 金币总是完全鉴定的

    // 检查基本标志
    IF NOT otmp->known OR NOT otmp->dknown OR NOT otmp->bknown
       (对 SCR_MAIL 跳过 bknown 检查):
        RETURN TRUE
    IF NOT objects[otmp->otyp].oc_name_known:
        RETURN TRUE

    // 检查容器相关标志
    IF (NOT cknown AND (是容器 OR 是雕像)):
        RETURN TRUE
    IF (NOT lknown AND 是箱子):
        RETURN TRUE

    // 检查神器
    IF otmp->oartifact AND undiscovered_artifact(otmp->oartifact):
        RETURN TRUE

    // 检查 rknown
    IF otmp->rknown:
        RETURN FALSE  // 防侵蚀状态已知
    IF otmp 不是护甲/武器/武器工具/铁球:
        RETURN FALSE  // 这些类别不需要 rknown
    ELSE:
        RETURN is_damageable(otmp)  // 只有可被侵蚀的物品才需要 rknown
```

---

## 3. 外观表系统

### 3.1 初始化流程

在 `init_objects()` 中：

1. 所有物品的 `oc_name_idx` 和 `oc_descr_idx` 初始化为自身索引 `i`
2. 宝石颜色随机化（`randomize_gem_colors()`）
3. 检查 `oc_name_known` 初始值的一致性
4. 调用 `shuffle_all()` 打乱外观描述

### 3.2 `shuffle_all()` 打乱的范围

**整类打乱**（`shuffle_classes[]`）:
- 护符类 (AMULET_CLASS) — 排除非魔法和唯一物品
- 药水类 (POTION_CLASS) — 排除 potion of water (固定为 "clear")
- 戒指类 (RING_CLASS) — 全类
- 卷轴类 (SCROLL_CLASS) — 排除非魔法和唯一物品
- 魔法书类 (SPBOOK_CLASS) — 排除非魔法和唯一物品
- 魔杖类 (WAND_CLASS) — 全类
- 毒液类 (VENOM_CLASS) — 全类

**子类打乱**（`shuffle_types[]`）:
- 头盔 (HELMET ~ HELM_OF_TELEPATHY)
- 手套 (LEATHER_GLOVES ~ GAUNTLETS_OF_DEXTERITY)
- 披风 (CLOAK_OF_PROTECTION ~ CLOAK_OF_DISPLACEMENT)
- 靴子 (SPEED_BOOTS ~ LEVITATION_BOOTS)

### 3.3 `shuffle()` 算法

```
FUNCTION shuffle(o_low, o_high, domaterial):
    // 计算可打乱的物品数（排除已知类型）
    num_to_shuffle = COUNT j IN [o_low..o_high] WHERE NOT oc_name_known
    IF num_to_shuffle < 2: RETURN

    FOR j FROM o_low TO o_high:
        IF objects[j].oc_name_known: CONTINUE
        DO:
            i = j + rn2(o_high - j + 1)  // [j, o_high] 范围内随机
        WHILE objects[i].oc_name_known

        SWAP objects[j].oc_descr_idx WITH objects[i].oc_descr_idx
        SWAP objects[j].oc_tough WITH objects[i].oc_tough
        SWAP objects[j].oc_color WITH objects[i].oc_color
        IF domaterial:
            SWAP objects[j].oc_material WITH objects[i].oc_material
```

**关键点**: 材质只在整类打乱时交换（`domaterial=TRUE`），子类（护甲子类）打乱时不交换材质（`domaterial=FALSE`）。这意味着戒指的材质（木、矿物、宝石、铁等）会被打乱，但靴子的材质（皮革 vs 铁）不会。

### 3.4 宝石颜色随机化

```
FUNCTION randomize_gem_colors():
    IF rn2(2): 绿松石 → 蓝色（复制蓝宝石的描述）
    IF rn2(2): 海蓝宝石 → 蓝色（复制蓝宝石的描述）
    SWITCH rn2(4):        // 萤石颜色
        0: 保持紫色
        1: 蓝色（复制蓝宝石的描述）
        2: 白色（复制钻石的描述）
        3: 绿色（复制祖母绿的描述）
```

### 3.5 `obj_shuffle_range()` — 外观打乱范围查询

该函数返回与给定 `otyp` 共享外观池的物品范围:

| 类别 | 范围 |
|------|------|
| 头盔 | HELMET ~ HELM_OF_TELEPATHY |
| 手套 | LEATHER_GLOVES ~ GAUNTLETS_OF_DEXTERITY |
| 披风 | CLOAK_OF_PROTECTION ~ CLOAK_OF_DISPLACEMENT |
| 靴子 | SPEED_BOOTS ~ LEVITATION_BOOTS |
| 药水 | bases[POTION_CLASS] ~ POT_WATER - 1 |
| 护符/卷轴/魔法书 | bases[class] ~ 首个非魔法或唯一物品前 |
| 戒指/魔杖/毒液 | 全类 (bases[class] ~ bases[class+1] - 1) |

---

## 4. 完整外观列表

### 4.1 药水颜色（25 种，其中 24 种参与打乱）

| 默认对应 | 外观描述 |
|----------|----------|
| gain ability | ruby |
| restore ability | pink |
| confusion | orange |
| blindness | yellow |
| paralysis | emerald |
| speed | dark green |
| levitation | cyan |
| hallucination | sky blue |
| invisibility | brilliant blue |
| see invisible | magenta |
| healing | purple-red |
| extra healing | puce |
| gain level | milky |
| enlightenment | swirly |
| monster detection | bubbly |
| object detection | smoky |
| gain energy | cloudy |
| sleeping | effervescent |
| full healing | black |
| polymorph | golden |
| booze | brown |
| sickness | fizzy |
| fruit juice | dark |
| acid | white |
| oil | murky |
| **water** (固定) | **clear** |

### 4.2 卷轴标签（21 种魔法 + 20 种额外，共 41 种参与打乱）

**魔法卷轴 (21 种)**:
ZELGO MER, JUYED AWK YACC, NR 9, XIXAXA XOXAXA XUXAXA, PRATYAVAYAH, DAIYEN FOOELS, LEP GEX VEN ZEA, PRIRUTSENIE, ELBIB YLOH, VERR YED HORRE, VENZAR BORGAVVE, THARR, YUM YUM, KERNOD WEL, ELAM EBOW, DUAM XNAHT, ANDOVA BEGARIN, KIRJE, VE FORBRYDERNE, HACKEM MUCHE, VELOX NEB

**额外标签 (20 种)**:
FOOBIE BLETCH, TEMOV, GARVEN DEH, READ ME, ETAOIN SHRDLU, LOREM IPSUM, FNORD, KO BATE, ABRA KA DABRA, ASHPD SODALG, ZLORFIK, GNIK SISI VLE, HAPAX LEGOMENON, EIRIS SAZUN IDISI, PHOL ENDE WODAN, GHOTI, MAPIRO MAHAMA DIROMAT, VAS CORP BET MANI, XOR OTA, STRC PRST SKRZ KRK

**固定描述** (不参与打乱):
- mail → "stamped"
- blank paper → "unlabeled"

### 4.3 戒指材质 (28 种)

wooden, granite, opal, clay, coral, black onyx, moonstone, tiger eye, jade, bronze, agate, topaz, sapphire, ruby, diamond, pearl, iron, brass, copper, twisted, steel, silver, gold, ivory, emerald, wire, engagement, shiny

### 4.4 魔杖材质 (27 种，含 3 种额外)

glass, balsa, crystal, maple, pine, oak, ebony, marble, tin, brass, copper, silver, platinum, iridium, zinc, aluminum, uranium, iron, steel, hexagonal, short, runed, long, curved, forked, spiked, jeweled

### 4.5 魔法书封面 (41 种描述对应 41 种魔法书，参与打乱)

parchment, vellum, ragged, dog eared, mottled, stained, cloth, leathery, white, pink, red, orange, yellow, velvet, light green, dark green, turquoise, cyan, light blue, dark blue, indigo, magenta, purple, violet, tan, plaid, light brown, dark brown, gray, wrinkled, dusty, bronze, copper, silver, gold, glittering, shining, dull, thin, thick, checkered

**固定描述** (不参与打乱):
- blank paper → "plain"
- novel → "paperback"
- Book of the Dead → "papyrus"

### 4.6 护符外观 (11 种参与打乱)

circular, spherical, oval, triangular, pyramidal, square, concave, hexagonal, octagonal, perforated, cubical

**固定描述**:
- cheap plastic imitation of the Amulet of Yendor → "Amulet of Yendor"
- Amulet of Yendor → "Amulet of Yendor"

### 4.7 头盔外观 (4 种参与打乱)

打乱范围: HELMET ~ HELM_OF_TELEPATHY（4 种）:
- plumed helmet, etched helmet, crested helmet, visored helmet

**不参与打乱的头盔**（描述固定）:
- elven leather helm → "leather hat"
- orcish helm → "iron skull cap"
- dwarvish iron helm → "hard hat"
- fedora → (无描述, `oc_name_known=1` 预设)
- cornuthaum → "conical hat" (固定)
- dunce cap → "conical hat" (固定，与 cornuthaum 共享)
- dented pot → (无描述, `oc_name_known=1` 预设)
- helm of brilliance → "crystal helmet" (固定，在打乱范围之前)

**设计要点**: cornuthaum 和 dunce cap 共享固定描述 "conical hat"，两者不参与打乱。这意味着 "conical hat" **每一局**都指 cornuthaum 或 dunce cap 之一，无法仅通过外观区分。但价格差异极大 (cornuthaum oc_cost=80, dunce cap oc_cost=1) 可通过商店价格区分。helm of brilliance 的 "crystal helmet" 也是固定的，不参与打乱。

### 4.8 手套外观 (4 种参与打乱)

old gloves, padded gloves, riding gloves, fencing gloves

### 4.9 披风外观 (4 种参与打乱)

tattered cape, opera cloak, ornamental cope, piece of cloth

**不参与打乱的披风**:
- mummy wrapping (无描述)
- elven cloak → "faded pall"
- orcish cloak → "coarse mantelet"
- dwarvish cloak → "hooded cloak"
- oilskin cloak → "slippery cloak"
- robe (无描述)
- alchemy smock → "apron"
- leather cloak (无描述)

### 4.10 靴子外观 (7 种参与打乱)

打乱范围: SPEED_BOOTS ~ LEVITATION_BOOTS（7 种）:
- combat boots, jungle boots, hiking boots, mud boots, buckled boots, riding boots, snow boots

**不参与打乱的靴子**（描述固定）:
- low boots → "walking shoes"
- iron shoes → "hard shoes"
- high boots → "jackboots"

---

## 5. 自动鉴定规则

### 5.1 外观自动设置 (`dknown`)

```
FUNCTION observe_object(obj):
    IF obj->otyp >= FIRST_OBJECT AND NOT Hallucination:
        obj->dknown = 1
        discover_object(oindx, FALSE, TRUE, FALSE)  // 标记 encountered，不标记 known
```

**调用时机**: `xname_flags()` 在非盲且非远距离命名时自动调用。

### 5.2 BUC 自动设置 (`bknown`)

| 触发条件 | 来源文件 |
|----------|----------|
| 牧师(Cleric)角色 — 看到物品时自动设 bknown=1 | `objnam.c:644` |
| 祭坛上掉落物品 | `pray.c:366` |
| 祈祷效果（移除诅咒等） | `pray.c` 多处 |
| 穿戴诅咒物品发现无法脱下 | `do_wear.c` 多处 |
| 吃下诅咒戒指（焊死在手上） | `eat.c:2895` |
| 宠物拾起/放下物品 | `pickup.c:539`, `pickup.c:2572` |
| 初始装备配置文件标记 | `files.c:2523` |
| 使用涂油罐/乐器等 | `apply.c` 多处 |
| 学习法术（看到魔法书） | `spell.c:348` |

### 5.3 使用后自动鉴定类型 (`oc_name_known`)

#### 药水

- 大部分药水在产生可观察效果后通过 `makeknown()` 被鉴定
- 特例: 普通水 (POT_WATER) 在被祝福/诅咒时通过 `makeknown(POT_WATER)` 鉴定
- 变形药水在使用后被鉴定

#### 卷轴

- 卷轴在读取后通过 `learnscrolltyp()` 被鉴定
- `learnscrolltyp()` 内部调用 `makeknown(scrolltyp)`
- 特例: 鉴定卷轴在迷惑或诅咒+未知时只鉴定自身类型

#### 魔杖

- `learnwand(obj)` 在产生可观察效果后调用:
  ```
  IF 类型已经发现: observe_object(obj)  // 设置 dknown
  ELSE:
      IF 非盲: observe_object(obj)
      IF obj->dknown: makeknown(obj->otyp)
  update_inventory()
  ```
- 重点: 盲目时使用魔杖不会自动鉴定类型，除非 `dknown` 已经被设置

#### 护甲

穿戴后产生效果时被鉴定（`do_wear.c`）:
- 速度靴: 穿上后 `makeknown(uarmf->otyp)`
- 水行靴: 踏入水中时 `makeknown(WATER_WALKING_BOOTS)`
- 跳跃靴、精灵靴、踢击靴: 效果触发时鉴定
- 笨拙靴: 触发效果时鉴定
- 悬浮靴: 效果触发时鉴定
- 披风类: 效果触发时鉴定（保护、隐身、魔法抗性、位移）
- 头盔类: 效果触发时鉴定
- 手套类: 效果触发时鉴定

#### 护符

穿戴后产生效果时被鉴定:
- 窒息护符: 开始窒息时
- 变化护符: 变性时
- 飞行护符: 获得飞行时
- 守卫护符: 穿戴时
- 魔法呼吸护符: 在水下存活时
- 等等

#### 戒指

- 多数戒指在穿戴时产生效果后被鉴定: `makeknown(ringtype)`

---

## 6. 鉴定卷轴/法术的效果

### 6.1 核心逻辑 (`seffect_identify()`)

```
FUNCTION seffect_identify(sobjp):
    is_scroll = (sobj 是卷轴类)
    already_known = (是法术书 OR objects[otyp].oc_name_known)

    IF is_scroll:
        useup(sobj)  // 先消耗卷轴

        // 迷惑 OR (诅咒 AND 未知类型) → 只鉴定卷轴自身
        IF confused OR (cursed AND NOT already_known):
            "You identify this as an identify scroll."
        ELSE IF NOT already_known:
            "This is an identify scroll."

        IF NOT already_known:
            learnscrolltyp(SCR_IDENTIFY)  // 发现鉴定卷轴类型

        IF confused OR (cursed AND NOT already_known):
            RETURN  // 不鉴定其他物品

    IF 有库存物品:
        cval = 1  // 默认鉴定 1 个

        IF blessed OR (NOT cursed AND rn2(5)==0):
            cval = rn2(5)  // 0~4，其中 0 = 鉴定全部
            IF cval == 1 AND blessed AND Luck > 0:
                cval = 2  // 祝福 + 正幸运 → 至少 2 个

        identify_pack(cval, !already_known)
    ELSE:
        "You're not carrying anything to be identified."
```

### 6.2 鉴定数量总结

| 条件 | 鉴定数量 |
|------|----------|
| 迷惑 (任何 BUC) | 0（只鉴定卷轴自身） |
| 诅咒 + 类型未知 | 0（只鉴定卷轴自身） |
| 诅咒 + 类型已知 | 1 |
| 无诅咒 | 1（80%概率）或 rn2(5) 即 0~4（20%概率）|
| 祝福 | rn2(5) 即 0~4 |
| 祝福 + Luck > 0 + rn2(5)==1 | 修正为 2 |
| cval == 0 | 鉴定全部库存 |

**鉴定全部的概率**:
- 祝福: 1/5 = 20%
- 无诅咒: 1/5 * 1/5 = 4%
- 诅咒(类型已知): 0%

### 6.3 `identify_pack()` 流程

```
FUNCTION identify_pack(id_limit, learning_id):
    unid_cnt = count_unidentified(invent)

    IF unid_cnt == 0:
        "You have already identified all/the rest of your possessions."
    ELSE IF id_limit == 0 OR id_limit >= unid_cnt:
        // 鉴定全部
        FOR 每个库存物品:
            IF not_fully_identified(obj): identify(obj)
    ELSE:
        // 让玩家选择鉴定哪些（菜单或传统模式）
        menu_identify(id_limit)  // 或 ggetobj("identify", ...)
```

---

## 7. 价格鉴定系统

### 7.1 基础价格 (`getprice()`)

```
FUNCTION getprice(obj, shk_buying) -> long:
    tmp = objects[obj->otyp].oc_cost  // 基础价格

    IF obj->oartifact:
        tmp = arti_cost(obj)
        IF shk_buying: tmp /= 4

    SWITCH obj->oclass:
        FOOD_CLASS:
            tmp += corpsenm_price_adj(obj)
            IF 英雄饥饿(u.uhs >= HUNGRY) AND NOT shk_buying:
                tmp *= u.uhs  // 2~4 倍
            IF obj->oeaten: tmp = 0
        WAND_CLASS:
            IF obj->spe == -1: tmp = 0  // 已耗尽
        POTION_CLASS:
            IF 是普通水(非祝福非诅咒): tmp = 0
        ARMOR_CLASS, WEAPON_CLASS:
            IF obj->spe > 0: tmp += 10 * obj->spe
        TOOL_CLASS:
            IF 是蜡烛 AND 部分使用: tmp /= 2

    RETURN tmp
```

### 7.2 商店售价 (`get_cost()`)

```
FUNCTION get_cost(obj, shkp) -> long:
    tmp = getprice(obj, FALSE)
    IF tmp == 0: tmp = 5

    multiplier = 1, divisor = 1

    // 未鉴定加价
    IF NOT (obj->dknown AND oc_name_known):
        IF 玻璃宝石:
            // 伪装为真宝石价格
            用 pseudorand = ((ubirthday % otyp) >= otyp / 2) 决定选哪个真宝石
            tmp = 对应真宝石的 oc_cost
        ELSE IF oid_price_adjustment(obj, obj->o_id) > 0:
            multiplier *= 4, divisor *= 3  // 加价 33%

    // 愚蠢/游客加价
    IF 戴着 dunce cap:
        multiplier *= 4, divisor *= 3
    ELSE IF (游客角色 AND 等级 < MAXULEV/2) OR (内衣可见):
        multiplier *= 4, divisor *= 3

    // 魅力调整
    IF CHA > 18:  divisor *= 2          // -50%
    ELSE IF CHA == 18: mult *= 2, div *= 3  // -33%
    ELSE IF CHA >= 16: mult *= 3, div *= 4  // -25%
    ELSE IF CHA <= 5:  multiplier *= 2      // +100%
    ELSE IF CHA <= 7:  mult *= 3, div *= 2  // +50%
    ELSE IF CHA <= 10: mult *= 4, div *= 3  // +33%

    tmp = tmp * multiplier
    IF divisor > 1:
        tmp = ((tmp * 10) / divisor + 5) / 10  // 四舍五入

    IF tmp <= 0: tmp = 1
    IF obj->oartifact: tmp *= 4  // 神器4倍
    IF shkp 愤怒: tmp += (tmp + 2) / 3  // 额外 33%

    RETURN tmp
```

### 7.3 `oid_price_adjustment()` — 基于物品 ID 的价格波动

```
FUNCTION oid_price_adjustment(obj, oid) -> int:
    IF NOT (dknown AND oc_name_known)
       AND NOT (宝石类 AND 玻璃材质):
        RETURN (oid % 4 == 0) ? 1 : 0
        // 25% 的未鉴定物品会被加价
    RETURN 0
```

### 7.4 价格鉴定的实践意义

玩家可以通过观察商店中物品的标价来推断其类型：

1. 商品标价 = `get_cost()` 的结果
2. 由于魅力、角色、饥饿等修正，需要先确定自己的价格系数
3. 基础价格 (`oc_cost`) 可以将同类物品分组
4. 未鉴定物品有 25% 概率被加价 33%，所以同一 `oc_cost` 的物品可能显示两种不同价格
5. 玻璃宝石会被标价为真宝石的价格（具体哪个真宝石取决于游戏种子）

---

## 8. `/` 命令 (whatis) 和 `;` 命令 (far_look) 的行为

### 8.1 `/` 命令 (`dowhatis()`)

调用 `do_look(0, NULL)`，提供以下选项:
- 从屏幕选择位置查看
- 输入符号查询
- 其他查询方式

### 8.2 `;` 命令 (`doquickwhatis()`)

调用 `do_look(1, NULL)`，快速模式——直接使用光标选择，不搜索数据库中的补充信息。

### 8.3 查看地面物品 (`look_at_object()`)

```
FUNCTION look_at_object(buf, x, y, glyph):
    otmp = object_from_map(glyph, x, y)  // 从地图图形恢复物品

    IF otmp:
        IF otmp->dknown:
            buf = distant_name(otmp, doname_with_price)  // 包含价格
        ELSE:
            buf = distant_name(otmp, doname_vague_quan)  // 模糊数量

    // 附加位置描述: "(buried)", "stuck in a tree", "embedded in stone" 等
```

**关键**: `distant_name()` 不会设置 `dknown`（与 `xname()` 不同），所以远距离查看不会改变物品的鉴定状态。但有距离阈值——近距离时仍会设置。

```
FUNCTION distant_name(obj, func):
    r = max(u.xray_range, 2)
    neardist = r*r*2 - r  // 近距离阈值

    // 获取物品位置
    IF 物品在地面 AND distu(ox, oy) > neardist:
        gd.distantname = TRUE  // 设置标志防止 observe_object 被调用

    str = func(obj)  // 调用格式化函数
    gd.distantname = FALSE
    RETURN str
```

### 8.4 `doname_with_price` vs `doname_vague_quan`

- `doname_with_price`: 商店物品显示价格标签 "(for sale, 200 zorkmids)"
- `doname_vague_quan`: 若 `dknown` 未设置，用 "some" 代替精确数量

---

## 9. "名为(named)" 和 "叫做(called)" 的区别

### 9.1 "named" — 个体命名 (`do_oname()`)

- 通过 `#name` → `i` (name individual item) 设置
- 存储在 `obj->oextra->oname` 中（字符串）
- 是**个体实例**的属性，不影响同类其他物品
- 显示格式: `<物品描述> named <名称>`
- 若物品被命名为有效的神器名称且类型匹配，物品会变成神器
- 已有名称的神器不能被重新命名

### 9.2 "called" — 类型命名 (`docall()`)

- 通过 `#name` → `o` (call type of object) 设置
- 存储在 `objects[otyp].oc_uname` 中（字符串指针）
- 是**整个类型**的属性，影响所有同类物品
- 显示格式: `<物品类别> called <名称>` (例如 "scroll called light")
- 前提: `dknown` 必须已设置（否则 "You would never recognize another one."）
- 只有未鉴定 (`oc_name_known == 0`) 的类型才会显示 "called" 名称；鉴定后显示真实名称

### 9.3 显示优先级

在 `xname_flags()` 的 switch 分支中，各类物品的显示逻辑遵循统一模式:

```
IF obj_is_pname(obj):  // 已鉴定的神器 → 直接用名称
    GOTO nameit

SWITCH (oclass):
    // 以药水为例:
    IF NOT dknown:      "potion"           // 完全未知
    ELSE IF nn:         "potion of <真名>"  // 类型已鉴定
    ELSE IF un:         "potion called <user名>" // 玩家命名
    ELSE:               "<颜色> potion"     // 只知道外观

// 在最后:
IF has_oname(obj) AND dknown:
    追加 " named <obj名>"
```

完整优先级链:
1. `obj_is_pname` → 个人名称（已鉴定神器）
2. `!dknown` → 只显示类别名（"potion", "scroll", "wand", etc.）
3. `oc_name_known (nn)` → 显示真实名称
4. `oc_uname (un)` → 显示 "called <用户名称>"
5. 默认 → 显示外观描述
6. 如果有 `oname` 且 `dknown` → 追加 "named <名称>"

---

## 10. 神器物品的特殊鉴定规则

### 10.1 `obj_is_pname()` — 神器是否显示个人名称

```
FUNCTION obj_is_pname(obj) -> boolean:
    IF NOT obj->oartifact OR NOT has_oname(obj):
        RETURN FALSE
    IF NOT gameover AND NOT override_ID:
        IF not_fully_identified(obj):
            RETURN FALSE  // 未完全鉴定的神器不用个人名称
    RETURN TRUE
```

这意味着:
- 未鉴定的神器显示为其基础物品类型的外观描述（如未知的 Excalibur 显示为 "a long sword"，因为长剑没有外观描述，`oc_name_known` 预设为 1）
- 但如果基础类型需要鉴定（如魔杖类神器），则显示打乱后的外观

### 10.2 `find_artifact()` — 发现神器

当 `xname_flags()` 格式化一个 `dknown` 的神器时，调用 `find_artifact(obj)`:
```
FUNCTION find_artifact(otmp):
    IF otmp->oartifact AND NOT artiexist[a].found:
        found_artifact(a)  // 标记为已发现
        livelog事件
```

这仅在物品被看到（`dknown` 设置）时触发，用于统计和 livelog。

### 10.3 `discover_artifact()` — 神器进入发现列表

```
FUNCTION discover_artifact(m):
    FOR i IN [0..NROFARTIFACTS):
        IF artidisco[i] == 0 OR artidisco[i] == m:
            artidisco[i] = m
            RETURN
```

仅通过 `fully_identify_obj()` 调用（即使用鉴定卷轴/法术时）。

### 10.4 `undiscovered_artifact()` — 神器是否未被发现

```
FUNCTION undiscovered_artifact(m) -> boolean:
    FOR i IN [0..NROFARTIFACTS):
        IF artidisco[i] == m: RETURN FALSE
        IF artidisco[i] == 0: BREAK
    RETURN TRUE
```

影响 `not_fully_identified()` — 若神器未在发现列表中，则视为未完全鉴定。

### 10.5 Amulet of Yendor 特殊规则

- 真 Amulet of Yendor 和 "cheap plastic imitation" 共享 **相同的外观描述** "Amulet of Yendor"
- 两者不参与打乱（固定描述）
- 未鉴定时两者看起来完全相同
- `the_unique_obj()` 对未鉴定的 fake amulet 返回 TRUE（故意撒谎，显示为 "the Amulet of Yendor"）
- 只有 `known` 标志能区分真假

---

## 11. 特殊鉴定机制

### 11.1 魔杖的特殊鉴定 (`learnwand()`)

```
FUNCTION learnwand(obj):
    IF obj->oclass == SPBOOK_CLASS: RETURN  // 抑制法术重新发现已遗忘的书

    IF objects[obj->otyp].oc_name_known:
        observe_object(obj)  // 类型已知，确保 dknown
    ELSE:
        IF NOT Blind: observe_object(obj)
        IF obj->dknown: makeknown(obj->otyp)  // 盲时不鉴定
    update_inventory()
```

### 11.2 nothing 魔杖的方向随机化

在 `init_objects()` 最后:
```
objects[WAN_NOTHING].oc_dir = rn2(2) ? NODIR : IMMEDIATE
```

这意味着 wand of nothing 每局随机为无方向型或立即型，进一步增加鉴定难度。

### 11.3 鉴定失明规则

- `observe_object()` 只在非幻觉时设置 `dknown`
- 失明时 `xname()` 中 `!Blind && !gd.distantname` 条件阻止调用 `observe_object()`
- 结果: 失明时拾取的物品 `dknown=0`，恢复视力后通过 `learn_unseen_invent()` 补设

### 11.4 `learn_unseen_invent()` — 恢复视力后更新库存

```
FUNCTION learn_unseen_invent():
    IF Blind: RETURN
    FOR 每个库存物品:
        IF 已 dknown (牧师也检查 bknown): CONTINUE
        调用 xname(otmp)  // 这会触发 observe_object，设置 dknown
        调用 addinv_core2(otmp)  // 触发拾取反应
```

---

## 12. 祭坛鉴定 (BUC 测试)

### 12.1 `doaltarobj()` — 在祭坛上掉落物品

当物品掉落或被怪物丢弃到祭坛上时，触发 BUC 检测（`src/do.c:362`）:

```
FUNCTION doaltarobj(obj):
    IF Blind: RETURN  // 盲目时无任何效果

    IF obj 不是金币:
        记录破戒行为（gnostic conduct）

    // 金币特殊处理：清除 BUC 状态
    IF obj 是金币:
        obj->blessed = 0
        obj->cursed = 0

    IF obj->blessed OR obj->cursed:
        "There is <an amber/a black> flash as <物品> hits the altar."
        //   blessed → amber flash
        //   cursed  → black flash
        IF NOT Hallucination:
            obj->bknown = 1
    ELSE:  // uncursed
        "<物品> lands on the altar."
        IF obj 不是金币:
            obj->bknown = 1  // 无闪光 = 无诅咒
```

### 12.2 BUC 闪光颜色

| BUC 状态 | 闪光颜色 | 消息 |
|----------|----------|------|
| blessed | amber | "There is an amber flash as ..." |
| cursed | black | "There is a black flash as ..." |
| uncursed | (无闪光) | "<物品> lands on the altar." |

**关键规则**:
- 幻觉(Hallucination)下看到闪光但**不设置 `bknown`**（颜色被替换为随机颜色，无法判断真实 BUC）
- 盲目(Blind)时完全跳过，**不设置 `bknown`**，也没有消息
- 金币掉在祭坛上会被强制设为 uncursed（清除 BUC 状态），且不设置 bknown
- 怪物丢物品到祭坛上也会触发（`svc.context.mon_moving` 时）
- `bknown` 通过直接赋值设置（绕过 `set_bknown()` 以避免触发 `update_inventory()`）

### 12.3 祭坛 BUC 测试的信息泄露

[疑似 bug] 当 `obj->blessed=1` 且幻觉状态时，消息仍然是 "There is <a random color> flash as..."。闪光的**存在与否**本身就泄露了信息——无闪光意味着 uncursed，有闪光意味着 blessed 或 cursed。幻觉只阻止了颜色识别（amber vs black），但不阻止有无闪光的二元判断。代码只在颜色判断层面禁用了 `bknown`，没有在闪光存在层面做处理。

---

## 13. 刻字鉴定 (魔杖效果)

### 13.1 总览

尝试用魔杖在地面刻字（`E` 命令 + 选择魔杖）会产生不同效果，这些效果可用来推断魔杖类型。代码在 `src/engrave.c` 中。

### 13.2 各魔杖的刻字效果

#### NODIR 型（无方向）

| 魔杖 | 效果 | 可观察现象 |
|------|------|-----------|
| WAN_LIGHT | 正常施放效果 (`zapnodir`) | 区域变亮 |
| WAN_SECRET_DOOR_DETECTION | 正常施放效果 | 发现秘门 |
| WAN_CREATE_MONSTER | 正常施放效果 | 出现怪物 |
| WAN_WISHING | 正常施放效果 | 许愿提示 |
| WAN_ENLIGHTENMENT | 正常施放效果 | 显示属性信息 |

#### IMMEDIATE 型（立即）

| 魔杖 | 效果 | 消息 |
|------|------|------|
| WAN_STRIKING | 文本不产生 | "The wand unsuccessfully fights your attempt to write!" |
| WAN_SLOW_MONSTER | 无刻字效果 | "The bugs on the <surface> slow down!" (非盲) |
| WAN_SPEED_MONSTER | 无刻字效果 | "The bugs on the <surface> speed up!" (非盲) |
| WAN_POLYMORPH | 已有刻字被变形 | 刻字文本被随机字符替换（非盲）；若无刻字则删除 |
| WAN_NOTHING | 无效果 | 无特殊消息 |
| WAN_UNDEAD_TURNING | 无效果 | 无特殊消息 |
| WAN_OPENING | 无效果 | 无特殊消息 |
| WAN_LOCKING | 无效果 | 无特殊消息 |
| WAN_PROBING | 无效果 | 无特殊消息 |
| WAN_CANCELLATION | 已有刻字被消除 | "The engraving on the <surface> vanishes!" (非盲) |
| WAN_MAKE_INVISIBLE | 已有刻字被消除 | "The engraving on the <surface> vanishes!" (非盲) |
| WAN_TELEPORTATION | 已有刻字被移除 | "The engraving on the <surface> vanishes!" (非盲) |
| WAN_SLEEP / WAN_DEATH | 无刻字效果 | "The bugs on the <surface> stop moving!" (非盲) |

#### ENGRAVE 型（刻入地面）

| 魔杖 | 类型 | 自动鉴定 | 消息 |
|------|------|----------|------|
| WAN_DIGGING | ENGRAVE | 是 | "This <wand> is a wand of digging!" + "Gravel flies up from the floor." |

#### BURN 型（烧入地面）

| 魔杖 | 类型 | 自动鉴定 | 消息 |
|------|------|----------|------|
| WAN_FIRE | BURN | 是 | "This <wand> is a wand of fire!" + "Flames fly from the wand." |
| WAN_LIGHTNING | BURN | 是 | "This <wand> is a wand of lightning!" + "Lightning arcs from the wand." |

#### MARK 型（标记，液体）

| 魔杖 | 类型 | 消息 |
|------|------|------|
| WAN_COLD | MARK (但未写入成功) | "A few ice cubes drop from the wand." (非盲) |

**注意**: WAN_COLD 如果有已有刻字且非墓碑，其行为同 WAN_CANCELLATION（消除刻字）。

### 13.3 自动鉴定逻辑

刻字过程中仅三根魔杖会自动鉴定（`de->doknown = TRUE`）:
- WAN_DIGGING: 总是产生可识别的刻入效果
- WAN_FIRE: 总是产生可识别的烧灼效果
- WAN_LIGHTNING: 总是产生可识别的烧灼效果

这些魔杖在使用时如果 `oc_name_known == 0`，会打印 "This <wand description> is a wand of <type>!" 并设置 `doknown=TRUE`，后续代码调用 `makeknown(otmp->otyp)` 完成类型发现。

### 13.4 刻字鉴定的推理逻辑

由于大多数魔杖不会自动鉴定，玩家需要通过**效果模式**来推理：

```
IF 产生 ENGRAVE 效果:          → wand of digging
IF 产生 BURN 效果 + 火焰:      → wand of fire
IF 产生 BURN 效果 + 闪电:      → wand of lightning
IF 产生冰块:                    → wand of cold
IF 虫子减速:                    → wand of slow monster
IF 虫子加速:                    → wand of speed monster
IF 虫子停止移动:                → wand of sleep 或 wand of death（不可区分）
IF 刻字被打乱:                  → wand of polymorph
IF 刻字消失 + 有旧刻字:        → wand of cancellation 或 make invisible 或 teleportation
IF "fights your attempt":       → wand of striking
IF 无任何效果:                  → wand of nothing/opening/locking/probing/undead turning
```

[疑似 bug] sleep 和 death 的刻字效果完全相同（"The bugs on the <surface> stop moving!"），代码中有注释 "can't tell sleep from death - Eric Backus"。这是有意的设计决策而非 bug，但在功能规格中值得标注。

---

## 14. 非正式鉴定方法

### 14.1 重量鉴定 (灰石)

灰色石头 (`gray stone`) 有四种：loadstone、flintstone、luckstone、touchstone。它们共享外观 "gray stone" 但重量不同：

| 石头 | 重量 (oc_weight) | oc_cost |
|------|-----------------|---------|
| loadstone | 500 | 1 |
| flintstone | 10 | 1 |
| luckstone | 10 | 60 |
| touchstone | 10 | 45 |

**鉴定方法**:
- **重量**: loadstone 重 500 cn，其他三种均重 10 cn。拾取后立即检查负重变化可区分 loadstone
- **放不下**: loadstone 如果诅咒（几乎总是），无法丢弃——尝试丢弃会 "You don't want to drop the loadstone"（已知时）或类似消息
- **价格**: flintstone 的 oc_cost=1，与 loadstone 相同；但 luckstone=60, touchstone=45，可在商店区分
- **踢击**: 踢灰石时，loadstone "thud" 声（重物撞击），其他为正常声

### 14.2 宝石鉴定（宝石类）

宝石可通过以下非正式方法鉴定：

- **touchstone (试金石)**: 未打磨的宝石在 touchstone 上划痕会显示 "You see <color> streaks on the stone"；玻璃石头只产生 "You make scratches on the stone"
- **向珠宝店掷宝石**: shopkeeper 会鉴定并告知真实名称 — `gem_accept()` 函数
- **估价卷轴**: 可鉴定宝石的真假

### 14.3 探测魔杖/听诊器 (Probing)

使用 wand of probing 或 stethoscope 对物品/怪物可获取详细信息：

**对怪物**（`mstatusline()`, `src/insight.c:3295`）:
```
显示内容:
- 怪物名称
- HP: "HP:当前/最大"
- 等级和 AC
- 状态标志: tame, peaceful, confused, blind, stunned, asleep, can't move,
            scared, trapped, fast/slow, invisible, shapechanger 等
- 隐藏/伪装状态
- 库存物品列表
```

**对自身**（`ustatusline()`）:
- 显示自身 HP、属性、等级、AC 等信息

### 14.4 宠物拾取行为推断 BUC

宠物有选择性地拾取物品：
- 宠物倾向于不拾取诅咒物品
- 宠物拾取后**放下**的物品会被标记 `bknown=1`（因为宠物行为暗示了 BUC 状态）
- 具体规则: 宠物从地面拾起物品后立即放下它（或拒绝拾取）→ 该物品被视为诅咒

### 14.5 水槽鉴定 (戒指掉入水槽)

脱下戒指时若在水槽上方，戒指可能掉入水槽并产生效果：
- 每种戒指掉入水槽后产生不同效果（闪光、怪物出现等）
- 效果后触发 `trycall()` 提示玩家给戒指类型命名
- 注意: 戒指会永久消失（除非有管道工具取回）

### 14.6 药水浸泡鉴定 (Alchemy)

将物品浸入药水、混合药水、将药水浸入泉水等操作会根据结果间接揭示药水类型。

### 14.7 卷轴使用效果推断

即使卷轴使用后不自动 `makeknown()`，效果本身允许推断：
- 例如: 读一张卷轴后传送了 → 可以用 `#name` 将该卷轴外观标记为 "teleport"
- `trycall(obj)` 在效果不明确时自动提示玩家命名

---

## 15. 发现列表 (`\` 命令)

### 15.1 `dodiscovered()` — 完整发现列表

`\` 命令调用 `dodiscovered()` (`src/o_init.c:756`)，显示所有已发现/已遇到的物品类型。

**数据来源**: `svd.disco[]` 数组——按类别存储已发现物品的 otyp。

**列表内容**:
1. **独特物品/圣物 (Unique items or Relics)**:
   - Amulet of Yendor, Bell of Opening, Book of the Dead, Candelabrum of Invocation
   - 仅在 `oc_name_known=1` 或 (`oc_encountered=1` 且不是 Amulet of Yendor) 时显示
2. **已发现神器**: 通过 `disp_artifact_discoveries()` 显示
3. **各物品类别**: 按 `flags.inv_order` 遍历，对每类遍历 `disco[]`

**显示格式**: 每行一个类型，使用 `disco_typename()` 格式化：
```
"* " 或 "  "   // "*" = 发现但未亲眼见过（oc_name_known=1 但 oc_encountered=0）
+  obj_typename(otyp)  // 包含真名（如已知）、外观描述、用户命名
```

**排序选项** (`flags.discosort`):
- `'o'`: 按发现顺序（默认）
- `'s'`: sortloot 顺序（按子类分组）
- `'c'`: 按类别内字母排序
- `'a'`: 跨类别字母排序

### 15.2 `doclassdisco()` — 单类发现列表

`` ` `` (反引号) 命令调用 `doclassdisco()`，先让玩家选择物品类别，然后显示该类已发现的物品。

额外支持伪类别:
- `'a'`: 已发现的神器
- `'u'`/`'r'`: 独特物品/圣物

### 15.3 `interesting_to_discover()` — 判断是否应出现在发现列表中

```
FUNCTION interesting_to_discover(i) -> boolean:
    // 武士角色的日语物品名总是显示
    IF Role_if(PM_SAMURAI) AND Japanese_item_name(i, NULL):
        RETURN TRUE

    // 有用户命名，或者已知/已遇到且有外观描述
    RETURN (oc_uname != NULL)
           OR ((oc_name_known OR oc_encountered) AND OBJ_DESCR != NULL)
```

**关键**: 没有外观描述（`OBJ_DESCR == NULL`）的物品即使被发现也不出现在发现列表中（因为发现列表的目的是将外观描述与真名对应起来）。

### 15.4 `oc_encountered` vs `oc_name_known`

| 标志 | 含义 | 设置时机 | 发现列表中的显示 |
|------|------|----------|-----------------|
| `oc_encountered=1, oc_name_known=0` | 见过但不知道是什么 | `observe_object()` | 只显示外观描述 + 用户命名（如有） |
| `oc_encountered=1, oc_name_known=1` | 见过且已鉴定 | `makeknown()` 等 | 显示 "真名 (外观描述)" |
| `oc_encountered=0, oc_name_known=1` | 未见过但已鉴定（如通过鉴定卷轴鉴定商店物品后离开）| `makeknown()` | 标记 `*`，显示 "真名 (外观描述)" |

---

## 16. 查看命令详解 (Far-look vs Here-look)

### 16.1 命令入口

| 命令 | 函数 | 模式 | 说明 |
|------|------|------|------|
| `/` | `dowhatis()` → `do_look(0, NULL)` | 完整模式 | 提供菜单选择查看方式；找到匹配后搜索 data.base 百科全书 |
| `;` | `doquickwhatis()` → `do_look(1, NULL)` | 快速模式 | 直接用光标选择，不搜索百科全书 |
| 右键 | `do_look(2, &cc)` | 点击模式 | 直接查看指定坐标 |

### 16.2 `/` 命令的菜单选项

```
/  something on the map          → 光标选择地图位置
i  something you're carrying     → 从库存选择物品，查百科全书
?  something else                → 输入文本查百科全书

-- 以下仅在非吞噬、非幻觉时可用 --
m  nearby monsters               → 列出附近 (BOLT_LIM 范围) 怪物
M  all monsters shown on map     → 列出地图上所有可见怪物
o  nearby objects                → 列出附近物品
O  all objects shown on map      → 列出地图上所有可见物品
t  nearby traps                  → 列出附近陷阱
T  all seen or remembered traps  → 列出所有已知陷阱
e  nearby engravings             → 列出附近刻字
E  all seen or remembered engravings → 列出所有已知刻字
```

### 16.3 `lookat()` — 查看指定坐标的核心函数

```
FUNCTION lookat(x, y, buf, monbuf) -> struct permonst*:
    glyph = glyph_at(x, y)

    IF 在英雄位置 AND canspotself():
        self_lookat(buf)  // 显示自身信息
        // 如果不可见，附加检测方式说明 [seen: infravision, telepathy, ...]
    ELSE IF 吞噬状态:
        "interior of <怪物名>"
    ELSE IF glyph 是怪物:
        look_at_monster(buf, monbuf, mtmp, x, y)
        // 显示: [tame|peaceful] <怪物名> [状态: sleeping, trapped, leashed, ...]
        // monbuf: 检测方式 (normal vision, see invisible, telepathy, ...)
    ELSE IF glyph 是物品:
        look_at_object(buf, x, y, glyph)
        // 显示: distant_name(物品) + 位置描述
    ELSE IF glyph 是陷阱:
        trap_description(buf, tnum, x, y)
    ELSE IF glyph 是警告:
        def_warnsyms[warnindx].explanation
    ELSE IF glyph 是隐形:
        "remembered, unseen, creature"
    ELSE IF glyph 是 nothing:
        "dark part of a room"
    ELSE IF glyph 是未探索:
        "unexplored area" (水下时 "land" 或 "unknown")
    ELSE IF glyph 是 cmap:
        根据具体 symidx 返回描述:
        - S_altar: "<alignment> [high ]altar"
        - S_ndoor: "doorway" / "broken door" / "open drawbridge portcullis"
        - S_pool/water/lava/ice: waterbody_name()
        - S_engroom/S_engrcorr: "engraving"
        - S_stone: "stone" / "unexplored"
        - 其他: defsyms[symidx].explanation
```

### 16.4 Far-look (远距离查看) 的信息限制

**`distant_name()` 的距离阈值**:

```
r = max(u.xray_range, 2)
neardist = r*r*2 - r
// 默认 r=2: neardist = 4*2 - 2 = 6
// 意味着 distu(x,y) <= 6 时视为"近距离"
```

| distu 值 | 相对位置 | 近/远 |
|----------|---------|-------|
| 1 | 正交相邻 | 近 |
| 2 | 对角相邻 | 近 |
| 4 | 骑士步（2+0） | 近 |
| 5 | 骑士步（2+1） | 近 |
| 8 | 两步对角 | 远 |
| 9 | 三步正交 | 远 |

**近距离时**:
- `object_from_map()` 会对相邻物品调用 `observe_object()` → 设置 `dknown`
- `distant_name()` 中 `gd.distantname` 为 FALSE，允许 `xname()` 内的 `observe_object()` 正常工作
- 效果: 近距离查看物品会**改变**其鉴定状态（设置 dknown）

**远距离时**:
- `gd.distantname` 为 TRUE，`xname()` 中的 `observe_object()` 被跳过
- `dknown` 不被设置
- 如果 `dknown=0`: 显示模糊信息（用 "some" 代替数量，不显示外观描述）
- 效果: 远距离查看**不改变**鉴定状态

### 16.5 Here-look (`:` 脚下查看) vs Far-look

`:` 命令查看脚下物品，调用链不同于 `/`:

- `:` 直接查看 `(u.ux, u.uy)`，不经过 `do_look()`
- 脚下物品总是 `distu=0`（近距离），所以 `dknown` 总会被设置
- `:` 显示的信息量与 `/` 相同，但总是近距离版本

### 16.6 查看后的百科全书查询

`/` 命令（非快速模式）在显示描述后，会调用 `checkfile()` 在 `data.base` 文件中查找匹配条目:

```
FUNCTION checkfile(inp, pm, chkflags, supplemental_name):
    // 打开 DATAFILE ("data")
    // 预处理 inp: 去除前缀 (a, an, the, some, 数字, tame, peaceful,
    //              invisible, blessed, uncursed, cursed, empty, partly eaten, ...)
    // 去除附魔值 (+N, -N)
    // 去除 "statue of" → "statue", "figurine of" → "figurine"
    // 在 data.base 中查找匹配条目
    // 找到则显示百科全书文本
```

**注意**: 快速模式 (`;`) 和 `LOOK_ONCE` 模式不进行百科全书查询。

---

## 17. 怪物鉴定

### 17.1 查看怪物 (`look_at_monster()`)

在查看命令中选中怪物时，显示以下信息（`src/pager.c:422`）:

```
显示格式:
"[tail of [a ]]<health><tame |peaceful ><monster name>"
+ 状态后缀:
  ", swallowing you" / ", engulfing you" / ", holding you" / ", being held"
  ", can't move (paralyzed or sleeping or busy)"
  ", asleep"
  ", meditating"
  ", leashed to you"
  ", trapped in <trap>"
+ 隐藏/伪装描述 (mhidden_description)
+ 检测方式 (monbuf): "normal vision", "see invisible", "infravision",
  "telepathy", "astral vision", "monster detection", "warned of <type>"
```

### 17.2 百科全书查询 (`checkfile()`)

`data.base` 文件包含大量怪物的百科全书条目。当 `/` 命令找到恰好一个匹配结果时:

- **普通模式** (`flags.help=TRUE`): 自动询问 "More info about <name>?" 或直接显示
- **详细模式** (`LOOK_VERBOSE`): 不询问，直接显示
- **快速模式**: 不查询

查询使用怪物的 `pmnames[NEUTRAL]`（中性名称形式），忽略前缀修饰词。

### 17.3 补充信息 (`do_supplemental_info()`)

对某些特定怪物（目前仅兽人族），在百科全书查询后额外提供游戏内神话故事信息:

- 兽人名字包含 "of <gang>" → 显示帮派背景故事 (`suptext1`)
- 兽人名字包含 "the Fence" → 显示特殊背景故事 (`suptext2`)

### 17.4 探测魔杖/听诊器对怪物的详细信息

使用 wand of probing 或 stethoscope 触发 `mstatusline()` (`src/insight.c:3295`):

```
显示: "<Monster name> (M 或 F): Level <lev>; HP <cur>/<max>; AC <ac>; HD <hd>"
+ 附加状态标志:
  tame (+ 驯顺度数值，仅 wizard 模式)
  peaceful
  shapechanger (多态怪)
  eating
  mimicking (隐藏/伪装描述)
  cancelled, confused, blind, stunned, asleep, can't move
  meditating, scared, trapped
  fast/slow
  invisible
  stuck to you / holding you / swallowing you
```

此外还显示怪物的库存物品列表。

### 17.5 怪物 HP 描述

`monhealthdescr()` 理论上可以为怪物提供健康状态描述（uninjured, barely wounded, heavily wounded, nearly deceased 等），但在 3.7 中被 `#if 0` 禁用，**该功能未启用**。

---

## 18. 知识状态总结

### 18.1 四种知识层级

| 层级 | 条件 | xname() 显示 | 示例 |
|------|------|-------------|------|
| 完全未知 | `dknown=0` | 类别名 | "potion", "scroll", "amulet" |
| 外观已知 | `dknown=1, oc_name_known=0, oc_uname=NULL` | 外观描述 | "bubbly potion", "ZELGO MER" |
| 用户命名 | `dknown=1, oc_name_known=0, oc_uname!=NULL` | "called <name>" | "potion called heal" |
| 正式鉴定 | `dknown=1, oc_name_known=1` | 真实名称 | "potion of healing" |

### 18.2 个体级附加知识

在正式鉴定之上，每个物品实例还有额外信息维度:

| 标志 | 信息 | 显示效果 |
|------|------|----------|
| `known=1` | 附魔值/充能数 | "+2 long sword", "wand of fire (0:4)" |
| `bknown=1` | BUC 状态 | "blessed", "uncursed", "cursed" 前缀 |
| `rknown=1` | 防侵蚀 | "rustproof", "fireproof", "corrodeproof" 等 |
| `cknown=1` | 容器内容 | "empty" 或 "containing N items" |
| `lknown=1` | 锁定状态 | "locked", "unlocked", "broken" |
| `tknown=1` | 陷阱状态 | "trapped" (箱子) |

### 18.3 obj_typename() vs xname() vs doname()

| 函数 | 用途 | 显示 known 信息 | 显示 BUC | 显示外观 |
|------|------|-----------------|----------|----------|
| `obj_typename(otyp)` | 发现列表 | 否 | 否 | 是（如已知则也显示真名） |
| `xname(obj)` | 基础名称 | 取决于 `obj->known` | 否 | 是 |
| `doname(obj)` | 完整名称 | 是 | 是（如 `bknown`） | 是 |
| `simple_typename(otyp)` | 简短名称 | 否 | 否 | 否（只显示一种形式） |

---

## 19. 测试向量

以下为精确的输入/输出对，假设标准游戏规则和默认设置。

### TV-1: 基本外观显示——未鉴定药水

**输入**: 一瓶 `otyp=POT_HEALING`，本局打乱后 `oc_descr_idx` 指向 "bubbly"，`dknown=1`, `known=0`, `bknown=0`, `oc_name_known=0`, `oc_uname=NULL`, `quan=1`

**输出** (`xname()`): `"bubbly potion"`

### TV-2: 类型已鉴定的药水

**输入**: 同上，但 `oc_name_known=1`

**输出** (`xname()`): `"potion of healing"`

### TV-3: 用户命名的药水

**输入**: 同 TV-1，但 `oc_uname="heal"`

**输出** (`xname()`): `"potion called heal"`

### TV-4: 完全鉴定的带附魔武器

**输入**: `otyp=LONG_SWORD`, `dknown=1`, `known=1`, `bknown=1`, `rknown=1`, `spe=+2`, `blessed=1`, `oerodeproof=1`, `quan=1`

**输出** (`doname()`): `"a blessed rustproof +2 long sword"`

### TV-5: 鉴定卷轴——祝福版，幸运值 > 0

**输入**: 祝福鉴定卷轴读取，Luck=3, rn2(5)返回1

**处理**: `cval = rn2(5)` → 1; `cval==1 AND blessed AND Luck>0` → `cval=2`

**输出**: 玩家选择并鉴定 **2** 个物品

### TV-6: 鉴定卷轴——迷惑状态

**输入**: 任何 BUC 的鉴定卷轴，玩家处于 Confusion 状态

**输出**: "You identify this as an identify scroll." + 鉴定卷轴类型被发现 + **不鉴定任何库存物品**

### TV-7: 边界条件——盲目时魔杖使用

**输入**: 玩家失明，持有 `otyp=WAN_LIGHT`, `dknown=0`（失明时拾取），使用该魔杖

**处理**: `learnwand()` → `NOT Blind` 为假 → 不调用 `observe_object()` → `obj->dknown` 仍为 0 → `makeknown()` 不被调用

**输出**: 魔杖类型**不被鉴定**（效果仍然发生但类型不被发现）

### TV-8: 边界条件——Amulet of Yendor vs 仿品

**输入A**: `otyp=AMULET_OF_YENDOR`, `dknown=1`, `known=0`
**输入B**: `otyp=FAKE_AMULET_OF_YENDOR`, `dknown=1`, `known=0`

**输出A** (`xname()`): `"Amulet of Yendor"` (外观描述)
**输出B** (`xname()`): `"Amulet of Yendor"` (外观描述，与真品相同)

**输出A** (`the_unique_obj()`): `TRUE`（真品即使未 known 也视为 unique）
**输出B** (`the_unique_obj()`): `TRUE`（仿品故意撒谎）

**鉴定后**:
**输出A** (`known=1`): `"Amulet of Yendor"` (真名即外观)
**输出B** (`known=1`): `"cheap plastic imitation of the Amulet of Yendor"` (揭示真名)

### TV-9: 价格鉴定——未鉴定物品的加价

**输入**: 商店中 `otyp=POT_HEALING` (oc_cost=20), `dknown=1`, `oc_name_known=0`, `obj->o_id=12`, CHA=11 (无修正), 非游客, 无 dunce cap, 店主平和

**处理**:
- `oid_price_adjustment(obj, 12)` → `12 % 4 == 0` → 返回 1 → 加价
- `multiplier = 4, divisor = 3`
- `tmp = 20 * 4 = 80`, `80 * 10 / 3 = 266`, `(266 + 5) / 10 = 27`

**输出**: 标价 **27 zorkmids**

### TV-10: 价格鉴定——相同药水无加价

**输入**: 同上但 `obj->o_id=13`

**处理**: `13 % 4 == 1` → 返回 0 → 无加价

**输出**: 标价 **20 zorkmids**

### TV-11: 边界条件——cornuthaum 与 dunce cap 共享外观

**输入**: `otyp=CORNUTHAUM`, `dknown=1`, `oc_name_known=0`

**输出** (`xname()`): `"conical hat"`

**输入**: `otyp=DUNCE_CAP`, `dknown=1`, `oc_name_known=0`

**输出** (`xname()`): `"conical hat"` (与 cornuthaum 无法区分)

**价格区分**: cornuthaum 的 oc_cost=80, dunce cap 的 oc_cost=1，可通过商店价格区分。

### TV-12: 边界条件——`not_fully_identified()` 对金币

**输入**: `oclass=COIN_CLASS` (gold pieces), 任意其他标志状态

**输出**: `not_fully_identified()` → `FALSE`（金币永远视为完全鉴定）

### TV-13: 鉴定卷轴——诅咒版 + 类型已知

**输入**: 诅咒鉴定卷轴，`oc_name_known=1`（类型已经发现），非迷惑

**处理**: `scursed=TRUE`, `already_known=TRUE`
- 不进入 "identify this as" 分支
- `cval=1`（因为非祝福且 rn2(5) 不为0 的概率为 80%）

**输出**: 鉴定 **1** 个物品（不浪费，因为类型已知不需要自鉴定）

### TV-14: 观察对象——幻觉下不设置 dknown

**输入**: 玩家处于幻觉状态，查看库存中的 `otyp=POT_SPEED`, `dknown=0`

**处理**: `observe_object()` 检查 `!Hallucination` → 为假 → 不设置 `dknown`

**输出**: `dknown` 仍为 0，药水显示为 `"potion"`（无外观描述）
