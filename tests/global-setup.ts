import { execFile } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import type { GlobalSetupContext } from 'vitest/node';
import { startServer, type TestServer } from './helpers/server';
import { TEST_USER } from './helpers/test-users';

export default async function setup({ provide }: GlobalSetupContext) {
  const directory = await mkdtemp(path.join(tmpdir(), 'mango-test-library-'));
  const libraryPath = path.join(directory, 'library');
  let server: TestServer | undefined;
  const cleanup = async () => {
    try {
      await server?.close();
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  };
  try {
    const fixtureScript = fileURLToPath(new URL('./fixtures/setup-test-library.sh', import.meta.url));
    await promisify(execFile)('bash', [fixtureScript, libraryPath]);
    server = await startServer({ libraryPath });
    const credentials = Buffer.from(`${TEST_USER.username}:${TEST_USER.password}`).toString('base64');
    const deadline = Date.now() + 10000;
    while (true) {
      const response = await fetch(`${server.url}/api/library`, {
        headers: { Authorization: `Basic ${credentials}` },
      });
      if (response.status !== 200) throw new Error(`Fixture library request failed: ${response.status}`);
      const library = await response.json() as { titles: { entries?: unknown[] }[] };
      if (library.titles.length === 7 && library.titles.every(title => title.entries?.length === 5)) break;
      if (Date.now() >= deadline) throw new Error('Fixture library scan did not complete');
      await delay(50);
    }
    provide('baseUrl', server.url);
    provide('libraryPath', libraryPath);
    return cleanup;
  } catch (error) {
    await cleanup();
    throw error;
  }
}
