# NetHack 3.7 拾取与放下系统机制规格

源代码版本：NetHack-3.7 分支，基于 `src/pickup.c`、`src/do.c`、`src/dokick.c`、`src/hack.c`、`include/obj.h`、`include/hack.h`、`include/weight.h`、`include/flag.h`。

---

## 1. 拾取物品（Pickup）

### 1.1 入口与调度

拾取的主入口函数是 `pickup(int what)`：

```
FUNCTION pickup(what):
    -- what > 0: 自动拾取（autopickup）
    -- what == 0: 交互式拾取
    -- what < 0: 拾取指定数量 count = -what

    IF autopickup AND (multi < 0 AND unconscious):
        RETURN 0  -- 昏迷/睡眠中到达此位置，跳过

    IF NOT swallowed:
        -- 自动拾取时：无物品 / 水中（非 Underwater）/ 岩浆中 → 不拾取
        -- 悬浮且无法触及地面 → 不拾取
        -- 同时在移动中（multi && !context.run）→ 不拾取
        -- notake(youmonst.data) → "You are physically incapable..."
        -- 有物品且在跑步中 → nomul(0) 停止跑步

    IF autopickup:
        n = autopick(objchain, traverse_how, &pick_list)
        GOTO menu_pickup

    -- 交互式：依据 menustyle 选择不同的拾取方式
    IF menustyle != TRADITIONAL or menu_requested:
        -- 菜单式拾取
        IF count > 0:  -- "pick N of"
            n = query_objlist("Pick %d of what?", ...)
        ELSE:
            n = query_objlist("Pick up what?", ...)
    ELSE:
        -- 传统式拾取（query_classes + askchain）

    menu_pickup:
        FOR EACH selected item:
            res = pickup_object(item, count, FALSE)
            IF res < 0: BREAK  -- 致命错误，中止循环
```

### 1.2 单物品拾取流程：`pickup_object()`

```
FUNCTION pickup_object(obj, count, telekinesis):
    -- 不能拾取的物品
    IF obj == uchain: RETURN 0                    -- 锁链不可拾取
    IF obj in engulfer minvent and worn: RETURN 0  -- 吞噬者穿戴的物品
    IF artifact and touch_artifact fails: RETURN 0

    -- 尸体特殊处理
    IF obj.otyp == CORPSE:
        IF fatal_corpse_mistake(obj, telekinesis): RETURN -1  -- 石化
        IF rider_corpse_revival(obj, telekinesis): RETURN -1  -- 骑士复活

    -- 恐吓怪物卷轴特殊处理
    IF obj.otyp == SCR_SCARE_MONSTER:
        先计算 carry_count 确定能拿多少张
        IF count < obj.quan: obj = splitobj(obj, count)
        IF obj.blessed:
            unbless(obj)  -- 祝福的 → 变为无诅无祝
        ELSE IF !obj.spe AND !obj.cursed:
            obj.spe = 1   -- 未被拾取过 → 标记已拾取
        ELSE:
            -- 已被拾取过(spe==1)或被诅咒的 → 化为灰尘
            "The scroll turns to dust as you pick it up."
            useupf(obj, obj.quan)
            RETURN 1  -- 消耗了行动但继续循环

    -- 提起物品重量检查
    res = lift_object(obj, NULL, &count, telekinesis)
    IF res <= 0: RETURN res

    -- 金币特殊处理：更新状态栏
    IF obj.oclass == COIN_CLASS: botl = TRUE

    -- 需要拆分堆叠时
    IF obj.quan != count AND obj.otyp != LOADSTONE:
        obj = splitobj(obj, count)

    -- 实际拾取
    obj = pick_obj(obj)
    -- 打印拾取消息
    pickup_prinv(obj, count, "lifting")
    RETURN 1
```

### 1.3 `pick_obj()` — 实际从地面移入背包

```
FUNCTION pick_obj(otmp):
    ox = otmp.ox; oy = otmp.oy
    robshop = (NOT swallowed AND NOT uball AND costly_spot(ox, oy))

    obj_extract_self(otmp)  -- 从地面链表移除
    newsym(ox, oy)          -- 刷新地面显示

    IF robshop:
        -- 在商店中拾取：先 addtobill
        -- 伪造 u.ushops 为物品所在房间
        addtobill(otmp, TRUE, FALSE, FALSE)
        -- 如果玩家不在该商店内，标记为盗窃
        robshop = otmp.unpaid AND 玩家不在该商店

    result = addinv(otmp)  -- 加入背包（可能触发合并）

    IF robshop:
        remote_burglary(ox, oy)  -- 触发远程盗窃警报

    RETURN result
```

### 1.4 AUTOSELECT_SINGLE 优化

当菜单查询 `query_objlist` 发现只有1个合格物品时，且设置了 `AUTOSELECT_SINGLE` 标志：

```
IF n == 1 AND (qflags & AUTOSELECT_SINGLE):
    pick_list = alloc(sizeof menu_item)
    pick_list[0].item = last_qualified_obj
    pick_list[0].count = last_qualified_obj.quan
    RETURN 1  -- 无需弹出菜单
```

此优化确保单物品地面不弹菜单，直接拾取。

---

## 2. 自动拾取系统（Autopickup）

### 2.1 相关选项

| 选项 | 类型 | 说明 |
|------|------|------|
| `pickup` | boolean | 总开关，默认 TRUE |
| `pickup_types` | string | 要自动拾取的物品类符号（如 `"$!?/"` 表示金币、药水、卷轴、魔杖） |
| `pickup_burden` | int | 最大负重等级（0-4），超过则不自动拾取 |
| `pickup_thrown` | boolean | 自动拾取自己扔出去的物品 |
| `pickup_stolen` | boolean | 自动拾取被怪物偷走的物品 |
| `nopick_dropped` | boolean | 不自动拾取自己丢弃的物品 |

### 2.2 自动拾取判定：`autopick_testobj()`

