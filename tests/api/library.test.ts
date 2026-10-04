import { beforeAll, describe, expect, it } from 'vitest';
import { api, login } from './client';
import { book, catalog, entryByVolume, entryNames, TITLE_NAMES, titleByName } from '../helpers/catalog';

describe('Catalog API', () => {
  beforeAll(async () => { await login(); });

  it('lists all fixture titles with their five ten-page volumes', async () => {
    const library = await catalog();
    expect(library.titles.map((title) => title.title)).toEqual(TITLE_NAMES);
    for (const title of library.titles) {
      expect(title.id).toEqual(expect.any(String));
      expect(title.entries.map((entry) => entry.title)).toEqual(entryNames(title.title as typeof TITLE_NAMES[number]));
      expect(title.entries.map((entry) => entry.pages)).toEqual([10, 10, 10, 10, 10]);
      expect(title.titles).toEqual([]);
    }
  });

  it('returns the requested title and its complete entries', async () => {
    const title = await titleByName('Test Manga Beta');
    const detail = await book(title.id);
    expect(detail.id).toBe(title.id);
    expect(detail.title).toBe('Test Manga Beta');
    expect(detail.entries.map((entry) => ({ title: entry.title, pages: entry.pages }))).toEqual(
      entryNames('Test Manga Beta').map((name) => ({ title: name, pages: 10 })),
    );
    expect(detail.titles).toEqual([]);
  });

  it('uses full-depth defaults for nonnumeric depth parameters', async () => {
    const library = await catalog();
    expect(await catalog('?depth=not-a-number')).toEqual(library);
    const title = await titleByName('Test Manga Alpha');
    expect(await book(title.id, '?depth=not-a-number')).toEqual(await book(title.id));
  });

  it('returns a plain-text 404 for a missing title', async () => {
    const response = await api.get('/api/book/nonexistent-id');
    expect(response.status).toBe(404);
    expect(response.headers.get('content-type')).toContain('text/plain');
  });

  it('returns the Mango download error for a missing entry', async () => {
    const response = await api.get('/api/download/nonexistent-title/nonexistent-entry');
    expect(response.status).toBe(404);
    expect(response.headers.get('content-type')).toContain('text/plain');
    expect(await response.text()).toBe('Nil assertion failed');
  });

  it('serves the first and final PNG pages but rejects pages outside the archive', async () => {
    const { title, entry } = await entryByVolume('Test Manga Charlie');
    for (const page of [1, 10]) {
      const response = await api.get(`/api/page/${title.id}/${entry.id}/${page}`);
      expect(response.status).toBe(200);
      expect(response.headers.get('content-type')).toBe('image/png');
      const bytes = new Uint8Array(await response.arrayBuffer());
      expect([...bytes.subarray(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
    }
    for (const page of [0, 11]) {
      const response = await api.get(`/api/page/${title.id}/${entry.id}/${page}`);
      expect(response.status).toBe(500);
    }
  });

  it('preserves the Mango numeric-path parse error', async () => {
    const response = await api.get('/api/page/nonexistent-title/nonexistent-entry/not-a-page');
    expect(response.status).toBe(500);
    expect(await response.text()).toBe('Invalid Int32: not-a-page');
  });
});
