# NetHack 3.7 物品命名系统机制规格

> 源文件: `src/objnam.c`, `include/obj.h`, `include/objclass.h`, `src/do_name.c`
> 提取日期: 2026-03-14
> 基于 commit: NetHack-3.7 分支 (objnam.c $NHDT-Date: 1745114235 rev 1.453)

---

## 1. 概念模型: 知识层次与三种名称

### 1.1 物品知识标志位

| 标志位 | 作用域 | 含义 |
|--------|--------|------|
| `obj->dknown` | 单个物品实例 | 已近距离观察 (能看到外观描述) |
| `objects[otyp].oc_name_known` (nn) | 物品类型全局 | 该类型已被"发现" (identify/use) |
| `obj->known` | 单个物品实例 | 精确属性已知 (附魔/充能数) |
| `obj->bknown` | 单个物品实例 | BUC 状态已知 |
| `obj->rknown` | 单个物品实例 | 防蚀状态已知 |
| `obj->cknown` | 单个物品实例 | 容器内容已知 |
| `obj->lknown` | 单个物品实例 | 锁定状态已知 |
| `obj->tknown` | 单个物品实例 | 陷阱状态已知 (箱子) |

### 1.2 三种名称来源

每个物品类型 (`otyp`) 有三种候选名称:

| 名称 | 来源 | 示例 |
|------|------|------|
| **actualn** | `OBJ_NAME(objects[otyp])` | "long sword", "scroll of identify", "healing" |
| **dn** | `OBJ_DESCR(objects[otyp])` | "milky", "ZELGO MER", "jade", (可能为 NULL) |
| **un** | `objects[otyp].oc_uname` | 玩家自定义类型名 (全局, 非实例) |

名称选择优先级: `nn (已发现) > un (玩家命名) > dn (外观描述)`

此外, 每个物品实例可有个人名: `ONAME(obj)` (通过 `obj->oextra->oname`).

---

## 2. xname(): 核心名称函数

`xname()` (objnam.c:582) 产出**不含**数量/BUC/附魔前缀的基础名称. 缓冲区从 `obuf[PREFIX]` 开始, 前 PREFIX=80 字节预留给 doname() 的前缀.

### 2.1 xname() 完整流程

```
fn xname(obj) -> &str:
    buf = nextobuf()[PREFIX..]   // 留出 80 字节前缀空间
    typ  = obj.otyp
    ocl  = &objects[typ]
    actualn = OBJ_NAME(*ocl)
    dn      = OBJ_DESCR(*ocl)   // 可能为 NULL
    un      = ocl.oc_uname       // 玩家自定义类型名
    nn      = ocl.oc_name_known  // 已发现?

    // 1. 武士角色名称替换
    if Role_if(PM_SAMURAI):
        actualn = Japanese_item_name(typ, actualn)
        if typ in {WOODEN_HARP, MAGIC_HARP}: dn = "koto"

    // 2. I18N 名称替换
    if i18n_active():
        tn = i18n_obj_name(typ)
        if tn: actualn = tn

    // 3. dn 回退
    if dn == NULL: dn = actualn

    // 4. 唯一物品知识修正
    if !nn && ocl.oc_uses_known && ocl.oc_unique:
        obj.known = 0   // 防止未发现的唯一物品泄露信息

    // 5. 设置 dknown
    if !Blind && !distantname:
        observe_object(obj)  // 设置 obj.dknown = 1

    // 6. 牧师自动 bknown
    if Role_if(PM_CLERIC):
        obj.bknown = 1

    // 7. 确定知识状态
    if iflags.override_ID:
        known = dknown = bknown = true; nn = 1
    else:
        known = obj.known; dknown = obj.dknown; bknown = obj.bknown

    // 8. 神器发现
    if obj.oartifact && obj.dknown:
        find_artifact(obj)

    // 9. pname 神器直接跳到命名
    if obj_is_pname(obj):
        goto nameit

    // 10. 按 oclass 分支生成基础名称 (见 2.2)
    switch obj.oclass: ...

    // 11. 复数化
    if obj.quan > 1 && !(CXN_SINGULAR):
        buf = makeplural(buf)

    // 12. 游戏结束时附加可读物品文本
    if program_state.gameover && obj.o_id != 0:
        T_SHIRT:       buf += ' with text "{tshirt_text}"'
        ALCHEMY_SMOCK: buf += ' with text "{apron_text}"'
        CANDY_BAR:     buf += ' labeled "{wrapper_text}"'
        HAWAIIAN_SHIRT: buf += ' with {an(motif)} motif'

    // 13. 个人名称
    if has_oname(obj) && dknown:
        buf += " named "
    nameit:
        buf += ONAME(obj)
        // 神器 "The X" -> "the X" (小写化)
        if obj.oartifact && ONAME starts with "The ":
            ONAME[0] = 't'

    // 14. 剥离前导 "the "
    if buf starts with "the ":
        buf = &buf[4..]

    return buf
```

### 2.2 按物品类别的 xname() 格式规则

以下伪码展示每个 oclass 的 buf 赋值. `nn` = 已发现, `dknown` = 外观已知, `un` = 玩家命名.

#### AMULET_CLASS
```
!dknown          -> "amulet"
AoY / FakeAoY    -> known ? actualn : dn
nn               -> actualn
un               -> "amulet called {un}"
else             -> "{dn} amulet"          // e.g. "triangular amulet"
```

#### WEAPON_CLASS (fall-through to VENOM/TOOL)
```
if is_poisonable && opoisoned:
    buf = "poisoned "                       // 后续 doname 会剥离再在正确位置重加

// VENOM_CLASS / TOOL_CLASS 共用下面逻辑:
if LENSES:        buf = "pair of "
if is_wet_towel:  buf = spe < 3 ? "moist " : "wet "

!dknown -> buf += dn
nn      -> buf += actualn
un      -> xcalled(buf, dn, un)             // "{dn} called {un}"
else    -> buf += dn

// FIGURINE 特殊: buf += " of {a/an} {monster_name}"
// 湿毛巾 wizard 模式: buf += " ({spe})"
```

#### ARMOR_CLASS
```
// 龙鳞: 无条件使用真名
GRAY_DRAGON_SCALES..YELLOW_DRAGON_SCALES:
    "set of {actualn}"

// 靴子/手套: 前缀 "pair of "
is_boots || is_gloves:
    buf = "pair of "

// 未观察的盾牌: 简化
is_shield && !dknown:
    ELVEN_SHIELD..ORCISH_SHIELD   -> "shield"
    SHIELD_OF_REFLECTION          -> "smooth shield"

nn   -> buf += actualn
un   -> xcalled(buf, armor_simple_name(obj), un)
else -> buf += dn
```

**armor_simple_name() 映射** (objnam.c:5473):

