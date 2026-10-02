import { beforeEach, describe, expect, it } from 'vitest';
import { api, login, logout, BASE_URL } from './client';

describe('API reference', () => {
  beforeEach(() => logout());

  it('retains Mango authentication behavior for the reference surfaces', async () => {
    const page = await fetch(`${BASE_URL}/api`, { redirect: 'manual' });
    expect(page.status).toBe(401);
    expect(await page.text()).toBe('Unauthorized');

    const spec = await fetch(`${BASE_URL}/openapi.json`, { redirect: 'manual' });
    expect(spec.status).toBe(303);
    expect(spec.headers.get('location')).toBe('/login');
  });

  it('renders ReDoc and serves generated operations for the Rust API', async () => {
    await login();

    const page = await api.get('/api');
    expect(page.status).toBe(200);
    expect(page.headers.get('content-type')).toContain('text/html');
    expect(await page.text()).toContain('<redoc spec-url="/openapi.json"></redoc>');

    const response = await api.get('/openapi.json');
    expect(response.status).toBe(200);
    expect(response.headers.get('content-type')).toContain('application/json');

    const spec = await response.json();
    expect(spec.info.title).toBe('Mango API');
    expect(spec.info.description).toContain('A Word of Caution');
    expect(spec.info.description).toContain('mango-sessid-{port}');
    expect(spec.paths['/api/admin/titles/missing'].get).toBeDefined();
    expect(spec.paths['/api/admin/titles/missing'].delete).toBeDefined();
    expect(spec.paths['/api/admin/titles/missing/{id}'].delete).toBeDefined();
    expect(spec.paths['/api/admin/mangadex/queue']).toBeUndefined();
    expect(spec.paths['/api/login'].post).toBeDefined();
    expect(spec.paths['/api/library'].get).toBeDefined();
    expect(spec.paths['/api/admin/users'].post).toBeDefined();
    expect(spec.paths['/api/login'].post.requestBody).toBeDefined();
    expect(spec.paths['/api/admin/users'].post.requestBody).toBeDefined();
    expect(spec.components.schemas.LoginForm.properties).toHaveProperty('username');
    expect(spec.components.schemas.CreateUserRequest.properties).toHaveProperty('is_admin');
    expect(spec.paths['/api/cache/clear'].post).toBeDefined();
  });
});
