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
      expect(response.headers.get('set-cookie')).toContain('id=');
      expect(response.headers.get('location')).toBe('/');
    });

    it('invalid credentials returns error and sets no session', async () => {
      const response = await fetch(`${BASE_URL}/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({ username: 'testuser', password: 'wrongpassword' }),
        redirect: 'manual',
      });

      // Server returns 200 with error message in HTML
      expect(response.status).toBe(200);
      const body = await response.text();
      expect(body).toContain('Invalid username or password');
      // Critical: no session cookie should be set on failed login
      expect(response.headers.get('set-cookie')).toBeNull();
    });

    it('missing credentials returns error and sets no session', async () => {
      const response = await fetch(`${BASE_URL}/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({}),
        redirect: 'manual',
      });

      // Should return error (200 with HTML error or 422 validation error)
      expect([200, 422]).toContain(response.status);
      // Critical: no session cookie should be set
      expect(response.headers.get('set-cookie')).toBeNull();
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
      expect(verifyResponse.status).toBe(303);
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
    it('unauthenticated request to /api/library redirects to login', async () => {
      const response = await fetch(`${BASE_URL}/api/library`, {
        redirect: 'manual',
      });

      expect(response.status).toBe(303);
      expect(response.headers.get('location')).toBe('/login');
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

      expect(response.status).toBe(303);
      expect(response.headers.get('location')).toBe('/login');
    });
  });
});
