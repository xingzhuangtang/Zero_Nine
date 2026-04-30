# Changelog

All notable changes to Zero_Nine will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Refactoring

- **zn-types**: Split monolithic `lib.rs` (3539 lines) into 9 focused modules:
  `core`, `state_machine`, `proposal`, `drift`, `execution`, `governance`,
  `evolution`, `github`, and `error`. All public APIs remain unchanged.

### Features

- **zn-types**: Introduce structured error types via `thiserror`:
  `BrainstormError`, `ProposalError`, `ExecutionError`, `SkillError`,
  `MemoryToolError`, and top-level `ZnError`. Replaces bare string errors
  at API boundaries for programmatic error matching.

### Infrastructure

- Add GitHub Actions CI workflow: format check, Clippy (zero-warning policy),
  cross-platform tests (Linux + macOS), security audit, and code coverage.
- Add GitHub Actions Release workflow: cross-platform binary builds and
  automated GitHub Release creation with changelog.
- Add Dependabot configuration for weekly Cargo and GitHub Actions updates.
- Add `rustfmt.toml` for consistent code formatting across all crates.
- Add `.clippy.toml` for project-wide Clippy configuration.
- Add `cliff.toml` for automated changelog generation from Conventional Commits.
- Add `CONTRIBUTING.md` with architectural principles (Three Laws) and
  development workflow documentation.

### Bug Fixes

- **zn-loop**: Replace `println!` in `run_terminal_brainstorm` with structured
  `RuntimeEvent` emission + `eprintln!` to stderr, separating structured output
  from interactive terminal output.