```
FUNCTION autopick_testobj(otmp, calc_costly):
    -- 1. 商店中非 no_charge 的物品 → 拒绝
    IF costly AND NOT otmp.no_charge:
        RETURN FALSE

    -- 2. 特殊 how_lost 覆盖规则（优先级高于 pickup_types 和异常规则）
    IF flags.pickup_thrown AND otmp.how_lost == LOST_THROWN: RETURN TRUE
    IF flags.pickup_stolen AND otmp.how_lost == LOST_STOLEN: RETURN TRUE
    IF flags.nopick_dropped AND otmp.how_lost == LOST_DROPPED: RETURN FALSE
    IF otmp.how_lost == LOST_EXPLODING: RETURN FALSE

    -- 3. 检查 pickup_types
    pickit = (pickup_types为空) OR (otmp.oclass IN pickup_types)

    -- 4. 检查 autopickup exceptions
    ape = check_autopickup_exceptions(otmp)
    IF ape != NULL:
        pickit = ape.grab  -- TRUE=强制拾取, FALSE=强制不拾取

    RETURN pickit
```

### 2.3 自动拾取异常规则（autopickup exceptions）

```
struct autopickup_exception:
    regex: nhregex*     -- 正则表达式（已编译）
    pattern: char*      -- 原始模式字符串
    grab: boolean       -- TRUE = 匹配时拾取; FALSE = 匹配时不拾取
    next: *autopickup_exception
```

匹配方式：
1. 对物品调用 `makesingular(doname(obj))` 生成描述文本
2. 遍历异常列表，用 `regex_match()` 匹配
3. **第一个**匹配到的异常决定结果（`grab` 字段）
4. 如果无匹配，回退到 `pickup_types` 判定

配置语法：`OPTIONS=autopickup_exception="<pattern"` 表示不拾取，`OPTIONS=autopickup_exception=">pattern"` 表示拾取。

### 2.4 自动拾取不触发的条件

- `flags.pickup == FALSE`（总开关关闭）
- `context.nopick == TRUE`（使用了 m 前缀移动）
- `multi < 0 AND unconscious()`（昏迷/睡眠）
- `notake(youmonst.data)`（当前变形不能拾取物品）
- 脚下没有物品
- 在水中（非 Underwater）或岩浆中
- 无法触及地面（悬浮且不在坑上方）

---

## 3. 金币拾取（Gold Pickup）

### 3.1 重量公式

```
#define GOLD_WT(n) (((n) + 50L) / 100L)
```

即每100金币重1单位（对50以上的余数向上取整）。

拾取时合并金币的重量调整：
```
已有金币 umoney, 拾取 count 枚
wt_delta = GOLD_WT(umoney) + GOLD_WT(count) - GOLD_WT(umoney + count)
```
因为整数除法取整，合并后的重量可能小于两者分别的重量之和。

### 3.2 金币容量公式

```
#define GOLD_CAPACITY(w, n) (((w) * -100L) - ((n) + 50L) - 1L)
```

当从容器中取出金币时，使用迭代方式精确计算：
```
以 50 - (umoney % 100) - 1 为起始量
每次增加 100 步进
计算合并后的 GOLD_WT 和容器 delta_cwt
直到超出负重为止
```

### 3.3 金币不占背包槽

金币使用专用 `$` 槽，不受 52 个字母槽的限制：
```
IF obj.oclass == COIN_CLASS:
    -- 跳过 inv_cnt(FALSE) >= invlet_basic 检查
    -- 金币可以无限合并到 $ 槽
```

如果其他物品用尽了52个槽但地面有金币，会提示 `"(except gold)"`。

---

## 4. 负重检查（Encumbrance on Pickup）

### 4.1 负重容量公式

```
FUNCTION weight_cap():
    carrcap = WT_WEIGHTCAP_STRCON * (STR + CON) + WT_WEIGHTCAP_SPARE
    -- = 25 * (STR + CON) + 50

    IF Upolyd:
        IF nymph: carrcap = MAX_CARR_CAP (= 1000)
        ELSE IF cwt == 0: carrcap = carrcap * msize / MZ_HUMAN
        ELSE IF not strong OR (strong AND cwt > WT_HUMAN):
            carrcap = carrcap * cwt / WT_HUMAN (= 1450)

    IF Levitation OR airlevel OR (riding strong mount):
        carrcap = MAX_CARR_CAP (= 1000)
    ELSE:
        carrcap = min(carrcap, 1000)
        IF NOT Flying:
            IF wounded left leg: carrcap -= 100
            IF wounded right leg: carrcap -= 100

    RETURN max(carrcap, 1)
```

### 4.2 负重等级

```
FUNCTION calc_capacity(xtra_wt):
    wt = inv_weight() + xtra_wt  -- inv_weight = 物品总重 - weight_cap
    IF wt <= 0: RETURN UNENCUMBERED (0)
    IF wc <= 1: RETURN OVERLOADED (5)
    cap = (wt * 2 / wc) + 1
    RETURN min(cap, OVERLOADED)
```

| 等级 | 枚举名 | 值 | 状态栏显示 |
|------|--------|-----|-----------|
| 0 | UNENCUMBERED | 0 | （无） |
| 1 | SLT_ENCUMBER | 1 | Burdened |
| 2 | MOD_ENCUMBER | 2 | Stressed |
| 3 | HVY_ENCUMBER | 3 | Strained |
| 4 | EXT_ENCUMBER | 4 | Overtaxed |
| 5 | OVERLOADED | 5 | Overloaded |

### 4.3 拾取时的负重交互：`lift_object()`

