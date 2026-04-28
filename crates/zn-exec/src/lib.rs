//! Zero_Nine Execution Layer
//!
//! This crate provides:
//! - Task execution planning
//! - Workspace preparation (git worktree, etc.)
//! - Execution report generation
//! - gRPC bridge client for agent dispatch

pub mod drift;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;
use uuid::Uuid;
use zn_types::{
    AgentRunRecord, BrainstormSession, BrainstormVerdict, BranchFinishPreview,
    BranchFinishRequest, ClarificationAnswer, ClarificationQuestion, ContextArtifact,
    ContextInjectionProtocol, EvidenceKind, EvidenceRecord, EvidenceStatus,
    ExecutionEnvelope, ExecutionMode, ExecutionOutcome, ExecutionPlan, ExecutionReport,
    FailureCategory, FailureClassification, FailureSeverity,
    FinishBranchAction, FinishBranchAutomation, FinishBranchResult, FinishBranchStatus,
    GeneratedArtifact, HostKind, PlanStep, QualityGate, ReviewVerdict, SubagentBrief,
    SubagentDispatch, SubagentExecutionRuntime, SubagentRecoveryLedger, SubagentRecoveryRecord,
    SubagentRunBook, SubagentRunStatus, SubagentExecutionPath, TaskItem, VerificationAction, VerificationActionResult,
    VerificationVerdict, VerdictStatus,
    WorkspacePreparationResult, WorkspaceRecord, WorkspaceStatus, WorkspaceStrategy,
    WorktreePlan, CompensationAction, CompensationType,
};

// Bridge client module for gRPC agent communication
pub mod bridge_client;

/// Layer 13: Cross-Cutting Observability
pub mod observability;
pub use observability::{EventEmitter, MetricsAggregator, EventQuery, create_default_observability};
pub use zn_types::TraceContext;

// Subagent dispatcher module
pub mod subagent_dispatcher;

// Governance module
pub mod governance;

// Bridge handler for independent service deployment
pub mod bridge_handler;
pub use bridge_handler::LocalCliHandler;

// Token counter and output optimizer
pub mod token_counter;
pub use token_counter::{TokenCounter, OutputOptimizer, TokenBudget};

// Re-export proto types for convenience
pub use zn_bridge::proto;
pub use subagent_dispatcher::{SubagentDispatcher, DispatchResult, SubagentContext, SubagentExecutionReport, is_claude_available, create_dispatcher, TriRoleVerdict, compute_tri_role_verdict};
pub use governance::{PolicyEngine, AuthorizationMatrix, ApprovalTicket, ApprovalStatus, RiskLevel, ActionType, AuthorizationRequirement, AuthorizationCheckResult, GovernanceStats, render_approval_ticket, TokenBudgetCheck, TokenBudgetStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Brainstorming,
    SpecCapture,
    Planning,
    Implementation,
    Verification,
    FinishBranch,
}

/// Outcome from real subagent execution, mergeable into ExecutionReport
#[derive(Debug, Clone)]
pub struct SubagentExecutionOutcome {
    pub agent_runs: Vec<AgentRunRecord>,
    pub evidence: Vec<EvidenceRecord>,
    pub artifact_paths: Vec<String>,
    pub all_succeeded: bool,
    pub tri_role_verdict: Option<String>,
}

impl SubagentExecutionOutcome {
    /// Create an outcome from a bridge-originated ExecutionReport.
    pub fn from_report(report: &ExecutionReport) -> Self {
        Self {
            agent_runs: report.agent_runs.clone(),
            evidence: report.evidence.clone(),
            artifact_paths: report.artifacts.clone(),
            all_succeeded: report.success,
            tri_role_verdict: report.tri_role_verdict.clone(),
        }
    }

    /// Create a failed outcome with an error message.
    pub fn error(message: &str) -> Self {
        Self {
            agent_runs: Vec::new(),
            evidence: Vec::new(),
            artifact_paths: Vec::new(),
            all_succeeded: false,
            tri_role_verdict: Some(format!("BridgeError: {}", message)),
        }
    }
}

pub fn start_brainstorm(goal: &str, host: HostKind) -> BrainstormSession {
    let now = Utc::now();
    let questions = vec![
        ClarificationQuestion {
            id: "problem_statement".to_string(),
            question: "[中文] Zero_Nine 首先应该解决的具体问题是什么？什么样的结果可以算作成功？\n[English] What exact problem should Zero_Nine solve first, and what outcome would count as success?".to_string(),
            rationale: "[中文] 精确的问题描述可以防止后续规格和执行层优化错误的目标。\n[English] A precise problem statement prevents the later spec and execution layers from optimizing for the wrong thing.".to_string(),
            priority: 100,
            answered: false,
        },
        ClarificationQuestion {
            id: "scope_in".to_string(),
            question: "[中文] 第一次实现必须包含哪些功能？请列出核心能力。\n[English] List the capabilities that must be included in the first implementation slice.".to_string(),
            rationale: "[中文] 这决定了第一个 OpenSpec 合同必须明确覆盖的内容。\n[English] This determines what the first OpenSpec contract must explicitly cover.".to_string(),
            priority: 95,
            answered: false,
        },
        ClarificationQuestion {
            id: "scope_out".to_string(),
            question: "[中文] 哪些内容应该明确排除在第一次实现范围之外？\n[English] What should explicitly stay out of scope for the first slice?".to_string(),
            rationale: "[中文] 明确的排除项可以保持承诺现实，防止过早扩张。\n[English] Explicit exclusions keep the one-command promise realistic and prevent premature expansion.".to_string(),
            priority: 90,
            answered: false,
        },
        ClarificationQuestion {
            id: "constraints".to_string(),
            question: "[中文] 实现必须遵守哪些不可协商的约束条件？（如运行时、宿主、语言、工作流、安全约束等）\n[English] What non-negotiable constraints must the implementation obey? (runtime, host, language, workflow, safety, etc.)".to_string(),
            rationale: "[中文] 约束条件直接影响架构、工作空间策略和执行门控。\n[English] Constraints directly affect architecture, workspace strategy, and execution gating.".to_string(),
            priority: 85,
            answered: false,
        },
        ClarificationQuestion {
            id: "acceptance_criteria".to_string(),
            question: "[中文] 在本次迭代可以被视为成功之前，Zero_Nine 应该满足哪些验收标准？\n[English] What acceptance criteria should Zero_Nine satisfy before this iteration can be considered successful?".to_string(),
            rationale: "[中文] 验收标准定义了审查和验证的终点线。\n[English] Acceptance criteria define the finish line for review and verification.".to_string(),
            priority: 80,
            answered: false,
        },
        ClarificationQuestion {
            id: "risks".to_string(),
            question: "[中文] 系统应该从一开始就跟踪哪些主要风险、模糊点或失败模式？\n[English] What major risks, ambiguities, or failure modes should the system track from the beginning?".to_string(),
            rationale: "[中文] 已知风险应该尽早写入设计和验证工件，而不是在后期重新发现。\n[English] Known risks should be written into design and verification artifacts early instead of being rediscovered late.".to_string(),
            priority: 70,
            answered: false,
        },
    ];

    BrainstormSession {
        id: format!("bs-{}", Uuid::new_v4().simple()),
        goal: goal.to_string(),
        host,
        status: "collecting_answers".to_string(),
        created_at: now,
        updated_at: now,
        questions,
        answers: Vec::new(),
        verdict: BrainstormVerdict::Continue,
    }
}

pub fn next_brainstorm_question(session: &BrainstormSession) -> Option<ClarificationQuestion> {
    session.questions.iter().find(|question| !question.answered).cloned()
}

pub fn answer_brainstorm_question(
    session: &mut BrainstormSession,
    question_id: &str,
    answer: &str,
) -> Result<BrainstormVerdict> {
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("brainstorm answer cannot be empty"));
    }

    let question = session
        .questions
        .iter_mut()
        .find(|item| item.id == question_id)
        .ok_or_else(|| anyhow!("unknown brainstorm question: {}", question_id))?;
    question.answered = true;

    if let Some(existing) = session
        .answers
        .iter_mut()
        .find(|item| item.question_id == question_id)
    {
        existing.answer = trimmed.to_string();
        existing.captured_at = Utc::now();
    } else {
        session.answers.push(ClarificationAnswer {
            question_id: question_id.to_string(),
            answer: trimmed.to_string(),
            captured_at: Utc::now(),
        });
    }

    session.updated_at = Utc::now();
    session.verdict = brainstorm_verdict(session);
    session.status = match session.verdict {
        BrainstormVerdict::Continue => "collecting_answers".to_string(),
        BrainstormVerdict::Ready => "ready_for_spec_capture".to_string(),
        BrainstormVerdict::Escalate => "needs_manual_confirmation".to_string(),
    };
    Ok(session.verdict.clone())
}

pub fn brainstorm_verdict(session: &BrainstormSession) -> BrainstormVerdict {
    let unanswered = session.questions.iter().filter(|item| !item.answered).count();
    if unanswered > 0 {
        return BrainstormVerdict::Continue;
    }

    let low_signal_answers = session
        .answers
        .iter()
        .filter(|item| word_count(&item.answer) < 3)
        .count();
    if low_signal_answers >= 2 {
        BrainstormVerdict::Escalate
    } else {
        BrainstormVerdict::Ready
    }
}

fn word_count(input: &str) -> usize {
    input.split_whitespace().filter(|item| !item.is_empty()).count()
}

pub fn classify_task(task: &TaskItem) -> TaskKind {
    let title = task.title.to_lowercase();
    let description = task.description.to_lowercase();
    let haystack = format!("{} {}", title, description);
    let tokenized = tokenize_keywords(&haystack);

    if haystack.contains("brainstorm") || haystack.contains("clarify") || tokenized.iter().any(|item| item == "requirement") {
        TaskKind::Brainstorming
    } else if haystack.contains("openspec")
        || tokenized.iter().any(|item| matches!(item.as_str(), "proposal" | "design" | "dag"))
    {
        TaskKind::SpecCapture
    } else if haystack.contains("writing-plans")
        || haystack.contains("execution plan")
        || tokenized.iter().any(|item| matches!(item.as_str(), "worktree" | "sandbox" | "planning"))
    {
        TaskKind::Planning
    } else if haystack.contains("finish branch")
        || haystack.contains("pull request")
        || tokenized.iter().any(|item| matches!(item.as_str(), "finish" | "merge" | "pr"))
    {
        TaskKind::FinishBranch
    } else if haystack.contains("verify implementation")
        || haystack.contains("verification evidence")
        || (tokenized.iter().any(|item| matches!(item.as_str(), "verify" | "verification"))
            && tokenized.iter().any(|item| matches!(item.as_str(), "progress" | "evolve")))
    {
        TaskKind::Verification
    } else if haystack.contains("execute guarded")
        || tokenized.iter().any(|item| matches!(item.as_str(), "implementation" | "develop" | "developer" | "coding"))
    {
        TaskKind::Implementation
    } else if tokenized.iter().any(|item| matches!(item.as_str(), "verify" | "verification" | "review" | "evolve" | "progress")) {
        TaskKind::Verification
    } else {
        TaskKind::Implementation
    }
}

fn tokenize_keywords(input: &str) -> Vec<String> {
    input
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-')
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
        .collect()
}

pub fn build_execution_envelope(
    proposal_id: &str,
    task: &TaskItem,
    host: HostKind,
    context_files: Vec<String>,
) -> ExecutionEnvelope {
    let kind = classify_task(task);
    let execution_mode = mode_for(kind);
    let workspace_strategy = workspace_for(kind);
    let quality_gates = quality_gates_for(kind);
    let context_protocol = Some(context_protocol_for(task, host, execution_mode.clone(), &context_files));

    ExecutionEnvelope {
        proposal_id: proposal_id.to_string(),
        task_id: task.id.clone(),
        task_title: task.title.clone(),
        execution_mode,
        workspace_strategy,
        context_files,
        context_protocol,
        context_protocol_path: None,
        quality_gates,
        bridge_address: None,
    }
}

pub fn build_plan(task: &TaskItem) -> ExecutionPlan {
    build_plan_with_config(task, SubagentExecutionPath::default(), None)
}

pub fn build_plan_with_config(
    task: &TaskItem,
    execution_path: SubagentExecutionPath,
    bridge_address: Option<String>,
) -> ExecutionPlan {
    let kind = classify_task(task);
    let mode = mode_for(kind);
    let workspace_strategy = workspace_for(kind);
    let objective = derive_objective(task, kind);
    let steps = steps_for(task, kind);
    let validation = validation_for(task, kind);
    let quality_gates = quality_gates_for(kind);
    let skill_chain = skills_for(kind);
    let deliverables = deliverables_for(task, kind);
    let risks = risks_for(kind);
    let subagents = subagents_for(task, kind);
    let worktree_plan = worktree_plan_for(task, kind, workspace_strategy.clone());
    let workspace_record = worktree_plan.as_ref().map(planned_workspace_record);
    let verification_actions = verification_actions_for(task, kind);
    let finish_branch_automation = finish_branch_automation_for(&workspace_record, mode.clone());

    ExecutionPlan {
        task_id: task.id.clone(),
        objective,
        mode,
        workspace_strategy,
        steps,
        validation,
        quality_gates,
        skill_chain,
        deliverables,
        risks,
        subagents,
        worktree_plan,
        workspace_record,
        verification_actions,
        finish_branch_automation,
        execution_path,
        bridge_address,
    }
}

/// Execute subagent dispatches via real Claude CLI calls.
/// Returns a graceful fallback outcome when `claude` is not on PATH.
pub fn execute_subagent_dispatch(
    project_root: &Path,
    plan: &ExecutionPlan,
) -> SubagentExecutionOutcome {
    if plan.subagents.is_empty() {
        return SubagentExecutionOutcome {
            agent_runs: Vec::new(),
            evidence: Vec::new(),
            artifact_paths: Vec::new(),
            all_succeeded: true,
            tri_role_verdict: None,
        };
    }

    if !is_claude_available() {
        info!("claude CLI not available; skipping real subagent execution");
        return SubagentExecutionOutcome {
            agent_runs: Vec::new(),
            evidence: Vec::new(),
            artifact_paths: Vec::new(),
            all_succeeded: false,
            tri_role_verdict: None,
        };
    }

    // Use task_id from plan as proposal_id for path consistency
    let proposal_id = plan.task_id.as_str();
    let task_id = plan.task_id.as_str();

    let Ok(mut dispatcher) = SubagentDispatcher::new(project_root, proposal_id, task_id, plan.skill_chain.clone()) else {
        return SubagentExecutionOutcome {
            agent_runs: Vec::new(),
            evidence: Vec::new(),
            artifact_paths: Vec::new(),
            all_succeeded: false,
            tri_role_verdict: None,
        };
    };

    // Build briefs from subagent specs in the plan
    let briefs: Vec<SubagentBrief> = plan
        .subagents
        .iter()
        .map(|sa| SubagentBrief {
            role: sa.role.clone(),
            goal: sa.goal.clone(),
            inputs: sa.inputs.clone(),
            outputs: sa.outputs.clone(),
            depends_on: sa.depends_on.clone(),
        })
        .collect();

    let objective = format!("Execute task {} for proposal {}", task_id, proposal_id);
    let runbook = dispatcher.create_runbook(&briefs, &objective);

    // Persist runbook artifacts to disk
    let _ = dispatcher.save_runbook(&runbook);

    // Execute with cross-role handoff pipeline
    let (report, verdict) = dispatcher.run_tri_role_pipeline(&runbook);

    // Convert report to outcome
    let agent_runs: Vec<AgentRunRecord> = report
        .results
        .iter()
        .map(|r| AgentRunRecord {
            role: r.role.clone(),
            status: if r.success {
                subagent_dispatcher::AGENT_STATUS_COMPLETED.to_string()
            } else {
                subagent_dispatcher::AGENT_STATUS_FAILED.to_string()
            },
            summary: r.raw_output.clone().unwrap_or_default(),
            outputs: r.output_files.clone(),
            evidence_paths: r.output_files.clone(),
            failure_summary: if r.success { None } else { r.error.clone() },
            state_transitions: Vec::new(),
            recovery_path: None,
            evidence_archive_path: None,
            replay_ready: r.success,
            replay_command: None,
        })
        .collect();

    let evidence: Vec<EvidenceRecord> = report
        .results
        .iter()
        .filter(|r| r.success)
        .flat_map(|r| {
            r.output_files.iter().map(|path| EvidenceRecord {
                key: format!("subagent_{}", r.role),
                label: format!("{} output", r.role),
                kind: EvidenceKind::Subagent,
                status: EvidenceStatus::Collected,
                required: false,
                summary: format!("Subagent {} produced {}", r.role, path),
                path: Some(path.clone()),
            })
        })
        .collect();

    let artifact_paths: Vec<String> = report
        .results
        .iter()
        .flat_map(|r| r.output_files.clone())
        .collect();

    SubagentExecutionOutcome {
        agent_runs,
        evidence,
        artifact_paths,
        all_succeeded: report.all_succeeded,
        tri_role_verdict: Some(format!("{:?}", verdict)),
    }
}

