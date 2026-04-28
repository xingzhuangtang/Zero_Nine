//! TUI Dashboard for Zero_Nine lifecycle overview
//!
//! M7 enhanced: state timeline, event stream, auto-refresh, color-coded proposals, key metrics

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
use std::{fs, io, path::Path, time::Duration, time::Instant};
use zn_types::{ProposalStatus, RuntimeEvent, StateTransition, TaskStatus};

/// Dashboard application state
pub struct DashboardApp {
    pub project_root: String,
    pub proposals: Vec<ProposalSummary>,
    pub loop_stage: String,
    pub loop_iteration: u32,
    pub retry_count: u8,
    pub elapsed_seconds: u64,
    pub max_iterations: u32,
    pub drift_status: String,
    pub governance_stats: GovernanceStats,
    pub transitions: Vec<StateTransition>,
    pub recent_events: Vec<RuntimeEvent>,
    pub selected_proposal: usize,
    pub should_quit: bool,
    pub auto_refresh: bool,
    pub last_refresh: Instant,
    pub refresh_interval: Duration,
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
        let (loop_stage, iteration, retries, elapsed, max_iter) = load_loop_details(project_root);
        let drift_status = "OK".to_string();
        let governance_stats = load_governance_stats(project_root)?;
        let transitions = load_transitions(project_root);
        let recent_events = load_recent_events(project_root, 30);

        Ok(Self {
            project_root: project_root.display().to_string(),
            proposals,
            loop_stage,
            loop_iteration: iteration,
            retry_count: retries,
            elapsed_seconds: elapsed,
            max_iterations: max_iter,
            drift_status,
            governance_stats,
            transitions,
            recent_events,
            selected_proposal: 0,
            should_quit: false,
            auto_refresh: false,
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_secs(5),
        })
    }

    pub fn tick(&mut self) {
        let project_path = std::path::PathBuf::from(&self.project_root);
        if let Ok(proposals) = load_proposal_summaries(&project_path) {
            self.proposals = proposals;
        }
        let (stage, iter, retries, elapsed, max_iter) = load_loop_details(&project_path);
        self.loop_stage = stage;
        self.loop_iteration = iter;
        self.retry_count = retries;
        self.elapsed_seconds = elapsed;
        self.max_iterations = max_iter;
        if let Ok(stats) = load_governance_stats(&project_path) {
            self.governance_stats = stats;
        }
        self.transitions = load_transitions(&project_path);
        self.recent_events = load_recent_events(&project_path, 30);
    }
}

fn load_proposal_summaries(project_root: &Path) -> Result<Vec<ProposalSummary>> {
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
                let completed = proposal
                    .tasks
                    .iter()
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

    proposals.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(proposals)
}

