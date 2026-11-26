import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runQueue } from './run.mjs';

test('resume skips verified work; a stale handoff or failed gate cannot advance the queue', () => {
  const cwd = process.cwd(), dir = mkdtempSync(join(tmpdir(), 'rustday-'));
  const seen = [];
  function work(id) {
    seen.push(id);
    writeFileSync(`handoffs/${id}.json`, JSON.stringify({ status: 'ready', summary: 'fixture', evidence: ['fixture check'], next: 'next packet' }));
  }
  try {
    process.chdir(dir);
    assert.throws(() => runQueue(3, work, id => { if (id === '02-postgres') throw Error('failed gate'); }), /failed gate/);
    assert.deepEqual(seen, ['01-board', '02-postgres']);
    assert.throws(() => runQueue(1, () => {}, () => {}), /ENOENT/);
    assert.ok(!existsSync('.rustday/02-postgres.ok'));
    runQueue(1, work, () => {});
    assert.deepEqual(seen, ['01-board', '02-postgres', '02-postgres']);
    assert.ok(!existsSync('.rustday/03-desktop.ok'));
  } finally { process.chdir(cwd); rmSync(dir, { recursive: true, force: true }); }
});
