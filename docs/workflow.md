# Zero_Nine 自动化工作流

**版本**: v1.1.0 | **日期**: 2026-04-19

---

## 一、总览

Zero_Nine 是一个基于 Rust 的 AI 编排内核，整合 OpenSpec、Superpowers、Ralph-loop 和 OpenSpace 四个上游项目，以 **Harness Engineering** 和 **Environment Engineering** 为设计原则，构建可落地的 AI 代理工作环境。

### 核心设计思想

> **不是构建更好的 AI 代理，而是构建更好的环境让 AI 代理在其中可靠工作。**
> 就像好的园丁不直接塑造每一片叶子，而是调配好土壤、光照、水分，让植物自然生长。

| 维度 | 含义 | 实现 |
|------|------|------|
| **土壤** | 结构化上下文、规范工件、技能库 | zn-spec, zn-types |
| **光照** | 奖励信号、反馈回路、置信度追踪 | zn-evolve (reward/belief) |
| **水分** | 课程学习、信念更新、演化候选 | zn-evolve (curriculum) |
| **围栏** | DAG 调度、验证关卡、治理权限 | zn-exec, zn-loop |
| **修剪** | 技能蒸馏、成对比较、信念更新 | zn-evolve (distiller/scorer) |

---

## 二、四层工作流（用户视角）

一个任务从需求到完成的完整生命周期：

```
用户输入: "加个搜索功能"
    │
    ▼
┌─────────────────────────────────────────────────┐
│ Layer 1: Brainstorming  ── 需求澄清               │
│   • 苏格拉底式提问                                │
│   • 回合制问答（不预设轮数）                       │
│   • 直到 verdict=Ready                           │
└─────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────┐
│ Layer 2: Spec Capture ── 规格捕获                │
│   • proposal.md  ── 需求提案                     │
│   • design.md    ── 设计方案                     │
│   • tasks.md     ── 任务列表                     │
│   • dag.json     ── 依赖图                       │
│   • acceptance.md ── 验收标准                    │
│   • spec-validation.json ── 规格验证报告          │
└─────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────┐
│ Layer 3: Execution ── 任务执行                   │
│   • DAG 依赖调度（已完成 → 可执行）               │
│   • Git worktree 隔离执行                        │
│   • Build / Test / Lint 质量关卡                 │
│   • 子代理调度（并行窗口 ≤ 2）                    │
│   • 重试预算（max_retries）                      │
└─────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────┐
│ Layer 4: Evolution ── 进化优化                   │
│   • 多维度奖励评分 (5 维度)                       │
│   • 贝叶斯信念更新 (置信度追踪)                   │
│   • ELO 课程学习 (动态难度调整)                   │
│   • 三系统融合决策 (Reward + Belief + Curriculum)│
│   • 技能蒸馏 (从执行记录提取模式)                 │
│   • 演化候选生成                                │
└─────────────────────────────────────────────────┘
    │
    ▼
代码提交 / PR 创建 / 验证归档
```

### 状态机

```
Idle → Brainstorming → SpecDrafting → Ready → RunningTask → Verifying → Archived
                                        ↓
                                  Retrying (重试)
                                        ↓
                                  Escalated (需人工介入)
```

---

## 三、十三层架构（实现视角）

支撑四层工作流的完整功能模块：

| Layer | 名称 | 功能 | 模块 |
|-------|------|------|------|
| 1 | Goal Intake | 接收并解析用户目标 | zn-cli, zn-loop |
| 2 | Context Assembly | 组装项目上下文 | zn-exec, zn-spec |
| 3 | Policy Injection | 注入策略约束 | zn-exec/governance |
| 4 | Skill Routing | 路由到技能处理器 | zn-evolve |
| 5 | Memory Integration | 集成记忆系统 | zn-spec/memory_tool |
| 6 | Subagent Dispatch | 调度子代理并行执行 | zn-exec/subagent_dispatcher |
| 7 | TUI Dashboard | 交互式可视化界面 | zn-cli/dashboard |
| 8 | Governance | 权限和审批控制 | zn-exec/governance |
| 9 | Evidence Collection | 收集执行证据 | zn-exec, zn-loop |
| 10 | Verification Gates | Build/Test/Lint 检查 | zn-exec, zn-loop |
| 11 | Branch Management | Git worktree / PR | zn-exec |
| 12 | Skill Distiller | 从执行记录蒸馏技能 | zn-evolve/distiller |
| 13 | Observability | 追踪、指标、日志 | zn-exec/observability |

---

## 四、模块职责与依赖

