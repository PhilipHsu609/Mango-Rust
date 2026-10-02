# Mango-Rust tests

## Suites

- Rust unit and library tests: `cargo test` from the repository root.
- HTTP/API contract tests: `tests/api/*.test.ts` with Vitest. Vitest starts and stops the Rust server; invalid form logins redirect to `/login` as in Mango.
- Archive fixtures: `tests/fixtures/setup-test-library.sh`.

## Running

Install application and test dependencies from the repository root:

```bash
npm ci
npm run build
(cd tests && npm ci)
```

Run the suites using disposable test data and retain the installed Rust toolchain/cache paths:

```bash
ORIGINAL_HOME="$HOME"
export CARGO_HOME="${CARGO_HOME:-$ORIGINAL_HOME/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$ORIGINAL_HOME/.rustup}"
TEST_HOME="$(mktemp -d)"
trap 'rm -rf "$TEST_HOME"' EXIT
export HOME="$TEST_HOME"

bash tests/fixtures/setup-test-library.sh
cargo test
(cd tests && CI=true npm test)
```

CI builds the Rust binary and runs the API tests. It does not run Rust unit tests; run `cargo test` separately.
