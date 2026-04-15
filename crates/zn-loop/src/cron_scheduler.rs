//! Cron Scheduler - Scheduled and recurring tasks for Zero_Nine
//!
//! This module provides:
//! - Cron-based scheduling for Zero_Nine workflows
//! - Recurring jobs (daily, weekly, etc.)
//! - One-shot reminders and delayed tasks
//! - Job persistence and recovery

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Cron job definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    /// Unique job ID
    pub id: String,
    /// Cron expression (5 fields: minute hour day-of-month month day-of-week)
    pub cron: String,
    /// Job description
    pub description: String,
    /// Job type
    pub job_type: JobType,
    /// Whether job is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Last run timestamp (ISO 8601)
    pub last_run: Option<String>,
    /// Next run timestamp (ISO 8601)
    pub next_run: Option<String>,
    /// Total run count
    #[serde(default)]
    pub run_count: u64,
    /// Job payload
    #[serde(default)]
    pub payload: Value,
}

fn default_true() -> bool {
    true
}

/// Job type - recurring or one-shot
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobType {
    /// Recurring job that fires on every cron match
    Recurring {
        /// Auto-delete after N days (default 7)
        expire_after_days: Option<u32>,
    },
    /// One-shot job that fires once then auto-deletes
    OneShot {
        /// Scheduled run time (ISO 8601)
        scheduled_at: String,
    },
}

/// Cron scheduler state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchedulerState {
    /// Registered jobs
    #[serde(default)]
    pub jobs: HashMap<String, CronJob>,
    /// Execution history (last N entries)
    #[serde(default)]
    pub history: Vec<ExecutionRecord>,
}

/// Execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Job ID
    pub job_id: String,
    /// Execution timestamp
    pub executed_at: String,
    /// Success or failure
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: Option<u64>,
}

/// Cron scheduler for Zero_Nine
pub struct CronScheduler {
    project_root: PathBuf,
    state: SchedulerState,
    state_path: PathBuf,
}

impl CronScheduler {
    /// Create a new cron scheduler
    pub fn new(project_root: &Path) -> Result<Self> {
        let state_path = project_root.join(".zero_nine/cron/scheduler_state.json");
        let mut scheduler = Self {
            project_root: project_root.to_path_buf(),
            state: SchedulerState::default(),
            state_path,
        };
        scheduler.load_state()?;
        Ok(scheduler)
    }

    /// Load scheduler state from disk
    pub fn load_state(&mut self) -> Result<()> {
        if !self.state_path.exists() {
            debug!("Cron state file not found, starting fresh");
            return Ok(());
        }

        let content = fs::read_to_string(&self.state_path)
            .with_context(|| format!("Failed to read cron state: {}", self.state_path.display()))?;

        self.state = serde_json::from_str(&content)
            .with_context(|| "Failed to parse cron state JSON")?;

        debug!("Loaded {} cron jobs", self.state.jobs.len());
        Ok(())
    }

