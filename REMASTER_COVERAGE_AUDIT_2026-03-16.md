# NetHack Babel 100% 重制覆盖审计（2026-03-16，代码重读版）

## 审计快照

- 仓库：`/Users/hz/Downloads/nethack-babel`
- 基线提交：`e51f37e`
- 审计时间：`2026-03-16 23:55 CST`
- 口径：**仅基于代码与测试结果**（不以文档声明为依据）

## 结论（先看）

- **当前仍未达到“100%重制”**。  
- 若按“模块存在 + 测试通过”口径：约 **90%+**，且从领域模块规模看可视为 **99.x% 结构覆盖**。  
- 若按“玩家端到端链路（启动→过程→配置→界面→结局）”口径：约 **80%~86%**。  
- 本轮相较上次继续前进：`turn.rs` + `combat.rs` 的剩余 TODO 已全部清零，`Engrave/Eat/Quaff/Read/Fire/Pray/EnhanceSkill` 均已接入真实逻辑分发（含 ECS 状态读写），gas cloud 已持久化到 `DungeonState` 并纳入新回合 tick，随机刷怪已接线为真实生成事件，mounted 命中修正已读取 ECS 的骑术技能；`Ride` 与 `ToggleTwoWeapon` 已从占位改为真实状态机接线；角色创建已把 `PlayerIdentity + PlayerSkills + ConductState` 写入 ECS，`Engrave/Eat/Read/Pray` 已写回 conduct 计数，结局榜单已读取真实身份字段，`dip` 的 Knight 判定已改为正确 RoleId 常量，且存档格式已把 `PlayerIdentity/PlayerSkills/ConductState` 纳入持久化（`SAVE_VERSION=0.3.1`）；输入层新增 `repeat/reqmenu/perminv/shell/suspend` 的 TUI 接线，不再静默吞键，其中 `Ctrl+A/#repeat` 可复用上一次可重放动作；`#wiz...` 与 `Ctrl` wizard 快捷键已完成 `--debug` 门控接线，text 模式也新增同门控下的 wizard 命令解析（含 `wizwish/wizgenesis/wizlevelport` 参数形态）并扩展了非 wizard 命令映射（如 `look/conduct/discoveries/attributes/twoweapon/turnundead`）；`Ctrl+R` 与 `#herecmdmenu/#therecmdmenu` 也已有显式行为；其余 wizard 扩展命令已统一为“有门控且有明确提示”（不再无反馈）；同时 TUI 的库存/装备视图都已接到真实 ECS 快照，库存字母映射每回合同步，text 模式补齐了库存/装备直接展示并新增上下文命令解析（不仅覆盖 `open h/drop a/throw a l/zap b k/dip a b`，也覆盖 `annotate/engrave/call/name/travel/jump/lookat/whatis/cast` 参数形态），并补充了 `offer/eat/quaff/read/whatis` 无参形态与 `takeoffall` 直达映射。
- 新增落地：text 模式事件输出已统一走 `events_to_messages`（显示本地化文案，而非 message key/debug 串）；text 模式命令支持 `#` 前缀；上下文命令新增 `retravel/showtrap` 坐标形态直达。
- 新增落地：text 模式已打通 `save/savequit` 命令到真实存档写盘（`SaveReason::Checkpoint/Quit`），与“可手动保存”的链路声明一致。
- 新增落地：text 模式已支持 `repeat`（复用上一次可重放动作，UI/存档类动作自动排除），与 TUI `#repeat` 语义进一步趋同。
- 新增落地：autopickup 已打通配置联动（启动时注入 + options 菜单变更后即时生效）；状态栏已接入真实 gold 统计与 encumbrance 显示；`--replay` 已从 stub 升级为可执行 NDJSON 回放；`--server` 已从 `NotImplemented` 升级为可运行 TCP 监听（多连接 + 连接上限 + line-based 会话协议）；测试临时路径已唯一化，串行/并发执行稳定性提升。
- 增量落地（2026-03-17）：`makemon` 生成链路已补齐 `MonsterAttacks/MonsterResistances/MonsterSpeciesFlags/Intelligence` 与 `Spellcaster/Covetous` 组件挂载；`mhitu` 的 `MagicMissile` 分支已优先接入 `mcastu::castmu`（无施法元数据时回退旧路径）；`pet_shop_steal_check` 已从 skeleton 升级为“越店门 + 携带货物”可触发的真实判定并回写 `shop_rooms` 抢劫状态。
- 增量落地（2026-03-17）：补齐 endgame 全流程 touchstone（`touchstone_25_*`）覆盖死亡/飞升下的结算、披露与榜单链路；新增 nightly 回归工作流 `.github/workflows/nightly-differential.yml`（differential + property + Monte Carlo，支持可选 C 侧语料刷新）；并修复 `scripts/generate_c_recording.sh` / `scripts/diff_test.sh` / `scripts/fuzz_c_recordings.sh` 参数错位，保证脚本链路可执行。

