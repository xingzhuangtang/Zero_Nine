# Phase 4: 独立 SDK 抽象 — zn-sdk crate

## Context

M1-M10 + 规格联动 全部完成。`docs/next-phase-design.md` 定义的 Phase 1-3 均已实现。Phase 4 是唯一待完成的阶段。

目标：创建 `zn-sdk` crate 作为统一门面，让 CLI 只依赖 SDK 而不直接调用 zn-* crate。

## 实现方案

### Step 1: 创建 zn-sdk crate

**文件**: `crates/zn-sdk/Cargo.toml` + `crates/zn-sdk/src/lib.rs`

- 添加到 workspace members
- 依赖: zn-types, zn-spec, zn-exec, zn-loop, zn-host (不依赖 zn-cli, 不依赖 clap)
- 导出 `ZeroNine` struct 作为统一入口

### Step 2: 实现 `ZeroNine` struct

**文件**: `crates/zn-sdk/src/lib.rs`

```rust
pub struct ZeroNine {
    config: ZeroNineSdkConfig,
}

impl ZeroNine {
    pub fn new(config: ZeroNineSdkConfig) -> Result<Self>
    pub fn init(&self) -> Result<String>                          // → zn_loop::initialize_project
    pub fn brainstorm(&self, goal: Option<&str>, resume: bool) -> Result<String>  // → zn_loop::brainstorm
    pub fn run_goal(&self, goal: &str) -> Result<ZeroNineRunResponse>  // → zn_loop::run_goal
    pub fn resume(&self) -> Result<String>                        // → zn_loop::resume
    pub fn status(&self) -> Result<ZeroNineStatusResponse>        // → zn_loop::status
    pub fn export(&self) -> Result<ZeroNineExportResponse>        // → zn_loop::export
    pub fn validate_spec(&self) -> Result<String>                 // → zn_loop::validate_spec
}
```

### Step 3: zn-cli 改用 zn-sdk

**文件**: `crates/zn-cli/src/main.rs`

- Cargo.toml 新增 `zn-sdk = { path = "../zn-sdk" }` 依赖
- 核心命令 (`init`, `brainstorm`, `run`, `resume`, `status`, `export`) 改为调用 `ZeroNine` 方法
- 子命令 (`skill`, `memory`, `mcp`, `cron`, `subagent`, `governance`, `github`, `dashboard`, `observe`, `bridge-server`) 暂时保留直接调用（这些是运维工具，不属于 SDK 核心 API）

### Step 4: 更新 zn-loop 返回值

`run_goal()`, `status()`, `export()` 等函数的返回值已经兼容（返回 `Result<String>` 或类似）。SDK 层只负责统一包装。

## 关键文件

| 文件 | 操作 |
|------|------|
| `Cargo.toml` (workspace root) | 添加 zn-sdk 到 members |
| `crates/zn-sdk/Cargo.toml` | **新建** |
| `crates/zn-sdk/src/lib.rs` | **新建** — ZeroNine struct |
| `crates/zn-cli/Cargo.toml` | 添加 zn-sdk 依赖 |
| `crates/zn-cli/src/main.rs` | 核心命令改用 SDK |

## 验证

```bash
cargo test --all-targets
cargo clippy --all-targets
cargo build
```
