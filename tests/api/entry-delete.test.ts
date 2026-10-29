import { appendFile, copyFile, mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, inject, it } from 'vitest';
import type { CatalogTitle } from '../helpers/catalog';
import { startServer, type TestServer } from '../helpers/server';
import { REGULAR_USER, TEST_USER, type LoginCredentials } from '../helpers/test-users';

interface DeletePreview {
  entry_name: string;
  size_bytes: number;
  mtime_ns: string;
  removes_title: boolean;
  redirect: string;
}

const ALPHA = 'Test Manga Alpha';
const BETA = 'Test Manga Beta';

describe('Archive entry delete API', () => {
  let libraryPath: string;
  let server: TestServer;
  let admin: string;
  let reader: string;

  const request = (path: string, cookie: string, method = 'GET') =>
    fetch(`${server.url}${path}`, { method, headers: { Cookie: cookie } });

  const signIn = async (credentials: LoginCredentials): Promise<string> => {
    const response = await fetch(`${server.url}/api/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(credentials),
    });
    expect(response.status).toBe(200);
    return response.headers.get('set-cookie')!.split(';')[0];
  };

  const titles = async (): Promise<CatalogTitle[]> => {
    const response = await request('/api/library', admin);
    expect(response.status).toBe(200);
    return ((await response.json()) as { titles: CatalogTitle[] }).titles;
  };

  const ids = async (titleName: string, volume: string): Promise<[string, string]> => {
    const title = (await titles()).find((candidate) => candidate.title === titleName)!;
    const entry = title.entries.find((candidate) => candidate.title === `${titleName} ${volume}`)!;
    return [title.id, entry.id];
  };

  const preview = async (tid: string, eid: string): Promise<DeletePreview> => {
    const response = await request(`/api/admin/entry-delete/${tid}/${eid}`, admin);
    expect(response.status).toBe(200);
    expect(response.headers.has('access-control-allow-origin')).toBe(false);
    return response.json();
  };

  const remove = (tid: string, eid: string, shown: DeletePreview) =>
    request(
      `/api/admin/entry-delete/${tid}/${eid}?size=${shown.size_bytes}&mtime_ns=${encodeURIComponent(shown.mtime_ns)}`,
      admin,
      'DELETE',
    );

  beforeAll(async () => {
    libraryPath = await mkdtemp(join(tmpdir(), 'mango-entry-delete-test-'));
    for (const [titleName, volumes] of [[ALPHA, ['Vol.01', 'Vol.02']], [BETA, ['Vol.01']]] as const) {
      await mkdir(join(libraryPath, titleName));
      for (const volume of volumes) {
        const archive = `${titleName} ${volume}.zip`;
        await copyFile(join(inject('libraryPath'), titleName, archive), join(libraryPath, titleName, archive));
      }
    }
    server = await startServer({ libraryPath });
    admin = await signIn(TEST_USER);
    reader = await signIn(REGULAR_USER);
    const deadline = Date.now() + 10_000;
    while ((await titles()).length < 2) {
      if (Date.now() > deadline) throw new Error('The private library scan did not finish');
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  });

  afterAll(async () => {
    try { await server?.close(); } finally { await rm(libraryPath, { recursive: true, force: true }); }
  });

  it('refuses non-admins on both routes', async () => {
    const [tid, eid] = await ids(ALPHA, 'Vol.01');
    expect((await request(`/api/admin/entry-delete/${tid}/${eid}`, reader)).status).toBe(403);
    expect((await request(`/api/admin/entry-delete/${tid}/${eid}?size=1&mtime_ns=1`, reader, 'DELETE')).status).toBe(403);
  });

  it('deletes an entry with the previewed values, then reports it gone', async () => {
    const [tid, eid] = await ids(ALPHA, 'Vol.01');
    const shown = await preview(tid, eid);
    expect(shown.entry_name).toBe(`${ALPHA} Vol.01.zip`);
    expect(shown.size_bytes).toBeGreaterThan(0);
    expect(typeof shown.mtime_ns).toBe('string');
    expect(shown.removes_title).toBe(false);

    const deleted = await remove(tid, eid, shown);
    expect(deleted.status).toBe(200);
    expect(deleted.headers.has('access-control-allow-origin')).toBe(false);
    expect(await deleted.json()).toEqual({ success: true, removes_title: false, redirect: `/book/${tid}`, warnings: [] });

    const detail: CatalogTitle = await (await request(`/api/book/${tid}`, admin)).json();
    expect(detail.entries.map((entry) => entry.id)).not.toContain(eid);
    const missing: { entries: { id: string }[] } = await (await request('/api/admin/entries/missing', admin)).json();
    expect(JSON.stringify(missing)).not.toContain(eid);

    const again = await remove(tid, eid, shown);
    expect(again.status).toBe(404);
    expect(await again.json()).toEqual({ success: false, error: 'Entry not found; it may already have been deleted' });
  });

  it('keeps a file that changed after the preview', async () => {
    const [tid, eid] = await ids(ALPHA, 'Vol.02');
    const shown = await preview(tid, eid);
    await appendFile(join(libraryPath, ALPHA, `${ALPHA} Vol.02.zip`), Buffer.from([0]));
    const response = await remove(tid, eid, shown);
    expect(response.status).toBe(409);
    expect(await response.json()).toEqual({ success: false, error: 'This file changed since you opened this dialog' });
  });

  it('rejects confirmation values that are not numbers', async () => {
    const [tid, eid] = await ids(ALPHA, 'Vol.02');
    const response = await request(`/api/admin/entry-delete/${tid}/${eid}?size=1&mtime_ns=soon`, admin, 'DELETE');
    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({ success: false, error: 'Invalid confirmation values' });
  });

  it('reports that deleting the last entry removes the title', async () => {
    const [tid, eid] = await ids(BETA, 'Vol.01');
    const shown = await preview(tid, eid);
    expect(shown.removes_title).toBe(true);
    const response = await remove(tid, eid, shown);
    expect(response.status).toBe(200);
    expect(await response.json()).toMatchObject({ success: true, removes_title: true, redirect: '/library' });
  });
});