| 类别 (oc_armcat) | 简称 |
|------------------|------|
| ARM_SUIT + 龙鳞甲 | "dragon mail" / "dragon scales" |
| ARM_SUIT + *mail | "mail" |
| ARM_SUIT + *jacket | "jacket" |
| ARM_SUIT + 其他 | "suit" |
| ARM_CLOAK + ROBE | "robe" |
| ARM_CLOAK + MUMMY_WRAPPING | "wrapping" |
| ARM_CLOAK + ALCHEMY_SMOCK (nn && dknown) | "smock" |
| ARM_CLOAK + ALCHEMY_SMOCK (else) | "apron" |
| ARM_CLOAK + 其他 | "cloak" |
| ARM_HELM | "helm" 或 "hat" (取决于具体物品) |
| ARM_GLOVES | "gloves" |
| ARM_BOOTS | "boots" |
| ARM_SHIELD | "shield" |
| ARM_SHIRT | "shirt" |

#### FOOD_CLASS
```
SLIME_MOLD:
    buf = fruit_name(obj.spe)
    // 复数化在此特殊完成: makesingular -> makeplural
    // 跳过后续全局复数化

if partly_eaten_hack && obj.oeaten:
    buf = "partly eaten " + ...            // 仅 shrink_glob 路径

if obj.globby:
    size = owt <= 100 ? "small"
         : owt <= 300 ? "medium"
         : owt <= 500 ? "large"
         : "very large"
    buf = "{size} {actualn}"               // e.g. "medium glob of gray ooze"

else:
    buf = actualn
    if TIN && known: buf += tin_details    // e.g. " (homemade)"
```

#### COIN_CLASS / CHAIN_CLASS
```
buf = actualn                               // "gold piece", "iron chain"
```

#### ROCK_CLASS
```
STATUE && omndx != NON_PM:
    historic = Role_if(PM_ARCHEOLOGIST) && (spe & CORPSTAT_HISTORIC)
    buf = "{historic ? 'historic ' : ''}statue of {article}{monster_name}"
    // article: type_is_pname -> "", the_unique_pm -> "the ", else -> "a "/"an "

BOULDER && obj.next_boulder == 1:
    buf = "next boulder"
    obj.next_boulder = 0                    // 自动重置

else:
    buf = actualn                           // "boulder", "statue"
```

#### BALL_CLASS
```
buf = "{owt > oc_weight ? 'very ' : ''}heavy iron ball"
```

#### POTION_CLASS
```
if dknown && obj.odiluted:
    buf = "diluted "

if nn || un || !dknown:
    buf += "potion"
    if !dknown: break
    if nn:
        buf += " of "
        if POT_WATER && bknown && (blessed || cursed):
            buf += blessed ? "holy " : "unholy "
        buf += actualn
    else:
        xcalled(buf, "", un)               // "potion called {un}"
else:
    buf += dn                              // e.g. "milky"
    buf += " potion"
```

#### SCROLL_CLASS
```
buf = "scroll"
if !dknown: break
if nn:     buf += " of " + actualn
elif un:   xcalled(buf, "", un)            // "scroll called {un}"
elif ocl.oc_magic:
    buf += " labeled " + dn               // "scroll labeled ZELGO MER"
else:
    buf = dn + " scroll"                  // 非魔法: "{dn} scroll" (罕见路径)
```

> [疑似 bug] `oc_magic` 区分: 几乎所有卷轴都是 `oc_magic=1`, 所以 "else" 路径极罕见. 原设计意图可能是让 "unlabeled scroll" 走此路径 (确实如此: `SCR_BLANK_PAPER` 的 `oc_magic=0`). 但如果通过自定义 objects 数据添加非魔法卷轴, 可能产生意外的格式差异. 实际游戏中此路径仅 `SCR_BLANK_PAPER` 会触发.

#### WAND_CLASS
```
!dknown -> "wand"
nn      -> "wand of {actualn}"
un      -> xcalled("wand", un)             // "wand called {un}"
else    -> "{dn} wand"                     // "oak wand"
```

#### SPBOOK_CLASS
```
if SPE_NOVEL:
    !dknown -> "book"
    nn      -> actualn                     // 小说的真名
    un      -> xcalled("novel", un)
    else    -> "{dn} book"                 // "paperback book"
else:
    !dknown -> "spellbook"
    nn:
        if typ != SPE_BOOK_OF_THE_DEAD:
            "spellbook of " + actualn
        else:
            actualn                        // "Book of the Dead" (不加 "spellbook of")
    un  -> xcalled("spellbook", un)
    else -> "{dn} spellbook"               // "parchment spellbook"
```

#### RING_CLASS
```
!dknown -> "ring"
nn      -> "ring of {actualn}"
un      -> xcalled("ring", un)
else    -> "{dn} ring"                     // "jade ring"
```

#### GEM_CLASS
```
rock = oc_material == MINERAL ? "stone" : "gem"

!dknown   -> rock
!nn && un -> xcalled(rock, un)             // "gem called {un}"
!nn       -> "{dn} {rock}"                // "green gem", "gray stone"
nn        -> actualn                       // "emerald"
             if GemStone(typ): += " stone" // "garnet stone"
```

---

## 3. doname(): 完整名称 (含所有前缀/后缀)

`doname()` -> `doname_base()` (objnam.c:1236) 在 xname() 结果上添加前缀和后缀.

### 3.1 完整片段组装顺序 (从左到右)

```
[数量/冠词]  [empty]  [BUC]  [trapped]  [locked/unlocked/broken]
[greased]  [partly eaten / partly used]  [poisoned]
[erosion_words]  [erodeproof]  [+N enchantment]
  {xname_result}
  [" named {oname}" 或直接 "{artifact pname}"]
  [状态后缀: (being worn), (lit), (N:charges), ...]
  [容器内容: containing N items]
  [持握状态: (weapon in hand), (wielded), (in quiver), ...]
  [价格: (unpaid, N zorkmids), (for sale, N zorkmids)]
  [重量: (N aum)] -- 仅 wizard 模式
```

### 3.2 doname_base() 完整流程

