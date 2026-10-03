import { describe, it, expect, beforeAll } from 'vitest';
import { api, login, BASE_URL, getSessionCookie } from './client';

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

  it('uses Mango full-depth defaults for nonnumeric depth parameters', async () => {
    const defaultLibraryResponse = await api.get('/api/library');
    expect(defaultLibraryResponse.status).toBe(200);
    const defaultLibrary = await defaultLibraryResponse.json();

    const invalidDepthResponse = await api.get('/api/library?depth=not-a-number');
    expect(invalidDepthResponse.status).toBe(200);
    expect(await invalidDepthResponse.json()).toEqual(defaultLibrary);

    const title = defaultLibrary.titles[0];
    const defaultBookResponse = await api.get(`/api/book/${encodeURIComponent(title.id)}`);
    expect(defaultBookResponse.status).toBe(200);
    const defaultBook = await defaultBookResponse.json();

    const invalidBookDepthResponse = await api.get(
      `/api/book/${encodeURIComponent(title.id)}?depth=not-a-number`,
    );
    expect(invalidBookDepthResponse.status).toBe(200);
    expect(await invalidBookDepthResponse.json()).toEqual(defaultBook);
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

    it('returns 404 for invalid title ID without changing the API body format', async () => {
      const response = await api.get('/api/book/nonexistent-id');
      expect(response.status).toBe(404);
      expect(response.headers.get('content-type')).toContain('text/plain');
      expect(await response.text()).toContain('not found');
    });
  });
  it('applies sort option changes to the next book response', async () => {
    const libraryResponse = await api.get('/api/library');
    const library = await libraryResponse.json();
    const title = library.titles.find(
      (item: { entries?: unknown[] }) => item.entries?.length,
    );
    expect(title).toBeDefined();
    if (!title) throw new Error('The test library must contain a title with entries');

    const titleId = encodeURIComponent(title.id);
    const originalResponse = await api.get(`/api/sort_opt?tid=${titleId}`);
    const original = await originalResponse.json();
    const originalLibraryResponse = await api.get('/api/sort_opt');
    const originalLibrarySort = await originalLibraryResponse.json();
    try {
      const updateResponse = await api.put('/api/sort_opt', {
        tid: title.id,
        sort: 'title',
        ascend: false,
      });
      expect(await updateResponse.json()).toEqual({ success: true });

      const bookResponse = await api.get(`/api/book/${titleId}`);
      const book = await bookResponse.json();
      expect(book.entries[0].title).toContain('Vol.05');

      const librarySortResponse = await api.put('/api/sort_opt', {
        sort: 'title',
        ascend: true,
      });
      expect(await librarySortResponse.json()).toEqual({ success: true });

      const sortedLibraryResponse = await api.get('/api/library');
      const sortedLibrary = await sortedLibraryResponse.json();
      const embeddedTitle = sortedLibrary.titles.find((item: { id: string }) => item.id === title.id);
      expect(embeddedTitle.entries[0].title).toContain('Vol.01');

      const bookAfterGlobalResponse = await api.get(`/api/book/${titleId}`);
      const bookAfterGlobal = await bookAfterGlobalResponse.json();
      expect(bookAfterGlobal.entries[0].title).toContain('Vol.05');
    } finally {
      await api.put('/api/sort_opt', {
        tid: title.id,
        sort: original.method,
        ascend: original.ascend,
      });
      await api.put('/api/sort_opt', {
        sort: originalLibrarySort.method,
        ascend: originalLibrarySort.ascend,
      });
    }
  });
  it('returns Crystal-compatible JSON errors for missing sort and progress targets', async () => {
    const sortResponse = await api.get('/api/sort_opt?tid=nonexistent-title');
    expect(sortResponse.status).toBe(200);
    expect(await sortResponse.json()).toEqual({
      success: false,
      error: 'Nil assertion failed',
    });

    const sortUpdateResponse = await api.put('/api/sort_opt', {
      tid: 'nonexistent-title',
      sort: 'name',
      ascend: true,
    });
    expect(sortUpdateResponse.status).toBe(200);
    expect(await sortUpdateResponse.json()).toEqual({
      success: false,
      error: 'Nil assertion failed',
    });

    const progressResponse = await api.put('/api/progress/nonexistent-title/1');
    expect(progressResponse.status).toBe(200);
    expect(await progressResponse.json()).toEqual({
      success: false,
      error: 'Nil assertion failed',
    });

    const tagsResponse = await api.get('/api/tags/nonexistent-title');
    expect(tagsResponse.status).toBe(200);
    expect(await tagsResponse.json()).toEqual({
      success: false,
      error: 'Nil assertion failed',
    });
  });
  it('matches Mango numeric-path parse failures', async () => {
    const libraryResponse = await api.get('/api/library');
    const library = await libraryResponse.json();
    const title = library.titles[0];
    const progressResponse = await fetch(
      `${BASE_URL}/api/progress/${encodeURIComponent(title.id)}/not-a-page?eid=missing-entry`,
      {
        method: 'PUT',
        headers: { Cookie: getSessionCookie()! },
      },
    );
    expect(progressResponse.status).toBe(200);
    expect(await progressResponse.json()).toEqual({
      success: false,
      error: 'Invalid Int32: not-a-page',
    });

    const pageResponse = await api.get(
      '/api/page/nonexistent-title/nonexistent-entry/not-a-page',
    );
    expect(pageResponse.status).toBe(500);
    expect(await pageResponse.text()).toBe('Invalid Int32: not-a-page');
  });
  it('keeps malformed sort and bulk-progress JSON failures in Mango envelopes', async () => {
    const response = await api.get('/api/library');
    const library = await response.json();
    const title = library.titles.find(
      (item: { entries?: unknown[] }) => item.entries?.length,
    );
    expect(title).toBeDefined();
    if (!title) throw new Error('The test library must contain a title with entries');

    const cookie = getSessionCookie()!;
    for (const path of [
      '/api/sort_opt',
      `/api/bulk_progress/read/${encodeURIComponent(title.id)}`,
    ]) {
      const malformedResponse = await fetch(`${BASE_URL}${path}`, {
        method: 'PUT',
        headers: { Cookie: cookie, 'Content-Type': 'application/json' },
        body: '{',
      });
      expect(malformedResponse.status).toBe(200);
      const body = await malformedResponse.json();
      expect(body.success).toBe(false);
      expect(typeof body.error).toBe('string');
    }
  });
  it('returns Mango success bodies while adding and deleting title tags', async () => {
    const libraryResponse = await api.get('/api/library');
    const library = await libraryResponse.json();
    const title = library.titles.find(
      (item: { entries?: unknown[] }) => item.entries?.length,
    );
    expect(title).toBeDefined();
    if (!title) throw new Error('The test library must contain a title with entries');

    const tag = `response-contract-${Date.now()}`;
    const tagPath = `/api/admin/tags/${encodeURIComponent(title.id)}/${encodeURIComponent(tag)}`;
    const cookie = getSessionCookie()!;
    const initialTagsResponse = await api.get(`/api/tags/${encodeURIComponent(title.id)}`);
    const initialTags = (await initialTagsResponse.json()).tags;
    expect(initialTagsResponse.status).toBe(200);
    try {
      const addResponse = await api.put(tagPath);
      expect(addResponse.status).toBe(200);
      expect(await addResponse.json()).toEqual({ success: true, error: null });

      const tagsResponse = await api.get(`/api/tags/${encodeURIComponent(title.id)}`);
      expect(await tagsResponse.json()).toEqual({
        success: true,
        tags: expect.arrayContaining([...initialTags, tag]),
      });

      const deleteResponse = await fetch(`${BASE_URL}${tagPath}`, {
        method: 'DELETE',
        headers: { Cookie: cookie },
      });
      expect(deleteResponse.status).toBe(200);
      expect(await deleteResponse.json()).toEqual({ success: true, error: null });

      const afterDelete = await api.get(`/api/tags/${encodeURIComponent(title.id)}`);
      expect(await afterDelete.json()).toEqual({ success: true, tags: initialTags });
    } finally {
      await fetch(`${BASE_URL}${tagPath}`, {
        method: 'DELETE',
        headers: { Cookie: cookie },
      });
    }
  });

  it('returns Mango plain-text 404 for a missing download entry', async () => {
    const response = await api.get('/api/download/nonexistent-title/nonexistent-entry');
    expect(response.status).toBe(404);
    expect(response.headers.get('content-type')).toContain('text/plain');
    expect(await response.text()).toBe('Nil assertion failed');
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