```
┌──────────────────────────────────────────────────────┐
│                    CLI (zero-nine)                     │
│  init / run / brainstorm / status / resume / export    │
└────────────────────────────────┬─────────────────────┘
                                 │
              ┌──────────────────▼──────────────────┐
              │              zn-loop                 │
              │  循环驱动 / 状态推进 / 事件写入       │
              │  DAG 调度 / 批次执行 / 重试管理       │
              └──┬──────┬──────┬──────┬──────────┬──┘
                 │      │      │      │          │
        ┌────────▼─┐ ┌──▼───┐ │ ┌───▼───┐ ┌────▼────┐
        │ zn-spec  │ │zn-exec│ │ │zn-evolve│ │ zn-host │
        │ 工件管理  │ │执行策略│ │ │ 技能演化 │ │宿主适配 │
        └──────────┘ └───────┘ │ └────┬───┘ └─────────┘
                               │      │
                    ┌──────────▼──────▼──────────┐
                    │        zn-types             │
                    │  统一数据模型 (共享类型)      │
                    └────────────┬───────────────┘
                                 │
                    ┌────────────▼───────────────┐
                    │        zn-bridge            │
                    │  gRPC 子代理通信 + MCP 集成  │
                    └────────────────────────────┘
```

### 各模块详细说明

| Crate | 文件 | 职责 |
|-------|------|------|
| **zn-types** | `crates/zn-types/src/lib.rs` | 统一数据模型：TaskItem, Proposal, ExecutionReport, BeliefState, RewardState, CurriculumState 等 |
| **zn-spec** | `crates/zn-spec/src/lib.rs` | 工件管理：proposal 读写、manifest、循环状态、事件日志、记忆系统、会话搜索 |
| **zn-exec** | `crates/zn-exec/src/lib.rs` | 执行引擎：执行计划、工作空间准备、安全命令执行、子代理调度、治理策略、Token 预算 |
| **zn-loop** | `crates/zn-loop/src/lib.rs` | 循环驱动：Brainstorming → Spec → Execution → Evolution 全流程编排 |
| **zn-evolve** | `crates/zn-evolve/src/lib.rs` | 进化引擎：奖励模型、信念系统、课程学习、技能蒸馏、AI 客户端、三系统融合 |
| **zn-host** | `crates/zn-host/src/lib.rs` | 宿主适配：Claude Code / OpenCode 适配文件生成 |
| **zn-cli** | `crates/zn-cli/src/main.rs` | CLI 入口：zero-nine 二进制文件 |
| **zn-bridge** | `crates/zn-bridge/src/lib.rs` | gRPC 桥接：子代理通信和 MCP 集成 |

---

## 五、完整执行流程详解

### 阶段 1: 项目初始化 (`init`)

```bash
zero-nine init --project . --host claude-code
```

**执行步骤**：

1. **创建目录结构**
   ```
   .zero_nine/
   ├── manifest.json              # 项目配置（host, policy）
   ├── proposals/                 # 提案工件
   ├── brainstorm/                # 头脑风暴会话
   ├── runtime/                   # 运行时事件
   ├── loop/                      # 循环状态
   ├── evolve/                    # 演化候选
   └── specs/                     # 知识模式
   ```

2. **写入 manifest.json**
   ```json
   {
     "default_host": "claude-code",
     "policy": { "max_retries": 3 }
   }
   ```

3. **导出宿主适配文件** (`export`)
   ```
   adapters/claude-code/.claude/
   ├── commands/zero-nine.md          # /zero-nine 斜杠命令
   └── skills/zero-nine-orchestrator/ # 编排器技能
       └── SKILL.md
   ```

### 阶段 2: 需求澄清 (`Brainstorming`)

```bash
# 方式 1: CLI 终端模式
zero-nine brainstorm --goal "添加搜索功能"

# 方式 2: 宿主集成（Claude Code）
/zero-nine 添加搜索功能
```

**流程**：

1. **启动会话** → 生成 session_id，写入 `.zero_nine/brainstorm/`
2. **生成澄清问题** → 基于 goal 生成苏格拉底式提问
3. **回合制问答** → 用户逐轮回答，系统记录答案
4. **判定收敛** → 当 `verdict == Ready` 且无未回答问题时
5. **生成提案** → 创建 proposal.md + design.md + tasks.md + dag.json

**数据结构**：

```rust
BrainstormSession {
    id: "bs-xxx",
    goal: "添加搜索功能",
    host: ClaudeCode,
    verdict: Ready,
    questions: [
        ClarificationQuestion { id, question, rationale },
        ...
    ],
    clarifications: [
        ClarificationAnswer { question_id, answer, timestamp },
        ...
    ]
}
```

### 阶段 3: 规格捕获 (`Spec Capture`)

当 Brainstorming 收敛为 `Ready` 后自动触发：

```
.zero_nine/proposals/<id>/
├── proposal.md          # 需求提案
├── design.md            # 设计方案
├── tasks.md             # 任务列表（含依赖关系）
├── dag.json             # 依赖图（JSON 格式）
├── acceptance.md        # 验收标准
└── spec-validation.json # 规格验证报告
```