```
fn doname_base(obj, flags) -> &str:
    bp = xname(obj)
    prefix = ""

    // === A. Poisoned 重定位 ===
    // xname 已写 "poisoned arrow", 但 doname 需要 "poisoned +0 arrow"
    // 所以先剥离 "poisoned ", 稍后在前缀中正确位置重加
    if bp starts with "poisoned " && obj.opoisoned:
        bp = bp[9..]     // 跳过 "poisoned "
        ispoisoned = true

    // 水果名匹配神器名检测
    fake_arti = (SLIME_MOLD && artifact_name(bp) != NULL)
    force_the = (fake_arti && artifact starts with "the ")

    // === B. 数量/冠词前缀 ===
    if obj.quan > 1:
        if dknown || !vague_quan:
            prefix = "{obj.quan} "          // "3 "
        else:
            prefix = "some "
    elif CORPSE:
        pass                                // corpse_xname 自己处理冠词
    elif force_the || obj_is_pname(obj) || the_unique_obj(obj):
        if bp starts with "the ": bp = bp[4..]
        prefix = "the "
    elif !fake_arti:
        prefix = "a "                       // 后续会被 just_an 调整为 "a "/"an "

    // === C. "empty" ===
    if cknown:
        if (BAG_OF_TRICKS || HORN_OF_PLENTY) && spe == 0 && !known:
            prefix += "empty "
        elif (Is_container || STATUE) && !Has_contents:
            prefix += "empty "

    // === D. BUC 前缀 ===
    if bknown && oclass != COIN_CLASS
       && !(POT_WATER && oc_name_known && (blessed || cursed)):
        if cursed:    prefix += "cursed "
        elif blessed: prefix += "blessed "
        elif show_uncursed():              // 见 3.3
            prefix += "uncursed "

    // === E. 容器状态 ===
    if Is_box && otrapped && tknown && dknown:
        prefix += "trapped "
    if lknown && Is_box:
        if obroken:   prefix += "broken "
        elif olocked: prefix += "locked "
        else:         prefix += "unlocked "

    // === F. Greased ===
    if greased:
        prefix += "greased "

    // === G. 容器内容后缀 ===
    if cknown && Has_contents:
        bp += " containing {N} item{s}"

    // === H. 按类别追加前缀和后缀 ===

    case AMULET_CLASS:
        if owornmask & W_AMUL: bp += " (being worn)"

    case ARMOR_CLASS:
        if owornmask & W_ARMOR:
            bp += " (being worn)" | " (embedded in your skin)"
                  | " (being donned)" | " (being doffed)"
            if uarmg && Glib: bp += "; slippery)" (替换末尾 ')')
        FALLTHROUGH ->

    case WEAPON_CLASS:     // 含 weptool
        if ispoisoned: prefix += "poisoned "
        add_erosion_words(obj, prefix)      // 见 6 节
        if known: prefix += "{+N} "         // e.g. "+3 ", "-1 "

    case TOOL_CLASS:
        if owornmask & (W_TOOL|W_SADDLE): bp += " (being worn)"
        if LEASH + leashmon: bp += " (attached to {mon})"
        if CANDELABRUM: bp += " ({N} of 7 candle{s}{attached/lit})"
        if OIL_LAMP|MAGIC_LAMP|BRASS_LANTERN|candle:
            if partly used: prefix += "partly used "
            if lamplit: bp += " (lit)"
        if oc_charged: goto charges

    case WAND_CLASS:
    charges:
        if known: bp += " ({recharged}:{spe})"   // e.g. " (0:5)"

    case POTION_CLASS:
        if POT_OIL && lamplit: bp += " (lit)"

    case RING_CLASS:
        if owornmask & W_RINGR: bp += " (on right "
        if owornmask & W_RINGL: bp += " (on left "
        if owornmask & W_RING:  bp += "{HAND})"
        if known && oc_charged: prefix += "{+N} "

    case FOOD_CLASS:
        if oeaten: prefix += "partly eaten "
        if CORPSE:
            // 调用 corpse_xname() 重建整个前缀
            cxstr = corpse_xname(obj, prefix, CXN_ARTICLE|CXN_NOCORPSE)
            prefix = cxstr + " "
        if EGG && (known || mvflags & MV_KNOWS_EGG):
            prefix += "{monster_name} "
            if spe == 1: bp += " (laid by you)"
        if MEAT_RING: goto ring

    case BALL_CLASS / CHAIN_CLASS:
        add_erosion_words(obj, prefix)
        if owornmask & (W_BALL|W_CHAIN):
            bp += " ({chained/attached} to you)"

    // === I. 武器持握状态 ===
    if owornmask & W_WEP && !mrg_to_wielded:
        // 多数量/弹药/投射物 -> "(wielded)"
        // 单件武器 -> "(weapon in {right/left} hand)"
        // 双手武器 -> "(weapon in hands)"
        // 双持主手 -> "(wielded in {right/left} hand)"
        // aklys -> "(tethered to {hand})"
        // 若 Sting 发光: 替换末尾 ')' 添加 ", glowing {color})"
    if owornmask & W_SWAPWEP:
        if twoweap: "(wielded in {left/right} hand)"
        else: "(alternate weapon{s}; not wielded)"
    if owornmask & W_QUIVER:
        // 弓箭 -> "(in quiver)"
        // 小型非弓 -> "(in quiver pouch)"
        // 其他 -> "(at the ready)"

    // === J. 价格 ===
    if is_unpaid:
        bp += " ({unpaid/contents}, {N} {zorkmids})"
    elif with_price:
        bp += " ({for sale/no charge}, {N} {zorkmids})"

    // === K. 冠词修正 ===
    if prefix starts with "a ":
        rest = prefix[2..]
        article = just_an(rest.is_empty() ? bp : rest)
        prefix = article + rest

    // === L. Wizard 重量 ===
    if wizard && wizweight:
        bp += " ({owt} aum)"

    // === M. 最终拼接 ===
    return strprepend(bp, prefix)
```

### 3.3 "uncursed" 显示的精确条件

当 `bknown == true`, 物品既非 blessed 也非 cursed 时:

```
显示 "uncursed " 当且仅当:
    (a) flags.implicit_uncursed == false
    OR
    (b) ALL of:
        - (!known || !oc_charged || oclass == ARMOR_CLASS || oclass == RING_CLASS)
        - otyp != SCR_MAIL
        - otyp != FAKE_AMULET_OF_YENDOR
        - otyp != AMULET_OF_YENDOR
        - !Role_if(PM_CLERIC)
```

**设计意图**: 当 `implicit_uncursed=true` (默认) 时:

| 条件 | 显示 "uncursed"? | 原因 |
|------|-----------------|------|
| known=T, oc_charged=T, 非甲非戒 | 否 | 已鉴定 + 有充能/附魔 = BUC 可推断 |
| known=T, oc_charged=T, 甲胄 | 是 | 甲胄特例: 总是显示以消除歧义 |
| known=T, oc_charged=T, 戒指 | 是 | 戒指特例: 同上 |
| known=F (附魔未知) | 是 | 需要区分 "BUC 未知" vs "已知 uncursed" |
| 牧师角色 | 否 | 牧师永远知道 BUC, 不需要消歧义 |

### 3.4 BUC 与圣水的交互

