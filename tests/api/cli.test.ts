import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

const BINARY = fileURLToPath(new URL('../../target/release/mango-rust', import.meta.url));
let testDir: string;
let configPath: string;

function runCli(...args: string[]): string {
  return execFileSync(BINARY, args, { encoding: 'utf-8', timeout: 30000 });
}

beforeAll(() => {
  testDir = mkdtempSync(path.join(os.tmpdir(), 'mango-rust-cli-'));
  configPath = path.join(testDir, 'config.yml');
  writeFileSync(
    configPath,
    `db_path: ${path.join(testDir, 'mango.db')}\nlog_level: warn\n`,
    'utf-8',
  );
});

afterAll(() => {
  rmSync(testDir, { recursive: true, force: true });
});

describe('Mango user management CLI', () => {
  it('shows nested user command help', () => {
    const help = runCli('admin', 'user');

    expect(help).toContain('add');
    expect(help).toContain('delete');
    expect(help).toContain('update');
    expect(help).toContain('list');
  });

  it('adds, lists, updates, and deletes users with global config options', () => {
    runCli('--config', configPath, 'admin', 'user', 'add', '-u', 'cli-user', '-p', 'first-pass', '-a');

    const addedUsers = runCli('admin', 'user', 'list', `--config=${configPath}`);
    expect(addedUsers).toMatch(/cli-user\s+true/);

    runCli('admin', 'user', '-c', configPath, 'update', 'cli-user', '-u', 'renamed-user', '-p', 'second-pass');

    const updatedUsers = runCli('admin', 'user', 'list', '--config', configPath);
    expect(updatedUsers).toMatch(/renamed-user\s+false/);
    expect(updatedUsers).not.toContain('cli-user');

    runCli('admin', 'user', 'delete', 'renamed-user', '-c', configPath);

    const deletedUsers = runCli('admin', 'user', 'list', '--config', configPath);
    expect(deletedUsers).not.toContain('renamed-user');
  });
});
