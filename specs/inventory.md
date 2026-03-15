# NetHack 3.7 背包管理系统机制规格

源代码版本：NetHack-3.7 分支，基于 `src/invent.c`、`src/pickup.c`、`src/mkobj.c`、`src/hack.c`、`include/obj.h`、`include/hack.h`、`include/weight.h`。

---

## 1. 背包字母分配系统

### 1.1 槽位空间

```
invlet_basic = 52    -- a-z (26) + A-Z (26)
invlet_gold  = 1     -- '$' 专用于金币
invlet_overflow = 1  -- '#' 溢出槽（NOINVSYM）
invlet_max = 54      -- 总计
```

特殊符号：
- `$` (`GOLD_SYM`) — 金币专用槽，始终在链表头部
- `#` (`NOINVSYM`) — 当 52 个字母全部用尽时的溢出槽
- `>` (`CONTAINED_SYM`) — 容器内物品的标识符（仅显示用）
- `-` (`HANDS_SYM`) — 表示"空手"（仅显示用）

### 1.2 分配算法：`assigninvlet()`

伪代码：
```
FUNCTION assigninvlet(otmp):
    -- 金币直接分配 '$'
    IF otmp.oclass == COIN_CLASS:
        otmp.invlet = '$'
        RETURN

    -- 构建 inuse[0..51] 标记数组
    FOR EACH obj IN invent (跳过 otmp 自身):
        IF obj.invlet 在 'a'..'z':
            inuse[obj.invlet - 'a'] = TRUE
        ELSE IF obj.invlet 在 'A'..'Z':
            inuse[obj.invlet - 'A' + 26] = TRUE
        -- 如果现有物品已占用 otmp 的旧字母，清除 otmp.invlet
        IF obj.invlet == otmp.invlet:
            otmp.invlet = 0

    -- 如果 otmp 保留了一个有效的、未被占用的旧字母，直接使用
    IF otmp.invlet 在 'a'..'z' 或 'A'..'Z':
        RETURN

    -- 从 lastinvnr+1 开始循环搜索第一个空闲位置
    i = lastinvnr + 1
    WHILE i != lastinvnr:
        IF i == 52: i = 0; CONTINUE  -- 回绕到开头
        IF NOT inuse[i]: BREAK
        i = i + 1

    -- 分配字母
    IF inuse[i]:
        otmp.invlet = '#'  -- 所有 52 个都被占用
    ELSE IF i < 26:
        otmp.invlet = 'a' + i
    ELSE:
        otmp.invlet = 'A' + i - 26
    lastinvnr = i
```

关键特性：
- `lastinvnr`（0..51）是全局游戏状态，在分配间持久化，永远不保存/恢复
- 搜索从上一次分配的位置+1 开始（轮转），而非总是从 'a' 开始
- 物品若保留了之前的有效字母（且该字母未被其他物品占用），会沿用旧字母
- `fixinv` 选项（`flags.invlet_constant`）控制是否保持字母不变

### 1.3 `fixinv` vs `!fixinv` 模式

**fixinv（默认开启）**：
- 物品一旦获得字母就不再改变
- 新物品插入链表头部，然后调用 `reorder_invent()` 按字母序排列
- 排序键：`inv_rank(o) = o->invlet XOR 0x20`（040 八进制），效果是小写字母排在大写字母前面

**!fixinv**：
- 新物品追加到链表末尾
- 每次显示背包时调用 `reassign()` 重新编号
- `reassign()` 算法：
  1. 从链表中提取金币对象
  2. 对剩余物品按链表顺序依次分配 'a'..'z','A'..'Z'，超过 52 个用 '#'
  3. 金币赋予 '$' 并插回链表头部

### 1.4 链表排序：`reorder_invent()`

使用冒泡排序（预期只有一个元素乱序，所以效率可接受），按 `inv_rank()` 排序：

```
inv_rank(o) = o->invlet XOR 0x20

排序结果（升序）：
  '$' (0x24 XOR 0x20 = 0x04)  -- 金币在最前
  'a' (0x61 XOR 0x20 = 0x41)
  'b' (0x62 XOR 0x20 = 0x42)
  ...
  'z' (0x7A XOR 0x20 = 0x5A)
  'A' (0x41 XOR 0x20 = 0x61)
  'B' (0x42 XOR 0x20 = 0x62)
  ...
  'Z' (0x5A XOR 0x20 = 0x7A)
  '#' (0x23 XOR 0x20 = 0x03)  -- [疑似 bug，如实记录原始行为]
      '#' (NOINVSYM=0x23) 的 inv_rank 为 0x03，比 '$' 的 0x04 还小
      但实际上 '#' 溢出物品应排在最后。因为当所有 52 位都被占用时
      '#' 物品极少出现且生存期很短（很快会因合并或丢弃而消失），
      所以这个排序异常在实践中几乎不会被观察到。
```

---

## 2. 物品合并规则

### 2.1 合并入口

物品进入背包时的合并流程 (`addinv_core0`)：
1. 优先尝试与箭袋（uquiver）合并
2. 依次遍历整个背包链表，尝试与每个物品合并
3. 如果无法合并，调用 `assigninvlet()` 分配字母

### 2.2 可合并性判定：`mergable(otmp, obj)`

以下是完整的条件列表，按判断顺序排列。任何一条返回 FALSE 都会阻止合并：

