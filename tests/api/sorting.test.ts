import { beforeAll, describe, expect, it } from 'vitest';
import { api, BASE_URL, getSessionCookie, login } from './client';
import { book, catalog, entryNames, TITLE_NAMES, titleByName } from '../helpers/catalog';

describe('Catalog sorting API', () => {
  beforeAll(async () => { await login(); });

  it('persists independent title and library orders', async () => {
    const title = await titleByName('Test Manga Delta');
    const titleId = encodeURIComponent(title.id);
    const original = await (await api.get(`/api/sort_opt?tid=${titleId}`)).json();
    const originalLibrary = await (await api.get('/api/sort_opt')).json();
    try {
      const update = await api.put('/api/sort_opt', { tid: title.id, sort: 'title', ascend: false });
      expect(update.status).toBe(200);
      expect(await update.json()).toEqual({ success: true });
      expect(await (await api.get(`/api/sort_opt?tid=${titleId}`)).json()).toEqual({ method: 'title', ascend: false });
      expect((await book(title.id)).entries.map((entry) => entry.title)).toEqual(entryNames('Test Manga Delta').reverse());

      for (const ascend of [false, true]) {
        const updateLibrary = await api.put('/api/sort_opt', { sort: 'title', ascend });
        expect(updateLibrary.status).toBe(200);
        expect(await updateLibrary.json()).toEqual({ success: true });
        const library = await catalog();
        expect(library.titles.map((item) => item.title)).toEqual(ascend ? TITLE_NAMES : [...TITLE_NAMES].reverse());
        const embedded = library.titles.find((item) => item.id === title.id)!;
        expect(embedded.entries.map((entry) => entry.title)).toEqual(
          ascend ? entryNames('Test Manga Delta') : entryNames('Test Manga Delta').reverse(),
        );
        expect((await book(title.id)).entries.map((entry) => entry.title)).toEqual(entryNames('Test Manga Delta').reverse());
      }
    } finally {
      await api.put('/api/sort_opt', { tid: title.id, sort: original.method, ascend: original.ascend });
      await api.put('/api/sort_opt', { sort: originalLibrary.method, ascend: originalLibrary.ascend });
    }
  });

  it('returns Mango JSON errors for missing sort targets', async () => {
    const read = await api.get('/api/sort_opt?tid=nonexistent-title');
    const update = await api.put('/api/sort_opt', { tid: 'nonexistent-title', sort: 'name', ascend: true });
    for (const response of [read, update]) {
      expect(response.status).toBe(200);
      expect(await response.json()).toEqual({ success: false, error: 'Nil assertion failed' });
    }
  });

  it('rejects malformed sort JSON without changing the selected order', async () => {
    const before = await (await api.get('/api/sort_opt')).json();
    const response = await fetch(`${BASE_URL}/api/sort_opt`, {
      method: 'PUT',
      headers: { Cookie: getSessionCookie()!, 'Content-Type': 'application/json' },
      body: '{',
    });
    expect(response.status).toBe(200);
    expect(await response.json()).toMatchObject({ success: false, error: expect.any(String) });
    expect(await (await api.get('/api/sort_opt')).json()).toEqual(before);
  });
});
