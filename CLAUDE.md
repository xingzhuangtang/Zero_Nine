# CLAUDE.md — Zero_Nine

## Core Design Philosophy / 核心设计理念

**Zero_Nine is built on Harness Engineering and Environment Engineering principles.**
**Zero_Nine 基于 Harness Engineering（驾驭工程）和 Environment Engineering（环境工程）原则构建。**

### The Future of AI Agents: Environment as the Foundation
### 未来智能体：环境为本

**Zero_Nine's Vision for Agent Development / Zero_Nine 的智能体发展愿景**

我们认为未来的智能体需要某种意义的东西来托举 —— 那是**环境工程**：

> 它不仅是技术组件，更是行为塑造的媒介
> 它不仅是隔离运行，更是演化学习的土壤
> 它不仅是工程实现，更是智能可靠性的根本保障

这是一种**"生态思维"** —— 好的园丁不直接塑造每一片叶子，而是调配好土壤、光照、水分，让植物自然生长。

**Zero_Nine is that environment / Zero_Nine 就是这样的环境：**
- **土壤 (Soil)**: 结构化上下文、规范工件、技能库
- **光照 (Light)**: 奖励信号、反馈回路、置信度追踪
- **水分 (Water)**: 课程学习、信念更新、演化候选

---

### Harness Engineering / 驾驭工程

Zero_Nine treats AI agents as the "model" — the intelligence that generates code. The project's purpose is to build the **harness** around that model:

- **Constraints & Guardrails**: DAG scheduling, verification gates, review verdicts, evidence collection
- **Feedback Loops**: Multi-dimensional reward signals, pairwise comparisons, user feedback integration
- **Recovery Mechanisms**: Subagent recovery ledgers, retry budgets, escalation protocols
- **Observability**: Event logs, iteration tracking, state transitions, artifact persistence

> **Key Insight**: The intelligence is not in the code we write — it's in the environment we design to channel AI behavior reliably.
> **核心洞察**：智能不在于我们编写的代码——而在于我们设计的环境，用于可靠地引导 AI 行为。

### Environment Engineering / 环境工程

Zero_Nine designs the **environment** in which AI agents operate, not the agents themselves:

- **Structured Context**: Context protocols, subagent dispatch bundles, specification artifacts
- **Execution Sandboxes**: Git worktree isolation, workspace preparation, file operation tracking
- **Verification Infrastructure**: Automated review, evidence-driven validation, deliverable checking
- **Evolution Ecosystem**: Skill scoring, curriculum learning, belief state tracking, reward learning

> **Key Insight**: Build the world the agent lives in, not the agent. The environment shapes behavior more than instructions.
> **核心洞察**：构建代理生活的世界，而不是代理本身。环境对行为的塑造作用超过指令。

### Design Principles / 设计原则

1. **Steering > Control**: Design constraints that guide rather than restrict
2. **Observability First**: Every state transition, decision, and outcome must be traceable
3. **Recovery by Design**: Assume failures will happen; build replay and resume capabilities
4. **Feedback-Driven Evolution**: All execution generates signals for continuous improvement
5. **Plugin Architecture**: Any AI agent/client can be integrated via configurable adapters

---

## Quick Start / 快速开始

```bash
# Build / 构建
cargo build

# Run tests / 运行测试
cargo test --all-targets

# Lint / 代码检查
cargo clippy

# Reinstall CLI binary (after modifying crates) / 重新安装 CLI
cargo install --path crates/zn-cli

# CLI usage / CLI 使用
zero-nine run --project . --host claude-code --goal "your goal"
```

## Project Overview / 项目概述

**Zero_Nine** is a Rust orchestration kernel unifying 4 upstream projects:
**Zero_Nine** 是一个 Rust 编排内核，统一 4 个上游项目：

| Layer / 层 | Crate | Purpose / 用途 |
|-----------|-------|---------------|
| Types / 类型 | `zn-types` | Shared data models / 共享数据模型 |
| Spec / 规格 | `zn-spec` | Proposal/artifact management / 提案/工件管理 |
| Exec / 执行 | `zn-exec` | Execution plans + verification / 执行计划 + 验证 |
| Loop / 循环 | `zn-loop` | Scheduler + recovery / 调度器 + 恢复 |
| Evolve / 进化 | `zn-evolve` | Skill scoring + candidates / 技能评分 + 候选 |
| Host / 宿主 | `zn-host` | Claude/OpenCode adapters / 适配器 |
| CLI / 命令行 | `zn-cli` | `zero-nine` binary / 二进制入口 |
| Bridge / 桥接 | `zn-bridge` | gRPC + proto + type conversion / 类型转换层 |