```
FUNCTION mergable(otmp, obj) -> BOOLEAN:
    -- 阶段 0：基础排除
    IF obj == otmp: RETURN FALSE                      -- 同一对象
    IF obj.otyp != otmp.otyp: RETURN FALSE             -- 不同物品类型
    IF obj.nomerge OR otmp.nomerge: RETURN FALSE       -- 临时合并禁止标记
    IF NOT objects[obj.otyp].oc_merge: RETURN FALSE    -- 该类型不允许合并

    -- 阶段 1：金币特例
    IF obj.oclass == COIN_CLASS: RETURN TRUE           -- 金币总是合并

    -- 阶段 2：BUC 状态
    IF obj.cursed != otmp.cursed: RETURN FALSE
    IF obj.blessed != otmp.blessed: RETURN FALSE

    -- 阶段 3：丢失状态
    IF obj.how_lost == LOST_EXPLODING OR otmp.how_lost == LOST_EXPLODING:
        RETURN FALSE
    IF otmp.how_lost != LOST_NONE AND obj.how_lost != otmp.how_lost:
        RETURN FALSE

    -- 阶段 4：黏糊体（glob）特例
    IF obj.globby: RETURN TRUE                         -- glob 跳过后续检查

    -- 阶段 5：数值属性
    IF obj.unpaid != otmp.unpaid: RETURN FALSE
    IF obj.spe != otmp.spe: RETURN FALSE
    IF obj.no_charge != otmp.no_charge: RETURN FALSE
    IF obj.obroken != otmp.obroken: RETURN FALSE
    IF obj.otrapped != otmp.otrapped: RETURN FALSE     -- (武器为 opoisoned)
    IF obj.lamplit != otmp.lamplit: RETURN FALSE

    -- 阶段 6：食物特殊
    IF obj.oclass == FOOD_CLASS:
        IF obj.oeaten != otmp.oeaten: RETURN FALSE
        IF obj.orotten != otmp.orotten: RETURN FALSE   -- (orotten 即 oeroded)

    -- 阶段 7：外观和侵蚀
    IF obj.dknown != otmp.dknown: RETURN FALSE
    IF obj.bknown != otmp.bknown
       AND NOT Role_if(PM_CLERIC)
       AND (Blind OR Hallucination): RETURN FALSE
    IF obj.oeroded != otmp.oeroded: RETURN FALSE
    IF obj.oeroded2 != otmp.oeroded2: RETURN FALSE
    IF obj.greased != otmp.greased: RETURN FALSE

    -- 阶段 8：防侵蚀
    IF erosion_matters(obj):
        IF obj.oerodeproof != otmp.oerodeproof: RETURN FALSE
        IF obj.rknown != otmp.rknown AND (Blind OR Hallucination):
            RETURN FALSE

    -- 阶段 9：尸体/蛋/罐头特殊
    IF obj.otyp IN {CORPSE, EGG, TIN}:
        IF obj.corpsenm != otmp.corpsenm: RETURN FALSE

    -- 阶段 10：孵化蛋和可复活尸体
    IF obj.otyp == EGG AND (obj.timed OR otmp.timed): RETURN FALSE
    IF obj.otyp == CORPSE AND otmp.corpsenm >= LOW_PM
       AND is_reviver(mons[otmp.corpsenm]): RETURN FALSE

    -- 阶段 11：蜡烛年龄
    IF Is_candle(obj) AND (obj.age / 25) != (otmp.age / 25): RETURN FALSE

    -- 阶段 12：燃烧中的油
    IF obj.otyp == POT_OIL AND obj.lamplit: RETURN FALSE

    -- 阶段 13：商店价格
    IF obj.unpaid AND NOT same_price(obj, otmp): RETURN FALSE

    -- 阶段 14：额外数据
    IF has_omonst(obj) OR has_omid(obj)
       OR has_omonst(otmp) OR has_omid(otmp): RETURN FALSE

    -- 阶段 15：命名
    objnamelth = strlen(safe_oname(obj))
    otmpnamelth = strlen(safe_oname(otmp))
    -- 规则：两个都有名字但名字不同 → 不合并
    --        其中一个有名字另一个没有 → 仅尸体不合并，其他允许
    IF (objnamelth != otmpnamelth
        AND ((objnamelth AND otmpnamelth) OR obj.otyp == CORPSE))
       OR (objnamelth AND otmpnamelth AND ONAME(obj) != ONAME(otmp)):
        RETURN FALSE

    -- 阶段 16：邮件命令
    IF 邮件命令不匹配: RETURN FALSE

    -- 阶段 17：邮件卷轴变体 (MAIL_STRUCTURES)
    IF obj.otyp == SCR_MAIL AND obj.spe > 0:
        IF (obj.o_id % 2) != (otmp.o_id % 2): RETURN FALSE

    -- 阶段 18：神器
    IF obj.oartifact != otmp.oartifact: RETURN FALSE

    -- 阶段 19：known 状态（盲/幻觉时）
    IF obj.known != otmp.known AND (Blind OR Hallucination): RETURN FALSE

    RETURN TRUE
```

### 2.3 `oc_merge` 标志

`oc_merge` 是 `struct objclass` 中的位字段（`include/objclass.h`），由 `BITS()` 宏的第 2 个参数设置。只有该标志为 1 的物品类型才可能合并。

各物品类定义宏中 merge 的默认值：

| 定义宏 | merge | 说明 |
|--------|-------|------|
| `PROJECTILE()` | 始终 1 | 箭、弩箭等弹药 |
| `WEAPON()` | 参数 `mg` | 飞镖、飞镖、手里剑等可叠加武器为 1；剑、斧等为 0 |
| `BOW()` | 始终 0 | 弓、弩、弹弓 |
| `ARMOR()` | 始终 0 | 所有盔甲 |
| `RING()` | 始终 0 | 所有戒指 |
| `AMULET()` | 始终 0 | 所有护身符 |
| `FOOD()` | 始终 1 | 所有食物（但尸体/蛋/罐头受额外限制） |
| `POTION()` | 始终 1 | 所有药水 |
| `SCROLL()` | 始终 1 | 所有卷轴 |
| `WAND()` | 始终 0 | 所有魔杖（每根有独立充能） |
| `SPBOOK()` | 始终 0 | 所有法术书 |
| `TOOL()` | 参数 `mrg` | 大部分为 0；蜡烛等少数为 1 |
| `CONTAINER()` | 始终 0 | 所有容器 |
| `GEM()` | 始终 1 | 所有宝石 |
| `ROCK()` | 始终 1 | 岩石（但巨石因重量不可能满足合并条件） |
| `COIN_CLASS` | 始终 1 | 金币（在 `mergable()` 中有特殊快速路径） |
| `BALL_CLASS` / `CHAIN_CLASS` | 0 | 铁球和铁链 |
| `VENOM_CLASS` | 0 | 毒液 |

核心规则：`oc_merge == 0` 的物品**永远不会合并**，即使其他所有条件都匹配。

### 2.4 合并时的年龄计算

```
IF NOT obj.lamplit AND NOT obj.globby:
    otmp.age = (otmp.age * otmp.quan + obj.age * obj.quan)
               / (otmp.quan + obj.quan)
```
加权平均。燃烧中的物品和 glob 不做年龄合并。

### 2.5 合并时的鉴定推理

当两个物品合并时，如果某个鉴定维度不一致，该维度变为已知：
- `known` 不同 → 合并后 `known = 1`
- `rknown` 不同 → 合并后 `rknown = 1`（如果有防侵蚀属性，触发发现）
- `bknown` 不同 → 合并后 `bknown = 1`（非牧师触发发现）

合并后如果确实发现了新信息，打印 "You learn more about your items by comparing them."（但投掷物品 `how_lost == LOST_THROWN` 时抑制此消息以避免刷屏）。

### 2.6 `merge_choice()`

此函数在 52 个槽位都被占用时使用，检查一个地面物品是否能与背包中任何物品合并。特殊处理：
- `SCR_SCARE_MONSTER` 始终返回 NULL（不尝试合并）
- 如果物品在有店主的商店地面上且 `no_charge` 为 0，则返回 NULL（因为拾取后会变成 unpaid，不会再合并）

---

## 3. 背包重量限制

### 3.1 负重能力：`weight_cap()`

```
carrcap = 25 * (ACURRSTR + ACURR(A_CON)) + 50
```

常量（来自 `include/weight.h`）：
- `WT_WEIGHTCAP_STRCON = 25`
- `WT_WEIGHTCAP_SPARE = 50`
- `MAX_CARR_CAP = 1000`

