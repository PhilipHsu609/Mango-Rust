import { test, expect } from '../helpers/fixtures.js';
import { test as baseTest } from '@playwright/test';
import { login, logout, TEST_USER, REGULAR_USER } from '../helpers/auth.js';

/**
 * Progress API Contract Tests
 *
 * Tests the HTTP API contracts for progress tracking endpoints.
 * Validates save/retrieve progress, error handling, and bulk progress retrieval.
 * Uses authenticated page fixture (auto-login).
 */

/**
 * Helper: Get valid title and entry IDs from library
 * Returns first available title and entry, or skips test if library is empty
 */
async function getValidIds(page: any): Promise<{ titleId: string; entryId: string } | null> {
  const library = await (await page.request.get('/api/library')).json();

  if (library.length === 0) {
    return null;
  }

  const titleId = library[0].id;
  const titleDetails = await (await page.request.get(`/api/title/${titleId}`)).json();

  if (titleDetails.entries.length === 0) {
    return null;
  }

  const entryId = titleDetails.entries[0].id;
  return { titleId, entryId };
}

test.describe('Progress API: Contract Tests', () => {
  test('POST then GET returns saved progress', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryId } = ids;

    // Save progress to page 42
    const saveResponse = await page.request.post(`/api/progress/${titleId}/${entryId}`, {
      data: { page: 42 },
    });
    expect(saveResponse.ok()).toBe(true);

    // Retrieve progress
    const getResponse = await page.request.get(`/api/progress/${titleId}/${entryId}`);
    expect(getResponse.ok()).toBe(true);

    const data = await getResponse.json();
    expect(data).toMatchObject({
      page: 42,
    });
  });

  test('GET unread entry returns page 1 (minimum clamping)', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryId } = ids;

    // Get progress for an entry we haven't set progress for yet
    // Use a different entry to avoid collision with other tests
    const library = await (await page.request.get('/api/library')).json();

    // Try to find a second entry, or use a fake ID that will return default
    let testTitleId = titleId;
    let testEntryId = entryId;

    if (library.length > 1) {
      testTitleId = library[1].id;
      const titleDetails = await (await page.request.get(`/api/title/${testTitleId}`)).json();
      if (titleDetails.entries.length > 0) {
        testEntryId = titleDetails.entries[0].id;
      }
    }

    const response = await page.request.get(`/api/progress/${testTitleId}/${testEntryId}`);
    expect(response.ok()).toBe(true);

    const data = await response.json();
    // Actual API returns page 1 for unread (minimum clamping), not 0 or null
    expect(data).toMatchObject({
      page: 1,
    });
    expect(data.page).toBeGreaterThanOrEqual(1);
  });

  test('POST with invalid title returns 404', async ({ page }) => {
    const response = await page.request.post('/api/progress/nonexistent-title-12345/some-entry-id', {
      data: { page: 10 },
    });

    expect(response.status()).toBe(404);
  });

  test('POST with valid title but invalid entry returns 404', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId } = ids;

    // Use valid title ID but invalid entry ID
    const response = await page.request.post(`/api/progress/${titleId}/nonexistent-entry-12345`, {
      data: { page: 10 },
    });

    expect(response.status()).toBe(404);
  });

  test('GET /api/progress returns all user progress', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryId } = ids;

    // Save some progress first
    await page.request.post(`/api/progress/${titleId}/${entryId}`, {
      data: { page: 25 },
    });

    // Get all progress
    const response = await page.request.get('/api/progress');
    expect(response.ok()).toBe(true);

    const data = await response.json();

    // Should be an object (HashMap in Rust)
    expect(typeof data).toBe('object');
    expect(data).not.toBeNull();
    expect(Array.isArray(data)).toBe(false);

    // Should have at least the entry we just saved
    // Key format: "titleId:entryId"
    const key = `${titleId}:${entryId}`;
    expect(data).toHaveProperty(key);
    expect(data[key]).toBe(25);
  });
});

