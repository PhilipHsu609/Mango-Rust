import { startServer, stopServer, waitForServerReady } from '../helpers/server.js';
import { createTestUser, REGULAR_USER } from '../helpers/auth.js';
import * as path from 'path';
import * as fs from 'fs/promises';

function getTestDataDir(): string {
  const home = process.env.HOME;
  if (!home) {
    throw new Error('HOME environment variable is not set.');
  }
  return path.join(home, 'test-manga-library');
}

const TEST_DATA_DIR = getTestDataDir();

export async function setup() {
  console.log('Global setup: Starting test environment...');

  // Create test directory
  await fs.mkdir(TEST_DATA_DIR, { recursive: true });

  // Write test config
  const configPath = path.join(TEST_DATA_DIR, 'config-test.yml');
  const testConfig = `host: localhost
port: 9000
library_path: ${TEST_DATA_DIR}
db_path: ${TEST_DATA_DIR}/mango-test.db
log_level: info
`;
  await fs.writeFile(configPath, testConfig, 'utf-8');

  // Start server
  await startServer();

  // Create test users
  const dbPath = path.join(TEST_DATA_DIR, 'mango-test.db');
  await createTestUser(dbPath);
  await createTestUser(dbPath, REGULAR_USER, false);

  console.log('Global setup: Complete');
}

export async function teardown() {
  console.log('Global teardown: Stopping server...');
  await stopServer();
  console.log('Global teardown: Complete');
}
