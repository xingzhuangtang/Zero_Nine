//! TUI Dashboard for Zero_Nine lifecycle overview
//!
//! This module provides:
//! - Real-time TUI dashboard for project status
//! - Proposal/task lifecycle visualization
//! - Drift, approval, and evolution metrics

use anyhow::Result;
use chrono::{DateTime, Utc};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame, Terminal,
};
use std::{io, path::Path, time::Duration};
use zn_types::{ProposalStatus, TaskStatus};

/// Dashboard application state
pub struct DashboardApp {
    pub project_root: String,
    pub proposals: Vec<ProposalSummary>,
    pub loop_stage: String,
    pub drift_status: String,
    pub governance_stats: GovernanceStats,
    pub selected_proposal: usize,
    pub should_quit: bool,
}

#[derive(Debug, Clone)]
pub struct ProposalSummary {
    pub id: String,
    pub title: String,
    pub goal: String,
    pub status: ProposalStatus,
    pub task_count: usize,
    pub completed_tasks: usize,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct GovernanceStats {
    pub pending_approvals: usize,
    pub total_tickets: usize,
    pub approved_count: usize,
    pub rejected_count: usize,
}

impl DashboardApp {
    pub fn new(project_root: &Path) -> Result<Self> {
        let proposals = load_proposal_summaries(project_root)?;
        let loop_stage = load_loop_stage(project_root)?;
        let drift_status = "OK".to_string();
        let governance_stats = load_governance_stats(project_root)?;

        Ok(Self {
            project_root: project_root.display().to_string(),
            proposals,
            loop_stage,
            drift_status,
            governance_stats,
            selected_proposal: 0,
            should_quit: false,
        })
    }

    pub fn tick(&mut self) {
        // Refresh data periodically
        let project_path = std::path::PathBuf::from(&self.project_root);
        if let Ok(proposals) = load_proposal_summaries(&project_path) {
            self.proposals = proposals;
        }
        if let Ok(stage) = load_loop_stage(&project_path) {
            self.loop_stage = stage;
        }
        if let Ok(stats) = load_governance_stats(&project_path) {
            self.governance_stats = stats;
        }
    }
}

fn load_proposal_summaries(project_root: &Path) -> Result<Vec<ProposalSummary>> {
    use std::fs;

    let mut proposals = Vec::new();
    let proposals_dir = project_root.join(".zero_nine/proposals");

    if !proposals_dir.exists() {
        return Ok(proposals);
    }

    for entry in fs::read_dir(proposals_dir)? {
        let entry = entry?;
        let proposal_file = entry.path().join("proposal.json");

        if let Ok(content) = fs::read_to_string(&proposal_file) {
            if let Ok(proposal) = serde_json::from_str::<zn_types::Proposal>(&content) {
                let completed = proposal.tasks.iter()
                    .filter(|t| matches!(t.status, TaskStatus::Completed))
                    .count();

                proposals.push(ProposalSummary {
                    id: proposal.id.clone(),
                    title: proposal.title.clone(),
                    goal: proposal.goal.clone(),
                    status: proposal.status,
                    task_count: proposal.tasks.len(),
                    completed_tasks: completed,
                    created_at: proposal.created_at,
                });
            }
        }
    }

    // Sort by created_at descending
    proposals.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(proposals)
}

fn load_loop_stage(project_root: &Path) -> Result<String> {
    use zn_spec::load_loop_state;

    match load_loop_state(project_root) {
        Ok(Some(state)) => Ok(format!("{:?} (iteration {})", state.stage, state.iteration)),
        Ok(None) => Ok("Idle".to_string()),
        Err(_) => Ok("Unknown".to_string()),
    }
}

fn load_governance_stats(project_root: &Path) -> Result<GovernanceStats> {
    use zn_exec::governance::PolicyEngine;

    match PolicyEngine::new(project_root) {
        Ok(engine) => {
            let stats = engine.get_stats();
            Ok(GovernanceStats {
                pending_approvals: stats.pending,
                total_tickets: stats.total_tickets,
                approved_count: stats.approved,
                rejected_count: stats.rejected,
            })
        }
        Err(_) => Ok(GovernanceStats {
            pending_approvals: 0,
            total_tickets: 0,
            approved_count: 0,
            rejected_count: 0,
        }),
    }
}

pub fn run_dashboard(project_root: &Path) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = DashboardApp::new(project_root)?;

    // Run UI loop
    let res = run_ui(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Dashboard error: {:?}", err);
    }

    Ok(())
}

fn run_ui<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut DashboardApp,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Up => {
                        if app.selected_proposal > 0 {
                            app.selected_proposal -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if app.selected_proposal < app.proposals.len().saturating_sub(1) {
                            app.selected_proposal += 1;
                        }
                    }
                    KeyCode::Char('r') => app.tick(),
                    _ => {}
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn ui(f: &mut Frame, app: &DashboardApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Min(0),
        ])
        .split(f.area());

    // Title bar
    let title = Paragraph::new(format!(" Zero_Nine Dashboard - {}", app.project_root))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Loop stage
    let stage_color = match app.loop_stage.as_str() {
        s if s.contains("Running") => Color::Green,
        s if s.contains("Verifying") => Color::Yellow,
        s if s.contains("Escalated") => Color::Red,
        _ => Color::White,
    };

    let loop_stage = Paragraph::new(format!(
        "Stage: {}\nDrift: {}\nLast Refresh: {}",
        app.loop_stage,
        app.drift_status,
        Utc::now().format("%H:%M:%S")
    ))
    .style(Style::default().fg(stage_color))
    .block(Block::default().title(" Loop State ").borders(Borders::ALL));
    f.render_widget(loop_stage, chunks[1]);

    // Governance stats
    let gov_stats = Paragraph::new(format!(
        "Pending Approvals: {}\nTotal Tickets: {}\nApproved: {}\nRejected: {}",
        app.governance_stats.pending_approvals,
        app.governance_stats.total_tickets,
        app.governance_stats.approved_count,
        app.governance_stats.rejected_count
    ))
    .style(Style::default().fg(Color::Yellow))
    .block(Block::default().title(" Governance ").borders(Borders::ALL));
    f.render_widget(gov_stats, chunks[2]);

    // Proposals table
    let header = Row::new(vec!["ID", "Title", "Status", "Progress", "Created"])
        .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app.proposals.iter().map(|p| {
        let progress = format!("{}/{}", p.completed_tasks, p.task_count);
        let status_str = format!("{:?}", p.status);
        let created = p.created_at.format("%Y-%m-%d %H:%M").to_string();

        Row::new(vec![
            p.id.clone(),
            p.title.clone(),
            status_str,
            progress,
            created,
        ])
    }).collect();

    let table = Table::new(rows, vec![
        Constraint::Length(20),
        Constraint::Min(20),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(15),
    ])
    .header(header)
    .block(Block::default().title(" Proposals (↑/↓ to navigate, r to refresh) ").borders(Borders::ALL));

    f.render_widget(table, chunks[3]);

    // Selected proposal details
    if let Some(selected) = app.proposals.get(app.selected_proposal) {
        let details = Paragraph::new(format!(
            "Selected: {}\n\nGoal: {}\n\nTasks: {}/{} completed",
            selected.title,
            selected.goal,
            selected.completed_tasks,
            selected.task_count
        ))
        .block(Block::default().title(" Details ").borders(Borders::ALL));
        f.render_widget(details, chunks[4]);
    }
}
