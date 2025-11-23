import { Page } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

/**
 * Test utilities for common operations in Playwright tests
 * Provides helpers for screenshots, page waits, browser state management, and error collection
 */

/**
 * Capture screenshot evidence for test debugging
 * @param page - Playwright page instance
 * @param name - Descriptive name for the screenshot
 * @param fullPage - Whether to capture full scrollable page (default: false)
 */
export async function captureEvidence(page: Page, name: string, fullPage = false): Promise<string> {
  const screenshotsDir = path.join(process.cwd(), 'screenshots');

  // Create screenshots directory if it doesn't exist
  if (!fs.existsSync(screenshotsDir)) {
    fs.mkdirSync(screenshotsDir, { recursive: true });
  }

  // Generate timestamp-based filename
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  const filename = `${name}_${timestamp}.png`;
  const filepath = path.join(screenshotsDir, filename);

  await page.screenshot({
    path: filepath,
    fullPage,
  });

  console.log(`Screenshot saved: ${filename}`);
  return filepath;
}

/**
 * Wait for page to be in stable state (no network activity, DOM stable)
 * @param page - Playwright page instance
 * @param options - Wait options
 */
export async function waitForPageLoad(
  page: Page,
  options: {
    timeout?: number;
    waitUntil?: 'load' | 'domcontentloaded' | 'networkidle';
  } = {}
): Promise<void> {
  const { timeout = 30000, waitUntil = 'networkidle' } = options;

  try {
    await page.waitForLoadState(waitUntil, { timeout });
  } catch (error) {
    console.warn(`Page load timeout after ${timeout}ms waiting for ${waitUntil}`);
    throw error;
  }
}

/**
 * Clear browser state (localStorage, sessionStorage, cookies)
 * @param page - Playwright page instance
 */
export async function clearBrowserState(page: Page): Promise<void> {
  // Clear localStorage
  await page.evaluate(() => {
    localStorage.clear();
  });

  // Clear sessionStorage
  await page.evaluate(() => {
    sessionStorage.clear();
  });

  // Clear cookies
  const context = page.context();
  await context.clearCookies();

  console.log('Browser state cleared (localStorage, sessionStorage, cookies)');
}

/**
 * Get console errors from the page
 * Useful for detecting JavaScript errors during test execution
 * @param page - Playwright page instance
 * @returns Array of console error messages
 */
export function getConsoleErrors(page: Page): string[] {
  const errors: string[] = [];

  // Listen for console messages
  page.on('console', (msg) => {
    if (msg.type() === 'error') {
      errors.push(msg.text());
    }
  });

  // Listen for page errors (uncaught exceptions)
  page.on('pageerror', (error) => {
    errors.push(`Uncaught exception: ${error.message}`);
  });

  return errors;
}

/**
 * Set up console error tracking for a page
 * Returns a function to retrieve collected errors
 * @param page - Playwright page instance
 * @returns Function that returns collected errors
 */
export function trackConsoleErrors(page: Page): () => string[] {
  const errors: string[] = [];

  page.on('console', (msg) => {
    if (msg.type() === 'error') {
      errors.push(msg.text());
    }
  });

  page.on('pageerror', (error) => {
    errors.push(`Uncaught exception: ${error.message}`);
  });

  return () => errors;
}

/**
 * Wait for an element to be visible and stable
 * @param page - Playwright page instance
 * @param selector - CSS selector for the element
 * @param options - Wait options
 */
export async function waitForElement(
  page: Page,
  selector: string,
  options: { timeout?: number; state?: 'visible' | 'hidden' | 'attached' } = {}
): Promise<void> {
  const { timeout = 10000, state = 'visible' } = options;

  await page.waitForSelector(selector, {
    timeout,
    state,
  });
}

/**
 * Retry an operation with exponential backoff
 * @param operation - Async function to retry
 * @param options - Retry configuration
 */
export async function retryOperation<T>(
  operation: () => Promise<T>,
  options: {
    maxAttempts?: number;
    initialDelay?: number;
    maxDelay?: number;
    onRetry?: (attempt: number, error: Error) => void;
  } = {}
): Promise<T> {
  const { maxAttempts = 3, initialDelay = 1000, maxDelay = 5000, onRetry } = options;

  let lastError: Error = new Error('Operation failed');

  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      return await operation();
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));

      if (attempt < maxAttempts) {
        const delay = Math.min(initialDelay * Math.pow(2, attempt - 1), maxDelay);

        if (onRetry) {
          onRetry(attempt, lastError);
        }

        console.log(`Retry attempt ${attempt}/${maxAttempts} after ${delay}ms...`);
        await sleep(delay);
      }
    }
  }

  throw lastError;
}

/**
 * Sleep for specified milliseconds
 * @param ms - Milliseconds to sleep
 */
export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Get current timestamp in ISO format
 */
export function getTimestamp(): string {
  return new Date().toISOString();
}

/**
 * Format timestamp for filenames (removes invalid characters)
 */
export function formatTimestampForFilename(timestamp?: string): string {
  const ts = timestamp || getTimestamp();
  return ts.replace(/[:.]/g, '-').replace('T', '_').split('.')[0] ?? ts;
}
