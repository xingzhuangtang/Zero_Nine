pub mod memory_tool;
pub mod session_search;
pub mod skill_format;
pub mod skill_manager;

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use zn_types::{
    default_spec_schema_version, BrainstormSession, ExecutionStrategy, LoopStage, LoopState,
    ProgressRecord, ProjectManifest, Proposal, ProposalStatus, RequirementPacket, RuntimeEvent,
    SpecBundle, SpecValidationIssue, SpecValidationReport, SpecValidationSeverity, TaskContract,
    TaskDependencyEdge, TaskGraph, TaskItem, TaskStatus,
};

pub fn zero_nine_dir(project_root: &Path) -> PathBuf {
    project_root.join(".zero_nine")
}

pub fn brainstorm_dir(project_root: &Path) -> PathBuf {
    zero_nine_dir(project_root).join("brainstorm")
}

pub fn ensure_layout(project_root: &Path) -> Result<()> {
    let root = zero_nine_dir(project_root);
    for rel in [
        "proposals",
        "archive",
        "brainstorm/sessions",
        "specs/patterns",
        "loop/locks",
        "evolve/skills",
        "evolve/candidates",
        "evolve/library",
        "runtime/cache",
        "../.issues",
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

pub fn load_manifest(project_root: &Path) -> Result<Option<ProjectManifest>> {
    let path = zero_nine_dir(project_root).join("manifest.json");
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(path)?;
    let manifest: ProjectManifest = serde_json::from_str(&data)?;
    Ok(Some(manifest))
}

pub fn default_tasks(goal: &str) -> Vec<TaskItem> {
    vec![
        TaskItem {
            id: "1".to_string(),
            title: "Superpowers brainstorming to clarify requirement".to_string(),
            description: format!(
                "Use a Socratic brainstorming flow to clarify the true user intent, scope, constraints, and acceptance criteria for: {goal}"
            ),
            status: TaskStatus::Pending,
            depends_on: vec![],
            kind: Some("brainstorming".to_string()),
            contract: TaskContract {
                acceptance_criteria: vec![
                    "All clarification questions are answered with actionable detail.".to_string(),
                    "The brainstorm verdict reaches Ready before execution is allowed.".to_string(),
                ],
                deliverables: vec![
                    "brainstorm-session.json".to_string(),
                    "brainstorm-session.md".to_string(),
                ],
                verification_points: vec![
                    "No unresolved clarification question remains in the requirement packet."
                        .to_string(),
                ],
            },
            max_retries: None,
            preconditions: vec![],
        },
        TaskItem {
            id: "2".to_string(),
            title: "Write OpenSpec proposal design tasks and DAG".to_string(),
            description:
                "Translate the requirement packet into proposal.md, requirements.md, acceptance.md, design.md, tasks.md, and dag.json."
                    .to_string(),
            status: TaskStatus::Pending,
            depends_on: vec!["1".to_string()],
            kind: Some("spec_capture".to_string()),
            contract: TaskContract {
                acceptance_criteria: vec![
                    "All OpenSpec core files exist and match the clarified goal.".to_string(),
                    "The task DAG reflects dependencies and task contracts explicitly.".to_string(),
                ],
                deliverables: vec![
                    "proposal.md".to_string(),
                    "requirements.md".to_string(),
                    "acceptance.md".to_string(),
                    "design.md".to_string(),
                    "tasks.md".to_string(),
                    "dag.json".to_string(),
                ],
                verification_points: vec![
                    "Spec validation report contains no error severity issue.".to_string(),
                ],
            },
            max_retries: Some(1),
            preconditions: vec![
                "brainstorming_verdict_ready".to_string(),
            ],
        },
        TaskItem {
            id: "3".to_string(),
            title: "Run writing-plans and prepare isolated execution workspace".to_string(),
            description:
                "Refine the executable plan for the next implementation loop and prepare worktree or sandbox strategy."
                    .to_string(),
            status: TaskStatus::Pending,
            depends_on: vec!["2".to_string()],
            kind: Some("planning".to_string()),
            contract: TaskContract {
                acceptance_criteria: vec![
                    "Execution plan enumerates steps, quality gates, risks, and deliverables."
                        .to_string(),
                    "Workspace strategy is explicit before code execution begins.".to_string(),
                ],
                deliverables: vec![
                    "execution envelope".to_string(),
                    "workspace plan".to_string(),
                ],
                verification_points: vec![
                    "Workspace preparation summary is recorded into the task report.".to_string(),
                ],
            },
            max_retries: Some(2),
            preconditions: vec![
                "planning_artifacts_exist".to_string(),
            ],
        },
        TaskItem {
            id: "4".to_string(),
            title: "Execute guarded implementation verification and branch finishing".to_string(),
            description:
                "Simulate the guarded execution chain with subagent briefs, TDD gates, review, verification, progress updates, and finish-branch options."
                    .to_string(),
            status: TaskStatus::Pending,
            depends_on: vec!["3".to_string()],
            kind: Some("execution".to_string()),
            contract: TaskContract {
                acceptance_criteria: vec![
                    "Verification actions emit evidence and a machine-readable verdict.".to_string(),
                    "Finish-branch automation respects explicit confirmation policy.".to_string(),
                ],
                deliverables: vec![
                    "verification.md".to_string(),
                    "task reports".to_string(),
                    "runtime artifacts".to_string(),
                ],
                verification_points: vec![
                    "Required tests and review gates pass before completion.".to_string(),
                ],
            },
            max_retries: Some(3),
            preconditions: vec![
                "worktree_isolated".to_string(),
                "plan_reviewed".to_string(),
            ],
        },
    ]
}

pub fn create_requirement_packet(goal: &str) -> RequirementPacket {
    RequirementPacket {
        schema_version: default_spec_schema_version(),
        user_goal: goal.to_string(),
        problem_statement: format!(
            "Transform the user request into a controllable engineering workflow that can start from one sentence and continue through spec, execution, loop orchestration, and evolution."
        ),
        scope_in: vec![
            "Capture the real user intent behind the initial goal.".to_string(),
            "Produce persistent specification artifacts for later execution.".to_string(),
            "Prepare a resumable loop with quality gates and progress tracking.".to_string(),
            "Export host-facing plugin entry points for Claude Code and OpenCode.".to_string(),
        ],
        scope_out: vec![
            "Do not assume cloud synchronization is already available.".to_string(),
            "Do not merge branches automatically without an explicit later confirmation step."
                .to_string(),
        ],
        constraints: vec![
            "Prefer plugin-first host integration while preserving a path toward an independent CLI and SDK."
                .to_string(),
            "Keep artifacts explicit, file-based, and inspectable by humans.".to_string(),
            "Preserve separation of concerns across spec, execution, loop, and evolution layers."
                .to_string(),
        ],
        acceptance_criteria: vec![
            "The requirement packet is explicit enough to drive planning without reinterpretation."
                .to_string(),
            "OpenSpec-style files exist and reflect the clarified goal.".to_string(),
            "Loop progress can be resumed from written state and progress files.".to_string(),
            "Host adapters expose slash-command entry points for Claude Code and OpenCode."
                .to_string(),
        ],
        risks: vec![
            "The initial user goal may still hide missing business context.".to_string(),
            "Execution can remain scaffold-like if plans are not further refined into actionable work units."
                .to_string(),
            "Without worktree isolation, implementation tasks may affect the main branch prematurely."
                .to_string(),
        ],
        next_questions: vec![
            "Should the first production implementation default to Git worktree for coding tasks?"
                .to_string(),
            "What merge policy should finishing-a-development-branch enforce by default?"
                .to_string(),
            "Which host should be treated as the primary runtime for the next iteration: Claude Code or OpenCode?"
                .to_string(),
        ],
        source_brainstorm_session_id: None,
        clarified: false,
    }
}

pub fn create_proposal(project_root: &Path, goal: &str) -> Result<Proposal> {
    ensure_layout(project_root)?;
    let packet = create_requirement_packet(goal);
    create_proposal_from_packet(
        project_root,
        goal,
        &packet,
        Some(
            "Zero_Nine generated an OpenSpec-oriented specification bundle from a Superpowers-style brainstorming entry point."
                .to_string(),
        ),
        None,
    )
}

pub fn create_proposal_from_brainstorm(
    project_root: &Path,
    session: &BrainstormSession,
) -> Result<Proposal> {
    ensure_layout(project_root)?;
    let packet = requirement_packet_from_brainstorm(session);
    create_proposal_from_packet(
        project_root,
        &session.goal,
        &packet,
        Some(format!(
            "Zero_Nine converged a multi-turn Brainstorming session ({}) into an OpenSpec-oriented specification bundle.",
            session.id
        )),
        Some(session),
    )
}

fn create_proposal_from_packet(
    project_root: &Path,
    goal: &str,
    packet: &RequirementPacket,
    design_summary: Option<String>,
    session: Option<&BrainstormSession>,
) -> Result<Proposal> {
    let slug = zn_types::slugify_goal(goal);
    let id = format!("{}-{}", Utc::now().format("%Y%m%d%H%M%S"), slug);
    let proposal_dir = zero_nine_dir(project_root).join("proposals").join(&id);
    fs::create_dir_all(proposal_dir.join("artifacts"))?;

    let now = Utc::now();

    // M1: Build structured spec contract from RequirementPacket
    let mut acceptance_criteria = Vec::new();
    for (idx, criterion) in packet.acceptance_criteria.iter().enumerate() {
        acceptance_criteria.push(zn_types::AcceptanceCriterion {
            id: format!("ac-{}", idx + 1),
            description: criterion.clone(),
            verification_method: zn_types::VerificationMethod::AutomatedTest,
            priority: zn_types::Priority::High,
            status: zn_types::CriterionStatus::Pending,
        });
    }

    let mut constraints = Vec::new();
    for (idx, constraint) in packet.constraints.iter().enumerate() {
        constraints.push(zn_types::Constraint {
            id: format!("c-{}", idx + 1),
            category: zn_types::ConstraintCategory::Technical,
            description: constraint.clone(),
            rationale: None,
            enforced: true,
        });
    }

    let mut risks = Vec::new();
    for (idx, risk) in packet.risks.iter().enumerate() {
        risks.push(zn_types::Risk {
            id: format!("r-{}", idx + 1),
            description: risk.clone(),
            probability: zn_types::RiskProbability::Medium,
            impact: zn_types::RiskImpact::Medium,
            mitigation: None,
            owner: None,
        });
    }

    let proposal = Proposal {
        schema_version: default_spec_schema_version(),
        id: id.clone(),
        title: goal.to_string(),
        goal: goal.to_string(),
        status: if packet.next_questions.is_empty() {
            ProposalStatus::Ready
        } else {
            ProposalStatus::Draft
        },
        created_at: now,
        updated_at: now,
        design_summary,
        source_brainstorm_session_id: session.map(|item| item.id.clone()),
        source_issue_number: None,
        source_repo: None,
        source_type: None,

        // M1: Structured spec contract fields
        problem_statement: Some(packet.problem_statement.clone()),
        scope_in: packet.scope_in.clone(),
        scope_out: packet.scope_out.clone(),
        constraints,
        acceptance_criteria,
        risks,
        dependencies: Vec::new(),
        non_goals: Vec::new(),
        execution_strategy: Some(ExecutionStrategy::LinearSequential),

        tasks: default_tasks(goal),
    };

    save_proposal(project_root, &proposal)?;
    write_requirement_packet(project_root, &proposal.id, packet)?;
    write_core_spec_files(project_root, &proposal, packet)?;
    if let Some(session) = session {
        write_brainstorm_session_files(project_root, &proposal.id, session)?;
    }
    append_event(
        project_root,
        RuntimeEvent {
            ts: Utc::now(),
            event: if session.is_some() {
                "proposal.created.from_brainstorm".to_string()
            } else {
                "proposal.created".to_string()
            },
            proposal_id: Some(proposal.id.clone()),
            task_id: None,
            payload: Some(json!({
                "goal": goal,
                "next_questions": packet.next_questions.clone(),
                "brainstorm_session_id": session.map(|item| item.id.clone()),
            })),
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            latency_ms: None,
            metadata: None,
        },
    )?;
    Ok(proposal)
}

pub fn proposal_dir(project_root: &Path, proposal_id: &str) -> PathBuf {
    zero_nine_dir(project_root)
        .join("proposals")
        .join(proposal_id)
}

pub fn save_brainstorm_session(
    project_root: &Path,
    session: &BrainstormSession,
) -> Result<PathBuf> {
    ensure_layout(project_root)?;
    let path = brainstorm_dir(project_root)
        .join("sessions")
        .join(format!("{}.json", session.id));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(session)?)?;
    fs::write(
        brainstorm_dir(project_root).join("latest-session.json"),
        serde_json::to_vec_pretty(session)?,
    )?;
    fs::write(
        brainstorm_dir(project_root).join("latest-session.md"),
        render_brainstorm_session_markdown(session),
    )?;
    Ok(path)
}

pub fn load_latest_brainstorm_session(project_root: &Path) -> Result<Option<BrainstormSession>> {
    let path = brainstorm_dir(project_root).join("latest-session.json");
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(path)?;
    let session: BrainstormSession = serde_json::from_str(&data)?;
    Ok(Some(session))
}

pub fn load_brainstorm_session(
    project_root: &Path,
    session_id: &str,
) -> Result<Option<BrainstormSession>> {
    let path = brainstorm_dir(project_root)
        .join("sessions")
        .join(format!("{}.json", session_id));
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(path)?;
    let session: BrainstormSession = serde_json::from_str(&data)?;
    Ok(Some(session))
}

pub fn requirement_packet_from_brainstorm(session: &BrainstormSession) -> RequirementPacket {
    let problem_statement = answer_or_default(
        session,
        "problem_statement",
        format!(
            "Refine the goal '{}' into a controlled engineering workflow with explicit scope, constraints, acceptance criteria, and execution gates.",
            session.goal
        ),
    );
    let scope_in = answer_list_or_default(
        session,
        "scope_in",
        vec![
            "Clarify the intended outcome and success criteria.".to_string(),
            "Translate confirmed needs into OpenSpec-style artifacts.".to_string(),
            "Prepare the later execution chain with explicit quality gates.".to_string(),
        ],
    );
    let scope_out = answer_list_or_default(
        session,
        "scope_out",
        vec![
            "Anything not explicitly confirmed during brainstorming remains outside the first implementation loop."
                .to_string(),
        ],
    );
    let constraints = answer_list_or_default(
        session,
        "constraints",
        vec![
            "Keep the workflow inspectable, file-based, and resumable.".to_string(),
            "Preserve compatibility with Claude Code CLI and OpenCode CLI slash-command entry points."
                .to_string(),
        ],
    );
    let acceptance_criteria = answer_list_or_default(
        session,
        "acceptance_criteria",
        vec![
            "The clarified goal is explicit enough to drive planning without guesswork."
                .to_string(),
            "OpenSpec artifacts reflect the clarified requirement contract.".to_string(),
        ],
    );
    let risks = answer_list_or_default(
        session,
        "risks",
        vec![
            "Important context may still be missing if answers remain vague.".to_string(),
            "Implementation quality will degrade if unresolved questions are ignored.".to_string(),
        ],
    );
    let next_questions = unresolved_questions(session);

    RequirementPacket {
        schema_version: default_spec_schema_version(),
        user_goal: session.goal.clone(),
        problem_statement,
        scope_in,
        scope_out,
        constraints,
        acceptance_criteria,
        risks,
        next_questions: next_questions.clone(),
        source_brainstorm_session_id: Some(session.id.clone()),
        clarified: next_questions.is_empty()
            && matches!(session.verdict, zn_types::BrainstormVerdict::Ready),
    }
}

pub fn spec_bundle(project_root: &Path, proposal_id: &str) -> SpecBundle {
    let dir = proposal_dir(project_root, proposal_id);
    SpecBundle {
        proposal_path: dir.join("proposal.md").display().to_string(),
        requirements_path: dir.join("requirements.md").display().to_string(),
        acceptance_path: dir.join("acceptance.md").display().to_string(),
        design_path: dir.join("design.md").display().to_string(),
        tasks_path: dir.join("tasks.md").display().to_string(),
        dag_path: dir.join("dag.json").display().to_string(),
        progress_path: dir.join("progress.txt").display().to_string(),
        verification_path: dir.join("verification.md").display().to_string(),
    }
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
    let mut output = String::from(
        "# Tasks\n\n| ID | Title | Status | Depends On |\n| --- | --- | --- | --- |\n",
    );
    for task in tasks {
        let deps = if task.depends_on.is_empty() {
            "-".to_string()
        } else {
            task.depends_on.join(", ")
        };
        output.push_str(&format!(
            "| {} | {} | {:?} | {} |\n",
            task.id, task.title, task.status, deps
        ));
    }
    output
}

pub fn write_requirement_packet(
    project_root: &Path,
    proposal_id: &str,
    packet: &RequirementPacket,
) -> Result<()> {
    let dir = proposal_dir(project_root, proposal_id);
    fs::write(
        dir.join("requirements.md"),
        render_requirements_markdown(packet),
    )?;
    fs::write(
        dir.join("acceptance.md"),
        render_acceptance_markdown(packet),
    )?;
    fs::write(
        dir.join("clarifications.md"),
        render_clarifications_markdown(packet),
    )?;
    fs::write(
        dir.join("requirement-packet.json"),
        serde_json::to_vec_pretty(packet)?,
    )?;
    Ok(())
}

pub fn write_progress_files(project_root: &Path, proposal: &Proposal) -> Result<()> {
    let completed: Vec<String> = proposal
        .tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Completed))
        .map(|task| task.id.clone())
        .collect();
    let pending: Vec<String> = proposal
        .tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Pending | TaskStatus::Running))
        .map(|task| task.id.clone())
        .collect();
    let blocked: Vec<String> = proposal
        .tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Blocked | TaskStatus::Failed))
        .map(|task| task.id.clone())
        .collect();
    let runnable = runnable_task_ids(proposal);
    let blocked_details = blocked_task_details(proposal);
    let scheduler_summary = format!(
        "{} runnable, {} blocked, {} pending under current DAG constraints",
        runnable.len(),
        blocked.len(),
        pending.len()
    );

    let record = ProgressRecord {
        proposal_id: proposal.id.clone(),
        completed: completed.clone(),
        pending: pending.clone(),
        blocked: blocked.clone(),
        runnable: runnable.clone(),
        blocked_details: blocked_details.clone(),
        scheduler_summary: scheduler_summary.clone(),
        summary: format!(
            "{} completed, {} pending, {} blocked, {} runnable",
            completed.len(),
            pending.len(),
            blocked.len(),
            runnable.len()
        ),
    };

    let dir = proposal_dir(project_root, &proposal.id);
    fs::write(
        dir.join("progress.json"),
        serde_json::to_vec_pretty(&record)?,
    )?;
    fs::write(
        dir.join("progress.txt"),
        format!(
            "proposal_id={}\ncompleted={}\npending={}\nblocked={}\nrunnable={}\nscheduler_summary={}\nblocked_details={}\nsummary={}\n",
            record.proposal_id,
            if completed.is_empty() {
                "-".to_string()
            } else {
                completed.join(",")
            },
            if pending.is_empty() {
                "-".to_string()
            } else {
                pending.join(",")
            },
            if blocked.is_empty() {
                "-".to_string()
            } else {
                blocked.join(",")
            },
            if runnable.is_empty() {
                "-".to_string()
            } else {
                runnable.join(",")
            },
            scheduler_summary,
            if blocked_details.is_empty() {
                "-".to_string()
            } else {
                blocked_details.join(" | ")
            },
            record.summary
        ),
    )?;
    Ok(())
}

