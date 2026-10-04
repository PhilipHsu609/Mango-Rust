import { inject } from 'vitest';
import { TEST_USER } from '../helpers/test-users';

const BASE_URL = inject('baseUrl');

export interface ApiClient {
  get: (path: string) => Promise<Response>;
  post: (path: string, body?: unknown) => Promise<Response>;
  put: (path: string, body?: unknown) => Promise<Response>;
}

let sessionCookie: string | null = null;

export async function login(username = TEST_USER.username, password = TEST_USER.password): Promise<void> {
  sessionCookie = null;
  const response = await fetch(`${BASE_URL}/api/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });

  if (response.status !== 200) {
    throw new Error(`Login failed for user '${username}': HTTP ${response.status}`);
  }
  const setCookie = response.headers.get('set-cookie');
  if (setCookie) {
    sessionCookie = setCookie.split(';')[0];
  }

  if (!sessionCookie) {
    throw new Error(`Login failed for user '${username}': no session cookie returned`);
  }
}

export function logout(): void {
  sessionCookie = null;
}

export function getSessionCookie(): string | null {
  return sessionCookie;
}

function getHeaders(): HeadersInit {
  const headers: HeadersInit = { 'Content-Type': 'application/json' };
  if (sessionCookie) {
    headers['Cookie'] = sessionCookie;
  }
  return headers;
}

export const api: ApiClient = {
  get: (path: string) => fetch(`${BASE_URL}${path}`, { headers: getHeaders() }),

  post: (path: string, body?: unknown) => fetch(`${BASE_URL}${path}`, {
    method: 'POST',
    headers: getHeaders(),
    body: body === undefined ? undefined : JSON.stringify(body),
  }),

  put: (path: string, body?: unknown) => fetch(`${BASE_URL}${path}`, {
    method: 'PUT',
    headers: getHeaders(),
    body: body === undefined ? undefined : JSON.stringify(body),
  }),

};

export { BASE_URL };
