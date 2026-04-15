use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use zn_types::HostKind;

pub mod github;
pub use github::{read_github_issues, create_pull_request, write_issue_comment, write_execution_summary};

pub fn detect_host(explicit: Option<&str>) -> HostKind {
    match explicit.unwrap_or_default().to_lowercase().as_str() {
        "claude" | "claude-code" => HostKind::ClaudeCode,
        "open" | "opencode" => HostKind::OpenCode,
        _ => HostKind::Terminal,
    }
}

pub fn claude_command_markdown() -> String {
    "Run the local Zero_Nine orchestration engine for the current project through one continuous host-native loop.\n\nUse this same command on every turn:\n\n`zero-nine run --host claude-code --project . --goal \"$ARGUMENTS\"`\n\nOn the first turn, `$ARGUMENTS` is the user goal. If Brainstorming is not yet Ready, Zero_Nine will treat the next invocation as the answer to the latest clarification question instead of starting a new run. Keep invoking the same command with only the latest answer until Zero_Nine reports that Brainstorming is Ready, the OpenSpec bundle is bound, and execution can continue.\n\nInspect live state at any time with `zero-nine status --project .`. The status view now exposes runnable tasks, DAG blocking details, the active loop stage, and the subagent runtime directory that contains dispatch records, recovery ledgers, and resumable evidence artifacts. Export or refresh host adapter files with `zero-nine export --project .` after upgrading the local binary so the host-visible instructions stay aligned with the latest scheduler and recovery protocol.\n"
        .to_string()
}

pub fn opencode_command_markdown() -> String {
    "---\ndescription: Run Zero_Nine through one continuous host-native clarify-to-execute loop\nsubtask: true\n---\nRun the local Zero_Nine orchestration engine for the current project through one continuous host-native loop.\n\nUse this same command on every turn:\n\n`zero-nine run --host opencode --project . --goal \"$ARGUMENTS\"`\n\nOn the first turn, `$ARGUMENTS` is the user goal. If Brainstorming is still collecting answers, Zero_Nine will interpret the next invocation as the answer to the latest clarification question rather than launching execution. Keep invoking the same command with only the latest answer until Zero_Nine reports that Brainstorming is Ready, the OpenSpec bundle is bound, and guarded execution can continue.\n\nInspect live state at any time with `zero-nine status --project .`. The status output now includes runnable tasks, DAG blocking details, current loop stage, and the subagent runtime directory where dispatch records, recovery ledgers, and evidence paths are written for resume and review. Re-run `zero-nine export --project .` after binary upgrades so command help and shared skill text stay synchronized with the current scheduler and recovery contract.\n"
        .to_string()
}

pub fn shared_skill_markdown() -> String {
    "---\nname: zero-nine-orchestrator\ndescription: Coordinate the Zero_Nine four-layer workflow. Use when you need one host command that can clarify requirements, bind an OpenSpec contract, run guarded execution, control the loop, and write back evolution artifacts.\n---\n## What to do\n\nRoute the request through four layers in order: Brainstorming, spec capture, execution, and evolution.\n\nFor Claude Code and OpenCode, treat the slash command as one continuous clarify-to-execute entry point. Reuse the same command for each answer until Zero_Nine reports that the session is Ready and the bound OpenSpec artifacts are complete. Do not bypass the clarification or specification gates by starting a separate execution command early.\n\nUse `zero-nine status --project .` whenever you need an inspectable checkpoint. The status view now highlights runnable tasks, DAG blocking details, loop stage, and the subagent runtime directory that stores dispatch records, recovery ledgers, and resumable evidence. Treat those runtime artifacts and per-task reports as the canonical trace when you need to explain why the scheduler is blocked or what each subagent returned.\n\nIf host adapter files were exported before a runtime upgrade, refresh them with `zero-nine export --project .` so the local command help remains aligned with the latest scheduler window, retry behavior, and subagent recovery protocol.\n\n## When to use me\n\nUse this skill when a user wants a single entry point that can clarify requirements, produce inspectable specification artifacts, run a guarded implementation workflow, and write back progress and learning artifacts with minimal manual intervention.\n"
        .to_string()
}

pub fn export_adapter_files(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();

    let claude_cmd = project_root.join(".claude/commands/zero-nine.md");
    let claude_skill = project_root.join(".claude/skills/zero-nine-orchestrator/SKILL.md");
    let opencode_cmd = project_root.join(".opencode/commands/zero-nine.md");
    let opencode_skill = project_root.join(".opencode/skills/zero-nine-orchestrator/SKILL.md");

    for path in [&claude_cmd, &claude_skill, &opencode_cmd, &opencode_skill] {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
    }

    fs::write(&claude_cmd, claude_command_markdown())?;
    fs::write(&claude_skill, shared_skill_markdown())?;
    fs::write(&opencode_cmd, opencode_command_markdown())?;
    fs::write(&opencode_skill, shared_skill_markdown())?;

    written.push(claude_cmd);
    written.push(claude_skill);
    written.push(opencode_cmd);
    written.push(opencode_skill);

    Ok(written)
}
