import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './smoke',
  fullyParallel: false,
  retries: 0,
  workers: 1,
  timeout: 30000,
  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:9000',
    headless: true,
    screenshot: 'only-on-failure',
  },
});
