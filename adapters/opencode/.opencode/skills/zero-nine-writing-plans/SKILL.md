---
name: zero-nine-writing-plans
description: Refine executable plan from spec artifacts
version: 1.0.0
category: execution
platforms: [claude-code, opencode]
metadata:
  zero-nine:
    layer: execution
    requires: [zero-nine-spec-capture]
    triggers: [task.planning, spec.validated]
---

# Writing Plans Skill

## When to Use
- Spec artifacts are complete and validated
- Before starting implementation tasks
- Need to prepare workspace and execution strategy

## Procedure

1. **Read spec bundle** - Load proposal, tasks, and DAG
2. **Identify runnable tasks** - Tasks with all dependencies completed
3. **Prepare workspace**:
   - Decide worktree isolation strategy
   - Set up branch naming convention
   - Prepare rollback plan
4. **Write execution plan**:
   - Step-by-step implementation order
   - Quality gates for each step
   - Risk mitigation strategies
5. **Document deliverables** - What artifacts will be produced

## Workspace Strategy
- Use git worktree for isolated development
- Branch naming: `zero-nine/<task-id>-<slug>`
- Always preserve main branch until verification passes

## Pitfalls
- Don't merge to main without verification
- Don't skip workspace preparation
- Document any deviations from plan

## Verification
- Execution plan written to `execution-plan.md`
- Workspace is prepared (worktree or branch created)
- Quality gates are explicit before coding begins
