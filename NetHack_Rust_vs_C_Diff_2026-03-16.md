# NetHack Rust 重制版 vs C 原版差异对比（详细版）

文档日期：2026-03-16  
对比目录：
- Rust：`/Users/hz/Downloads/nethack-babel`
- C：`/Users/hz/Downloads/NetHack`

说明：
- 本文基于本地源码静态核对，重点按玩家完整流程整理。
- 不是逐帧实机通关报告，但核心差异均有代码证据。

---

## 1. 总览结论

当前 Rust 版已经不再是“只能跑前期”的阶段，特殊关卡、Quest、终局分支在代码上覆盖明显提升；但和 C 原版相比，仍有三类关键差距：
- 机制细节的 1:1 一致性（典型如 Luck 影响概率）
- 命令入口到交互壳层的完整度（不少扩展命令还没接全）
- 结局展示链路（披露菜单、墓碑细节、榜单接入）完整度

如果目标是经典行为一致性，C 原版仍是标准；如果目标是后续可维护扩展，Rust 版方向更优。

---

## 2. 启动与建角阶段

## 2.1 启动入口与参数系统

### C 原版
- Unix 常见入口是包装脚本 `sys/unix/nethack.sh`，负责 `HACKDIR`、`XUSERFILESEARCHPATH`、`PAGER` 等环境设置后再执行游戏。
- `doc/nethack.6` 参数体系成熟且丰富，包含：
  - `-w/--windowtype`
  - `--nethackrc` / `--no-nethackrc`
  - `-u`、`-p`、`-r`
  - `-s/--scores`
  - 多个职业快捷参数

### Rust 版
- `clap` 参数定义在 `crates/cli/src/main.rs`，主要包括：
  - `--config`
  - `--language`
  - `-D/--debug`
  - `-u/--name`
  - `--data-dir`
  - `--text`
  - `--role`、`--race`
  - `--record`
  - `--server`、`--replay`（当前明确是 stub）

差异总结：
- C 的参数体系更“历史全覆盖”。
- Rust 的 CLI 更现代化，但 server/replay 仍未落地。

## 2.2 建角流程（role/race/alignment/name）

Rust 的建角流程在 `crates/cli/src/game_start.rs` 中结构化实现：
- 13 职业定义
- 合法种族组合、合法阵营组合
- TUI 菜单模式和 text 模式都支持
- `--role/--race` 支持前缀匹配和大小写不敏感

这部分在工程组织上比 C 更清晰，但最终体验受 UI 链路成熟度影响。

## 2.3 存档恢复入口

### C
- 文档中明确了 save/lock/recover 的传统流程（`nethack.6`）。

### Rust
- 启动时 `try_load_save()` 会按玩家名尝试恢复存档。
- 存档默认目录是 XDG 风格（`$XDG_DATA_HOME/nethack-babel` 或 `~/.local/share/nethack-babel`）。
- 格式有 magic/version 校验（`.nbsv`）。

---

## 3. 游戏过程核心差异

## 3.1 架构风格

### C
- 全局状态 + 过程式逻辑 + windowport 深度耦合。

### Rust
- ECS + 事件驱动 + TOML 数据驱动。
- RNG 显式传递并可序列化。

结论：
- Rust 可维护性和可测试性更好。
- C 的历史行为稳定性仍更强。

## 3.2 中后期内容覆盖（特殊关、Quest、终局）

Rust 当前代码显示已覆盖大量中后期内容：
- `special_levels.rs`：Castle、Medusa、Fort Ludios、Vlad、Wizard Tower、Sanctum、Elemental Planes、Astral 等
- `data/dungeons/dungeon_topology.toml`：Main/Mines/Sokoban/Quest/Gehennom/VladsTower/Endgame 分支与连接
- `quest.rs`：13 角色 Quest 状态机、leader/nemesis/artifact 映射、资格判定、对话键、遭遇推进

注意：
- `DIFFERENCES.md` 里部分“未实现”表述和现状不一致，文档存在滞后。

## 3.3 机制细节对齐：门交互是明确差异点

### C
- 开门、踢门等使用 `rnl(...)`，会受 Luck 调整（`src/lock.c`, `src/dokick.c`）。

### Rust
- `crates/engine/src/movement.rs` 明确注释：开门公式参考 C，但当前把 `rnl` 简化为均匀随机（Luck 调整待补）。

影响：
- 老玩家会感知到概率体感差异，尤其在门交互与相关风险控制上。

## 3.4 扩展命令链路完整度

`crates/tui/src/input.rs` 中可看到不少扩展命令仍是占位映射（`None`），例如：
- `travel`
- `chat`
- `offer`
- `dip`
- `throw`
- `wear` / `wield` / `open` / `close` / `kick`
- `cast` / `invoke` / `jump`

含义：
- 引擎动作能力在推进，但“输入命令 -> 方向/物品/位置提示 -> 真正执行”的前端链路尚未全部接完。

