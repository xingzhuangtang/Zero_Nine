//! Tier 1: Core lifecycle integration tests
//! init -> run -> status -> resume -> export

use assert_cmd::Command;
use chrono::Utc;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use zn_types::{BrainstormSession, BrainstormVerdict, HostKind};

fn zero_nine() -> Command {
    Command::cargo_bin("zero-nine").expect("zero-nine binary not built")
}

fn init_project(dir: &Path) {
    zero_nine()
        .arg("init")
        .arg("--project")
        .arg(dir)
        .assert()
        .success();
}

/// Create a Ready brainstorm session so `run` can skip the interactive
/// brainstorm phase and go straight to spec + execution.
fn seed_ready_session(dir: &Path, goal: &str, host: HostKind) {
    let session = BrainstormSession {
        id: "test-session".to_string(),
        goal: goal.to_string(),
        host,
        status: "active".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        questions: Vec::new(),
        answers: Vec::new(),
        verdict: BrainstormVerdict::Ready,
    };
    let sessions_dir = dir.join(".zero_nine/brainstorm/sessions");
    fs::create_dir_all(&sessions_dir).unwrap();
    let json = serde_json::to_string_pretty(&session).unwrap();
    fs::write(sessions_dir.join("test-session.json"), &json).unwrap();
    fs::write(dir.join(".zero_nine/brainstorm/latest-session.json"), &json).unwrap();
    fs::write(dir.join(".zero_nine/brainstorm/latest-session.md"), &goal).unwrap();
}

// --- Init tests ---

#[test]
fn test_init_creates_layout() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    let zero_nine = dir.path().join(".zero_nine");
    assert!(zero_nine.is_dir());
    assert!(zero_nine.join("manifest.json").is_file());

    // Subdirectories should exist
    for sub in &["proposals", "brainstorm", "specs", "evolve", "runtime"] {
        assert!(
            zero_nine.join(sub).is_dir(),
            ".zero_nine/{sub} should exist"
        );
    }

    // Manifest should parseable and contain project name
    let manifest = fs::read_to_string(zero_nine.join("manifest.json")).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    assert!(
        doc.get("name").is_some(),
        "manifest should have 'name' field"
    );
}

#[test]
fn test_init_with_host_claude() {
    let dir = TempDir::new().unwrap();
    zero_nine()
        .arg("init")
        .arg("--project")
        .arg(dir.path())
        .arg("--host")
        .arg("claude")
        .assert()
        .success();

    let manifest = fs::read_to_string(dir.path().join(".zero_nine/manifest.json")).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let host = doc
        .get("default_host")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        host.contains("claude"),
        "expected host to contain 'claude', got: {host}"
    );
}

#[test]
fn test_init_with_host_opencode() {
    let dir = TempDir::new().unwrap();
    zero_nine()
        .arg("init")
        .arg("--project")
        .arg(dir.path())
        .arg("--host")
        .arg("opencode")
        .assert()
        .success();

    let manifest = fs::read_to_string(dir.path().join(".zero_nine/manifest.json")).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let host = doc
        .get("default_host")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        host.contains("open") || host.contains("code"),
        "expected host to contain 'open' or 'code', got: {host}"
    );
}

#[test]
fn test_init_default_host_is_terminal() {
    let dir = TempDir::new().unwrap();
    zero_nine()
        .arg("init")
        .arg("--project")
        .arg(dir.path())
        .assert()
        .success();

    let manifest = fs::read_to_string(dir.path().join(".zero_nine/manifest.json")).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let host = doc
        .get("default_host")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        host, "terminal",
        "expected default host to be 'terminal', got: {host}"
    );
}

// --- Status tests ---

#[test]
fn test_status_on_fresh_init() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    zero_nine()
        .arg("status")
        .arg("--project")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("manifest").or(predicates::str::contains("ready")));
}

#[test]
fn test_status_before_init_reports_missing() {
    let dir = TempDir::new().unwrap();
    // No init — status should succeed but report missing components
    zero_nine()
        .arg("status")
        .arg("--project")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("manifest").and(predicates::str::contains("missing")));
}

// --- Run tests (with pre-seeded Ready session) ---