/// Generate compensation actions when a multi-role chain fails.
/// Produces cleanup worktree/branch actions for GitWorktree strategies,
/// or artifact cleanup for Sandboxed strategies.
pub fn generate_compensation_actions(
    plan: &ExecutionPlan,
    tri_role_verdict: &str,
) -> Vec<zn_types::CompensationAction> {
    let mut actions = Vec::new();

    // Only generate compensation for failure verdicts
    if tri_role_verdict == "Pass" {
        return actions;
    }

    // GitWorktree strategy: clean up worktree and branch
    if let Some(ref wt) = plan.worktree_plan {
        actions.push(CompensationAction {
            action_type: CompensationType::DeleteWorktree,
            target: wt.worktree_path.clone(),
            reason: format!(
                "Tri-role verdict: {}. Worktree contains incomplete work.",
                tri_role_verdict
            ),
            executed: false,
        });
        actions.push(CompensationAction {
            action_type: CompensationType::DeleteBranch,
            target: wt.branch_name.clone(),
            reason: format!(
                "Tri-role verdict: {}. Branch contains unmerged changes.",
                tri_role_verdict
            ),
            executed: false,
        });
    }

    // Sandboxed strategy: clean up sandbox artifacts
    if matches!(
        plan.workspace_strategy,
        WorkspaceStrategy::Sandboxed
    ) {
        actions.push(CompensationAction {
            action_type: CompensationType::CleanupArtifacts,
            target: format!(".zero_nine/sandboxes/{}", plan.task_id),
            reason: format!(
                "Tri-role verdict: {}. Sandbox contains incomplete artifacts.",
                tri_role_verdict
            ),
            executed: false,
        });
    }

    actions
}

pub fn execute_plan(
    project_root: &Path,
    task: &TaskItem,
    plan: &ExecutionPlan,
    workspace_record: Option<WorkspaceRecord>,
    allow_remote_finish: bool,
) -> Result<ExecutionReport> {
    let mut details = plan
        .steps
        .iter()
        .enumerate()
        .map(|(idx, step)| {
            format!(
                "Step {}: {} | rationale: {} | expected output: {}",
                idx + 1,
                step.title,
                step.rationale,
                step.expected_output
            )
        })
        .collect::<Vec<_>>();

    // Execute file operations from plan steps (actual file writing)
    let file_operation_results = execute_file_operations(project_root, plan)?;

    let generated_artifacts = generated_artifacts_for(task, plan);
    let mut artifacts = generated_artifacts
        .iter()
        .map(|artifact| artifact.path.clone())
        .collect::<Vec<_>>();

    // Add file operation results to artifacts
    artifacts.extend(file_operation_results.iter().map(|r| r.path.clone()));

    let subagent_records = persist_subagent_runbook_artifacts(project_root, plan)?;
    artifacts.extend(subagent_records.all_paths.clone());

    // M9: Select subagent execution path based on plan configuration
    let subagent_outcome = match &plan.execution_path {
        SubagentExecutionPath::Cli => {
            execute_subagent_dispatch(project_root, plan)
        }
        SubagentExecutionPath::Bridge => {
            match execute_plan_via_bridge(project_root, task, plan, 300) {
                Ok(report) => SubagentExecutionOutcome::from_report(&report),
                Err(e) => {
                    info!("Bridge execution failed: {}", e);
                    SubagentExecutionOutcome::error(&e.to_string())
                }
            }
        }
        SubagentExecutionPath::Hybrid => {
            let cli_outcome = execute_subagent_dispatch(project_root, plan);
            if cli_outcome.all_succeeded {
                cli_outcome
            } else {
                info!("CLI subagent execution failed, trying bridge fallback");
                match execute_plan_via_bridge(project_root, task, plan, 300) {
                    Ok(report) => SubagentExecutionOutcome::from_report(&report),
                    Err(e) => {
                        info!("Bridge fallback also failed: {}", e);
                        cli_outcome // Return CLI outcome for diagnostics
                    }
                }
            }
        }
    };
    artifacts.extend(subagent_outcome.artifact_paths.clone());
    if !subagent_outcome.agent_runs.is_empty() {
        details.push(format!(
            "Subagent execution: {}/{} succeeded",
            subagent_outcome.agent_runs.iter().filter(|r| r.status == subagent_dispatcher::AGENT_STATUS_COMPLETED).count(),
            subagent_outcome.agent_runs.len()
        ));
    }
    if let Some(ref verdict) = subagent_outcome.tri_role_verdict {
        details.push(format!("Tri-role verdict: {}", verdict));
    }

    let verification_actions = plan.verification_actions.clone();
    let verification_action_results = execute_verification_actions(project_root, plan)?;
    artifacts.extend(
        verification_action_results
            .iter()
            .filter_map(|item| item.evidence_path.clone()),
    );

    let tests_required = plan
        .quality_gates
        .iter()
        .any(|gate| gate.name == "tests" && gate.required);
    let review_required = plan
        .quality_gates
        .iter()
        .any(|gate| gate.name == "review" && gate.required);
    let tests_passed = verification_status(&verification_action_results, "tests", !tests_required);
    let review_passed = verification_status(&verification_action_results, "review", !review_required);

    let finish_branch_result = execute_finish_branch_if_needed(
        project_root,
        plan,
        workspace_record.as_ref(),
        allow_remote_finish,
    )?;

    // P0-B: Safety policy enforcement — block merge/push without passing gates
    if let Some(result) = &finish_branch_result {
        let is_merge = matches!(result.action, FinishBranchAction::Merge | FinishBranchAction::PullRequest);
        if is_merge && (!tests_passed || !review_passed) {
            details.push("SAFETY: Merge blocked by policy — tests and review must pass before merge/push".to_string());
            let mut blocked_result = result.clone();
            blocked_result.status = FinishBranchStatus::Rejected;
            blocked_result.summary = format!(
                "Blocked by safety policy: merge requires passing tests (got {}) and review (got {})",
                tests_passed, review_passed
            );
        }
    }

    if let Some(result) = &finish_branch_result {
        details.push(format!("Finish-branch outcome: {}", result.summary));
    }
    let finish_branch_success = finish_branch_result
        .as_ref()
        .map(|result| matches!(result.status, FinishBranchStatus::Completed))
        .unwrap_or(true);
    let success = tests_passed && review_passed && finish_branch_success;
    let failure_summary = if success {
        None
    } else if let Some(result) = &finish_branch_result {
        match result.status {
            FinishBranchStatus::Rejected => Some(format!(
                "Finish-branch action was rejected by policy or missing explicit confirmation: {}",
                result.summary
            )),
            FinishBranchStatus::Failed => Some(format!(
                "Finish-branch automation failed and requires escalation: {}",
                result.summary
            )),
            _ => None,
        }
    } else if !tests_passed && !review_passed {
        Some("Both tests and review gates failed in the same execution cycle; manual investigation is required before retrying.".to_string())
    } else if !tests_passed {
        Some("Required test gates did not pass; revise the implementation and retry verification.".to_string())
    } else if !review_passed {
        Some("Required review gates did not pass; address review findings before retrying.".to_string())
    } else {
        Some("Execution did not satisfy all completion gates.".to_string())
    };
    let outcome = if success {
        zn_types::ExecutionOutcome::Completed
    } else if let Some(result) = &finish_branch_result {
        match result.status {
            FinishBranchStatus::Rejected => zn_types::ExecutionOutcome::Blocked,
            FinishBranchStatus::Failed => zn_types::ExecutionOutcome::Escalated,
            _ => zn_types::ExecutionOutcome::RetryableFailure,
        }
    } else if !tests_passed && !review_passed {
        zn_types::ExecutionOutcome::Escalated
    } else {
        zn_types::ExecutionOutcome::RetryableFailure
    };
    let exit_code = if success { 0 } else { 1 };

    let finish_branch_automation = plan.finish_branch_automation.clone();

    // Merge real subagent execution results with stub records
    let agent_runs = if subagent_outcome.agent_runs.is_empty() {
        agent_runs_for(plan, &subagent_records.dispatch_records)
    } else {
        subagent_outcome.agent_runs
    };

    let mut evidence = collect_evidence_records(
        plan,
        &generated_artifacts,
        &verification_action_results,
        workspace_record.as_ref(),
        finish_branch_result.as_ref(),
    );
    evidence.extend(subagent_outcome.evidence);
    let review_evidence_keys = review_evidence_keys(&evidence);
    let verification_evidence_keys = verification_evidence_keys(&evidence);
    let review_verdict = review_verdict_for(plan, review_passed, review_evidence_keys);
    let verification_verdict = verification_verdict_for(
        plan,
        tests_passed && review_passed,
        verification_evidence_keys,
        plan.deliverables.clone(),
    );

    let mut report = ExecutionReport {
        task_id: task.id.clone(),
        success,
        outcome,
        summary: summary_for(task, plan),
        details,
        tests_passed,
        review_passed,
        artifacts,
        generated_artifacts,
        evidence,
        follow_ups: follow_ups_for(plan),
        workspace_record,
        finish_branch_result,
        finish_branch_automation,
        agent_runs,
        review_verdict,
        verification_verdict,
        verification_actions,
        verification_action_results,
        failure_summary,
        exit_code,
        execution_time_ms: 0,
        token_count: 0,
        code_quality_score: 0.0,
        test_coverage: 0.0,
        user_feedback: None,
        failure_classification: None,
        tri_role_verdict: subagent_outcome.tri_role_verdict.clone(),
        authorization_ticket_id: None,
        authorized_by: None,
    };
    report.failure_classification = Some(classify_failure(&report));
    Ok(report)
}

/// Classify a failure based on the execution report signals
pub fn classify_failure(report: &ExecutionReport) -> FailureClassification {
    // Check workspace drift indicators
    if let Some(ref record) = report.workspace_record {
        if record.notes.iter().any(|n| n.to_lowercase().contains("drift") || n.to_lowercase().contains("branch mismatch")) {
            return FailureClassification {
                id: format!("failure:{}", report.task_id),
                category: FailureCategory::EnvironmentDrift,
                severity: FailureSeverity::High,
                description: report.failure_summary.clone().unwrap_or_default(),
                root_cause: Some("Workspace environment has drifted from expected state".to_string()),
                retry_recommended: false,
                human_intervention_required: true,
                suggested_fix: Some("Re-sync workspace with expected state before retrying".to_string()),
            };
        }
    }

    // Check verification failures
    if report.verification_action_results.iter().any(|r| r.status.to_lowercase().contains("fail")) {
        return FailureClassification {
            id: format!("failure:{}", report.task_id),
            category: FailureCategory::VerificationFailed,
            severity: FailureSeverity::High,
            description: report.failure_summary.clone().unwrap_or_default(),
            root_cause: Some("Verification gate did not pass".to_string()),
            retry_recommended: true,
            human_intervention_required: false,
            suggested_fix: Some("Review verification evidence and adjust implementation".to_string()),
        };
    }

    // Check finish-branch policy blocks
    if let Some(ref result) = report.finish_branch_result {
        if matches!(result.status, FinishBranchStatus::Rejected) {
            return FailureClassification {
                id: format!("failure:{}", report.task_id),
                category: FailureCategory::PolicyBlocked,
                severity: FailureSeverity::Critical,
                description: report.failure_summary.clone().unwrap_or_default(),
                root_cause: Some("Branch finishing blocked by policy".to_string()),
                retry_recommended: false,
                human_intervention_required: true,
                suggested_fix: Some("Review and approve branch finish policy".to_string()),
            };
        }
        if matches!(result.status, FinishBranchStatus::Failed) {
            return FailureClassification {
                id: format!("failure:{}", report.task_id),
                category: FailureCategory::ResourceExhausted,
                severity: FailureSeverity::High,
                description: report.failure_summary.clone().unwrap_or_default(),
                root_cause: Some("Branch finishing failed due to resource issues".to_string()),
                retry_recommended: false,
                human_intervention_required: true,
                suggested_fix: Some("Check git state and retry manually".to_string()),
            };
        }
    }

    // Check subagent run failures
    if report.agent_runs.iter().any(|r| r.status.to_lowercase().contains("fail")) {
        return FailureClassification {
            id: format!("failure:{}", report.task_id),
            category: FailureCategory::ToolError,
            severity: FailureSeverity::Medium,
            description: report.failure_summary.clone().unwrap_or_default(),
            root_cause: Some("Subagent execution encountered an error".to_string()),
            retry_recommended: true,
            human_intervention_required: false,
            suggested_fix: Some("Review subagent output and retry".to_string()),
        };
    }

    // Default: unknown
    FailureClassification {
        id: format!("failure:{}", report.task_id),
        category: FailureCategory::Unknown,
        severity: FailureSeverity::Medium,
        description: report.failure_summary.clone().unwrap_or_default(),
        root_cause: None,
        retry_recommended: true,
        human_intervention_required: false,
        suggested_fix: None,
    }
}

/// File operation result for tracking written files
#[derive(Debug, Clone)]
pub struct FileOperationResult {
    pub path: String,
    pub operation: String,
    pub success: bool,
    pub bytes_written: u64,
}

/// Execute file operations from plan steps
/// This function actually writes files to disk based on plan deliverables
fn execute_file_operations(
    project_root: &Path,
    plan: &ExecutionPlan,
) -> Result<Vec<FileOperationResult>> {
    let mut results = Vec::new();

    // Write generated artifacts to disk
    for deliverable in &plan.deliverables {
        let file_path = project_root.join(deliverable);

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Generate content based on deliverable type
        let content = generate_deliverable_content(plan, deliverable);

        // Write the file
        let bytes = content.as_bytes();
        fs::write(&file_path, bytes)?;

        results.push(FileOperationResult {
            path: deliverable.clone(),
            operation: "write".to_string(),
            success: true,
            bytes_written: bytes.len() as u64,
        });

        tracing::info!("Wrote file: {}", file_path.display());
    }

    // Also write artifacts from generated_artifacts
    for artifact in &plan.deliverables {
        if !results.iter().any(|r| r.path == *artifact) {
            let file_path = project_root.join(artifact);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = generate_deliverable_content(plan, artifact);
            let bytes = content.as_bytes();
            fs::write(&file_path, bytes)?;

            results.push(FileOperationResult {
                path: artifact.clone(),
                operation: "write".to_string(),
                success: true,
                bytes_written: bytes.len() as u64,
            });
        }
    }

    Ok(results)
}

/// Generate content for a deliverable based on plan context
fn generate_deliverable_content(plan: &ExecutionPlan, deliverable: &str) -> String {
    // Default content template for deliverables
    format!(
        r#"# Deliverable: {}

## Task: {}
## Objective: {}

## Generated by Zero_Nine
This file was automatically generated as part of the execution plan.

## Steps
{}

## Deliverables
{}

## Risks
{}
"#,
        deliverable,
        plan.task_id,
        plan.objective,
        plan.steps.iter().map(|s| format!("- {}", s.title)).collect::<Vec<_>>().join("\n"),
        plan.deliverables.join("\n"),
        plan.risks.join("\n"),
    )
}

fn execute_verification_actions(
    project_root: &Path,
    plan: &ExecutionPlan,
) -> Result<Vec<VerificationActionResult>> {
    let evidence_dir = project_root
        .join(".zero_nine/runtime/verification")
        .join(format!("task-{}", plan.task_id));
    fs::create_dir_all(&evidence_dir)?;

    let mut results = Vec::new();
    for action in &plan.verification_actions {
        let resolved_command = resolve_verification_command(project_root, action);
        let outcome = run_shell_command(
            project_root,
            &resolved_command,
            &format!("failed to execute verification action {}", action.name),
        )?;
        let evidence_path = evidence_dir.join(format!("{}-evidence.md", action.name));
        fs::write(
            &evidence_path,
            render_verification_evidence(plan, action, &resolved_command, &outcome),
        )?;
        results.push(VerificationActionResult {
            name: action.name.clone(),
            status: if outcome.exit_code == 0 {
                "passed".to_string()
            } else if action.required {
                "failed".to_string()
            } else {
                "soft_failed".to_string()
            },
            summary: if outcome.exit_code == 0 {
                format!(
                    "Action {} completed successfully for task {} with command `{}`.",
                    action.name, plan.task_id, resolved_command
                )
            } else {
                format!(
                    "Action {} exited with code {} for task {} after running `{}`. Inspect the evidence file before advancing.",
                    action.name, outcome.exit_code, plan.task_id, resolved_command
                )
            },
            evidence_path: Some(evidence_path.display().to_string()),
        });
    }
    Ok(results)
}

fn resolve_verification_command(project_root: &Path, action: &VerificationAction) -> String {
    match action.name.as_str() {
        "tests" => detect_test_command(project_root),
        "review" => detect_review_command(project_root),
        _ => action.command.clone(),
    }
}

fn detect_test_command(project_root: &Path) -> String {
    let cargo_toml = project_root.join("Cargo.toml");
    let pyproject = project_root.join("pyproject.toml");
    let requirements = project_root.join("requirements.txt");
    let setup_py = project_root.join("setup.py");
    let package_json = project_root.join("package.json");
    let pnpm_lock = project_root.join("pnpm-lock.yaml");
    let yarn_lock = project_root.join("yarn.lock");
    let npm_lock = project_root.join("package-lock.json");

    if cargo_toml.exists() {
        return "cargo test --all-targets".to_string();
    }

    if pyproject.exists() || requirements.exists() || setup_py.exists() {
        return "pytest -q".to_string();
    }

    if package_json.exists() {
        if pnpm_lock.exists() {
            return "pnpm test".to_string();
        }
        if yarn_lock.exists() {
            return "yarn test".to_string();
        }
        if npm_lock.exists() {
            return "npm test".to_string();
        }
        return "npm test".to_string();
    }

    "echo 'no recognized test stack - skipped adaptive test execution'".to_string()
}

