# Zero_Nine 下一阶段实施设计

## 目标说明

本阶段的目标不是继续堆叠静态工件，而是把 **Zero_Nine** 从“可编排、可导出、可落盘”的工程骨架，推进为一个具备真实交互、真实隔离执行、真实代理桥接与未来可嵌入能力的统一系统。实现顺序遵循你刚确认的路线：先做 **真实交互式 Brainstorming**，再做 **Git worktree / branch / finish-branch**，然后做 **子代理执行与审查桥接**，最后抽出 **独立 Rust SDK**。

这一路线的核心原则是：**先补主链的决策质量，再补工程执行约束，再补执行主体扩展，最后补产品化接口稳定性**。这样可以避免 SDK 先抽象、但核心行为还在频繁变化的结构性返工。

| 阶段 | 核心目标 | 首要落点 |
| --- | --- | --- |
| Phase 1 | 真实交互式 Brainstorming | 多轮澄清会话、问题队列、需求压缩与 OpenSpec 强绑定写入 |
| Phase 2 | 可执行 Git 工作区能力 | worktree 创建、分支命名、状态记录、finish-branch 操作 |
| Phase 3 | 子代理桥接 | developer / reviewer / verifier 三类代理协议与执行适配 |
| Phase 4 | 独立 SDK | 提炼公共 API，保持 CLI 仅做参数解析与输出 |

## Phase 1：真实交互式 Brainstorming

当前项目里虽然已经有 `TaskKind::Brainstorming` 和 `RequirementPacket`，但仍然属于“单轮生成式澄清”。下一阶段应把它升级为**会话驱动的多轮澄清机制**。实现重点不是让模型“更会写文档”，而是让系统知道：什么时候应该继续追问、什么时候信息足够进入规格沉淀、什么时候必须把未决问题显式保留给后续阶段。

| 新增对象 | 作用 | 建议放置 |
| --- | --- | --- |
| `BrainstormSession` | 保存一轮需求澄清会话状态 | `zn-types` |
| `ClarificationQuestion` | 表示待问问题及其优先级 | `zn-types` |
| `ClarificationAnswer` | 表示用户回答与时间戳 | `zn-types` |
| `BrainstormVerdict` | 表示“继续追问 / 可以收敛 / 升级人工确认” | `zn-types` |

在命令层面，需要新增一个与现有 `run` 并列的入口，例如 `brainstorm`。该命令应支持两种模式：一种是 **Terminal 交互模式**，直接在终端中逐轮提问；另一种是 **Host 辅助模式**，生成 `clarifications.md` 与 `brainstorm-session.json`，让 Claude Code / OpenCode 在宿主中继续对话时可读可续。这样既兼容当前 CLI，也为宿主插件保留交互空间。

| 命令 | 作用 | 备注 |
| --- | --- | --- |
| `zero-nine brainstorm --goal ...` | 启动交互式需求澄清 | 终端优先可先落地 |
| `zero-nine brainstorm --resume` | 继续未完成澄清会话 | 依赖 `.zero_nine/brainstorm/` 状态 |
| `zero-nine run --goal ...` | 若信息不足，自动跳转到 brainstorming | 保持单入口体验 |

Brainstorming 完成后，系统不应只写一个 requirements 摘要，而应形成**强绑定写入链**。也就是说，`problem_statement`、`scope_in`、`scope_out`、`constraints`、`acceptance_criteria`、`risks`、`next_questions` 必须分别映射到固定工件区块，并记录来源会话。这样后续 Ralph-loop、writing-plans、verification 才能知道哪些内容是“确认过的”，哪些仍然是“待决项”。

| Brainstorm 字段 | 写入目标 | 约束 |
| --- | --- | --- |
| `problem_statement` | `proposal.md` | 必须作为问题定义段落 |
| `scope_in` / `scope_out` | `requirements.md` | 必须分区展示 |
| `constraints` | `requirements.md` / `design.md` | 同时影响执行策略 |
| `acceptance_criteria` | `acceptance.md` | 必须序号化 |
| `risks` | `design.md` | 需进入风险章节 |
| `next_questions` | `clarifications.md` | 未完成时禁止标记 Ready |

## Phase 2：Git Worktree / Branch / Finish-Branch

第二阶段的目标，是把当前执行层里的 `WorktreePlan` 从“说明文档”升级为“真实可执行动作”。这一阶段必须尽量使用 Git 自身的原生命令，而不是在 Zero_Nine 内部重新发明一套版本控制逻辑。系统只负责：生成安全的分支命名、建立工作树、记录生命周期、在结束时提供标准化收尾选项。

| 能力 | 最小可行实现 | 后续增强 |
| --- | --- | --- |
| worktree 创建 | `git worktree add` 新目录并检出 feature branch | 自动检测冲突与路径复用 |
| branch 命名 | 基于 proposal/task 生成稳定分支名 | 加入用户自定义策略 |
| 生命周期记录 | 写入 `workspace.json` 与 `finish-branch.md` | 关联 PR URL、合并 SHA |
| finish-branch | merge / pr / discard / keep 四选一 | 宿主 UI 交互确认 |