特殊调整：
```
IF Upolyd:
    IF 当前形态为仙女(nymph): carrcap = MAX_CARR_CAP (1000)
    ELSE IF 当前形态 cwt == 0:
        carrcap = carrcap * 当前形态.msize / MZ_HUMAN
    ELSE IF 非强壮怪物 OR (强壮怪物 AND cwt > WT_HUMAN(1450)):
        carrcap = carrcap * 当前形态.cwt / WT_HUMAN(1450)

IF Levitation OR 在空气层 OR (骑乘且坐骑为强壮怪物):
    carrcap = MAX_CARR_CAP (1000)
ELSE:
    carrcap = min(carrcap, MAX_CARR_CAP)
    IF NOT Flying:
        IF 左腿受伤: carrcap -= 100 (WT_WOUNDEDLEG_REDUCT)
        IF 右腿受伤: carrcap -= 100

RETURN max(carrcap, 1)  -- 永远不返回 0
```

### 3.2 当前负重：`inv_weight()`

```
wt = 0
FOR EACH otmp IN invent:
    IF otmp.oclass == COIN_CLASS:
        wt += (otmp.quan + 50) / 100   -- 金币重量公式
    ELSE IF otmp.otyp != BOULDER OR NOT throws_rocks(当前形态):
        wt += otmp.owt                  -- 巨人搬石头不计重
RETURN wt - weight_cap()
```

返回值为负表示低于负重上限，为正表示超出。

### 3.3 负重等级：`calc_capacity(xtra_wt)`

```
wt = inv_weight() + xtra_wt
IF wt <= 0: RETURN UNENCUMBERED (0)
IF weight_cap <= 1: RETURN OVERLOADED (5)
cap = (wt * 2 / weight_cap) + 1
RETURN min(cap, OVERLOADED)
```

等级定义：
```
UNENCUMBERED = 0   -- 无负担
SLT_ENCUMBER = 1   -- Burdened（负担）
MOD_ENCUMBER = 2   -- Stressed（压力）
HVY_ENCUMBER = 3   -- Strained（吃力）
EXT_ENCUMBER = 4   -- Overtaxed（超载）
OVERLOADED   = 5   -- Overloaded（过载）
```

等级对应的重量区间（设 WC = weight_cap()）：
```
UNENCUMBERED:  inv_weight() <= 0        (即携带总重 <= WC)
SLT_ENCUMBER:  0 < inv_weight() <= WC/2   (cap = 1)
MOD_ENCUMBER:  WC/2 < inv_weight() <= WC  (cap = 2)
HVY_ENCUMBER:  WC < inv_weight() <= 3*WC/2 (cap = 3)
EXT_ENCUMBER:  3*WC/2 < inv_weight() <= 2*WC (cap = 4)
OVERLOADED:    inv_weight() > 2*WC       (cap = 5)
```

`near_capacity()` = `calc_capacity(0)`
`max_capacity()` = `inv_weight() - 2 * weight_cap()`

### 3.4 金币重量公式

```
GOLD_WT(n) = (n + 50) / 100
```

即每 100 金币重 1 单位（向下取整，但含 +50 的修正偏移）。最小重量为 1（3.7 新增）。

### 3.5 槽位限制

- 非金币物品最多 52 个独立堆栈（`inv_cnt(FALSE) >= invlet_basic`）
- 金币不计入 52 限制
- 超过限制时，如果新物品无法与现有物品合并，拒绝拾取

---

## 4. 背包显示排序

### 4.1 `sortpack` 选项

当 `flags.sortpack` 为 TRUE（默认）时，背包按 `inv_order` 中的类别顺序分组显示。

默认 `inv_order`：
```
COIN_CLASS, AMULET_CLASS, WEAPON_CLASS, ARMOR_CLASS, FOOD_CLASS,
SCROLL_CLASS, SPBOOK_CLASS, POTION_CLASS, RING_CLASS, WAND_CLASS,
TOOL_CLASS, GEM_CLASS, ROCK_CLASS, BALL_CLASS, CHAIN_CLASS
```

玩家可通过 `packorder` 选项自定义此顺序。

### 4.2 `sortloot` 选项

```
'n' (none)  — 仅按背包字母排序
'l' (loot)  — 拾取/地面物品列表使用排序
'f' (full)  — 拾取和背包显示都使用排序
```

### 4.3 排序比较器：`sortloot_cmp()`

按以下优先级排序，从高到低：

**模式 SORTLOOT_INUSE（仅限"正在使用"显示）：**
- 按 in-use 评级降序（高评级优先）
- 评级基于 `inuse_classify()`，分为 4 组：
  1. Accessories（护身符 > 主手戒指 > 副手戒指 > 盲眼布）
  2. Weapons（主武器 > 副武器 > 箭袋）
  3. Armor（身体甲 > 斗篷 > 盾牌 > 头盔 > 手套 > 靴子 > 衬衫）
  4. Miscellaneous（使用中的绳索/灯光工具）

**正常排序模式：**

1. **类别顺序（orderclass）**
   - 由 `sortpack` 决定使用 `inv_order`（玩家自定义）还是 `def_srt_order`（内置拾取优化序）
   - `def_srt_order` = COIN, AMULET, RING, WAND, POTION, SCROLL, SPBOOK, GEM, FOOD, TOOL, WEAPON, ARMOR, ROCK, BALL, CHAIN

2. **子类别（subclass）**（仅在非 sortpack+invlet 模式下）
   - 盔甲：头盔(1) > 手套(2) > 靴子(3) > 盾牌(4) > 斗篷(5) > 衬衫(6) > 身体甲(7)
   - 武器：弹药(1) > 发射器(2) > 投射物(3) > 可叠加(4) > 其他(5) > 长柄(6)
   - 工具：容器(1) > 已知伪容器(2) > 乐器(3) > 其他(4)
   - 食物：果实(1) > 普通食物(2) > 罐头(3) > 蛋(4) > 尸体(5) > glob(6)
   - 宝石：复杂的按材质/发现状态分组（1-8）

3. **发现状态（disco）**
   - 1=未见, 2=已见未鉴定, 3=已命名, 4=已鉴定

4. **背包字母（invlet）**（SORTLOOT_INVLET 模式）
   - 使用 `invletter_value(c)`：'$'=1, 'a'-'z'=2..27, 'A'-'Z'=28..53, '#'=54

5. **字母序（SORTLOOT_LOOT 模式）**
   - 使用 `loot_xname()` 生成排序用名称（去除前缀如 diluted/holy）
   - 毛巾加后缀使湿→潮→干排序
   - glob 加后缀使小→大排序

6. **BUC 状态**：blessed(3) > uncursed(2) > cursed(1) > unknown(0) — 大值优先

7. **涂油**：涂油优先

8. **侵蚀**：侵蚀少的优先

9. **防侵蚀**：已知防侵蚀优先

10. **附魔**：高附魔优先

11. **稳定排序兜底**：相同则保持原始顺序 (`indx`)

---

## 5. 物品选择菜单

### 5.1 `PICK_NONE / PICK_ONE / PICK_ANY`

```
PICK_NONE = 0  -- 仅显示，不可选择
PICK_ONE  = 1  -- 只能选一个
PICK_ANY  = 2  -- 可选多个
```