---

## 4. 设置选项差异

## 4.1 配置来源兼容

### C
- `.nethackrc` + `NETHACKOPTIONS` 是核心，文档与实现成熟。
- `include/optlist.h` 选项体系非常庞大。

### Rust
- 原生主配置是 TOML：`config.toml`。
- 同时支持传统 `OPTIONS=` 行和 `.nethackrc` 解析（`parse_options_line`, `load_nethackrc`）。
- `ALL_OPTIONS` 元数据较多（代码里约 146 条定义）。

## 4.2 运行时菜单可改项

Rust 当前 in-game 选项菜单主要分：
- Game：autopickup / autopickup_types / legacy
- Display：若干显示开关
- Sound：开关和音量
- Language：切语言

对比 C：
- C 的 in-game options 深度和覆盖显著更高，且和各 window port 能力联动更成熟。

---

## 5. 界面与窗口系统差异

## 5.1 端口生态

### C
- 多 window port 生态（tty/curses/X11/Qt/win32 等），`doc/window.txt` 文档完备。

### Rust
- 目前主实现是单一 TUI 端口（`impl WindowPort for TuiPort`）。
- `--text` 是回退模式，不等于完整多端口体系。

## 5.2 交互成熟度

### C
- 消息窗口、`--More--` 节奏、菜单与状态栏行为都非常成熟。

### Rust
- TUI 现代化程度高，本地化友好。
- 但命令链路未完全覆盖，体验上会遇到“能看到命令名但尚未完整可用”的情况。

---

## 6. 结局阶段差异（死亡/逃脱/升天）

## 6.1 计分公式

### C
- `src/end.c` 的经典链路：净金币（死亡税）、深度奖励、深层奖励、升天倍率。

### Rust
- `crates/engine/src/end.rs` 的 `calculate_final_score()`基本按同类逻辑实现：
  - 净金币 + 死亡税
  - 深度奖励
  - 深层奖励
  - Ascension 2x / 1.5x

## 6.2 披露菜单链路

### C
- `done()` -> `disclose()` 全链路成熟：
  - inventory
  - attributes/enlightenment
  - vanquished/genocided
  - conduct/achievements
  - dungeon overview

### Rust
- 引擎层有相关结构与逻辑储备。
- 但当前主流程结束展示更简化，尚未看到 C 同级别的完整披露流程接入。

## 6.3 墓碑与榜单

### C
- `rip.c` 经典墓碑模板，含姓名、金币、死因、年份。
- `topten.c` 写 record/logfile/xlogfile 并展示排名。

### Rust
- `end.rs` 有 tombstone 数据/渲染工具。
- `topten.rs` 有 record/xlog/leaderboard 结构与格式化能力。
- 但当前主循环 GameOver 的 UI 呈现是简化墓碑（`tui_port.rs`），榜单链路没有完全连成 C 那样的可见流程。

---

## 7. 存档与容错差异

### Rust
- `SaveReason` 包含 `Quit/Checkpoint/Panic`。
- 可配置 checkpoint 周期。
- 存档版本兼容检查清晰。

### C
- 传统 save/lock/bones/recover 生态，实战成熟度高。

---

## 8. 对玩家的实际影响（从开局到结算）

1. 开局体验：Rust 不差，且语言/配置现代。  
2. 中后期内容：Rust 已明显超出“半成品早期”。  
3. 细节手感：关键概率与行为仍有未对齐点。  
4. 选项和界面：C 的深度、端口生态、稳定性更强。  
5. 结算仪式感：C 的披露+墓碑+榜单完整度领先。

---

## 9. 推荐结论（资深玩家视角）

- 追求经典一致性与稳定流程：优先 C 原版。
- 接受阶段性差异并关注长期演进：Rust 版值得持续跟进。
- Rust 下一阶段最关键的体验项：
  - Luck 相关概率细节对齐
  - 扩展命令交互壳层补齐
  - 结局披露 + 排行榜链路完整接入
  - 运行时选项菜单覆盖扩展到与配置声明一致

---

## 10. 关键证据文件（便于复查）

Rust：
- `crates/cli/src/main.rs`
- `crates/cli/src/game_start.rs`
- `crates/cli/src/config.rs`
- `crates/cli/src/save.rs`
- `crates/engine/src/movement.rs`
- `crates/engine/src/special_levels.rs`
- `crates/engine/src/quest.rs`
- `crates/engine/src/end.rs`
- `crates/engine/src/topten.rs`
- `crates/tui/src/tui_port.rs`
- `data/dungeons/dungeon_topology.toml`

C：
- `doc/nethack.6`
- `doc/window.txt`
- `sys/unix/nethack.sh`
- `include/optlist.h`
- `src/lock.c`
- `src/dokick.c`
- `src/end.c`
- `src/rip.c`
- `src/topten.c`

