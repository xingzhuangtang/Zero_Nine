//! Layer 13: Cross-Cutting Observability
//!
//! This module provides:
//! - Structured event emission with trace context
//! - Metrics aggregation and latency tracking
//! - Query interface for historical analysis
//! - Span-based distributed tracing

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use zn_types::{MetricsSnapshot, RuntimeEvent, TraceContext};

/// Event emitter for structured observability
pub struct EventEmitter {
    events_file: PathBuf,
    trace_context: Option<TraceContext>,
}

impl EventEmitter {
    /// Create a new EventEmitter
    pub fn new(events_file: PathBuf) -> Result<Self> {
        if let Some(parent) = events_file.parent() {
            fs::create_dir_all(parent)?;
        }

        Ok(Self {
            events_file,
            trace_context: None,
        })
    }

    /// Set the current trace context
    pub fn set_trace_context(&mut self, ctx: TraceContext) {
        self.trace_context = Some(ctx);
    }

    /// Get the current trace context
    pub fn trace_context(&self) -> Option<&TraceContext> {
        self.trace_context.as_ref()
    }

    /// Emit a structured event
    pub fn emit(&self, event_type: &str, payload: Option<serde_json::Value>) -> Result<()> {
        let now = Utc::now();

        let event = RuntimeEvent {
            ts: now,
            event: event_type.to_string(),
            proposal_id: None,
            task_id: None,
            payload,
            trace_id: self.trace_context.as_ref().map(|c| c.trace_id.clone()),
            span_id: self.trace_context.as_ref().map(|c| c.span_id.clone()),
            parent_span_id: self
                .trace_context
                .as_ref()
                .and_then(|c| c.parent_span_id.clone()),
            latency_ms: None,
            metadata: None,
        };

        self.append_event(&event)
    }

    /// Emit an event with proposal/task context
    pub fn emit_with_context(
        &self,
        event_type: &str,
        proposal_id: Option<&str>,
        task_id: Option<&str>,
        payload: Option<serde_json::Value>,
    ) -> Result<()> {
        let now = Utc::now();

        let event = RuntimeEvent {
            ts: now,
            event: event_type.to_string(),
            proposal_id: proposal_id.map(|s| s.to_string()),
            task_id: task_id.map(|s| s.to_string()),
            payload,
            trace_id: self.trace_context.as_ref().map(|c| c.trace_id.clone()),
            span_id: self.trace_context.as_ref().map(|c| c.span_id.clone()),
            parent_span_id: self
                .trace_context
                .as_ref()
                .and_then(|c| c.parent_span_id.clone()),
            latency_ms: None,
            metadata: None,
        };

        self.append_event(&event)
    }

    /// Emit a span completion event with latency
    pub fn emit_span_complete(
        &self,
        event_type: &str,
        latency_ms: u64,
        success: bool,
    ) -> Result<()> {
        let now = Utc::now();
        let metadata = serde_json::json!({
            "success": success,
            "latency_ms": latency_ms,
        });

        let event = RuntimeEvent {
            ts: now,
            event: event_type.to_string(),
            proposal_id: None,
            task_id: None,
            payload: Some(metadata),
            trace_id: self.trace_context.as_ref().map(|c| c.trace_id.clone()),
            span_id: self.trace_context.as_ref().map(|c| c.span_id.clone()),
            parent_span_id: self
                .trace_context
                .as_ref()
                .and_then(|c| c.parent_span_id.clone()),
            latency_ms: Some(latency_ms),
            metadata: None,
        };

        self.append_event(&event)
    }

    fn append_event(&self, event: &RuntimeEvent) -> Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.events_file)
            .with_context(|| {
                format!("Failed to open events file: {}", self.events_file.display())
            })?;

        let mut writer = std::io::BufWriter::new(file);
        let line = serde_json::to_string(event)?;
        writeln!(writer, "{}", line)?;
        writer.flush()?;

        Ok(())
    }

    /// Create a child span for nested operations
    pub fn child_span(&self, span_name: &str) -> TraceContext {
        if let Some(ctx) = &self.trace_context {
            ctx.child(span_name)
        } else {
            let mut ctx = TraceContext::new();
            ctx.span_id = format!("{}-{}", &ctx.span_id[..8], span_name);
            ctx
        }
    }

    /// Start a new trace for a proposal
    pub fn start_proposal_trace(&mut self, proposal_id: &str) -> TraceContext {
        let mut ctx = TraceContext::new();
        ctx.attributes
            .insert("proposal_id".to_string(), proposal_id.to_string());
        self.trace_context = Some(ctx.clone());
        ctx
    }

    /// Start a new trace for a task
    pub fn start_task_trace(&mut self, proposal_id: &str, task_id: &str) -> TraceContext {
        let mut ctx = TraceContext::new();
        ctx.attributes
            .insert("proposal_id".to_string(), proposal_id.to_string());
        ctx.attributes
            .insert("task_id".to_string(), task_id.to_string());
        self.trace_context = Some(ctx.clone());
        ctx
    }
}

