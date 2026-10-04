# Mango-Rust tests

Requires Rust, Node.js 22+, `zip`, and the application's libarchive/pkg-config development dependencies.

## Run

From the repository root:

```sh
cargo test --all-targets
npm --prefix tests ci
npm --prefix tests run typecheck
npm --prefix tests test
```

Filter HTTP tests with `npm --prefix tests test -- api/progress.test.ts`.

## Ownership and isolation

- Rust tests live with their library modules and exercise behavior at the owning module's interface.
- HTTP tests in `api/` are grouped by feature: users, metadata, maintenance, catalog, sorting, tags, reading progress, authentication, CORS, OPDS, CLI, and reference authorization.
- `global-setup.ts` owns suite startup/teardown; `helpers/server.ts` owns individual server processes, CLI execution, and temporary configuration/database paths. `helpers/catalog.ts` resolves named fixtures through HTTP.
- `npm test` builds the actual Rust binary, creates seven deterministic titles with five ten-page ZIP entries each, seeds users through the CLI, waits for the initial scan, and cleans up afterward. No manual fixture setup or `HOME` changes are needed.
- Servers use private loopback ports. Shared mutation tests restore their state; authentication modes, CLI mutations, and cover uploads use private servers. Test files remain sequential because the main server is shared.

CI runs both the Rust and HTTP suites, plus TypeScript checking. Application assets are built by the workflow.
