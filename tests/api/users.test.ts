import { beforeAll, describe, expect, it } from 'vitest';
import { api, BASE_URL, getSessionCookie, login } from './client';

async function loginCookie(username: string, password: string) {
  const response = await fetch(`${BASE_URL}/api/login`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });
  expect(response.status).toBe(200);
  const cookie = response.headers.get('set-cookie')!.split(';')[0];
  const authenticated = await fetch(`${BASE_URL}/api/library`, { headers: { Cookie: cookie } });
  expect(authenticated.status).toBe(200);
  return cookie;
}

async function deleteUser(username: string) {
  const response = await fetch(`${BASE_URL}/api/admin/users/${encodeURIComponent(username)}`, {
    method: 'DELETE', headers: { Cookie: getSessionCookie()! },
  });
  expect(response.status).toBe(204);
}

describe('User administration API', () => {
  beforeAll(async () => { await login(); });

  it('lists the seeded principals and their roles', async () => {
    const response = await api.get('/api/admin/users');
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual(expect.arrayContaining([
      { username: 'testuser', is_admin: true },
      { username: 'testuser2', is_admin: false },
    ]));
  });

  it('deletes an owned user with the Mango success body and removes its login', async () => {
    const username = 'users-delete-response';
    try {
      const created = await api.post('/api/admin/users', { username, password: 'delete-password', is_admin: false });
      expect(created.status).toBe(201);
      const response = await fetch(`${BASE_URL}/api/admin/user/delete/${username}`, {
        method: 'DELETE', headers: { Cookie: getSessionCookie()! },
      });
      expect(response.status).toBe(200);
      expect(await response.json()).toEqual({ success: true });
      expect(await (await api.get('/api/admin/users')).json()).not.toContainEqual({ username, is_admin: false });
      const rejected = await fetch(`${BASE_URL}/api/login`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password: 'delete-password' }),
      });
      expect(rejected.status).toBe(403);
    } finally { await deleteUser(username); }
  });

  it('allows self-deletion and makes deletion of an absent user idempotent', async () => {
    const username = 'users-self-delete';
    try {
      const created = await api.post('/api/admin/users', { username, password: 'self-delete-password', is_admin: true });
      expect(created.status).toBe(201);
      const cookie = await loginCookie(username, 'self-delete-password');
      const selfDelete = await fetch(`${BASE_URL}/api/admin/user/delete/${username}`, {
        method: 'DELETE', headers: { Cookie: cookie },
      });
      expect(selfDelete.status).toBe(200);
      expect(await selfDelete.json()).toEqual({ success: true });
      const absentDelete = await fetch(`${BASE_URL}/api/admin/user/delete/${username}`, {
        method: 'DELETE', headers: { Cookie: getSessionCookie()! },
      });
      expect(absentDelete.status).toBe(200);
      expect(await absentDelete.json()).toEqual({ success: true });
      expect(await (await api.get('/api/admin/users')).json()).not.toContainEqual({ username, is_admin: true });
    } finally { await deleteUser(username); }
  });

  it('renames an owned user without changing its role or password', async () => {
    const original = 'users-rename-source';
    const renamed = 'users-rename-target';
    try {
      const created = await api.post('/api/admin/users', { username: original, password: 'rename-password', is_admin: false });
      expect(created.status).toBe(201);
      const response = await fetch(`${BASE_URL}/admin/user/edit/${original}`, {
        method: 'POST', headers: { Cookie: getSessionCookie()!, 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({ username: renamed }), redirect: 'manual',
      });
      expect(response.status).toBe(303);
      const users = await (await api.get('/api/admin/users')).json();
      expect(users).toContainEqual({ username: renamed, is_admin: false });
      expect(users).not.toContainEqual({ username: original, is_admin: false });
      const cookie = await loginCookie(renamed, 'rename-password');
      const admin = await fetch(`${BASE_URL}/api/admin/users`, { headers: { Cookie: cookie } });
      expect(admin.status).toBe(403);
    } finally {
      await deleteUser(original);
      await deleteUser(renamed);
    }
  });

  it('rejects invalid credentials at creation and accepts the minimum valid boundaries', async () => {
    const invalid = [
      { username: 'ab', password: 'valid1' },
      { username: '1users-invalid', password: 'valid1' },
      { username: 'users-invalid!', password: 'valid1' },
      { username: 'éusers-invalid', password: 'valid1' },
      { username: 'users-short-password', password: '12345' },
      { username: 'users-nonascii-password', password: 'abcdeé' },
    ];
    try {
      for (const credentials of invalid) {
        const response = await api.post('/api/admin/users', { ...credentials, is_admin: false });
        expect(response.status).toBe(400);
      }
      const users = await (await api.get('/api/admin/users')).json();
      for (const { username } of invalid) expect(users).not.toContainEqual({ username, is_admin: false });
      const boundary = await api.post('/api/admin/users', { username: 'A_1', password: '123456', is_admin: false });
      expect(boundary.status).toBe(201);
      await loginCookie('A_1', '123456');
    } finally {
      for (const username of [...invalid.map((user) => user.username), 'A_1']) await deleteUser(username);
    }
  });

  it('rejects browser creation and renaming without changing the existing principal', async () => {
    const username = 'users-validation-source';
    try {
      const created = await api.post('/api/admin/users', { username, password: 'valid-password', is_admin: false });
      expect(created.status).toBe(201);
      for (const [path, name] of [['/admin/user/edit', 'ab'], [`/admin/user/edit/${username}`, 'invalid!']]) {
        const response = await fetch(`${BASE_URL}${path}`, {
          method: 'POST', headers: { Cookie: getSessionCookie()!, 'Content-Type': 'application/x-www-form-urlencoded' },
          body: new URLSearchParams({ username: name, password: 'valid-password' }), redirect: 'manual',
        });
        expect(response.status).toBe(303);
        const target = new URL(response.headers.get('location')!, BASE_URL);
        expect(target.pathname).toBe('/admin/user/edit');
        expect(target.searchParams.has('error')).toBe(true);
        const page = await fetch(target, { headers: { Cookie: getSessionCookie()! } });
        expect(page.status).toBe(200);
      }
      const users = await (await api.get('/api/admin/users')).json();
      expect(users).toContainEqual({ username, is_admin: false });
      expect(users).not.toContainEqual({ username: 'ab', is_admin: false });
      expect(users).not.toContainEqual({ username: 'invalid!', is_admin: false });
      await loginCookie(username, 'valid-password');
    } finally {
      for (const name of [username, 'ab', 'invalid!']) await deleteUser(name);
    }
  });

  it('retains the old password on empty or invalid password updates', async () => {
    const username = 'users-password-source';
    try {
      const created = await api.post('/api/admin/users', { username, password: 'valid-password', is_admin: false });
      expect(created.status).toBe(201);
      for (const [password, status] of [['', 204], ['abcdeé', 400]] as const) {
        const response = await fetch(`${BASE_URL}/api/admin/users/${username}`, {
          method: 'PATCH', headers: { Cookie: getSessionCookie()!, 'Content-Type': 'application/json' },
          body: JSON.stringify({ is_admin: false, password }),
        });
        expect(response.status).toBe(status);
        await loginCookie(username, 'valid-password');
      }
      const cookie = await loginCookie(username, 'valid-password');
      const invalidChange = await fetch(`${BASE_URL}/api/user/change-password`, {
        method: 'POST', headers: { Cookie: cookie, 'Content-Type': 'application/json' },
        body: JSON.stringify({ current_password: 'valid-password', new_password: 'abcdeé' }),
      });
      expect(invalidChange.status).toBe(400);
      await loginCookie(username, 'valid-password');
    } finally { await deleteUser(username); }
  });
});
