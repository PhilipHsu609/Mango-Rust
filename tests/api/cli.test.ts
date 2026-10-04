import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { runCli, startServer, type TestServer } from '../helpers/server';

let server: TestServer;

beforeAll(async () => { server = await startServer(); });
afterAll(async () => { await server?.close(); });

async function authenticate(username: string, password: string, expectedStatus = 200) {
  const response = await fetch(`${server.url}/api/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });
  expect(response.status).toBe(expectedStatus);
  return response.json();
}

describe('user management CLI', () => {
  it('adds, renames, changes credentials and roles, and deletes users with global config options', async () => {
    await runCli('--config', server.configPath, 'admin', 'user', 'add', '-u', 'cli-user', '-p', 'first-pass', '-a');
    const addedUsers = await runCli('admin', 'user', 'list', `--config=${server.configPath}`);
    expect(addedUsers).toMatch(/cli-user\s+true/);
    expect(await authenticate('cli-user', 'first-pass')).toMatchObject({ success: true, is_admin: true });

    await runCli('admin', 'user', '-c', server.configPath, 'update', 'cli-user', '-u', 'renamed-user', '-p', 'second-pass');
    const updatedUsers = await runCli('admin', 'user', 'list', '--config', server.configPath);
    expect(updatedUsers).toMatch(/renamed-user\s+false/);
    expect(updatedUsers).not.toContain('cli-user');
    expect(await authenticate('cli-user', 'first-pass', 403)).toMatchObject({ success: false });
    expect(await authenticate('renamed-user', 'first-pass', 403)).toMatchObject({ success: false });
    expect(await authenticate('renamed-user', 'second-pass')).toMatchObject({ success: true, is_admin: false });

    await runCli('admin', 'user', 'delete', 'renamed-user', '-c', server.configPath);
    const deletedUsers = await runCli('admin', 'user', 'list', '--config', server.configPath);
    expect(deletedUsers).not.toContain('renamed-user');
    expect(await authenticate('renamed-user', 'second-pass', 403)).toMatchObject({ success: false });
  });
});
