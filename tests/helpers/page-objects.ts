import { Page, Locator } from '@playwright/test';
import { toggleTheme as toggleThemeHelper } from './theme-utils.js';

/**
 * Page Object Models for Mango-Rust application
 * Encapsulates page interactions and element selectors
 */

/**
 * Navigation Component Page Object
 * Handles both desktop and mobile navigation
 */
export class NavigationComponent {
  private page: Page;

  // Desktop navigation selectors
  private desktopNav: Locator;
  private libraryLink: Locator;
  private tagsLink: Locator;
  private adminLink: Locator;
  private themeToggle: Locator;

  // Mobile navigation selectors
  private hamburgerButton: Locator;
  private mobileNav: Locator;
  private mobileLibraryLink: Locator;
  private mobileTagsLink: Locator;
  private mobileAdminLink: Locator;

  constructor(page: Page) {
    this.page = page;

    // Desktop navigation
    this.desktopNav = page.locator('ul.uk-navbar-nav').first();
    this.libraryLink = this.desktopNav.locator('a[href="/library"]');
    this.tagsLink = this.desktopNav.locator('a[href="/tags"]');
    this.adminLink = this.desktopNav.locator('a[href="/admin"]');
    this.themeToggle = page.locator('a[onclick*="toggleTheme"]');

    // Mobile navigation
    this.hamburgerButton = page.locator('[uk-toggle="target: #mobile-nav"]');
    this.mobileNav = page.locator('#mobile-nav');
    this.mobileLibraryLink = this.mobileNav.locator('a[href="/library"]');
    this.mobileTagsLink = this.mobileNav.locator('a[href="/tags"]');
    this.mobileAdminLink = this.mobileNav.locator('a[href="/admin"]');
  }

  /**
   * Navigate to Library page
   * @param mobile - Use mobile navigation (default: false)
   */
  async navigateToLibrary(mobile = false): Promise<void> {
    if (mobile) {
      await this.openMobileMenu();
      // Force click using JavaScript to avoid Playwright actionability issues
      // The menu is open and the link is there, just click it
      await this.page.evaluate(() => {
        const link = document.querySelector('#mobile-nav a[href="/library"]') as HTMLElement;
        link.click();
      });
    } else {
      await this.libraryLink.click();
    }
    await this.page.waitForURL('**/library');
  }

  /**
   * Navigate to Tags page
   * @param mobile - Use mobile navigation (default: false)
   */
  async navigateToTags(mobile = false): Promise<void> {
    if (mobile) {
      await this.openMobileMenu();
      // Force click using JavaScript to avoid Playwright actionability issues
      await this.page.evaluate(() => {
        const link = document.querySelector('#mobile-nav a[href="/tags"]') as HTMLElement;
        link.click();
      });
    } else {
      await this.tagsLink.click();
    }
    await this.page.waitForURL('**/tags');
  }

  /**
   * Navigate to Admin page
   * @param mobile - Use mobile navigation (default: false)
   */
  async navigateToAdmin(mobile = false): Promise<void> {
    if (mobile) {
      await this.openMobileMenu();
      // Force click using JavaScript to avoid Playwright actionability issues
      await this.page.evaluate(() => {
        const link = document.querySelector('#mobile-nav a[href="/admin"]') as HTMLElement;
        link.click();
      });
    } else {
      await this.adminLink.click();
    }
    await this.page.waitForURL('**/admin');
  }

  /**
   * Toggle theme using the theme toggle button
   */
  async toggleTheme(): Promise<void> {
    await toggleThemeHelper(this.page);
  }

  /**
   * Verify active navigation link
   * @param page - Expected active page ('library' | 'tags' | 'admin')
   */
  async verifyActiveLink(page: 'library' | 'tags' | 'admin'): Promise<boolean> {
    const linkMap = {
      library: this.libraryLink,
      tags: this.tagsLink,
      admin: this.adminLink,
    };

    const link = linkMap[page];
    const parentLi = link.locator('xpath=ancestor::li[1]');

    // Check if parent <li> has uk-active class
    const hasActiveClass = await parentLi.evaluate((el: HTMLElement) =>
      el.classList.contains('uk-active')
    );

    return hasActiveClass;
  }

  /**
   * Open mobile hamburger menu
   */
  async openMobileMenu(): Promise<void> {
    // Check if menu is already open
    const isOpen = await this.mobileNav.evaluate((el: HTMLElement) => {
      return el.classList.contains('uk-open');
    });

    if (!isOpen) {
      await this.hamburgerButton.click();

      // Wait for UIkit to add uk-open class (indicates animation complete)
      await this.page.waitForFunction(
        () => {
          const nav = document.querySelector('#mobile-nav');
          return nav && nav.classList.contains('uk-open');
        },
        { timeout: 3000 }
      );
    }
  }