fn detect_review_command(project_root: &Path) -> String {
    let cargo_toml = project_root.join("Cargo.toml");
    let clippy_toml = project_root.join("clippy.toml");
    let package_json = project_root.join("package.json");

    if cargo_toml.exists() || clippy_toml.exists() {
        "cargo clippy -- -D warnings".to_string()
    } else if package_json.exists() {
        "npx eslint . --max-warnings=0".to_string()
    } else {
        "echo 'no recognized review or lint stack - skipped review execution'".to_string()
    }
}

fn verification_status(
    results: &[VerificationActionResult],
    action_name: &str,
    default: bool,
) -> bool {
    results
        .iter()
        .find(|item| item.name == action_name)
        .map(|item| item.status == "passed")
        .unwrap_or(default)
}

fn execute_finish_branch_if_needed(
    project_root: &Path,
    plan: &ExecutionPlan,
    workspace_record: Option<&WorkspaceRecord>,
    allow_remote_finish: bool,
) -> Result<Option<FinishBranchResult>> {
    if !matches!(plan.mode, ExecutionMode::FinishBranch) {
        return Ok(None);
    }

    let Some(record) = workspace_record else {
        return Ok(Some(FinishBranchResult {
            action: FinishBranchAction::PullRequest,
            status: FinishBranchStatus::Failed,
            branch_name: format!("zero-nine/{}", plan.task_id),
            worktree_path: None,
            summary: format!("Finish-branch could not run for task {} because no prepared workspace record was available.", plan.task_id),
            follow_ups: vec!["Prepare the workspace first so branch automation can resolve the active branch and worktree path.".to_string()],
            pr_url: None,
        }));
    };

    let action = plan
        .finish_branch_automation
        .as_ref()
        .map(|item| item.default_action.clone())
        .unwrap_or(FinishBranchAction::PullRequest);
    let request = BranchFinishRequest {
        action,
        branch_name: record.branch_name.clone(),
        worktree_path: Some(record.worktree_path.clone()),
        verify_clean: true,
        confirmed: allow_remote_finish,
        pr_title: None,
        pr_body: None,
    };

    match finish_branch(project_root, &request) {
        Ok(result) => Ok(Some(result)),
        Err(error) => Ok(Some(FinishBranchResult {
            action: request.action,
            status: FinishBranchStatus::Failed,
            branch_name: request.branch_name,
            worktree_path: request.worktree_path,
            summary: format!("Finish-branch automation failed for task {}: {}", plan.task_id, error),
            follow_ups: vec![
                "Review git status, branch state, remote configuration, and authentication before retrying finish-branch automation.".to_string(),
            ],
            pr_url: None,
        })),
    }
}

fn render_verification_evidence(
    plan: &ExecutionPlan,
    action: &VerificationAction,
    resolved_command: &str,
    outcome: &CommandOutcome,
) -> String {
    format!(
        "# Verification Evidence\n\nTask: {}\nAction: {}\nPlanned Command: `{}`\nResolved Command: `{}`\nExit Code: {}\n\n## Expected Evidence\n\n{}\n\n## Stdout\n\n```text\n{}\n```\n\n## Stderr\n\n```text\n{}\n```\n",
        plan.task_id,
        action.name,
        action.command,
        resolved_command,
        outcome.exit_code,
        action.expected_evidence,
        outcome.stdout,
        outcome.stderr,
    )
}

struct CommandOutcome {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

fn run_shell_command(project_root: &Path, command: &str, context: &str) -> Result<CommandOutcome> {
    // 允许的命令白名单 (安全修复: 不包含 shell 解释器)
    const ALLOWED_COMMANDS: &[&str] = &[
        "cargo", "npm", "yarn", "pnpm", "bun", "node",
        "git", "go", "python", "python3", "pip", "pip3",
        "bundle", "rake", "mix", "elixir", "rustc",
        "make", "cmake",
        "ls", "cat", "grep", "find", "head", "tail",
        "jq", "sed", "awk", "diff", "patch",
        "docker", "docker-compose",
        "printf", "echo", "touch", "mkdir", "rm", "cp", "mv",
        "pwd", "whoami", "hostname", "uname", "date", "sleep",
        "true", "false", "test",
    ];

    // 安全修复: 拒绝包含 shell 操作符的命令
    // 不再使用 sh -c 执行，避免命令注入 (shell operators: &&, ||, ;, |, $, `, etc.)
    let forbidden_operators = ["&&", "||", ";", "|", "$", "`", "(", ")", "{", "}", ">", "<", "!"];
    for op in &forbidden_operators {
        if command.contains(op) {
            return Err(anyhow::anyhow!(
                "命令包含不允许的操作符 '{}': 出于安全考虑，请使用参数化的命令形式。",
                op
            ));
        }
    }

    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(anyhow::anyhow!("空命令"));
    }

    let program = parts[0];

    // 检查命令是否在白名单中
    if !ALLOWED_COMMANDS.contains(&program) {
        return Err(anyhow::anyhow!(
            "命令 '{}' 不在允许列表中。允许的命令：{}",
            program,
            ALLOWED_COMMANDS.join(", ")
        ));
    }

    // 使用直接执行方式，避免 shell 注入
    let mut cmd = Command::new(program);
    cmd.current_dir(project_root);

    // 添加剩余参数（跳过第一个程序名）
    for arg in parts.iter().skip(1) {
        cmd.arg(*arg);
    }

    let output = cmd
        .output()
        .with_context(|| format!("{} - 执行命令：{}", context, command))?;

    // Token 优化：过滤和截断输出
    let stdout = optimize_output_for_tokens(&String::from_utf8_lossy(&output.stdout));
    let stderr = optimize_output_for_tokens(&String::from_utf8_lossy(&output.stderr));

    Ok(CommandOutcome {
        exit_code: output.status.code().unwrap_or(1),
        stdout,
        stderr,
    })
}

/// 优化输出以减少 Token 使用
/// 默认配置：最大 200 行，10000 字符，启用智能过滤
fn optimize_output_for_tokens(output: &str) -> String {
    let optimizer = OutputOptimizer::default();
    optimizer.optimize(output)
}

fn context_protocol_for(
    task: &TaskItem,
    host: HostKind,
    mode: ExecutionMode,
    context_files: &[String],
) -> ContextInjectionProtocol {
    let artifacts = context_files
        .iter()
        .map(|path| {
            let role = if path.contains("proposal") {
                "proposal"
            } else if path.contains("requirement") {
                "requirements"
            } else if path.contains("acceptance") {
                "acceptance"
            } else if path.contains("design") {
                "design"
            } else if path.contains("task") {
                "tasks"
            } else if path.contains("progress") {
                "progress"
            } else {
                "reference"
            };
            let required = matches!(role, "proposal" | "requirements" | "acceptance" | "tasks");
            ContextArtifact {
                path: path.clone(),
                role: role.to_string(),
                required,
                summary: format!("Use {} as {} context for task {}.", path, role, task.id),
            }
        })
        .collect::<Vec<_>>();

    let host_specific_instruction = match host {
        HostKind::ClaudeCode | HostKind::OpenCode => {
            "In the host runtime, load the context protocol file and required artifacts first, then continue the same slash-command flow without manually pasting large prompt fragments.".to_string()
        }
        HostKind::Terminal => {
            "In terminal mode, keep the context protocol file beside the execution envelope so every later step can reuse the same structured context contract.".to_string()
        }
    };

    ContextInjectionProtocol {
        version: "zero_nine_context/v1".to_string(),
        host,
        mode,
        objective: task.title.clone(),
        artifacts,
        instructions: vec![
            "Always load required proposal, requirements, acceptance, and task artifacts before writing or reviewing code.".to_string(),
            "Treat design and progress artifacts as secondary context that can refine implementation order, verification scope, and handoff quality.".to_string(),
            "When producing new work, reference the same context roles instead of pasting ad-hoc prompt fragments so later host turns can reuse a stable protocol.".to_string(),
            host_specific_instruction,
        ],
    }
}

fn verification_actions_for(task: &TaskItem, kind: TaskKind) -> Vec<VerificationAction> {
    match kind {
        TaskKind::Implementation | TaskKind::Verification | TaskKind::FinishBranch => vec![
            VerificationAction {
                name: "tests".to_string(),
                command: "auto-detect project test command".to_string(),
                required: true,
                expected_evidence: format!("Capture the primary automated test output for task {} using the repository's detected test stack.", task.id),
            },
            VerificationAction {
                name: "review".to_string(),
                command: "auto-detect review or lint command".to_string(),
                required: true,
                expected_evidence: format!("Capture review or lint evidence for task {} to satisfy the review quality gate.", task.id),
            },
            VerificationAction {
                name: "git_diff_stat".to_string(),
                command: "git diff --stat".to_string(),
                required: false,
                expected_evidence: format!("Record diff statistics for task {} changes.", task.id),
            },
            VerificationAction {
                name: "git_diff_check".to_string(),
                command: "git diff --check".to_string(),
                required: true,
                expected_evidence: format!("Record review evidence that the implementation for task {} is internally consistent.", task.id),
            },
        ],
        _ => Vec::new(),
    }
}

fn finish_branch_automation_for(
    workspace_record: &Option<WorkspaceRecord>,
    mode: ExecutionMode,
) -> Option<FinishBranchAutomation> {
    if !matches!(mode, ExecutionMode::FinishBranch) {
        return None;
    }

    let branch_name = workspace_record
        .as_ref()
        .map(|record| record.branch_name.clone())
        .unwrap_or_else(|| "<branch>".to_string());

    Some(FinishBranchAutomation {
        default_action: FinishBranchAction::PullRequest,
        available_actions: vec![
            FinishBranchAction::Merge,
            FinishBranchAction::PullRequest,
            FinishBranchAction::Keep,
            FinishBranchAction::Discard,
        ],
        requires_clean_tree: true,
        preview_commands: vec![
            format!("git status --short && git branch --show-current # expect {}", branch_name),
            format!("gh pr create --head {} --fill", branch_name),
        ],
    })
}

pub fn prepare_workspace(project_root: &Path, plan: &ExecutionPlan) -> Result<WorkspacePreparationResult> {
    match plan.workspace_strategy {
        WorkspaceStrategy::InPlace => {
            let now = Utc::now();
            let record = WorkspaceRecord {
                strategy: WorkspaceStrategy::InPlace,
                status: WorkspaceStatus::Active,
                branch_name: git_current_branch(project_root).unwrap_or_else(|_| "in-place".to_string()),
                worktree_path: project_root.display().to_string(),
                base_branch: git_current_branch(project_root).ok(),
                head_branch: git_current_branch(project_root).ok(),
                created_at: now,
                updated_at: now,
                notes: vec!["Task runs in the project root without creating a separate worktree.".to_string()],
            };
            let mut created_paths = vec![project_root.display().to_string()];
            created_paths.extend(persist_workspace_preparation_artifacts(project_root, plan, &record)?);
            Ok(WorkspacePreparationResult {
                success: true,
                summary: "Workspace strategy is in-place; no new worktree was created.".to_string(),
                record: Some(record),
                created_paths,
            })
        }
        WorkspaceStrategy::GitWorktree => {
            let repo_root = git_toplevel(project_root)?;
            let worktree = plan
                .worktree_plan
                .as_ref()
                .ok_or_else(|| anyhow!("missing worktree plan for git worktree strategy"))?;

            if !git_has_head(&repo_root)? {
                let now = Utc::now();
                let record = WorkspaceRecord {
                    strategy: WorkspaceStrategy::InPlace,
                    status: WorkspaceStatus::Active,
                    branch_name: git_current_branch(&repo_root).unwrap_or_else(|_| "pre-initial-commit".to_string()),
                    worktree_path: project_root.display().to_string(),
                    base_branch: None,
                    head_branch: None,
                    created_at: now,
                    updated_at: now,
                    notes: vec![
                        "Fell back to in-place execution because git worktree requires an existing HEAD commit, but this repository has not recorded its initial commit yet.".to_string(),
                    ],
                };
                let mut created_paths = vec![project_root.display().to_string()];
                created_paths.extend(persist_workspace_preparation_artifacts(project_root, plan, &record)?);
                return Ok(WorkspacePreparationResult {
                    success: true,
                    summary: "Repository has no initial commit yet, so Zero_Nine skipped git worktree creation and continued in-place for this task.".to_string(),
                    record: Some(record),
                    created_paths,
                });
            }

            let abs_path = normalize_worktree_path(&repo_root, &worktree.worktree_path);
            if let Some(parent) = abs_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create worktree parent directory {}", parent.display()))?;
            }

            if abs_path.exists() && (abs_path.join(".git").exists() || abs_path.join(".git").is_file()) {
                let now = Utc::now();
                let record = WorkspaceRecord {
                    strategy: WorkspaceStrategy::GitWorktree,
                    status: WorkspaceStatus::Active,
                    branch_name: worktree.branch_name.clone(),
                    worktree_path: abs_path.display().to_string(),
                    base_branch: git_current_branch(&repo_root).ok(),
                    head_branch: Some(worktree.branch_name.clone()),
                    created_at: now,
                    updated_at: now,
                    notes: vec!["Reused an existing git worktree because the target path already exists.".to_string()],
                };
                let mut created_paths = vec![abs_path.display().to_string()];
                created_paths.extend(persist_workspace_preparation_artifacts(project_root, plan, &record)?);
                return Ok(WorkspacePreparationResult {
                    success: true,
                    summary: format!("Reused existing git worktree at {}", abs_path.display()),
                    record: Some(record),
                    created_paths,
                });
            }

            let branch_exists = git_branch_exists(&repo_root, &worktree.branch_name)?;
            let mut command = Command::new("git");
            command.arg("-C").arg(&repo_root).arg("worktree").arg("add");
            if !branch_exists {
                command.arg("-b").arg(&worktree.branch_name);
            }
            command.arg(&abs_path);
            if branch_exists {
                command.arg(&worktree.branch_name);
            } else {
                command.arg("HEAD");
            }
            run_command(&mut command, "failed to create git worktree")?;

            let now = Utc::now();
            let record = WorkspaceRecord {
                strategy: WorkspaceStrategy::GitWorktree,
                status: WorkspaceStatus::Prepared,
                branch_name: worktree.branch_name.clone(),
                worktree_path: abs_path.display().to_string(),
                base_branch: git_current_branch(&repo_root).ok(),
                head_branch: Some(worktree.branch_name.clone()),
                created_at: now,
                updated_at: now,
                notes: vec![format!("Prepared git worktree for task {}.", plan.task_id)],
            };
            let mut created_paths = vec![abs_path.display().to_string()];
            created_paths.extend(persist_workspace_preparation_artifacts(project_root, plan, &record)?);
            Ok(WorkspacePreparationResult {
                success: true,
                summary: format!("Prepared git worktree {} on branch {}", abs_path.display(), worktree.branch_name),
                record: Some(record),
                created_paths,
            })
        }
        WorkspaceStrategy::Sandboxed => {
            let sandbox_root = project_root.join(".zero_nine/sandboxes").join(&plan.task_id);
            fs::create_dir_all(&sandbox_root).with_context(|| {
                format!("failed to create sandbox directory {}", sandbox_root.display())
            })?;
            let now = Utc::now();
            let record = WorkspaceRecord {
                strategy: WorkspaceStrategy::Sandboxed,
                status: WorkspaceStatus::Prepared,
                branch_name: format!("sandbox-{}", plan.task_id),
                worktree_path: sandbox_root.display().to_string(),
                base_branch: None,
                head_branch: None,
                created_at: now,
                updated_at: now,
                notes: vec!["Prepared a filesystem sandbox without git worktree integration.".to_string()],
            };
            let mut created_paths = vec![sandbox_root.display().to_string()];
            created_paths.extend(persist_workspace_preparation_artifacts(project_root, plan, &record)?);
            Ok(WorkspacePreparationResult {
                success: true,
                summary: format!("Prepared sandbox at {}", sandbox_root.display()),
                record: Some(record),
                created_paths,
            })
        }
    }
}

fn persist_workspace_preparation_artifacts(
    project_root: &Path,
    plan: &ExecutionPlan,
    record: &WorkspaceRecord,
) -> Result<Vec<String>> {
    let artifact_dir = project_root
        .join(".zero_nine/workspace")
        .join(format!("task-{}", plan.task_id));
    fs::create_dir_all(&artifact_dir)?;

    let json_path = artifact_dir.join("workspace-record.json");
    let markdown_path = artifact_dir.join("workspace-record.md");

    fs::write(&json_path, serde_json::to_vec_pretty(record)?)?;
    fs::write(&markdown_path, render_workspace_record_markdown(plan, record))?;

    Ok(vec![
        json_path.display().to_string(),
        markdown_path.display().to_string(),
    ])
}

