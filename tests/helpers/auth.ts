import { Page, BrowserContext } from '@playwright/test';

/**
 * Authentication helpers for Playwright tests
 * Provides login functionality and session management
 */

export interface LoginCredentials {
  username: string;
  password: string;
}

/**
 * Default test user credentials
 */
export const TEST_USER: LoginCredentials = {
  username: 'testuser',
  password: 'testpass123',
};

/**
 * Login to Mango application
 * @param page - Playwright page instance
 * @param credentials - Login credentials (defaults to TEST_USER)
 * @returns Promise that resolves when login is complete
 */
export async function login(
  page: Page,
  credentials: LoginCredentials = TEST_USER
): Promise<void> {
  console.log(`Attempting login as ${credentials.username}...`);

  // Navigate to login page
  await page.goto('/login');
  console.log(`- Navigated to /login`);

  // Fill in login form
  await page.fill('input[name="username"]', credentials.username);
  console.log(`- Filled username: ${credentials.username}`);

  await page.fill('input[name="password"]', credentials.password);
  console.log(`- Filled password`);

  // Submit form
  await page.click('button[type="submit"]');
  console.log(`- Clicked submit button`);

  // Wait for redirect to home page (successful login)
  try {
    await page.waitForURL('/', { timeout: 10000 });
    console.log(`✓ Logged in as ${credentials.username}`);
  } catch (error) {
    const currentUrl = page.url();
    console.error(`✗ Login failed! Current URL: ${currentUrl}`);

    // Check for error message on page
    const errorText = await page.textContent('body').catch(() => 'Could not get page text');
    console.error(`Page content: ${errorText?.substring(0, 500)}`);

    throw new Error(`Login failed for ${credentials.username}. Still on: ${currentUrl}`);
  }
}

/**
 * Logout from Mango application
 * @param page - Playwright page instance
 */
export async function logout(page: Page): Promise<void> {
  await page.goto('/logout');
  await page.waitForURL('/login');
  console.log('✓ Logged out');
}

/**
 * Check if user is currently logged in
 * @param page - Playwright page instance
 * @returns True if logged in, false otherwise
 */
export async function isLoggedIn(page: Page): Promise<boolean> {
  await page.goto('/');

  // If we're redirected to login, user is not logged in
  await page.waitForLoadState('domcontentloaded');
  const url = page.url();

  return !url.includes('/login');
}

/**
 * Save authentication state to file
 * Useful for reusing authentication across test workers
 * @param context - Browser context with authentication
 * @param path - File path to save state to
 */
export async function saveAuthState(
  context: BrowserContext,
  path: string
): Promise<void> {
  await context.storageState({ path });
  console.log(`✓ Auth state saved to ${path}`);
}

/**
 * Create a test user via direct database manipulation
 * This should be called during global setup before tests run
 * @param dbPath - Path to SQLite database
 * @param credentials - User credentials to create
 */
export async function createTestUser(
  dbPath: string,
  credentials: LoginCredentials = TEST_USER
): Promise<void> {
  const Database = await import('better-sqlite3');
  const bcrypt = await import('bcryptjs');

  const db = Database.default(dbPath);

  try {
    // Check if user already exists
    const existingUser = db.prepare('SELECT username FROM users WHERE username = ?').get(credentials.username);

    if (existingUser) {
      console.log(`✓ Test user '${credentials.username}' already exists`);
      return;
    }

    // Hash password
    const passwordHash = bcrypt.hashSync(credentials.password, 10);

    // Insert test user (non-admin)
    db.prepare('INSERT INTO users (username, password, token, admin) VALUES (?, ?, NULL, 0)')
      .run(credentials.username, passwordHash);

    console.log(`✓ Test user created: ${credentials.username}`);
  } finally {
    db.close();
  }
}
