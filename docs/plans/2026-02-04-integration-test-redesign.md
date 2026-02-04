# Integration Test Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace 121 brittle Playwright E2E tests with ~40 robust tests that act as ground truth for API contracts, data integrity, and core functionality.

**Architecture:** Layered testing pyramid - Rust unit/integration tests at the base, HTTP API contract tests in the middle, minimal Playwright smoke tests at the top.

**Tech Stack:** Rust `cargo test` with `axum_test`, Node.js `vitest` for API contracts, Playwright for 5 smoke tests only.

---

## Task 1: Clean Up Existing Test Infrastructure

**Files:**
- Delete: `tests/integration/*.spec.ts` (all 16 spec files)
- Keep: `tests/helpers/auth.ts` (reuse user creation logic)
- Keep: `tests/helpers/server.ts` (reuse server lifecycle)
- Modify: `tests/package.json`

**Step 1: Remove all Playwright spec files**

```bash
rm -rf tests/integration/*.spec.ts
```

**Step 2: Remove unused helpers**

```bash
rm tests/helpers/fixtures.ts      # Playwright fixtures - not needed
rm tests/helpers/page-objects.ts  # Page objects - not needed
rm tests/helpers/theme-utils.ts   # Theme testing - not needed
rm tests/helpers/test-utils.ts    # Screenshot utils - not needed
```

**Step 3: Update package.json**

Replace test scripts with new structure:

```json
{
  "scripts": {
    "test": "vitest run",
    "test:watch": "vitest",
    "test:smoke": "playwright test",
    "test:ci": "vitest run && playwright test"
  },
  "devDependencies": {
    "@playwright/test": "^1.48.0",
    "vitest": "^2.0.0",
    "better-sqlite3": "^12.4.6",
    "bcryptjs": "^3.0.3",
    "@types/better-sqlite3": "^7.6.13",
    "@types/bcryptjs": "^2.4.6",
    "@types/node": "^22.9.0",
    "typescript": "^5.6.3"
  }
}
```

**Step 4: Commit**

```bash
git add -A
git commit -m "test: remove brittle Playwright E2E tests"
```

---

## Task 2: Set Up Vitest for API Contract Tests

**Files:**
- Create: `tests/vitest.config.ts`
- Create: `tests/api/setup.ts`
- Modify: `tests/tsconfig.json`

**Step 1: Create vitest config**

Create `tests/vitest.config.ts`:

```typescript
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    include: ['api/**/*.test.ts'],
    setupFiles: ['./api/setup.ts'],
    testTimeout: 10000,
    hookTimeout: 30000,
  },
});
```

**Step 2: Create test setup**

Create `tests/api/setup.ts`:

```typescript
import { beforeAll, afterAll } from 'vitest';
import { startServer, stopServer, waitForServerReady } from '../helpers/server.js';
import { createTestUser, TEST_USER } from '../helpers/auth.js';
import * as path from 'path';
import * as fs from 'fs/promises';

const TEST_DATA_DIR = path.join(process.env.HOME || '', 'test-manga-library');

beforeAll(async () => {
  // Create test config
  const configPath = path.join(TEST_DATA_DIR, 'config-test.yml');
  await fs.mkdir(TEST_DATA_DIR, { recursive: true });

  const testConfig = `host: localhost
port: 9000
library_path: ${TEST_DATA_DIR}
db_path: ${TEST_DATA_DIR}/mango-test.db
log_level: info
`;
  await fs.writeFile(configPath, testConfig, 'utf-8');

  // Start server
  await startServer();
  await waitForServerReady();

  // Create test user
  const dbPath = path.join(TEST_DATA_DIR, 'mango-test.db');
  await createTestUser(dbPath);
}, 60000);

afterAll(async () => {
  await stopServer();
});
```

**Step 3: Update tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "esModuleInterop": true,
    "strict": true,
    "skipLibCheck": true,
    "types": ["vitest/globals", "node"]
  },
  "include": ["**/*.ts"],
  "exclude": ["node_modules"]
}
```

**Step 4: Commit**

```bash
git add tests/vitest.config.ts tests/api/setup.ts tests/tsconfig.json
git commit -m "test: set up vitest for API contract tests"
```

---

## Task 3: Create API Contract Test Utilities

**Files:**
- Create: `tests/api/client.ts`

**Step 1: Create HTTP client helper**

Create `tests/api/client.ts`:

```typescript
const BASE_URL = process.env.BASE_URL || 'http://localhost:9000';

