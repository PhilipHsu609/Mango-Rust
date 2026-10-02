import { startServer } from '../helpers/server.js';
import { createTestUser, REGULAR_USER } from '../helpers/test-users.js';
import * as path from 'path';
import * as fs from 'fs/promises';

async function globalSetup() {
  console.log('Smoke tests: Starting server...');

  const testDataDir = path.join(process.env.HOME || '', 'test-manga-library');

  // Create test directory and config
  await fs.mkdir(testDataDir, { recursive: true });

  const configPath = path.join(testDataDir, 'config-test.yml');
  const testConfig = `host: localhost
port: 9000
library_path: ${testDataDir}
db_path: ${testDataDir}/mango-test.db
log_level: info
`;
  await fs.writeFile(configPath, testConfig, 'utf-8');

  // Start server
  await startServer();

  // Create test users
  const dbPath = path.join(testDataDir, 'mango-test.db');
  createTestUser(dbPath);
  createTestUser(dbPath, REGULAR_USER, false);

  console.log('Smoke tests: Server ready');
}

export default globalSetup;
