# Zero_Nine

**作者：Manus AI**

## 项目定位

**Zero_Nine** 是一个以 **Rust** 编写的统一编排内核，目标是把 **OpenSpec**、**Superpowers**、**Ralph-loop** 与 **OpenSpace** 的核心能力科学整合为一个可落地的工程系统。它并不机械合并四个上游仓库的源码，而是抽取四个项目各自最稳定、最有价值的语义边界，再通过统一的数据模型、状态机、工件目录与宿主适配层，将它们组合为一个可持续演进的工作流引擎。

从职责上看，Zero_Nine 将四个项目分别映射为四层体系。**OpenSpec** 负责需求、设计和任务工件管理；**Superpowers** 负责结构化执行、质量约束与验证节奏；**Ralph-loop** 负责长时循环、状态恢复与任务推进；**OpenSpace** 负责技能评估、演化候选与持续优化。最终对用户只暴露一个统一入口，即类似 `/zero-nine <需求>` 的单一斜杠命令。

## 核心设计理念：Harness Engineering & Environment Engineering

**Zero_Nine 的发展遵循 Harness Engineering（驾驭工程）和 Environment Engineering（环境工程）的原则。**

### 什么是 Harness Engineering？

Harness Engineering 是设计环境、约束和反馈回路以使 AI 代理可靠运行的工程学科。Zero_Nine 本身就是一个 AI 代理的"驾驭系统"：

- **约束与护栏**：DAG 调度、验证关卡、审查裁决、证据收集
- **反馈回路**：多维度奖励信号、成对比较、用户反馈集成
- **恢复机制**：子代理恢复日志、重试预算、升级协议
- **可观测性**：事件日志、迭代跟踪、状态转换、工件持久化

> **核心洞察**：智能不在于我们编写的代码——而在于我们设计的环境，用于可靠地引导 AI 行为。

### 什么是 Environment Engineering？

Environment Engineering 专注于构建 AI 代理运行的环境，而不是代理本身。Zero_Nine 设计的是 AI 代理生活的"世界"：

- **结构化上下文**：上下文协议、子代理调度包、规范工件
- **执行沙盒**：Git worktree 隔离、工作空间准备、文件操作跟踪
- **验证基础设施**：自动审查、基于证据的验证、交付物检查
- **进化生态系统**：技能评分、课程学习、信念状态跟踪、奖励学习

> **核心洞察**：构建代理生活的世界，而不是代理本身。环境对行为的塑造作用超过指令。

### 设计原则

1. **Steering > Control**：设计引导而非限制的约束
2. **Observability First**：每个状态转换、决策和结果都必须可追溯
3. **Recovery by Design**：假设失败会发生；构建重放和恢复能力
4. **Feedback-Driven Evolution**：所有执行都生成持续改进的信号
5. **Plugin Architecture**：任何 AI 代理/客户端都可通过可配置适配器集成

---

## 可行性结论

这件事是**能做到的**，但合理做法不是"把四个仓库硬拼成一个仓库"，而是构建一个 **Rust 编排核心 + 双宿主适配壳层 + 可复用技能资产** 的新项目。Claude Code 已支持插件市场形式分发命令、技能、代理与相关扩展；OpenCode 已支持项目级或全局级的命令目录与技能目录，并且还能发现 Claude 兼容技能目录。因此，Zero_Nine 既可以作为 **技能包** 工作，也可以继续演进为 **插件形态**，并通过同一个 Rust 核心向两个宿主暴露统一能力。

---

## 架构详解：四层工作流 vs 十三层架构

Zero_Nine 有两个不同的"架构层"概念：

### 四层工作流（用户视角）

这是用户使用层面的流程，描述一个任务从需求到完成的生命周期：

| 层 | 名称 | 作用 |
|---|------|------|
| Layer 1 | Brainstorming | 需求澄清层 - 通过苏格拉底式提问，将模糊的用户意图转化为明确的可执行需求 |
| Layer 2 | Spec Capture | 规格捕获层 - 将澄清后的需求转化为结构化的 OpenSpec 工件（proposal、design、tasks、DAG） |
| Layer 3 | Execution | 执行层 - 按照任务 DAG 依赖图，以测试先行的方式执行代码实现，并收集证据 |
| Layer 4 | Evolution | 进化层 - 对执行结果进行评分、生成演化候选，实现系统的持续自优化 |

