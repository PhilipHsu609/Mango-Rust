import { execFile, spawn, type ChildProcess } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { REGULAR_USER, TEST_USER } from './test-users';

const execute = promisify(execFile);
const ROOT = fileURLToPath(new URL('../../', import.meta.url));
const BINARY = path.join(ROOT, 'target/debug/mango-rust');

export async function runCli(...args: string[]): Promise<string> {
  const { stdout } = await execute(BINARY, args, { cwd: ROOT, timeout: 15000 });
  return stdout;
}

export interface TestServer {
  url: string;
  configPath: string;
  close(): Promise<void>;
}

interface ServerOptions {
  libraryPath?: string;
  settings?: Record<string, string | number | boolean>;
  configArgs?: (configPath: string) => string[];
  env?: NodeJS.ProcessEnv;
}

async function availablePort(): Promise<number> {
  const socket = createServer();
  const ready = Promise.withResolvers<void>();
  socket.once('error', ready.reject);
  socket.listen(0, '127.0.0.1', ready.resolve);
  await ready.promise;
  const address = socket.address();
  if (!address || typeof address === 'string') throw new Error('Missing test listener address');
  const closed = Promise.withResolvers<void>();
  socket.close(error => error ? closed.reject(error) : closed.resolve());
  await closed.promise;
  return address.port;
}

/** Each handle owns its process, configuration, database, and temporary paths. */
export async function startServer(options: ServerOptions = {}): Promise<TestServer> {
  const directory = await mkdtemp(path.join(tmpdir(), 'mango-test-server-'));
  let process: ChildProcess | undefined;
  let exited: Promise<void> | undefined;
  let closing: Promise<void> | undefined;
  const close = (): Promise<void> => closing ??= (async () => {
    try {
      if (process?.pid && process.exitCode === null && process.signalCode === null) {
        const force = setTimeout(() => process?.kill('SIGKILL'), 5000);
        try {
          process.kill('SIGTERM');
          await exited;
        } finally {
          clearTimeout(force);
        }
      } else {
        await exited;
      }
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  })();

  try {
    const port = await availablePort();
    const url = `http://127.0.0.1:${port}`;
    const libraryPath = options.libraryPath ?? path.join(directory, 'library');
    if (!options.libraryPath) await mkdir(libraryPath);
    const configPath = path.join(directory, 'config.yml');
    const settings = {
      host: '127.0.0.1',
      port,
      library_path: libraryPath,
      db_path: path.join(directory, 'mango.db'),
      queue_db_path: path.join(directory, 'queue.db'),
      library_cache_path: path.join(directory, 'cache.bin'),
      upload_path: path.join(directory, 'uploads'),
      plugin_path: path.join(directory, 'plugins'),
      scan_interval_minutes: 0,
      thumbnail_generation_interval_hours: 0,
      ...options.settings,
      log_level: 'info',
    };
    await writeFile(configPath, Object.entries(settings)
      .map(([key, value]) => `${key}: ${JSON.stringify(value)}`).join('\n') + '\n');
    for (const [credentials, admin] of [[TEST_USER, true], [REGULAR_USER, false]] as const) {
      await runCli('--config', configPath, 'admin', 'user', 'add',
        '--username', credentials.username, '--password', credentials.password,
        ...(admin ? ['--admin'] : []));
    }

    process = spawn(BINARY, options.configArgs?.(configPath) ?? ['--config', configPath], {
      cwd: ROOT,
      env: { ...globalThis.process.env, ...options.env },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let output = '';
    let failure: Error | undefined;
    const stopped = Promise.withResolvers<void>();
    exited = stopped.promise;
    process.once('close', stopped.resolve);
    process.once('error', error => { failure = error; });
    const capture = (chunk: Buffer) => { output = (output + chunk.toString()).slice(-16000); };
    process.stdout?.on('data', capture);
    process.stderr?.on('data', capture);
    const deadline = Date.now() + 10000;
    while (Date.now() < deadline) {
      if (failure || process.exitCode !== null || process.signalCode !== null) {
        throw new Error(`Test server exited during startup: ${failure?.message ?? process.exitCode}\n${output}`);
      }
      // Require our child's successful bind, not a response from an unrelated app.
      if (output.includes(`Server listening on 127.0.0.1:${port}`)) {
        try {
          const response = await fetch(`${url}/login`, { signal: AbortSignal.timeout(500), redirect: 'manual' });
          if (response.status === 200) return { url, configPath, close };
        } catch {
          // The listener can bind before its first request is accepted.
        }
      }
      await delay(50);
    }
    throw new Error(`Test server did not become ready at ${url}\n${output}`);
  } catch (error) {
    await close();
    throw error;
  }
}
