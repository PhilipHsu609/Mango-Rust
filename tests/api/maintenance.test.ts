import { beforeAll, describe, expect, it } from 'vitest';
import { api, login } from './client';
import { catalog, TITLE_NAMES } from '../helpers/catalog';

describe('Library maintenance API', () => {
  beforeAll(async () => { await login(); });

  it('rescans the complete fixture library and reports elapsed milliseconds', async () => {
    const response = await api.post('/api/admin/scan');
    expect(response.status).toBe(200);
    const result = await response.json();
    expect(result.titles).toBe(7);
    expect(result.milliseconds).toEqual(expect.any(Number));
    expect(Number.isFinite(result.milliseconds)).toBe(true);
    expect(result.milliseconds).toBeGreaterThanOrEqual(0);
    expect((await catalog()).titles.map((title) => title.title)).toEqual(TITLE_NAMES);
  });
});