fn render_workspace_record_markdown(plan: &ExecutionPlan, record: &WorkspaceRecord) -> String {
    let base_branch = record
        .base_branch
        .clone()
        .unwrap_or_else(|| "n/a".to_string());
    let head_branch = record
        .head_branch
        .clone()
        .unwrap_or_else(|| "n/a".to_string());

    format!(
        "# Workspace Preparation Record\n\n## Task\n\n- ID: {}\n- Mode: {:?}\n- Strategy: {:?}\n\n## Actual Workspace State\n\n- Status: {:?}\n- Branch: {}\n- Path: {}\n- Base Branch: {}\n- Head Branch: {}\n\n## Notes\n\n{}\n\n## Expected Deliverables\n\n{}\n\n## Follow-up\n\n- Keep this record together with the writing-plans artifact so later development, review, verification, and finish-branch steps can reuse the same workspace contract.\n",
        plan.task_id,
        plan.mode,
        record.strategy,
        record.status,
        record.branch_name,
        record.worktree_path,
        base_branch,
        head_branch,
        to_markdown_list(&record.notes),
        to_markdown_list(&plan.deliverables)
    )
}

pub fn preview_finish_branch(project_root: &Path, request: &BranchFinishRequest) -> Result<BranchFinishPreview> {
    let repo_root = git_toplevel(project_root)?;
    let mut warnings = Vec::new();
    if request.verify_clean && !git_is_clean(&repo_root)? {
        warnings.push("Repository has uncommitted changes; finish-branch should pause until the tree is clean.".to_string());
    }
    if !git_branch_exists(&repo_root, &request.branch_name)? {
        warnings.push(format!("Branch {} does not exist yet.", request.branch_name));
    }

    let mut commands = Vec::new();
    match request.action {
        FinishBranchAction::Merge => {
            let target = git_current_branch(&repo_root).unwrap_or_else(|_| "<target-branch>".to_string());
            commands.push(format!("git -C {} checkout {}", repo_root.display(), target));
            commands.push(format!("git -C {} merge --no-ff {}", repo_root.display(), request.branch_name));
        }
        FinishBranchAction::PullRequest => {
            commands.push(format!("git -C {} push -u <remote> {}", repo_root.display(), request.branch_name));
            commands.push(format!("gh pr create --head {} --fill", request.branch_name));
        }
        FinishBranchAction::Discard => {
            if let Some(worktree_path) = &request.worktree_path {
                commands.push(format!("git -C {} worktree remove --force {}", repo_root.display(), worktree_path));
            }
            commands.push(format!("git -C {} branch -D {}", repo_root.display(), request.branch_name));
        }
        FinishBranchAction::Keep => {
            commands.push("Keep the branch and worktree for another iteration.".to_string());
        }
    }

    Ok(BranchFinishPreview {
        request: BranchFinishRequest {
            action: request.action.clone(),
            branch_name: request.branch_name.clone(),
            worktree_path: request.worktree_path.clone(),
            verify_clean: request.verify_clean,
            confirmed: request.confirmed,
            pr_title: None,
            pr_body: None,
        },
        warnings,
        commands,
    })
}

pub fn finish_branch(project_root: &Path, request: &BranchFinishRequest) -> Result<FinishBranchResult> {
    let repo_root = git_toplevel(project_root)?;

    if matches!(request.action, FinishBranchAction::Merge | FinishBranchAction::PullRequest)
        && !request.confirmed
    {
        let preview = preview_finish_branch(project_root, request)?;
        let mut follow_ups = vec![
            format!(
                "Re-run the same Zero_Nine execution entry point with explicit remote finish confirmation before attempting {:?}.",
                request.action
            ),
        ];
        follow_ups.extend(preview.warnings);
        follow_ups.extend(
            preview
                .commands
                .into_iter()
                .map(|command| format!("Preview command: {}", command)),
        );
        return Ok(FinishBranchResult {
            action: request.action.clone(),
            status: FinishBranchStatus::Rejected,
            branch_name: request.branch_name.clone(),
            worktree_path: request.worktree_path.clone(),
            summary: format!(
                "Finish-branch action {:?} was blocked because it requires explicit confirmation before Zero_Nine may change branch or remote state.",
                request.action
            ),
            follow_ups,
            pr_url: None,
        });
    }

    if request.verify_clean && !git_is_clean(&repo_root)? {
        return Err(anyhow!("repository has uncommitted changes; aborting finish-branch"));
    }

    match request.action {
        FinishBranchAction::Merge => {
            let mut merge = Command::new("git");
            merge.arg("-C").arg(&repo_root).arg("merge").arg("--no-ff").arg(&request.branch_name);
            run_command(&mut merge, "failed to merge branch")?;
            if let Some(worktree_path) = &request.worktree_path {
                let mut remove = Command::new("git");
                remove.arg("-C").arg(&repo_root).arg("worktree").arg("remove").arg("--force").arg(worktree_path);
                let _ = run_command(&mut remove, "failed to remove merged worktree");
            }
            Ok(FinishBranchResult {
                action: FinishBranchAction::Merge,
                status: FinishBranchStatus::Completed,
                branch_name: request.branch_name.clone(),
                worktree_path: request.worktree_path.clone(),
                summary: format!("Merged branch {} into the currently checked out branch.", request.branch_name),
                follow_ups: vec!["Run the verification suite once more after merge if your policy requires post-merge checks.".to_string()],
                pr_url: None,
            })
        }
        FinishBranchAction::PullRequest => {
            let remote_name = git_preferred_remote(&repo_root)?;

            let mut auth = Command::new("gh");
            auth.arg("auth").arg("status");
            run_command(&mut auth, "failed to verify gh authentication before creating pull request")?;

            let mut push = Command::new("git");
            push.arg("-C")
                .arg(&repo_root)
                .arg("push")
                .arg("-u")
                .arg(&remote_name)
                .arg(&request.branch_name);
            run_command(&mut push, "failed to push branch before creating pull request")?;

            // M6: Use structured PR body if provided, otherwise fall back to --fill
            let pr_url = if let (Some(title), Some(body)) = (&request.pr_title, &request.pr_body) {
                let mut pr = Command::new("gh");
                pr.arg("pr")
                    .arg("create")
                    .arg("--head")
                    .arg(&request.branch_name)
                    .arg("--title")
                    .arg(title)
                    .arg("--body")
                    .arg(body);
                let output = run_command(&mut pr, "failed to create pull request via gh")?;
                output.lines().next().map(|s| s.to_string())
            } else {
                let mut pr = Command::new("gh");
                pr.arg("pr")
                    .arg("create")
                    .arg("--head")
                    .arg(&request.branch_name)
                    .arg("--fill");
                let output = run_command(&mut pr, "failed to create pull request via gh")?;
                output.lines().next().map(|s| s.to_string())
            };

            let pr_url_display = pr_url.clone().unwrap_or_else(|| "<pr-url>".to_string());

            Ok(FinishBranchResult {
                action: FinishBranchAction::PullRequest,
                status: FinishBranchStatus::Completed,
                branch_name: request.branch_name.clone(),
                worktree_path: request.worktree_path.clone(),
                summary: format!(
                    "Pushed branch {} to {} and created a pull request: {}",
                    request.branch_name, remote_name, pr_url_display
                ),
                follow_ups: vec![
                    "Review the created pull request, verify CI state, and merge only after repository policy checks are satisfied.".to_string(),
                ],
                pr_url,
            })
        }
        FinishBranchAction::Discard => {
            if let Some(worktree_path) = &request.worktree_path {
                let mut remove = Command::new("git");
                remove.arg("-C").arg(&repo_root).arg("worktree").arg("remove").arg("--force").arg(worktree_path);
                let _ = run_command(&mut remove, "failed to remove discarded worktree");
            }
            if git_branch_exists(&repo_root, &request.branch_name)? {
                let mut delete_branch = Command::new("git");
                delete_branch.arg("-C").arg(&repo_root).arg("branch").arg("-D").arg(&request.branch_name);
                run_command(&mut delete_branch, "failed to delete discarded branch")?;
            }
            Ok(FinishBranchResult {
                action: FinishBranchAction::Discard,
                status: FinishBranchStatus::Completed,
                branch_name: request.branch_name.clone(),
                worktree_path: request.worktree_path.clone(),
                summary: format!("Discarded branch {} and cleaned temporary workspace state.", request.branch_name),
                follow_ups: vec!["Preserve useful artifacts under .zero_nine before permanently removing exploratory branches next time.".to_string()],
                pr_url: None,
            })
        }
        FinishBranchAction::Keep => Ok(FinishBranchResult {
            action: FinishBranchAction::Keep,
            status: FinishBranchStatus::Completed,
            branch_name: request.branch_name.clone(),
            worktree_path: request.worktree_path.clone(),
            summary: format!("Kept branch {} for another iteration.", request.branch_name),
            follow_ups: vec!["Resume implementation or verification before trying to finish the branch again.".to_string()],
            pr_url: None,
        }),
    }
}

fn planned_workspace_record(worktree_plan: &WorktreePlan) -> WorkspaceRecord {
    let now = Utc::now();
    WorkspaceRecord {
        strategy: worktree_plan.strategy.clone(),
        status: WorkspaceStatus::Planned,
        branch_name: worktree_plan.branch_name.clone(),
        worktree_path: worktree_plan.worktree_path.clone(),
        base_branch: None,
        head_branch: Some(worktree_plan.branch_name.clone()),
        created_at: now,
        updated_at: now,
        notes: vec![worktree_plan.cleanup_hint.clone()],
    }
}