export interface ApiClient {
  get: (path: string) => Promise<Response>;
  post: (path: string, body?: unknown) => Promise<Response>;
  patch: (path: string, body?: unknown) => Promise<Response>;
  delete: (path: string) => Promise<Response>;
}

let sessionCookie: string | null = null;

export async function login(username = 'testuser', password = 'testpass123'): Promise<void> {
  const response = await fetch(`${BASE_URL}/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({ username, password }),
    redirect: 'manual',
  });

  const setCookie = response.headers.get('set-cookie');
  if (setCookie) {
    sessionCookie = setCookie.split(';')[0];
  }
}

export async function logout(): Promise<void> {
  sessionCookie = null;
}

function getHeaders(): HeadersInit {
  const headers: HeadersInit = { 'Content-Type': 'application/json' };
  if (sessionCookie) {
    headers['Cookie'] = sessionCookie;
  }
  return headers;
}

export const api: ApiClient = {
  get: (path: string) => fetch(`${BASE_URL}${path}`, { headers: getHeaders() }),
  post: (path: string, body?: unknown) => fetch(`${BASE_URL}${path}`, {
    method: 'POST',
    headers: getHeaders(),
    body: body ? JSON.stringify(body) : undefined,
  }),
  patch: (path: string, body?: unknown) => fetch(`${BASE_URL}${path}`, {
    method: 'PATCH',
    headers: getHeaders(),
    body: body ? JSON.stringify(body) : undefined,
  }),
  delete: (path: string) => fetch(`${BASE_URL}${path}`, {
    method: 'DELETE',
    headers: getHeaders(),
  }),
};
```

**Step 2: Commit**

```bash
git add tests/api/client.ts
git commit -m "test: add HTTP client helper for API tests"
```

---

## Task 4: Write Auth API Contract Tests

**Files:**
- Create: `tests/api/auth.test.ts`

**Step 1: Create auth contract tests**

Create `tests/api/auth.test.ts`:

```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import { api, login, logout } from './client.js';

const BASE_URL = process.env.BASE_URL || 'http://localhost:9000';

describe('Auth API', () => {
  beforeEach(async () => {
    await logout();
  });

  describe('POST /login', () => {
    it('valid credentials sets session cookie and redirects', async () => {
      const response = await fetch(`${BASE_URL}/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({ username: 'testuser', password: 'testpass123' }),
        redirect: 'manual',
      });

      expect(response.status).toBe(303);
      expect(response.headers.get('set-cookie')).toContain('session=');
      expect(response.headers.get('location')).toBe('/');
    });

    it('invalid credentials returns 401', async () => {
      const response = await fetch(`${BASE_URL}/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({ username: 'testuser', password: 'wrongpassword' }),
        redirect: 'manual',
      });

      expect(response.status).toBe(401);
    });
  });

  describe('GET /logout', () => {
    it('clears session and redirects to login', async () => {
      await login();

      const response = await fetch(`${BASE_URL}/logout`, {
        headers: { 'Cookie': 'session=test' },
        redirect: 'manual',
      });

      expect(response.status).toBe(303);
      expect(response.headers.get('location')).toBe('/login');
    });
  });

  describe('Protected routes', () => {
    it('unauthenticated request to /api/library redirects to login', async () => {
      const response = await fetch(`${BASE_URL}/api/library`, {
        redirect: 'manual',
      });

      expect([401, 303]).toContain(response.status);
    });

    it('authenticated request to /api/library succeeds', async () => {
      await login();
      const response = await api.get('/api/library');

      expect(response.status).toBe(200);
    });
  });
});
```

**Step 2: Run test to verify**

```bash
cd tests && npm test -- api/auth.test.ts
```

**Step 3: Commit**

```bash
git add tests/api/auth.test.ts
git commit -m "test: add auth API contract tests"
```

---

## Task 5: Write Library API Contract Tests

**Files:**
- Create: `tests/api/library.test.ts`

**Step 1: Create library contract tests**

Create `tests/api/library.test.ts`:

```typescript
import { describe, it, expect, beforeAll } from 'vitest';
import { api, login } from './client.js';

describe('Library API', () => {
  beforeAll(async () => {
    await login();
  });

  describe('GET /api/library', () => {
    it('returns array of titles', async () => {
      const response = await api.get('/api/library');
      expect(response.status).toBe(200);

      const data = await response.json();
      expect(Array.isArray(data)).toBe(true);
    });

    it('title objects have required fields', async () => {
      const response = await api.get('/api/library');
      const data = await response.json();

      if (data.length > 0) {
        const title = data[0];
        expect(title).toHaveProperty('id');
        expect(title).toHaveProperty('title');
        expect(title).toHaveProperty('entries');
        expect(typeof title.id).toBe('string');
        expect(typeof title.title).toBe('string');
        expect(typeof title.entries).toBe('number');
      }
    });

    it('respects sort parameter', async () => {
      const ascResponse = await api.get('/api/library?sort=title&order=asc');
      const descResponse = await api.get('/api/library?sort=title&order=desc');

      expect(ascResponse.status).toBe(200);
      expect(descResponse.status).toBe(200);

      const ascData = await ascResponse.json();
      const descData = await descResponse.json();

      if (ascData.length > 1) {
        expect(ascData[0].title).not.toBe(descData[0].title);
      }
    });
  });

  describe('GET /api/title/:id', () => {
    it('returns title details with entries array', async () => {
      const libraryResponse = await api.get('/api/library');
      const library = await libraryResponse.json();

      if (library.length > 0) {
        const titleId = library[0].id;
        const response = await api.get(`/api/title/${titleId}`);

        expect(response.status).toBe(200);

        const title = await response.json();
        expect(title).toHaveProperty('id');
        expect(title).toHaveProperty('title');
        expect(title).toHaveProperty('entries');
        expect(Array.isArray(title.entries)).toBe(true);
      }
    });

    it('returns 404 for invalid title ID', async () => {
      const response = await api.get('/api/title/nonexistent-id');
      expect(response.status).toBe(404);
    });
  });

  describe('GET /api/stats', () => {
    it('returns library statistics', async () => {
      const response = await api.get('/api/stats');
      expect(response.status).toBe(200);

      const stats = await response.json();
      expect(stats).toHaveProperty('titles');
      expect(stats).toHaveProperty('entries');
      expect(stats).toHaveProperty('pages');
      expect(typeof stats.titles).toBe('number');
      expect(typeof stats.entries).toBe('number');
      expect(typeof stats.pages).toBe('number');
    });
  });
});
```

**Step 2: Commit**

```bash
git add tests/api/library.test.ts
git commit -m "test: add library API contract tests"
```

---

## Task 6: Write Admin API Contract Tests

**Files:**
- Create: `tests/api/admin.test.ts`

**Step 1: Create admin contract tests**

Create `tests/api/admin.test.ts`:

```typescript
import { describe, it, expect, beforeAll } from 'vitest';
import { api, login } from './client.js';

describe('Admin API', () => {
  beforeAll(async () => {
    await login(); // testuser is admin
  });

  describe('POST /api/admin/scan', () => {
    it('triggers library scan and returns results', async () => {
      const response = await api.post('/api/admin/scan');

      expect(response.status).toBe(200);

      const result = await response.json();
      expect(result).toHaveProperty('titles');
      expect(result).toHaveProperty('entries');
      expect(typeof result.titles).toBe('number');
      expect(typeof result.entries).toBe('number');
    });
  });

  describe('GET /api/admin/users', () => {
    it('returns list of users', async () => {
      const response = await api.get('/api/admin/users');

      expect(response.status).toBe(200);

      const users = await response.json();
      expect(Array.isArray(users)).toBe(true);

      if (users.length > 0) {
        expect(users[0]).toHaveProperty('username');
        expect(users[0]).toHaveProperty('admin');
      }
    });
  });

  describe('Admin access control', () => {
    it('non-admin gets 403 for admin endpoints', async () => {
      await login('testuser2', 'testpass123'); // Regular user

      const response = await api.get('/api/admin/users');
      expect(response.status).toBe(403);

      // Re-login as admin for other tests
      await login();
    });
  });
});
```

**Step 2: Commit**

```bash
git add tests/api/admin.test.ts
git commit -m "test: add admin API contract tests"
```

---

## Task 7: Write Progress API Contract Tests

**Files:**
- Create: `tests/api/progress.test.ts`

**Step 1: Create progress contract tests**

Create `tests/api/progress.test.ts`:

```typescript
import { describe, it, expect, beforeAll } from 'vitest';
import { api, login } from './client.js';

describe('Progress API', () => {
  beforeAll(async () => {
    await login();
  });

  describe('PUT /api/progress/:tid/:eid', () => {
    it('updates reading progress', async () => {
      // Get a title with entries first
      const libraryResponse = await api.get('/api/library');
      const library = await libraryResponse.json();

      if (library.length === 0) {
        console.log('No titles in library, skipping progress test');
        return;
      }

      const titleId = library[0].id;
      const titleResponse = await api.get(`/api/title/${titleId}`);
      const title = await titleResponse.json();

      if (!title.entries || title.entries.length === 0) {
        console.log('No entries in title, skipping progress test');
        return;
      }

      const entryId = title.entries[0].id;

      // Update progress
      const response = await fetch(
        `http://localhost:9000/api/progress/${titleId}/${entryId}`,
        {
          method: 'PUT',
          headers: {
            'Content-Type': 'application/json',
            Cookie: 'session=test',
          },
          body: JSON.stringify({ page: 5 }),
        }
      );

      expect([200, 204]).toContain(response.status);
    });
  });

  describe('GET /api/progress', () => {
    it('returns user progress for all entries', async () => {
      const response = await api.get('/api/progress');

      expect(response.status).toBe(200);

      const progress = await response.json();
      expect(typeof progress).toBe('object');
    });
  });
});
```

**Step 2: Commit**

```bash
git add tests/api/progress.test.ts
git commit -m "test: add progress API contract tests"
```

---

## Task 8: Write OPDS API Contract Tests

**Files:**
- Create: `tests/api/opds.test.ts`

**Step 1: Create OPDS contract tests**

Create `tests/api/opds.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';

