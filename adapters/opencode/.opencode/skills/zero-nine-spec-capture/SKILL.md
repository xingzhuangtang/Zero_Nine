---
name: zero-nine-spec-capture
description: Translate requirements into OpenSpec artifacts
version: 1.0.0
category: spec
platforms: [claude-code, opencode]
metadata:
  zero-nine:
    layer: spec
    requires: [zero-nine-brainstorming]
    triggers: [task.spec_capture, brainstorm.ready]
---

# Spec Capture Skill

## When to Use
- Brainstorming verdict is Ready
- User provides clear requirements
- Need to produce structured specification artifacts

## Procedure

1. **Read requirement packet** - Load clarified requirements from brainstorm output
2. **Create proposal structure** - Generate `.zero_nine/proposals/<id>/` directory
3. **Write core artifacts**:
   - `proposal.md` - Goal, problem statement, status
   - `requirements.md` - Scope in/out, constraints
   - `acceptance.md` - Acceptance criteria checklist
   - `design.md` - Architecture and approach
   - `tasks.md` - Task breakdown with dependencies
   - `dag.json` - Machine-readable task graph
4. **Validate spec** - Run validation to ensure completeness
5. **Save session state** - Update loop state to Ready

## Output Files
- `proposal.json` - Structured proposal data
- `requirement-packet.json` - Source requirements
- All markdown files for human readability

## Pitfalls
- Don't start without Ready brainstorm verdict
- Don't skip validation step
- Ensure task dependencies form valid DAG (no cycles)

## Verification
- All 6 core files exist
- Spec validation report shows no errors
- Task DAG is valid (no orphan tasks, no cycles)
