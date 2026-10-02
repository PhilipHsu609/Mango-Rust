import { describe, it, expect } from 'vitest';
import { BASE_URL } from './client';

const AUTH_HEADER = 'Basic ' + Buffer.from('testuser:testpass123').toString('base64');

describe('OPDS API', () => {
  it('returns a populated Atom feed with navigable title entries', async () => {
    const response = await fetch(`${BASE_URL}/opds`, {
      headers: { Authorization: AUTH_HEADER },
    });

    expect(response.status).toBe(200);
    expect(response.headers.get('content-type')).toMatch(/application\/(atom\+)?xml/);

    const xml = await response.text();
    expect(xml).toContain('<?xml');
    expect(xml).toContain('<feed');
    expect(xml).toContain('<title>Test Manga Beta</title>');
    expect(xml).toContain('rel="subsection"');
    expect(xml).toContain('/opds/book/');
    expect(xml.match(/<entry>/g)?.length).toBeGreaterThanOrEqual(7);
  });
});
