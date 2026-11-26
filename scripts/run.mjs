import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { resolve } from 'node:path';

export const packets = [
  { id: '01-board', target: 'board_edit', database: false },
  { id: '02-postgres', target: 'postgres_board', database: true },
  { id: '03-desktop', target: 'desktop_edit', database: true, buildBins: true },
  { id: '04-mobile-edit-service', target: 'mobile_edit_service', database: true,
    cases: ['rename_then_reconnect', 'reject_then_retry'] },
  { id: '05-android-emulator-edit', target: 'android_emulator_edit', database: true, android: true,
    cases: ['emulator_rename_then_reopen', 'emulator_reject_then_correct'] },
  { id: '06-android-kit-frame', database: true, kitFrame: true },
  { id: '07-android-kit-edit', database: true, kitFrame: true, kitEdit: true },
  { id: '08-cross-client-reopen', database: true, kitFrame: true, kitEdit: true, delivery: true },
  { id: '09-windows-name-cell', database: true, kitFrame: true, kitEdit: true, delivery: true,
    windowsCases: ['rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter'] },
  { id: '10-windows-name-recovery', database: true, buildBins: true, kitFrame: true, kitEdit: true, delivery: true,
    windowsCases: ['rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter', 'parent_failure_retry_dismiss_and_commit'] },
  { id: '11-windows-failure-reopen', database: true, model: 'openai/gpt-6.2-astra#high',
    windowsCases: ['rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter', 'parent_failure_retry_dismiss_and_commit', 'failed_name_reopens_only_for_actor_and_board'] },
  { id: '12-windows-name-draft', database: true, model: 'openai/gpt-6.2-astra#high',
    windowsCases: ['rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter', 'parent_failure_retry_dismiss_and_commit', 'failed_name_reopens_only_for_actor_and_board', 'unsubmitted_name_reopens_with_entry_baseline'] },
  { id: '13-account-session', target: 'account_session', database: true, model: 'openai/gpt-6.2-astra#high',
    cases: ['rejected_credentials_then_sign_in', 'resolve_retry_and_revoke'] },
  { id: '14-loopback-board-access', target: 'loopback_board_access', database: true, model: 'openai/gpt-6.2-astra#high',
    cases: ['viewer_reads_but_cannot_rename', 'member_edits_within_scope'] },
];
const root = fileURLToPath(new URL('../', import.meta.url));
const read = path => JSON.parse(readFileSync(path, 'utf8'));
const text = value => typeof value === 'string' && value.trim().length > 0;

function command(bin, args, capture = false, timeout = 2 * 60 * 60 * 1000) {
  const result = spawnSync(bin, args, { stdio: capture ? ['ignore', 'pipe', 'inherit'] : ['ignore', 'inherit', 'inherit'], encoding: 'utf8', timeout });
  if (result.error) throw result.error;
  if (capture) process.stdout.write(result.stdout ?? '');
  if (result.status !== 0) throw Error(`${bin} stopped (${result.signal ?? result.status})`);
  return result.stdout;
}

// Only probe a selected, already-booted disposable AVD. Never enumerate or select a device by default.
export function requireEditEmulator(exec = command, serial = process.env.RUSTDAY_ANDROID_SERIAL) {
  if (!/^emulator-[0-9]+$/.test(serial ?? '')) throw Error('Set RUSTDAY_ANDROID_SERIAL to the dedicated emulator serial');
  const adb = (...args) => exec('adb', ['-s', serial, ...args], true, 10_000).trim();
  if (adb('emu', 'avd', 'name').split('\n')[0].trim() !== 'Rustday_Edit_X64') throw Error('Expected Rustday_Edit_X64 AVD');
  if (adb('shell', 'getprop', 'sys.boot_completed') !== '1') throw Error('Emulator has not booted');
  if (adb('shell', 'getprop', 'ro.product.cpu.abi') !== 'x86_64') throw Error('Expected x86_64 Android');
}

export function requireKitEmulator(exec = command, env = process.env) {
  const port = env.ANDROID_ADB_SERVER_PORT;
  if (!/^[1-9]\d{0,4}$/.test(port ?? '') || Number(port) > 65535 || port === '5037' ||
      env.ADB_SERVER_SOCKET !== `tcp:${port}` || env.ADB_USB !== '0' || env.ADB_EMU !== '0') {
    throw Error('Expected private loopback adb server with discovery disabled');
  }
  requireEditEmulator(exec, env.RUSTDAY_ANDROID_SERIAL ?? '');
}

