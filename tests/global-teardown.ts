import { stopServer } from './helpers/server.js';

/**
 * Global teardown - runs once after all tests
 * Stops the Mango server and cleans up test artifacts
 */
async function globalTeardown(): Promise<void> {
  console.log('🧹 Global teardown: Starting...');

  try {
    // Step 1: Stop Mango server
    console.log('🛑 Stopping Mango server...');
    await stopServer();
    console.log('✅ Server stopped successfully');

    // Step 2: Clean up test artifacts (optional - keep screenshots for debugging)
    // Note: Screenshots are kept by default for debugging purposes
    // To enable cleanup, uncomment the following code:
    // import * as fs from 'fs';
    // import * as path from 'path';
    // const screenshotsDir = path.join(process.cwd(), 'screenshots');
    // if (fs.existsSync(screenshotsDir)) {
    //   fs.rmSync(screenshotsDir, { recursive: true, force: true });
    //   console.log('✅ Cleaned up test artifacts');
    // }

    console.log('🎉 Global teardown: Complete');
  } catch (error) {
    console.error('❌ Global teardown failed:', error);
    // Don't throw - allow tests to complete even if teardown fails
  }
}

export default globalTeardown;
