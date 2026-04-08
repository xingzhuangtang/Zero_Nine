# Zero_Nine 架构设计与 Rust 实现方案

**作者：Manus AI**

## 概述

**Zero_Nine** 的目标不是把 OpenSpec、Superpowers、Ralph-loop、OpenSpace 四个项目的源码机械拼接，而是抽取它们各自已经被社区验证过的核心语义，然后以一个统一的 **Rust 编排内核** 重新装配为一个可分发、可扩展、可接入多宿主 CLI 的工程化系统。其对外能力应尽量收敛为一个单一入口命令，例如 `/zero-nine <需求>` 或 `/zn <需求>`，而对内则保持四层职责分离：**需求层负责工件与知识，执行层负责实现与验证，调度层负责循环与恢复，进化层负责观测与演化**。[1] [2] [3] [4] [5] [6]

从官方资料看，这一目标具有现实可行性。OpenSpec 已证明“规格先行 + 斜杠命令 + 工件化目录”是可行的需求治理方式；Superpowers 已证明同一套技能资产可以跨 Claude Code、OpenCode 等宿主复用；Ralph-loop 已证明 fresh-context 的长时任务循环适合作为自动执行心脏；OpenSpace 则提供了自进化技能与集体智能的成熟语义。[1] [2] [3] [4]

## 设计原则

Zero_Nine 的架构设计遵循四条原则。第一，**统一协议，不统一实现细节**。也就是说，我们不试图把四个上游项目的内部实现强行收敛到一套代码，而是定义统一的事件、工件和状态协议。第二，**宿主适配层尽量薄**。Claude Code 与 OpenCode 只负责接收 slash command、加载技能、传递工作目录与参数，真正的流程编排全部交由 Rust 核心处理。第三，**所有自动化都必须可恢复、可审计、可降级**。因此每一步都必须有状态文件、事件日志和退出码语义。第四，**技能资产与执行内核解耦**。技能应该描述“怎么想、怎么做、怎么验”，而内核负责“什么时候做、做到哪里、失败怎么办”。

## 四层职责映射

| Zero_Nine 层 | 上游项目语义来源 | 在 Zero_Nine 中的实现职责 | Rust 子系统 |
| --- | --- | --- | --- |
| 需求层 | OpenSpec | 管理 proposal、spec、design、tasks、archive、spec index | `zn-spec` |
| 执行层 | Superpowers | 触发 brainstorming、writing-plans、TDD、review、verification 等工作流 | `zn-exec` |
| 调度层 | Ralph-loop | 选择下一个任务、迭代执行、记录状态、恢复中断、处理重试 | `zn-loop` |
| 进化层 | OpenSpace | 记录技能表现、自动修复、自动优化、生成候选技能版本 | `zn-evolve` |
| 宿主适配层 | Claude/OpenCode | 提供 slash command、技能入口、桥接脚本与安装说明 | `zn-adapters` |

这种职责分层意味着 Zero_Nine 的核心不是单体 CLI，而是一个 **workspace + orchestration runtime**。用户看见的是一个命令，系统内部运行的是一个可组合的状态机。

## 总体运行流

当用户输入 `/zero-nine 为当前仓库新增 XXX 能力` 后，宿主命令层不直接向大模型塞入大段工作流提示，而是先调用 Rust 核心，例如：

```bash
zero-nine run --host claude --project . --goal "为当前仓库新增 XXX 能力"
```

随后 Rust 核心按以下步骤工作。首先进入 **需求层**，生成或更新工作提案目录，沉淀规格、设计和任务清单。接着进入 **调度层**，扫描当前工作包中的任务 DAG，选出下一个可执行任务，并为该任务创建一次独立迭代上下文。然后进入 **执行层**，选择合适的技能链条，生成子任务计划，执行 TDD、代码审查和验证流程，并将结果结构化返回。最后进入 **进化层**，分析本次成功或失败原因，更新技能评分、失败模式和候选优化补丁，再将这些演化结果写入本地知识库，并在下一轮迭代前注入最新技能版本。[2] [3] [4]