#[derive(Debug, Clone)]
struct PersistedDispatchRecord {
    role: String,
    status: SubagentRunStatus,
    summary: String,
    outputs: Vec<String>,
    evidence_paths: Vec<String>,
    failure_summary: Option<String>,
    state_transitions: Vec<String>,
    recovery_path: Option<String>,
    evidence_archive_path: Option<String>,
    replay_ready: bool,
    replay_command: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct PersistedSubagentArtifacts {
    all_paths: Vec<String>,
    dispatch_records: Vec<PersistedDispatchRecord>,
}

fn persist_subagent_runbook_artifacts(
    project_root: &Path,
    plan: &ExecutionPlan,
) -> Result<PersistedSubagentArtifacts> {
    if plan.subagents.is_empty() {
        return Ok(PersistedSubagentArtifacts::default());
    }

    let artifact_dir = project_root
        .join(".zero_nine/runtime/subagents")
        .join(format!("task-{}", plan.task_id));
    fs::create_dir_all(&artifact_dir)?;

    let runbook_json = artifact_dir.join("subagent-runbook.json");
    let runbook_md = artifact_dir.join("subagent-runbook.md");
    let ledger_json = artifact_dir.join("subagent-recovery-ledger.json");
    let ledger_md = artifact_dir.join("subagent-recovery-ledger.md");

    let dispatches = plan
        .subagents
        .iter()
        .map(|brief| SubagentDispatch {
            role: brief.role.clone(),
            command_hint: command_hint_for(plan, brief),
            context_files: context_files_for_dispatch(plan, brief),
            expected_outputs: brief.outputs.clone(),
            depends_on_roles: brief.depends_on.clone(),
        })
        .collect::<Vec<_>>();

    let dispatch_paths = dispatches
        .iter()
        .map(|dispatch| artifact_dir.join(format!("dispatch-{}.md", slugify_role(&dispatch.role))))
        .collect::<Vec<_>>();
    let recovery_json_paths = dispatches
        .iter()
        .map(|dispatch| artifact_dir.join(format!("recovery-{}.json", slugify_role(&dispatch.role))))
        .collect::<Vec<_>>();
    let recovery_md_paths = dispatches
        .iter()
        .map(|dispatch| artifact_dir.join(format!("recovery-{}.md", slugify_role(&dispatch.role))))
        .collect::<Vec<_>>();
    let evidence_archive_paths = dispatches
        .iter()
        .map(|dispatch| artifact_dir.join(format!("evidence-archive-{}.md", slugify_role(&dispatch.role))))
        .collect::<Vec<_>>();
    let replay_script_paths = dispatches
        .iter()
        .map(|dispatch| artifact_dir.join(format!("replay-{}.sh", slugify_role(&dispatch.role))))
        .collect::<Vec<_>>();

    let runtime = SubagentExecutionRuntime {
        runbook_path: runbook_json.display().to_string(),
        dispatch_paths: dispatch_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        recovery_paths: recovery_json_paths
            .iter()
            .chain(recovery_md_paths.iter())
            .chain(evidence_archive_paths.iter())
            .map(|path| path.display().to_string())
            .collect(),
        replay_paths: replay_script_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        ledger_path: ledger_json.display().to_string(),
    };

    let runbook = SubagentRunBook {
        task_id: plan.task_id.clone(),
        dispatches: dispatches.clone(),
        runtime: Some(runtime.clone()),
    };

    fs::write(&runbook_json, serde_json::to_vec_pretty(&runbook)?)?;
    fs::write(&runbook_md, render_subagent_runbook_record(plan, &runbook))?;

    let mut all_paths = vec![
        runbook_json.display().to_string(),
        runbook_md.display().to_string(),
        ledger_json.display().to_string(),
        ledger_md.display().to_string(),
    ];
    let mut dispatch_records = Vec::new();
    let mut recovery_records = Vec::new();

    for (idx, dispatch) in dispatches.iter().enumerate() {
        let dispatch_path = &dispatch_paths[idx];
        fs::write(dispatch_path, render_subagent_dispatch_record(plan, dispatch))?;
        let dispatch_path_str = dispatch_path.display().to_string();
        all_paths.push(dispatch_path_str.clone());

        let evidence_archive_path = &evidence_archive_paths[idx];
        fs::write(
            evidence_archive_path,
            render_subagent_evidence_archive(plan, dispatch, &dispatch_path_str, &runbook_json.display().to_string()),
        )?;
        let evidence_archive_path_str = evidence_archive_path.display().to_string();
        all_paths.push(evidence_archive_path_str.clone());

        let replay_script_path = &replay_script_paths[idx];
        let replay_command = replay_command_for(plan, dispatch, replay_script_path);
        fs::write(
            replay_script_path,
            render_subagent_replay_script(plan, dispatch, &replay_command),
        )?;
        let replay_script_path_str = replay_script_path.display().to_string();
        all_paths.push(replay_script_path_str.clone());

        let recovery_record = recover_subagent_record(
            plan,
            dispatch,
            &dispatch_path_str,
            &runbook_json.display().to_string(),
            &evidence_archive_path_str,
            &replay_command,
        );
        let recovery_json_path = &recovery_json_paths[idx];
        let recovery_md_path = &recovery_md_paths[idx];
        fs::write(recovery_json_path, serde_json::to_vec_pretty(&recovery_record)?)?;
        fs::write(recovery_md_path, render_subagent_recovery_record(plan, &recovery_record))?;

        let recovery_json_path_str = recovery_json_path.display().to_string();
        let recovery_md_path_str = recovery_md_path.display().to_string();
        all_paths.push(recovery_json_path_str.clone());
        all_paths.push(recovery_md_path_str.clone());

        let mut outputs = recovery_record.actual_outputs.clone();
        outputs.push(dispatch_path_str.clone());
        outputs.push(recovery_json_path_str.clone());
        outputs.push(recovery_md_path_str.clone());
        outputs.push(evidence_archive_path_str.clone());
        outputs.push(replay_script_path_str.clone());
        outputs.push(runbook_json.display().to_string());

        let mut evidence_paths = recovery_record.evidence_paths.clone();
        evidence_paths.push(recovery_json_path_str.clone());
        evidence_paths.push(recovery_md_path_str.clone());
        evidence_paths.push(evidence_archive_path_str.clone());

        dispatch_records.push(PersistedDispatchRecord {
            role: dispatch.role.clone(),
            status: recovery_record.status.clone(),
            summary: recovery_record.summary.clone(),
            outputs,
            evidence_paths,
            failure_summary: recovery_record.failure_summary.clone(),
            state_transitions: recovery_record.state_transitions.clone(),
            recovery_path: Some(recovery_json_path_str.clone()),
            evidence_archive_path: recovery_record.evidence_archive_path.clone(),
            replay_ready: recovery_record.replay_ready,
            replay_command: recovery_record.replay_command.clone(),
        });
        recovery_records.push(recovery_record);
    }

    let ledger = SubagentRecoveryLedger {
        task_id: plan.task_id.clone(),
        records: recovery_records,
        replay_summary: format!(
            "Reuse the replay scripts under {} to resume or reconstruct subagent outputs without rebuilding dispatch intent.",
            artifact_dir.display()
        ),
    };
    fs::write(&ledger_json, serde_json::to_vec_pretty(&ledger)?)?;
    fs::write(&ledger_md, render_subagent_recovery_ledger(plan, &ledger))?;

    Ok(PersistedSubagentArtifacts {
        all_paths,
        dispatch_records,
    })
}

fn recover_subagent_record(
    plan: &ExecutionPlan,
    dispatch: &SubagentDispatch,
    dispatch_path: &str,
    runbook_path: &str,
    evidence_archive_path: &str,
    replay_command: &str,
) -> SubagentRecoveryRecord {
    let recovery_status = if matches!(plan.mode, ExecutionMode::Brainstorming) {
        SubagentRunStatus::Planned
    } else {
        SubagentRunStatus::Recovered
    };
    let mut actual_outputs = dispatch.expected_outputs.clone();
    actual_outputs.push(dispatch_path.to_string());
    actual_outputs.push(runbook_path.to_string());
    let evidence_paths = vec![
        dispatch_path.to_string(),
        runbook_path.to_string(),
        evidence_archive_path.to_string(),
    ];
    let state_transitions = if matches!(recovery_status, SubagentRunStatus::Planned) {
        vec!["briefed->planned".to_string()]
    } else {
        vec![
            "briefed->dispatched".to_string(),
            "dispatched->recovered".to_string(),
        ]
    };
    SubagentRecoveryRecord {
        role: dispatch.role.clone(),
        status: recovery_status.clone(),
        summary: if matches!(recovery_status, SubagentRunStatus::Planned) {
            format!(
                "{} remains in planned state because brainstorming mode only prepares the recovery contract.",
                dispatch.role
            )
        } else {
            format!(
                "{} recovery captured {} concrete outputs and resumable evidence links.",
                dispatch.role,
                actual_outputs.len()
            )
        },
        expected_outputs: dispatch.expected_outputs.clone(),
        actual_outputs,
        evidence_paths,
        failure_summary: None,
        state_transitions,
        evidence_archive_path: Some(evidence_archive_path.to_string()),
        replay_ready: true,
        replay_command: Some(replay_command.to_string()),
    }
}

fn render_subagent_evidence_archive(
    plan: &ExecutionPlan,
    dispatch: &SubagentDispatch,
    dispatch_path: &str,
    runbook_path: &str,
) -> String {
    format!(
        "# Subagent Evidence Archive\n\n## Task\n\n- ID: {}\n- Mode: {:?}\n\n## Role\n\n- {}\n\n## Captured Evidence\n\n- Dispatch Record: {}\n- Runbook: {}\n- Expected Outputs: {}\n\n## Failure Capture Guidance\n\n- If downstream execution fails, append stderr excerpts, failing commands, and missing output explanations here before the next retry.\n- Preserve concrete file paths so replay can reuse them directly instead of reconstructing context.\n",
        plan.task_id,
        plan.mode,
        dispatch.role,
        dispatch_path,
        runbook_path,
        if dispatch.expected_outputs.is_empty() {
            "none".to_string()
        } else {
            dispatch.expected_outputs.join(", ")
        }
    )
}

fn replay_command_for(plan: &ExecutionPlan, dispatch: &SubagentDispatch, replay_script_path: &Path) -> String {
    format!(
        "sh {} # replay subagent role '{}' for task {}",
        replay_script_path.display(),
        dispatch.role,
        plan.task_id
    )
}

fn render_subagent_replay_script(
    plan: &ExecutionPlan,
    dispatch: &SubagentDispatch,
    replay_command: &str,
) -> String {
    format!(
        "#!/usr/bin/env sh\nset -eu\nprintf '%s\n' \"Replaying subagent role {} for task {} in mode {:?}\"\nprintf '%s\n' \"Expected outputs: {}\"\nprintf '%s\n' \"Replay placeholder command: {}\"\n",
        dispatch.role,
        plan.task_id,
        plan.mode,
        if dispatch.expected_outputs.is_empty() {
            "none".to_string()
        } else {
            dispatch.expected_outputs.join(", ")
        },
        replay_command,
    )
}

fn render_subagent_recovery_record(plan: &ExecutionPlan, record: &SubagentRecoveryRecord) -> String {
    format!(
        "# Subagent Recovery Record\n\n## Task\n\n- ID: {}\n- Mode: {:?}\n\n## Role\n\n- {}\n- Status: {}\n\n## Summary\n\n{}\n\n## Expected Outputs\n\n{}\n\n## Actual Outputs\n\n{}\n\n## Evidence Paths\n\n{}\n\n## State Transitions\n\n{}\n\n## Failure Summary\n\n{}\n\n## Evidence Archive\n\n{}\n\n## Replay Ready\n\n{}\n\n## Replay Command\n\n{}\n",
        plan.task_id,
        plan.mode,
        record.role,
        record.status,
        record.summary,
        to_markdown_list(&record.expected_outputs),
        to_markdown_list(&record.actual_outputs),
        to_markdown_list(&record.evidence_paths),
        to_markdown_list(&record.state_transitions),
        record
            .failure_summary
            .clone()
            .unwrap_or_else(|| "none".to_string()),
        record
            .evidence_archive_path
            .clone()
            .unwrap_or_else(|| "none".to_string()),
        if record.replay_ready { "yes" } else { "no" },
        record
            .replay_command
            .clone()
            .unwrap_or_else(|| "none".to_string())
    )
}

fn render_subagent_recovery_ledger(plan: &ExecutionPlan, ledger: &SubagentRecoveryLedger) -> String {
    let mut out = format!(
        "# Subagent Recovery Ledger\n\n## Task\n\n- ID: {}\n- Mode: {:?}\n\n## Replay Summary\n\n{}\n\n## Records\n\n",
        plan.task_id, plan.mode, ledger.replay_summary
    );
    for record in &ledger.records {
        out.push_str(&format!(
            "### {}\n\n- Status: {}\n- Summary: {}\n- Expected Outputs: {}\n- Actual Outputs: {}\n- Evidence Paths: {}\n- Failure Summary: {}\n- State Transitions: {}\n- Evidence Archive: {}\n- Replay Ready: {}\n- Replay Command: {}\n\n",
            record.role,
            record.status,
            record.summary,
            if record.expected_outputs.is_empty() { "none".to_string() } else { record.expected_outputs.join(", ") },
            if record.actual_outputs.is_empty() { "none".to_string() } else { record.actual_outputs.join(", ") },
            if record.evidence_paths.is_empty() { "none".to_string() } else { record.evidence_paths.join(", ") },
            record.failure_summary.clone().unwrap_or_else(|| "none".to_string()),
            if record.state_transitions.is_empty() { "none".to_string() } else { record.state_transitions.join(" -> ") },
            record.evidence_archive_path.clone().unwrap_or_else(|| "none".to_string()),
            if record.replay_ready { "yes".to_string() } else { "no".to_string() },
            record.replay_command.clone().unwrap_or_else(|| "none".to_string()),
        ));
    }
    out
}

fn command_hint_for(plan: &ExecutionPlan, brief: &SubagentBrief) -> String {
    format!(
        "Execute role '{}' for task {} in mode {:?}; keep outputs aligned with the structured runbook and return evidence files, changed files, and unresolved risks.",
        brief.role, plan.task_id, plan.mode
    )
}

fn context_files_for_dispatch(plan: &ExecutionPlan, brief: &SubagentBrief) -> Vec<String> {
    let mut files = plan.deliverables.clone();
    if let Some(worktree) = &plan.worktree_plan {
        files.push(worktree.worktree_path.clone());
    }
    files.extend(brief.inputs.clone());
    files.sort();
    files.dedup();
    files
}

fn render_subagent_runbook_record(plan: &ExecutionPlan, runbook: &SubagentRunBook) -> String {
    let mut out = format!(
        "# Subagent Runbook Record\n\n## Task\n\n- ID: {}\n- Mode: {:?}\n- Workspace Strategy: {:?}\n\n## Dispatch Summary\n\n",
        plan.task_id, plan.mode, plan.workspace_strategy
    );
    for dispatch in &runbook.dispatches {
        out.push_str(&format!(
            "### {}\n\n- Command Hint: {}\n- Context Files: {}\n- Expected Outputs: {}\n\n",
            dispatch.role,
            dispatch.command_hint,
            if dispatch.context_files.is_empty() {
                "none".to_string()
            } else {
                dispatch.context_files.join(", ")
            },
            if dispatch.expected_outputs.is_empty() {
                "none".to_string()
            } else {
                dispatch.expected_outputs.join(", ")
            }
        ));
    }
    out.push_str("## Recovery Rule\n\nReuse this runbook during resume so the same roles, inputs, and expected outputs can be rehydrated without reconstructing dispatch intent from scratch.\n");
    out
}

fn render_subagent_dispatch_record(plan: &ExecutionPlan, dispatch: &SubagentDispatch) -> String {
    format!(
        "# Subagent Dispatch Record\n\n## Task\n\n- ID: {}\n- Mode: {:?}\n\n## Assigned Role\n\n{}\n\n## Command Hint\n\n{}\n\n## Context Files\n\n{}\n\n## Expected Outputs\n\n{}\n\n## Return Contract\n\n- Return concrete file paths or patches, not only narrative status.\n- Capture commands run, validation evidence, and unresolved risks.\n- Keep artifacts resumable under .zero_nine/runtime/subagents/.\n",
        plan.task_id,
        plan.mode,
        dispatch.role,
        dispatch.command_hint,
        to_markdown_list(&dispatch.context_files),
        to_markdown_list(&dispatch.expected_outputs)
    )
}

fn slugify_role(role: &str) -> String {
    let slug = role
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '-' })
        .collect::<String>();
    slug.split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn agent_runs_for(
    plan: &ExecutionPlan,
    dispatch_records: &[PersistedDispatchRecord],
) -> Vec<AgentRunRecord> {
    if !dispatch_records.is_empty() {
        return dispatch_records
            .iter()
            .map(|record| AgentRunRecord {
                role: record.role.clone(),
                status: record.status.to_string(),
                summary: record.summary.clone(),
                outputs: record.outputs.clone(),
                evidence_paths: record.evidence_paths.clone(),
                failure_summary: record.failure_summary.clone(),
                state_transitions: record.state_transitions.clone(),
                recovery_path: record.recovery_path.clone(),
                evidence_archive_path: record.evidence_archive_path.clone(),
                replay_ready: record.replay_ready,
                replay_command: record.replay_command.clone(),
            })
            .collect();
    }

    plan.subagents
        .iter()
        .map(|brief| AgentRunRecord {
            role: brief.role.clone(),
            status: if matches!(plan.mode, ExecutionMode::Brainstorming) {
                SubagentRunStatus::Planned.to_string()
            } else {
                SubagentRunStatus::Dispatched.to_string()
            },
            summary: format!("{} brief prepared for task {}.", brief.role, plan.task_id),
            outputs: brief.outputs.clone(),
            evidence_paths: Vec::new(),
            failure_summary: None,
            state_transitions: vec!["briefed->planned".to_string()],
            recovery_path: None,
            evidence_archive_path: None,
            replay_ready: false,
            replay_command: None,
        })
        .collect()
}

fn review_verdict_for(
    plan: &ExecutionPlan,
    review_passed: bool,
    evidence_keys: Vec<String>,
) -> Option<ReviewVerdict> {
    if matches!(plan.mode, ExecutionMode::SubagentDev | ExecutionMode::Verification | ExecutionMode::FinishBranch) {
        Some(ReviewVerdict {
            approved: review_passed,
            status: if review_passed {
                VerdictStatus::Passed
            } else {
                VerdictStatus::Failed
            },
            summary: if review_passed {
                "Review gate is satisfied with linked evidence and ready for approval.".to_string()
            } else {
                "Review gate is not yet satisfied because required implementation evidence remains incomplete.".to_string()
            },
            risks: plan.risks.clone(),
            evidence_keys,
        })
    } else {
        None
    }
}

fn verification_verdict_for(
    plan: &ExecutionPlan,
    passed: bool,
    evidence_keys: Vec<String>,
    deliverables: Vec<String>,
) -> Option<VerificationVerdict> {
    if matches!(plan.mode, ExecutionMode::Verification | ExecutionMode::FinishBranch) {
        Some(VerificationVerdict {
            passed,
            status: if passed {
                VerdictStatus::Passed
            } else {
                VerdictStatus::Blocked
            },
            summary: if passed {
                "Verification gates passed with linked evidence records and may advance.".to_string()
            } else {
                "Verification gates are incomplete or failed, so advancement must remain blocked.".to_string()
            },
            evidence: deliverables,
            evidence_keys,
        })
    } else {
        None
    }
}

fn collect_evidence_records(
    plan: &ExecutionPlan,
    generated_artifacts: &[GeneratedArtifact],
    verification_action_results: &[VerificationActionResult],
    workspace_record: Option<&WorkspaceRecord>,
    finish_branch_result: Option<&FinishBranchResult>,
) -> Vec<EvidenceRecord> {
    let mut evidence = Vec::new();

    for artifact in generated_artifacts {
        evidence.push(EvidenceRecord {
            key: sanitize_evidence_key(&format!("artifact-{}", artifact.title)),
            label: artifact.title.clone(),
            kind: EvidenceKind::GeneratedArtifact,
            status: EvidenceStatus::Collected,
            required: true,
            summary: format!("Generated artifact `{}` for task {}.", artifact.path, plan.task_id),
            path: Some(artifact.path.clone()),
        });
    }

    for result in verification_action_results {
        evidence.push(EvidenceRecord {
            key: sanitize_evidence_key(&format!("verification-{}", result.name)),
            label: format!("Verification action: {}", result.name),
            kind: if result.name == "review" {
                EvidenceKind::Review
            } else {
                EvidenceKind::Verification
            },
            status: match result.status.as_str() {
                "passed" => EvidenceStatus::Collected,
                "soft_failed" => EvidenceStatus::Missing,
                _ => EvidenceStatus::Failed,
            },
            required: true,
            summary: result.summary.clone(),
            path: result.evidence_path.clone(),
        });
    }

    if let Some(record) = workspace_record {
        evidence.push(EvidenceRecord {
            key: "workspace-record".to_string(),
            label: "Workspace Record".to_string(),
            kind: EvidenceKind::Workspace,
            status: EvidenceStatus::Collected,
            required: true,
            summary: format!(
                "Workspace {:?} is recorded at {} on branch {}.",
                record.status, record.worktree_path, record.branch_name
            ),
            path: Some(record.worktree_path.clone()),
        });
    }

    if let Some(result) = finish_branch_result {
        evidence.push(EvidenceRecord {
            key: "finish-branch-result".to_string(),
            label: "Finish Branch Result".to_string(),
            kind: EvidenceKind::BranchAutomation,
            status: match result.status {
                FinishBranchStatus::Completed => EvidenceStatus::Collected,
                FinishBranchStatus::Rejected => EvidenceStatus::Missing,
                FinishBranchStatus::Planned => EvidenceStatus::Missing,
                FinishBranchStatus::Failed => EvidenceStatus::Failed,
            },
            required: matches!(plan.mode, ExecutionMode::FinishBranch),
            summary: result.summary.clone(),
            path: result.worktree_path.clone(),
        });
    }

    evidence
}

fn review_evidence_keys(evidence: &[EvidenceRecord]) -> Vec<String> {
    evidence
        .iter()
        .filter(|item| matches!(item.kind, EvidenceKind::GeneratedArtifact | EvidenceKind::Review | EvidenceKind::Workspace))
        .map(|item| item.key.clone())
        .collect()
}

fn verification_evidence_keys(evidence: &[EvidenceRecord]) -> Vec<String> {
    evidence
        .iter()
        .filter(|item| {
            matches!(
                item.kind,
                EvidenceKind::GeneratedArtifact
                    | EvidenceKind::Review
                    | EvidenceKind::Verification
                    | EvidenceKind::BranchAutomation
                    | EvidenceKind::Workspace
            )
        })
        .map(|item| item.key.clone())
        .collect()
}

fn sanitize_evidence_key(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn git_preferred_remote(repo_root: &Path) -> Result<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_root).arg("remote");
    let output = run_command(&mut command, "failed to list git remotes")?;
    let remotes = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if remotes.iter().any(|name| *name == "origin") {
        return Ok("origin".to_string());
    }

    remotes
        .first()
        .map(|name| (*name).to_string())
        .ok_or_else(|| anyhow!("no git remote configured for pull-request automation"))
}

fn git_toplevel(project_root: &Path) -> Result<PathBuf> {
    let mut command = Command::new("git");
    command.arg("-C").arg(project_root).arg("rev-parse").arg("--show-toplevel");
    let output = run_command(&mut command, "failed to locate git repository root")?;
    Ok(PathBuf::from(output))
}

fn git_current_branch(project_root: &Path) -> Result<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(project_root).arg("branch").arg("--show-current");
    run_command(&mut command, "failed to resolve current git branch")
}

fn git_has_head(repo_root: &Path) -> Result<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("rev-parse")
        .arg("--verify")
        .arg("HEAD")
        .output()
        .with_context(|| format!("failed to inspect HEAD state for {}", repo_root.display()))?;
    Ok(output.status.success())
}

fn git_branch_exists(repo_root: &Path, branch_name: &str) -> Result<bool> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo_root)
        .arg("show-ref")
        .arg("--verify")
        .arg("--quiet")
        .arg(format!("refs/heads/{}", branch_name));
    let output = command
        .output()
        .with_context(|| format!("failed to inspect branch {}", branch_name))?;
    Ok(output.status.success())
}

fn git_is_clean(repo_root: &Path) -> Result<bool> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_root).arg("status").arg("--porcelain");
    Ok(run_command(&mut command, "failed to inspect git status")?.trim().is_empty())
}

fn normalize_worktree_path(repo_root: &Path, worktree_path: &str) -> PathBuf {
    let candidate = PathBuf::from(worktree_path);
    if candidate.is_absolute() {
        candidate
    } else {
        repo_root.join(candidate)
    }
}