```
                    oc_name_known=0          oc_name_known=1
                    (水未被发现)              (水已被发现)
blessed, bknown:   "a blessed clear potion"  "a potion of holy water"
cursed, bknown:    "a cursed clear potion"   "a potion of unholy water"
uncursed, bknown:  "an uncursed clear potion" "an uncursed potion of water"
```

关键: 当 `oc_name_known=1 && (blessed||cursed)` 时, BUC 前缀被跳过, 因为 xname 中已将 `actualn` 改为 "holy water"/"unholy water".

---

## 4. 外观名 vs 真实名 (Shuffled Descriptions)

### 4.1 有 Shuffle 的类别

以下类别每局游戏开始时随机分配外观描述 (由 `init_objects()` in `o_init.c` 完成):

| 类别 | 描述类型 | 示例 dn |
|------|---------|---------|
| POTION | 颜色 | "milky", "pink", "ruby", "emerald" |
| SCROLL | 标签 | "ZELGO MER", "DUAM XNAHT", "PRATYAVAYAH" |
| WAND | 材质 | "glass", "oak", "maple", "balsa" |
| RING | 材质/外观 | "wooden", "granite", "iron", "jade" |
| SPBOOK | 封面 | "parchment", "vellum", "ragged", "wrinkled" |
| AMULET | 形状 | "circular", "spherical", "triangular" |

### 4.2 名称选择逻辑

对于有 shuffle 的类别, xname 的四级优先:

```
!dknown:         类别通称 ("potion", "scroll", "wand", "ring", "spellbook", "amulet")
nn (discovered): 真实名  ("potion of healing", "wand of fire")
un (called):     "{类别} called {un}" ("potion called heal?")
dknown only:     外观名  ("pink potion", "oak wand")
```

### 4.3 无 Shuffle 的类别

武器、甲胄、工具、食物、宝石、金币、锁链、球、岩石等没有 shuffled description. 对于这些:
- `dn` 常等于 `actualn`, 或为固定描述 (如 ELVEN_CLOAK 的 dn = "faded pall")
- `dn` 可能为 NULL, 此时 xname 回退: `if !dn: dn = actualn`

### 4.4 SCR_BLANK_PAPER 特殊路径

`SCR_BLANK_PAPER` 的 `oc_magic = 0`, 当未发现且 dknown 时:
- 走 `else` 分支: `"{dn} scroll"` -> "unlabeled scroll"
- 而非 `"scroll labeled {dn}"` (魔法卷轴路径)

---

## 5. a/an 冠词选择

### 5.1 just_an() 完整规则 (objnam.c:2113)

输入 `str`, 返回 `""` / `"a "` / `"an "`:

```
fn just_an(str) -> &str:
    c0 = lowercase(str[0])

    // 单字符 (str[1] == '\0' || str[1] == ' ')
    if single_char:
        c0 in "aefhilmnosx" -> "an "
        else                 -> "a "

    // 特殊无冠词情况
    if starts_with("the ")          -> ""
    if str == "molten lava"         -> ""
    if str == "iron bars"           -> ""
    if str == "ice"                 -> ""

    // 标准元音/辅音规则 (带例外)
    if c0 is vowel (in "aeiou"):
        // 发音为辅音的元音开头词 -> "a "
        if starts_with("one") && str[3] not in "-_ " && str[3] != '\0':
            -> "a "
        if starts_with("eu"):          -> "a "    // eucalyptus
        if starts_with("uke"):         -> "a "
        if starts_with("ukulele"):     -> "a "
        if starts_with("unicorn"):     -> "a "
        if starts_with("uranium"):     -> "a "
        if starts_with("useful"):      -> "a "
        else:                          -> "an "

    // x + 辅音 -> "an " (发音为 /z/ 或 /ks/ 需要 an)
    if c0 == 'x' && str[1] not in "aeiouAEIOU":
        -> "an "

    // 默认辅音开头
    -> "a "
```

所有比较大小写不敏感. 元音集: `vowels = "aeiouAEIOU"`.

### 5.2 doname 中的冠词修正

在 doname_base 末尾 (objnam.c:1691):

```
if prefix starts with "a ":
    rest = prefix[2..]              // 去掉 "a " 后剩余部分, 如 "cursed "
    if rest is non-empty:
        article = just_an(rest)     // 检查 "cursed" 的首字母 'c' -> "a "
    else:
        article = just_an(bp)       // 检查基础名称首字母
    prefix = article + rest
```

**关键**: 冠词取决于**紧跟其后的首个单词**, 而非物品名. 例如:
- "a cursed long sword" -> 检查 "cursed": 'c' -> "a "
- "an uncursed long sword" -> 检查 "uncursed": 'u' -> "an "
- "a +0 long sword" -> 检查 "+0": '+' 非字母非元音 -> "a "

### 5.3 the() 函数规则 (objnam.c:2182)

```
fn the(str) -> &str:
    if starts_with("the "): lowercase 'T' and return
    if first_char < 'A' || > 'Z':  insert "the "    // 小写开头
    elif CapitalMon(str):           insert "the "    // 如 "Oracle"
    elif is_named_fruit && !artifact_name_match_starting_with_the:
                                    insert "the "    // 水果名
    elif has_space_or_hyphen:
        if last_word is lowercase:
            if no apostrophe:       insert "the "
        elif has_spaces && contains " of " before " named "/" called ":
                                    insert "the "
        elif "Platinum Yendorian Express Card":
                                    insert "the "    // 硬编码特例
    else:                           no "the"         // 大写专有名词
```

### 5.4 I18N 冠词跳过

当 `i18n_skip_article()` 返回 true (如 `has_articles=false` 的 CJK 语言):
- `an(str)` -> 直接返回 str
- `An(str)` -> 直接返回 str (不大写)
- `the(str)` -> 直接返回 str

---

## 6. 侵蚀 (Erosion) 形容词

### 6.1 add_erosion_words() (objnam.c:1156)

仅对 `is_damageable(obj)` 或 CRYSKNIFE 生效.

```
fn add_erosion_words(obj, prefix):
    iscrys = (otyp == CRYSKNIFE)
    rknown = override_ID ? true : obj.rknown

    if !is_damageable(obj) && !iscrys: return

    // --- 主侵蚀 (oeroded: 0-3) ---
    if obj.oeroded > 0 && !iscrys:
        match obj.oeroded:
            2 -> prefix += "very "
            3 -> prefix += "thoroughly "
        if is_rustprone(obj):   prefix += "rusty "
        elif is_crackable(obj): prefix += "cracked "
        else:                   prefix += "burnt "

    // --- 次侵蚀 (oeroded2: 0-3) ---
    if obj.oeroded2 > 0 && !iscrys:
        match obj.oeroded2:
            2 -> prefix += "very "
            3 -> prefix += "thoroughly "
        if is_corrodeable(obj): prefix += "corroded "
        else:                   prefix += "rotted "

    // --- 防蚀 ---
    if rknown && obj.oerodeproof:
        if iscrys:              prefix += "fixed "
        elif is_rustprone:      prefix += "rustproof "
        elif is_corrodeable:    prefix += "corrodeproof "
        elif is_flammable:      prefix += "fireproof "
        elif is_crackable:      prefix += "tempered "
        elif is_rottable:       prefix += "rotproof "
        else:                   // 无前缀
```