```
FUNCTION lift_object(obj, container, cnt_p, telekinesis):
    -- Sokoban 中不能拾取巨石
    IF obj.otyp == BOULDER AND Sokoban:
        "You cannot get your hands around this boulder."
        RETURN -1

    -- 诅咒的负重石 或 巨人形态的巨石：强制拾取（如果有空槽）
    IF obj.otyp == LOADSTONE
       OR (obj.otyp == BOULDER AND throws_rocks(youmonst.data)):
        IF inv_cnt(FALSE) < 52 OR NOT carrying(obj.otyp)
           OR merge_choice(invent, obj):
            RETURN 1  -- 直接拾取，无视重量

    -- 计算能拿多少
    *cnt_p = carry_count(obj, container, *cnt_p, ...)
    IF *cnt_p < 1: RETURN -1  -- 太重，一个都拿不了

    -- 检查背包槽（非金币）
    IF obj.oclass != COIN_CLASS
       AND inv_cnt(FALSE) >= 52
       AND NOT merge_choice(invent, obj):
        "Your knapsack cannot accommodate any more items."
        RETURN -1

    -- 负重变化提示
    prev_encumbr = max(near_capacity(), flags.pickup_burden)
    next_encumbr = calc_capacity(new_wt - old_wt)
    IF next_encumbr > prev_encumbr:
        IF telekinesis: RETURN 0  -- 远距不拾取
        -- 提示玩家
        prompt = "{difficulty prefix} lifting {obj}. Continue?"
        -- difficulty prefix 由 next_encumbr 决定：
        --   EXT_ENCUMBER+: "You have extreme difficulty"
        --   HVY_ENCUMBER:  "You have much trouble"
        --   MOD_ENCUMBER:  "You have trouble"
        --   SLT_ENCUMBER:  "You have a little trouble"
        SWITCH ynq(prompt):
            'q': RETURN -1   -- 退出
            'n': RETURN 0    -- 不拾取
            'y': (继续)

    -- 恐吓怪物卷轴特殊：拒绝拾取时设 spe=0
    IF obj.otyp == SCR_SCARE_MONSTER AND result <= 0 AND NOT container:
        obj.spe = 0

    RETURN result
```

### 4.4 `pickup_burden` 选项

`flags.pickup_burden` 设置了自动拾取时能容忍的最大负重等级，从 `UNENCUMBERED`(0) 到 `EXT_ENCUMBER`(4)。在 `lift_object()` 中：

```
prev_encumbr = max(near_capacity(), flags.pickup_burden)
```

如果当前负重已经超过 `pickup_burden`，则 `prev_encumbr` 以当前实际值为准。只有 `next_encumbr > prev_encumbr` 时才会提示用户确认。

对于自动拾取（telekinesis 参数为 FALSE 但通过 autopick 路径调用），负重超限会直接跳过（不提示）。

---

## 5. 放下物品（Drop）

### 5.1 单物品放下：`dodrop()` → `drop()`

```
FUNCTION dodrop():
    IF in shop: sellobj_state(SELL_DELIBERATE)
    result = drop(getobj("drop", ...))
    IF in shop: sellobj_state(SELL_NORMAL)
    RETURN result

FUNCTION drop(obj):
    IF NOT canletgo(obj, "drop"): RETURN FAIL
    -- canletgo 检查：
    --   穿戴中的盔甲/饰品 → 不可丢
    --   welded 武器 → 不可丢
    --   cursed loadstone → 不可丢
    --   拴着宠物的缰绳 → 不可丢
    --   骑乘中的鞍 → 不可丢

    IF obj == CORPSE AND better_not_try_to_drop_that(obj):
        -- 石化尸体无手套保护时确认

    -- 解除装备状态
    IF obj == uwep: setuwep(NULL)
    IF obj == uquiver: setuqwep(NULL)
    IF obj == uswapwep: setuswapwep(NULL)

    IF swallowed:
        -- 丢入吞噬者胃中
        "You drop {obj} into {monster's stomach}."
    ELSE:
        -- 戒指丢入水槽 → dosinkring(obj) 特殊效果
        IF (ring OR meat_ring) AND on_sink:
            dosinkring(obj)
            RETURN TIME

        -- 悬浮时无法触及地面 → hitfloor(obj)
        IF NOT can_reach_floor(TRUE):
            freeinv(obj); hitfloor(obj, TRUE)
            RETURN TIME

        -- 祭坛上丢物品 → doaltarobj()
        IF on_altar AND NOT verbose:
            (不打印 "You drop" 消息，交由 doaltarobj)

    obj.how_lost = LOST_DROPPED
    dropx(obj)
    RETURN TIME
```

### 5.2 `dropx()` → `dropy()` → `dropz()`

```
FUNCTION dropx(obj):
    freeinv(obj)
    IF NOT swallowed:
        IF ship_object(obj, ...): RETURN  -- 物品落入洞/陷阱
        IF on altar: doaltarobj(obj)      -- 祭坛效果（BUC 闪光）
    dropy(obj)

FUNCTION dropy(obj):
    dropz(obj, FALSE)

FUNCTION dropz(obj, with_impact):
    IF swallowed:
        -- 丢入吞噬者
        IF obj != uball:
            IF unpaid: stolen_value(...)    -- 商店物品算被盗
            IF NOT engulfer_digests_food:
                mpickobj(u.ustuck, obj)    -- 加入怪物背包
    ELSE:
        IF flooreffects(obj, ...): RETURN   -- 特殊地面效果
        place_object(obj, u.ux, u.uy)
        IF with_impact: container_impact_dmg(obj, ...)
        IF obj == uball: drop_ball(...)
        ELSE IF has_shop: sellobj(obj, ...)
        stackobj(obj)                       -- 自动堆叠
        newsym(u.ux, u.uy)
    encumber_msg()  -- 更新负重消息
```

### 5.3 多物品放下：`doddrop()` → `menu_drop()`

```
FUNCTION doddrop():
    IF no inventory: "You have nothing to drop." RETURN

    IF menustyle != TRADITIONAL:
        result = menu_drop(...)
    ELSE:
        result = ggetobj("drop", drop, 0, FALSE, ...)

FUNCTION menu_drop(retry):
    -- FULL menustyle: 先 query_category 选类别
    --   支持 A (全部), P (刚拾取的), a (所有类型), BUC过滤
    -- COMBINATION menustyle: ggetobj + query_objlist 混合模式

    IF autopick (选了 'A'):
        -- 使用 bypass 机制安全遍历
        FOR EACH unbypassed item IN invent:
            IF matching category: drop(item)
    ELSE IF drop_justpicked AND only 1 stack:
        -- 直接丢该物品，支持指定数量
        menudrop_split(otmp, justpicked_quan)
    ELSE:
        -- query_objlist 菜单选择
        n = query_objlist("What would you like to drop?", ...)
        FOR EACH selected:
            menudrop_split(otmp, count)
```