/// Metrics aggregator for latency and throughput analysis
pub struct MetricsAggregator {
    metrics_file: PathBuf,
    snapshots: Vec<MetricsSnapshot>,
}

impl MetricsAggregator {
    /// Create a new MetricsAggregator
    pub fn new(metrics_file: PathBuf) -> Result<Self> {
        let mut aggregator = Self {
            metrics_file,
            snapshots: Vec::new(),
        };
        aggregator.load_existing_metrics()?;
        Ok(aggregator)
    }

    /// Load existing metrics from file
    fn load_existing_metrics(&mut self) -> Result<()> {
        if !self.metrics_file.exists() {
            return Ok(());
        }

        let file = fs::File::open(&self.metrics_file)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(snapshot) = serde_json::from_str::<MetricsSnapshot>(&line) {
                self.snapshots.push(snapshot);
            }
        }

        Ok(())
    }

    /// Record a metrics snapshot
    pub fn record(&mut self, snapshot: MetricsSnapshot) -> Result<()> {
        self.snapshots.push(snapshot.clone());
        self.save_snapshot(&snapshot)
    }

    fn save_snapshot(&self, snapshot: &MetricsSnapshot) -> Result<()> {
        if let Some(parent) = self.metrics_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.metrics_file)?;

        let mut writer = std::io::BufWriter::new(file);
        let line = serde_json::to_string(snapshot)?;
        writeln!(writer, "{}", line)?;
        writer.flush()?;

        Ok(())
    }

    /// Get latency statistics for a task
    pub fn get_latency_stats(&self, task_id: Option<&str>) -> LatencyStats {
        let filtered: Vec<_> = self
            .snapshots
            .iter()
            .filter(|s| task_id.map_or(true, |t| s.task_id == t))
            .collect();

        if filtered.is_empty() {
            return LatencyStats::default();
        }

        let latencies: Vec<u64> = filtered.iter().map(|s| s.latency_ms).collect();
        let count = latencies.len() as u64;
        let sum: u64 = latencies.iter().sum();
        let min = *latencies.iter().min().unwrap_or(&0);
        let max = *latencies.iter().max().unwrap_or(&0);
        let avg = if count > 0 { sum / count } else { 0 };

        // Calculate p95
        let mut sorted = latencies.clone();
        sorted.sort();
        let p95_idx = (sorted.len() as f64 * 0.95) as usize;
        let p95 = sorted.get(p95_idx).copied().unwrap_or(max);

        LatencyStats {
            count,
            min,
            max,
            avg,
            p95,
        }
    }

    /// Get success rate for a proposal
    pub fn get_success_rate(&self, proposal_id: Option<&str>) -> f32 {
        let filtered: Vec<_> = self
            .snapshots
            .iter()
            .filter(|s| {
                proposal_id.map_or(true, |p| s.proposal_id.as_ref().map_or(true, |sp| sp == p))
            })
            .collect();

        if filtered.is_empty() {
            return 0.0;
        }

        let success_count = filtered.iter().filter(|s| s.success).count() as f32;
        success_count / filtered.len() as f32
    }

    /// Get all unique task IDs
    pub fn get_all_task_ids(&self) -> Vec<String> {
        let mut ids: HashSet<_> = self.snapshots.iter().map(|s| s.task_id.clone()).collect();
        let mut vec: Vec<_> = ids.drain().collect();
        vec.sort();
        vec
    }

    /// Get recent metrics
    pub fn get_recent(&self, limit: usize) -> &[MetricsSnapshot] {
        let start = self.snapshots.len().saturating_sub(limit);
        &self.snapshots[start..]
    }
}

/// Latency statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LatencyStats {
    pub count: u64,
    pub min: u64,
    pub max: u64,
    pub avg: u64,
    pub p95: u64,
}

/// Query interface for historical analysis
pub struct EventQuery {
    events_file: PathBuf,
}

impl EventQuery {
    /// Create a new EventQuery
    pub fn new(events_file: PathBuf) -> Result<Self> {
        Ok(Self { events_file })
    }

