import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { api, BASE_URL, getSessionCookie, login } from './client';
import { book, CatalogEntry, CatalogTitle, entryByVolume, TITLE_NAMES } from '../helpers/catalog';

async function entryPercentages(titleId: string) {
  const title = await book(titleId, '?percentage=true');
  expect(title.entry_percentages).toHaveLength(5);
  return title.entry_percentages!;
}

async function continueReading() {
  const response = await api.get('/api/library/continue_reading');
  expect(response.status).toBe(200);
  const data: { success: boolean; entries: CatalogEntry[]; entry_percentages: number[] } = await response.json();
  expect(data.success).toBe(true);
  return data;
}

async function startReadingNames() {
  const response = await api.get('/api/library/start_reading');
  expect(response.status).toBe(200);
  const data: { success: boolean; titles: CatalogTitle[] } = await response.json();
  expect(data.success).toBe(true);
  return data.titles.map((title) => title.title).sort();
}

describe('Reading progress API', () => {
  const username = 'progress-owner';
  let adminCookie: string;
  beforeAll(async () => {
    await login();
    adminCookie = getSessionCookie()!;
    const created = await api.post('/api/admin/users', { username, password: 'progress-password', is_admin: false });
    expect(created.status).toBe(201);
    await login(username, 'progress-password');
  });
  afterAll(async () => {
    const response = await fetch(`${BASE_URL}/api/admin/users/${username}`, { method: 'DELETE', headers: { Cookie: adminCookie } });
    expect(response.status).toBe(204);
  });

  it('persists first/final page progress, rejects both boundaries, and selects the next unread volume', async () => {
    const { title, entry } = await entryByVolume('Test Manga Alpha');
    const next = title.entries.find((candidate) => candidate.title === 'Test Manga Alpha Vol.02')!;
    const path = `/api/progress/${title.id}`;
    expect(await entryPercentages(title.id)).toEqual([0, 0, 0, 0, 0]);
    expect(await continueReading()).toEqual({ success: true, entries: [], entry_percentages: [] });
    expect(await startReadingNames()).toEqual(TITLE_NAMES);
    try {
      for (const page of [1, 10]) {
        const response = await api.put(`${path}/${page}?eid=${encodeURIComponent(entry.id)}`);
        expect(response.status).toBe(200);
        expect(await response.json()).toEqual({ success: true });
        expect(await entryPercentages(title.id)).toEqual([page / 10, 0, 0, 0, 0]);
        const continued = await continueReading();
        expect(continued.entries.map((candidate) => candidate.id)).toEqual([page === 1 ? entry.id : next.id]);
        expect(continued.entry_percentages).toEqual([page === 1 ? 0.1 : 0]);
        expect(await startReadingNames()).toEqual(TITLE_NAMES.filter((name) => name !== 'Test Manga Alpha'));
      }
      for (const page of [-1, 11]) {
        const response = await api.put(`${path}/${page}?eid=${encodeURIComponent(entry.id)}`);
        expect(response.status).toBe(200);
        expect(await response.json()).toEqual({ success: false, error: 'incorrect page value' });
        expect(await entryPercentages(title.id)).toEqual([1, 0, 0, 0, 0]);
      }
    } finally { await api.put(`${path}/0?eid=${encodeURIComponent(entry.id)}`); }
    expect(await entryPercentages(title.id)).toEqual([0, 0, 0, 0, 0]);
    expect(await startReadingNames()).toEqual(TITLE_NAMES);
    expect(await continueReading()).toEqual({ success: true, entries: [], entry_percentages: [] });
  });

  it('bulk read/unread changes only selected volumes and rejects invalid actions', async () => {
    const { title, entry } = await entryByVolume('Test Manga Beta');
    const second = title.entries.find((candidate) => candidate.title === 'Test Manga Beta Vol.02')!;
    const ids = [entry.id, second.id];
    try {
      for (const action of ['read', 'unread']) {
        const response = await api.put(`/api/bulk_progress/${action}/${title.id}`, { ids });
        expect(response.status).toBe(200);
        expect(await response.json()).toEqual({ success: true });
        expect(await entryPercentages(title.id)).toEqual(action === 'read' ? [1, 1, 0, 0, 0] : [0, 0, 0, 0, 0]);
      }
      const invalid = await api.put(`/api/bulk_progress/invalid/${title.id}`, { ids });
      expect(invalid.status).toBe(200);
      expect(await invalid.json()).toEqual({ success: false, error: 'Unknow action invalid' });
      expect(await entryPercentages(title.id)).toEqual([0, 0, 0, 0, 0]);
    } finally { await api.put(`/api/bulk_progress/unread/${title.id}`, { ids }); }
  });

  it('preserves Mango missing-target and numeric parse error envelopes', async () => {
    const missing = await api.put('/api/progress/nonexistent-title/1');
    expect(missing.status).toBe(200);
    expect(await missing.json()).toEqual({ success: false, error: 'Nil assertion failed' });
    const { title, entry } = await entryByVolume('Test Manga Charlie');
    const invalid = await api.put(`/api/progress/${title.id}/not-a-page?eid=${encodeURIComponent(entry.id)}`);
    expect(invalid.status).toBe(200);
    expect(await invalid.json()).toEqual({ success: false, error: 'Invalid Int32: not-a-page' });
    expect(await entryPercentages(title.id)).toEqual([0, 0, 0, 0, 0]);
  });

  it('rejects malformed bulk JSON without mutating reading progress', async () => {
    const { title } = await entryByVolume('Test Manga Delta');
    const response = await fetch(`${BASE_URL}/api/bulk_progress/read/${title.id}`, {
      method: 'PUT', headers: { Cookie: getSessionCookie()!, 'Content-Type': 'application/json' }, body: '{',
    });
    expect(response.status).toBe(200);
    expect(await response.json()).toMatchObject({ success: false, error: expect.any(String) });
    expect(await entryPercentages(title.id)).toEqual([0, 0, 0, 0, 0]);
  });

  it('groups all newly scanned volumes into seven recent title cards', async () => {
    const response = await api.get('/api/library/recently_added');
    expect(response.status).toBe(200);
    const data: { success: boolean; items: { item: CatalogTitle; percentage: number; count: number }[] } = await response.json();
    expect(data.success).toBe(true);
    expect(data.items.map(({ item }) => item.title).sort()).toEqual(TITLE_NAMES);
    expect(data.items.map(({ count }) => count)).toEqual([5, 5, 5, 5, 5, 5, 5]);
    expect(data.items.map(({ percentage }) => percentage)).toEqual([-1, -1, -1, -1, -1, -1, -1]);
  });
});
