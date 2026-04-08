# TDD Cycle

## Task

Execute guarded implementation verification and branch finishing

## Required Gates

- **tests**: TDD or at least explicit test execution is required before completion. (required: true)
- **review**: Implementation must be reviewable and ready for a reviewer brief. (required: true)

## Test Results

| # | Test | Result |
|---|------|--------|
| 1 | Core spec files exist (proposal.md, requirements.md, acceptance.md, design.md, tasks.md, dag.json, progress.json, verification.md) | PASS |
| 2 | Task 1 artifacts exist (brainstorming, requirement-packet) | PASS |
| 3 | Task 2 artifacts exist (brainstorming, requirement-packet) | PASS |
| 4 | Task 3 artifacts exist (writing-plans, workspace-plan, subagents) | PASS |
| 5 | Task 4 artifacts exist (implementation, tdd-cycle, review-brief) | PASS |
| 6 | All task reports exist (task-1 through task-4) | PASS |
| 7 | Runtime state files exist (manifest, session-state, events, envelope) | PASS |
| 8 | Evolution artifacts exist (evaluations.jsonl, candidates) | PASS |
| 9 | Host command adapters exist (opencode, claude) | PASS |
| 10 | DAG consistency: all tasks completed | PASS |
| 11 | Event log: proposal lifecycle complete | PASS |
| 12 | Event log: all 4 task completions recorded | PASS |

## Suggested Loop

- Write or identify failing tests first.
- Implement the smallest safe change.
- Run regression checks and capture evidence.
- Escalate unresolved failures instead of silently proceeding.
