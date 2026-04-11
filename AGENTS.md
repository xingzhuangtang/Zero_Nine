# AGENTS.md — Zero_Nine

## Project Essence / 项目本质

**Zero_Nine** is a Rust orchestration kernel that unifies four upstream projects into one CLI:
**Zero_Nine** 是一个 Rust 编排内核，将四个上游项目统一为一个 CLI：

- **OpenSpec** → `zn-spec`: proposal/design/tasks artifacts
- **OpenSpec** → `zn-spec`: 提案/设计/任务工件管理
- **Superpowers** → `zn-exec`: structured execution + verification
- **Superpowers** → `zn-exec`: 结构化执行 + 验证
- **Ralph-loop** → `zn-loop`: long-running scheduler with recovery
- **Ralph-loop** → `zn-loop`: 长时调度循环 + 恢复
- **OpenSpace** → `zn-evolve`: skill scoring + evolution candidates
- **OpenSpace** → `zn-evolve`: 技能评分 + 演化候选

Single user entry: `/zero-nine <goal>` via Claude Code or OpenCode.
单一用户入口：`/zero-nine <goal>` 通过 Claude Code 或 OpenCode。

## Workspace Structure / 工作区结构

```
Zero_Nine/
├── Cargo.toml                 # workspace root
├── crates/
│   ├── zn-types/              # shared types (1290+ lines)
│   ├── zn-spec/               # proposal/artifact management
│   ├── zn-exec/               # execution plans, workspaces, verification
│   ├── zn-loop/               # scheduler, retries, recovery
│   ├── zn-evolve/             # scoring, candidates
│   ├── zn-host/               # Claude/OpenCode adapters
│   └── zn-cli/                # CLI entry (zero-nine binary)
├── adapters/                  # host adapter templates
├── demo_project/              # test project with .zero_nine/ runtime
└── docs/                      # architecture + usage docs
```

## Critical Commands / 核心命令

```bash
# Build / 构建
cargo build

# Test (6 tests total) / 测试（共 6 个测试）
cargo test --all-targets

# Lint / 检查
cargo clippy

# CLI help / CLI 帮助
cargo run -p zn-cli -- --help

# Run a goal / 运行目标
zero-nine run --project . --host opencode --goal "your goal"

# Resume after interrupt / 中断后恢复
zero-nine resume --project . --host opencode

# Status check / 状态检查
zero-nine status --project .

# Export adapters / 导出适配器
zero-nine export --project .
```

## CLI Commands / CLI 命令

| Command / 命令 | Purpose / 用途 |
|---------|---------|
| `init` | Create `.zero_nine/` + manifest / 创建 `.zero_nine/` + 项目清单 |
| `brainstorm` | Clarify requirements (host-native turn-based) / 澄清需求（宿主原生回合制） |
| `run` | Execute full 4-layer workflow / 执行完整四层工作流 |
| `status` | Show proposal + loop state / 显示提案 + 循环状态 |
| `resume` | Continue from last state / 从最后状态继续 |
| `export` | Write adapter command/skill files / 写入适配器命令/技能文件 |

## Runtime Directory (.zero_nine/) / 运行时目录

```
.zero_nine/
├── manifest.json              # project config / 项目配置
├── proposals/<id>/            # proposal artifacts / 提案工件
│   ├── proposal.json, design.md, tasks.md, dag.json
│   └── progress.json, verification.md
├── brainstorm/                # clarification sessions / 澄清会话
├── loop/session-state.json    # scheduler state / 调度器状态
├── runtime/events.ndjson      # event log / 事件日志
├── evolve/                    # evaluations + candidates / 评估 + 候选
└── specs/                     # knowledge patterns / 知识模式
```

## Four-Layer Flow / 四层工作流

1. **Brainstorming** → generate clarification questions, collect answers, verdict=Ready
   **头脑风暴** → 生成澄清问题，收集答案，裁决=就绪
2. **Spec Capture** → create proposal, requirements.md, acceptance.md, design.md, tasks.md + dag.json
   **规格捕获** → 创建提案、需求文档、验收标准、设计文档、任务清单 + DAG
3. **Execution** → DAG scheduler, Git worktree prep, plan execution, verification (tests + review)
   **执行** → DAG 调度、Git worktree 准备、计划执行、验证（测试 + 审查）
4. **Evolution** → score execution (0.33-0.97), generate evolution candidates
   **进化** → 评分执行结果 (0.33-0.97)，生成演化候选

## Key Data Models (`zn-types`) / 关键数据模型

- `HostKind`: ClaudeCode | OpenCode | Terminal
- `ProposalStatus`: Draft → Ready → Running → Completed → Archived
  提案状态：草稿 → 就绪 → 运行中 → 已完成 → 已归档
- `TaskStatus`: Pending → Running → Completed → Failed → Blocked
  任务状态：待处理 → 运行中 → 已完成 → 失败 → 已阻塞
- `LoopStage`: Idle → SpecDrafting → Ready → RunningTask → Verifying → Retrying → Escalated → Archived
  循环阶段：空闲 → 规格起草 → 就绪 → 任务执行 → 验证 → 重试 → 升级 → 已归档
