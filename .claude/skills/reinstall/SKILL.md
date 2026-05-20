---
name: reinstall
description: Rebuild and reinstall the Zero_Nine CLI binary after modifying any crate.
---

Run `cargo install --path crates/zn-cli` to rebuild and reinstall the `zero-nine` CLI.

This is required after modifying any crate (`zn-types`, `zn-exec`, `zn-spec`, etc.) so that the `zero-nine` command in your PATH picks up the new code.

After reinstalling, verify with:
```bash
zero-nine --help
```
