# Zero_Nine 增强架构设计

## 设计目标

**Zero_Nine** 的下一阶段目标，不再只是“能跑通一个骨架式编排”，而是要把你要求的主链真正落成一个可持续演进的工程系统：**Superpowers Brainstorming → OpenSpec 工件写入 → Ralph-loop 调度推进 → Superpowers writing-plans 二次拆分 → worktree/隔离环境 → 子代理开发与审查 → TDD/验证 → finishing-a-development-branch → OpenSpec 进度更新 → OpenSpace 持续监控与技能进化**。

这个系统在对外形态上，应当优先表现为 **Claude Code CLI** 与 **OpenCode CLI** 的原生插件／技能入口，支持单一斜杠命令触发；在对内形态上，则要逐步形成一个可独立演进的 **Rust CLI + Rust SDK** 双层结构。

## 总体架构

| 层级 | 目标能力 | Zero_Nine 模块 | 下一阶段增强重点 |
| --- | --- | --- | --- |
| 需求层 | 苏格拉底式需求澄清、规格沉淀、变更追踪 | `zn-spec` | 把 brainstorming 结果写入正式 OpenSpec 工件 |
| 执行层 | writing-plans、TDD、review、finishing 等工程执行约束 | `zn-exec` | 引入技能链、阶段式执行协议、分支与工区生命周期 |
| 调度层 | fresh-context、任务循环、失败重试、卡点推进 | `zn-loop` | 引入阶段机、progress.txt、可恢复迭代与任务选择器 |
| 进化层 | 观测、评分、自动修复、自动注入、经验沉淀 | `zn-evolve` | 从事后评分升级为持续监控与候选注入 |
| 宿主层 | Claude / OpenCode 插件入口与独立 CLI/SDK 兼容 | `zn-host` / `zn-cli` | 从薄包装命令升级为宿主原生命令集与能力导出 |

## 关键设计原则

Zero_Nine 不应机械复制四个上游项目的全部源码，而应**抽取其工作流语义并重新实现为统一协议**。因此，增强版架构采用“统一状态模型 + 统一工件模型 + 统一执行协议 + 多宿主适配”的方式推进。

这意味着后续实现不应围绕“把所有外部技能原样塞进项目”展开，而应围绕下面四个核心对象建立：

| 核心对象 | 作用 |
| --- | --- |
| `RequirementPacket` | 承接 Superpowers Brainstorming 的结构化输出 |
| `SpecBundle` | 表示 OpenSpec 侧 proposal/design/tasks/DAG/progress 的统一视图 |
| `ExecutionEnvelope` | 表示单个 Ralph-loop 迭代交给执行层的任务上下文 |
| `EvolutionSignal` | 表示 OpenSpace 观测层对每轮任务的评价、建议与可注入补丁 |

## 目标数据流

### 1. Brainstorming 到 OpenSpec

第一步要把 `Brainstorming` 从“任务分类标签”升级为**有输入、有澄清、有沉淀产物**的完整阶段。其输出应至少包含以下字段：

| 字段 | 含义 |
| --- | --- |
| `user_goal` | 用户原始一句话目标 |
| `problem_statement` | 明确化后的问题陈述 |
| `scope_in` / `scope_out` | 范围边界 |
| `constraints` | 技术、时间、宿主、分支、质量约束 |
| `acceptance_criteria` | 验收标准 |
| `risks` | 需求和执行层面的风险 |
| `next_questions` | 尚需追问的问题 |

这些结构化信息随后自动映射进 OpenSpec 工件：

| Brainstorming 输出 | 写入工件 |
| --- | --- |
| `problem_statement` | `proposal.md` |
| `constraints` / `scope_*` | `requirements.md` |
| `acceptance_criteria` | `acceptance.md` |
| `risks` | `design.md` 的风险章节 |
| `next_questions` | `clarifications.md` 或下一轮会话提示 |

### 2. OpenSpec 到 Ralph-loop

`zn-spec` 应成为真正的规格工件总线，而不仅仅是写几个 Markdown 文件。后续 `zn-loop` 读取时，不能只依赖 `proposal.tasks` 的内存结构，而应显式读取：