### 5.4 `menudrop_split()` — 分堆丢弃

```
FUNCTION menudrop_split(otmp, cnt):
    IF cnt > 0 AND cnt < otmp.quan:
        IF welded(otmp): 不拆分
        ELSE IF LOADSTONE AND cursed:
            otmp.corpsenm = cnt  -- kludge, 用于 canletgo 消息
        ELSE:
            otmp = splitobj(otmp, cnt)
    RETURN drop(otmp)
```

---

## 6. 地面效果（`flooreffects()`）

物品被放下、抛出、或掉落到地面时调用 `flooreffects()`：

```
FUNCTION flooreffects(obj, x, y, verb):
    -- 巨石进入水/岩浆 → boulder_hits_pool()
    IF obj.otyp == BOULDER AND pool_or_lava:
        -- 水：90% 填满，10% 沉没
        -- 岩浆：10% 填满，90% 沉没
        -- 水墙：50% 填满
        -- 水之位面：0% 填满
        IF fills_up:
            levl[rx][ry].typ = ROOM
            -- 可能压死 monster
        -- 巨石消失
        RETURN TRUE

    -- 巨石进入坑/洞
    IF obj.otyp == BOULDER AND (pit OR hole):
        -- 填坑 / 堵洞 / 压怪

    -- 物品落入水中
    IF is_pool: water_damage(obj, ...)

    -- 物品落入岩浆
    IF is_lava: lava_damage(obj, ...)

    -- 在坑边掉入坑
    IF at(pit) AND teetering:
        "The {obj} tumbles into the pit."

    -- 球状怪（glob）合并
    IF obj.globby:
        WHILE 相邻有同类 glob:
            pudding_merge_message(...)
            obj_meld(...)

    -- 药水落在高温地面
    IF obj is potion AND level.temperature > 0:
        -- survival_chance = blessed ? 70 : 50
        -- 如果曾被玩家持有: survival_chance += Luck * 2
        -- POT_OIL: survival_chance = 100
        IF NOT obj_resists(obj, survival_chance, 100):
            breakobj(obj, ...)  -- 碎裂

    -- 怪物移动时的祭坛效果
    IF mon_moving AND altar: doaltarobj(obj)

    RETURN FALSE  -- 物品正常放置
```

---

## 7. 踢物品（Kick Objects）

### 7.1 `really_kick_object()`

```
FUNCTION really_kick_object(x, y):
    -- 巨石、铁球、锁链不可踢
    IF kickedobj.otyp == BOULDER OR uball OR uchain: RETURN 0

    -- 在坑/网中的物品不可踢
    IF trap at (x,y) AND (pit OR web): RETURN 1

    -- Fumbling 时 1/3 概率踢空
    IF Fumbling AND rn2(3): RETURN 1

    -- 赤脚踢石化尸体 → 可能石化
    IF no boots AND cockatrice corpse AND NOT Stone_resistance:
        instapetrify(...)

    -- 计算踢飞距离
    k_owt = 单个物品重量（堆叠时按1个算）
    range = ACURRSTR / 2 - k_owt / 40
    IF martial(): range += rnd(3)
    IF in water: range = range / 3 + 1
    IF airlevel/waterlevel: range += rnd(3)
    IF on ice: range += rnd(3), slide = TRUE
    IF greased: range += rnd(3), slide = TRUE
    IF Mjollnir: range = 1  -- 魔法太重
    IF 目标方向有墙/门: range = 1

    -- range < 2: 物品不移动
    IF range < 2: "Thump!" RETURN

    -- 堆叠处理：非金币踢出1个
    IF kickedobj.quan > 1:
        IF NOT gold:
            kickedobj = splitobj(kickedobj, 1)
        ELSE:
            -- 金币堆：95% 概率散落（scatter）
            IF rn2(20): scatter(x, y, ...); RETURN 1
            -- > 300 枚金币太重
            IF kickedobj.quan > 300: "Thump!"; RETURN

    -- 踢出：使用 bhit() 模拟抛物线
    obj_extract_self(kickedobj)
    mon = bhit(u.dx, u.dy, range, KICKED_WEAPON, ...)
```

### 7.2 箱子踢开锁

```
IF Is_box(kickedobj):
    IF locked:
        -- 普通 1/5 概率踢开锁; martial() 额外 1/2 概率
        IF !rn2(5) OR (martial() AND !rn2(2)):
            "You break open the lock!"
            breakchestlock(kickedobj, FALSE)
    ELSE:
        -- 1/3 概率或 martial 1/2 概率盖子弹开
        "The lid slams open, then falls shut."
```

---

## 8. 物品堆叠（Item Stacking on Floor）

### 8.1 堆叠规则

地面物品使用 `nexthere` 链表维护，调用 `stackobj(obj)` 在放置时自动合并。物品可以合并的条件与背包合并相同（`merge_choice()`）：相同 otyp、相同 BUC、相同 erosion 等。

### 8.2 物品堆显示

- 单物品：直接显示该物品的 glyph
- 多物品：显示最上面的物品 glyph
- `look_here()`：列出当前位置所有物品
- 检查物品数量 `ct`：遍历 `level.objects[u.ux][u.uy]` 的 nexthere 链

### 8.3 拾取一些物品后

```
IF n_picked > 0:
    newsym_force(u.ux, u.uy)  -- 刷新显示
IF autopickup:
    check_here(n_picked > 0)  -- 检查是否还有物品
```

---

## 9. 容器交互（Container Interaction）

### 9.1 打开容器：`use_container()`

容器动作菜单（`in_or_out_menu`）：

