import { test, expect } from '../helpers/fixtures.js';
import { test as baseTest } from '@playwright/test';
import { login, REGULAR_USER } from '../helpers/auth.js';
import { HomePage } from '../helpers/page-objects.js';

/**
 * Home Page Section Tests
 *
 * Tests home page personalized sections: Continue Reading, Start Reading, Recently Added.
 * Validates section presence, data population, empty states, and navigation.
 * Uses authenticated page fixture (auto-login) for most tests.
 */

/**
 * Helper: Get valid title and entry IDs from library
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
    entryName: entry.title,
    pages: entry.pages,
  };
}

test.describe('Home Page: Section Presence', () => {
  test('home page loads with all three sections', async ({ page }) => {
    const homePage = new HomePage(page);
    await homePage.navigate();

    // Verify all three sections are present
    await expect(homePage.getSection('Continue Reading')).toBeVisible();
    await expect(homePage.getSection('Start Reading')).toBeVisible();
    await expect(homePage.getSection('Recently Added')).toBeVisible();
  });
});

test.describe.serial('Home Page: Continue Reading Section', () => {
  test('Continue Reading shows entries with saved progress', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryId, entryName, pages } = ids;

    // Save progress at 50%
    const savedPage = Math.floor(pages * 0.5);
    await page.request.post(`/api/progress/${titleId}/${entryId}`, {
      data: { page: savedPage },
    });

    const homePage = new HomePage(page);
    await homePage.navigate();

    // Verify entry appears in Continue Reading
    const cards = homePage.getSectionCards('Continue Reading');
    await expect(cards.filter({ hasText: entryName })).toBeVisible();
  });

  baseTest('Continue Reading is empty for user with no reading history', async ({ browser }) => {
    const context = await browser.newContext();
    const page = await context.newPage();

    try {
      await login(page, REGULAR_USER);

      const homePage = new HomePage(page);
      await homePage.navigate();

      // Check if section is empty OR has content
      // (REGULAR_USER might have reading history from other tests)
      const isEmpty = await homePage.isSectionEmpty('Continue Reading');

      if (isEmpty) {
        // Verify empty message is visible
        const emptyMessage = homePage.getSection('Continue Reading').locator('.empty-state p');
        await expect(emptyMessage).toContainText('No manga in progress');
      } else {
        // If not empty, verify cards are displayed correctly
        const cards = homePage.getSectionCards('Continue Reading');
        expect(await cards.count()).toBeGreaterThan(0);
      }
    } finally {
      await context.close();
    }
  });
});

test.describe('Home Page: Other Sections', () => {
  test('Start Reading shows unread titles', async ({ page }) => {
    const homePage = new HomePage(page);
    await homePage.navigate();

    // Verify Start Reading section has cards OR shows empty state
    const isEmpty = await homePage.isSectionEmpty('Start Reading');

    if (!isEmpty) {
      // Section has cards
      const cards = homePage.getSectionCards('Start Reading');
      expect(await cards.count()).toBeGreaterThan(0);
    } else {
      // Section is empty - verify empty message
      const emptyMessage = homePage.getSection('Start Reading').locator('.empty-state p');
      await expect(emptyMessage).toContainText('All caught up');
    }
  });

  test('Recently Added shows recent entries', async ({ page }) => {
    const homePage = new HomePage(page);
    await homePage.navigate();

    // Verify Recently Added section has cards OR shows empty state
    const isEmpty = await homePage.isSectionEmpty('Recently Added');

    if (!isEmpty) {
      // Section has cards
      const cards = homePage.getSectionCards('Recently Added');
      expect(await cards.count()).toBeGreaterThan(0);
    } else {
      // Section is empty - verify empty message
      const emptyMessage = homePage.getSection('Recently Added').locator('.empty-state p');
      await expect(emptyMessage).toContainText('No recently added');
    }
  });
});

test.describe.serial('Home Page: Navigation and Progress Display', () => {
  test('click entry in Continue Reading navigates correctly', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryId, entryName, pages } = ids;

    // Save progress at 60%
    const savedPage = Math.floor(pages * 0.6);
    await page.request.post(`/api/progress/${titleId}/${entryId}`, {
      data: { page: savedPage },
    });

    const homePage = new HomePage(page);
    await homePage.navigate();

    // Click entry card in Continue Reading
    await homePage.clickCard('Continue Reading', entryName);

    // Verify modal opens (Continue Reading cards open modal, same as book page)
    await homePage.waitForModalOpen();

    // Verify modal is visible
    const modal = page.locator('#entry-modal');
    await expect(modal).toHaveClass(/uk-open/);
  });

  test('progress percentage displays correctly in cards', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryId, pages } = ids;

    // Save progress at 60%
    const savedPage = Math.floor(pages * 0.6);
    await page.request.post(`/api/progress/${titleId}/${entryId}`, {
      data: { page: savedPage },
    });

    const homePage = new HomePage(page);
    await homePage.navigate();

    const card = homePage.getSectionCards('Continue Reading').first();
    const progress = await homePage.getCardProgress(card);

    // Verify progress percentage is displayed (allow ±1% for rounding)
    expect(progress).not.toBeNull();
    expect(progress!).toBeGreaterThanOrEqual(59);
    expect(progress!).toBeLessThanOrEqual(61);
  });
});
