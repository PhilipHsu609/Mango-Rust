import { expect } from 'vitest';
import { api } from '../api/client';

export const TITLE_NAMES = [
  'Test Manga Alpha',
  'Test Manga Beta',
  'Test Manga Charlie',
  'Test Manga Delta',
  'Test Manga Echo',
  'Test Manga Foxtrot',
  'Test Manga Golf',
] as const;

export type TitleName = (typeof TITLE_NAMES)[number];

export interface CatalogEntry {
  id: string;
  title: string;
  pages: number;
  sort_title: string;
  display_name: string;
  cover_url: string;
}

export interface CatalogTitle {
  id: string;
  title: string;
  sort_title: string;
  display_name: string;
  cover_url: string;
  entries: CatalogEntry[];
  titles: CatalogTitle[];
  entry_percentages?: number[];
}

export interface Catalog {
  titles: CatalogTitle[];
}

export function entryNames(title: TitleName): string[] {
  return [1, 2, 3, 4, 5].map((volume) => `${title} Vol.${String(volume).padStart(2, '0')}`);
}

export async function catalog(query = ''): Promise<Catalog> {
  const response = await api.get(`/api/library${query}`);
  expect(response.status).toBe(200);
  return response.json();
}

export async function titleByName(name: TitleName): Promise<CatalogTitle> {
  const library = await catalog();
  const title = library.titles.find((candidate) => candidate.title === name);
  if (!title) throw new Error(`Fixture title missing: ${name}`);
  return title;
}

export async function entryByVolume(name: TitleName, volume = 1): Promise<{
  title: CatalogTitle;
  entry: CatalogEntry;
}> {
  const title = await titleByName(name);
  const entryName = `${name} Vol.${String(volume).padStart(2, '0')}`;
  const entry = title.entries.find((candidate) => candidate.title === entryName);
  if (!entry) throw new Error(`Fixture entry missing: ${entryName}`);
  return { title, entry };
}

export async function book(titleId: string, query = ''): Promise<CatalogTitle> {
  const response = await api.get(`/api/book/${encodeURIComponent(titleId)}${query}`);
  expect(response.status).toBe(200);
  return response.json();
}
