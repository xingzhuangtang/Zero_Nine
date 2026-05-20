# JiuwenSwarm 借鉴实施计划

> **目标**: 将 JiuwenSwarm 的核心能力科学融入 Zero_Nine，补齐多 Agent 协同、记忆系统、技能自演进、安全治理等关键能力
>
> **参考源**: [JiuwenSwarm](https://github.com/openJiuwen-ai/jiuwenswarm) — Python AI Agent 系统
>
> **来源**: 由第二个 Qoder 窗口研究产出，请第一个窗口按此计划执行
>
> **执行方式**: 按 Phase 顺序推进，每完成一个 Phase 跑一次 `cargo test --all-targets && cargo clippy`

---

## 总体架构对比

| 维度 | Zero_Nine（当前） | JiuwenSwarm | 借鉴方向 |
|------|------------------|-------------|---------|
| 语言 | Rust | Python | 保持 Rust，借鉴设计理念 |
| 多渠道接入 | Claude Code + OpenCode（2 个） | Web/飞书/钉钉/Discord/WhatsApp/小艺（6+） | **新增 Channel 抽象层** |
| 记忆系统 | 仅 brainstorm 会话 + session_search | Session/Task/Coding 三级记忆 + SQLite | **新增三级记忆系统** |
| 多 Agent 协作 | Subagent Dispatch（平铺式） | Leader-Worker 分层架构 | **新增 Team Coordinator** |
| 技能自演进 | Evolution Engine（评分→候选） | 信号检测→补丁→验证→注入闭环 | **新增 Skill Evolver** |
| Agent 通信 | NDJSON 事件日志 | A2A / E2A 协议 | **新增 A2A 协议支持** |
| 工具权限 | governance.rs 基础检查 | 命令白名单 + 路径验证 + 输入限制 + SQL 转义 | **扩展权限矩阵** |
| 定时任务 | cron 命令（未持久化） | 持久化调度 + 心跳唤醒 | **完善 cron 持久化** |

---

## Phase 1: Team Leader-Worker 多 Agent 协作架构

### 1.1 设计思路

JiuwenSwarm 的 Team 模式采用 **Leader-Worker 分层架构**，相比 Zero_Nine 当前平铺式的 `SubagentDispatcher`，优势在于有明确的角色分工和结果聚合。

### 1.2 任务分解

| # | 任务 | 涉及文件 | 优先级 | 依赖 |
|---|------|---------|--------|------|
| 1.1 | 设计 `TeamRole` 枚举（Leader/Worker/Reviewer） | `zn-types/src/team.rs` | P0 | — |
| 1.2 | 设计 `TeamSession` 数据结构（成员、任务分配、聚合结果） | `zn-types/src/team.rs` | P0 | 1.1 |
| 1.3 | 实现 `TeamCoordinator` trait（编排接口） | `zn-loop/src/team_coordinator.rs` | P0 | 1.1, 1.2 |
| 1.4 | 实现 `LeaderAgent`：需求分析 + 任务分解逻辑 | `zn-loop/src/team_coordinator.rs` | P1 | 1.3 |
| 1.5 | 实现 `WorkerAgent`：子任务执行 + 进度上报 | `zn-loop/src/team_coordinator.rs` | P1 | 1.3 |
| 1.6 | 实现结果聚合：汇总 Worker 输出，生成综合报告 | `zn-loop/src/team_coordinator.rs` | P1 | 1.4, 1.5 |
| 1.7 | 在 `zn-loop` 调度器中集成 Team 模式入口 | `zn-loop/src/lib.rs` | P1 | 1.3-1.6 |
| 1.8 | 持久化 Team 会话状态到 `.zero_nine/team/` | `zn-spec/src/team_session.rs` | P2 | 1.2 |
| 1.9 | CLI 命令：`zero-nine team create/status/list` | `zn-cli/src/cmd/team.rs` | P2 | 1.7, 1.8 |
| 1.10 | 单元测试：Team 协作全流程 | `zn-loop/tests/team_test.rs` | P1 | 1.4-1.6 |

### 1.3 关键数据模型

```rust
// zn-types/src/team.rs

/// Agent 在团队中的角色
pub enum TeamRole {
    Leader,     // 任务分解、Worker选择、结果聚合
    Worker,     // 执行子任务
    Reviewer,   // 质量审查（可选）
}

/// 团队成员
pub struct TeamMember {
    pub id: String,
    pub role: TeamRole,
    pub capabilities: Vec<String>,  // 能力标签: ["rust", "frontend", "testing"]
    pub assigned_tasks: Vec<String>,
    pub results: Vec<TaskResult>,
}

/// Team 会话
pub struct TeamSession {
    pub session_id: String,
    pub goal: String,
    pub leader: TeamMember,
    pub workers: Vec<TeamMember>,
    pub reviewer: Option<TeamMember>,
    pub subtasks: Vec<Subtask>,
    pub status: TeamStatus,
    pub aggregated_report: Option<AggregatedReport>,
}

/// 子任务（Leader 分解产生）
pub struct Subtask {
    pub id: String,
    pub description: String,
    pub assigned_to: String,  // worker id
    pub status: SubtaskStatus,
    pub result: Option<TaskResult>,
    pub dependencies: Vec<String>,  // 其他 subtask id
}

/// 聚合报告
pub struct AggregatedReport {
    pub overall_status: String,
    pub worker_reports: Vec<WorkerReport>,
    pub conflicts: Vec<Conflict>,      // 结果冲突检测
    pub recommendations: Vec<String>,
    pub evidence: Vec<String>,
}
```

### 1.4 Leader 决策逻辑

```
Leader 工作流程:
1. 接收 Goal → 分析复杂度 → 确定是否需要 Team 模式
2. 任务分解 → 生成 Subtask DAG（复用现有 dag.json）
3. Worker 匹配 → 根据 capability 标签选择合适 Worker
4. 分发任务 → 调用 SubagentDispatcher 执行
5. 等待完成 → 收集所有 Worker 结果
6. 冲突检测 → 检测多个 Worker 的结果是否冲突
7. 结果聚合 → 生成 AggregatedReport
8. 质量裁决 → 决定是否需要 Reviewer 介入或重试
```

---

## Phase 2: 三级记忆系统

### 2.1 设计思路

JiuwenSwarm 的记忆分三层，Zero_Nine 目前只有 brainstorm 会话级别的短期记忆。

| 记忆层级 | 生命周期 | 存储内容 | 检索方式 |
|---------|---------|---------|---------|
| **Session Memory** | 单次会话 | 对话历史、澄清问题/答案 | 内存缓存 |
| **Task Memory** | 项目级 | 任务经验、失败模式、解决方案 | 关键词 + 相似度 |
| **Coding Memory** | 全局级 | 代码风格偏好、常见模式、用户习惯 | 标签 + 语义搜索 |

### 2.2 任务分解

| # | 任务 | 涉及文件 | 优先级 | 依赖 |
|---|------|---------|--------|------|
| 2.1 | 设计 `MemoryEntry` 统一数据结构（含类型、层级、标签、内容） | `zn-types/src/memory.rs` | P0 | — |
| 2.2 | 设计 `MemoryStore` trait（CRUD + 搜索接口） | `zn-types/src/memory.rs` | P0 | 2.1 |
| 2.3 | 实现 `SqliteMemoryStore`（使用 rusqlite） | `zn-memory/src/store.rs` | P0 | 2.2 |
| 2.4 | 实现 Session Memory（内存缓存，会话结束自动清理） | `zn-memory/src/session.rs` | P1 | 2.2 |
| 2.5 | 实现 Task Memory（SQLite 持久化，按项目隔离） | `zn-memory/src/task.rs` | P1 | 2.3 |
| 2.6 | 实现 Coding Memory（SQLite 持久化，全局共享） | `zn-memory/src/coding.rs` | P1 | 2.3 |
| 2.7 | 实现上下文压缩策略（自动检测 token 超限 → 压缩非关键信息） | `zn-exec/src/context_compressor.rs` | P1 | 2.4 |
| 2.8 | 在 `zn-loop` 中集成记忆查询（执行前检索相关 Task/Coding Memory） | `zn-loop/src/lib.rs` | P2 | 2.4-2.6 |
| 2.9 | CLI 命令：`zero-nine memory add/search/read/clear` | `zn-cli/src/cmd/memory.rs` | P2 | 2.4-2.6 |
| 2.10 | 单元测试：记忆 CRUD + 搜索 | `zn-memory/tests/memory_test.rs` | P1 | 2.3-2.6 |

### 2.3 数据库 Schema

```sql
-- .zero_nine/memory.db

CREATE TABLE memories (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,      -- 'session' | 'task' | 'coding'
    project_id  TEXT,               -- NULL 表示全局记忆
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    tags        TEXT,               -- JSON 数组
    importance  REAL DEFAULT 0.5,   -- 0.0-1.0
    usage_count INTEGER DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_memories_kind ON memories(kind);
CREATE INDEX idx_memories_project ON memories(project_id);
CREATE INDEX idx_memories_tags ON memories(tags);
CREATE VIRTUAL TABLE memories_fts USING fts5(content, tags);
```

### 2.4 记忆注入时机

```
执行前:
  Goal Intake → 检索 Task Memory（相似任务经验）
             → 检索 Coding Memory（代码风格/用户偏好）
             → 组装 Context（注入 prompt）

执行后:
  ExecutionReport → 提取经验教训 → 写入 Task Memory
  用户反馈        → 更新 Coding Memory（偏好修正）
  失败模式        → 写入 Task Memory（防重复踩坑）
```

---

## Phase 3: 技能自演进闭环

### 3.1 设计思路

Zero_Nine 已有进化引擎（评分→候选），但**缺少"自动修复并注入"的最后一公里**。JiuwenSwarm 的闭环是：

```
信号检测 → 候选生成 → 验证 → 注入
   ↓          ↓          ↓      ↓
执行失败   改进方案    测试通过   新技能
用户修正   成功模式    证据齐全   替换旧版
```

### 3.2 任务分解

| # | 任务 | 涉及文件 | 优先级 | 依赖 |
|---|------|---------|--------|------|
| 3.1 | 设计 `EvolutionSignal` 枚举（Failure/UserCorrection/SkillAttribution） | `zn-types/src/evolution.rs` | P0 | — |
| 3.2 | 实现信号检测器（从 ExecutionReport 和用户反馈中提取信号） | `zn-evolve/src/signal_detector.rs` | P0 | 3.1 |
| 3.3 | 实现 `SkillEvolver`：基于信号生成技能补丁 | `zn-evolve/src/skill_evolver.rs` | P1 | 3.2 |
| 3.4 | 实现补丁验证：在沙箱中测试新技能（不污染现有技能） | `zn-evolve/src/skill_evolver.rs` | P1 | 3.3 |
| 3.5 | 实现技能注入：验证通过后写入技能目录（带版本管理） | `zn-evolve/src/skill_evolver.rs` | P1 | 3.3, 3.4 + Phase4技能版本系统 |
| 3.6 | 实现回滚机制：新版本效果差时自动回退 | `zn-evolve/src/skill_evolver.rs` | P2 | 3.5 |
| 3.7 | 在 `zn-loop` 中集成演进触发（验证失败 → 自动触发演进） | `zn-loop/src/lib.rs` | P2 | 3.3-3.5 |
| 3.8 | CLI 命令：`zero-nine evolve auto --on` | `zn-cli/src/cmd/evolve.rs` | P2 | 3.7 |
| 3.9 | 单元测试：信号检测 + 演进流程 | `zn-evolve/tests/evolution_test.rs` | P1 | 3.2-3.5 |

### 3.3 信号检测规则

```rust
// zn-evolve/src/signal_detector.rs

pub enum EvolutionSignal {
    /// 执行失败信号
    ExecutionFailure {
        task_id: String,
        skill_id: String,
        error_pattern: String,
        retry_count: u32,
    },
    /// 用户修正信号
    UserCorrection {
        task_id: String,
        skill_id: String,
        correction_text: String,
        sentiment: Sentiment,  // Positive / Negative / Neutral
    },
    /// 技能归因信号（某个技能被频繁调用但效果一般）
    SkillAttribution {
        skill_id: String,
        invocation_count: u32,
        avg_score: f64,
        trend: Trend,  // Improving / Degrading / Stable
    },
}

/// 信号检测规则
impl SignalDetector {
    pub fn detect(&self, report: &ExecutionReport) -> Vec<EvolutionSignal> {
        let mut signals = vec![];

        // 规则 1: 执行失败 → 触发 Failure 信号
        if report.status == ExecutionStatus::Failed {
            signals.push(EvolutionSignal::ExecutionFailure { ... });
        }

        // 规则 2: 用户反馈中包含修正关键词 → 触发 Correction 信号
        if let Some(feedback) = &report.user_feedback {
            if feedback.contains_correction_keywords() {
                signals.push(EvolutionSignal::UserCorrection { ... });
            }
        }

        // 规则 3: 技能调用频率高但评分低 → 触发 Attribution 信号
        for skill in &report.skills_used {
            if skill.invocation_count > 5 && skill.avg_score < 0.6 {
                signals.push(EvolutionSignal::SkillAttribution { ... });
            }
        }

        signals
    }
}
```

### 3.4 演进工作流

```
ExecutionReport
      │
      ▼
SignalDetector ───→ EvolutionSignal
      │
      ▼
SkillEvolver
  ├── generate_patch()  → 生成技能补丁（diff 格式）
  ├── validate_patch()  → 沙箱测试新技能
  ├── inject_patch()    → 写入 .zero_nine/specs/skills/<id>/v<N>/
  └── rollback()        → 效果差时回退到上一版本
      │
      ▼
SkillRegistry ────→ 更新技能版本索引
```

---

## Phase 4: A2A 通信协议

### 4.1 设计思路

JiuwenSwarm 使用 A2A (Agent-to-Agent) 和 E2A (Environment-to-Agent) 协议进行 Agent 间通信。Zero_Nine 当前使用 NDJSON 事件日志作为通信媒介，适合异步但**不支持实时请求-响应**。

### 4.2 任务分解

| # | 任务 | 涉及文件 | 优先级 | 依赖 |
|---|------|---------|--------|------|
| 4.1 | 设计 `A2AMessage` 数据结构（请求/响应/事件/心跳） | `zn-types/src/a2a.rs` | P0 | — |
| 4.2 | 实现 `A2ABus`（内存消息总线，支持 pub/sub + req/rep） | `zn-bridge/src/a2a_bus.rs` | P0 | 4.1 |
| 4.3 | 实现 `A2AChannel`（gRPC 通道，用于跨进程 Agent 通信） | `zn-bridge/src/a2a_channel.rs` | P1 | 4.2 |
| 4.4 | 定义标准消息类型：TaskAssign / TaskResult / StatusQuery / Heartbeat | `zn-types/src/a2a.rs` | P0 | 4.1 |
| 4.5 | Leader-Worker 通过 A2A 通信（替代 NDJSON 轮询） | `zn-loop/src/team_coordinator.rs` | P1 | 4.2 + Phase1 |
| 4.6 | 实现消息序列化/反序列化（JSON + 可选 MessagePack） | `zn-bridge/src/a2a_serialization.rs` | P1 | 4.1 |
| 4.7 | 实现消息路由（根据 Agent ID 路由到目标） | `zn-bridge/src/a2a_bus.rs` | P1 | 4.2 |
| 4.8 | 实现消息持久化（关键消息写入事件日志，用于恢复） | `zn-spec/src/a2a_persistence.rs` | P2 | 4.2 |
| 4.9 | 单元测试：消息收发 + 路由 + 序列化 | `zn-bridge/tests/a2a_test.rs` | P1 | 4.2-4.7 |

### 4.3 消息协议

```rust
// zn-types/src/a2a.rs

/// A2A 消息类型
pub enum A2AMessage {
    /// 请求-响应模式
    Request {
        id: String,
        from: AgentId,
        to: AgentId,
        kind: RequestKind,
        payload: JsonValue,
        timeout_ms: Option<u64>,
    },
    Response {
        id: String,       // 对应 Request.id
        from: AgentId,
        to: AgentId,
        status: ResponseStatus,
        payload: JsonValue,
    },
    /// 发布-订阅模式
    Event {
        id: String,
        from: AgentId,
        topic: String,     // "task.status", "team.update", "evolution.signal"
        payload: JsonValue,
    },
    /// 心跳
    Heartbeat {
        agent_id: AgentId,
        status: AgentStatus,
        load: Option<f64>,
    },
}

/// 请求类型
pub enum RequestKind {
    TaskAssign,       // Leader → Worker: 分配任务
    TaskResult,       // Worker → Leader: 提交结果
    StatusQuery,      // 查询 Agent 状态
    ResourceRequest,  // 请求共享资源（文件、工具）
    SkillRequest,     // 请求技能执行
}
```

---

## Phase 5: 工具权限矩阵与安全护栏

### 5.1 设计思路

Zero_Nine 已有 `governance.rs` 和 10 项安全修复，但**缺少系统化的权限声明和沙箱等级**。借鉴 JiuwenSwarm 的工具权限模型。

### 5.2 任务分解

| # | 任务 | 涉及文件 | 优先级 | 依赖 |
|---|------|---------|--------|------|
| 5.1 | 设计 `SandboxLevel` 枚举（ReadOnly / WriteProject / FullAccess） | `zn-types/src/governance.rs` | P0 | — |
| 5.2 | 设计 `ToolPermission` 数据结构（工具名、权限级别、白名单参数） | `zn-types/src/governance.rs` | P0 | 5.1 |
| 5.3 | 实现 `PermissionMatrix`：加载/评估/更新权限配置 | `zn-exec/src/permission_matrix.rs` | P0 | 5.2 |
| 5.4 | 实现命令白名单验证（在 `safe_command.rs` 中集成） | `zn-exec/src/safe_command.rs` | P1 | 5.3 |
| 5.5 | 实现路径验证增强（canonicalize + 项目根目录限制） | `zn-exec/src/safe_command.rs` | P1 | 已有基础，增强 |
| 5.6 | 实现输入验证（JSON 大小限制、SQL 转义、敏感信息过滤） | `zn-exec/src/input_validator.rs` | P1 | 5.3 |
| 5.7 | 实现权限审计日志（所有权限检查写入事件日志） | `zn-spec/src/audit_log.rs` | P2 | 5.3 |
| 5.8 | CLI 命令：`zero-nine governance matrix/view/audit` | `zn-cli/src/cmd/governance.rs` | P2 | 5.3, 5.7 |
| 5.9 | 单元测试：权限评估 + 边界条件 | `zn-exec/tests/governance_test.rs` | P1 | 5.3-5.6 |

### 5.3 权限模型

```rust
// zn-types/src/governance.rs

/// 沙箱等级
pub enum SandboxLevel {
    /// 只读：仅允许读取项目文件、查看状态
    ReadOnly,
    /// 项目写入：允许读写项目目录内的文件
    WriteProject,
    /// 完全访问：允许执行任何命令（需要人工审批）
    FullAccess,
}

/// 工具权限
pub struct ToolPermission {
    pub tool_name: String,
    pub allowed: bool,
    pub sandbox_level: SandboxLevel,
    pub whitelist: Vec<String>,   // 允许的具体命令/路径
    pub blacklist: Vec<String>,   // 禁止的命令/路径
    pub requires_approval: bool,  // 是否需要人工审批
    pub risk_level: RiskLevel,    // Low / Medium / High / Critical
}

/// 权限矩阵配置（从 .zero_nine/governance/permissions.json 加载）
pub struct PermissionMatrix {
    pub default_level: SandboxLevel,
    pub tool_permissions: HashMap<String, ToolPermission>,
    pub agent_overrides: HashMap<AgentId, SandboxLevel>,
}
```

### 5.4 配置文件示例

```json
// .zero_nine/governance/permissions.json

{
  "default_level": "WriteProject",
  "tool_permissions": {
    "read_file": {
      "allowed": true,
      "sandbox_level": "ReadOnly",
      "whitelist": ["**/*"],
      "blacklist": [".zero_nine/**", ".git/**", "**/*.env"],
      "requires_approval": false,
      "risk_level": "Low"
    },
    "write_file": {
      "allowed": true,
      "sandbox_level": "WriteProject",
      "whitelist": ["src/**", "tests/**", "docs/**"],
      "blacklist": [".zero_nine/**", ".git/**", "**/*.env", "**/Cargo.toml"],
      "requires_approval": false,
      "risk_level": "Medium"
    },
    "execute_command": {
      "allowed": true,
      "sandbox_level": "WriteProject",
      "whitelist": ["cargo build", "cargo test", "cargo clippy", "npm test"],
      "blacklist": ["rm -rf", "sudo", "curl | bash", "**/*>&/dev/tcp/**"],
      "requires_approval": true,
      "risk_level": "High"
    },
    "git_push": {
      "allowed": true,
      "sandbox_level": "FullAccess",
      "requires_approval": true,
      "risk_level": "Critical"
    }
  }
}
```

---

## Phase 6: 多渠道接入抽象

### 6.1 设计思路

JiuwenSwarm 通过 Channel 层统一接入 Web、IM、AI 助手等多种渠道。Zero_Nine 当前硬编码了 Claude Code 和 OpenCode 两种宿主，新增渠道需要修改核心代码。

### 6.2 任务分解

| # | 任务 | 涉及文件 | 优先级 | 依赖 |
|---|------|---------|--------|------|
| 6.1 | 设计 `Channel` trait（统一消息收发接口） | `zn-types/src/channel.rs` | P0 | — |
| 6.2 | 实现 `ChannelRegistry`：注册/查找/路由渠道 | `zn-host/src/channel_registry.rs` | P0 | 6.1 |
| 6.3 | 将现有 Claude Code 适配器改造为 `ClaudeChannel` | `zn-host/src/channels/claude.rs` | P1 | 6.1, 6.2 |
| 6.4 | 将现有 OpenCode 适配器改造为 `OpenCodeChannel` | `zn-host/src/channels/opencode.rs` | P1 | 6.1, 6.2 |
| 6.5 | 实现 `TerminalChannel`（纯终端交互） | `zn-host/src/channels/terminal.rs` | P2 | 6.1, 6.2 |
| 6.6 | 实现 Web Channel 骨架（预留 HTTP/WebSocket 接口） | `zn-host/src/channels/web.rs` | P3 | 6.1, 6.2 |
| 6.7 | 在 CLI 中支持 `--channel` 参数（替代 `--host`） | `zn-cli/src/main.rs` | P2 | 6.3-6.5 |
| 6.8 | 更新 `zero-nine export` 支持多渠道导出 | `zn-cli/src/cmd/export.rs` | P2 | 6.3-6.5 |
| 6.9 | 单元测试：Channel trait + Registry | `zn-host/tests/channel_test.rs` | P1 | 6.1-6.5 |

### 6.3 Channel Trait 设计

```rust
// zn-types/src/channel.rs

/// 统一渠道接口
pub trait Channel: Send + Sync {
    /// 渠道名称
    fn name(&self) -> &str;

    /// 接收消息（从渠道到 Zero_Nine）
    fn receive(&self) -> Result<InboundMessage>;

    /// 发送消息（从 Zero_Nine 到渠道）
    fn send(&self, message: OutboundMessage) -> Result<()>;

    /// 注册命令/技能（让渠道发现 Zero_Nine 能力）
    fn register(&self, project_path: &Path) -> Result<()>;

    /// 渠道健康检查
    fn health_check(&self) -> bool;
}

/// 入站消息
pub struct InboundMessage {
    pub channel: String,
    pub kind: MessageKind,    // Goal / answer / Command / Feedback
    pub content: String,
    pub metadata: JsonValue,
}

/// 出站消息
pub struct OutboundMessage {
    pub kind: MessageKind,    // Question / Status / Result / Error
    pub content: String,
    pub metadata: JsonValue,
}
```

---

## Phase 7: 持久化 Cron 调度

### 7.1 设计思路

JiuwenSwarm 有持久化的定时任务管理器，支持唤醒后自动执行 to-do。Zero_Nine 的 `cron` 命令目前仅存在于 README 文档中，未实现。

### 7.2 任务分解

| # | 任务 | 涉及文件 | 优先级 | 依赖 |
|---|------|---------|--------|------|
| 7.1 | 设计 `CronJob` 数据结构（id、表达式、目标、状态） | `zn-types/src/cron.rs` | P0 | — |
| 7.2 | 实现 `CronScheduler`：解析 cron 表达式、调度任务 | `zn-loop/src/cron_scheduler.rs` | P0 | 7.1 |
| 7.3 | 实现 SQLite 持久化存储（任务队列 + 执行历史） | `zn-spec/src/cron_store.rs` | P1 | 7.2 |
| 7.4 | 实现心跳机制（定期检查待执行任务） | `zn-loop/src/cron_scheduler.rs` | P1 | 7.2, 7.3 |
| 7.5 | 实现唤醒后自动执行（resume 时检查过期任务） | `zn-loop/src/lib.rs` | P1 | 7.3 |
| 7.6 | CLI 命令：`zero-nine cron schedule/remind/cancel/list` | `zn-cli/src/cmd/cron.rs` | P2 | 7.2-7.5 |
| 7.7 | 单元测试：cron 解析 + 调度 + 持久化 | `zn-loop/tests/cron_test.rs` | P1 | 7.2-7.5 |

### 7.3 Cron 数据库 Schema

```sql
-- .zero_nine/cron.db

CREATE TABLE cron_jobs (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,      -- 'cron' | 'remind' | 'once'
    schedule    TEXT,               -- cron 表达式或时间点
    goal        TEXT NOT NULL,
    description TEXT,
    status      TEXT DEFAULT 'active',
    last_run    TEXT,
    next_run    TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE cron_history (
    id          TEXT PRIMARY KEY,
    job_id      TEXT NOT NULL,
    started_at  TEXT NOT NULL,
    completed_at TEXT,
    status      TEXT,               -- 'success' | 'failed' | 'timeout'
    output      TEXT,
    FOREIGN KEY (job_id) REFERENCES cron_jobs(id)
);
```

---

## 实施路线图

```
Week 1-2: Phase 4 (A2A) + Phase 1 (Team)  ← 先建通信，再建协作
Week 3:   Phase 2 (Memory)                ← 记忆系统
Week 4:   Phase 3 (Evolution) + Phase 5 (Security)  ← 自演进 + 安全
Week 5:   Phase 6 (Channel) + Phase 7 (Cron)        ← 渠道 + 定时
Week 6:   集成测试 + 文档完善
```

### 依赖关系图

```
Phase 4 (A2A)  ────────┐
                       ├──→ Phase 1 (Team)  ← Team 通过 A2A 通信
Phase 1 trait化完成 ────┘
                       │
Phase 2 (Memory) ──────┤
                       ├──→ Phase 3 (Evolution)  ← 演进需要记忆 + A2A
Phase 5 (Security) ────┤
                       ──→ Phase 6 (Channel)    ← 所有渠道受权限控制
Phase 7 (Cron) ─────────┘
```

### 与第一个窗口的协调

| 第一个窗口（当前进行中） | 本计划 | 协调方式 |
|------------------------|--------|---------|
| Phase 1.5: SubagentDispatcher trait 化 | Phase 1 + Phase 4 | 等 trait 化完成后，先做 A2A 再上 Team |
| zn-exec 重构 | zn-exec 安全增强 | 不同文件，不冲突 |
| — | zn-memory 新 crate | 无依赖 |
| — | zn-bridge A2A 扩展 | 无依赖 |

**建议执行顺序**：
1. 第一个窗口完成 Phase 1.5（SubagentDispatcher trait 化）后
2. 本计划从 Phase 4（A2A 协议）开始 → 为 Team 提供通信基础
3. 然后 Phase 1（Team） → 使用 A2A + trait 化的 Dispatcher
4. 之后并行推进 Phase 2/3/5/6/7

---

## 测试策略

每个 Phase 完成后必须满足：

| 指标 | 要求 |
|------|------|
| 单元测试覆盖率 | ≥ 80% |
| 集成测试 | 至少 1 个端到端场景 |
| Clippy | 0 错误 |
| TODO/stub | 0 |
| 安全审查 | 通过 `cargo audit` |

---

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| A2A 协议设计过于复杂 | 延迟 Team 实现 | 先实现内存版消息总线，gRPC 通道后续 |
| 记忆系统 SQLite 依赖增加编译复杂度 | 构建时间增加 | 使用 `rusqlite` 的 `bundled` feature，无需系统库 |
| 多渠道适配文件维护成本 | 适配器膨胀 | 抽象公共逻辑，各渠道只实现差异部分 |
| 技能自演进可能"越进化越差" | 技能质量下降 | 强制验证 + 版本回滚 + 效果对比 |

---

**文档版本**: v1.0
**生成时间**: 2026-05-18
**研究窗口**: 第二个 Qoder 窗口
**执行窗口**: 第一个 Qoder 窗口