fn load_loop_details(project_root: &Path) -> (String, u32, u8, u64, u32) {
    use zn_spec::load_loop_state;

    match load_loop_state(project_root) {
        Ok(Some(state)) => (
            format!("{:?}", state.stage),
            state.iteration,
            state.retry_count,
            state.elapsed_seconds,
            state.max_iterations.unwrap_or(50),
        ),
        Ok(None) => ("Idle".to_string(), 0, 0, 0, 50),
        Err(_) => ("Unknown".to_string(), 0, 0, 0, 50),
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

fn load_transitions(project_root: &Path) -> Vec<StateTransition> {
    let path = project_root.join(".zero_nine/loop/transitions.ndjson");
    if !path.exists() {
        return Vec::new();
    }
    fs::read_to_string(&path)
        .ok()
        .map(|content| {
            content
                .lines()
                .filter_map(|line| serde_json::from_str::<StateTransition>(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn load_recent_events(project_root: &Path, limit: usize) -> Vec<RuntimeEvent> {
    let path = project_root.join(".zero_nine/runtime/events.ndjson");
    if !path.exists() {
        return Vec::new();
    }
    fs::read_to_string(&path)
        .ok()
        .map(|content| {
            content
                .lines()
                .rev()
                .take(limit)
                .filter_map(|line| serde_json::from_str::<RuntimeEvent>(line).ok())
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        })
        .unwrap_or_default()
}

fn format_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

pub fn run_dashboard(project_root: &Path) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = DashboardApp::new(project_root)?;

    let res = run_ui(&mut terminal, &mut app);

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
                    KeyCode::Char('a') => app.auto_refresh = !app.auto_refresh,
                    _ => {}
                }
            }
        }

        if app.auto_refresh && app.last_refresh.elapsed() >= app.refresh_interval {
            app.tick();
            app.last_refresh = Instant::now();
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
            Constraint::Length(3),  // Title
            Constraint::Length(8),  // Key Metrics
            Constraint::Length(12), // State Timeline
            Constraint::Length(12), // Event Stream
            Constraint::Length(10), // Proposals table
            Constraint::Min(0),     // Details + Governance
        ])
        .split(f.area());

    // Title bar
    let refresh_indicator = if app.auto_refresh { " [auto]" } else { "" };
    let title = Paragraph::new(format!(
        " Zero_Nine Dashboard - {}{}",
        app.project_root, refresh_indicator
    ))
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Key Metrics
    let budget_iter = app.max_iterations;
    let metrics_text = format!(
        "Loop Stage:    {} (iteration {})\nTasks:         {}/{} completed  |  Retries: {}\nElapsed:       {}  |  Budget: {}/{} iterations\nDrift:         {}",
        app.loop_stage,
        app.loop_iteration,
        app.proposals.iter().map(|p| p.completed_tasks).sum::<usize>(),
        app.proposals.iter().map(|p| p.task_count).sum::<usize>(),
        app.retry_count,
        format_duration(app.elapsed_seconds),
        app.loop_iteration,
        budget_iter,
        app.drift_status,
    );
    let metrics_color = match app.loop_stage.as_str() {
        "RunningTask" => Color::Green,
        "Verifying" => Color::Yellow,
        "Escalated" | "Retrying" => Color::Red,
        "Completed" => Color::Cyan,
        _ => Color::White,
    };
    let metrics = Paragraph::new(metrics_text)
        .style(Style::default().fg(metrics_color))
        .block(
            Block::default()
                .title(" Key Metrics ")
                .borders(Borders::ALL),
        );
    f.render_widget(metrics, chunks[1]);

    // State Timeline
    let timeline_lines: Vec<String> = app
        .transitions
        .iter()
        .rev()
        .take(15)
        .map(|t| {
            let ts = t.triggered_at.format("%H:%M:%S");
            let task_info = t
                .task_id
                .as_ref()
                .map(|id| format!(" ({})", id))
                .unwrap_or_default();
            let marker = if t.to == "Escalated" { "!! " } else { "   " };
            format!(
                "{}{}[{}] → [{}]{} {}",
                marker, ts, t.from, t.to, task_info, t.reason
            )
        })
        .collect();

    let timeline_text = if timeline_lines.is_empty() {
        "No state transitions recorded.".to_string()
    } else {
        timeline_lines.join("\n")
    };

    let timeline = Paragraph::new(timeline_text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(" State Timeline (recent 15) ")
                .borders(Borders::ALL),
        );
    f.render_widget(timeline, chunks[2]);

    // Event Stream
    let event_lines: Vec<String> = app
        .recent_events
        .iter()
        .rev()
        .take(15)
        .map(|e| {
            let ts = e.ts.format("%H:%M:%S");
            let ctx = e
                .task_id
                .as_ref()
                .map(|id| format!(" {}", id))
                .unwrap_or_default();
            format!("[{}] {}{}", ts, e.event, ctx)
        })
        .collect();

    let event_text = if event_lines.is_empty() {
        "No runtime events recorded.".to_string()
    } else {
        event_lines.join("\n")
    };

    let events = Paragraph::new(event_text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(" Event Stream (recent 15) ")
                .borders(Borders::ALL),
        );
    f.render_widget(events, chunks[3]);

    // Proposals table with color-coded rows
    let header = Row::new(vec!["ID", "Title", "Status", "Progress", "Created"]).style(
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .proposals
        .iter()
        .map(|p| {
            let progress = format!("{}/{}", p.completed_tasks, p.task_count);
            let status_str = format!("{:?}", p.status);
            let created = p.created_at.format("%Y-%m-%d %H:%M").to_string();
            let row_color = match p.status {
                ProposalStatus::Completed => Color::Green,
                ProposalStatus::Running => Color::Yellow,
                ProposalStatus::Draft | ProposalStatus::Ready => Color::White,
                ProposalStatus::Archived => Color::DarkGray,
            };
            Row::new(vec![
                p.id.clone(),
                p.title.clone(),
                status_str,
                progress,
                created,
            ])
            .style(Style::default().fg(row_color))
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(20),
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(15),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" Proposals (↑/↓ nav, r refresh, a auto-refresh) ")
            .borders(Borders::ALL),
    );

    f.render_widget(table, chunks[4]);

    // Governance + Selected proposal details
    let gov_text = format!(
        "Pending: {}  |  Total: {}  |  Approved: {}  |  Rejected: {}",
        app.governance_stats.pending_approvals,
        app.governance_stats.total_tickets,
        app.governance_stats.approved_count,
        app.governance_stats.rejected_count
    );

    let details_text = if let Some(selected) = app.proposals.get(app.selected_proposal) {
        format!(
            "{}\n\nSelected: {}\n\nGoal: {}",
            gov_text, selected.title, selected.goal
        )
    } else {
        gov_text
    };

    let details = Paragraph::new(details_text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .title(" Governance + Details ")
                .borders(Borders::ALL),
        );
    f.render_widget(details, chunks[5]);
}
