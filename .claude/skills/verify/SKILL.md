---
name: verify
description: Run full project verification — build, test, lint. Reports test count changes and new warnings.
---

Run the following commands sequentially and report results:

1. `cargo build` — must compile without errors
2. `cargo test --all-targets 2>&1 | grep "test result"` — report test counts per crate
3. `cargo clippy --all-targets 2>&1 | grep -E "warning|error"` — report any warnings/errors

If any step fails, identify the specific crate and issue, then suggest fixes. Do not commit until all steps pass cleanly.
