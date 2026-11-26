import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { packets, runQueue, validLimit, verify } from './run.mjs';

test('resume skips verified work; a stale handoff or failed gate cannot advance the queue', () => {
  const cwd = process.cwd(), dir = mkdtempSync(join(tmpdir(), 'rustday-'));
  const seen = [];
  function work({ id }) {
    seen.push(id);
    writeFileSync(`handoffs/${id}.json`, JSON.stringify({ status: 'ready', summary: 'fixture', evidence: ['fixture check'], next: 'next packet' }));
  }
  try {
    process.chdir(dir);
    assert.throws(() => runQueue(3, work, ({ id }) => { if (id === '02-postgres') throw Error('failed gate'); }), /failed gate/);
    assert.deepEqual(seen, ['01-board', '02-postgres']);
    assert.throws(() => runQueue(1, () => {}, () => {}), /ENOENT/);
    assert.ok(!existsSync('.rustday/02-postgres.ok'));
    runQueue(1, work, () => {});
    assert.deepEqual(seen, ['01-board', '02-postgres', '02-postgres']);
    assert.ok(!existsSync('.rustday/03-desktop.ok'));
    const checked = [];
    runQueue(2, work, ({ target }) => checked.push(target));
    assert.deepEqual(checked, ['desktop_edit', 'mobile_edit_service']);
    assert.ok(existsSync('.rustday/04-mobile-edit-service.ok'));
  } finally { process.chdir(cwd); rmSync(dir, { recursive: true, force: true }); }
});

test('batch limits follow the queue and reject malformed counts', () => {
  for (let i = 1; i <= packets.length; i++) assert.ok(validLimit(String(i)));
  for (const value of [undefined, '0', '-1', '01', '1.1', '1x', String(packets.length + 1)]) {
    assert.ok(!validLimit(value));
  }
});

test('the mobile service gate requires both cases, an enabled suite and workspace checks', () => {
  const packet = packets.find(p => p.id === '04-mobile-edit-service');
  assert.equal(packet.database, true);
  const success = 'test rename_then_reconnect ... ok';
  const recovery = 'test reject_then_retry ... ok';
  const summary = 'test result: ok. 2 passed; 0 failed; 0 ignored';
  for (const missing of [success, recovery]) {
    assert.throws(() => verify(packet, () => `${missing}\n${summary}`), /Missing passing/);
  }
  assert.throws(() => verify(packet, () => 'test result: ok. 0 passed; 0 failed; 2 ignored'), /No passing/);
  const calls = [];
  verify(packet, (bin, args) => {
    calls.push([bin, ...args].join(' '));
    return [success, recovery, summary].join('\r\n');
  });
  assert.deepEqual(calls, ['cargo fmt --all -- --check', 'cargo test --locked --workspace --test mobile_edit_service', 'cargo test --locked --workspace']);
});