const BASE_URL = process.env.BASE_URL || 'http://localhost:9000';
const AUTH_HEADER = 'Basic ' + Buffer.from('testuser:testpass123').toString('base64');

describe('OPDS API', () => {
  describe('GET /opds', () => {
    it('returns valid Atom XML feed', async () => {
      const response = await fetch(`${BASE_URL}/opds`, {
        headers: { Authorization: AUTH_HEADER },
      });

      expect(response.status).toBe(200);
      expect(response.headers.get('content-type')).toContain('application/atom+xml');

      const xml = await response.text();
      expect(xml).toContain('<?xml');
      expect(xml).toContain('<feed');
      expect(xml).toContain('xmlns="http://www.w3.org/2005/Atom"');
    });

    it('requires authentication', async () => {
      const response = await fetch(`${BASE_URL}/opds`);

      expect(response.status).toBe(401);
    });

    it('supports HTTP Basic Auth', async () => {
      const response = await fetch(`${BASE_URL}/opds`, {
        headers: { Authorization: AUTH_HEADER },
      });

      expect(response.status).toBe(200);
    });
  });

  describe('GET /opds/all', () => {
    it('returns library feed', async () => {
      const response = await fetch(`${BASE_URL}/opds/all`, {
        headers: { Authorization: AUTH_HEADER },
      });

      expect(response.status).toBe(200);

      const xml = await response.text();
      expect(xml).toContain('<feed');
    });
  });
});
```

**Step 2: Commit**

```bash
git add tests/api/opds.test.ts
git commit -m "test: add OPDS API contract tests"
```

---

## Task 9: Create Minimal Playwright Smoke Tests

**Files:**
- Create: `tests/smoke/smoke.spec.ts`
- Create: `tests/playwright.config.ts`

**Step 1: Create simplified Playwright config**

Create `tests/playwright.config.ts`:

```typescript
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './smoke',
  fullyParallel: false,
  retries: 0,
  workers: 1,
  timeout: 30000,
  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:9000',
    headless: true,
    screenshot: 'only-on-failure',
  },
});
```

**Step 2: Create smoke tests**

Create `tests/smoke/smoke.spec.ts`:

```typescript
import { test, expect } from '@playwright/test';

