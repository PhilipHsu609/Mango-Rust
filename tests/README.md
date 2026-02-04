# Mango-Rust Tests

Lightweight test suite acting as ground truth for the application.

## Test Structure

```
tests/
  api/           # API contract tests (vitest, no browser)
    auth.test.ts     # Authentication, authorization, session management
    library.test.ts  # Library listing, title details, stats
    admin.test.ts    # Admin scan, user management
    progress.test.ts # Reading progress tracking
    opds.test.ts     # OPDS feed format and auth
    client.ts        # HTTP client helper
    setup.ts         # Test setup (server lifecycle, user creation)
  smoke/         # Minimal E2E tests (Playwright)
    smoke.spec.ts    # Login, navigation, logout
  helpers/       # Shared utilities
    auth.ts          # User creation
    server.ts        # Server lifecycle
```

## Running Tests

```bash
cd tests

# Install dependencies (first time)
npm install

# Run API contract tests (requires running server)
npm test

# Run smoke tests (requires running server)
npm run test:smoke

# Run both (CI mode)
npm run test:ci

# Watch mode during development
npm run test:watch
```

## Philosophy

- **API contracts are ground truth** - Tests verify response shapes, not CSS
- **Minimal E2E** - Only 4 smoke tests to verify the app boots and works
- **Fast feedback** - API tests run in seconds, not minutes
- **Stable** - Tests don't break when UI changes

## Test Counts

| Suite | Tests | Purpose |
|-------|-------|---------|
| Auth | 15 | Login, logout, session, admin access |
| Library | 6 | Listings, details, sorting, stats |
| Admin | 2 | Scan, user management |
| Progress | 2 | Read/write progress |
| OPDS | 3 | Feed format, auth |
| Smoke | 4 | Login, navigate, logout |

**Total: ~32 tests** (down from 121 brittle Playwright tests)

## Requirements

- Node.js 20+
- Running Mango server on localhost:9000
- Test users created (done automatically by setup)
