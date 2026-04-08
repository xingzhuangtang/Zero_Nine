# Zero_Nine 缺口分析

## 目标链路

目标链路为：**Superpowers Brainstorming（苏格拉底式需求澄清）→ OpenSpec 自动写入 proposal/design/tasks/DAG/progress → Ralph-loop fresh-context 调度 → Superpowers writing-plans 二次科学拆分 → 隔离沙盒与 worktree 新分支执行 → 子代理开发与审查 → TDD 与验证 → finishing-a-development-branch 标准化收尾 → OpenSpec 进度更新 → OpenSpace 实时监控、自动优化、自动注入技能**。

## 当前已具备的基础

| 组件 | 当前状态 | 说明 |
| --- | --- | --- |
| `zn-cli` | 基础可用 | 仅有 `init/run/status/resume/export` 五个命令 |
| `zn-host` | 基础可用 | 能导出 Claude Code / OpenCode 的最小命令与技能包装 |
| `zn-spec` | 基础可用 | 能创建 proposal、design、tasks、progress、verification 等工件 |
| `zn-loop` | 基础可用 | 能按顺序迭代任务、写入状态、记录事件与演化候选 |
| `zn-exec` | 初步增强 | 已具备 brainstorming / planning / implementation / verification 分类与结构化工件输出 |
| `zn-evolve` | 占位实现 | 仅根据执行报告生成粗粒度评分和候选改进 |

## 核心缺口

| 目标能力 | 当前状态 | 缺口说明 |
| --- | --- | --- |
| Superpowers 原生 Brainstorming | 缺失 | 当前只有 `TaskKind::Brainstorming` 的结构化模板，没有苏格拉底式问答、澄清轮次、需求压缩与自动归档策略 |
| OpenSpec 自动写入正式规格流 | 部分具备 | 已写 proposal/design/tasks/progress，但尚未形成“brainstorming 产物 → OpenSpec 正式规格字段”的强绑定数据流 |
| Ralph-loop fresh-context / session 隔离 | 缺失 | 当前是本地顺序循环，没有真实 session 重启、上下文刷新、恢复点重入机制 |
| writing-plans 二次拆分 | 缺失 | 当前 planning 只生成路线文档，没有从 `tasks.md` 再细化为迭代级执行计划 |
| 隔离沙盒与 worktree 新分支 | 缺失 | 当前没有 Git worktree、隔离目录、分支命名策略或临时环境生命周期管理 |
| 子代理开发与审查 | 缺失 | 当前没有 subagent 分工、开发代理/审查代理输出协议、聚合裁决机制 |
| TDD 循环与质量门禁 | 缺失 | 当前只有布尔字段 `tests_passed/review_passed`，没有真实测试计划、失败回路、门禁策略 |
| finishing-a-development-branch | 缺失 | 当前没有标准化收尾动作、合并/PR/放弃选项与临时环境清理 |
| OpenSpec 进度联动 | 部分具备 | 已更新 `progress.json`，但尚未形成 `progress.txt` / 任务说明 / 变更摘要三件套 |
| OpenSpace 实时监控与自动注入 | 缺失 | 当前 `zn-evolve` 只是执行后打分，不是后台监控、自动修复、自动派发到所有层 |
| 宿主插件原生体验 | 较弱 | Claude/OpenCode 目前只是薄包装命令，不是带多命令、多技能、多阶段提示的原生插件体验 |
| 独立 CLI/SDK 演进接口 | 缺失 | 当前只有 CLI，没有 Rust SDK 模块化 API、对外库接口和嵌入式调用示例 |

## 优先级建议

### 第一优先级

先打通**Brainstorming → OpenSpec 规格工件 → Loop 调度 → Writing-plans 细化**这条主链，因为这决定 Zero_Nine 是否真正具备从一句话需求到可执行工件的能力。

### 第二优先级

补上**worktree / 分支隔离 / TDD 门禁 / review / finishing**，因为这部分决定 Zero_Nine 是否像 Superpowers 一样具备真正工程执行约束，而不是仅做文档编排。

### 第三优先级

把 `zn-host` 从“命令包装器”升级为“宿主原生适配层”，同时为未来独立 CLI/SDK 预留 `zn-sdk` 或公共库接口。

### 第四优先级

重构 `zn-evolve`，让其从“事后评分”升级为“持续监控、候选补丁、自动注入、经验沉淀”的背景层。
