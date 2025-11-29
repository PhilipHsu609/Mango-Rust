import { test, expect } from '../helpers/fixtures.js';
import { BookPage } from '../helpers/page-objects.js';

/**
 * Book Page and Entry Modal Tests
 *
 * Tests book detail page functionality and entry modal interactions.
 * Validates navigation, modal display, and reader navigation buttons.
 * Uses authenticated page fixture (auto-login).
 */

/**
 * Helper: Get valid title and entry IDs from library
 * Returns first available title and entry, or skips test if library is empty
 */
async function getValidIds(page: any): Promise<{ titleId: string; entryId: string; entryName: string; pages: number } | null> {
  const library = await (await page.request.get('/api/library')).json();

  if (library.length === 0) {
    return null;
  }

  const titleId = library[0].id;
  const titleDetails = await (await page.request.get(`/api/title/${titleId}`)).json();

  if (titleDetails.entries.length === 0) {
    return null;
  }

  const entry = titleDetails.entries[0];
  return {
    titleId,
    entryId: entry.id,
    entryName: entry.title,  // API returns 'title' not 'display_name'
    pages: entry.pages,
  };
}

test.describe('Book Page: Navigation and Entry Display', () => {
  test('navigates to book page and displays entries', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId } = ids;

    const bookPage = new BookPage(page);
    await bookPage.navigate(titleId);

    // Verify entries loaded
    const entryCount = await bookPage.getEntryCount();
    expect(entryCount).toBeGreaterThan(0);

    // Verify page URL
    expect(page.url()).toContain(`/book/${titleId}`);
  });

  test('entry cards display metadata (title and pages)', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId } = ids;

    const bookPage = new BookPage(page);
    await bookPage.navigate(titleId);

    // Get first entry card
    const firstCard = bookPage.getEntryCards().first();

    // Verify card structure
    await expect(firstCard.locator('.entry-name')).toBeVisible();
    await expect(firstCard.locator('.entry-stats')).toBeVisible();

    // Verify stats contain "pages"
    const statsText = await firstCard.locator('.entry-stats').textContent();
    expect(statsText).toContain('pages');
  });

  test('click entry opens modal', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryName } = ids;

    const bookPage = new BookPage(page);
    await bookPage.navigate(titleId);

    // Click entry card
    await bookPage.clickEntry(entryName);

    // Verify modal is open
    expect(await bookPage.isModalOpen()).toBe(true);

    // Verify modal contains entry name
    const modalTitle = page.locator('#entry-modal h3');
    await expect(modalTitle).toContainText(entryName);
  });
});

test.describe.serial('Book Page: Entry Modal Buttons', () => {
  test('entry with no progress shows only FROM BEGINNING button', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryId, entryName } = ids;

    // CRITICAL: Clear any existing progress first (page: 0 deletes progress)
    await page.request.post(`/api/progress/${titleId}/${entryId}`, {
      data: { page: 0 },
    });

    const bookPage = new BookPage(page);
    await bookPage.navigate(titleId); // Navigate AFTER clearing progress

    await bookPage.clickEntry(entryName);

    // Verify modal is open
    expect(await bookPage.isModalOpen()).toBe(true);

    // Verify only FROM BEGINNING is visible
    expect(await bookPage.isContinueVisible()).toBe(false);

    // Verify FROM BEGINNING href points to page 1
    const href = await bookPage.getFromBeginningHref();
    expect(href).toBe(`/reader/${titleId}/${entryId}/1`);
  });

  test('entry with progress shows both FROM BEGINNING and CONTINUE buttons', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryId, entryName, pages } = ids;

    // Save progress at 40% of pages (ensures 0 < progress < 100)
    // Use Math.max to ensure we're at least on page 2, and Math.min to ensure we're not at the last page
    const savedPage = Math.max(2, Math.min(Math.floor(pages * 0.4), pages - 2));
    await page.request.post(`/api/progress/${titleId}/${entryId}`, {
      data: { page: savedPage },
    });

    const bookPage = new BookPage(page);
    await bookPage.navigate(titleId); // Navigate AFTER saving progress (server-side render)

    await bookPage.clickEntry(entryName);

    // Verify modal is open
    expect(await bookPage.isModalOpen()).toBe(true);

    // Verify both buttons are visible
    expect(await bookPage.isContinueVisible()).toBe(true);

    // Verify FROM BEGINNING href
    const fromBeginningHref = await bookPage.getFromBeginningHref();
    expect(fromBeginningHref).toBe(`/reader/${titleId}/${entryId}/1`);

    // Verify CONTINUE href points to saved page
    const continueHref = await bookPage.getContinueHref();
    expect(continueHref).toBe(`/reader/${titleId}/${entryId}/${savedPage}`);
  });

  test('FROM BEGINNING button navigates to reader at page 1', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryId, entryName } = ids;

    const bookPage = new BookPage(page);
    await bookPage.navigate(titleId);

    await bookPage.clickEntry(entryName);

    // Click FROM BEGINNING
    await bookPage.clickFromBeginning();

    // Verify navigated to reader at page 1
    expect(page.url()).toContain('/reader/');
    expect(page.url()).toContain(`/${titleId}/${entryId}/1`);
  });

  test('CONTINUE button navigates to reader at saved page', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryId, entryName, pages } = ids;

    // Save progress at 50% of pages (ensures visible CONTINUE button)
    // Use Math.max to ensure we're at least on page 3, and Math.min to ensure we're not at the last page
    const savedPage = Math.max(3, Math.min(Math.floor(pages * 0.5), pages - 2));
    await page.request.post(`/api/progress/${titleId}/${entryId}`, {
      data: { page: savedPage },
    });

    const bookPage = new BookPage(page);
    await bookPage.navigate(titleId); // Navigate AFTER saving progress

    await bookPage.clickEntry(entryName);

    // Verify CONTINUE button is visible
    expect(await bookPage.isContinueVisible()).toBe(true);

    // Click CONTINUE
    await bookPage.clickContinue();

    // Verify navigated to reader at saved page
    expect(page.url()).toContain('/reader/');
    expect(page.url()).toContain(`/${titleId}/${entryId}/${savedPage}`);
  });
});
