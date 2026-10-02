import bcrypt from 'bcryptjs';
import Database from 'better-sqlite3';

export interface LoginCredentials {
  username: string;
  password: string;
}

export const TEST_USER: LoginCredentials = {
  username: 'testuser',
  password: 'testpass123',
};

export const REGULAR_USER: LoginCredentials = {
  username: 'testuser2',
  password: 'testpass123',
};

export function createTestUser(
  dbPath: string,
  credentials: LoginCredentials = TEST_USER,
  admin: boolean = true
): void {
  const db = new Database(dbPath);

  try {
    const passwordHash = bcrypt.hashSync(credentials.password, 10);
    db.prepare(`
      INSERT INTO users (username, password, token, admin)
      VALUES (?, ?, NULL, ?)
      ON CONFLICT(username) DO UPDATE SET
        password = excluded.password,
        token = NULL,
        admin = excluded.admin
    `).run(credentials.username, passwordHash, admin ? 1 : 0);
  } finally {
    db.close();
  }
}