`select_menu()` 返回值：
- `> 0`：选中项数量
- `== 0`：什么都没选（正常退出）
- `< 0`（`-1`）：用户按 ESC 取消

对于 `PICK_ONE`，如果有预选项（preselected），选择另一项时返回 `n=2`（第一个为新选择，第二个为旧预选）；重新选择同一项时返回 `n=1`。

### 5.2 `query_objlist()` 查询标志

```
BY_NEXTHERE       -- 用 obj->nexthere 遍历（地面物品链）
AUTOSELECT_SINGLE -- 如果只有 1 个合格物品，自动选择不弹菜单
USE_INVLET        -- 使用物品的背包字母
INVORDER_SORT     -- 使用玩家的 packorder 排序
INCLUDE_HERO      -- 被吞噬时显示英雄
SIGNAL_NOMENU     -- 如果没有合格物品返回 -1 而非 0
SIGNAL_ESCAPE     -- 如果玩家按 ESC 返回 -1 而非 0
FEEL_COCKATRICE   -- 触发鸡蛇尸体触摸检查
```

---

## 6. 快速丢弃（`#droptype` / doddrop）

### 6.1 菜单样式行为

- **Traditional**：调用 `ggetobj("drop", drop, 0, ...)`，按类别符号选择
  - 'a' 全部拾取该类、'A' 逐个确认、'm' 转菜单模式
  - 'B'/'U'/'C'/'X'/'P' 按 BUC 状态/刚拾取过滤
- **Full / Combination**：使用 `menu_drop()` 弹出分类选择菜单
  - 可选 "All types"、按类别、按 BUC 状态、'P' (Just picked up)
  - 选 'A' (drop everything) 配合 paranoid 确认

### 6.2 丢弃限制

不可丢弃的物品：
- 焊接的武器（cursed wielded weapon，通过 `welded()` 检查）
- 被诅咒的 LOADSTONE（使用 `corpsenm` 字段临时记录分割数量的 kludge）
- 球和链（uball / uchain）
- 必须先脱下穿戴中的装备

### 6.3 安全提示

丢弃鸡蛇尸体时，如果没有手套保护且无石化抗性，会询问确认（`better_not_try_to_drop_that()`）。

---

## 7. 自动拾取规则

### 7.1 `autopick_testobj()` 判定流程

```
FUNCTION autopick_testobj(otmp):
    -- 1. 商店地面的待售物品：拒绝
    IF costly AND NOT otmp.no_charge: RETURN FALSE

    -- 2. 投掷/被盗覆盖：这些标志优先于类型过滤和例外
    IF flags.pickup_thrown AND otmp.how_lost == LOST_THROWN: RETURN TRUE
    IF flags.pickup_stolen AND otmp.how_lost == LOST_STOLEN: RETURN TRUE
    IF flags.nopick_dropped AND otmp.how_lost == LOST_DROPPED: RETURN FALSE
    IF otmp.how_lost == LOST_EXPLODING: RETURN FALSE

    -- 3. 类型过滤
    pickit = (pickup_types 为空) OR (otmp.oclass 在 pickup_types 中)

    -- 4. 自动拾取例外规则（正则匹配物品描述）
    ape = check_autopickup_exceptions(otmp)
    IF ape: pickit = ape.grab  -- grab=TRUE 强制拾取，grab=FALSE 强制不拾取

    RETURN pickit
```

### 7.2 配置选项

- `flags.pickup` — 全局自动拾取开关（`autopickup` 选项）
- `flags.pickup_types` — 类别字符串，如 `"$?!/"`
- `flags.pickup_thrown` — 自动拾取你投掷过的物品
- `flags.pickup_stolen` — 自动拾取被怪物偷走的物品
- `flags.nopick_dropped` — 不自动拾取你丢弃的物品
- `flags.pickup_burden` — 负重限制等级（默认 `MOD_ENCUMBER`=2，即 Stressed）
  - 仅在新物品会使负重超过 `max(当前等级, pickup_burden)` 时提示

### 7.3 自动拾取例外

通过 `autopickup_exception` 配置，格式：
```
AUTOPICKUP_EXCEPTION="<pattern"    -- 不拾取匹配的物品
AUTOPICKUP_EXCEPTION=">pattern"    -- 拾取匹配的物品
```

匹配使用正则表达式，对物品的 `doname()` 单数形式进行匹配。

### 7.4 拾取流程中的负重检查

`lift_object()` 中：
```
prev_encumbr = max(near_capacity(), flags.pickup_burden)
next_encumbr = calc_capacity(新物品重量变化)

IF next_encumbr > prev_encumbr:
    IF telekinesis: 拒绝拾取
    ELSE: 根据等级显示提示消息并询问：
        EXT_ENCUMBER+: "You have extreme difficulty ..."
        HVY_ENCUMBER:  "You have much trouble ..."
        MOD_ENCUMBER:  "You have trouble ..."
        SLT_ENCUMBER:  "You have a little trouble ..."
        选项：y(继续) / n(跳过此物) / q(停止所有拾取)
```

---

## 8. 容器交互

### 8.1 容器类型

```
Is_container(o) = (o->otyp >= LARGE_BOX AND o->otyp <= BAG_OF_TRICKS)
Is_box(o) = (o->otyp == LARGE_BOX OR o->otyp == CHEST)
Is_mbag(o) = (o->otyp == BAG_OF_HOLDING OR o->otyp == BAG_OF_TRICKS)
```

### 8.2 放入容器限制（`in_container`）

以下物品无法放入容器：
- 球和链（uball, uchain）
- 容器自身（"an interesting topological exercise"）
- 穿戴中的盔甲/饰品（`owornmask & (W_ARMOR | W_ACCESSORY)`）
- 被诅咒的 LOADSTONE
- 四大神器：AMULET_OF_YENDOR, CANDELABRUM_OF_INVOCATION, BELL_OF_OPENING, SPE_BOOK_OF_THE_DEAD
- 拴着宠物的绳索
- 冰箱、箱子、大箱子、巨石
- 大型怪物的雕像（`bigmonst`）

放入焊接武器会先尝试解除焊接（失败则取消）。

### 8.3 储物袋爆炸（Bag of Holding）

```
FUNCTION mbag_explodes(obj, depthin):
    -- 空的取消魔杖或空的戏法袋不会爆炸
    IF (obj.otyp == WAN_CANCELLATION OR obj.otyp == BAG_OF_TRICKS)
       AND obj.spe <= 0: RETURN FALSE

    -- 爆炸概率
    IF (Is_mbag(obj) OR obj.otyp == WAN_CANCELLATION):
        IF rn2(1 << min(depthin, 7)) <= depthin: RETURN TRUE

    -- 递归检查容器内容
    FOR EACH otmp IN obj.cobj:
        IF mbag_explodes(otmp, depthin + 1): RETURN TRUE

    RETURN FALSE
```

