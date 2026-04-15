# zn-bridge gRPC 通信协议设计

**Created**: 2026-04-13
**Author**: Zero_Nine Team
**Status**: Draft

## 概述

本文档描述 `zn-bridge` crate 的设计，它实现 Rust 内核与宿主 agent（Claude Code / OpenCode）之间的 gRPC 通信协议。

## 架构

```
┌─────────────────────────────────────────────────────────────────┐
│                     Agent (Claude Code)                         │
│  ┌─────────────┐                                                │
│  │ Skill/Command│                                               │
│  └──────┬──────┘                                                │
│         │                                                       │
│         ▼                                                       │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              gRPC Client (Agent Side)                    │   │
│  │  - TaskDispatch::dispatch_task()                         │   │
│  │  - TaskStatus::get_status()                              │   │
│  │  - EvidenceStream::stream_evidence()                     │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                            │
                            │ gRPC (HTTP/2)
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Zero_Nine Kernel (Rust)                        │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              gRPC Server (zn-bridge)                     │   │
│  │  - TaskDispatch::dispatch_task() → TaskId                │   │
│  │  - TaskStatus::get_status() → TaskStatus                 │   │
│  │  - EvidenceStream::stream_evidence() → EvidenceRecord    │   │
│  └─────────────────────────────────────────────────────────┘   │
│         │                                                       │
│         ▼                                                       │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              zn-exec (Execution Layer)                   │   │
│  │  - build_plan()                                          │   │
│  │  - execute_plan()                                        │   │
│  │  - persist_subagent_runbook_artifacts()                  │   │
│  └─────────────────────────────────────────────────────────┘   │
│         │                                                       │
│         ▼                                                       │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              .zero_nine/ Runtime                         │   │
│  │  - dispatch_records/                                     │   │
│  │  - recovery_ledgers/                                     │   │
│  │  - evidence/                                             │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## Protobuf Service 定义

```protobuf
syntax = "proto3";
package zero_nine.bridge.v1;

// === Task Dispatch Service ===

service TaskDispatch {
  // Dispatch a task to an agent for execution
  rpc DispatchTask(DispatchRequest) returns (DispatchResponse);
  
  // Cancel a running task
  rpc CancelTask(CancelRequest) returns (CancelResponse);
}

message DispatchRequest {
  string task_id = 1;
  string proposal_id = 2;
  string task_title = 3;
  string task_description = 4;
  repeated string context_files = 5;  // Paths to context artifacts
  ExecutionMode mode = 6;
  WorkspaceStrategy workspace_strategy = 7;
  repeated QualityGate quality_gates = 8;
}

message DispatchResponse {
  string agent_task_id = 1;  // Agent-assigned task identifier
  string status = 2;  // "accepted", "rejected", "queued"
  string message = 3;
}

// === Task Status Service ===

service TaskStatus {
  // Get current status of a dispatched task
  rpc GetStatus(StatusRequest) returns (StatusResponse);
  
  // Stream status updates for a task
  rpc StreamStatus(StatusRequest) returns (stream StatusUpdate);
}

message StatusRequest {
  string task_id = 1;
  string agent_task_id = 2;
}

message StatusResponse {
  string task_id = 1;
  TaskState state = 2;
  string summary = 3;
  repeated string artifacts = 4;  // Generated file paths
  int32 progress_percent = 5;
}

message StatusUpdate {
  TaskState state = 1;
  string summary = 2;
  int32 progress_percent = 3;
  repeated string new_artifacts = 4;
}

enum TaskState {
  TASK_STATE_UNKNOWN = 0;
  TASK_STATE_QUEUED = 1;
  TASK_STATE_RUNNING = 2;
  TASK_STATE_COMPLETED = 3;
  TASK_STATE_FAILED = 4;
  TASK_STATE_CANCELLED = 5;
}

// === Evidence Service ===