**验证规则**：
- 所有任务必须有 title 和 kind
- DAG 无循环依赖
- 所有依赖引用的 task_id 存在
- acceptance.md 存在

### 阶段 4: 任务执行 (`Execution`)

**4.1 DAG 调度**

```rust
// 每轮选择可执行任务的规则：
1. 所有依赖已完成 (TaskStatus::Completed)
2. 状态为 Pending / Running / Failed
3. 重试次数 < max_retries
4. 并行窗口 ≤ 2（最多 2 个并发任务）
5. worktree slots ≤ 2, finish_branch slots ≤ 1
6. 优先选择: 失败重试 > finish_branch > verification > execution
```

**4.2 执行包装 (Execution Envelope)**

```rust
ExecutionEnvelope {
    execution_mode: "guided",           // guided / autonomous
    workspace_strategy: "worktree",     // worktree / in-place
    quality_gates: ["build", "test"],
    context_protocol: ContextInjectionProtocol,
    finish_branch_automation: BranchFinishAutomation,
}
```

**4.3 工作空间准备**

```
准备 git worktree → 隔离执行环境 → 记录 WorkspaceRecord
```

**4.4 任务执行**

```
构建执行计划 → 准备 worktree → 安全命令执行
  → 代码审查 → 质量验证 → finish-branch → 生成报告
```

**4.5 执行结果处理**

| 结果 | 动作 |
|------|------|
| `Completed` | 标记完成，进入下一批次 |
| `RetryableFailure` (重试 < max_retries) | 标记为 Pending，加入下一批次重试 |
| `RetryableFailure` (重试 >= max_retries) | 标记为 Failed，暂停执行 |
| `Blocked` | 标记为 Blocked，需人工介入 |
| `Escalated` | 标记为 Failed，需人工介入 |

### 阶段 5: 进化优化 (`Evolution`)

每个任务完成后，`zn-loop` 中的 `execute_proposal()` 函数自动触发进化引擎：

```rust
// zn-loop/src/lib.rs — 任务完成后的进化触发点 (L701-L728)

// 1. 基础评估评分
let evaluation = evaluate(&report);

// 2. 写入评估记录
writeln!(evals, "{}", serde_json::to_string(&evaluation)?);

// 3. 更新奖励模型
let mut reward_model = RewardModel::new(
    project_root.join(".zero_nine/evolve/pairwise_comparisons.ndjson")
)?;
reward_model.record_from_report(&report);
reward_model.save()?;

// 4. 生成演化候选
if let Some(candidate) = propose_candidate(&report) {
    fs::write(path, serde_json::to_string_pretty(&candidate)?)?;
}
```

完整的数据流如下：

```
                    ExecutionReport (任务执行结果)
                           │
              ┌────────────┼────────────────────────────┐
              │            │                            │
              ▼            ▼                            ▼
    ┌──────────────┐ ┌──────────────┐        ┌───────────────────┐
    │  evaluate()  │ │Reward Model  │        │ propose_candidate │
    │  基础评分     │ │ 奖励学习     │        │ 演化候选生成       │
    └──────┬───────┘ └──────┬───────┘        └────────┬──────────┘
           │                │                         │
           ▼                ▼                         ▼
    SkillEvaluation   RewardBreakdown           EvolutionCandidate
    (0.33~0.97分)    (5维度评分 + EMA)          (AutoImprove/AutoFix)
           │                │                         │
           │         写入 NDJSON                写入 JSON 文件
           │                │
           │                ▼
           │         Integration Engine (完整联动)
           │                │
           ▼                ▼
    evaluations.jsonl  三系统决策 + 技能蒸馏 + 课程学习
```

---

#### 5.1 基础评估 (`evaluate()`)

**文件**: `zn-evolve/src/lib.rs:35-94`

根据执行结果的结构化特征给出 0.33~0.97 的评分：

| 条件 | 分数 |
|------|------|
| 完成 + 成功 + 审查通过 + 验证通过 + 证据齐全 | 0.97 |
| 完成 + 成功（但证据/验证不全） | 0.84 |
| 完成（但不完全成功） | 0.61 |
| 可重试失败 | 0.56 |
| 被阻塞 | 0.42 |
| 已升级（需人工） | 0.33 |

```rust
pub fn evaluate(report: &ExecutionReport) -> SkillEvaluation {
    let score = match report.outcome {
        ExecutionOutcome::Completed if report.success
            && review_passed && verification_passed
            && missing_required == 0 => 0.97,
        ExecutionOutcome::Completed if report.success => 0.84,
        ExecutionOutcome::Completed => 0.61,
        ExecutionOutcome::RetryableFailure => 0.56,
        ExecutionOutcome::Blocked => 0.42,
        ExecutionOutcome::Escalated => 0.33,
    };
    SkillEvaluation { skill_name, task_type, score, notes, ... }
}
```