### 6.2 侵蚀等级与描述

| oeroded/oeroded2 值 | 修饰词 | 完整前缀示例 (iron 物品) |
|---------------------|--------|------------------------|
| 0 | (无) | |
| 1 | (无修饰) | "rusty " / "corroded " |
| 2 | "very " | "very rusty " / "very corroded " |
| 3 | "thoroughly " | "thoroughly rusty " / "thoroughly corroded " |

### 6.3 材质与侵蚀类型

| 条件 | oeroded 显示词 | oeroded2 显示词 |
|------|---------------|----------------|
| is_rustprone (IRON) | "rusty" | - |
| is_crackable (GLASS + ARMOR) | "cracked" | - |
| 其他 (flammable/organic) | "burnt" | - |
| is_corrodeable (IRON or COPPER) | - | "corroded" |
| 其他 (rottable) | - | "rotted" |

### 6.4 双重侵蚀与侵蚀+防蚀

物品可同时具有 oeroded 和 oeroded2, 两组形容词均出现:

```
"very rusty corroded long sword"            // oeroded=2, oeroded2=1, IRON
"thoroughly burnt very rotted leather armor" // oeroded=3, oeroded2=2, LEATHER
```

也可同时有侵蚀和防蚀 (通过 confused 读 cursed destroy armor 卷轴):

```
"rusty rustproof long sword"                // oeroded=1, oerodeproof=1
```

### 6.5 CRYSKNIFE 特殊处理

CRYSKNIFE (水晶刀) 的 `iscrys = true` 使得:
- 所有侵蚀描述 (rusty/burnt/corroded/rotted) 被跳过
- 防蚀显示为 "fixed " (而非 "rustproof " 等)

### 6.6 is_damageable 判定

```
is_damageable = is_rustprone || is_flammable || is_rottable
             || is_corrodeable || is_crackable

is_rustprone:   oc_material == IRON
is_flammable:   oc_material in {WAX..WOOD} (排除 LIQUID) || PLASTIC
                (further exclusions in mkobj.c: candles, fire-resistant items)
is_rottable:    oc_material in {WAX..WOOD} (排除 LIQUID) || DRAGON_HIDE
is_corrodeable: oc_material in {COPPER, IRON}
is_crackable:   oc_material == GLASS && oclass == ARMOR_CLASS
```

### 6.7 erosion_matters()

决定侵蚀是否有实际影响 (而非仅是显示):
- WEAPON_CLASS, ARMOR_CLASS, BALL_CLASS, CHAIN_CLASS: 总是
- TOOL_CLASS: 仅 is_weptool
- 其他: 否

---

## 7. #name 命令: "called" vs 个人名称

### 7.1 命令入口 (do_name.c:499, docallcmd)

`#name` (或 `#call`) 提供以下选项:

| 键 | 加速键 | 操作 |
|----|--------|------|
| m | C | 给可见怪物命名 |
| i | y | 给库存中具体物品命名 (个人名, ONAME) |
| o | n | 给库存中物品类型命名 ("called", oc_uname) |
| f | , | 给地板物品类型命名 |
| d | \ | 给发现列表中的类型命名 |
| a | l | 给当前楼层添加注释 |

### 7.2 个人名称 (do_oname, 选项 i)

存储位置: `obj->oextra->oname` (ONAME 宏), 通过 `oname()` 函数设置.

xname 中的显示:
```
"{base_name} named {oname}"
```

规则与限制:
1. SPE_NOVEL 不可命名: "already has a published name"
2. 已有神器不可重命名: "resists the attempt"
3. 命名为已存在神器名: 手打滑 (wipeout_text), 刻出乱码, 消耗识字行为
4. 命名为尚不存在的同类型神器名: 自动创建该神器 (如给 ELVEN_DAGGER 命名 "Sting")
5. 名称长度上限: PL_PSIZ (32) 字节

### 7.3 类型名称 (docall, 选项 o)

存储位置: `objects[otyp].oc_uname`, 通过 `docall()` 函数设置.

xname 中的显示:
```
"{class_prefix} called {uname}"
```

class_prefix 因类别而异:
- POTION: "potion"
- SCROLL: "scroll"
- WAND: "wand"
- RING: "ring"
- SPBOOK: "spellbook" (novel: "novel")
- AMULET: "amulet"
- GEM: "stone" 或 "gem"
- ARMOR: `armor_simple_name(obj)` (如 "boots", "cloak", "mail")
- WEAPON/TOOL: `dn` (外观描述名)

可 call 的条件 (`objtyp_is_callable`):
- 已有 `oc_uname` -> 可 call (允许修改)
- AMULET_OF_YENDOR / FAKE_AMULET_OF_YENDOR -> **不可** call (防止鉴定)
- 有 `OBJ_DESCR` 的 SCROLL/POTION/WAND/RING/GEM/SPBOOK/ARMOR/TOOL/VENOM -> 可 call
- 其他 -> 不可 call

优先级: 一旦类型被发现 (`oc_name_known=1`), xname 使用 actualn 而非 "called" 名.

### 7.4 取消命名

输入空字符串或仅空格:
- 个人名: 移除 ONAME (new_oname(obj, 0))
- 类型名: free oc_uname, 设为 NULL, 可能从发现列表中移除 (undiscover_object)

---

## 8. 复数化规则 (makeplural)

### 8.1 处理顺序 (objnam.c:2858)

```
1. 代词映射: he/him/his -> they/them/their (保留首字母大小写)
2. "pair of " 开头 -> 不变
3. 复合词分割: 在分隔符处切断, 仅处理前半部分
   分隔符: " of ", " labeled ", " called ", " named ", " above",
           " versus ", " from ", " in ", " on ", " a la ", " with",
           " de ", " d'", " du ", " au ", "-in-", "-at-"
4. 单字符或非字母结尾 -> 加 "'s"
5. as_is[] / one_off[] 查表 (见 8.2, 8.3)
6. ...man -> ...men (排除 badman 列表)
7. [aeioulr]f -> [aeioulr]ves (排除 -erf)
8. ...ium -> ...ia
9. ...alga/hypha/larva/amoeba/vertebra -> +e
10. ...us -> ...i (排除 lotus, wumpus)
11. ...sis -> ...ses
12. ...eau -> ...eaux (排除 bureau)
13. ...matzoh/matzah -> ...matzot; ...matzo/matza -> ...matzot
14. ...dex/dix/tex -> ...ices (排除 index)
15. ...[zxs] / ...ch / ...sh -> +es (ch 排除 k-sound 词)
    ...ato / ...dingo -> +es
16. 辅音+y -> ...ies
17. 默认: +s
```

