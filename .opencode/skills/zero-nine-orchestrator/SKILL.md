---
name: zero-nine-orchestrator
description: Coordinate the Zero_Nine four-layer workflow. Use when you need one host command that can clarify requirements, bind an OpenSpec contract, run guarded execution, control the loop, and write back evolution artifacts.
---
## What to do

Route the request through four layers in order: Brainstorming, spec capture, execution, and evolution.

For Claude Code and OpenCode, treat the slash command as one continuous clarify-to-execute entry point. Reuse the same command for each answer until Zero_Nine reports that the session is Ready and the bound OpenSpec artifacts are complete. Do not bypass the clarification or specification gates by starting a separate execution command early.

## When to use me

Use this skill when a user wants a single entry point that can clarify requirements, produce inspectable specification artifacts, run a guarded implementation workflow, and write back progress and learning artifacts with minimal manual intervention.
