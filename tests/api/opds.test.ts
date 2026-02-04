import { describe, it, expect } from 'vitest';
import { BASE_URL } from './client';

const AUTH_HEADER = 'Basic ' + Buffer.from('testuser:testpass123').toString('base64');

describe('OPDS API', () => {
  describe('GET /opds', () => {
    it('returns valid Atom XML feed', async () => {
      const response = await fetch(`${BASE_URL}/opds`, {
        headers: { Authorization: AUTH_HEADER },
      });

      expect(response.status).toBe(200);
      expect(response.headers.get('content-type')).toMatch(/application\/(atom\+)?xml/);

      const xml = await response.text();
      expect(xml).toContain('<?xml');
      expect(xml).toContain('<feed');
    });

    it('requires authentication', async () => {
      const response = await fetch(`${BASE_URL}/opds`);

      expect(response.status).toBe(401);
      expect(response.headers.get('www-authenticate')).toContain('Basic');
    });
  });

  describe('GET /opds/all', () => {
    it('returns library feed', async () => {
      const response = await fetch(`${BASE_URL}/opds/all`, {
        headers: { Authorization: AUTH_HEADER },
      });

      expect(response.status).toBe(200);
      expect(response.headers.get('content-type')).toMatch(/application\/(atom\+)?xml/);

      const xml = await response.text();
      expect(xml).toContain('<feed');
    });
  });
});