### 8.2 不变复数 (as_is[])

```
boots, shoes, gloves, lenses, scales, eyes, gauntlets, iron bars,
bison, deer, elk, fish, fowl, tuna, yaki, -hai, krill, manes,
moose, ninja, sheep, ronin, roshi, shito, tengu, ki-rin, Nazgul,
gunyoki, piranha, samurai, shuriken, haggis, Bordeaux
```

也包括:
- 以 "craft" 结尾 (长度 > 5): aircraft, hovercraft
- "ya" (整词或以 " ya" 结尾)
- 已是复数的后缀: "ae", "eaux", "matzot"

### 8.3 一对一不规则 (one_off[])

| 单数后缀 | 复数后缀 | 示例 |
|---------|---------|------|
| child | children | |
| cubus | cubi | incubus -> incubi |
| culus | culi | homunculus -> homunculi |
| Cyclops | Cyclopes | |
| djinni | djinn | |
| erinys | erinyes | |
| foot | feet | |
| fungus | fungi | |
| goose | geese | |
| knife | knives | |
| labrum | labra | candelabrum -> candelabra |
| louse | lice | |
| mouse | mice | |
| mumak | mumakil | |
| nemesis | nemeses | |
| ovum | ova | |
| ox | oxen | (仅独立 "ox" 或 "muskox") |
| passerby | passersby | |
| rtex | rtices | vortex -> vortices |
| serum | sera | |
| staff | staves | |
| tooth | teeth | |

**注意**: 这些是**后缀匹配**, 如 "knife" 匹配 "crysknife".

### 8.4 特殊防误匹配

- "slice" -> "slices" (避免 "lice" 后缀匹配)
- "mongoose" -> "mongooses" (避免 "goose" 后缀匹配)
- "<X>ox" (X 非 "musk") -> "<X>oxes" (避免 ox->oxen; "fox" -> "foxes")
- "muskox" -> "muskoxen"

### 8.5 badman 列表 (不用 man->men 变换)

pluralize 时 *man 不变为 *men 的前缀:
```
albu, antihu, anti, ata, auto, bildungsro, cai, cay, ceru, corner, decu, des,
dura, fir, hanu, het, infrahu, inhu, nonhu, otto, out, prehu, protohu, subhu,
superhu, talis, unhu, sha, hu, un, le, re, so, to, at, a
```

singularize 时 *men 不变为 *man 的前缀:
```
abdo, acu, agno, ceru, cogno, cycla, fleh, grava, hegu, preno, sonar, speci,
dai, exa, fla, sta, teg, tegu, vela, da, hy, lu, no, nu, ra, ru, se, vi, ya, o, a
```

### 8.6 ch 的 k-sound 词 (加 s 而非 es)

```
monarch, poch, tech, mech, stomach, psych, amphibrach, anarch, atriarch,
azedarach, broch, gastrotrich, isopach, loch, oligarch, peritrich,
sandarach, sumach, symposiarch
```

### 8.7 I18N 复数跳过

当 `i18n_skip_plural()` 返回 true:
- `makeplural(str)` -> 返回 str 不变
- `makesingular(str)` -> 返回 str 不变

---

## 9. 数量处理

### 9.1 xname 中的复数化

xname() 在基础名确定后, 若 `obj->quan > 1` 且非 CXN_SINGULAR:
```
buf = makeplural(buf)
```

复合词分割确保只处理主词:
```
"potion of healing" -> "potions of healing"    // " of " 分割
"wand called death" -> "wands called death"    // " called " 分割
"pair of leather gloves" -> 不变               // "pair of " 特殊处理
```

### 9.2 doname 中的数量前缀

```
quan > 1:  prefix = "{quan} "       // "3 "
quan == 1 && corpse:  无冠词        // corpse_xname 自行处理
quan == 1 && unique:  "the "
quan == 1 && normal:  "a " / "an "  // 由 just_an 决定
```

### 9.3 组合示例

```
quan=1:  "a +0 long sword"
quan=3:  "3 +0 long swords"
quan=1:  "the blessed rustproof +5 Excalibur"
quan=5:  "5 uncursed poisoned +0 arrows"
quan=1:  "a blessed +0 set of silver dragon scales"
quan=2:  "2 potions of healing"
```

---

## 10. 特殊格式与边界情况

### 10.1 日语替换名 (Samurai)

| 物品 | 英文 actualn | 日语替换 |
|------|-------------|---------|
| SHORT_SWORD | short sword | wakizashi |
| BROADSWORD | broadsword | ninja-to |
| FLAIL | flail | nunchaku |
| GLAIVE | glaive | naginata |
| LOCK_PICK | lock pick | osaku |
| WOODEN_HARP | wooden harp | koto |
| MAGIC_HARP | magic harp | magic koto |
| KNIFE | knife | shito |
| PLATE_MAIL | plate mail | tanko |
| HELMET | helmet | kabuto |
| LEATHER_GLOVES | leather gloves | yugake |
| FOOD_RATION | food ration | gunyoki |
| POT_BOOZE | booze | sake |

WOODEN_HARP / MAGIC_HARP 额外将 dn 也替换为 "koto".

### 10.2 GemStone 宏

```
GemStone(typ) =
    typ == FLINT
    OR (objects[typ].oc_material == GEMSTONE
        AND typ NOT IN {DILITHIUM_CRYSTAL, RUBY, DIAMOND,
                        SAPPHIRE, BLACK_OPAL, EMERALD, OPAL})
```

当 `GemStone(typ) == true` 且已发现, 宝石名后追加 " stone":
- "garnet stone", "turquoise stone", "amethyst stone", "flint stone"

不追加的: DILITHIUM_CRYSTAL, RUBY, DIAMOND, SAPPHIRE, BLACK_OPAL, EMERALD, OPAL.

### 10.3 球的重量

```
owt > objects[HEAVY_IRON_BALL].oc_weight -> "very heavy iron ball"
else                                     -> "heavy iron ball"
```

### 10.4 稀释药水

`obj->odiluted` (复用 oeroded 位) 在 xname 中:
```
if dknown && odiluted: buf = "diluted " + rest
```

### 10.5 尸体格式 (corpse_xname)