    /// Save scheduler state to disk
    pub fn save_state(&self) -> Result<()> {
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self.state)?;
        fs::write(&self.state_path, content)?;
        Ok(())
    }

    /// Schedule a new job
    pub fn schedule(&mut self, job: CronJob) -> Result<()> {
        let job_id = job.id.clone();
        self.state.jobs.insert(job_id.clone(), job);
        self.save_state()?;
        info!("Scheduled cron job: {}", job_id);
        Ok(())
    }

    /// Cancel a job
    pub fn cancel(&mut self, job_id: &str) -> Result<bool> {
        let removed = self.state.jobs.remove(job_id).is_some();
        if removed {
            self.save_state()?;
            info!("Cancelled cron job: {}", job_id);
        }
        Ok(removed)
    }

    /// List all jobs
    pub fn list_jobs(&self) -> Vec<&CronJob> {
        self.state.jobs.values().collect()
    }

    /// Get a specific job
    pub fn get_job(&self, job_id: &str) -> Option<&CronJob> {
        self.state.jobs.get(job_id)
    }

    /// Enable/disable a job
    pub fn toggle_job(&mut self, job_id: &str, enabled: bool) -> Result<bool> {
        if let Some(job) = self.state.jobs.get_mut(job_id) {
            job.enabled = enabled;
            self.save_state()?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Get next run time for a job
    pub fn next_run_time(&self, job_id: &str) -> Option<DateTime<Local>> {
        let job = self.state.jobs.get(job_id)?;
        parse_cron_next(&job.cron)
    }

    /// Run pending jobs and return jobs that should be executed
    pub fn get_pending_jobs(&self) -> Vec<CronJob> {
        let now = Local::now();
        let mut pending = Vec::new();

        for job in self.state.jobs.values() {
            if !job.enabled {
                continue;
            }

            let should_run = match &job.job_type {
                JobType::Recurring { .. } => {
                    // Check if cron matches current time
                    cron_matches(&job.cron, now)
                }
                JobType::OneShot { scheduled_at } => {
                    // Check if scheduled time has passed
                    if let Ok(scheduled) = DateTime::parse_from_rfc3339(scheduled_at) {
                        now >= scheduled
                    } else {
                        false
                    }
                }
            };

            if should_run {
                // Check if already run this cycle (for recurring jobs)
                if let Some(last_run) = &job.last_run {
                    if let Ok(last) = DateTime::parse_from_rfc3339(last_run) {
                        let interval = match &job.job_type {
                            JobType::Recurring { .. } => {
                                // Don't re-run within the same minute
                                now.signed_duration_since(last).num_seconds() < 60
                            }
                            JobType::OneShot { .. } => true, // One-shot already handled
                        };
                        if interval {
                            continue;
                        }
                    }
                }
                pending.push(job.clone());
            }
        }

        pending
    }

    /// Record job execution
    pub fn record_execution(&mut self, job_id: &str, success: bool, error: Option<String>, duration_ms: Option<u64>) -> Result<()> {
        if let Some(job) = self.state.jobs.get_mut(job_id) {
            job.last_run = Some(Local::now().to_rfc3339());
            job.run_count += 1;

            // Update next run for recurring jobs
            if let JobType::Recurring { expire_after_days } = &job.job_type {
                // Calculate next run separately to avoid borrow issues
                let next_run = parse_cron_next(&job.cron).map(|t| t.to_rfc3339());
                job.next_run = next_run;

                // Check expiration
                if let Some(days) = expire_after_days {
                    if let Some(last_run) = &job.last_run {
                        if let Ok(last) = DateTime::parse_from_rfc3339(last_run) {
                            let age = Local::now().signed_duration_since(last).num_days();
                            if age >= *days as i64 {
                                job.enabled = false;
                            }
                        }
                    }
                }
            }

            // Add to history
            let record = ExecutionRecord {
                job_id: job_id.to_string(),
                executed_at: Local::now().to_rfc3339(),
                success,
                error,
                duration_ms,
            };
            self.state.history.push(record);

            // Keep only last 100 entries
            if self.state.history.len() > 100 {
                self.state.history.remove(0);
            }

            self.save_state()?;
        }
        Ok(())
    }

    /// Clean up expired one-shot jobs
    pub fn cleanup_expired(&mut self) -> Result<usize> {
        let mut removed = 0;
        let expired_ids: Vec<String> = self.state.jobs.iter()
            .filter(|(_, job)| {
                if !job.enabled {
                    return true;
                }
                if let JobType::OneShot { .. } = &job.job_type {
                    return job.last_run.is_some();
                }
                false
            })
            .map(|(id, _)| id.clone())
            .collect();

        for id in expired_ids {
            self.state.jobs.remove(&id);
            removed += 1;
        }

        if removed > 0 {
            self.save_state()?;
            info!("Cleaned up {} expired cron jobs", removed);
        }
        Ok(removed)
    }

    /// Get scheduler statistics
    pub fn get_stats(&self) -> CronStats {
        let total = self.state.jobs.len();
        let enabled = self.state.jobs.values().filter(|j| j.enabled).count();
        let disabled = total - enabled;
        let recurring = self.state.jobs.values()
            .filter(|j| matches!(j.job_type, JobType::Recurring { .. }))
            .count();
        let one_shot = self.state.jobs.values()
            .filter(|j| matches!(j.job_type, JobType::OneShot { .. }))
            .count();
        let total_runs: u64 = self.state.jobs.values().map(|j| j.run_count).sum();
        let failed_runs = self.state.history.iter().filter(|r| !r.success).count();

        CronStats {
            total,
            enabled,
            disabled,
            recurring,
            one_shot,
            total_runs,
            failed_runs,
        }
    }
}

/// Cron statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronStats {
    pub total: usize,
    pub enabled: usize,
    pub disabled: usize,
    pub recurring: usize,
    pub one_shot: usize,
    pub total_runs: u64,
    pub failed_runs: usize,
}

/// Parse cron expression and get next run time
fn parse_cron_next(cron: &str) -> Option<DateTime<Local>> {
    let parts: Vec<&str> = cron.split_whitespace().collect();
    if parts.len() != 5 {
        return None;
    }

    let now = Local::now();
    let mut next = now;

    // Simple implementation - in production, use cron crate
    for _ in 0..366 * 24 * 60 {
        next = next + chrono::Duration::minutes(1);
        if cron_matches(cron, next) {
            return Some(next);
        }
    }
    None
}

/// Check if current time matches cron expression
fn cron_matches(cron: &str, time: DateTime<Local>) -> bool {
    let parts: Vec<&str> = cron.split_whitespace().collect();
    if parts.len() != 5 {
        return false;
    }

    let minute = time.minute() as u32;
    let hour = time.hour() as u32;
    let day = time.day() as u32;
    let month = time.month() as u32;
    let weekday = time.weekday().num_days_from_sunday();

    matches_field(parts[0], minute, 0, 59)
        && matches_field(parts[1], hour, 0, 23)
        && matches_field(parts[2], day, 1, 31)
        && matches_field(parts[3], month, 1, 12)
        && matches_field(parts[4], weekday, 0, 6)
}

