// Use explicit port to avoid vitest/vite import.meta.env.BASE_URL conflicts
const SERVER_PORT = 9000;
const SERVER_HOST = 'localhost';
const BASE_URL = `http://${SERVER_HOST}:${SERVER_PORT}`;

export interface ApiClient {
  get: (path: string) => Promise<Response>;
  post: (path: string, body?: unknown) => Promise<Response>;
  put: (path: string, body?: unknown) => Promise<Response>;
  patch: (path: string, body?: unknown) => Promise<Response>;
  delete: (path: string) => Promise<Response>;
}

let sessionCookie: string | null = null;

export async function login(username = 'testuser', password = 'testpass123'): Promise<void> {
  const response = await fetch(`${BASE_URL}/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({ username, password }),
    redirect: 'manual',
  });

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
    body: body ? JSON.stringify(body) : undefined,
  }),

  put: (path: string, body?: unknown) => fetch(`${BASE_URL}${path}`, {
    method: 'PUT',
    headers: getHeaders(),
    body: body ? JSON.stringify(body) : undefined,
  }),

  patch: (path: string, body?: unknown) => fetch(`${BASE_URL}${path}`, {
    method: 'PATCH',
    headers: getHeaders(),
    body: body ? JSON.stringify(body) : undefined,
  }),

  delete: (path: string) => fetch(`${BASE_URL}${path}`, {
    method: 'DELETE',
    headers: getHeaders(),
  }),
};

export { BASE_URL };
