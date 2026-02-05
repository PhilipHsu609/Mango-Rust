import { describe, it, expect, beforeAll } from 'vitest';
import { api, login } from './client';

describe('Progress API', () => {
  beforeAll(async () => {
    await login();
  });

  describe('POST /api/progress/:tid/:eid', () => {
    it('updates reading progress', async () => {
      // Get a title with entries first
      const libraryResponse = await api.get('/api/library');
      const library = await libraryResponse.json();

      if (library.length === 0) {
        console.log('No titles in library, skipping progress test');
        return;
      }

      const titleId = library[0].id;
      const titleResponse = await api.get(`/api/title/${titleId}`);
      const title = await titleResponse.json();

      if (!title.entries || title.entries.length === 0) {
        console.log('No entries in title, skipping progress test');
        return;
      }

      const entryId = title.entries[0].id;

      // Update progress via POST
      const response = await api.post(`/api/progress/${titleId}/${entryId}`, { page: 5 });

      expect([200, 204]).toContain(response.status);
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