  /**
   * Close mobile hamburger menu
   */
  async closeMobileMenu(): Promise<void> {
    // Check if menu is open
    const isOpen = await this.mobileNav.isVisible();

    if (isOpen) {
      // Click outside or use close button
      await this.page.keyboard.press('Escape');
      await this.mobileNav.waitFor({ state: 'hidden', timeout: 2000 });
    }
  }

  /**
   * Check if navigation is in mobile mode
   */
  async isMobileMode(): Promise<boolean> {
    const hamburgerVisible = await this.hamburgerButton.isVisible();
    return hamburgerVisible;
  }

  /**
   * Check if admin link is visible (user is admin)
   */
  async isAdminLinkVisible(): Promise<boolean> {
    const visible = await this.adminLink.isVisible().catch(() => false);
    return visible;
  }
}

/**
 * Library Page Object
 * Handles library page interactions
 */
export class LibraryPage {
  private page: Page;
  private nav: NavigationComponent;

  // Selectors
  private searchInput: Locator;
  private sortSelect: Locator;
  private titleCards: Locator;
  private titlesGrid: Locator;

  constructor(page: Page) {
    this.page = page;
    this.nav = new NavigationComponent(page);

    // Library page selectors
    this.searchInput = page.locator('input[type="search"]');
    this.sortSelect = page.locator('select.uk-select');
    this.titleCards = page.locator('.title-card');
    this.titlesGrid = page.locator('.titles-grid');
  }

  /**
   * Navigate to library page
   */
  async navigate(): Promise<void> {
    await this.page.goto('/library');
    await this.page.waitForLoadState('domcontentloaded');
  }

  /**
   * Search for titles
   * @param query - Search query string
   */
  async search(query: string): Promise<void> {
    await this.searchInput.fill(query);

    // Wait for Alpine.js x-model to update and filter to apply
    await this.page.waitForFunction(
      (expected: string) => {
        const input = document.querySelector('input[type="search"]') as HTMLInputElement;
        if (!input || input.value !== expected) return false;

        // Wait one more frame for Alpine.js reactivity to complete
        return new Promise(resolve => {
          requestAnimationFrame(() => {
            requestAnimationFrame(() => resolve(true));
          });
        });
      },
      query,
      { timeout: 2000 }
    );
  }

  /**
   * Select sort option
   * @param option - Sort option ('name' | 'date' | 'progress')
   */
  async selectSort(option: 'name' | 'date' | 'progress'): Promise<void> {
    // Map friendly names to actual option values (field:ascending)
    const optionMap: Record<string, string> = {
      'name': 'title:1',      // Sort by name ascending
      'date': 'modified:0',    // Sort by date descending (most recent first)
      'progress': 'progress:0' // Sort by progress descending (highest first)
    };

    // Use JavaScript to change sort - avoid Playwright actionability issues
    await this.page.evaluate((value) => {
      const select = document.querySelector('select.uk-select') as HTMLSelectElement;
      select.value = value;
      select.dispatchEvent(new Event('change', { bubbles: true }));
    }, optionMap[option]);

    // Wait for page to reload (sort triggers form submission)
    await this.page.waitForLoadState('domcontentloaded');
    // Wait for titles grid to be ready
    await this.page.waitForSelector('.titles-grid');
  }

  /**
   * Get all title cards
   */
  getTitleCards(): Locator {
    return this.titleCards;
  }

  /**
   * Get count of visible title cards
   */
  async getTitleCount(): Promise<number> {
    // Count only visible cards (Alpine.js x-show sets display:none, so we filter those out)
    const allCards = await this.titleCards.all();
    let visibleCount = 0;
    for (const card of allCards) {
      const display = await card.evaluate((el) => window.getComputedStyle(el).display);
      if (display !== 'none') {
        visibleCount++;
      }
    }
    return visibleCount;
  }

  /**
   * Get visible title names in order
   */
  async getTitleNames(): Promise<string[]> {
    const allCards = await this.titleCards.all();
    const names: string[] = [];

    for (const card of allCards) {
      const display = await card.evaluate((el) => window.getComputedStyle(el).display);
      if (display !== 'none') {
        const nameElement = card.locator('.title-name');
        const name = await nameElement.textContent();
        if (name) names.push(name.trim());
      }
    }

    return names;
  }

