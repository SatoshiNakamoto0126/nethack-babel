# NetHack Babel（迷宫通灵塔）

**NetHack 3.7 的现代 Rust 重新实现，内建多语言支持。**

![CI](https://github.com/user/nethack-babel/actions/workflows/ci.yml/badge.svg)
[![License: NGPL](https://img.shields.io/badge/License-NGPL-blue.svg)](LICENSE)
![Rust: nightly](https://img.shields.io/badge/Rust-nightly-orange.svg)
![Tests: 1195](https://img.shields.io/badge/tests-1195_passing-brightgreen.svg)

## 概述

NetHack Babel 是 [NetHack 3.7](https://github.com/NetHack/NetHack) 的 Rust 从零重写版本，保持公式级别的精确度，同时采用现代架构。它用 hecs ECS 替代原版的全局状态和手动内存管理，将游戏逻辑与所有 IO 完全分离，并将所有游戏内容——怪物、物品、地牢——定义在 TOML 数据文件中而非编译时表格。引擎发出类型化事件而非格式化字符串，通过 Project Fluent 实现内建三语支持（英文、简体中文、繁体中文），无需修改任何游戏逻辑。

## 特性

- **真彩色终端渲染** — 基于 ratatui 的 TUI，BUC 着色背包、语法高亮消息、小地图
- **内建三语支持** — 英文、简体中文、繁体中文，运行时 `O` 键热切换
- **CJK 物品命名** — 量词系统："3把匕首"而非"3 daggers"；BUC 前缀："祝福的+2长剑"
- **数据驱动架构** — 383 种怪物、369 种物品，TOML 定义，无需重新编译即可修改内容
- **ECS 游戏状态** — hecs 实体组件系统，显式回合解算与类型化事件
- **公式级精确** — 28 份机制规格（31,274 行）从原版 C 源码提取；1,195 个测试验证忠实度
- **深度对齐** — 战斗公式、药水/卷轴/魔杖 BUC 矩阵、商店定价、祈祷机制、怪物 AI、饥饿系统、陷阱伤害——全部与原版 NetHack 行为一致，包括已记录的边缘情况
- **跨平台** — macOS、Linux、Windows
- **遗骨系统** — 死亡后留下幽灵和被诅咒的物品供未来角色发现
- **确定性回放** — 显式 RNG 传递；相同种子 + 相同输入 = 相同游戏
- **计划中** — Steam 集成（云存档、成就、创意工坊）、SSH 多人服务器、asciinema 录制

## 快速开始

```sh
git clone https://github.com/user/nethack-babel.git
cd nethack-babel
cargo run -- --data-dir data --language zh_CN
```

需要 Rust nightly，通过 `rust-toolchain.toml` 自动选择。使用 [rustup](https://rustup.rs/) 无需手动设置。

## 操作按键

| 按键 | 动作 | 按键 | 动作 |
|------|------|------|------|
| `h` `j` `k` `l` | 移动（vi 键） | `y` `u` `b` `n` | 对角移动 |
| 方向键 | 移动（四方向） | `.` | 等待一回合 |
| `i` | 查看背包 | `,` | 拾取物品 |
| `d` | 丢弃物品 | `e` | 进食 |
| `q` | 饮用药水 | `r` | 阅读（卷轴） |
| `z` | 使用魔杖 | `f` | 射击（远程） |
| `w` | 装备武器 | `W` | 穿戴护甲 |
| `T` | 脱下护甲 | `P` | 佩戴饰品 |
| `R` | 摘下饰品 | `s` | 搜索（隐藏门/陷阱） |
| `<` | 上楼 | `>` | 下楼 |
| `o` | 开门 | `c` | 关门 |
| `p` | 向店主付款 | `O` | 选项 / 切换语言 |
| `S` | 保存并退出 | `Ctrl+C` | 退出 |
| `?` | 帮助 | `Ctrl+P` | 消息历史 |
| `#` | 扩展命令 | `0`-`9` | 计数前缀 |

### 扩展命令

按 `#` 后输入命令名（Tab 自动补全）：

| 命令 | 动作 | 命令 | 动作 |
|------|------|------|------|
| `pray` | 祈祷 | `loot` | 搜刮容器 |
| `enhance` | 提升武器技能 | `name` | 命名物品 |
| `dip` | 浸泡物品 | `ride` | 骑乘 |
| `offer` | 在祭坛献祭 | `quit` | 退出游戏 |

## 架构

六个 Cargo crate 的工作空间，依赖关系严格单向；引擎不执行任何 IO。

| Crate | 职责 | 代码行数 | 测试数 |
|-------|------|---------|--------|
| `engine` | 纯游戏逻辑——战斗、怪物、物品、地牢、回合循环，27 个模块 | 47,000 | 1,059 |
| `data` | TOML 数据定义与加载器：怪物、物品、地牢 | 3,920 | 23 |
| `i18n` | 基于 Fluent 的本地化、物品命名（doname）、CJK 量词 | 3,644 | 66 |
| `tui` | 基于 ratatui + crossterm 的终端界面 | 2,696 | 13 |
| `audio` | 基于 rodio 的音效，由引擎事件触发 | 340 | 10 |
| `cli` | 可执行文件入口——配置、存读档、主循环编排 | 3,147 | 24 |

## 游戏系统

| 系统 | 测试数 | 主要特性 |
|------|--------|---------|
| 近战战斗 | 66 | 完整命中/伤害链、负 AC 二次判定、背刺、双武器、武僧武术 |
| 怪物 AI | 34 | 逃跑逻辑、开门交互、飞行/游泳/穿墙、传送、觊觎行为 |
| 地牢生成 | 58 | 房间/走廊、13 种特殊房间、8 个地牢分支、楼梯可达性 |
| 陷阱（25 种） | 33 | 按深度放置、伤害公式、探测、回避、陷阱特效 |
| 药水（26 种） | 51 | 完整 BUC × 混乱矩阵、治疗公式、抗酸、隐形术 |
| 卷轴（23 种） | 40 | 鉴定 rn2(5)、高附魔武器附魔、灭绝 BUC×混乱、混乱怪物 |
| 魔杖（24 种） | 25 | 死亡杖 vs 不死族、充能爆炸、自我使用、最后一击、交叉抗性 |
| 商店系统 | 94 | 完整定价管线、魅力表、价格鉴定、盗窃、信用、12 种商店、基石警察 |
| 宗教/祈祷 | 103 | 祈祷冷却、成功链、效果优先级、冥界规则、加冕、运气 -13..+13 |
| 宠物系统 | 69 | 食物品质、战斗 AI、宠物坟场复活、跨层跟随、牵绳 |
| 饥饿/进食 | 98 | 戒指/护符/再生饥饿、尸体腐烂、种族修正、昏厥、饿死 |
| 状态效果 | 45 | 11 种计时效果与衰减、尸体内在能力、混乱方向随机化 |
| 遗骨系统 | 25 | 死亡快照、幽灵行为、物品诅咒/降级、反作弊 |
| 得分与经验 | 41 | 怪物经验公式（8 种加成）、等级阈值、分数计算 |
| 经典策略验证 | 20 | Elbereth、果冻养殖、价格鉴定、Excalibur 浸泡、独角兽角 |

## 国际化系统

NetHack Babel 将所有玩家可见文本与游戏逻辑分离：

- **消息模板** — [Project Fluent](https://projectfluent.org/) `.ftl` 文件
- **实体名称翻译** — TOML 文件映射英文怪物/物品名到中文
- **量词（量词）** — `classifiers.toml` 将物品类别映射到正确的中文量词
- **物品命名** — `doname()` 管线产生"祝福的+2长剑"（中文）或 "a blessed +2 long sword"（英文）
- **热切换** — 游戏内按 `O` 切换，无需重启

### 添加新语言

1. 创建 `data/locale/<代码>/manifest.toml`
2. 添加 `messages.ftl` 消息翻译
3. 可选添加 `monsters.toml`、`objects.toml` 实体名称翻译
4. CJK 语言需添加 `classifiers.toml` 量词映射

## 机制规格

`specs/` 目录包含 28 份机制规格，共 31,274 行，从原版 NetHack C 源码提取、审阅并验证。每份规格记录了某个游戏子系统的精确公式、常数、边缘情况和测试向量。

## 构建

```sh
cargo build                              # 构建
cargo test --workspace                   # 运行全部 1,195 个测试
cargo run -- --data-dir data             # 运行游戏（英文）
cargo run -- --data-dir data --language zh_CN  # 运行游戏（简体中文）
cargo run -- --data-dir data --text      # 纯文本模式（无 TUI）
cargo clippy --workspace --all-targets   # 代码检查
cargo build --release                    # 发布构建
```

## 项目状态

游戏引擎已完成对齐方案第 4 阶段。所有核心系统——战斗、物品、怪物、地牢生成、宠物、宗教、陷阱、商店、饥饿、状态效果、鉴定、操守、遗骨、存读档和终端界面——均已实现，并通过 1,195 个测试与原版 NetHack 源码交叉验证。

详见 [ALIGNMENT_REPORT_PHASE2.md](ALIGNMENT_REPORT_PHASE2.md)（对齐报告）和 [DIFFERENCES.md](DIFFERENCES.md)（已知差异）。

## 许可证

[NetHack 通用公共许可证 (NGPL)](LICENSE)。

第三方 crate 依赖限制为 MIT、Apache-2.0、BSD 或 LGPL 许可证。

## 致谢

NetHack Babel 建立在 [NetHack 开发团队](https://nethack.org/) 数十年工作的基础之上。原版 NetHack 源码：[github.com/NetHack/NetHack](https://github.com/NetHack/NetHack)。
