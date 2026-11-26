import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { packets, requireEditEmulator, requireKitEmulator, runQueue, validLimit, verify } from './run.mjs';
import { matchBoard } from './android-kit-frame.mjs';
import { editFrame } from './android-kit-edit.mjs';
import { checkWindow, instance } from './cross-client-reopen.mjs';

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
  const packet = packets.find(p => p.id === '05-android-emulator-edit');
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

test('06 rejects shared/remote servers and requires the dedicated AVD', () => {
  const env = { ANDROID_ADB_SERVER_PORT: '5039', ADB_SERVER_SOCKET: 'tcp:5039', ADB_USB: '0', ADB_EMU: '0', RUSTDAY_ANDROID_SERIAL: 'emulator-5682' };
  for (const change of [{ ADB_SERVER_SOCKET: 'tcp:remote:5039' }, { ANDROID_ADB_SERVER_PORT: '5037', ADB_SERVER_SOCKET: 'tcp:5037' }, { ADB_USB: '1' }]) {
    assert.throws(() => requireKitEmulator(assert.fail, { ...env, ...change }), /private loopback/);
  }
  assert.throws(() => requireKitEmulator(() => 'another', env), /Expected Rustday_Edit_X64/);
});

test('06 requires the frame gate, not the process probe or just a successful build', () => {
  const p = packets.find(p => p.id === '06-android-kit-frame');
  assert.equal(p.id, '06-android-kit-frame');
  const calls = [];
  verify(p, (bin, args) => calls.push([bin, ...args].join(' ')));
  assert.deepEqual(calls, ['cargo fmt --all -- --check', 'cargo fmt --manifest-path android/Cargo.toml -- --check', 'node scripts/android-kit-frame.mjs', 'cargo test --locked --workspace']);
  assert.throws(() => verify(p, bin => { if (bin === 'node') throw Error('missing editor frame'); }), /missing editor frame/);
});

test('ordered OCR frames distinguish an editor from a stale, blank or unrelated screen', () => {
  const frame = ['Sample board', 'Review sketch', 'Editing name', 'Review artwork'].map((text, i) => ({ text, x: 64, y: 80 + 80*i }));
  const board = frame.filter(l => l.text !== 'Editing name');
  assert.deepEqual(matchBoard(board, false), frame[3]);
  assert.deepEqual(matchBoard(frame, true), frame[3]);
  for (const wrong of [[], board, frame.slice(1), [...frame, frame[1]], [...frame.slice(0,3), { ...frame[3], y: 20 }]]) {
    assert.equal(matchBoard(wrong, true), undefined);
  }
  assert.equal(matchBoard(frame, false), undefined);
});

test('07 requires live editor, recovery and store gate after the existing frame gate', () => {
  const packet = packets.find(p => p.id === '07-android-kit-edit');
  const calls = [];
  verify(packet, (bin, args) => calls.push([bin,...args].join(' ')));
  assert.deepEqual(calls.slice(2), ['node scripts/android-kit-frame.mjs','node --test scripts/run.test.mjs','node scripts/android-kit-edit.mjs','cargo test --locked --workspace']);
  assert.throws(() => verify(packet, (_, args) => { if (args.includes('scripts/android-kit-edit.mjs')) throw Error('store mismatch'); }), /store mismatch/);
  const expected = ['Sample board','Review sketch','Editing name','Coral','Save'];
  const frame = expected.map((text,i) => ({text, x:60, y:50+i*50}));
  assert.deepEqual(editFrame(frame, expected).Coral, frame[3]);
  for (const bad of [frame.slice(0,-1), [...frame,frame[3]], [...frame.slice(0,3),frame[4],{...frame[3],y:500}], [...frame,{text:'Ink',x:30,y:65}]]) {
    assert.equal(editFrame(bad, expected, ['Ink']), undefined);
  }
});

test('08 requires actual delivery and Windows regression; missing activity or wrong frame cannot pass', () => {
  const packet = packets.find(p => p.id === '08-cross-client-reopen');
  const calls = [];
  verify(packet, (bin, args) => calls.push([bin,...args].join(' ')));
  assert.ok(calls.includes('node scripts/cross-client-reopen.mjs'));
  assert.ok(calls.some(c => c.endsWith('--test windows_smoke')));
  assert.equal(instance('ACTIVITY dev.boardcraft.kitfixture/.App abcd pid=42'), undefined);
  assert.throws(() => checkWindow('', 'board'));
  assert.throws(() => checkWindow('row board 00000000-0000-0000-0000-000000000011 Review sketch\nrow board 00000000-0000-0000-0000-00000000002a Coral','board'));
});

test('09 requires both enabled Windows journeys and inherits the real Android/Windows delivery gate', () => {
  const packet = packets.find(p => p.id === '09-windows-name-cell');
  const cases = ['rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter'];
  const calls = [];
  const run = names => verify(packet, (bin, args, capture) => {
    calls.push(args);
    if (args.includes('windows_smoke')) {
      assert.ok(capture && args.includes('--test-threads=1'));
      return [...names.map(n => `test ${n} ... ok`), `test result: ok. ${names.length} passed; 0 failed; 0 ignored`].join('\n');
    }
  });
  for (const name of cases) assert.throws(() => run([name]), /Missing passing/);
  calls.length = 0;
  run(cases);
  for (const gate of ['scripts/android-kit-frame.mjs', 'scripts/cross-client-reopen.mjs', '--workspace']) {
    assert.ok(calls.some(args => args.includes(gate)));
  }
});

test('10 requires parent recovery, both Windows regressions, same-board delivery and the binary build', () => {
  const packet = packets.find(p => p.id === '10-windows-name-recovery');
  const cases = [...packets.find(p => p.id === '09-windows-name-cell').windowsCases, 'parent_failure_retry_dismiss_and_commit'];
  const calls = [];
  const run = names => verify(packet, (bin, args) => {
    calls.push([bin, ...args].join(' '));
    return [...names.map(n => `test ${n} ... ok`), `test result: ok. ${names.length} passed; 0 failed; 0 ignored`].join('\n');
  });
  for (const name of cases) assert.throws(() => run(cases.filter(n => n !== name)), /Missing passing windows_smoke case/);
  calls.length = 0;
  run(cases);
  for (const gate of ['node scripts/android-kit-frame.mjs', 'node scripts/cross-client-reopen.mjs',
    'cargo test --locked --workspace', 'cargo build --locked --workspace --bins']) assert.ok(calls.includes(gate));
});
