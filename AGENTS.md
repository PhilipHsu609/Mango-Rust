# Agent Guidance

Mango-Rust is an incomplete, actively revived Rust port of the Crystal project in `Mango/`.

## Preserve Mango behavior

- Treat `Mango/` as the primary reference for existing behavior. Before changing behavior, inspect the corresponding Crystal implementation and tests where available.
- Preserve Mango’s observable behavior and data contracts, including database compatibility, unless the port cannot reasonably do so.
- When the reference is unclear or a necessary divergence is unavoidable, explain the choice and discuss it with the maintainer before inventing behavior.
- Do not treat accidental behavior in the unfinished Rust port as a compatibility requirement.

## Keep changes direct

- Prefer the simplest implementation that matches Mango. Avoid speculative features, unnecessary abstractions, and unrelated cleanup.
- This project has no users yet and is under active development: do not add compatibility shims, legacy fallbacks, or parallel ways to do the same thing. Update callers and remove obsolete paths when changing an interface.
- We are seeking logical equivalence with Mango, not a exact behavioral match. If a behavior is logically equivalent but implemented differently, it is acceptable to diverge from Mango’s implementation.
- Avoid unnecessary dependencies. If a dependency is needed, prefer a small, well-maintained crate with a permissive license.
- Avoid reinventing the wheel. Prefer existing crates over custom implementations, unless the crate is unmaintained or has a restrictive license.

## Local comparison workflow

- `local-dev/run-local.sh` starts or restarts the local Crystal and Rust containers. It expects `mango-rust:local` and `config.yml` plus `library/` for both apps under `MANGO_COMPARE_DIR` (default `/home/philip/mango-compare`). It sets both `admin` passwords to `MANGO_DEV_ADMIN_PASSWORD` (default `mango-dev`).
- From the repository root, run `./local-dev/run-local.sh`. Crystal listens on `http://localhost:9000/`; Rust listens on `http://localhost:9001/`.
- For UI changes or behavior verification, use the browser tool to open both apps, sign in with the launcher’s printed credentials, exercise the affected flow, and compare the visible results. HTTP readiness and automated tests alone do not verify the UI.
