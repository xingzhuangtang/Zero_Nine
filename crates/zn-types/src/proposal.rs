//! Proposal, task items, task graph, DAG validation, and related spec types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::*;
use crate::state_machine::*;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskContract {
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub deliverables: Vec<String>,
    #[serde(default)]
    pub verification_points: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub contract: TaskContract,
    #[serde(default)]
    pub max_retries: Option<u8>,
    #[serde(default)]
    pub preconditions: Vec<String>,
}

/// M1-1: Constraint 结构 (蓝图 M1-1)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Constraint {
    pub id: String,
    pub category: ConstraintCategory,
    pub description: String,
    pub rationale: Option<String>,
    #[serde(default)]
    pub enforced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintCategory {
    Technical,
    Business,
    Regulatory,
    Performance,
    Security,
    Timeline,
    Resource,
}

/// M1-3: AcceptanceCriterion 结构 (蓝图 M1-3)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcceptanceCriterion {
    pub id: String,
    pub description: String,
    pub verification_method: VerificationMethod,
    pub priority: Priority,
    #[serde(default)]
    pub status: CriterionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    AutomatedTest,
    ManualInspection,
    PerformanceBenchmark,
    SecurityAudit,
    UserAcceptance,
    DocumentationReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum CriterionStatus {
    #[default]
    Pending,
    Verified,
    Failed,
    Blocked,
}


/// M1-1: Risk 结构 (蓝图 M1-1)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Risk {
    pub id: String,
    pub description: String,
    pub probability: RiskProbability,
    pub impact: RiskImpact,
    pub mitigation: Option<String>,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskProbability {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskImpact {
    Low,
    Medium,
    High,
    Critical,
}

/// M1-1: Dependency 结构
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dependency {
    pub id: String,
    pub description: String,
    pub kind: DependencyKind,
    pub status: DependencyStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Internal,
    External,
    ThirdParty,
    Infrastructure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyStatus {
    Satisfied,
    Pending,
    Blocked,
    AtRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDependencyEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    #[serde(default = "default_spec_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub proposal_id: String,
    pub tasks: Vec<TaskItem>,
    #[serde(default)]
    pub edges: Vec<TaskDependencyEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    #[serde(default = "default_spec_schema_version")]
    pub schema_version: String,
    pub id: String,
    pub title: String,
    pub goal: String,
    pub status: ProposalStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub design_summary: Option<String>,
    #[serde(default)]
    pub source_brainstorm_session_id: Option<String>,

    // M6: Issue 来源追踪
    #[serde(default)]
    pub source_issue_number: Option<u64>,
    #[serde(default)]
    pub source_repo: Option<String>,
    #[serde(default)]
    pub source_type: Option<IssueSource>,

    // M1: Structured Spec Contract Fields (蓝图 M1-1)
    #[serde(default)]
    pub problem_statement: Option<String>,
    #[serde(default)]
    pub scope_in: Vec<String>,
    #[serde(default)]
    pub scope_out: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    #[serde(default)]
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    #[serde(default)]
    pub risks: Vec<Risk>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub non_goals: Vec<String>,
    #[serde(default)]
    pub execution_strategy: Option<ExecutionStrategy>,

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
    #[serde(default)]
    pub max_iterations: Option<u32>,
    #[serde(default)]
    pub iteration_start: DateTime<Utc>,
    #[serde(default)]
    pub elapsed_seconds: u64,
    /// M7: 状态转换历史
    #[serde(default)]
    pub transition_history: Vec<StateTransition>,
}

// ==================== Default Implementations ====================

impl Default for Constraint {
    fn default() -> Self {
        Self {
            id: String::new(),
            category: ConstraintCategory::Technical,
            description: String::new(),
            rationale: None,
            enforced: false,
        }
    }
}

impl Default for AcceptanceCriterion {
    fn default() -> Self {
        Self {
            id: String::new(),
            description: String::new(),
            verification_method: VerificationMethod::AutomatedTest,
            priority: Priority::Medium,
            status: CriterionStatus::Pending,
        }
    }
}

impl Default for Risk {
    fn default() -> Self {
        Self {
            id: String::new(),
            description: String::new(),
            probability: RiskProbability::Medium,
            impact: RiskImpact::Medium,
            mitigation: None,
            owner: None,
        }
    }
}

impl Default for Dependency {
    fn default() -> Self {
        Self {
            id: String::new(),
            description: String::new(),
            kind: DependencyKind::Internal,
            status: DependencyStatus::Pending,
        }
    }
}

impl Default for Proposal {
    fn default() -> Self {
        Self {
            schema_version: default_spec_schema_version(),
            id: String::new(),
            title: String::new(),
            goal: String::new(),
            status: ProposalStatus::Draft,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            design_summary: None,
            source_brainstorm_session_id: None,
            source_issue_number: None,
            source_repo: None,
            source_type: None,
            problem_statement: None,
            scope_in: Vec::new(),
            scope_out: Vec::new(),
            constraints: Vec::new(),
            acceptance_criteria: Vec::new(),
            risks: Vec::new(),
            dependencies: Vec::new(),
            non_goals: Vec::new(),
            execution_strategy: None,
            tasks: Vec::new(),
        }
    }
}

// ==================== Issue Source (used by Proposal) ====================

/// Issue 来源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueSource {
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "manual")]
    Manual,
}

