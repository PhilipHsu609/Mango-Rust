import { describe, it, expect, beforeAll } from 'vitest';
import { api, login } from './client';

describe('Library API', () => {
  beforeAll(async () => {
    await login();
  });

  describe('GET /api/library', () => {
    it('returns Mango library object', async () => {
      const response = await api.get('/api/library');
      expect(response.status).toBe(200);

      const data = await response.json();
      expect(data).toHaveProperty('dir');
      expect(data).toHaveProperty('titles');
      expect(Array.isArray(data.titles)).toBe(true);
    });

    it('title objects have Mango fields', async () => {
      const response = await api.get('/api/library');
      const data = await response.json();

      if (data.titles.length > 0) {
        const title = data.titles[0];
        expect(title).toHaveProperty('id');
        expect(title).toHaveProperty('title');
        expect(title).toHaveProperty('entries');
        expect(Array.isArray(title.entries)).toBe(true);
      }
    });
  });

  describe('GET /api/book/:tid', () => {
    it('returns title details with Mango fields', async () => {
      const libraryResponse = await api.get('/api/library');
      const library = await libraryResponse.json();

      if (library.titles.length > 0) {
        const titleId = library.titles[0].id;
        const response = await api.get(`/api/book/${titleId}`);

        expect(response.status).toBe(200);

        const title = await response.json();
        expect(title).toHaveProperty('id');
        expect(title).toHaveProperty('title');
        expect(title).toHaveProperty('entries');
        expect(Array.isArray(title.entries)).toBe(true);
      }
    });

    it('returns 404 for invalid title ID', async () => {
      const response = await api.get('/api/book/nonexistent-id');
      expect(response.status).toBe(404);
    });
  });


  describe('Mango homepage API', () => {
    it('returns wrapped Continue Reading entries and percentages', async () => {
      const response = await api.get('/api/library/continue_reading');
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data.success).toBe(true);
      expect(Array.isArray(data.entries)).toBe(true);
      expect(Array.isArray(data.entry_percentages)).toBe(true);
    });

    it('returns wrapped Recently Added items', async () => {
      const response = await api.get('/api/library/recently_added');
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data.success).toBe(true);
      expect(Array.isArray(data.items)).toBe(true);
      if (data.items.length > 0) {
        expect(data.items[0]).toHaveProperty('item');
        expect(data.items[0]).toHaveProperty('percentage');
        expect(data.items[0]).toHaveProperty('count');
      }
    });

    it('returns wrapped Start Reading titles', async () => {
      const response = await api.get('/api/library/start_reading');
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data.success).toBe(true);
      expect(Array.isArray(data.titles)).toBe(true);
    });
  });

});
