import { describe, expect, it } from 'vitest';
import { BASE_URL } from './client';

const origin = 'https://reader.example';
const methods = 'HEAD,GET,PUT,POST,DELETE,OPTIONS';
const headers = 'X-Requested-With,X-HTTP-Method-Override, Content-Type, Cache-Control, Accept,Authorization';

function expectMangoCors(response: Response): void {
  expect(response.headers.get('access-control-allow-origin')).toBe('*');
  expect(response.headers.get('access-control-allow-methods')).toBe(methods);
  expect(response.headers.get('access-control-allow-headers')).toBe(headers);
}

describe('Mango CORS and preflight contract', () => {
  it.each(['/api/library', '/api/no-such', '/uploads/no-such', '/img/no-such'])(
    'answers unauthenticated OPTIONS %s with an empty CORS response',
    async (path) => {
      const response = await fetch(`${BASE_URL}${path}`, {
        method: 'OPTIONS',
        headers: {
          Origin: origin,
          'Access-Control-Request-Method': 'PUT',
          'Access-Control-Request-Headers': 'authorization,content-type',
        },
      });
      expect(response.status).toBe(200);
      expectMangoCors(response);
      expect(await response.text()).toBe('');
    },
  );

  it('applies preflight to path roots without requiring an Origin header', async () => {
    for (const path of ['/api', '/uploads', '/img']) {
      const response = await fetch(`${BASE_URL}${path}`, { method: 'OPTIONS' });
      expect(response.status).toBe(200);
      expectMangoCors(response);
      expect(await response.text()).toBe('');
    }
  });

  it('adds CORS headers to API responses even without Origin or authentication', async () => {
    const unauthorized = await fetch(`${BASE_URL}/api/library`);
    expect(unauthorized.status).toBe(401);
    expectMangoCors(unauthorized);

    const failedLogin = await fetch(`${BASE_URL}/api/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{}',
    });
    expect(failedLogin.status).toBe(403);
    expectMangoCors(failedLogin);
  });

  it('does not apply CORS to ordinary pages, static assets, or non-matching preflights', async () => {
    const ordinary = await fetch(`${BASE_URL}/login`);
    const staticAsset = await fetch(`${BASE_URL}/static/favicon.ico`);
    const unrelatedPreflight = await fetch(`${BASE_URL}/login`, { method: 'OPTIONS' });
    const nearPrefix = await fetch(`${BASE_URL}/apiary`, { method: 'OPTIONS' });
    expect(staticAsset.status).toBe(200);
    expect(nearPrefix.status).not.toBe(200);
    for (const response of [ordinary, staticAsset, unrelatedPreflight, nearPrefix]) {
      expect(response.headers.has('access-control-allow-origin')).toBe(false);
    }
  });
});
