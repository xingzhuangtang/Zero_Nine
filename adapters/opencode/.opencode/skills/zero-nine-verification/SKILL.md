---
name: zero-nine-verification
description: Verification gate with evidence collection
version: 1.0.0
category: verification
platforms: [claude-code, opencode]
metadata:
  zero-nine:
    layer: verification
    requires: [zero-nine-tdd-cycle]
    triggers: [task.verification, pre_merge]
---

# Verification Skill

## When to Use
- After TDD cycle completes
- Before merging to main branch
- User requests verification
- Loop requires quality gate confirmation

## Procedure

1. **Collect evidence**:
   - Test results from task execution
   - Code diff summary
   - Coverage reports
2. **Run verification checks**:
   - Build succeeds (`cargo build`)
   - All tests pass (`cargo test --all-targets`)
   - Linting passes (`cargo clippy`)
   - Format check (`cargo fmt --check`)
3. **Review against acceptance criteria**:
   - Check all task acceptance criteria met
   - Verify deliverables are complete
4. **Generate verification report**:
   - Summary of checks run
   - Pass/fail verdict for each check
   - Overall verification status
5. **Submit evidence** - Send to kernel for loop update

## Verification Report Format
```markdown
# Verification Report

## Checks
- [ ] Build: PASS/FAIL
- [ ] Tests: PASS/FAIL
- [ ] Lint: PASS/FAIL
- [ ] Format: PASS/FAIL

## Acceptance Criteria
- [ ] Criterion 1: EVIDENCE
- [ ] Criterion 2: EVIDENCE

## Verdict
PASS/FAIL - Ready to merge / Requires fixes
```

## Pitfalls
- Don't skip any verification check
- Don't proceed with failing checks
- Document any known issues in report

## Verification
- `verification.md` written with full report
- Evidence files collected in task directory
- Verdict is explicit (PASS or FAIL)
