import { afterEach, describe, expect, it } from 'vitest';
import { startServer, type TestServer } from '../helpers/server';
import { REGULAR_USER, TEST_USER } from '../helpers/test-users';

let server: TestServer | undefined;

async function startWithConfig(
  settings: Record<string, string | number | boolean>,
  configArgs?: (configPath: string) => string[],
): Promise<TestServer> {
  server = await startServer({ settings, configArgs, env: { PORT: '1' } });
  return server;
}

afterEach(async () => {
  await server?.close();
  server = undefined;
});

describe('configured authentication modes', () => {
  it('uses the configured default user and its administrator role when login is disabled', async () => {
    const { url } = await startWithConfig({ disable_login: true, default_username: TEST_USER.username });
    const response = await fetch(`${url}/api/admin/users`);
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual(expect.arrayContaining([
      { username: TEST_USER.username, is_admin: true },
      { username: REGULAR_USER.username, is_admin: false },
    ]));
  });

  it('rejects a configured default username that does not exist', async () => {
    const { url } = await startWithConfig(
      { disable_login: true, default_username: 'missing-user' },
      configPath => ['-c', configPath],
    );
    const response = await fetch(`${url}/api/library`);
    expect(response.status).toBe(401);
  });

  it('uses the proxy identity and preserves administrator authorization', async () => {
    const { url } = await startWithConfig(
      { auth_proxy_header_name: 'X-Auth-User' },
      configPath => [`--config=${configPath}`],
    );
    const noIdentity = await fetch(`${url}/api/library`);
    const unknownIdentity = await fetch(`${url}/api/library`, { headers: { 'X-Auth-User': 'missing-user' } });
    const adminIdentity = await fetch(`${url}/api/admin/users`, { headers: { 'X-Auth-User': TEST_USER.username } });
    const regularIdentity = await fetch(`${url}/api/admin/users`, { headers: { 'X-Auth-User': REGULAR_USER.username } });
    expect(noIdentity.status).toBe(401);
    expect(unknownIdentity.status).toBe(401);
    expect(adminIdentity.status).toBe(200);
    expect(regularIdentity.status).toBe(403);
  });

  it('scopes login cookies to the configured reverse-proxy base URL', async () => {
    const { url } = await startWithConfig({ base_url: '/mango' });
    const response = await fetch(`${url}/api/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(TEST_USER),
    });
    expect(response.status).toBe(200);
    expect(response.headers.get('set-cookie')).toMatch(/(?:^|;\s*)Path=\/mango\/(?:;|$)/);
  });
});
