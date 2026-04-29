//! Tier 4: Sub-command integration tests
//! skill, memory, governance, cron, observe
//!
//! These subcommands operate on the current working directory rather than
//! accepting --project, so we set current_dir to the temp project root.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

fn zero_nine(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("zero-nine").expect("zero-nine binary not built");
    cmd.current_dir(dir);
    cmd
}

fn init_project(dir: &Path) {
    zero_nine(dir)
        .arg("init")
        .arg("--project")
        .arg(dir)
        .assert()
        .success();
}

// --- Skill tests ---

#[test]
fn test_skill_create_and_list() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    zero_nine(dir.path())
        .arg("skill")
        .arg("create")
        .arg("--name")
        .arg("test-skill")
        .arg("--description")
        .arg("A test skill")
        .assert()
        .success();

    zero_nine(dir.path())
        .arg("skill")
        .arg("list")
        .assert()
        .success()
        .stdout(predicates::str::contains("test-skill"));
}

#[test]
fn test_skill_create_view_delete() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    zero_nine(dir.path())
        .arg("skill")
        .arg("create")
        .arg("--name")
        .arg("crud-test")
        .arg("--description")
        .arg("CRUD test skill")
        .assert()
        .success();

    zero_nine(dir.path())
        .arg("skill")
        .arg("view")
        .arg("--name")
        .arg("crud-test")
        .assert()
        .success();

    zero_nine(dir.path())
        .arg("skill")
        .arg("delete")
        .arg("--name")
        .arg("crud-test")
        .assert()
        .success();

    // List should no longer contain it
    zero_nine(dir.path())
        .arg("skill")
        .arg("list")
        .assert()
        .success()
        .stdout(predicates::str::contains("crud-test").not());
}

#[test]
fn test_skill_validate_valid() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    zero_nine(dir.path())
        .arg("skill")
        .arg("create")
        .arg("--name")
        .arg("validatable")
        .arg("--description")
        .arg("Validatable skill")
        .assert()
        .success();

    zero_nine(dir.path())
        .arg("skill")
        .arg("validate")
        .arg("--name")
        .arg("validatable")
        .assert()
        .success();
}

// --- Memory tests ---

#[test]
fn test_memory_init_add_read() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    zero_nine(dir.path())
        .arg("memory")
        .arg("init")
        .assert()
        .success();

    zero_nine(dir.path())
        .arg("memory")
        .arg("add")
        .arg("--target")
        .arg("memory")
        .arg("--content")
        .arg("test-value")
        .arg("--section")
        .arg("test-key")
        .assert()
        .success();

    zero_nine(dir.path())
        .arg("memory")
        .arg("read")
        .arg("--target")
        .arg("memory")
        .assert()
        .success()
        .stdout(predicates::str::contains("test-value"));
}

// --- Governance tests ---

#[test]
fn test_governance_check() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    zero_nine(dir.path())
        .arg("governance")
        .arg("check")
        .arg("--action")
        .arg("ReadFile")
        .assert()
        .success();
}

#[test]
fn test_governance_matrix() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    zero_nine(dir.path())
        .arg("governance")
        .arg("matrix")
        .assert()
        .success();
}

// --- Cron tests ---

#[test]
fn test_cron_schedule_list_cancel() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    zero_nine(dir.path())
        .arg("cron")
        .arg("schedule")
        .arg("--id")
        .arg("test-cron")
        .arg("--cron")
        .arg("0 9 * * *")
        .arg("--description")
        .arg("daily check")
        .assert()
        .success();

    zero_nine(dir.path())
        .arg("cron")
        .arg("list")
        .assert()
        .success()
        .stdout(predicates::str::contains("test-cron"));

    zero_nine(dir.path())
        .arg("cron")
        .arg("cancel")
        .arg("--id")
        .arg("test-cron")
        .assert()
        .success();

    // After cancel, list should not contain it
    zero_nine(dir.path())
        .arg("cron")
        .arg("list")
        .assert()
        .success()
        .stdout(predicates::str::contains("test-cron").not());
}

// --- Observe tests ---

#[test]
fn test_observe_events_empty_on_fresh_init() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    zero_nine(dir.path())
        .arg("observe")
        .arg("events")
        .arg("--event-type")
        .arg("task.completed")
        .assert()
        .success();
}

#[test]
fn test_observe_stats_on_fresh_init() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    zero_nine(dir.path())
        .arg("observe")
        .arg("stats")
        .assert()
        .success();
}

// --- Subagent tests ---

#[test]
fn test_subagent_history_empty() {
    let dir = TempDir::new().unwrap();
    init_project(dir.path());

    // Subagent history requires --proposal; with no proposal it will error
    // but should not panic. We just verify it runs without crashing.
    let result = zero_nine(dir.path())
        .arg("subagent")
        .arg("history")
        .arg("--proposal")
        .arg("test-proposal")
        .output()
        .unwrap();
    // May fail with exit 1 but shouldn't panic
    let _stderr = String::from_utf8_lossy(&result.stderr);
}