fn runnable_task_ids(proposal: &Proposal) -> Vec<String> {
    let completed = proposal
        .tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Completed))
        .map(|task| task.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    proposal
        .tasks
        .iter()
        .filter(|task| {
            matches!(
                task.status,
                TaskStatus::Pending | TaskStatus::Running | TaskStatus::Failed
            )
        })
        .filter(|task| {
            task.depends_on
                .iter()
                .all(|dependency| completed.contains(dependency.as_str()))
        })
        .map(|task| task.id.clone())
        .collect()
}

fn blocked_task_details(proposal: &Proposal) -> Vec<String> {
    let completed = proposal
        .tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Completed))
        .map(|task| task.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    proposal
        .tasks
        .iter()
        .filter_map(|task| {
            let unresolved = task
                .depends_on
                .iter()
                .filter(|dependency| !completed.contains(dependency.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            if !unresolved.is_empty() {
                Some(format!("{} waiting_on {}", task.id, unresolved.join(",")))
            } else if matches!(task.status, TaskStatus::Blocked | TaskStatus::Failed) {
                Some(format!("{} in_terminal_state {:?}", task.id, task.status))
            } else {
                None
            }
        })
        .collect()
}

pub fn update_progress_markdown(project_root: &Path, proposal: &Proposal) -> Result<()> {
    let dir = proposal_dir(project_root, &proposal.id);
    fs::write(dir.join("tasks.md"), render_tasks_markdown(&proposal.tasks))?;
    write_progress_files(project_root, proposal)?;
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
        max_iterations: None,
        iteration_start: Utc::now(),
        elapsed_seconds: 0,
        transition_history: Vec::new(),
    }
}

pub fn status_summary(project_root: &Path) -> Result<String> {
    let manifest_path = zero_nine_dir(project_root).join("manifest.json");
    let proposal = load_latest_proposal(project_root)?;
    let state = load_loop_state(project_root)?;
    let brainstorm = load_latest_brainstorm_session(project_root)?;
    let mut lines = Vec::new();
    lines.push(format!(
        "manifest: {}",
        if manifest_path.exists() {
            "present"
        } else {
            "missing"
        }
    ));
    if let Some(session) = brainstorm {
        lines.push(format!("brainstorm_session: {}", session.id));
        lines.push(format!("brainstorm_verdict: {:?}", session.verdict));
    } else {
        lines.push("brainstorm_session: none".to_string());
    }
    if let Some(proposal) = proposal {
        let done = proposal
            .tasks
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::Completed))
            .count();
        let runnable = runnable_task_ids(&proposal);
        let blocked_details = blocked_task_details(&proposal);
        lines.push(format!("proposal: {}", proposal.id));
        lines.push(format!("goal: {}", proposal.goal));
        lines.push(format!("status: {:?}", proposal.status));
        lines.push(format!("tasks: {done}/{} completed", proposal.tasks.len()));
        lines.push(format!(
            "scheduler: {} runnable / {} blocked / {} pending",
            runnable.len(),
            proposal
                .tasks
                .iter()
                .filter(|t| matches!(t.status, TaskStatus::Blocked | TaskStatus::Failed))
                .count(),
            proposal
                .tasks
                .iter()
                .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::Running))
                .count(),
        ));
        lines.push(format!(
            "runnable_tasks: {}",
            if runnable.is_empty() {
                "none".to_string()
            } else {
                runnable.join(", ")
            }
        ));
        lines.push(format!(
            "blocked_details: {}",
            if blocked_details.is_empty() {
                "none".to_string()
            } else {
                blocked_details.join("; ")
            }
        ));
        let bundle = spec_bundle(project_root, &proposal.id);
        let subagent_runtime_dir = zero_nine_dir(project_root).join("runtime/subagents");
        let current_task_dir = state
            .as_ref()
            .and_then(|loop_state| loop_state.current_task.as_deref())
            .map(|task_id| subagent_runtime_dir.join(format!("task-{}", task_id)))
            .unwrap_or_else(|| subagent_runtime_dir.join("latest"));
        lines.push(format!("progress_file: {}", bundle.progress_path));
        lines.push(format!(
            "subagent_runtime_dir: {}",
            subagent_runtime_dir.display()
        ));
        lines.push(format!(
            "subagent_recovery_ledgers: {}",
            current_task_dir
                .join("subagent-recovery-ledger.json")
                .display()
        ));
        lines.push(format!(
            "subagent_replay_scripts: {}",
            current_task_dir.display()
        ));
    } else {
        lines.push("proposal: none".to_string());
    }
    if let Some(state) = state {
        lines.push(format!("loop_stage: {:?}", state.stage));
        lines.push(format!("iteration: {}", state.iteration));
        lines.push(format!(
            "current_task: {}",
            state.current_task.unwrap_or_else(|| "none".to_string())
        ));
    } else {
        lines.push("loop_stage: none".to_string());
    }
    Ok(lines.join("\n"))
}