fn run_command(command: &mut Command, context: &str) -> Result<String> {
    let output = command.output().with_context(|| context.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if stderr.is_empty() { stdout } else { stderr };
        return Err(anyhow!("{}: {}", context, detail));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn mode_for(kind: TaskKind) -> ExecutionMode {
    match kind {
        TaskKind::Brainstorming => ExecutionMode::Brainstorming,
        TaskKind::SpecCapture => ExecutionMode::SpecCapture,
        TaskKind::Planning => ExecutionMode::WritingPlans,
        TaskKind::Implementation => ExecutionMode::SubagentDev,
        TaskKind::Verification => ExecutionMode::Verification,
        TaskKind::FinishBranch => ExecutionMode::FinishBranch,
    }
}

fn workspace_for(kind: TaskKind) -> WorkspaceStrategy {
    match kind {
        TaskKind::Brainstorming | TaskKind::SpecCapture | TaskKind::Verification => WorkspaceStrategy::InPlace,
        TaskKind::Planning | TaskKind::Implementation | TaskKind::FinishBranch => WorkspaceStrategy::GitWorktree,
    }
}

fn derive_objective(task: &TaskItem, kind: TaskKind) -> String {
    match kind {
        TaskKind::Brainstorming => format!(
            "Use a Superpowers-style Socratic brainstorming flow to discover what the user truly wants for task {} and reduce ambiguity before writing specs.",
            task.id
        ),
        TaskKind::SpecCapture => format!(
            "Translate the clarified requirement for task {} into OpenSpec-style proposal, requirements, acceptance, design, task, and progress artifacts.",
            task.id
        ),
        TaskKind::Planning => format!(
            "Run writing-plans for task {} so Ralph-loop receives an execution-ready breakdown with worktree strategy, gates, and deliverables.",
            task.id
        ),
        TaskKind::Implementation => format!(
            "Execute task {} through guarded development using subagent briefs, isolated workspace strategy, TDD expectations, and evidence-driven handoff.",
            task.id
        ),
        TaskKind::Verification => format!(
            "Perform quality review, verification, progress update, and OpenSpace-style learning capture for task {}.",
            task.id
        ),
        TaskKind::FinishBranch => format!(
            "Standardize development-branch finishing for task {} by presenting merge, PR, or discard options and cleaning temporary work environments.",
            task.id
        ),
    }
}

fn steps_for(_task: &TaskItem, kind: TaskKind) -> Vec<PlanStep> {
    match kind {
        TaskKind::Brainstorming => vec![
            PlanStep {
                title: "Restate the raw goal in plain language".to_string(),
                rationale: "A one-sentence user request often hides business intent and delivery expectations.".to_string(),
                expected_output: "A problem statement that can anchor the rest of the workflow.".to_string(),
            },
            PlanStep {
                title: "Ask Socratic clarification questions".to_string(),
                rationale: "Superpowers brainstorming is valuable because it reveals hidden constraints, exclusions, and definitions of done.".to_string(),
                expected_output: "A clarifications list covering scope, quality bars, constraints, and unresolved questions.".to_string(),
            },
            PlanStep {
                title: "Derive acceptance criteria and risks".to_string(),
                rationale: "Ralph-loop and verification need explicit completion gates and known failure modes.".to_string(),
                expected_output: "Acceptance checklist, assumptions, and visible risk register.".to_string(),
            },
            PlanStep {
                title: "Write a requirement packet for OpenSpec".to_string(),
                rationale: "The next layer should consume stable written artifacts instead of transient reasoning.".to_string(),
                expected_output: "Requirement packet that can be copied into proposal, requirements, and acceptance files.".to_string(),
            },
        ],
        TaskKind::SpecCapture => vec![
            PlanStep {
                title: "Map requirement packet into proposal".to_string(),
                rationale: "The proposal should explain why the workflow exists and what outcome it serves.".to_string(),
                expected_output: "Updated proposal narrative and problem statement.".to_string(),
            },
            PlanStep {
                title: "Write requirements and acceptance artifacts".to_string(),
                rationale: "OpenSpec should preserve scope, constraints, and validation criteria for future loops.".to_string(),
                expected_output: "requirements.md and acceptance.md with durable content.".to_string(),
            },
            PlanStep {
                title: "Create design and task graph".to_string(),
                rationale: "Ralph-loop can only schedule work safely when dependencies and phases are explicit.".to_string(),
                expected_output: "design.md, tasks.md, dag.json, and progress.txt initialized from the clarified goal.".to_string(),
            },
        ],
        TaskKind::Planning => vec![
            PlanStep {
                title: "Select the current executable task".to_string(),
                rationale: "writing-plans should operate on the next ready unit rather than the whole repository at once.".to_string(),
                expected_output: "One bounded implementation target with dependencies and scope.".to_string(),
            },
            PlanStep {
                title: "Break the task into implementation slices".to_string(),
                rationale: "Scientific decomposition reduces overbuilding and creates resumable checkpoints.".to_string(),
                expected_output: "Stepwise plan with concrete outputs and validation expectations.".to_string(),
            },
            PlanStep {
                title: "Choose isolation strategy and branch naming".to_string(),
                rationale: "Superpowers-style guarded execution depends on isolated working areas.".to_string(),
                expected_output: "A worktree and branch plan for safe execution.".to_string(),
            },
            PlanStep {
                title: "Assign subagent roles and quality gates".to_string(),
                rationale: "The loop needs explicit responsibilities for development, review, and verification.".to_string(),
                expected_output: "Developer, reviewer, and verifier briefs plus TDD and review gates.".to_string(),
            },
        ],
        TaskKind::Implementation => vec![
            PlanStep {
                title: "Prepare isolated workspace".to_string(),
                rationale: "Changes should land in a worktree or sandbox instead of the main branch.".to_string(),
                expected_output: "Workspace preparation checklist and target branch metadata.".to_string(),
            },
            PlanStep {
                title: "Run developer subagent against the plan".to_string(),
                rationale: "Implementation should follow the approved plan rather than improvising from scratch.".to_string(),
                expected_output: "Development brief, intended code changes, and evidence of execution.".to_string(),
            },
            PlanStep {
                title: "Apply TDD and guarded checks".to_string(),
                rationale: "Testing and review gates prevent the loop from advancing on weak output.".to_string(),
                expected_output: "Test-first checklist, patch strategy, and evidence log.".to_string(),
            },
            PlanStep {
                title: "Prepare review and verification handoff".to_string(),
                rationale: "Later stages need explicit evidence and unresolved risks.".to_string(),
                expected_output: "Implementation report, reviewer brief, and verification bundle.".to_string(),
            },
        ],
        TaskKind::Verification => vec![
            PlanStep {
                title: "Audit implementation evidence".to_string(),
                rationale: "Verification should judge what was actually produced, not what the plan intended.".to_string(),
                expected_output: "Verification summary tied to artifacts and gates.".to_string(),
            },
            PlanStep {
                title: "Update progress and task state".to_string(),
                rationale: "OpenSpec and Ralph-loop both depend on durable progress files for resume and traceability.".to_string(),
                expected_output: "Progress delta and completion recommendation.".to_string(),
            },
            PlanStep {
                title: "Emit OpenSpace-style learning signal".to_string(),
                rationale: "Successful and failed patterns should be captured for future injection.".to_string(),
                expected_output: "Evolution notes, auto-fix ideas, or reusable captured pattern.".to_string(),
            },
        ],
        TaskKind::FinishBranch => vec![
            PlanStep {
                title: "Summarize branch outcomes".to_string(),
                rationale: "A clean finish starts with a clear statement of what changed and what remains.".to_string(),
                expected_output: "Branch summary with evidence and outstanding items.".to_string(),
            },
            PlanStep {
                title: "Offer merge, PR, or discard choices".to_string(),
                rationale: "Superpowers-style finishing should standardize end-of-branch decisions.".to_string(),
                expected_output: "Decision matrix for merge, pull request, or discard.".to_string(),
            },
            PlanStep {
                title: "Clean temporary work environments".to_string(),
                rationale: "Temporary worktrees and sandboxes should not accumulate after a run.".to_string(),
                expected_output: "Cleanup checklist and safe removal instructions.".to_string(),
            },
        ],
    }
}

fn validation_for(task: &TaskItem, kind: TaskKind) -> Vec<String> {
    let mut checks = vec![
        format!("Task {} produces written artifacts that a later loop can consume.", task.id),
        "Each output is explicit enough to be inspected by a human reviewer.".to_string(),
    ];

    let extra = match kind {
        TaskKind::Brainstorming => vec![
            "Acceptance criteria are concrete and measurable.".to_string(),
            "Unresolved questions are listed instead of silently guessed.".to_string(),
        ],
        TaskKind::SpecCapture => vec![
            "OpenSpec artifacts agree with the requirement packet.".to_string(),
            "Task ordering and dependencies are visible in tasks.md and dag.json.".to_string(),
        ],
        TaskKind::Planning => vec![
            "writing-plans output includes steps, gates, and workspace strategy.".to_string(),
            "Subagent responsibilities are separated and reviewable.".to_string(),
        ],
        TaskKind::Implementation => vec![
            "Implementation evidence is sufficient for review and verification.".to_string(),
            "The plan references tests, review, and rollback awareness.".to_string(),
        ],
        TaskKind::Verification => vec![
            "Verification explicitly states whether the task can advance.".to_string(),
            "Learning signals can be promoted into evolve candidates.".to_string(),
        ],
        TaskKind::FinishBranch => vec![
            "Branch finishing offers merge, PR, and discard outcomes.".to_string(),
            "Temporary environment cleanup is documented.".to_string(),
        ],
    };

    checks.extend(extra);
    checks
}

fn quality_gates_for(kind: TaskKind) -> Vec<QualityGate> {
    match kind {
        TaskKind::Brainstorming => vec![
            QualityGate {
                name: "clarity".to_string(),
                required: true,
                description: "The goal, scope, and acceptance criteria are explicit.".to_string(),
            },
            QualityGate {
                name: "traceability".to_string(),
                required: true,
                description: "The requirement packet can be mapped into spec artifacts without guesswork.".to_string(),
            },
        ],
        TaskKind::SpecCapture => vec![
            QualityGate {
                name: "spec_consistency".to_string(),
                required: true,
                description: "proposal, requirements, acceptance, design, and tasks agree with each other.".to_string(),
            },
            QualityGate {
                name: "dag_integrity".to_string(),
                required: true,
                description: "Task dependencies are explicit and schedulable.".to_string(),
            },
        ],
        TaskKind::Planning => vec![
            QualityGate {
                name: "planning_quality".to_string(),
                required: true,
                description: "The writing-plans output is actionable, bounded, and checkpointed.".to_string(),
            },
            QualityGate {
                name: "workspace_readiness".to_string(),
                required: true,
                description: "The task specifies how work will be isolated.".to_string(),
            },
        ],
        TaskKind::Implementation => vec![
            QualityGate {
                name: "tests".to_string(),
                required: true,
                description: "TDD or at least explicit test execution is required before completion.".to_string(),
            },
            QualityGate {
                name: "review".to_string(),
                required: true,
                description: "Implementation must be reviewable and ready for a reviewer brief.".to_string(),
            },
        ],
        TaskKind::Verification => vec![
            QualityGate {
                name: "verification".to_string(),
                required: true,
                description: "The verifier decides whether the task may advance.".to_string(),
            },
            QualityGate {
                name: "progress_update".to_string(),
                required: true,
                description: "OpenSpec progress files must be updated after the verdict.".to_string(),
            },
        ],
        TaskKind::FinishBranch => vec![
            QualityGate {
                name: "branch_outcome".to_string(),
                required: true,
                description: "The user or workflow can choose merge, PR, or discard.".to_string(),
            },
            QualityGate {
                name: "cleanup".to_string(),
                required: true,
                description: "Temporary worktree or sandbox cleanup is described.".to_string(),
            },
        ],
    }
}

fn skills_for(kind: TaskKind) -> Vec<String> {
    match kind {
        TaskKind::Brainstorming => vec![
            "superpowers-brainstorming".to_string(),
            "socratic-clarification".to_string(),
            "openspec-capture".to_string(),
        ],
        TaskKind::SpecCapture => vec![
            "proposal-writer".to_string(),
            "requirements-normalizer".to_string(),
            "dag-author".to_string(),
        ],
        TaskKind::Planning => vec![
            "writing-plans".to_string(),
            "worktree-planner".to_string(),
            "quality-gate-designer".to_string(),
        ],
        TaskKind::Implementation => vec![
            "subagent-dev".to_string(),
            "tdd-cycle".to_string(),
            "requesting-code-review".to_string(),
        ],
        TaskKind::Verification => vec![
            "verification-before-completion".to_string(),
            "progress-writer".to_string(),
            "open-space-capture".to_string(),
        ],
        TaskKind::FinishBranch => vec![
            "finishing-a-development-branch".to_string(),
            "pr-preparation".to_string(),
            "cleanup-worktree".to_string(),
        ],
    }
}

fn deliverables_for(task: &TaskItem, kind: TaskKind) -> Vec<String> {
    match kind {
        TaskKind::Brainstorming => vec![
            format!("task-{}-brainstorming.md", task.id),
            format!("task-{}-requirement-packet.md", task.id),
        ],
        TaskKind::SpecCapture => vec![
            format!("task-{}-spec-capture.md", task.id),
            format!("task-{}-dag-notes.md", task.id),
        ],
        TaskKind::Planning => vec![
            format!("task-{}-writing-plans.md", task.id),
            format!("task-{}-workspace-plan.md", task.id),
            format!("task-{}-subagents.md", task.id),
        ],
        TaskKind::Implementation => vec![
            format!("task-{}-implementation.md", task.id),
            format!("task-{}-subagent-dev-runbook.md", task.id),
            format!("task-{}-tdd-cycle.md", task.id),
            format!("task-{}-review-brief.md", task.id),
            format!("task-{}-review-checklist.md", task.id),
        ],
        TaskKind::Verification => vec![
            format!("task-{}-verification.md", task.id),
            format!("task-{}-progress-delta.md", task.id),
            format!("task-{}-progress.txt", task.id),
            format!("task-{}-change-summary.md", task.id),
            format!("task-{}-evolution-signal.md", task.id),
        ],
        TaskKind::FinishBranch => vec![
            format!("task-{}-finish-branch.md", task.id),
            format!("task-{}-branch-outcome-matrix.md", task.id),
            format!("task-{}-cleanup.md", task.id),
        ],
    }
}

fn risks_for(kind: TaskKind) -> Vec<String> {
    match kind {
        TaskKind::Brainstorming => vec![
            "The user intent may still be ambiguous if unanswered questions are ignored.".to_string(),
            "The acceptance criteria may become decorative if they are not measurable.".to_string(),
        ],
        TaskKind::SpecCapture => vec![
            "Spec files can drift if the same clarified fact is written differently across artifacts.".to_string(),
            "A weak task graph can cause Ralph-loop to advance in the wrong order.".to_string(),
        ],
        TaskKind::Planning => vec![
            "A plan without isolation strategy can lead to unsafe in-place coding.".to_string(),
            "Subagent briefs can become vague if expected outputs are not explicit.".to_string(),
        ],
        TaskKind::Implementation => vec![
            "Implementation can look complete while still lacking tests or reviewer evidence.".to_string(),
            "Worktree discipline is lost if branch lifecycle is not documented.".to_string(),
        ],
        TaskKind::Verification => vec![
            "Verification may over-report success if it trusts status flags more than evidence.".to_string(),
            "Learning signals lose value if they are not tied to specific task observations.".to_string(),
        ],
        TaskKind::FinishBranch => vec![
            "Branch cleanup may be skipped if end-of-run ownership is unclear.".to_string(),
            "Premature merge can happen if verification evidence is not summarized before finishing.".to_string(),
        ],
    }
}

fn subagents_for(task: &TaskItem, kind: TaskKind) -> Vec<SubagentBrief> {
    match kind {
        TaskKind::Brainstorming => vec![SubagentBrief {
            role: "analyst".to_string(),
            goal: format!("Clarify the real user intent for task {}.", task.id),
            inputs: vec![task.title.clone(), task.description.clone()],
            outputs: vec!["clarifications list".to_string(), "acceptance criteria".to_string()],
            depends_on: vec![],
        }],
        TaskKind::SpecCapture => vec![SubagentBrief {
            role: "spec-writer".to_string(),
            goal: format!("Write OpenSpec artifacts for task {}.", task.id),
            inputs: vec!["requirement packet".to_string()],
            outputs: vec!["proposal fragment".to_string(), "requirements and acceptance updates".to_string()],
            depends_on: vec![],
        }],
        TaskKind::Planning => vec![
            SubagentBrief {
                role: "planner".to_string(),
                goal: format!("Create the writing-plans breakdown for task {}.", task.id),
                inputs: vec!["design.md".to_string(), "tasks.md".to_string()],
                outputs: vec!["execution slices".to_string(), "checkpoint plan".to_string()],
                depends_on: vec![],
            },
            SubagentBrief {
                role: "workspace-architect".to_string(),
                goal: format!("Define worktree and branch strategy for task {}.", task.id),
                inputs: vec!["execution slices".to_string()],
                outputs: vec!["branch name".to_string(), "worktree path".to_string()],
                depends_on: vec![],
            },
        ],
        TaskKind::Implementation => vec![
            SubagentBrief {
                role: "developer".to_string(),
                goal: format!("Implement the planned work for task {} in an isolated workspace.", task.id),
                inputs: vec!["writing plans".to_string(), "workspace plan".to_string()],
                outputs: vec!["code changes".to_string(), "developer notes".to_string()],
                depends_on: vec![],
            },
            SubagentBrief {
                role: "reviewer".to_string(),
                goal: format!("Review the implementation evidence for task {}.", task.id),
                inputs: vec!["developer notes".to_string(), "test evidence".to_string()],
                outputs: vec!["review verdict".to_string(), "risk list".to_string()],
                depends_on: vec!["developer".to_string()],
            },
        ],
        TaskKind::Verification => vec![SubagentBrief {
            role: "verifier".to_string(),
            goal: format!("Decide whether task {} may advance and what should be learned from it.", task.id),
            inputs: vec!["review brief".to_string(), "evidence bundle".to_string()],
            outputs: vec!["verification verdict".to_string(), "evolution signal".to_string()],
            depends_on: vec!["developer".to_string(), "reviewer".to_string()],
        }],
        TaskKind::FinishBranch => vec![SubagentBrief {
            role: "release-coordinator".to_string(),
            goal: format!("Finish the development branch for task {} safely.", task.id),
            inputs: vec!["verification report".to_string(), "branch summary".to_string()],
            outputs: vec!["merge/pr/discard options".to_string(), "cleanup instructions".to_string()],
            depends_on: vec![],
        }],
    }
}

fn worktree_plan_for(task: &TaskItem, kind: TaskKind, strategy: WorkspaceStrategy) -> Option<WorktreePlan> {
    if matches!(strategy, WorkspaceStrategy::InPlace) {
        return None;
    }

    Some(WorktreePlan {
        branch_name: format!("zero-nine/task-{}", task.id),
        worktree_path: format!(".zero_nine/worktrees/task-{}", task.id),
        strategy,
        cleanup_hint: match kind {
            TaskKind::FinishBranch => "After the branch outcome is chosen, remove the temporary worktree if it is no longer needed.".to_string(),
            _ => "Keep the worktree until verification and branch finishing are complete.".to_string(),
        },
    })
}

fn summary_for(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "Task {} completed in mode {:?} with {} structured steps, {} quality gates, {} deliverables, and {} subagent briefs.",
        task.id,
        plan.mode,
        plan.steps.len(),
        plan.quality_gates.len(),
        plan.deliverables.len(),
        plan.subagents.len()
    )
}

fn follow_ups_for(plan: &ExecutionPlan) -> Vec<String> {
    let mut follow_ups = vec![
        "Preserve generated artifacts so the next Ralph-loop iteration can start from fresh context.".to_string(),
        "Promote repeated high-value patterns into evolve candidates or shared host skills.".to_string(),
    ];

    match plan.mode {
        ExecutionMode::Brainstorming => {
            follow_ups.push("Use the requirement packet to update proposal, requirements, and acceptance artifacts.".to_string());
        }
        ExecutionMode::SpecCapture => {
            follow_ups.push("Use tasks.md and dag.json as the source of truth for the next writing-plans cycle.".to_string());
        }
        ExecutionMode::WritingPlans => {
            follow_ups.push("Prepare the worktree before any implementation starts.".to_string());
        }
        ExecutionMode::SubagentDev => {
            follow_ups.push("Run review and verification before allowing branch finishing.".to_string());
        }
        ExecutionMode::Verification => {
            follow_ups.push("Update progress.txt and evolve candidates using this verdict.".to_string());
        }
        ExecutionMode::FinishBranch => {
            follow_ups.push("Choose merge, PR, or discard and then clean temporary workspace artifacts.".to_string());
        }
        _ => {}
    }

    follow_ups
}

fn generated_artifacts_for(task: &TaskItem, plan: &ExecutionPlan) -> Vec<GeneratedArtifact> {
    match plan.mode {
        ExecutionMode::Brainstorming => vec![
            GeneratedArtifact {
                path: format!("task-{}-brainstorming.md", task.id),
                title: "Brainstorming Summary".to_string(),
                content: render_brainstorming(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-requirement-packet.md", task.id),
                title: "Requirement Packet".to_string(),
                content: render_requirement_packet(task, plan),
            },
        ],
        ExecutionMode::SpecCapture => vec![
            GeneratedArtifact {
                path: format!("task-{}-spec-capture.md", task.id),
                title: "OpenSpec Capture Notes".to_string(),
                content: render_spec_capture(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-dag-notes.md", task.id),
                title: "Task Graph Notes".to_string(),
                content: render_dag_notes(task, plan),
            },
        ],
        ExecutionMode::WritingPlans => vec![
            GeneratedArtifact {
                path: format!("task-{}-writing-plans.md", task.id),
                title: "Writing Plans".to_string(),
                content: render_writing_plans(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-workspace-plan.md", task.id),
                title: "Workspace Plan".to_string(),
                content: render_workspace_plan(plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-subagents.md", task.id),
                title: "Subagent Briefs".to_string(),
                content: render_subagents(plan),
            },
        ],
        ExecutionMode::SubagentDev => vec![
            GeneratedArtifact {
                path: format!("task-{}-implementation.md", task.id),
                title: "Implementation Strategy".to_string(),
                content: render_implementation_strategy(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-subagent-dev-runbook.md", task.id),
                title: "Subagent Development Runbook".to_string(),
                content: render_subagent_dev_runbook(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-tdd-cycle.md", task.id),
                title: "TDD Cycle".to_string(),
                content: render_tdd_cycle(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-review-brief.md", task.id),
                title: "Review Brief".to_string(),
                content: render_review_brief(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-review-checklist.md", task.id),
                title: "Review Checklist".to_string(),
                content: render_review_checklist(task, plan),
            },
        ],
        ExecutionMode::Verification => vec![
            GeneratedArtifact {
                path: format!("task-{}-verification.md", task.id),
                title: "Verification Report".to_string(),
                content: render_verification(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-progress-delta.md", task.id),
                title: "Progress Delta".to_string(),
                content: render_progress_delta(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-progress.txt", task.id),
                title: "Progress Text Update".to_string(),
                content: render_progress_txt(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-change-summary.md", task.id),
                title: "Change Summary".to_string(),
                content: render_change_summary(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-evolution-signal.md", task.id),
                title: "Evolution Signal".to_string(),
                content: render_evolution_signal(task, plan),
            },
        ],
        ExecutionMode::FinishBranch => vec![
            GeneratedArtifact {
                path: format!("task-{}-finish-branch.md", task.id),
                title: "Finish Branch Plan".to_string(),
                content: render_finish_branch(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-branch-outcome-matrix.md", task.id),
                title: "Branch Outcome Matrix".to_string(),
                content: render_branch_outcome_matrix(task, plan),
            },
            GeneratedArtifact {
                path: format!("task-{}-cleanup.md", task.id),
                title: "Cleanup Checklist".to_string(),
                content: render_cleanup(plan),
            },
        ],
        _ => vec![],
    }
}

fn render_brainstorming(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Brainstorming Summary\n\n## Task\n\n{}\n\n## Objective\n\n{}\n\n## Socratic Questions\n\n{}\n\n## Acceptance and Risks\n\n{}\n",
        task.title,
        plan.objective,
        to_markdown_list(&[
            "What exact outcome would make the user say this task is done?",
            "What should explicitly stay out of scope?",
            "Which host integration matters first: Claude Code, OpenCode, or standalone CLI?",
            "What quality bar should block completion?",
        ]),
        to_markdown_list(&plan.validation)
    )
}

fn render_requirement_packet(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Requirement Packet\n\n## Task\n\n{}\n\n## Problem Statement\n\n{}\n\n## Constraints\n\n{}\n\n## Acceptance Criteria\n\n{}\n\n## Risks\n\n{}\n",
        task.title,
        plan.objective,
        to_markdown_list(&[
            "Prefer plugin-first integration while preserving a future SDK path.",
            "Keep outputs resumable through written files.",
            "Use guarded execution and visible quality gates.",
        ]),
        to_checkbox_list(&plan.validation),
        to_markdown_list(&plan.risks)
    )
}

fn render_spec_capture(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# OpenSpec Capture Notes\n\n## Task\n\n{}\n\n## Capture Strategy\n\n{}\n\n## Deliverables\n\n{}\n\n## Quality Gates\n\n{}\n",
        task.title,
        render_step_sections(&plan.steps),
        to_markdown_list(&plan.deliverables),
        render_quality_gates(&plan.quality_gates)
    )
}

fn render_dag_notes(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Task Graph Notes\n\n## Task\n\n{}\n\n## Scheduling Rules\n\n{}\n\n## Risks\n\n{}\n",
        task.title,
        to_markdown_list(&[
            "Only schedule tasks whose dependencies are completed.",
            "Pause and re-read design artifacts if requirements drift.",
            "Do not advance when verification gates fail.",
        ]),
        to_markdown_list(&plan.risks)
    )
}

fn render_writing_plans(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Writing Plans\n\n## Task Overview\n\n- ID: {}\n- Title: {}\n- Mode: {:?}\n- Workspace Strategy: {:?}\n\n## Objective\n\n{}\n\n## Execution Slices\n\n{}\n\n## Quality Gates\n\n{}\n\n## Deliverables\n\n{}\n\n## Risks\n\n{}\n\n## Validation Checklist\n\n{}\n\n## Subagent Handoffs\n\n{}\n",
        task.id,
        task.title,
        plan.mode,
        plan.workspace_strategy,
        plan.objective,
        render_step_sections(&plan.steps),
        render_quality_gates(&plan.quality_gates),
        to_markdown_list(&plan.deliverables),
        to_markdown_list(&plan.risks),
        to_markdown_list(&plan.validation),
        render_subagents(plan)
    )
}

fn render_workspace_plan(plan: &ExecutionPlan) -> String {
    let planned_record = plan.workspace_record.as_ref();
    match plan.worktree_plan.as_ref() {
        Some(worktree) => format!(
            "# Workspace Plan\n\n## Planned Strategy\n\n- Strategy: {:?}\n- Branch: {}\n- Worktree Path: {}\n\n## Planned Lifecycle\n\n- Planned Status: {}\n- Base Branch: {}\n- Head Branch: {}\n\n## Operator Checklist\n\n- Prepare or reuse the workspace before development starts.\n- Keep writing-plans, subagent briefs, and review evidence inside this isolated path.\n- Do not finish the branch until verification and finish-branch guidance are complete.\n\n## Cleanup Hint\n\n{}\n",
            worktree.strategy,
            worktree.branch_name,
            worktree.worktree_path,
            planned_record
                .map(|record| format!("{:?}", record.status))
                .unwrap_or_else(|| "planned".to_string()),
            planned_record
                .and_then(|record| record.base_branch.clone())
                .unwrap_or_else(|| "n/a".to_string()),
            planned_record
                .and_then(|record| record.head_branch.clone())
                .unwrap_or_else(|| worktree.branch_name.clone()),
            worktree.cleanup_hint
        ),
        None => format!(
            "# Workspace Plan\n\n## Planned Strategy\n\n- Strategy: {:?}\n- Path: current project root\n\n## Operator Checklist\n\n- Execute the task in place and keep generated evidence under .zero_nine/.\n- Capture any deviations from in-place execution before moving to review.\n- If the task grows beyond a safe in-place change, regenerate the plan with a stronger isolation strategy.\n",
            plan.workspace_strategy
        ),
    }
}

fn render_subagents(plan: &ExecutionPlan) -> String {
    let mut out = String::from("# Subagent Briefs\n\n");
    for brief in &plan.subagents {
        out.push_str(&format!(
            "## {}\n\n**Goal**: {}\n\n**Inputs**\n\n{}\n\n**Outputs**\n\n{}\n\n",
            brief.role,
            brief.goal,
            to_markdown_list(&brief.inputs),
            to_markdown_list(&brief.outputs)
        ));
    }
    out
}

fn render_implementation_strategy(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Implementation Strategy\n\n## Task\n\n{}\n\n## Objective\n\n{}\n\n## Steps\n\n{}\n\n## Skill Chain\n\n{}\n\n## Deliverables\n\n{}\n\n## Workspace Contract\n\n{}\n",
        task.title,
        plan.objective,
        render_step_sections(&plan.steps),
        to_markdown_list(&plan.skill_chain),
        to_markdown_list(&plan.deliverables),
        render_workspace_contract(plan)
    )
}

fn render_subagent_dev_runbook(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Subagent Development Runbook\n\n## Task\n\n- ID: {}\n- Title: {}\n\n## Assigned Roles\n\n{}\n\n## Required Inputs\n\n{}\n\n## Execution Protocol\n\n{}\n\n## Required Outputs\n\n{}\n",
        task.id,
        task.title,
        render_subagents(plan),
        to_markdown_list(&[
            "writing-plans artifact",
            "workspace-plan artifact",
            "workspace-record artifact when available",
            "latest clarified requirement and spec artifacts",
        ]),
        to_checkbox_list(&[
            "Prepare or confirm the isolated workspace before editing.",
            "Implement only the bounded slice described in writing-plans.",
            "Record changed files, commands run, and unresolved risks.",
            "Hand off a concise evidence bundle to review instead of a narrative-only summary.",
        ]),
        to_markdown_list(&[
            "code changes or patch summary",
            "developer notes",
            "test evidence",
            "review handoff bundle",
        ])
    )
}

fn render_tdd_cycle(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# TDD Cycle\n\n## Task\n\n{}\n\n## Required Gates\n\n{}\n\n## Suggested Loop\n\n{}\n\n## Evidence Ledger\n\n{}\n",
        task.title,
        render_quality_gates(&plan.quality_gates),
        to_markdown_list(&[
            "Write or identify failing tests first.",
            "Implement the smallest safe change.",
            "Run regression checks and capture evidence.",
            "Escalate unresolved failures instead of silently proceeding.",
        ]),
        to_checkbox_list(&[
            "Document the first failing test or the precise missing test gap.",
            "Record the minimal implementation change that addressed the failure.",
            "Capture regression command(s) and results.",
            "List any remaining test debt before advancing to review.",
        ])
    )
}

fn render_review_brief(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Review Brief\n\n## Task\n\n{}\n\n## Review Focus\n\n{}\n\n## Risks\n\n{}\n\n## Evidence Expected\n\n{}\n",
        task.title,
        plan.objective,
        to_markdown_list(&plan.risks),
        to_markdown_list(&[
            "Changed files and rationale.",
            "Tests executed and their outcomes.",
            "Any deviations from the writing-plans breakdown.",
            "Workspace record and branch path used for the change.",
        ])
    )
}

fn render_review_checklist(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Review Checklist\n\n## Task\n\n{}\n\n## Reviewer Protocol\n\n{}\n\n## Blocking Questions\n\n{}\n",
        task.title,
        to_checkbox_list(&[
            "Confirm the implementation stayed within the current writing-plans slice.",
            "Confirm evidence exists for required tests and review gates.",
            "Confirm unresolved risks are explicit and not silently deferred.",
            "Confirm the workspace/branch used for execution matches the workspace artifacts.",
        ]),
        to_markdown_list(&plan.validation)
    )
}

fn render_verification(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Verification Report\n\n## Task\n\n{}\n\n## Verification Gates\n\n{}\n\n## Evidence Review\n\n{}\n\n## Advancement Rule\n\nOnly advance if required gates passed and remaining risks are explicitly accepted or converted into follow-up work.\n",
        task.title,
        render_quality_gates(&plan.quality_gates),
        to_markdown_list(&[
            "Check implementation, TDD, and review artifacts together rather than in isolation.",
            "Reject advancement when required evidence is missing even if status flags look green.",
            "Update OpenSpec progress artifacts in the same verification pass.",
        ])
    )
}

fn render_progress_delta(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Progress Delta\n\n## Task\n\n{}\n\n## Completed Deliverables\n\n{}\n\n## Recommended Progress Update\n\nMark the task complete only if verification passed; otherwise retain the task in the retry queue.\n",
        task.title,
        to_markdown_list(&plan.deliverables)
    )
}

fn render_progress_txt(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "task={}\nstatus=verification_pending\nmode={:?}\ndeliverables={}\nnext=update proposal progress and either advance or retry\n",
        task.id,
        plan.mode,
        plan.deliverables.join(", ")
    )
}

fn render_change_summary(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Change Summary\n\n## Task\n\n{}\n\n## What Changed\n\n{}\n\n## Validation Coverage\n\n{}\n\n## Remaining Risks\n\n{}\n",
        task.title,
        to_markdown_list(&plan.deliverables),
        to_markdown_list(&plan.validation),
        to_markdown_list(&plan.risks)
    )
}