**核心设计思想：**
- **Spec-Driven**：规格先行，避免 AI 直接写代码导致的偏离
- **Test-First**：测试先行，保证质量
- **Evidence-Based**：基于证据的验证，而非主观判断
- **Continuous Improvement**：每次执行都是进化的燃料

### 十三层架构（实现视角）

这是技术实现层面的架构，是支撑四层工作流所需的功能模块：

| Layer | 名称 | 功能职责 |
|-------|------|----------|
| 1 | Goal Intake | 目标接收 - 接收并解析用户目标 |
| 2 | Context Assembly | 上下文组装 - 组装项目上下文（文件、git、配置） |
| 3 | Policy Injection | 策略注入 - 注入策略约束（权限、风险等级） |
| 4 | Skill Routing | 技能路由 - 路由到合适的技能处理器 |
| 5 | Memory Integration | 记忆集成 - 集成记忆系统（用户偏好、历史决策） |
| 6 | Subagent Dispatch | 子代理调度 - 调度子代理并行执行 |
| 7 | TUI Dashboard | 仪表盘 - 提供交互式可视化界面 |
| 8 | Governance | 治理/权限控制 |
| 9 | Evidence Collection | 证据收集 - 收集执行证据（测试结果、diff） |
| 10 | Verification Gates | 质量关卡 - build/test/lint 检查 |
| 11 | Branch Management | 分支管理 - git worktree、PR |
| 12 | Skill Distiller | 技能蒸馏 - 从执行记录中蒸馏可复用技能 |
| 13 | Observability | 可观测性 - 追踪、指标、日志 |

### 两者的关系

```
用户说："加个搜索功能"
        ↓
┌─────────────────────────┐
│   四层工作流 (用户视角)    │
│ Brainstorming → Spec →  │
│ Execution → Evolution   │
└─────────────────────────┘
        ↓
┌─────────────────────────┐
│  十三层架构 (实现视角)    │
│ Layer 1-13 协同工作     │
│ Context/Governance/     │
│ Evidence/Verification   │
└─────────────────────────┘
        ↓
  功能完成，代码提交
```

**一句话总结：**
- **四层** = 工作流程（用户怎么用）
- **十三层** = 功能模块（开发者怎么实现）
- **类比**：四层是"做菜的四步流程"，十三层是"后厨的所有设备"

---

## 当前已实现内容

当前交付版本聚焦于 **最小可行骨架**。它已经具备一个可编译的 Rust workspace，包含共享类型、需求工件管理、执行策略、循环调度、技能演化和宿主适配六个主要模块，并提供可执行 CLI `zero-nine`。同时，项目已经生成 Claude Code 与 OpenCode 的基础适配文件，使得后续可以通过单一 slash command 将用户目标传入 Rust 核心。

| 模块 | 说明 |
| --- | --- |
| `zn-types` | 定义统一数据模型，包括 proposal、task、loop state、execution report、evolution candidate 等 |
| `zn-spec` | 管理 `.zero_nine/` 工件目录、proposal、tasks、progress 与 runtime events |
| `zn-exec` | 提供任务分类、执行计划生成与统一执行报告结构（含安全命令执行） |
| `zn-loop` | 实现 Zero_Nine 的循环驱动、状态推进、事件写入与结果汇总 |
| `zn-evolve` | 负责执行结果评分和演化候选生成（含技能蒸馏） |
| `zn-host` | 输出 Claude/OpenCode 适配文件并处理宿主识别 |
| `zn-cli` | 提供 `init`、`run`、`status`、`resume`、`export` 命令 |
| `zn-bridge` | gRPC 桥接层，支持子代理通信和 MCP 集成 |

### 安全特性（v1.0.1）

已修复以下安全漏洞：

| 漏洞等级 | 位置 | 修复措施 |
|----------|------|----------|
| **高危** | `zn-exec/src/lib.rs` | 命令注入修复 - 实现命令白名单验证，替换 `sh -lc` 为直接执行 |
| **高危** | `zn-host/src/github.rs` | 路径遍历修复 - 使用 `canonicalize()` 验证路径在项目目录内 |
| **中危** | `zn-bridge/src/mcp_client.rs` | MCP 文件系统验证 - 添加项目根目录限制 |
| **中危** | `zn-spec/src/session_search.rs` | FTS5 SQL 注入修复 - 实现查询参数转义 |
| **中危** | `zn-exec/src/governance.rs` | JSON 反序列化验证 - 添加输入验证和大小限制 |

---

