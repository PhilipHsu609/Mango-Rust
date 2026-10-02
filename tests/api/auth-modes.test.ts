import fs from 'node:fs/promises';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { startServer, stopServer } from '../helpers/server';

const DATA_DIR = path.join(process.env.HOME ?? '', 'test-manga-library');
const CONFIG_PATH = path.join(DATA_DIR, 'auth-mode-test.yml');
const BASE_URL = 'http://localhost:19001';

async function startWithConfig(
  authSettings: string,
  configArgs: string[] = ['--config', CONFIG_PATH],
): Promise<void> {
  await fs.writeFile(
    CONFIG_PATH,
    `host: localhost
port: 19001
library_path: ${DATA_DIR}
library_cache_path: ${DATA_DIR}/mango-test-cache.bin
db_path: ${DATA_DIR}/mango-test.db
log_level: warn
scan_interval_minutes: 0
${authSettings}`,
    'utf-8',
  );
  await startServer(
    { host: 'localhost', port: 19001, maxStartupTime: 3000, args: configArgs },
    { PORT: '19002' },
  );
}

afterEach(async () => {
  await stopServer();
});

describe('configured authentication modes', () => {
  it('uses the configured default user when login is disabled', async () => {
    await startWithConfig('disable_login: true\ndefault_username: testuser\n');

    const response = await fetch(`${BASE_URL}/api/admin/users`);

    expect(response.status).toBe(200);
  });

  it('rejects a configured default username that does not exist', async () => {
    await startWithConfig(
      'disable_login: true\ndefault_username: missing-user\n',
      ['-c', CONFIG_PATH],
    );

    const response = await fetch(`${BASE_URL}/api/library`);

    expect(response.status).toBe(401);
  });

  it('authenticates a known user from the configured proxy header', async () => {
    await startWithConfig('auth_proxy_header_name: X-Auth-User\n', [`--config=${CONFIG_PATH}`]);

    const noIdentity = await fetch(`${BASE_URL}/api/library`);
    const adminIdentity = await fetch(`${BASE_URL}/api/admin/users`, {
      headers: { 'X-Auth-User': 'testuser' },
    });
    const regularIdentity = await fetch(`${BASE_URL}/api/admin/users`, {
      headers: { 'X-Auth-User': 'testuser2' },
    });

    expect(noIdentity.status).toBe(401);
    expect(adminIdentity.status).toBe(200);
    expect(regularIdentity.status).toBe(403);
  });
});