| 键 | 动作 | 说明 |
|----|------|------|
| `:` | Look | 查看容器内容 |
| `o` | Out | 取出物品 |
| `i` | In | 放入物品 |
| `b` | Both | 先取出再放入 |
| `r` | Reversed | 先放入再取出 |
| `s` | Stash | 放入单个物品 |
| `n` | Next | 跳到下一个容器 |
| `q` | Quit | 退出 |

### 9.2 放入容器：`in_container()`

```
FUNCTION in_container(obj):
    -- 不可放入的物品
    IF uball OR uchain: "You must be kidding."
    IF obj == current_container: "Interesting topological exercise."
    IF worn armor/accessory: "Cannot stash what you're wearing."
    IF cursed loadstone: "The stone won't leave your person."
    IF quest artifact (Amulet/Candelabrum/Bell/Book): "Cannot be confined."
    IF leash with pet: "Attached to your pet."
    IF welded weapon: weldmsg

    -- 致命尸体检查
    IF fatal_corpse_mistake(obj, FALSE): RETURN -1

    -- 太大的物品
    IF ICE_BOX OR large_box OR chest OR (STATUE AND bigmonst):
        "Cannot fit into container."

    -- 从背包移除
    freeinv(obj)

    -- 冰箱特殊：停止腐烂定时器
    IF Icebox AND NOT age_is_relative(obj):
        obj.age = moves - obj.age  -- 记录实际年龄
        IF corpse: stop_timer(ROT_CORPSE); stop_timer(REVIVE_MON)

    -- 持有之袋（Bag of Holding）爆炸检查
    ELSE IF Is_mbag(current_container) AND mbag_explodes(obj, 0):
        -- 爆炸！
        "You are blasted by a magical explosion!"
        do_boh_explosion(current_container, floor_container)
        -- 销毁容器
        losehp(d(6,6), "magical explosion", KILLED_BY_AN)
        current_container = NULL

    -- 正常放入
    IF current_container:
        "You put {obj} into {container}."
        add_to_container(current_container, obj)
        current_container.owt = weight(current_container)

    RETURN current_container ? 1 : -1
```

### 9.3 持有之袋爆炸概率：`mbag_explodes()`

```
FUNCTION mbag_explodes(obj, depthin):
    -- 空的取消术魔杖/trick_bag不会爆炸
    IF (obj.otyp == WAN_CANCELLATION OR obj.otyp == BAG_OF_TRICKS)
       AND obj.spe <= 0:
        RETURN FALSE

    -- 概率：depth 0→1/1, 1→2/2, 2→3/4, 3→4/8, ...
    -- 公式: rn2(1 << min(depthin, 7)) <= depthin
    IF (Is_mbag(obj) OR obj.otyp == WAN_CANCELLATION):
        IF rn2(1 << min(depthin, 7)) <= depthin:
            RETURN TRUE

    -- 递归检查容器内容
    IF Has_contents(obj):
        FOR EACH otmp IN obj.cobj:
            IF mbag_explodes(otmp, depthin + 1):
                RETURN TRUE

    RETURN FALSE
```

直接把 bag of holding / wand of cancellation（有电荷）放入 bag of holding 时，`in_container` 调用 `mbag_explodes(obj, 0)`：

判定公式（源代码注释 "odds: 1/1, 2/2, 3/4, 4/8, 5/16, ..."）：
```
test: rn2(1 << min(depthin, 7)) <= depthin
```

| depthin | rn2 范围 | 通过条件 | 爆炸概率 |
|---------|---------|---------|---------|
| 0 | rn2(1) → {0} | 0 <= 0 | 1/1 = **100%** |
| 1 | rn2(2) → {0,1} | <= 1 | 2/2 = **100%** |
| 2 | rn2(4) → {0..3} | <= 2 | 3/4 = **75%** |
| 3 | rn2(8) → {0..7} | <= 3 | 4/8 = **50%** |
| 4 | rn2(16) → {0..15} | <= 4 | 5/16 = **31.25%** |
| 5 | rn2(32) → {0..31} | <= 5 | 6/32 = **18.75%** |
| 6 | rn2(64) → {0..63} | <= 6 | 7/64 = **10.94%** |
| 7+ | rn2(128) → {0..127} | <= d | (d+1)/128 (capped at d=7 分母) |

即：直接放入（depthin=0）和嵌套1层（depthin=1）都是 **100% 爆炸**。嵌套越深概率越低。

### 9.4 诅咒的持有之袋：物品消失

```
FUNCTION boh_loss(container, held):
    IF Is_mbag(container) AND container.cursed AND Has_contents:
        FOR EACH item IN container.cobj:
            IF is_boh_item_gone():  -- !rn2(13) → 1/13 概率
                obj_extract_self(item)
                loss += mbag_item_gone(held, item, FALSE)
                -- "X have vanished!"
        RETURN loss
    RETURN 0
```

每次打开诅咒的 bag of holding，每件物品有 **1/13 ≈ 7.69%** 的概率消失。

### 9.5 取出容器：`out_container()`

```
FUNCTION out_container(obj):
    -- 触摸神器检查
    IF artifact AND NOT touch_artifact: RETURN 0
    -- 致命尸体检查
    IF fatal_corpse_mistake: RETURN -1

    count = obj.quan
    res = lift_object(obj, current_container, &count, FALSE)
    IF res <= 0: RETURN res

    -- 拆分
    IF obj.quan != count AND obj.otyp != LOADSTONE:
        obj = splitobj(obj, count)

    obj_extract_self(obj)
    current_container.owt = weight(current_container)

    -- 冰箱：恢复腐烂计时
    IF Icebox: removed_from_icebox(obj)

    -- 商店处理：非 unpaid 但在商店地板上的容器中取出 → addtobill
    IF NOT obj.unpaid AND NOT carried(current_container)
       AND costly_spot(container.ox, container.oy):
        addtobill(obj, ...)

    otmp = addinv(obj)
    RETURN 1
```

### 9.6 翻倒容器（#tip）

`tipcontainer()` 将容器内容直接倾倒到地面（或另一个容器）：

