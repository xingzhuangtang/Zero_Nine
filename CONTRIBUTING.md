# Contributing to Zero_Nine

Thank you for your interest in contributing to Zero_Nine — the Rust orchestration kernel for AI agent workflows. This guide explains how to contribute effectively while upholding the three core principles that guide this project's evolution.

## Three Architectural Principles

Before writing any code, internalize these principles. They are not guidelines — they are **laws**:

### Law 1: Keep the Skeleton Lean

> "骨架要瘦" — The orchestration core must remain thin and deterministic.

- The Rust crates (`zn-types`, `zn-loop`, `zn-exec`, etc.) handle **only** what must be deterministic: state machines, DAG scheduling, verification gates, event logging, and error classification.
- **Never** add business logic to the Rust layer that could instead be expressed as a `SKILL.md` file.
- If a function grows beyond 80 lines, it is a signal to extract or move logic to the skill layer.
- Tool definitions and prompt fragments must be minimal. A tool description that takes longer to read than to execute is a defect.

### Law 2: Fatten the Skills

> "本事要肥" — The skill layer is where expertise lives.

- Skills are the primary unit of value. Every domain workflow, every heuristic, every hard-won lesson must be encoded as a `SKILL.md` file.
- A skill is not a description of *what* to do — it is a step-by-step *how*, with decision branches, pitfalls, and verification criteria.
- **The 3-to-10 Sample Rule**: Before writing a new skill, manually execute the workflow 3–10 times and document the patterns. Only then crystallize it into a skill file.
- Skills must be self-contained: a skill that requires reading three other documents to understand has failed.

### Law 3: Never Do One-Off Work

> "绝不允许做一次性工作" — If it might happen again, make it a skill.

- If you find yourself writing the same type of code or prompt twice, stop and write a skill first.
- The test: *"If someone asked me to do this exact thing tomorrow, could they invoke a skill and get the same result?"* If not, the skill is incomplete.
- This principle applies to code reviews, debugging sessions, deployment procedures, and documentation updates — not just feature development.

---

## Development Workflow

### Prerequisites

```bash
# Install Rust (stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install development tools
cargo install cargo-tarpaulin  # coverage
cargo install git-cliff         # changelog generation
```

### Setting Up

```bash
git clone https://github.com/xingzhuangtang/Zero_Nine.git
cd Zero_Nine
cargo build
cargo test --all-targets
```

### Commit Convention

This project uses [Conventional Commits](https://www.conventionalcommits.org/):

| Prefix | Purpose | Example |
|--------|---------|---------|
| `feat` | New feature | `feat(zn-loop): add cron scheduler` |
| `fix` | Bug fix | `fix(zn-exec): handle empty worktree plan` |
| `refactor` | Code restructuring | `refactor(zn-types): split lib.rs into modules` |
| `test` | Tests only | `test(zn-spec): add skill validation coverage` |
| `docs` | Documentation | `docs: update architecture diagram` |
| `chore(deps)` | Dependency updates | `chore(deps): bump serde to 1.0.200` |
| `perf` | Performance | `perf(zn-loop): cache manifest reads` |

Breaking changes must include `!` after the prefix: `feat(zn-types)!: rename LoopStage variants`.

### Pull Request Checklist

Before opening a PR, verify all of the following:

- [ ] `cargo fmt --all` — no formatting changes
- [ ] `cargo clippy --all-targets -- -D warnings` — zero warnings
- [ ] `cargo test --all-targets` — all tests pass
- [ ] New public functions have doc comments
- [ ] New error variants use `thiserror` in `zn-types/src/error.rs`
- [ ] No new `println!` in library crates (`zn-loop`, `zn-exec`, `zn-spec`, `zn-evolve`)
- [ ] If the change introduces a repeatable workflow, a `SKILL.md` has been created or updated

### Adding a New Skill

Skills live in `adapters/claude-code/.claude/skills/` and follow the format defined in `crates/zn-spec/src/skill_format.rs`.

```bash
# Create a new skill directory
mkdir -p adapters/claude-code/.claude/skills/zero-nine-<skill-name>

# Create the SKILL.md file
cat > adapters/claude-code/.claude/skills/zero-nine-<skill-name>/SKILL.md << 'EOF'
---
name: zero-nine-<skill-name>
description: One-line description of what this skill does
version: 1.0.0
category: <execution|planning|verification|evolution>
platforms: [claude-code, opencode]
metadata:
  zero-nine:
    layer: <execution|spec|loop|evolve>
    requires: []
    triggers: []
---

## Purpose
What problem does this skill solve?

## When to Use
Specific conditions that trigger this skill.

## Steps
1. Step one with decision criteria
2. Step two...

## Pitfalls
- Common mistake to avoid

## Verification
- How to confirm the skill succeeded
EOF
```

### Adding a New Error Type

All structured errors belong in `crates/zn-types/src/error.rs`:

```rust
// Add to the appropriate error enum
#[derive(Debug, Error)]
pub enum ExecutionError {
    // ... existing variants ...

    #[error("your new error: {detail}")]
    YourNewError { detail: String },
}
```

Then add a unit test in the same file:

```rust
#[test]
fn test_your_new_error_display() {
    let e = ExecutionError::YourNewError { detail: "test".to_string() };
    assert!(e.to_string().contains("your new error"));
}
```

---

## Architecture Overview

```
Zero_Nine/
├── crates/
│   ├── zn-types/     # Shared types + structured errors (NO business logic)
│   ├── zn-spec/      # Proposal/artifact management + skill format
│   ├── zn-exec/      # Execution plans, workspace, verification
│   ├── zn-loop/      # Scheduler, retries, recovery (the "skeleton")
│   ├── zn-evolve/    # Skill scoring + distillation (the "skill factory")
│   ├── zn-host/      # Claude/OpenCode adapter output
│   └── zn-cli/       # CLI entry point only
└── adapters/
    └── claude-code/.claude/skills/   # The "fat skills" live here
```

The guiding metaphor: **the Rust crates are the chassis; the SKILL.md files are the engine**.

---

## Questions?

Open a [GitHub Discussion](https://github.com/xingzhuangtang/Zero_Nine/discussions) for design questions, or a [GitHub Issue](https://github.com/xingzhuangtang/Zero_Nine/issues) for bugs and feature requests.
