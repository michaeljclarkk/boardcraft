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

test('13 requires both account journeys against the disposable database', () => {
  const packet = packets.find(p => p.id === '13-account-session');
  assert.equal(packet.database, true);
  assert.equal(packet.model, 'openai/gpt-6.2-astra#high');
  const cases = ['rejected_credentials_then_sign_in', 'resolve_retry_and_revoke'];
  const calls = [];
  const run = names => verify(packet, (bin, args) => {
    calls.push([bin, ...args].join(' '));
    return [...names.map(n => `test ${n} ... ok`), `test result: ok. ${names.length} passed; 0 failed; 0 ignored`].join('\n');
  });
  for (const one of cases) assert.throws(() => run([one]), /Missing passing account_session case/);
  calls.length = 0;
  run(cases);
  assert.deepEqual(calls, ['cargo fmt --all -- --check', 'cargo test --locked --workspace --test account_session', 'cargo test --locked --workspace']);
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

test('16 requires session isolation before its graphical logout gate', () => {
  const packet = packets.find(p => p.id === '16-android-kit-logout');
  const run = name => verify(packet, (_bin, args) => {
    if (args.includes('loopback_board_access')) return `test ${name} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored`;
    if (args.includes('--logout')) throw Error('logout UI gate reached');
  });
  assert.throws(() => run('viewer_reads_but_cannot_rename'), /Missing passing/);
  assert.throws(() => run('explicit_logout_isolates_sessions'), /logout UI gate reached/);
});

test('17 cannot pass on an ordinary cold sign-in without the session restoration journey', () => {
  const p = packets.find(p => p.id === '17-android-session-restore');
  assert.ok(p.probeRegression && p.logout && p.delivery && p.kitEdit && p.kitFrame);
  assert.throws(() => verify(p, (_bin,args) => {
    if (args.includes('scripts/cross-client-reopen.mjs') && args.includes('--restore')) throw Error('restore verification failed');
  }), /restore verification failed/);
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

test('15 stops before delivery if either real authenticated probe case is missing', t => {
  const p = packets.find(p => p.id === '15-android-kit-sign-in');
  const old = process.env.RUSTDAY_ANDROID_SERIAL;
  process.env.RUSTDAY_ANDROID_SERIAL = 'emulator-5682';
  t.after(() => { if (old === undefined) delete process.env.RUSTDAY_ANDROID_SERIAL; else process.env.RUSTDAY_ANDROID_SERIAL = old; });
  const seen = [];
  const adb = ['Rustday_Edit_X64\nOK', '1', 'x86_64'];
  assert.throws(() => verify(p, (bin, args) => {
    seen.push(...args);
    if (bin === 'adb') return adb.shift();
    if (args.includes('android_emulator_edit')) return 'test emulator_rename_then_reopen ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored';
  }), /Missing passing/);
  assert.ok(!seen.includes('scripts/cross-client-reopen.mjs'));
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

for (const [id, previous, added] of [
  ['11-windows-failure-reopen', '10-windows-name-recovery', 'failed_name_reopens_only_for_actor_and_board'],
  ['12-windows-name-draft', '11-windows-failure-reopen', 'unsubmitted_name_reopens_with_entry_baseline'],
]) test(`${id} requires its fresh-process case and previous Windows journeys without a device gate`, () => {
  const packet = packets.find(p => p.id === id);
  const required = [...packets.find(p => p.id === previous).windowsCases, added];
  const run = names => verify(packet, (bin, args) => {
    assert.ok(!args.some(arg => /android|cross-client/.test(arg)));
    if (args.includes('windows_smoke')) return [...names.map(n => `test ${n} ... ok`),
      `test result: ok. ${names.length} passed; 0 failed; 0 ignored`].join('\n');
  });
  for (const name of required) assert.throws(() => run(required.filter(n => n !== name)), /Missing passing windows_smoke case/);
  run(required);
});

for (const [id, addedHost, addedWindows] of [
  ['20-windows-connected-name', [], []],
  ['21-windows-connected-logout', ['explicit_logout_isolates_sessions'], ['configured_logout_failure_then_retry', 'interactive_viewer_logs_out']],
  ['22-visible-board-list', ['explicit_logout_isolates_sessions', 'list_visible_boards_in_navigation_order', 'workspace_membership_changes_refresh_list_and_access'], ['configured_logout_failure_then_retry', 'interactive_viewer_logs_out']],
  ['24-android-board-navigation', ['explicit_logout_isolates_sessions', 'list_visible_boards_in_navigation_order', 'workspace_membership_changes_refresh_list_and_access', 'selected_board_read_checks_current_membership'], ['configured_logout_failure_then_retry', 'interactive_viewer_logs_out', 'opens_selected_board_by_id', 'revoked_membership_blocks_board_selection']],
  ['23-windows-board-navigation', ['explicit_logout_isolates_sessions', 'list_visible_boards_in_navigation_order', 'workspace_membership_changes_refresh_list_and_access', 'selected_board_read_checks_current_membership'], ['configured_logout_failure_then_retry', 'interactive_viewer_logs_out', 'opens_selected_board_by_id', 'revoked_membership_blocks_board_selection']],
  ['25-windows-board-link', ['explicit_logout_isolates_sessions', 'list_visible_boards_in_navigation_order', 'workspace_membership_changes_refresh_list_and_access', 'selected_board_read_checks_current_membership'], ['configured_logout_failure_then_retry', 'interactive_viewer_logs_out', 'opens_selected_board_by_id', 'revoked_membership_blocks_board_selection']],
]) test(`${id} requires every named host and Windows journey and the locked workspace checks`, () => {
  const p = packets.find(p => p.id === id);
  assert.equal(p.database, true);
  const host = ['member_edits_within_scope', 'viewer_reads_but_cannot_rename', ...addedHost];
  const windows = [...packets.find(p => p.id === '19-windows-env-session').windowsCases,
    'connected_member_renames_then_reopens', 'connected_viewer_cannot_rename', ...addedWindows];
  const calls = [];
  const run = (absent, ignored = 0, fail) => verify(p, (bin, args, capture) => {
    calls.push([bin, ...args].join(' '));
    if (calls.at(-1) === fail) throw Error('missing device journey');
    if (args.includes('--exact')) return `test ${args[args.indexOf('--')-1]} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored`;
    const names = args.includes('windows_smoke') ? windows : args.includes('loopback_board_access') ? host : undefined;
    if (!names) return;
    assert.ok(capture);
    const present = names.filter(n => n !== absent);
    return [...present.map(n => `test ${n} ... ok`), `test result: ok. ${present.length} passed; 0 failed; ${ignored} ignored`].join('\n');
  });
  for (const name of [...host, ...windows]) assert.throws(() => run(name), /Missing passing/);
  assert.throws(() => run(undefined, 1), /No passing/);
  calls.length = 0;
  run();
  assert.deepEqual(calls, ['cargo fmt --all -- --check', ...(id === '25-windows-board-link' ? ['cargo test --locked -p boardcraft-desktop --bin boardcraft-desktop connected::board_link::tests::only_a_canonical_board_at_the_connected_origin -- --exact'] : []), 'node --test scripts/run.test.mjs',
    'cargo test --locked --workspace --test loopback_board_access', 'cargo test --locked --workspace',
    'cargo test --locked -p boardcraft-desktop --features native-smoke --test windows_smoke -- --test-threads=1',
    ...(id === '24-android-board-navigation' ? ['cargo fmt --manifest-path android/Cargo.toml -- --check', 'node scripts/android-kit-frame.mjs', 'node scripts/android-kit-edit.mjs', 'node scripts/android-kit-edit.mjs --browse'] : [])]);
  if (id === '24-android-board-navigation') {
    assert.equal(p.kitBrowse, true);
    for (const cmd of calls.slice(-3)) assert.throws(() => run(undefined, 0, cmd), /missing device journey/);
  }
});

test('26 adds two enabled browser journeys to the unattended packet 25 gate', () => {
  const p = packets.find(p => p.id === '26-browser-board-preview');
  const names = ['browser_reject_correct_list_logout', 'browser_empty_unavailable_then_retry'];
  const previous = packets.find(p => p.id === '25-windows-board-link');
  const { browserCases, id, ...inherited } = p;
  assert.deepEqual({ ...inherited, id: previous.id }, previous);
  assert.deepEqual(browserCases, names);
  const run = (present, ignored = 0) => verify(p, (bin, args, capture) => {
    if (args.includes('browser_preview')) {
      assert.equal(bin, 'cargo');
      assert.deepEqual(args, ['test','--locked','-p','boardcraft-mobile-edit-host','--features','browser-smoke',
        '--test','browser_preview','--','--test-threads=1']);
      assert.equal(capture, true);
      return [...present.map(n => `test ${n} ... ok`),
        `test result: ok. ${present.length} passed; 0 failed; ${ignored} ignored`].join('\n');
    }
    if (args.includes('--exact')) throw Error('reached inherited board link gate');
  });
  for (const n of names) assert.throws(() => run(names.filter(other => n !== other)), /Missing passing browser_preview case/);
  assert.throws(() => run([], 2), /No passing, enabled browser_preview suite/);
  assert.throws(() => run(names, 1), /No passing, enabled browser_preview suite/);
  assert.throws(() => run(names), /reached inherited board link gate/);
});

test('27 requires both browser selection outcomes, previous browser cases and scoped access without foreground windows', () => {
  const p = packets.find(p => p.id === '27-browser-board-navigation');
  const prev = packets.find(p => p.id === '26-browser-board-preview');
  const names = [...prev.browserCases, 'browser_opens_selected_board_by_id', 'browser_revoked_membership_blocks_selection'];
  assert.equal(p.database, true);
  assert.deepEqual(p.cases, prev.cases);
  const output = (names, ignored) => [...names.map(n => `test ${n} ... ok`),
    `test result: ok. ${names.length} passed; 0 failed; ${ignored} ignored`].join('\n');
  const calls = [];
  const run = (present, ignored = 0) => verify(p, (bin, args, capture) => {
    calls.push([bin, ...args].join(' '));
    if (args.includes('browser_preview') || args.includes('loopback_board_access')) {
      assert.equal(capture, true);
      return args.includes('browser_preview') ? output(present, ignored) : output(prev.cases, 0);
    }
  });
  for (const n of names) assert.throws(() => run(names.filter(x => x !== n)), /Missing passing browser_preview case/);
  assert.throws(() => run(names, 1), /No passing, enabled browser_preview suite/);
  calls.length = 0;
  run(names);
  assert.deepEqual(calls, ['cargo fmt --all -- --check',
    'cargo test --locked -p boardcraft-mobile-edit-host --features browser-smoke --test browser_preview -- --test-threads=1',
    'node --test scripts/run.test.mjs', 'cargo test --locked --workspace --test loopback_board_access', 'cargo test --locked --workspace']);
});

test('28 adds real browser traversal, signed-out direct entry and a scoped document/API boundary', () => {
  const previous = packets.find(p => p.id === '27-browser-board-navigation');
  const packet = packets.find(p => p.id === '28-browser-board-history');
  const browser = [...previous.browserCases, 'browser_history_rechecks_board_access', 'browser_direct_board_survives_sign_in'];
  const host = [...previous.cases, 'preview_document_preserves_scoped_api'];
  assert.deepEqual(packet, { ...previous, id: '28-browser-board-history', cases: host, browserCases: browser });
  const output = names => [...names.map(n => `test ${n} ... ok`),
    `test result: ok. ${names.length} passed; 0 failed; 0 ignored`].join('\n');
  const run = missing => verify(packet, (_bin, args) => {
    if (args.includes('browser_preview')) return output(browser.filter(n => n !== missing));
    if (args.includes('loopback_board_access')) return output(host.filter(n => n !== missing));
  });
  run();
  for (const n of [...browser, ...host]) assert.throws(() => run(n), /Missing passing/);
});

test('29 requires selected rename recovery, stale Viewer denial and current catalog write access', () => {
  const previous = packets.find(p => p.id === '28-browser-board-history');
  const packet = packets.find(p => p.id === '29-browser-selected-name');
  const browser = [...previous.browserCases, 'browser_member_recovers_selected_name', 'browser_demoted_viewer_cannot_rename'];
  const host = [...previous.cases, 'preview_selected_rename_requires_current_access'];
  assert.deepEqual(packet, { ...previous, id: '29-browser-selected-name', cases: host, browserCases: browser });
  const run = missing => verify(packet, (_bin, args) => {
    const names = args.includes('browser_preview') ? browser : args.includes('loopback_board_access') ? host : [];
    const present = names.filter(n => n !== missing);
    return [...present.map(n => `test ${n} ... ok`), `test result: ok. ${present.length} passed; 0 failed; 0 ignored`].join('\n');
  });
  run();
  for (const n of [...browser, ...host]) assert.throws(() => run(n), /Missing passing/);
});

test('30 requires same-origin tab resume, distinct failures and an isolated browser session transport', () => {
  const previous = packets.find(p => p.id === '29-browser-selected-name');
  const packet = packets.find(p => p.id === '30-browser-session-resume');
  const browser = [...previous.browserCases, 'browser_reload_and_independent_tab_resume', 'browser_unavailable_then_rejected_resume'];
  const host = [...previous.cases, 'preview_browser_session_is_origin_bound'];
  assert.deepEqual(packet, { ...previous, id: '30-browser-session-resume', cases: host, browserCases: browser });
  const run = missing => verify(packet, (_bin, args) => {
    const names = args.includes('browser_preview') ? browser : args.includes('loopback_board_access') ? host : [];
    const present = names.filter(n => n !== missing);
    return [...present.map(n => `test ${n} ... ok`), `test result: ok. ${present.length} passed; 0 failed; 0 ignored`].join('\n');
  });
  run();
  for (const n of [...browser, ...host]) assert.throws(() => run(n), /Missing passing/);
});

test('31 requires the physical group grid, identity-preserving edit, revoked access and migrated rows', () => {
  const previous = packets.find(p => p.id === '30-browser-session-resume');
  const packet = packets.find(p => p.id === '31-browser-board-groups');
  const browser = [...previous.browserCases, 'browser_opens_and_edits_grouped_board', 'browser_grouped_board_rechecks_membership'];
  const host = [...previous.cases, 'preview_groups_require_active_membership', 'group_migration_preserves_flat_boards'];
  assert.deepEqual(packet, { ...previous, id: '31-browser-board-groups', cases: host, browserCases: browser });
  const run = missing => verify(packet, (_bin, args) => {
    const names = args.includes('browser_preview') ? browser : args.includes('loopback_board_access') ? host : [];
    const present = names.filter(n => n !== missing);
    return [...present.map(n => `test ${n} ... ok`), `test result: ok. ${present.length} passed; 0 failed; 0 ignored`].join('\n');
  });
  run();
  for (const n of [...browser, ...host]) assert.throws(() => run(n), /Missing passing/);
});

test('32 requires grouped creation, current access, stable replay and atomic failure', () => {
  const previous = packets.find(p => p.id === '31-browser-board-groups');
  const packet = packets.find(p => p.id === '32-browser-group-item-create');
  const browser = [...previous.browserCases, 'browser_creates_item_in_selected_group', 'browser_demoted_viewer_cannot_create_item'];
  const host = [...previous.cases, 'preview_create_requires_current_group_access', 'preview_create_is_atomic_and_replay_safe'];
  assert.deepEqual(packet, { ...previous, id: '32-browser-group-item-create', cases: host, browserCases: browser });
  const run = missing => verify(packet, (_bin, args) => {
    const names = args.includes('browser_preview') ? browser : args.includes('loopback_board_access') ? host : [];
    const present = names.filter(n => n !== missing);
    return [...present.map(n => `test ${n} ... ok`), `test result: ok. ${present.length} passed; 0 failed; 0 ignored`].join('\n');
  });
  run();
  for (const n of [...browser, ...host]) assert.throws(() => run(n), /Missing passing/);
});
