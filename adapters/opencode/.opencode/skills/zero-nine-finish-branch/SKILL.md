---
name: zero-nine-finish-branch
description: Complete task and handle branch finishing
version: 1.0.0
category: evolution
platforms: [claude-code, opencode]
metadata:
  zero-nine:
    layer: evolution
    requires: [zero-nine-verification]
    triggers: [task.finish, verification.passed]
---

# Finish Branch Skill

## When to Use
- Verification passes with PASS verdict
- All acceptance criteria met
- Ready to merge or finalize task

## Procedure

1. **Confirm verification status** - Check verification.md shows PASS
2. **Prepare finish options**:
   - Merge to main (requires user confirmation)
   - Keep branch for review
   - Create pull request
3. **Update task status** - Mark task as Completed
4. **Update loop state** - Notify kernel of task completion
5. **Document completion**:
   - Summary of what was accomplished
   - Links to artifacts and evidence
   - Any follow-up recommendations

## Merge Policy
- Default: Ask user before merging to main
- Can be configured with `confirm_remote_finish` setting
- Never auto-merge without explicit confirmation

## Output
```markdown
## Task Completion Report

**Task ID**: <id>
**Status**: Completed
**Verification**: PASS

### Artifacts
- <list of produced artifacts>

### Evidence
- <list of evidence files>

### Recommendations
- <any follow-up items>
```

## Pitfalls
- Don't merge without verification PASS
- Don't skip user confirmation (unless explicitly configured)
- Document any technical debt or follow-ups

## Verification
- Task status updated to Completed in progress files
- Loop state reflects task completion
- Completion report written
