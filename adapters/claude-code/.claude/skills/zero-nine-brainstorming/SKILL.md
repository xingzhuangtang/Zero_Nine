---
name: zero-nine-brainstorming
description: Socratic questioning for requirement clarification
version: 1.0.0
category: brainstorming
platforms: [claude-code, opencode]
metadata:
  zero-nine:
    layer: requirement
    requires: []
    triggers: [task.brainstorming, user.goal_unclear]
---

# Brainstorming Skill

## When to Use
- User provides a vague or high-level goal
- Before any spec capture or execution begins
- When requirements need clarification through dialogue

## Procedure

1. **Read the user goal** - Understand what the user wants to achieve
2. **Ask clarification questions** - Use the 6-question framework:
   - Purpose: Why is this needed? What problem does it solve?
   - Scope: What's in scope vs out of scope?
   - Constraints: Any technical, time, or resource constraints?
   - Acceptance: How will we know this is complete?
   - Context: What existing systems/context should I know?
   - Priority: What's most important vs nice to have?
3. **Record answers** - Store answers in the brainstorm session
4. **Check readiness** - When all questions are answered, verdict = Ready
5. **Produce output** - Generate requirement packet for spec capture

## Pitfalls
- Don't ask all questions at once - prioritize based on goal
- Don't proceed to execution without Ready verdict
- Don't assume - ask explicitly when unclear

## Verification
- Brainstorm session JSON exists in `.zero_nine/brainstorm/sessions/`
- Verdict is `Ready` before spec capture begins
- Requirement packet has no unresolved questions