## 目录结构设计

为了兼容用户文档设想与 OpenSpec 官方现行目录范式，Zero_Nine 采用“**统一内部格式 + 可选兼容视图**”的方案。内部以 `.zero_nine/` 为唯一事实来源，并提供对 `.ospec/` 或 `openspec/` 的镜像导出能力。

```text
project-root/
├── .zero_nine/
│   ├── proposals/
│   │   └── 20260404-feature-name/
│   │       ├── proposal.md
│   │       ├── design.md
│   │       ├── tasks.md
│   │       ├── dag.json
│   │       ├── progress.json
│   │       ├── verification.md
│   │       └── artifacts/
│   ├── archive/
│   ├── specs/
│   │   ├── index.md
│   │   └── patterns/
│   ├── loop/
│   │   ├── session-state.json
│   │   ├── iteration-log.ndjson
│   │   └── locks/
│   ├── evolve/
│   │   ├── skills/
│   │   ├── candidates/
│   │   ├── evaluations.jsonl
│   │   └── evolution.log
│   └── runtime/
│       ├── events.ndjson
│       └── cache/
├── .opencode/
│   ├── commands/
│   └── skills/
├── .claude/
│   └── skills/
└── src/
```

这里最关键的不是目录名，而是每个目录的 **语义稳定性**。例如 `progress.json` 不是简单文本状态，而是可恢复的任务执行状态对象；`events.ndjson` 不是普通日志，而是后续演化层学习的原始事件流。

## 核心数据模型

Zero_Nine 的 Rust 内核应首先稳定数据模型，因为这决定了宿主适配、状态恢复与技能演化是否可持续。建议使用 `serde` 序列化为 JSON/YAML，并以版本化结构保存。

| 数据对象 | 作用 | 关键字段 |
| --- | --- | --- |
| `ProjectManifest` | 项目级配置 | `version`、`default_host`、`skill_dirs`、`policy` |
| `Proposal` | 一次需求变更的元信息 | `id`、`title`、`goal`、`status`、`created_at` |
| `DesignDoc` | 技术设计快照 | `problem`、`scope`、`approach`、`verification` |
| `TaskGraph` | 任务 DAG | `tasks[]`、`edges[]`、`priority`、`estimates` |
| `LoopState` | Ralph 式循环状态 | `proposal_id`、`current_task`、`iteration`、`retry_count` |
| `ExecutionReport` | 单任务执行结果 | `success`、`tests_passed`、`review_passed`、`artifacts` |
| `SkillEvaluation` | 技能表现记录 | `skill_name`、`task_type`、`latency`、`token_cost`、`score` |
| `EvolutionCandidate` | 待注入新技能版本 | `source_skill`、`reason`、`patch`、`confidence` |

这些模型应被集中放在 `zn-types` crate 中，供所有其他 crate 共享。

## Rust Workspace 结构

推荐把 Zero_Nine 设计成 Cargo workspace，而不是单 crate。这样做的原因在于：第一，宿主适配层与编排内核的生命周期不同；第二，未来可以独立发布 CLI 与 SDK；第三，测试和依赖边界更清晰。

```text
Zero_Nine/
├── Cargo.toml
├── crates/
│   ├── zn-types/        # 共享数据模型
│   ├── zn-spec/         # 提案、设计、任务、归档
│   ├── zn-exec/         # 技能桥接、任务执行、验证
│   ├── zn-loop/         # 状态机、调度器、重试与恢复
│   ├── zn-evolve/       # 技能观测、评分、候选补丁生成
│   ├── zn-host/         # 宿主识别、环境变量、路径适配
│   ├── zn-cli/          # 对外 CLI
│   └── zn-bridge/       # 与外部代理/脚本通信
└── adapters/
    ├── claude-code/
    └── opencode/
```

## 模块职责说明

### `zn-spec`

