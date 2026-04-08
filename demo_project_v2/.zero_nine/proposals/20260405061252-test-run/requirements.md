# Requirements

## User Goal

test-run

## Problem Statement

Transform the user request into a controllable engineering workflow that can start from one sentence and continue through spec, execution, loop orchestration, and evolution.

## Scope In

- Capture the real user intent behind the initial goal.
- Produce persistent specification artifacts for later execution.
- Prepare a resumable loop with quality gates and progress tracking.
- Export host-facing plugin entry points for Claude Code and OpenCode.

## Scope Out

- Do not assume cloud synchronization is already available.
- Do not merge branches automatically without an explicit later confirmation step.

## Constraints

- Prefer plugin-first host integration while preserving a path toward an independent CLI and SDK.
- Keep artifacts explicit, file-based, and inspectable by humans.
- Preserve separation of concerns across spec, execution, loop, and evolution layers.