export function verify({ target, buildBins, cases = [], android, kitFrame, kitEdit, delivery, windowsCases }, exec = command) {
  exec('cargo', ['fmt', '--all', '--', '--check']);
  const windows = () => {
    const args = ['test', '--locked', '-p', 'boardcraft-desktop', '--features', 'native-smoke', '--test', 'windows_smoke'];
    if (windowsCases) requireCases('windows_smoke', windowsCases, exec('cargo', [...args, '--', '--test-threads=1'], true));
    else exec('cargo', args);
  };
  if (windowsCases && !kitFrame) {
    exec('node', ['--test', 'scripts/run.test.mjs']);
    exec('cargo', ['test', '--locked', '-p', 'boardcraft-desktop']);
    windows();
    return;
  }
  if (kitFrame) {
    exec('cargo', ['fmt', '--manifest-path', 'android/Cargo.toml', '--', '--check']);
    exec('node', ['scripts/android-kit-frame.mjs']);
    if (kitEdit) {
      exec('node', ['--test', 'scripts/run.test.mjs']);
      // The delivery gate drives every 07 step before continuing on the same fixture.
      exec('node', [delivery ? 'scripts/cross-client-reopen.mjs' : 'scripts/android-kit-edit.mjs']);
      if (delivery) windows();
    }
    exec('cargo', ['test', '--locked', '--workspace']);
    if (buildBins) exec('cargo', ['build', '--locked', '--workspace', '--bins']);
    return;
  }
  if (android) {
    requireEditEmulator(exec);
    exec('cargo', ['build', '--locked', '-p', 'boardcraft-mobile-edit-client', '--bin', 'android-edit-probe', '--target', 'x86_64-linux-android']);
  }
  const output = android
    ? exec('cargo', ['test', '--locked', '-p', 'boardcraft-mobile-edit-host', '--features', 'android-smoke', '--test', target, '--', '--test-threads=1'], true, 10 * 60_000)
    : exec('cargo', ['test', '--locked', '--workspace', '--test', target], true);
  requireCases(target, cases, output);
  exec('cargo', ['test', '--locked', '--workspace']);
  if (buildBins) exec('cargo', ['build', '--locked', '--workspace', '--bins']);
}

function requireCases(target, cases, output) {
  if (!/test result: ok\. [1-9]\d* passed; 0 failed; 0 ignored/.test(output)) throw Error(`No passing, enabled ${target} suite`);
  const lines = output.split(/\r?\n/);
  for (const name of cases) {
    if (!lines.includes(`test ${name} ... ok`)) throw Error(`Missing passing ${target} case: ${name}`);
  }
}

export function validLimit(count) {
  return /^[1-9]\d*$/.test(count) && Number(count) <= packets.length;
}

function stateDirectory() {
  if (!existsSync('.rustday')) return '.boardcraft';
  if (existsSync('.boardcraft')) throw Error('Both .rustday and .boardcraft runner states exist; resolve the conflict before continuing.');
  return '.rustday';
}

export function runQueue(limit, work, check) {
  if (!validLimit(limit)) throw Error(`Expected 1-${packets.length} packets`);
  const state = stateDirectory();
  mkdirSync(state, { recursive: true });
  mkdirSync('handoffs', { recursive: true });
  // Exclusive create: a second runner must never dispatch the same packet.
  writeFileSync(`${state}/lock`, `${process.pid}\n`, { flag: 'wx' });
  try {
    for (const packet of packets) {
      const { id } = packet;
      if (existsSync(`${state}/${id}.ok`)) continue;
      if (limit-- === 0) break;
      const path = `handoffs/${id}.json`;
      const prior = existsSync(path) ? readFileSync(path, 'utf8') : null;
      if (prior !== null) { writeFileSync(`${state}/${id}.previous.json`, prior); rmSync(path); }
      work(packet, state);
      const handoff = read(path);
      if (handoff.status !== 'ready') throw Error(`${id}: ${handoff.summary || 'incomplete'}`);
      if (!text(handoff.summary) || !text(handoff.next) || !Array.isArray(handoff.evidence) || !handoff.evidence.length || !handoff.evidence.every(text)) throw Error(`Incomplete ${path}`);
      check(packet);
      writeFileSync(`${state}/${id}.ok`, new Date().toISOString(), { flag: 'wx' });
      console.log(`Verified ${id}`);
    }
  } finally { rmSync(`${state}/lock`); }
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  process.chdir(root);
  const [mode = 'status', count, ...extra] = process.argv.slice(2);
  try {
    if (mode === 'status' && count === undefined) {
      const state = stateDirectory();
      for (const { id } of packets) console.log(id, existsSync(`${state}/${id}.ok`) ? 'verified' : 'pending');
    } else if (mode === 'run' && validLimit(count) && !extra.length) {
      const cli = process.env.OPENCODE_BIN || 'opencode';
      command('cargo', ['--version']);
      command(cli, ['--version']);
      runQueue(Number(count), ({ id, database, android, kitFrame, model }, state) => {
        if (database && !process.env.RUSTDAY_TEST_DATABASE_URL) throw Error('Set RUSTDAY_TEST_DATABASE_URL to the disposable boardcraft DB.');
        if (android) requireEditEmulator();
        if (kitFrame) requireKitEmulator();
        command(cli, ['run', '--standalone', '--agent', 'build', ...(model ? ['--model', model] : []), '--title', `boardcraft ${id}`, `Complete only ${id} from ROADMAP.md. Read AGENTS.md, prior handoffs and any ${state}/${id}.previous.json. Inspect existing edits before continuing. Write handoffs/${id}.json and stop.`]);
      }, verify);
    } else throw Error(`Usage: node scripts/run.mjs status | run <1-${packets.length}>`);
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
