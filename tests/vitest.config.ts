import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: false,
    environment: 'node',
    include: ['api/**/*.test.ts'],
    globalSetup: ['./global-setup.ts'],
    testTimeout: 10000,
    hookTimeout: 60000,
    fileParallelism: false, // Run test files sequentially to share server
  },
});