- 可选目标：地面（默认）或另一个随身容器
- 诅咒的 bag of holding 倾倒时，每件物品有 1/13 概率消失
- 倾倒到另一个 bag of holding 时可能触发爆炸
- 特殊容器（bag of tricks, horn of plenty）：反复 apply 直到用完
- 薛定谔之箱：先 observe_quantum_cat

### 9.7 薛定谔之猫

```
FUNCTION observe_quantum_cat(box, makecat, givemsg):
    itsalive = !rn2(2)  -- 50% 概率存活
    IF itsalive:
        -- 创建和平的 housecat，名为 "Schroedinger's Cat"
        -- 移除箱中的猫尸体
        box.spe = 0  -- 不再是薛定谔之箱
    ELSE:
        box.spe = 0
        -- 设置猫尸体的 corpsenm 为 PM_HOUSECAT
        -- 给予 20 经验值（作为杀死猫的经验）
```

---

## 10. 水/岩浆中的拾取

### 10.1 自动拾取限制

```
-- pickup() 中:
IF autopickup AND (is_pool(u.ux, u.uy) AND NOT Underwater):
    RETURN 0  -- 水面上不自动拾取

IF autopickup AND is_lava(u.ux, u.uy):
    RETURN 0  -- 岩浆上不自动拾取
```

在 Underwater 状态下可以正常拾取水底物品。

### 10.2 容器在水/岩浆中

```
FUNCTION able_to_loot(x, y, looting):
    IF (is_pool(x,y) AND (looting OR NOT Underwater)) OR is_lava(x,y):
        "You cannot loot/tip things deep in the water/lava."
        RETURN FALSE
```

即使 Underwater 也不能 loot 容器（但可以 tip）。岩浆中任何操作都不行。

### 10.3 `can_reach_floor()`

悬浮/飞行时能否触及地面物品取决于：
- `Levitation` 且 `!Is_airlevel` 且 `!Is_waterlevel` → 不能触及
- 在坑边（teeter）或逃离竖井 → 不能触及
- 骑乘且骑术不足 → 不能触及

---

## 11. 商店交互（Shop Interaction）

### 11.1 拾取商店物品

```
-- autopick_testobj 中:
IF costly AND NOT otmp.no_charge:
    RETURN FALSE  -- 自动拾取不拾取商店付费物品

-- pick_obj 中:
IF costly_spot(ox, oy):
    addtobill(otmp, TRUE, FALSE, FALSE)
    -- 标记 obj.unpaid = TRUE
    -- 如果玩家不在该商店内:
    IF robshop: remote_burglary(ox, oy)
```

手动拾取商店物品会将其加入账单（`unpaid`）。如果拾取后离开商店，触发盗窃。

### 11.2 放下物品到商店

```
-- drop() 入口:
IF in shop: sellobj_state(SELL_DELIBERATE)

-- dropz() 中:
IF has_shop: sellobj(obj, u.ux, u.uy)
```

在商店中放下已付款的物品会卖给店主。放下未付款的物品相当于归还。

### 11.3 容器与商店

```
-- in_container() 中:
IF floor_container AND costly_spot:
    IF obj.oclass != COIN_CLASS:
        sellobj(obj, ...)  -- 可能触发卖出

-- 金币放入地面容器:
sellobj(obj, container.ox, container.oy)  -- 金币转为 credit

-- out_container() 中:
IF NOT obj.unpaid AND NOT carried(container) AND costly_spot:
    addtobill(obj, ...)  -- 从商店容器取出 → 加入账单
```

### 11.4 踢商店物品

踢出商店物品需要处理账单，如果踢到商店外则触发 `stolen_value()`。

---

## 12. 巨石推动 vs 拾取

### 12.1 推动巨石

当玩家移动到巨石所在位置时调用 `moverock()`：

```
FUNCTION moverock_core(sx, sy):
    -- sx, sy = 巨石位置 = u.ux + u.dx, u.uy + u.dy
    rx, ry = u.ux + 2*u.dx, u.uy + 2*u.dy  -- 巨石目标位置

    -- m<dir> 移动（nopick）:
    IF context.nopick:
        IF throws_rocks(youmonst.data): step over
        ELSE IF could_move_onto_boulder: squeeze past
        ELSE: "There is a boulder in your way." RETURN -1

    -- 悬浮时不能推
    IF Levitation OR airlevel:
        "You don't have enough leverage."
        RETURN -1

    -- 太小不能推
    IF verysmall AND NOT riding:
        "You're too small to push that boulder."
        RETURN cannot_push(...)

    -- Sokoban 中不能对角推
    IF Sokoban AND u.dx AND u.dy:
        "Won't roll diagonally on this surface."
        RETURN cannot_push(...)

    -- 目标位置检查：墙、铁栏、门、已有巨石 → 不可推
    IF 目标位置有障碍:
        RETURN cannot_push(...)

    -- 推动成功
    dopush(sx, sy, rx, ry, otmp, costly)
```

### 12.2 巨人形态拾取巨石

```
IF throws_rocks(youmonst.data):  -- 巨人、泰坦等
    -- lift_object 中：直接 RETURN 1（跳过重量检查）
    -- 但要求有空背包槽或能合并
    -- Sokoban 中不能拾取巨石（lift_object 中检查）
```

### 12.3 推不动时的备选方案：`cannot_push()`

```
FUNCTION cannot_push(otmp, sx, sy):
    IF throws_rocks(youmonst.data):
        -- 巨人可以跨过巨石
        -- 如果 autopickup 会拾取巨石则说 "easily pick it up"
        -- 否则 "maneuver over it"
        sokoban_guilt()  -- 在 Sokoban 中扣分
        RETURN 0  -- 允许移动到巨石位置

    IF could_move_onto_boulder(sx, sy):
        -- 小体型或极轻装备可以挤过去
        "You can squeeze yourself into a small opening."
        sokoban_guilt()
        RETURN 0

    RETURN -1  -- 不能移动
```

---

## 13. 石化尸体处理（Cockatrice Corpse）

