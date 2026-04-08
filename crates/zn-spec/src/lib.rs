use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use zn_types::{
    default_spec_schema_version, BrainstormSession, LoopStage, LoopState, ProgressRecord,
    ProjectManifest, Proposal, ProposalStatus, RequirementPacket, RuntimeEvent, SpecBundle,
    SpecValidationIssue, SpecValidationReport, SpecValidationSeverity, TaskContract,
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

        // M1: Structured spec contract fields
        problem_statement: Some(packet.problem_statement.clone()),
        scope_in: packet.scope_in.clone(),
        scope_out: packet.scope_out.clone(),
        constraints,
        acceptance_criteria,
        risks,
        dependencies: Vec::new(),
        non_goals: Vec::new(),

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
        ));
    }

    if proposal.goal.trim().is_empty() {
        issues.push(validation_issue(
            SpecValidationSeverity::Error,
            "proposal.goal_missing",
            "proposal.json",
            "Proposal goal must not be empty.",
        ));
    }

    if proposal.tasks.is_empty() {
        issues.push(validation_issue(
            SpecValidationSeverity::Error,
            "proposal.tasks_missing",
            "proposal.json",
            "Proposal must contain at least one task.",
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
        ));
    } else {
        let packet: RequirementPacket = serde_json::from_str(&fs::read_to_string(&packet_path)?)?;
        if packet.schema_version.trim().is_empty() {
            issues.push(validation_issue(
                SpecValidationSeverity::Error,
                "packet.schema_version_missing",
                "requirement-packet.json",
                "Requirement packet schema_version must be present.",
            ));
        }
        if !packet.next_questions.is_empty() {
            issues.push(validation_issue(
                SpecValidationSeverity::Error,
                "packet.unresolved_questions",
                "requirement-packet.json",
                "Requirement packet still contains unresolved clarification questions.",
            ));
        }
        if !packet.clarified {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "packet.not_marked_clarified",
                "requirement-packet.json",
                "Requirement packet is not marked clarified even though execution is being considered.",
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
            ));
        }
        if task.contract.acceptance_criteria.is_empty() {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "task.contract_acceptance_missing",
                &format!("tasks.{}.contract.acceptance_criteria", task.id),
                "Task contract should include acceptance criteria.",
            ));
        }
        if task.contract.verification_points.is_empty() {
            issues.push(validation_issue(
                SpecValidationSeverity::Warning,
                "task.contract_verification_missing",
                &format!("tasks.{}.contract.verification_points", task.id),
                "Task contract should include verification points.",
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
                ));
            }
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
    let report = validate_proposal_spec(project_root, proposal)?;
    let path = proposal_dir(project_root, &proposal.id).join("spec-validation.json");
    fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    Ok(path)
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
) -> SpecValidationIssue {
    SpecValidationIssue {
        severity,
        code: code.to_string(),
        path: path.to_string(),
        message: message.to_string(),
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
        id: "rule-read".to_string(), name: "Read Operations".to_string(),
        action_pattern: "file.read".to_string(), risk_level: zn_types::ActionRiskLevel::Low,
        default_decision: zn_types::PolicyDecision::Allow, conditions: vec![], exceptions: vec![],
    });
    engine.rules.push(zn_types::PolicyRule {
        id: "rule-write".to_string(), name: "Write Operations".to_string(),
        action_pattern: "file.write".to_string(), risk_level: zn_types::ActionRiskLevel::Medium,
        default_decision: zn_types::PolicyDecision::Allow, conditions: vec!["worktree_isolated".to_string()], exceptions: vec![],
    });
    engine.rules.push(zn_types::PolicyRule {
        id: "rule-merge".to_string(), name: "Merge Operations".to_string(),
        action_pattern: "git.merge".to_string(), risk_level: zn_types::ActionRiskLevel::Critical,
        default_decision: zn_types::PolicyDecision::Ask, conditions: vec!["tests_passed".to_string(), "review_approved".to_string()], exceptions: vec![],
    });
    engine
}

// ==================== M12: Skill Library ====================

pub fn create_default_skill_library() -> zn_types::SkillLibrary {
    let mut lib = zn_types::SkillLibrary::default();
    lib.bundles.push(zn_types::SkillBundle {
        id: "skill-brainstorm".to_string(), name: "Brainstorming".to_string(),
        version: zn_types::SkillVersion { major: 1, minor: 0, patch: 0 },
        description: "Socratic questioning for requirement clarification".to_string(),
        applicable_scenarios: vec!["requirement_gathering".to_string()],
        preconditions: vec!["user_goal_provided".to_string()], disabled_conditions: vec![],
        risk_level: zn_types::ActionRiskLevel::Low, skill_chain: vec!["brainstorming".to_string()],
        artifacts: vec!["brainstorm-session.md".to_string()], usage_count: 0, success_rate: 0.0,
        created_at: Utc::now(), updated_at: Utc::now(),
    });
    lib.bundles.push(zn_types::SkillBundle {
        id: "skill-tdd".to_string(), name: "Test-Driven Development".to_string(),
        version: zn_types::SkillVersion { major: 1, minor: 0, patch: 0 },
        description: "Test-first implementation cycle".to_string(),
        applicable_scenarios: vec!["feature_implementation".to_string()],
        preconditions: vec!["requirements_clear".to_string()], disabled_conditions: vec![],
        risk_level: zn_types::ActionRiskLevel::Medium, skill_chain: vec!["test-driven-development".to_string()],
        artifacts: vec![], usage_count: 0, success_rate: 0.0,
        created_at: Utc::now(), updated_at: Utc::now(),
    });
    lib.active_bundle_ids = lib.bundles.iter().map(|b| b.id.clone()).collect();
    lib
}

pub fn save_skill_library(project_root: &Path, library: &zn_types::SkillLibrary) -> Result<PathBuf> {
    let path = zero_nine_dir(project_root).join("evolve/skill-library.json");
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    fs::write(&path, serde_json::to_vec_pretty(library)?)?;
    Ok(path)
}

pub fn load_skill_library(project_root: &Path) -> Result<Option<zn_types::SkillLibrary>> {
    let path = zero_nine_dir(project_root).join("evolve/skill-library.json");
    if !path.exists() { return Ok(None); }
    let data = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&data)?))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