## Claude Code Integration / Claude Code 集成

### Slash Command / 斜杠命令

Use `/zero-nine <goal>` in Claude Code.
在 Claude Code 中使用 `/zero-nine <goal>`。

Adapter location / 适配器位置：
```
adapters/claude-code/.claude/commands/zero-nine.md
```

### Skill / 技能

Skill location / 技能位置：
```
adapters/claude-code/.claude/skills/zero-nine-orchestrator/SKILL.md
```

### Turn-Based Brainstorming / 回合制头脑风暴

**Important**: Brainstorming requires multiple turns. Keep using `/zero-nine <answer>` until verdict=Ready.
**重要**：头脑风暴需要多轮对话。持续使用 `/zero-nine <答案>` 直到裁决=就绪。

## Key Commands / 核心命令

| Command / 命令 | Description / 描述 |
|---------|-------------|
| `zero-nine init --project . --host claude-code` | Initialize project / 初始化项目 |
| `zero-nine run --project . --host claude-code --goal "..."` | Run workflow / 运行工作流 |
| `zero-nine status --project .` | Check status / 检查状态 |
| `zero-nine resume --project . --host claude-code` | Resume after interrupt / 中断后恢复 |
| `zero-nine export --project .` | Export adapters / 导出适配器 |

## Runtime Directory / 运行时目录

```
.zero_nine/
├── manifest.json           # Project config / 项目配置
├── proposals/<id>/         # Proposals / 提案
├── brainstorm/             # Sessions / 会话
├── loop/session-state.json # Scheduler state / 调度状态
├── runtime/events.ndjson   # Event log / 事件日志
├── evolve/                 # Evaluations / 评估
└── specs/                  # Knowledge patterns / 知识模式
```

## Four-Layer Flow / 四层工作流

1. **Brainstorming** → Clarify requirements / 澄清需求
2. **Spec Capture** → Create proposal + design / 创建提案 + 设计
3. **Execution** → Run tasks + verify / 执行任务 + 验证
4. **Evolution** → Score + generate candidates / 评分 + 生成候选

## Critical Conventions / 关键约定

1. **Turn-based brainstorming**: Answer questions until Ready / 回合制头脑风暴：回答问题直到就绪
2. **DAG scheduling**: Tasks need all deps completed / DAG 调度：任务需所有依赖完成
3. **Parallel limit**: Max 2 concurrent tasks / 并行限制：最多 2 个并发任务
4. **Finish branch confirmation**: Requires `confirm_remote_finish=true` / 完成分支确认：需要 `confirm_remote_finish=true`
5. **NDJSON events**: Append-only log / NDJSON 事件：仅追加日志

## Testing / 测试

```bash
# Run all tests / 运行所有测试
cargo test --all-targets

# Test counts / 测试数量 (107 total)
# - zn-types: 16 tests
# - zn-exec: 31 tests
# - zn-evolve: 21 tests
# - zn-spec: 3 tests
# - zn-host: 10 tests
# - zn-cli: 2 tests
# - zn-bridge: 5 tests
# - zn-loop: 22 tests (incl. integration)
```

## Code Quality / 代码质量

- **`cargo clippy --all-targets`**: 零新增警告策略 — 修改代码不应引入新 clippy 警告
- **Default `rustfmt`**: 无自定义 `.rustfmt.toml`，使用 Rust 默认格式规范

## Behavioral Guidelines / 行为准则

> Inspired by [andrej-karpathy-skills](https://github.com/forrestchang/andrej-karpathy-skills)

Behavioral guidelines to reduce common LLM coding mistakes.

**Tradeoff**: These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them — don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it — don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if**: fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.

## Files to Read First / 优先阅读

1. `README.md` — Overview / 概述
2. `docs/architecture.md` — Architecture / 架构
3. `crates/zn-types/src/lib.rs` — Data models / 数据模型
4. `crates/zn-loop/src/lib.rs` — Scheduler / 调度器
5. `crates/zn-exec/src/lib.rs` — Execution / 执行
6. `crates/zn-bridge/src/types.rs` — gRPC type conversion / 类型转换
7. `AGENTS.md` — Detailed project guide / 详细项目指南
