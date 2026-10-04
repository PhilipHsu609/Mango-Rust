import { beforeAll, describe, expect, it } from 'vitest';
import { BASE_URL, login } from './client';
import { catalog, entryByVolume, entryNames, TITLE_NAMES } from '../helpers/catalog';

const AUTH_HEADER = 'Basic ' + Buffer.from('testuser:testpass123').toString('base64');

async function feed(path: string) {
  const response = await fetch(`${BASE_URL}${path}`, { headers: { Authorization: AUTH_HEADER } });
  expect(response.status).toBe(200);
  expect(response.headers.get('content-type')).toMatch(/application\/(atom\+)?xml/);
  return response.text();
}

function feedEntries(xml: string) {
  return [...xml.matchAll(/<entry>([\s\S]*?)<\/entry>/g)].map((match) => match[1]);
}

function entryTitle(xml: string) {
  return xml.match(/<title>([^<]*)<\/title>/)?.[1];
}

describe('OPDS catalog API', () => {
  beforeAll(async () => { await login(); });

  it('lists all seven fixture titles with their navigable subsection links', async () => {
    const library = await catalog();
    const entries = feedEntries(await feed('/opds'));
    expect(entries.map(entryTitle).sort()).toEqual(TITLE_NAMES);
    for (const title of library.titles) {
      const entry = entries.find((candidate) => entryTitle(candidate) === title.title)!;
      expect(entry).toContain('rel="subsection"');
      expect(entry).toContain(`/opds/book/${title.id}`);
    }
  });

  it('provides acquisition links that download the requested ZIP volume', async () => {
    const { title, entry } = await entryByVolume('Test Manga Beta', 3);
    const entries = feedEntries(await feed(`/opds/book/${title.id}`));
    expect(entries.map(entryTitle)).toEqual(entryNames('Test Manga Beta'));
    const volume = entries.find((candidate) => entryTitle(candidate) === entry.title)!;
    expect(volume).toContain('rel="http://opds-spec.org/acquisition"');
    const downloadPath = `/api/download/${title.id}/${entry.id}`;
    expect(volume).toContain(downloadPath);
    const download = await fetch(`${BASE_URL}${downloadPath}`, { headers: { Authorization: AUTH_HEADER } });
    expect(download.status).toBe(200);
    const bytes = new Uint8Array(await download.arrayBuffer());
    expect([...bytes.subarray(0, 4)]).toEqual([80, 75, 3, 4]);
  });
});