## 目录结构

当前项目结构保持尽量精简，避免无关文件膨胀。核心目录如下：

```text
Zero_Nine/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── CLAUDE.md
├── docs/
│   └── architecture.md
├── scripts/
│   └── bootstrap_zero_nine.sh
├── crates/
│   ├── zn-types/
│   ├── zn-spec/
│   ├── zn-exec/
│   ├── zn-loop/
│   ├── zn-evolve/
│   ├── zn-host/
│   ├── zn-bridge/
│   └── zn-cli/
└── adapters/
    ├── claude-code/
    └── opencode/
```

---

## 快速开始

### 编译 Zero_Nine

```bash
cd /Users/tangxingzhuang/Freedom/Zero_Nine

# Debug 模式（开发用）
cargo build
# 输出：target/debug/zero-nine
# 编译时间：~30 秒 | 文件大小：~50 MB

# Release 模式（生产用）
cargo build --release
# 输出：target/release/zero-nine
# 编译时间：~5-10 分钟 | 文件大小：~5 MB | 性能提升 10-50x
```

**Debug vs Release 模式对比：**

| 特性 | Debug 模式 | Release 模式 |
|------|-----------|-------------|
| 编译命令 | `cargo build` | `cargo build --release` |
| 优化级别 | 无优化 | 最大优化 |
| 调试信息 | 完整 | 无 |
| 编译速度 | 快 | 慢 |
| 运行速度 | 慢 | 快（10-50x） |
| 用途 | 开发、测试 | 生产部署 |

### 安装到系统

```bash
# 方式 1：使用 cargo install（推荐 Release 模式）
cargo install --path crates/zn-cli

# 方式 2：手动创建软链接
ln -s /Users/tangxingzhuang/Freedom/Zero_Nine/target/release/zero-nine /usr/local/bin/zero-nine
```

### 初始化项目

```bash
# 当前目录初始化
zero-nine init

# 指定项目路径和宿主
zero-nine init --project /path/to/project --host claude-code
```

---

## 核心命令详解

### CLI 命令（终端中使用）

| 命令 | 作用 | 示例 |
|------|------|------|
| `init` | 初始化 `.zero_nine/` 工作目录 | `zero-nine init --project . --host claude-code` |
| `run` | 一句话目标启动完整流程 | `zero-nine run --goal "添加登录功能"` |
| `brainstorm` | 独立头脑风暴模式 | `zero-nine brainstorm --goal "添加积分系统"` |
| `status` | 查看当前状态 | `zero-nine status --project .` |
| `resume` | 从中断处继续执行 | `zero-nine resume --host claude-code` |
| `export` | 导出宿主适配文件 | `zero-nine export --project .` |
| `dashboard` | TUI 仪表盘 | `zero-nine dashboard --project .` |

### 完整使用示例

```bash
# 1. 基本用法
zero-nine run --goal "为登录页面添加验证码功能"

# 2. 完整用法（指定项目和宿主）
zero-nine run --project /Users/tangxingzhuang/OperaCoach-AI \
              --host claude-code \
              --goal "为戏曲作品列表页添加搜索和筛选功能"

# 3. 查看状态
zero-nine status --project /Users/tangxingzhuang/OperaCoach-AI

# 4. 从中断恢复
zero-nine resume --project . --host claude-code
```

### 执行流程

```
Brainstorming → Spec Capture → Execution → Evolution
     ↓              ↓              ↓            ↓
  需求澄清       规格生成        代码执行      验证完成
```

---

## 宿主集成

### Claude Code CLI 集成

**安装适配文件：**

```bash
# 复制适配文件到目标项目
cp -r /Users/tangxingzhuang/Freedom/Zero_Nine/adapters/claude-code/.claude/ \
      /Users/tangxingzhuang/OperaCoach-AI/.claude/
```

**使用方式：**

```bash
# 启动 Claude Code
cd /Users/tangxingzhuang/OperaCoach-AI
claude

# 使用 Zero_Nine
/zero-nine 为戏曲作品列表页添加搜索功能
```

**可用的技能命令：**

| 命令 | 作用 |
|------|------|
| `/zero-nine <目标>` | 主编排器，自动执行四层流程 |
| `/zero-nine-orchestrator` | 手动调用编排器 |
| `/zero-nine-brainstorming` | 需求澄清 |
| `/zero-nine-spec-capture` | 规格捕获 |
| `/zero-nine-writing-plans` | 计划编写 |
| `/zero-nine-tdd-cycle` | TDD 执行 |
| `/zero-nine-verification` | 验证关卡 |
| `/zero-nine-finish-branch` | 完成分支 |

