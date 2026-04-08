# Zero_Nine

**作者：Manus AI**

## 项目定位

**Zero_Nine** 是一个以 **Rust** 编写的统一编排内核，目标是把 **OpenSpec**、**Superpowers**、**Ralph-loop** 与 **OpenSpace** 的核心能力科学整合为一个可落地的工程系统。它并不机械合并四个上游仓库的源码，而是抽取四个项目各自最稳定、最有价值的语义边界，再通过统一的数据模型、状态机、工件目录与宿主适配层，将它们组合为一个可持续演进的工作流引擎。[1] [2] [3] [4]

从职责上看，Zero_Nine 将四个项目分别映射为四层体系。**OpenSpec** 负责需求、设计与任务工件管理；**Superpowers** 负责结构化执行、质量约束与验证节奏；**Ralph-loop** 负责长时循环、状态恢复与任务推进；**OpenSpace** 负责技能评估、演化候选与持续优化。[1] [2] [3] [4] 最终对用户只暴露一个统一入口，即类似 `/zero-nine <需求>` 的单一斜杠命令。

## 可行性结论

这件事是**能做到的**，但合理做法不是“把四个仓库硬拼成一个仓库”，而是构建一个 **Rust 编排核心 + 双宿主适配壳层 + 可复用技能资产** 的新项目。Claude Code 已支持插件市场形式分发命令、技能、代理与相关扩展；OpenCode 已支持项目级或全局级的命令目录与技能目录，并且还能发现 Claude 兼容技能目录。[5] [6] [7] 因此，Zero_Nine 既可以作为 **技能包** 工作，也可以继续演进为 **插件形态**，并通过同一个 Rust 核心向两个宿主暴露统一能力。

## 当前已实现内容

当前交付版本聚焦于 **最小可行骨架**。它已经具备一个可编译的 Rust workspace，包含共享类型、需求工件管理、执行策略、循环调度、技能演化和宿主适配六个主要模块，并提供可执行 CLI `zero-nine`。同时，项目已经生成 Claude Code 与 OpenCode 的基础适配文件，使得后续可以通过单一 slash command 将用户目标传入 Rust 核心。

| 模块 | 说明 |
| --- | --- |
| `zn-types` | 定义统一数据模型，包括 proposal、task、loop state、execution report、evolution candidate 等 |
| `zn-spec` | 管理 `.zero_nine/` 工件目录、proposal、tasks、progress 与 runtime events |
| `zn-exec` | 提供任务分类、执行计划生成与统一执行报告结构 |
| `zn-loop` | 实现 Zero_Nine 的循环驱动、状态推进、事件写入与结果汇总 |
| `zn-evolve` | 负责执行结果评分和演化候选生成 |
| `zn-host` | 输出 Claude/OpenCode 适配文件并处理宿主识别 |
| `zn-cli` | 提供 `init`、`run`、`status`、`resume`、`export` 命令 |

## 目录结构

当前项目结构保持尽量精简，避免无关文件膨胀。核心目录如下：

```text
Zero_Nine/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── docs/
│   └── architecture.md
├── scripts/
│   └── bootstrap_zero_nine.sh
├── crates/
│   ├── zn-types/
│   ├── zn-spec/
│   ├── zn-exec/
│   ├── zn-loop/
│   ├── zn-evolve/
│   ├── zn-host/
│   └── zn-cli/
└── adapters/
    ├── claude-code/
    └── opencode/
```

## 核心命令

当前 CLI 已实现以下核心命令：

| 命令 | 作用 |
| --- | --- |
| `zero-nine init --project <path> --host <host>` | 初始化 `.zero_nine/` 工作目录与项目 manifest |
| `zero-nine run --project <path> --host <host> --goal "..."` | 以一句话目标启动四层骨架流程 |
| `zero-nine status --project <path>` | 查看当前 proposal 与 loop 状态 |
| `zero-nine resume --project <path> --host <host>` | 从已有状态继续推进 |
| `zero-nine export --project <path>` | 导出 Claude Code 与 OpenCode 适配命令和技能文件 |

