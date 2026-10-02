import { describe, it, expect, beforeAll } from 'vitest';
import { api, login } from './client';

describe('Library API', () => {
  beforeAll(async () => {
    await login();
  });

  describe('GET /api/library', () => {
    it('returns the library and its populated title contract', async () => {
      const response = await api.get('/api/library');
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data).toMatchObject({
        dir: expect.any(String),
        titles: expect.any(Array),
      });
      expect(data.titles.length).toBeGreaterThan(0);

      const title = data.titles.find((item: { entries?: unknown[] }) => item.entries?.length);
      expect(title).toMatchObject({
        id: expect.any(String),
        title: expect.any(String),
      });
      expect(Array.isArray(title.entries)).toBe(true);
      expect(title.entries.length).toBeGreaterThan(0);
    });
  });

  describe('GET /api/book/:tid', () => {
    it('returns title details with entries for a populated title', async () => {
      const libraryResponse = await api.get('/api/library');
      const library = await libraryResponse.json();
      const title = library.titles.find(
        (item: { entries?: { length: number }[] }) => item.entries?.length,
      );
      expect(title).toBeDefined();
      if (!title) throw new Error('The test library must contain a title with entries');

      const response = await api.get(`/api/book/${title.id}`);
      expect(response.status).toBe(200);
      const detail = await response.json();
      expect(detail).toMatchObject({
        id: title.id,
        title: expect.any(String),
      });
      expect(Array.isArray(detail.entries)).toBe(true);
      expect(detail.entries.length).toBeGreaterThan(0);
    });

    it('returns 404 for invalid title ID', async () => {
      const response = await api.get('/api/book/nonexistent-id');
      expect(response.status).toBe(404);
    });
  });


  it('rejects page zero instead of serving the first page', async () => {
    const libraryResponse = await api.get('/api/library');
    const library = await libraryResponse.json();
    const pending = [...library.titles];
    let pageLocation: { titleId: string; entryId: string } | undefined;
    while (pending.length > 0 && !pageLocation) {
      const title = pending.shift();
      if (title.entries?.length > 0) {
        pageLocation = { titleId: title.id, entryId: title.entries[0].id };
      }
      pending.push(...(title.titles ?? []));
    }
    expect(pageLocation).toBeDefined();
    if (!pageLocation) throw new Error('The test library must contain at least one entry');

    const response = await api.get(
      `/api/page/${pageLocation.titleId}/${pageLocation.entryId}/0`,
    );
    expect(response.status).toBe(500);
  });

  describe('Mango homepage API', () => {
    it('returns wrapped Continue Reading entries and percentages', async () => {
      const response = await api.get('/api/library/continue_reading');
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data.success).toBe(true);
      expect(Array.isArray(data.entry_percentages)).toBe(true);
      expect(data.entries.length).toBe(data.entry_percentages.length);
    });

    it('returns populated Recently Added groups', async () => {
      const response = await api.get('/api/library/recently_added');
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data.success).toBe(true);
      expect(data.items.length).toBeGreaterThan(0);
      expect(data.items[0]).toMatchObject({
        item: expect.any(Object),
        percentage: expect.any(Number),
        count: expect.any(Number),
      });
    });

    it('returns unread root titles for Start Reading', async () => {
      const response = await api.get('/api/library/start_reading');
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data.success).toBe(true);
      expect(data.titles.length).toBeGreaterThan(0);
      expect(data.titles[0]).toMatchObject({
        id: expect.any(String),
        title: expect.any(String),
      });
    });
  });

});