评分同时产出技能名称标记：
- 测试通过 + 证据齐全 → `"guarded-execution"`
- 否则 → `"evidence-driven-verification"`

---

#### 5.2 多维度奖励模型 (`RewardModel`)

**文件**: `zn-evolve/src/reward.rs`
**持久化**: `.zero_nine/evolve/pairwise_comparisons.ndjson`

**五个评分维度**：

| 维度 | 归一化公式 | 含义 |
|------|-----------|------|
| `code_quality` | 直接来自 report.code_quality_score | 代码质量评分 |
| `test_coverage` | 直接来自 report.test_coverage | 测试覆盖率 |
| `execution_speed` | `(1.0 - time_ms/10000.0).max(0.0)` | < 10s 为好 |
| `token_efficiency` | `(1.0 - tokens/10000.0).max(0.0)` | < 10k tokens 为好 |
| `user_satisfaction` | `rating / 5.0`（来自用户反馈） | 1-5 分归一化 |

**平滑更新（指数移动平均 EMA）**：

```rust
fn smooth_update(current: f32, new: f32) -> f32 {
    current * 0.7 + new * 0.3  // 70%历史 + 30%新值
}
```

**从报告自动记录**：

```rust
pub fn record_from_report(&mut self, report: &ExecutionReport) {
    let user_satisfaction = report.user_feedback.as_ref()
        .map(|fb| (fb.rating as f32) / 5.0);
    self.record_execution(
        &report.task_id,
        report.code_quality_score,
        report.test_coverage,
        report.execution_time_ms,
        report.token_count,
        user_satisfaction,
    );
}
```

**成对比较（Pairwise Comparison）**：

每次执行自动记录 A/B 比较，用于学习用户偏好：

```rust
let comparison = PairwiseComparison {
    task_id: task_id.to_string(),
    option_a: format!("quality={:.2}, coverage={:.2}", code_quality, test_coverage),
    option_b: String::new(),
    chosen: "A".to_string(),
    preferred_reason: Some(format!("task: {}", task_id)),
    timestamp: Utc::now(),
};
self.reward.record_comparison(comparison);
```

最多保留 500 条比较记录，超出则淘汰最早的。

**加权评分**：

```rust
pub fn weighted_reward(&self) -> f32 {
    // 使用 learned_weights 加权
    // 默认权重均匀分布
}
```

---

#### 5.3 贝叶斯信念更新 (`BeliefTracker`)

**文件**: `zn-evolve/src/belief.rs`
**持久化**: `.zero_nine/evolve/belief_states.ndjson`

**贝叶斯更新公式**：

```
P(H|E) = P(E|H) × P(H) / [P(E|H) × P(H) + P(E|¬H) × (1-P(H))]
```

具体实现（`belief.rs:98-123`）：

```rust
let prior = state.confidence;           // 先验 = 当前置信度
let likelihood_success = 0.8;           // 假设正确时成功的概率
let likelihood_failure = 0.3;           // 假设正确时失败的概率

if success {
    let numerator = prior * likelihood_success;
    let denominator = numerator + (1.0 - prior) * (1.0 - likelihood_success);
    state.confidence = (numerator / denominator).clamp(0.0, 1.0);
} else {
    let numerator = prior * likelihood_failure;
    let denominator = numerator + (1.0 - prior) * (1.0 - likelihood_failure);
    state.confidence = (numerator / denominator).clamp(0.0, 1.0);
}
```

**证据加权**：

```rust
struct WeightedEvidence {
    content: String,    // 证据内容
    weight: f32,        // 权重 (成功=0.7, 失败=0.5)
    credibility: f32,   // 可信度 (默认 0.8)
    timestamp: DateTime<Utc>,
}

// 调整后的权重 = weight × credibility
fn adjusted_weight(&self) -> f32 {
    self.weight * self.credibility
}
```

**置信度历史追踪**：

保留最近 20 轮的置信度值，用于趋势分析：

```rust
state.confidence_history.push(state.confidence);
if state.confidence_history.len() > 20 {
    state.confidence_history.remove(0);
}
```

**决策逻辑**（`get_decision()`）：

| 条件 | 决策 |
|------|------|
| `confidence > 0.7 && evidence_balance > 0.3` | `should_continue = true` |
| `confidence < 0.3 || 反面证据 > 正面证据 × 2` | `should_change_hypothesis = true` |
| `0.4 < confidence < 0.8 && 有未回答问题` | `should_run_experiment = true` |
| `confidence < 0.2 || (反面 > 3 且 正面 < 2)` | `should_escalate = true` |

**六种推荐行动**：

```rust
enum RecommendedAction {
    ProceedToExecution,    // 置信度 > 0.85 且证据平衡 > 0.5
    ReconsiderHypothesis,  // 置信度 < 0.3
    AnswerQuestions,       // 未回答问题 > 2
    RunVerification,       // 置信度 > 0.6
    GatherMoreEvidence,    // 默认
    EscalateToHuman,       // 需人工介入
}
```

