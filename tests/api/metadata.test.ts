import { copyFile, mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { beforeAll, describe, expect, inject, it } from 'vitest';
import { api, login } from './client';
import { book, catalog, CatalogTitle, entryByVolume, TITLE_NAMES, titleByName } from '../helpers/catalog';
import { startServer, type TestServer } from '../helpers/server';

describe('Title metadata API', () => {
  beforeAll(async () => { await login(); });

  it.each([
    '/api/admin/display_name/nonexistent-title/name',
    '/api/admin/sort_title/nonexistent-title?name=ignored',
  ])('returns the Mango JSON error for %s', async (path) => {
    const response = await api.put(path);
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ success: false, error: 'Nil assertion failed' });
  });

  it('persists decoded display names for API and HTML readers', async () => {
    const title = await titleByName('Test Manga Echo');
    const displayName = 'Contract Display & Volume';
    try {
      const response = await api.put(`/api/admin/display_name/${encodeURIComponent(title.id)}/${encodeURIComponent(displayName)}`);
      expect(response.status).toBe(200);
      expect(await response.json()).toEqual({ success: true });
      expect((await book(title.id, '?depth=0')).display_name).toBe(displayName);
      const page = await api.get(`/book/${encodeURIComponent(title.id)}`);
      expect(page.status).toBe(200);
      expect(await page.text()).toContain('Contract Display &amp; Volume');
    } finally {
      await api.put(`/api/admin/display_name/${encodeURIComponent(title.id)}/${encodeURIComponent(title.display_name)}`);
    }
  });

  it('persists sort-title overrides and changes the complete library order', async () => {
    const title = await titleByName('Test Manga Golf');
    try {
      const response = await api.put(`/api/admin/sort_title/${encodeURIComponent(title.id)}?name=Aardvark`);
      expect(response.status).toBe(200);
      expect(await response.json()).toEqual({ success: true });
      expect((await book(title.id, '?depth=0')).sort_title).toBe('Aardvark');
      expect((await catalog('?depth=0')).titles.map((item) => item.title)).toEqual([
        'Test Manga Golf', ...TITLE_NAMES.filter((name) => name !== 'Test Manga Golf'),
      ]);
    } finally {
      await api.put(`/api/admin/sort_title/${encodeURIComponent(title.id)}`);
    }
  });

  it('does not apply an entry sort-title override through the wrong title', async () => {
    const requestedTitle = await titleByName('Test Manga Alpha');
    const { title: owner, entry } = await entryByVolume('Test Manga Beta');
    try {
      const response = await api.put(`/api/admin/sort_title/${encodeURIComponent(requestedTitle.id)}?eid=${encodeURIComponent(entry.id)}&name=Must%20Not%20Be%20Applied`);
      expect(response.status).toBe(200);
      expect(await response.json()).toEqual({ success: true });
      expect((await book(owner.id)).entries.find((candidate) => candidate.id === entry.id)?.sort_title).toBe(entry.sort_title);
    } finally {
      await api.put(`/api/admin/sort_title/${encodeURIComponent(owner.id)}?eid=${encodeURIComponent(entry.id)}`);
    }
  });

  it('persists and serves title and entry cover uploads and exposes the entry cover in OPDS', async () => {
    const libraryPath = await mkdtemp(join(tmpdir(), 'mango-cover-test-'));
    let server: TestServer | undefined;
    try {
      const titleName = 'Test Manga Alpha';
      await mkdir(join(libraryPath, titleName));
      const archive = `${titleName} Vol.01.zip`;
      await copyFile(join(inject('libraryPath'), titleName, archive), join(libraryPath, titleName, archive));
      server = await startServer({ libraryPath });
      const loginResponse = await fetch(`${server.url}/api/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: 'testuser', password: 'testpass123' }),
      });
      expect(loginResponse.status).toBe(200);
      const cookie = loginResponse.headers.get('set-cookie')!.split(';')[0];
      const libraryResponse = await fetch(`${server.url}/api/library`, { headers: { Cookie: cookie } });
      expect(libraryResponse.status).toBe(200);
      const library: { titles: CatalogTitle[] } = await libraryResponse.json();
      const title = library.titles.find((candidate) => candidate.title === titleName)!;
      const entry = title.entries.find((candidate) => candidate.title === `${titleName} Vol.01`)!;
      const images = [
        '<svg xmlns="http://www.w3.org/2000/svg"><title>title cover</title></svg>',
        '<svg xmlns="http://www.w3.org/2000/svg"><title>entry cover</title></svg>',
      ];
      for (const [index, image] of images.entries()) {
        const form = new FormData();
        form.append('file', new Blob([image], { type: 'image/svg+xml' }), 'cover.svg');
        const entryQuery = index === 1 ? `&eid=${encodeURIComponent(entry.id)}` : '';
        const upload = await fetch(`${server.url}/api/admin/upload/cover?tid=${encodeURIComponent(title.id)}${entryQuery}`, {
          method: 'POST', headers: { Cookie: cookie }, body: form,
        });
        expect(upload.status).toBe(200);
        expect(await upload.json()).toEqual({ success: true });
        const detailResponse = await fetch(`${server.url}/api/book/${encodeURIComponent(title.id)}`, { headers: { Cookie: cookie } });
        expect(detailResponse.status).toBe(200);
        const detail: CatalogTitle = await detailResponse.json();
        const coverUrl = index === 0 ? detail.cover_url : detail.entries.find((candidate) => candidate.id === entry.id)!.cover_url;
        const imageResponse = await fetch(new URL(coverUrl, server.url));
        expect(imageResponse.status).toBe(200);
        expect(await imageResponse.text()).toBe(image);
        if (index === 1) {
          const feed = await fetch(`${server.url}/opds/book/${encodeURIComponent(title.id)}`, { headers: { Cookie: cookie } });
          expect(feed.status).toBe(200);
          expect(await feed.text()).toContain(coverUrl);
        }
      }
    } finally {
      try { await server?.close(); } finally { await rm(libraryPath, { recursive: true, force: true }); }
    }
  });
});
