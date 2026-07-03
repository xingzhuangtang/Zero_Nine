---
name: quick-build
description: Fast cargo check + targeted test for the crate you're working on. Use during iterative development when full /verify is too slow.
---

Run a fast verification cycle focused on the crate being edited, not the entire workspace.

## Steps

1. **Ask which crate** the user is working on (or infer from recent edits). Default to `zn-cli` if unclear.

2. **Fast check**: Run `cargo check -p <crate>`. If it fails, stop and report errors.

3. **Targeted test**: Run `cargo test -p <crate> --all-targets`. Skip if the crate has no tests.

4. **Report**: Show check + test results. If both pass, say "✓ <crate> clean". If either fails, show the first 30 lines of errors and suggest next steps.

## When to Use

- During active editing of a single crate
- When you want to verify a change before running full `/verify`
- When iterating on a feature and full workspace tests take too long

## Notes

- This is a subset of `/verify` — recommend running full `/verify` before committing
- For integration tests that span crates, `cargo test -p <crate>` may miss cross-crate issues