pub fn write_brainstorm_session_files(
    project_root: &Path,
    proposal_id: &str,
    session: &BrainstormSession,
) -> Result<()> {
    let dir = proposal_dir(project_root, proposal_id);
    fs::write(
        dir.join("brainstorm-session.json"),
        serde_json::to_vec_pretty(session)?,
    )?;
    fs::write(
        dir.join("brainstorm-session.md"),
        render_brainstorm_session_markdown(session),
    )?;
    Ok(())
}

fn write_core_spec_files(
    project_root: &Path,
    proposal: &Proposal,
    packet: &RequirementPacket,
) -> Result<()> {
    let dir = proposal_dir(project_root, &proposal.id);
    fs::write(
        dir.join("proposal.md"),
        render_proposal_markdown(proposal, packet),
    )?;
    fs::write(
        dir.join("design.md"),
        render_design_markdown(proposal, packet),
    )?;
    fs::write(dir.join("tasks.md"), render_tasks_markdown(&proposal.tasks))?;
    fs::write(
        dir.join("dag.json"),
        serde_json::to_vec_pretty(&build_task_graph(proposal))?,
    )?;
    fs::write(
        dir.join("verification.md"),
        "# Verification\n\nPending verification. The loop must complete execution gates before this proposal can be marked production-ready.\n",
    )?;
    write_progress_files(project_root, proposal)?;
    write_spec_validation_report(project_root, proposal)?;
    Ok(())
}

