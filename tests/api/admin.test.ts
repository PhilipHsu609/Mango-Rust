import { describe, it, expect, beforeAll } from 'vitest';
import { api, login } from './client';

describe('Admin API', () => {
  beforeAll(async () => {
    await login(); // testuser is admin
  });

  describe('POST /api/admin/scan', () => {
    it('triggers library scan and returns results', async () => {
      const response = await api.post('/api/admin/scan');

      expect(response.status).toBe(200);

      const result = await response.json();
      expect(result).toHaveProperty('titles');
      expect(result).toHaveProperty('entries');
      expect(typeof result.titles).toBe('number');
      expect(typeof result.entries).toBe('number');
    });
  });

  describe('GET /api/admin/users', () => {
    it('returns list of users', async () => {
      const response = await api.get('/api/admin/users');

      expect(response.status).toBe(200);

      const users = await response.json();
      expect(Array.isArray(users)).toBe(true);

      if (users.length > 0) {
        expect(users[0]).toHaveProperty('username');
        expect(users[0]).toHaveProperty('admin');
      }
    });
  });
});
