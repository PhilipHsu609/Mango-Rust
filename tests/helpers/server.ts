import { spawn, ChildProcess } from 'child_process';

/**
 * Server management utilities for integration tests
 * Handles starting, stopping, and health checking the Mango server
 */

let serverProcess: ChildProcess | null = null;

interface ServerConfig {
  port: number;
  host: string;
  maxStartupTime: number; // milliseconds
  pollInterval: number; // milliseconds
}

const defaultConfig: ServerConfig = {
  port: 9000,
  host: 'localhost',
  maxStartupTime: 30000, // 30 seconds
  pollInterval: 500, // 500ms
};

/**
 * Start the Mango server using cargo run
 * @param config - Optional server configuration
 * @returns Promise that resolves when server is ready
 */
export async function startServer(config: Partial<ServerConfig> = {}): Promise<void> {
  const cfg = { ...defaultConfig, ...config };

  if (serverProcess) {
    console.log('Server already running, skipping startup');
    return;
  }

  console.log('Starting Mango server...');

  // Use isolated test directory for all test data
  const testDataDir = process.env.HOME + '/test-manga-library';

  // Spawn cargo run process
  serverProcess = spawn('cargo', ['run', '--release'], {
    cwd: process.cwd().replace('/tests', ''), // Run from project root
    env: {
      ...process.env,
      RUST_LOG: 'info',
      MANGO_PORT: cfg.port.toString(),
      MANGO_HOST: cfg.host,
      // All test data in test library directory
      MANGO_LIBRARY_PATH: testDataDir,
      MANGO_DB_PATH: `${testDataDir}/mango-test.db`,
      MANGO_CACHE_PATH: `${testDataDir}/mango-test-cache.bin`,
      MANGO_CONFIG_PATH: `${testDataDir}/config-test.yml`,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  // Capture server logs for debugging
  const logs: string[] = [];
  serverProcess.stdout?.on('data', (data: Buffer) => {
    const line = data.toString().trim();
    logs.push(line);
    if (process.env.DEBUG) {
      console.log(`[Server] ${line}`);
    }
  });

  serverProcess.stderr?.on('data', (data: Buffer) => {
    const line = data.toString().trim();
    logs.push(`[ERROR] ${line}`);
    if (process.env.DEBUG) {
      console.error(`[Server Error] ${line}`);
    }
  });

  serverProcess.on('error', (error) => {
    console.error('Failed to start server process:', error.message);
    throw new Error(`Server startup failed: ${error.message}`);
  });

  serverProcess.on('exit', (code) => {
    if (code !== null && code !== 0) {
      console.error(`Server exited with code ${code}`);
      console.error('Recent logs:', logs.slice(-10).join('\n'));
    }
    serverProcess = null;
  });

  // Wait for server to be ready
  try {
    await waitForServerReady(cfg);
    console.log('Server started successfully');
  } catch (error) {
    // If startup fails, kill the process and clean up
    if (serverProcess) {
      serverProcess.kill('SIGTERM');
      serverProcess = null;
    }
    console.error('Server startup failed:', error);
    console.error('Recent logs:', logs.slice(-20).join('\n'));
    throw error;
  }
}

/**
 * Wait for the server to become ready by polling the base URL
 * Uses exponential backoff for polling
 * @param config - Server configuration
 */
export async function waitForServerReady(config: Partial<ServerConfig> = {}): Promise<void> {
  const cfg = { ...defaultConfig, ...config };
  const baseUrl = `http://${cfg.host}:${cfg.port}`;
  const startTime = Date.now();
  let attempt = 0;

  while (Date.now() - startTime < cfg.maxStartupTime) {
    attempt++;

    try {
      // Try to fetch the root URL
      const response = await fetch(baseUrl, {
        method: 'GET',
        signal: AbortSignal.timeout(2000), // 2 second timeout per request
      });

      // If we get any response (even 404), server is up
      if (response.status) {
        console.log(`Server ready after ${Date.now() - startTime}ms (${attempt} attempts)`);
        return;
      }
    } catch (error) {
      // Server not ready yet, continue polling
      const errorMessage = error instanceof Error ? error.message : String(error);
      if (process.env.DEBUG) {
        console.log(`Attempt ${attempt}: Server not ready (${errorMessage})`);
      }
    }

    // Exponential backoff: 500ms, 1000ms, 1500ms, 2000ms (max)
    const backoff = Math.min(cfg.pollInterval * attempt, 2000);
    await sleep(backoff);
  }

  throw new Error(
    `Server failed to start within ${cfg.maxStartupTime}ms after ${attempt} attempts`
  );
}

/**
 * Stop the Mango server gracefully
 * Sends SIGTERM and waits for process to exit
 */
export async function stopServer(): Promise<void> {
  if (!serverProcess) {
    console.log('No server process to stop');
    return;
  }

  console.log('Stopping Mango server...');

  return new Promise((resolve, reject) => {
    if (!serverProcess) {
      resolve();
      return;
    }

    const timeout = setTimeout(() => {
      console.warn('Server did not stop gracefully, forcing kill');
      if (serverProcess) {
        serverProcess.kill('SIGKILL');
      }
      reject(new Error('Server shutdown timeout'));
    }, 10000); // 10 second timeout

    serverProcess.on('exit', () => {
      clearTimeout(timeout);
      serverProcess = null;
      console.log('Server stopped successfully');
      resolve();
    });

    // Send SIGTERM for graceful shutdown
    serverProcess.kill('SIGTERM');
  });
}

/**
 * Get the current server process (for debugging)
 */
export function getServerProcess(): ChildProcess | null {
  return serverProcess;
}

/**
 * Check if server is currently running
 */
export function isServerRunning(): boolean {
  return serverProcess !== null && !serverProcess.killed;
}

/**
 * Sleep utility
 */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