---

#### 5.4 ELO 课程学习 (`CurriculumManager`)

**文件**: `zn-evolve/src/curriculum.rs`
**持久化**: `.zero_nine/evolve/curriculum_history.ndjson`

**基础难度适应**（`adapt_difficulty()`）：

```rust
// 最近 5 次成功率
let recent_avg = success_history.rev().take(5).sum() / 5.0;

if recent_avg > 0.8 {
    // 太简单 → 提升难度
    current_difficulty = (current_difficulty + 0.1).min(0.9);
} else if recent_avg < 0.4 {
    // 太困难 → 降低难度
    current_difficulty = (current_difficulty - 0.1).max(0.1);
}
```

**ELO 风格难度调整**（`adapt_difficulty_elo()`）：

```rust
// 期望成功率（基于当前评分与全局难度差距）
let expected = 1.0 / (1.0 + 10^((rating - difficulty) / 0.4));

// K 因子：新任务变化快，老任务变化慢
let k = if mastery_exists { 0.1 } else { 0.2 };

// ELO 更新
new_rating = rating + k × (actual - expected);
```

**最近发展区推荐**（`get_optimal_next_task()`）：

```rust
// 当前平均掌握程度
let current_mastery = mastery_level.values().sum() / count;

// 最优难度 = 当前能力 + 0.1（略高于能力）
let optimal_difficulty = current_mastery + 0.1;

// 寻找最优难度区间 ±0.15 内的任务
let candidates = tasks.filter(|diff| |diff - optimal| < 0.15);
```

**掌握度更新**（指数移动平均）：

```rust
*mastery = (*mastery * 0.8 + success_rate * 0.2).clamp(0.0, 1.0);
```

**前置依赖检查**：

```rust
pub fn check_prerequisites(&self, task_id: &str) -> bool {
    // 检查所有前置技能的掌握度是否 > 0.5
}
```

---

#### 5.5 三系统融合决策 (`IntegrationEngine`)

**文件**: `zn-evolve/src/integration_engine.rs`
**架构**: Reward → Belief → Curriculum 三路并行 + 决策融合层

**完整调用链**（`record_execution()`）：

```rust
pub fn record_execution(
    &mut self,
    task_id: &str,
    success: bool,
    evidence: &str,
    report: &ExecutionReport,
) -> Result<()> {
    // 1. 更新奖励模型
    self.reward_model.record_from_report(report);

    // 2. 更新课程学习
    let task_diff = TaskDifficulty {
        task_id: task_id.to_string(),
        estimated_difficulty: 0.5,
        actual_difficulty: if success { 0.4 } else { 0.7 },
        completion_time_ms: report.execution_time_ms,
        success,
    };
    self.curriculum_manager.record_task_completion(&task_diff);

    // 3. 更新信念状态
    self.belief_tracker.update_belief(success, evidence, None);

    // 4. 保存所有状态
    self.reward_model.save()?;
    self.curriculum_manager.save()?;
    self.belief_tracker.save()?;
}
```

**决策融合**（`get_integrated_decision()`）：

```rust
// 是否继续执行：信念系统同意 AND 奖励分数 > 0.5
let should_continue = belief.should_continue
    && reward.weighted_score > 0.5;

// 是否改变假设：信念系统建议 OR (低奖励 + 低置信度)
let should_change_hypothesis = belief.should_change_hypothesis
    || (reward.weighted_score < 0.3 && belief.confidence < 0.4);

// 是否升级：信念系统建议 OR (极低置信度 + 低代码质量)
let should_escalate = belief.should_escalate
    || (belief.confidence < 0.2 && reward.code_quality < 0.3);
```

**行动推荐融合**（`fuse_actions()`）优先级：

```
1. 奖励 < 0.3       → GatherMoreEvidence    (证据不足优先)
2. 信念建议升级      → EscalateToHuman       (人工介入优先)
3. 奖励 > 0.7 + 信念同意执行 → ProceedToExecution
4. 默认             → 使用信念系统的推荐
```

**冲突检测**（3 种冲突类型）：

| 冲突 | 检测条件 | 含义 |
|------|---------|------|
| 高置信 + 低奖励 | `confidence > 0.7 && reward < 0.4` | 系统认为假设正确但执行质量低 |
| 低置信 + 高奖励 | `confidence < 0.3 && reward > 0.7` | 执行质量高但系统对假设不确定 |
| 难度差距过大 | `|optimal - mastery| > 0.3` | 推荐难度与当前能力不匹配 |

**输出结构**：

