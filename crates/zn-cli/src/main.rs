use anyhow::{anyhow, Result};
use chrono::{Local, TimeZone};
use clap::{Parser, Subcommand};
use rustyline::DefaultEditor;
use std::path::PathBuf;
use zn_evolve::scorer::create_default_scorer;
use zn_host::detect_host;
use zn_loop::TerminalInput;
use zn_sdk::from_project;
use zn_spec::memory_tool::{
    create_default_manager as create_memory_manager, MemoryAction, MemoryTarget,
};
use zn_spec::session_search::create_default_searcher;
use zn_spec::skill_format::SkillSummary;
use zn_spec::skill_manager::create_default_manager;
use zn_types::HostKind;

mod tui_dashboard;

/// Rustyline-backed terminal input — implements TerminalInput for zn-loop.
struct RustylineInput {
    editor: DefaultEditor,
}

impl RustylineInput {
    fn new() -> Result<Self> {
        Ok(Self {
            editor: DefaultEditor::new()?,
        })
    }
}

impl TerminalInput for RustylineInput {
    fn readline(&mut self, prompt: &str) -> Result<String> {
        let answer = self.editor.readline(prompt)?;
        self.editor.add_history_entry(&answer)?;
        Ok(answer)
    }
}

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
    Brainstorm {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        goal: Option<String>,
        #[arg(long, default_value_t = false)]
        resume: bool,
    },
    Run {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        goal: String,
        #[arg(long, default_value_t = false)]
        confirm_remote_finish: bool,
        #[arg(long)]
        bridge_address: Option<String>,
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
        #[arg(long, default_value_t = false)]
        confirm_remote_finish: bool,
        #[arg(long)]
        bridge_address: Option<String>,
    },
    Export {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Skill management commands
    Skill {
        #[command(subcommand)]
        command: SkillCommands,
    },
    /// Memory management commands
    Memory {
        #[command(subcommand)]
        command: MemoryCommands,
    },
    /// MCP management commands
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
    /// Cron management commands
    Cron {
        #[command(subcommand)]
        command: CronCommands,
    },
    /// Subagent management commands
    Subagent {
        #[command(subcommand)]
        command: SubagentCommands,
    },
    /// Safety governance commands
    Governance {
        #[command(subcommand)]
        command: GovernanceCommands,
    },
    /// GitHub integration commands
    Github {
        #[command(subcommand)]
        command: GithubCommands,
    },
    /// TUI dashboard for lifecycle overview
    Dashboard {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Observability and tracing commands
    Observe {
        #[command(subcommand)]
        command: ObserveCommands,
    },
    /// Start gRPC bridge server for independent agent dispatch
    BridgeServer {
        #[arg(long, default_value_t = 50051)]
        port: u16,
    },
}

/// Observability and tracing commands
#[derive(Subcommand, Debug)]
enum ObserveCommands {
    /// Query events by type
    Events {
        #[arg(long)]
        event_type: String,
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Query events by proposal ID
    Proposal {
        #[arg(long)]
        proposal_id: String,
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Replay a trace by trace ID
    Trace {
        #[arg(long)]
        trace_id: String,
    },
    /// Show latency statistics
    Stats {
        #[arg(long)]
        task_id: Option<String>,
    },
    /// Show recent metrics
    Metrics {
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

impl ObserveCommands {}

#[derive(Subcommand, Debug)]
enum SkillCommands {
    /// Create a new skill
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: String,
        #[arg(long, default_value = "execution")]
        category: String,
        #[arg(long, default_value = "1.0.0")]
        version: String,
        #[arg(long)]
        content: Option<String>,
    },
    /// List all available skills
    List {
        #[arg(long, default_value = "false")]
        detailed: bool,
    },
    /// View a skill file
    View {
        #[arg(long)]
        name: String,
    },
    /// Patch an existing skill (replace text)
    Patch {
        #[arg(long)]
        name: String,
        #[arg(long)]
        old: String,
        #[arg(long)]
        new: String,
    },
    /// Edit an entire skill
    Edit {
        #[arg(long)]
        name: String,
        #[arg(long)]
        content: String,
    },
    /// Delete a skill
    Delete {
        #[arg(long)]
        name: String,
    },
    /// Validate a skill file
    Validate {
        #[arg(long)]
        name: String,
    },
    /// Get skill score
    Score {
        #[arg(long)]
        name: String,
    },
    /// List all skill scores
    Scores {
        #[arg(long, default_value = "false")]
        detailed: bool,
    },
    /// Get improvement suggestions for a skill
    Suggest {
        #[arg(long)]
        name: String,
    },
    /// Distill skills from execution history
    Distill {
        #[arg(long, default_value_t = false)]
        run: bool,
    },
    /// List distilled skills
    ListDistilled {
        #[arg(long, default_value_t = false)]
        detailed: bool,
    },
    /// Match distilled skills to a task description
    Match {
        #[arg(long)]
        task: String,
        #[arg(long, default_value = "5")]
        limit: usize,
    },
    /// Apply a distilled skill to a task
    Apply {
        #[arg(long)]
        skill_id: String,
        #[arg(long)]
        task_description: String,
    },
}

/// Memory management commands
#[derive(Subcommand, Debug)]
enum MemoryCommands {
    /// Initialize memory system
    Init,
    /// Add content to memory
    Add {
        #[arg(long)]
        target: String, // memory or user
        #[arg(long)]
        content: String,
        #[arg(long)]
        section: Option<String>,
    },
    /// Remove content from memory
    Remove {
        #[arg(long)]
        target: String,
        #[arg(long)]
        content: String,
    },
    /// Read memory content
    Read {
        #[arg(long)]
        target: String,
    },
    /// Search historical sessions
    Search {
        #[arg(long)]
        query: String,
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// List recent sessions
    Recent {
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Show memory statistics
    Stats,
}

/// MCP management commands
#[derive(Subcommand, Debug)]
enum McpCommands {
    /// Initialize MCP configuration
    Init,
    /// List available MCP servers and tools
    List {
        #[arg(long, default_value = "false")]
        detailed: bool,
    },
    /// Call an MCP tool
    Call {
        #[arg(long)]
        server: String,
        #[arg(long)]
        tool: String,
        #[arg(long)]
        args: Option<String>,
    },
}

/// Cron management commands
#[derive(Subcommand, Debug)]
enum CronCommands {
    /// Schedule a recurring job
    Schedule {
        #[arg(long)]
        id: String,
        #[arg(long)]
        cron: String,
        #[arg(long)]
        description: String,
        #[arg(long, default_value = "7")]
        expire_after_days: u32,
    },
    /// Schedule a one-shot reminder
    Remind {
        #[arg(long)]
        id: String,
        #[arg(long)]
        at: String,
        #[arg(long)]
        description: String,
    },
    /// Cancel a scheduled job
    Cancel {
        #[arg(long)]
        id: String,
    },
    /// List all scheduled jobs
    List,
    /// Show cron statistics
    Stats,
}

/// Subagent management commands
#[derive(Subcommand, Debug)]
enum SubagentCommands {
    /// Dispatch a task to subagents
    Dispatch {
        #[arg(long)]
        proposal: String,
        #[arg(long)]
        task: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        context: Option<String>,
    },
    /// List subagent dispatch history
    History {
        #[arg(long)]
        proposal: String,
    },
    /// Show subagent recovery ledger
    Ledger {
        #[arg(long)]
        proposal: String,
        #[arg(long)]
        task: String,
    },
}

/// Safety governance commands
#[derive(Subcommand, Debug)]
enum GovernanceCommands {
    /// Check if an action is allowed
    Check {
        #[arg(long)]
        action: String,
    },
    /// List authorization matrix
    Matrix {
        #[arg(long, default_value = "false")]
        detailed: bool,
    },
    /// Create an approval ticket
    Ticket {
        #[arg(long)]
        action: String,
        #[arg(long)]
        description: String,
        #[arg(long, default_value = "high")]
        risk: String,
    },
    /// List pending approval tickets
    Tickets,
    /// Approve a ticket
    Approve {
        #[arg(long)]
        ticket_id: String,
        #[arg(long)]
        approver: String,
    },
    /// Reject a ticket
    Reject {
        #[arg(long)]
        ticket_id: String,
        #[arg(long)]
        reason: String,
    },
    /// Show governance statistics
    Stats,
}

/// GitHub integration commands
#[derive(Subcommand, Debug)]
enum GithubCommands {
    /// Import GitHub issues as proposals
    Import {
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        issues: Option<Vec<u64>>,
    },
    /// Create a pull request
    CreatePR {
        #[arg(long)]
        branch: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        base: Option<String>,
    },
    /// Write comment to issue/PR
    Comment {
        #[arg(long)]
        issue: u64,
        #[arg(long)]
        body: String,
    },
    /// Write execution summary to issue
    Summarize {
        #[arg(long)]
        issue: u64,
        #[arg(long)]
        proposal: String,
    },
}

/// Set bridge address in the project manifest
fn set_bridge_address(project: &PathBuf, addr: &str) -> Result<()> {
    use zn_spec::{load_manifest, save_manifest};
    let mut manifest = load_manifest(project)?.unwrap_or_default();
    manifest.bridge_address = Some(addr.to_string());
    save_manifest(project, &manifest)?;
    println!("Bridge address set to: {}", addr);
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { project, host } => {
            let sdk = from_project(&project.display().to_string(), detect_host(host.as_deref()));
            sdk.init()?;
            println!("Initialized Zero_Nine at {}", project.display());
        }
        Commands::Brainstorm {
            project,
            host,
            goal,
            resume,
        } => {
            let sdk = from_project(&project.display().to_string(), detect_host(host.as_deref()));
            let output = if !resume && !matches!(sdk.host(), HostKind::Terminal) {
                let input = goal.as_deref().ok_or_else(|| {
                    anyhow!("goal or answer input is required for host-native brainstorming")
                })?;
                sdk.brainstorm_host_turn(input)?
            } else if matches!(sdk.host(), HostKind::Terminal) {
                let mut terminal_input = RustylineInput::new()?;
                sdk.brainstorm(goal.as_deref(), resume, &mut terminal_input)?
            } else {
                sdk.brainstorm_headless(goal.as_deref(), resume)?
            };
            println!("{}", output);
        }
        Commands::Run {
            project,
            host,
            goal,
            confirm_remote_finish,
            bridge_address,
        } => {
            if let Some(addr) = &bridge_address {
                set_bridge_address(&project, addr)?;
            }
            let sdk = from_project(&project.display().to_string(), detect_host(host.as_deref()));
            let output = if matches!(sdk.host(), HostKind::Terminal) {
                let mut terminal_input = RustylineInput::new()?;
                sdk.run_goal(&goal, confirm_remote_finish, &mut terminal_input)?
            } else {
                sdk.run_goal_headless(&goal, confirm_remote_finish)?
            };
            println!("{}", output);
        }
        Commands::Status { project } => {
            let sdk = from_project(&project.display().to_string(), HostKind::Terminal);
            println!("{}", sdk.status()?.message);
        }
        Commands::Resume {
            project,
            host,
            confirm_remote_finish,
            bridge_address,
        } => {
            if let Some(addr) = &bridge_address {
                set_bridge_address(&project, addr)?;
            }
            let sdk = from_project(&project.display().to_string(), detect_host(host.as_deref()));
            let output = if matches!(sdk.host(), HostKind::Terminal) {
                let mut terminal_input = RustylineInput::new()?;
                sdk.resume(confirm_remote_finish, &mut terminal_input)?
            } else {
                sdk.resume_headless(confirm_remote_finish)?
            };
            println!("{}", output);
        }
        Commands::Export { project } => {
            let sdk = from_project(&project.display().to_string(), HostKind::Terminal);
            println!("{}", sdk.export()?);
        }
        Commands::Skill { command } => {
            let project = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            match command {
                SkillCommands::Create {
                    name,
                    description,
                    category,
                    version,
                    content,
                } => {
                    let manager = create_default_manager(&project);
                    let content_str = content.unwrap_or_else(|| DEFAULT_SKILL_CONTENT.to_string());
                    let path =
                        manager.create(&name, &content_str, &category, &description, &version)?;
                    println!("Created skill at: {}", path.display());
                }
                SkillCommands::List { detailed } => {
                    let manager = create_default_manager(&project);
                    let skills = manager.list()?;
                    if skills.is_empty() {
                        println!("No skills found in project");
                    } else {
                        println!("Found {} skill(s):\n", skills.len());
                        for skill in skills {
                            print_skill_summary(&skill, detailed);
                        }
                    }
                }
                SkillCommands::View { name } => {
                    let manager = create_default_manager(&project);
                    let skill = manager.view(&name)?;
                    println!("{}", skill.render());
                }
                SkillCommands::Patch { name, old, new } => {
                    let manager = create_default_manager(&project);
                    let path = manager.patch(&name, &old, &new)?;
                    println!("Patched skill at: {}", path.display());
                }
                SkillCommands::Edit { name, content } => {
                    let manager = create_default_manager(&project);
                    let path = manager.edit(&name, &content)?;
                    println!("Edited skill at: {}", path.display());
                }
                SkillCommands::Delete { name } => {
                    let manager = create_default_manager(&project);
                    manager.delete(&name)?;
                    println!("Deleted skill: {}", name);
                }
                SkillCommands::Validate { name } => {
                    let manager = create_default_manager(&project);
                    let issues = manager.validate(&name)?;
                    if issues.is_empty() {
                        println!("Skill '{}' is valid", name);
                    } else {
                        println!("Validation issues for '{}':\n", name);
                        for issue in issues {
                            println!("[{}] {}: {}", issue.severity, issue.code, issue.message);
                        }
                    }
                }
                SkillCommands::Score { name } => {
                    let scorer = create_default_scorer(&project)?;
                    match scorer.get_score_summary(&name) {
                        Some(summary) => {
                            println!("Skill: {}", summary.skill_name);
                            println!("  Average Score: {:.2}", summary.average_score);
                            println!("  Execution Count: {}", summary.execution_count);
                            println!("  Success Rate: {:.1}%", summary.success_rate * 100.0);
                            println!("  Avg Latency: {}ms", summary.average_latency_ms);
                        }
                        None => {
                            println!("No execution data found for skill '{}'", name);
                        }
                    }
                }
                SkillCommands::Scores { detailed } => {
                    let scorer = create_default_scorer(&project)?;
                    let summaries = scorer.get_all_summaries();
                    if summaries.is_empty() {
                        println!("No skill execution data found");
                    } else {
                        println!("Found {} skill(s) with execution data:\n", summaries.len());
                        for summary in summaries {
                            if detailed {
                                println!("  {} (v{})", summary.skill_name, summary.execution_count);
                                println!(
                                    "    Avg Score: {:.2}, Success Rate: {:.1}%, Latency: {}ms\n",
                                    summary.average_score,
                                    summary.success_rate * 100.0,
                                    summary.average_latency_ms
                                );
                            } else {
                                println!(
                                    "  {}: score={:.2}, success={:.1}%",
                                    summary.skill_name,
                                    summary.average_score,
                                    summary.success_rate * 100.0
                                );
                            }
                        }
                    }
                }
                SkillCommands::Suggest { name } => {
                    let scorer = create_default_scorer(&project)?;
                    let suggestions = scorer.suggest_improvements(&name);
                    if suggestions.is_empty() {
                        println!("No improvement suggestions for skill '{}'", name);
                        println!("(Not enough execution data or skill is performing well)");
                    } else {
                        println!("Improvement suggestions for '{}':\n", name);
                        for (i, sugg) in suggestions.iter().enumerate() {
                            println!("{}. [Priority {}] {}", i + 1, sugg.priority, sugg.category);
                            println!("   {}", sugg.suggestion);
                            println!("   Expected: {}\n", sugg.expected_impact);
                        }
                    }
                }
                SkillCommands::Distill { run } => {
                    if run {
                        // Load recent execution reports and distill
                        let events_file = project.join(".zero_nine/runtime/events.ndjson");
                        if !events_file.exists() {
                            println!("No execution events found at {}", events_file.display());
                        } else {
                            use std::io::{BufRead, BufReader};
                            let file = std::fs::File::open(&events_file)?;
                            let reader = BufReader::new(file);
                            let mut distilled_count = 0;

                            for line in reader.lines() {
                                let line = line?;
                                if let Ok(event) =
                                    serde_json::from_str::<zn_types::RuntimeEvent>(&line)
                                {
                                    if event.event == "task_execution" {
                                        if let Some(payload) = event.payload {
                                            if let Ok(report) =
                                                serde_json::from_value::<zn_types::ExecutionReport>(
                                                    payload,
                                                )
                                            {
                                                let mut dist =
                                                    zn_evolve::distiller::create_default_distiller(
                                                        &project,
                                                    )?;
                                                let skills = dist.distill_from_report(&report)?;
                                                distilled_count += skills.len();
                                            }
                                        }
                                    }
                                }
                            }

                            println!(
                                "Distilled {} new skill(s) from execution history",
                                distilled_count
                            );
                        }
                    } else {
                        // Just show existing distilled skills
                        println!("Use --run to distill skills from execution history");
                    }
                }
                SkillCommands::ListDistilled { detailed } => {
                    let distiller = zn_evolve::distiller::create_default_distiller(&project)?;
                    let skills = distiller.get_all_skills();

                    if skills.is_empty() {
                        println!("No distilled skills found");
                        println!("Run `zero-nine skill distill --run` to extract skills from execution history");
                    } else {
                        println!("Found {} distilled skill(s):\n", skills.len());
                        for skill in skills {
                            if detailed {
                                println!(
                                    "  Skill: {} (v{})",
                                    skill.bundle.name, skill.bundle.version
                                );
                                println!("    ID: {}", skill.bundle.id);
                                println!("    Description: {}", skill.bundle.description);
                                println!("    Confidence: {:.2}", skill.confidence_score);
                                println!("    Usage Count: {}", skill.bundle.usage_count);
                                println!(
                                    "    Success Rate: {:.1}%",
                                    skill.bundle.success_rate * 100.0
                                );
                                println!("    Preconditions: {:?}", skill.bundle.preconditions);
                                println!("    Recommendations:");
                                for rec in &skill.usage_recommendations {
                                    println!("      - {}", rec);
                                }
                                if !skill.anti_patterns.is_empty() {
                                    println!("    Anti-patterns:");
                                    for anti in &skill.anti_patterns {
                                        println!("      - {}", anti);
                                    }
                                }
                                println!();
                            } else {
                                println!(
                                    "  {} (v{}): {} [confidence: {:.2}, usage: {}]",
                                    skill.bundle.name,
                                    skill.bundle.version,
                                    skill.bundle.description,
                                    skill.confidence_score,
                                    skill.bundle.usage_count
                                );
                            }
                        }
                    }
                }
                SkillCommands::Match { task, limit } => {
                    let distiller = zn_evolve::distiller::create_default_distiller(&project)?;
                    let matched = distiller.match_skills_for_task(&task);

                    if matched.is_empty() {
                        println!("No matching skills found for task: {}", task);
                    } else {
                        println!(
                            "Found {} matching skill(s) for task '{}':\n",
                            matched.len(),
                            task
                        );
                        for (i, skill) in matched.iter().take(limit).enumerate() {
                            println!(
                                "{}. {} (confidence: {:.2}, success: {:.1}%)",
                                i + 1,
                                skill.bundle.name,
                                skill.confidence_score * 100.0,
                                skill.bundle.success_rate * 100.0
                            );
                            println!("   Description: {}", skill.bundle.description);
                            println!("   Recommendations:");
                            for rec in &skill.usage_recommendations {
                                println!("     - {}", rec);
                            }
                            println!();
                        }
                    }
                }
                SkillCommands::Apply {
                    skill_id,
                    task_description,
                } => {
                    let distiller = zn_evolve::distiller::create_default_distiller(&project)?;

                    // Create a mock execution plan to demonstrate skill application
                    let mut plan = zn_types::ExecutionPlan {
                        task_id: "demo-task".to_string(),
                        objective: task_description.clone(),
                        mode: zn_types::ExecutionMode::SubagentReview,
                        workspace_strategy: zn_types::WorkspaceStrategy::InPlace,
                        steps: vec![],
                        validation: vec![],
                        quality_gates: vec![],
                        skill_chain: vec![],
                        deliverables: vec![],
                        risks: vec![],
                        subagents: vec![],
                        worktree_plan: None,
                        workspace_record: None,
                        verification_actions: vec![],
                        finish_branch_automation: None,
                        execution_path: zn_types::SubagentExecutionPath::default(),
                        bridge_address: None,
                    };

                    match distiller.apply_skill_to_plan(&skill_id, &mut plan) {
                        Ok(applied) => {
                            println!("Skill '{}' applied to task: {}", skill_id, task_description);
                            println!("\nModified execution plan:");
                            println!("  Skill chain: {:?}", plan.skill_chain);
                            println!("  Validation points: {:?}", plan.validation);
                            println!("  Deliverables: {:?}", plan.deliverables);
                            println!("  Risks/Notes: {:?}", plan.risks);

                            if applied {
                                println!("\nSkill successfully integrated into execution plan.");
                            }
                        }
                        Err(e) => {
                            println!("Failed to apply skill: {}", e);
                        }
                    }
                }
            }
        }
        Commands::Memory { command } => {
            let project = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            match command {
                MemoryCommands::Init => {
                    let _manager = create_memory_manager(&project)?;
                    let _ = create_default_searcher(&project)?;
                    println!(
                        "Memory system initialized at: {}/.zero_nine/memory/",
                        project.display()
                    );
                }
                MemoryCommands::Add {
                    target,
                    content,
                    section,
                } => {
                    let mut manager = create_memory_manager(&project)?;
                    let memory_target = if target.to_lowercase() == "memory" {
                        MemoryTarget::Memory
                    } else if target.to_lowercase() == "user" {
                        MemoryTarget::User
                    } else {
                        anyhow::bail!("Invalid target. Use 'memory' or 'user'");
                    };
                    let action = MemoryAction::Add {
                        target: memory_target.clone(),
                        content,
                        section,
                    };
                    let result = manager.execute(&action)?;
                    println!("Memory updated: {} - {}", result.target, result.message);
                }
                MemoryCommands::Remove { target, content } => {
                    let mut manager = create_memory_manager(&project)?;
                    let memory_target = if target.to_lowercase() == "memory" {
                        MemoryTarget::Memory
                    } else if target.to_lowercase() == "user" {
                        MemoryTarget::User
                    } else {
                        anyhow::bail!("Invalid target. Use 'memory' or 'user'");
                    };
                    let action = MemoryAction::Remove {
                        target: memory_target,
                        old_text: content,
                    };
                    let result = manager.execute(&action)?;
                    println!("Memory updated: {} - {}", result.target, result.message);
                }
                MemoryCommands::Read { target } => {
                    let manager = create_memory_manager(&project)?;
                    let memory_target = if target.to_lowercase() == "memory" {
                        MemoryTarget::Memory
                    } else if target.to_lowercase() == "user" {
                        MemoryTarget::User
                    } else {
                        anyhow::bail!("Invalid target. Use 'memory' or 'user'");
                    };
                    let content = manager.read(&memory_target)?;
                    println!("{}", content);
                }
                MemoryCommands::Search { query, limit } => {
                    let searcher = create_default_searcher(&project)?;
                    let results = searcher.search(&query, limit)?;
                    if results.results.is_empty() {
                        println!("No sessions found matching '{}'", query);
                    } else {
                        println!("Found {} session(s) matching '{}':\n", results.total, query);
                        for (i, result) in results.results.iter().enumerate() {
                            println!(
                                "{}. [{}] {} - {} ({}) [score: {:.2}]",
                                i + 1,
                                result.session_type,
                                result.goal,
                                result.created_at.format("%Y-%m-%d"),
                                if result.success { "✓" } else { "✗" },
                                result.relevance_score.unwrap_or(0.0)
                            );
                        }
                    }
                }
                MemoryCommands::Recent { limit } => {
                    let searcher = create_default_searcher(&project)?;
                    let sessions = searcher.get_recent(limit)?;
                    if sessions.is_empty() {
                        println!("No recent sessions found");
                    } else {
                        println!("Recent {} session(s):\n", sessions.len());
                        for (i, session) in sessions.iter().enumerate() {
                            println!(
                                "{}. [{}] {} - {} ({})",
                                i + 1,
                                session.session_type,
                                session.goal,
                                session.created_at.format("%Y-%m-%d"),
                                if session.success { "✓" } else { "✗" }
                            );
                        }
                    }
                }
                MemoryCommands::Stats => {
                    let searcher = create_default_searcher(&project)?;
                    let stats = searcher.get_stats()?;
                    println!("Session Statistics:");
                    println!("  Total Sessions: {}", stats.total_sessions);
                    println!(
                        "  Successful Sessions: {} ({:.1}%)",
                        stats.successful_sessions,
                        stats.success_rate * 100.0
                    );
                    println!("  Brainstorm Sessions: {}", stats.brainstorm_sessions);
                    println!("  Execution Sessions: {}", stats.execution_sessions);
                }
            }
        }
        Commands::Mcp { command } => {
            let project = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            match command {
                McpCommands::Init => {
                    let client = zn_bridge::mcp_client::load_or_create_mcp_config(&project)?;
                    println!(
                        "MCP system initialized at: {}/.zero_nine/mcp_config.yaml",
                        project.display()
                    );
                    println!("Configured servers: {:?}", client.get_server_names());
                }
                McpCommands::List { detailed } => {
                    let client = zn_bridge::mcp_client::load_or_create_mcp_config(&project)?;
                    let tools = client.list_tools();
                    if tools.is_empty() {
                        println!("No MCP tools available");
                    } else {
                        println!("Found {} MCP tool(s):\n", tools.len());
                        for tool in tools {
                            if detailed {
                                println!("  {} (server: {})", tool.name, tool.server);
                                println!("    Description: {}", tool.description);
                                println!("    Schema: {:?}\n", tool.input_schema);
                            } else {
                                println!("  {} [{}]", tool.name, tool.server);
                            }
                        }
                    }
                }
                McpCommands::Call { server, tool, args } => {
                    let client = zn_bridge::mcp_client::load_or_create_mcp_config(&project)?;
                    let args_value: serde_json::Value = if let Some(args_str) = args {
                        serde_json::from_str(&args_str)
                            .unwrap_or_else(|_| serde_json::json!({ "raw": args_str }))
                    } else {
                        serde_json::json!({})
                    };

                    let result = tokio::runtime::Runtime::new()
                        .unwrap()
                        .block_on(client.call_tool(&server, &tool, args_value));

                    match result {
                        Ok(res) => {
                            if res.success {
                                println!("Success! Result: {:?}", res.content);
                            } else {
                                println!(
                                    "Tool execution failed: {}",
                                    res.error.unwrap_or_default()
                                );
                            }
                        }
                        Err(e) => {
                            println!("Error calling tool: {}", e);
                        }
                    }
                }
            }
        }
        Commands::Cron { command } => {
            let project = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            match command {
                CronCommands::Schedule {
                    id,
                    cron,
                    description,
                    expire_after_days,
                } => {
                    let mut scheduler = zn_loop::cron_scheduler::CronScheduler::new(&project)?;
                    let job = zn_loop::cron_scheduler::create_recurring_job(
                        &id,
                        &cron,
                        &description,
                        serde_json::json!({}),
                        Some(expire_after_days),
                    );
                    scheduler.schedule(job)?;
                    println!("Scheduled recurring job: {}", id);
                    println!("  Cron: {}", cron);
                    println!("  Description: {}", description);
                    println!("  Expires after: {} days", expire_after_days);
                }
                CronCommands::Remind {
                    id,
                    at,
                    description,
                } => {
                    let mut scheduler = zn_loop::cron_scheduler::CronScheduler::new(&project)?;

                    // Parse the scheduled time (supports formats like "2024-01-15T10:30:00" or "10:30")
                    let scheduled_at = parse_datetime(&at)?;

                    let job = zn_loop::cron_scheduler::create_one_shot_job(
                        &id,
                        scheduled_at,
                        &description,
                        serde_json::json!({}),
                    );
                    scheduler.schedule(job)?;
                    println!("Scheduled one-shot reminder: {}", id);
                    println!(
                        "  Scheduled at: {}",
                        scheduled_at.format("%Y-%m-%d %H:%M:%S")
                    );
                    println!("  Description: {}", description);
                }
                CronCommands::Cancel { id } => {
                    let mut scheduler = zn_loop::cron_scheduler::CronScheduler::new(&project)?;
                    if scheduler.cancel(&id)? {
                        println!("Cancelled job: {}", id);
                    } else {
                        println!("Job not found: {}", id);
                    }
                }
                CronCommands::List => {
                    let scheduler = zn_loop::cron_scheduler::CronScheduler::new(&project)?;
                    let jobs = scheduler.list_jobs();
                    if jobs.is_empty() {
                        println!("No scheduled jobs");
                    } else {
                        println!("Scheduled jobs ({} total):\n", jobs.len());
                        for job in jobs {
                            let status = if job.enabled { "enabled" } else { "disabled" };
                            let job_type = match &job.job_type {
                                zn_loop::cron_scheduler::JobType::Recurring {
                                    expire_after_days,
                                } => {
                                    format!("recurring ({} days)", expire_after_days.unwrap_or(7))
                                }
                                zn_loop::cron_scheduler::JobType::OneShot { scheduled_at } => {
                                    format!("one-shot (scheduled: {})", scheduled_at)
                                }
                            };
                            println!("  {} [{}] - {}", job.id, status, job.description);
                            println!("    Type: {}", job_type);
                            println!("    Cron: {}", job.cron);
                            println!("    Next run: {}", job.next_run.as_deref().unwrap_or("N/A"));
                            println!("    Run count: {}\n", job.run_count);
                        }
                    }
                }
                CronCommands::Stats => {
                    let scheduler = zn_loop::cron_scheduler::CronScheduler::new(&project)?;
                    let stats = scheduler.get_stats();
                    println!("Cron Scheduler Statistics:");
                    println!("  Total jobs: {}", stats.total);
                    println!("  Enabled: {}", stats.enabled);
                    println!("  Disabled: {}", stats.disabled);
                    println!("  Recurring: {}", stats.recurring);
                    println!("  One-shot: {}", stats.one_shot);
                    println!("  Total runs: {}", stats.total_runs);
                    println!("  Failed runs: {}", stats.failed_runs);
                }
            }
        }
        Commands::Subagent { command } => {
            let project = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            match command {
                SubagentCommands::Dispatch {
                    proposal,
                    task,
                    role,
                    context,
                } => {
                    let dispatcher = zn_exec::subagent_dispatcher::create_dispatcher(
                        &project,
                        &proposal,
                        &task,
                        vec![],
                    )?;

                    // Load context files if provided
                    let context_files = if let Some(ctx) = context {
                        let mut files = std::collections::HashMap::new();
                        for path in ctx.split(',') {
                            let path = path.trim();
                            if let Ok(content) = std::fs::read_to_string(path) {
                                let name = std::path::Path::new(path)
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                files.insert(name, content);
                            }
                        }
                        files
                    } else {
                        std::collections::HashMap::new()
                    };

                    let subagent_ctx = zn_exec::subagent_dispatcher::SubagentContext {
                        role,
                        context_files,
                        expected_outputs: vec![],
                        objective: "Execute subagent task".to_string(),
                    };

                    let bundle = dispatcher.prepare_context(&subagent_ctx)?;
                    println!("Subagent context prepared:");
                    println!("  Role: {}", bundle.role);
                    println!("  Bundle directory: {}", bundle.bundle_dir.display());
                    println!("  Manifest: {}", bundle.manifest_path.display());
                }
                SubagentCommands::History { proposal } => {
                    let history_dir = project
                        .join(".zero_nine/runtime/subagents/runbooks")
                        .join(&proposal);

                    if !history_dir.exists() {
                        println!("No dispatch history found for proposal: {}", proposal);
                    } else {
                        let mut entries: Vec<_> = std::fs::read_dir(&history_dir)?
                            .filter_map(|e| e.ok())
                            .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
                            .collect();
                        entries.sort_by_key(|e| e.file_name());

                        println!(
                            "Dispatch history for proposal {} ({} entries):\n",
                            proposal,
                            entries.len()
                        );
                        for entry in entries {
                            println!("  - {}", entry.file_name().to_string_lossy());
                        }
                    }
                }
                SubagentCommands::Ledger { proposal, task } => {
                    let ledger_path = project
                        .join(".zero_nine/runtime/subagents")
                        .join(&proposal)
                        .join(format!("{}-recovery.json", task));

                    if !ledger_path.exists() {
                        println!("No recovery ledger found for task: {}", task);
                    } else {
                        let content = std::fs::read_to_string(&ledger_path)?;
                        let ledger: zn_types::SubagentRecoveryLedger =
                            serde_json::from_str(&content)?;

                        println!("Recovery Ledger for task {}:", task);
                        println!("  Records: {}", ledger.records.len());
                        for record in &ledger.records {
                            println!("  - Role: {}, Status: {:?}", record.role, record.status);
                        }
                    }
                }
            }
        }
        Commands::Governance { command } => {
            let project = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            match command {
                GovernanceCommands::Check { action } => {
                    let engine = zn_exec::governance::PolicyEngine::new(&project)?;

                    // Parse action type from string
                    let action_type = parse_action_type(&action);

                    match action_type {
                        Some(action) => {
                            let result = engine.check_action(&action);
                            println!("Action: {:?}", action);
                            println!("  Risk Level: {}", result.risk_level);
                            println!("  Allowed: {}", result.allowed);
                            println!("  Requires Confirmation: {}", result.requires_confirmation);
                            println!("  Requires Approval: {}", result.requires_approval);
                            println!("  Blocked: {}", result.blocked);
                        }
                        None => {
                            println!("Unknown action type: {}", action);
                            println!("Available actions: ReadFile, WriteFile, DeleteFile, RunCommand, GitCommit, GitPush, GitMerge, CreatePR, etc.");
                        }
                    }
                }
                GovernanceCommands::Matrix { detailed } => {
                    let engine = zn_exec::governance::PolicyEngine::new(&project)?;
                    let matrix = engine.get_matrix();

                    println!("Authorization Matrix ({} entries):\n", matrix.entries.len());

                    let mut entries: Vec<_> = matrix.entries.values().collect();
                    entries.sort_by_key(|e| e.risk_level);

                    for entry in entries {
                        if detailed {
                            println!(
                                "  {:?}: {} ({:?})",
                                entry.action, entry.authorization, entry.risk_level
                            );
                            if !entry.conditions.is_empty() {
                                println!("    Conditions: {:?}", entry.conditions);
                            }
                        } else {
                            println!(
                                "  {:?} - {:?} [{}]",
                                entry.action,
                                entry.risk_level,
                                match &entry.authorization {
                                    zn_exec::governance::AuthorizationRequirement::None => "auto",
                                    zn_exec::governance::AuthorizationRequirement::Log => "log",
                                    zn_exec::governance::AuthorizationRequirement::Confirm =>
                                        "confirm",
                                    zn_exec::governance::AuthorizationRequirement::Approval {
                                        ..
                                    } => "approval",
                                    zn_exec::governance::AuthorizationRequirement::Blocked {
                                        ..
                                    } => "blocked",
                                }
                            );
                        }
                    }
                }
                GovernanceCommands::Ticket {
                    action,
                    description,
                    risk,
                } => {
                    let engine = zn_exec::governance::PolicyEngine::new(&project)?;

                    let risk_level = match risk.to_lowercase().as_str() {
                        "low" => zn_exec::governance::RiskLevel::Low,
                        "medium" => zn_exec::governance::RiskLevel::Medium,
                        "high" => zn_exec::governance::RiskLevel::High,
                        "critical" => zn_exec::governance::RiskLevel::Critical,
                        _ => {
                            println!("Invalid risk level: {}. Using 'high' as default.", risk);
                            zn_exec::governance::RiskLevel::High
                        }
                    };

                    let ticket = engine.create_approval_ticket(&action, &description, risk_level);
                    engine.save_ticket(&ticket)?;

                    println!("{}", zn_exec::governance::render_approval_ticket(&ticket));
                }
                GovernanceCommands::Tickets => {
                    let engine = zn_exec::governance::PolicyEngine::new(&project)?;
                    let tickets = engine.load_pending_tickets()?;

                    if tickets.is_empty() {
                        println!("No pending approval tickets");
                    } else {
                        println!("Pending approval tickets ({} total):\n", tickets.len());
                        for ticket in &tickets {
                            println!("{}", zn_exec::governance::render_approval_ticket(ticket));
                            println!("---");
                        }
                    }
                }
                GovernanceCommands::Approve {
                    ticket_id,
                    approver,
                } => {
                    // Load tickets and find the one to approve
                    let engine = zn_exec::governance::PolicyEngine::new(&project)?;
                    let mut tickets = engine.load_pending_tickets()?;

                    if let Some(ticket) = tickets.iter_mut().find(|t| t.id == ticket_id) {
                        ticket.approve(&approver);
                        engine.save_ticket(ticket)?;
                        println!("Ticket {} approved by {}", ticket_id, approver);
                        println!("{}", zn_exec::governance::render_approval_ticket(ticket));
                    } else {
                        println!("Ticket not found: {}", ticket_id);
                    }
                }
                GovernanceCommands::Reject { ticket_id, reason } => {
                    let engine = zn_exec::governance::PolicyEngine::new(&project)?;
                    let mut tickets = engine.load_pending_tickets()?;

                    if let Some(ticket) = tickets.iter_mut().find(|t| t.id == ticket_id) {
                        ticket.reject(&reason);
                        engine.save_ticket(ticket)?;
                        println!("Ticket {} rejected: {}", ticket_id, reason);
                        println!("{}", zn_exec::governance::render_approval_ticket(ticket));
                    } else {
                        println!("Ticket not found: {}", ticket_id);
                    }
                }
                GovernanceCommands::Stats => {
                    let engine = zn_exec::governance::PolicyEngine::new(&project)?;
                    let stats = engine.get_stats();

                    println!("Governance Statistics:");
                    println!("  Total Tickets: {}", stats.total_tickets);
                    println!("  Pending: {}", stats.pending);
                    println!("  Approved: {}", stats.approved);
                    println!("  Rejected: {}", stats.rejected);
                }
            }
        }
        Commands::Github { command } => {
            let project = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            match command {
                GithubCommands::Import { repo, issues } => {
                    let imported = zn_host::read_github_issues(&project, repo.as_deref(), issues)?;

                    if imported.is_empty() {
                        println!("No issues imported");
                    } else {
                        println!("Imported {} issue(s) as proposal(s):\n", imported.len());
                        for item in imported {
                            println!(
                                "  - {} -> {} ({})",
                                item.source, item.proposal.title, item.proposal.id
                            );
                        }
                    }
                }
                GithubCommands::CreatePR {
                    branch,
                    title,
                    body,
                    base,
                } => {
                    let body_text =
                        body.unwrap_or_else(|| format!("Generated by Zero_Nine for {}", branch));
                    match zn_host::create_pull_request(
                        None,
                        &branch,
                        &title,
                        &body_text,
                        base.as_deref(),
                    ) {
                        Ok(result) => {
                            println!("PR created successfully!");
                            println!("  URL: {}", result.pr_url);
                            println!("  Message: {}", result.message);
                        }
                        Err(e) => {
                            println!("Failed to create PR: {}", e);
                        }
                    }
                }
                GithubCommands::Comment { issue, body } => {
                    match zn_host::write_issue_comment(None, issue, &body) {
                        Ok(result) => {
                            println!("Comment posted: {}", result.message);
                        }
                        Err(e) => {
                            println!("Failed to post comment: {}", e);
                        }
                    }
                }
                GithubCommands::Summarize { issue, proposal } => {
                    // Load proposal from disk
                    use zn_spec::load_latest_proposal;

                    let proposal_obj = load_latest_proposal(&project)?
                        .ok_or_else(|| anyhow!("Proposal not found: {}", proposal))?;

                    let summary = format!("Execution completed for proposal {}", proposal);

                    match zn_host::write_execution_summary(None, issue, &proposal_obj, &summary) {
                        Ok(result) => {
                            println!("Summary written to issue #{}: {}", issue, result.message);
                        }
                        Err(e) => {
                            println!("Failed to write summary: {}", e);
                        }
                    }
                }
            }
        }
        Commands::Dashboard { project } => {
            tui_dashboard::run_dashboard(&project)?;
        }
        Commands::Observe { command } => {
            let project = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let (_emitter, aggregator, query) =
                zn_exec::observability::create_default_observability(&project)?;

            match command {
                ObserveCommands::Events { event_type, limit } => {
                    let events = query.query_by_type(&event_type, limit)?;
                    if events.is_empty() {
                        println!("No events found for type: {}", event_type);
                    } else {
                        println!(
                            "Found {} event(s) of type '{}':\n",
                            events.len(),
                            event_type
                        );
                        for event in &events {
                            println!(
                                "  [{}] {} (proposal: {:?}, task: {:?})",
                                event.ts.format("%Y-%m-%d %H:%M:%S"),
                                event.event,
                                event.proposal_id,
                                event.task_id
                            );
                        }
                    }
                }
                ObserveCommands::Proposal { proposal_id, limit } => {
                    let events = query.query_by_proposal(&proposal_id, limit)?;
                    if events.is_empty() {
                        println!("No events found for proposal: {}", proposal_id);
                    } else {
                        println!(
                            "Found {} event(s) for proposal '{}':\n",
                            events.len(),
                            proposal_id
                        );
                        for event in &events {
                            println!(
                                "  [{}] {} (task: {:?})",
                                event.ts.format("%Y-%m-%d %H:%M:%S"),
                                event.event,
                                event.task_id
                            );
                        }
                    }
                }
                ObserveCommands::Trace { trace_id } => {
                    let tree = query.replay_trace(&trace_id)?;
                    println!("Trace: {}", tree.trace_id);
                    println!("Root spans: {}", tree.root_spans.len());
                    println!("\nSpan tree:");
                    for (i, span_id) in tree.root_spans.iter().enumerate() {
                        print_span_tree(&tree, span_id, i == 0, i == tree.root_spans.len() - 1, "");
                    }
                }
                ObserveCommands::Stats { task_id } => {
                    let stats = aggregator.get_latency_stats(task_id.as_deref());
                    let success_rate = aggregator.get_success_rate(task_id.as_deref());

                    println!("Latency Statistics:");
                    if task_id.is_some() {
                        println!("  Task: {}", task_id.as_ref().unwrap());
                    }
                    println!("  Count: {}", stats.count);
                    println!("  Min: {}ms", stats.min);
                    println!("  Max: {}ms", stats.max);
                    println!("  Avg: {}ms", stats.avg);
                    println!("  P95: {}ms", stats.p95);
                    println!("Success Rate: {:.1}%", success_rate * 100.0);
                }
                ObserveCommands::Metrics { limit } => {
                    let metrics = aggregator.get_recent(limit);
                    if metrics.is_empty() {
                        println!("No recent metrics found");
                    } else {
                        println!("Recent {} metric(s):\n", metrics.len());
                        for m in metrics {
                            println!(
                                "  Task: {} | Latency: {}ms | Success: {} | Tokens: {}",
                                m.task_id, m.latency_ms, m.success, m.token_usage
                            );
                        }
                    }
                }
            }
        }
        Commands::BridgeServer { port } => {
            use std::net::SocketAddr;
            use zn_bridge::{BridgeConfig, BridgeServer};
            use zn_exec::LocalCliHandler;

            let project = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
            let config = BridgeConfig {
                bind_addr: addr,
                ..Default::default()
            };

            let handler = LocalCliHandler::new(&project);

            println!("Starting gRPC bridge server on {}", addr);
            println!("Project root: {}", project.display());
            println!("Press Ctrl+C to stop");

            let server = BridgeServer::new(config)
                .with_dispatch_handler(handler.clone())
                .with_status_handler(handler.clone())
                .with_evidence_handler(handler);

            // Run the server (this blocks)
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(server.run())?;
        }
    }
    Ok(())
}

/// Print span tree recursively
fn print_span_tree(
    tree: &zn_exec::observability::TraceTree,
    span_id: &str,
    _is_first: bool,
    is_last: bool,
    prefix: &str,
) {
    if let Some(span) = tree.all_spans.get(span_id) {
        let connector = if is_last { "└─" } else { "├─" };
        println!(
            "  {} {} {} ({}ms)",
            prefix,
            connector,
            span.event_type,
            span.latency_ms.unwrap_or(0)
        );

        let child_prefix = format!("{}{}", prefix, if is_last { "  " } else { "│ " });
        for (i, child_id) in span.child_span_ids.iter().enumerate() {
            let is_last_child = i == span.child_span_ids.len() - 1;
            print_span_tree(tree, child_id, false, is_last_child, &child_prefix);
        }
    }
}

/// Parse datetime string
fn parse_datetime(s: &str) -> Result<chrono::DateTime<Local>> {
    // Try ISO 8601 format first
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Local));
    }

    // Try YYYY-MM-DD HH:MM:SS format
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok(Local
            .from_local_datetime(&dt)
            .single()
            .ok_or_else(|| anyhow!("Invalid datetime: {}", s))?);
    }

    // Try HH:MM format (today)
    if let Ok(dt) = chrono::NaiveTime::parse_from_str(s, "%H:%M") {
        let today = Local::now().date_naive();
        let naive_dt = chrono::NaiveDateTime::new(today, dt);
        let local_dt = Local
            .from_local_datetime(&naive_dt)
            .single()
            .ok_or_else(|| anyhow!("Invalid datetime: {}", s))?;

        // If time has passed today, schedule for tomorrow
        let scheduled = if local_dt <= Local::now() {
            local_dt + chrono::Duration::days(1)
        } else {
            local_dt
        };
        return Ok(scheduled);
    }

    Err(anyhow!(
        "Unable to parse datetime. Supported formats: RFC3339, YYYY-MM-DD HH:MM:SS, HH:MM"
    ))
}

/// Parse action type from string
fn parse_action_type(s: &str) -> Option<zn_exec::governance::ActionType> {
    match s.to_lowercase().as_str() {
        "readfile" => Some(zn_exec::governance::ActionType::ReadFile),
        "readdir" => Some(zn_exec::governance::ActionType::ReadDir),
        "readenv" => Some(zn_exec::governance::ActionType::ReadEnv),
        "writefile" => Some(zn_exec::governance::ActionType::WriteFile),
        "deletefile" => Some(zn_exec::governance::ActionType::DeleteFile),
        "modifyconfig" => Some(zn_exec::governance::ActionType::ModifyConfig),
        "runcommand" => Some(zn_exec::governance::ActionType::RunCommand),
        "runtest" => Some(zn_exec::governance::ActionType::RunTest),
        "runbuild" => Some(zn_exec::governance::ActionType::RunBuild),
        "gitstatus" => Some(zn_exec::governance::ActionType::GitStatus),
        "gitdiff" => Some(zn_exec::governance::ActionType::GitDiff),
        "gitbranch" => Some(zn_exec::governance::ActionType::GitBranch),
        "gitcommit" => Some(zn_exec::governance::ActionType::GitCommit),
        "gitpush" => Some(zn_exec::governance::ActionType::GitPush),
        "gitmerge" => Some(zn_exec::governance::ActionType::GitMerge),
        "gitdelete" => Some(zn_exec::governance::ActionType::GitDelete),
        "createissue" => Some(zn_exec::governance::ActionType::CreateIssue),
        "createpr" => Some(zn_exec::governance::ActionType::CreatePR),
        "mergepr" => Some(zn_exec::governance::ActionType::MergePR),
        "closepr" => Some(zn_exec::governance::ActionType::ClosePR),
        "dispatchsubagent" => Some(zn_exec::governance::ActionType::DispatchSubagent),
        "spawnworktree" => Some(zn_exec::governance::ActionType::SpawnWorktree),
        _ => None,
    }
}

const DEFAULT_SKILL_CONTENT: &str = r#"
# Skill Name

## When to Use
- When you need to do something

## Procedure
1. Do the thing
2. Verify the result

## Pitfalls
- Don't skip step 1
"#;

fn print_skill_summary(skill: &SkillSummary, detailed: bool) {
    if detailed {
        println!(
            "  Name: {}\n  Description: {}\n  Version: {}\n  Category: {}\n  Valid: {}\n",
            skill.name, skill.description, skill.version, skill.category, skill.valid
        );
    } else {
        println!(
            "  {} (v{}) - {} [{}]",
            skill.name, skill.version, skill.description, skill.category
        );
    }
}
