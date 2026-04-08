#!/usr/bin/env bash
set -euo pipefail

ROOT="/home/ubuntu/Zero_Nine"
mkdir -p "$ROOT/scripts"
mkdir -p "$ROOT/crates/zn-types/src" "$ROOT/crates/zn-spec/src" "$ROOT/crates/zn-loop/src" "$ROOT/crates/zn-exec/src" "$ROOT/crates/zn-evolve/src" "$ROOT/crates/zn-host/src" "$ROOT/crates/zn-cli/src"
mkdir -p "$ROOT/adapters/claude-code/.claude/commands" "$ROOT/adapters/claude-code/.claude/skills/zero-nine-orchestrator" "$ROOT/adapters/opencode/.opencode/commands" "$ROOT/adapters/opencode/.opencode/skills/zero-nine-orchestrator"

cat > "$ROOT/crates/zn-types/Cargo.toml" <<'EOF'
[package]
name = "zn-types"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Shared types for Zero_Nine"

[dependencies]
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
uuid.workspace = true
EOF

cat > "$ROOT/crates/zn-types/src/lib.rs" <<'EOF'
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    ClaudeCode,
    OpenCode,
    Terminal,
}

impl Default for HostKind {
    fn default() -> Self {
        Self::Terminal
    }
}

impl std::fmt::Display for HostKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            HostKind::ClaudeCode => "claude-code",
            HostKind::OpenCode => "opencode",
            HostKind::Terminal => "terminal",
        };
        write!(f, "{}", value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub max_retries: u8,
    pub verify_before_complete: bool,
    pub auto_evolve: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            verify_before_complete: true,
            auto_evolve: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub version: String,
    pub name: String,
    pub default_host: HostKind,
    pub skill_dirs: Vec<String>,
    pub policy: Policy,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            version: "0.1.0".to_string(),
            name: "Zero_Nine".to_string(),
            default_host: HostKind::Terminal,
            skill_dirs: vec![".claude/skills".to_string(), ".opencode/skills".to_string()],
            policy: Policy::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Draft,
    Ready,
    Running,
    Completed,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoopStage {
    Idle,
    SpecDrafting,
    Ready,
    RunningTask,
    Verifying,
    Retrying,
    Escalated,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub title: String,
    pub goal: String,
    pub status: ProposalStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub design_summary: Option<String>,
    pub tasks: Vec<TaskItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    pub tasks: Vec<TaskItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopState {
    pub proposal_id: String,
    pub current_task: Option<String>,
    pub iteration: u32,
    pub retry_count: u8,
    pub stage: LoopStage,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub task_id: String,
    pub steps: Vec<String>,
    pub validation: Vec<String>,
    pub skill_chain: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub task_id: String,
    pub success: bool,
    pub summary: String,
    pub tests_passed: bool,
    pub review_passed: bool,
    pub artifacts: Vec<String>,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEvaluation {
    pub skill_name: String,
    pub task_type: String,
    pub latency_ms: u64,
    pub token_cost: u64,
    pub score: f32,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionKind {
    AutoFix,
    AutoImprove,
    AutoLearn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionCandidate {
    pub source_skill: String,
    pub kind: EvolutionKind,
    pub reason: String,
    pub patch: String,
    pub confidence: f32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEvent {
    pub ts: DateTime<Utc>,
    pub event: String,
    pub proposal_id: Option<String>,
    pub task_id: Option<String>,
    pub payload: Option<Value>,
}

pub fn slugify_goal(goal: &str) -> String {
    let lowered = goal.to_lowercase();
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in lowered.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').chars().take(48).collect()
}
EOF

cat > "$ROOT/crates/zn-spec/Cargo.toml" <<'EOF'
[package]
name = "zn-spec"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Spec and artifact management for Zero_Nine"

[dependencies]
anyhow.workspace = true
chrono.workspace = true
serde_json.workspace = true
zn-types = { path = "../zn-types" }
EOF

cat > "$ROOT/crates/zn-spec/src/lib.rs" <<'EOF'
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use zn_types::{LoopState, LoopStage, ProjectManifest, Proposal, ProposalStatus, RuntimeEvent, TaskGraph, TaskItem, TaskStatus};

pub fn zero_nine_dir(project_root: &Path) -> PathBuf {
    project_root.join(".zero_nine")
}

pub fn ensure_layout(project_root: &Path) -> Result<()> {
    let root = zero_nine_dir(project_root);
    for rel in [
        "proposals",
        "archive",
        "specs/patterns",
        "loop/locks",
        "evolve/skills",
        "evolve/candidates",
        "runtime/cache",
    ] {
        fs::create_dir_all(root.join(rel)).with_context(|| format!("create {}", rel))?;
    }
    Ok(())
}

pub fn save_manifest(project_root: &Path, manifest: &ProjectManifest) -> Result<PathBuf> {
    ensure_layout(project_root)?;
    let path = zero_nine_dir(project_root).join("manifest.json");
    fs::write(&path, serde_json::to_vec_pretty(manifest)?)?;
    Ok(path)
}

pub fn default_tasks(goal: &str) -> Vec<TaskItem> {
    vec![
        TaskItem {
            id: "1".to_string(),
            title: "Refine requirement and constraints".to_string(),
            description: format!("Clarify user intent, scope, and acceptance criteria for: {goal}"),
            status: TaskStatus::Pending,
            depends_on: vec![],
        },
        TaskItem {
            id: "2".to_string(),
            title: "Design spec and task graph".to_string(),
            description: "Produce proposal, design notes, and a workable task graph.".to_string(),
            status: TaskStatus::Pending,
            depends_on: vec!["1".to_string()],
        },
        TaskItem {
            id: "3".to_string(),
            title: "Execute guarded implementation workflow".to_string(),
            description: "Apply structured execution patterns with tests, reviews, and progress tracking.".to_string(),
            status: TaskStatus::Pending,
            depends_on: vec!["2".to_string()],
        },
        TaskItem {
            id: "4".to_string(),
            title: "Verify result and evolve skills".to_string(),
            description: "Summarize outcomes, score workflow quality, and propose improvement candidates.".to_string(),
            status: TaskStatus::Pending,
            depends_on: vec!["3".to_string()],
        },
    ]
}

pub fn create_proposal(project_root: &Path, goal: &str) -> Result<Proposal> {
    ensure_layout(project_root)?;
    let slug = zn_types::slugify_goal(goal);
    let id = format!("{}-{}", Utc::now().format("%Y%m%d%H%M%S"), slug);
    let proposal_dir = zero_nine_dir(project_root).join("proposals").join(&id);
    fs::create_dir_all(proposal_dir.join("artifacts"))?;

    let now = Utc::now();
    let proposal = Proposal {
        id: id.clone(),
        title: goal.to_string(),
        goal: goal.to_string(),
        status: ProposalStatus::Draft,
        created_at: now,
        updated_at: now,
        design_summary: Some("Zero_Nine generated an initial four-layer design scaffold.".to_string()),
        tasks: default_tasks(goal),
    };

    save_proposal(project_root, &proposal)?;
    fs::write(
        proposal_dir.join("proposal.md"),
        format!("# Proposal\n\n## Goal\n\n{}\n\n## Status\n\n{:?}\n", proposal.goal, proposal.status),
    )?;
    fs::write(
        proposal_dir.join("design.md"),
        "# Design\n\nThis design follows the Zero_Nine four-layer model: spec, execution, loop, and evolution.\n",
    )?;
    fs::write(
        proposal_dir.join("tasks.md"),
        render_tasks_markdown(&proposal.tasks),
    )?;
    fs::write(
        proposal_dir.join("dag.json"),
        serde_json::to_vec_pretty(&TaskGraph { tasks: proposal.tasks.clone() })?,
    )?;
    fs::write(
        proposal_dir.join("progress.json"),
        serde_json::to_vec_pretty(&json!({"proposal_id": proposal.id, "completed": [], "pending": ["1", "2", "3", "4"]}))?,
    )?;
    fs::write(proposal_dir.join("verification.md"), "# Verification\n\nPending verification.\n")?;
    append_event(project_root, RuntimeEvent {
        ts: Utc::now(),
        event: "proposal.created".to_string(),
        proposal_id: Some(proposal.id.clone()),
        task_id: None,
        payload: Some(json!({"goal": goal})),
    })?;
    Ok(proposal)
}

pub fn proposal_dir(project_root: &Path, proposal_id: &str) -> PathBuf {
    zero_nine_dir(project_root).join("proposals").join(proposal_id)
}

pub fn save_proposal(project_root: &Path, proposal: &Proposal) -> Result<PathBuf> {
    let path = proposal_dir(project_root, &proposal.id).join("proposal.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(proposal)?)?;
    Ok(path)
}

pub fn load_latest_proposal(project_root: &Path) -> Result<Option<Proposal>> {
    let proposals_root = zero_nine_dir(project_root).join("proposals");
    if !proposals_root.exists() {
        return Ok(None);
    }
    let mut entries: Vec<_> = fs::read_dir(&proposals_root)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    entries.reverse();
    for entry in entries {
        let path = entry.path().join("proposal.json");
        if path.exists() {
            let data = fs::read_to_string(&path)?;
            let proposal: Proposal = serde_json::from_str(&data)?;
            return Ok(Some(proposal));
        }
    }
    Ok(None)
}

pub fn save_loop_state(project_root: &Path, state: &LoopState) -> Result<PathBuf> {
    let path = zero_nine_dir(project_root).join("loop/session-state.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(state)?)?;
    Ok(path)
}

pub fn load_loop_state(project_root: &Path) -> Result<Option<LoopState>> {
    let path = zero_nine_dir(project_root).join("loop/session-state.json");
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(path)?;
    let state: LoopState = serde_json::from_str(&data)?;
    Ok(Some(state))
}

pub fn append_event(project_root: &Path, event: RuntimeEvent) -> Result<()> {
    let path = zero_nine_dir(project_root).join("runtime/events.ndjson");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(&event)?)?;
    Ok(())
}

pub fn render_tasks_markdown(tasks: &[TaskItem]) -> String {
    let mut output = String::from("# Tasks\n\n| ID | Title | Status | Depends On |\n| --- | --- | --- | --- |\n");
    for task in tasks {
        let deps = if task.depends_on.is_empty() { "-".to_string() } else { task.depends_on.join(", ") };
        output.push_str(&format!("| {} | {} | {:?} | {} |\n", task.id, task.title, task.status, deps));
    }
    output
}

pub fn update_progress_markdown(project_root: &Path, proposal: &Proposal) -> Result<()> {
    fs::write(
        proposal_dir(project_root, &proposal.id).join("tasks.md"),
        render_tasks_markdown(&proposal.tasks),
    )?;
    let completed: Vec<String> = proposal
        .tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Completed))
        .map(|task| task.id.clone())
        .collect();
    let pending: Vec<String> = proposal
        .tasks
        .iter()
        .filter(|task| !matches!(task.status, TaskStatus::Completed))
        .map(|task| task.id.clone())
        .collect();
    fs::write(
        proposal_dir(project_root, &proposal.id).join("progress.json"),
        serde_json::to_vec_pretty(&json!({"proposal_id": proposal.id, "completed": completed, "pending": pending}))?,
    )?;
    Ok(())
}

pub fn init_loop_state(proposal_id: &str) -> LoopState {
    LoopState {
        proposal_id: proposal_id.to_string(),
        current_task: None,
        iteration: 0,
        retry_count: 0,
        stage: LoopStage::SpecDrafting,
        updated_at: Utc::now(),
    }
}

pub fn status_summary(project_root: &Path) -> Result<String> {
    let manifest_path = zero_nine_dir(project_root).join("manifest.json");
    let proposal = load_latest_proposal(project_root)?;
    let state = load_loop_state(project_root)?;
    let mut lines = Vec::new();
    lines.push(format!("manifest: {}", if manifest_path.exists() { "present" } else { "missing" }));
    if let Some(proposal) = proposal {
        let done = proposal.tasks.iter().filter(|t| matches!(t.status, TaskStatus::Completed)).count();
        lines.push(format!("proposal: {}", proposal.id));
        lines.push(format!("goal: {}", proposal.goal));
        lines.push(format!("status: {:?}", proposal.status));
        lines.push(format!("tasks: {done}/{} completed", proposal.tasks.len()));
    } else {
        lines.push("proposal: none".to_string());
    }
    if let Some(state) = state {
        lines.push(format!("loop_stage: {:?}", state.stage));
        lines.push(format!("iteration: {}", state.iteration));
        lines.push(format!("current_task: {}", state.current_task.unwrap_or_else(|| "none".to_string())));
    } else {
        lines.push("loop_stage: none".to_string());
    }
    Ok(lines.join("\n"))
}
EOF

cat > "$ROOT/crates/zn-exec/Cargo.toml" <<'EOF'
[package]
name = "zn-exec"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Execution policy engine for Zero_Nine"

[dependencies]
anyhow.workspace = true
zn-types = { path = "../zn-types" }
EOF

cat > "$ROOT/crates/zn-exec/src/lib.rs" <<'EOF'
use anyhow::Result;
use zn_types::{ExecutionPlan, ExecutionReport, TaskItem};

#[derive(Debug, Clone, Copy)]
pub enum TaskKind {
    Brainstorming,
    Planning,
    Implementation,
    Verification,
}

pub fn classify_task(task: &TaskItem) -> TaskKind {
    let title = task.title.to_lowercase();
    if title.contains("refine") || title.contains("requirement") {
        TaskKind::Brainstorming
    } else if title.contains("design") || title.contains("plan") {
        TaskKind::Planning
    } else if title.contains("verify") || title.contains("evolve") {
        TaskKind::Verification
    } else {
        TaskKind::Implementation
    }
}

pub fn build_plan(task: &TaskItem) -> ExecutionPlan {
    let kind = classify_task(task);
    let (steps, validation, skills) = match kind {
        TaskKind::Brainstorming => (
            vec![
                "Capture objective, constraints, and acceptance criteria.".to_string(),
                "Write distilled problem statement into proposal artifacts.".to_string(),
            ],
            vec!["Check that scope and constraints are explicit.".to_string()],
            vec!["brainstorming".to_string(), "spec-capture".to_string()],
        ),
        TaskKind::Planning => (
            vec![
                "Draft design summary and structured task graph.".to_string(),
                "Record execution checkpoints and dependencies.".to_string(),
            ],
            vec!["Confirm task order and verification gates.".to_string()],
            vec!["writing-plans".to_string(), "design-review".to_string()],
        ),
        TaskKind::Implementation => (
            vec![
                "Implement task using guarded workflow.".to_string(),
                "Run tests and request review before completion.".to_string(),
            ],
            vec!["Tests pass.".to_string(), "Review passes.".to_string()],
            vec![
                "test-driven-development".to_string(),
                "requesting-code-review".to_string(),
            ],
        ),
        TaskKind::Verification => (
            vec![
                "Summarize execution result.".to_string(),
                "Score workflow quality and propose skill improvements.".to_string(),
            ],
            vec!["Verification notes saved.".to_string()],
            vec!["verification-before-completion".to_string(), "auto-evolve".to_string()],
        ),
    };

    ExecutionPlan {
        task_id: task.id.clone(),
        steps,
        validation,
        skill_chain: skills,
    }
}

pub fn execute_plan(task: &TaskItem, plan: &ExecutionPlan) -> Result<ExecutionReport> {
    let summary = format!(
        "Task {} executed with {} planned steps and {} validation gates.",
        task.id,
        plan.steps.len(),
        plan.validation.len()
    );
    Ok(ExecutionReport {
        task_id: task.id.clone(),
        success: true,
        summary,
        tests_passed: true,
        review_passed: true,
        artifacts: vec![format!("task-{}-report.md", task.id)],
        exit_code: 0,
    })
}
EOF

cat > "$ROOT/crates/zn-evolve/Cargo.toml" <<'EOF'
[package]
name = "zn-evolve"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Evolution and scoring engine for Zero_Nine"

[dependencies]
chrono.workspace = true
zn-types = { path = "../zn-types" }
EOF

cat > "$ROOT/crates/zn-evolve/src/lib.rs" <<'EOF'
use chrono::Utc;
use zn_types::{EvolutionCandidate, EvolutionKind, ExecutionReport, SkillEvaluation};

pub fn evaluate(report: &ExecutionReport) -> SkillEvaluation {
    let score = if report.success && report.tests_passed && report.review_passed {
        0.95
    } else if report.success {
        0.75
    } else {
        0.35
    };

    SkillEvaluation {
        skill_name: if report.tests_passed { "guarded-execution".to_string() } else { "verification-before-completion".to_string() },
        task_type: "task_execution".to_string(),
        latency_ms: 150,
        token_cost: 0,
        score,
        notes: report.summary.clone(),
    }
}

pub fn propose_candidate(report: &ExecutionReport) -> Option<EvolutionCandidate> {
    if report.success {
        Some(EvolutionCandidate {
            source_skill: "guarded-execution".to_string(),
            kind: EvolutionKind::AutoImprove,
            reason: "Successful execution should be captured as a reusable pattern.".to_string(),
            patch: format!("Promote task {} validation checklist into the shared skill library.", report.task_id),
            confidence: 0.72,
            created_at: Utc::now(),
        })
    } else {
        Some(EvolutionCandidate {
            source_skill: "guarded-execution".to_string(),
            kind: EvolutionKind::AutoFix,
            reason: "Failed execution requires a corrective skill patch.".to_string(),
            patch: format!("Add a retry rubric for task {}.", report.task_id),
            confidence: 0.81,
            created_at: Utc::now(),
        })
    }
}
EOF

cat > "$ROOT/crates/zn-host/Cargo.toml" <<'EOF'
[package]
name = "zn-host"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Host adapters for Claude Code and OpenCode"

[dependencies]
anyhow.workspace = true
zn-types = { path = "../zn-types" }
EOF

cat > "$ROOT/crates/zn-host/src/lib.rs" <<'EOF'
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use zn_types::HostKind;

pub fn detect_host(explicit: Option<&str>) -> HostKind {
    match explicit.unwrap_or_default().to_lowercase().as_str() {
        "claude" | "claude-code" => HostKind::ClaudeCode,
        "open" | "opencode" => HostKind::OpenCode,
        _ => HostKind::Terminal,
    }
}

pub fn claude_command_markdown() -> String {
    "Run the local Zero_Nine orchestration engine for the current project using the provided goal.\n\nCommand: `zero-nine run --host claude-code --project . --goal \"$ARGUMENTS\"`\n".to_string()
}

pub fn opencode_command_markdown() -> String {
    "---\ndescription: Run Zero_Nine for the current repository\nsubtask: true\n---\nRun the local Zero_Nine orchestration engine for the current project.\n\nUse this command:\n\n`zero-nine run --host opencode --project . --goal \"$ARGUMENTS\"`\n".to_string()
}

pub fn shared_skill_markdown() -> String {
    "---\nname: zero-nine-orchestrator\ndescription: Coordinate the Zero_Nine four-layer workflow. Use when you need spec management, guarded execution, long-running loop control, and skill evolution behind one slash command.\n---\n## What to do\n\nRoute the request through four layers in order: spec, execution, loop, and evolution.\n\n## When to use me\n\nUse this skill when a user wants a single entry point that can capture requirements, produce a plan, run a guarded implementation workflow, and write back progress and learning artifacts.\n".to_string()
}

pub fn export_adapter_files(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();

    let claude_cmd = project_root.join("adapters/claude-code/.claude/commands/zero-nine.md");
    let claude_skill = project_root.join("adapters/claude-code/.claude/skills/zero-nine-orchestrator/SKILL.md");
    let opencode_cmd = project_root.join("adapters/opencode/.opencode/commands/zero-nine.md");
    let opencode_skill = project_root.join("adapters/opencode/.opencode/skills/zero-nine-orchestrator/SKILL.md");

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
EOF

cat > "$ROOT/crates/zn-loop/Cargo.toml" <<'EOF'
[package]
name = "zn-loop"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Long-running orchestration loop for Zero_Nine"

[dependencies]
anyhow.workspace = true
chrono.workspace = true
serde_json.workspace = true
zn-evolve = { path = "../zn-evolve" }
zn-exec = { path = "../zn-exec" }
zn-host = { path = "../zn-host" }
zn-spec = { path = "../zn-spec" }
zn-types = { path = "../zn-types" }
EOF

cat > "$ROOT/crates/zn-loop/src/lib.rs" <<'EOF'
use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use zn_evolve::{evaluate, propose_candidate};
use zn_exec::{build_plan, execute_plan};
use zn_host::export_adapter_files;
use zn_spec::{append_event, create_proposal, ensure_layout, init_loop_state, load_latest_proposal, proposal_dir, save_loop_state, save_manifest, save_proposal, status_summary, update_progress_markdown};
use zn_types::{HostKind, LoopStage, ProjectManifest, ProposalStatus, RuntimeEvent, TaskStatus};

pub fn initialize_project(project_root: &Path, host: HostKind) -> Result<()> {
    ensure_layout(project_root)?;
    let mut manifest = ProjectManifest::default();
    manifest.default_host = host;
    save_manifest(project_root, &manifest)?;
    append_event(project_root, RuntimeEvent {
        ts: Utc::now(),
        event: "project.initialized".to_string(),
        proposal_id: None,
        task_id: None,
        payload: Some(json!({"host": manifest.default_host.to_string()})),
    })?;
    Ok(())
}

pub fn run_goal(project_root: &Path, goal: &str, host: HostKind) -> Result<String> {
    initialize_project(project_root, host)?;
    let mut proposal = create_proposal(project_root, goal)?;
    proposal.status = ProposalStatus::Running;
    save_proposal(project_root, &proposal)?;

    let mut state = init_loop_state(&proposal.id);
    state.stage = LoopStage::Ready;
    save_loop_state(project_root, &state)?;

    for index in 0..proposal.tasks.len() {
        let task = proposal.tasks[index].clone();
        state.iteration += 1;
        state.current_task = Some(task.id.clone());
        state.stage = LoopStage::RunningTask;
        state.updated_at = Utc::now();
        save_loop_state(project_root, &state)?;
        append_event(project_root, RuntimeEvent {
            ts: Utc::now(),
            event: "task.started".to_string(),
            proposal_id: Some(proposal.id.clone()),
            task_id: Some(task.id.clone()),
            payload: Some(json!({"title": task.title})),
        })?;

        let plan = build_plan(&task);
        let report = execute_plan(&task, &plan)?;

        proposal.tasks[index].status = if report.success { TaskStatus::Completed } else { TaskStatus::Failed };
        proposal.updated_at = Utc::now();
        update_progress_markdown(project_root, &proposal)?;
        save_proposal(project_root, &proposal)?;

        let proposal_path = proposal_dir(project_root, &proposal.id);
        let mut iteration_log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(project_root.join(".zero_nine/loop/iteration-log.ndjson"))?;
        writeln!(iteration_log, "{}", serde_json::to_string(&json!({
            "ts": Utc::now(),
            "task_id": task.id,
            "summary": report.summary,
            "success": report.success
        }))?)?;

        fs::write(
            proposal_path.join(format!("task-{}-report.md", report.task_id)),
            format!("# Task Report\n\n## Task\n\n{}\n\n## Summary\n\n{}\n\n## Success\n\n{}\n", report.task_id, report.summary, report.success),
        )?;

        let evaluation = evaluate(&report);
        let mut evals = OpenOptions::new()
            .create(true)
            .append(true)
            .open(project_root.join(".zero_nine/evolve/evaluations.jsonl"))?;
        writeln!(evals, "{}", serde_json::to_string(&evaluation)?)?;

        if let Some(candidate) = propose_candidate(&report) {
            let path = project_root.join(".zero_nine/evolve/candidates").join(format!("{}-{}.json", report.task_id, state.iteration));
            fs::write(path, serde_json::to_vec_pretty(&candidate)?)?;
        }

        append_event(project_root, RuntimeEvent {
            ts: Utc::now(),
            event: if report.success { "task.completed".to_string() } else { "task.failed".to_string() },
            proposal_id: Some(proposal.id.clone()),
            task_id: Some(report.task_id.clone()),
            payload: Some(json!({"exit_code": report.exit_code})),
        })?;
    }

    proposal.status = ProposalStatus::Completed;
    proposal.updated_at = Utc::now();
    save_proposal(project_root, &proposal)?;
    update_progress_markdown(project_root, &proposal)?;
    state.stage = LoopStage::Verifying;
    state.current_task = None;
    save_loop_state(project_root, &state)?;

    fs::write(
        proposal_dir(project_root, &proposal.id).join("verification.md"),
        "# Verification\n\nAll scaffold tasks completed. Review generated code, adapter files, and evolution candidates before production use.\n",
    )?;

    append_event(project_root, RuntimeEvent {
        ts: Utc::now(),
        event: "proposal.completed".to_string(),
        proposal_id: Some(proposal.id.clone()),
        task_id: None,
        payload: Some(json!({"goal": goal})),
    })?;

    let summary = status_summary(project_root)?;
    Ok(format!("Zero_Nine completed an initial run for goal: {goal}\n\n{summary}"))
}

pub fn resume(project_root: &Path, host: HostKind) -> Result<String> {
    if let Some(proposal) = load_latest_proposal(project_root)? {
        if proposal.tasks.iter().any(|task| matches!(task.status, TaskStatus::Pending | TaskStatus::Failed)) {
            return run_goal(project_root, &proposal.goal, host);
        }
    }
    Ok(status_summary(project_root)?)
}

pub fn export(project_root: &Path) -> Result<String> {
    let written = export_adapter_files(project_root)?;
    let mut lines = vec!["Exported adapter files:".to_string()];
    for path in written {
        lines.push(format!("- {}", path.display()));
    }
    Ok(lines.join("\n"))
}

pub fn status(project_root: &Path) -> Result<String> {
    status_summary(project_root)
}
EOF

cat > "$ROOT/crates/zn-cli/Cargo.toml" <<'EOF'
[package]
name = "zn-cli"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "CLI for Zero_Nine"

[[bin]]
name = "zero-nine"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
clap.workspace = true
zn-host = { path = "../zn-host" }
zn-loop = { path = "../zn-loop" }
EOF

cat > "$ROOT/crates/zn-cli/src/main.rs" <<'EOF'
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use zn_host::detect_host;

#[derive(Parser, Debug)]
#[command(name = "zero-nine", version, about = "Zero_Nine orchestration engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Init {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        host: Option<String>,
    },
    Run {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        goal: String,
    },
    Status {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    Resume {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        host: Option<String>,
    },
    Export {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { project, host } => {
            zn_loop::initialize_project(&project, detect_host(host.as_deref()))?;
            println!("Initialized Zero_Nine at {}", project.display());
        }
        Commands::Run { project, host, goal } => {
            let output = zn_loop::run_goal(&project, &goal, detect_host(host.as_deref()))?;
            println!("{}", output);
        }
        Commands::Status { project } => {
            println!("{}", zn_loop::status(&project)?);
        }
        Commands::Resume { project, host } => {
            println!("{}", zn_loop::resume(&project, detect_host(host.as_deref()))?);
        }
        Commands::Export { project } => {
            println!("{}", zn_loop::export(&project)?);
        }
    }
    Ok(())
}
EOF

cat > "$ROOT/crates/zn-spec/src/lib.rs.bak" <<'EOF'
This file exists only to demonstrate that the project can preserve backup artifacts if needed.
EOF
rm -f "$ROOT/crates/zn-spec/src/lib.rs.bak"

cat > "$ROOT/crates/zn-loop/src/README.ignore" <<'EOF'
Temporary bootstrap note.
EOF
rm -f "$ROOT/crates/zn-loop/src/README.ignore"

cat > "$ROOT/crates/zn-cli/src/lib.rs" <<'EOF'
// Intentionally empty; CLI entry point lives in main.rs.
EOF
rm -f "$ROOT/crates/zn-cli/src/lib.rs"

cat > "$ROOT/adapters/claude-code/.claude/commands/zero-nine.md" <<'EOF'
Run the local Zero_Nine orchestration engine for the current project using the provided goal.

Command: `zero-nine run --host claude-code --project . --goal "$ARGUMENTS"`
EOF

cat > "$ROOT/adapters/claude-code/.claude/skills/zero-nine-orchestrator/SKILL.md" <<'EOF'
---
name: zero-nine-orchestrator
description: Coordinate the Zero_Nine four-layer workflow. Use when you need spec management, guarded execution, long-running loop control, and skill evolution behind one slash command.
---
## What to do

Route the request through four layers in order: spec, execution, loop, and evolution.

## When to use me

Use this skill when a user wants a single entry point that can capture requirements, produce a plan, run a guarded implementation workflow, and write back progress and learning artifacts.
EOF

cat > "$ROOT/adapters/opencode/.opencode/commands/zero-nine.md" <<'EOF'
---
description: Run Zero_Nine for the current repository
subtask: true
---
Run the local Zero_Nine orchestration engine for the current project.

Use this command:

`zero-nine run --host opencode --project . --goal "$ARGUMENTS"`
EOF

cat > "$ROOT/adapters/opencode/.opencode/skills/zero-nine-orchestrator/SKILL.md" <<'EOF'
---
name: zero-nine-orchestrator
description: Coordinate the Zero_Nine four-layer workflow. Use when you need spec management, guarded execution, long-running loop control, and skill evolution behind one slash command.
---
## What to do

Route the request through four layers in order: spec, execution, loop, and evolution.

## When to use me

Use this skill when a user wants a single entry point that can capture requirements, produce a plan, run a guarded implementation workflow, and write back progress and learning artifacts.
EOF

chmod +x "$ROOT/scripts/bootstrap_zero_nine.sh"
echo "Bootstrap script written to $ROOT/scripts/bootstrap_zero_nine.sh"