test.describe('Smoke Tests', () => {
  test('login page loads', async ({ page }) => {
    await page.goto('/login');
    await expect(page.locator('input[name="username"]')).toBeVisible();
    await expect(page.locator('input[name="password"]')).toBeVisible();
  });

  test('can log in and reach home page', async ({ page }) => {
    await page.goto('/login');
    await page.fill('input[name="username"]', 'testuser');
    await page.fill('input[name="password"]', 'testpass123');
    await page.click('button[type="submit"]');

    await expect(page).toHaveURL('/');
  });

  test('library page shows content', async ({ page }) => {
    // Login first
    await page.goto('/login');
    await page.fill('input[name="username"]', 'testuser');
    await page.fill('input[name="password"]', 'testpass123');
    await page.click('button[type="submit"]');

    // Go to library
    await page.goto('/library');
    await expect(page).toHaveURL('/library');
  });

  test('reader can display an image', async ({ page }) => {
    // Login
    await page.goto('/login');
    await page.fill('input[name="username"]', 'testuser');
    await page.fill('input[name="password"]', 'testpass123');
    await page.click('button[type="submit"]');

    // Get first title from API
    const libraryResponse = await page.request.get('/api/library');
    const library = await libraryResponse.json();

    if (library.length === 0) {
      test.skip();
      return;
    }

    const titleResponse = await page.request.get(`/api/title/${library[0].id}`);
    const title = await titleResponse.json();

    if (!title.entries || title.entries.length === 0) {
      test.skip();
      return;
    }

    // Navigate to reader
    await page.goto(`/reader/${title.id}/${title.entries[0].id}/1`);

    // Verify image loads
    const img = page.locator('img').first();
    await expect(img).toBeVisible({ timeout: 10000 });
  });

  test('logout works', async ({ page }) => {
    // Login
    await page.goto('/login');
    await page.fill('input[name="username"]', 'testuser');
    await page.fill('input[name="password"]', 'testpass123');
    await page.click('button[type="submit"]');

    // Logout
    await page.goto('/logout');

    // Should be redirected to login
    await expect(page).toHaveURL(/\/login/);
  });
});
```

**Step 3: Commit**

```bash
git add tests/smoke/ tests/playwright.config.ts
git commit -m "test: add minimal Playwright smoke tests (5 tests)"
```

---

## Task 10: Update Documentation and Clean Up

**Files:**
- Modify: `tests/README.md`
- Delete: Unused files

**Step 1: Clean up old files**

```bash
rm -f tests/global-setup.ts tests/global-teardown.ts
rm -f tests/eslint.config.js tests/.prettierrc.json
rm -rf tests/helpers/fixtures.ts tests/helpers/page-objects.ts
rm -rf tests/helpers/theme-utils.ts tests/helpers/test-utils.ts
rm -rf tests/RELIABILITY_REPORT.md
```

**Step 2: Update README.md**

Create `tests/README.md`:

```markdown
# Mango-Rust Tests