| 工件 | Ralph-loop 用途 |
| --- | --- |
| `design.md` | 当前 PRD / 设计上下文 |
| `tasks.md` | 初始任务清单 |
| `dag.json` | 依赖判断与可执行任务选择 |
| `progress.txt` | 当前推进位置与阻塞项 |
| `verification.md` | 验证状态与是否允许进入下一阶段 |

### 3. Ralph-loop 到 Superpowers

调度层不直接执行“实现”，而是把每轮选中的任务包装成一个 `ExecutionEnvelope`，送给执行层。这个对象应至少包含：

| 字段 | 含义 |
| --- | --- |
| `proposal_id` | 当前提案 ID |
| `task_id` | 当前任务 ID |
| `task_title` | 当前任务标题 |
| `context_files` | 当前轮必须读取的工件文件 |
| `execution_mode` | `brainstorming` / `writing_plans` / `implementation` / `review` / `verification` / `finish_branch` |
| `workspace_strategy` | `in_place` / `git_worktree` / `sandbox` |
| `quality_gates` | 测试、审查、验证门禁 |

## 执行层增强设计

### A. 引入阶段式执行模式

`zn-exec` 需要从目前的四类任务模板，升级成可串联的技能链。建议定义以下执行模式：

| 模式 | 作用 |
| --- | --- |
| `brainstorming` | 苏格拉底式需求澄清，沉淀需求包 |
| `spec_capture` | 把需求包写入 OpenSpec 工件 |
| `writing_plans` | 针对当前任务进一步拆解为迭代计划 |
| `workspace_prepare` | 建立 worktree、命名分支、准备隔离目录 |
| `subagent_dev` | 生成开发代理任务说明 |
| `subagent_review` | 生成代码审查代理任务说明 |
| `tdd_cycle` | 测试先行、修复、回归 |
| `verification` | 最终验证与证据审计 |
| `finish_branch` | 标准化分支收尾、合并/PR/放弃选项 |

### B. 引入工作区策略

你特别强调了 **Superpowers 的隔离沙盒和 worktree 新分支**，因此下一阶段应在类型系统中增加 `WorkspaceStrategy`。建议支持：

| 策略 | 说明 |
| --- | --- |
| `InPlace` | 当前目录直接运行，适合纯规格任务 |
| `GitWorktree` | 为实现类任务创建独立 worktree 与 feature branch |
| `Sandboxed` | 为高风险任务创建临时隔离工区 |

后续 प्रथम版可以先实现 **GitWorktree 计划与文档生成**，再逐步升级到真实自动执行。

### C. 引入子代理协议

子代理不是一定要马上真的多线程跑起来，但应先定义输出协议。建议每一轮可产生：

| 子代理 | 输出 |
| --- | --- |
| `developer` | `dev-brief.md`、实施建议、修改清单 |
| `reviewer` | `review-brief.md`、风险点、拒绝/通过理由 |
| `verifier` | `verification-checklist.md`、证据清单 |

这样就算前期仍以单代理执行，也已经为后续接入真正 subagent 机制预留接口。

## 调度层增强设计

`zn-loop` 需要从“按数组顺序跑任务”升级为**阶段机 + 任务选择器 + 验证卡点**。

建议引入以下推进规则：

| 规则 | 说明 |
| --- | --- |
| fresh-context 规则 | 每轮执行前重新构造 `ExecutionEnvelope`，不依赖上轮推理上下文 |
| DAG 规则 | 仅选择依赖全部完成的任务 |
| Gate 规则 | 若测试或审查未通过，禁止推进到下一任务 |
| Resume 规则 | 任意中断后根据 `progress.txt` 和 `session-state.json` 恢复 |
| Drift 规则 | 若 `design.md` 或 `tasks.md` 发生变更，则暂停当前轮并重新规划 |

## 进化层增强设计

目前 `zn-evolve` 仍是“结束后评分”。增强版应转向背景观测层，哪怕第一阶段仍以文件方式模拟，也要让模型上具备这些概念：

