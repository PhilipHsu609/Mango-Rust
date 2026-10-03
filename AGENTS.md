# Agent Guidance

Mango-Rust's migration from Crystal is mostly complete: all important features have been migrated to Rust. The original Crystal project in `Mango/` is retained for reference only and remains valuable for understanding behavior and design decisions.

## Preserve Mango behavior

- Consult `Mango/` when intended behavior or historical design decisions are unclear; inspect the corresponding Crystal implementation and tests where available.
- Preserve Mango’s observable behavior and data contracts.
- When the reference is unclear or a necessary divergence is unavoidable, explain the choice and discuss it with the maintainer before inventing behavior.
- Distinguish intended behavior from accidental implementation details in either codebase.

## Keep changes direct

- Prefer the simplest implementation that matches Mango. Avoid speculative features, unnecessary abstractions, and unrelated cleanup.
- This project has no users yet and is under active development. When changing an interface, update every caller and remove obsolete paths.
- Seek logical equivalence with Mango rather than identical implementation details.
- Avoid unnecessary dependencies. If a dependency is needed, prefer a small, well-maintained crate with a permissive license.
- Avoid reinventing the wheel. Prefer existing crates over custom implementations, unless the crate is unmaintained or has a restrictive license.

## Repository hygiene task

- Work on the `repo-hygiene` branch.
- Identify and implement evidence-backed refactoring and cleanup opportunities; keep behavior unchanged unless a change is explicitly agreed.
- Redesign data structures where it simplifies the code. Make module boundaries, ownership, mutation, and shared state explicit; prefer clear ownership over unnecessary cloning or synchronization.
- Follow idiomatic Rust and the Rust Style Guide. Use `rustfmt` for formatting and Clippy to identify actionable issues; prefer existing repository conventions where they are idiomatic.
- Keep refactors focused and verify affected behavior, not just compilation.
- Optionally maintain one task-specific note under `docs/` for findings, design decisions, verification, and handoff context. Keep it current rather than creating multiple overlapping notes.

## Local comparison workflow

- `local-dev/run-local.sh` starts or restarts the local Crystal and Rust containers. It expects `mango-rust:local` and `config.yml` plus `library/` for both apps under `MANGO_COMPARE_DIR` (default `/home/philip/mango-compare`). It sets both `admin` passwords to `MANGO_DEV_ADMIN_PASSWORD` (default `mango-dev`).
- From the repository root, run `./local-dev/run-local.sh`. Crystal listens on `http://localhost:9000/`; Rust listens on `http://localhost:9001/`.
- For UI changes or behavior verification, use the browser tool to open both apps, sign in with the launcher’s printed credentials, exercise the affected flow, and compare the visible results. HTTP readiness and automated tests alone do not verify the UI.
