pub mod cron_scheduler;

use anyhow::{anyhow, Result};
use chrono::Utc;
use rustyline::DefaultEditor;
use serde_json::json;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::thread;
use tracing::info;
use zn_evolve::{evaluate, propose_candidate, RewardModel};
use zn_exec::{
    answer_brainstorm_question, build_execution_envelope, build_plan, build_plan_with_config,
    execute_plan, next_brainstorm_question, prepare_workspace, start_brainstorm,
};
use zn_host::{export_adapter_files, write_execution_summary};
use zn_spec::{
    append_event, check_spec_completeness, create_proposal_from_brainstorm, ensure_layout,
    init_loop_state, load_latest_brainstorm_session, load_latest_proposal, load_manifest,
    proposal_dir, save_brainstorm_session, save_loop_state, save_manifest, save_proposal,
    spec_bundle, status_summary, update_progress_markdown, validate_proposal_spec,
    write_spec_validation_report,
};
use zn_types::{
    BrainstormSession, BrainstormVerdict, ExecutionEnvelope, ExecutionOutcome, ExecutionPlan,
    ExecutionReport, FailureCategory, FailureClassification, FailureSeverity, HostKind, LoopStage,
    ProjectManifest, Proposal, ProposalStatus, RuntimeEvent, SafetyEvent, StateTransition,
    TaskStatus,
};
use zn_types::{CompensationAction, CompensationType, EvolutionCandidate, EvolutionKind};

pub fn initialize_project(project_root: &Path, host: HostKind) -> Result<()> {
    ensure_layout(project_root)?;
    let mut manifest = ProjectManifest::default();
    manifest.default_host = host;
    save_manifest(project_root, &manifest)?;
    append_event(
        project_root,
        RuntimeEvent::new(
            "project.initialized".to_string(),
            Some(json!({"host": manifest.default_host.to_string()})),
        ),
    )?;
    Ok(())
}

pub fn brainstorm(
    project_root: &Path,
    goal: Option<&str>,
    host: HostKind,
    resume: bool,
) -> Result<String> {
    initialize_project(project_root, host.clone())?;

    let mut session = if resume {
        load_latest_brainstorm_session(project_root)?
            .ok_or_else(|| anyhow!("no brainstorm session found to resume"))?
    } else {
        let goal =
            goal.ok_or_else(|| anyhow!("goal is required when starting a new brainstorm session"))?;
        let session = start_brainstorm(goal, host.clone());
        append_event(
            project_root,
            RuntimeEvent::new(
                "brainstorm.started".to_string(),
                Some(json!({"goal": goal, "session_id": session.id, "host": host.to_string()})),
            ),
        )?;
        session
    };

    save_brainstorm_session(project_root, &session)?;

    match host {
        HostKind::Terminal => {
            run_terminal_brainstorm(project_root, &mut session)?;
            finalize_brainstorm(project_root, &session)
        }
        HostKind::ClaudeCode | HostKind::OpenCode => {
            render_host_brainstorm_status(project_root, &session, None)
        }
    }
}

pub fn brainstorm_host_turn(project_root: &Path, input: &str, host: HostKind) -> Result<String> {
    brainstorm_host_turn_internal(project_root, input, host, false, false)
}

fn brainstorm_host_turn_internal(
    project_root: &Path,
    input: &str,
    host: HostKind,
    auto_continue_execution: bool,
    allow_remote_finish: bool,
) -> Result<String> {
    initialize_project(project_root, host.clone())?;

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("brainstorm input cannot be empty"));
    }

    if let Some(mut session) = load_latest_brainstorm_session(project_root)? {
        if session.host == host && !matches!(session.verdict, BrainstormVerdict::Ready) {
            if let Some(question) = next_brainstorm_question(&session) {
                let verdict = answer_brainstorm_question(&mut session, &question.id, trimmed)?;
                save_brainstorm_session(project_root, &session)?;
                append_event(
                    project_root,
                    RuntimeEvent::new(
                        "brainstorm.host_answered".to_string(),
                        Some(json!({
                            "session_id": session.id,
                            "host": host.to_string(),
                            "question_id": question.id,
                            "verdict": verdict,
                            "auto_continue_execution": auto_continue_execution,
                        })),
                    ),
                )?;

                if auto_continue_execution
                    && matches!(session.verdict, BrainstormVerdict::Ready)
                    && next_brainstorm_question(&session).is_none()
                {
                    return continue_host_run_after_brainstorm(
                        project_root,
                        &session,
                        host,
                        Some(format!("Captured your answer for: {}", question.question)),
                        allow_remote_finish,
                    );
                }

                return render_host_brainstorm_status(
                    project_root,
                    &session,
                    Some(format!("Captured your answer for: {}", question.question)),
                );
            }
            return render_host_brainstorm_status(
                project_root,
                &session,
                Some("No unanswered clarification questions remained, so Zero_Nine re-evaluated the existing session state.".to_string()),
            );
        }
    }

    let session = start_brainstorm(trimmed, host.clone());
    append_event(
        project_root,
        RuntimeEvent::new(
            "brainstorm.host_started".to_string(),
            Some(json!({"goal": trimmed, "session_id": session.id, "host": host.to_string()})),
        ),
    )?;
    save_brainstorm_session(project_root, &session)?;
    render_host_brainstorm_status(
        project_root,
        &session,
        Some(
            "Started a host-native Brainstorming session from your latest slash-command input."
                .to_string(),
        ),
    )
}

fn continue_host_run_after_brainstorm(
    project_root: &Path,
    session: &BrainstormSession,
    host: HostKind,
    preface: Option<String>,
    allow_remote_finish: bool,
) -> Result<String> {
    let proposal = match load_latest_proposal(project_root)? {
        Some(existing)
            if existing.goal == session.goal
                && has_bound_spec_contract(project_root, &existing.id)? =>
        {
            existing
        }
        _ => create_proposal_from_brainstorm(project_root, session)?,
    };

    ensure_spec_execution_ready(project_root, session, &proposal)?;
    let execution = execute_proposal(
        project_root,
        proposal,
        &session.goal,
        host,
        allow_remote_finish,
    )?;

    let mut lines = Vec::new();
    if let Some(preface) = preface {
        lines.push(preface);
    }
    lines.push("Brainstorming reached Ready, the OpenSpec bundle is bound, and Zero_Nine automatically continued into guarded execution from the same host run command.".to_string());
    lines.push(execution);
    Ok(lines.join("\n\n"))
}

fn render_host_brainstorm_status(
    project_root: &Path,
    session: &BrainstormSession,
    preface: Option<String>,
) -> Result<String> {
    if matches!(session.verdict, BrainstormVerdict::Ready)
        && next_brainstorm_question(session).is_none()
    {
        let finalized = finalize_brainstorm(project_root, session)?;
        let mut lines = Vec::new();
        if let Some(preface) = preface {
            lines.push(preface);
        }
        lines.push(finalized);
        lines.push(format!(
            "Session files: {} | {}",
            project_root
                .join(".zero_nine/brainstorm/latest-session.md")
                .display(),
            project_root
                .join(".zero_nine/brainstorm/latest-session.json")
                .display()
        ));
        lines.push(
            "Brainstorming is Ready. From the same host workspace, switch to the execution entry point only after reviewing the generated proposal directory and confirmed goal wording.".to_string(),
        );
        return Ok(lines.join("\n"));
    }

    let next = next_brainstorm_question(session);
    let answered = session
        .questions
        .iter()
        .filter(|item| item.answered)
        .count();
    let total = session.questions.len();
    let mut lines = vec![
        format!("Brainstorm session: {}", session.id),
        format!("Goal: {}", session.goal),
        format!("Verdict: {:?}", session.verdict),
        format!(
            "Progress: {}/{} clarification questions answered",
            answered, total
        ),
        format!(
            "Session files: {} | {}",
            project_root
                .join(".zero_nine/brainstorm/latest-session.md")
                .display(),
            project_root
                .join(".zero_nine/brainstorm/latest-session.json")
                .display()
        ),
    ];
    if let Some(preface) = preface {
        lines.push(preface);
    }
    if let Some(question) = next {
        lines.push("Next clarification question:".to_string());
        lines.push(question.question);
        lines.push(format!("Rationale: {}", question.rationale));
        lines.push(
            "Reply by invoking the same host Zero_Nine command again with only your answer to the question above. Zero_Nine will append that answer to the active session and continue the loop without starting a new goal.".to_string(),
        );
    } else {
        lines.push("No unanswered clarification questions remain, but the session is not yet Ready. Review clarifications.md and session notes before forcing execution.".to_string());
    }
    Ok(lines.join("\n"))
}

