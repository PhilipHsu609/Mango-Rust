import { test, expect } from '../helpers/fixtures.js';
import { TEST_USER } from '../helpers/auth.js';

/**
 * OPDS Catalog Tests
 *
 * Tests OPDS catalog structure and XML format for e-reader compatibility.
 * Authentication is covered in auth.spec.ts - these tests focus on catalog structure.
 * Uses Basic Auth via httpCredentials for OPDS spec compliance.
 */

/**
 * OPDS Main Catalog Tests
 * Tests GET /opds endpoint - main catalog listing all titles
 */
test.describe('OPDS: Main Catalog', () => {
  test('GET /opds returns valid Atom XML', async ({ page }) => {
    const response = await page.request.get('/opds', {
      httpCredentials: {
        username: TEST_USER.username,
        password: TEST_USER.password,
      },
    });

    expect(response.ok()).toBe(true);

    // Verify Content-Type header
    const contentType = response.headers()['content-type'];
    expect(contentType).toContain('application/atom+xml');

    // Verify basic XML structure
    const body = await response.text();
    expect(body).toContain('<?xml');
    expect(body).toContain('<feed');
    expect(body).toContain('xmlns="http://www.w3.org/2005/Atom"');
  });

  test('OPDS catalog contains feed elements', async ({ page }) => {
    const response = await page.request.get('/opds', {
      httpCredentials: {
        username: TEST_USER.username,
        password: TEST_USER.password,
      },
    });

    const body = await response.text();

    // Required Atom feed elements
    expect(body).toContain('<feed');
    expect(body).toContain('<id>');
    expect(body).toContain('<title>');
    expect(body).toContain('</feed>');
  });
});

/**
 * OPDS Title Feed Tests
 * Tests GET /opds/book/:id endpoint - individual title with entries
 */
test.describe('OPDS: Title Feed', () => {
  test('GET /opds/book/:id returns entry feed', async ({ page }) => {
    // Get valid title ID from library
    const library = await (await page.request.get('/api/library')).json();

    if (library.length === 0) {
      test.skip();
      return;
    }

    const titleId = library[0].id;

    const response = await page.request.get(`/opds/book/${titleId}`, {
      httpCredentials: {
        username: TEST_USER.username,
        password: TEST_USER.password,
      },
    });

    expect(response.ok()).toBe(true);

    // Verify Content-Type header
    const contentType = response.headers()['content-type'];
    expect(contentType).toContain('application/atom+xml');

    // Verify feed contains title ID
    const body = await response.text();
    expect(body).toContain('<feed');
    expect(body).toContain(`urn:mango:${titleId}`);
  });

  test('OPDS entry includes acquisition links', async ({ page }) => {
    // Get valid title with entries
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

    const response = await page.request.get(`/opds/book/${titleId}`, {
      httpCredentials: {
        username: TEST_USER.username,
        password: TEST_USER.password,
      },
    });

    const body = await response.text();

    // Verify entry with acquisition link exists
    expect(body).toContain('<entry>');
    expect(body).toContain('rel="http://opds-spec.org/acquisition"');
    expect(body).toContain('href=');
    expect(body).toContain('/api/download/');
  });

  test('download links are valid URLs', async ({ page }) => {
    // Get valid title with entries
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

    const response = await page.request.get(`/opds/book/${titleId}`, {
      httpCredentials: {
        username: TEST_USER.username,
        password: TEST_USER.password,
      },
    });

    const body = await response.text();

    // Verify download link follows expected pattern
    const expectedDownloadPath = `/api/download/${titleId}/${entryId}`;
    expect(body).toContain(expectedDownloadPath);
  });
});
