---
name: zero-nine-code-review
description: Code review protocol for quality assurance
version: 1.0.0
category: verification
platforms: [claude-code, opencode]
metadata:
  zero-nine:
    layer: verification
    requires: [zero-nine-tdd-cycle]
    triggers: [task.review, pre_verification]
---

# Code Review Skill

## When to Use
- After TDD cycle completes
- Before verification gate
- User requests code review
- Complex changes need second pair of eyes

## Procedure

1. **Read change summary** - Understand what was changed and why
2. **Review code diff**:
   - Architecture alignment
   - Code style consistency
   - Error handling completeness
   - Test coverage adequacy
3. **Check for common issues**:
   - Security vulnerabilities (OWASP Top 10)
   - Performance anti-patterns
   - Memory safety issues (if applicable)
   - Logic errors or edge cases
4. **Provide feedback**:
   - Blockers (must fix before merge)
   - Suggestions (nice to have)
   - Questions (clarification needed)
5. **Record review decision** - Approve, Request Changes, or Comment

## Review Checklist
- [ ] Code follows project conventions
- [ ] Tests cover new functionality
- [ ] Error handling is complete
- [ ] No security vulnerabilities introduced
- [ ] Performance impact is acceptable
- [ ] Documentation updated if needed

## Pitfalls
- Don't approve without understanding the change
- Don't focus only on style - check logic too
- Document all blockers explicitly

## Output
```markdown
## Code Review

**Reviewer**: <agent>
**Decision**: APPROVED / CHANGES_REQUESTED / COMMENT

### Blockers
- <list of must-fix issues>

### Suggestions
- <list of nice-to-have improvements>

### Questions
- <list of clarification questions>
```