pub fn run_goal(
    project_root: &Path,
    goal: &str,
    host: HostKind,
    allow_remote_finish: bool,
) -> Result<String> {
    initialize_project(project_root, host.clone())?;

    if matches!(host, HostKind::ClaudeCode | HostKind::OpenCode) {
        if let Some(session) = load_latest_brainstorm_session(project_root)? {
            if session.host == host && !matches!(session.verdict, BrainstormVerdict::Ready) {
                return brainstorm_host_turn_internal(
                    project_root,
                    goal,
                    host,
                    true,
                    allow_remote_finish,
                );
            }
        }
    }

    let session = ensure_brainstorm_ready(project_root, goal, host.clone())?;
    let Some(session) = session else {
        return match host {
            HostKind::ClaudeCode | HostKind::OpenCode => {
                brainstorm_host_turn(project_root, goal, host)
            }
            HostKind::Terminal => Ok(host_brainstorm_pause_message(project_root, goal, host)),
        };
    };

    let proposal = match load_latest_proposal(project_root)? {
        Some(existing)
            if existing.goal == goal && has_bound_spec_contract(project_root, &existing.id)? =>
        {
            existing
        }
        _ => create_proposal_from_brainstorm(project_root, &session)?,
    };

    ensure_spec_execution_ready(project_root, &session, &proposal)?;
    execute_proposal(project_root, proposal, goal, host, allow_remote_finish)
}