/**
 * Multi-User Progress Isolation Tests
 *
 * Tests that separate users maintain independent progress for the same entries.
 * Uses browser.newContext() to create separate sessions for different users.
 * Does NOT use auto-login fixture - manages authentication explicitly.
 */

baseTest.describe('Progress API: Multi-User Isolation', () => {
  baseTest('separate users maintain independent progress for same entry', async ({ browser }) => {
    const contextA = await browser.newContext();
    const contextB = await browser.newContext();
    const pageA = await contextA.newPage();
    const pageB = await contextB.newPage();

    try {
      await login(pageA, TEST_USER);
      await login(pageB, REGULAR_USER);

      const ids = await getValidIds(pageA);
      if (!ids) {
        baseTest.skip();
        return;
      }
      const { titleId, entryId } = ids;

      // User A saves page 10
      const saveA = await pageA.request.post(`/api/progress/${titleId}/${entryId}`, {
        data: { page: 10 },
      });
      expect(saveA.ok()).toBe(true);

      // User B saves page 5 for SAME entry
      const saveB = await pageB.request.post(`/api/progress/${titleId}/${entryId}`, {
        data: { page: 5 },
      });
      expect(saveB.ok()).toBe(true);

      // Verify isolation - each user retrieves their own value
      const respA = await pageA.request.get(`/api/progress/${titleId}/${entryId}`);
      const respB = await pageB.request.get(`/api/progress/${titleId}/${entryId}`);

      expect((await respA.json()).page).toBe(10);
      expect((await respB.json()).page).toBe(5);
    } finally {
      await contextA.close();
      await contextB.close();
    }
  });

  baseTest('user progress lists are isolated', async ({ browser }) => {
    const contextA = await browser.newContext();
    const contextB = await browser.newContext();
    const pageA = await contextA.newPage();
    const pageB = await contextB.newPage();

    try {
      await login(pageA, TEST_USER);
      await login(pageB, REGULAR_USER);

      const ids = await getValidIds(pageA);
      if (!ids) {
        baseTest.skip();
        return;
      }
      const { titleId, entryId } = ids;

      // User A saves progress with distinctive page number
      await pageA.request.post(`/api/progress/${titleId}/${entryId}`, {
        data: { page: 77 },
      });

      // User B's progress list should NOT contain User A's progress value
      const respB = await pageB.request.get('/api/progress');
      const progressB = await respB.json();

      const key = `${titleId}:${entryId}`;
      // Either key doesn't exist in User B's progress, OR if it exists, it's NOT 77
      if (key in progressB) {
        expect(progressB[key]).not.toBe(77);
      }
      // If key doesn't exist, test passes (User B has no progress for this entry)
    } finally {
      await contextA.close();
      await contextB.close();
    }
  });
});

/**
 * Progress Persistence Tests
 *
 * Tests that progress survives across logout/login cycles.
 * Uses single page context (not multi-user testing).
 */

baseTest.describe('Progress API: Persistence', () => {
  baseTest('progress persists across logout/login', async ({ page }) => {
    // Use REGULAR_USER to avoid interference with contract tests (which use TEST_USER)
    await login(page, REGULAR_USER);

    const ids = await getValidIds(page);
    if (!ids) {
      baseTest.skip();
      return;
    }
    const { titleId, entryId } = ids;

    // Save progress with distinctive page number
    await page.request.post(`/api/progress/${titleId}/${entryId}`, {
      data: { page: 99 },
    });

    // Logout (navigate to /logout, should redirect to /login)
    await logout(page);
    expect(page.url()).toContain('/login');

    // Login again as same user
    await login(page, REGULAR_USER);

    // Verify progress persisted across logout/login cycle
    const resp = await page.request.get(`/api/progress/${titleId}/${entryId}`);
    expect((await resp.json()).page).toBe(99);
  });
});
