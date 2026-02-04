import { beforeAll, afterAll } from 'vitest';
import { startServer, stopServer } from '../helpers/server';
import { createTestUser, REGULAR_USER } from '../helpers/auth';
import * as path from 'path';
import * as fs from 'fs/promises';

function getTestDataDir(): string {
  const home = process.env.HOME;
  if (!home) {
    throw new Error(
      'HOME environment variable is not set. Cannot determine test data directory.'
    );
  }
  return path.join(home, 'test-manga-library');
}

const TEST_DATA_DIR = getTestDataDir();

beforeAll(async () => {
  console.log('Setting up test environment...');

  // Step 1: Create test directory
  try {
    await fs.mkdir(TEST_DATA_DIR, { recursive: true });
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to create test directory '${TEST_DATA_DIR}': ${msg}`);
  }

  // Step 2: Write test config
  const configPath = path.join(TEST_DATA_DIR, 'config-test.yml');
  try {
    const testConfig = `host: localhost
port: 9000
library_path: ${TEST_DATA_DIR}
db_path: ${TEST_DATA_DIR}/mango-test.db
log_level: info
`;
    await fs.writeFile(configPath, testConfig, 'utf-8');
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to write test config: ${msg}`);
  }

  // Step 3: Start server (includes waitForServerReady internally)
  try {
    await startServer();
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to start server: ${msg}`);
  }

  // Step 4: Create test users
  const dbPath = path.join(TEST_DATA_DIR, 'mango-test.db');
  try {
    await createTestUser(dbPath);
    await createTestUser(dbPath, REGULAR_USER, false);
  } catch (error) {
    await stopServer().catch(() => {}); // Best effort cleanup
    const msg = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to create test users: ${msg}`);
  }

  console.log('Test environment ready');
}, 60000);

afterAll(async () => {
  try {
    await stopServer();
  } catch (error) {
    console.error('Failed to stop server:', error);
  }
});