```rust
struct IntegratedDecision {
    should_continue: bool,
    should_change_hypothesis: bool,
    should_escalate: bool,
    recommended_difficulty: f32,
    recommended_task_id: Option<String>,
    recommended_action: RecommendedAction,
    confidence: f32,
    evidence_balance: f32,
    reward_score: f32,
    reasoning: DecisionReasoning {  // 可解释性
        belief_reasoning: String,
        curriculum_reasoning: String,
        reward_reasoning: String,
        conflicts: Vec<String>,
    },
}
```

---

#### 5.6 技能蒸馏 (`SkillDistiller`)

**文件**: `zn-evolve/src/distiller.rs`
**持久化**: `.zero_nine/evolve/patterns.ndjson` + `.zero_nine/evolve/distilled_skills.ndjson`

**模式提取流程**：

```
ExecutionReport
      │
      ▼
┌─────────────────────────────────────────┐
│         PatternExtractor                 │
│                                          │
│  从报告的 5 个区域提取模式：              │
│  1. workspace_record → WorkspacePreparation
│  2. agent_runs       → SubagentCoordination
│  3. evidence         → EvidenceCollection
│  4. verdicts         → VerificationWorkflow
│  5. failures         → ErrorRecovery
└────────────────────┬────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────┐
│         Pattern Merge                    │
│                                          │
│  匹配规则：description 相同 或           │
│          evidence_key 有重叠             │
│                                          │
│  合并策略：                               │
│  - frequency += 1                        │
│  - success_rate = 加权平均               │
│  - avg_confidence = 算术平均             │
│  - source_task_ids 追加                  │
└────────────────────┬────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────┐
│         SkillDistiller                   │
│                                          │
│  过滤条件：confidence >= 0.7             │
│          frequency >= 2                  │
│                                          │
│  转换为 SkillBundle:                      │
│  - id (UUID 前 8 位)                     │
│  - name (pattern-{category})             │
│  - version (1.0.0)                       │
│  - preconditions / skill_chain           │
│  - artifacts (evidence keys)             │
│  - usage_count / success_rate            │
│                                          │
│  生成：                                   │
│  - usage_recommendations (按类别)        │
│  - anti_patterns (低成功率/高频风险)     │
└─────────────────────────────────────────┘
```

**8 种模式类别**：

| 类别 | 触发条件 | 提取内容 |
|------|---------|---------|
| `WorkspacePreparation` | 有 workspace_record | 策略类型 (in_place / git_worktree / sandboxed) |
| `SubagentCoordination` | 有 agent_runs | 角色列表、成功率、证据路径 |
| `EvidenceCollection` | 有 collected evidence | 证据 key、收集率 |
| `VerificationWorkflow` | 有 review/verification verdict | 审查状态、验证状态 |
| `ErrorRecovery` | RetryableFailure | 失败摘要、恢复路径 |
| `BranchManagement` | 有 finish_branch_result | 分支操作记录 |
| `SpecRefinement` | 规格变更 | 需求澄清记录 |
| `TaskDecomposition` | DAG 结构调整 | 任务拆分模式 |

**技能应用**：

蒸馏出的技能可以被应用到后续执行计划中：

```rust
pub fn apply_skill_to_plan(&self, skill_id: &str, plan: &mut ExecutionPlan) -> Result<bool> {
    // 1. 添加技能链到执行计划
    plan.skill_chain.extend(skill.bundle.skill_chain);
    // 2. 添加前置条件为验证点
    plan.validation.extend(skill.bundle.preconditions);
    // 3. 添加使用建议为风险提示
    plan.risks.extend(skill.usage_recommendations);
    // 4. 添加产物到交付物列表
    plan.deliverables.extend(skill.bundle.artifacts);
}
```

**技能匹配**：

```rust
pub fn match_skills_for_task(&self, task_description: &str) -> Vec<&DistilledSkill> {
    // 匹配任务描述与技能的 applicable_scenarios
    // 按 confidence_score 和 success_rate 排序
}
```

---

#### 5.7 演化候选 (`propose_candidate`)

**文件**: `zn-evolve/src/lib.rs:96-150`

每次执行完成后自动生成改进建议：

```rust
pub fn propose_candidate(report: &ExecutionReport) -> Option<EvolutionCandidate> {
    if report.outcome == ExecutionOutcome::Completed && report.success {
        // 成功执行 → 推广为可复用模式
        EvolutionCandidate {
            source_skill: "guarded-execution",
            kind: EvolutionKind::AutoImprove,
            reason: "Successful execution with structured verdicts...",
            confidence: 0.76,
        }
    } else {
        // 失败执行 → 生成修复建议
        EvolutionCandidate {
            source_skill: "evidence-driven-verification",
            kind: match report.outcome {
                RetryableFailure => EvolutionKind::AutoImprove,
                _ => EvolutionKind::AutoFix,
            },
            confidence: 0.83,
        }
    }
}
```

演化类型：