爆炸概率表（depthin 为嵌套深度，从 0 开始）：
```
depthin=0: rn2(1) <= 0 → 1/1 = 100%
depthin=1: rn2(2) <= 1 → 2/2 = 100%
depthin=2: rn2(4) <= 2 → 3/4 = 75%
depthin=3: rn2(8) <= 3 → 4/8 = 50%
depthin=4: rn2(16) <= 4 → 5/16 = 31.25%
depthin=5: rn2(32) <= 5 → 6/32 = 18.75%
depthin=6: rn2(64) <= 6 → 7/64 ≈ 10.9%
depthin=7+: rn2(128) <= depthin → (depthin+1)/128
```

注意：将一个 Bag of Holding 放入另一个 Bag of Holding 时，`depthin=0`，`rn2(1)` 总是返回 0，`0 <= 0` 为真，因此**必定爆炸**。

爆炸后果：
- 6d6 点伤害
- 容器被销毁
- 内容物以 1/13 概率逐个销毁，其余散落在周围

### 8.4 储物袋重量计算

```
容器总重 = 容器自身重量 + 调整后的内容物重量

内容物重量调整（仅 BAG_OF_HOLDING）：
  cursed:    cwt * 2
  blessed:   (cwt + 3) / 4    -- 向上取整
  uncursed:  (cwt + 1) / 2    -- 向上取整
  (cwt = 内容物实际总重)
```

### 8.5 诅咒储物袋的物品丢失

每次打开诅咒的储物袋时 (`boh_loss`)：
```
FOR EACH item IN bag.contents:
    IF rn2(13) == 0:  -- 1/13 概率
        物品被销毁
```

### 8.6 冰箱特殊行为

- 放入时：`obj.age = moves - obj.age`（转为相对年龄，冻结计时器）
- 尸体计时器（腐烂、复活）被停止
- 取消的冰巨魔尸体在冰箱中会解除取消
- 取出时：`obj.age = moves - obj.age`（恢复绝对年龄），重启计时器

### 8.7 从容器取出

`out_container()` 流程：
1. 检查神器触摸 (`touch_artifact`)
2. 检查致命尸体 (`fatal_corpse_mistake`)
3. 调用 `lift_object()` 检查重量限制
4. 从容器提取，更新容器重量
5. 如果是冰箱，调用 `removed_from_icebox()`
6. 如果在商店内取出非 unpaid 物品，加入账单
7. 调用 `addinv()` 加入背包

---

## 9. 物品鉴定状态追踪

### 9.1 鉴定位字段

struct obj 中的鉴定相关位字段：

| 字段 | 含义 |
|------|------|
| `known` | 精确性质已知（附魔值、充能数等） |
| `dknown` | 外观描述已知（物品被近距离观察过） |
| `bknown` | BUC 状态已知 |
| `rknown` | 防侵蚀状态已知 |
| `cknown` | 容器内容已知（也用于罐头和丰饶之角） |
| `lknown` | 锁定状态已知 |
| `tknown` | 陷阱状态已知（箱子） |

### 9.2 类鉴定状态

全局的类型级鉴定存储在 `objects[otyp]` 中：
- `oc_name_known` — 该类型已被鉴定（如"治愈药水"），对所有同类物品生效
- `oc_uname` — 玩家给该外观命的名（如"标记为紫色的药水为治愈"）

### 9.3 发现列表中的排序

在 `sortloot_cmp` 的 disco 字段中：
```
1 = 未见（Blind 或从未观察到的）
2 = 已见但未鉴定
3 = 已命名（部分鉴定）
4 = 已鉴定（或无描述需鉴定）
```

---

## 10. 交易物品追踪（Shop Items）

### 10.1 相关字段

- `obj.unpaid` — 位标志，表示物品由商店拥有但在英雄手中
- `obj.no_charge` — 位标志，表示商店不应收费（样品、垃圾等）
- 有效位置：`unpaid` 对英雄背包中或其中容器内的物品有效

### 10.2 拾取时的计费

`pick_obj()` 中：
```
IF 在商店内且是昂贵地点:
    addtobill(otmp, TRUE, FALSE, FALSE)  -- 加入商店账单
    -- 这会设置 obj.unpaid
IF 拾取点在商店内但英雄在商店外:
    remote_burglary(ox, oy)  -- 通知店主偷窃
```

### 10.3 放入容器时的处理

当在商店地面上操作容器时：
- 放入物品调用 `sellobj()` 处理账单
- 金币放入后额外调用 `sellobj()` 记入信用
- 如果容器有 `no_charge` 标记（你自己的容器），使用 `SELL_DONTSELL` 状态

### 10.4 合并约束

`mergable()` 检查 `unpaid` 必须相同。`merge_choice()` 中，如果物品在有店主的商店地面上且非 `no_charge`，直接拒绝合并（因为拾取后会获得 unpaid 标记，不再匹配）。

---

## 11. 背包字母调整命令（#adjust）

### 11.1 命令入口

`doorganize()` / `doorganize_core()`：

### 11.2 功能

1. **移动**：将物品 A 的字母改为 B
   - 如果 B 槽已有不可合并物品，两者交换字母（"Swapping:"）
   - 如果 B 槽已有可合并物品，合并（"Merging:"）
2. **收集**：选择物品后选择相同字母 → 收集所有兼容堆栈到该槽（"Collecting:"）
3. **分割**：对堆叠物品指定数量后移至新槽
   - 分割后如果目标槽有可合并物品，合并（"Splitting and merging:"）
   - 如果目标槽有不可合并物品且背包已满，取消分割

### 11.3 特殊规则

- `$` 槽只能放金币（`obj.oclass == COIN_CLASS`）
- `#` 溢出槽可以作为目标（前提是已有物品在用）
- 该操作不使用 `freeinv/addinv`，避免重复触发神器效果、灯光熄灭、运气变化等

### 11.4 可选目标显示

按 `?` 显示当前使用中的字母表，标出哪些槽被占用、哪些可用、哪些可合并。

---

## 12. 物品分割（splitobj）

### 12.1 分割操作：`splitobj(obj, num)`

将一个堆叠物品拆分为两个独立对象。`obj` 保留 `quan - num` 件，返回新对象持有 `num` 件。

```
FUNCTION splitobj(obj, num) -> new_obj:
    -- 前置检查
    ASSERT obj.cobj == NULL    -- 容器不可分割
    ASSERT num > 0
    ASSERT obj.quan > num

    -- 创建新对象（完整复制 struct obj）
    otmp = newobj()
    *otmp = *obj               -- 结构体整体复制
    otmp.oextra = NULL         -- 额外数据单独复制
    otmp.o_id = nextoid(obj, otmp)  -- 分配新 o_id（保持商店价格一致）
    otmp.timed = 0             -- 新对象暂无计时器
    otmp.lamplit = 0           -- 新对象不燃烧
    otmp.owornmask = 0         -- 新对象未穿戴
    otmp.lua_ref_cnt = 0
    otmp.pickup_prev = 0

    -- 调整数量和重量
    obj.quan -= num
    obj.owt = weight(obj)
    otmp.quan = num
    otmp.owt = weight(otmp)

    -- 记录分割上下文（用于撤销）
    context.objsplit.parent_oid = obj.o_id
    context.objsplit.child_oid = otmp.o_id

    -- 插入链表：otmp 紧接在 obj 之后
    obj.nobj = otmp

    -- 如果在地面上，也插入 nexthere 链
    IF obj.where == OBJ_FLOOR:
        obj.nexthere = otmp

    -- 处理账单分割、额外数据复制、计时器分割、光源分割
    IF obj.unpaid: splitbill(obj, otmp)
    copy_oextra(otmp, obj)
    IF has_omid(otmp): free_omid(otmp)  -- m_id 关联只保留一份
    IF obj.timed: obj_split_timers(obj, otmp)
    IF obj_sheds_light(obj): obj_split_light_source(obj, otmp)

    RETURN otmp
```

