import { describe, it, expect, beforeAll } from 'vitest';
import { api, login } from './client';

describe('Progress API', () => {
  beforeAll(async () => {
    await login();
  });

  describe('PUT /api/progress/:tid/:page?eid=:entryId', () => {
    it('updates reading progress using Mango request shape', async () => {
      const libraryResponse = await api.get('/api/library');
      const library = await libraryResponse.json();

      if (library.titles.length === 0) {
        console.log('No titles in library, skipping progress test');
        return;
      }

      const titleId = library.titles[0].id;
      const titleResponse = await api.get(`/api/book/${titleId}`);
      const title = await titleResponse.json();

      if (!title.entries || title.entries.length === 0) {
        console.log('No entries in title, skipping progress test');
        return;
      }

      const entryId = title.entries[0].id;
      const response = await api.put(`/api/progress/${titleId}/1?eid=${entryId}`);
      expect(response.status).toBe(200);
      expect(await response.json()).toEqual({ success: true });
      await api.put(`/api/progress/${titleId}/0?eid=${entryId}`);
    });
  });

  describe('GET /api/progress', () => {
    it('returns user progress', async () => {
      const response = await api.get('/api/progress');

      expect(response.status).toBe(200);

      const progress = await response.json();
      expect(typeof progress).toBe('object');
    });
  });
});