pub fn resume(project_root: &Path, host: HostKind, allow_remote_finish: bool) -> Result<String> {
    if let Some(session) = load_latest_brainstorm_session(project_root)? {
        if !matches!(session.verdict, BrainstormVerdict::Ready) {
            return brainstorm(project_root, None, host, true);
        }
    }

    if let Some(proposal) = load_latest_proposal(project_root)? {
        if let Some(session) = load_latest_brainstorm_session(project_root)? {
            ensure_spec_execution_ready(project_root, &session, &proposal)?;
        }
        if proposal.tasks.iter().any(|task| {
            matches!(
                task.status,
                TaskStatus::Pending | TaskStatus::Failed | TaskStatus::Running
            )
        }) {
            return execute_proposal(
                project_root,
                proposal.clone(),
                &proposal.goal,
                host,
                allow_remote_finish,
            );
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

pub fn validate_spec(project_root: &Path) -> Result<String> {
    let proposal = load_latest_proposal(project_root)?
        .ok_or_else(|| anyhow!("no proposal found to validate"))?;
    let mut report = validate_proposal_spec(project_root, &proposal)?;
    let completeness_issues = check_spec_completeness(project_root, &proposal)?;
    report.issues.extend(completeness_issues);
    let path = write_spec_validation_report(project_root, &proposal)?;

    let mut lines = Vec::new();
    lines.push(format!("proposal: {}", proposal.id));
    lines.push(format!("valid: {}", report.valid));
    lines.push(format!("report: {}", path.display()));
    if report.issues.is_empty() {
        lines.push("issues: none".to_string());
    } else {
        lines.push("issues:".to_string());
        for issue in report.issues {
            let fix = issue
                .suggested_fix
                .as_deref()
                .map(|s| format!(" — fix: {s}"))
                .unwrap_or_default();
            lines.push(format!(
                "- [{:?}] {} @ {} — {}{}",
                issue.severity, issue.code, issue.path, issue.message, fix
            ));
        }
    }
    Ok(lines.join("\n"))
}

fn ensure_brainstorm_ready(
    project_root: &Path,
    goal: &str,
    host: HostKind,
) -> Result<Option<BrainstormSession>> {
    if let Some(session) = load_latest_brainstorm_session(project_root)? {
        if session.goal == goal {
            if matches!(session.verdict, BrainstormVerdict::Ready) {
                return Ok(Some(session));
            }
            return match host {
                HostKind::Terminal => {
                    let _ = brainstorm(project_root, None, host, true)?;
                    Ok(load_latest_brainstorm_session(project_root)?)
                }
                HostKind::ClaudeCode | HostKind::OpenCode => Ok(None),
            };
        }
    }

    match host {
        HostKind::Terminal => {
            let _ = brainstorm(project_root, Some(goal), host, false)?;
            Ok(load_latest_brainstorm_session(project_root)?)
        }
        HostKind::ClaudeCode | HostKind::OpenCode => {
            let _ = brainstorm(project_root, Some(goal), host, false)?;
            Ok(None)
        }
    }
}

fn host_brainstorm_pause_message(project_root: &Path, goal: &str, host: HostKind) -> String {
    let host_label = match host {
        HostKind::ClaudeCode => "Claude Code",
        HostKind::OpenCode => "OpenCode",
        HostKind::Terminal => "terminal",
    };
    format!(
        "Zero_Nine paused before execution because Brainstorming is not yet Ready for goal: {goal}. Continue inside {host_label} by invoking the same Zero_Nine host command again with only the answer to the latest clarification question. Session snapshot: {}",
        project_root.join(".zero_nine/brainstorm/latest-session.md").display()
    )
}

fn ensure_spec_execution_ready(
    project_root: &Path,
    session: &BrainstormSession,
    proposal: &Proposal,
) -> Result<()> {
    if !matches!(session.verdict, BrainstormVerdict::Ready) {
        return Err(anyhow!(
            "cannot execute proposal {} because the brainstorm session {} is not Ready",
            proposal.id,
            session.id
        ));
    }
    if session.goal != proposal.goal {
        return Err(anyhow!(
            "cannot execute proposal {} because it does not match the latest Ready brainstorm goal",
            proposal.id
        ));
    }
    if !matches!(proposal.status, ProposalStatus::Ready) {
        return Err(anyhow!(
            "cannot execute proposal {} because its status is {:?} instead of Ready",
            proposal.id,
            proposal.status
        ));
    }
    let validation_report = validate_proposal_spec(project_root, proposal)?;
    if !validation_report.valid {
        write_spec_validation_report(project_root, proposal)?;
        return Err(anyhow!(
            "cannot execute proposal {} because the bound OpenSpec contract failed validation with {} issue(s)",
            proposal.id,
            validation_report.issues.len()
        ));
    }
    Ok(())
}

fn has_bound_spec_contract(project_root: &Path, proposal_id: &str) -> Result<bool> {
    let Some(proposal) = load_latest_proposal(project_root)? else {
        return Ok(false);
    };
    if proposal.id != proposal_id {
        return Ok(false);
    }

    let report = validate_proposal_spec(project_root, &proposal)?;
    Ok(report.valid)
}

fn run_terminal_brainstorm(project_root: &Path, session: &mut BrainstormSession) -> Result<()> {
    let mut rl = DefaultEditor::new()?;

    while let Some(question) = next_brainstorm_question(session) {
        println!("\n[Zero_Nine Brainstorming] {}", question.question);
        println!("Why this matters: {}", question.rationale);

        let answer = rl.readline("> ")?;
        rl.add_history_entry(&answer)?;
        let verdict = answer_brainstorm_question(session, &question.id, &answer)?;
        save_brainstorm_session(project_root, session)?;
        append_event(
            project_root,
            RuntimeEvent::new(
                "brainstorm.answered".to_string(),
                Some(json!({
                    "session_id": session.id,
                    "question_id": question.id,
                    "verdict": verdict,
                })),
            ),
        )?;
    }
    Ok(())
}

fn finalize_brainstorm(project_root: &Path, session: &BrainstormSession) -> Result<String> {
    save_brainstorm_session(project_root, session)?;
    append_event(
        project_root,
        RuntimeEvent::new(
            "brainstorm.finalized".to_string(),
            Some(json!({
                "session_id": session.id,
                "goal": session.goal,
                "verdict": session.verdict,
            })),
        ),
    )?;

    if matches!(session.verdict, BrainstormVerdict::Ready) {
        let proposal = match load_latest_proposal(project_root)? {
            Some(existing)
                if existing.goal == session.goal
                    && has_bound_spec_contract(project_root, &existing.id)? =>
            {
                existing
            }
            _ => create_proposal_from_brainstorm(project_root, session)?,
        };
        Ok(format!(
            "Brainstorming converged and OpenSpec artifacts were written under {} for proposal {}.",
            proposal_dir(project_root, &proposal.id).display(),
            proposal.id
        ))
    } else {
        Ok(format!(
            "Brainstorming ended with verdict {:?}. Review the saved session under {} before execution.",
            session.verdict,
            project_root.join(".zero_nine/brainstorm/latest-session.md").display()
        ))
    }
}

fn execute_proposal(
    project_root: &Path,
    mut proposal: Proposal,
    goal: &str,
    host: HostKind,
    allow_remote_finish: bool,
) -> Result<String> {
    if !matches!(proposal.status, ProposalStatus::Ready) {
        return Err(anyhow!(
            "proposal {} must be Ready before execution can start; current status is {:?}",
            proposal.id,
            proposal.status
        ));
    }

    let manifest = load_manifest(project_root)?.unwrap_or_default();
    let max_retries = manifest.policy.max_retries;
    let max_total_iterations = manifest.policy.max_total_iterations.unwrap_or(50);
    let max_elapsed_seconds = manifest.policy.max_elapsed_seconds.unwrap_or(3600);
    let loop_start = Utc::now();

    // M9: Read subagent execution path config — supports env override for independent deployment
    let bridge_address = manifest.bridge_address.clone();
    let execution_path = std::env::var("ZN_EXECUTION_PATH")
        .ok()
        .and_then(|s| match s.as_str() {
            "bridge" => Some(zn_types::SubagentExecutionPath::Bridge),
            "hybrid" => Some(zn_types::SubagentExecutionPath::Hybrid),
            _ => Some(zn_types::SubagentExecutionPath::Cli),
        })
        .unwrap_or_default();

    // M8: Initialize policy engine for safety governance
    let policy_engine = zn_exec::PolicyEngine::new(project_root)?;

    proposal.status = ProposalStatus::Running;
    save_proposal(project_root, &proposal)?;

    let mut state = init_loop_state(&proposal.id);
    transition_state(
        project_root,
        &mut state,
        LoopStage::Ready,
        "proposal_started",
    );
    state.retry_count = 0;
    save_loop_state(project_root, &state)?;

    let mut halted = false;
    let mut halt_reason: Option<String> = None;

    // M8: Global token budget check before entering execution loop
    let estimated_tokens = proposal.tasks.len() as u64 * 5000; // rough per-task estimate
    let budget_check = policy_engine.check_token_budget(estimated_tokens);
    if budget_check.blocked {
        append_event(
            project_root,
            RuntimeEvent::new(
                "proposal.halted".to_string(),
                Some(json!({
                    "proposal_id": proposal.id,
                    "reason": "token_budget_exceeded",
                    "estimated": estimated_tokens,
                    "remaining": budget_check.remaining_tokens,
                })),
            )
            .with_context(Some(proposal.id.clone()), None),
        )?;
        return Ok(format!(
            "Execution halted: token budget exceeded (estimated {} tokens, {} remaining)",
            estimated_tokens, budget_check.remaining_tokens
        ));
    }

    // P0-1: Drift detection — verify workspace state before execution
    if let Some(first_pending) = proposal.tasks.iter().find(|t| {
        matches!(
            t.status,
            TaskStatus::Pending | TaskStatus::Running | TaskStatus::Failed
        )
    }) {
        let plan = build_plan(first_pending);
        let desired = zn_exec::drift::build_desired_state(&proposal, &plan);
        let actual = zn_exec::drift::capture_actual_state(project_root);
        let check = zn_exec::drift::check_drift(&desired, &actual);
        if check.blocking {
            append_event(
                project_root,
                RuntimeEvent::new(
                    "proposal.paused".to_string(),
                    Some(json!({
                        "proposal_id": proposal.id,
                        "reason": "blocking_drift",
                        "drift_summary": check.report.summary,
                    })),
                )
                .with_context(Some(proposal.id.clone()), None),
            )?;
            return Ok(format!(
                "Execution halted due to project drift: {}",
                check.report.summary
            ));
        }
    }

    // M5: Global budget check before entering execution loop
    loop {
        let elapsed = (Utc::now() - loop_start).num_seconds() as u64;
        if state.iteration >= max_total_iterations {
            halt_reason = Some(format!(
                "Global budget exceeded: max {} iterations reached (current: {})",
                max_total_iterations, state.iteration
            ));
            break;
        }
        if elapsed >= max_elapsed_seconds {
            halt_reason = Some(format!(
                "Time budget exceeded: {}s elapsed (limit: {}s)",
                elapsed, max_elapsed_seconds
            ));
            break;
        }

        let Some(schedule) = choose_next_ready_batch(&proposal.tasks, max_retries) else {
            break;
        };
        if schedule.selected_indices.is_empty() {
            halt_reason = Some(schedule.summary);
            break;
        }

        write_spec_validation_report(project_root, &proposal)?;
        let bundle = spec_bundle(project_root, &proposal.id);
        let mut base_context_files = vec![
            bundle.proposal_path.clone(),
            bundle.requirements_path.clone(),
            bundle.acceptance_path.clone(),
            bundle.design_path.clone(),
            bundle.tasks_path.clone(),
            bundle.dag_path.clone(),
            bundle.progress_path.clone(),
            bundle.verification_path.clone(),
        ];
        base_context_files.push(
            proposal_dir(project_root, &proposal.id)
                .join("spec-validation.json")
                .display()
                .to_string(),
        );

        let mut batch_runtimes = Vec::new();
        let batch_task_ids = schedule
            .selected_indices
            .iter()
            .map(|index| proposal.tasks[*index].id.clone())
            .collect::<Vec<_>>();

        for index in &schedule.selected_indices {
            let task = proposal.tasks[*index].clone();
            let retry_count = retry_count_for_task(&task);
            state.iteration += 1;
            proposal.tasks[*index].status = TaskStatus::Running;
            batch_runtimes.push(BatchTaskRuntime {
                index: *index,
                task: task.clone(),
                plan: {
                    let mut plan = build_plan_with_config(
                        &task,
                        execution_path.clone(),
                        bridge_address.clone(),
                    );
                    if let Some(guidance) = zn_evolve::consume_candidates(project_root, &task.id) {
                        plan.risks.extend(
                            guidance
                                .fix_instructions
                                .iter()
                                .map(|f| format!("[EVOLUTION-FIX] {}", f)),
                        );
                        plan.risks.extend(
                            guidance
                                .improve_patterns
                                .iter()
                                .map(|p| format!("[EVOLUTION-IMPROVE] {}", p)),
                        );
                        plan.skill_chain.extend(
                            guidance
                                .fix_instructions
                                .iter()
                                .map(|f| format!("[FIX] {}", f)),
                        );
                        plan.skill_chain.extend(
                            guidance
                                .improve_patterns
                                .iter()
                                .map(|p| format!("[IMPROVE] {}", p)),
                        );
                        info!(
                            "Evolution injected for task {}: {} fix, {} improve (conf={:.2})",
                            task.id,
                            guidance.fix_instructions.len(),
                            guidance.improve_patterns.len(),
                            guidance.confidence,
                        );
                    }
                    plan
                },
                retry_count,
                iteration_label: state.iteration.to_string(),
            });
        }

        state.current_task = Some(format!("batch:{}", batch_task_ids.join(",")));
        let next_stage = if batch_runtimes.iter().any(|runtime| runtime.retry_count > 0) {
            LoopStage::Retrying
        } else {
            LoopStage::RunningTask
        };
        transition_state(project_root, &mut state, next_stage, "batch_started");
        state.retry_count = batch_runtimes
            .iter()
            .map(|runtime| runtime.retry_count)
            .max()
            .unwrap_or(0);
        state.updated_at = Utc::now();
        proposal.updated_at = Utc::now();
        update_progress_markdown(project_root, &proposal)?;
        save_proposal(project_root, &proposal)?;
        save_loop_state(project_root, &state)?;

        for runtime in &batch_runtimes {
            let envelope = build_execution_envelope(
                &proposal.id,
                &runtime.task,
                host.clone(),
                base_context_files.clone(),
            );
            let envelope =
                save_execution_envelope(project_root, &proposal.id, &runtime.task.id, &envelope)?;

            append_event(
                project_root,
                RuntimeEvent::new(
                    "task.started".to_string(),
                    Some(json!({
                        "title": runtime.task.title,
                        "execution_mode": envelope.execution_mode,
                        "workspace_strategy": envelope.workspace_strategy,
                        "quality_gates": envelope.quality_gates,
                        "context_protocol": envelope.context_protocol,
                        "context_protocol_path": envelope.context_protocol_path,
                        "retry_count": runtime.retry_count,
                        "max_retries": max_retries,
                        "scheduler_parallel_window": schedule.parallel_window,
                        "scheduler_resource_summary": schedule.resource_summary,
                        "scheduler_retry_priority": schedule.retry_priority,
                        "scheduler_runnable_tasks": schedule.runnable_tasks,
                        "batch_execution": true,
                    })),
                )
                .with_context(Some(proposal.id.clone()), Some(runtime.task.id.clone())),
            )?;
        }

        let batch_results = run_ready_batch(project_root, &batch_runtimes, allow_remote_finish);
        for output in batch_results {
            let task = output.task;
            let plan = output.plan;
            let report = output.report;
            let retry_count = output.retry_count;
            let proposal_path = proposal_dir(project_root, &proposal.id);
            let artifacts_dir = proposal_path
                .join("artifacts")
                .join(format!("task-{}", task.id));
            fs::create_dir_all(&artifacts_dir)?;

            let mut event_name = "task.completed".to_string();
            match report.outcome {
                ExecutionOutcome::Completed => {
                    proposal.tasks[output.index].status = TaskStatus::Completed;
                    transition_state(
                        project_root,
                        &mut state,
                        LoopStage::Verifying,
                        "task_completed",
                    );
                    state.retry_count = 0;
                }
                ExecutionOutcome::RetryableFailure => {
                    let should_retry = report
                        .failure_classification
                        .as_ref()
                        .map(|c| c.retry_recommended)
                        .unwrap_or(true);
                    if !should_retry {
                        proposal.tasks[output.index].status = TaskStatus::Failed;
                        transition_state(
                            project_root,
                            &mut state,
                            LoopStage::Escalated,
                            "retry_not_recommended",
                        );
                        state.retry_count = retry_count;
                        event_name = "task.escalated".to_string();
                        if halt_reason.is_none() {
                            halt_reason =
                                Some(report.failure_summary.clone().unwrap_or_else(|| {
                                    format!(
                                        "Task {} exceeded the retry budget of {}.",
                                        task.id, max_retries
                                    )
                                }));
                        }
                        halted = true;
                    } else if retry_count < task.max_retries.unwrap_or(max_retries) {
                        proposal.tasks[output.index].status = TaskStatus::Pending;
                        transition_state(
                            project_root,
                            &mut state,
                            LoopStage::Retrying,
                            "retry_scheduled",
                        );
                        state.retry_count = retry_count.saturating_add(1);
                        event_name = "task.retry_scheduled".to_string();
                    } else {
                        proposal.tasks[output.index].status = TaskStatus::Failed;
                        transition_state(
                            project_root,
                            &mut state,
                            LoopStage::Escalated,
                            "retry_budget_exceeded",
                        );
                        state.retry_count = retry_count;
                        event_name = "task.escalated".to_string();
                        if halt_reason.is_none() {
                            let effective_max = task.max_retries.unwrap_or(max_retries);
                            halt_reason =
                                Some(report.failure_summary.clone().unwrap_or_else(|| {
                                    format!(
                                        "Task {} exceeded the retry budget of {}.",
                                        task.id, effective_max
                                    )
                                }));
                        }
                        halted = true;
                    }
                }
                ExecutionOutcome::Blocked => {
                    proposal.tasks[output.index].status = TaskStatus::Blocked;
                    transition_state(
                        project_root,
                        &mut state,
                        LoopStage::Escalated,
                        "task_blocked",
                    );
                    state.retry_count = retry_count;
                    event_name = "task.blocked".to_string();
                    if halt_reason.is_none() {
                        halt_reason = Some(report.failure_summary.clone().unwrap_or_else(|| {
                            format!(
                                "Task {} is blocked and requires manual intervention.",
                                task.id
                            )
                        }));
                    }
                    halted = true;
                }
                ExecutionOutcome::Escalated => {
                    proposal.tasks[output.index].status = TaskStatus::Failed;
                    transition_state(
                        project_root,
                        &mut state,
                        LoopStage::Escalated,
                        "manual_intervention",
                    );
                    state.retry_count = retry_count;
                    event_name = "task.escalated".to_string();
                    if halt_reason.is_none() {
                        halt_reason = Some(report.failure_summary.clone().unwrap_or_else(|| {
                            format!("Task {} escalated to manual intervention.", task.id)
                        }));
                    }
                    halted = true;
                }
            }

            proposal.updated_at = Utc::now();
            state.current_task = Some(report.task_id.clone());
            state.updated_at = Utc::now();
            update_progress_markdown(project_root, &proposal)?;
            save_proposal(project_root, &proposal)?;
            save_loop_state(project_root, &state)?;

            persist_generated_artifacts(&artifacts_dir, &report)?;
            persist_iteration_log(project_root, &task.id, &plan, &report)?;

            fs::write(
                proposal_path.join(format!("task-{}-report.md", report.task_id)),
                render_task_report(&task.title, &plan, &report),
            )?;

            // Belief system update — feed execution outcome into belief tracker
            if let Err(e) = zn_evolve::belief::update_belief_from_report(project_root, &report) {
                info!("Belief update skipped: {}", e);
            }

            // Safety event emission — record policy/guardrail triggers
            if let Some(safety_event) = SafetyEvent::from_report(&report, &proposal.id) {
                persist_safety_event(project_root, &safety_event)?;
            }

            let evaluation = evaluate(&report);
            let mut evals = OpenOptions::new()
                .create(true)
                .append(true)
                .open(project_root.join(".zero_nine/evolve/evaluations.jsonl"))?;
            writeln!(evals, "{}", serde_json::to_string(&evaluation)?)?;

            // Update reward model from execution report
            let reward_result = (|| -> Result<()> {
                let mut reward_model = RewardModel::new(
                    project_root.join(".zero_nine/evolve/pairwise_comparisons.ndjson"),
                )?;
                reward_model.record_from_report(&report);
                reward_model.save()?;
                Ok(())
            })();

            if let Err(e) = reward_result {
                // Log error but don't fail the execution
                info!("Reward model update failed: {}", e);
            }

            if let Some(candidate) = propose_candidate(&report) {
                let path = project_root
                    .join(".zero_nine/evolve/candidates")
                    .join(format!(
                        "{}-{}.json",
                        report.task_id, output.iteration_label
                    ));
                fs::write(path, serde_json::to_vec_pretty(&candidate)?)?;
            }

            append_event(
                project_root,
                RuntimeEvent::new(
                    event_name,
                    Some(json!({
                        "exit_code": report.exit_code,
                        "outcome": report.outcome,
                        "failure_summary": report.failure_summary,
                        "artifacts": report.artifacts,
                        "follow_ups": report.follow_ups,
                        "tests_passed": report.tests_passed,
                        "review_passed": report.review_passed,
                        "quality_gate_count": plan.quality_gates.len(),
                        "subagent_count": plan.subagents.len(),
                        "workspace_record": report.workspace_record,
                        "finish_branch_result": report.finish_branch_result,
                        "finish_branch_automation": report.finish_branch_automation,
                        "review_verdict": report.review_verdict,
                        "verification_verdict": report.verification_verdict,
                        "verification_actions": report.verification_actions,
                        "verification_action_results": report.verification_action_results,
                        "retry_count": state.retry_count,
                        "max_retries": max_retries,
                        "batch_execution": true,
                    })),
                )
                .with_context(Some(proposal.id.clone()), Some(report.task_id.clone())),
            )?;
        }

        if halted {
            break;
        }
        // Update elapsed time tracking
        state.elapsed_seconds = (Utc::now() - loop_start).num_seconds() as u64;
    }

    // M5: Execute compensation actions on halt
    if halted {
        execute_compensation_actions(project_root, &proposal)?;
    }

    if !halted {
        if let Some(reason) = blocked_dependency_summary(&proposal.tasks, max_retries) {
            halt_reason = Some(reason);
        }
    }

    let completed = proposal
        .tasks
        .iter()
        .all(|task| matches!(task.status, TaskStatus::Completed));
    proposal.status = if completed {
        ProposalStatus::Completed
    } else {
        ProposalStatus::Ready
    };
    proposal.updated_at = Utc::now();
    save_proposal(project_root, &proposal)?;
    update_progress_markdown(project_root, &proposal)?;
    state.current_task = None;
    state.updated_at = Utc::now();
    let final_stage = if completed {
        LoopStage::Completed
    } else {
        LoopStage::Escalated
    };
    transition_state(
        project_root,
        &mut state,
        final_stage,
        if completed {
            "proposal_completed"
        } else {
            "proposal_escalated"
        },
    );
    save_loop_state(project_root, &state)?;

    if completed {
        fs::write(
            proposal_dir(project_root, &proposal.id).join("verification.md"),
            render_proposal_verification(goal, &proposal.id, &proposal.tasks),
        )?;

        // M8: Enforce merge safety before finalizing completed proposal
        if let Err(_msg) = policy_engine.enforce_merge_safety(true, true) {
            let event = SafetyEvent::merge_blocked("proposal", &proposal.id, true, true);
            persist_safety_event(project_root, &event)?;
        }

        append_event(
            project_root,
            RuntimeEvent::new(
                "proposal.completed".to_string(),
                Some(json!({"goal": goal, "iterations": state.iteration})),
            )
            .with_context(Some(proposal.id.clone()), None),
        )?;

        // M6: Auto writeback to GitHub Issue if proposal came from GitHub
        sync_proposal_to_github(project_root, &proposal)?;

        let summary = status_summary(project_root)?;
        Ok(format!(
            "Zero_Nine completed an enhanced run for goal: {goal}\n\n{summary}"
        ))
    } else {
        append_event(
            project_root,
            RuntimeEvent::new(
                "proposal.paused".to_string(),
                Some(json!({
                    "goal": goal,
                    "iterations": state.iteration,
                    "retry_count": state.retry_count,
                    "reason": halt_reason,
                })),
            )
            .with_context(Some(proposal.id.clone()), None),
        )?;

        // M6: Auto writeback even on pause/failure
        sync_proposal_to_github(project_root, &proposal)?;

        let summary = status_summary(project_root)?;
        Ok(format!(
            "Zero_Nine paused execution for goal: {goal}\n\nReason: {}\n\n{summary}",
            halt_reason.unwrap_or_else(
                || "Execution requires manual review before continuing.".to_string()
            )
        ))
    }
}

#[derive(Debug, Clone)]
struct BatchTaskRuntime {
    index: usize,
    task: zn_types::TaskItem,
    plan: ExecutionPlan,
    retry_count: u8,
    iteration_label: String,
}

#[derive(Debug, Clone)]
struct BatchTaskExecutionOutput {
    index: usize,
    task: zn_types::TaskItem,
    plan: ExecutionPlan,
    retry_count: u8,
    iteration_label: String,
    report: ExecutionReport,
}

fn run_ready_batch(
    project_root: &Path,
    batch_runtimes: &[BatchTaskRuntime],
    allow_remote_finish: bool,
) -> Vec<BatchTaskExecutionOutput> {
    let project_root = project_root.to_path_buf();
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for runtime in batch_runtimes.iter().cloned() {
            let runtime_for_thread = runtime.clone();
            let thread_project_root = project_root.clone();
            handles.push((
                runtime,
                scope.spawn(move || {
                    execute_task_attempt(
                        &thread_project_root,
                        runtime_for_thread,
                        allow_remote_finish,
                    )
                }),
            ));
        }

        let mut outputs = Vec::new();
        for (runtime, handle) in handles {
            match handle.join() {
                Ok(output) => outputs.push(output),
                Err(_) => outputs.push(panic_batch_execution_output(&runtime)),
            }
        }
        outputs
    })
}

fn execute_task_attempt(
    project_root: &Path,
    runtime: BatchTaskRuntime,
    allow_remote_finish: bool,
) -> BatchTaskExecutionOutput {
    match prepare_workspace(project_root, &runtime.plan) {
        Ok(workspace_preparation) => match execute_plan(
            project_root,
            &runtime.task,
            &runtime.plan,
            workspace_preparation.record.clone(),
            allow_remote_finish,
        ) {
            Ok(mut report) => {
                report.workspace_record = workspace_preparation.record.clone();
                report.details.insert(
                    0,
                    format!("Workspace preparation: {}", workspace_preparation.summary),
                );
                for created_path in workspace_preparation.created_paths {
                    report.artifacts.push(created_path);
                }
                BatchTaskExecutionOutput {
                    index: runtime.index,
                    task: runtime.task,
                    plan: runtime.plan,
                    retry_count: runtime.retry_count,
                    iteration_label: runtime.iteration_label,
                    report,
                }
            }
            Err(error) => BatchTaskExecutionOutput {
                index: runtime.index,
                task: runtime.task.clone(),
                plan: runtime.plan.clone(),
                retry_count: runtime.retry_count,
                iteration_label: runtime.iteration_label.clone(),
                report: infrastructure_failure_report(
                    &runtime.task,
                    &runtime.plan,
                    format!("execute_plan failed: {}", error),
                    workspace_preparation.record,
                    vec![format!(
                        "Workspace preparation succeeded before execution failed: {}",
                        workspace_preparation.summary
                    )],
                ),
            },
        },
        Err(error) => BatchTaskExecutionOutput {
            index: runtime.index,
            task: runtime.task.clone(),
            plan: runtime.plan.clone(),
            retry_count: runtime.retry_count,
            iteration_label: runtime.iteration_label.clone(),
            report: infrastructure_failure_report(
                &runtime.task,
                &runtime.plan,
                format!("prepare_workspace failed: {}", error),
                None,
                Vec::new(),
            ),
        },
    }
}

fn panic_batch_execution_output(runtime: &BatchTaskRuntime) -> BatchTaskExecutionOutput {
    BatchTaskExecutionOutput {
        index: runtime.index,
        task: runtime.task.clone(),
        plan: runtime.plan.clone(),
        retry_count: runtime.retry_count,
        iteration_label: runtime.iteration_label.clone(),
        report: infrastructure_failure_report(
            &runtime.task,
            &runtime.plan,
            "batch worker panicked before Zero_Nine could recover a normal execution report"
                .to_string(),
            None,
            vec![format!(
                "Threaded execution for task {} terminated unexpectedly while the batch executor was active.",
                runtime.task.id
            )],
        ),
    }
}

fn infrastructure_failure_report(
    task: &zn_types::TaskItem,
    plan: &ExecutionPlan,
    failure_summary: String,
    workspace_record: Option<zn_types::WorkspaceRecord>,
    extra_details: Vec<String>,
) -> ExecutionReport {
    let mut details = vec![format!(
        "Execution infrastructure failure: {}",
        failure_summary
    )];
    details.extend(extra_details);
    ExecutionReport {
        task_id: task.id.clone(),
        success: false,
        outcome: ExecutionOutcome::Escalated,
        summary: format!(
            "Task {} ended in infrastructure escalation before the normal execution report could be completed.",
            task.id
        ),
        details,
        tests_passed: false,
        review_passed: false,
        artifacts: vec![],
        generated_artifacts: vec![],
        evidence: vec![],
        follow_ups: vec![
            "Inspect runtime logs, workspace preparation artifacts, and recovery ledgers before retrying this task."
                .to_string(),
        ],
        workspace_record,
        finish_branch_result: None,
        finish_branch_automation: plan.finish_branch_automation.clone(),
        agent_runs: vec![],
        review_verdict: None,
        verification_verdict: None,
        verification_actions: plan.verification_actions.clone(),
        verification_action_results: vec![],
        failure_summary: Some(failure_summary.clone()),
        exit_code: 1,
        execution_time_ms: 0,
        token_count: 0,
        code_quality_score: 0.0,
        test_coverage: 0.0,
        user_feedback: None,
        failure_classification: Some(FailureClassification {
            id: "infrastructure_escalation".to_string(),
            category: FailureCategory::ResourceExhausted,
            severity: FailureSeverity::High,
            description: failure_summary.to_string(),
            root_cause: Some("Infrastructure or runtime failure".to_string()),
            retry_recommended: false,
            human_intervention_required: true,
            suggested_fix: Some("Check execution environment and retry manually.".to_string()),
        }),
        tri_role_verdict: None,
        authorization_ticket_id: None,
        authorized_by: None,
    }
}

#[derive(Debug, Clone)]
struct ReadyBatch {
    selected_indices: Vec<usize>,
    summary: String,
    parallel_window: usize,
    resource_summary: String,
    retry_priority: bool,
    runnable_tasks: Vec<String>,
}

fn choose_next_ready_batch(tasks: &[zn_types::TaskItem], max_retries: u8) -> Option<ReadyBatch> {
    let completed: HashSet<&str> = tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Completed))
        .map(|task| task.id.as_str())
        .collect();

    let mut runnable = tasks
        .iter()
        .enumerate()
        .filter(|(_, task)| {
            matches!(
                task.status,
                TaskStatus::Pending | TaskStatus::Running | TaskStatus::Failed
            )
        })
        .filter(|(_, task)| {
            task.depends_on
                .iter()
                .all(|dependency| completed.contains(dependency.as_str()))
        })
        .filter(|(_, task)| {
            // P0-3: Check preconditions — task-ID references must be completed
            task.preconditions.iter().all(|pre| {
                // If the precondition looks like a task ID (matches an existing task),
                // require that task to be completed
                tasks.iter().any(|candidate| {
                    candidate.id == *pre && candidate.status == TaskStatus::Completed
                }) || !tasks.iter().any(|candidate| candidate.id == *pre)
                // If no task has this ID, it's a non-task precondition (skip)
            })
        })
        .collect::<Vec<_>>();

    if runnable.is_empty() {
        return None;
    }

    runnable.sort_by_key(|(_, task)| scheduler_priority(task));
    let parallel_window = scheduler_parallel_window(tasks);
    let max_worktree_slots = scheduler_worktree_slots(tasks);
    let max_finish_slots = 1usize;
    let mut selected_indices = Vec::new();
    let mut runnable_tasks = Vec::new();
    let mut worktree_slots_used = 0usize;
    let mut finish_slots_used = 0usize;
    let mut retry_priority = false;

    for (index, task) in runnable {
        let kind = normalized_task_kind(task);
        let estimated_retry = retry_count_for_task(task);
        let needs_worktree = matches!(
            kind.as_str(),
            "planning" | "execution" | "implementation" | "finish_branch"
        );
        let is_finish = matches!(kind.as_str(), "finish_branch");

        // P0-3: Use task-level max_retries if set, otherwise manifest default
        let task_max_retries = task.max_retries.unwrap_or(max_retries);
        if estimated_retry >= task_max_retries && matches!(task.status, TaskStatus::Failed) {
            continue;
        }
        if needs_worktree && worktree_slots_used >= max_worktree_slots {
            continue;
        }
        if is_finish && finish_slots_used >= max_finish_slots {
            continue;
        }
        if selected_indices.len() >= parallel_window {
            break;
        }

        if needs_worktree {
            worktree_slots_used += 1;
        }
        if is_finish {
            finish_slots_used += 1;
        }
        if estimated_retry > 0 || matches!(task.status, TaskStatus::Failed | TaskStatus::Running) {
            retry_priority = true;
        }
        runnable_tasks.push(format!("{}:{}", task.id, task.title));
        selected_indices.push(index);
    }

    if selected_indices.is_empty() {
        return Some(ReadyBatch {
            selected_indices,
            summary: blocked_dependency_summary(tasks, max_retries).unwrap_or_else(|| {
                "Execution paused because the scheduler found runnable tasks, but all were filtered out by retry budget or resource constraints.".to_string()
            }),
            parallel_window,
            resource_summary: format!(
                "parallel_window={}, worktree_slots={}, finish_slots={}",
                parallel_window, max_worktree_slots, max_finish_slots
            ),
            retry_priority,
            runnable_tasks,
        });
    }

    Some(ReadyBatch {
        summary: format!(
            "Selected {} runnable task(s) under a scheduler window of {} with worktree slots {} and finish slots {}.",
            selected_indices.len(), parallel_window, max_worktree_slots, max_finish_slots
        ),
        selected_indices,
        parallel_window,
        resource_summary: format!(
            "parallel_window={}, worktree_slots={}, finish_slots={}",
            parallel_window, max_worktree_slots, max_finish_slots
        ),
        retry_priority,
        runnable_tasks,
    })
}

