import { defineConfig, devices } from '@playwright/test';

// Use chromium in CI (works on native Linux), Firefox locally (works better in WSL)
const browserProject = process.env.CI
  ? {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        headless: true,
      },
    }
  : {
      name: 'firefox',
      use: {
        ...devices['Desktop Firefox'],
        headless: process.env.HEADED ? false : true,
      },
    };

export default defineConfig({
  testDir: './smoke',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: [
    ['html', { outputFolder: 'playwright-report', open: 'never' }],
    ['list'],
  ],
  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:9000',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    actionTimeout: 10000,
    navigationTimeout: 30000,
  },
  timeout: 60000,
  expect: {
    timeout: 10000,
  },
  projects: [browserProject],
  globalSetup: './smoke/globalSetup.ts',
  globalTeardown: './smoke/globalTeardown.ts',
});