pub fn validate_proposal_spec(
    project_root: &Path,
    proposal: &Proposal,
) -> Result<SpecValidationReport> {
    let dir = proposal_dir(project_root, &proposal.id);
    let bundle = spec_bundle(project_root, &proposal.id);
    let mut issues = Vec::new();

    if proposal.schema_version.trim().is_empty() {
        issues.push(validation_issue(
            SpecValidationSeverity::Error,
            "proposal.schema_version_missing",
            "proposal.json",
            "Proposal schema_version must be present.",
            Some("Set schema_version to \"zero_nine.stage1.v1\" in proposal.json."),
        ));
    }

    if proposal.goal.trim().is_empty() {
        issues.push(validation_issue(
            SpecValidationSeverity::Error,
            "proposal.goal_missing",
            "proposal.json",
            "Proposal goal must not be empty.",
            Some("Provide a non-empty goal string describing the intended outcome."),
        ));
    }

    if proposal.tasks.is_empty() {
        issues.push(validation_issue(
            SpecValidationSeverity::Error,
            "proposal.tasks_missing",
            "proposal.json",
            "Proposal must contain at least one task.",
            Some("Add at least one task to the proposal tasks array."),
        ));
    }

    for required in [
        &bundle.proposal_path,
        &bundle.requirements_path,
        &bundle.acceptance_path,
        &bundle.design_path,
        &bundle.tasks_path,
        &bundle.dag_path,
    ] {
        if !Path::new(required).exists() {
            issues.push(validation_issue(
                SpecValidationSeverity::Error,
                "spec.file_missing",
                required,
                "Required spec artifact is missing.",
                Some("Run the spec generation step to create the missing artifact file."),
            ));
        }
    }

    let packet_path = dir.join("requirement-packet.json");
    if !packet_path.exists() {
        issues.push(validation_issue(
            SpecValidationSeverity::Error,
            "spec.requirement_packet_missing",
            "requirement-packet.json",
            "Requirement packet must exist before execution.",
            Some(
                "Run brainstorming or create a requirement-packet.json in the proposal directory.",
            ),
        ));
    } else {
        let packet: RequirementPacket = serde_json::from_str(&fs::read_to_string(&packet_path)?)?;
        if packet.schema_version.trim().is_empty() {
            issues.push(validation_issue(
                SpecValidationSeverity::Error,
                "packet.schema_version_missing",
                "requirement-packet.json",
                "Requirement packet schema_version must be present.",
                Some("Set schema_version in requirement-packet.json to \"zero_nine.stage1.v1\"."),
            ));
        }
        if !packet.next_questions.is_empty() {
            issues.push(validation_issue(
                SpecValidationSeverity::Error,
                "packet.unresolved_questions",
                "requirement-packet.json",
                "Requirement packet still contains unresolved clarification questions.",
                Some("Answer all clarification questions and remove them from next_questions."),
            ));
        }
        if !packet.clarified {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "packet.not_marked_clarified",
                "requirement-packet.json",
                "Requirement packet is not marked clarified even though execution is being considered.",
                Some("Set clarified: true in requirement-packet.json after reviewing all questions."),
            ));
        }
    }

    for task in &proposal.tasks {
        if task.title.trim().is_empty() {
            issues.push(validation_issue(
                SpecValidationSeverity::Error,
                "task.title_missing",
                &format!("tasks.{}.title", task.id),
                "Task title must not be empty.",
                Some("Provide a descriptive title for the task in the tasks array."),
            ));
        }
        if task.contract.acceptance_criteria.is_empty() {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "task.contract_acceptance_missing",
                &format!("tasks.{}.contract.acceptance_criteria", task.id),
                "Task contract should include acceptance criteria.",
                Some("Add acceptance criteria strings to the task's contract.acceptance_criteria array."),
            ));
        }
        if task.contract.verification_points.is_empty() {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "task.contract_verification_missing",
                &format!("tasks.{}.contract.verification_points", task.id),
                "Task contract should include verification points.",
                Some("Add verification point strings to the task's contract.verification_points array."),
            ));
        }
        for dependency in &task.depends_on {
            if !proposal
                .tasks
                .iter()
                .any(|candidate| &candidate.id == dependency)
            {
                issues.push(validation_issue(
                    SpecValidationSeverity::Error,
                    "task.dependency_missing",
                    &format!("tasks.{}.depends_on", task.id),
                    &format!(
                        "Task dependency {} does not exist in proposal tasks.",
                        dependency
                    ),
                    Some("Correct the depends_on reference to match an existing task ID, or add the missing task."),
                ));
            }
        }
    }

    // DAG ring detection as blocking validation gate
    let graph = build_task_graph(proposal);
    let dag_result = graph.validate_dag();
    if !dag_result.valid {
        for error in &dag_result.errors {
            issues.push(validation_issue(
                SpecValidationSeverity::Error,
                "dag.validation_failed",
                "dag.json",
                &format!("DAG error: {}", error.message),
                Some("Remove the circular dependency or fix the invalid task references."),
            ));
        }
    }

    Ok(SpecValidationReport {
        schema_version: default_spec_schema_version(),
        proposal_id: proposal.id.clone(),
        valid: !issues
            .iter()
            .any(|issue| matches!(issue.severity, SpecValidationSeverity::Error)),
        issues,
    })
}

pub fn write_spec_validation_report(project_root: &Path, proposal: &Proposal) -> Result<PathBuf> {
    let mut report = validate_proposal_spec(project_root, proposal)?;
    let gate_issues = check_spec_completeness_gate(project_root, proposal)?;
    report.issues.extend(gate_issues);
    let completeness_issues = check_spec_completeness(project_root, proposal)?;
    report.issues.extend(completeness_issues);
    // Recompute valid with all issues
    report.valid = !report
        .issues
        .iter()
        .any(|i| matches!(i.severity, SpecValidationSeverity::Error));
    let path = proposal_dir(project_root, &proposal.id).join("spec-validation.json");
    fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    Ok(path)
}

