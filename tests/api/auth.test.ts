import { describe, it, expect, beforeEach } from 'vitest';
import { api, login, logout, getSessionCookie, BASE_URL } from './client';

describe('Auth API', () => {
  beforeEach(() => {
    logout();
  });

  describe('POST /login', () => {
    it('valid credentials sets session cookie and redirects to home', async () => {
      const response = await fetch(`${BASE_URL}/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({ username: 'testuser', password: 'testpass123' }),
        redirect: 'manual',
      });

      expect(response.status).toBe(303);
      expect(response.headers.get('set-cookie')).toContain('mango-sessid-');
      expect(response.headers.get('location')).toBe('/');
    });

    it('invalid and missing credentials redirect to login without a session', async () => {
      for (const body of [
        new URLSearchParams({ username: 'testuser', password: 'wrongpassword' }),
        new URLSearchParams({}),
      ]) {
        const response = await fetch(`${BASE_URL}/login`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
          body,
          redirect: 'manual',
        });

        expect(response.status).toBe(303);
        expect(response.headers.get('location')).toBe('/login');
        expect(response.headers.get('set-cookie')).toBeNull();
      }
    });
  });

  describe('POST /api/login', () => {
    it('returns Mango login JSON and a Mango-named session cookie', async () => {
      const response = await fetch(`${BASE_URL}/api/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: 'testuser', password: 'testpass123' }),
      });

      expect(response.status).toBe(200);
      expect(await response.json()).toMatchObject({
        success: true,
        session_id: expect.any(String),
        is_admin: true,
      });
      expect(response.headers.get('set-cookie')).toContain('mango-sessid-');
    });

    it('returns Mango login errors for invalid credentials', async () => {
      const response = await fetch(`${BASE_URL}/api/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: 'testuser', password: 'wrongpassword' }),
      });

      expect(response.status).toBe(403);
      expect(await response.json()).toEqual({
        success: false,
        error: 'Nil assertion failed',
      });
      expect(response.headers.get('set-cookie')).toBeNull();
    });

    it('accepts Bearer session IDs on protected API routes', async () => {
      const loginResponse = await fetch(`${BASE_URL}/api/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: 'testuser', password: 'testpass123' }),
      });
      const { session_id: sessionId } = await loginResponse.json();

      const response = await fetch(`${BASE_URL}/api/library`, {
        headers: { Authorization: `Bearer ${sessionId}` },
      });

      expect(response.status).toBe(200);
    });

    it('accepts Basic credentials on protected non-OPDS routes', async () => {
      const credentials = Buffer.from('testuser:testpass123').toString('base64');

      const response = await fetch(`${BASE_URL}/api/library`, {
        headers: { Authorization: `Basic ${credentials}` },
      });

      expect(response.status).toBe(200);
      expect(response.headers.get('set-cookie')).toContain('mango-sessid-');
    });

    it('gives a valid session priority over Basic credentials', async () => {
      const loginResponse = await fetch(`${BASE_URL}/api/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: 'testuser', password: 'testpass123' }),
      });
      const sessionCookie = loginResponse.headers.get('set-cookie')?.split(';')[0];
      const credentials = Buffer.from('testuser2:testpass123').toString('base64');

      const response = await fetch(`${BASE_URL}/api/admin/users`, {
        headers: {
          Cookie: sessionCookie ?? '',
          Authorization: `Basic ${credentials}`,
        },
      });

      expect(response.status).toBe(200);
    });
  });

  describe('GET /login', () => {
    it('returns login page HTML', async () => {
      const response = await fetch(`${BASE_URL}/login`);

      expect(response.status).toBe(200);
      expect(response.headers.get('content-type')).toContain('text/html');

      const body = await response.text();
      expect(body).toContain('username');
      expect(body).toContain('password');
    });
  });

  describe('GET /logout', () => {
    it('clears session and redirects to login page', async () => {
      await login();
      const sessionCookie = getSessionCookie();
      expect(sessionCookie).toBeTruthy();

      const response = await fetch(`${BASE_URL}/logout`, {
        headers: { Cookie: sessionCookie! },
        redirect: 'manual',
      });

      expect(response.status).toBe(303);
      expect(response.headers.get('location')).toBe('/login');

      // Verify old session is actually invalidated
      const verifyResponse = await fetch(`${BASE_URL}/api/library`, {
        headers: { Cookie: sessionCookie! },
        redirect: 'manual',
      });
      expect(verifyResponse.status).toBe(401);
    });

    it('redirects to login even without session', async () => {
      const response = await fetch(`${BASE_URL}/logout`, {
        redirect: 'manual',
      });

      expect(response.status).toBe(303);
      expect(response.headers.get('location')).toBe('/login');
    });
  });

  describe('Protected routes', () => {
    it('unauthenticated API requests return Mango 401 responses', async () => {
      const response = await fetch(`${BASE_URL}/api/library`, {
        redirect: 'manual',
      });

      expect(response.status).toBe(401);
      expect(await response.text()).toBe('Unauthorized');
      const apiRootResponse = await fetch(`${BASE_URL}/api`, {
        redirect: 'manual',
      });
      expect(apiRootResponse.status).toBe(401);
      expect(await apiRootResponse.text()).toBe('Unauthorized');
    });

    it('authenticated request to /api/library succeeds', async () => {
      await login();
      const response = await api.get('/api/library');

      expect(response.status).toBe(200);
      expect(response.headers.get('content-type')).toContain('application/json');
    });

    it('unauthenticated request to home page redirects to login', async () => {
      const response = await fetch(`${BASE_URL}/`, {
        redirect: 'manual',
      });

      expect(response.status).toBe(303);
      expect(response.headers.get('location')).toBe('/login');
    });

    it('authenticated request to home page succeeds', async () => {
      await login();
      const response = await api.get('/');

      expect(response.status).toBe(200);
      expect(response.headers.get('content-type')).toContain('text/html');
    });
  });

  describe('Admin routes', () => {
    it('non-admin user gets 403 on /admin', async () => {
      await login('testuser2', 'testpass123'); // non-admin user
      const response = await api.get('/admin');

      expect(response.status).toBe(403);
    });

    it('admin user accesses /admin successfully', async () => {
      await login(); // testuser is admin
      const response = await api.get('/admin');

      expect(response.status).toBe(200);
    });

    it('non-admin user gets 403 on /api/admin/users', async () => {
      await login('testuser2', 'testpass123');
      const response = await api.get('/api/admin/users');

      expect(response.status).toBe(403);
    });

    it('admin user can access /api/admin/users', async () => {
      await login();
      const response = await api.get('/api/admin/users');

      expect(response.status).toBe(200);
    });
  });

  describe('OPDS authentication (Basic Auth)', () => {
    it('unauthenticated request to /opds returns 401 with WWW-Authenticate', async () => {
      const response = await fetch(`${BASE_URL}/opds`, {
        redirect: 'manual',
      });

      expect(response.status).toBe(401);
      expect(response.headers.get('www-authenticate')).toContain('Basic');
    });

    it('valid basic auth credentials allows OPDS access', async () => {
      const credentials = Buffer.from('testuser:testpass123').toString('base64');

      const response = await fetch(`${BASE_URL}/opds`, {
        headers: { 'Authorization': `Basic ${credentials}` },
      });

      expect(response.status).toBe(200);
      expect(response.headers.get('content-type')).toMatch(/application\/(atom\+)?xml/);
    });

    it('invalid basic auth credentials returns 401', async () => {
      const credentials = Buffer.from('testuser:wrongpassword').toString('base64');

      const response = await fetch(`${BASE_URL}/opds`, {
        headers: { 'Authorization': `Basic ${credentials}` },
        redirect: 'manual',
      });

      expect(response.status).toBe(401);
    });
  });

  describe('Session persistence', () => {
    it('session cookie persists across requests', async () => {
      await login();

      const response1 = await api.get('/api/library');
      const response2 = await api.get('/');

      expect(response1.status).toBe(200);
      expect(response2.status).toBe(200);
    });

    it('invalid session cookie is rejected', async () => {
      const response = await fetch(`${BASE_URL}/api/library`, {
        headers: { Cookie: 'id=invalid_session_token' },
        redirect: 'manual',
      });

      expect(response.status).toBe(401);
    });
  });
});