## 外部反馈核对（本次新增）

- 已核对并确认以下数据成立：
  - `crates/engine/src` 为 **80** 个 Rust 模块文件。
  - `crates/engine/src` 总行数 **133,607**。
  - 反馈时点 `crates/engine/src` 的 `TODO` 为 **21**；当前代码已降至 **0**（`turn.rs` 与 `combat.rs` 均已清零）。
  - 巨型模块体量与反馈一致：`special_levels.rs` 7323、`monster_ai.rs` 4691，`potions/combat/hunger/turn` 均在 4k+。
- 同时需要保留的边界判断：
  - “99.x 结构覆盖”不等于“玩家链路 100%”；`turn.rs` / `input.rs` 仍有剩余可达性缺口（以 wizard 与 text-mode 为主）。
  - `--server` / `--replay` 已均从显式 stub 升级为 MVP 可运行形态，但 server 侧仍是“协议层 MVP”，尚未接入完整远程回合玩法体验。

## 客观指标（当前代码）

- 工作区 Rust 源文件：**115** 个 `.rs`
- 工作区总 LOC（所有 crate `.rs`）：**173,913**
- 各 crate LOC：
  - `engine`: 144,476
  - `cli`: 9,268
  - `i18n`: 7,209
  - `tui`: 7,692
  - `data`: 4,928
  - `audio`: 340
- 引擎核心域（仅 `crates/engine/src`）：
  - 模块文件数：**80**
  - 总行数：**138,244**
  - `tokei` 统计：`Code=103,198 / Comments=10,828 / Blanks=15,592`（Rust）
- 全量测试：**4,217 passed / 0 failed**（另有 2 ignored doctest）

## 按玩家链路的覆盖评估

### 1) 启动与运行模式：**中高（约 80%）**

- 主流程可运行，且新增了配置/恢复接线：
  - 启动时会读取 `~/.nethackrc`（若存在）。
  - 安装 panic save hook，并尝试 `panic.nbsv` 恢复。
  - TUI 回合中启用自动 checkpoint。
  - 角色创建后会将 `PlayerIdentity + PlayerSkills + ConductState` 写回玩家 ECS 组件（不再只改基础属性/名称）。
  - 存档/读档已纳入 `PlayerIdentity + PlayerSkills + ConductState`（save format `0.3.1`）。
- `--server` 已具备 MVP 监听与会话处理（不再是 `NotImplemented`），`--replay` 已打通 NDJSON 回放。

### 2) 游戏过程（turn loop 与动作执行）：**中高（约 70%~80%）**

- `resolve_turn` 框架完整（玩家→怪物→新回合边界）。
- 关键动作占位面已明显收缩：
  - `Engrave/Eat/Quaff/Read/Fire/Pray` 已完成主链路接线，并保留必要的 fallback 分支（例如无法推断物品类型时的 generic 提示）。
  - `Ride`、`ToggleTwoWeapon`、`EnhanceSkill` 均已接线。
