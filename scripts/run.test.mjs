import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { packets, requireEditEmulator, runQueue, validLimit, verify } from './run.mjs';

test('resume skips verified work; a stale handoff or failed gate cannot advance the queue', () => {
  const cwd = process.cwd(), dir = mkdtempSync(join(tmpdir(), 'boardcraft-'));
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
    assert.ok(!existsSync('.boardcraft/02-postgres.ok'));
    runQueue(1, work, () => {});
    assert.deepEqual(seen, ['01-board', '02-postgres', '02-postgres']);
    assert.ok(!existsSync('.boardcraft/03-desktop.ok'));
    const checked = [];
    runQueue(2, work, ({ target }) => checked.push(target));
    assert.deepEqual(checked, ['desktop_edit', 'mobile_edit_service']);
    assert.ok(existsSync('.boardcraft/04-mobile-edit-service.ok'));
  } finally { process.chdir(cwd); rmSync(dir, { recursive: true, force: true }); }
});

test('an upgraded checkout honors its existing lock and resumes from its markers', () => {
  const cwd = process.cwd(), dir = mkdtempSync(join(tmpdir(), 'boardcraft-'));
  try {
    process.chdir(dir);
    mkdirSync('.rustday');
    writeFileSync('.rustday/01-board.ok', 'verified');
    writeFileSync('.rustday/lock', 'other runner');
    assert.throws(() => runQueue(1, assert.fail, assert.fail), { code: 'EEXIST' });
    assert.equal(readFileSync('.rustday/lock', 'utf8'), 'other runner');
    rmSync('.rustday/lock');
    runQueue(1, ({ id }, state) => {
      assert.equal(id, '02-postgres');
      assert.equal(state, '.rustday');
      writeFileSync(`handoffs/${id}.json`, JSON.stringify({ status: 'ready', summary: 'fixture', evidence: ['checked'], next: 'continue' }));
    }, () => {});
    assert.ok(existsSync('.rustday/02-postgres.ok'));
    assert.ok(!existsSync('.boardcraft'));
    mkdirSync('.boardcraft');
    assert.throws(() => runQueue(1, assert.fail, assert.fail), /Both .rustday and .boardcraft/);
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

test('emulator preflight fails closed before any device mutation', () => {
  for (const serial of ['', '192.0.2.1:5555', 'usb-phone', 'emulator-1 extra']) {
    assert.throws(() => requireEditEmulator(() => assert.fail('unexpected adb'), serial), /dedicated emulator serial/);
  }
  for (const [replies, error] of [
    [['another_avd\r\nOK'], /Expected Rustday_Edit_X64/],
    [['Rustday_Edit_X64\r\nOK', '0'], /not booted/],
    [['Rustday_Edit_X64\r\nOK', '1', 'arm64-v8a'], /Expected x86_64/],
  ]) {
    assert.throws(() => requireEditEmulator((bin, args, capture, timeout) => {
      assert.equal(bin, 'adb');
      assert.deepEqual(args.slice(0, 2), ['-s', 'emulator-5582']);
      assert.ok(['emu avd name', 'shell getprop sys.boot_completed', 'shell getprop ro.product.cpu.abi'].includes(args.slice(2).join(' ')));
      assert.ok(capture && timeout > 0 && timeout <= 10_000);
      return replies.shift();
    }, 'emulator-5582'), error);
  }
});

test('05 cross-builds the device binary and requires both enabled real-suite cases before workspace checks', t => {
  const packet = packets.at(-1);
  assert.equal(packet.id, '05-android-emulator-edit');
  assert.equal(packet.database, true);
  const prior = process.env.RUSTDAY_ANDROID_SERIAL;
  t.after(() => { if (prior === undefined) delete process.env.RUSTDAY_ANDROID_SERIAL; else process.env.RUSTDAY_ANDROID_SERIAL = prior; });
  process.env.RUSTDAY_ANDROID_SERIAL = 'emulator-5582';
  const success = 'test emulator_rename_then_reopen ... ok', recovery = 'test emulator_reject_then_correct ... ok';
  const summary = 'test result: ok. 2 passed; 0 failed; 0 ignored';
  const calls = [];
  function run(lines) {
    const replies = ['Rustday_Edit_X64\r\r\nOK\r\r\n', '1', 'x86_64'];
    verify(packet, (bin, args, capture, timeout) => {
      if (bin === 'adb') return replies.shift();
      calls.push([bin, ...args].join(' '));
      if (args.includes('android_emulator_edit')) {
        assert.ok(capture && timeout > 0 && timeout <= 600_000);
        return lines.join('\n');
      }
    });
  }
  for (const one of [success, recovery]) assert.throws(() => run([one, summary]), /Missing passing/);
  assert.throws(() => run([success, recovery, 'test result: ok. 2 passed; 0 failed; 1 ignored']), /No passing/);
  calls.length = 0;
  run([recovery, success, summary]);
  assert.deepEqual(calls, ['cargo fmt --all -- --check',
    'cargo build --locked -p boardcraft-mobile-edit-client --bin android-edit-probe --target x86_64-linux-android',
    'cargo test --locked -p boardcraft-mobile-edit-host --features android-smoke --test android_emulator_edit -- --test-threads=1',
    'cargo test --locked --workspace']);
});