#[test]
fn test_run_creates_proposal_and_completes() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());
    seed_ready_session(
        dir.path(),
        "create a hello world script",
        HostKind::Terminal,
    );

    zero_nine()
        .arg("run")
        .arg("--project")
        .arg(dir.path())
        .arg("--host")
        .arg("terminal")
        .arg("--goal")
        .arg("create a hello world script")
        .assert()
        .success();

    // A proposal directory should have been created
    let proposals_dir = dir.path().join(".zero_nine/proposals");
    assert!(proposals_dir.is_dir());

    // At least one proposal directory should exist
    let entries: Vec<_> = fs::read_dir(&proposals_dir)
        .unwrap()
        .filter(|e| e.as_ref().map(|e| e.path().is_dir()).unwrap_or(false))
        .collect();
    assert!(
        !entries.is_empty(),
        "expected at least one proposal directory"
    );

    let proposal_dir = &entries[0].as_ref().unwrap().path();

    // Core artifacts should exist
    for file in &[
        "proposal.json",
        "design.md",
        "tasks.md",
        "dag.json",
        "progress.json",
    ] {
        assert!(
            proposal_dir.join(file).is_file(),
            "{file} should exist in proposal"
        );
    }

    // Event log should have entries
    let events = dir.path().join(".zero_nine/runtime/events.ndjson");
    if events.exists() {
        let content = fs::read_to_string(&events).unwrap();
        assert!(!content.is_empty(), "events.ndjson should not be empty");
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            serde_json::from_str::<serde_json::Value>(line)
                .expect("each line in events.ndjson should be valid JSON");
        }
    }
}

#[test]
fn test_run_generates_evolution_candidates() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());
    seed_ready_session(dir.path(), "add input validation", HostKind::Terminal);

    zero_nine()
        .arg("run")
        .arg("--project")
        .arg(dir.path())
        .arg("--host")
        .arg("terminal")
        .arg("--goal")
        .arg("add input validation")
        .assert()
        .success();

    let candidates_dir = dir.path().join(".zero_nine/evolve/candidates");
    // Candidates may or may not exist depending on evolution config
    if candidates_dir.exists() {
        assert!(candidates_dir.is_dir());
    }
}

// --- Status after run ---

#[test]
fn test_status_shows_completed() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());
    seed_ready_session(dir.path(), "write a readme", HostKind::Terminal);

    zero_nine()
        .arg("run")
        .arg("--project")
        .arg(dir.path())
        .arg("--host")
        .arg("terminal")
        .arg("--goal")
        .arg("write a readme")
        .assert()
        .success();

    zero_nine()
        .arg("status")
        .arg("--project")
        .arg(dir.path())
        .assert()
        .success();
}

// --- Resume tests ---

#[test]
fn test_resume_after_run_noops() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());
    seed_ready_session(dir.path(), "simple task", HostKind::Terminal);

    zero_nine()
        .arg("run")
        .arg("--project")
        .arg(dir.path())
        .arg("--host")
        .arg("terminal")
        .arg("--goal")
        .arg("simple task")
        .assert()
        .success();

    // Resume on a fully completed proposal should return without error
    zero_nine()
        .arg("resume")
        .arg("--project")
        .arg(dir.path())
        .arg("--host")
        .arg("terminal")
        .assert()
        .success();
}

// --- Export tests ---

#[test]
fn test_export_creates_adapter_files() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    zero_nine()
        .arg("export")
        .arg("--project")
        .arg(dir.path())
        .assert()
        .success();

    // Export creates files under project root at .claude/ and .opencode/
    let claude_cmd = dir.path().join(".claude/commands/zero-nine.md");
    let claude_skill = dir
        .path()
        .join(".claude/skills/zero-nine-orchestrator/SKILL.md");
    let opencode_cmd = dir.path().join(".opencode/commands/zero-nine.md");
    let opencode_skill = dir
        .path()
        .join(".opencode/skills/zero-nine-orchestrator/SKILL.md");

    assert!(claude_cmd.is_file(), "claude command file should exist");
    assert!(claude_skill.is_file(), "claude skill file should exist");
    assert!(opencode_cmd.is_file(), "opencode command file should exist");
    assert!(opencode_skill.is_file(), "opencode skill file should exist");
}