### OpenCode CLI 集成

**安装适配文件：**

```bash
# 复制适配文件到目标项目
cp -r /Users/tangxingzhuang/Freedom/Zero_Nine/adapters/opencode/.opencode/ \
      /Users/tangxingzhuang/OperaCoach-AI/.opencode/
```

**使用方式：**

```bash
# 启动 OpenCode
cd /Users/tangxingzhuang/OperaCoach-AI
opencode

# 使用 Zero_Nine
/zero-nine 为登录页面添加手机号验证
```

---

## 扩展命令参考

### Skill 技能管理

```bash
# 列出所有技能
zero-nine skill list

# 创建新技能
zero-nine skill create --name "my-skill" --description "技能描述"

# 查看技能
zero-nine skill view --name "my-skill"

# 编辑技能
zero-nine skill patch --name "my-skill" --old "旧内容" --new "新内容"

# 技能评分
zero-nine skill score --name "my-skill"

# 技能蒸馏（从执行历史中提取）
zero-nine skill distill --run

# 应用技能到任务
zero-nine skill apply --skill-id "skill-xxx" --task-description "添加搜索功能"
```

### Memory 记忆管理

```bash
# 初始化记忆系统
zero-nine memory init

# 添加记忆
zero-nine memory add --target memory --content "用户偏好使用 git worktree"

# 读取记忆
zero-nine memory read --target memory

# 搜索记忆
zero-nine memory search --query "worktree" --limit 10
```

### MCP 管理

```bash
# 初始化
zero-nine mcp init

# 列出工具
zero-nine mcp list --detailed

# 调用工具
zero-nine mcp call --server "filesystem" --tool "read_file" \
                   --args '{"path": "/tmp/test.txt"}'
```

### Cron 定时任务

```bash
# 创建循环任务
zero-nine cron schedule --id "daily-backup" --cron "0 2 * * *" \
                        --description "每日备份"

# 创建一次性提醒
zero-nine cron remind --id "meeting-reminder" --at "14:30" \
                      --description "会议提醒"

# 取消任务
zero-nine cron cancel --id "daily-backup"

# 列出任务
zero-nine cron list
```

### Subagent 子代理

```bash
# 分发任务
zero-nine subagent dispatch --proposal "prop-001" --task "task-003" \
                            --role "researcher"

# 查看历史
zero-nine subagent history --proposal "prop-001"

# 查看恢复记录
zero-nine subagent ledger --proposal "prop-001" --task "task-003"
```

### Governance 治理

```bash
# 检查权限
zero-nine governance check --action "gitpush"

# 查看授权矩阵
zero-nine governance matrix

# 创建审批单
zero-nine governance ticket --action "git-push" \
                            --description "推送到 main 分支" \
                            --risk high

# 查看待审批
zero-nine governance tickets

# 审批/拒绝
zero-nine governance approve --ticket-id "ticket-001" --approver "admin"
zero-nine governance reject --ticket-id "ticket-001" --reason "需要额外测试"
```

### GitHub 集成

```bash
# 导入 Issue
zero-nine github import --repo "owner/repo" --issues 123 456

# 创建 PR
zero-nine github create-pr --branch "feature/new-feature" \
                           --title "添加新功能" \
                           --base "main"

# 评论 Issue/PR
zero-nine github comment --issue 123 --body "评论内容..."
```

### Observe 可观测性

```bash
# 查询事件
zero-nine observe events --event-type "task_completed" --limit 10

# 按提案查询
zero-nine observe proposal --proposal-id "prop-001" --limit 20

# 重放追踪
zero-nine observe trace --trace-id "trace-xxx"

# 延迟统计
zero-nine observe stats --task-id "task-001"
```

---

## 运行时目录结构

```
.zero_nine/
├── manifest.json              # 项目配置
├── proposals/<id>/            # 提案目录
│   ├── proposal.md            # 需求提案
│   ├── design.md              # 设计方案
│   ├── tasks.md               # 任务列表
│   ├── dag.json               # 依赖图
│   └── acceptance.md          # 验收标准
├── brainstorm/                # 头脑风暴会话
├── loop/
│   ├── session-state.json     # 编排器状态
│   └── iteration-log.ndjson   # 迭代日志
├── runtime/
│   ├── events.ndjson          # 事件日志
│   ├── subagents/             # 子代理记录
│   └── verification/          # 验证记录
├── evolve/                    # 演化候选
└── specs/                     # 知识模式
```

