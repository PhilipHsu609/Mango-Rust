import { execSync } from 'child_process';
import * as path from 'path';
import { startServer, waitForServerReady } from './helpers/server.js';
import { createTestUser } from './helpers/auth.js';

/**
 * Global setup - runs once before all tests
 * Builds CSS, starts the Mango server, and creates test user
 */
async function globalSetup(): Promise<void> {
  console.log('🔧 Global setup: Starting...');

  try {
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

    // Step 4: Create test user
    console.log('👤 Creating test user...');
    const dbPath = path.join(process.env.HOME || '', 'mango', 'mango.db');
    await createTestUser(dbPath);
    console.log('✅ Test user ready');

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
