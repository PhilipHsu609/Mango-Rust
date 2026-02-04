import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    include: ['api/**/*.test.ts'],
    setupFiles: ['./api/setup.ts'],
    testTimeout: 10000,
    hookTimeout: 30000,
  },
});