| Kind | 含义 | 触发条件 |
|------|------|---------|
| `AutoImprove` | 自动改进 | 成功执行 / 可重试失败 |
| `AutoFix` | 自动修复 | 阻塞 / 升级 |

---

#### 5.8 完整联动时序图

```
zn-loop                        zn-evolve
  │                               │
  │ 任务完成                       │
  │─────────── report ──────────→ │
  │                               │
  │                        ┌──────┴──────┐
  │                        │ evaluate()  │ 基础评分
  │                        │ (0.33~0.97) │
  │                        └──────┬──────┘
  │                               │
  │                        ┌──────▼──────┐
  │                        │   Reward    │ EMA 平滑更新
  │                        │   Model     │ 5维度评分
  │                        └──────┬──────┘
  │                               │
  │                        ┌──────▼──────┐
  │                        │   Belief    │ 贝叶斯更新
  │                        │  Tracker    │ 置信度追踪
  │                        └──────┬──────┘
  │                               │
  │                        ┌──────▼──────┐
  │                        │ Curriculum  │ ELO难度调整
  │                        │  Manager    │ 最近发展区
  │                        └──────┬──────┘
  │                               │
  │                        ┌──────▼──────────┐
  │                        │Integration Engine│
  │                        │  三路融合 + 冲突  │
  │                        └──────┬──────────┘
  │                               │
  │                        ┌──────▼──────┐
  │                        │  Distiller  │ 模式提取
  │                        │  (freq≥2)   │ 技能蒸馏
  │                        └──────┬──────┘
  │                               │
  │                        ┌──────▼──────┐
  │                        │  Candidate  │ 演化建议
  │                        │  (Improve)  │
  │                        └─────────────┘
  │                               │
  │←────── 决策 + 技能 + 候选 ────│
  │                               │
  │ 更新下一批次调度策略            │
  │ (难度/优先级/重试)              │
```

---

## 六、AI 客户端集成

### 阿里云 Coding Plan（默认）

```bash
# 环境变量
export ANTHROPIC_AUTH_TOKEN=sk-sp-xxx
export ANTHROPIC_BASE_URL=https://coding.dashscope.aliyuncs.com/apps/anthropic
export ANTHROPIC_MODEL=qwen3.6-plus
```

**协议**：Anthropic Messages API 兼容模式
- Endpoint: `{base_url}/messages`
- Auth: `x-api-key: sk-sp-xxx`
- Version: `anthropic-version: 2023-06-01`

### 支持的 AI Provider

```rust
AIProvider {
    AlibabaCodingPlan { api_key, model, base_url }  // 默认
    Anthropic { api_key, model }                     // 原生 Claude
    OpenAI { api_key, model }                        // 待实现
    Custom { endpoint, api_key }                     // 自定义
}
```

---

## 七、安全与治理

### 命令安全

| 措施 | 实现 |
|------|------|
| 命令白名单 | 仅允许 build/test/lint 等安全命令 |
| 路径验证 | `canonicalize()` 验证路径在项目目录内 |
| 输入验证 | JSON 反序列化时验证大小和内容 |
| SQL 防护 | FTS5 查询参数转义 |

### 治理策略

```rust
PolicyEngine {
    max_retries: 3,           // 最大重试次数
    parallel_limit: 2,        // 并行任务上限
    risk_levels: [            // 风险等级
        Low,                  // 自由操作
        Medium,               // 需要通知
        High,                 // 需要审批
        Critical,             // 需要多方审批
    ],
    authorization_matrix: {   // 操作权限矩阵
        "git-push-main" → High,
        "deploy-prod" → Critical,
        "run-tests" → Low,
    }
}
```

---

## 八、核心命令参考

### 基础命令

| 命令 | 作用 | 示例 |
|------|------|------|
| `init` | 初始化工作目录 | `zero-nine init --project . --host claude-code` |
| `run` | 启动完整流程 | `zero-nine run --goal "添加登录功能"` |
| `brainstorm` | 独立头脑风暴 | `zero-nine brainstorm --goal "添加积分系统"` |
| `status` | 查看当前状态 | `zero-nine status --project .` |
| `resume` | 从中断恢复 | `zero-nine resume --host claude-code` |
| `export` | 导出宿主适配 | `zero-nine export --project .` |
| `validate-spec` | 规格验证 | `zero-nine validate-spec --project .` |
| `dashboard` | TUI 仪表盘 | `zero-nine dashboard --project .` |

### 扩展命令

| 命令 | 作用 |
|------|------|
| `skill list/create/view/patch/score/distill/apply` | 技能管理 |
| `memory init/add/read/search` | 记忆管理 |
| `mcp init/list/call` | MCP 集成 |
| `cron schedule/remind/cancel/list` | 定时任务 |
| `subagent dispatch/history/ledger` | 子代理管理 |
| `governance check/matrix/ticket/approve/reject` | 治理审批 |
| `github import/create-pr/comment` | GitHub 集成 |
| `observe events/proposal/trace/stats` | 可观测性查询 |