`zn-spec` 负责需求侧全部工件的生命周期管理。它应支持从一句话需求生成提案目录，支持将 brainstorming 结果落盘为 `proposal.md` 与 `design.md`，支持生成 `tasks.md` 与 `dag.json`，还应支持归档和知识索引更新。这里不强制 OpenSpec 原样目录布局，但应保留它的工件思维与变更分箱机制。[1]

### `zn-loop`

`zn-loop` 是 Zero_Nine 的心脏。它应实现一个确定性调度循环，包括：加载 `LoopState`，扫描任务 DAG，选出当前可执行任务，创建本轮迭代上下文目录，调用执行层，读取 `ExecutionReport`，决定成功推进、失败重试、升级修复或人工暂停。它必须提供显式退出码，以支持宿主脚本和 CI 场景。其运行哲学应继承 Ralph-loop 的“每轮独立、进度持久化、失败可恢复”。[3]

### `zn-exec`

`zn-exec` 不直接实现“智能”，而是负责调度技能链和校验约束。它应具有任务分类器，用来识别当前任务属于 brainstorming、planning、implementation、debugging、review、verification 中的哪一类；还应具有执行报告标准化器，把来自不同宿主和代理的输出收敛为统一结构。其内部可以预置 Superpowers 风格的执行模版，如 brainstorming、writing-plans、test-driven-development、requesting-code-review、verification-before-completion 等。[2]

### `zn-evolve`

`zn-evolve` 负责把 OpenSpace 的价值变成实际工程能力。它至少应具备三种策略：当某技能连续失败时，生成 `AUTO-FIX` 候选补丁；当某类任务稳定成功且成本较高时，生成 `AUTO-IMPROVE` 建议；当出现新的高复用执行模式时，生成 `AUTO-LEARN` 候选技能。它不必在第一个版本中实现真正的云端协同训练，但必须设计好本地 `evaluations.jsonl` 和 `candidates/` 的格式，以便未来接入共享技能源。[4]

### `zn-host` 与 `zn-bridge`

`zn-host` 用于宿主环境探测，例如识别当前是 Claude Code、OpenCode，还是纯终端模式，并解析各宿主可用的环境变量、工作目录约定和技能目录位置。`zn-bridge` 则负责让 Rust CLI 和宿主的脚本命令对接，例如输出 Markdown 提示、生成技能清单、渲染宿主适配模板、调用外部辅助脚本等。

## 命令设计

对终端用户，Zero_Nine 应坚持“少而强”的命令面。可以先实现以下一级命令：

| 命令 | 作用 | 典型场景 |
| --- | --- | --- |
| `zero-nine run` | 以一句话目标启动完整四层工作流 | slash command 桥接入口 |
| `zero-nine init` | 初始化项目目录、技能目录和适配文件 | 首次接入项目 |
| `zero-nine status` | 查看当前 proposal、任务和迭代状态 | 用户查询进度 |
| `zero-nine resume` | 从最近一次状态恢复循环 | 中断后恢复 |
| `zero-nine archive` | 归档当前 proposal 并更新知识库 | 全部完成后 |
| `zero-nine evolve` | 评估技能表现并生成候选优化 | 离线维护或定时执行 |
| `zero-nine export` | 导出 Claude/OpenCode 适配文件 | 分发或安装 |

真正供 Claude Code 和 OpenCode 使用的 slash command 只需要调用这些命令即可。

## 宿主适配设计

### Claude Code 适配

Claude Code 的官方插件市场机制允许一个插件同时包含命令、技能、代理、钩子与服务器扩展。[5] 因此 Zero_Nine 在 Claude 侧的最佳落地方式是一个 **完整插件**，目录可以类似：

```text
adapters/claude-code/
├── .claude-plugin/
│   └── plugin.json
├── commands/
│   └── zero-nine.md
├── skills/
│   ├── zero-nine-orchestrator/
│   │   └── SKILL.md
│   ├── zero-nine-spec/
│   │   └── SKILL.md
│   ├── zero-nine-exec/
│   │   └── SKILL.md
│   └── zero-nine-evolve/
│       └── SKILL.md
└── bin/
    └── zero-nine-bridge.sh
```