- 本轮新增实装：
  - `Name` 的 `Item/Monster/Level` 目标已接入真实语义（分别落到实体名/层注释状态）。
  - `Adjust` 已接入真实语义（切换库存字母，并在冲突时交换字母）。
  - `CallType` 不再只发事件，已落地到可持久状态（`called_item_classes`）。
  - `Annotate` 不再是 UI-only 空分支，已写入当前层注释状态。
  - 玩家移动后已接 `inventory::autopickup()`（默认金币自动拾取）。
  - `ZapWand` 已接入真实语义：从物品实体读取 `WandTypeTag + WandCharges`，调用 `wands::zap_wand()` 并回写充能；缺失组件时保留 generic 回退路径。
  - `Untrap` 已改为读取玩家 `Attributes.dexterity`（不再使用常量 dex=14）。
  - `Swap` 已改为读取装备状态（副手是否存在、主武器是否诅咒）再调用 `do_swap_weapons()`。
  - `Wipe` 已改为读取/写回 `HeroCounters.creamed`，并识别诅咒毛巾（off-hand 命名包含 towel + cursed）阻断擦脸。
  - `TurnUndead` 已改为读取玩家角色（`PlayerIdentity.role`，Priest/Knight 视为 clerical）、玩家等级与周边真实 undead 数量（`MonsterSpeciesFlags::UNDEAD`）。
  - `Jump` 已改为读取 boots 装备状态判断跳跃能力，且按是否 `jumping boots` 动态设置最大跳跃距离（3/2）。
  - `Kick` 已接入 Monk 角色判定（`PlayerIdentity.role == MONK`）并启用武僧徒手加成分支。
  - `Engrave` 已按当前持握物推断刻写方式（Dust/Blade/Fire/Lightning/Dig）。
  - `Eat` 已从物品实体推导 `FoodDef` 并走 `hunger::eat_food()`。
  - `Quaff` 已从实体推导 `PotionType` 并走 `potions::quaff_potion()`。
  - `Read` 已从实体推导 `ScrollType` 并走 `scrolls::read_scroll()`（含 confused 分支）。
  - `Fire` 已接入装备槽读取（weapon/off-hand）并调用 `ranged::resolve_fire()`。
  - `Pray` 已接入 `ReligionState` 组件化持久读写并调用 `religion::pray_simple()`。
  - `Offer`（有物品时）已接入 `ConductState.gnostic` 写回（atheist 口径）。
  - `Engrave/Eat/Read/Pray` 已把 conduct 计数写回玩家 `ConductState` 组件（不再仅动作局部态）。
  - `Ride` 已接线为“已骑乘则下马，否则尝试挂载相邻 tame 怪物”的真实流程（复用 `steed` 子系统）。
  - `ToggleTwoWeapon` 已接线为真实模式切换：读取装备槽校验主/副手，写回 `PlayerSkills.two_weapon`，并发出 on/off 消息。
  - 新回合 `gas cloud` 已接入 `DungeonState.gas_clouds` 持久状态（不再是局部临时向量）。
  - 新回合随机刷怪已接线为真实 monster 生成事件。
- 系统级缺口仍在：
  - autopickup 已与外部配置选项联动（启动时应用，且 options 菜单变更后即时写回 `DungeonState`）。
  - 若物品缺少足够类型信息（无稳定 tag/可解析命名），仍会走 generic fallback；属于“鲁棒兜底”而非主链路缺失。

### 3) 命令输入与可达性：**较高（约 82%~90%）**

- `map_extended_command` 统计：`Some=45, None=47`（但 `None` 中大量为“需二次提示”的已接线路径）。
- `#extended` 现已覆盖两类分发：
  - 统一 `PromptKind`（方向/物品/物品+方向）：`open/close/kick/chat/fight/run/rush/untrap`、`wear/wield/.../force`、`throw/zap`。
  - 自定义文本/位置/类别提示：`travel/retravel/jump/whatis/showtrap/glance/annotate/engrave/call/name/adjust/knownclass/cast/dip`。