Lightweight test suite acting as ground truth for the application.

## Test Structure

```
tests/
  api/           # API contract tests (vitest, no browser)
    auth.test.ts
    library.test.ts
    admin.test.ts
    progress.test.ts
    opds.test.ts
  smoke/         # Minimal E2E smoke tests (Playwright)
    smoke.spec.ts
  helpers/       # Shared utilities
    auth.ts      # User creation
    server.ts    # Server lifecycle
```

## Running Tests

```bash
# All API contract tests (fast, ~10 seconds)
npm test

# Smoke tests only (requires running server)
npm run test:smoke

# Both (CI mode)
npm run test:ci

# Watch mode during development
npm run test:watch
```

## Philosophy

- **API contracts are ground truth** - Tests verify response shapes, not CSS
- **Minimal E2E** - Only 5 smoke tests to verify the app boots
- **Fast feedback** - Full suite runs in seconds, not minutes
- **Stable** - Tests don't break when UI changes

## Test Counts

| Suite | Tests | Purpose |
|-------|-------|---------|
| Auth API | 5 | Login, logout, session, protection |
| Library API | 6 | List, details, sorting, stats |
| Admin API | 3 | Scan, users, access control |
| Progress API | 2 | Read/write progress |
| OPDS API | 4 | Feed format, auth |
| Smoke | 5 | App boots, can login/read |

**Total: ~25 tests** (down from 121)
```

**Step 3: Final commit**

```bash
git add -A
git commit -m "test: complete test redesign - 25 robust tests replacing 121 brittle ones"
```

---

## Summary

After completing all tasks:

| Before | After |
|--------|-------|
| 121 Playwright E2E tests | 25 focused tests |
| ~3 minutes runtime | ~10 seconds runtime |
| Breaks on CSS changes | Stable contracts |
| Tests HTML structure | Tests API behavior |

**Test pyramid now correct:**

```
      /-----\
     / 5 E2E \       <- Smoke only
    /---------\
   /  20 API   \     <- Contract tests
  /-------------\
 / Rust unit/int \   <- Core logic (future)
 -----------------
```