### 12.2 撤销分割：`unsplitobj(obj)`

尝试找到分割的父对象并重新合并。通过 `context.objsplit` 中记录的 parent_oid / child_oid 追踪。

```
FUNCTION unsplitobj(obj) -> merged_obj OR NULL:
    IF context.objsplit.child_oid != obj.o_id: RETURN NULL
    -- 在 obj 的前一个链表节点中寻找 parent
    parent = find_obj_by_oid(context.objsplit.parent_oid)
    IF parent AND merged(&parent, &obj): RETURN parent
    RETURN NULL
```

### 12.3 `nextoid()` — 商店价格保持

分割时新对象的 `o_id` 不是简单递增，而是搜索一个使 `oid_price_adjustment()` 结果与原对象一致的值。这确保分割不会改变商店定价（因为某些物品的价格受 `o_id` 影响）。

```
FUNCTION nextoid(oldobj, newobj) -> unsigned:
    olddif = oid_price_adjustment(oldobj, oldobj.o_id)
    oid = context.ident - 1
    DO:
        oid += 1
        IF oid == 0: oid = 1     -- 避免使用 0
        newdif = oid_price_adjustment(newobj, oid)
    WHILE newdif != olddif AND trylimit-- > 0
    context.ident = oid
    next_ident()                  -- 推进全局 ident（+rnd(2)）
    RETURN oid
```

### 12.4 分割触发场景

- `getobj()` 中玩家指定数量（如 "5a" 表示使用 5 个 'a' 物品）
- `#adjust` 命令的分割子功能
- `pickup_object()` 中只拾取部分堆叠
- `out_container()` 中只取出部分堆叠

---

## 13. 物品重量计算：`weight(obj)`

### 13.1 完整算法

```
FUNCTION weight(obj) -> int:
    wt = objects[obj.otyp].oc_weight   -- 单个物品的基础重量

    -- 数量为 0 的异常情况
    IF obj.quan < 1: RETURN 0  (+ impossible 消息)

    -- Glob 特殊：重量由吸收机制管理，直接返回当前 owt
    IF obj.globby: RETURN obj.owt

    -- 容器和雕像：自身重量 + 内容物重量
    IF Is_container(obj) OR obj.otyp == STATUE:
        IF obj.otyp == STATUE AND ismnum(obj.corpsenm):
            msize = mons[obj.corpsenm].msize  -- 0..7
            minwt = (msize * 2 + 1) * 100
            wt = 3 * mons[obj.corpsenm].cwt / 2  -- 1.5 倍尸体重量
            IF wt < minwt: wt = minwt

        cwt = SUM(weight(c) FOR c IN obj.cobj)  -- 递归计算内容物

        -- Bag of Holding 重量减免
        IF obj.otyp == BAG_OF_HOLDING:
            IF obj.cursed:  cwt = cwt * 2
            ELIF obj.blessed: cwt = (cwt + 3) / 4
            ELSE:             cwt = (cwt + 1) / 2

        RETURN wt + cwt

    -- 尸体：使用怪物重量
    IF obj.otyp == CORPSE AND ismnum(obj.corpsenm):
        long_wt = obj.quan * mons[obj.corpsenm].cwt
        wt = min(long_wt, LARGEST_INT)
        IF obj.oeaten: wt = eaten_stat(wt, obj)
        RETURN wt

    -- 部分食用的食物
    IF obj.oclass == FOOD_CLASS AND obj.oeaten:
        RETURN eaten_stat(obj.quan * wt, obj)

    -- 金币
    IF obj.oclass == COIN_CLASS:
        wt = (obj.quan + 50) / 100
        RETURN max(wt, 1)   -- 3.7: 至少 1 单位

    -- 加重铁球
    IF obj.otyp == HEAVY_IRON_BALL AND obj.owt != 0:
        RETURN obj.owt

    -- 烛台（加上蜡烛重量）
    IF obj.otyp == CANDELABRUM_OF_INVOCATION AND obj.spe:
        RETURN wt + obj.spe * objects[TALLOW_CANDLE].oc_weight

    -- 通用：基础重量 * 数量（重量为 0 的物品用 (quan+1)/2）
    IF wt: RETURN wt * obj.quan
    ELSE:  RETURN (obj.quan + 1) / 2
```

### 13.2 `delta_cwt()` — Bag of Holding 取出重量差

从 Bag of Holding 中取出物品时，负重变化不等于物品的 `owt`，需要计算容器的实际重量变化：

```
FUNCTION delta_cwt(container, obj) -> int:
    IF container.otyp != BAG_OF_HOLDING:
        RETURN obj.owt           -- 普通容器，重量变化 = 物品重量

    -- 对于 BoH：临时移除物品，计算容器重量差
    owt = container.owt
    临时从 container.cobj 链表中移除 obj
    nwt = weight(container)
    恢复 obj 到链表
    RETURN owt - nwt
```

这意味着从 blessed BoH 中取出 100 重量的物品，容器实际减重 `(cwt+3)/4 - (cwt-100+3)/4`，而非 100。

### 13.3 `container_weight()` — 递归更新

当容器内容变化时（放入/取出物品），需要递归更新所有外层容器的重量：

```
FUNCTION container_weight(object):
    object.owt = weight(object)
    IF object.where == OBJ_CONTAINED:
        container_weight(object.ocontainer)  -- 递归更新外层
```

---

## 14. 持久背包窗口（Persistent Inventory）

### 14.1 概述

当 `iflags.perm_invent` 为 TRUE 时，窗口系统维护一个持久显示的背包窗口（`WIN_INVEN`）。此窗口在背包内容变化时自动刷新。

### 14.2 更新机制：`update_inventory()`

```
FUNCTION update_inventory():
    -- 仅在主游戏循环中更新
    IF NOT program_state.in_moveloop: RETURN
    IF suppress_map_output(): RETURN  -- 恢复游戏时抑制

    -- 临时恢复正常价格显示（避免商店交互期间的抑制影响持久窗口）
    save = iflags.suppress_price
    iflags.suppress_price = 0
    windowprocs.win_update_inventory(0)
    iflags.suppress_price = save
```

### 14.3 更新触发点