pub fn check_spec_completeness(
    project_root: &Path,
    proposal: &Proposal,
) -> Result<Vec<SpecValidationIssue>> {
    let bundle = spec_bundle(project_root, &proposal.id);
    let mut issues = Vec::new();

    if let Ok(content) = fs::read_to_string(&bundle.requirements_path) {
        check_requirements_coverage(&content, proposal, &bundle.requirements_path, &mut issues);
    }
    if let Ok(content) = fs::read_to_string(&bundle.acceptance_path) {
        check_acceptance_coverage(&content, proposal, &bundle.acceptance_path, &mut issues);
    }
    if let Ok(content) = fs::read_to_string(&bundle.design_path) {
        check_design_coverage(&content, proposal, &bundle.design_path, &mut issues);
    }
    if let Ok(content) = fs::read_to_string(&bundle.tasks_path) {
        check_task_deliverable_coverage(&content, proposal, &bundle.tasks_path, &mut issues);
    }
    check_verification_file(&bundle, &mut issues);
    check_dag_task_alignment(proposal, &mut issues);

    Ok(issues)
}

/// Blocking completeness gate — checks that produce Error-level issues
/// and prevent execution when critical spec content is missing.
pub fn check_spec_completeness_gate(
    project_root: &Path,
    proposal: &Proposal,
) -> Result<Vec<SpecValidationIssue>> {
    let bundle = spec_bundle(project_root, &proposal.id);
    let mut issues = Vec::new();

    // Acceptance criteria must be documented — blocking
    if let Ok(content) = fs::read_to_string(&bundle.acceptance_path) {
        check_acceptance_coverage_blocking(
            &content,
            proposal,
            &bundle.acceptance_path,
            &mut issues,
        );
    }

    // DAG must reference only existing task IDs — blocking
    check_dag_task_alignment_blocking(proposal, &mut issues);

    Ok(issues)
}

fn check_acceptance_coverage_blocking(
    content: &str,
    proposal: &Proposal,
    acc_path: &str,
    issues: &mut Vec<SpecValidationIssue>,
) {
    for criterion in &proposal.acceptance_criteria {
        if !criterion.description.trim().is_empty()
            && !content_has_coverage(content, &criterion.description)
        {
            issues.push(validation_issue(
                SpecValidationSeverity::Error,
                "completeness.acceptance_gap",
                acc_path,
                &format!(
                    "Acceptance criterion not covered in acceptance.md: {}",
                    criterion.description
                ),
                Some(&format!(
                    "Add coverage for acceptance criterion '{}' to acceptance.md before execution.",
                    criterion.description
                )),
            ));
        }
    }
}

fn check_dag_task_alignment_blocking(proposal: &Proposal, issues: &mut Vec<SpecValidationIssue>) {
    let task_ids: std::collections::HashSet<&str> =
        proposal.tasks.iter().map(|t| t.id.as_str()).collect();
    let mut referenced_ids = std::collections::HashSet::new();
    for task in &proposal.tasks {
        referenced_ids.insert(task.id.as_str());
        for dep in &task.depends_on {
            referenced_ids.insert(dep.as_str());
        }
    }
    for id in &referenced_ids {
        if !task_ids.contains(id) {
            issues.push(validation_issue(
                SpecValidationSeverity::Error,
                "completeness.dag_orphan_edge",
                "dag.json",
                &format!("DAG references task ID '{id}' which does not exist in proposal tasks."),
                Some(&format!("Remove the orphan reference to task '{id}' from dag.json or add the missing task.")),
            ));
        }
    }
}

fn content_has_coverage(content: &str, text: &str) -> bool {
    if text.len() <= 30 {
        return content.contains(text);
    }
    let words: Vec<_> = text.split_whitespace().filter(|w| w.len() > 3).collect();
    if words.is_empty() {
        return false;
    }
    let matched = words.iter().filter(|w| content.contains(*w)).count();
    (matched as f64 / words.len() as f64) >= 0.4
}

fn check_requirements_coverage(
    content: &str,
    proposal: &Proposal,
    req_path: &str,
    issues: &mut Vec<SpecValidationIssue>,
) {
    for item in &proposal.scope_in {
        if !content_has_coverage(content, item) {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "completeness.requirements_gap",
                req_path,
                &format!("Scope-in item not covered in requirements.md: {item}"),
                Some(&format!(
                    "Add coverage for scope_in item '{item}' to requirements.md."
                )),
            ));
        }
    }
    for item in &proposal.scope_out {
        if !content_has_coverage(content, item) {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "completeness.requirements_gap",
                req_path,
                &format!("Scope-out item not covered in requirements.md: {item}"),
                Some(&format!(
                    "Add coverage for scope_out item '{item}' to requirements.md."
                )),
            ));
        }
    }
    for constraint in &proposal.constraints {
        if !constraint.description.trim().is_empty()
            && !content_has_coverage(content, &constraint.description)
        {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "completeness.requirements_gap",
                req_path,
                &format!(
                    "Constraint not covered in requirements.md: {}",
                    constraint.description
                ),
                Some(&format!(
                    "Add coverage for constraint '{}' to requirements.md.",
                    constraint.description
                )),
            ));
        }
    }
}

fn check_acceptance_coverage(
    content: &str,
    proposal: &Proposal,
    acc_path: &str,
    issues: &mut Vec<SpecValidationIssue>,
) {
    for criterion in &proposal.acceptance_criteria {
        if !criterion.description.trim().is_empty()
            && !content_has_coverage(content, &criterion.description)
        {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "completeness.acceptance_gap",
                acc_path,
                &format!(
                    "Acceptance criterion not covered in acceptance.md: {}",
                    criterion.description
                ),
                Some(&format!(
                    "Add coverage for acceptance criterion '{}' to acceptance.md.",
                    criterion.description
                )),
            ));
        }
    }
}

fn check_design_coverage(
    content: &str,
    proposal: &Proposal,
    design_path: &str,
    issues: &mut Vec<SpecValidationIssue>,
) {
    for task in &proposal.tasks {
        if !task.title.trim().is_empty() && !content_has_coverage(content, &task.title) {
            issues.push(validation_issue(
                SpecValidationSeverity::Info,
                "completeness.design_gap",
                design_path,
                &format!(
                    "Task not covered in design.md: {} ({})",
                    task.id, task.title
                ),
                Some(&format!(
                    "Add design coverage for task '{}: {}' to design.md.",
                    task.id, task.title
                )),
            ));
        }
    }
}

fn check_task_deliverable_coverage(
    content: &str,
    proposal: &Proposal,
    tasks_path: &str,
    issues: &mut Vec<SpecValidationIssue>,
) {
    for criterion in &proposal.acceptance_criteria {
        if !criterion.description.trim().is_empty()
            && !content_has_coverage(content, &criterion.description)
        {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "completeness.task_deliverable_gap",
                tasks_path,
                &format!(
                    "Acceptance criterion not referenced in tasks.md: {}",
                    criterion.description
                ),
                Some(&format!(
                    "Ensure tasks.md deliverables reference the acceptance criterion '{}'.",
                    criterion.description
                )),
            ));
        }
    }
}

fn check_verification_file(bundle: &SpecBundle, issues: &mut Vec<SpecValidationIssue>) {
    let path = &bundle.verification_path;
    if !Path::new(path).exists() {
        issues.push(validation_issue(
            SpecValidationSeverity::Warning,
            "completeness.verification_missing",
            path,
            "Verification report file is missing.",
            Some("Write verification results to verification.md after execution."),
        ));
    } else if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() < 10 {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "completeness.verification_missing",
                path,
                "Verification report file is nearly empty.",
                Some("Write verification results to verification.md after execution."),
            ));
        }
    }
}

fn check_dag_task_alignment(proposal: &Proposal, issues: &mut Vec<SpecValidationIssue>) {
    let task_ids: std::collections::HashSet<&str> =
        proposal.tasks.iter().map(|t| t.id.as_str()).collect();
    let mut referenced_ids = std::collections::HashSet::new();
    for task in &proposal.tasks {
        referenced_ids.insert(task.id.as_str());
        for dep in &task.depends_on {
            referenced_ids.insert(dep.as_str());
        }
    }
    for id in &referenced_ids {
        if !task_ids.contains(id) {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "completeness.dag_orphan_edge",
                "dag.json",
                &format!("DAG references task ID '{id}' which does not exist in proposal tasks."),
                Some(&format!("Remove the orphan reference to task '{id}' from dag.json or add the missing task.")),
            ));
        }
    }
}