---

## 九、运行时目录详解

```
.zero_nine/
├── manifest.json                      # 项目配置
├── proposals/
│   └── <proposal-id>/
│       ├── proposal.md                # 需求提案
│       ├── design.md                  # 设计方案
│       ├── tasks.md                   # 任务列表
│       ├── dag.json                   # 依赖图
│       ├── acceptance.md              # 验收标准
│       ├── spec-validation.json       # 规格验证报告
│       ├── progress.md                # 进度追踪
│       ├── verification.md            # 验证汇总
│       └── artifacts/
│           └── task-<id>/             # 任务产物
│               └── <artifact-files>
├── brainstorm/
│   ├── <session-id>.json              # 会话记录
│   ├── latest-session.md              # 最新会话（软链）
│   └── latest-session.json            # 最新会话（软链）
├── loop/
│   ├── session-state.json             # 循环状态
│   └── iteration-log.ndjson           # 迭代日志
├── runtime/
│   ├── events.ndjson                  # 全局事件日志
│   ├── current-envelope.json          # 当前执行包装
│   └── subagents/                     # 子代理记录
├── evolve/
│   ├── evaluations.jsonl              # 评估记录
│   ├── pairwise_comparisons.ndjson    # 成对比较
│   ├── candidates/                    # 演化候选
│   ├── skills/                        # 蒸馏技能
│   └── reward_state.json              # 奖励状态
└── specs/                             # 知识模式
```

---

## 十、事件系统

所有状态变更都记录为 NDJSON 事件：

```json
// events.ndjson
{"event":"project.initialized","data":{"host":"claude-code"},"timestamp":"..."}
{"event":"brainstorm.started","data":{"goal":"...","session_id":"..."},"timestamp":"..."}
{"event":"brainstorm.answered","data":{"question_id":"...","verdict":"..."},"timestamp":"..."}
{"event":"task.started","data":{"title":"...","execution_mode":"guided"},"timestamp":"..."}
{"event":"task.completed","data":{"exit_code":0,"outcome":"Completed"},"timestamp":"..."}
{"event":"task.retry_scheduled","data":{"retry_count":1,"max_retries":3},"timestamp":"..."}
{"event":"task.escalated","data":{"failure_summary":"..."},"timestamp":"..."}
{"event":"proposal.completed","data":{"goal":"...","iterations":5},"timestamp":"..."}
```

---

## 十一、技术栈

| 技术 | 用途 |
|------|------|
| **Rust** | 核心语言 |
| **Serde** | 序列化/反序列化 |
| **Tokio** | 异步运行时 (AI 客户端) |
| **Reqwest** | HTTP 客户端 |
| **Tonic** | gRPC 通信 (子代理) |
| **Rustyline** | 终端交互 |
| **Chrono** | 时间处理 |
| **UUID** | 唯一标识符 |

---

## 十二、构建与测试

```bash
# 编译
cargo build --release

# 测试
cargo test --all-targets

# 代码检查
cargo clippy

# 安装到系统
cargo install --path crates/zn-cli
```

### 测试结果

| Crate | 测试数 | 状态 |
|-------|--------|------|
| zn-types | 16 | ✅ |
| zn-spec | 22 | ✅ |
| zn-exec | 3 | ✅ |
| zn-evolve | 29 | ✅ |
| **总计** | **127** | **✅ 全部通过** |

---

## 十三、宿主集成

### 使用流程

```
1. 初始化项目
   zero-nine init --project . --host claude-code

2. 导出宿主适配
   zero-nine export --project .

3. 在宿主中使用
   claude
   /zero-nine 添加搜索功能

4. 查看状态
   zero-nine status --project .
```

### 支持的宿主

| 宿主 | 命令 | 状态 |
|------|------|------|
| Claude Code | `/zero-nine <goal>` | ✅ |
| OpenCode | `/zero-nine <goal>` | ✅ |
| Terminal | `zero-nine run --goal "..."` | ✅ |

---

## 十四、附录

### 关键设计模式

1. **Spec-Driven**: 规格先行，避免 AI 直接写代码导致偏离
2. **Test-First**: 测试先行，保证质量
3. **Evidence-Based**: 基于证据的验证，而非主观判断
4. **Continuous Improvement**: 每次执行都是进化燃料
5. **Recovery by Design**: 假设失败会发生，构建恢复能力
6. **Plugin Architecture**: 任何 AI 代理都可通过适配器集成

### 未来路线图

- [ ] OpenAI Provider 支持
- [ ] 流式响应处理 (SSE)
- [ ] 可视化仪表盘 (TUI 增强)
- [ ] 技能市场 (Skill Marketplace)
- [ ] 冲突自动解决 (Automated Conflict Resolution)
- [ ] 多项目并行编排
- [ ] 远程子代理执行