  /**
   * Verify title exists by name
   * @param name - Title name to search for
   */
  async verifyTitleExists(name: string): Promise<boolean> {
    const card = this.titleCards.filter({ hasText: name });
    const count = await card.count();
    return count > 0;
  }

  /**
   * Click on a title card by name
   * @param name - Title name
   */
  async clickTitle(name: string): Promise<void> {
    const card = this.titleCards.filter({ hasText: name }).first();
    await card.click();
    await this.page.waitForLoadState('domcontentloaded');
  }

  /**
   * Get navigation component
   */
  getNavigation(): NavigationComponent {
    return this.nav;
  }
}

/**
 * Reader Page Object
 * Handles reader page interactions
 */
export class ReaderPage {
  private page: Page;

  // Reader elements
  private settingsButton: Locator;
  private settingsModal: Locator;
  private pagedImage: Locator;
  private continuousContainer: Locator;
  private clickZoneLeft: Locator;
  private clickZoneRight: Locator;

  // Settings modal elements
  private modeSelect: Locator;
  private fitSelect: Locator;
  private pageSelect: Locator;

  constructor(page: Page) {
    this.page = page;

    // Reader elements
    this.settingsButton = page.locator('#settings-btn');
    this.settingsModal = page.locator('#settings-modal');
    this.pagedImage = page.locator('#paged-image');
    this.continuousContainer = page.locator('#continuous-container');
    this.clickZoneLeft = page.locator('.click-zone-left');
    this.clickZoneRight = page.locator('.click-zone-right');

    // Settings modal
    this.modeSelect = page.locator('#mode-select');
    this.fitSelect = page.locator('#fit-select');
    this.pageSelect = page.locator('#page-select');
  }

  /**
   * Verify reader page loaded without errors
   */
  async verifyReaderLoaded(): Promise<void> {
    await this.settingsButton.waitFor({ state: 'visible', timeout: 10000 });

    // Wait for either paged or continuous mode to be visible
    await this.page.waitForFunction(
      () => {
        const paged = document.querySelector('#paged-image');
        const continuous = document.querySelector('#continuous-container');
        return (paged && window.getComputedStyle(paged).display !== 'none') ||
               (continuous && window.getComputedStyle(continuous).display !== 'none');
      },
      { timeout: 5000 }
    );
  }

  /**
   * Navigate to next page (paged mode)
   * @param method - Navigation method ('click' | 'keyboard')
   */
  async navigateNextPage(method: 'click' | 'keyboard' = 'click'): Promise<void> {
    // Get current page before navigation
    const currentPage = await this.getCurrentPage();

    if (method === 'click') {
      await this.clickZoneRight.click();
    } else {
      await this.page.keyboard.press('ArrowRight');
    }

    // Wait for page number to change
    await this.page.waitForFunction(
      (expected: number) => {
        const select = document.querySelector('#page-select') as HTMLSelectElement;
        return select && parseInt(select.value, 10) > expected;
      },
      currentPage,
      { timeout: 2000 }
    );
  }

  /**
   * Navigate to previous page (paged mode)
   * @param method - Navigation method ('click' | 'keyboard')
   */
  async navigatePreviousPage(method: 'click' | 'keyboard' = 'click'): Promise<void> {
    // Get current page before navigation
    const currentPage = await this.getCurrentPage();

    if (method === 'click') {
      await this.clickZoneLeft.click();
    } else {
      await this.page.keyboard.press('ArrowLeft');
    }

    // Wait for page number to change
    await this.page.waitForFunction(
      (expected: number) => {
        const select = document.querySelector('#page-select') as HTMLSelectElement;
        return select && parseInt(select.value, 10) < expected;
      },
      currentPage,
      { timeout: 2000 }
    );
  }

  /**
   * Open settings modal
   */
  async openSettings(): Promise<void> {
    // Check if modal is already open
    const isOpen = await this.settingsModal.isVisible();
    if (!isOpen) {
      // Use JavaScript click to avoid Playwright actionability issues
      await this.page.evaluate(() => {
        const btn = document.querySelector('#settings-btn') as HTMLElement;
        btn.click();
      });
      await this.settingsModal.waitFor({ state: 'visible', timeout: 3000 });
    }
  }

  /**
   * Change reading mode
   * @param mode - Reading mode ('paged' | 'continuous')
   */
  async changeMode(mode: 'paged' | 'continuous'): Promise<void> {
    await this.openSettings();

    // Use JavaScript to change mode - avoid Playwright actionability issues
    await this.page.evaluate((m) => {
      const select = document.querySelector('#mode-select') as HTMLSelectElement;
      select.value = m;
      select.dispatchEvent(new Event('change', { bubbles: true }));
    }, mode);

    // Wait for mode to switch
    if (mode === 'paged') {
      await this.pagedImage.waitFor({ state: 'visible', timeout: 2000 });
    } else {
      await this.continuousContainer.waitFor({ state: 'visible', timeout: 2000 });
    }
  }