其中 `/zero-nine` 命令只做三件事：接收用户意图、确定项目根目录、调用本地 `zero-nine run`。技能目录则负责让宿主代理在必要时能按需了解四层工作流与行为约束。

### OpenCode 适配

OpenCode 官方支持在 `.opencode/commands/` 中放置 Markdown 命令，也支持在 `.opencode/skills/` 或 `.claude/skills/` 中发现技能。[6] [7] 因此 OpenCode 侧不一定需要完整插件系统，第一版可直接输出项目级适配包：

```text
adapters/opencode/
├── .opencode/
│   ├── commands/
│   │   └── zero-nine.md
│   └── skills/
│       ├── zero-nine-orchestrator/
│       │   └── SKILL.md
│       ├── zero-nine-spec/
│       └── zero-nine-exec/
└── scripts/
    └── zero-nine-bridge.sh
```

命令模板中可以使用 `$ARGUMENTS`，把用户输入直接传给 Rust CLI；必要时还可以通过 `subtask: true` 强制子代理执行，降低主上下文污染。[6]

## 单斜杠命令设计

要实现用户所说的“一句话实现想要结果的最终完美呈现”，关键不在于把所有逻辑塞进一段 prompt，而在于把斜杠命令变成 **工作流入口**。建议统一命令名为 `/zero-nine`，并保留别名 `/zn`。其语义可以定义为：

> 接收一句话目标；若当前不存在活动 proposal，则自动创建；若存在未完成 proposal，则判定是继续推进、修订规格，还是查询状态；整个决策由 Rust 内核根据本地状态和输入意图完成。

这意味着 `/zero-nine` 实际是一个 **智能路由入口**，而不是单纯的 prompt 宏。

## 状态机设计

建议将调度层状态机显式化，否则很难保证恢复能力与错误处理质量。最小状态集如下：

| 状态 | 含义 | 进入条件 | 离开条件 |
| --- | --- | --- | --- |
| `Idle` | 无活动任务 | 项目初始化后或归档完成后 | 用户输入新目标 |
| `SpecDrafting` | 需求澄清与设计生成中 | 新目标进入系统 | design/tasks 准备完毕 |
| `Ready` | 设计已批准，待执行 | 任务图有效 | 进入调度迭代 |
| `RunningTask` | 正在执行某个任务 | 选出可执行任务 | 执行成功或失败 |
| `Verifying` | 进行测试与验收 | 实现完成 | 验证通过或失败 |
| `Retrying` | 针对失败任务自动重试 | 失败且未超过重试上限 | 返回 RunningTask 或 Escalated |
| `Escalated` | 升级给进化层修复 | 重试耗尽或错误模式稳定出现 | 修复完成或人工暂停 |
| `Archived` | 提案归档完成 | 所有任务完成 | 新需求创建新提案 |

Rust 代码中可以用枚举实现这一状态机，并为每次状态迁移写出事件。

## 事件总线与日志设计

Zero_Nine 若要支持 OpenSpace 风格的学习，必须把每轮执行写成 **结构化事件** 而不是普通文本。推荐使用 NDJSON，每个事件一行，便于追加写入和离线分析。

```json
{"ts":"2026-04-04T10:00:00Z","event":"proposal.created","proposal_id":"20260404-dark-mode"}
{"ts":"2026-04-04T10:02:00Z","event":"task.started","task_id":"1.2","skill":"writing-plans"}
{"ts":"2026-04-04T10:05:00Z","event":"task.failed","task_id":"1.2","reason":"tests_failed"}
{"ts":"2026-04-04T10:06:00Z","event":"evolution.candidate.created","skill":"test-driven-development","kind":"auto-fix"}
```

这样的事件模型能够同时支持状态恢复、审计、统计和技能演化。

## 技能资产设计

