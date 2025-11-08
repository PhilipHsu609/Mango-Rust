-- Migration 001: Create users table
-- Matches original Mango schema from migrations/users.1.cr

CREATE TABLE IF NOT EXISTS users (
    username TEXT NOT NULL PRIMARY KEY,
    password TEXT NOT NULL,  -- bcrypt hashed password
    token TEXT UNIQUE,       -- session token (generated on login)
    admin INTEGER NOT NULL   -- 0 = user, 1 = admin
);

CREATE UNIQUE INDEX IF NOT EXISTS username_idx ON users (username);
CREATE UNIQUE INDEX IF NOT EXISTS token_idx ON users (token);