fn build_task_graph(proposal: &Proposal) -> TaskGraph {
    let mut edges = Vec::new();
    for task in &proposal.tasks {
        for dependency in &task.depends_on {
            edges.push(TaskDependencyEdge {
                from: dependency.clone(),
                to: task.id.clone(),
            });
        }
    }

    TaskGraph {
        schema_version: default_spec_schema_version(),
        proposal_id: proposal.id.clone(),
        tasks: proposal.tasks.clone(),
        edges,
    }
}

fn validation_issue(
    severity: SpecValidationSeverity,
    code: &str,
    path: &str,
    message: &str,
    suggested_fix: Option<&str>,
) -> SpecValidationIssue {
    SpecValidationIssue {
        severity,
        code: code.to_string(),
        path: path.to_string(),
        message: message.to_string(),
        suggested_fix: suggested_fix.map(|s| s.to_string()),
    }
}

fn render_proposal_markdown(proposal: &Proposal, packet: &RequirementPacket) -> String {
    format!(
        "# Proposal\n\n## Goal\n\n{}\n\n## Problem Statement\n\n{}\n\n## Status\n\n{:?}\n\n## Why This Matters\n\nThe proposal exists to turn a one-line request into an observable engineering workflow with spec artifacts, loop control, guarded execution, and evolution feedback.\n\n## Primary Risks\n\n{}\n",
        proposal.goal,
        packet.problem_statement,
        proposal.status,
        render_list(&packet.risks)
    )
}

fn render_requirements_markdown(packet: &RequirementPacket) -> String {
    format!(
        "# Requirements\n\n## User Goal\n\n{}\n\n## Problem Statement\n\n{}\n\n## Scope In\n\n{}\n\n## Scope Out\n\n{}\n\n## Constraints\n\n{}\n",
        packet.user_goal,
        packet.problem_statement,
        render_list(&packet.scope_in),
        render_list(&packet.scope_out),
        render_list(&packet.constraints)
    )
}

fn render_acceptance_markdown(packet: &RequirementPacket) -> String {
    format!(
        "# Acceptance Criteria\n\n{}\n\n## Risks\n\n{}\n",
        render_checkbox_list(&packet.acceptance_criteria),
        render_list(&packet.risks)
    )
}

fn render_clarifications_markdown(packet: &RequirementPacket) -> String {
    let status = if packet.next_questions.is_empty() {
        "The current Brainstorming packet is sufficiently converged for the next execution phase."
    } else {
        "These are the next questions that should be answered if the user wants a tighter implementation contract."
    };
    let questions = if packet.next_questions.is_empty() {
        vec!["No open clarification questions remain in the current packet.".to_string()]
    } else {
        packet.next_questions.clone()
    };
    format!(
        "# Clarifications\n\n{}\n\n{}\n",
        status,
        render_list(&questions)
    )
}

fn render_design_markdown(proposal: &Proposal, packet: &RequirementPacket) -> String {
    format!(
        "# Design\n\n## Proposal\n\n{}\n\n## Four-Layer Strategy\n\n1. Use Brainstorming to clarify intent and acceptance.\n2. Write OpenSpec artifacts for planning and traceability.\n3. Use Ralph-loop to select tasks, maintain progress, and gate completion.\n4. Use OpenSpace-style observation to capture improvements and reduce repeated mistakes.\n\n## Constraints\n\n{}\n\n## Risks\n\n{}\n\n## Notes\n\nThis design is intended to be consumed by the execution layer and re-read by the loop before each fresh iteration.\n",
        proposal.id,
        render_list(&packet.constraints),
        render_list(&packet.risks)
    )
}

fn render_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("- {}", item))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_checkbox_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("- [ ] {}", item))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_brainstorm_session_markdown(session: &BrainstormSession) -> String {
    let mut output = String::new();
    output.push_str("# Brainstorm Session\n\n");
    output.push_str("## Goal\n\n");
    output.push_str(&format!("{}\n\n", session.goal));
    output.push_str("## Status\n\n");
    output.push_str(&format!("{} / {:?}\n\n", session.status, session.verdict));
    output.push_str("## Questions and Answers\n\n");
    for question in &session.questions {
        output.push_str(&format!("### {}\n\n", question.question));
        output.push_str(&format!("- Rationale: {}\n", question.rationale));
        output.push_str(&format!("- Priority: {}\n", question.priority));
        if let Some(answer) = session
            .answers
            .iter()
            .find(|item| item.question_id == question.id)
        {
            output.push_str(&format!("- Answer: {}\n\n", answer.answer));
        } else {
            output.push_str("- Answer: Pending\n\n");
        }
    }
    output
}

fn answer_or_default(session: &BrainstormSession, question_id: &str, fallback: String) -> String {
    session
        .answers
        .iter()
        .find(|answer| answer.question_id == question_id)
        .map(|answer| answer.answer.trim().to_string())
        .filter(|answer| !answer.is_empty())
        .unwrap_or(fallback)
}

fn answer_list_or_default(
    session: &BrainstormSession,
    question_id: &str,
    fallback: Vec<String>,
) -> Vec<String> {
    let answer = session
        .answers
        .iter()
        .find(|item| item.question_id == question_id)
        .map(|item| item.answer.trim().to_string())
        .unwrap_or_default();
    let parsed = split_multiline_answer(&answer);
    if parsed.is_empty() {
        fallback
    } else {
        parsed
    }
}

fn unresolved_questions(session: &BrainstormSession) -> Vec<String> {
    let mut pending = Vec::new();
    for question in &session.questions {
        let answered = session
            .answers
            .iter()
            .any(|answer| answer.question_id == question.id && !answer.answer.trim().is_empty());
        if !answered {
            pending.push(question.question.clone());
        }
    }
    if pending.is_empty() && !matches!(session.verdict, zn_types::BrainstormVerdict::Ready) {
        pending.push(
            "Brainstorming has answers recorded, but the session verdict is not yet Ready; review the contract before execution."
                .to_string(),
        );
    }
    pending
}

fn split_multiline_answer(answer: &str) -> Vec<String> {
    answer
        .lines()
        .flat_map(|line| line.split(';'))
        .map(|item| {
            item.trim()
                .trim_start_matches(|c: char| {
                    c.is_ascii_digit() || c == '.' || c == ')' || c == '-' || c == ' '
                })
                .trim()
                .to_string()
        })
        .filter(|item| !item.is_empty())
        .collect()
}

// ==================== M10: Policy Integration ====================

pub fn create_default_policy_engine() -> zn_types::PolicyEngine {
    let mut engine = zn_types::PolicyEngine::default();
    engine.rules.push(zn_types::PolicyRule {
        id: "rule-read".to_string(),
        name: "Read Operations".to_string(),
        action_pattern: "file.read".to_string(),
        risk_level: zn_types::ActionRiskLevel::Low,
        default_decision: zn_types::PolicyDecision::Allow,
        conditions: vec![],
        exceptions: vec![],
    });
    engine.rules.push(zn_types::PolicyRule {
        id: "rule-write".to_string(),
        name: "Write Operations".to_string(),
        action_pattern: "file.write".to_string(),
        risk_level: zn_types::ActionRiskLevel::Medium,
        default_decision: zn_types::PolicyDecision::Allow,
        conditions: vec!["worktree_isolated".to_string()],
        exceptions: vec![],
    });
    engine.rules.push(zn_types::PolicyRule {
        id: "rule-merge".to_string(),
        name: "Merge Operations".to_string(),
        action_pattern: "git.merge".to_string(),
        risk_level: zn_types::ActionRiskLevel::Critical,
        default_decision: zn_types::PolicyDecision::Ask,
        conditions: vec!["tests_passed".to_string(), "review_approved".to_string()],
        exceptions: vec![],
    });
    engine
}

// ==================== M12: Skill Library ====================