- `BrainstormSession`: questions[], answers[], verdict
  头脑风暴会话：问题列表，答案列表，裁决
- `ExecutionReport`: success, tests_passed, review_passed, evidence[]
  执行报告：成功，测试通过，审查通过，证据列表
- `EvolutionCandidate`: source_skill, kind (AutoFix/AutoImprove/AutoLearn), patch, confidence
  演化候选：源技能，类型（自动修复/自动优化/自动学习），补丁，置信度

## Execution Modes / 执行模式

| Task Type / 任务类型 | Mode / 模式 | Skill Chain / 技能链 |
|-----------|------|-------------|
| Brainstorming / 头脑风暴 | Brainstorming | brainstorming, spec-capture |
| Spec Capture / 规格捕获 | SpecCapture | writing-plans, design-review |
| Planning / 规划 | WritingPlans | writing-plans, using-git-worktrees |
| Implementation / 实现 | TDD Cycle | test-driven-development, requesting-code-review |
| Verification / 验证 | Verification | verification-before-completion |
| Finish Branch / 完成分支 | FinishBranch | finishing-a-development-branch |

## Workspace Strategy / 工作空间策略

- **GitWorktree** (default): isolated worktrees per task, max 2 slots
  **GitWorktree**（默认）：每个任务独立 worktree，最多 2 个槽位
- **In-Place** (fallback): if no initial commit exists
  **In-Place**（降级）：如果没有初始提交
- **Sandboxed**: reserved for future use
  **Sandboxed**：预留给未来使用

## Verification Gates / 验证门控

| Gate / 门控 | Command / 命令 | Required / 必需 |
|------|---------|----------|
| tests / 测试 | Auto-detect: cargo test / pytest / npm test | Yes / 是 |
| review / 审查 | git diff --stat && git diff --check | Yes / 是 |

## Retry Policy / 重试策略

```
Failed → Retrying → Running (max 2 retries) → Escalated → generate evolution candidate
失败 → 重试中 → 运行中（最多 2 次） → 升级 → 生成演化候选
```

## Host Integration / 宿主集成

### Claude Code
- Adapter: `adapters/claude-code/.claude/commands/zero-nine.md`
- Skill: `adapters/claude-code/.claude/skills/zero-nine-orchestrator/SKILL.md`
- Usage: `/zero-nine <goal>` then answer clarification questions with same command
  用法：`/zero-nine <goal>` 然后用同一命令回答澄清问题

### OpenCode
- Adapter: `adapters/opencode/.opencode/commands/zero-nine.md`
- Skill: `.opencode/skills/zero-nine-orchestrator/SKILL.md` (also in `~/.config/opencode/skills/`)
  技能：`.opencode/skills/zero-nine-orchestrator/SKILL.md`（也在 `~/.config/opencode/skills/`）
- Usage: `/zero-nine <goal>` with `$ARGUMENTS` passed to Rust CLI
  用法：`/zero-nine <goal>` 通过 `$ARGUMENTS` 传递给 Rust CLI

## Critical Conventions / 关键约定

1. **Brainstorming is turn-based**: Keep invoking `/zero-nine <answer>` until verdict=Ready
   **头脑风暴是回合制的**：持续调用 `/zero-nine <答案>` 直到裁决=就绪
2. **DAG scheduling**: Tasks execute only when all dependencies are Completed
   **DAG 调度**：任务仅当所有依赖完成后才执行
3. **Parallel limit**: Max 2 concurrent tasks (1 if finish_branch pending)
   **并行限制**：最多 2 个并发任务（如果有 finish_branch 待处理则为 1 个）
4. **Finish branch requires confirmation**: `confirm_remote_finish=true` for merge/PR actions
   **完成分支需要确认**：合并/PR 操作需要 `confirm_remote_finish=true`
5. **Events are NDJSON**: Append-only in `runtime/events.ndjson` for audit + learning
   **事件是 NDJSON**：仅追加写入 `runtime/events.ndjson` 用于审计 + 学习

## Testing / 测试

- `zn-exec`: 3 tests (planning, implementation, verification)
- `zn-spec`: 3 tests (policy, skill library)
- `zn-types`: 16 tests (blueprint coverage)
- No integration tests yet / 暂无集成测试

## Known Gaps / Next Phase / 已知缺口 / 下一阶段

1. `zn-exec` is scaffold — needs real host/agent bridging
   `zn-exec` 是骨架 — 需要真正的宿主/代理桥接
2. Claude Code plugin manifest incomplete
   Claude Code 插件清单不完整
3. Evolution is local-only — no cloud sync or version comparison
   进化是本地-only — 没有云同步或版本比较
4. No snapshot testing for artifacts
   没有工件的快照测试
5. No CI/CD pipeline
   没有 CI/CD 流水线

## Files to Read First / 优先阅读的文件

1. `README.md` — project overview / 项目概述
2. `docs/architecture.md` — full design rationale / 完整设计原理
3. `crates/zn-types/src/lib.rs` — all data models / 所有数据模型
4. `crates/zn-loop/src/lib.rs` — scheduler + run_goal / 调度器 + 目标运行
5. `crates/zn-exec/src/lib.rs` — execution plans + verification / 执行计划 + 验证