## 宿主接入方式

### Claude Code CLI

Claude Code 官方支持以插件市场形式分发插件，插件内可以包含 commands、skills、agents、hooks 以及相关服务组件。[5] 因而 Zero_Nine 在 Claude 侧最合理的路径是继续包装为完整插件。当前交付中已提供基础命令与技能适配文件，位于：

```text
adapters/claude-code/.claude/commands/zero-nine.md
adapters/claude-code/.claude/skills/zero-nine-orchestrator/SKILL.md
```

其核心思想是让 Claude 侧的 `/zero-nine` 命令把用户输入桥接到本地 Rust CLI，例如：

```bash
zero-nine run --host claude-code --project . --goal "用户需求"
```

### OpenCode CLI

OpenCode 原生支持通过 `.opencode/commands/` 定义自定义 slash commands，也支持通过 `.opencode/skills/`、`.claude/skills/` 等目录发现技能。[6] [7] 当前交付已经包含如下适配文件：

```text
adapters/opencode/.opencode/commands/zero-nine.md
adapters/opencode/.opencode/skills/zero-nine-orchestrator/SKILL.md
```

OpenCode 中的命令同样通过 `$ARGUMENTS` 传入用户需求，再调用本地 Rust CLI 执行统一编排流程。

## 一句话调用的实际语义

用户所说的“**一句话实现想要结果的最终完美呈现**”，在 Zero_Nine 中不应被理解为单段 prompt 魔法，而应被理解为：

> 用一条斜杠命令接收目标，再由 Rust 内核自动决定是否创建 proposal、是否更新设计、是否推进任务、是否进行验证、以及是否写回演化候选。

因此，`/zero-nine` 的本质不是 prompt 宏，而是一个**工作流入口**。这也是它能长期稳定扩展的原因。

## 已验证状态

当前项目已经完成本地编译验证，并完成最小烟雾测试。测试覆盖了初始化、运行目标、状态查询和适配导出四个核心路径。这表明项目骨架已经具备继续扩展的工程基础。

| 验证项 | 结果 |
| --- | --- |
| Rust workspace 编译 | 通过 |
| `zero-nine init` | 通过 |
| `zero-nine run` | 通过 |
| `zero-nine status` | 通过 |
| `zero-nine export` | 通过 |
| 中文目标 proposal 标识回退逻辑 | 已修复 |

## 下一阶段建议

如果你要把 Zero_Nine 继续推进到真正可投入日常使用的版本，下一步最值得优先做的是三件事。第一，把当前 `zn-exec` 从“骨架执行器”扩展成真正可调用宿主代理与外部工具的执行桥。第二，为 Claude Code 补齐完整插件清单与安装说明。第三，把 OpenSpace 式的演化逻辑从本地候选文件提升到可比较、可回滚、可选注入的技能版本系统。这样，Zero_Nine 就会从“可运行骨架”进一步进化为“可持续自优化的统一代理内核”。

## References

[1]: https://github.com/Fission-AI/OpenSpec "GitHub - Fission-AI/OpenSpec: Spec-driven development (SDD) for AI coding assistants"
[2]: https://github.com/obra/superpowers "GitHub - obra/superpowers: An agentic skills framework & software development methodology that works"
[3]: https://github.com/PageAI-Pro/ralph-loop "GitHub - PageAI-Pro/ralph-loop: A long-running AI agent loop"
[4]: https://github.com/HKUDS/OpenSpace "GitHub - HKUDS/OpenSpace: Make Your Agents: Smarter, Low-Cost, Self-Evolving"
[5]: https://code.claude.com/docs/en/plugin-marketplaces "Claude Code Docs - Create and distribute a plugin marketplace"
[6]: https://opencode.ai/docs/commands/ "OpenCode Docs - Commands"
[7]: https://opencode.ai/docs/skills/ "OpenCode Docs - Agent Skills"