service EvidenceStream {
  // Stream evidence records as they are generated
  rpc StreamEvidence(EvidenceRequest) returns (stream EvidenceRecord);
  
  // Submit final evidence when task completes
  rpc SubmitEvidence(EvidenceRequest) returns (SubmitEvidenceResponse);
}

message EvidenceRequest {
  string task_id = 1;
  string agent_task_id = 2;
}

message EvidenceRecord {
  string id = 1;
  EvidenceKind kind = 2;
  string content = 3;  // JSON-encoded or file path
  string file_path = 4;
  int64 timestamp = 5;
}

message SubmitEvidenceResponse {
  bool success = 1;
  repeated string evidence_paths = 2;
  string message = 3;
}

// === Enums (mirrored from zn-types) ===

enum ExecutionMode {
  EXECUTION_MODE_UNKNOWN = 0;
  EXECUTION_MODE_LOCAL = 1;
  EXECUTION_MODE_WORKTREE = 2;
  EXECUTION_MODE_REMOTE = 3;
}

enum WorkspaceStrategy {
  WORKSPACE_STRATEGY_UNKNOWN = 0;
  WORKSPACE_STRATEGY_IN_PLACE = 1;
  WORKSPACE_STRATEGY_TEMP_DIR = 2;
  WORKSPACE_STRATEGY_WORKTREE = 3;
}

message QualityGate {
  string name = 1;
  bool required = 2;
  string command = 3;
}

enum EvidenceKind {
  EVIDENCE_KIND_UNKNOWN = 0;
  EVIDENCE_KIND_TEST_OUTPUT = 1;
  EVIDENCE_KIND_REVIEW_REPORT = 2;
  EVIDENCE_KIND_CODE_DIFF = 3;
  EVIDENCE_KIND_LOG_FILE = 4;
  EVIDENCE_KIND_ARTIFACT_PATH = 5;
}
```

## Rust 实现结构

### zn-bridge crate 结构

```
crates/zn-bridge/
├── Cargo.toml
├── proto/
│   └── bridge.proto
├── src/
│   ├── lib.rs          # 公共 API 导出
│   ├── server.rs       # gRPC 服务器实现
│   ├── service/
│   │   ├── task_dispatch.rs
│   │   ├── task_status.rs
│   │   └── evidence_stream.rs
│   ├── proto/          # 生成的 protobuf 代码
│   └── types.rs        # gRPC ↔ zn-types 转换
```

### 核心 API

```rust
// Server 启动
pub struct BridgeServer {
    addr: SocketAddr,
    shutdown_tx: oneshot::Sender<()>,
}

impl BridgeServer {
    pub async fn bind(addr: SocketAddr) -> Result<Self>;
    pub async fn run(self) -> Result<()>;
    pub async fn shutdown(self) -> Result<()>;
}

// 任务派发处理器
pub trait DispatchHandler: Send + Sync {
    async fn dispatch_task(&self, request: DispatchRequest) -> Result<DispatchResponse>;
    async fn cancel_task(&self, request: CancelRequest) -> Result<CancelResponse>;
}

// 状态查询处理器
pub trait StatusHandler: Send + Sync {
    async fn get_status(&self, request: StatusRequest) -> Result<StatusResponse>;
    fn stream_status(&self, request: StatusRequest) -> Result<mpsc::Receiver<StatusUpdate>>;
}