| 能力 | 第一阶段实现方式 |
| --- | --- |
| 执行观测 | 从 `events.ndjson` 与 `iteration-log.ndjson` 聚合信号 |
| 失败分析 | 对失败轮生成 `autofix-*.md` 候选 |
| 成功捕获 | 对高质量流程生成 `captured-pattern-*.md` |
| 自动注入 | 在下一轮 `skill_chain` 里追加推荐技能 |
| 经验沉淀 | 写入 `.zero_nine/evolve/library/` |

## Claude Code / OpenCode 插件化设计

宿主层应从“把 `zero-nine run` 包进一个命令”升级为更接近原生体验的入口结构。

### OpenCode

建议最终形成如下目录：

| 路径 | 作用 |
| --- | --- |
| `.opencode/commands/zero-nine.md` | 单入口 Slash 命令 |
| `.opencode/skills/zero-nine-orchestrator/SKILL.md` | 总控技能 |
| `.opencode/skills/zero-nine-brainstorming/SKILL.md` | 需求澄清技能 |
| `.opencode/skills/zero-nine-writing-plans/SKILL.md` | 任务二次拆分技能 |
| `.opencode/skills/zero-nine-finish-branch/SKILL.md` | 分支收尾技能 |

### Claude Code

建议最终形成如下目录：

| 路径 | 作用 |
| --- | --- |
| `.claude/commands/zero-nine.md` | 单入口 Slash 命令 |
| `.claude/skills/zero-nine-orchestrator/SKILL.md` | 总控技能 |
| `.claude/skills/zero-nine-brainstorming/SKILL.md` | 需求澄清技能 |
| `.claude/skills/zero-nine-writing-plans/SKILL.md` | 任务细化技能 |
| `.claude/skills/zero-nine-finish-branch/SKILL.md` | 收尾技能 |

## 独立 CLI / SDK 演进路线

为了未来独立发展，Zero_Nine 不应停留在“宿主命令包装”。建议在 Rust workspace 中预留更清晰的边界：

| 模块 | 角色 |
| --- | --- |
| `zn-types` | 对外共享类型 |
| `zn-spec` | 规格工件 API |
| `zn-exec` | 执行链 API |
| `zn-loop` | 调度引擎 API |
| `zn-evolve` | 观测与进化 API |
| `zn-host` | 宿主适配层 |
| `zn-cli` | 终端入口 |
| `zn-sdk` | 面向嵌入式调用的公共封装（下一阶段新增） |

第一阶段不一定马上新建 `zn-sdk` crate，但至少要在现有 crate 中保证：**核心逻辑函数可被其他 Rust 程序直接调用，而不仅仅供 CLI 调用**。

## 推荐落地顺序

| 阶段 | 目标 |
| --- | --- |
| Phase A | 把 Brainstorming → OpenSpec 工件写入真正打通 |
| Phase B | 把 Writing-plans → 迭代计划 → progress.txt 更新打通 |
| Phase C | 引入 worktree / branch / finish-branch 计划与工件 |
| Phase D | 增强宿主技能导出，形成更原生的 Claude/OpenCode 插件体验 |
| Phase E | 重构 evolve 层，引入观测、捕获、自动注入 |
| Phase F | 抽取 SDK 层，形成独立 CLI/SDK 双形态 |

## 本轮代码升级范围

基于当前项目状态，本轮最合理的升级范围是：

1. 增强 `zn-types`，补充 brainstorming、worktree、进度、子代理等结构体。
2. 增强 `zn-spec`，让其写出更完整的 OpenSpec 风格工件，包括 `requirements.md`、`acceptance.md`、`progress.txt`。
3. 增强 `zn-exec`，让其支持 `brainstorming`、`writing_plans`、`workspace_prepare`、`finish_branch` 等模式的工件生成。
4. 增强 `zn-loop`，让其按阶段机推进，并把 `tasks.md` 的结果继续拆分为迭代级计划。
5. 增强 `zn-host`，导出更丰富的宿主技能目录，而不只是一个总命令包装。
6. 更新 `README.md` 和 `quickstart.md`，明确“插件入口优先、CLI/SDK 演进并行”的产品路线。

这意味着本轮不会一次性实现真正的云同步 OpenSpace，也不会立刻实现完全自动化的真实多代理并行开发；但会把所有关键接口、工件和主链打通，使 Zero_Nine 从“骨架演示”升级为“清晰可扩展的工程底座”。