pub fn create_default_skill_library() -> zn_types::SkillLibrary {
    let mut lib = zn_types::SkillLibrary::default();
    lib.bundles.push(zn_types::SkillBundle {
        id: "skill-brainstorm".to_string(),
        name: "Brainstorming".to_string(),
        version: zn_types::SkillVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        description: "Socratic questioning for requirement clarification".to_string(),
        applicable_scenarios: vec!["requirement_gathering".to_string()],
        preconditions: vec!["user_goal_provided".to_string()],
        disabled_conditions: vec![],
        risk_level: zn_types::ActionRiskLevel::Low,
        skill_chain: vec!["brainstorming".to_string()],
        artifacts: vec!["brainstorm-session.md".to_string()],
        usage_count: 0,
        success_rate: 0.0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    });
    lib.bundles.push(zn_types::SkillBundle {
        id: "skill-tdd".to_string(),
        name: "Test-Driven Development".to_string(),
        version: zn_types::SkillVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        description: "Test-first implementation cycle".to_string(),
        applicable_scenarios: vec!["feature_implementation".to_string()],
        preconditions: vec!["requirements_clear".to_string()],
        disabled_conditions: vec![],
        risk_level: zn_types::ActionRiskLevel::Medium,
        skill_chain: vec!["test-driven-development".to_string()],
        artifacts: vec![],
        usage_count: 0,
        success_rate: 0.0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    });
    lib.active_bundle_ids = lib.bundles.iter().map(|b| b.id.clone()).collect();
    lib
}

pub fn save_skill_library(
    project_root: &Path,
    library: &zn_types::SkillLibrary,
) -> Result<PathBuf> {
    let path = zero_nine_dir(project_root).join("evolve/skill-library.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(library)?)?;
    Ok(path)
}

pub fn load_skill_library(project_root: &Path) -> Result<Option<zn_types::SkillLibrary>> {
    let path = zero_nine_dir(project_root).join("evolve/skill-library.json");
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&data)?))
}

// ==================== M6: Issue Mapping ====================

use zn_types::IssueMapping;

/// 记录 Issue → Proposal 映射关系
pub fn record_issue_mapping(
    issue_number: u64,
    repo: &str,
    proposal_id: &str,
    project_root: &Path,
) -> Result<()> {
    let mappings_path = zero_nine_dir(project_root).join("runtime/issue-mappings.json");
    let mut mappings: Vec<IssueMapping> = if mappings_path.exists() {
        let data = fs::read_to_string(&mappings_path)?;
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        Vec::new()
    };

    mappings.push(IssueMapping {
        issue_number,
        repo: repo.to_string(),
        proposal_id: proposal_id.to_string(),
        created_at: Utc::now(),
    });

    fs::write(&mappings_path, serde_json::to_string_pretty(&mappings)?)?;
    Ok(())
}