fn blocked_dependency_summary(tasks: &[zn_types::TaskItem], max_retries: u8) -> Option<String> {
    let completed: HashSet<&str> = tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Completed))
        .map(|task| task.id.as_str())
        .collect();

    let blocked = tasks
        .iter()
        .filter(|task| {
            matches!(
                task.status,
                TaskStatus::Pending | TaskStatus::Running | TaskStatus::Failed
            )
        })
        .map(|task| {
            let unresolved = task
                .depends_on
                .iter()
                .filter(|dependency| !completed.contains(dependency.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            let retry_budget_exhausted = matches!(task.status, TaskStatus::Failed)
                && retry_count_for_task(task) >= max_retries;
            (task, unresolved, retry_budget_exhausted)
        })
        .filter(|(_, unresolved, retry_budget_exhausted)| {
            !unresolved.is_empty() || *retry_budget_exhausted
        })
        .collect::<Vec<_>>();

    if blocked.is_empty() {
        return None;
    }

    let detail = blocked
        .into_iter()
        .map(|(task, unresolved, _retry_budget_exhausted)| {
            if !unresolved.is_empty() {
                format!(
                    "{} is waiting on unresolved dependencies: {}",
                    task.id,
                    unresolved.join(", ")
                )
            } else {
                format!(
                    "{} exhausted the retry budget ({}/{}) and now requires manual intervention",
                    task.id,
                    retry_count_for_task(task),
                    max_retries
                )
            }
        })
        .collect::<Vec<_>>()
        .join("; ");

    Some(format!(
        "Execution paused because no task is runnable under the current DAG, retry, and resource constraints. {}.",
        detail
    ))
}

fn scheduler_parallel_window(tasks: &[zn_types::TaskItem]) -> usize {
    let runnable_count = tasks
        .iter()
        .filter(|task| {
            matches!(
                task.status,
                TaskStatus::Pending | TaskStatus::Running | TaskStatus::Failed
            )
        })
        .count();
    if runnable_count >= 3 {
        2
    } else {
        1
    }
}

fn scheduler_worktree_slots(tasks: &[zn_types::TaskItem]) -> usize {
    let has_finish = tasks
        .iter()
        .any(|task| normalized_task_kind(task) == "finish_branch");
    if has_finish {
        1
    } else {
        2
    }
}

fn normalized_task_kind(task: &zn_types::TaskItem) -> String {
    task.kind
        .clone()
        .unwrap_or_else(|| task.title.to_lowercase().replace(' ', "_"))
        .to_lowercase()
}

fn retry_count_for_task(task: &zn_types::TaskItem) -> u8 {
    match task.status {
        TaskStatus::Failed => 1,
        TaskStatus::Running => 1,
        _ => 0,
    }
}

fn scheduler_priority(task: &zn_types::TaskItem) -> (u8, u8, String) {
    let retry_bias = if matches!(task.status, TaskStatus::Failed | TaskStatus::Running) {
        0
    } else {
        1
    };
    let resource_bias = match normalized_task_kind(task).as_str() {
        "finish_branch" => 3,
        "verification" => 2,
        "execution" | "implementation" => 1,
        _ => 0,
    };
    (retry_bias, resource_bias, task.id.clone())
}

fn save_execution_envelope(
    project_root: &Path,
    proposal_id: &str,
    task_id: &str,
    envelope: &ExecutionEnvelope,
) -> Result<ExecutionEnvelope> {
    let runtime_path = project_root.join(".zero_nine/runtime");
    fs::create_dir_all(&runtime_path)?;

    let mut enriched = envelope.clone();
    if let Some(protocol) = &enriched.context_protocol {
        let runtime_protocol_path = runtime_path.join("current-context-protocol.json");
        let proposal_protocol_path = proposal_dir(project_root, proposal_id)
            .join(format!("task-{}-context-protocol.json", task_id));
        fs::write(&runtime_protocol_path, serde_json::to_vec_pretty(protocol)?)?;
        fs::write(
            &proposal_protocol_path,
            serde_json::to_vec_pretty(protocol)?,
        )?;
        enriched.context_protocol_path = Some(runtime_protocol_path.display().to_string());
    }

    fs::write(
        runtime_path.join("current-envelope.json"),
        serde_json::to_vec_pretty(&enriched)?,
    )?;
    fs::write(
        proposal_dir(project_root, proposal_id).join(format!("task-{}-envelope.json", task_id)),
        serde_json::to_vec_pretty(&enriched)?,
    )?;
    Ok(enriched)
}

fn persist_generated_artifacts(artifacts_dir: &Path, report: &ExecutionReport) -> Result<()> {
    for artifact in &report.generated_artifacts {
        fs::write(artifacts_dir.join(&artifact.path), &artifact.content)?;
    }
    Ok(())
}

fn persist_iteration_log(
    project_root: &Path,
    task_id: &str,
    plan: &ExecutionPlan,
    report: &ExecutionReport,
) -> Result<()> {
    let log_path = project_root.join(".zero_nine/loop/iterations.jsonl");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let record = json!({
        "task_id": task_id,
        "mode": plan.mode,
        "workspace_strategy": plan.workspace_strategy,
        "quality_gates": plan.quality_gates,
        "artifacts": report.artifacts,
        "tests_passed": report.tests_passed,
        "review_passed": report.review_passed,
        "follow_ups": report.follow_ups,
        "workspace_record": report.workspace_record,
        "finish_branch_result": report.finish_branch_result,
        "finish_branch_automation": report.finish_branch_automation,
        "review_verdict": report.review_verdict,
        "verification_verdict": report.verification_verdict,
        "verification_actions": report.verification_actions,
        "verification_action_results": report.verification_action_results,
        "evidence": report.evidence,
        "failure_summary": report.failure_summary,
        "outcome": report.outcome,
    });
    writeln!(file, "{}", serde_json::to_string(&record)?)?;
    Ok(())
}

fn persist_safety_event(project_root: &Path, event: &SafetyEvent) -> Result<()> {
    let path = project_root.join(".zero_nine/runtime/safety_events.ndjson");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(event)?)?;
    info!(
        "Safety event recorded: {:?} — {}",
        event.event_type, event.description
    );
    Ok(())
}

