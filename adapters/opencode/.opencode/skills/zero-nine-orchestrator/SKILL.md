---
name: zero-nine-orchestrator
description: Coordinate the Zero_Nine four-layer workflow through skill delegation
version: 1.1.0
category: spec
platforms: [claude-code, opencode]
metadata:
  zero-nine:
    layer: orchestration
    requires: [zero-nine-brainstorming, zero-nine-spec-capture, zero-nine-writing-plans, zero-nine-tdd-cycle, zero-nine-verification, zero-nine-finish-branch]
    triggers: [user.goal, zero-nine.run]
---

# Zero_Nine Orchestrator

## What to do

Route the request through the layered skill system. Each layer has a dedicated skill that handles specific concerns.

## Skill Layers

```
┌─────────────────────────────────────────┐
│  1. Brainstorming (requirement layer)   │
│     - Clarify user intent               │
│     - Socratic questioning              │
│     - Verdict: Ready / Needs Clarity    │
└─────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────┐
│  2. Spec Capture (spec layer)           │
│     - Translate to OpenSpec artifacts   │
│     - Create proposal + DAG             │
│     - Validation report                 │
└─────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────┐
│  3. Writing Plans (execution layer)     │
│     - Refine executable plan            │
│     - Prepare workspace (worktree)      │
│     - Quality gates definition          │
└─────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────┐
│  4. TDD Cycle (execution layer)         │
│     - Test-first implementation         │
│     - Evidence collection               │
│     - Verification preparation          │
└─────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────┐
│  5. Verification (verification layer)   │
│     - Build + test + lint checks        │
│     - Acceptance criteria review        │
│     - PASS/FAIL verdict                 │
└─────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────┐
│  6. Finish Branch (evolution layer)     │
│     - Completion report                 │
│     - Merge handling                    │
│     - Loop state update                 │
└─────────────────────────────────────────┘
```

## When to use me

Use this skill when a user wants a single entry point that:
- Clarifies requirements through brainstorming
- Produces structured specification artifacts
- Runs guarded implementation with quality gates
- Tracks progress and updates loop state

## Procedure

1. **Check current state** - Read `.zero_nine/loop/session-state.json`
2. **Determine next layer** - Based on current state and task status
3. **Delegate to appropriate skill**:
   - If no proposal exists → `zero-nine-brainstorming`
   - If proposal is Draft → `zero-nine-spec-capture`
   - If proposal is Ready → `zero-nine-writing-plans`
   - If tasks are Running → `zero-nine-tdd-cycle`
   - If task complete → `zero-nine-verification`
   - If verification passes → `zero-nine-finish-branch`
4. **Update orchestrator state** - Record which skill was invoked
5. **Wait for completion** - Skills report back when done

## State Transitions

```
Idle → Brainstorming → SpecDrafting → Ready → RunningTask → Verifying → Archived
                          │                                      │
                          └──────────→ Retrying ←────────────────┘
```

## Key Files

| File | Purpose |
|------|---------|
| `.zero_nine/manifest.json` | Project configuration |
| `.zero_nine/proposals/<id>/` | Proposal artifacts |
| `.zero_nine/brainstorm/sessions/` | Brainstorm session records |
| `.zero_nine/loop/session-state.json` | Orchestrator state |
| `.zero_nine/runtime/events.ndjson` | Event log |

## Pitfalls

- Don't skip brainstorming - unclear requirements cause rework
- Don't proceed without Ready verdict
- Don't merge without verification PASS
- Always update loop state after skill completion

## Verification

- Loop state file is updated after each skill completes
- Progress files reflect current task status
- Evidence artifacts are collected in task directories