以下操作会调用 `update_inventory()`：
- `addinv_core0()` — 物品加入背包（可通过参数抑制）
- `freeinv()` — 物品移出背包
- `useup()` / `useupall()` — 物品消耗
- `#adjust` — 字母调整后
- `out_container()` — 从容器取出物品后

### 14.4 显示模式

持久窗口支持多种显示模式（通过 `wri_info.fromcore.invmode` 位掩码控制）：
- `InvShowGold` — 显示金币条目
- `InvInUse` — 仅显示正在使用的物品（使用 `SORTLOOT_INUSE` 排序）

对于 TTY 接口，持久背包窗口强制使用 `SORTLOOT_INVLET` 排序（不使用 sortpack），以保持简洁。

### 14.5 `#perminv` 命令

`doperminv()` 命令允许玩家与持久背包窗口交互（如滚动）：
- 如果窗口系统不支持（`WC_PERM_INVENT` capability 缺失）：打印错误
- 如果选项未启用：提示启用
- 如果背包为空：提示为空
- 否则：调用 `win_update_inventory(1)` 进入交互模式

### 14.6 `display_pickinv()` 双模式

`display_pickinv()` 函数既用于交互式背包查看（返回选中字母），也用于持久背包窗口的内容填充。两种模式的区别：

| 特性 | 交互模式 | 持久窗口模式 |
|------|----------|--------------|
| 窗口 | `cached_pickinv_win`（临时） | `WIN_INVEN`（持久） |
| 菜单行为 | `MENU_BEHAVE_STANDARD` | `MENU_BEHAVE_PERMINV` |
| 空背包 | 打印 "Not carrying anything" | 仍然更新（清除旧内容） |
| 单物品 | 使用 message_menu 快捷显示 | 使用完整菜单 |
| 金币显示 | 始终显示 | 受 `InvShowGold` 控制 |

---

## 15. 背包物品计数：`inv_cnt()`

```
FUNCTION inv_cnt(incl_gold) -> int:
    ct = 0
    FOR EACH otmp IN invent:
        IF incl_gold OR otmp.invlet != '$':
            ct += 1
    RETURN ct
```

- `inv_cnt(FALSE)` — 不含金币的物品数（用于 52 槽位限制检查）
- `inv_cnt(TRUE)` — 含金币的物品数

---

## 16. 其他重要机制

### 16.1 `hold_another_object()`

用于被动获得物品（许愿、偷窃归还等）。流程：
1. 如果是神器，先检查是否可以触摸
2. 如果 Fumbling，物品掉落
3. 如果是致命尸体且是许愿所得，物品掉落
4. 调用 `addinv_core0()` 尝试加入背包
5. 如果背包满或超重：撤销合并，物品掉落
6. 否则成功持有

### 16.2 投掷物品的箭袋自动填充

在 `addinv_core0()` 中：
```
IF obj_was_thrown AND flags.pickup_thrown AND uquiver 为空
   AND obj 非 Mjollnir AND obj 非 AKLYS
   AND (obj 是投掷武器 OR obj 是弹药):
    setuqwep(obj)  -- 自动放入箭袋
```

### 16.3 `autoquiver` 选项

在 `hold_another_object()` 中：
```
IF flags.autoquiver AND uquiver 为空 AND obj 非穿戴中
   AND (obj 是投射物 OR obj 是当前武器的弹药 OR obj 是副武器的弹药):
    setuqwep(obj)
```

---

## 测试向量

### TV1: 基础字母分配

```
输入: 空背包，lastinvnr=0，拾取一把短剑
期望: 短剑获得字母 'b'（lastinvnr+1=1 → index 1 → 'b'）
     lastinvnr 更新为 1
```

### TV2: 金币字母分配

```
输入: 任何背包状态，拾取金币
期望: 金币获得 '$'，插入链表头部
     lastinvnr 不变
```

### TV3: 负重能力计算

```
输入: STR=18, CON=16, 非变身, 非漂浮, 无腿伤
期望: weight_cap() = 25 * (18 + 16) + 50 = 25 * 34 + 50 = 900
```

### TV4: 负重能力上限

```
输入: STR=25, CON=25, 非变身, 非漂浮
期望: weight_cap() = 25 * (25 + 25) + 50 = 1300 → capped to MAX_CARR_CAP = 1000
```

### TV5: 负重等级 - 边界条件

```
输入: weight_cap = 500, 携带总重 500
期望: inv_weight() = 500 - 500 = 0
     calc_capacity(0): wt=0, wt<=0 → UNENCUMBERED (0)
```

### TV6: 负重等级 - 刚过边界

```
输入: weight_cap = 500, 携带总重 501
期望: inv_weight() = 501 - 500 = 1
     calc_capacity(0): wt=1, cap = (1*2/500)+1 = 0+1 = 1 → SLT_ENCUMBER (1)
```

### TV7: 金币重量 - 边界条件

```
输入: 1 枚金币
期望: GOLD_WT(1) = (1 + 50) / 100 = 0
     但 weight() 函数中有 max(wt, 1)，所以实际重量 = 1

输入: 50 枚金币
期望: GOLD_WT(50) = (50 + 50) / 100 = 1

输入: 100 枚金币
期望: GOLD_WT(100) = (100 + 50) / 100 = 1
```

### TV8: 合并 - BUC 不同阻止合并

```
输入: 背包中有一把 blessed 短剑 (a)，拾取一把 uncursed 短剑
期望: 不合并，新短剑获得独立字母
```

### TV9: 合并 - 蜡烛年龄差

```
输入: 背包中有蜡烛 age=100，地面有蜡烛 age=130
      100/25=4, 130/25=5 → 不同
期望: 不合并

输入: 背包中有蜡烛 age=100，地面有蜡烛 age=120
      100/25=4, 120/25=4 → 相同
期望: 合并，新 age = (100*Q1 + 120*Q2) / (Q1+Q2)
```

### TV10: 储物袋爆炸 - 必定爆炸

```
输入: 将 BAG_OF_HOLDING (spe>0) 放入另一个 BAG_OF_HOLDING
      depthin=0, rn2(1 << 0) = rn2(1) = 0, 0 <= 0 → TRUE
期望: 必定爆炸，6d6 伤害，两个袋子都被销毁
```

### TV11: 52 槽位已满 - 边界条件

```
输入: 背包中有 52 个不同的非金币物品（a-z, A-Z），拾取金币
期望: 金币进入 '$' 槽，不受 52 限制
     inv_cnt(FALSE) = 52, inv_cnt(TRUE) = 53

输入: 背包中有 52 个不同物品，拾取一个不可合并的新类型物品
期望: "Your knapsack cannot accommodate any more items."
     拾取失败

输入: 背包中有 52 个物品，其中有箭 x20，拾取相同类型的箭 x5
期望: 合并成功（通过 merge_choice 检查），箭变为 x25
```

### TV12: 储物袋重量 - 边界条件