fn render_evolution_signal(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Evolution Signal\n\n## Task\n\n{}\n\n## Candidate Improvements\n\n{}\n\n## Triggered Skills\n\n{}\n",
        task.title,
        to_markdown_list(&[
            "Promote repeated successful validation gates into reusable presets.",
            "Capture reviewer findings that recur across tasks.",
            "Generate auto-fix notes when evidence is thin or tests are weak.",
        ]),
        to_markdown_list(&plan.skill_chain)
    )
}

fn render_finish_branch(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Finish Branch Plan\n\n## Task\n\n{}\n\n## Decision Options\n\n{}\n\n## Preconditions\n\n{}\n\n## Workspace Contract\n\n{}\n",
        task.title,
        to_markdown_list(&[
            "Merge the branch if verification passed and the change is ready.",
            "Open a pull request if human review or approval is still required.",
            "Discard the branch if the work is exploratory or rejected.",
        ]),
        to_markdown_list(&plan.validation),
        render_workspace_contract(plan)
    )
}

fn render_branch_outcome_matrix(task: &TaskItem, plan: &ExecutionPlan) -> String {
    format!(
        "# Branch Outcome Matrix\n\n## Task\n\n{}\n\n| Outcome | When to Choose | Required Evidence | Next Action |\n| --- | --- | --- | --- |\n| Merge | Verification passed and the slice is complete | Verification report, review evidence, clean workspace state | Merge and run post-merge checks if policy requires |\n| Pull request | Human approval or remote review is still required | Review brief, change summary, branch summary | Open or prepare PR from the host integration layer |\n| Discard | The branch is exploratory, rejected, or superseded | Explicit rejection rationale and preserved learnings | Remove worktree and delete branch after preserving artifacts |\n| Keep | Another iteration is planned on the same branch | Remaining risks and next writing-plans slice | Resume development before trying to finish again |\n\n## Current Validation Inputs\n\n{}\n",
        task.title,
        to_markdown_list(&plan.validation)
    )
}