---

## 状态流转图

```
Idle 
  → Brainstorming      (需求澄清中)
  → SpecDrafting       (规格编写中)
  → Ready              (准备执行)
  → RunningTask        (任务执行中)
  → Verifying          (验证中)
  → Archived           (已完成)
  
                    ↓
              Retrying (需要重试)
```

---

## 已验证状态

当前项目已经完成本地编译验证，并完成最小烟雾测试。

| 验证项 | 结果 |
| --- | --- |
| Rust workspace 编译 | ✅ 通过 |
| `zero-nine init` | ✅ 通过 |
| `zero-nine run` | ✅ 通过 |
| `zero-nine status` | ✅ 通过 |
| `zero-nine export` | ✅ 通过 |
| 中文目标 proposal 标识回退逻辑 | ✅ 已修复 |
| 安全漏洞修复（5 项） | ✅ 已完成 |
| 全部测试（84 个） | ✅ 通过 |

---

## 常见问题

### Q: `init`、`--host`、`export` 都是什么意思？它们有什么区别？

<details>
<summary>📘 通俗比喻版（开餐厅）</summary>

想象 Zero_Nine 是一个**餐厅管理系统**，你要开一家餐厅。

| 命令/参数 | 类比 | 实际作用 |
|-----------|------|----------|
| `init` | 租店面、搞装修 | 创建 `.zero_nine/` 工作目录 |
| `--host` | 选美团还是饿了么 | 指定用哪个 AI 宿主 |
| `export` | 把菜单上架到外卖平台 | 复制适配文件到 `.claude/commands/` |

**详细解释：**

1. **`init` - 租店面装修**
   ```bash
   zero-nine init --host claude-code
   ```
   就像租店面、搞装修：把空房子变成能开店的状态，准备好厨房、仓库、收银台。

2. **`--host` - 选外卖平台**
   ```bash
   --host claude-code    # 选美团
   --host opencode       # 选饿了么
   --host terminal       # 只做堂食，不上外卖
   ```
   就是告诉系统：我主要通过哪个渠道接单。

3. **`export` - 上架外卖平台**
   ```bash
   zero-nine export --project .
   ```
   就像把菜单放到外卖平台上：美团/饿了么的骑手才能看到你的店，顾客才能在 App 里点你的菜。

**为什么 `--host` 和 `export` 要分开？**

因为这是两步独立的事：
- `--host` = 在后台注册"我要做美团外卖"（保存到配置文件）
- `export` = 实际把菜单上传到美团 App（物理复制文件）

你注册了美团（`--host`），但不上传菜单（`export`），顾客还是看不到你的店。

**一句话总结：**
```
init  = 准备好自己的店
--host = 选哪个平台接单
export = 把店上架到平台
```

</details>

<details>
<summary>🔧 技术详解版</summary>

#### 1. `init` 的目的是什么？

**作用**：初始化项目的 `.zero_nine/` 工作目录结构。

```bash
zero-nine init --project . --host claude-code
```

执行后会创建：
- `.zero_nine/manifest.json` - 项目配置文件
- `.zero_nine/proposals/` - 提案目录
- `.zero_nine/brainstorm/` - 头脑风暴会话目录
- `.zero_nine/runtime/` - 运行时事件日志目录

**类比**：就像 `git init` 初始化 `.git/` 目录一样。

---

#### 2. `--host` 的目的是什么？

**作用**：指定宿主环境（AI 助手平台）。

支持的值：
- `claude-code` - Claude Code CLI
- `opencode` - OpenCode CLI  
- `terminal` - 纯终端模式（无宿主）

**为什么需要**：Zero_Nine 本身是 Rust 内核，但需要通过宿主适配器来与 AI 助手通信。`--host` 决定：
1. 生成哪个宿主适配文件
2. 使用哪种宿主特定的命令格式

---

#### 3. `export` 和 `init` 有什么区别？

| 命令 | 作用 | 输出内容 |
|------|------|----------|
| `init` | 初始化**运行时目录** | `.zero_nine/manifest.json`, `proposals/`, `runtime/` 等 |
| `export` | 导出**宿主适配文件** | `adapters/claude-code/.claude/commands/` 等 |

---

#### 4. `export` 的目的是什么？

