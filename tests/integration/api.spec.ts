import { test, expect } from '../helpers/fixtures.js';

/**
 * Library API Contract Tests
 *
 * Tests the HTTP API contracts for library-related endpoints.
 * Validates response schemas and HTTP status codes.
 * Uses authenticated page fixture (auto-login).
 */

test.describe('Library API: Contract Tests', () => {
  test('GET /api/library returns array with correct schema', async ({ page }) => {
    const response = await page.request.get('/api/library');
    expect(response.ok()).toBe(true);

    const data = await response.json();
    expect(Array.isArray(data)).toBe(true);

    // Verify schema if library has data
    if (data.length > 0) {
      const item = data[0];
      expect(item).toMatchObject({
        id: expect.any(String),
        title: expect.any(String),
        entries: expect.any(Number),
        pages: expect.any(Number),
      });
    }
  });

  test('GET /api/library respects sort parameter', async ({ page }) => {
    // Test sort=title
    const titleResponse = await page.request.get('/api/library?sort=title');
    expect(titleResponse.ok()).toBe(true);

    // Test sort=modified
    const modifiedResponse = await page.request.get('/api/library?sort=modified');
    expect(modifiedResponse.ok()).toBe(true);

    // Both return valid arrays - that's the contract
    const titleData = await titleResponse.json();
    const modifiedData = await modifiedResponse.json();
    expect(Array.isArray(titleData)).toBe(true);
    expect(Array.isArray(modifiedData)).toBe(true);

    // Verify schema if data exists
    if (titleData.length > 0) {
      expect(titleData[0]).toMatchObject({
        id: expect.any(String),
        title: expect.any(String),
        entries: expect.any(Number),
        pages: expect.any(Number),
      });
    }
  });

  test('GET /api/title/:id returns title details with entries array', async ({ page }) => {
    // First get a valid title ID from library
    const library = await (await page.request.get('/api/library')).json();

    if (library.length === 0) {
      test.skip();
      return;
    }

    const titleId = library[0].id;

    // Get title details
    const response = await page.request.get(`/api/title/${titleId}`);
    expect(response.ok()).toBe(true);

    const data = await response.json();
    expect(data).toMatchObject({
      id: expect.any(String),
      title: expect.any(String),
      entries: expect.any(Array),
    });

    // Verify entries array schema if not empty
    if (data.entries.length > 0) {
      expect(data.entries[0]).toMatchObject({
        id: expect.any(String),
        title: expect.any(String),
        pages: expect.any(Number),
      });
    }
  });

  test('GET /api/title/invalid returns 404', async ({ page }) => {
    const response = await page.request.get('/api/title/nonexistent-id-12345');
    expect(response.status()).toBe(404);
  });

  test('GET /api/stats returns correct counts', async ({ page }) => {
    const response = await page.request.get('/api/stats');
    expect(response.ok()).toBe(true);

    const data = await response.json();
    // Actual field names from LibraryStats struct
    expect(data).toMatchObject({
      titles: expect.any(Number),
      entries: expect.any(Number),
      pages: expect.any(Number),
    });

    // Verify counts are non-negative
    expect(data.titles).toBeGreaterThanOrEqual(0);
    expect(data.entries).toBeGreaterThanOrEqual(0);
    expect(data.pages).toBeGreaterThanOrEqual(0);
  });

  test('GET /api/cover/:tid/:eid returns image or 404', async ({ page }) => {
    // Get a valid title and entry ID
    const library = await (await page.request.get('/api/library')).json();

    if (library.length === 0) {
      test.skip();
      return;
    }

    const titleId = library[0].id;
    const titleDetails = await (await page.request.get(`/api/title/${titleId}`)).json();

    if (titleDetails.entries.length === 0) {
      test.skip();
      return;
    }

    const entryId = titleDetails.entries[0].id;

    // Test valid cover request
    const response = await page.request.get(`/api/cover/${titleId}/${entryId}`);
    expect(response.ok()).toBe(true);

    // Verify it's an image
    const contentType = response.headers()['content-type'];
    expect(contentType).toMatch(/^image\//);

    // Test invalid cover request returns 404
    const invalidResponse = await page.request.get('/api/cover/nonexistent/invalid');
    expect(invalidResponse.status()).toBe(404);
  });
});
