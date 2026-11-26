// Operator/runner gate. The worker owns the build-only script and Rust APK;
// acceptance reads Android's composited pixels, never strings from the app.
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { requireKitEmulator } from './run.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const board = JSON.parse(readFileSync(new URL('./fixtures/android-kit-board.json', import.meta.url)));
const pkg = 'dev.boardcraft.kitfixture';

export function matchBoard(lines, editing) {
  const expected = [board.title, board.items[0].title, ...(editing ? ['Editing name'] : []), board.items[1].title];
  if (!Array.isArray(lines) || (!editing && lines.some(l => l.text === 'Editing name'))) return;
  let lastY = -1;
  for (const text of expected) {
    const hits = lines.filter(l => l.text === text);
    if (hits.length !== 1) return;
    const [{ x, y }] = hits;
    if (![x, y].every(Number.isSafeInteger) || x <= 0 || y <= lastY) return;
    lastY = y;
  }
  return lines.find(l => l.text === board.items[1].title);
}

function run(bin, args, { timeout = 30_000, binary = false, inherit = false } = {}) {
  const r = spawnSync(bin, args, { cwd: root, encoding: binary ? null : 'utf8', timeout,
    maxBuffer: 16 * 1024 * 1024, stdio: inherit ? 'inherit' : ['ignore', 'pipe', 'pipe'] });
  if (r.error) throw r.error;
  if (r.status !== 0) throw Error(`${bin} failed (${r.status}): ${r.stderr ?? ''}`);
  return r.stdout;
}

async function gate() {
  requireKitEmulator();
  const serial = process.env.RUSTDAY_ANDROID_SERIAL;
  const adb = (...args) => run('adb', ['-s', serial, ...args]);
  const work = mkdtempSync(join(tmpdir(), 'boardcraft-frame-'));
  const apk = join(work, 'fixture.apk'), png = join(work, 'frame.png');
  const ps = ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File'];
  const installed = () => adb('shell', 'pm', 'list', 'packages', '-u', pkg).trim().split(/\s+/).includes(`package:${pkg}`);
  const foreground = () => adb('shell', 'dumpsys', 'activity', 'activities').split('\n').some(l =>
    l.includes('topResumedActivity=ActivityRecord{') && l.includes(` ${pkg}/`));
  async function frame(editing) {
    const deadline = Date.now() + 45_000;
    do {
      if (foreground()) {
        writeFileSync(png, run('adb', ['-s', serial, 'exec-out', 'screencap', '-p'], { binary: true }));
        const hit = matchBoard(JSON.parse(run('powershell', [...ps, 'scripts/ocr-frame.ps1', png])), editing);
        if (hit && foreground()) return hit;
      }
      await delay(350);
    } while (Date.now() < deadline);
    throw Error(`No matching foreground ${editing ? 'editor' : 'board'} frame`);
  }
  let own = false;
  try {
    run('powershell', [...ps, 'android/build-debug.ps1', '-OutputApk', apk], { inherit: true, timeout: 60 * 60_000 });
    const badging = run(join(process.env.ANDROID_HOME, 'build-tools', '35.0.0', 'aapt.exe'), ['dump', 'badging', apk]);
    if (!badging.startsWith(`package: name='${pkg}' `) || !/^application-debuggable\s*$/m.test(badging) ||
      !/^native-code: 'x86_64'\s*$/m.test(badging)) throw Error('Expected fixture x86_64 debug APK');
    const activity = /^launchable-activity: name='([a-zA-Z0-9_.]+)'/m.exec(badging)?.[1];
    if (!activity) throw Error('Expected launchable fixture activity');
    requireKitEmulator();
    if (installed()) throw Error('Fixture package already present; refusing replacement');
    own = true; // absent on this exclusive AVD; also clean up a timed-out install
    if (!/^Success\s*$/m.test(adb('install', '--no-incremental', apk))) throw Error('Install failed');
    adb('shell', 'am', 'start', '-W', '-n', `${pkg}/${activity}`);
    const { x, y } = await frame(false);
    adb('shell', 'input', 'tap', String(x), String(y));
    await frame(true);
  } finally {
    try {
      if (own) {
        requireKitEmulator();
        if (installed() && !/^Success\s*$/m.test(adb('uninstall', pkg))) throw Error('Uninstall failed');
        if (installed()) throw Error('Fixture still installed');
      }
    } finally { rmSync(work, { recursive: true, force: true }); }
  }
  console.log('Android Kit: ordered board frame, touch on second row, inline editor frame');
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) await gate();