Zero_Nine 最终既是一个插件，也是一个技能包。为了复用性，技能不应只有一个超大 `SKILL.md`，而应拆成多层能力：

| 技能名 | 作用 | 适用宿主 |
| --- | --- | --- |
| `zero-nine-orchestrator` | 解释四层总流程与路由规则 | Claude/OpenCode |
| `zero-nine-spec` | 需求澄清、design/tasks 生成规范 | Claude/OpenCode |
| `zero-nine-exec` | 执行层技能链与验证约束 | Claude/OpenCode |
| `zero-nine-loop` | 调度与恢复规则 | Claude/OpenCode |
| `zero-nine-evolve` | 技能评估、补丁候选与注入策略 | Claude/OpenCode |

这种拆法比单体技能更适合按需加载，也更符合 OpenCode 原生 skill tool 的发现方式。[7]

## 最小可行版本建议

为了尽快让项目落地，而不是无限扩张，建议把 Zero_Nine 第一版约束在以下范围：

| 范围 | 第一版是否纳入 | 说明 |
| --- | --- | --- |
| Rust 统一 CLI 核心 | 是 | 必须 |
| 单斜杠命令 `/zero-nine` | 是 | 必须 |
| Claude Code 插件适配 | 是 | 用户目标明确要求 |
| OpenCode 命令与技能适配 | 是 | 用户目标明确要求 |
| 本地 proposal/design/tasks/progress 管理 | 是 | 必须 |
| 基础 Ralph 式循环、重试、恢复 | 是 | 必须 |
| 基础技能评分与候选补丁文件生成 | 是 | 必须，但先做本地版 |
| 真正的云端 OpenSpace 同步 | 否 | 可留到第二版 |
| 完整克隆四个上游项目所有功能 | 否 | 不现实且没必要 |

## 可行性结论

基于现有公开资料，可以明确回答用户最初的问题：**能做到，但应以“Rust 统一编排内核 + 技能资产复用 + 双宿主适配层”的方式实现，而不是源码层面的硬融合。** 这种方案既符合 OpenSpec/Superpowers/Ralph-loop/OpenSpace 的真实能力边界，也符合 Claude Code 与 OpenCode 当前的插件与技能扩展机制。[1] [2] [3] [4] [5] [6] [7]

换句话说，Zero_Nine 最合理的工程定义应是：

> 一个用 Rust 编写的四层代理编排系统，能够把规格管理、执行约束、长时任务循环与技能进化统一到一个状态机里，并通过 Claude Code 与 OpenCode 的命令/技能机制暴露为单一斜杠命令入口。

## 下一步实现策略

接下来的实现阶段应立即进入工程落地。第一步，初始化 Cargo workspace 与各 crates。第二步，先打通 `zero-nine init`、`zero-nine run`、`zero-nine status` 三个核心命令。第三步，生成 Claude Code 与 OpenCode 适配目录及其命令文件。第四步，补齐技能包与基础桥接脚本。第五步，完成一次本地编译与最小运行验证。到这一步，用户就已经可以在仓库中通过单一 slash command 调起 Zero_Nine 的骨架流程。

## References

[1]: https://github.com/Fission-AI/OpenSpec "GitHub - Fission-AI/OpenSpec: Spec-driven development (SDD) for AI coding assistants"
[2]: https://github.com/obra/superpowers "GitHub - obra/superpowers: An agentic skills framework & software development methodology that works"
[3]: https://github.com/PageAI-Pro/ralph-loop "GitHub - PageAI-Pro/ralph-loop: A long-running AI agent loop"
[4]: https://github.com/HKUDS/OpenSpace "GitHub - HKUDS/OpenSpace: Make Your Agents: Smarter, Low-Cost, Self-Evolving"
[5]: https://code.claude.com/docs/en/plugin-marketplaces "Claude Code Docs - Create and distribute a plugin marketplace"
[6]: https://opencode.ai/docs/commands/ "OpenCode Docs - Commands"
[7]: https://opencode.ai/docs/skills/ "OpenCode Docs - Agent Skills"
