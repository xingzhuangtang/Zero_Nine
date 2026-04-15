---
name: zero-nine-tdd-cycle
description: Test-Driven Development implementation cycle
version: 1.0.0
category: execution
platforms: [claude-code, opencode]
metadata:
  zero-nine:
    layer: execution
    requires: [zero-nine-writing-plans]
    triggers: [task.tdd_cycle, task.implementation]
---

# TDD Cycle Skill

## When to Use
- Task mode is `tdd_cycle` or `implementation`
- Implementation requires test coverage
- Working on features with clear acceptance criteria

## Procedure

1. **Read task contract** - Load acceptance criteria and deliverables
2. **Write failing test first**:
   - Based on acceptance criteria
   - Run test to confirm it fails
3. **Implement minimum code** - Write just enough to pass the test
4. **Run verification**:
   - Execute tests
   - Check all acceptance criteria met
5. **Refactor** - Clean up while keeping tests green
6. **Record evidence** - Save test output, code diff, metrics

## Evidence Artifacts
- `test-output.txt` - Test execution results
- `code.diff` - Changes made
- `coverage.txt` - Code coverage report (if applicable)

## Pitfalls
- Don't write tests after implementation
- Don't implement more than needed for current test
- Don't skip refactoring step
- Don't proceed with failing tests

## Verification
- All tests pass (`cargo test --all-targets` or equivalent)
- Code coverage >= 80% for new code
- Evidence artifacts saved to task directory
