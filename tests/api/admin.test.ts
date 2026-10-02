import { describe, it, expect, beforeAll } from 'vitest';
import { api, login, BASE_URL, getSessionCookie } from './client';

describe('Admin API', () => {
  beforeAll(async () => {
    await login(); // testuser is admin
  });

  describe('POST /api/admin/scan', () => {
    it('triggers library scan and returns results', async () => {
      const response = await api.post('/api/admin/scan');

      expect(response.status).toBe(200);

      const result = await response.json();
      expect(result.titles).toBeGreaterThan(0);
      const libraryResponse = await api.get('/api/library');
      const library = await libraryResponse.json();
      expect(result.titles).toBe(library.titles.length);
      expect(typeof result.milliseconds).toBe('number');
    });
  });

  describe('GET /api/admin/users', () => {
    it('returns the seeded users with their admin roles', async () => {
      const response = await api.get('/api/admin/users');
      expect(response.status).toBe(200);
      const users = await response.json();
      expect(users).toEqual(expect.arrayContaining([
        { username: 'testuser', is_admin: true },
        { username: 'testuser2', is_admin: false },
      ]));
    });
  });
  describe('POST /api/admin/upload/cover', () => {
    it('persists title and entry cover URLs, serves uploads, and exposes entry covers in OPDS', async () => {
      const libraryResponse = await api.get('/api/library');
      const library = await libraryResponse.json();
      const title = library.titles[0];
      expect(title).toBeDefined();

      const image = '<svg xmlns="http://www.w3.org/2000/svg"></svg>';
      const form = new FormData();
      form.append('file', new Blob([image], { type: 'image/svg+xml' }), 'contract-cover.svg');
      const uploadResponse = await fetch(
        `${BASE_URL}/api/admin/upload/cover?tid=${encodeURIComponent(title.id)}`,
        {
          method: 'POST',
          headers: { Cookie: getSessionCookie()! },
          body: form,
        },
      );
      expect(uploadResponse.status).toBe(200);
      expect(await uploadResponse.json()).toEqual({ success: true });

      const detailResponse = await api.get(`/api/book/${encodeURIComponent(title.id)}?depth=0`);
      const detail = await detailResponse.json();
      const coverUrl = detail.cover_url as string;
      expect(coverUrl).toContain('/uploads/img/');

      const imageResponse = await fetch(new URL(coverUrl, BASE_URL));
      expect(imageResponse.status).toBe(200);
      expect(await imageResponse.text()).toBe(image);
      const entry = title.entries[0];
      expect(entry).toBeDefined();
      const entryImage = '<svg xmlns="http://www.w3.org/2000/svg"><title>entry</title></svg>';
      const entryForm = new FormData();
      entryForm.append(
        'file',
        new Blob([entryImage], { type: 'image/svg+xml' }),
        'entry-cover.svg',
      );
      const entryUploadResponse = await fetch(
        `${BASE_URL}/api/admin/upload/cover?tid=${encodeURIComponent(title.id)}&eid=${encodeURIComponent(entry.id)}`,
        {
          method: 'POST',
          headers: { Cookie: getSessionCookie()! },
          body: entryForm,
        },
      );
      expect(entryUploadResponse.status).toBe(200);
      expect(await entryUploadResponse.json()).toEqual({ success: true });

      const detailAfterEntryUpload = await api.get(`/api/book/${encodeURIComponent(title.id)}`);
      const updatedTitle = await detailAfterEntryUpload.json();
      const updatedEntry = updatedTitle.entries.find(
        (candidate: { id: string }) => candidate.id === entry.id,
      );
      expect(updatedEntry.cover_url).toContain('/uploads/img/');

      const opdsResponse = await api.get(`/opds/book/${encodeURIComponent(title.id)}`);
      expect(await opdsResponse.text()).toContain(updatedEntry.cover_url);
    });
  });
  describe('PUT /api/admin/display_name', () => {
    it('persists decoded title display names for API and HTML readers', async () => {
      const libraryResponse = await api.get('/api/library');
      const library = await libraryResponse.json();
      const title = library.titles[0];
      const displayName = 'Contract Display & Volume';
      const updateResponse = await api.put(
        `/api/admin/display_name/${encodeURIComponent(title.id)}/${encodeURIComponent(displayName)}`,
      );
      expect(updateResponse.status).toBe(200);
      expect(await updateResponse.json()).toEqual({ success: true });

      const detailResponse = await api.get(`/api/book/${encodeURIComponent(title.id)}?depth=0`);
      expect(detailResponse.status).toBe(200);
      const detail = await detailResponse.json();
      expect(detail.display_name).toBe(displayName);

      const pageResponse = await fetch(`${BASE_URL}/book/${encodeURIComponent(title.id)}`, {
        headers: { Cookie: getSessionCookie()! },
      });
      expect(await pageResponse.text()).toContain('Contract Display &amp; Volume');
      expect(pageResponse.status).toBe(200);
    });
  });

  describe('PUT /api/admin/sort_title', () => {
    it('persists sort-title overrides and applies them to library order', async () => {
      const libraryResponse = await api.get('/api/library');
      const library = await libraryResponse.json();
      const title = library.titles.find((item: { title: string }) => item.title === 'Test Manga Golf');
      expect(title).toBeDefined();
      const sortTitle = 'Aardvark Parity';
      const updateResponse = await api.put(
        `/api/admin/sort_title/${encodeURIComponent(title.id)}?name=${encodeURIComponent(sortTitle)}`,
      );
      expect(updateResponse.status).toBe(200);
      expect(await updateResponse.json()).toEqual({ success: true });

      const sortedResponse = await api.get('/api/library?depth=0');
      const sorted = await sortedResponse.json();
      expect(sorted.titles[0].id).toBe(title.id);
      expect(sorted.titles[0].sort_title).toBe(sortTitle);
    });
  });
});