/// 根据 proposal_id 查找对应的 Issue 信息
pub fn find_issue_for_proposal(proposal_id: &str, project_root: &Path) -> Option<IssueMapping> {
    let mappings_path = zero_nine_dir(project_root).join("runtime/issue-mappings.json");
    if !mappings_path.exists() {
        return None;
    }
    let data = fs::read_to_string(&mappings_path).ok()?;
    let mappings: Vec<IssueMapping> = serde_json::from_str(&data).ok()?;
    mappings.into_iter().find(|m| m.proposal_id == proposal_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_policy_engine_creation() {
        let engine = create_default_policy_engine();
        assert_eq!(engine.rules.len(), 3);
        assert_eq!(engine.max_allowed_risk, zn_types::ActionRiskLevel::High);
    }

    #[test]
    fn test_skill_library_creation() {
        let library = create_default_skill_library();
        assert!(library.bundles.len() >= 2);
        assert!(!library.active_bundle_ids.is_empty());
    }

    #[test]
    fn test_skill_library_save_load() {
        use std::env::temp_dir;
        let tmp_dir = temp_dir().join("zn_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        let library = create_default_skill_library();
        save_skill_library(&tmp_dir, &library).unwrap();

        let loaded = load_skill_library(&tmp_dir).unwrap().unwrap();
        assert_eq!(loaded.bundles.len(), library.bundles.len());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_issue_mapping_crud() {
        let tmp_dir = temp_dir().join("zn_issue_mapping");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();
        ensure_layout(&tmp_dir).unwrap();

        record_issue_mapping(42, "owner/repo", "proposal-1", &tmp_dir).unwrap();
        record_issue_mapping(99, "owner/repo", "proposal-2", &tmp_dir).unwrap();

        let found = find_issue_for_proposal("proposal-1", &tmp_dir);
        assert!(found.is_some());
        let mapping = found.unwrap();
        assert_eq!(mapping.issue_number, 42);
        assert_eq!(mapping.repo, "owner/repo");

        let not_found = find_issue_for_proposal("nonexistent", &tmp_dir);
        assert!(not_found.is_none());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    fn setup_test_project(tmp_dir: &Path, goal: &str, tasks: Vec<zn_types::TaskItem>) -> Proposal {
        std::fs::create_dir_all(tmp_dir).unwrap();
        ensure_layout(tmp_dir).unwrap();

        let mut proposal = Proposal {
            id: "test-proposal".to_string(),
            goal: goal.to_string(),
            schema_version: "zero_nine.stage1.v1".to_string(),
            tasks,
            ..Proposal::default()
        };
        proposal.scope_in = vec!["User authentication system".to_string()];
        proposal.scope_out = vec!["Payment processing".to_string()];
        proposal.constraints = vec![zn_types::Constraint {
            id: "c1".to_string(),
            category: zn_types::ConstraintCategory::Technical,
            description: "Must use PostgreSQL".to_string(),
            rationale: None,
            enforced: true,
        }];
        proposal.acceptance_criteria = vec![zn_types::AcceptanceCriterion {
            id: "ac1".to_string(),
            description: "Users can log in with email and password".to_string(),
            verification_method: zn_types::VerificationMethod::AutomatedTest,
            priority: zn_types::Priority::High,
            status: zn_types::CriterionStatus::Pending,
        }];

        save_proposal(tmp_dir, &proposal).unwrap();

        // Create requirement packet
        let packet = zn_types::RequirementPacket {
            schema_version: "zero_nine.stage1.v1".to_string(),
            user_goal: goal.to_string(),
            problem_statement: "Need auth system".to_string(),
            scope_in: vec!["User authentication system".to_string()],
            scope_out: vec!["Payment processing".to_string()],
            constraints: vec!["Must use PostgreSQL".to_string()],
            acceptance_criteria: vec!["Users can log in with email and password".to_string()],
            risks: vec![],
            next_questions: vec![],
            source_brainstorm_session_id: None,
            clarified: true,
        };
        let packet_path = proposal_dir(tmp_dir, "test-proposal").join("requirement-packet.json");
        fs::write(&packet_path, serde_json::to_string_pretty(&packet).unwrap()).unwrap();

        // Write minimal spec files
        let bundle = spec_bundle(tmp_dir, "test-proposal");
        fs::write(
            &bundle.proposal_path,
            "# Proposal\n\n## Goal\n\nBuild auth system.\n",
        )
        .unwrap();
        fs::write(&bundle.requirements_path, "# Requirements\n\n## Scope In\n\n- User authentication system\n\n## Scope Out\n\n- Payment processing\n\n## Constraints\n\n- Must use PostgreSQL\n").unwrap();
        fs::write(
            &bundle.acceptance_path,
            "# Acceptance\n\n- Users can log in with email and password\n",
        )
        .unwrap();
        fs::write(
            &bundle.design_path,
            "# Design\n\n## Overview\n\nUser authentication system design.\n",
        )
        .unwrap();
        fs::write(
            &bundle.tasks_path,
            "# Tasks\n\n## Task 1\n\nSetup auth infrastructure. Users can log in with email and password.\n",
        )
        .unwrap();
        fs::write(&bundle.dag_path, "[]").unwrap();
        fs::write(
            &bundle.verification_path,
            "# Verification\n\nAll checks passed.\n",
        )
        .unwrap();

        proposal
    }

    #[test]
    fn test_suggested_fix_on_validation_issues() {
        let tmp_dir = temp_dir().join("zn_fix_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();
        ensure_layout(&tmp_dir).unwrap();

        // Create proposal with known validation errors: empty goal
        let proposal = Proposal {
            id: "broken".to_string(),
            goal: "".to_string(),
            schema_version: "zero_nine.stage1.v1".to_string(),
            tasks: vec![zn_types::TaskItem {
                id: "t1".to_string(),
                title: "Test task".to_string(),
                description: "desc".to_string(),
                status: zn_types::TaskStatus::Pending,
                depends_on: vec![],
                kind: None,
                contract: zn_types::TaskContract {
                    acceptance_criteria: vec!["ok".to_string()],
                    deliverables: vec![],
                    verification_points: vec!["check".to_string()],
                },
                max_retries: None,
                preconditions: vec![],
            }],
            ..Proposal::default()
        };
        save_proposal(&tmp_dir, &proposal).unwrap();

        let packet = zn_types::RequirementPacket {
            schema_version: "zero_nine.stage1.v1".to_string(),
            user_goal: "".to_string(),
            problem_statement: "".to_string(),
            scope_in: vec![],
            scope_out: vec![],
            constraints: vec![],
            acceptance_criteria: vec![],
            risks: vec![],
            next_questions: vec![],
            source_brainstorm_session_id: None,
            clarified: true,
        };
        let packet_path = proposal_dir(&tmp_dir, "broken").join("requirement-packet.json");
        fs::write(&packet_path, serde_json::to_string_pretty(&packet).unwrap()).unwrap();

        let bundle = spec_bundle(&tmp_dir, "broken");
        for path in [
            &bundle.proposal_path,
            &bundle.requirements_path,
            &bundle.acceptance_path,
            &bundle.design_path,
            &bundle.tasks_path,
            &bundle.dag_path,
        ] {
            fs::write(path, "# placeholder\n").unwrap();
        }

        let report = validate_proposal_spec(&tmp_dir, &proposal).unwrap();
        let error_issues: Vec<_> = report
            .issues
            .iter()
            .filter(|i| matches!(i.severity, SpecValidationSeverity::Error))
            .collect();
        assert!(!error_issues.is_empty());
        for issue in &error_issues {
            assert!(
                issue.suggested_fix.is_some(),
                "Issue {} has no suggested_fix",
                issue.code
            );
            assert!(
                !issue.suggested_fix.as_ref().unwrap().is_empty(),
                "Issue {} has empty fix suggestion",
                issue.code
            );
        }

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_check_completeness_requirements_gap() {
        let tmp_dir = temp_dir().join("zn_gap_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let task = zn_types::TaskItem {
            id: "t1".to_string(),
            title: "Auth".to_string(),
            description: "desc".to_string(),
            status: zn_types::TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: zn_types::TaskContract {
                acceptance_criteria: vec!["ok".to_string()],
                deliverables: vec![],
                verification_points: vec!["check".to_string()],
            },
            max_retries: None,
            preconditions: vec![],
        };
        let proposal = setup_test_project(&tmp_dir, "Build auth system", vec![task]);

        // Remove scope_in mention from requirements.md to create a gap
        let bundle = spec_bundle(&tmp_dir, "test-proposal");
        fs::write(
            &bundle.requirements_path,
            "# Requirements\n\n## Scope Out\n\n- Payment processing\n",
        )
        .unwrap();

        let issues = check_spec_completeness(&tmp_dir, &proposal).unwrap();
        let req_gaps: Vec<_> = issues
            .iter()
            .filter(|i| i.code == "completeness.requirements_gap")
            .collect();
        assert!(
            !req_gaps.is_empty(),
            "Expected requirements_gap issues but found none"
        );

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_check_completeness_full_coverage() {
        let tmp_dir = temp_dir().join("zn_full_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let task = zn_types::TaskItem {
            id: "t1".to_string(),
            title: "Auth".to_string(),
            description: "desc".to_string(),
            status: zn_types::TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: zn_types::TaskContract {
                acceptance_criteria: vec!["ok".to_string()],
                deliverables: vec![],
                verification_points: vec!["check".to_string()],
            },
            max_retries: None,
            preconditions: vec![],
        };
        let proposal = setup_test_project(&tmp_dir, "Build auth system", vec![task]);

        let issues = check_spec_completeness(&tmp_dir, &proposal).unwrap();
        let warnings: Vec<_> = issues
            .iter()
            .filter(|i| matches!(i.severity, SpecValidationSeverity::Warning))
            .collect();
        assert!(
            warnings.is_empty(),
            "Expected no warnings with full coverage, got {:?}",
            warnings
        );

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_check_completeness_dag_orphan_edge() {
        let tmp_dir = temp_dir().join("zn_orphan_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let tasks = vec![
            zn_types::TaskItem {
                id: "t1".to_string(),
                title: "First".to_string(),
                description: "desc".to_string(),
                status: zn_types::TaskStatus::Pending,
                depends_on: vec!["nonexistent_task".to_string()],
                kind: None,
                contract: zn_types::TaskContract {
                    acceptance_criteria: vec!["ok".to_string()],
                    deliverables: vec![],
                    verification_points: vec!["check".to_string()],
                },
                max_retries: None,
                preconditions: vec![],
            },
            zn_types::TaskItem {
                id: "t2".to_string(),
                title: "Second".to_string(),
                description: "desc".to_string(),
                status: zn_types::TaskStatus::Pending,
                depends_on: vec!["t1".to_string()],
                kind: None,
                contract: zn_types::TaskContract {
                    acceptance_criteria: vec!["ok".to_string()],
                    deliverables: vec![],
                    verification_points: vec!["check".to_string()],
                },
                max_retries: None,
                preconditions: vec![],
            },
        ];
        let proposal = setup_test_project(&tmp_dir, "Build system", tasks);

        let issues = check_spec_completeness(&tmp_dir, &proposal).unwrap();
        let orphan_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.code == "completeness.dag_orphan_edge")
            .collect();
        assert!(
            !orphan_issues.is_empty(),
            "Expected dag_orphan_edge issues but found none"
        );

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_check_completeness_verification_missing() {
        let tmp_dir = temp_dir().join("zn_verify_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let task = zn_types::TaskItem {
            id: "t1".to_string(),
            title: "Auth".to_string(),
            description: "desc".to_string(),
            status: zn_types::TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: zn_types::TaskContract {
                acceptance_criteria: vec!["ok".to_string()],
                deliverables: vec![],
                verification_points: vec!["check".to_string()],
            },
            max_retries: None,
            preconditions: vec![],
        };
        let proposal = setup_test_project(&tmp_dir, "Build auth system", vec![task]);

        // Delete verification file
        let bundle = spec_bundle(&tmp_dir, "test-proposal");
        let _ = fs::remove_file(&bundle.verification_path);

        let issues = check_spec_completeness(&tmp_dir, &proposal).unwrap();
        let verify_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.code == "completeness.verification_missing")
            .collect();
        assert!(
            !verify_issues.is_empty(),
            "Expected verification_missing issues but found none"
        );

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_write_report_merges_both_passes() {
        let tmp_dir = temp_dir().join("zn_merge_test");
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let task = zn_types::TaskItem {
            id: "t1".to_string(),
            title: "Auth".to_string(),
            description: "desc".to_string(),
            status: zn_types::TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: zn_types::TaskContract {
                acceptance_criteria: vec!["ok".to_string()],
                deliverables: vec![],
                verification_points: vec!["check".to_string()],
            },
            max_retries: None,
            preconditions: vec![],
        };
        let proposal = setup_test_project(&tmp_dir, "Build auth system", vec![task]);

        // Create a completeness gap by removing scope_in from requirements
        let bundle = spec_bundle(&tmp_dir, "test-proposal");
        fs::write(
            &bundle.requirements_path,
            "# Requirements\n\nNo scope mentioned.\n",
        )
        .unwrap();

        let path = write_spec_validation_report(&tmp_dir, &proposal).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let report: SpecValidationReport = serde_json::from_str(&content).unwrap();

        let _validation_issues: Vec<_> = report
            .issues
            .iter()
            .filter(|i| !i.code.starts_with("completeness."))
            .collect();
        let completeness_issues: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.code.starts_with("completeness."))
            .collect();

        assert!(
            !completeness_issues.is_empty(),
            "Expected completeness issues in merged report"
        );
        assert!(
            completeness_issues
                .iter()
                .all(|i| i.suggested_fix.is_some()),
            "All completeness issues should have fix suggestions"
        );
        // Validation should pass (no errors) even with completeness warnings
        assert!(
            report.valid,
            "Report should be valid (completeness is advisory)"
        );

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
