# AGENTS

## Purpose
This file defines how human contributors and AI agents collaborate in this repository.

## Scope
These instructions apply to the entire repository unless a deeper directory contains another `AGENTS.md` with more specific rules.

## Repository Context
- Workspace language: Rust (`cargo` workspace)
- Main components:
- `cli/`: end-user `statichub` CLI
- `server/`: `statichub-server` API + static host
- `shared/`: shared types across CLI and server
- `skills/`: reusable AI agent skill definitions

## Working Principles
- Prefer small, focused changes with clear intent.
- Preserve backward compatibility unless explicitly asked to break it.
- Do not mix unrelated refactors with feature or bugfix work.
- Ask before destructive actions (deleting data, force-pushing, resetting history).

## Coding Conventions
- Follow existing Rust style and module organization.
- Keep public APIs in `shared/` stable and version-aware.
- Add concise comments only where logic is non-obvious.
- Avoid opportunistic formatting-only edits.

## Validation Checklist
Run the smallest useful check first, then expand:

1. Targeted tests for touched package(s), for example:
```bash
cargo test -p statichub-server
cargo test -p statichub-cli
```
2. Full workspace tests before final handoff for meaningful logic changes:
```bash
cargo test --workspace
```
3. Build verification when needed:
```bash
cargo check --workspace
```

If any check is skipped or fails, report it clearly with reason and impact.

## Deployment and Safety Notes
- For deploy-path behavior, ensure both directory deploy and single HTML deploy paths remain valid.
- Do not change auth, domain routing, or rollback behavior without tests covering regressions.
- Prefer explicit config over hidden defaults when editing server runtime behavior.

## Commit Guidance
- Use imperative commit messages (example: `Add project name validation for deploy`).
- Keep commits logically grouped and reviewable.
- Exclude generated artifacts unless they are intentionally tracked.

## Handoff Expectations
When finishing work, summarize:
- What changed
- Why it changed
- How it was validated
- Any risks, assumptions, or follow-up items