/// M7: Transition loop state with validation and persistence.
fn transition_state(
    project_root: &Path,
    state: &mut zn_types::LoopState,
    target: LoopStage,
    reason: &str,
) {
    if let Ok(transition) =
        state
            .stage
            .transition_to(target.clone(), reason, state.current_task.as_deref())
    {
        let transition_clone = transition.clone();
        state.transition_history.push(transition);
        state.stage = target;
        if let Err(e) = persist_state_transition(project_root, &transition_clone) {
            info!("State transition persist failed: {}", e);
        }
    } else {
        info!(
            "Skipped illegal state transition: {:?} -> {:?} (reason: {})",
            state.stage, target, reason
        );
    }
}

/// M7: Persist state transition to NDJSON log
fn persist_state_transition(project_root: &Path, transition: &StateTransition) -> Result<()> {
    let path = zn_spec::zero_nine_dir(project_root).join("loop/transitions.ndjson");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{}", serde_json::to_string(transition)?)?;
    Ok(())
}

/// M6: Write execution summary back to GitHub Issue if proposal originated from there.
fn sync_proposal_to_github(project_root: &Path, proposal: &Proposal) -> Result<()> {
    let (Some(issue_num), Some(repo)) =
        (proposal.source_issue_number, proposal.source_repo.as_ref())
    else {
        return Ok(()); // Not from GitHub, skip
    };

    let completed = proposal
        .tasks
        .iter()
        .filter(|t| matches!(t.status, TaskStatus::Completed))
        .count();
    let failed = proposal
        .tasks
        .iter()
        .filter(|t| matches!(t.status, TaskStatus::Failed))
        .count();
    let summary = format!(
        "Tasks: {}/{} completed, {} failed. Status: {:?}",
        completed,
        proposal.tasks.len(),
        failed,
        proposal.status
    );

    match write_execution_summary(Some(repo), issue_num, proposal, &summary) {
        Ok(result) => {
            info!(
                "GitHub sync: wrote execution summary to issue #{} ({})",
                issue_num, repo
            );
            append_event(
                project_root,
                RuntimeEvent::new(
                    "github.synced".to_string(),
                    Some(json!({
                        "issue_number": issue_num,
                        "repo": repo,
                        "success": result.success,
                        "proposal_id": proposal.id,
                    })),
                )
                .with_context(Some(proposal.id.clone()), None),
            )?;
        }
        Err(e) => {
            info!("GitHub sync failed for issue #{}: {}", issue_num, e);
            append_event(
                project_root,
                RuntimeEvent::new(
                    "github.sync_failed".to_string(),
                    Some(json!({
                        "issue_number": issue_num,
                        "repo": repo,
                        "error": e.to_string(),
                    })),
                )
                .with_context(Some(proposal.id.clone()), None),
            )?;
        }
    }

    Ok(())
}

