import { execSync } from 'child_process';
import * as path from 'path';
import { startServer, waitForServerReady } from './helpers/server.js';
import { createTestUser, REGULAR_USER } from './helpers/auth.js';

/**
 * Global setup - runs once before all tests
 * Builds CSS, starts the Mango server, and creates test user
 */
async function globalSetup(): Promise<void> {
  console.log('🔧 Global setup: Starting...');

  try {
    // Step 0: Create test config file
    const testDataDir = path.join(process.env.HOME || '', 'test-manga-library');
    const configPath = path.join(testDataDir, 'config-test.yml');

    // Ensure test directory exists
    const fs = await import('fs/promises');
    await fs.mkdir(testDataDir, { recursive: true });

    // Create minimal test config
    const testConfig = `host: localhost
port: 9000
library_path: ${testDataDir}
db_path: ${testDataDir}/mango-test.db
log_level: info
cache_enabled: true
cache_size_mbs: 50
library_cache_path: ${testDataDir}/mango-test-cache.bin
`;
    await fs.writeFile(configPath, testConfig, 'utf-8');
    console.log('✅ Test config created at:', configPath);

    // Step 1: Build CSS
    console.log('📦 Building CSS with LESS...');
    const projectRoot = process.cwd().replace('/tests', '');

    try {
      execSync('./build-css.sh', {
        cwd: projectRoot,
        stdio: 'inherit',
        env: { ...process.env },
      });
      console.log('✅ CSS build complete');
    } catch (error) {
      console.error('❌ CSS build failed:', error);
      throw new Error('Failed to build CSS');
    }

    // Step 2: Start Mango server
    console.log('🚀 Starting Mango server...');
    await startServer();
    console.log('✅ Server started successfully');

    // Step 3: Wait for server to be ready
    console.log('⏳ Waiting for server to be ready...');
    await waitForServerReady();
    console.log('✅ Server is ready');

    // Step 4: Create test users
    console.log('👤 Creating test users...');
    const dbPath = path.join(testDataDir, 'mango-test.db');
    await createTestUser(dbPath);                       // testuser (admin) - default
    await createTestUser(dbPath, REGULAR_USER, false);  // testuser2 (regular)
    console.log('✅ Test users ready');

    // Step 5: Trigger library scan
    console.log('📚 Triggering library scan...');
    try {
      const response = await fetch('http://localhost:9000/api/admin/scan', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
      });
      if (response.ok) {
        console.log('✅ Library scan initiated');
        // Wait a bit for scan to complete
        await new Promise((resolve) => setTimeout(resolve, 2000));
      } else {
        console.warn(`⚠️  Library scan returned status ${response.status}`);
      }
    } catch (error) {
      console.warn('⚠️  Failed to trigger library scan:', error);
    }

    console.log('🎉 Global setup: Complete');
  } catch (error) {
    console.error('❌ Global setup failed:', error);
    throw error;
  }
}

export default globalSetup;
