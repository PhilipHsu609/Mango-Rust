import { test, expect } from '../helpers/fixtures.js';
import { NavigationComponent } from '../helpers/page-objects.js';
import { captureEvidence } from '../helpers/test-utils.js';

/**
 * Navigation Integration Tests
 * Tests navigation links, active states, mobile menu, and page routing
 * Covers: Desktop navigation, mobile hamburger menu, active link highlighting
 */

test.describe('Navigation', () => {
  test('should navigate to Library page', async ({ page }) => {
    await page.goto('/');

    const nav = new NavigationComponent(page);

    // Navigate to library
    await nav.navigateToLibrary();

    // Verify URL
    expect(page.url()).toContain('/library');

    // Verify page loaded
    const pageHeading = page.locator('h1, h2').first();
    await expect(pageHeading).toBeVisible();

    console.log('✓ Navigated to Library page successfully');
  });

  test('should navigate to Tags page', async ({ page }) => {
    await page.goto('/');

    const nav = new NavigationComponent(page);

    // Navigate to tags
    await nav.navigateToTags();

    // Verify URL
    expect(page.url()).toContain('/tags');

    // Verify page loaded
    await page.waitForLoadState('domcontentloaded');

    console.log('✓ Navigated to Tags page successfully');
  });

  test('should highlight active navigation link on Library page', async ({ page }) => {
    await page.goto('/library');

    const nav = new NavigationComponent(page);

    // Verify library link is active
    const isLibraryActive = await nav.verifyActiveLink('library');
    expect(isLibraryActive).toBe(true);

    console.log('✓ Library link is highlighted as active');
  });

  test('should highlight active navigation link on Tags page', async ({ page }) => {
    await page.goto('/tags');

    const nav = new NavigationComponent(page);

    // Verify tags link is active
    const isTagsActive = await nav.verifyActiveLink('tags');
    expect(isTagsActive).toBe(true);

    console.log('✓ Tags link is highlighted as active');
  });

  test('should update active link when navigating between pages', async ({ page }) => {
    await page.goto('/library');

    const nav = new NavigationComponent(page);

    // Verify library is active
    const isLibraryActiveInitial = await nav.verifyActiveLink('library');
    expect(isLibraryActiveInitial).toBe(true);

    // Navigate to tags
    await nav.navigateToTags();

    // Library should no longer be active
    const isLibraryActiveAfter = await nav.verifyActiveLink('library');
    expect(isLibraryActiveAfter).toBe(false);

    // Tags should be active
    const isTagsActive = await nav.verifyActiveLink('tags');
    expect(isTagsActive).toBe(true);

    console.log('✓ Active link updates correctly when navigating');
  });

  test('should display all navigation links', async ({ page }) => {
    await page.goto('/library');

    // Check desktop navigation is visible
    const desktopNav = page.locator('ul.uk-navbar-nav').first();
    await expect(desktopNav).toBeVisible();

    // Check main links exist
    const libraryLink = page.locator('ul.uk-navbar-nav a[href="/library"]');
    await expect(libraryLink).toBeVisible();

    const tagsLink = page.locator('ul.uk-navbar-nav a[href="/tags"]');
    await expect(tagsLink).toBeVisible();

    // Check theme toggle exists
    const themeToggle = page.locator('ul.uk-navbar-nav a[onclick*="toggleTheme"]');
    await expect(themeToggle).toBeVisible();

    console.log('✓ All navigation links are displayed');
  });

  test('should open mobile hamburger menu', async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });

    await page.goto('/library');

    const nav = new NavigationComponent(page);

    // Verify in mobile mode
    const isMobile = await nav.isMobileMode();
    expect(isMobile).toBe(true);

    // Open mobile menu
    await nav.openMobileMenu();

    // Verify menu is visible
    const mobileNav = page.locator('#mobile-nav');
    await expect(mobileNav).toBeVisible();

    // Capture screenshot
    await captureEvidence(page, 'mobile-menu-open');

    console.log('✓ Mobile menu opens successfully');
  });

  test('should navigate using mobile menu', async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });

    await page.goto('/');

    const nav = new NavigationComponent(page);

    // Navigate to library using mobile menu
    await nav.navigateToLibrary(true);

    // Verify URL
    expect(page.url()).toContain('/library');

    // Navigate to tags using mobile menu
    await nav.navigateToTags(true);

    // Verify URL
    expect(page.url()).toContain('/tags');

    console.log('✓ Mobile menu navigation works correctly');
  });

  test('should close mobile menu with escape key', async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });

    await page.goto('/library');

    const nav = new NavigationComponent(page);

    // Open mobile menu
    await nav.openMobileMenu();

    const mobileNav = page.locator('#mobile-nav');
    await expect(mobileNav).toBeVisible();

    // Close with escape
    await nav.closeMobileMenu();

    // Verify menu is hidden
    await expect(mobileNav).toBeHidden();

    console.log('✓ Mobile menu closes with escape key');
  });

  test('should display navigation on all pages', async ({ page }) => {
    const pagesToTest = [
      { name: 'home', url: '/' },
      { name: 'library', url: '/library' },
      { name: 'tags', url: '/tags' },
    ];

    for (const pageToTest of pagesToTest) {
      await page.goto(pageToTest.url);

      // Check navigation is visible
      const navbar = page.locator('.uk-navbar-container');
      await expect(navbar).toBeVisible();

      console.log(`✓ Navigation visible on ${pageToTest.name} page`);
    }
  });

  test('should maintain navigation state during page transitions', async ({ page }) => {
    await page.goto('/library');

    const nav = new NavigationComponent(page);

    // Click on a title to go to book page
    const firstTitle = page.locator('.title-card').first();
    const titleExists = (await firstTitle.count()) > 0;

    if (titleExists) {
      await firstTitle.click();
      await page.waitForLoadState('domcontentloaded');

      // Navigation should still be visible
      const navbar = page.locator('.uk-navbar-container');
      await expect(navbar).toBeVisible();

      // Can still navigate back to library
      await nav.navigateToLibrary();
      expect(page.url()).toContain('/library');

      console.log('✓ Navigation state maintained during transitions');
    } else {
      console.log('⊘ Skipped: No titles available to test navigation state');
    }
  });

  test('should work with both desktop and mobile viewports', async ({ page }) => {
    // Test desktop viewport
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.goto('/library');

    let nav = new NavigationComponent(page);
    let isMobile = await nav.isMobileMode();
    expect(isMobile).toBe(false);

    // Desktop navigation should be visible
    const desktopNav = page.locator('ul.uk-navbar-nav').first();
    await expect(desktopNav).toBeVisible();

    await captureEvidence(page, 'navigation-desktop');

    // Test mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto('/library');

    nav = new NavigationComponent(page);
    isMobile = await nav.isMobileMode();
    expect(isMobile).toBe(true);

    // Hamburger button should be visible
    const hamburger = page.locator('[uk-toggle="target: #mobile-nav"]');
    await expect(hamburger).toBeVisible();

    await captureEvidence(page, 'navigation-mobile');

    console.log('✓ Navigation adapts to different viewports');
  });
});