// ==================== DAG Validation ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DagValidationError {
    pub error_code: DagErrorCode,
    pub message: String,
    pub involved_task_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DagErrorCode {
    CircularDependency,
    MissingDependency,
    SelfReference,
    DuplicateTaskId,
    EmptyTaskGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagValidationResult {
    pub valid: bool,
    pub errors: Vec<DagValidationError>,
    pub warnings: Vec<String>,
    pub critical_path: Vec<String>,
    pub max_depth: u32,
}

impl TaskGraph {
    /// M1-2: Validate DAG for cycles, missing dependencies, self-references
    pub fn validate_dag(&self) -> DagValidationResult {
        let mut errors = Vec::new();
        let warnings = Vec::new();

        if self.tasks.is_empty() {
            errors.push(DagValidationError {
                error_code: DagErrorCode::EmptyTaskGraph,
                message: "Task graph contains no tasks".to_string(),
                involved_task_ids: Vec::new(),
            });
            return DagValidationResult {
                valid: false,
                errors,
                warnings,
                critical_path: Vec::new(),
                max_depth: 0,
            };
        }

        let mut task_ids = std::collections::HashMap::new();
        for task in &self.tasks {
            if task_ids.contains_key(&task.id) {
                errors.push(DagValidationError {
                    error_code: DagErrorCode::DuplicateTaskId,
                    message: format!("Duplicate task ID: {}", task.id),
                    involved_task_ids: vec![task.id.clone()],
                });
            } else {
                task_ids.insert(task.id.clone(), task);
            }
        }

        for task in &self.tasks {
            for dep in &task.depends_on {
                if dep == &task.id {
                    errors.push(DagValidationError {
                        error_code: DagErrorCode::SelfReference,
                        message: format!("Task {} references itself", task.id),
                        involved_task_ids: vec![task.id.clone()],
                    });
                } else if !task_ids.contains_key(dep) {
                    errors.push(DagValidationError {
                        error_code: DagErrorCode::MissingDependency,
                        message: format!("Task {} depends on non-existent task {}", task.id, dep),
                        involved_task_ids: vec![task.id.clone(), dep.clone()],
                    });
                }
            }
        }

        if errors.is_empty() {
            let cycles = self.detect_cycles();
            if !cycles.is_empty() {
                let involved: Vec<String> = cycles.iter().flatten().cloned().collect();
                errors.push(DagValidationError {
                    error_code: DagErrorCode::CircularDependency,
                    message: format!("Circular dependency detected: {}", cycles[0].join(" -> ")),
                    involved_task_ids: involved,
                });
            }
        }

        let critical_path = self.compute_critical_path();
        let max_depth = self.compute_max_depth();

        DagValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
            critical_path,
            max_depth,
        }
    }

    fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut in_degree: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut adj: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for task in &self.tasks {
            in_degree.entry(task.id.clone()).or_insert(0);
            adj.entry(task.id.clone()).or_default();
        }

        for task in &self.tasks {
            for dep in &task.depends_on {
                if in_degree.contains_key(dep) {
                    adj.entry(dep.clone())
                        .or_default()
                        .push(task.id.clone());
                    *in_degree.entry(task.id.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut processed = 0;

        while let Some(node) = queue.pop() {
            processed += 1;
            if let Some(neighbors) = adj.get(&node) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push(neighbor.clone());
                        }
                    }
                }
            }
        }

        if processed != self.tasks.len() {
            let cycle_nodes: Vec<String> = in_degree
                .iter()
                .filter(|(_, &deg)| deg > 0)
                .map(|(id, _)| id.clone())
                .collect();

            if !cycle_nodes.is_empty() {
                return vec![cycle_nodes];
            }
        }

        Vec::new()
    }

    fn compute_critical_path(&self) -> Vec<String> {
        let mut dist: std::collections::HashMap<String, (u32, String)> =
            std::collections::HashMap::new();

        for task in &self.tasks {
            dist.insert(task.id.clone(), (0, String::new()));
        }

        let mut result = Vec::new();
        let mut in_degree: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for task in &self.tasks {
            in_degree.insert(task.id.clone(), task.depends_on.len());
        }

        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        while let Some(task_id) = queue.pop() {
            result.push(task_id.clone());
            if let Some(&(d, _)) = dist.get(&task_id) {
                for task in &self.tasks {
                    if task.depends_on.contains(&task_id) {
                        let new_dist = d + 1;
                        if let Some(entry) = dist.get_mut(&task.id) {
                            if new_dist > entry.0 {
                                *entry = (new_dist, task_id.clone());
                            }
                        }
                        if let Some(deg) = in_degree.get_mut(&task.id) {
                            *deg -= 1;
                            if *deg == 0 {
                                queue.push(task.id.clone());
                            }
                        }
                    }
                }
            }
        }

        if result.is_empty() {
            return Vec::new();
        }

        let mut max_task = result[0].clone();
        let mut max_dist = 0;
        for (task_id, &(d, _)) in &dist {
            if d > max_dist {
                max_dist = d;
                max_task = task_id.clone();
            }
        }

        let mut critical_path = Vec::new();
        let mut current = max_task;
        while !current.is_empty() {
            critical_path.push(current.clone());
            if let Some((_, prev)) = dist.get(&current) {
                current = prev.clone();
            } else {
                break;
            }
        }
        critical_path.reverse();
        critical_path
    }

    fn compute_max_depth(&self) -> u32 {
        if self.tasks.is_empty() {
            return 0;
        }

        let mut in_degree: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut depth: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

        for task in &self.tasks {
            in_degree.insert(task.id.clone(), task.depends_on.len());
            depth.insert(task.id.clone(), 1);
        }

        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut max_depth = 0;

        while let Some(task_id) = queue.pop() {
            let current_depth = depth.get(&task_id).copied().unwrap_or(1);
            max_depth = max_depth.max(current_depth);

            for task in &self.tasks {
                if task.depends_on.contains(&task_id) {
                    let new_depth = current_depth + 1;
                    if let Some(d) = depth.get_mut(&task.id) {
                        *d = (*d).max(new_depth);
                    }
                    if let Some(deg) = in_degree.get_mut(&task.id) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(task.id.clone());
                        }
                    }
                }
            }
        }

        max_depth
    }
}