- `#name` 已提供 item/monster/level 目标选择；monster 通过坐标选点接线到引擎语义（`MonsterAt`）。
- 普通按键路径新增了同类自定义提示接线（`;` `/` `^` `` ` `` `E` `C`），不再只能通过 `#extended` 间接触发。
- `A` / `#takeoffall` 已接入 `TakeOffAll` 真实动作（不再是吞键/空转）。
- `repeat/reqmenu/perminv/shell/suspend` 已在 TUI 层接线：  
  - `Ctrl+A/#repeat`：回放上一次可重放动作。  
  - `m/#reqmenu`：弹出命令索引。  
  - `|/#perminv`：可达并分发到库存查看（已改为真实 ECS 背包渲染，不再传空列表）。  
  - `!/#shell`、`Ctrl+Z/#suspend`：给出明确系统级提示，不再静默吞键。
- TUI 主循环已每回合同步 inventory-letter 映射（从 ECS 实际背包读取），使 `drop/wield/wear/remove/throw/zap/...` 等“输入字母选物品”的动作使用当前帧一致的映射。
- text 模式命令集补充了 `eq/equip/equipment`（映射到 `ViewEquipped`），且 `inventory/equipment` 在 text 模式下直接展示真实快照（不再只看事件 key）；同时新增上下文参数解析，支持方向/物品字母命令与文本/坐标命令直接生成真实动作。
- text 模式现在支持 `#` 扩展命令前缀（例如 `#pray/#drink/#wizmap`），并且 `retravel/showtrap` 也可在 text 模式通过坐标参数直达动作解析。
- text 模式 `save/savequit` 已不再是空映射：`save` 执行 checkpoint 存档，`savequit` 执行 quit 存档并退出循环。
- text 模式 `repeat` 已接线：可复用上一条可重放动作，并对 `View*/Help/Save/Quit` 等非回合推进动作做过滤，避免无意义重复。
- 额外系统输入接线：`Ctrl+R` 已映射到 `Redraw`；`#herecmdmenu/#therecmdmenu` 已映射到命令索引展示；`#exploremode` 已给出显式占位提示。
- `wizard` 命令接线已从“不可达”提升到“可达且门控”：
  - TUI 下 `#wizidentify/#wizmap/#wizdetect/#wizwhere/#wizkill/#wizwish/#wizgenesis/#wizlevelport` 已接线。
  - `Ctrl+E/F/G/I/V/W` 快捷键已接线到对应 wizard 动作（`--debug` 开启时可用，关闭时给出提示）。
  - text 模式解析新增 `wizidentify/wizmap/wizdetect/wizwhere/wizkill`，同样受 `--debug` 门控。
- 剩余可达性缺口主要集中在“部分 wizard 子命令仅有占位提示、尚无真实语义实现”（如 `wizloadlua/wizloaddes/wizfliplevel/wizmakemap`）以及 text 模式与 TUI 的功能深度仍不一致（尤其是需二次提示的复杂命令）。
- text 模式 `parse_command` 已不再是最小集合，已覆盖库存/装备查看、若干信息查询与一批无提示动作别名；另新增 `parse_text_mode_contextual_command` 处理“命令 + 方向/字母/文本/坐标参数”形态，但仍缺少与 TUI 同级的交互式 prompt 流程。

### 4) 选项与配置：**中高（约 72%~80%）**

- `load_nethackrc()` 已接入 `main` 启动流程（相较上次是实质进展）。
- `show_game_options()` 改为使用 `options_menu_items()`，当前可编辑约 **45** 项。
- 仍有边界：
  - 顶层 options 结构仍是 `game/display/sound/language` 四入口。
  - 部分机制配置仍未与对应 gameplay 逻辑完全闭环。

