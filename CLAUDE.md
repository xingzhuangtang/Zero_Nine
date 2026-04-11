# CLAUDE.md — Zero_Nine

## Quick Start / 快速开始

```bash
# Build / 构建
cargo build

# Run tests / 运行测试
cargo test --all-targets

# Lint / 代码检查
cargo clippy

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

# Test counts / 测试数量
# - zn-types: 16 tests
# - zn-exec: 3 tests
# - zn-spec: 3 tests
```

## Files to Read First / 优先阅读

1. `README.md` — Overview / 概述
2. `docs/architecture.md` — Architecture / 架构
3. `crates/zn-types/src/lib.rs` — Data models / 数据模型
4. `crates/zn-loop/src/lib.rs` — Scheduler / 调度器
5. `crates/zn-exec/src/lib.rs` — Execution / 执行
