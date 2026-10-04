import { beforeEach, describe, expect, it } from 'vitest';
import { api, login, logout, BASE_URL } from './client';

describe('API reference authorization', () => {
  beforeEach(() => logout());

  it('retains distinct Mango authentication responses for the reference surfaces', async () => {
    const page = await fetch(`${BASE_URL}/api`, { redirect: 'manual' });
    expect(page.status).toBe(401);
    expect(await page.text()).toBe('Unauthorized');
    const spec = await fetch(`${BASE_URL}/openapi.json`, { redirect: 'manual' });
    expect(spec.status).toBe(303);
    expect(spec.headers.get('location')).toBe('/login');
  });

  it('allows an authenticated reader to access both reference surfaces', async () => {
    await login('testuser2', 'testpass123');
    const page = await api.get('/api');
    expect(page.status).toBe(200);
    expect(page.headers.get('content-type')).toContain('text/html');
    const spec = await api.get('/openapi.json');
    expect(spec.status).toBe(200);
    expect(spec.headers.get('content-type')).toContain('application/json');
  });
});