// 证据流处理器
pub trait EvidenceHandler: Send + Sync {
    fn stream_evidence(&self, request: EvidenceRequest) -> Result<mpsc::Receiver<EvidenceRecord>>;
    async fn submit_evidence(&self, request: EvidenceRequest) -> Result<SubmitEvidenceResponse>;
}
```

### 与 zn-exec 集成

```rust
// zn-exec/src/lib.rs 新增
pub fn execute_plan_with_bridge(
    project_root: &Path,
    task: &TaskItem,
    plan: &ExecutionPlan,
    bridge_addr: SocketAddr,
) -> Result<ExecutionReport> {
    // 1. 通过 gRPC 派发任务给 agent
    let dispatch_response = dispatch_task_via_grpc(bridge_addr, task, plan).await?;
    
    // 2. 等待 agent 执行完成（轮询或流式等待）
    let execution_result = wait_for_agent_execution(bridge_addr, &dispatch_response.agent_task_id).await?;
    
    // 3. 将 agent 结果转换为 ExecutionReport
    build_report_from_agent_result(task, plan, execution_result)
}
```

## gRPC 调用流程

### 任务派发流程

```
Agent                          Zero_Nine Kernel
  │                                   │
  │──DispatchTask────────────────────>│
  │                                   │
  │                                   ├─> 写入 dispatch_record
  │                                   ├─> 创建任务状态跟踪
  │                                   │
  │<─────{agent_task_id, status}──────│
  │                                   │
```

### 状态查询流程

```
Agent                          Zero_Nine Kernel
  │                                   │
  │──StreamStatus───────────────────>│
  │                                   │
  │                                   ├─> 订阅任务状态
  │                                   │
  │<──stream {state, progress}────────│
  │<──────────────────────────────────│
  │<──────────────────────────────────│
```

### 证据回写流程

```
Agent                          Zero_Nine Kernel
  │                                   │
  │──SubmitEvidence─────────────────>│
  │  {task_id, evidence}              │
  │                                   │
  │                                   ├─> 写入 evidence 目录
  │                                   ├─> 更新 recovery_ledger
  │                                   │
  │<─────{success, paths}─────────────│
  │                                   │
```

## 错误处理

| 错误类型 | gRPC Status | 处理策略 |
|---------|-------------|----------|
| 任务不存在 | NOT_FOUND | 返回错误，agent 重新派发 |
| 服务器忙 | UNAVAILABLE | agent 重试，指数退避 |
| 任务已取消 | CANCELLED | 清理资源，不重试 |
| 证据写入失败 | INTERNAL | 记录日志，尝试恢复 |
| 连接超时 | DEADLINE_EXCEEDED | agent 重连，查询任务状态 |

## 安全考虑

1. **本地绑定**: 默认绑定 `127.0.0.1:50051`，不暴露到外部网络
2. **任务验证**: 验证派发请求的 task_id 是否属于当前 proposal
3. **速率限制**: 限制每个 agent 的派发频率
4. **证据验证**: 验证提交的证据文件路径在允许的目录内

## 与现有架构的兼容

- **向后兼容**: 现有的 `execute_plan()` 保持不变，新增 `execute_plan_with_bridge()`
- **配置开关**: 通过 manifest.policy 控制是否启用 gRPC 桥接
- **降级路径**: gRPC 不可用时回退到本地模拟执行

## 实施阶段

### Phase 1: 基础 gRPC 框架
- [ ] 添加 `zn-bridge` crate 和 protobuf 依赖
- [ ] 定义 `.proto` 文件
- [ ] 实现 gRPC 服务器骨架
- [ ] 编写基础集成测试

### Phase 2: 任务派发协议
- [ ] 实现 `TaskDispatch` service
- [ ] 与 `zn-exec` 的 dispatch_record 集成
- [ ] 实现任务状态跟踪

### Phase 3: 证据流协议
- [ ] 实现 `EvidenceStream` service
- [ ] 与 `.zero_nine/runtime/evidence/` 集成
- [ ] 实现 recovery_ledger 更新

### Phase 4: Agent 侧客户端
- [ ] 编写 agent 侧 gRPC 客户端 SDK
- [ ] 更新 Claude Code command/skill
- [ ] 端到端集成测试

## 参考文件

- `crates/zn-exec/src/lib.rs` - 执行计划生成
- `crates/zn-types/src/lib.rs` - 数据类型定义
- `docs/architecture.md` - 整体架构
- `docs/gap-analysis.md` - 缺口分析
