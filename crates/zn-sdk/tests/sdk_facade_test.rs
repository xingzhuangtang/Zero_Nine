//! Tier 3: SDK facade programmatic tests
//! Tests the ZeroNine struct API directly (not via CLI).

use chrono::Utc;
use std::fs;
use tempfile::TempDir;
use zn_sdk::{from_project, NoopInput, ZeroNine};
use zn_types::{BrainstormSession, BrainstormVerdict, HostKind};

fn sdk(dir: &TempDir, host: HostKind) -> ZeroNine {
    from_project(dir.path().to_str().unwrap(), host)
}

/// Create a Ready brainstorm session so `run` can skip interactive brainstorming.
fn seed_ready_session(dir: &TempDir, goal: &str, host: HostKind) {
    let session = BrainstormSession {
        id: "test-sdk-session".to_string(),
        goal: goal.to_string(),
        host,
        status: "active".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        questions: Vec::new(),
        answers: Vec::new(),
        verdict: BrainstormVerdict::Ready,
    };
    let sessions_dir = dir.path().join(".zero_nine/brainstorm/sessions");
    fs::create_dir_all(&sessions_dir).unwrap();
    let json = serde_json::to_string_pretty(&session).unwrap();
    fs::write(sessions_dir.join("test-sdk-session.json"), &json).unwrap();
    fs::write(
        dir.path().join(".zero_nine/brainstorm/latest-session.json"),
        &json,
    )
    .unwrap();
    fs::write(
        dir.path().join(".zero_nine/brainstorm/latest-session.md"),
        goal,
    )
    .unwrap();
}

#[test]
fn test_sdk_init_then_status() {
    let dir = TempDir::new().unwrap();
    let sdk = &sdk(&dir, HostKind::Terminal);

    sdk.init().expect("init should succeed");
    assert!(dir.path().join(".zero_nine/manifest.json").is_file());

    let status = sdk.status().expect("status should succeed");
    assert_eq!(status.status, "ready");
}

#[test]
fn test_sdk_init_creates_layout() {
    let dir = TempDir::new().unwrap();
    let sdk = &sdk(&dir, HostKind::ClaudeCode);

    sdk.init().expect("init should succeed");

    let zero_nine = dir.path().join(".zero_nine");
    assert!(zero_nine.is_dir());

    for sub in &["proposals", "brainstorm", "specs", "evolve", "runtime"] {
        assert!(zero_nine.join(sub).is_dir(), "{sub} should exist");
    }

    let manifest = fs::read_to_string(zero_nine.join("manifest.json")).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let host = doc
        .get("default_host")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(host.contains("claude"), "expected claude host, got: {host}");
}

#[test]
fn test_sdk_run_goal() {
    let dir = TempDir::new().unwrap();
    let sdk = &sdk(&dir, HostKind::Terminal);

    sdk.init().expect("init should succeed");
    seed_ready_session(&dir, "create a hello world script", HostKind::Terminal);

    sdk.run_goal_headless("create a hello world script", false)
        .expect("run_goal should succeed");

    let proposals = dir.path().join(".zero_nine/proposals");
    assert!(proposals.is_dir());

    let entries: Vec<_> = fs::read_dir(&proposals)
        .unwrap()
        .filter(|e| e.as_ref().map(|e| e.path().is_dir()).unwrap_or(false))
        .collect();
    assert!(!entries.is_empty(), "expected at least one proposal");
}

#[test]
fn test_sdk_resume() {
    let dir = TempDir::new().unwrap();
    let sdk = &sdk(&dir, HostKind::Terminal);

    sdk.init().expect("init should succeed");
    seed_ready_session(&dir, "simple task", HostKind::Terminal);
    sdk.run_goal_headless("simple task", false)
        .expect("run should succeed");

    let result = sdk.resume_headless(false);
    assert!(result.is_ok(), "resume should succeed: {:?}", result);
}

#[test]
fn test_sdk_export() {
    let dir = TempDir::new().unwrap();
    let sdk = &sdk(&dir, HostKind::Terminal);

    sdk.init().expect("init should succeed");
    let export_result = sdk.export();
    assert!(
        export_result.is_ok(),
        "export should succeed: {:?}",
        export_result
    );

    // Export creates files at .claude/ and .opencode/ under project root
    let claude_cmd = dir.path().join(".claude/commands/zero-nine.md");
    let claude_skill = dir
        .path()
        .join(".claude/skills/zero-nine-orchestrator/SKILL.md");
    assert!(claude_cmd.is_file(), "claude command should exist");
    assert!(claude_skill.is_file(), "claude skill should exist");
}

#[test]
fn test_sdk_validate_spec_after_run() {
    let dir = TempDir::new().unwrap();
    let sdk = &sdk(&dir, HostKind::Terminal);

    sdk.init().expect("init should succeed");
    seed_ready_session(&dir, "write documentation", HostKind::Terminal);
    sdk.run_goal_headless("write documentation", false)
        .expect("run should succeed");

    let result = sdk.validate_spec();
    assert!(
        result.is_ok(),
        "validate_spec should return ok: {:?}",
        result
    );
}

#[test]
fn test_sdk_from_project_convenience() {
    let dir = TempDir::new().unwrap();
    let sdk = from_project(dir.path().to_str().unwrap(), HostKind::OpenCode);

    assert!(matches!(sdk.host(), HostKind::OpenCode));

    sdk.init().expect("init should succeed");
    assert!(dir.path().join(".zero_nine/manifest.json").is_file());
}
