import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for Mango-Rust integration tests
 * See https://playwright.dev/docs/test-configuration
 */
export default defineConfig({
  // Directory for test files
  testDir: './integration',

  // Run tests in files in parallel
  fullyParallel: true,

  // Fail the build on CI if you accidentally left test.only in the source code
  forbidOnly: !!process.env.CI,

  // Retry on CI only
  retries: process.env.CI ? 2 : 0,

  // Reporter configuration
  reporter: [
    ['html', { outputFolder: 'playwright-report', open: 'never' }],
    ['json', { outputFile: 'test-results/results.json' }],
    ['list'],
  ],

  // Shared settings for all projects
  use: {
    // Base URL for navigation
    baseURL: process.env.BASE_URL || 'http://localhost:9000',

    // Collect trace on failure for debugging
    trace: 'on-first-retry',

    // Screenshot on failure only
    screenshot: 'only-on-failure',

    // Video on failure only
    video: 'retain-on-failure',

    // Default timeout for actions (click, fill, etc.)
    actionTimeout: 10000,

    // Default navigation timeout
    navigationTimeout: 30000,
  },

  // Global timeout for each test
  timeout: 60000,

  // Maximum time for expect() assertions
  expect: {
    timeout: 10000,
  },

  // Configure projects for different browsers
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        // Run headless in CI, headed in local dev for debugging
        headless: process.env.CI ? true : false,
      },
    },

    // Uncomment to test in Firefox
    // {
    //   name: 'firefox',
    //   use: { ...devices['Desktop Firefox'] },
    // },

    // Uncomment to test in WebKit (Safari)
    // {
    //   name: 'webkit',
    //   use: { ...devices['Desktop Safari'] },
    // },

    // Mobile viewports for responsive testing
    // {
    //   name: 'Mobile Chrome',
    //   use: { ...devices['Pixel 5'] },
    // },
  ],

  // Run global setup before all tests (start server)
  globalSetup: './global-setup.ts',

  // Run global teardown after all tests (stop server)
  globalTeardown: './global-teardown.ts',

  // Web server configuration (optional - for local development)
  // This will start the server automatically if not running
  // Commented out by default as we use global-setup for more control
  // webServer: {
  //   command: 'cargo run --release',
  //   url: 'http://localhost:9000',
  //   reuseExistingServer: !process.env.CI,
  //   timeout: 120000,
  // },
});