```
普通怪物:  "{a/an} {monster_name} corpse"
唯一怪物:  "the {Monster_name's} corpse"       // 所有格
pname:     "{Medusa's} corpse"                  // 无 "the"
多数量:    "{N} {monster_name} corpses"          // 无冠词
前缀:      "{adjective} {article?} {monster_name} corpse"
           // 唯一: "{Monster's} {adjective} corpse" (所有格在前)
           // 普通: "{adjective} {monster_name} corpse"
```

### 10.6 雕像格式

```
Archeologist + CORPSTAT_HISTORIC:
    "historic statue of {article?} {monster_name}"
else:
    "statue of {article?} {monster_name}"

article: type_is_pname -> ""
         the_unique_pm -> "the "
         else          -> just_an(anbuf, name)
```

### 10.7 幻觉对物品名无影响

`xname()` 和 `doname()` **不**进行幻觉名称替换. 幻觉仅影响怪物名、地图符号、陷阱名等外部环境感知, 不影响库存物品命名.

### 10.8 I18N 对命名顺序的影响

I18N 仅替换 `actualn`, 不改变组装顺序. 这意味着:
```
中文 "长剑" 替换 "long sword" 后:
    "2 cursed +0 长剑"   // 英文前缀 + 中文基础名
    而非 "被诅咒的+0长剑×2"
```

[疑似设计限制: I18N 系统替换了基础名称但未重构组装顺序. CJK 语言的正确格式需要完全不同的组装逻辑. Babel 项目需要为此设计新的 trait 体系.]

---

## 11. 辅助名称函数汇总

| 函数 | 作用 | 典型输出 |
|------|------|----------|
| `xname(obj)` | 基础名, 无冠词/数量/BUC | "long sword" |
| `doname(obj)` | 完整库存名 | "a +0 long sword" |
| `doname_with_price(obj)` | 含价格 | "a +0 long sword (for sale, 15 zorkmids)" |
| `doname_vague_quan(obj)` | 模糊数量 | "some gold pieces" |
| `cxname(obj)` | xname + 尸体怪物类型 | "newt corpse" |
| `cxname_singular(obj)` | 强制单数的 cxname | "newt corpse" (即使 quan > 1) |
| `yname(obj)` | "your"/"Izchak's" + cxname | "your long sword" |
| `Yname2(obj)` | 大写 yname | "Your long sword" |
| `Doname2(obj)` | 大写 doname | "A +0 long sword" |
| `aobjnam(obj, verb)` | 数量 + cxname + 动词 | "3 arrows hit" |
| `yobjnam(obj, verb)` | your + aobjnam | "your 3 arrows hit" |
| `Tobjnam(obj, verb)` | The + xname + 动词 | "The long sword welds" |
| `an(str)` | "a"/"an" + str | "an arrow" |
| `the(str)` | "the" + str (视情况) | "the Amulet of Yendor" |
| `singular(obj, func)` | 强制 quan=1 调用 func | "arrow" |
| `makeplural(str)` | 英语复数化 | "scrolls of identify" |
| `makesingular(str)` | 英语单数化 | "scroll of identify" |
| `obj_typename(otyp)` | 类型全名 (含 called + dn) | "scroll of identify (ZELGO MER)" |
| `simple_typename(otyp)` | 简化类型名 | "scroll of identify" |
| `minimal_xname(obj)` | 最简名 (无用户名) | "potion" / "brown potion" / "potion of healing" |
| `killer_xname(obj)` | 死因名 (已鉴定, 无 BUC) | "a wand of fire" |
| `corpse_xname(obj, adj, flags)` | 尸体专用 | "a partly eaten newt corpse" |
| `bare_artifactname(obj)` | 仅神器名 | "Excalibur" |
| `actualoname(obj)` | 视为已发现的最简名 | "potion of healing" |
| `short_oname(obj, func, alt, limit)` | 截断长名 | 渐进截短策略 |
| `mshot_xname(obj)` | 多次射击前缀 (xname 增强) | "the 2nd arrow" |
| `paydoname(obj)` | 商店逐项购买名 (抑制价格, 隐藏容器内容) | "an unpaid chest and its contents" |

**doname_base flags**: `doname_base(obj, flags)` 接受位标志:
- `DONAME_WITH_PRICE (1)`: 附加商店价格后缀
- `DONAME_VAGUE_QUAN (2)`: 未 dknown 时用 "some " 代替数量
- `DONAME_FOR_MENU (4)`: 菜单显示模式 (影响缓冲区边界检查, 不改变输出格式; 目前未被调用)

---

## 12. 测试向量

### 12.1 基础武器

| # | 状态 | doname() 输出 |
|---|------|--------------|
| 1 | long sword, quan=1, known=T, spe=+2, bknown=T, blessed=T, dknown=T, nn=T | `"a blessed +2 long sword"` |
| 2 | long sword, quan=3, known=T, spe=+0, bknown=T, uncursed, dknown=T, nn=T, oeroded=1, IRON, implicit_uncursed=T | `"3 rusty +0 long swords"` |
| 3 | long sword, quan=1, known=F, bknown=T, uncursed, dknown=T, nn=T, implicit_uncursed=T | `"an uncursed long sword"` |

**#2 说明**: known=T && oc_charged=T && WEAPON (非 ARMOR/RING) -> uncursed 省略. oeroded=1 + IRON -> "rusty ". 侵蚀词在附魔值之前: add_erosion_words() (objnam.c:1435) 先于 spe 格式化 (objnam.c:1437).

**#3 说明**: known=F -> `(!known || !oc_charged || ...)` 为 true -> 显示 "uncursed". "an" 因 "uncursed" 首字母 'u' 是元音.

### 12.2 BUC 条件

| # | 状态 | 输出 | 原因 |
|---|------|------|------|
| 4 | long sword, known=T, bknown=T, uncursed, spe=+1, implicit_uncursed=T | `"a +1 long sword"` | known + charged + WEAPON -> 省略 uncursed |
| 5 | plate mail, known=T, bknown=T, uncursed, spe=+0, implicit_uncursed=T | `"an uncursed +0 plate mail"` | ARMOR_CLASS 特例 -> 强制 uncursed |
| 6 | RIN_TELEPORTATION, known=T, bknown=T, uncursed, spe=+0, implicit_uncursed=T, nn=T | `"an uncursed +0 ring of teleportation"` | RING_CLASS 特例 -> 强制 uncursed |
| 7 | WAN_FIRE, known=T, bknown=T, uncursed, spe=4, recharged=1, implicit_uncursed=T, nn=T | `"a wand of fire (1:4)"` | known + charged + WAND -> 省略 uncursed |

### 12.3 冠词边界

| # | 前缀+基础名 (just_an 输入) | 结果冠词 |
|---|--------------------------|---------|
| 8 | "uncursed long sword" | "an " (u 是元音) |
| 9 | "eucalyptus leaf" | "a " (eu 例外) |
| 10 | "unicorn horn" | "a " (unicorn 例外) |
| 11 | "+0 long sword" | "a " ('+' 非元音非特殊) |