```
输入: uncursed BAG_OF_HOLDING (自身重 15)，内含物品总重 1
期望: cwt = (1+1)/2 = 1, 总重 = 15 + 1 = 16

输入: blessed BAG_OF_HOLDING (自身重 15)，内含物品总重 1
期望: cwt = (1+3)/4 = 1, 总重 = 15 + 1 = 16

输入: blessed BAG_OF_HOLDING，内含物品总重 0
期望: cwt = (0+3)/4 = 0, 总重 = 15 + 0 = 15

输入: cursed BAG_OF_HOLDING，内含物品总重 100
期望: cwt = 100*2 = 200, 总重 = 15 + 200 = 215
```

### TV13: invletter_value 排序

```
输入: '$' → 1
      'a' → 2
      'z' → 27
      'A' → 28
      'Z' → 53
      '#' → 54
      其他 → 55
```

### TV14: 漂浮/骑乘时的负重

```
输入: STR=10, CON=10, Levitation 活跃
期望: weight_cap() = MAX_CARR_CAP = 1000
     （无视正常计算的 25*(10+10)+50 = 550）
```

### TV15: 物品分割

```
输入: 背包中箭 x20 (字母 'a', owt=20)，splitobj(箭, 5)
期望: 原对象 'a': quan=15, owt=15
     新对象: quan=5, owt=5, invlet='a'(复制), owornmask=0, lamplit=0
     新对象紧接在原对象之后（obj.nobj = otmp）
     context.objsplit.parent_oid = 原对象.o_id
     context.objsplit.child_oid = 新对象.o_id
```

### TV16: 分割边界条件

```
输入: splitobj(obj, 0) 其中 obj.quan=10
期望: panic（num <= 0 不允许）

输入: splitobj(obj, 10) 其中 obj.quan=10
期望: panic（num >= quan 不允许，必须留至少 1 件）

输入: splitobj(container_with_contents, 1)
期望: panic（容器不可分割，obj.cobj != NULL）
```

### TV17: delta_cwt - Bag of Holding 取出重量差

```
输入: blessed BoH 含 3 件物品总重 400
      取出重量 100 的物品
      取出前: cwt_before = (400+3)/4 = 100
      取出后: cwt_after = (300+3)/4 = 75
期望: delta_cwt = 容器旧重 - 容器新重 = (15+100) - (15+75) = 25
     （不是 100！因为 BoH 减免效果使实际减重仅 25）

输入: cursed BoH 含物品总重 100，取出重量 50 的物品
      取出前: cwt_before = 100*2 = 200
      取出后: cwt_after = 50*2 = 100
期望: delta_cwt = (15+200) - (15+100) = 100
     （诅咒 BoH 取出 50 重物品，实际减重 100）
```

### TV18: 雕像重量

```
输入: 杀手蜜蜂雕像 (corpsenm=PM_KILLER_BEE, cwt=1, msize=MZ_TINY=0)
      wt = 3 * 1 / 2 = 1
      minwt = (0*2 + 1) * 100 = 100
      wt < minwt → wt = 100
期望: weight = 100 + 内容物重量

输入: 人类雕像 (cwt=1450, msize=MZ_HUMAN=4)
      wt = 3 * 1450 / 2 = 2175
      minwt = (4*2 + 1) * 100 = 900
      wt >= minwt → wt = 2175
期望: weight = 2175 + 内容物重量
```

### TV19: 负重等级 - OVERLOADED 边界

```
输入: weight_cap = 500, 携带总重 1500
期望: inv_weight() = 1500 - 500 = 1000
     calc_capacity(0): wt=1000, cap = (1000*2/500)+1 = 4+1 = 5 → OVERLOADED (5)

输入: weight_cap = 500, 携带总重 1501
期望: inv_weight() = 1001, cap = (1001*2/500)+1 = 4+1 = 5 → OVERLOADED (5)
     （cap 超过 5 被 min 裁剪到 5）

输入: weight_cap = 1, 携带总重 2
期望: inv_weight() = 2-1 = 1, wc=1 → 直接返回 OVERLOADED (5)
     （weight_cap <= 1 时的特殊快速路径）
```

### TV20: 金币 inv_weight 贡献 - 边界条件

```
输入: 背包仅有 49 枚金币，weight_cap = 100
期望: 金币重量 = (49+50)/100 = 0
     但 weight() 中 max(wt,1) → 实际 owt = 1
     inv_weight 中: (49+50)/100 = 0 （inv_weight 用自己的公式，不调用 weight()）
     inv_weight() = 0 - 100 = -100
注意: inv_weight() 对金币使用 (quan+50)/100 而非 weight()。
     weight() 额外有 max(wt,1) 保底但 inv_weight() 没有。
     [疑似 bug: inv_weight 的金币重量公式与 weight() 的不完全一致——
      weight() 保证至少 1，但 inv_weight() 对 1-49 枚金币计为 0 重量]
```

---

## 不确定之处

1. **`inv_rank` 中 '#' 的排序位置**：如上文标注，'#' (0x23 XOR 0x20 = 0x03) 的 inv_rank 值比 '$' (0x04) 还小，理论上会排到金币前面。但由于 '#' 溢出物品极为罕见（需要所有 52 个字母都被占用且有合并不了的新物品），且通常很快会被处理掉，此行为在实践中几乎不可观测。[疑似 bug，如实记录原始行为]

2. **`mergable()` 中 `bknown` 的牧师例外**：牧师（PM_CLERIC）在非盲/非幻觉状态下，即使两个物品的 `bknown` 不同也允许合并。这可能是因为牧师天然就能感知 BUC 状态。但在 Blind/Hallucination 状态下，此例外失效，与非牧师行为相同——这可能是有意的（盲/幻觉时牧师也无法可靠感知 BUC），也可能是疏忽。

3. **`reassign()` 的 lastinvnr 更新**：函数末尾 `if (i >= 52) i = 52 - 1; gl.lastinvnr = i;` 这里 `i` 是最后一个被分配字母的物品索引。但如果物品少于 52 个，`lastinvnr` 会被设为最后一个物品的索引而非 51，这意味着后续 `assigninvlet()` 会从链表中间开始搜索——对 `!fixinv` 模式这是合理的，因为新物品总是追加到末尾。

4. **`inv_weight()` 与 `weight()` 的金币重量不一致**：`weight()` 对金币使用 `max((quan+50)/100, 1)` 保证至少 1 单位，但 `inv_weight()` 直接使用 `(quan+50)/100` 不带保底。对于 1-49 枚金币，`weight()` 返回 1 但 `inv_weight()` 计为 0。这在实践中影响极小（50 金币以下的重量差异微不足道），但对于精确重量追踪可能是个问题。[疑似 bug]

5. **`mergable()` 中 `how_lost` 的非对称检查**：代码检查 `otmp.how_lost != LOST_NONE AND obj.how_lost != otmp.how_lost`，意味着如果 `otmp.how_lost == LOST_NONE`（正常状态），无论 `obj.how_lost` 是什么都允许合并。但反过来如果 `obj.how_lost == LOST_NONE` 而 `otmp.how_lost != LOST_NONE`，则不允许。这个非对称性可能是有意的（让"正常"物品吸收"特殊"物品），也可能是遗漏。