    /// Query events by type
    pub fn query_by_type(&self, event_type: &str, limit: usize) -> Result<Vec<RuntimeEvent>> {
        if !self.events_file.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&self.events_file)?;
        let reader = BufReader::new(file);
        let mut results = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<RuntimeEvent>(&line) {
                if event.event == event_type {
                    results.push(event);
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(results)
    }

    /// Query events by proposal ID
    pub fn query_by_proposal(&self, proposal_id: &str, limit: usize) -> Result<Vec<RuntimeEvent>> {
        if !self.events_file.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&self.events_file)?;
        let reader = BufReader::new(file);
        let mut results = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<RuntimeEvent>(&line) {
                if event
                    .proposal_id
                    .as_ref()
                    .map_or(false, |p| p == proposal_id)
                {
                    results.push(event);
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(results)
    }

    /// Query events by trace ID
    pub fn query_by_trace(&self, trace_id: &str, limit: usize) -> Result<Vec<RuntimeEvent>> {
        if !self.events_file.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&self.events_file)?;
        let reader = BufReader::new(file);
        let mut results = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<RuntimeEvent>(&line) {
                if event.trace_id.as_ref().map_or(false, |t| t == trace_id) {
                    results.push(event);
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(results)
    }

    /// Replay a trace as a span tree
    pub fn replay_trace(&self, trace_id: &str) -> Result<TraceTree> {
        let events = self.query_by_trace(trace_id, 1000)?;

        let mut tree = TraceTree {
            trace_id: trace_id.to_string(),
            root_spans: Vec::new(),
            all_spans: HashMap::new(),
        };

        // Build span tree
        for event in &events {
            let span_id = event
                .span_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let parent_id = event.parent_span_id.clone();

            let span = TraceSpan {
                span_id: span_id.clone(),
                parent_span_id: parent_id.clone(),
                event_type: event.event.clone(),
                timestamp: event.ts,
                latency_ms: event.latency_ms,
                proposal_id: event.proposal_id.clone(),
                task_id: event.task_id.clone(),
                child_span_ids: Vec::new(),
            };

            tree.all_spans.insert(span_id.clone(), span.clone());

            if let Some(parent) = parent_id {
                if let Some(parent_span) = tree.all_spans.get_mut(&parent) {
                    parent_span.child_span_ids.push(span_id);
                }
            } else {
                tree.root_spans.push(span_id);
            }
        }

        Ok(tree)
    }
}

/// Trace tree for visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceTree {
    pub trace_id: String,
    pub root_spans: Vec<String>,
    pub all_spans: HashMap<String, TraceSpan>,
}

/// Individual span in a trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub latency_ms: Option<u64>,
    pub proposal_id: Option<String>,
    pub task_id: Option<String>,
    #[serde(default)]
    pub child_span_ids: Vec<String>,
}

/// Create default observability components for a project
pub fn create_default_observability(
    project_root: &Path,
) -> Result<(EventEmitter, MetricsAggregator, EventQuery)> {
    let events_file = project_root
        .join(".zero_nine")
        .join("runtime")
        .join("events.ndjson");

    let metrics_file = project_root
        .join(".zero_nine")
        .join("runtime")
        .join("metrics.ndjson");

    let emitter = EventEmitter::new(events_file.clone())?;
    let aggregator = MetricsAggregator::new(metrics_file.clone())?;
    let query = EventQuery::new(events_file)?;

    Ok((emitter, aggregator, query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_trace_context() {
        let ctx = TraceContext::new();
        assert!(!ctx.trace_id.is_empty());
        assert!(!ctx.span_id.is_empty());
        assert!(ctx.parent_span_id.is_none());

        let child = ctx.child("002");
        assert_eq!(child.trace_id, ctx.trace_id);
        assert!(child.parent_span_id.is_some());
    }

    #[test]
    fn test_event_emitter() {
        let tmp_file = temp_dir().join("test_events.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut emitter = EventEmitter::new(tmp_file.clone()).unwrap();
        emitter
            .emit("test_event", Some(serde_json::json!({"key": "value"})))
            .unwrap();

        // Verify event was written
        let query = EventQuery::new(tmp_file.clone()).unwrap();
        let events = query.query_by_type("test_event", 10).unwrap();
        assert_eq!(events.len(), 1);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_metrics_aggregator() {
        let tmp_file = temp_dir().join("test_metrics.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut aggregator = MetricsAggregator::new(tmp_file.clone()).unwrap();

        // Record some metrics
        for i in 0..10 {
            let snapshot = MetricsSnapshot {
                task_id: format!("task-{}", i % 3),
                proposal_id: Some("prop-1".to_string()),
                start_ts: Utc::now(),
                end_ts: None,
                latency_ms: 100 + i * 10,
                token_usage: 500,
                subagent_count: 2,
                evidence_count: 3,
                success: i % 2 == 0,
                custom_metrics: HashMap::new(),
            };
            aggregator.record(snapshot).unwrap();
        }

        // Check latency stats
        let stats = aggregator.get_latency_stats(Some("task-1"));
        assert!(stats.count > 0);
        assert!(stats.avg > 0);

        // Check success rate
        let rate = aggregator.get_success_rate(Some("prop-1"));
        assert!(rate > 0.0 && rate <= 1.0);

        let _ = fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_trace_propagation() {
        let tmp_file = temp_dir().join("test_trace.ndjson");
        let _ = fs::remove_file(&tmp_file);

        let mut emitter = EventEmitter::new(tmp_file.clone()).unwrap();

        // Start a proposal trace
        let trace_ctx = emitter.start_proposal_trace("test-proposal");
        let span1 = emitter.child_span("spec_capture");

        // Emit events with trace context
        emitter.set_trace_context(trace_ctx);
        emitter.emit("proposal_created", None).unwrap();

        emitter.set_trace_context(span1);
        emitter.emit("spec_captured", None).unwrap();

        // Query by trace
        let query = EventQuery::new(tmp_file.clone()).unwrap();
        let events = query
            .query_by_trace(&emitter.trace_context().unwrap().trace_id, 10)
            .unwrap();
        assert!(!events.is_empty());

        let _ = fs::remove_file(&tmp_file);
    }
}