### 12.4 复数化

| # | 输入 | makeplural() 输出 |
|---|------|-------------------|
| 12 | "knife" | "knives" |
| 13 | "staff" | "staves" |
| 14 | "pair of leather gloves" | "pair of leather gloves" |
| 15 | "human" | "humans" (badman -> 加 s) |
| 16 | "potion of healing" | "potions of healing" |
| 17 | "fungus" | "fungi" |
| 18 | "fox" | "foxes" |
| 19 | "vortex" | "vortices" |
| 20 | "homunculus" | "homunculi" |

### 12.5 侵蚀描述

| # | 物品 | oeroded | oeroded2 | oerodeproof | rknown | 侵蚀前缀 |
|---|------|---------|----------|-------------|--------|---------|
| 21 | IRON long sword | 2 | 1 | 0 | F | "very rusty corroded " |
| 22 | IRON long sword | 0 | 0 | 1 | T | "rustproof " |
| 23 | IRON long sword | 3 | 0 | 1 | T | "thoroughly rusty rustproof " |
| 24 | CRYSKNIFE | 2 | 1 | 1 | T | "fixed " (侵蚀描述被 iscrys 跳过) |
| 25 | LEATHER armor | 1 | 2 | 0 | F | "burnt very rotted " |

### 12.6 药水特殊情况

| # | 状态 | 输出 |
|---|------|------|
| 26 | POT_HEALING, quan=1, nn=0, dknown=T, bknown=F, dn="pink" | `"a pink potion"` |
| 27 | POT_HEALING, quan=2, nn=1, dknown=T, bknown=T, blessed=T, odiluted=1 | `"2 blessed diluted potions of healing"` |
| 28 | POT_WATER, nn=1, dknown=T, bknown=T, blessed=T | `"a potion of holy water"` |
| 29 | POT_WATER, nn=0, dknown=T, bknown=T, blessed=T, dn="clear" | `"a blessed clear potion"` |
| 30 | POT_WATER, nn=1, dknown=T, bknown=T, uncursed, implicit_uncursed=F | `"an uncursed potion of water"` |

### 12.7 神器

| # | 状态 | 输出 |
|---|------|------|
| 31 | LONG_SWORD, oartifact=EXCALIBUR, known=T, spe=+5, bknown=T, blessed=T, rknown=T, oerodeproof=1, fully_identified | `"the blessed rustproof +5 Excalibur"` |
| 32 | LONG_SWORD, oartifact=EXCALIBUR, known=F, dknown=T, bknown=F, not_fully_identified | `"a long sword named Excalibur"` |

**#32 说明**: `obj_is_pname` 返回 false (not fully identified), 所以走正常 WEAPON 路径生成 "long sword", 再由 `has_oname && dknown` 追加 " named Excalibur".

### 12.8 尸体

| # | 状态 | 输出 |
|---|------|------|
| 33 | CORPSE, corpsenm=PM_NEWT, quan=1, bknown=T, uncursed, implicit_uncursed=F | `"an uncursed newt corpse"` |
| 34 | CORPSE, corpsenm=PM_MEDUSA, quan=1, bknown=F | `"Medusa's corpse"` |
| 35 | CORPSE, corpsenm=PM_ORACLE, quan=1, bknown=F | `"the Oracle's corpse"` |

### 12.9 容器

| # | 状态 | 输出 |
|---|------|------|
| 36 | LARGE_BOX, cknown=T, empty, lknown=T, olocked=T, bknown=T, uncursed, implicit_uncursed=F | `"an empty uncursed locked large box"` |
| 37 | CHEST, cknown=T, 含 3 个物品堆, lknown=T, obroken=T, otrapped=T, tknown=T, dknown=T, bknown=T, cursed | `"a cursed trapped broken chest containing 3 items"` |

### 12.10 "called" 名称

| # | 状态 | 输出 |
|---|------|------|
| 38 | SCR_IDENTIFY, nn=0, dknown=T, dn="KERNOD WEL", un="id?", bknown=T, uncursed, implicit_uncursed=F | `"an uncursed scroll called id?"` |
| 39 | RIN_TELEPORTATION, nn=0, dknown=T, dn="jade", un="teleport", bknown=F | `"a jade ring called teleport"` |

**#39 说明**: 有 un 时使用 "ring called {un}", 不显示 dn "jade". 但等等 -- 代码路径是: `nn=false, un != NULL -> xcalled(buf, "ring", un)`. 结果是 "ring called teleport", 不含 dn.

> [疑似 bug] 对 RING_CLASS, 当 `un` 存在时, `xcalled(buf, "ring", un)` 丢弃了 dn. 这意味着玩家无法同时看到外观描述和 "called" 名. 但这是**设计意图**: "called" 名替代了外观描述作为识别标签. 如果同时显示两者, 就成了 `"jade ring called teleport"`, 但代码确实没走这条路. 检查 xname 的 RING_CLASS 分支: `elif un: xcalled(buf, "ring", un)` -- 是的, dn 被跳过. 这与 WAND/SCROLL/SPBOOK 一致, 与 WEAPON/TOOL 不同 (后者使用 `xcalled(buf, dn, un)`, 即 `"{dn} called {un}"`).

修正 #39 输出: `"a ring called teleport"` (无 dn, 无 bknown 信息).

---

## 附录 A: 关键常量

```
PREFIX   = 80     // doname 前缀预留空间 (字节)
BUFSZ    = 256    // 缓冲区大小 (平台相关, 通常 256)
NUMOBUF  = 12     // 循环缓冲区数量
SPE_LIM  = 99     // |obj->spe| 上限
MAX_ERODE = 3     // 最大侵蚀等级
PL_PSIZ  = 32     // 个人名称最大长度
```

## 附录 B: obj_is_pname() 条件

```
fn obj_is_pname(obj) -> bool:
    if !obj.oartifact || !has_oname(obj): return false
    if !gameover && !override_ID:
        if not_fully_identified(obj): return false
    return true
```

仅当神器已被完全鉴定时, xname 才跳过常规名称直接用神器名.

## 附录 C: the_unique_obj() 条件

```
fn the_unique_obj(obj) -> bool:
    known = obj.known || override_ID
    if !obj.dknown && !override_ID: return false
    if FAKE_AMULET_OF_YENDOR && !known: return true   // 伪装
    return oc_unique && (known || otyp == AMULET_OF_YENDOR)
```

## 附录 D: observe_object 副作用

`observe_object(obj)` 在 xname 中非盲非远距离时被调用:
- 设置 `obj->dknown = 1`
- 可能触发 `discover_object()` (更新发现列表)
- 对神器: 可能调用 `find_artifact()` (livelog 事件)