  /**
   * Change page fit (paged mode only)
   * @param fit - Fit option ('height' | 'width' | 'real')
   */
  async changeFit(fit: 'height' | 'width' | 'real'): Promise<void> {
    // Use JavaScript to change fit - avoid Playwright actionability issues
    await this.page.evaluate((f) => {
      const select = document.querySelector('#fit-select') as HTMLSelectElement;
      select.value = f;
      select.dispatchEvent(new Event('change', { bubbles: true }));
    }, fit);

    // Wait for image to adjust to new fit
    await this.pagedImage.waitFor({ state: 'visible', timeout: 2000 });
  }

  /**
   * Jump to specific page
   * @param pageNumber - Page number to jump to
   */
  async jumpToPage(pageNumber: number): Promise<void> {
    // Use JavaScript to change page - avoid Playwright actionability issues
    await this.page.evaluate((page) => {
      const select = document.querySelector('#page-select') as HTMLSelectElement;
      select.value = page.toString();
      select.dispatchEvent(new Event('change', { bubbles: true }));
    }, pageNumber);

    // Wait for page to update
    await this.page.waitForFunction(
      (expected: number) => {
        const select = document.querySelector('#page-select') as HTMLSelectElement;
        return select && parseInt(select.value, 10) === expected;
      },
      pageNumber,
      { timeout: 2000 }
    );
  }

  /**
   * Get current page number
   */
  async getCurrentPage(): Promise<number> {
    const value = await this.pageSelect.inputValue();
    return parseInt(value, 10);
  }

  /**
   * Check if in paged mode
   */
  async isPagedMode(): Promise<boolean> {
    return await this.pagedImage.isVisible();
  }

  /**
   * Check if in continuous mode
   */
  async isContinuousMode(): Promise<boolean> {
    return await this.continuousContainer.isVisible();
  }
}

/**
 * Book Page Object (Task 3.1)
 * Handles book detail page and entry modal interactions
 */
export class BookPage {
  private page: Page;
  private nav: NavigationComponent;

  // Entry card selectors
  private entryCards: Locator;

  // Modal selectors
  private modal: Locator;
  private modalTitle: Locator;
  private fromBeginningLink: Locator;
  private continueLink: Locator;

  constructor(page: Page) {
    this.page = page;
    this.nav = new NavigationComponent(page);

    // Entry cards
    this.entryCards = page.locator('.entry-card');

    // Modal elements
    this.modal = page.locator('#entry-modal');
    this.modalTitle = this.modal.locator('h3');
    this.fromBeginningLink = this.modal.locator('a.uk-button-default');
    this.continueLink = this.modal.locator('a.uk-button-primary');
  }

  /**
   * Navigate to book page
   * @param titleId - Title ID to navigate to
   */
  async navigate(titleId: string): Promise<void> {
    await this.page.goto(`/book/${titleId}`);
    await this.page.waitForLoadState('domcontentloaded');
  }

  /**
   * Get all entry cards
   */
  getEntryCards(): Locator {
    return this.entryCards;
  }

  /**
   * Get count of entry cards
   */
  async getEntryCount(): Promise<number> {
    return await this.entryCards.count();
  }

  /**
   * Click on an entry card by name
   * @param entryName - Entry name to click
   */
  async clickEntry(entryName: string): Promise<void> {
    const card = this.entryCards.filter({ hasText: entryName }).first();
    await card.click();
    await this.waitForModalOpen();
  }

  /**
   * Wait for entry modal to open
   * UIkit adds uk-open class when animation completes
   */
  async waitForModalOpen(): Promise<void> {
    await this.page.waitForFunction(
      () => {
        const modal = document.querySelector('#entry-modal');
        return modal && modal.classList.contains('uk-open');
      },
      { timeout: 3000 }
    );
  }

  /**
   * Check if entry modal is open
   */
  async isModalOpen(): Promise<boolean> {
    return await this.modal.evaluate((el: HTMLElement) =>
      el.classList.contains('uk-open')
    );
  }

  /**
   * Get FROM BEGINNING link href
   */
  async getFromBeginningHref(): Promise<string | null> {
    return await this.fromBeginningLink.getAttribute('href');
  }

  /**
   * Get CONTINUE link href (returns null if button not visible)
   */
  async getContinueHref(): Promise<string | null> {
    if (!(await this.isContinueVisible())) return null;
    return await this.continueLink.getAttribute('href');
  }

