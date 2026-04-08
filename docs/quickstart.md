# Zero_Nine 安装与接入指南

**作者：Manus AI**

## 安装前提

当前版本的 **Zero_Nine** 是一个 Rust workspace 项目，因此本地需要可用的 Rust 与 Cargo 环境。建议使用较新的 stable 工具链，以避免依赖解析与 edition 兼容问题。项目已经在本次构建过程中使用较新 stable 版本完成编译验证。

## 本地编译

进入项目根目录后，执行以下命令即可完成编译：

```bash
cd Zero_Nine
cargo build
```

如果需要直接运行 CLI，可使用：

```bash
cargo run -p zn-cli -- --help
```

编译成功后，可执行文件名为 **`zero-nine`**。

## 初始化项目

若要在某个目标仓库中启用 Zero_Nine，可先初始化工作目录：

```bash
zero-nine init --project /path/to/your/repo --host claude-code
```

或：

```bash
zero-nine init --project /path/to/your/repo --host opencode
```

执行后会生成 `.zero_nine/` 工作目录，用于保存 proposal、tasks、loop state、runtime events 与 evolve 工件。

## 运行一句话目标

Zero_Nine 的核心入口是 **一句话目标驱动**。例如：

```bash
zero-nine run --project /path/to/your/repo --host claude-code --goal "为当前仓库增加统一的插件化工作流"
```

运行后，系统会依次执行以下四层流程：

| 层次 | 行为 |
| --- | --- |
| 需求层 | 生成 proposal、design、tasks 与 progress |
| 执行层 | 为任务生成执行计划与验证约束 |
| 调度层 | 迭代推进任务并写入 loop 状态与事件日志 |
| 进化层 | 记录评分并生成演化候选文件 |

## 查看状态

如需查看当前项目状态，可执行：

```bash
zero-nine status --project /path/to/your/repo
```

如果中途被打断，则可以使用：

```bash
zero-nine resume --project /path/to/your/repo --host claude-code
```

## 导出宿主适配文件

如果你想把当前项目导出为 Claude Code 或 OpenCode 可消费的命令与技能适配文件，可执行：

```bash
zero-nine export --project /path/to/your/repo
```

当前版本会写出以下基础文件：

| 宿主 | 输出路径 |
| --- | --- |
| Claude Code | `adapters/claude-code/.claude/commands/zero-nine.md` |
| Claude Code | `adapters/claude-code/.claude/skills/zero-nine-orchestrator/SKILL.md` |
| OpenCode | `adapters/opencode/.opencode/commands/zero-nine.md` |
| OpenCode | `adapters/opencode/.opencode/skills/zero-nine-orchestrator/SKILL.md` |

## Claude Code 接入建议

Claude Code 官方支持插件市场机制，并允许插件包含命令、技能、代理、钩子和相关服务。[1] 因此，当前这套适配文件更适合作为 **完整插件的初始资产**。建议下一步在此基础上补齐插件清单、安装清单与桥接脚本，再将 `/zero-nine` 作为正式插件命令发布。

## OpenCode 接入建议

OpenCode 官方支持通过 `.opencode/commands/` 与 `.opencode/skills/` 定义命令和技能。[2] [3] 因而当前导出的 `zero-nine.md` 与 `SKILL.md` 已经构成了项目级接入基础。将这些文件复制到目标仓库对应目录后，即可让宿主发现该命令与技能。

## 推荐演进路线

如果你要把 Zero_Nine 从当前骨架版本推进到实际生产版本，建议按以下顺序演进：先补齐完整插件桥接层，再强化执行层的真实调用能力，最后完善进化层的版本化注入与回滚机制。这样能够在保持结构清晰的前提下，逐步逼近你想要的“一句话出结果”的最终体验。

## References

[1]: https://code.claude.com/docs/en/plugin-marketplaces "Claude Code Docs - Create and distribute a plugin marketplace"
[2]: https://opencode.ai/docs/commands/ "OpenCode Docs - Commands"
[3]: https://opencode.ai/docs/skills/ "OpenCode Docs - Agent Skills"