### 13.1 拾取石化尸体

```
FUNCTION fatal_corpse_mistake(obj, remotely):
    -- 安全条件（任一满足即安全）:
    IF uarmg (wearing gloves): safe
    IF obj.otyp != CORPSE: safe
    IF NOT touch_petrifies(mons[obj.corpsenm]): safe
    IF Stone_resistance: safe
    IF remotely (telekinesis): safe

    -- 不安全时:
    IF poly_when_stoned AND polymon(PM_STONE_GOLEM): safe（变为石巨人）

    -- 致命:
    "Touching {corpse} is a fatal mistake."
    instapetrify(killer_xname(obj))
    RETURN TRUE
```

受影响的怪物类型（`touch_petrifies`）：cockatrice, chickatrice 等。

### 13.2 影响范围

石化检查出现在以下场景：
- `pickup_object()` — 从地面拾取
- `in_container()` — 放入容器
- `out_container()` — 从容器取出
- `really_kick_object()` — 赤脚踢（脚部石化）
- `doloot_core()` — 盲人在有石化尸体的位置 #loot 时

### 13.3 骑士尸体特殊处理

```
FUNCTION rider_corpse_revival(obj, remotely):
    IF obj.otyp == CORPSE AND is_rider(mons[obj.corpsenm]):
        "At your touch, the corpse suddenly moves..."
        revive_corpse(obj)
        exercise(A_WIS, FALSE)  -- 降低智慧
        RETURN TRUE
```

Death、Pestilence、Famine 的尸体在尝试拾取时会复活。

---

## 14. 恐吓怪物卷轴（Scroll of Scare Monster）

### 14.1 拾取状态机

卷轴的 `spe` 字段跟踪拾取历史：

```
状态转换（仅对非祝福且非诅咒的卷轴）：
  spe == 0 (从未拾取):
    拾取 → spe = 1, 卷轴存活

  spe == 1 (已拾取过一次):
    再次拾取 → 化为灰尘，卷轴消失

祝福的卷轴:
  拾取 → unbless(obj), 变为无诅无祝, spe 不变
  下次行为取决于当时的 spe 值

诅咒的卷轴:
  拾取 → 直接化为灰尘

特殊：lift_object 中拒绝拾取（太重等）时:
  IF result <= 0 AND NOT container:
      obj.spe = 0  -- 重置拾取计数
```

[疑似 bug] 当 `lift_object` 返回 0（玩家选择不拾取）时，`spe` 被设为 0。这意味着玩家可以通过反复尝试拾取再拒绝来重置恐吓卷轴的 `spe` 状态，使其永远不会化为灰尘。但这个行为可能是有意为之——卷轴只对"真正拿起来"的动作计数。

---

## 15. "刚拾取" 标记系统（pickup_prev / Just Picked）

### 15.1 标记机制

```
-- 物品拾取时：
obj.pickup_prev = 1  -- 在 addinv() 中设置

-- 在每次新的拾取操作开始前：
reset_justpicked(invent)  -- 清除所有现有物品的 pickup_prev

-- 查询：
count_justpicked(olist)  -- 计数有 pickup_prev 标记的物品
find_justpicked(olist)   -- 找到第一个有标记的物品
```

### 15.2 用途

- **丢弃**：`menu_drop` 中 'P' 选项允许快速丢弃刚拾取的物品
- **容器**：`menu_loot` 中 'P' 选项允许快速将刚拾取的物品放入容器
- **快捷操作**：当只有1个刚拾取的堆叠时，直接操作无需菜单

---

## 16. 吞噬中的拾取/放下

### 16.1 从吞噬者内部拾取

```
IF u.uswallow:
    objchain_p = &u.ustuck->minvent  -- 从怪物背包拾取
    traverse_how = 0 (by nobj)
    -- 不能拾取怪物穿戴中的物品
```

### 16.2 丢入吞噬者

```
-- dropz() 中:
IF swallowed:
    IF obj != uball:
        IF is_unpaid(obj): stolen_value(...)
        IF NOT engulfer_digests_food(obj):
            mpickobj(u.ustuck, obj)  -- 加入吞噬者背包
        ELSE:
            -- 紫虫等消化类吞噬者直接消化肉类
            "It is instantly digested!"
            -- 可能引发特殊效果（石化吞噬者、变形等）
            delobj(obj)
```

---

## 测试向量

### 基础拾取

| # | 场景 | 输入/条件 | 预期输出 |
|---|------|-----------|----------|
| 1 | 地面单物品，菜单模式 | 地面有1把剑，玩家按 `,` | AUTOSELECT_SINGLE 生效，直接拾取，无菜单 |
| 2 | 地面多物品 | 地面有剑+盾，玩家按 `,` | 弹出菜单，可选择性拾取 |
| 3 | 拾取指定数量 | 地面有 20 支箭，玩家输入 `5,` | 拾取 5 支箭，地面剩 15 支 |
| 4 | 背包已满（52 物品） | 尝试拾取非金币新物品 | "Your knapsack cannot accommodate any more items." |
| 5 | 背包已满但地面有金币 | 尝试拾取新物品，金币也在地面 | "...cannot accommodate any more items (except gold)." |

### 负重边界

| # | 场景 | 输入/条件 | 预期输出 |
|---|------|-----------|----------|
| 6 | 恰好进入 Burdened | STR=18, CON=18, weight_cap=950, 当前负重 949, 拾取 owt=2 的物品 | inv_weight()=1, cap=(1*2/950)+1=1=SLT_ENCUMBER; 提示 "You have a little trouble lifting..." |
| 7 | OVERLOADED 边界 | weight_cap=1, inv_weight=1 | calc_capacity: wc<=1 → 直接 OVERLOADED |

### 金币重量

