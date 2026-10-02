import { describe, it, expect, beforeAll } from 'vitest';
import { api, login } from './client';

type TestEntry = { id: string; pages: number };
type TestTitle = { id: string; entries?: TestEntry[] };

async function firstEntry(): Promise<{ title: TestTitle; entry: TestEntry }> {
  const response = await api.get('/api/library');
  expect(response.status).toBe(200);
  const library = await response.json();
  const title: TestTitle | undefined = library.titles.find(
    (candidate: TestTitle) => candidate.entries?.length,
  );
  expect(title).toBeDefined();
  if (!title?.entries?.length) {
    throw new Error('The test library must contain a title with an entry');
  }
  return { title, entry: title.entries[0] };
}

async function entryPercentage(titleId: string, entryId: string): Promise<number> {
  const response = await api.get(`/api/book/${titleId}?percentage=true`);
  expect(response.status).toBe(200);
  const title = await response.json();
  const index = title.entries.findIndex((entry: TestEntry) => entry.id === entryId);
  expect(index).toBeGreaterThanOrEqual(0);
  expect(title.entry_percentages).toHaveLength(title.entries.length);
  return title.entry_percentages[index];
}

describe('Progress API', () => {
  beforeAll(async () => {
    await login();
  });

  it('persists individual progress, accepts the final page, and rejects out-of-range pages', async () => {
    const { title, entry } = await firstEntry();
    expect(entry.pages).toBeGreaterThan(1);
    const progressBase = `/api/progress/${title.id}`;

    const unread = await api.put(
      `${progressBase}/0?eid=${encodeURIComponent(entry.id)}`,
    );
    expect(unread.status).toBe(200);
    expect(await unread.json()).toEqual({ success: true });
    expect(await entryPercentage(title.id, entry.id)).toBe(0);

    try {
      const update = await api.put(
        `${progressBase}/1?eid=${encodeURIComponent(entry.id)}`,
      );
      expect(update.status).toBe(200);
      expect(await update.json()).toEqual({ success: true });
      expect(await entryPercentage(title.id, entry.id)).toBeCloseTo(1 / entry.pages);

      const continueReadingResponse = await api.get('/api/library/continue_reading');
      const continueReading = await continueReadingResponse.json();
      const continuedIndex = continueReading.entries.findIndex(
        (candidate: TestEntry) => candidate.id === entry.id,
      );
      expect(continuedIndex).toBeGreaterThanOrEqual(0);
      expect(continueReading.entry_percentages[continuedIndex]).toBeCloseTo(1 / entry.pages);

      const finalPage = await api.put(
        `${progressBase}/${entry.pages}?eid=${encodeURIComponent(entry.id)}`,
      );
      expect(finalPage.status).toBe(200);
      expect(await finalPage.json()).toEqual({ success: true });
      expect(await entryPercentage(title.id, entry.id)).toBe(1);

      const invalidPage = await api.put(
        `${progressBase}/${entry.pages + 1}?eid=${encodeURIComponent(entry.id)}`,
      );
      expect(invalidPage.status).toBe(200);
      expect(await invalidPage.json()).toMatchObject({ success: false });
      expect(await entryPercentage(title.id, entry.id)).toBe(1);
    } finally {
      const reset = await api.put(
        `${progressBase}/0?eid=${encodeURIComponent(entry.id)}`,
      );
      expect(reset.status).toBe(200);
      expect(await reset.json()).toEqual({ success: true });
    }

    expect(await entryPercentage(title.id, entry.id)).toBe(0);
  });

  it('bulk read and unread persist the requested state and reject invalid actions', async () => {
    const { title, entry } = await firstEntry();
    const bulkPath = `/api/bulk_progress`;

    try {
      const read = await api.put(`${bulkPath}/read/${title.id}`, { ids: [entry.id] });
      expect(read.status).toBe(200);
      expect(await read.json()).toEqual({ success: true });
      expect(await entryPercentage(title.id, entry.id)).toBe(1);

      const unread = await api.put(`${bulkPath}/unread/${title.id}`, { ids: [entry.id] });
      expect(unread.status).toBe(200);
      expect(await unread.json()).toEqual({ success: true });
      expect(await entryPercentage(title.id, entry.id)).toBe(0);

      const invalidAction = await api.put(`${bulkPath}/invalid/${title.id}`, {
        ids: [entry.id],
      });
      expect(invalidAction.status).toBe(200);
      expect(await invalidAction.json()).toMatchObject({ success: false });
      expect(await entryPercentage(title.id, entry.id)).toBe(0);
    } finally {
      const unread = await api.put(`${bulkPath}/unread/${title.id}`, { ids: [entry.id] });
      expect(unread.status).toBe(200);
      expect(await unread.json()).toEqual({ success: true });
    }
  });
});