### 5) 界面与本地化：**中高（约 75%~85%）**

- TUI 链路可玩，包含消息、菜单、墓碑、榜单、语言切换。
- text 模式消息渲染已从“事件 key 调试输出”切换为统一本地化文案（复用 `events_to_messages`），与 TUI 消息语义趋同。
- 语言 key 完整度仍不均：
  - `en/zh-CN/zh-TW`: 1627
  - `de/fr`: 191

### 6) 结局与披露：**中（约 65%）**

- 已有墓碑、conduct 区块、排行榜保存与展示。
- 但仍存在占位：
  - conduct 披露已读取玩家 `ConductState` 组件；新开局会初始化该组件，且 `Engrave/Eat/Read/Pray` 已接线写回，并随存档持久化。其余动作尚未全量写回，异常路径缺失组件时仍回退默认值。
  - 排行榜 role/race/gender/alignment 已读取 `PlayerIdentity`，组件缺失时回退默认文案。

## 模块存在 vs 主流程接线

- `engine` 导出模块很多，但“导出并不等于主流程可达”。
- 静态引用检查（排除模块自身文件）显示：
  - `shop` / `wish` 在工作区里主要被 `touchstone` 测试导入。
  - `quest` 未看到明确主流程调用链引用。
- 这意味着当前“机制覆盖”与“玩家可体验覆盖”之间仍有差距。
- 换句话说：领域层已经接近“全量搬运”，但交互触发层仍有缝隙。

## 迈向 100% 的优先顺序（当前版本）

1. 收敛输入层剩余断点：补齐仍为 `None` 的 wizard 子命令与 text 模式高频命令映射，继续压缩“有模块但不可达”。  
2. 继续压缩 generic fallback 覆盖面，并把 `conduct` 等状态从“局部写回”推进到“全链路实时写回”。  
3. 将结局披露与排行榜的剩余 fallback（组件缺失回退）进一步缩小到异常路径。  
4. 把 server 从“协议层 MVP”推进到“远程可玩 MVP”（把 line-based 会话与真实游戏回合/UI 进一步闭环）。  
5. 在 nightly 回归（differential/property/Monte Carlo）基础上继续扩大 C 侧语料规模，并把偏差 triage 流程产品化（自动归档失败样本 + 关联已知偏差表）。  

---

## 关键证据（代码定位）