| # | 场景 | 输入/条件 | 预期输出 |
|---|------|-----------|----------|
| 8 | GOLD_WT 取整 | 持有 0 金币, 拾取 49 枚 | GOLD_WT(49)=(49+50)/100=0; 重量差 = 0+0-0 = 0 |
| 9 | GOLD_WT 边界 | 持有 50 金币, 拾取 50 枚 | 分别: GOLD_WT(50)=1, GOLD_WT(50)=1; 合并: GOLD_WT(100)=1; 差=1+1-1=1 |
| 10 | GOLD_WT 大值 | 持有 0 金币, 拾取 150 枚 | GOLD_WT(150)=(150+50)/100=2 |

### 恐吓怪物卷轴

| # | 场景 | 输入/条件 | 预期输出 |
|---|------|-----------|----------|
| 11 | 首次拾取 uncursed spe=0 | 拾取 | spe 变为 1, 卷轴存活 |
| 12 | 第二次拾取 uncursed spe=1 | 拾取 | "The scroll turns to dust"; 卷轴消失 |
| 13 | 拾取 blessed 卷轴 | 拾取 | unbless → 变为 uncursed; spe 不变; 卷轴存活 |
| 14 | 拾取 cursed 卷轴 | 拾取 | "The scroll turns to dust"; 卷轴消失 |

### Bag of Holding 爆炸

| # | 场景 | 输入/条件 | 预期输出 |
|---|------|-----------|----------|
| 15 | WoC(spe>0) 放入 BoH | depthin=0, rn2(1)<=0 | TRUE (100%) — 必然爆炸; d(6,6) 伤害 |
| 16 | 空 WoC(spe=0) 放入 BoH | 检查 spe<=0 | FALSE — 不爆炸 |
| 17 | BoH 嵌套 depth=3 | rn2(8)<=3 | 50% 爆炸概率 |
| 18 | 诅咒 BoH 打开 | 10 件物品 | 每件 1/13 消失; 期望 ~0.77 件消失 |

### 石化尸体

| # | 场景 | 输入/条件 | 预期输出 |
|---|------|-----------|----------|
| 19 | 无手套拾取 cockatrice 尸体 | 非 Stone_resistance, 非远程 | "Touching ... is a fatal mistake." → instapetrify |
| 20 | 有手套拾取 cockatrice 尸体 | uarmg != NULL | 安全拾取 |
| 21 | 赤脚踢 cockatrice 尸体 | 无靴子, 非 Stone_resistance | "kick with bare feet" → instapetrify |

### 商店交互

| # | 场景 | 输入/条件 | 预期输出 |
|---|------|-----------|----------|
| 22 | 自动拾取商店物品 | autopickup, costly_spot, NOT no_charge | autopick_testobj → FALSE; 不拾取 |
| 23 | 手动拾取商店物品 | 在商店内拾取 | addtobill → obj.unpaid=TRUE |
| 24 | 从商店外拾取商店物品 | 使用某种方式（如 telekinesis 假想） | addtobill + remote_burglary → 触发盗窃警报 |

### 巨石

| # | 场景 | 输入/条件 | 预期输出 |
|---|------|-----------|----------|
| 25 | Sokoban 中拾取巨石 | poly'd giant, Sokoban | "You cannot get your hands around this boulder." |
| 26 | 非 Sokoban 巨人拾取巨石 | throws_rocks, 有空槽 | 直接拾取，跳过重量检查 |
| 27 | 推巨石入水 | push boulder into pool | 90% 填满水池, 10% 沉没; 巨石消失 |
| 28 | 推巨石入岩浆 | push boulder into lava | 10% 填满, 90% 沉没+溅射 d(1or3, 6) 伤害 |

### 容器

| # | 场景 | 输入/条件 | 预期输出 |
|---|------|-----------|----------|
| 29 | 打开锁住的箱子 | container.olocked=TRUE | "It is locked." + autounlock 机制 |
| 30 | 打开有陷阱的箱子 | container.otrapped=TRUE | chest_trap() 触发; 消耗1回合 |
| 31 | 打开 bag of tricks | 地面 bag of tricks, #loot | "Develops huge teeth and bites you!"; rnd(10) 伤害 |
| 32 | 打开薛定谔之箱 | SchroedingersBox(box) | 50% 活猫出现 / 50% 死猫尸体; box.spe=0 |

---

## 附录 A: 物品 where 字段值

| 值 | 枚举名 | 含义 |
|----|--------|------|
| 0 | OBJ_FREE | 不附着于任何链表 |
| 1 | OBJ_FLOOR | 地面物品 |
| 2 | OBJ_CONTAINED | 在容器中 |
| 3 | OBJ_INVENT | 在玩家背包中 |
| 4 | OBJ_MINVENT | 在怪物背包中 |
| 5 | OBJ_MIGRATING | 迁移中（跨楼层） |
| 6 | OBJ_BURIED | 被埋葬 |
| 7 | OBJ_ONBILL | 在商店账单上 |
| 8 | OBJ_LUAFREE | 已释放但被 Lua 引用 |
| 9 | OBJ_DELETED | 标记待删除 |

## 附录 B: how_lost 字段值

| 值 | 枚举名 | 含义 |
|----|--------|------|
| 0 | LOST_NONE | 仍在背包中 / 未追踪 |
| 1 | LOST_THROWN | 被玩家投掷 |
| 2 | LOST_DROPPED | 被玩家丢弃 |
| 3 | LOST_STOLEN | 被怪物偷走 |
| 4 | LOST_EXPLODING | 物品正在爆炸 |

---

## 附录 C: 已知源代码注释 Bug

1. **盲人查看石化尸体**（pickup.c:72-73）：`simple_look()` 头部注释记载 "BUG: this lets you look at cockatrice corpses while blind without touching them"。盲人通过 `query_classes()` 的 `:` 查看选项可以安全查看石化尸体描述。[疑似 bug — 源代码自注]

2. **恐吓卷轴 spe 重置**（pickup.c:1792-1793）：`lift_object` 对 `SCR_SCARE_MONSTER` 在 `result <= 0` 时设 `spe = 0`。当 result == 0（玩家选择不拾取）时也会重置，理论上允许通过反复"尝试拾取但取消"来保持卷轴可无限次安全拾取。[疑似 bug]