**作用**：将宿主适配文件复制到项目目录中，使宿主（Claude Code/OpenCode）能够发现并加载 Zero_Nine 技能。

```bash
zero-nine export --project .
```

导出内容示例：
```
adapters/claude-code/
└── .claude/
    ├── commands/zero-nine.md      # /zero-nine 斜杠命令
    └── skills/zero-nine-orchestrator/SKILL.md  # 编排器技能
```

---

#### 5. `export` 和 `cp -r` 有什么区别？

| 方面 | `zero-nine export` | `cp -r` |
|------|-------------------|---------|
| 智能处理 | 根据 `--host` 选择正确的适配器 | 机械复制所有文件 |
| 模板渲染 | 会填充项目路径等动态内容 | 原样复制 |
| 验证 | 检查适配文件是否存在 | 不检查 |
| 更新逻辑 | 只复制必要的文件 | 全部覆盖 |

**简单说**：`export` 是"智能复制"，会根据配置选择正确的文件并可能做模板渲染；`cp -r` 是无脑全量复制。

---

#### 6. 不是已经用 `--host` 指定了吗？为什么还要 `export`？

这是两个**不同阶段**的操作：

| 阶段 | 命令 | 作用 | 何时使用 |
|------|------|------|----------|
| **阶段 1** | `init --host claude-code` | 在 `.zero_nine/manifest.json` 中**记录**宿主类型 | 初始化项目时 |
| **阶段 2** | `export --project .` | 将适配文件**物理复制**到项目目录 | 需要在宿主中使用时 |

**典型工作流**：
```bash
# 1. 初始化（记录宿主配置）
zero-nine init --project . --host claude-code

# 2. 导出适配文件（让 Claude Code 能发现技能）
zero-nine export --project .

# 3. 在 Claude Code 中使用
claude
/zero-nine 添加搜索功能
```

</details>

---

### Q: `--project .` 中的点是什么意思？

**A:** `.` 表示当前目录，是 Unix/Linux 路径约定的标准写法。`..` 表示父目录。

```bash
# 以下两个命令等价
zero-nine init --project . --host claude-code
zero-nine init --host claude-code  # 默认就是当前目录
```

### Q: 如何查看 CLI 版本？

```bash
zero-nine --version
```

### Q: 执行到一半被打断怎么办？

```bash
# 查看当前状态
zero-nine status

# 恢复执行
zero-nine resume --host claude-code
```

### Q: 如何查看执行历史？

```bash
# 查看事件日志
zero-nine observe events --limit 20

# 查看追踪
zero-nine observe trace --trace-id "trace-xxx"
```

### Q: TUI 仪表盘无法启动？

```bash
# 确保终端支持 Unicode（推荐使用 iTerm2, Kitty, Alacritty）
# 检查终端尺寸（至少 80x24）
echo $COLUMNS $LINES
```

---

## 下一阶段建议

如果你要把 Zero_Nine 继续推进到真正可投入日常使用的版本，下一步最值得优先做的是三件事：

1. **扩展执行桥** - 把当前 `zn-exec` 从"骨架执行器"扩展成真正可调用宿主代理与外部工具的执行桥
2. **补齐插件清单** - 为 Claude Code 补齐完整插件清单与安装说明
3. **技能版本系统** - 把 OpenSpace 式的演化逻辑从本地候选文件提升到可比较、可回滚、可选注入的技能版本系统

---

## References

[1]: https://github.com/Fission-AI/OpenSpec "GitHub - Fission-AI/OpenSpec: Spec-driven development (SDD) for AI coding assistants"
[2]: https://github.com/obra/superpowers "GitHub - obra/superpowers: An agentic skills framework & software development methodology that works"
[3]: https://github.com/PageAI-Pro/ralph-loop "GitHub - PageAI-Pro/ralph-loop: A long-running AI agent loop"
[4]: https://github.com/HKUDS/OpenSpace "GitHub - HKUDS/OpenSpace: Make Your Agents: Smarter, Low-Cost, Self-Evolving"
[5]: https://code.claude.com/docs/en/plugin-marketplaces "Claude Code Docs - Create and distribute a plugin marketplace"
[6]: https://opencode.ai/docs/commands/ "OpenCode Docs - Commands"
[7]: https://opencode.ai/docs/skills/ "OpenCode Docs - Agent Skills"

---

**最后更新**: 2026-04-15  
**版本**: v1.0.1 (安全修复版)