  /**
   * Check if CONTINUE button is visible
   * Alpine x-show="modalData.progress > 0 && modalData.progress < 100" sets display:none
   */
  async isContinueVisible(): Promise<boolean> {
    const display = await this.continueLink.evaluate(
      (el) => window.getComputedStyle(el).display
    );
    return display !== 'none';
  }

  /**
   * Click FROM BEGINNING button and wait for reader navigation
   */
  async clickFromBeginning(): Promise<void> {
    await this.fromBeginningLink.click();
    await this.page.waitForURL('**/reader/**');
  }

  /**
   * Click CONTINUE button and wait for reader navigation
   */
  async clickContinue(): Promise<void> {
    await this.continueLink.click();
    await this.page.waitForURL('**/reader/**');
  }

  /**
   * Get navigation component
   */
  getNavigation(): NavigationComponent {
    return this.nav;
  }
}

/**
 * Home Page Object (Task 3.2)
 * Handles home page sections: Continue Reading, Start Reading, Recently Added
 */
export class HomePage {
  private page: Page;
  private nav: NavigationComponent;

  // Section selectors
  private sections: Locator;

  constructor(page: Page) {
    this.page = page;
    this.nav = new NavigationComponent(page);

    // All sections on home page
    this.sections = page.locator('.section');
  }

  /**
   * Navigate to home page
   */
  async navigate(): Promise<void> {
    await this.page.goto('/');
    await this.page.waitForLoadState('domcontentloaded');
    // Wait for Alpine.js to finish loading data
    await this.waitForSectionsLoaded();
  }

  /**
   * Wait for Alpine.js to finish loading all sections
   */
  private async waitForSectionsLoaded(): Promise<void> {
    // Wait for Alpine.js to render sections (either cards-grid or empty-state visible)
    await this.page.waitForFunction(
      () => {
        // Check if at least one section has rendered content (cards-grid or empty-state visible)
        const sections = document.querySelectorAll('.section');
        for (const section of sections) {
          const cardsGrid = section.querySelector('.cards-grid');
          const emptyState = section.querySelector('.empty-state');

          // Check if either cards or empty state is visible
          if (cardsGrid && window.getComputedStyle(cardsGrid).display !== 'none') {
            return true;
          }
          if (emptyState && window.getComputedStyle(emptyState).display !== 'none') {
            return true;
          }
        }
        return false;
      },
      { timeout: 5000 }
    );
  }

  /**
   * Get a section by its heading text
   * @param name - Section name ('Continue Reading' | 'Start Reading' | 'Recently Added')
   */
  getSection(name: 'Continue Reading' | 'Start Reading' | 'Recently Added'): Locator {
    return this.sections.filter({ has: this.page.locator(`h2:text("${name}")`) });
  }

  /**
   * Get cards in a section
   * Returns all .card elements within the section's cards-grid
   */
  getSectionCards(name: 'Continue Reading' | 'Start Reading' | 'Recently Added'): Locator {
    return this.getSection(name).locator('.cards-grid .card');
  }

  /**
   * Check if a section shows empty state
   */
  async isSectionEmpty(name: 'Continue Reading' | 'Start Reading' | 'Recently Added'): Promise<boolean> {
    const emptyState = this.getSection(name).locator('.empty-state');
    const display = await emptyState.evaluate((el) => window.getComputedStyle(el).display);
    return display !== 'none';
  }

  /**
   * Click on a card by entry name within a section
   */
  async clickCard(
    sectionName: 'Continue Reading' | 'Start Reading' | 'Recently Added',
    entryName: string
  ): Promise<void> {
    const card = this.getSectionCards(sectionName).filter({ hasText: entryName }).first();
    await card.click();
  }

  /**
   * Get progress percentage from a card's badge
   * Returns null if no badge visible
   */
  async getCardProgress(card: Locator): Promise<number | null> {
    const badge = card.locator('.progress-badge');
    const display = await badge.evaluate((el) => window.getComputedStyle(el).display);
    if (display === 'none') return null;

    const text = await badge.textContent();
    if (!text) return null;
    return parseInt(text.replace('%', ''), 10);
  }

  /**
   * Wait for entry modal to open (same as BookPage pattern)
   */
  async waitForModalOpen(): Promise<void> {
    await this.page.waitForFunction(
      () => {
        const modal = document.querySelector('#entry-modal');
        return modal && modal.classList.contains('uk-open');
      },
      { timeout: 3000 }
    );
  }

  /**
   * Get navigation component
   */
  getNavigation(): NavigationComponent {
    return this.nav;
  }
}