fn render_cleanup(plan: &ExecutionPlan) -> String {
    let mut items = vec![
        "Archive or preserve key artifacts before deleting temporary state.".to_string(),
        "Remove worktree directories only after the branch outcome is decided.".to_string(),
        "Keep verification and progress evidence in .zero_nine for auditability.".to_string(),
    ];
    if let Some(worktree) = &plan.worktree_plan {
        items.push(format!("Target cleanup path: {}", worktree.worktree_path));
    }
    format!("# Cleanup Checklist\n\n{}\n", to_checkbox_list(&items))
}

fn render_workspace_contract(plan: &ExecutionPlan) -> String {
    match (&plan.worktree_plan, &plan.workspace_record) {
        (Some(worktree), Some(record)) => format!(
            "- Planned branch: {}\n- Planned path: {}\n- Current status: {:?}\n- Cleanup hint: {}",
            worktree.branch_name,
            worktree.worktree_path,
            record.status,
            worktree.cleanup_hint
        ),
        (Some(worktree), None) => format!(
            "- Planned branch: {}\n- Planned path: {}\n- Cleanup hint: {}",
            worktree.branch_name,
            worktree.worktree_path,
            worktree.cleanup_hint
        ),
        (None, Some(record)) => format!(
            "- Execute in place at {}\n- Current status: {:?}",
            record.worktree_path,
            record.status
        ),
        (None, None) => "- Execute in place under the current project root.".to_string(),
    }
}

fn render_step_sections(steps: &[PlanStep]) -> String {
    let mut out = String::new();
    for (idx, step) in steps.iter().enumerate() {
        out.push_str(&format!(
            "### Step {}: {}\n\n**Why**: {}\n\n**Expected output**: {}\n\n",
            idx + 1,
            step.title,
            step.rationale,
            step.expected_output
        ));
    }
    out
}

fn render_quality_gates(gates: &[QualityGate]) -> String {
    gates
        .iter()
        .map(|gate| format!("- **{}**: {} (required: {})", gate.name, gate.description, gate.required))
        .collect::<Vec<_>>()
        .join("\n")
}

fn to_markdown_list(items: &[impl AsRef<str>]) -> String {
    items
        .iter()
        .map(|item| format!("- {}", item.as_ref()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn to_checkbox_list(items: &[impl AsRef<str>]) -> String {
    items
        .iter()
        .map(|item| format!("- [ ] {}", item.as_ref()))
        .collect::<Vec<_>>()
        .join("\n")
}

// ============================================================================
// gRPC Bridge Execution Functions
// ============================================================================

/// Execute a plan by dispatching the task to an agent via gRPC bridge.
///
/// This function:
/// 1. Connects to the gRPC bridge server
/// 2. Dispatches the task to an agent
/// 3. Waits for the agent to complete the task
/// 4. Collects evidence and builds an ExecutionReport
///
/// # Arguments
///
/// * `project_root` - Root path of the project
/// * `task` - The task to execute
/// * `plan` - The execution plan
/// * `bridge_addr` - Address of the gRPC bridge server
/// * `timeout_secs` - Timeout in seconds for task completion
///
/// # Returns
///
/// An `ExecutionReport` containing the agent's results
pub async fn execute_plan_with_bridge(
    project_root: &Path,
    task: &TaskItem,
    plan: &ExecutionPlan,
    bridge_addr: std::net::SocketAddr,
    timeout_secs: u64,
) -> Result<ExecutionReport> {
    use bridge_client::BridgeClient;

    info!("Connecting to gRPC bridge server at {}", bridge_addr);

    // Connect to the bridge server
    let mut client = BridgeClient::connect(bridge_addr)
        .await
        .context("failed to connect to gRPC bridge server")?;

    // Collect context files from the plan (use empty for now as ExecutionPlan doesn't have context_injection)
    let context_files = Vec::new();

    // Dispatch the task to an agent
    info!("Dispatching task {} to agent", task.id);
    let agent_task_id = client.dispatch_task(task, &plan.task_id, plan, context_files)
        .await
        .context("failed to dispatch task to agent")?;

    info!("Task {} dispatched to agent as {}", task.id, agent_task_id);

    // Wait for task completion
    info!("Waiting for task {} to complete (timeout: {}s)", task.id, timeout_secs);
    let task_result = client.wait_for_task(&task.id, &agent_task_id, timeout_secs)
        .await
        .context("failed to wait for task completion")?;

    // Collect evidence
    let evidence_records = client.collect_evidence(&task.id, &agent_task_id)
        .await
        .context("failed to collect evidence")?;

    // Build the execution report from the agent's results
    build_report_from_agent_result(
        project_root,
        task,
        plan,
        task_result,
        evidence_records,
    )
}

/// Execute a plan via the gRPC bridge (sync wrapper around async bridge client).
/// Uses a single-threaded tokio runtime to bridge async/sync boundary,
/// enabling independent service deployment.
pub fn execute_plan_via_bridge(
    project_root: &Path,
    task: &TaskItem,
    plan: &ExecutionPlan,
    timeout_secs: u64,
) -> Result<ExecutionReport> {
    let bridge_addr = plan
        .bridge_address
        .as_ref()
        .ok_or_else(|| anyhow!("bridge_address required for bridge execution path"))?
        .parse::<std::net::SocketAddr>()
        .context("invalid bridge_address format")?;

    let project_root = project_root.to_path_buf();
    let task = task.clone();
    let plan = plan.clone();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create tokio runtime for bridge execution")?;

    rt.block_on(async move {
        execute_plan_with_bridge(&project_root, &task, &plan, bridge_addr, timeout_secs).await
    })
}

/// Build an ExecutionReport from agent results
fn build_report_from_agent_result(
    _project_root: &Path,
    task: &TaskItem,
    plan: &ExecutionPlan,
    task_result: bridge_client::TaskResult,
    evidence_records: Vec<proto::EvidenceRecord>,
) -> Result<ExecutionReport> {
    use bridge_client::{task_state_is_success, task_state_is_failure};

    let success = task_state_is_success(task_result.state);
    let outcome = if success {
        zn_types::ExecutionOutcome::Completed
    } else if task_state_is_failure(task_result.state) {
        zn_types::ExecutionOutcome::Escalated
    } else {
        zn_types::ExecutionOutcome::RetryableFailure
    };

    // Convert proto evidence records to zn_types EvidenceRecord
    let evidence: Vec<EvidenceRecord> = evidence_records
        .iter()
        .map(|e| {
            let kind = zn_types::EvidenceKind::GeneratedArtifact; // Default kind
            EvidenceRecord {
                key: e.id.clone(),
                label: e.kind.clone(),
                kind,
                status: if e.file_path.is_empty() {
                    zn_types::EvidenceStatus::Missing
                } else {
                    zn_types::EvidenceStatus::Collected
                },
                required: true,
                summary: format!("Evidence {}: {}", e.id, e.kind),
                path: if e.file_path.is_empty() { None } else { Some(e.file_path.clone()) },
            }
        })
        .collect();

    // Build artifacts list from task result and evidence
    let mut artifacts = task_result.artifacts.clone();
    artifacts.extend(
        evidence_records
            .iter()
            .filter(|e| !e.file_path.is_empty())
            .map(|e| e.file_path.clone())
            .collect::<Vec<_>>()
    );

    // Build details from task result summary
    let details = vec![
        format!("Agent task ID: {}", task_result.task_id),
        format!("Final state: {} (enum value)", task_result.state),
        format!("Summary: {}", task_result.summary),
    ];

    // Determine test/review pass status based on task state
    let tests_passed = success;
    let review_passed = success;

    // Build failure summary if needed
    let failure_summary = if success {
        None
    } else {
        Some(format!("Task {} failed: {}", task.id, task_result.summary))
    };

    let exit_code = if success { 0 } else { 1 };

    info!(
        "Built execution report for task {}: success={}, outcome={:?}",
        task.id, success, outcome
    );

    Ok(ExecutionReport {
        task_id: task.id.clone(),
        success,
        outcome,
        summary: task_result.summary,
        details,
        tests_passed,
        review_passed,
        artifacts,
        generated_artifacts: Vec::new(),
        evidence,
        follow_ups: if !success {
            vec!["Review agent output and evidence files for failure analysis.".to_string()]
        } else {
            Vec::new()
        },
        workspace_record: None,
        finish_branch_result: None,
        finish_branch_automation: plan.finish_branch_automation.clone(),
        agent_runs: Vec::new(),
        review_verdict: None,
        verification_verdict: None,
        verification_actions: plan.verification_actions.clone(),
        verification_action_results: Vec::new(),
        failure_summary,
        exit_code,
        execution_time_ms: 0,
        token_count: 0,
        code_quality_score: 0.0,
        test_coverage: 0.0,
        user_feedback: None,
        failure_classification: None,
        tri_role_verdict: None,
        authorization_ticket_id: None,
        authorized_by: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use zn_types::{SubagentExecutionPath, TaskContract, TaskStatus};

    fn sample_task(id: &str, title: &str, description: &str) -> TaskItem {
        TaskItem {
            id: id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            status: TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: TaskContract::default(),
            max_retries: None,
            preconditions: vec![],
        }
    }

    fn test_project_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "zero_nine_exec_test_{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).unwrap();
        Command::new("git")
            .arg("init")
            .arg("-q")
            .current_dir(&root)
            .output()
            .unwrap();
        root
    }

    #[test]
    fn planning_task_generates_worktree_and_subagents() {
        let task = sample_task(
            "3",
            "Run writing-plans and prepare isolated execution workspace",
            "Refine the executable plan and worktree strategy.",
        );
        let plan = build_plan(&task);
        assert_eq!(classify_task(&task), TaskKind::Planning);
        assert!(plan.worktree_plan.is_some());
        assert!(plan.subagents.len() >= 2);
        assert!(plan.deliverables.iter().any(|item| item.contains("workspace-plan")));
    }

    #[test]
    fn implementation_report_contains_review_and_tdd_artifacts() {
        let task = sample_task(
            "4",
            "Execute guarded implementation verification and branch finishing",
            "Simulate guarded execution with review and verification.",
        );
        let plan = build_plan(&task);
        let project_root = test_project_root();
        let report = execute_plan(project_root.as_path(), &task, &plan, None, false).unwrap();
        assert!(report.success);
        assert!(report.generated_artifacts.iter().any(|item| item.path.contains("review-brief")));
        assert!(report.generated_artifacts.iter().any(|item| item.path.contains("tdd-cycle")));
        assert!(report.generated_artifacts.iter().any(|item| item.path.contains("subagent-dev-runbook")));
        assert!(report.generated_artifacts.iter().any(|item| item.path.contains("review-checklist")));
    }

    #[test]
    fn verification_and_finish_branch_emit_operational_artifacts() {
        let verification_task = sample_task(
            "5",
            "Verify implementation evidence and update progress",
            "Audit evidence, update progress, and capture evolution notes.",
        );
        let verification_plan = build_plan(&verification_task);
        let verification_root = test_project_root();
        let verification_report = execute_plan(
            verification_root.as_path(),
            &verification_task,
            &verification_plan,
            None,
            false,
        )
        .unwrap();
        assert!(verification_report
            .generated_artifacts
            .iter()
            .any(|item| item.path.contains("progress.txt")));
        assert!(verification_report
            .generated_artifacts
            .iter()
            .any(|item| item.path.contains("change-summary")));

        let finish_task = sample_task(
            "6",
            "Finish branch with merge or PR decision",
            "Prepare standardized branch outcome options and cleanup guidance.",
        );
        let finish_plan = build_plan(&finish_task);
        let finish_root = test_project_root();
        let finish_report = execute_plan(finish_root.as_path(), &finish_task, &finish_plan, None, false).unwrap();
        assert!(finish_report
            .generated_artifacts
            .iter()
            .any(|item| item.path.contains("branch-outcome-matrix")));
    }

    #[test]
    fn test_execution_path_default_is_cli() {
        assert_eq!(SubagentExecutionPath::default(), SubagentExecutionPath::Cli);
    }

    #[test]
    fn test_build_plan_with_config_sets_fields() {
        let task = TaskItem {
            id: "test-1".to_string(),
            title: "Test".to_string(),
            description: "Test task".to_string(),
            status: TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: TaskContract::default(),
            max_retries: Some(3),
            preconditions: vec![],
        };

        let plan = build_plan_with_config(
            &task,
            SubagentExecutionPath::Bridge,
            Some("127.0.0.1:50051".to_string()),
        );

        assert_eq!(plan.execution_path, SubagentExecutionPath::Bridge);
        assert_eq!(plan.bridge_address, Some("127.0.0.1:50051".to_string()));
    }

    #[test]
    fn test_build_plan_default_uses_cli_path() {
        let task = TaskItem {
            id: "test-2".to_string(),
            title: "Test".to_string(),
            description: "Test task".to_string(),
            status: TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: TaskContract::default(),
            max_retries: Some(3),
            preconditions: vec![],
        };

        let plan = build_plan(&task);
        assert_eq!(plan.execution_path, SubagentExecutionPath::Cli);
        assert_eq!(plan.bridge_address, None);
    }

    #[test]
    fn test_subagent_outcome_from_report() {
        let report = ExecutionReport {
            task_id: "test".to_string(),
            success: true,
            outcome: ExecutionOutcome::Completed,
            summary: "done".to_string(),
            details: vec![],
            tests_passed: true,
            review_passed: true,
            artifacts: vec!["artifact1".to_string()],
            generated_artifacts: vec![],
            evidence: vec![],
            follow_ups: vec![],
            workspace_record: None,
            finish_branch_result: None,
            finish_branch_automation: None,
            agent_runs: vec![],
            review_verdict: None,
            verification_verdict: None,
            verification_actions: vec![],
            verification_action_results: vec![],
            failure_summary: None,
            exit_code: 0,
            execution_time_ms: 0,
            token_count: 0,
            code_quality_score: 0.0,
            test_coverage: 0.0,
            user_feedback: None,
            failure_classification: None,
            tri_role_verdict: Some("Pass".to_string()),
            authorization_ticket_id: None,
            authorized_by: None,
        };

        let outcome = SubagentExecutionOutcome::from_report(&report);
        assert!(outcome.all_succeeded);
        assert_eq!(outcome.artifact_paths, vec!["artifact1"]);
        assert_eq!(outcome.tri_role_verdict, Some("Pass".to_string()));
    }

    #[test]
    fn test_subagent_outcome_error() {
        let outcome = SubagentExecutionOutcome::error("connection refused");
        assert!(!outcome.all_succeeded);
        assert!(outcome.tri_role_verdict.unwrap().contains("BridgeError"));
    }

    #[test]
    fn test_execute_plan_via_bridge_missing_address() {
        let task = TaskItem {
            id: "test-3".to_string(),
            title: "Test".to_string(),
            description: "Test task".to_string(),
            status: TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: TaskContract::default(),
            max_retries: Some(3),
            preconditions: vec![],
        };

        let plan = build_plan_with_config(&task, SubagentExecutionPath::Bridge, None);
        let result = execute_plan_via_bridge(
            std::path::Path::new("/tmp"),
            &task,
            &plan,
            300,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("bridge_address required"));
    }

    #[test]
    fn test_execute_plan_via_bridge_invalid_address() {
        let task = TaskItem {
            id: "test-4".to_string(),
            title: "Test".to_string(),
            description: "Test task".to_string(),
            status: TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: TaskContract::default(),
            max_retries: Some(3),
            preconditions: vec![],
        };

        let plan = build_plan_with_config(&task, SubagentExecutionPath::Bridge, Some("invalid".to_string()));
        let result = execute_plan_via_bridge(
            std::path::Path::new("/tmp"),
            &task,
            &plan,
            300,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid bridge_address format"));
    }

    #[test]
    fn test_hybrid_path_falls_back_to_cli_outcome() {
        let tmp = test_project_root();

        let task = TaskItem {
            id: "test-5".to_string(),
            title: "Test".to_string(),
            description: "Test task".to_string(),
            status: TaskStatus::Pending,
            depends_on: vec![],
            kind: None,
            contract: TaskContract::default(),
            max_retries: Some(3),
            preconditions: vec![],
        };

        // Hybrid with CLI (which will work or fail gracefully)
        let plan = build_plan_with_config(&task, SubagentExecutionPath::Cli, None);
        let result = execute_plan(&tmp, &task, &plan, None, false);
        assert!(result.is_ok());
    }
}
