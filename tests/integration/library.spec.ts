import { test, expect } from '../helpers/fixtures.js';
import { LibraryPage } from '../helpers/page-objects.js';
import { captureEvidence } from '../helpers/test-utils.js';

/**
 * Library Search and Sort Integration Tests
 * Tests library page functionality including search, sort, and title display
 * Covers: Title loading, search filtering, sort options, title card rendering
 */

test.describe('Library Search and Sort', () => {
  test('should load library page with titles', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    // Verify Alpine.js initialized properly by checking for x-data attribute AND no JS errors
    const alpineStatus = await page.evaluate(() => {
      const appDiv = document.querySelector('[x-data]');
      const hasAttribute = appDiv !== null && appDiv.hasAttribute('x-data');

      // CRITICAL: Check that Alpine actually bound to the element
      // If libraryData() has syntax error, Alpine won't initialize
      const hasAlpineData = appDiv && '__x' in appDiv; // Alpine adds __x property

      return { hasAttribute, hasAlpineData };
    });
    expect(alpineStatus.hasAttribute).toBe(true);
    expect(alpineStatus.hasAlpineData).toBe(true); // Alpine must have initialized

    // Get title count
    const titleCount = await library.getTitleCount();

    // Should have at least some titles (if library is populated)
    // For empty library, count will be 0 which is also valid
    expect(titleCount).toBeGreaterThanOrEqual(0);

    // Capture screenshot
    await captureEvidence(page, 'library-loaded');

    console.log(`✓ Library loaded with ${titleCount} titles (Alpine initialized: ${alpineStatus.hasAlpineData})`);
  });

  test('should display title cards with correct structure', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    const titleCount = await library.getTitleCount();

    if (titleCount > 0) {
      // Check first title card structure
      const firstCard = page.locator('.title-card').first();
      await expect(firstCard).toBeVisible();

      // Title cards should have thumbnail or placeholder
      const thumbnail = firstCard.locator('img, .uk-card-media-top');
      const hasThumbnail = (await thumbnail.count()) > 0;
      expect(hasThumbnail).toBe(true);

      // Should have title text
      const titleText = firstCard.locator('.uk-card-title, h3');
      await expect(titleText).toBeVisible();

      console.log('✓ Title cards have correct structure');
    } else {
      console.log('⊘ Skipped: No titles to verify structure');
    }
  });

  test('should search and filter titles', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    const initialCount = await library.getTitleCount();

    if (initialCount > 1) { // Need at least 2 titles to verify filtering works
      // Get text from first title
      const firstCard = page.locator('.title-card').first();
      const titleElement = firstCard.locator('.uk-card-title, h3').first();
      const titleText = await titleElement.textContent();

      if (titleText && titleText.length > 2) {
        // Search for part of the title
        const searchTerm = titleText.substring(0, 3);

        await library.search(searchTerm);

        // Capture screenshot
        await captureEvidence(page, 'library-search-filtered');

        // CRITICAL: Search MUST actually filter (not just return all cards)
        const filteredCount = await library.getTitleCount();
        expect(filteredCount).toBeGreaterThan(0);

        // If bug makes regex.test() always return true, filteredCount === initialCount
        // So we verify it's actually less (search filtered something out)
        // OR if all titles match the search, we need a different search term
        if (filteredCount === initialCount && initialCount > 1) {
          // Try searching for something that won't match all titles
          await library.search('xyzzynotfound123');
          const noMatchCount = await library.getTitleCount();
          expect(noMatchCount).toBe(0); // Proves filtering actually works

          // Search back to verify original search worked
          await library.search(searchTerm);
          const retryCount = await library.getTitleCount();
          expect(retryCount).toBeGreaterThan(0);
        }

        console.log(`✓ Search filtered from ${initialCount} to ${filteredCount} titles`);
      } else {
        console.log('⊘ Skipped: Title text too short for search test');
      }
    } else {
      console.log('⊘ Skipped: Need at least 2 titles to verify search filtering');
    }
  });

  test('should clear search and show all titles', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    const initialCount = await library.getTitleCount();

    if (initialCount > 0) {
      // Search for something
      await library.search('test');

      const searchedCount = await library.getTitleCount();

      // Clear search
      await library.search('');

      // Should show all titles again
      const clearedCount = await library.getTitleCount();
      expect(clearedCount).toBe(initialCount);

      // Verify that search actually filtered (unless all titles match 'test')
      // The key issue: we need to verify empty search is explicitly handled
      // Check that the search input is actually empty
      const searchInputValue = await page.locator('#search-input').inputValue();
      expect(searchInputValue).toBe('');

      console.log('✓ Search cleared and all titles displayed');
    } else {
      console.log('⊘ Skipped: No titles to test search clear');
    }
  });

  test('should search case-insensitively', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    const titleCount = await library.getTitleCount();

    if (titleCount > 0) {
      // Get first title text
      const firstCard = page.locator('.title-card').first();
      const titleElement = firstCard.locator('.uk-card-title, h3').first();
      const titleText = await titleElement.textContent();

      if (titleText && titleText.length > 2) {
        const searchTerm = titleText.substring(0, 3);

        // Search lowercase
        await library.search(searchTerm.toLowerCase());
        const lowercaseCount = await library.getTitleCount();

        // Search uppercase
        await library.search(searchTerm.toUpperCase());
        const uppercaseCount = await library.getTitleCount();

        // Should have same results
        expect(uppercaseCount).toBe(lowercaseCount);

        // CRITICAL: Both should be greater than 0 (otherwise test is meaningless)
        expect(lowercaseCount).toBeGreaterThan(0);
        expect(uppercaseCount).toBeGreaterThan(0);

        console.log('✓ Search is case-insensitive');
      } else {
        console.log('⊘ Skipped: Title text too short for case test');
      }
    } else {
      console.log('⊘ Skipped: No titles to test case-insensitive search');
    }
  });

  test('should sort by name', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    const titleCount = await library.getTitleCount();

    if (titleCount >= 2) {
      // Select name sort
      await library.selectSort('name');

      // Get titles
      const titles = await page.locator('.title-card .title-name').allTextContents();

      // Verify sorted (case-insensitive)
      const sortedTitles = [...titles].sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));

      expect(titles).toEqual(sortedTitles);

      console.log('✓ Titles sorted by name');
    } else {
      console.log('⊘ Skipped: Need at least 2 titles to verify sorting');
    }
  });

  test('should sort by date', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    const titleCount = await library.getTitleCount();

    if (titleCount >= 2) {
      // Get initial order
      const initialOrder = await library.getTitleNames();

      // Select date sort
      await library.selectSort('date');

      // Capture screenshot
      await captureEvidence(page, 'library-sorted-by-date');

      // Verify sort completed without error
      const countAfterSort = await library.getTitleCount();
      expect(countAfterSort).toBe(titleCount);

      // Verify order changed (date sort should reorder titles)
      const newOrder = await library.getTitleNames();
      expect(newOrder).not.toEqual(initialOrder);

      console.log('✓ Titles sorted by date');
    } else {
      console.log('⊘ Skipped: Need at least 2 titles to verify sorting');
    }
  });

  test('should sort by progress', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    const titleCount = await library.getTitleCount();

    if (titleCount >= 2) {
      // Get initial order
      const initialOrder = await library.getTitleNames();

      // Select progress sort
      await library.selectSort('progress');

      // Capture screenshot
      await captureEvidence(page, 'library-sorted-by-progress');

      // Verify sort completed without error
      const countAfterSort = await library.getTitleCount();
      expect(countAfterSort).toBe(titleCount);

      // Verify order changed (progress sort should reorder titles)
      const newOrder = await library.getTitleNames();
      expect(newOrder).not.toEqual(initialOrder);

      console.log('✓ Titles sorted by progress');
    } else {
      console.log('⊘ Skipped: Need at least 2 titles to verify sorting');
    }
  });

  test('should maintain search filter when changing sort', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    const initialCount = await library.getTitleCount();

    if (initialCount >= 2) {
      // Get first title for search
      const firstCard = page.locator('.title-card').first();
      const titleElement = firstCard.locator('.title-name').first();
      const titleText = await titleElement.textContent();

      if (titleText && titleText.length > 2) {
        const searchTerm = titleText.substring(0, 3);

        // Search
        await library.search(searchTerm);
        const filteredCount = await library.getTitleCount();

        // Change sort (this will reload the page and clear the search)
        await library.selectSort('name');

        // Search should be cleared after sort (page reload)
        const countAfterSort = await library.getTitleCount();
        expect(countAfterSort).toBe(initialCount); // Back to full count

        // Verify URL params were properly handled during sort
        // (Bug was: Break URL param handling in sort)
        const currentUrl = page.url();
        expect(currentUrl).toContain('sort=name');

        // Search again after sorting to verify search still works
        await library.search(searchTerm);

        const refilteredCount = await library.getTitleCount();
        expect(refilteredCount).toBe(filteredCount);

        console.log('✓ Search works after changing sort');
      } else {
        console.log('⊘ Skipped: Title text too short');
      }
    } else {
      console.log('⊘ Skipped: Need at least 2 titles to test');
    }
  });

  test('should click on title card and navigate to book page', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    const titleCount = await library.getTitleCount();

    if (titleCount > 0) {
      // CRITICAL: Verify title card HAS href attribute and it's a valid link
      const firstCard = page.locator('.title-card').first();
      const href = await firstCard.getAttribute('href');
      expect(href).toBeTruthy(); // href must exist
      expect(href).toMatch(/^\/book\//); // must be book link

      // Also verify the <a> tag actually has the href in the HTML
      const hasHrefInHtml = await page.evaluate(() => {
        const card = document.querySelector('.title-card') as HTMLAnchorElement;
        return card && card.hasAttribute('href') && card.href.includes('/book/');
      });
      expect(hasHrefInHtml).toBe(true);

      // Get first title name
      const titleElement = firstCard.locator('.uk-card-title, h3').first();
      const titleText = await titleElement.textContent();

      if (titleText) {
        // Click on title
        await library.clickTitle(titleText);

        // Should navigate to book page
        await page.waitForLoadState('domcontentloaded');

        // Verify we're on a different page (not library)
        const currentUrl = page.url();
        expect(currentUrl).not.toContain('/library');
        expect(currentUrl).toContain('/book/'); // Must be on book page

        console.log('✓ Clicked title card and navigated to book page');
      }
    } else {
      console.log('⊘ Skipped: No titles to click');
    }
  });

  test('should show empty state with no results', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    const titleCount = await library.getTitleCount();

    if (titleCount > 0) {
      // Search for something that won't match
      await library.search('xyzzynotfound123456');

      // Should have no visible cards
      const noResultsCount = await library.getTitleCount();
      expect(noResultsCount).toBe(0);

      // Capture screenshot
      await captureEvidence(page, 'library-no-results');

      console.log('✓ No results shown for non-matching search');
    } else {
      console.log('⊘ Skipped: Library already empty');
    }
  });

  test('should have working navigation while on library page', async ({ page }) => {
    const library = new LibraryPage(page);

    await library.navigate();

    // Get navigation component
    const nav = library.getNavigation();

    // Navigate to tags
    await nav.navigateToTags();

    // Verify URL changed
    expect(page.url()).toContain('/tags');

    // Navigate back to library
    await nav.navigateToLibrary();

    // Verify back on library
    expect(page.url()).toContain('/library');

    console.log('✓ Navigation works while on library page');
  });
});
