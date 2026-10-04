import { beforeAll, describe, expect, it } from 'vitest';
import { api, BASE_URL, getSessionCookie, login } from './client';
import { titleByName } from '../helpers/catalog';

describe('Title tags API', () => {
  beforeAll(async () => { await login(); });

  it('returns the Mango JSON error for a missing title', async () => {
    const response = await api.get('/api/tags/nonexistent-title');
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ success: false, error: 'Nil assertion failed' });
  });

  it('adds and deletes exactly the requested tag with Mango success bodies', async () => {
    const title = await titleByName('Test Manga Foxtrot');
    const tag = 'tags-response-contract';
    const titlePath = `/api/tags/${encodeURIComponent(title.id)}`;
    const tagPath = `/api/admin/tags/${encodeURIComponent(title.id)}/${tag}`;
    const initialResponse = await api.get(titlePath);
    expect(initialResponse.status).toBe(200);
    const initial = await initialResponse.json();
    expect(initial.success).toBe(true);
    expect(initial.tags).not.toContain(tag);
    const remove = () => fetch(`${BASE_URL}${tagPath}`, { method: 'DELETE', headers: { Cookie: getSessionCookie()! } });
    try {
      const added = await api.put(tagPath);
      expect(added.status).toBe(200);
      expect(await added.json()).toEqual({ success: true, error: null });
      const tags = await api.get(titlePath);
      expect(tags.status).toBe(200);
      const tagged = await tags.json();
      expect(tagged.success).toBe(true);
      expect([...tagged.tags].sort()).toEqual([...initial.tags, tag].sort());
      const deleted = await remove();
      expect(deleted.status).toBe(200);
      expect(await deleted.json()).toEqual({ success: true, error: null });
      expect(await (await api.get(titlePath)).json()).toEqual(initial);
    } finally { await remove(); }
  });
});
