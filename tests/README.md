# Mango-Rust Integration Tests

Comprehensive integration testing framework for Mango-Rust using Playwright and TypeScript.

## Overview

This test framework provides end-to-end integration tests covering:
- **Theme System**: Light/dark theme toggle, persistence, cross-page consistency
- **Reader Functionality**: Page navigation, mode switching (paged/continuous), settings
- **Navigation**: Desktop/mobile navigation, active link highlighting, hamburger menu
- **Library Features**: Search filtering, sorting (name/date/progress), title display

## Prerequisites

- **Node.js** 18+ and npm
- **Rust** toolchain (for building Mango server)
- **Mango** database populated with test data (optional but recommended)

## Quick Start

### 1. Install Dependencies

```bash
cd tests
npm install
```

### 2. Run Tests

```bash
# Run all tests (headless mode)
npm test

# Run with browser visible (debugging)
npm run test:headed

# Run specific test suite
npm run test:theme
npm run test:reader
npm run test:navigation
npm run test:library

# Interactive UI mode
npm run test:ui

# Debug mode with Playwright Inspector
npm run test:debug
```

### 3. View Reports

```bash
# Open HTML test report
npm run test:report
```

## Test Organization

```
tests/
├── integration/           # Test suites
│   ├── theme.spec.ts     # Theme toggle tests (9 tests)
│   ├── reader.spec.ts    # Reader functionality tests (10 tests)
│   ├── navigation.spec.ts # Navigation tests (12 tests)
│   └── library.spec.ts   # Library search/sort tests (12 tests)
├── helpers/              # Reusable utilities
│   ├── server.ts         # Server lifecycle management
│   ├── test-utils.ts     # Common test utilities
│   ├── theme-utils.ts    # Theme verification helpers
│   ├── page-objects.ts   # Page Object Models
│   └── fixtures.ts       # Test fixtures
├── global-setup.ts       # Build CSS and start server
├── global-teardown.ts    # Stop server and cleanup
└── playwright.config.ts  # Playwright configuration
```

## Writing Tests

### Using Page Objects

```typescript
import { test, expect } from '@playwright/test';
import { LibraryPage } from '../helpers/page-objects.js';

test('should search library', async ({ page }) => {
  const library = new LibraryPage(page);

  await library.navigate();
  await library.search('manga');

  const count = await library.getTitleCount();
  expect(count).toBeGreaterThan(0);
});
```

### Using Test Fixtures

```typescript
import { test, expect } from '../helpers/fixtures.js';

test('test with dark theme', async ({ darkThemePage }) => {
  // Page already has dark theme applied
  await darkThemePage.goto('/library');
  // ... rest of test
});
```

### Using Theme Utilities

```typescript
import { verifyTheme, toggleTheme } from '../helpers/theme-utils.js';

test('should toggle theme', async ({ page }) => {
  await page.goto('/library');

  await toggleTheme(page);
  await verifyTheme(page, 'dark');
});
```

## Configuration

### Test Execution

Edit `playwright.config.ts` to customize:
- Browsers to test (currently Chromium)
- Timeouts (default 60s)
- Retries (2 retries in CI, 0 locally)
- Base URL (default http://localhost:9000)
- Screenshots (only on failure)

### Code Quality

```bash
# Run ESLint
npm run lint

# Fix linting issues automatically
npm run lint:fix

# Format code with Prettier
npm run format

# Check formatting
npm run format:check
```

## CI/CD Integration

Tests are designed to run in CI environments with automatic server startup and cleanup.

### GitHub Actions Example

See `.github/workflows/integration-tests.yml` for full CI workflow.

Key CI configuration:
- Builds CSS before tests
- Starts Mango server automatically
- Uploads test reports and screenshots as artifacts
- Fails PR if tests fail

## Troubleshooting

### Server Won't Start

**Problem**: "Server failed to start within 30000ms"

**Solutions**:
- Ensure Mango builds successfully: `cargo build --release`
- Check database exists: `$HOME/mango/mango.db`
- Verify port 9000 is not in use: `lsof -i :9000`
- Check server logs in test output

### Tests Timing Out

**Problem**: Tests fail with timeout errors

**Solutions**:
- Increase timeout in `playwright.config.ts`: `timeout: 90000`
- Check network is stable
- Ensure Mango server is responsive
- Run tests headed to see what's happening: `npm run test:headed`

### Theme Tests Failing

**Problem**: Theme toggle tests fail

**Solutions**:
- Ensure CSS is built: Run `./build-css.sh` from project root
- Clear browser state: Tests should use `clearBrowserState()` helper
- Check localStorage is enabled in test browser

### Screenshot/Evidence Not Captured

**Problem**: No screenshots in `screenshots/` directory

**Solutions**:
- Screenshots only captured on failure by default
- Use `captureEvidence(page, 'name')` manually in tests
- Check `screenshots/` directory is not ignored

### Linting Errors

**Problem**: ESLint reports errors

**Solutions**:
- Auto-fix: `npm run lint:fix`
- Check ESLint config: `eslint.config.js`
- Common warnings about conditionals in tests are acceptable (needed for dynamic testing)

## Test Coverage

Current test coverage:
- ✅ **Theme System**: 9 tests (initial state, toggle, persistence, cross-page, colors)
- ✅ **Reader**: 10 tests (loading, navigation, modes, settings, localStorage)
- ✅ **Navigation**: 12 tests (desktop, mobile, active states, routing)
- ✅ **Library**: 12 tests (search, sort, title cards, case-insensitive)

**Total**: 43 integration tests

## Performance

- Test execution: ~2-3 minutes for full suite
- Server startup: ~10-15 seconds
- CSS build: ~2-3 seconds

Shared server instance optimizes performance - server starts once, runs for all tests.

## Contributing

When adding new tests:
1. Use Page Object Model pattern for reusable page interactions
2. Use helper utilities (`test-utils.ts`, `theme-utils.ts`) for common operations
3. Add screenshots with `captureEvidence()` for visual verification
4. Follow existing test structure and naming conventions
5. Ensure tests pass in both local and CI environments

## License

MIT