这里建议在 `zn-exec` 新增一个更接近执行语义的函数，例如 `prepare_workspace()`，并把实际 Git 调用放进单独模块，避免和纯计划生成逻辑混在一起。与此同时，在 `zn-loop` 中新增“只有工作区准备成功，Implementation 才能进入 Running”的门禁。这样系统就不会在主分支上误执行实现任务。

## Phase 3：子代理执行与审查桥接

第三阶段的目标不是立刻做复杂的并行调度，而是先定义 **桥接协议**。Zero_Nine 应该先知道如何把一个任务分发成 developer / reviewer / verifier 三个角色，并为每个角色准备上下文输入、预期输出和结果回写位置。先把协议建稳，后续无论接 Claude Code、OpenCode，还是未来自己的 agent runtime，都能复用。

| 角色 | 输入 | 输出 | 当前实现建议 |
| --- | --- | --- | --- |
| developer | writing plan、design、task context | `dev-brief.md`、patch note、evidence | 先生成任务包与调用占位 |
| reviewer | diff、风险、测试结果 | `review-brief.md`、通过/拒绝意见 | 先生成审查包 |
| verifier | acceptance、evidence、review verdict | `verification-checklist.md`、最终结论 | 先生成验证包 |

这里可以在 `ExecutionReport` 上继续扩展，例如加入 `agent_runs`、`review_verdict`、`verification_verdict` 等结构化字段，而不再只用一个 `summary` 和若干字符串数组。这样未来真正接外部代理时，不需要再推翻现有数据模型。

## Phase 4：独立 SDK 抽象

SDK 阶段应尽量晚于前面三项，因为 SDK 的价值在于**稳定复用**，不是提前抽象。当前更合理的做法是先让 `zn-spec`、`zn-exec`、`zn-loop` 的核心函数以“纯 Rust API”形式存在，再在工作区中新增 `zn-sdk` 作为统一门面，把多 crate 能力组织成嵌入式调用接口。

| SDK 接口 | 作用 | 来源 |
| --- | --- | --- |
| `ZeroNine::init()` | 初始化项目结构 | `zn-loop` / `zn-spec` |
| `ZeroNine::brainstorm()` | 运行或恢复澄清会话 | `zn-exec` / `zn-spec` |
| `ZeroNine::run_goal()` | 从目标推进完整链路 | `zn-loop` |
| `ZeroNine::export_hosts()` | 导出 Claude/OpenCode 宿主文件 | `zn-host` |
| `ZeroNine::status()` | 查询 proposal 与 loop 状态 | `zn-loop` |

SDK 不应知道命令行参数解析，也不应直接打印到 stdout。CLI 只负责参数解析、调用 SDK、渲染文本输出。这样以后如果你想把 Zero_Nine 做成独立桌面端、Web 服务端，或者其他 Rust 程序的依赖库，就不需要再次拆解 CLI 内核。

## 本轮建议的直接代码动作

基于当前项目基础，下一轮实现建议按下面顺序实际改代码，而不是同时大改所有 crate。先把行为链做通，再逐步替换占位实现。

| 顺序 | 动作 | 涉及模块 |
| --- | --- | --- |
| 1 | 给 `zn-types` 增加 Brainstorm 会话模型与分支收尾结果模型 | `zn-types` |
| 2 | 在 `zn-cli` 新增 `brainstorm` 子命令与 `--resume` 入口 | `zn-cli` |
| 3 | 在 `zn-spec` 增加会话状态落盘、`clarifications.md`、来源追踪写入 | `zn-spec` |
| 4 | 在 `zn-exec` 增加真实问题生成、工作区准备和 finish-branch 执行函数 | `zn-exec` |
| 5 | 在 `zn-loop` 中加入“信息不足先 Brainstorm、工作区未就绪不实现”的门禁 | `zn-loop` |
| 6 | 在 `zn-host` 导出更细分的宿主技能文件 | `zn-host` |
| 7 | 在稳定后再新增 `zn-sdk` crate | workspace |

## 验证标准

本阶段完成后，至少应满足四个可见结果。第一，用户可以在终端或宿主中经历**不止一轮**需求澄清；第二，澄清结果会稳定写入 OpenSpec 风格工件，而不是只出现在执行摘要里；第三，实现任务开始前，系统能够真实创建或记录独立工作区；第四，后续新增 SDK 时，不需要把 CLI 逻辑再拆一次。

| 验证项 | 合格标准 |
| --- | --- |
| Brainstorming | 至少支持启动、提问、回答、恢复、收敛 |
| OpenSpec 写入 | `proposal.md`、`requirements.md`、`acceptance.md`、`clarifications.md` 均可追溯 |
| Worktree 执行 | 能真实创建 worktree 或明确失败原因 |
| Finish-branch | 能输出 merge / pr / discard / keep 的标准结果 |
| SDK 预备度 | 核心函数可被 CLI 以外调用 |

## 结论

接下来的正确推进方式，不是再增加更多“看起来很全”的模板，而是**让每个阶段具备最小但真实的执行能力**。因此，本轮我会按这个设计先落地：**交互式 Brainstorming 的会话模型与 CLI 入口**，随后紧接 **Git 工作区与 finish-branch 的真实执行接口**，再继续推进 **子代理桥接协议** 与 **SDK 门面抽象**。这条路线既符合你提出的优先级，也能最大限度减少未来返工。