/// Check if a value matches a cron field
fn matches_field(field: &str, value: u32, _min: u32, _max: u32) -> bool {
    if field == "*" {
        return true;
    }

    // Handle */N (every N)
    if field.starts_with("*/") {
        if let Ok(step) = field[2..].parse::<u32>() {
            return step > 0 && value % step == 0;
        }
    }

    // Handle N-M (range)
    if let Some(dash_pos) = field.find('-') {
        if let (Ok(start), Ok(end)) = (
            field[..dash_pos].parse::<u32>(),
            field[dash_pos + 1..].parse::<u32>(),
        ) {
            return value >= start && value <= end;
        }
    }

    // Handle N,M,O (list)
    if field.contains(',') {
        for part in field.split(',') {
            if let Ok(n) = part.parse::<u32>() {
                if n == value {
                    return true;
                }
            }
        }
        return false;
    }

    // Single value
    if let Ok(n) = field.parse::<u32>() {
        return n == value;
    }

    false
}

/// Create a recurring cron job
pub fn create_recurring_job(
    id: &str,
    cron: &str,
    description: &str,
    payload: Value,
    expire_after_days: Option<u32>,
) -> CronJob {
    let next_run = parse_cron_next(cron).map(|t| t.to_rfc3339());
    CronJob {
        id: id.to_string(),
        cron: cron.to_string(),
        description: description.to_string(),
        job_type: JobType::Recurring { expire_after_days },
        enabled: true,
        last_run: None,
        next_run,
        run_count: 0,
        payload,
    }
}

/// Create a one-shot job
pub fn create_one_shot_job(
    id: &str,
    scheduled_at: DateTime<Local>,
    description: &str,
    payload: Value,
) -> CronJob {
    CronJob {
        id: id.to_string(),
        cron: "0 0 * * *".to_string(), // Placeholder
        description: description.to_string(),
        job_type: JobType::OneShot {
            scheduled_at: scheduled_at.to_rfc3339(),
        },
        enabled: true,
        last_run: None,
        next_run: Some(scheduled_at.to_rfc3339()),
        run_count: 0,
        payload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_cron_matches() {
        let time = Local::now();

        // Every minute
        assert!(cron_matches("* * * * *", time));

        // Specific minute
        let specific = format!("{} * * * *", time.minute());
        assert!(cron_matches(&specific, time));

        // Wrong minute
        let wrong = format!("{} * * * *", (time.minute() + 1) % 60);
        assert!(!cron_matches(&wrong, time));
    }

    #[test]
    fn test_matches_field() {
        assert!(matches_field("*", 15, 0, 59));
        assert!(matches_field("15", 15, 0, 59));
        assert!(!matches_field("14", 15, 0, 59));
        assert!(matches_field("*/5", 15, 0, 59));
        assert!(matches_field("*/5", 10, 0, 59));
        assert!(!matches_field("*/5", 12, 0, 59));
        assert!(matches_field("10-20", 15, 0, 59));
        assert!(!matches_field("10-20", 25, 0, 59));
        assert!(matches_field("10,15,20", 15, 0, 59));
        assert!(!matches_field("10,15,20", 12, 0, 59));
    }

    #[test]
    fn test_create_recurring_job() {
        let job = create_recurring_job(
            "test-daily",
            "0 9 * * *",
            "Daily 9am task",
            serde_json::json!({"action": "run"}),
            Some(7),
        );
        assert_eq!(job.id, "test-daily");
        assert_eq!(job.cron, "0 9 * * *");
        assert!(job.enabled);
        assert!(job.next_run.is_some());
    }

    #[test]
    fn test_create_one_shot_job() {
        let scheduled = Local::now() + chrono::Duration::hours(1);
        let job = create_one_shot_job(
            "test-oneshot",
            scheduled,
            "One-time reminder",
            serde_json::json!({"reminder": "test"}),
        );
        assert_eq!(job.id, "test-oneshot");
        assert!(matches!(job.job_type, JobType::OneShot { .. }));
    }

    #[test]
    fn test_scheduler_lifecycle() {
        let tmp_dir = temp_dir().join("zn_cron_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let mut scheduler = CronScheduler::new(&tmp_dir).unwrap();

        let job = create_recurring_job(
            "test-job",
            "0 9 * * *",
            "Test job",
            serde_json::json!({}),
            None,
        );
        scheduler.schedule(job).unwrap();

        assert_eq!(scheduler.list_jobs().len(), 1);
        assert!(scheduler.get_job("test-job").is_some());

        scheduler.cancel("test-job").unwrap();
        assert_eq!(scheduler.list_jobs().len(), 0);

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
