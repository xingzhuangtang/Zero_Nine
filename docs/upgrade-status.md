# Zero_Nine 增强升级说明

## 当前结论

本次升级已经把 **Zero_Nine** 从“可运行骨架”推进到“具备增强版编排与工件输出能力的原型系统”。

当前版本已经能够围绕一个目标，生成并串联以下链路：

1. **Brainstorming 阶段**：生成更结构化的问题澄清、需求摘要与 requirement packet。
2. **OpenSpec 风格工件写入**：写入 `proposal.md`、`requirements.md`、`acceptance.md`、`clarifications.md`、`design.md`、`tasks.md`、`dag.json`、`progress.txt`。
3. **Ralph-loop 风格推进**：按任务顺序推进，记录 `iteration-log`、`events`、`session-state`。
4. **Superpowers 风格 planning 与执行链**：生成 `writing-plans`、`workspace-plan`、`subagents`、`review-brief`、`tdd-cycle`、`implementation` 等工件。
5. **进化层沉淀**：写入 `evaluations.jsonl` 与 candidate 文件，为后续 OpenSpace 式优化预留入口。
6. **宿主适配导出**：仍可导出 Claude Code 与 OpenCode 所需的命令和技能文件。

## 已验证结果

当前本地工程已通过：

```bash
cargo test
```

其中新增的执行层测试已经通过，说明增强后的 planning、implementation artifact 与质量门禁输出逻辑可正常工作。

此外，直接运行编译产物时，增强版流程已经可以成功完成一次端到端执行：

```bash
./target/debug/zero-nine run --project ./demo_project_v2 --host opencode --goal test-run
```

## 当前仍然属于“原型增强版”的部分

虽然链路已经更完整，但它还不是四个原项目的原版完整集成。当前仍然属于 **Zero_Nine 自主实现的增强版编排内核**，主要边界如下：

| 能力 | 当前状态 |
| --- | --- |
| Superpowers 苏格拉底式真实交互问答 | 仍是文件化与流程化模拟，不是宿主内实时多轮问答 |
| OpenSpec 原版协议与命令完全兼容 | 还未做到一比一兼容 |
| Ralph-loop 真正长时后台调度 | 目前是同步 CLI 运行模型 |
| OpenSpace 实时监控自动注入技能 | 目前是候选与评估沉淀，未做自动热注入 |
| worktree 真正创建与 Git 自动操作 | 目前输出工作区计划，尚未真实执行 git worktree 命令 |
| finishing-a-development-branch 真正 PR/merge 工作流 | 目前输出收尾工件与建议，未接 GitHub/Git 提交链 |

## 你现在最推荐的实操命令

建议你以后优先用已经编译好的二进制直接跑，这样比 `cargo run` 更稳定，也更容易判断问题：

```bash
cd /Users/tangxingzhuang/Freedom/Zero_Nine
cargo build
./target/debug/zero-nine init --project ./demo_project_v3 --host opencode
./target/debug/zero-nine run --project ./demo_project_v3 --host opencode --goal "把 Superpowers Brainstorming、OpenSpec、Ralph-loop、OpenSpace 串成可执行插件链路"
./target/debug/zero-nine status --project ./demo_project_v3
./target/debug/zero-nine export --project ./demo_project_v3
```

## 工件检查位置

运行成功后，请重点查看：

```bash
find ./demo_project_v3/.zero_nine -type f | sort
```

特别关注以下目录：

| 路径 | 说明 |
| --- | --- |
| `.zero_nine/proposals/<proposal-id>/artifacts/task-1/` | Brainstorming 结果 |
| `.zero_nine/proposals/<proposal-id>/artifacts/task-3/` | writing-plans、workspace-plan、subagents |
| `.zero_nine/proposals/<proposal-id>/artifacts/task-4/` | implementation、review-brief、tdd-cycle |
| `.zero_nine/loop/iteration-log.ndjson` | Ralph-loop 风格迭代记录 |
| `.zero_nine/evolve/` | 进化评估与候选技能 |

## OpenCode 接线提醒

执行导出后，目前适配文件仍导出到：

```text
adapters/opencode/.opencode/...
adapters/claude-code/.claude/...
```

如果你要让 OpenCode 立即识别，仍需把 `.opencode` 目录复制到目标项目根目录；Claude Code 同理，需要把 `.claude` 放到目标项目根目录。

## 下一阶段最值得继续开发的方向

下一阶段建议按以下顺序推进：

1. 先把 **真实交互式 Brainstorming** 做进去，让用户在宿主里多轮澄清。
2. 再把 **git worktree / branch / finish-branch** 做成真正可执行命令。
3. 然后接入 **子代理执行与审查桥接**，把当前工件化计划变成真实代理调用。
4. 最后再做 **独立 SDK**，把当前 CLI 内核抽成稳定 Rust API。
