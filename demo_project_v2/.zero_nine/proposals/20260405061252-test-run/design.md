# Design

## Proposal

20260405061252-test-run

## Four-Layer Strategy

1. Use Brainstorming to clarify intent and acceptance.
2. Write OpenSpec artifacts for planning and traceability.
3. Use Ralph-loop to select tasks, maintain progress, and gate completion.
4. Use OpenSpace-style observation to capture improvements and reduce repeated mistakes.

## Constraints

- Prefer plugin-first host integration while preserving a path toward an independent CLI and SDK.
- Keep artifacts explicit, file-based, and inspectable by humans.
- Preserve separation of concerns across spec, execution, loop, and evolution layers.

## Risks

- The initial user goal may still hide missing business context.
- Execution can remain scaffold-like if plans are not further refined into actionable work units.
- Without worktree isolation, implementation tasks may affect the main branch prematurely.

## Notes

This design is intended to be consumed by the execution layer and re-read by the loop before each fresh iteration.