/// M5: Execute compensation actions to clean up after failed multi-role chains.
fn execute_compensation_actions(project_root: &Path, proposal: &Proposal) -> Result<()> {
    // Collect compensation actions from failed tasks' tri_role_verdict
    let runtime_dir = project_root.join(".zero_nine/runtime/subagents");
    if !runtime_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&runtime_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
    {
        let recovery_file = entry.path();
        if !recovery_file.ends_with("-recovery.json") {
            continue;
        }
        let content = match fs::read_to_string(&recovery_file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let ledger: zn_types::SubagentRecoveryLedger = match serde_json::from_str(&content) {
            Ok(l) => l,
            Err(_) => continue,
        };

        // Check if any role failed
        let has_failure = ledger
            .records
            .iter()
            .any(|r| r.status == zn_types::SubagentRunStatus::Failed);
        if !has_failure {
            continue;
        }

        // Generate compensation actions from the plan
        let task_id = &ledger.task_id;
        let failed_roles: Vec<_> = ledger
            .records
            .iter()
            .filter(|r| r.status == zn_types::SubagentRunStatus::Failed)
            .map(|r| r.role.clone())
            .collect();

        let reason = format!(
            "Task {} failed in roles: {:?}. Compensating to clean up incomplete state.",
            task_id, failed_roles
        );

        // Look up worktree plan for this task
        let worktree_path = format!(".zero_nine/worktrees/task-{}", task_id);
        let branch_name = format!("zero-nine/task-{}", task_id);

        let actions = vec![
            CompensationAction {
                action_type: CompensationType::DeleteWorktree,
                target: worktree_path,
                reason: reason.clone(),
                executed: false,
            },
            CompensationAction {
                action_type: CompensationType::DeleteBranch,
                target: branch_name,
                reason,
                executed: false,
            },
        ];

        for mut action in actions {
            if let Err(e) = run_compensation_action(project_root, &mut action) {
                info!(
                    "Compensation action failed: {:?} — {}",
                    action.action_type, e
                );
            }
            if action.executed {
                append_event(
                    project_root,
                    RuntimeEvent::new(
                        "compensation.executed".to_string(),
                        Some(serde_json::json!({
                            "action_type": format!("{:?}", action.action_type),
                            "target": action.target,
                            "reason": action.reason,
                        })),
                    ),
                )?;
            }
        }
    }

    Ok(())
}

/// Execute a single compensation action.
fn run_compensation_action(project_root: &Path, action: &mut CompensationAction) -> Result<()> {
    match action.action_type {
        CompensationType::DeleteWorktree => {
            let wt_path = project_root.join(&action.target);
            if wt_path.exists() {
                let output = std::process::Command::new("git")
                    .args(["worktree", "remove", "-f", wt_path.to_str().unwrap_or("")])
                    .current_dir(project_root)
                    .output()?;
                if output.status.success() {
                    action.executed = true;
                }
            }
        }
        CompensationType::DeleteBranch => {
            let output = std::process::Command::new("git")
                .args(["branch", "-D", &action.target])
                .current_dir(project_root)
                .output()?;
            if output.status.success() {
                action.executed = true;
            }
        }
        CompensationType::CleanupArtifacts => {
            let target_path = project_root.join(&action.target);
            if target_path.exists() {
                fs::remove_dir_all(&target_path)?;
                action.executed = true;
            }
        }
        CompensationType::ResetWorkspace => {
            let output = std::process::Command::new("git")
                .args(["reset", "--hard", "HEAD"])
                .current_dir(project_root)
                .output()?;
            if output.status.success() {
                action.executed = true;
            }
        }
    }
    Ok(())
}

fn render_task_report(task_title: &str, plan: &ExecutionPlan, report: &ExecutionReport) -> String {
    let mut output = String::new();
    output.push_str(&format!("# Task Report: {}\n\n", task_title));
    output.push_str(&format!("Summary: {}\n\n", report.summary));
    output.push_str("## Execution Outcome\n\n");
    output.push_str(&format!(
        "- outcome: {:?}\n- success: {}\n- exit_code: {}\n- tests_passed: {}\n- review_passed: {}\n",
        report.outcome, report.success, report.exit_code, report.tests_passed, report.review_passed
    ));
    if let Some(failure_summary) = &report.failure_summary {
        output.push_str(&format!(
            "- blocking_or_failure_reason: {}\n",
            failure_summary
        ));
    }
    output.push_str("\n## Plan\n\n");
    for step in &plan.steps {
        output.push_str(&format!(
            "- {} — {} => {}\n",
            step.title, step.rationale, step.expected_output
        ));
    }
    output.push_str("\n## Details\n\n");
    for detail in &report.details {
        output.push_str(&format!("- {}\n", detail));
    }
    if !plan.verification_actions.is_empty() {
        output.push_str("\n## Verification Actions\n\n");
        for action in &plan.verification_actions {
            output.push_str(&format!(
                "- {} — command: `{}` — required: {} — evidence: {}\n",
                action.name, action.command, action.required, action.expected_evidence
            ));
        }
    }
    if !report.verification_action_results.is_empty() {
        output.push_str("\n## Verification Results\n\n");
        for result in &report.verification_action_results {
            output.push_str(&format!(
                "- {} — status: {} — {}{}\n",
                result.name,
                result.status,
                result.summary,
                result
                    .evidence_path
                    .as_ref()
                    .map(|path| format!(" (evidence: {})", path))
                    .unwrap_or_default()
            ));
        }
    }
    if let Some(verdict) = &report.review_verdict {
        output.push_str("\n## Review Verdict\n\n");
        output.push_str(&format!(
            "- approved: {}\n- status: {:?}\n- summary: {}\n",
            verdict.approved, verdict.status, verdict.summary
        ));
        if !verdict.risks.is_empty() {
            output.push_str("- risks:\n");
            for risk in &verdict.risks {
                output.push_str(&format!("  - {}\n", risk));
            }
        }
        if !verdict.evidence_keys.is_empty() {
            output.push_str("- evidence keys:\n");
            for key in &verdict.evidence_keys {
                output.push_str(&format!("  - {}\n", key));
            }
        }
    }
    if let Some(verdict) = &report.verification_verdict {
        output.push_str("\n## Verification Verdict\n\n");
        output.push_str(&format!(
            "- passed: {}\n- status: {:?}\n- summary: {}\n",
            verdict.passed, verdict.status, verdict.summary
        ));
        if !verdict.evidence.is_empty() {
            output.push_str("- deliverables:\n");
            for item in &verdict.evidence {
                output.push_str(&format!("  - {}\n", item));
            }
        }
        if !verdict.evidence_keys.is_empty() {
            output.push_str("- evidence keys:\n");
            for key in &verdict.evidence_keys {
                output.push_str(&format!("  - {}\n", key));
            }
        }
    }
    if !report.evidence.is_empty() {
        output.push_str("\n## Evidence Bundle\n\n");
        for evidence in &report.evidence {
            output.push_str(&format!(
                "- {} [{} / {:?}] required={} — {}{}\n",
                evidence.label,
                evidence.key,
                evidence.status,
                evidence.required,
                evidence.summary,
                evidence
                    .path
                    .as_ref()
                    .map(|path| format!(" (path: {})", path))
                    .unwrap_or_default()
            ));
        }
    }
    if !report.agent_runs.is_empty() {
        output.push_str("\n## Subagent Recovery Ledger\n\n");
        for run in &report.agent_runs {
            output.push_str(&format!(
                "- {} — status: {} — {}\n",
                run.role, run.status, run.summary
            ));
            if !run.outputs.is_empty() {
                output.push_str("  - outputs:\n");
                for item in &run.outputs {
                    output.push_str(&format!("    - {}\n", item));
                }
            }
            if !run.evidence_paths.is_empty() {
                output.push_str("  - evidence_paths:\n");
                for item in &run.evidence_paths {
                    output.push_str(&format!("    - {}\n", item));
                }
            }
            if let Some(recovery_path) = &run.recovery_path {
                output.push_str(&format!("  - recovery_record: {}\n", recovery_path));
            }
            if let Some(evidence_archive_path) = &run.evidence_archive_path {
                output.push_str(&format!(
                    "  - evidence_archive: {}\n",
                    evidence_archive_path
                ));
            }
            output.push_str(&format!(
                "  - replay_ready: {}\n",
                if run.replay_ready { "yes" } else { "no" }
            ));
            if let Some(replay_command) = &run.replay_command {
                output.push_str(&format!("  - replay_command: {}\n", replay_command));
            }
            if !run.state_transitions.is_empty() {
                output.push_str("  - state_transitions:\n");
                for item in &run.state_transitions {
                    output.push_str(&format!("    - {}\n", item));
                }
            }
            if let Some(failure_summary) = &run.failure_summary {
                output.push_str(&format!("  - failure_summary: {}\n", failure_summary));
            }
        }
    }
    if let Some(failure_summary) = &report.failure_summary {
        output.push_str("\n## Failure Summary\n\n");
        output.push_str(&format!("- {}\n", failure_summary));
    }
    if let Some(automation) = &report.finish_branch_automation {
        output.push_str("\n## Finish-Branch Automation\n\n");
        output.push_str(&format!(
            "- Default action: {:?}\n- Requires clean tree: {}\n",
            automation.default_action, automation.requires_clean_tree
        ));
        output.push_str("- Preview commands:\n");
        for command in &automation.preview_commands {
            output.push_str(&format!("  - `{}`\n", command));
        }
    }
    output.push_str("\n## Follow-ups\n\n");
    for item in &report.follow_ups {
        output.push_str(&format!("- {}\n", item));
    }
    output
}

fn render_proposal_verification(
    goal: &str,
    proposal_id: &str,
    tasks: &[zn_types::TaskItem],
) -> String {
    let completed = tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Completed))
        .count();
    format!(
        "# Verification Summary\n\nGoal: {}\nProposal: {}\nCompleted tasks: {}/{}\n\nAll tasks were executed through the enhanced Zero_Nine loop with planning, workspace preparation, review, verification, and finish-branch artifacts persisted under the proposal directory.\n",
        goal,
        proposal_id,
        completed,
        tasks.len()
    )
}