- 扩展命令 `#` 与按键路径已接入 prompt/custom-prompt 分发，系统命令在 TUI 层已有显式处理（`repeat/reqmenu/perminv/shell/suspend`）：  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/input.rs:176`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/input.rs:1570`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:200`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:210`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:288`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:799`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:825`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:2207`
- wizard 命令输入可达性与 `--debug` 门控：  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:143`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:166`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:218`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:291`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:989`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1396`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1404`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1482`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2695`
- text 模式命令解析扩展（方向 token、坐标 token、spell token、上下文参数命令、debug 门控与非 wizard 别名）：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1110`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1141`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1147`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1158`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1264`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1620`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1668`
- text 模式 `#` 前缀与上下文别名接线（`retravel/showtrap`）及回归测试：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1358`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1376`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1397`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1636`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1762`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1778`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2816`
- text 模式 `save/savequit` 解析与执行（含帮助文案与回归断言）：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1489`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1490`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1621`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1624`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2851`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2915`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2930`
- text 模式 `repeat` 接线与可重放动作过滤（含回归断言）：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1508`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1692`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2892`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2917`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2921`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2938`
- text 模式事件本地化渲染（含 `ItemDropped/Wielded/Worn/Removed` 事件翻译接线）：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:611`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:753`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:763`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:773`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:783`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1099`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2904`
- TUI 库存链路已从占位改为真实快照：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2185`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2214`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2358`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2458`
- 装备视图链路已从占位改为真实快照（TUI + text mode）：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2234`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2464`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2695`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2816`
- text 模式 `equipment` 命令解析：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1459`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1540`
- text 模式无参直达映射（`offer/eat/quaff/read/whatis/takeoffall`）：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1461`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1464`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1466`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1482`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1605`
- `repeat` 所需的“上一条可重放动作”已在主循环记录：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2655`
- `turn.rs` 新增动作接线（已覆盖 `Engrave/Eat/Quaff/Read/Fire/Pray` 主链路）：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:739`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:798`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:811`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:825`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:903`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1008`
- 类型推断与持久状态辅助函数（food/potion/scroll、engrave、religion、随机刷怪方向/落点）：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:2587`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:2637`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:2684`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:2730`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:2818`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:2882`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:2968`
- gas cloud 已从临时变量改为 `DungeonState` 持久字段并在新回合 tick：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/dungeon.rs:655`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:451`
- 随机刷怪已接线为真实 `MonsterGenerated` 事件：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:507`
- `Ride` / `ToggleTwoWeapon` 已接入真实流程（含状态辅助函数与回归测试）：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1061`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1174`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1586`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1604`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:5315`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:5374`
- `EnhanceSkill` 已接入真实升级判定与消息分发（含回归测试）：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1074`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:5530`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:5568`
- `ToggleTwoWeapon` 新增多语言消息键（on/off）：  
  `/Users/hz/Downloads/nethack-babel/data/locale/en/messages.ftl:1726`  
  `/Users/hz/Downloads/nethack-babel/data/locale/zh-CN/messages.ftl:1678`
- 角色创建已写入 `PlayerIdentity + PlayerSkills`（并含回归测试）：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/game_start.rs:603`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/game_start.rs:625`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/game_start.rs:641`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/game_start.rs:649`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/game_start.rs:922`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/game_start.rs:989`
- conduct 状态已接入动作写回（`Engrave/Eat/Read/Pray`）：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:748`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:808`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:847`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1024`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1053`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1057`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:3152`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:3177`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:6154`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:6179`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:6230`
- 身份/技能/conduct 已纳入存档持久化（save format `0.3.1`）：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/save.rs:29`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/save.rs:139`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/save.rs:142`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/save.rs:145`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/save.rs:320`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/save.rs:574`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/save.rs:581`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/save.rs:587`
- mounted 命中修正已读取 ECS 骑术技能（`PlayerSkills[WeaponSkill::Riding]`），并含等级映射与单测：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/combat.rs:892`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/combat.rs:952`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/combat.rs:4669`
- `name/adjust` 主链路已落地（含输入提示与引擎语义）：  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:608`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:599`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/action.rs:217`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1095`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1131`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1775`
- `name(monster)` 可达性与解析链路（位置选怪 -> `MonsterAt` -> 实体改名）：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/action.rs:109`  
  `/Users/hz/Downloads/nethack-babel/crates/tui/src/app.rs:626`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1130`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1775`
- `annotate/call` 已写入可持久状态（随 `DungeonState` 保存）：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/dungeon.rs:648`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/dungeon.rs:750`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/dungeon.rs:778`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1289`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:1362`
- autopickup 状态（已接配置联动）：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/dungeon.rs:726`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/turn.rs:2445`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:1972`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2003`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2887`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:3350`
- replay / server 状态：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/recording.rs:207`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:3155`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:3173`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/server.rs:76`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/server.rs:80`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/server.rs:197`
- 本轮新接线（配置/状态栏/回放）：  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:389`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:430`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:2003`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/recording.rs:207`
- 结局披露/榜单已改为读取组件状态（含 fallback）：
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:823`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:831`  
  `/Users/hz/Downloads/nethack-babel/crates/cli/src/main.rs:857`
- fountain dip 的 Knight 判定已修正为角色常量：  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/dip.rs:449`  
  `/Users/hz/Downloads/nethack-babel/crates/engine/src/dip.rs:911`
