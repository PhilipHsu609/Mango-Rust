import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    include: ['api/**/*.test.ts'],
    globalSetup: ['./api/globalSetup.ts'],
    testTimeout: 10000,
    hookTimeout: 60000,
    fileParallelism: false, // Run test files sequentially to share server
  },
});
