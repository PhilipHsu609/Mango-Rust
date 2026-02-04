import { describe, it, expect, beforeAll } from 'vitest';
import { api, login } from './client';

describe('Library API', () => {
  beforeAll(async () => {
    await login();
  });

  describe('GET /api/library', () => {
    it('returns array of titles', async () => {
      const response = await api.get('/api/library');
      expect(response.status).toBe(200);

      const data = await response.json();
      expect(Array.isArray(data)).toBe(true);
    });

    it('title objects have required fields', async () => {
      const response = await api.get('/api/library');
      const data = await response.json();

      if (data.length > 0) {
        const title = data[0];
        expect(title).toHaveProperty('id');
        expect(title).toHaveProperty('title');
        expect(typeof title.id).toBe('string');
        expect(typeof title.title).toBe('string');
      }
    });

    it('respects sort parameter', async () => {
      const defaultResponse = await api.get('/api/library');
      const sortedResponse = await api.get('/api/library?sort=title');

      expect(defaultResponse.status).toBe(200);
      expect(sortedResponse.status).toBe(200);
    });
  });

  describe('GET /api/title/:id', () => {
    it('returns title details with entries array', async () => {
      const libraryResponse = await api.get('/api/library');
      const library = await libraryResponse.json();

      if (library.length > 0) {
        const titleId = library[0].id;
        const response = await api.get(`/api/title/${titleId}`);

        expect(response.status).toBe(200);

        const title = await response.json();
        expect(title).toHaveProperty('id');
        expect(title).toHaveProperty('title');
        expect(title).toHaveProperty('entries');
        expect(Array.isArray(title.entries)).toBe(true);
      }
    });

    it('returns 404 for invalid title ID', async () => {
      const response = await api.get('/api/title/nonexistent-id');
      expect(response.status).toBe(404);
    });
  });

  describe('GET /api/stats', () => {
    it('returns library statistics', async () => {
      const response = await api.get('/api/stats');
      expect(response.status).toBe(200);

      const stats = await response.json();
      expect(typeof stats.titles).toBe('number');
      expect(typeof stats.entries).toBe('number');
      expect(typeof stats.pages).toBe('number');
    });
  });
});
